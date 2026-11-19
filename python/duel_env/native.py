"""Direct, one-process access to both seats of the Rust duel."""
import ctypes as ct
from functools import lru_cache
from pathlib import Path
import sys

import numpy as np

ACTIONS = ("approach", "retreat", "weapon", "unarmed", "second_wind", "end_turn",
           "opportunity", "pass")
FIGHTER_FIELDS = ("hp", "position", "winds", "arrows", "reaction", "sapped", "slowed")
STATE_FIELDS = (
    "actor", "turn_owner", "phase", "winner", "turns_started", "distance",
    "movement_left", "action_available", "bonus_available", "movement_spent",
    *(f"{seat}_{field}" for seat in ("shield", "archer") for field in FIGHTER_FIELDS),
)
ACTOR = STATE_FIELDS.index("actor")
WINNER = STATE_FIELDS.index("winner")


@lru_cache(maxsize=1)
def _library():
    filename = {"darwin": "libdnd_duel_engine.dylib", "linux": "libdnd_duel_engine.so",
                "win32": "dnd_duel_engine.dll"}[sys.platform]
    path = Path(__file__).resolve().parents[2] / "target" / "release" / filename
    if not path.is_file():
        raise RuntimeError(f"Build the native core with cargo build --release (missing {path})")
    lib = ct.CDLL(str(path))
    lib.duel_abi_version.argtypes = []
    lib.duel_abi_version.restype = ct.c_uint32
    if lib.duel_abi_version() != 1:
        raise RuntimeError("Rebuild the matching Rust core with cargo build --release")
    ptr = ct.c_void_p
    integer_array = np.ctypeslib.ndpointer(dtype=np.int32, ndim=1, flags="C_CONTIGUOUS")
    for name, args, result in (
        ("duel_new", [ct.c_uint64], ptr),
        ("duel_free", [ptr], None),
        ("duel_step", [ptr, ct.c_uint32], ct.c_bool),
        ("duel_bot", [ptr], ct.c_int32),
        ("duel_read", [ptr, integer_array, ct.c_size_t], ct.c_int32),
    ):
        func = getattr(lib, name)
        func.argtypes = args
        func.restype = result
    return lib


class NativeDuel:
    """Owns one Rust Duel. Each instance is driven by one thread."""
    def __init__(self, seed: int):
        if not 0 <= seed < 2**64:
            raise ValueError("seed must fit u64")
        self._lib = _library()
        self._handle = self._lib.duel_new(seed)

    def snapshot(self):
        state = np.empty(len(STATE_FIELDS), dtype=np.int32)
        bits = self._lib.duel_read(self._handle, state, state.size)
        if bits < 0:
            raise RuntimeError("closed duel or incompatible ABI")
        mask = np.array([(bits >> i) & 1 for i in range(len(ACTIONS))], dtype=np.int8)
        return state, mask

    def step(self, action):
        if not isinstance(action, (int, np.integer)) or not 0 <= action < len(ACTIONS):
            raise ValueError(f"invalid action ID {action!r}")
        if not self._lib.duel_step(self._handle, int(action)):
            raise ValueError(f"illegal action {ACTIONS[action]}")

    def bot_action(self):
        action = self._lib.duel_bot(self._handle)
        if action < 0:
            raise RuntimeError("no decision pending")
        return action

    def close(self):
        if getattr(self, "_handle", None):
            self._lib.duel_free(self._handle)
            self._handle = None

    def __del__(self):
        self.close()

    def __copy__(self):
        raise TypeError("NativeDuel owns its Rust handle")

    def __deepcopy__(self, memo):
        raise TypeError("NativeDuel owns its Rust handle")

    def __reduce__(self):
        raise TypeError("NativeDuel owns its Rust handle")
