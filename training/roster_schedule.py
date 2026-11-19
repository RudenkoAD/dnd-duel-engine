"""Level-five fixtures for new runs, with a fresh training and development domain."""

from dataclasses import dataclass

import numpy as np

from arena.native import ROSTER_LEVEL as LEVEL, ROSTER_SIZE as BUILD_COUNT
from arena.native import VARIANTS_PER_CLASS as BUILDS_PER_CLASS
from arena.native import arena_count


TRAIN_DOMAIN = 0x5235540000000000
DEV_DOMAIN = 0x5235440000000000
DEV_BOUTS = 2 * BUILD_COUNT**2


@dataclass(frozen=True)
class RosterFixtures:
    seeds: np.ndarray
    builds: np.ndarray
    arenas: np.ndarray
    seats: np.ndarray

    def reset(self, batch):
        batch.reset_builds(self.seeds, self.builds, arenas=self.arenas)


class RosterSchedule:
    """Sweep build pairs, assigning the focal build to each physical seat."""

    def __init__(self, seed, *, training, arenas=None):
        self.seed = seed
        self.training = training
        self.arenas = arena_count() if arenas is None else arenas
        if self.arenas <= 0:
            raise ValueError("at least one arena is required")
        self.rng = np.random.default_rng(seed)
        self.base = np.array([
            (left, right)
            for left in range(BUILD_COUNT)
            for right in range(BUILD_COUNT)
        ], dtype=np.int32)
        self.order = self._next_sweep()
        self.pairs_started = 0

    def _next_sweep(self):
        return self.rng.permutation(self.base) if self.training else self.base

    def draw(self, n):
        if n <= 0 or n % 2:
            raise ValueError("schedule batch must be positive and even")
        count = n // 2
        ids = np.arange(self.pairs_started, self.pairs_started + count)
        chunks = []
        while count:
            offset = self.pairs_started % len(self.order)
            take = min(count, len(self.order) - offset)
            chunks.append(self.order[offset:offset + take])
            count -= take
            self.pairs_started += take
            if offset + take == len(self.order):
                self.order = self._next_sweep()
        pairs = np.concatenate(chunks)
        builds = np.empty((n, 2), dtype=np.int32)
        builds[::2] = pairs
        builds[1::2] = pairs[:, ::-1]
        # Development visits every arena across consecutive sweeps.
        sweep, position = np.divmod(ids, len(self.order))
        arenas = (position + sweep) % self.arenas
        return RosterFixtures(
            np.repeat(ids.astype(np.uint64) + np.uint64(self.seed), 2),
            builds,
            np.repeat(arenas.astype(np.int32), 2),
            np.tile(np.array([0, 1], dtype=np.int32), len(ids)),
        )
