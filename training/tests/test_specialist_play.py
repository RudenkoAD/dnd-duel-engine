import unittest
from unittest.mock import Mock, patch

import numpy as np
import torch

from arena.native import ACTIONS, FEATURES, Batch
from specialist_play import CELLS, DEV, TRAIN, Fixtures, layout, run_bouts
from train_arena import ActorCritic


class FixturesTests(unittest.TestCase):
    def test_cover_all_ordered_class_level_map_cells_in_both_seats(self):
        schedule = Fixtures(TRAIN, training=True)
        seen = set()
        for _ in range(15):
            seeds, classes, levels, seats = schedule.draw(384)
            np.testing.assert_array_equal(seats[::2], 0)
            np.testing.assert_array_equal(seats[1::2], 1)
            np.testing.assert_array_equal(seeds[::2], seeds[1::2])
            np.testing.assert_array_equal(classes[::2], classes[1::2, ::-1])
            np.testing.assert_array_equal(levels[::2], levels[1::2])
            seen.update((left, right, level, layout(int(seed))) for (left, right), level, seed in
                        zip(classes[::2], levels[::2, 0], seeds[::2]))
        self.assertSetEqual(seen, set(map(tuple, CELLS)))

    def test_dev_matches_native_maps_and_uses_nontraining_seeds(self):
        seeds, classes, levels, _ = Fixtures(DEV, training=False).draw(384)
        self.assertTrue(all(int(seed) >> 40 == DEV >> 40 for seed in seeds))
        self.assertNotEqual(DEV >> 40, TRAIN >> 40)
        with Batch(384) as batch:
            batch.reset(np.arange(384), seeds, classes, levels)
            features, *_ = batch.observe()
        np.testing.assert_array_equal(features[:, 132:136].argmax(axis=1),
                                      [layout(int(seed)) for seed in seeds])


class RolloutTests(unittest.TestCase):
    def test_each_policy_handles_its_own_reactions_and_only_learner_enters_trace(self):
        torch.set_num_threads(1)
        fixture = Fixtures(TRAIN, training=True).draw(2)
        batch = Mock(n=2)
        features = np.zeros((2, FEATURES), np.float32)
        masks = np.ones((2, ACTIONS), bool)
        pending = np.array([-1, -1], dtype=np.int32)
        wins = np.array([0, 1], dtype=np.int32)
        batch.observe.side_effect = [
            (features, masks, np.array([0, 0]), pending),
            (features, masks, np.array([1, 1]), pending),
            (features, masks, np.array([-1, -1]), wins),
        ]
        batch.result_reasons.return_value = np.array([1, 1], dtype=np.int32)
        learner = ActorCritic(FEATURES, ACTIONS)
        champion = Mock()
        champion.choose.return_value = np.array([15], dtype=np.int32)
        with patch("specialist_play.sample_actions", return_value=torch.tensor([14])) as sample:
            trace, outcomes, counts, _ = run_bouts(batch, fixture, learner, champion)
        np.testing.assert_array_equal(batch.step.call_args_list[0].args[0], [14, 15])
        np.testing.assert_array_equal(batch.step.call_args_list[1].args[0], [15, 14])
        np.testing.assert_array_equal(outcomes, wins)
        np.testing.assert_array_equal(trace.last, [[0, -1], [-1, 1]])
        self.assertEqual(trace.size, 2)
        self.assertEqual(sample.call_count, 2)
        self.assertEqual(champion.choose.call_count, 2)
        self.assertEqual(counts["champion_decisions"], 2)
        self.assertEqual(counts["learner_decisions"], 2)


if __name__ == "__main__":
    unittest.main()
