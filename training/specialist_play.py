"""Train a PPO exploiter against the frozen, deployed champion."""

import argparse
from collections import Counter
import hashlib
from itertools import product
from pathlib import Path
import time

import numpy as np
import torch

from arena.native import ACTIONS, ARENA_ABI, FEATURES, Batch
from arena.policy import CHECKPOINT_VERSION, Policy
from self_play import EpisodeTrace, RepeatedChoices, record, termination, update
from train_arena import ActorCritic, sample_actions

CHAMPION_SHA = "96f9bddabc0eeff4a70c111db443dea064490c6e576ff8bb569966024cb26e31"
TRAIN = int.from_bytes(b"X1T", "big") << 40
DEV = int.from_bytes(b"X1D", "big") << 40
MASK64 = (1 << 64) - 1
CELLS = np.array(list(product(range(12), range(12), range(1, 6), range(4))),
                 dtype=np.int32)


def layout(seed):
    """Map index from the native SplitMix64 finalizer."""
    seed = ((seed ^ (seed >> 30)) * 0xbf58476d1ce4e5b9) & MASK64
    seed = ((seed ^ (seed >> 27)) * 0x94d049bb133111eb) & MASK64
    return (seed ^ (seed >> 31)) % 4


class Fixtures:
    def __init__(self, seed, training):
        self.next_seed = seed
        self.training = training
        self.rng = np.random.default_rng(seed)
        self.cells = self.rng.permutation(CELLS) if training else CELLS
        self.cursor = 0

    def draw(self, bouts):
        if bouts <= 0 or bouts % 2 or bouts // 2 > len(CELLS):
            raise ValueError("fixture batch must contain 1–2880 whole pairs")
        count = bouts // 2
        first = self.cells[self.cursor:self.cursor + count]
        self.cursor += len(first)
        if self.cursor == len(CELLS):
            self.cells = self.rng.permutation(CELLS) if self.training else CELLS
            self.cursor = count - len(first)
            first = np.concatenate((first, self.cells[:self.cursor]))
        seeds = np.empty(count, np.uint64)
        for i, wanted in enumerate(first[:, 3]):
            while layout(self.next_seed) != wanted:
                self.next_seed += 1
            seeds[i] = self.next_seed
            self.next_seed += 1
        classes = np.repeat(first[:, :2], 2, axis=0)
        classes[1::2] = classes[1::2, ::-1]
        levels = np.repeat(first[:, 2], 2)
        seats = np.tile([0, 1], count)
        return np.repeat(seeds, 2), classes, np.repeat(levels[:, None], 2, axis=1), seats


@torch.no_grad()
def run_bouts(batch, fixtures, learner, champion, repeat_cost=0.0):
    """Route each observed decision, including reactions, by its acting seat."""
    seeds, classes, levels, seats = fixtures
    lanes = np.arange(batch.n, dtype=np.int32)
    training = isinstance(learner, ActorCritic)
    trace = EpisodeTrace(batch.n) if training else None
    repeats = RepeatedChoices(batch.n) if repeat_cost else None
    counts = Counter()
    main_actions = np.zeros(ACTIONS, dtype=np.int64)
    batch.reset(lanes, seeds, classes, levels)
    for step in range(2049):
        x, masks, actors, outcomes = batch.observe()
        active = actors >= 0
        if not active.any():
            counts["bouts"] = batch.n
            counts.update(termination(batch))
            counts["focal_win"] = int((outcomes == seats).sum())
            counts["focal_draw"] = int((outcomes == 2).sum())
            return trace, outcomes, counts, main_actions
        if step == 2048:
            raise RuntimeError("native arena exceeded its maximum decision count")
        actions = np.full(batch.n, -1, dtype=np.int32)
        focal = lanes[active & (actors == seats)]
        other = lanes[active & (actors != seats)]
        counts["learner_decisions"] += len(focal)
        counts["champion_decisions"] += len(other)
        if len(other):
            actions[other] = champion.choose(x[other], masks[other])
        if len(focal):
            features, legal = x[focal], masks[focal]
            if training:
                logits, values = learner(torch.from_numpy(features), torch.from_numpy(legal))
                selected = sample_actions(logits)
                chosen = selected.numpy()
                rewards = None
                if repeats is not None:
                    repeated = repeats.check(focal, actors[focal], features, chosen)
                    counts["same_turn_repeat"] += int(repeated.sum())
                    rewards = repeated.astype(np.float32) * -repeat_cost
                log_probs = logits.log_softmax(1).gather(1, selected[:, None]).squeeze(1)
                trace.append(focal, actors[focal], features, legal, chosen,
                             log_probs.numpy(), values.numpy(), rewards)
            else:
                chosen = learner.choose(features, legal)
                main = features[:, 256] == 1
                main_actions += np.bincount(chosen[main], minlength=ACTIONS)
            actions[focal] = chosen
        batch.step(actions)
    raise AssertionError("unreachable")


