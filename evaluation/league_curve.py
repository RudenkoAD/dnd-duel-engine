"""Measure checkpoints on the frozen, seat-balanced L4D development schedule."""

import argparse
from collections import Counter
import hashlib
from io import BytesIO
import json
from pathlib import Path
import subprocess
import time

import numpy as np

from arena.native import ACTIONS, ARENA_ABI, Batch, CLASS_NAMES, FEATURES
from arena.policy import Policy
from schedule import ARENAS, arena_for_seed


START = ("models/arena_policy.npz",
         "d81965d6d1554a3df34c6265551e637b243224988b7c7efe7bbec679c719353e")
CONTROL = ("training/runs/viewer_relative/control.npz",
           "99b99391f0d80b090ae74d93ce25d7011956da836bdd2f5ad64293b1ff5f3f00")
INTERMEDIATE = ("training/runs/viewer_relative/target-aware-update-0090.npz",
                "c9be34b39f4793e84bd051a1e6d66f697a6880051ed86f543f5d6df4adb6b480")
ANCHORS = {"champion": START, "control": CONTROL, "intermediate": INTERMEDIATE}
ROOT = Path(__file__).resolve().parents[1]
REVISION = "0260a49"
DOMAIN = b"L4D"
MIN_IMPROVEMENT = 0.005
PATIENCE = 4


def schedule():
    seen = set()
    cells = []
    for focal in range(len(CLASS_NAMES)):
        for opponent in range(focal, len(CLASS_NAMES)):
            for level in range(1, 6):
                for arena in ARENAS:
                    nonce = 0
                    while True:
                        key = f"{focal},{opponent},{level},{arena},{nonce}".encode()
                        seed = int.from_bytes(DOMAIN + hashlib.blake2b(key, digest_size=5).digest(), "big")
                        if arena_for_seed(seed) == arena:
                            break
                        nonce += 1
                    if seed in seen:
                        raise RuntimeError("duplicate development seed")
                    seen.add(seed)
                    cells.append((focal, opponent, level, arena, seed))
                    if focal != opponent:
                        cells.append((opponent, focal, level, arena, seed))
    return cells


def load(data):
    return Policy.load(BytesIO(data)).require_arena(ARENA_ABI, FEATURES, ACTIONS)


def frozen(spec):
    path, expected = spec
    live = ROOT / path
    data = live.read_bytes() if live.exists() else b""
    if hashlib.sha256(data).hexdigest() != expected and spec in (START, CONTROL):
        data = subprocess.check_output(["git", "show", f"{REVISION}:{path}"], cwd=ROOT)
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError(f"fixed opponent SHA-256 mismatch: {path}")
    return load(data)


def bouts(rows, focal, opponent, batch_size):
    trials = [(seat, a, b, level, seed)
              for a, b, level, _, seed in rows for seat in (0, 1)]
    counts = [Counter(), Counter()]
    for start in range(0, len(trials), batch_size):
        chunk = trials[start:start + batch_size]
        indices = np.arange(len(chunk), dtype=np.int32)
        seats = np.array([seat for seat, *_ in chunk], dtype=np.int32)
        classes = np.array([(a, b) if seat == 0 else (b, a)
                            for seat, a, b, _, _ in chunk], dtype=np.int32)
        levels = np.array([(level, level) for _, _, _, level, _ in chunk], dtype=np.int32)
        seeds = np.array([seed for *_, seed in chunk], dtype=np.uint64)
        actions = np.empty(len(chunk), dtype=np.int32)
        with Batch(len(chunk)) as arena:
            arena.reset(indices, seeds, classes, levels)
            for step in range(801):
                if opponent is None:
                    arena.advance(seats, expert=True)
                features, masks, actors, winners = arena.observe()
                if (actors < 0).all():
                    break
                actions.fill(-1)
                if focal is opponent:
                    active = indices[actors >= 0]
                    actions[active] = focal.choose(features[active], masks[active])
                else:
                    active = indices[actors == seats]
                    if active.size:
                        actions[active] = focal.choose(features[active], masks[active])
                    other = indices[(actors >= 0) & (actors != seats)]
                    if other.size:
                        actions[other] = opponent.choose(features[other], masks[other])
                arena.step(actions)
            else:
                raise RuntimeError(f"matches still active after {step + 1} iterations")
        for seat, winner in zip(seats, winners):
            counts[seat]["wins" if winner == seat else "losses" if winner == 1 - seat else "draws"] += 1
    return {**summarize(counts[0] + counts[1]), "seats": [summarize(count) for count in counts]}


