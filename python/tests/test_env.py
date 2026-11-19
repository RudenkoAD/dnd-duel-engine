import unittest

import gymnasium as gym
from gymnasium.utils.env_checker import check_env
import numpy as np

from duel_env import ACTIONS, DuelEnv, NativeDuel, STATE_FIELDS

PHASE = STATE_FIELDS.index("phase")
ACTOR = STATE_FIELDS.index("actor")


class EnvTests(unittest.TestCase):
    def test_gymnasium_contract(self):
        with gym.make("duel_env:DndDuel-v0") as env:
            check_env(env.unwrapped)

    def test_knockout_on_last_permitted_step_takes_priority(self):
        with DuelEnv() as env:
            env.reset(seed=42)
            steps = 0
            while not env.done:
                env.step(env.duel.bot_action())
                steps += 1
        with DuelEnv(max_steps=steps) as env:
            env.reset(seed=42)
            for _ in range(steps):
                _, _, terminated, truncated, _ = env.step(env.duel.bot_action())
            self.assertTrue(terminated)
            self.assertFalse(truncated)

    def test_episode_matches_two_seat_core_at_every_agent_boundary(self):
        seen_reacting = set()
        seen_acting = set()
        for seed in range(64):
            expected = NativeDuel(seed)
            with DuelEnv() as env:
                obs, info = env.reset(seed=seed)
                self.assertEqual(info["duel_seed"], seed)
                seat = obs["seat"]
                seen_acting.add(seat)
                for _ in range(256):
                    state, mask = expected.snapshot()
                    np.testing.assert_array_equal(state, obs["state"])
                    np.testing.assert_array_equal(mask, obs["action_mask"])
                    self.assertTrue(env.observation_space.contains(obs))
                    self.assertEqual(state[ACTOR], seat)
                    if state[PHASE] == 1:
                        seen_reacting.add(seat)
                    # Decline every reaction to exercise a different agent policy.
                    action = ACTIONS.index("pass") if state[PHASE] == 1 else expected.bot_action()
                    expected.step(action)
                    opponent_decisions = 0
                    while expected.snapshot()[0][ACTOR] == 1 - seat:
                        expected.step(expected.bot_action())
                        opponent_decisions += 1
                    obs, reward, terminated, truncated, info = env.step(action)
                    self.assertEqual(info["opponent_decisions"], opponent_decisions)
                    self.assertFalse(truncated)
                    if terminated:
                        final, mask = expected.snapshot()
                        np.testing.assert_array_equal(obs["state"], final)
                        self.assertFalse(mask.any() or obs["action_mask"].any())
                        self.assertEqual(reward, 1.0 if info["winner"] == seat else -1.0)
                        break
                else:
                    self.fail("episode budget exhausted")
            expected.close()
        self.assertEqual(seen_acting, {0, 1})
        # The baseline shield chases; the baseline archer retreats. In these
        # policies the shield's seat therefore receives learner reaction windows.
        self.assertIn(0, seen_reacting)

    def test_truncation_and_masked_out_choice(self):
        with DuelEnv(max_steps=1) as env:
            obs, _ = env.reset(seed=42)
            state = obs["state"].copy()
            obs["action_mask"][:] = 1
            obs, reward, terminated, truncated, info = env.step(ACTIONS.index("pass"))
            self.assertTrue(info["invalid_action"])
            self.assertLess(reward, 0)
            self.assertTrue(truncated)
            self.assertFalse(terminated)
            np.testing.assert_array_equal(state, obs["state"])
            with self.assertRaises(gym.error.ResetNeeded):
                env.step(0)
            env.reset(seed=42)
            _, _, terminated, truncated, info = env.step(ACTIONS.index("approach"))
            self.assertFalse(terminated or info["invalid_action"])
            self.assertTrue(truncated)

    def test_instance_isolation_and_rejection(self):
        a, b = NativeDuel(137), NativeDuel(137)
        try:
            with self.assertRaises(ValueError):
                a.step(ACTIONS.index("pass"))
            a.step(a.bot_action())
            b.step(b.bot_action())
            for x, y in zip(a.snapshot(), b.snapshot()):
                np.testing.assert_array_equal(x, y)
        finally:
            a.close()
            b.close()
        a.close()
        with self.assertRaises(RuntimeError):
            a.snapshot()
