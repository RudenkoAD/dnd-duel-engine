"""Forward-only tests. These do not create an optimizer or train either policy."""

from pathlib import Path
import tempfile
from types import SimpleNamespace
import time
import unittest
from unittest.mock import Mock, patch

import numpy as np
import torch

from training.migrate_abi1003 import migrate
from training.self_play import (
    DEV_DOMAIN, PROJECTILE_AIM_FEATURE, TRAIN_DOMAIN, EpisodeTrace, League,
    ROSTER_NEW_INPUTS, RepeatedChoices, Schedule, collect, load_model, phase_ids,
    projectile_target_loss,
)
from arena.policy import Policy


class MockBatch:
    """Two interleaved seat sequences, with a reaction before the opponent turn."""

    n = 2

    def reset(self, *_):
        self.turn = 0
        self.choices = []

    def observe(self):
        actors = ([0, 1], [1, 0], [0, 1], [-1, -1])[self.turn]
        features = np.zeros((self.n, 4), dtype=np.float32)
        masks = np.ones((self.n, 2), dtype=bool)
        outcomes = [-1, -1] if self.turn < 3 else [0, 1]
        return features, masks, np.array(actors), np.array(outcomes)

    def step(self, actions):
        self.choices.append(actions.tolist())
        self.turn += 1


class FixedPolicy(torch.nn.Module):
    def __init__(self, action):
        super().__init__()
        self.action = action

    def forward(self, features, mask):
        logits = torch.full(mask.shape, -1e9)
        logits[:, self.action] = 0
        return logits, torch.zeros(len(features))


