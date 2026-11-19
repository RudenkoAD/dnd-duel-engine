"""Play fixed final fixtures for mixed-controller strength comparisons."""

import argparse
import hashlib
import json
from pathlib import Path
import time

import numpy as np
import torch

from arena.native import ACTIONS, ARENA_ABI, ARENA_NAMES, FEATURES, Batch, _lib, arena_count
from fixtures import MANIFEST, create
from self_play import load_model


HERE = Path(__file__).resolve().parent
INITIAL = HERE.parents[1] / "training/runs/long-width/checkpoints/update-2901.npz"


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def checkpoint(path, *, expect_initialized, roster_run=True):
    policy, initialized = load_model(path, ARENA_ABI, FEATURES, ACTIONS, roster_run=roster_run)
    if initialized != expect_initialized:
        raise ValueError(f"Unexpected roster input interpretation for {path}: {initialized}")
    return policy.eval().requires_grad_(False)


def layout(pairs, seeds):
    # Orientation takes the focal build index first, then the focal seat.
    # Equal-build fixtures need just the two focal seats for the same assignment.
    pair, arena, repeat, orientation = np.indices((*seeds.shape, 4)).reshape(4, -1)
    valid = (pairs[pair, 0] != pairs[pair, 1]) | (orientation < 2)
    pair, arena, repeat, orientation = [column[valid] for column in (pair, arena, repeat, orientation)]
    seats = orientation % 2
    builds = pairs[pair].copy()
    reverse = orientation // 2 != seats
    builds[reverse] = builds[reverse, ::-1]
    return (pair, arena, repeat, orientation), seeds[pair, arena, repeat], builds, seats.astype(np.int32)


def choose(policy, state, legal):
    logits, _ = policy(torch.from_numpy(state), torch.from_numpy(legal))
    return logits.argmax(dim=1).numpy()


@torch.inference_mode()
def play(seeds, builds, arenas, seats, focal, opponent):
    actions = np.empty(len(seeds), dtype=np.int32)
    with Batch(len(seeds)) as batch:
        batch.reset_builds(seeds, builds, arenas=arenas)
        for _ in range(2049):
            if opponent is None:
                batch.advance(seats, expert=True)
            state, legal, actors, winners = batch.observe()
            active = actors >= 0
            if not active.any():
                reasons = batch.result_reasons()
                break
            actions.fill(-1)
            focal_lanes = active & (actors == seats)
            if focal_lanes.any():
                actions[focal_lanes] = choose(focal, state[focal_lanes], legal[focal_lanes])
            other_lanes = active & (actors != seats)
            if other_lanes.any():
                if opponent is None:
                    raise RuntimeError("Native expert advance returned an opponent decision")
                actions[other_lanes] = choose(opponent, state[other_lanes], legal[other_lanes])
            batch.step(actions)
        else:
            raise RuntimeError("Fixture batch did not terminate")
    if not np.isin(winners, [0, 1, 2]).all() or not np.isin(reasons, [1, 2, 3, 4]).all():
        raise RuntimeError("Invalid terminal results")
    outcome = np.where(winners == 2, 0, np.where(winners == seats, 1, -1))
    return outcome, reasons


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("arm", choices=[
        "initial-expert", "original-expert", "trained-expert", "trained-initial", "trained-original",
    ])
    parser.add_argument("--trained", type=Path)
    parser.add_argument("--batch", type=int, default=1024)
    parser.add_argument("--pilot", type=int)
    args = parser.parse_args()
    if args.batch <= 0 or (args.pilot is not None and args.pilot <= 0):
        parser.error("batch size and pilot size must be positive")
    if args.arm.startswith("trained") and args.trained is None:
        parser.error("trained arms require --trained")
    torch.set_num_threads(1)
    torch.set_num_interop_threads(1)
    started = time.monotonic()
    with np.load(MANIFEST, allow_pickle=False) as manifest:
        pairs, seeds = manifest["pairs"], manifest["seeds"]
    if any(not np.array_equal(actual, expected) for actual, expected in zip((pairs, seeds), create())):
        raise RuntimeError("Fixture archive disagrees with its generator")
    if arena_count() != len(ARENA_NAMES):
        raise RuntimeError("The native library has an unexpected number of arenas")
    focal_path = args.trained if args.arm.startswith("trained") else INITIAL
    opponent_path = INITIAL if args.arm in ("trained-initial", "trained-original") else None
    neutralized_focal = args.arm == "initial-expert"
    focal = checkpoint(focal_path, expect_initialized=neutralized_focal,
                       roster_run=args.arm != "original-expert")
    neutralized_opponent = args.arm == "trained-initial"
    opponent = None
    if opponent_path:
        opponent = checkpoint(opponent_path, expect_initialized=neutralized_opponent,
                              roster_run=neutralized_opponent)
    inputs = {"focal": focal_path, "native": Path(_lib()._name), "fixtures": MANIFEST}
    if opponent_path:
        inputs["opponent"] = opponent_path
    hashes = {label: sha256(path) for label, path in inputs.items()}
    keys, flat_seeds, builds, seats = layout(pairs, seeds)
    # Mix classes and arenas within batches to make a first-chunk pilot representative.
    order = np.random.default_rng(61439).permutation(len(flat_seeds))
    if args.pilot:
        order = order[:args.pilot]
    outcomes = np.full((*seeds.shape, 4), 9, dtype=np.int8)
    reasons = np.zeros_like(outcomes)
    for start in range(0, len(order), args.batch):
        ranks = order[start:start + args.batch]
        coords = tuple(key[ranks] for key in keys)
        outcomes[coords], reasons[coords] = play(
            flat_seeds[ranks], builds[ranks], coords[1], seats[ranks], focal, opponent,
        )
        print(f"{args.arm} {start + len(ranks)}/{len(order)} in {time.monotonic() - started:.1f}s",
              flush=True)
    if not args.pilot:
        if any(sha256(path) != hashes[label] for label, path in inputs.items()):
            raise RuntimeError("Evaluation inputs changed during execution")
        metadata = {
            "arm": args.arm,
            "arena_abi": ARENA_ABI,
            "bouts": len(order),
            "focal_roster_inputs_neutralized": neutralized_focal,
            "opponent_roster_inputs_neutralized": neutralized_opponent,
            "inference": f"torch {torch.__version__} single-threaded masked argmax",
            "input_files": {
                label: {"path": str(path.resolve()), "sha256": hashes[label]}
                for label, path in inputs.items()
            },
        }
        destination = HERE / f"{args.arm}.npz"
        temporary = destination.with_suffix(".tmp.npz")
        np.savez_compressed(temporary, outcomes=outcomes, reasons=reasons,
                            metadata=json.dumps(metadata, sort_keys=True))
        temporary.replace(destination)


if __name__ == "__main__":
    main()
