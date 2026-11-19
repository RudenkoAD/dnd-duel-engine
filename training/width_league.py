"""Train one ordinary-league arm with a process CPU budget."""

import argparse
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
import time

import numpy as np
import torch

from arena.native import ACTIONS, FEATURES, Batch, _lib
from arena.policy import CHECKPOINT_VERSION, Policy
from league_play import Matchmaking, Member, cpu_seconds, sha256
from self_play import (League, Schedule, TrainingDeadline, check_deadline,
                       collect, record, termination, update)
from train_arena import ActorCritic

DOMAIN = 0x574C470000000000
AUDIT_STEPS = 96
GROWTH_INTERVAL = 8
SELECTION_INTERVAL = 24


@dataclass
class CompletedUpdate:
    model: ActorCritic
    receipt: dict
    league: dict


def eligible_candidate(previous, current, threshold):
    """Use the latest complete update at or below a CPU boundary."""
    if current is not None and current.receipt["cpu_s"] <= threshold:
        return current
    if previous is not None and previous.receipt["cpu_s"] <= threshold:
        return previous
    raise ValueError("CPU boundary reached before a complete training update")


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--warm", type=Path, required=True)
    parser.add_argument("--anchor", type=Path, action="append", required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--audit-dir", type=Path, required=True)
    parser.add_argument("--run-id", type=int, required=True)
    parser.add_argument("--cpu-cap", type=float, default=1576.0)
    parser.add_argument("--init-cpu", type=float, default=0.0)
    parser.add_argument("--hidden", type=int, choices=(128, 256), required=True)
    config = parser.parse_args()
    if (len(config.anchor) != 4 or not 0 <= config.run_id < 65536
            or not 0 <= config.init_cpu < config.cpu_cap / 4
            or not np.isfinite(config.cpu_cap)):
        parser.error("provide four anchors, a valid run ID, and a finite CPU cap")
    return config


def train(config):
    config.batch = 384
    config.minibatch = 768
    config.epochs = 3
    config.threads = 2
    config.lr = 8e-5
    config.final_lr = 4e-5
    config.lr_decay_fraction = 0.5
    config.entropy = 0.012
    config.repeat_cost = 0.02
    config.timeout_penalty = 0.35
    config.projectile_target_weight = 0.04
    config.live_fraction = 0.25
    config.gamma = 0.997
    config.gae_lambda = 0.96

    torch.set_num_threads(config.threads)
    torch.set_num_interop_threads(1)
    torch.manual_seed(config.run_id)
    domain = DOMAIN + (config.run_id << 32)
    abi = _lib().arena_schema()
    native_sha256 = sha256(Path(_lib()._name))
    if abi != 2002:
        raise ValueError("projectile target loss requires arena ABI 2002")

    def load_model(path, expected_width):
        Policy.load(path).require_arena(abi, FEATURES, ACTIONS)
        with np.load(path, allow_pickle=False) as archive:
            actual_width = archive["b1"].size
        if actual_width != expected_width:
            raise ValueError(f"{path} has width {actual_width}, expected {expected_width}")
        model = ActorCritic(FEATURES, ACTIONS, hidden=actual_width)
        model.load(path)
        return model

    warm_hash = sha256(config.warm)
    anchors = [Member(f"anchor-{i}-{path.stem}",
                      load_model(path, 128).eval().requires_grad_(False), sha256(path))
               for i, path in enumerate(config.anchor)]
    if len({member.sha256 for member in anchors}) != 4:
        raise ValueError("provide four distinct anchors")
    league = Matchmaking(anchors)
    model = load_model(config.warm, config.hidden)
    optimizer = torch.optim.Adam(model.parameters(), lr=config.lr, eps=1e-5)
    schedule = Schedule(domain, training=True)
    cumulative = Counter()
    seen = Counter()
    checkpoint_root = config.run_dir / "checkpoints"
    checkpoint_root.mkdir(parents=True, exist_ok=False)
    config.audit_dir.mkdir(parents=True, exist_ok=False)
    boundary = 1
    previous = current = None

    with (config.run_dir / "training.jsonl").open("x") as log, Batch(config.batch) as batch:
        record(log, "start", config=vars(config) | {
            "warm": str(config.warm), "anchor": [str(path) for path in config.anchor],
            "run_dir": str(config.run_dir), "audit_dir": str(config.audit_dir),
        }, seed_namespace=f"{domain:016x}", native_sha256=native_sha256, arena_abi=abi,
               torch_version=torch.__version__, numpy_version=np.__version__,
               warm_sha256=warm_hash, league=league.report(), audit_steps=AUDIT_STEPS,
               growth_interval=GROWTH_INTERVAL, selection_interval=SELECTION_INTERVAL)
        wall_start = time.monotonic()
        cpu_start = cpu_seconds()

        def charged_cpu():
            return config.init_cpu + cpu_seconds() - cpu_start

        def reached_cpu_cap():
            return charged_cpu() >= config.cpu_cap

        def candidate(boundary, completed):
            fraction = boundary / AUDIT_STEPS
            selection = boundary % SELECTION_INTERVAL == 0
            label = f"cpu-{round(100 * fraction):03d}" if selection else f"audit-{boundary:03d}"
            path = (checkpoint_root if selection else config.audit_dir) / (label + ".npz")
            completed.model.export(path, {
                "checkpoint_version": CHECKPOINT_VERSION, "arena_abi": abi,
                "observation_features": FEATURES, "action_count": ACTIONS,
                "classes": list(range(12)), "levels": list(range(1, 6)),
                "training_seats": [0, 1], "stage": "width_league_self_play_ppo",
                "hidden": config.hidden, "training_run_id": config.run_id,
                "training_update": completed.receipt["update"], "budget_fraction": fraction,
                "cpu_s": completed.receipt["cpu_s"], "cpu_cap": config.cpu_cap,
                "seed_namespace": f"{domain:016x}", "warm_sha256": warm_hash,
                "league": completed.league, "native_sha256": native_sha256,
            })
            digest = sha256(path)
            join_league = boundary % GROWTH_INTERVAL == 0 and boundary < AUDIT_STEPS
            if join_league:
                league.snapshots.append(Member(label, completed.model, digest))
            record(log, "checkpoint" if selection else "audit_checkpoint", path=str(path),
                   sha256=digest, budget_fraction=fraction, join_league=join_league,
                   **completed.receipt)

        update_id = 0
        stop_reason = "cpu_cap"
        interrupted_stage = None
        while boundary <= AUDIT_STEPS:
            threshold = boundary / AUDIT_STEPS * config.cpu_cap
            if charged_cpu() >= threshold:
                if boundary == AUDIT_STEPS:
                    break
                candidate(boundary, eligible_candidate(previous, current, threshold))
                boundary += 1
                continue
            if (config.run_dir / "STOP").exists():
                stop_reason = "stop_file"
                break
            update_id += 1
            try:
                interrupted_stage = "rollout"
                check_deadline(reached_cpu_cap)
                fixtures = schedule.draw(config.batch)
                _, classes, levels, seats = fixtures
                torch.manual_seed(domain + 2 * update_id)
                matchup_rng = np.random.default_rng([domain, update_id, 0])
                history_rng = np.random.default_rng([domain, update_id, 1])
                controllers = league.assignments(seats, matchup_rng, config.live_fraction,
                                                 matched=True, history_rng=history_rng)
                trace, outcomes, decisions = collect(
                    batch, model, league, fixtures, controllers, reached_cpu_cap, config.repeat_cost,
                )
                reasons = batch.result_reasons()
                lr = np.interp(charged_cpu() / config.cpu_cap, [0, config.lr_decay_fraction],
                               [config.lr, config.final_lr])
                optimizer.param_groups[0]["lr"] = float(lr)
                interrupted_stage = "ppo"
                torch.manual_seed(domain + 2 * update_id + 1)
                stats = update(model, optimizer, trace.finish(
                    outcomes, config.gamma, config.gae_lambda,
                    draw_rewards=np.where(reasons >= 3, -config.timeout_penalty, 0),
                ), config, reached_cpu_cap)
            except TrainingDeadline:
                continue

            interrupted_stage = None
            league.observe(seats, controllers, outcomes)
            cumulative.update(decisions)
            cumulative["bouts"] += config.batch
            cumulative["optimizer_minibatches"] += config.epochs * (
                (trace.size + config.minibatch - 1) // config.minibatch
            )
            cumulative["live_vs_live_bouts"] += int(np.all(controllers == -1, axis=1).sum())
            cumulative["unequal_level_bouts"] += int((levels[:, 0] != levels[:, 1]).sum())
            cumulative.update({f"result_{key}": count
                               for key, count in termination(batch, reasons).items()})
            for seat in (0, 1):
                played = trace.last[:, seat] >= 0
                seen.update((int(cls), int(level), seat)
                            for cls, level in zip(classes[played, seat], levels[played, seat]))
            frozen = League.freeze(model)
            member_report = league.report()
            receipt = {
                "update": update_id, "cpu_s": charged_cpu(),
                "elapsed_s": time.monotonic() - wall_start,
                "bouts": cumulative["bouts"], "learner_decisions": cumulative["learner"],
                "frozen_decisions": cumulative["snapshot"],
                "optimizer_minibatches": cumulative["optimizer_minibatches"],
                "live_vs_live_bouts": cumulative["live_vs_live_bouts"],
                "unequal_level_bouts": cumulative["unequal_level_bouts"],
                "coverage_cells": len(seen), "min_games_per_cell": min(seen.values()),
            }
            previous, current = current, CompletedUpdate(frozen, receipt, member_report)
            record(log, "update", learning_rate=float(lr), **receipt, **stats)
        stopped_cpu = charged_cpu()
        stopped_wall = time.monotonic() - wall_start
        if stop_reason == "cpu_cap":
            final = eligible_candidate(previous, current, config.cpu_cap)
            candidate(AUDIT_STEPS, final)
        else:
            final = current
        record(log, "stop", reason=stop_reason, last_completed=final.receipt if final else None,
               cpu_s=stopped_cpu, elapsed_s=stopped_wall, attempt_update=update_id,
               interrupted_stage=interrupted_stage,
               overshoot_cpu_s=max(0, stopped_cpu - config.cpu_cap),
               postprocessing_cpu_s=charged_cpu() - stopped_cpu)


if __name__ == "__main__":
    train(parse_args())
