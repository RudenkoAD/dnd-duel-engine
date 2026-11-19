"""Continue the frozen 256-wide league to exact duel-count milestones."""

import argparse
from collections import Counter
import gzip
import json
from pathlib import Path
from types import SimpleNamespace
import time

import numpy as np
import torch

from arena.native import ACTIONS, FEATURES, Batch, _lib
from arena.policy import CHECKPOINT_VERSION, Policy
from league_play import Matchmaking, Member, cpu_seconds, sha256
from self_play import Schedule, collect, record, termination, update
from train_arena import ActorCritic

DOMAIN = 0x4232540000000000
BASE = 757
MILESTONES = (967, 1934, 2901)
GROWTH_INTERVAL = 64
WARM_SHA = "1c8cfe14f7a18f5455e022dfcc27376ecb273ae276a1a493914680c4d7fc51b5"
NATIVE_SHA = "621964e798659ba534fab6c8494664c8e7f9d91abb9a4e28ccb0ed9f40888e3f"
PREVIOUS = Path("training/runs/width-league")
WIDE = PREVIOUS / "wide"
ANCHORS = (
    Path("training/runs/league/checkpoints/update-1300.npz"),
    Path("models/arena_policy.npz"),
    Path("training/runs/viewer_relative/control.npz"),
    Path("training/runs/viewer_relative/target-aware-update-0090.npz"),
)
RECIPE = dict(batch=384, minibatch=768, epochs=3, threads=2, lr=4e-5,
              entropy=0.012, repeat_cost=0.02, timeout_penalty=0.35,
              projectile_target_weight=0.04, live_fraction=0.25,
              gamma=0.997, gae_lambda=0.96)


def verified_model(path, digest, width):
    if sha256(path) != digest:
        raise ValueError(f"Checkpoint SHA-256 mismatch: {path}")
    policy = Policy.load(path)
    policy.require_arena(2002, FEATURES, ACTIONS)
    if policy.weights["w1"].shape[1] != width:
        raise ValueError(f"Expected width {width}: {path}")
    return ActorCritic.from_checkpoint(path), policy.metadata


def original_league(archive):
    warm = WIDE / "checkpoints/cpu-100.npz"
    model, metadata = verified_model(warm, WARM_SHA, 256)
    with gzip.open(WIDE / "training.jsonl.gz", "rt") as handle:
        events = [json.loads(line) for line in handle]
    base = events[-1]["last_completed"]
    if base["update"] != BASE or base["bouts"] != BASE * RECIPE["batch"]:
        raise ValueError("Unexpected origin update or duel count")
    prior = metadata["league"]

    def member(label, path, width, digest):
        counters = prior[label]
        if counters["sha256"] != digest:
            raise ValueError(f"League digest mismatch: {label}")
        frozen, _ = verified_model(path, digest, width)
        return Member(label, frozen.eval().requires_grad_(False), digest,
                      counters["bouts"], counters["learner_score_ema"])

    league = Matchmaking([
        member(label, path, 128, prior[label]["sha256"])
        for i, path in enumerate(ANCHORS) for label in (f"anchor-{i}-{path.stem}",)
    ])
    for event in events:
        if not event.get("join_league"):
            continue
        source = Path(event["path"])
        path = WIDE / "checkpoints" / source.name if source.stem.startswith("cpu-") else archive / source.name
        league.snapshots.append(member(source.stem, path, 256, event["sha256"]))
    if league.report() != prior or len(league.snapshots) != 11:
        raise ValueError("Incomplete restoration of the original wide league")
    return model, league, base


def save_json(path, data):
    temp = path.with_suffix(".tmp")
    temp.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    temp.replace(path)


