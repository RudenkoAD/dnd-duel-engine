from gymnasium.envs.registration import register

from .env import DuelEnv
from .native import ACTIONS, NativeDuel, STATE_FIELDS

register(id="DndDuel-v0", entry_point="duel_env.env:DuelEnv")

__all__ = ["DuelEnv", "NativeDuel", "ACTIONS", "STATE_FIELDS"]