def summarize(counts):
    total = counts.total()
    wins, draws, losses = (counts[field] for field in ("wins", "draws", "losses"))
    return dict(wins=wins, draws=draws, losses=losses, raw_win=wins / total,
                score=(wins + draws / 2) / total)


def plateau(records, metric):
    threshold_best = records[0][metric]
    best = records[0]
    stagnant = 0
    for record in records[1:]:
        if record[metric] > best[metric]:
            best = record
        if record[metric] >= threshold_best + MIN_IMPROVEMENT - 1e-12:
            threshold_best = record[metric]
            stagnant = 0
        else:
            stagnant += 1
    return dict(best_update=best["update"], best_score=best[metric],
                threshold_best=threshold_best, stagnant_intervals=stagnant,
                plateau=stagnant >= PATIENCE)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--update", type=int, default=0)
    parser.add_argument("--batch-size", default=512, type=int)
    args = parser.parse_args()
    if (args.update == 0) != (args.checkpoint is None) or args.update < 0:
        parser.error("specify a checkpoint only with a positive update")
    if args.batch_size < 1:
        parser.error("batch size must be positive")

    rows = schedule()
    rivals = {name: frozen(spec) for name, spec in ANCHORS.items()}
    data = args.checkpoint.read_bytes() if args.checkpoint else None
    candidate = load(data) if data is not None else rivals["champion"]
    candidate_hash = hashlib.sha256(data).hexdigest() if data is not None else START[1]
    results = {}
    start = time.monotonic()
    for label, rival in (*rivals.items(), ("expert", None)):
        results[label] = bouts(rows, candidate, rival, args.batch_size)
        print(f"{label}: {results[label]} ({time.monotonic() - start:.1f}s cumulative)", flush=True)
    if not args.update and results["champion"]["score"] != 0.5:
        raise RuntimeError("starting champion self-play failed seat symmetry")
    league = sum(results[name]["score"] for name in ANCHORS) / len(ANCHORS)
    expert = results["expert"]["score"]
    record = dict(update=args.update, checkpoint_sha256=candidate_hash,
                  opponents=results, league=league, expert=expert, balanced=(league + expert) / 2,
                  elapsed_seconds=round(time.monotonic() - start, 3))
    header = dict(domain=DOMAIN.decode(), schedule_sha256=hashlib.sha256(
        json.dumps(rows, separators=(",", ":")).encode()).hexdigest(),
        ordered_cells=len(rows), distinct_seeds=len({row[-1] for row in rows}),
        bouts_per_opponent=2 * len(rows), opponent_sha256={name: spec[1] for name, spec in ANCHORS.items()},
        patience=PATIENCE, minimum_improvement=MIN_IMPROVEMENT)
    if args.output.exists():
        result = json.loads(args.output.read_text())
        if result["protocol"] != header:
            parser.error("output protocol differs")
        if args.update != result["checkpoints"][-1]["update"] + 100:
            parser.error("updates must be appended at 100-update intervals")
    else:
        if args.update != 0:
            parser.error("run the starting baseline first")
        result = dict(protocol=header, checkpoints=[])
    result["checkpoints"].append(record)
    metrics = {key: plateau(result["checkpoints"], key) for key in ("balanced", "league", "expert")}
    result["selection"] = dict(**metrics, plateau=all(value["plateau"] for value in metrics.values()))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(".tmp")
    temporary.write_text(json.dumps(result, indent=2) + "\n")
    temporary.replace(args.output)
    print(json.dumps(result["selection"]), flush=True)


if __name__ == "__main__":
    main()
