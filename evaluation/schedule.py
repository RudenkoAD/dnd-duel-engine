"""Reproducible held-out duel schedule. Seed domains are reserved before training."""

from dataclasses import dataclass
import hashlib


ARENAS = tuple(range(4))


@dataclass(frozen=True)
class BoutPair:
    focal: int
    opponent: int
    level: int
    arena: int
    seed: int


def seed_in_domain(domain: str, *keys: int) -> int:
    prefix = int.from_bytes({
        "train": b"TRA", "dev": b"DEV", "final": b"FIN",
        "s2-train": b"S2T", "s2-dev": b"S2D", "s2-final": b"S2F",
        "s3-final": b"S3F", "s3-smoke": b"S3X",
        "roster-parity": b"R5P",
    }[domain], "big")
    payload = ",".join(map(str, keys)).encode("ascii")
    suffix = int.from_bytes(hashlib.blake2b(payload, digest_size=5).digest(), "big")
    return (prefix << 40) | suffix


def arena_for_seed(seed: int) -> int:
    mask = (1 << 64) - 1
    seed = ((seed ^ (seed >> 30)) * 0xbf58476d1ce4e5b9) & mask
    seed = ((seed ^ (seed >> 27)) * 0x94d049bb133111eb) & mask
    return (seed ^ (seed >> 31)) % len(ARENAS)


def held_out_schedule(classes: int, levels: tuple[int, ...], repeats: int, *, domain: str = "final"):
    if classes < 2 or repeats < len(ARENAS) or not levels:
        raise ValueError("evaluation requires at least two classes, a level and four repeats")

    seeds = set()
    for focal in range(classes):
        for level in levels:
            for repeat in range(repeats):
                for offset in range(classes):
                    opponent = (focal + offset) % classes
                    arena = (repeat + focal + level + offset) % len(ARENAS)
                    nonce = 0
                    while True:
                        seed = seed_in_domain(domain, focal, opponent, level, arena, repeat, nonce)
                        if arena_for_seed(seed) == arena:
                            break
                        nonce += 1
                    if seed in seeds:
                        raise RuntimeError("duplicate held-out pair seed")
                    seeds.add(seed)
                    yield BoutPair(focal, opponent, level, arena, seed)
