import unittest

import numpy as np

from league_play import Matchmaking, Member, probabilities


def member(label, score=0.5):
    return Member(label, object(), label, score=score)


class MatchmakingTests(unittest.TestCase):
    def test_prioritizes_hard_opponents_and_preserves_exploration(self):
        weights = probabilities([member("hard", 0.25), member("easy", 0.95)])
        self.assertGreater(weights[0], weights[1])
        self.assertGreaterEqual(weights[1], 0.15)
        self.assertAlmostEqual(weights.sum(), 1.0)

    def test_each_assignment_uses_the_same_opponent_in_both_seats(self):
        league = Matchmaking([member("initial"), member("control")])
        league.snapshots.extend(member(f"checkpoint-{i}") for i in range(10))
        fixture_seats = np.tile([0, 1], 1000)
        controllers = league.assignments(fixture_seats, np.random.default_rng(5), 0.25)
        rows = np.arange(len(fixture_seats))
        self.assertTrue(np.all(controllers[rows, fixture_seats] == -1))
        opponent_ids = controllers[rows, 1 - fixture_seats]
        np.testing.assert_array_equal(opponent_ids[::2], opponent_ids[1::2])
        self.assertTrue(0.20 < (opponent_ids == -1).mean() < 0.30)
        self.assertTrue(0.45 < (opponent_ids[opponent_ids >= 0] < 2).mean() < 0.55)
        self.assertEqual(len(league.opponents), 5)
        self.assertIs(league.active[-1], league.snapshots[-1])
        self.assertEqual(len(league.report()), 12)

    def test_matched_specialists_replace_only_historical_anchor_fixtures(self):
        control = Matchmaking([member("initial"), member("historical")])
        treatment = Matchmaking([member("initial"), member("historical")],
                                [member("specialist-a"), member("specialist-b")])
        seats = np.tile([0, 1], 192)
        rows = np.arange(len(seats))
        for snapshot_count in (0, 1, 5):
            with self.subTest(snapshots=snapshot_count):
                assigned = []
                for league in (control, treatment):
                    league.snapshots = [member(f"checkpoint-{i}") for i in range(snapshot_count)]
                    controllers = league.assignments(seats, np.random.default_rng(5), 0.25,
                                                     matched=True)
                    assigned.append(controllers[rows, 1 - seats])
                control_opponents, treatment_opponents = assigned
                np.testing.assert_array_equal(treatment_opponents[::2], treatment_opponents[1::2])
                self.assertEqual((control_opponents == -1).sum(), 96)
                self.assertEqual((control_opponents >= 2).sum(), 144 if snapshot_count else 0)
                replaced = treatment_opponents >= len(control.active)
                self.assertTrue(np.all((0 <= control_opponents[replaced])
                                       & (control_opponents[replaced] < 2)))
                self.assertEqual((treatment_opponents == len(control.active)).sum(), 48)
                self.assertEqual((treatment_opponents == len(control.active) + 1).sum(), 48)
                np.testing.assert_array_equal(control_opponents[~replaced],
                                              treatment_opponents[~replaced])

    def test_observes_scores_only_for_frozen_opponents(self):
        league = Matchmaking([member("wins"), member("losses"), member("draws")])
        seats = np.tile([0, 1], 4)
        controllers = np.full((len(seats), 2), -1)
        controllers[np.arange(len(seats)), 1 - seats] = np.repeat([0, 1, 2, -1], 2)
        league.observe(seats, controllers, np.array([0, 1, 1, 0, 2, 2, 0, 1]))
        self.assertGreater(league.anchors[0].score, 0.5)
        self.assertLess(league.anchors[1].score, 0.5)
        self.assertEqual(league.anchors[2].score, 0.5)
        self.assertEqual([anchor.bouts for anchor in league.anchors], [2, 2, 2])


if __name__ == "__main__":
    unittest.main()
