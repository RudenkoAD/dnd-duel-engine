"""Verify the fixed roster PPO run with its frozen launcher on PYTHONPATH."""

import argparse
from collections import Counter
from hashlib import sha256
import json
import math
from pathlib import Path

import numpy as np

from arena.native import ROSTER_NEW_INPUTS, _lib
from arena.policy import Policy
from roster_schedule import BUILD_COUNT, LEVEL, RosterSchedule


ROOT = Path(__file__).resolve().parent
EXPECTED_UPDATES = 1000
BATCH = 384
GROW_EVERY = 64


def digest(path):
    return sha256(path.read_bytes()).hexdigest()


def scheduled_coverage(seed):
    """Replay the deterministic fixtures used by the frozen launcher."""
    schedule = RosterSchedule(seed, training=True)
    layout_bouts = np.zeros(schedule.arenas, dtype=np.int64)
    focal = np.zeros((BUILD_COUNT, 2), dtype=np.int64)
    pair_layouts = np.zeros((BUILD_COUNT, BUILD_COUNT, schedule.arenas), dtype=np.int64)
    for _ in range(EXPECTED_UPDATES):
        fixtures = schedule.draw(BATCH)
        np.testing.assert_array_equal(fixtures.seats[::2], 0)
        np.testing.assert_array_equal(fixtures.seats[1::2], 1)
        np.testing.assert_array_equal(fixtures.seeds[::2], fixtures.seeds[1::2])
        np.testing.assert_array_equal(fixtures.arenas[::2], fixtures.arenas[1::2])
        np.testing.assert_array_equal(fixtures.builds[::2], fixtures.builds[1::2, ::-1])
        left, right = fixtures.builds[::2].T
        np.add.at(pair_layouts, (left, right, fixtures.arenas[::2]), 1)
        np.add.at(focal, (fixtures.builds[np.arange(BATCH), fixtures.seats], fixtures.seats), 1)
        np.add.at(layout_bouts, fixtures.arenas, 1)
    np.testing.assert_array_equal(focal[:, 0], focal[:, 1])
    assert int(layout_bouts.max() - layout_bouts.min()) <= 4
    assert (pair_layouts > 0).all()
    pair_counts = pair_layouts.sum(axis=2)
    assert int(pair_counts.max() - pair_counts.min()) == 1
    return {
        "bouts_by_layout": layout_bouts.tolist(),
        "focal_bouts_by_build_and_seat": focal.tolist(),
        "fixture_pairs_per_ordered_build_pair": [int(pair_counts.min()), int(pair_counts.max())],
        "fixture_pairs_per_ordered_build_pair_and_layout": [
            int(pair_layouts.min()), int(pair_layouts.max())
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-original-native", action="store_true",
                        help="also require the exact macOS library used by the recorded run")
    args = parser.parse_args()
    rows = [json.loads(line) for line in (ROOT / "training.jsonl").read_text().splitlines()]
    start, stop = rows[0], rows[-1]
    assert Counter(row["event"] for row in rows) == {
        "start": 1, "update": EXPECTED_UPDATES, "checkpoint": 16, "stop": 1,
    }
    assert start["event"] == "start" and stop["event"] == "stop"
    local_native = digest(Path(_lib()._name))
    if args.require_original_native:
        assert start["native_sha256"] == local_native
    assert start["arena_abi"] == 2002 and start["hidden"] == 256
    assert start["build_ids"] == list(range(BUILD_COUNT)) and start["arenas"] == list(range(5))
    assert start["neutralized_roster_inputs"] == list(ROSTER_NEW_INPUTS)
    config = start["config"]
    assert config["run_id"] == 2901 and config["updates"] == EXPECTED_UPDATES
    assert config["batch"] == BATCH and config["checkpoint_every"] == GROW_EVERY
    assert config["matched"] and not config["legacy_schedule"] and not config["specialist"]
    assert start["specialist_inference"] is None
    assert len(start["league"]) == len(start["neutralized_league_members"]) == 3
    assert set(start["league"]) == set(start["neutralized_league_members"])
    for source, entry in zip(
        (config["start"], *config["anchor"]),
        (start["league"][label] for label in start["neutralized_league_members"]),
    ):
        assert digest(Path(source)) == entry["sha256"]

    updates = [row for row in rows if row["event"] == "update"]
    for update, row in enumerate(updates, 1):
        assert row["update"] == update and row["bouts"] == update * BATCH
        assert row["live_vs_live"] == update * BATCH // 4
        assert len(row["league"]) == 3 + (update - 1) // GROW_EVERY
        assert sum(member["bouts"] for member in row["league"].values()) == update * BATCH * 3 // 4
        assert all(math.isfinite(row[key]) for key in (
            "policy_loss", "value_loss", "entropy", "kl", "projectile_target_loss",
        ))
        assert row["learning_rate"] == 0.00004
    assert stop["reason"] == "update_limit"
    assert stop["last_update"] == stop["last_checkpoint"] == EXPECTED_UPDATES
    assert stop["coverage_cells"] == 2 * BUILD_COUNT and stop["min_games_per_cell"] > 0
    cumulative = stop["cumulative"]
    assert cumulative["unequal_level_bouts"] == cumulative["result_ongoing"] == 0
    assert cumulative["bouts"] == updates[-1]["bouts"]
    assert cumulative["optimizer_minibatches"] == updates[-1]["optimizer_minibatches"] > 0
    assert sum(count for key, count in cumulative.items() if key.startswith("result_")) == cumulative["bouts"]

    checkpoints = [row for row in rows if row["event"] == "checkpoint"]
    assert [row["update"] for row in checkpoints] == [*range(GROW_EVERY, EXPECTED_UPDATES, GROW_EVERY), EXPECTED_UPDATES]
    for row in checkpoints:
        path = ROOT / "checkpoints" / Path(row["path"]).name
        assert digest(path) == row["sha256"]
        policy = Policy.load(path).require_arena(2002, 1024, 128)
        assert policy.metadata["training_update"] == row["update"]
        assert policy.metadata["build_ids"] == start["build_ids"]
        assert policy.metadata["levels"] == [LEVEL]
        assert all(np.isfinite(weights).all() for weights in policy.weights.values())
        if row["update"] < EXPECTED_UPDATES:
            assert stop["league"][path.stem]["sha256"] == row["sha256"]
    assert policy.metadata["league"] == stop["league"]
    assert policy.metadata["cumulative"] == cumulative
    assert policy.weights["w1"].shape == (1024, 256)
    assert policy.weights["w2"].shape == (256, 256)
    assert np.any(policy.weights["w1"][list(ROSTER_NEW_INPUTS)] != 0)
    warmed = Policy.load(Path(config["start"]))
    assert any(not np.array_equal(policy.weights[key], warmed.weights[key])
               for key in policy.weights if key != "w1")
    assert all(member["bouts"] > 0 for member in stop["league"].values())
    assert sum(member["bouts"] for label, member in stop["league"].items()
               if label in start["league"]) == (
                   GROW_EVERY * (BATCH * 3 // 4)
                   + (EXPECTED_UPDATES - GROW_EVERY) * (BATCH * 3 // 8)
               )

    print(json.dumps({
        "checkpoint": checkpoints[-1]["path"],
        "checkpoint_sha256": checkpoints[-1]["sha256"],
        "training_log_sha256": digest(ROOT / "training.jsonl"),
        "native_sha256": start["native_sha256"],
        "verification_native_sha256": local_native,
        "wall_seconds": stop["elapsed_s"],
        "process_cpu_seconds": stop["cpu_s"],
        "learner_decision_coverage_cells": stop["coverage_cells"],
        "learner_decision_coverage_minimum": stop["min_games_per_cell"],
        "fixture_schedule": scheduled_coverage(int(start["seed_namespace"], 16)),
        "league": stop["league"],
        "cumulative": cumulative,
    }, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
