"""Single-agent Gymnasium interface over the Rust decision loop."""
import gymnasium as gym
from gymnasium import spaces
import numpy as np

from .native import ACTIONS, ACTOR, WINNER, NativeDuel, STATE_FIELDS


class DuelEnv(gym.Env):
    metadata = {"render_modes": []}

    def __init__(self, max_steps=256):
        if not isinstance(max_steps, int) or max_steps < 1:
            raise ValueError("max_steps must be a positive integer")
        self.max_steps = max_steps
        self.action_space = spaces.Discrete(len(ACTIONS))
        self.observation_space = spaces.Dict({
            "state": spaces.Box(-1, np.iinfo(np.int32).max,
                                shape=(len(STATE_FIELDS),), dtype=np.int32),
            "seat": spaces.Discrete(2),
            "action_mask": spaces.MultiBinary(len(ACTIONS)),
        })
        self.duel = None
        self.done = True

    def reset(self, *, seed=None, options=None):
        super().reset(seed=seed)
        self.close()
        # A u64 seed replays the same fight in Rust. Larger Gym seeds are folded
        # through its own seeded generator. seed=None continues that generator.
        if seed is None or seed >= 2**64:
            seed = int(self.np_random.bit_generator.random_raw())
        self.duel = NativeDuel(seed)
        state, mask = self.duel.snapshot()
        self.seat = int(state[ACTOR])
        self.count = 0
        self.done = False
        return self._observe(state, mask), {"duel_seed": seed}

    def _observe(self, state, mask):
        self._mask = mask.copy()
        return {"state": state, "action_mask": mask, "seat": self.seat}

    def step(self, action):
        if self.done:
            raise gym.error.ResetNeeded("reset before stepping")
        if not self.action_space.contains(action):
            raise ValueError(f"invalid action ID {action!r}")
        action = int(action)
        self.count += 1
        invalid = not bool(self._mask[action])
        opponent_decisions = 0
        if not invalid:
            self.duel.step(action)
        state, mask = self.duel.snapshot()
        while state[ACTOR] == 1 - self.seat:
            self.duel.step(self.duel.bot_action())
            opponent_decisions += 1
            state, mask = self.duel.snapshot()
        winner = int(state[WINNER])
        terminated = winner >= 0
        truncated = not terminated and self.count >= self.max_steps
        self.done = terminated or truncated
        reward = (1.0 if winner == self.seat else -1.0) if terminated else (-0.05 if invalid else 0.0)
        info = {"winner": winner, "invalid_action": invalid, "opponent_decisions": opponent_decisions}
        return self._observe(state, mask), reward, terminated, truncated, info

    def close(self):
        if self.duel is not None:
            self.duel.close()
            self.duel = None
        self.done = True
