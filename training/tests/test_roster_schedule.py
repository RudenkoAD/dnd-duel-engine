"""Coverage, seat and seed properties of the active self-play schedule."""

from itertools import product
import unittest
from unittest.mock import Mock

import numpy as np

from training.roster_schedule import BUILD_COUNT, DEV_BOUTS, DEV_DOMAIN, TRAIN_DOMAIN, RosterSchedule


class RosterScheduleTests(unittest.TestCase):
    def test_training_sweep_covers_each_pair_and_mirrors_builds(self):
        schedule = RosterSchedule(TRAIN_DOMAIN, training=True, arenas=4)
        pairs = set(product(range(BUILD_COUNT), repeat=2))
        one = schedule.draw(2 * len(pairs))
        np.testing.assert_equal(one.seeds[::2], one.seeds[1::2])
        np.testing.assert_equal(one.arenas[::2], one.arenas[1::2])
        np.testing.assert_equal(one.builds[::2], one.builds[1::2, ::-1])
        np.testing.assert_equal(one.seats[::2], np.zeros(len(pairs), dtype=np.int32))
        np.testing.assert_equal(one.seats[1::2], np.ones(len(pairs), dtype=np.int32))
        self.assertEqual({tuple(row) for row in one.builds[::2]}, pairs)
        for seat in (0, 1):
            np.testing.assert_equal(np.bincount(one.builds[one.seats == seat, seat]),
                                    np.full(BUILD_COUNT, BUILD_COUNT))
        two = schedule.draw(2 * len(pairs))
        self.assertEqual({tuple(row) for row in two.builds[::2]}, pairs)
        self.assertEqual(np.intersect1d(one.seeds, two.seeds).size, 0)
        self.assertEqual(set(two.arenas), set(range(4)))

    def test_development_sweep_assigns_every_pair_to_both_controller_seats(self):
        fixtures = RosterSchedule(DEV_DOMAIN, training=False, arenas=4).draw(DEV_BOUTS)
        self.assertEqual({tuple(row) for row in fixtures.builds[::2]},
                         set(product(range(BUILD_COUNT), repeat=2)))
        self.assertEqual(fixtures.seeds.size, DEV_BOUTS)
        batch = Mock()
        fixtures.reset(batch)
        batch.reset_builds.assert_called_once_with(
            fixtures.seeds, fixtures.builds, arenas=fixtures.arenas,
        )

    def test_drawing_across_sweeps_does_not_depend_on_batch_boundaries(self):
        full = RosterSchedule(TRAIN_DOMAIN, training=True, arenas=4)
        split = RosterSchedule(TRAIN_DOMAIN, training=True, arenas=4)
        all_rows = full.draw(3000)
        first = split.draw(782)
        second = split.draw(2218)
        for attr in ("seeds", "builds", "arenas", "seats"):
            np.testing.assert_equal(
                getattr(all_rows, attr), np.concatenate((getattr(first, attr), getattr(second, attr))),
            )