def metadata_for(receipt, league, native_sha):
    return dict(checkpoint_version=CHECKPOINT_VERSION, arena_abi=2002,
                observation_features=FEATURES, action_count=ACTIONS, classes=list(range(12)),
                levels=list(range(1, 6)), training_seats=[0, 1], hidden=256,
                stage="long_width_league_self_play_ppo", training_update=receipt["update"],
                seed_namespace=f"{DOMAIN:016x}", warm_sha256=WARM_SHA,
                native_sha256=native_sha, league=league.report(), **receipt)


def publish(root, model, league, receipt, stats, log, native_sha):
    update_id = receipt["update"]
    if update_id % GROWTH_INTERVAL == 0:
        label = f"update-{update_id:04d}"
        path = root / "history" / f"{label}.npz"
        if not path.exists():
            model.export(path, metadata_for(receipt, league, native_sha))
        frozen, _ = verified_model(path, sha256(path), 256)
        league.snapshots.append(Member(label, frozen.eval().requires_grad_(False), sha256(path)))
    if update_id in MILESTONES:
        path = root / "checkpoints" / f"update-{update_id:04d}.npz"
        if not path.exists():
            model.export(path, metadata_for(receipt, league, native_sha))
        ready = path.with_suffix(".json")
        if not ready.exists():
            save_json(ready, receipt | {"path": str(path), "sha256": sha256(path)})
        record(log, "checkpoint", **json.loads(ready.read_text()))
    record(log, "update", learning_rate=RECIPE["lr"], **receipt, **stats)


