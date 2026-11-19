"""Fixed final fixtures shared by the strength and balance evaluations."""

import hashlib
from itertools import combinations_with_replacement
from pathlib import Path
import struct

import numpy as np

from arena.native import ARENA_NAMES, ROSTER_SIZE

DOMAIN = "R36-L2901-FINAL-v1"
PREFIX = int.from_bytes(b"R36", "big")
REPEATS = 8
MANIFEST = Path(__file__).with_name("fixtures.npz")


def create():
    pairs = np.asarray(list(combinations_with_replacement(range(ROSTER_SIZE), 2)), dtype=np.uint8)
    seeds = np.empty((len(pairs), len(ARENA_NAMES), REPEATS), dtype=np.uint64)
    for pair_index, (first, second) in enumerate(pairs):
        for arena in range(len(ARENA_NAMES)):
            for repeat in range(REPEATS):
                message = DOMAIN.encode("ascii") + struct.pack("4B", first, second, arena, repeat)
                suffix = int.from_bytes(hashlib.blake2b(message, digest_size=5).digest(), "big")
                seeds[pair_index, arena, repeat] = (PREFIX << 40) | suffix
    if np.unique(seeds).size != seeds.size:
        raise RuntimeError("Duplicate final fixture seed")
    return pairs, seeds


if __name__ == "__main__":
    pairs, seeds = create()
    np.savez_compressed(MANIFEST, pairs=pairs, seeds=seeds, domain=DOMAIN, arenas=ARENA_NAMES)
