import unittest

import numpy as np

from league_play import Matchmaking, Member
from self_play import TrainingDeadline, check_deadline
from width_league import CompletedUpdate, eligible_candidate


class WidthLeagueTests(unittest.TestCase):
    def test_only_a_complete_update_within_boundary_is_eligible(self):
        previous = CompletedUpdate(None, {"cpu_s": 392.7}, {})
        current = CompletedUpdate(None, {"cpu_s": 394.4}, {})
        self.assertIs(eligible_candidate(previous, current, 394.0), previous)
        self.assertIs(eligible_candidate(previous, current, 394.4), current)
        self.assertIs(eligible_candidate(previous, None, 394.0), previous)
        with self.assertRaisesRegex(ValueError, "before a complete training update"):
            eligible_candidate(None, current, 394.0)

    def test_cpu_deadline_uses_the_supplied_clock(self):
        check_deadline(lambda: False)
        with self.assertRaises(TrainingDeadline):
            check_deadline(lambda: True)

    def test_history_draw_does_not_shift_matched_role_assignments(self):
        def make_members(prefix, count):
            return [Member(f"{prefix}-{i}", object(), str(i)) for i in range(count)]

        seats = np.tile([0, 1], 192)
        rows = np.arange(len(seats))
        assignments = []
        for archive_size in (1, 5):
            league = Matchmaking(make_members("anchor", 4))
            league.snapshots = make_members("snapshot", archive_size)
            controllers = league.assignments(
                seats, np.random.default_rng(23), 0.25, matched=True,
                history_rng=np.random.default_rng(47),
            )
            opponents = controllers[rows, 1 - seats]
            np.testing.assert_array_equal(opponents[::2], opponents[1::2])
            self.assertEqual((opponents == -1).sum(), 96)
            self.assertEqual((opponents >= 4).sum(), 144)
            assignments.append(np.minimum(opponents, 4))
        np.testing.assert_array_equal(*assignments)


if __name__ == "__main__":
    unittest.main()