def evaluate(batch, learner, champion):
    fixtures = Fixtures(DEV, training=False)
    totals = Counter()
    main_choices = np.zeros(ACTIONS, dtype=np.int64)
    class_scores = np.zeros((12, 3), dtype=np.int64)
    layouts = np.zeros((4, 3), dtype=np.int64)
    lanes = np.arange(batch.n)
    for _ in range(len(CELLS) * 2 // batch.n):
        pairs = fixtures.draw(batch.n)
        _, outcomes, counts, actions = run_bouts(batch, pairs, learner, champion)
        seeds, classes, _, seats = pairs
        wins, draws = outcomes == seats, outcomes == 2
        labels = np.where(wins, 0, np.where(draws, 1, 2))
        np.add.at(class_scores, (classes[lanes, seats], labels), 1)
        np.add.at(layouts, ([layout(int(seed)) for seed in seeds], labels), 1)
        totals.update(counts)
        main_choices += actions
    return dict(totals, score=(totals["focal_win"] + 0.5 * totals["focal_draw"]) / totals["bouts"],
                class_wdl=class_scores[:, [0, 2, 1]].tolist(),
                map_wdl=layouts[:, [0, 2, 1]].tolist(), main_actions=main_choices.tolist())


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def train(config):
    started, cpu_started = time.monotonic(), time.process_time()
    torch.set_num_threads(config.threads)
    torch.set_num_interop_threads(1)
    torch.manual_seed(config.run_id)
    champion_hash = sha256(config.champion)
    if champion_hash != CHAMPION_SHA:
        raise ValueError("the frozen update-1300 champion hash does not match")
    champion = Policy.load(config.champion).require_arena(ARENA_ABI, FEATURES, ACTIONS)
    initial = Policy.load(config.start).require_arena(ARENA_ABI, FEATURES, ACTIONS)
    initial_hash = sha256(config.start)
    model = ActorCritic(FEATURES, ACTIONS)
    model.load(config.start)
    optimizer = torch.optim.Adam(model.parameters(), lr=config.lr, eps=1e-5)
    seed = TRAIN + (config.run_id << 24)
    fixtures = Fixtures(seed, training=True)
    config.run_dir.mkdir(parents=True, exist_ok=False)
    metrics, eval_metrics = Counter(), Counter()
    train_wall = train_cpu = eval_wall = eval_cpu = 0.0
    candidates = []

    with (config.run_dir / "training.jsonl").open("x") as log, Batch(config.batch) as batch, Batch(384) as dev:
        record(log, "start", train_seed=f"{seed:016x}", dev_seed=f"{DEV:016x}",
               initial_sha256=initial_hash, champion_sha256=champion_hash,
               config={k: str(v) if isinstance(v, Path) else v for k, v in vars(config).items()})

        def run_dev(path, update_id, candidate):
            nonlocal eval_wall, eval_cpu
            before, before_cpu = time.monotonic(), time.process_time()
            result = evaluate(dev, candidate, champion)
            eval_wall += time.monotonic() - before
            eval_cpu += time.process_time() - before_cpu
            for key in ("bouts", "learner_decisions", "champion_decisions"):
                eval_metrics[key] += result[key]
            checkpoint_hash = sha256(path)
            record(log, "dev", update=update_id, path=str(path), sha256=checkpoint_hash, **result)
            return (result["score"], -update_id, path, checkpoint_hash)

        run_dev(config.start, 0, initial)
        for update_id in range(1, config.updates + 1):
            before, before_cpu = time.monotonic(), time.process_time()
            draw = fixtures.draw(config.batch)
            trace, outcomes, counts, _ = run_bouts(batch, draw, model, champion, config.repeat_cost)
            metrics.update(counts)
            metrics["learner_west_bouts"] += int((draw[3] == 0).sum())
            metrics["learner_east_bouts"] += int((draw[3] == 1).sum())
            metrics["minibatches"] += config.epochs * ((trace.size + config.minibatch - 1) // config.minibatch)
            optimizer.param_groups[0]["lr"] = config.lr + (config.final_lr - config.lr) * (
                (update_id - 1) / max(1, config.updates - 1))
            penalties = np.where(batch.result_reasons() >= 3, -config.timeout_penalty, 0)
            stats = update(model, optimizer, trace.finish(
                outcomes, 0.997, 0.96, draw_rewards=penalties,
            ), config, float("inf"))
            train_wall += time.monotonic() - before
            train_cpu += time.process_time() - before_cpu
            if update_id % config.checkpoint_every == 0 or update_id == config.updates:
                path = config.run_dir / f"update-{update_id:04}.npz"
                model.export(path, {
                    "checkpoint_version": CHECKPOINT_VERSION, "arena_abi": ARENA_ABI,
                    "observation_features": FEATURES, "action_count": ACTIONS,
                    "stage": "frozen_champion_ppo", "training_update": update_id,
                    "training_run_id": config.run_id, "initial_sha256": initial_hash,
                    "opponent_sha256": champion_hash, "train_seed": f"{seed:016x}",
                    "opponent_action_selection": "deployed_greedy",
                    "training_bouts": metrics["bouts"],
                })
                path.chmod(0o444)
                candidates.append(run_dev(path, update_id, Policy.load(path)))
            if update_id % 10 == 0 or update_id == config.updates:
                record(log, "progress", update=update_id, train_wall_s=train_wall,
                       train_cpu_s=train_cpu, totals=dict(metrics), **stats)
        best_score, negative_update, best_path, best_hash = max(candidates)
        if sha256(config.champion) != champion_hash or sha256(config.start) != initial_hash:
            raise RuntimeError("an input checkpoint changed during training")
        record(log, "complete", selected_update=-negative_update, selected_path=str(best_path),
               selected_sha256=best_hash, dev_score=best_score, updates=config.updates,
               elapsed_s=time.monotonic() - started, cpu_s=time.process_time() - cpu_started,
               train_wall_s=train_wall, train_cpu_s=train_cpu, dev_wall_s=eval_wall,
               dev_cpu_s=eval_cpu, totals=dict(metrics), evaluation=dict(eval_metrics))


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--champion", type=Path, required=True)
    parser.add_argument("--start", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--run-id", type=int, required=True)
    parser.add_argument("--updates", type=int, default=200)
    parser.add_argument("--checkpoint-every", type=int, default=50)
    parser.add_argument("--batch", type=int, default=384)
    parser.add_argument("--minibatch", type=int, default=768)
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--lr", type=float, default=8e-5)
    parser.add_argument("--final-lr", type=float, default=4e-5)
    parser.add_argument("--entropy", type=float, default=0.012)
    parser.add_argument("--repeat-cost", type=float, default=0.02)
    parser.add_argument("--timeout-penalty", type=float, default=0.35)
    parser.add_argument("--projectile-target-weight", type=float, default=0.04)
    args = parser.parse_args()
    if (not 0 <= args.run_id < 65536 or not 0 < args.batch <= 5760 or args.batch % 2
            or min(args.updates, args.checkpoint_every, args.epochs, args.threads, args.minibatch) < 1
            or args.updates * args.batch >= 1 << 20 or not 0 < args.final_lr <= args.lr
            or not all(0 <= value < 1 for value in (
                args.entropy, args.repeat_cost, args.timeout_penalty, args.projectile_target_weight))):
        parser.error("invalid specialist configuration")
    return args


if __name__ == "__main__":
    train(parse_args())
