"""Train a masked arena policy with behavior cloning followed by clipped PPO."""

import argparse
from collections import Counter, deque
import json
from pathlib import Path
import time

import numpy as np
import torch
from torch import nn
from torch.nn import functional as F

from arena.policy import CHECKPOINT_VERSION, DEFAULT_CHECKPOINT, Policy

TRAIN_SEED = 0x5452410000000000
DEV_SEED = 0x4445560000000000
HIDDEN = 128
GAMMA = 0.995


class ActorCritic(nn.Module):
    def __init__(self, features, actions, hidden=HIDDEN):
        super().__init__()
        self.first = nn.Linear(features, hidden)
        self.second = nn.Linear(hidden, hidden)
        self.policy = nn.Linear(hidden, actions)
        self.value = nn.Linear(hidden, 1)
        for layer, gain in ((self.first, 1.4), (self.second, 1.4),
                            (self.policy, 0.01), (self.value, 1.0)):
            nn.init.orthogonal_(layer.weight, gain=gain)
            nn.init.zeros_(layer.bias)

    def forward(self, features, mask):
        hidden = torch.tanh(self.first(features))
        hidden = torch.tanh(self.second(hidden))
        logits = self.policy(hidden).masked_fill(~mask, -1e9)
        return logits, self.value(hidden).squeeze(-1)

    def export(self, path, metadata):
        weights = {}
        for layer, key in ((self.first, "1"), (self.second, "2"),
                           (self.policy, "_policy"), (self.value, "_value")):
            weights["w" + key] = layer.weight.detach().numpy().T
            weights["b" + key] = layer.bias.detach().numpy()
        path.parent.mkdir(parents=True, exist_ok=True)
        temp = path.with_suffix(".tmp.npz")
        np.savez_compressed(temp, metadata=json.dumps(metadata, sort_keys=True), **weights)
        temp.replace(path)

    @classmethod
    def from_checkpoint(cls, path):
        policy = Policy.load(path)
        first, second = policy.weights["w1"], policy.weights["w2"]
        if first.shape[1] != second.shape[1]:
            raise ValueError("ActorCritic requires equally wide hidden layers")
        model = cls(policy.features, policy.actions, hidden=first.shape[1])
        model.load(path)
        return model

    def load(self, path):
        with np.load(path, allow_pickle=False) as archive, torch.no_grad():
            for layer, key in ((self.first, "1"), (self.second, "2"),
                               (self.policy, "_policy"), (self.value, "_value")):
                layer.weight.copy_(torch.from_numpy(archive["w" + key].T))
                layer.bias.copy_(torch.from_numpy(archive["b" + key]))


class Rollout:
    def __init__(self, steps, batch, features, actions):
        shape = (steps, batch)
        self.features = torch.empty((*shape, features))
        self.mask = torch.empty((*shape, actions), dtype=torch.bool)
        self.actions = torch.empty(shape, dtype=torch.int64)
        self.expert = torch.empty(shape, dtype=torch.int64)
        self.log_prob = torch.empty(shape)
        self.values = torch.empty(shape)
        self.rewards = torch.empty(shape)
        self.done = torch.empty(shape)
        self.advantages = torch.empty(shape)

    def returns(self, next_value, gamma, gae_lambda):
        advantage = torch.zeros_like(next_value)
        for step in reversed(range(self.rewards.shape[0])):
            uninterrupted = 1 - self.done[step]
            delta = self.rewards[step] + gamma * next_value * uninterrupted - self.values[step]
            advantage = delta + gamma * gae_lambda * uninterrupted * advantage
            self.advantages[step] = advantage
            next_value = self.values[step]
        return self.advantages + self.values


def sample_actions(logits):
    return torch.multinomial(logits.softmax(dim=-1), 1).squeeze(1)


