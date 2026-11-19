import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import numpy as np

from arena.native import _lib
from long_width_league import BASE, DOMAIN, MILESTONES, RECIPE, original_league
from self_play import Schedule


class LongWidthLeagueTests(unittest.TestCase):
    def test_exact_duel_targets_and_fixture_namespace(self):
        self.assertEqual(tuple(update * RECIPE["batch"] for update in MILESTONES),
                         (371328, 742656, 1113984))
        self.assertEqual((MILESTONES[-1] - BASE) * RECIPE["batch"], 823296)
        schedule = Schedule(DOMAIN, training=True)
        for offset in (0, 192):
            seeds, classes, levels, seats = schedule.draw(RECIPE["batch"])
            np.testing.assert_array_equal(seeds[::2], seeds[1::2])
            np.testing.assert_array_equal(seeds[::2], DOMAIN + np.arange(offset, offset + 192))
            np.testing.assert_array_equal(classes[::2], classes[1::2])
            np.testing.assert_array_equal(levels[::2], levels[1::2])
            np.testing.assert_array_equal(seats[::2], 1 - seats[1::2])
            self.assertEqual(int((levels[:, 0] != levels[:, 1]).sum()), 96)

    def test_original_archive_restores_all_experienced_members(self):
        _, league, base = original_league(Path("training/runs/long-width/origin-history"))
        self.assertEqual(len(league.anchors), 4)
        self.assertEqual(len(league.snapshots), 11)
        self.assertEqual(sum(entry["bouts"] for entry in league.report().values()),
                         base["bouts"] - base["live_vs_live_bouts"])

    def test_atomic_resume_rebuilds_the_log_and_preserves_weights(self):
        # Exercise a recent growth snapshot plus a subsequent PPO step through
        # the genuine native batch, restarting in a separate Python process.
        # The historical launcher still requires its pinned binary by default.
        native_sha = hashlib.sha256(Path(_lib()._name).read_bytes()).hexdigest()
        driver = """
import sys
from pathlib import Path
import long_width_league as experiment
experiment.MILESTONES = tuple(map(int, sys.argv[3:]))
experiment.GROWTH_INTERVAL = 758
root = Path(sys.argv[1])
experiment.train(root, Path("training/runs/long-width/origin-history"), root.exists(),
                 expected_native_sha=sys.argv[2])
"""
        def run(root, *milestones):
            result = subprocess.run(
                [sys.executable, "-c", driver, str(root), native_sha, *map(str, milestones)],
                text=True, capture_output=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)

        with tempfile.TemporaryDirectory() as folder:
            baseline, restarted = (Path(folder) / name for name in ("baseline", "restart"))
            run(baseline, 758, 759)
            run(restarted, 758)
            with (restarted / "training.jsonl").open("a") as log:
                log.write("partial tail")
            run(restarted, 758, 759)
            with np.load(baseline / "checkpoints/update-0759.npz") as expected, np.load(
                restarted / "checkpoints/update-0759.npz"
            ) as actual:
                for name in expected.files:
                    if name != "metadata":
                        np.testing.assert_array_equal(actual[name], expected[name])
            events = [json.loads(line) for line in
                      (restarted / "training.jsonl").read_text().splitlines()]
            self.assertEqual([event["update"] for event in events if event["event"] == "update"],
                             [758, 759])
            self.assertEqual(events[-1]["reason"], "exact_update_budget")
