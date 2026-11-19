"""Check that parity comparisons preserve the experimental block design."""

import unittest
from unittest.mock import patch

import numpy as np

from arena.native import Batch, VARIANTS_PER_CLASS
from evaluation import roster_parity as parity
from schedule import arena_for_seed


class RosterParityTests(unittest.TestCase):
    def test_shared_arena_seeds_and_balanced_legacy_levels(self):
        plan = parity.fixtures(8)
        for arena in range(4):
            self.assertEqual(set(map(arena_for_seed, map(int, plan.seeds[:, arena].flat))), {arena})
        mixed = plan.legacy_levels[..., 0] != plan.legacy_levels[..., 1]
        self.assertEqual(mixed.mean(), .25)
        np.testing.assert_equal(plan.legacy_levels[:, :, 3], plan.legacy_levels[:, :, 7, ::-1])
        np.testing.assert_equal(plan.legacy_levels[:, ::2, 3], plan.legacy_levels[:, 1::2, 3, ::-1])
        self.assertEqual(set(plan.legacy_levels.ravel()), set(range(1, 6)))

    def test_canonical_observations_and_masks_match_at_reset(self):
        plan = parity.fixtures(8)
        classes = np.asarray(parity.CLASSES, dtype=np.int32)
        size = classes.shape[0]
        seeds = plan.seeds[:, 0, 0]
        with Batch(size) as legacy, Batch(size) as roster:
            legacy.reset(np.arange(size, dtype=np.int32), seeds, classes, np.full_like(classes, 5))
            roster.reset_builds(seeds, classes * VARIANTS_PER_CLASS,
                                arenas=np.zeros(size, dtype=np.int32))
            for legacy_part, roster_part in zip(legacy.observe(), roster.observe()):
                np.testing.assert_equal(legacy_part, roster_part)

    def test_canonical_reset_agrees_with_roster_default_in_paired_bouts(self):
        with patch.object(parity, "CLASSES", ((0, 5), (4, 11))):
            plan = parity.fixtures(8)
            canonical = parity.run_arm(plan, "level5", 256)
            roster = parity.run_arm(plan, "roster", 256)
        np.testing.assert_equal(roster[:, 0], canonical[:, 0])

    def test_paired_intervals_use_all_bouts_for_one_sided_finishes(self):
        shape = (1, 1, 4, 4, 2, 4)
        legacy = np.empty(shape, dtype=np.int16)
        legacy[...] = [1, 1, 3, 4]
        canonical = legacy.copy()
        canonical[..., 2] = 2
        roster = np.tile(canonical, (1, 2, 1, 1, 1, 1))
        roster[:, 1] = legacy[:, 0]
        results = parity.paired_intervals(
            {"legacy": legacy, "level5": canonical, "roster": roster}, samples=20,
        )
        self.assertEqual(results["class_gap"], [[0, 0]] * 3)
        self.assertEqual(results["one_sided_bouts"], [[-1, -1], [.5, .5], [-.5, -.5]])

    def test_parity_score_keeps_draws_and_build_gaps_visible(self):
        raw = np.ones((1, 2, 1, 2, 2, 4), dtype=np.int16)
        raw[:, 1, ..., 0] = -1
        raw[:, 0, :, 0, :, :2] = [0, 3]
        raw[..., 2:] = [3, 4]
        raw[:, 1, :, 1, :, 2:] = [2, 4]
        stats = parity.statistics(raw)
        self.assertEqual(stats["draw_rate"], .25)
        self.assertEqual(stats["mean_class_gap"], .125)
        self.assertEqual(stats["mean_build_gap"], .375)
        self.assertEqual(stats["class_scores"], [.375])
        self.assertEqual(stats["knockouts_at_75_percent_hp"], 4)
        self.assertAlmostEqual(stats["share_of_knockouts_at_75_percent_hp"], 4 / 6)
        self.assertEqual(stats["share_of_bouts_at_75_percent_hp"], .5)