class TurnMovement:
    """Tracks traversed tiles for the learner's current arena turn."""

    def __init__(self, batch):
        self.tiles = np.zeros(batch, dtype=np.uint64)
        self.turn = np.full(batch, -1, dtype=np.int32)
        self.moves = 0
        self.revisits = 0

    @staticmethod
    def _tile(features):
        x = np.rint(features[:, 36] * 8).astype(np.uint64)
        y = np.rint(features[:, 37] * 6).astype(np.uint64)
        return np.left_shift(np.uint64(1), y * 9 + x)

    def update(self, before, after, actions, done):
        turn = np.rint(before[:, 30] * 60).astype(np.int32)
        self.tiles[self.turn != turn] = 0
        self.turn[:] = turn
        origin = self._tile(before)
        destination = self._tile(after)
        self.tiles |= origin
        # Movement can trigger an opponent reaction before the next learner
        # decision. A completed episode has already reset its arena lane.
        same_turn = (before[:, 30] == after[:, 30]) & ~done
        main_phase = before[:, 136] == 1
        moves = (actions < 8) & main_phase & same_turn & (origin != destination)
        repeat = moves & ((self.tiles & destination) != 0)
        self.tiles[moves] |= destination[moves]
        self.turn[done] = -1
        self.moves += int(moves.sum())
        self.revisits += int(repeat.sum())
        return moves, repeat


def lethal_without_rider(features, mask):
    # The visible pending damage has already accounted for Rage. Monk and
    # Rogue may have a pending damage reduction whose roll is still unknown.
    enemy_hp = np.rint(features[:, 25] * features[:, 59] * 60)
    enemy_temp = np.rint(features[:, 60] * 20)
    damage = np.rint(features[:, 151] * 40)
    no_pending_reduction = (features[:, 17] == 0) & (features[:, 20] == 0)
    return mask[:, 17] & no_pending_reduction & (damage >= enemy_hp + enemy_temp)


def target_supervision(features, action_mask, expert):
    # ABI 1003 distinguishes live target decisions using the marker and
    # a nonempty mask. Damage spells can have multiple equivalent aim cells.
    targeting = (features[:, 140] == 1) & (features[:, 224:233] > 0).any(axis=1)
    rows = np.flatnonzero(targeting)
    allowed = np.zeros(action_mask.shape, dtype=bool)
    if not len(rows):
        return targeting, allowed
    candidates = np.concatenate((
        features[rows, 160:224].reshape(-1, 8, 8),
        features[rows, 146:154, None].transpose(0, 2, 1),
    ), axis=1)
    chosen = candidates[np.arange(len(rows)), expert[rows]]
    equivalent = (candidates[:, :, 2:6] == chosen[:, None, 2:6]).all(axis=2)
    # A column that cannot affect the enemy immediately can still be better
    # positioned for an ongoing area. Traps use the expert route choice.
    same_distance = candidates[:, :, 6] == chosen[:, None, 6]
    equivalent &= same_distance | (chosen[:, None, 2] == 1)
    equivalent &= features[rows, 154, None] != 4
    equivalent &= action_mask[rows, :9]
    allowed[rows, :9] = equivalent
    allowed[rows, expert[rows]] = True
    return targeting, allowed


@torch.no_grad()
def collect(model, arena, rollout, states, clone_probability, recent, config, movement):
    for step in range(rollout.rewards.shape[0]):
        features, mask, expert = states
        rollout.features[step] = torch.from_numpy(features)
        rollout.mask[step] = torch.from_numpy(mask)
        rollout.expert[step] = torch.from_numpy(expert)
        logits, values = model(rollout.features[step], rollout.mask[step])
        actions = sample_actions(logits)
        if clone_probability:
            actions = torch.where(torch.rand(len(actions)) < clone_probability,
                                  rollout.expert[step], actions)
        rollout.actions[step] = actions
        rollout.log_prob[step] = logits.log_softmax(dim=-1).gather(1, actions[:, None]).squeeze(1)
        rollout.values[step] = values
        action_ids = actions.numpy().astype(np.int32)
        states, rewards, done, scores = arena.step(action_ids)
        if config.hp_shaping:
            # A terminal state's future potential is zero, including when the
            # lane has already recycled to its next fight.
            previous = rollout.features[step, :, 24] - rollout.features[step, :, 25]
            following = states[0][:, 24] - states[0][:, 25]
            rewards += config.hp_shaping * (GAMMA * np.where(done, 0, following) - previous.numpy())
        moves, revisits = movement.update(rollout.features[step].numpy(), states[0], action_ids, done)
        rewards -= config.move_cost * moves + config.revisit_cost * revisits
        if config.full_hp_wind_cost:
            before = rollout.features[step].numpy()
            # Fighter's Special1 is Second Wind. Level 5 can legitimately
            # spend a use at full HP for the Tactical Shift movement.
            needless_wind = (action_ids == 18) & (before[:, 4] == 1) & (
                before[:, 24] >= 1) & (before[:, 32] < 1)
            rewards -= config.full_hp_wind_cost * needless_wind
        if config.lethal_rider_cost:
            lethal = lethal_without_rider(features, mask)
            rewards -= config.lethal_rider_cost * (lethal & (action_ids == 17))
        rollout.rewards[step] = torch.from_numpy(rewards)
        rollout.done[step] = torch.from_numpy(done)
        recent.extend(scores[done])
    features, mask, _ = states
    _, next_value = model(torch.from_numpy(features), torch.from_numpy(mask))
    return states, next_value


