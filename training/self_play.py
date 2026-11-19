"""Train both sides of complete arena bouts with masked, league-based PPO."""

import argparse
from collections import Counter, deque
from contextlib import contextmanager
import copy
import fcntl
import hashlib
import json
from pathlib import Path
import time

import numpy as np
import torch
from torch import nn
from torch.nn import functional as F

from arena.native import ROSTER_NEW_INPUTS
from arena.policy import CHECKPOINT_VERSION, Policy
from train_arena import ActorCritic, sample_actions

try:
    from . import roster_schedule as roster
except ImportError:  # Direct script invocation.
    import roster_schedule as roster

TRAIN_DOMAIN = 0x5332540000000000
DEV_DOMAIN = 0x5332440000000000
BASE_PHASES = slice(256, 330)
EXTENDED_PHASES = slice(616, 632)
LATE_PHASES = slice(864, 896)
ROSTER_PHASE_START = 140
ROSTER_PHASE_REMAINDER = slice(116, 124)
ROSTER_PHASE_QUOTIENT = slice(124, 132)
PHASE_COUNT = 204
# Phase 80 aims damaging projectiles. On ABI 2002, IDs 0/1 choose
# one projectile and IDs 16/17 assign all remaining Magic Missile darts.
PROJECTILE_AIM_FEATURE = EXTENDED_PHASES.start + 80 - 74
PROJECTILE_SELF = (0, 16)
PROJECTILE_FOE = (1, 17)
RESULT_REASONS = ("ongoing", "defeat", "double_ko", "turn_limit", "decision_limit")
RUNS = Path(__file__).resolve().parent / "runs"
DEFAULT_RUNS = RUNS / "roster-self-play"
BUDGET_SECONDS = 5 * 3600
PREVIOUS_TRAINING_SECONDS = 1549.98


def phase_ids(features):
    base = features[:, BASE_PHASES]
    extended = features[:, EXTENDED_PHASES]
    late = features[:, LATE_PHASES]
    ids = np.where(extended.any(axis=1), 74 + extended.argmax(axis=1), base.argmax(axis=1))
    ids = np.where(late.any(axis=1), 90 + late.argmax(axis=1), ids)
    remainder = features[:, ROSTER_PHASE_REMAINDER]
    groups = features[:, ROSTER_PHASE_QUOTIENT]
    active = remainder.any(axis=1) & groups.any(axis=1)
    quotient = groups.argmax(axis=1)
    return np.where(active, ROSTER_PHASE_START + 8 * quotient + remainder.argmax(axis=1), ids)


def projectile_target_loss(logits, features, masks):
    """Prefer the foe within the probability mass assigned to either adventurer."""
    eligible = ((features[:, PROJECTILE_AIM_FEATURE] == 1)
                & masks[:, PROJECTILE_SELF].any(dim=1)
                & masks[:, PROJECTILE_FOE].any(dim=1))
    aim_logits = logits[eligible]
    self_score = aim_logits[:, PROJECTILE_SELF].logsumexp(dim=1)
    foe_score = aim_logits[:, PROJECTILE_FOE].logsumexp(dim=1)
    return F.softplus(self_score - foe_score).sum() / eligible.sum().clamp_min(1)


class TrainingDeadline(Exception):
    """The elapsed training budget has been reached."""


def check_deadline(deadline):
    reached = deadline() if callable(deadline) else time.monotonic() >= deadline
    if reached:
        raise TrainingDeadline


def record(handle, event, **fields):
    line = json.dumps({"event": event, **fields}, sort_keys=True)
    handle.write(line + "\n")
    handle.flush()
    print(line, flush=True)


