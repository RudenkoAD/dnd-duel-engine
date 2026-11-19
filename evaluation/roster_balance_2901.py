"""Run same-policy games on the R36-L2901-FINAL-v1 manifest."""

import argparse
import hashlib
from itertools import combinations_with_replacement
import json
from pathlib import Path
import time

import numpy as np

from arena.native import ACTIONS, ARENA_ABI, FEATURES, ROSTER_NEW_INPUTS, ROSTER_SIZE, Batch, _lib
from arena.policy import Policy


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def greedy(policy, state, legal):
    weights = policy.weights
    hidden = np.tanh(state @ weights["w1"] + weights["b1"])
    hidden = np.tanh(hidden @ weights["w2"] + weights["b2"])
    logits = hidden @ weights["w_policy"] + weights["b_policy"]
    masked = np.where(legal, logits, -np.inf)
    actions = masked.argmax(axis=1).astype(np.int32)
    # Revisit close calls with the reference evaluator to avoid differences
    # in floating-point accumulation moving an argmax across a near tie.
    best = masked[np.arange(len(actions)), actions].copy()
    masked[np.arange(len(actions)), actions] = -np.inf
    close = best - masked.max(axis=1) < 0.001
    if close.any():
        actions[close] = policy.choose(state[close], legal[close])
    return actions


def play(policy, seeds, builds, arenas):
    with Batch(seeds.size) as batch:
        batch.reset_builds(seeds, builds, arenas=arenas)
        for _ in range(2048):
            state, legal, actors, winners = batch.observe()
            active = actors >= 0
            if not active.any():
                break
            if policy is None:
                actions = batch.bot(expert=True)
            else:
                actions = np.full(batch.n, -1, dtype=np.int32)
                actions[active] = greedy(policy, state[active], legal[active])
            batch.step(actions)
        else:
            raise RuntimeError("battle did not terminate")
        reasons = batch.result_reasons()
        if not np.isin(winners, (0, 1, 2)).all() or not np.isin(reasons, (1, 2, 3, 4)).all():
            raise RuntimeError("invalid outcome or termination")
        hp = np.zeros((batch.n, 2), dtype=np.int16)
        raw = np.empty(224, dtype=np.int32)
        terrain = np.empty((7, 9), dtype=np.uint8)
        for lane in np.flatnonzero(reasons == 1):
            winner = int(winners[lane])
            if winner == 2 or not batch._lib.arena_raw(
                batch._open(), lane, winner, raw.ctypes.data, terrain.ctypes.data
            ):
                raise RuntimeError("cannot read KO winner HP")
            offset = 14 + winner * 35
            hp[lane] = raw[offset:offset + 2]
        if np.any(hp[reasons == 1, 0] <= 0) or np.any(hp[:, 0] > hp[:, 1]):
            raise RuntimeError("invalid KO winner HP")
    return np.column_stack((winners, reasons, hp)).astype(np.int16)


def evaluate(policy, pairs, seeds, batch_size):
    expected = np.asarray(list(combinations_with_replacement(range(ROSTER_SIZE), 2)))
    if not np.array_equal(pairs, expected) or seeds.shape[:2] != (len(expected), 5):
        raise ValueError("expected full 36-build manifest with five arenas")
    if seeds.dtype != np.uint64 or np.unique(seeds).size != seeds.size:
        raise ValueError("manifest seeds must be unique uint64 values")
    shape = (*seeds.shape, 2)
    included = np.ones(shape, dtype=bool)
    included[pairs[:, 0] == pairs[:, 1], :, :, 1] = False
    keys = np.argwhere(included)
    raw = np.full((*shape, 4), -1, dtype=np.int16)
    started = time.monotonic()
    # Mix matchups across batches to avoid successive large batches of
    # unusually long mirrors. Seeded shuffling makes the allocation repeatable.
    np.random.default_rng(2901).shuffle(keys)
    for start in range(0, len(keys), batch_size):
        matchup, arenas, repeats, focal_seat = keys[start:start + batch_size].T
        builds = pairs[matchup].copy()
        swapped = focal_seat == 1
        builds[swapped] = builds[swapped, ::-1]
        raw[matchup, arenas, repeats, focal_seat] = play(
            policy, seeds[matchup, arenas, repeats], builds, arenas
        )
        done = min(start + batch_size, len(keys))
        if start == 0 or (start // batch_size + 1) % 16 == 0 or done == len(keys):
            print(f"{done:,}/{len(keys):,} bouts at {time.monotonic() - started:.1f}s", flush=True)
    return raw


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, required=True)
    controller = parser.add_mutually_exclusive_group(required=True)
    controller.add_argument("--checkpoint", type=Path)
    controller.add_argument("--expert", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--neutralize-roster-inputs", action="store_true")
    parser.add_argument("--batch", type=int, default=256)
    args = parser.parse_args()
    if not 1 <= args.batch <= 65536:
        parser.error("--batch must be in 1..65536")
    if args.expert and args.neutralize_roster_inputs:
        parser.error("--neutralize-roster-inputs requires --checkpoint")
    paths = {"fixtures": args.fixtures, "native": Path(_lib()._name)}
    if args.checkpoint:
        paths["checkpoint"] = args.checkpoint
    digests = {key: digest(value) for key, value in paths.items()}
    with np.load(args.fixtures, allow_pickle=False) as manifest:
        pairs, seeds = manifest["pairs"], manifest["seeds"]
    neutralized = list(ROSTER_NEW_INPUTS) if args.neutralize_roster_inputs else []
    policy = None
    if args.checkpoint:
        policy = Policy.load(args.checkpoint).require_arena(ARENA_ABI, FEATURES, ACTIONS)
        if neutralized:
            if "build_ids" in policy.metadata:
                parser.error("--neutralize-roster-inputs requires a pre-roster checkpoint")
            policy.weights["w1"][neutralized] = 0
    raw = evaluate(policy, pairs, seeds, args.batch)
    if digests != {key: digest(value) for key, value in paths.items()}:
        raise RuntimeError("evaluation input changed during the run")
    metadata = {
        "files": {key: str(path) for key, path in paths.items()},
        "sha256": digests,
        "arena_abi": ARENA_ABI,
        "neutralized_inputs": neutralized,
        "raw_axes": ["build pair", "arena", "repeat", "seat of first build", "field"],
        "raw_fields": ["winner seat (2 for draw)", "reason", "winner HP", "winner max HP"],
        "mirror_encoding": "Only seat-0 representation persisted, omitted entries filled with -1",
        "numpy": np.__version__,
        "batch": args.batch,
        "inference": ("native expert" if args.expert else
                      "float32 NumPy matmul, with Policy.choose fallback for action margins < 0.001"),
    }
    np.savez_compressed(args.output, pairs=pairs, seeds=seeds, raw=raw, metadata=json.dumps(metadata))


if __name__ == "__main__":
    main()