def update(model, optimizer, rollout, returns, config, cloning, imitation_weight):
    features = rollout.features.flatten(0, 1)
    masks = rollout.mask.flatten(0, 1)
    actions = rollout.actions.flatten()
    expert = rollout.expert.flatten()
    old_log_prob = rollout.log_prob.flatten()
    advantages = rollout.advantages.flatten()
    advantages = (advantages - advantages.mean()) / advantages.std().clamp_min(1e-6)
    returns = returns.flatten()
    safe_pass = other_rider = None
    if config.lethal_rider_supervision or config.other_rider_supervision:
        safe_pass = torch.from_numpy(lethal_without_rider(features.numpy(), masks.numpy()))
        other_rider = masks[:, 17] & ~safe_pass
        expert = torch.where(safe_pass, 15, expert)
    target_rows = target_choices = None
    if config.target_supervision:
        target_rows, target_choices = (
            torch.from_numpy(arr) for arr in target_supervision(
                features.numpy(), masks.numpy(), expert.numpy(),
            )
        )
    stats = np.zeros(7)
    count = 0
    for _ in range(config.epochs):
        for indices in torch.randperm(len(features)).split(config.minibatch):
            logits, values = model(features[indices], masks[indices])
            log_prob = logits.log_softmax(dim=-1)
            target_loss = torch.zeros(())
            if target_rows is None:
                imitation = F.nll_loss(log_prob, expert[indices])
            else:
                single_label_loss = -log_prob.gather(1, expert[indices, None]).squeeze(1)
                is_target = target_rows[indices]
                equivalent = target_choices[indices]
                log_mass = log_prob.masked_fill(~equivalent, -1e9).logsumexp(dim=1)
                target_loss = -(log_mass * is_target).sum() / is_target.sum().clamp_min(1)
                imitation = torch.where(is_target, -log_mass, single_label_loss).mean()
            value_loss = F.mse_loss(values, returns[indices])
            entropy = -(log_prob.exp() * log_prob).sum(dim=1).mean()
            ratio = (log_prob.gather(1, actions[indices, None]).squeeze(1)
                     - old_log_prob[indices]).exp()
            weighted = ratio * advantages[indices]
            clipped = ratio.clamp(0.8, 1.2) * advantages[indices]
            policy_loss = -torch.min(weighted, clipped).mean()
            loss = (imitation if cloning else
                    policy_loss + imitation_weight * imitation - config.entropy * entropy)
            safe_pass_loss = other_rider_loss = torch.zeros(())
            if safe_pass is not None:
                eligible = safe_pass[indices]
                safe_pass_loss = -(log_prob[:, 15] * eligible).sum() / eligible.sum().clamp_min(1)
                eligible = other_rider[indices]
                other_rider_loss = -(log_prob[:, 17] * eligible).sum() / eligible.sum().clamp_min(1)
            loss = (loss + 0.5 * value_loss + config.lethal_rider_supervision * safe_pass_loss
                    + config.other_rider_supervision * other_rider_loss
                    + config.target_supervision * target_loss)
            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 0.5)
            optimizer.step()
            stats += [float(x.detach()) for x in
                      (policy_loss, value_loss, imitation, entropy, safe_pass_loss, other_rider_loss,
                       target_loss)]
            count += 1
    return dict(zip(("policy_loss", "value_loss", "imitation_loss", "entropy", "safe_pass_loss",
                    "other_rider_loss", "target_loss"),
                    stats / count))