def train(root, archive, resume, expected_native_sha=NATIVE_SHA):
    config = SimpleNamespace(**RECIPE)
    torch.set_num_threads(config.threads)
    torch.set_num_interop_threads(1)
    native = _lib()
    native_sha = sha256(Path(native._name))
    if native.arena_schema() != 2002 or native_sha != expected_native_sha:
        raise ValueError(f"Unexpected native binary: {native._name} ({native_sha})")
    model, league, base = original_league(archive)
    optimizer = torch.optim.Adam(model.parameters(), lr=config.lr, eps=1e-5)
    schedule = Schedule(DOMAIN, training=True)
    cumulative = Counter()
    seen = Counter()
    state_path = root / "resume.pt"
    log_path = root / "training.jsonl"
    if resume:
        with log_path.open() as log:
            original_native_sha = json.loads(log.readline())["native_sha256"]
        if original_native_sha != native_sha:
            raise ValueError("Native binary changed since starting this run")
        state = torch.load(state_path, weights_only=False)
        model.load_state_dict(state["model"])
        optimizer.load_state_dict(state["optimizer"])
        schedule.__dict__.update(state["schedule"])
        cumulative, seen = state["cumulative"], state["seen"]
        receipt = state["receipt"]
        for label, member in state["league"].items():
            existing = next((item for item in (*league.anchors, *league.snapshots)
                             if item.label == label), None)
            if existing is None:
                path = root / "history" / f"{label}.npz"
                frozen, _ = verified_model(path, member["sha256"], 256)
                existing = Member(label, frozen.eval().requires_grad_(False), member["sha256"])
                league.snapshots.append(existing)
            existing.bouts = member["bouts"]
            existing.score = member["learner_score_ema"]
        with log_path.open("rb+") as raw:
            raw.truncate(state["log_offset"])
    else:
        root.mkdir(parents=True, exist_ok=False)
        receipt = dict(update=BASE, cpu_s=0, elapsed_s=0)

    cpu_start, wall_start = cpu_seconds(), time.monotonic()
    prior_cpu, prior_wall = receipt["cpu_s"], receipt["elapsed_s"]
    with log_path.open("a") as log, Batch(config.batch) as batch:
        if resume:
            publish(root, model, league, receipt, state["stats"], log, native_sha)
        else:
            record(log, "start", recipe=RECIPE, batch=config.batch, base=base,
                   milestones=MILESTONES, growth_interval=GROWTH_INTERVAL,
                   warm_sha256=WARM_SHA, optimizer="fresh Adam, epsilon 1e-5",
                   seed_namespace=f"{DOMAIN:016x}", native_sha256=native_sha,
                   torch_version=torch.__version__, numpy_version=np.__version__,
                   archived_history=str(archive), league=league.report())

        for update_id in range(receipt["update"] + 1, MILESTONES[-1] + 1):
            if (root / "STOP").exists():
                record(log, "stop", reason="stop_file", last_completed=receipt)
                return
            fixtures = schedule.draw(config.batch)
            _, classes, levels, seats = fixtures
            torch.manual_seed(DOMAIN + 2 * update_id)
            controllers = league.assignments(
                seats, np.random.default_rng([DOMAIN, update_id, 0]),
                config.live_fraction, matched=True,
                history_rng=np.random.default_rng([DOMAIN, update_id, 1]),
            )
            live = int(np.all(controllers == -1, axis=1).sum())
            unequal = int((levels[:, 0] != levels[:, 1]).sum())
            if (live, unequal) != (96, 96):
                raise ValueError("Expected exactly 25% all-live and 25% mixed-level duels")
            trace, outcomes, decisions = collect(
                batch, model, league, fixtures, controllers, lambda: False, config.repeat_cost,
            )
            reasons = batch.result_reasons()
            torch.manual_seed(DOMAIN + 2 * update_id + 1)
            stats = update(model, optimizer, trace.finish(
                outcomes, config.gamma, config.gae_lambda,
                draw_rewards=np.where(reasons >= 3, -config.timeout_penalty, 0),
            ), config, lambda: False)
            league.observe(seats, controllers, outcomes)
            cumulative.update(decisions)
            cumulative["learner_decisions"] += decisions["learner"]
            cumulative["frozen_decisions"] += decisions["snapshot"]
            cumulative["optimizer_minibatches"] += config.epochs * (
                (trace.size + config.minibatch - 1) // config.minibatch
            )
            cumulative["live_vs_live_bouts"] += live
            cumulative["unequal_level_bouts"] += unequal
            cumulative.update({f"result_{key}": count
                               for key, count in termination(batch, reasons).items()})
            for seat in (0, 1):
                played = trace.last[:, seat] >= 0
                seen.update((int(cls), int(level), seat)
                            for cls, level in zip(classes[played, seat], levels[played, seat]))
            new_updates = update_id - BASE
            receipt = dict(
                update=update_id, bouts=update_id * config.batch,
                new_updates=new_updates, new_bouts=new_updates * config.batch,
                cpu_s=prior_cpu + cpu_seconds() - cpu_start,
                elapsed_s=prior_wall + time.monotonic() - wall_start,
                coverage_cells=len(seen), min_games_per_cell=min(seen.values()),
                termination={key[7:]: int(value) for key, value in cumulative.items()
                             if key.startswith("result_")},
                **{key: int(cumulative[key]) + base[key]
                   for key in ("learner_decisions", "frozen_decisions", "optimizer_minibatches",
                               "live_vs_live_bouts", "unequal_level_bouts")},
            )
            # Commit model and deterministic stream state before publishing the matching receipt.
            # A recovery truncates any partial tail and republishes that same completed update.
            temp = state_path.with_suffix(".tmp")
            torch.save(dict(model=model.state_dict(), optimizer=optimizer.state_dict(),
                            schedule=schedule.__dict__, cumulative=cumulative, seen=seen,
                            league=league.report(), receipt=receipt, stats=stats,
                            log_offset=log.tell()), temp)
            temp.replace(state_path)
            publish(root, model, league, receipt, stats, log, native_sha)
        record(log, "stop", reason="exact_update_budget", last_completed=receipt,
               cpu_s=prior_cpu + cpu_seconds() - cpu_start,
               elapsed_s=prior_wall + time.monotonic() - wall_start)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", required=True, type=Path)
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--native-sha256", default=NATIVE_SHA,
                        help="expected ABI 2002 native library digest for a new run")
    args = parser.parse_args()
    train(args.run_dir, args.archive, args.resume, args.native_sha256)