class SelfPlayTests(unittest.TestCase):
    def test_only_designated_frozen_opponents_use_greedy_actions(self):
        seats = np.array([0, 1])
        fixtures = np.zeros(2), np.zeros((2, 2)), np.ones((2, 2)), seats
        controllers = np.array([[-1, 0], [1, -1]])
        learner = FixedPolicy(1)
        league = SimpleNamespace(opponents=(FixedPolicy(1), FixedPolicy(1)))
        batch = MockBatch()

        with patch("training.self_play.sample_actions",
                   side_effect=lambda logits: torch.zeros(len(logits), dtype=torch.long)):
            collect(batch, learner, league, fixtures, controllers, time.monotonic() + 1)
            self.assertEqual(batch.choices, [[0, 0], [0, 0], [0, 0]])
            deployed = Mock(spec=Policy)
            deployed.choose.return_value = np.array([1], dtype=np.int32)
            league.opponents = (deployed, FixedPolicy(1))
            collect(batch, learner, league, fixtures, controllers, time.monotonic() + 1)
            deployed.choose.assert_called_once()
            self.assertEqual(batch.choices, [[0, 0], [1, 0], [0, 0]])

    def test_projectile_loss_prefers_legal_foe_only_during_aim(self):
        features = torch.zeros(3, 1024)
        features[:2, PROJECTILE_AIM_FEATURE] = 1
        features[2, 336] = 1
        masks = torch.zeros(3, 128, dtype=torch.bool)
        masks[:, (0, 1, 16, 17, 126)] = True
        masks[1, (1, 17)] = False
        logits = torch.zeros(3, 128, requires_grad=True)
        loss = projectile_target_loss(logits.masked_fill(~masks, -1e9), features, masks)
        torch.testing.assert_close(loss, torch.log(torch.tensor(2.)))
        loss.backward()
        torch.testing.assert_close(logits.grad[0, (0, 1, 16, 17, 126)],
                                   torch.tensor([0.25, -0.25, 0.25, -0.25, 0.]))
        self.assertEqual(torch.count_nonzero(logits.grad[1:]), 0)

    def test_expanded_phase_onehots_remain_distinguishable(self):
        observations = np.zeros((6, 1024), dtype=np.float32)
        observations[0, 256] = observations[1, 329] = 1
        observations[2, 616] = observations[3, 631] = 1
        observations[4, 864] = observations[5, 895] = 1
        np.testing.assert_equal(phase_ids(observations), [0, 73, 74, 89, 90, 121])

    def test_subclass_phase_ids_use_both_factors(self):
        observations = np.zeros((5, 1024), dtype=np.float32)
        phases = np.array([140, 147, 148, 195, 203])
        rows = np.arange(phases.size)
        observations[rows, 116 + (phases - 140) % 8] = 1
        observations[rows, 124 + (phases - 140) // 8] = 1
        np.testing.assert_equal(phase_ids(observations), phases)

    def test_roster_warm_start_preserves_legacy_prediction(self):
        warm = Path(__file__).resolve().parents[1] / "runs" / "long-width" / "checkpoints" / "update-2901.npz"
        original = Policy.load(warm)
        model, initialized = load_model(warm, 2002, 1024, 128, roster_run=True)
        self.assertTrue(initialized)
        self.assertEqual(model.first.out_features, 256)
        np.testing.assert_equal(model.first.weight.detach().numpy()[:, ROSTER_NEW_INPUTS], 0)
        rng = np.random.default_rng(123)
        states = rng.normal(size=(4, 1024)).astype(np.float32)
        states[:, ROSTER_NEW_INPUTS] = 0
        choices = np.ones((4, 128), dtype=bool)
        logits, _ = model(torch.from_numpy(states), torch.from_numpy(choices))
        np.testing.assert_allclose(logits.detach().numpy(), original.logits(states), atol=2e-5)

    def test_same_actor_same_turn_repeats_only(self):
        tracker = RepeatedChoices(1)
        features = np.zeros((1, 1024), np.float32)
        lanes = np.array([0])
        chosen = np.array([25])
        self.assertFalse(tracker.check(lanes, np.array([0]), features, chosen)[0])
        self.assertFalse(tracker.check(lanes, np.array([1]), features, chosen)[0])
        self.assertTrue(tracker.check(lanes, np.array([0]), features, chosen)[0])
        features[0, 30] = 1. / 60
        self.assertFalse(tracker.check(lanes, np.array([0]), features, chosen)[0])

    def test_schedule_pairs_and_seed_disjointness(self):
        sets = []
        for namespace, training, n in ((TRAIN_DOMAIN, True, 1040), (DEV_DOMAIN, False, 660)):
            s = Schedule(namespace, training=training)
            seeds, classes, levels, seats = s.draw(n)
            np.testing.assert_equal(seeds[::2], seeds[1::2])
            np.testing.assert_equal(classes[::2], classes[1::2])
            np.testing.assert_equal(levels[::2], levels[1::2])
            np.testing.assert_equal(seats, np.tile([0, 1], n // 2))
            focal = classes[np.arange(n), seats]
            focal_level = levels[np.arange(n), seats]
            self.assertEqual(len(set(zip(focal, focal_level, seats))), 120)
            sets.append(set(seeds))
        self.assertTrue(sets[0].isdisjoint(sets[1]))

    def test_training_keeps_all_same_level_pairs_and_samples_all_25_level_pairs(self):
        schedule = Schedule(TRAIN_DOMAIN, training=True)
        seeds, classes, levels, _ = schedule.draw(1040)
        self.assertEqual(int((levels[:, 0] != levels[:, 1]).sum()), 260)
        same = levels[::2, 0] == levels[::2, 1]
        pairs = {(min(a, b), max(a, b), level) for (a, b), (level, _) in
                 zip(classes[::2][same], levels[::2][same])}
        self.assertEqual(len(pairs), 390)
        more_seeds, _, more_levels, _ = schedule.draw(23040)
        self.assertTrue(set(seeds).isdisjoint(more_seeds))
        self.assertEqual(set(map(tuple, more_levels)),
                         {(a, b) for a in range(1, 6) for b in range(1, 6)})
        dev = Schedule(DEV_DOMAIN, training=False).draw(660)
        self.assertTrue((dev[2][:, 0] == dev[2][:, 1]).all())

    def test_credit_follows_each_seat_across_interrupts(self):
        trace = EpisodeTrace(1)
        for seat in (0, 1, 0):
            trace.append(np.array([0]), np.array([seat]), np.ones((1, 2), np.float32),
                         np.ones((1, 2), bool), np.array([0]), np.zeros(1, np.float32),
                         np.zeros(1, np.float32))
        outcomes = np.array([0])
        _, _, _, _, advantage, returns = trace.finish(outcomes, 0.9, 0.75)
        torch.testing.assert_close(advantage, torch.tensor([0.675, -1.0, 1.0]))
        torch.testing.assert_close(advantage, returns)

    def test_draw_penalty_is_symmetric_and_does_not_replace_a_knockout(self):
        trace = EpisodeTrace(2)
        for seat in (0, 1):
            trace.append(np.array([0, 1]), np.array([seat, seat]),
                         np.ones((2, 2), np.float32), np.ones((2, 2), bool),
                         np.zeros(2, np.int64), np.zeros(2, np.float32),
                         np.zeros(2, np.float32))
        _, _, _, _, advantage, _ = trace.finish(
            np.array([2, 1]), 0.9, 0.75, draw_rewards=np.array([-0.2, -0.2]),
        )
        torch.testing.assert_close(advantage, torch.tensor([-0.2, -1.0, -0.2, 1.0]))

    def test_both_actors_use_assigned_learned_policy(self):
        batch = MockBatch()
        model = FixedPolicy(0)
        league = League(FixedPolicy(1), 4)
        controllers = np.array([[-1, 0], [-1, -1]])
        fixtures = Schedule(TRAIN_DOMAIN, training=True).draw(2)
        trace, outcomes, counts = collect(
            batch, model, league, fixtures, controllers, time.monotonic() + 5,
        )
        self.assertEqual(batch.choices, [[0, 0], [1, 0], [0, 0]])
        self.assertEqual(counts, {"learner": 5, "snapshot": 1})
        self.assertEqual(trace.last.tolist(), [[3, -1], [2, 4]])
        self.assertEqual(outcomes.tolist(), [0, 1])

    def test_migration_preserves_legacy_logits_and_masks_new_actions(self):
        source = Path(__file__).resolve().parents[1] / "checkpoints" / "arena_policy_abi1003.npz"
        old = Policy.load(source)
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "warm.npz"
            migrate(source, output)
            new = Policy.load(output).require_arena(2001, 1024, 128)
        random = np.random.default_rng(11)
        features = random.normal(size=(8, 1024)).astype(np.float32)
        np.testing.assert_allclose(old.logits(features[:, :256]), new.logits(features)[:, :32],
                                   atol=1e-5)
        mask = np.zeros((8, 128), dtype=bool)
        mask[:, 101] = True
        np.testing.assert_equal(new.choose(features, mask), np.repeat(101, len(features)))
        with self.assertRaises(ValueError):
            old.require_arena(2001, 1024, 128)


if __name__ == "__main__":
    unittest.main()