@torch.no_grad()
def evaluate(model, arena, n):
    states = arena.observe()
    results = Counter()
    wins_by_class = np.zeros(12, dtype=np.int32)
    movement = TurnMovement(n)
    rider_lethal = 0
    rider_lethal_spent = 0
    rider_other = 0
    rider_other_spent = 0
    target_counts = np.zeros((2, 4), dtype=np.int32)
    target_equivalent = np.zeros_like(target_counts)
    games_by_class = np.zeros(12, dtype=np.int32)
    for _ in range(512):
        features, mask, expert = states
        if model is None:
            actions = expert
        else:
            logits, _ = model(torch.from_numpy(features), torch.from_numpy(mask))
            actions = logits.argmax(dim=1).numpy().astype(np.int32)
        lethal = lethal_without_rider(features, mask)
        rider_lethal += int(lethal.sum())
        rider_lethal_spent += int((lethal & (actions == 17)).sum())
        other_rider = mask[:, 17] & ~lethal
        rider_other += int(other_rider.sum())
        rider_other_spent += int((other_rider & (actions == 17)).sum())
        if arena.expert_labels:
            targeting, choices = target_supervision(features, mask, expert)
            target_indices = np.flatnonzero(targeting)
            phase = features[target_indices, 157].astype(np.int32)
            kind = features[target_indices, 154].astype(np.int32) - 1
            np.add.at(target_counts, (phase, kind), 1)
            np.add.at(target_equivalent, (phase, kind), choices[target_indices, actions[target_indices]])
        states, _, done, scores = arena.step(actions)
        movement.update(features, states[0], actions, done)
        results.update(scores[done].tolist())
        np.add.at(wins_by_class, arena.controlled_classes[done], scores[done] == 1)
        np.add.at(games_by_class, arena.controlled_classes[done], 1)
        if results.total() >= n:
            return {
                "wins": results[1],
                "losses": results[-1],
                "draws": results[0],
                "score": (results[1] + 0.5 * results[0]) / results.total(),
                "lethal_rider_offered": rider_lethal,
                "lethal_rider_spent": rider_lethal_spent,
                "other_rider_offered": rider_other,
                "other_rider_spent": rider_other_spent,
                "target_decisions_by_phase_kind": target_counts.tolist(),
                "target_equivalent_by_phase_kind": target_equivalent.tolist(),
                "turn_moves": movement.moves,
                "turn_revisits": movement.revisits,
                "turn_revisit_fraction": movement.revisits / max(1, movement.moves),
                "wins_by_class": wins_by_class.tolist(),
                "games_by_class": games_by_class.tolist(),
            }
    raise RuntimeError("development evaluation exceeded the duel turn budget")


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_CHECKPOINT)
    parser.add_argument("--resume", type=Path)
    parser.add_argument("--log", type=Path, default=DEFAULT_CHECKPOINT.with_suffix(".jsonl"))
    parser.add_argument("--seconds", type=float, default=1800)
    parser.add_argument("--updates", type=int, default=1000)
    parser.add_argument("--batch", type=int, default=384)
    parser.add_argument("--steps", type=int, default=32)
    parser.add_argument("--minibatch", type=int, default=768)
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--cloning-updates", type=int, default=12)
    parser.add_argument("--eval-every", type=int, default=10)
    parser.add_argument("--dev-games", type=int, default=660)
    parser.add_argument("--lr", type=float, default=3e-4)
    parser.add_argument("--final-lr", type=float,
                        help="linearly anneal the Adam learning rate to this value")
    parser.add_argument("--entropy", type=float, default=0.02)
    parser.add_argument("--imitation", type=float, default=0.06)
    parser.add_argument("--hp-shaping", type=float, default=0.2)
    parser.add_argument("--move-cost", type=float, default=0.0)
    parser.add_argument("--revisit-cost", type=float, default=0.0)
    parser.add_argument("--full-hp-wind-cost", type=float, default=0.0)
    parser.add_argument("--lethal-rider-cost", type=float, default=0.0)
    parser.add_argument("--lethal-rider-supervision", type=float, default=0.0)
    parser.add_argument("--other-rider-supervision", type=float, default=0.0)
    parser.add_argument("--target-supervision", type=float, default=0.0,
                        help="teach equivalent expert ground targets with arena ABI 1003")
    parser.add_argument("--run-id", type=int, default=0)
    parser.add_argument("--expert-opponent", action="store_true",
                        help="train against the tactical expert during rollouts")
    parser.add_argument("--frozen-opponent", type=Path,
                        help="mix baseline, expert, uniform exploration and this older policy")
    parser.add_argument("--dev-repeats", type=int, default=1,
                        help="independent seeds for each paired development matchup")
    parser.add_argument("--select-revisit-weight", type=float, default=0.0,
                        help="subtract this multiple of revisit rate from dev score")
    parser.add_argument("--select-target-weight", type=float, default=0.0,
                        help="add this multiple of equivalent target rate to dev score")
    parser.add_argument("--select-rider-weight", type=float, default=0.0,
                        help="add this multiple of balanced on-hit rider preference accuracy")
    parser.add_argument("--expert-dev", action="store_true",
                        help="record expert results on the development set before training")
    return parser.parse_args()


