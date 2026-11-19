"""Continue a learned arena policy with paired, prioritized league PPO."""

import argparse
from collections import Counter
from dataclasses import dataclass
import hashlib
import resource
from pathlib import Path
import sys
import time

import numpy as np
import torch

from arena.native import ROSTER_NEW_INPUTS
from arena.policy import CHECKPOINT_VERSION, Policy
import roster_schedule as roster
from self_play import (BUDGET_SECONDS, League, Schedule, TrainingDeadline,
                       collect, load_model, record, termination, update)
from train_arena import ActorCritic

LEAGUE_DOMAIN = 0x4C47540000000000
# The lower 48 bits carry a 16-bit run ID and 32-bit fixture offset.
ROSTER_LEAGUE_DOMAIN = 0x4C35000000000000


@dataclass
class Member:
    label: str
    model: ActorCritic | Policy
    sha256: str
    bouts: int = 0
    score: float = 0.5


def probabilities(members):
    """Prioritize tough opponents while reserving 30% for uniform sampling."""
    scores = np.array([member.score for member in members])
    hard = (1.0 - scores).clip(min=0.05) ** 2
    return 0.3 / len(members) + 0.7 * hard / hard.sum()


class Matchmaking:
    def __init__(self, anchors, specialists=()):
        self.anchors = tuple(anchors)
        self.specialists = tuple(specialists)
        self.snapshots = []
        self.active = self.anchors

    @property
    def opponents(self):
        return tuple(member.model for member in self.active)

    def assignments(self, seats, rng, live_fraction, *, matched=False, history_rng=None):
        # Keep inference batched even as the archive grows. Include the newest
        # frozen version and sample up to two older versions each rollout.
        older = self.snapshots[:-1]
        archive_rng = rng if history_rng is None else history_rng
        sampled = (
            archive_rng.choice(len(older), size=min(2, len(older)), replace=False,
                               p=probabilities(older)) if older else ()
        )
        history = (*(older[index] for index in sampled), *self.snapshots[-1:])
        self.active = self.anchors + history
        if matched:
            pairs = self.matched_pairs(len(seats) // 2, history, rng, live_fraction)
            self.active += self.specialists
        else:
            if history:
                # Half the frozen fixtures always go to the original reference pool.
                frozen = np.concatenate((0.5 * probabilities(self.anchors),
                                         0.5 * probabilities(history)))
            else:
                frozen = probabilities(self.anchors)
            pairs = rng.choice(len(self.active), size=len(seats) // 2, p=frozen)
            pairs[rng.random(len(pairs)) < live_fraction] = -1
        controllers = np.full((len(seats), 2), -1, dtype=np.int32)
        controllers[np.arange(len(seats)), 1 - seats] = np.repeat(pairs, 2)
        return controllers

    def matched_pairs(self, count, history, rng, live_fraction):
        """Share role and opponent draw streams across the paired experiment."""
        live_count = int(count * live_fraction)
        history_count = (count - live_count) // 2 if history else 0
        _, growing, anchors = np.split(rng.permutation(count),
                                          [live_count, live_count + history_count])
        pairs = np.full(count, -1, dtype=np.int32)
        # Draw common quantiles even when a specialist replaces a historical anchor.
        anchor_weights = probabilities(self.anchors).cumsum()
        anchor_weights[-1] = 1
        pairs[anchors] = np.searchsorted(anchor_weights, rng.random(len(anchors)), side="right")
        if history:
            history_weights = probabilities(history).cumsum()
            history_weights[-1] = 1
            pairs[growing] = len(self.anchors) + np.searchsorted(
                history_weights, rng.random(len(growing)), side="right",
            )
        if self.specialists:
            specialist_count = count // 4
            pairs[anchors[:specialist_count]] = len(self.active) + np.repeat(
                np.arange(len(self.specialists)), specialist_count // len(self.specialists),
            )
        return pairs

    def observe(self, seats, controllers, outcomes):
        frozen = controllers[np.arange(len(seats)), 1 - seats]
        values = (outcomes == seats) + 0.5 * (outcomes == 2)
        for index, member in enumerate(self.active):
            matches = frozen == index
            count = int(matches.sum())
            if count:
                sample = float(values[matches].mean())
                weight = 1.0 - np.exp(-count / 400)
                member.score += float(weight * (sample - member.score))
                member.bouts += count

    def report(self):
        return {member.label: {"bouts": member.bouts, "learner_score_ema": member.score,
                               "sha256": member.sha256}
                for member in (*self.anchors, *self.snapshots, *self.specialists)}


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--start", required=True, type=Path)
    parser.add_argument("--anchor", required=True, action="append", type=Path,
                        help="frozen historical checkpoint, repeatable")
    parser.add_argument("--run-dir", required=True, type=Path,
                        help="new run directory, with optional STOP file after launch")
    parser.add_argument("--run-id", required=True, type=int)
    parser.add_argument("--legacy-schedule", action="store_true",
                        help="reproduce the historical preset and mixed-level schedule")
    parser.add_argument("--updates", type=int, default=2000)
    parser.add_argument("--seconds", type=float, default=BUDGET_SECONDS)
    parser.add_argument("--checkpoint-every", type=int, default=100)
    parser.add_argument("--audit-every", type=int, default=0,
                        help="write extra compute-audit checkpoints without adding opponents")
    parser.add_argument("--batch", type=int, default=384)
    parser.add_argument("--minibatch", type=int, default=768)
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--lr", type=float, default=8e-5)
    parser.add_argument("--final-lr", type=float, default=4e-5)
    parser.add_argument("--decay-updates", type=int, default=600)
    parser.add_argument("--entropy", type=float, default=0.012)
    parser.add_argument("--repeat-cost", type=float, default=0.02)
    parser.add_argument("--timeout-penalty", type=float, default=0.35)
    parser.add_argument("--projectile-target-weight", type=float, default=0.04)
    parser.add_argument("--live-fraction", type=float, default=0.25)
    parser.add_argument("--matched", action="store_true",
                        help="use shared experiment role streams with exact allocation")
    parser.add_argument("--specialist", action="append", type=Path, default=[],
                        help="two frozen specialists, each receiving 12.5%% of all duels")
    config = parser.parse_args()
    if (min(config.updates, config.checkpoint_every, config.batch, config.minibatch,
            config.epochs, config.threads, config.decay_updates) <= 0
            or config.audit_every < 0 or config.batch % 2 or not 0 <= config.run_id < 65536
            or not 0 < config.seconds <= BUDGET_SECONDS
            or not 0 < config.final_lr <= config.lr or not 0 <= config.entropy < 1
            or not 0 < config.live_fraction < 1
            or not all(0 <= value < 1 for value in (
                config.repeat_cost, config.timeout_penalty, config.projectile_target_weight))):
        parser.error("invalid league configuration")
    if config.matched and (config.batch % 16 or config.live_fraction != 0.25):
        parser.error("matched runs require batches divisible by 16 and 25% live fixtures")
    if config.specialist and (not config.matched or len(config.specialist) != 2):
        parser.error("specialist allocation requires exactly two checkpoints and --matched")
    return config


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cpu_seconds():
    usage = resource.getrusage(resource.RUSAGE_SELF)
    return usage.ru_utime + usage.ru_stime


def train(config):
    from arena.native import ACTIONS, FEATURES, Batch, _lib

    torch.set_num_threads(config.threads)
    torch.set_num_interop_threads(1)
    torch.manual_seed(config.run_id)
    legacy = config.legacy_schedule
    namespace = LEAGUE_DOMAIN if legacy else ROSTER_LEAGUE_DOMAIN
    domain = namespace + (config.run_id << 32)
    rng = np.random.default_rng(domain)
    abi = _lib().arena_schema()
    if config.projectile_target_weight and abi != 2002:
        raise ValueError("projectile target loss requires arena ABI 2002")

    initialized_members = []

    def load_member(label, path, *, deployed=False):
        if deployed:
            member = Policy.load(path).require_arena(abi, FEATURES, ACTIONS)
            initialized = not legacy and "build_ids" not in member.metadata
            if initialized:
                member.weights["w1"][list(ROSTER_NEW_INPUTS)] = 0
        else:
            member, initialized = load_model(path, abi, FEATURES, ACTIONS, roster_run=not legacy)
            member.eval().requires_grad_(False)
        if initialized:
            initialized_members.append(label)
        return Member(label, member, sha256(path))

    anchors = [load_member("initial", config.start)]
    anchors.extend(load_member(f"anchor-{i}-{path.stem}", path)
                   for i, path in enumerate(config.anchor, 1))
    matched = getattr(config, "matched", False)
    specialist_paths = getattr(config, "specialist", [])
    specialists = [load_member(f"specialist-{i}-{path.stem}", path, deployed=True)
                   for i, path in enumerate(specialist_paths, 1)]
    all_members = anchors + specialists
    if len({member.sha256 for member in all_members}) != len(all_members):
        raise ValueError("league must contain distinct checkpoints")
    league = Matchmaking(anchors, specialists)
    model = League.freeze(anchors[0].model).requires_grad_(True)
    optimizer = torch.optim.Adam(model.parameters(), lr=config.lr, eps=1e-5)
    schedule_type = Schedule if legacy else roster.RosterSchedule
    schedule = schedule_type(domain, training=True)
    roster_metadata = {} if legacy else {
        "build_ids": list(range(roster.BUILD_COUNT)),
        "arenas": list(range(schedule.arenas)),
        "hidden": model.first.out_features,
        "neutralized_roster_inputs": list(ROSTER_NEW_INPUTS) if initialized_members else [],
        "neutralized_league_members": initialized_members,
    }
    cumulative = Counter()
    seen = Counter()
    checkpoint_root = config.run_dir / "checkpoints"
    config.run_dir.mkdir(parents=True, exist_ok=False)
    checkpoint_root.mkdir()
    audit_every = getattr(config, "audit_every", 0)
    audit_root = config.run_dir / "compute-audit"
    if audit_every:
        audit_root.mkdir()
    started = time.monotonic()
    cpu_started = cpu_seconds()
    native_sha256 = sha256(Path(_lib()._name))
    deadline = started + config.seconds

    with (config.run_dir / "training.jsonl").open("x") as log:
        record(log, "start", config=vars(config) | {
            "start": str(config.start), "anchor": [str(path) for path in config.anchor],
            "run_dir": str(config.run_dir), "specialist": [str(path) for path in specialist_paths],
        }, arena_abi=abi, torch_version=torch.__version__, numpy_version=np.__version__,
               python_version=sys.version.split()[0], seed_namespace=f"{domain:016x}",
               native_sha256=native_sha256,
               specialist_inference="greedy" if specialists else None, league=league.report(),
               **roster_metadata)
        last_update = saved_update = 0
        stop_reason = "update_limit"

        def save_checkpoint(update_id, *, join_league, audit=False):
            label = f"update-{update_id:04}"
            path = (audit_root if audit else checkpoint_root) / (label + ".npz")
            model.export(path, {
                "checkpoint_version": CHECKPOINT_VERSION,
                "arena_abi": abi,
                "observation_features": FEATURES,
                "action_count": ACTIONS,
                "classes": list(range(12)),
                "levels": list(range(1, 6)) if legacy else [roster.LEVEL],
                "training_seats": [0, 1],
                "stage": "league_self_play_ppo" if legacy else "roster_league_self_play_ppo",
                "training_run_id": config.run_id,
                "training_update": update_id,
                "seed_namespace": f"{domain:016x}",
                "warm_sha256": anchors[0].sha256,
                "cumulative": dict(cumulative),
                "coverage_cells": len(seen),
                "min_games_per_cell": min(seen.values()),
                "repeat_cost": config.repeat_cost,
                "timeout_penalty": config.timeout_penalty,
                "projectile_target_weight": config.projectile_target_weight,
                "specialist_inference": "greedy" if specialists else None,
                "league": league.report(),
                **roster_metadata,
            })
            digest = sha256(path)
            if join_league:
                league.snapshots.append(Member(label, League.freeze(model), digest))
            record(log, "audit_checkpoint" if audit else "checkpoint",
                   update=update_id, path=str(path), sha256=digest,
                   elapsed_s=round(time.monotonic() - started, 4),
                   cpu_s=round(cpu_seconds() - cpu_started, 4))

        try:
            with Batch(config.batch) as batch:
                for update_id in range(1, config.updates + 1):
                    if (config.run_dir / "STOP").exists():
                        stop_reason = "stop_file"
                        break
                    fixtures = schedule.draw(config.batch)
                    if legacy:
                        _, classes, levels, seats = fixtures
                    else:
                        seats = fixtures.seats
                        classes = fixtures.builds
                        levels = np.full_like(fixtures.builds, roster.LEVEL)
                    if matched:
                        torch.manual_seed(domain + 2 * update_id)
                    controllers = league.assignments(seats, rng, config.live_fraction, matched=matched)
                    trace, outcomes, decisions = collect(
                        batch, model, league, fixtures, controllers, deadline, config.repeat_cost,
                    )
                    reasons = batch.result_reasons()
                    lr = np.interp(update_id, [0, config.decay_updates],
                                   [config.lr, config.final_lr])
                    optimizer.param_groups[0]["lr"] = float(lr)
                    if matched:
                        torch.manual_seed(domain + 2 * update_id + 1)
                    stats = update(model, optimizer, trace.finish(
                        outcomes, 0.997, 0.96,
                        draw_rewards=np.where(reasons >= 3, -config.timeout_penalty, 0),
                    ), config, deadline)
                    last_update = update_id
                    league.observe(seats, controllers, outcomes)
                    cumulative.update(decisions)
                    cumulative["bouts"] += config.batch
                    cumulative["optimizer_minibatches"] += (
                        config.epochs * ((trace.size + config.minibatch - 1) // config.minibatch)
                    )
                    cumulative["unequal_level_bouts"] += int((levels[:, 0] != levels[:, 1]).sum())
                    cumulative["live_vs_live_bouts"] += int(np.all(controllers == -1, axis=1).sum())
                    cumulative.update({f"result_{key}": count
                                       for key, count in termination(batch, reasons).items()})
                    for seat in (0, 1):
                        played = trace.last[:, seat] >= 0
                        seen.update((int(cls), int(level), seat)
                                    for cls, level in zip(classes[played, seat], levels[played, seat]))
                    record(log, "update", update=update_id, learning_rate=float(lr), **stats,
                           elapsed_s=round(time.monotonic() - started, 4),
                           cpu_s=round(cpu_seconds() - cpu_started, 4), league=league.report(),
                           optimizer_minibatches=cumulative["optimizer_minibatches"],
                           bouts=cumulative["bouts"], live_vs_live=cumulative["live_vs_live_bouts"],
                           learner_decisions=cumulative["learner"],
                           frozen_decisions=cumulative["snapshot"],
                           coverage_cells=len(seen), min_games_per_cell=min(seen.values()))
                    if update_id % config.checkpoint_every == 0:
                        save_checkpoint(update_id, join_league=True)
                        saved_update = update_id
                    elif audit_every and update_id % audit_every == 0:
                        save_checkpoint(update_id, join_league=False, audit=True)
            if last_update > saved_update:
                save_checkpoint(last_update, join_league=False)
                saved_update = last_update
        except TrainingDeadline:
            # The final optimizer update can be interrupted mid-minibatch.
            # Leave previously published, complete checkpoints as-is.
            stop_reason = "time_limit"
        record(log, "stop", reason=stop_reason, last_update=last_update,
               last_checkpoint=saved_update, elapsed_s=round(time.monotonic() - started, 4),
               cpu_s=round(cpu_seconds() - cpu_started, 4),
               cumulative=dict(cumulative), league=league.report(), coverage_cells=len(seen),
               min_games_per_cell=min(seen.values(), default=0))


if __name__ == "__main__":
    train(parse_args())
