import unittest
from unittest.mock import patch

import gymnasium as gym
from gymnasium.utils.env_checker import check_env
import numpy as np

from arena.gym_env import ArenaEnv, LegacyArenaEnv
from arena.native import ACTION_NAMES, CLASS_NAMES, Batch

END = ACTION_NAMES.index("End")
PASS = ACTION_NAMES.index("Pass")
DEFEND = ACTION_NAMES.index("Defend")


class ArenaEnvTests(unittest.TestCase):
    def test_gymnasium_contract(self):
        with gym.make("arena.gym_env:DndArena-v0") as env:
            check_env(env.unwrapped)

    def test_reset_options_set_classes_and_levels_in_native_seat_order(self):
        classes = [CLASS_NAMES.index("Monk"), CLASS_NAMES.index("Bard")]
        levels = [1, 5]
        seed = 2**64 - 1
        with LegacyArenaEnv() as env, Batch(1) as expected:
            options = {"classes": classes, "levels": levels}
            obs, info = env.reset(seed=seed, options=options)
            expected.reset([0], [seed], [classes], [levels])
            states, masks, actors, _ = expected.observe()
            self.assertEqual(info["duel_seed"], seed)
            self.assertEqual(obs["seat"], actors[0])
            np.testing.assert_array_equal(obs["state"], states[0])
            np.testing.assert_array_equal(obs["action_mask"], masks[0])

    def test_roster_options_match_native_fixtures(self):
        with ArenaEnv() as env, Batch(1) as expected:
            options = {"builds": [19, 35], "arena": 2}
            obs, _ = env.reset(seed=81, options=options)
            expected.reset_builds([81], [options["builds"]], [options["arena"]])
            states, masks, actors, _ = expected.observe()
            self.assertEqual(obs["seat"], actors[0])
            np.testing.assert_array_equal(obs["state"], states[0])
            np.testing.assert_array_equal(obs["action_mask"], masks[0])
            with self.assertRaises(ValueError):
                env.reset(seed=81, options={**options, "levels": [3, 4]})

    def test_default_and_canonical_class_selection_are_level_five(self):
        with ArenaEnv() as env:
            env.reset(seed=18)
            fighters = env.batch.state()["fighters"]
            self.assertEqual([f["build_id"] for f in fighters], [12, 33])
            self.assertEqual([f["level"] for f in fighters], [5, 5])
            env.reset(seed=19, options={"classes": [1, 7], "arena": 2, "levels": [5, 5]})
            state = env.batch.state()
            self.assertEqual(state["layout"], 2)
            self.assertEqual([f["build_id"] for f in state["fighters"]], [3, 21])
            with self.assertRaises(ValueError):
                env.reset(seed=18, options={"classes": [1, 7], "levels": [4, 5]})

    def test_episode_matches_core_through_opponent_and_reaction_decisions(self):
        saw_normal_decisions = False
        reaction_choices = set()
        saw_winner = False
        for seed in range(12):
            classes, levels = [4, 11], [3, 3]
            expected = Batch(1)
            try:
                expected.reset([0], [seed], [classes], [levels])
                with LegacyArenaEnv() as env:
                    obs, info = env.reset(seed=seed, options={"classes": classes, "levels": levels})
                    self.assertEqual(info["duel_seed"], seed)
                    for _ in range(env.max_steps):
                        states, masks, actors, _ = expected.observe()
                        np.testing.assert_array_equal(obs["state"], states[0])
                        np.testing.assert_array_equal(obs["action_mask"], masks[0])
                        self.assertTrue(env.observation_space.contains(obs))
                        self.assertEqual(obs["seat"], actors[0])
                        saw_normal_decisions |= bool(masks[0, END])
                        action = int(expected.bot(expert=False)[0])
                        if masks[0, PASS]:
                            if seed % 2:
                                action = PASS
                            reaction_choices.add(action)
                        self.assertTrue(masks[0, action])
                        expected.step([action])
                        opponent_decisions = 0
                        while True:
                            _, _, actors, results = expected.observe()
                            if results[0] != -1 or actors[0] == obs["seat"]:
                                break
                            expected.step(expected.bot(expert=False))
                            opponent_decisions += 1
                        obs, reward, terminated, truncated, info = env.step(action)
                        self.assertEqual(info["opponent_decisions"], opponent_decisions)
                        self.assertEqual(info["result"], results[0])
                        self.assertFalse(info["invalid_action"])
                        if terminated or truncated:
                            self.assertEqual(terminated, results[0] in (0, 1))
                            if terminated:
                                saw_winner = True
                                self.assertFalse(truncated)
                                self.assertFalse(obs["action_mask"].any())
                                self.assertEqual(reward, 1.0 if results[0] == obs["seat"] else -1.0)
                            break
                    else:
                        self.fail("episode budget exhausted")
            finally:
                expected.close()
        self.assertTrue(saw_normal_decisions)
        self.assertTrue({PASS, DEFEND}.issubset(reaction_choices))
        self.assertTrue(saw_winner)

    def test_masked_choice_truncates_without_advancing_native_state(self):
        with ArenaEnv(max_steps=1) as env:
            obs, _ = env.reset(seed=42)
            initial_state = obs["state"].copy()
            invalid = int(np.flatnonzero(obs["action_mask"] == 0)[0])
            obs["action_mask"][:] = 1
            with self.assertRaises(ValueError):
                env.step(env.action_space.n)
            obs, reward, terminated, truncated, info = env.step(invalid)
            self.assertTrue(info["invalid_action"])
            self.assertEqual(info["result"], -1)
            self.assertEqual(reward, -0.05)
            self.assertTrue(truncated)
            self.assertFalse(terminated)
            np.testing.assert_array_equal(initial_state, obs["state"])
            with self.assertRaises(gym.error.ResetNeeded):
                env.step(0)
            env.reset(seed=42)
            _, _, terminated, truncated, info = env.step(int(env.batch.bot(expert=False)[0]))
            self.assertFalse(terminated or info["invalid_action"])
            self.assertTrue(truncated)

    def test_double_knockout_is_a_terminal_draw(self):
        wizard = CLASS_NAMES.index("Wizard")
        fireball = ACTION_NAMES.index("Spell4")

        def aim_at_both_fighters(mask, phase):
            if phase == 0:
                return fireball if mask[fireball] else END
            return {5: 3, 6: 4, 19: 0}[phase]  # E4, on the arena floor

        def opponent(_features, mask):
            return aim_at_both_fighters(mask, env.batch.state(viewer=1)["phase"])

        with ArenaEnv(max_steps=16, opponent_policy=opponent) as env:
            obs, _ = env.reset(seed=1, options={"classes": [wizard, wizard], "levels": [5, 5], "seat": 0})
            for _ in range(env.max_steps):
                phase = env.batch.state(viewer=0)["phase"]
                action = aim_at_both_fighters(obs["action_mask"], phase)
                self.assertTrue(obs["action_mask"][action])
                obs, reward, terminated, truncated, info = env.step(action)
                if terminated or truncated:
                    break
            self.assertTrue(terminated)
            self.assertFalse(truncated)
            self.assertEqual(reward, 0.0)
            self.assertEqual(info["result"], 2)
            self.assertEqual(info["winner"], -1)
            self.assertTrue(info["double_ko"])
            self.assertFalse(obs["action_mask"].any())

    def test_native_turn_limit_truncates_with_both_fighters_alive(self):
        with ArenaEnv() as env:
            env.reset(seed=5)
            with patch.object(env.batch, "bot", return_value=[END]):
                for _ in range(30):
                    obs, reward, terminated, truncated, info = env.step(END)
            self.assertFalse(terminated)
            self.assertTrue(truncated)
            self.assertEqual(reward, 0.0)
            self.assertEqual(info["result"], 2)
            self.assertEqual(info["winner"], -1)
            self.assertFalse(info["double_ko"])
            self.assertFalse(obs["action_mask"].any())