class MatchSchedule:
    """Cycle through all class pairings, shared levels, and player seats."""

    def __init__(self, seed, shuffle):
        self.seed = seed
        self.random = np.random.default_rng(seed)
        self.shuffle = shuffle
        self.position = 0
        pairs = [(first, second) for first in range(12) for second in range(12)
                 if shuffle or first < second]
        if not shuffle:
            # Rotating which class occupies the first seat avoids confounding
            # canonical class IDs with seat on the paired development suite.
            pairs = [(second, first) if (first + second) % 2 else (first, second)
                     for first, second in pairs]
        self.matrix = np.array([
            (first, second, level, seat)
            for first, second in pairs
            for level in range(1, 6)
            for seat in range(2)
        ], dtype=np.int32)
        if shuffle:
            self.matrix = self.random.permutation(self.matrix)

    def draw(self, count):
        previous = self.position
        self.position += count
        indices = np.arange(previous, self.position)
        rows = self.matrix[indices % len(self.matrix)]
        level = rows[:, 2]
        return (self.seed + (indices // 2).astype(np.uint64),
                np.ascontiguousarray(rows[:, :2]), np.stack([level, level], axis=1),
                rows[:, 3])


class ControlledArena:
    """Advance bot turns to expose one controlled decision per active lane."""

    def __init__(self, n, schedule, expert_opponent=False, recycle=True, expert_labels=True,
                 frozen_opponent=None):
        from arena.native import Batch

        self.batch = Batch(n)
        self.n = n
        self.schedule = schedule
        self.expert_opponent = expert_opponent
        self.recycle = recycle
        self.expert_labels = expert_labels
        self.frozen_opponent = frozen_opponent
        self.opponent_rng = np.random.default_rng(schedule.seed ^ 0x3210)
        self.opponents = np.zeros(n, dtype=np.int8)
        self.seats = np.empty(n, dtype=np.int32)
        self.live = np.zeros(n, dtype=bool)
        self.progress = np.zeros(n, dtype=np.int32)
        self.completed = Counter()
        self.opening_scores = Counter()
        self.controlled_classes = np.empty(n, dtype=np.int32)
        self.levels = np.empty(n, dtype=np.int32)
        self._start(np.arange(n, dtype=np.int32))
        self._advance()
        if self.recycle:
            self._replace_opening_outcomes()

    def _start(self, indices):
        if not len(indices):
            return
        seeds, classes, levels, seats = self.schedule.draw(len(indices))
        self.batch.reset(indices, seeds, classes, levels)
        self.seats[indices] = seats
        self.live[indices] = True
        self.progress[indices] = 0
        self.controlled_classes[indices] = classes[np.arange(len(indices)), seats]
        self.levels[indices] = levels[:, 0]
        if self.frozen_opponent is not None:
            self.opponents[indices] = self.opponent_rng.choice(
                4, size=len(indices), p=(0.50, 0.20, 0.10, 0.20),
            )

    def _coverage_keys(self, indices):
        return zip(self.controlled_classes[indices].tolist(),
                   self.levels[indices].tolist(), self.seats[indices].tolist())

    def _advance(self):
        seats = np.where(self.live, self.seats, -1)
        if self.frozen_opponent is None:
            self.batch.advance(seats, expert=self.expert_opponent)
            self.features, self.masks, self.actors, self.outcomes = self.batch.observe()
        else:
            self.batch.advance(np.where(self.opponents == 0, seats, -1))
            self.batch.advance(np.where(self.opponents == 1, seats, -1), expert=True)
            self._advance_exploratory()

    @torch.no_grad()
    def _advance_exploratory(self):
        for _ in range(128):
            self.features, self.masks, self.actors, self.outcomes = self.batch.observe()
            needs_action = self.live & (self.actors >= 0) & (self.actors != self.seats)
            if not needs_action.any():
                return
            actions = np.full(self.n, -1, dtype=np.int32)
            random = needs_action & (self.opponents == 2)
            frozen = needs_action & (self.opponents == 3)
            if random.any():
                mask = self.masks[random]
                scores = self.opponent_rng.uniform(size=mask.shape)
                actions[random] = np.where(mask, scores, -1).argmax(axis=1)
            if frozen.any():
                features = torch.from_numpy(self.features[frozen])
                mask = torch.from_numpy(self.masks[frozen])
                logits, _ = self.frozen_opponent(features, mask)
                actions[frozen] = sample_actions(logits).numpy()
            self.batch.step(actions)
        raise RuntimeError("exploratory opponent did not yield after 128 decisions")

    def _replace_opening_outcomes(self):
        # Opening KOs can finish before the learner acts. They have no
        # transition to attribute to the learner, so record them separately.
        for _ in range(128):
            terminal = (self.actors < 0) & (self.outcomes >= 0)
            if not terminal.any():
                return
            scores = self._scores(terminal)
            self.opening_scores.update(scores[terminal].tolist())
            self._start(np.flatnonzero(terminal).astype(np.int32))
            self._advance()
        raise RuntimeError("failed to find a fight with a learner decision")

    def _scores(self, done):
        decisive = done & (self.outcomes >= 0) & (self.outcomes < 2)
        return np.where(decisive, 2 * (self.outcomes == self.seats) - 1, 0)

    def observe(self):
        expert = self.batch.bot(expert=True) if self.expert_labels else np.zeros(self.n, dtype=np.int32)
        active = self.live & (self.actors >= 0)
        if active.all():
            return self.features, self.masks, expert
        masks = self.masks.copy()
        masks[~active, 0] = True
        return self.features, masks, expert

    def step(self, actions):
        self.batch.step(np.where(self.live & (self.actors == self.seats), actions, -1))
        self.progress[self.live] += 1
        self._advance()
        done = self.live & ((self.outcomes >= 0) | (self.progress >= 512))
        # Positive rewards for wins, negative rewards for losses, and zero for draws.
        scores = self._scores(done)
        rewards = scores.astype(np.float32)
        completed = np.flatnonzero(done).astype(np.int32)
        self.completed.update(self._coverage_keys(completed))
        if self.recycle:
            self._start(completed)
            if done.any():
                self._advance()
                self._replace_opening_outcomes()
        else:
            self.live[done] = False
        return self.observe(), rewards, done, scores

    def close(self):
        self.batch.close()


def log_record(path, record):
    text = json.dumps(record, sort_keys=True)
    print(text, flush=True)
    with path.open("a") as handle:
        handle.write(text + "\n")


def train(config):
    from arena.native import ACTIONS, FEATURES, _lib

    config.arena_abi = _lib().arena_schema()
    if config.target_supervision and config.arena_abi != 1003:
        raise ValueError("ground target supervision requires arena ABI 1003")
    for checkpoint in (config.resume, config.frozen_opponent):
        if checkpoint:
            Policy.load(checkpoint).require_arena(config.arena_abi, FEATURES, ACTIONS)

    if not 1 <= config.dev_games <= 660 or config.dev_games % 2:
        raise ValueError("--dev-games must be even and between 2 and 660")
    sizes = (config.updates, config.batch, config.steps, config.minibatch, config.epochs,
             config.threads, config.eval_every)
    if any(size < 1 for size in sizes) or config.batch * config.steps < 2:
        raise ValueError("training batch, rollout, and optimizer settings must be positive")
    if not 0 < config.seconds <= 18000 or not 0 <= config.cloning_updates <= config.updates:
        raise ValueError("invalid runtime budget or cloning phase length")
    if not 1 <= config.dev_repeats <= 16 or config.expert_opponent and config.frozen_opponent:
        raise ValueError("invalid development repeat count or opponent settings")
    if not config.lr > 0 or (config.final_lr is not None and not 0 < config.final_lr <= config.lr):
        raise ValueError("invalid learning rate schedule")
    if not 0 <= config.run_id < 2**16:
        raise ValueError("--run-id must fit 16 bits")
    config.log.parent.mkdir(parents=True, exist_ok=True)
    torch.set_num_threads(config.threads)
    torch.set_num_interop_threads(1)
    torch.manual_seed(config.run_id)
    model = ActorCritic(FEATURES, ACTIONS)
    if config.resume:
        model.load(config.resume)
    frozen = None
    if config.frozen_opponent:
        frozen = ActorCritic(FEATURES, ACTIONS)
        frozen.load(config.frozen_opponent)
        frozen.eval()
    optimizer = torch.optim.Adam(model.parameters(), lr=config.lr, eps=1e-5)
    start = time.monotonic()
    log_record(config.log, {"event": "start", **{
        key: str(value) if isinstance(value, Path) else value
        for key, value in vars(config).items()
    }})
    deadline = start + config.seconds
    train_arena = None
    try:
        train_arena = ControlledArena(
            config.batch, MatchSchedule(TRAIN_SEED + (config.run_id << 24), shuffle=True),
            expert_opponent=config.expert_opponent, frozen_opponent=frozen,
        )
        run_training(model, optimizer, train_arena, config, start, deadline)
    finally:
        if train_arena is not None:
            train_arena.close()
        log_record(config.log, {"event": "stop", "run_id": config.run_id,
                                "elapsed_s": round(time.monotonic() - start, 2)})


def dev_score(model, n, repeats=1, expert_opponent=False, show_target=False):
    # Pair both seats on a fixed set of previously unseen duels. Repeated
    # sweeps of all 660 games use independent die streams for each matchup.
    schedule = MatchSchedule(DEV_SEED, shuffle=False)
    dev = ControlledArena(n * repeats, schedule, expert_opponent=expert_opponent,
                          recycle=False, expert_labels=model is None or show_target)
    try:
        return evaluate(model, dev, n * repeats)
    finally:
        dev.close()


def selection_score(dev, expert_dev, config):
    def adjusted(stats):
        target_success = sum(map(sum, stats["target_equivalent_by_phase_kind"]))
        total_targets = sum(map(sum, stats["target_decisions_by_phase_kind"]))
        safe_pass = 1 - stats["lethal_rider_spent"] / max(1, stats["lethal_rider_offered"])
        other_rider = stats["other_rider_spent"] / max(1, stats["other_rider_offered"])
        return (stats["score"] - config.select_revisit_weight * stats["turn_revisit_fraction"]
                + config.select_target_weight * target_success / max(1, total_targets)
                + config.select_rider_weight * (safe_pass + other_rider) / 2)

    primary = adjusted(dev)
    return primary if expert_dev is None else 0.7 * primary + 0.3 * adjusted(expert_dev)


def run_training(model, optimizer, arena, config, start, deadline):
    from arena.native import ACTIONS, FEATURES

    rollout = Rollout(config.steps, config.batch, FEATURES, ACTIONS)
    recent = deque(maxlen=4096)
    movement = TurnMovement(config.batch)
    states = arena.observe()
    show_target = bool(config.target_supervision)
    best_score = -1
    if config.resume:
        dev = dev_score(model, config.dev_games, config.dev_repeats, show_target=show_target)
        expert_dev = (dev_score(model, config.dev_games, config.dev_repeats, expert_opponent=True,
                                show_target=show_target)
                      if config.frozen_opponent else None)
        best_score = selection_score(dev, expert_dev, config)
        log_record(config.log, {"event": "resume_dev", "run_id": config.run_id, "dev": dev,
                                "expert_opponent_dev": expert_dev, "selection_score": best_score})
    if config.expert_dev:
        expert_dev = dev_score(None, config.dev_games, config.dev_repeats)
        log_record(config.log, {"event": "expert_dev", "run_id": config.run_id, "dev": expert_dev})
    games = 0
    for index in range(config.updates):
        if time.monotonic() >= deadline:
            break
        lr = config.lr
        if config.final_lr is not None:
            lr += (config.final_lr - config.lr) * index / max(1, config.updates - 1)
            for group in optimizer.param_groups:
                group["lr"] = lr
        cloning = index < config.cloning_updates
        clone_probability = max(0.25, 1 - index / max(config.cloning_updates, 1)) if cloning else 0.0
        moves_before, revisits_before = movement.moves, movement.revisits
        states, next_value = collect(
            model, arena, rollout, states, clone_probability, recent, config, movement,
        )
        returns = rollout.returns(next_value, GAMMA, 0.96)
        imitation_weight = config.imitation / (1 + max(0, index - config.cloning_updates) / 25)
        stats = update(model, optimizer, rollout, returns, config, cloning, imitation_weight)
        games += int(rollout.done.sum())
        update_number = index + 1
        report = {
            "event": "update",
            "run_id": config.run_id,
            "update": update_number,
            "stage": "cloning" if cloning else "ppo",
            "learning_rate": lr,
            "elapsed_s": round(time.monotonic() - start, 2),
            "decisions": update_number * config.steps * config.batch,
            "completed_games": games,
            "opening_games_without_actions": arena.opening_scores.total(),
            "turn_moves": movement.moves - moves_before,
            "turn_revisits": movement.revisits - revisits_before,
            "coverage_cells": len(arena.completed),
            "coverage_min": min(arena.completed.values(), default=0),
            "rolling_win_pct": round(100 * sum(x == 1 for x in recent) / max(1, len(recent)), 2),
            **{key: round(value, 5) for key, value in stats.items()},
        }
        due = (update_number == max(config.cloning_updates, 1)
               or update_number % config.eval_every == 0
               or update_number == config.updates)
        if due and time.monotonic() < deadline:
            dev = dev_score(model, config.dev_games, config.dev_repeats, show_target=show_target)
            report["dev"] = dev
            if config.frozen_opponent:
                report["expert_opponent_dev"] = dev_score(
                    model, config.dev_games, config.dev_repeats, expert_opponent=True,
                    show_target=show_target,
                )
            score = selection_score(dev, report.get("expert_opponent_dev"), config)
            report["selection_score"] = score
            if score > best_score and len(arena.completed) == 120:
                best_score = score
                classes = sorted({cls for cls, _, _ in arena.completed})
                levels = sorted({level for _, level, _ in arena.completed})
                model.export(config.output, {
                    "checkpoint_version": CHECKPOINT_VERSION,
                    "arena_abi": config.arena_abi,
                    "classes": classes,
                    "levels": levels,
                    "training_seats": [0, 1],
                    "method": "expert behavior cloning followed by clipped PPO",
                    "observation_features": FEATURES,
                    "action_count": ACTIONS,
                    "training_run_id": config.run_id,
                    "training_update": update_number,
                    "completed_training_games": games,
                    "completed_games_per_class_level_seat_min": min(arena.completed.values()),
                    "stage": report["stage"],
                    "dev": dev,
                    "expert_opponent_dev": report.get("expert_opponent_dev"),
                    "selection_score": score,
                    "seed_namespace": f"{TRAIN_SEED:016x}",
                    "move_cost": config.move_cost,
                    "same_turn_tile_revisit_cost": config.revisit_cost,
                    "full_hp_wind_cost_below_level_five": config.full_hp_wind_cost,
                    "already_lethal_rider_cost": config.lethal_rider_cost,
                    "already_lethal_pass_supervision": config.lethal_rider_supervision,
                    "other_rider_supervision": config.other_rider_supervision,
                    "ground_target_supervision": config.target_supervision,
                    "training_opponents": (
                        {"baseline": 0.50, "tactical_expert": 0.20, "uniform_random": 0.10,
                         "frozen_policy": 0.20} if config.frozen_opponent else
                        {"tactical_expert" if config.expert_opponent else "baseline": 1.0}
                    ),
                })
                report["saved"] = True
        log_record(config.log, report)


if __name__ == "__main__":
    train(parse_args())
