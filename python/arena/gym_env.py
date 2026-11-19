"""Single-agent Gymnasium interface for the tactical arena."""
import gymnasium as gym
from gymnasium import spaces
import numpy as np

from .native import ACTIONS, CLASS_NAMES, FEATURES, ROSTER_LEVEL, VARIANTS_PER_CLASS, Batch, _integers

DEFAULT_CLASSES = (CLASS_NAMES.index("Fighter"), CLASS_NAMES.index("Wizard"))


class ArenaEnv(gym.Env):
    """Control one seat while a callback or the built-in baseline plays the other.

    Reset options accept two level-five roster IDs in `builds` or two
    `classes` indices to select the canonical presets, in native seat order.
    Select a layout with `arena` and a fixed seat with `seat`. By default the
    seed chooses the layout and the opening actor is controlled. The same action
    space applies to regular turns and reactions. A supplied opponent policy
    receives its own features and action mask and returns a legal action ID.
    Knockouts end the episode. A double knockout gives zero reward, while
    the native and Gymnasium time limits truncate play.
    """

    metadata = {"render_modes": []}

    def __init__(self, max_steps=256, opponent_policy=None):
        if not isinstance(max_steps, int) or isinstance(max_steps, bool) or max_steps < 1:
            raise ValueError("max_steps must be a positive integer")
        self.max_steps = max_steps
        if opponent_policy is not None and not callable(opponent_policy):
            raise TypeError("opponent_policy must be callable")
        self.opponent_policy = opponent_policy
        self.action_space = spaces.Discrete(ACTIONS)
        feature_limit = np.finfo(np.float32).max
        self.observation_space = spaces.Dict({
            "state": spaces.Box(-feature_limit, feature_limit, shape=(FEATURES,), dtype=np.float32),
            "seat": spaces.Discrete(2),
            "action_mask": spaces.MultiBinary(ACTIONS),
        })
        self.batch = None
        self.done = True

    def reset(self, *, seed=None, options=None):
        super().reset(seed=seed)
        if seed is None or seed >= 2**64:
            seed = int(self.np_random.bit_generator.random_raw())
        options = options or {}
        seat = options.get("seat")
        if seat is not None and (isinstance(seat, (bool, np.bool_)) or not isinstance(seat, (int, np.integer)) or seat not in (0, 1)):
            raise ValueError("seat must be 0 or 1")
        if self.batch is None:
            self.batch = Batch(1)
        self.done = True
        self._reset_batch(seed, options)
        observation = self.batch.observe()
        self.seat = int(observation[2][0] if seat is None else seat)
        self.count = 0
        observation, opening = self._advance_opponent(observation)
        result = int(observation[3][0])
        self.done = result != -1
        return self._observe(observation), {"duel_seed": seed, "opening_opponent_decisions": opening, "result": result}

    def _reset_batch(self, seed, options):
        if "levels" in options:
            _integers(options["levels"], (2,), ROSTER_LEVEL, ROSTER_LEVEL, np.int32)
        if "builds" in options:
            if "classes" in options:
                raise ValueError("choose builds or classes")
            builds = options["builds"]
        else:
            classes = _integers(options.get("classes", DEFAULT_CLASSES),
                                (2,), 0, len(CLASS_NAMES) - 1, np.int32)
            builds = classes * VARIANTS_PER_CLASS
        arenas = [options["arena"]] if "arena" in options else None
        self.batch.reset_builds([seed], [builds], arenas)

    def _observe(self, observation):
        states, masks, actors, _ = observation
        terminal = actors[0] < 0
        self._mask = np.zeros(ACTIONS, dtype=np.int8) if terminal else masks[0].astype(np.int8)
        return {
            "state": np.zeros(FEATURES, dtype=np.float32) if terminal else states[0].copy(),
            "action_mask": self._mask.copy(),
            "seat": self.seat,
        }

    def _advance_opponent(self, observation):
        decisions = 0
        while int(observation[3][0]) == -1 and int(observation[2][0]) == 1 - self.seat:
            features, masks, _, _ = observation
            if self.opponent_policy is None:
                action = self.batch.bot(expert=False)[0]
            else:
                action = self.opponent_policy(features[0], masks[0])
            if (not isinstance(action, (int, np.integer)) or isinstance(action, (bool, np.bool_))
                    or not 0 <= action < ACTIONS or not masks[0, action]):
                raise ValueError(f"opponent returned illegal action {action!r}")
            self.batch.step([action])
            decisions += 1
            observation = self.batch.observe()
        return observation, decisions

    def step(self, action):
        if self.done:
            raise gym.error.ResetNeeded("reset before stepping")
        if not self.action_space.contains(action):
            raise ValueError(f"invalid action ID {action!r}")
        action = int(action)
        self.count += 1
        invalid = not bool(self._mask[action])
        if not invalid:
            self.batch.step([action])
        observation, opponent_decisions = self._advance_opponent(self.batch.observe())
        result = int(observation[3][0])
        winner = result if result in (0, 1) else -1
        # The result code is shared by double knockouts and timeouts. Use the
        # public result reason rather than inspecting private combatant HP.
        double_ko = result == 2 and self.batch.state(viewer=self.seat)["result_reason"] == "double_ko"
        terminated = winner >= 0 or double_ko
        truncated = not terminated and (result == 2 or self.count >= self.max_steps)
        self.done = terminated or truncated
        if winner >= 0:
            reward = 1.0 if winner == self.seat else -1.0
        else:
            reward = -0.05 if invalid else 0.0
        info = {
            "winner": winner,
            "result": result,
            "double_ko": double_ko,
            "invalid_action": invalid,
            "opponent_decisions": opponent_decisions,
        }
        return self._observe(observation), reward, terminated, truncated, info

    def close(self):
        if self.batch is not None:
            self.batch.close()
            self.batch = None
        self.done = True


class LegacyArenaEnv(ArenaEnv):
    """Historical class-and-level fixtures for reproducing older episodes."""

    def _reset_batch(self, seed, options):
        if "builds" in options or "arena" in options:
            raise ValueError("use ArenaEnv for roster builds and explicit arenas")
        classes = options.get("classes", DEFAULT_CLASSES)
        levels = options.get("levels", (3, 3))
        self.batch.reset([0], [seed], [classes], [levels])


gym.register(id="DndArena-v0", entry_point="arena.gym_env:ArenaEnv")
gym.register(id="DndArenaLegacy-v0", entry_point="arena.gym_env:LegacyArenaEnv")