@contextmanager
def ledger(path, run_id, planned_seconds):
    """Allow only one trainee per shared ledger, including after interruption."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        handle.seek(0)
        starts, stops = {}, {}
        for line in handle:
            event = json.loads(line)
            if event["event"] == "start":
                starts[event["run_id"]] = event
            elif event["event"] == "stop":
                stops[event["run_id"]] = event
        if run_id in starts:
            raise ValueError(f"run ID {run_id} has already used training seeds in this ledger")
        spent = sum(stops[run]["elapsed_s"] if run in stops else start["reserved_s"]
                    for run, start in starts.items())
        # Count a crashed run's entire reservation. Leave room to persist its
        # terminal record after the last deadline check.
        remaining = min(planned_seconds, BUDGET_SECONDS - spent - 2)
        if remaining <= 0:
            raise TrainingDeadline("shared training budget exhausted")
        started = time.monotonic()
        record(handle, "start", run_id=run_id, reserved_s=remaining,
               previous_project_s=PREVIOUS_TRAINING_SECONDS, previous_self_play_s=spent)
        try:
            yield handle, started, started + remaining
        finally:
            record(handle, "stop", run_id=run_id, elapsed_s=round(time.monotonic() - started, 4))


class Schedule:
    """Paired fixtures. Three in four training pairs have the same level."""

    def __init__(self, seed, *, training):
        self.seed = seed
        self.training = training
        self.rng = np.random.default_rng(seed)
        self.base = np.array([
            (left, right, level, level)
            for level in range(1, 6)
            for left in range(12)
            for right in range(left if training else left + 1, 12)
        ], dtype=np.int32)
        self.order = self.rng.permutation(self.base) if training else self.base
        self.mixed_base = np.array([
            (left, right, first, second)
            for first in range(1, 6)
            for second in range(1, 6) if first != second
            for left in range(12)
            for right in range(12)
        ], dtype=np.int32) if training else None
        self.mixed_order = self.rng.permutation(self.mixed_base) if training else None
        self.pairs_started = 0
        self.equal_pairs_started = 0
        self.mixed_pairs_started = 0

    def _rows(self, count, offset, order, original):
        chunks = []
        while count:
            position = offset % len(order)
            take = min(count, len(order) - position)
            chunks.append(order[position:position + take])
            count -= take
            offset += take
            if self.training and position + take == len(order):
                order = self.rng.permutation(original)
        return np.concatenate(chunks), order

    def draw(self, n):
        if n <= 0 or n % 2:
            raise ValueError("schedule batch must be positive and even")
        count = n // 2
        previous = self.pairs_started
        self.pairs_started += count
        ids = np.arange(previous, self.pairs_started)
        mixed = (ids % 4 == 3) if self.training else np.zeros(count, dtype=bool)
        pairs = np.empty((count, 4), dtype=np.int32)
        equal_count = int((~mixed).sum())
        if equal_count:
            rows, self.order = self._rows(equal_count, self.equal_pairs_started,
                                           self.order, self.base)
            pairs[~mixed] = rows
            self.equal_pairs_started += equal_count
        mixed_count = count - equal_count
        if mixed_count:
            rows, self.mixed_order = self._rows(mixed_count, self.mixed_pairs_started,
                                                 self.mixed_order, self.mixed_base)
            pairs[mixed] = rows
            self.mixed_pairs_started += mixed_count
        seed = ids.astype(np.uint64) + np.uint64(self.seed)
        # Alternate physical class assignments across pairings. Keep each
        # class attached to its level, and assign each controller to both seats.
        flip = (pairs.sum(axis=1) % 2).astype(bool)
        pairs[flip] = pairs[flip][:, [1, 0, 3, 2]]
        classes = np.repeat(pairs[:, :2], 2, axis=0)
        levels = np.repeat(pairs[:, 2:], 2, axis=0)
        seats = np.tile(np.array([0, 1], dtype=np.int32), len(pairs))
        return np.repeat(seed, 2), classes, levels, seats


class EpisodeTrace:
    """Packed learner turns. Edges connect the same seat even across interrupts."""

    def __init__(self, lanes):
        self.last = np.full((lanes, 2), -1, dtype=np.int32)
        self.chunks = []
        self.size = 0

    def append(self, lanes, seats, features, masks, actions, log_prob, values, rewards=None):
        previous = self.last[lanes, seats]
        start = self.size
        self.size += len(lanes)
        self.last[lanes, seats] = np.arange(start, self.size)
        if rewards is None:
            rewards = np.zeros(len(lanes), dtype=np.float32)
        self.chunks.append((features, masks, actions, log_prob, values, previous, rewards))

    def finish(self, outcomes, gamma, lam, *, draw_rewards=None):
        if not self.chunks:
            raise RuntimeError("rollout recorded no learner decisions")
        features, masks, actions, log_prob, values, previous, rewards = [
            np.concatenate(chunks) for chunks in zip(*self.chunks)
        ]
        next_index = np.full(self.size, -1, dtype=np.int32)
        ids = np.arange(self.size)
        linked = previous >= 0
        next_index[previous[linked]] = ids[linked]
        if draw_rewards is None:
            draw_rewards = np.zeros(len(outcomes), dtype=np.float32)
        for seat in (0, 1):
            last = self.last[:, seat]
            active = last >= 0
            signed = np.where(outcomes == 2, draw_rewards, np.where(outcomes == seat, 1.0, -1.0))
            rewards[last[active]] += signed[active]
        advantages = np.zeros(self.size + 1, dtype=np.float32)
        value_next = np.append(values, np.float32(0))
        end = self.size
        for chunk in reversed(self.chunks):
            start = end - len(chunk[0])
            following = next_index[start:end]
            advantages[start:end] = (rewards[start:end] + gamma * value_next[following]
                                     - values[start:end] + gamma * lam * advantages[following])
            end = start
        advantages = advantages[:-1]
        return tuple(torch.from_numpy(array) for array in
                     (features, masks, actions, log_prob, advantages, advantages + values))


class RepeatedChoices:
    """Detect taking the same action in the same actor-visible turn state."""

    def __init__(self, lanes):
        self.turns = np.full(lanes, -1, dtype=np.int32)
        self.seen = [[set(), set()] for _ in range(lanes)]

    def check(self, lanes, seats, features, actions):
        turns = np.rint(60 * features[:, 30]).astype(np.int32)
        repeated = np.zeros(len(lanes), dtype=bool)
        for j, (lane, seat, turn, row, action) in enumerate(
                zip(lanes, seats, turns, features, actions)):
            history = self.seen[lane]
            if turn != self.turns[lane]:
                history[0].clear()
                history[1].clear()
                self.turns[lane] = turn
            key = (row.tobytes(), int(action))
            repeated[j] = key in history[seat]
            history[seat].add(key)
        return repeated


class League:
    """Warm-start, recent, and older snapshots all remain learned opponents."""

    def __init__(self, warm, size):
        self.warm = self.freeze(warm)
        self.snapshots = deque(maxlen=max(1, size - 1))

    @staticmethod
    def freeze(model):
        return copy.deepcopy(model).eval().requires_grad_(False)

    def add(self, model):
        self.snapshots.append(self.freeze(model))

    @property
    def opponents(self):
        return (self.warm, *self.snapshots)

    def assignments(self, seats, rng, live_fraction):
        pool_size = len(self.opponents)
        opponent = rng.integers(0, pool_size, size=len(seats) // 2)
        opponent[rng.random(len(opponent)) < live_fraction] = -1
        # -1 denotes the current learner. Both seats of mirrored fixtures
        # face the same frozen opponent; all-live fixtures train both seats.
        controllers = np.full((len(seats), 2), -1, dtype=np.int32)
        controllers[np.arange(len(seats)), 1 - seats] = np.repeat(opponent, 2)
        return controllers


@torch.no_grad()
def collect(batch, model, league, fixtures, controllers, deadline, repeat_cost=0.0):
    lanes = np.arange(batch.n, dtype=np.int32)
    trace = EpisodeTrace(len(lanes))
    if isinstance(fixtures, roster.RosterFixtures):
        fixtures.reset(batch)
    else:
        seeds, classes, levels, _ = fixtures
        batch.reset(lanes, seeds, classes, levels)
    models = league.opponents
    x, mask, actors, outcomes = batch.observe()
    decisions = Counter()
    phase_counts = np.zeros((2, PHASE_COUNT), dtype=np.int64)
    cycles = RepeatedChoices(len(lanes)) if repeat_cost else None
    for _ in range(2048):
        check_deadline(deadline)
        active = lanes[actors >= 0]
        if not len(active):
            for role, counts in zip(("learner", "snapshot"), phase_counts):
                decisions.update({f"{role}_phase_{i}": int(n) for i, n in enumerate(counts) if n})
            return trace, outcomes, decisions
        owner = controllers[active, actors[active]]
        actions = np.full(len(lanes), -1, dtype=np.int32)
        for index in np.unique(owner):
            selected = active[owner == index]
            policy = model if index == -1 else models[index]
            features, legal = x[selected], mask[selected]
            if isinstance(policy, Policy):
                chosen = policy.choose(features, legal)
            else:
                logits, values = policy(torch.from_numpy(features), torch.from_numpy(legal))
                choice = sample_actions(logits)
                chosen = choice.numpy()
            actions[selected] = chosen
            role = "learner" if index == -1 else "snapshot"
            decisions[role] += len(selected)
            expanded = chosen >= 32
            if features.shape[1] >= LATE_PHASES.stop:
                phase = phase_ids(features)
                phase_counts[int(index >= 0)] += np.bincount(phase, minlength=PHASE_COUNT)
                decisions[f"{role}_expanded_main"] += int((expanded & (phase == 0)).sum())
            rewards = None
            if cycles is not None:
                repeats = cycles.check(selected, actors[selected], features, chosen)
                decisions[f"{role}_repeated_choice"] += int(repeats.sum())
                rewards = repeats.astype(np.float32) * -repeat_cost
            if index == -1:
                log_prob = logits.log_softmax(dim=1).gather(1, choice[:, None]).squeeze(1)
                trace.append(selected, actors[selected], features, legal, chosen,
                             log_prob.numpy(), values.numpy(), rewards)
        batch.step(actions)
        x, mask, actors, outcomes = batch.observe()
    raise RuntimeError("native arena exceeded its maximum decision count")


def update(model, optimizer, rollout, config, deadline):
    features, masks, actions, old_log_prob, advantage, returns = rollout
    advantage = (advantage - advantage.mean()) / advantage.std(unbiased=False).clamp_min(1e-6)
    totals = np.zeros(5)
    minibatches = 0
    for _ in range(config.epochs):
        for indices in torch.randperm(len(actions)).split(config.minibatch):
            check_deadline(deadline)
            observations, legal = features[indices], masks[indices]
            logits, values = model(observations, legal)
            log_probs = logits.log_softmax(dim=1)
            selected = log_probs.gather(1, actions[indices, None]).squeeze(1)
            ratio = (selected - old_log_prob[indices]).exp()
            objective = ratio * advantage[indices]
            clipped = ratio.clamp(0.8, 1.2) * advantage[indices]
            policy_loss = -torch.minimum(objective, clipped).mean()
            value_loss = F.mse_loss(values, returns[indices])
            entropy = -(log_probs.exp() * log_probs).sum(dim=1).mean()
            loss = policy_loss + 0.5 * value_loss - config.entropy * entropy
            target_loss = torch.zeros(())
            if config.projectile_target_weight:
                target_loss = projectile_target_loss(logits, observations, legal)
                loss += config.projectile_target_weight * target_loss
            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 0.5)
            optimizer.step()
            kl = (old_log_prob[indices] - selected).mean()
            totals += [float(x.detach()) for x in (policy_loss, value_loss, entropy, kl, target_loss)]
            minibatches += 1
    return dict(zip(("policy_loss", "value_loss", "entropy", "kl", "projectile_target_loss"),
                    totals / minibatches))


@torch.no_grad()
def evaluate(batch, model, prior, opponent, deadline, *, legacy=True):
    lanes = np.arange(batch.n, dtype=np.int32)
    if legacy:
        seeds, classes, levels, seats = Schedule(DEV_DOMAIN, training=False).draw(batch.n)
        batch.reset(lanes, seeds, classes, levels)
        domain = DEV_DOMAIN
    else:
        domain = roster.DEV_DOMAIN
        fixtures = roster.RosterSchedule(domain, training=False).draw(batch.n)
        fixtures.reset(batch)
        seats = fixtures.seats
        classes = fixtures.builds // roster.BUILDS_PER_CLASS
    prior_rng = torch.Generator().manual_seed(domain + 0x1000)
    decisions = 0
    expanded_main = 0
    for _ in range(2048):
        check_deadline(deadline)
        if opponent in ("baseline", "expert"):
            batch.advance(seats, expert=opponent == "expert")
        x, mask, actors, results = batch.observe()
        active = lanes[actors >= 0]
        if not len(active):
            break
        actions = np.full(batch.n, -1, dtype=np.int32)
        learner = active[actors[active] == seats[active]]
        other = active[actors[active] != seats[active]]
        decisions += len(learner)
        if len(learner):
            logits, _ = model(torch.from_numpy(x[learner]), torch.from_numpy(mask[learner]))
            actions[learner] = logits.argmax(dim=1).numpy()
        if len(other):
            policy = model if opponent == "self" else prior
            logits, _ = policy(torch.from_numpy(x[other]), torch.from_numpy(mask[other]))
            if opponent == "prior":
                actions[other] = torch.multinomial(
                    logits.softmax(dim=1), 1, generator=prior_rng,
                ).squeeze(1).numpy()
            else:
                actions[other] = logits.argmax(dim=1).numpy()
        expanded_main += int(((actions[learner] >= 32) & (x[learner, BASE_PHASES.start] == 1)).sum())
        batch.step(actions)
    else:
        raise RuntimeError("development fights did not finish")
    wins = results == seats
    draws = results == 2
    return {
        "wins": int(wins.sum()),
        "losses": int((~wins & ~draws).sum()),
        "draws": int(draws.sum()),
        "score": float((wins + 0.5 * draws).mean()),
        "wins_by_class": np.bincount(classes[lanes, seats], weights=wins, minlength=12).astype(int).tolist(),
        **({} if legacy else {
            "wins_by_build": np.bincount(fixtures.builds[lanes, seats], weights=wins,
                                         minlength=roster.BUILD_COUNT).astype(int).tolist(),
        }),
        "learner_decisions": decisions,
        "opponent_action_selection": "seeded_stochastic" if opponent == "prior" else "deterministic",
        "expanded_main_choices": expanded_main,
        "termination": termination(batch),
    }


def termination(batch, reasons=None):
    if reasons is None:
        reasons = batch.result_reasons()
    results = np.bincount(reasons, minlength=len(RESULT_REASONS))
    return dict(zip(RESULT_REASONS, map(int, results)))


def all_dev(batch, model, prior, deadline, *, legacy=True):
    return {label: evaluate(batch, model, prior, label, deadline, legacy=legacy)
            for label in ("baseline", "expert", "prior", "self")}


def dev_score(results):
    self_termination = results["self"]["termination"]
    truncated = self_termination["turn_limit"] + self_termination["decision_limit"]
    return (0.40 * results["baseline"]["score"]
            + 0.30 * results["expert"]["score"]
            + 0.30 * results["prior"]["score"]
            - 0.50 * truncated / max(1, sum(self_termination.values())))


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--warm", type=Path, required=True)
    parser.add_argument("--resume", type=Path)
    parser.add_argument("--legacy-schedule", action="store_true",
                        help="reproduce the historical preset and mixed-level schedule")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--log", type=Path)
    parser.add_argument("--run-id", type=int, required=True)
    parser.add_argument("--seconds", type=float, default=1200)
    parser.add_argument("--updates", type=int, default=1000)
    parser.add_argument("--batch", type=int, default=384)
    parser.add_argument("--minibatch", type=int, default=768)
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--lr", type=float, default=2e-4)
    parser.add_argument("--final-lr", type=float, default=6e-5)
    parser.add_argument("--entropy", type=float, default=0.012)
    parser.add_argument("--repeat-cost", type=float, default=0.0,
                        help="training penalty for an identical repeated public choice in one turn")
    parser.add_argument("--projectile-target-weight", type=float, default=0.0,
                        help="auxiliary foe-versus-self loss in eligible projectile choices (ABI 2002)")
    parser.add_argument("--timeout-penalty", type=float, default=0.20,
                        help="both learned seats receive this penalty for exhausting a native limit")
    parser.add_argument("--live-fraction", type=float, default=0.35)
    parser.add_argument("--pool-size", type=int, default=6)
    parser.add_argument("--add-every", type=int, default=50)
    parser.add_argument("--eval-every", type=int, default=50)
    parser.add_argument("--dev-batches", type=int, default=2,
                        help="full sweeps of development games per opponent")
    parser.add_argument("--checkpoint-dir", type=Path,
                        help="also retain each development checkpoint for separate diagnostics")
    config = parser.parse_args()
    run_path = (RUNS / "self_play") if config.legacy_schedule else DEFAULT_RUNS
    config.output = config.output or run_path / "selected.npz"
    config.log = config.log or run_path / "training.jsonl"
    return config


def load_model(path, arena_abi, features, actions, *, roster_run, model=None):
    policy = Policy.load(path).require_arena(arena_abi, features, actions)
    if model is None:
        model = ActorCritic.from_checkpoint(path)
    else:
        if policy.weights["w1"].shape[1] != model.first.out_features:
            raise ValueError("--resume and --warm must use the same hidden width")
        model.load(path)
    initialized_inputs = roster_run and "build_ids" not in policy.metadata
    if initialized_inputs:
        # These source columns were reserved and had no training signal.
        # Initialize their contributions to zero before seeing roster states.
        with torch.no_grad():
            model.first.weight[:, ROSTER_NEW_INPUTS] = 0
    return model, initialized_inputs


def train(config):
    from arena.native import ACTIONS, FEATURES, Batch, _lib

    if (config.batch <= 0 or config.batch % 2 or config.updates <= 0
            or not 0 <= config.run_id < 256 or not 0 < config.seconds <= BUDGET_SECONDS
            or not 0 < config.final_lr <= config.lr or not 0 < config.live_fraction < 1
            or not 0 <= config.timeout_penalty < 1 or not 0 <= config.repeat_cost < 1
            or not 0 <= config.projectile_target_weight < 1
            or min(config.epochs, config.minibatch, config.threads, config.add_every,
                   config.eval_every, config.dev_batches) <= 0 or config.pool_size < 2):
        raise ValueError("invalid self-play configuration")
    if config.warm.resolve() == config.output.resolve():
        raise ValueError("output must be separate from the frozen warm start")
    torch.set_num_threads(config.threads)
    torch.set_num_interop_threads(1)
    torch.manual_seed(config.run_id)
    legacy = config.legacy_schedule
    domain = TRAIN_DOMAIN if legacy else roster.TRAIN_DOMAIN
    rng = np.random.default_rng(domain ^ config.run_id)
    arena_abi = _lib().arena_schema()
    if config.projectile_target_weight and arena_abi < 2002:
        raise ValueError("projectile target loss requires viewer-relative arena ABI 2002 or later")
    prior, initialized_warm_inputs = load_model(
        config.warm, arena_abi, FEATURES, ACTIONS, roster_run=not legacy,
    )
    model = League.freeze(prior).requires_grad_(True)
    initialized_resume_inputs = False
    if config.resume:
        _, initialized_resume_inputs = load_model(
            config.resume, arena_abi, FEATURES, ACTIONS, roster_run=not legacy, model=model,
        )
    optimizer = torch.optim.Adam(model.parameters(), lr=config.lr, eps=1e-5)
    league = League(prior, config.pool_size)
    if config.resume:
        league.add(model)
    schedule_type = Schedule if legacy else roster.RosterSchedule
    schedule = schedule_type(domain + (config.run_id << 32), training=True)
    dev_bouts = 660 if legacy else roster.DEV_BOUTS
    full_coverage = 120 if legacy else roster.BUILD_COUNT * 2
    seen = Counter()
    cumulative = Counter()
    warm_hash = hashlib.sha256(config.warm.read_bytes()).hexdigest()
    with ledger(config.log, config.run_id, config.seconds) as (log, start, deadline):
        initialization = {} if legacy else {
            "zero_initialized_warm_inputs": list(ROSTER_NEW_INPUTS) if initialized_warm_inputs else [],
            "zero_initialized_resume_inputs": list(ROSTER_NEW_INPUTS) if initialized_resume_inputs else [],
        }
        record(log, "parameters", run_id=config.run_id, arena_abi=arena_abi,
               warm_sha256=warm_hash, **initialization,
               config={key: str(value) if isinstance(value, Path) else value
                       for key, value in vars(config).items()})
        try:
            with Batch(config.batch) as batch, Batch(dev_bouts * config.dev_batches) as dev:
                reference = all_dev(dev, model, prior, deadline, legacy=legacy)
                best = dev_score(reference)
                record(log, "initial_dev", run_id=config.run_id, dev=reference, score=best)
                for update_id in range(1, config.updates + 1):
                    check_deadline(deadline)
                    fixtures = schedule.draw(config.batch)
                    if legacy:
                        _, classes, levels, seats = fixtures
                    else:
                        seats = fixtures.seats
                        classes = fixtures.builds
                        levels = np.full_like(fixtures.builds, roster.LEVEL)
                    controllers = league.assignments(seats, rng, config.live_fraction)
                    trace, results, decisions = collect(
                        batch, model, league, fixtures, controllers, deadline, config.repeat_cost,
                    )
                    cumulative.update(decisions)
                    cumulative["bouts"] += config.batch
                    cumulative["unequal_level_bouts"] += int((levels[:, 0] != levels[:, 1]).sum())
                    combo_counts = np.bincount((levels[:, 0] - 1) * 5 + (levels[:, 1] - 1),
                                               minlength=25)
                    cumulative.update({f"seat_level_pair_{i//5+1}_{i%5+1}": int(n)
                                       for i, n in enumerate(combo_counts) if n})
                    reasons = batch.result_reasons()
                    cumulative.update({f"result_{k}": v for k, v in termination(batch, reasons).items()})
                    cumulative["live_vs_live_bouts"] += int(np.all(controllers == -1, axis=1).sum())
                    for seat in (0, 1):
                        played = trace.last[:, seat] >= 0
                        seen.update((int(cls), int(level), seat)
                                    for cls, level in zip(classes[played, seat], levels[played, seat]))
                    lr = config.lr + (config.final_lr - config.lr) * (update_id - 1) / max(1, config.updates - 1)
                    optimizer.param_groups[0]["lr"] = lr
                    draw_rewards = np.where(reasons >= 3, -config.timeout_penalty, 0)
                    stats = update(model, optimizer, trace.finish(
                        results, 0.997, 0.96, draw_rewards=draw_rewards,
                    ), config, deadline)
                    report = {
                        "run_id": config.run_id, "update": update_id,
                        "elapsed_s": round(time.monotonic() - start, 4),
                        "cumulative": dict(cumulative), "coverage_cells": len(seen),
                        "min_games_per_cell": min(seen.values()),
                        "learning_rate": lr, **stats,
                    }
                    due = update_id % config.eval_every == 0 or update_id == config.updates
                    if due:
                        results = all_dev(dev, model, prior, deadline, legacy=legacy)
                        score = dev_score(results)
                        report.update(dev=results, score=score)
                        improved = score > best and len(seen) == full_coverage
                        if improved or config.checkpoint_dir:
                            metadata = {
                                "checkpoint_version": CHECKPOINT_VERSION,
                                "arena_abi": arena_abi,
                                "observation_features": FEATURES,
                                "action_count": ACTIONS,
                                "hidden": model.first.out_features,
                                "classes": list(range(12)),
                                "levels": list(range(1, 6)) if legacy else [roster.LEVEL],
                                "training_seats": [0, 1],
                                "stage": "self_play_ppo" if legacy else "roster_self_play_ppo",
                                "training_run_id": config.run_id,
                                "training_update": update_id,
                                "seed_namespace": f"{domain:016x}",
                                "warm_sha256": warm_hash,
                                "cumulative": dict(cumulative),
                                "coverage_cells": len(seen),
                                "timeout_penalty": config.timeout_penalty,
                                "repeat_cost": config.repeat_cost,
                                "projectile_target_weight": config.projectile_target_weight,
                                "min_games_per_cell": min(seen.values()),
                                "dev": results, "selection_score": score,
                            }
                            if not legacy:
                                metadata.update(
                                    build_ids=list(range(roster.BUILD_COUNT)),
                                    arenas=list(range(schedule.arenas)),
                                    **initialization,
                                )
                        if config.checkpoint_dir:
                            model.export(config.checkpoint_dir / f"update-{update_id:04}.npz",
                                         metadata)
                        if improved:
                            best = score
                            model.export(config.output, metadata)
                            report["saved"] = True
                    record(log, "update", **report)
                    if update_id % config.add_every == 0:
                        league.add(model)
        except TrainingDeadline:
            record(log, "budget_limit", run_id=config.run_id, cumulative=dict(cumulative),
                   coverage_cells=len(seen))


if __name__ == "__main__":
    train(parse_args())
