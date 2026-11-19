"""Greedy checkpoint DEV probe for ABI 2002 projectile targeting.

Run from the repository root with PYTHONPATH=python. Every checkpoint uses the
same mirrored fixtures and, against uniform, the same per-fixture indexed
opponent draws. The report counts aim decisions, so separate darts each count
once and an all-dart choice counts once.
"""

import argparse
import hashlib
import json
from pathlib import Path

import numpy as np

from arena.native import ACTION_NAMES, ACTIONS, ARENA_ABI, CLASS_NAMES, FEATURES, Batch
from arena.policy import Policy

OPPONENTS = ("uniform", "heuristic", "expert", "self")
DEFAULT_OPPONENTS = ("uniform", "expert", "self")
# Extended phases 74–89 occupy observation channels 616–631.
PROJECTILE_AIM = 616 + (80 - 74)
PROJECTILE_SPELL = 545
SINGLE_SELF, SINGLE_FOE, ALL_SELF, ALL_FOE = 0, 1, 16, 17
PASS, GUESS = ACTION_NAMES.index("Pass"), 126
MAX_STEPS = 1024
NAMESPACE = "DEV/target-probe-v1"


def seed_for(*keys):
    payload = (NAMESPACE + ":" + ",".join(map(str, keys))).encode("ascii")
    return int.from_bytes(b"DEV" + hashlib.blake2b(payload, digest_size=5).digest(), "big")


def fixtures(levels, replicates):
    n = len(CLASS_NAMES)
    rows = [(focal, foe, level, seed_for(focal, foe, level, repeat))
            for level in levels for repeat in range(replicates)
            for focal in range(n) for foe in range(n)]
    classes = np.array([(a, b) for a, b, _, _ in rows], dtype=np.int32)
    level_pairs = np.array([(level, level) for _, _, level, _ in rows], dtype=np.int32)
    seeds = np.array([seed for _, _, _, seed in rows], dtype=np.uint64)
    if len(np.unique(seeds)) != len(seeds):
        raise RuntimeError("duplicate probe fixture seed")
    return classes, level_pairs, seeds


def rate(numerator, denominator):
    return numerator / denominator if denominator else None


def play(policy, opponent, seat, classes, levels, seeds):
    lanes = np.arange(len(seeds), dtype=np.int32)
    if opponent == "uniform":
        draws = np.array([
            np.random.default_rng(seed_for(int(seed), 1 - seat)).random(MAX_STEPS)
            for seed in seeds
        ])
        opponent_steps = np.zeros(len(lanes), dtype=np.int32)
    choices = np.zeros(ACTIONS, dtype=np.int64)
    choices_with_foe = np.zeros_like(choices)
    missile_choices = np.zeros_like(choices)
    missile_choices_with_foe = np.zeros_like(choices)
    bouts_self_aim = np.zeros(len(lanes), dtype=bool)
    bouts_self_aim_with_foe = np.zeros_like(bouts_self_aim)
    verified_labels = set()

    with Batch(len(lanes)) as batch:
        batch.reset(lanes, seeds, classes[:, ::-1].copy() if seat else classes, levels)
        for _ in range(MAX_STEPS):
            x, masks, actors, results = batch.observe()
            if not (actors >= 0).any():
                break
            focal = lanes[actors == seat]
            other = lanes[actors == 1 - seat]
            actions = np.full(len(lanes), -1, dtype=np.int32)
            if opponent == "self":
                active = lanes[actors >= 0]
                actions[active] = policy.choose(x[active], masks[active])
            else:
                if len(focal):
                    actions[focal] = policy.choose(x[focal], masks[focal])
                if len(other) and opponent == "uniform":
                    legal = masks[other]
                    rank = (draws[other, opponent_steps[other]] * legal.sum(axis=1)).astype(int)
                    actions[other] = (legal.cumsum(axis=1) > rank[:, None]).argmax(axis=1)
                    opponent_steps[other] += 1
                elif len(other):
                    actions[other] = batch.bot(expert=opponent == "expert")[other]

            aim = focal[x[focal, PROJECTILE_AIM] == 1]
            if len(aim):
                selected = actions[aim]
                foe_legal = masks[aim, SINGLE_FOE] | masks[aim, ALL_FOE]
                missiles = x[aim, PROJECTILE_SPELL] == 1
                np.add.at(choices, selected, 1)
                np.add.at(choices_with_foe, selected[foe_legal], 1)
                np.add.at(missile_choices, selected[missiles], 1)
                np.add.at(missile_choices_with_foe, selected[missiles & foe_legal], 1)
                self_aim = (selected == SINGLE_SELF) | (selected == ALL_SELF)
                bouts_self_aim[aim[self_aim]] = True
                bouts_self_aim_with_foe[aim[self_aim & foe_legal]] = True
                # Validate the ABI mapping against actual rendered aim menus.
                for action, target in ((SINGLE_SELF, "yourself"), (ALL_SELF, "yourself"),
                                       (SINGLE_FOE, "opposing adventurer"),
                                       (ALL_FOE, "opposing adventurer")):
                    if action not in verified_labels:
                        offering = aim[masks[aim, action]]
                        if len(offering):
                            label = batch._text(int(offering[0]), seat, action)
                            if target not in label:
                                raise RuntimeError(f"phase 80 action {action} has unexpected label {label!r}")
                            verified_labels.add(action)
            batch.step(actions)
        else:
            raise RuntimeError("target probe exceeded the engine's decision limit")
        reasons = batch.result_reasons()

    return {
        "bouts": len(lanes),
        "wins": int((results == seat).sum()),
        "draws": int((results == 2).sum()),
        "losses": int((results == 1 - seat).sum()),
        "bouts_with_self_aim": int(bouts_self_aim.sum()),
        "bouts_with_self_aim_when_foe_legal": int(bouts_self_aim_with_foe.sum()),
        "projectile": aim_summary(choices, choices_with_foe, len(lanes)),
        "magic_missile": aim_summary(missile_choices, missile_choices_with_foe, len(lanes)),
        "turn_limit_draws": int((reasons == 3).sum()),
        "decision_limit_draws": int((reasons == 4).sum()),
        "verified_label_ids": sorted(verified_labels),
    }


def aim_summary(choices, with_foe, bouts):
    def counts(values):
        self_single, foe_single, self_all, foe_all = map(
            int, values[[SINGLE_SELF, SINGLE_FOE, ALL_SELF, ALL_FOE]]
        )
        decisions = int(values.sum())
        guesses, declined = int(values[GUESS]), int(values[PASS])
        own_companion = int(values[[2, 18]].sum())
        foe_companion = int(values[[3, 19]].sum())
        objects = int(values[4:14].sum())
        aimed = decisions - declined
        self_aim = self_single + self_all
        foe_aim = foe_single + foe_all
        other = aimed - self_aim - foe_aim
        offensive = foe_aim + foe_companion + objects + guesses
        return dict(
            decisions=decisions, aimed=aimed, self_aim=self_aim, foe_aim=foe_aim, other_aim=other,
            self_aim_rate=rate(self_aim, decisions), foe_aim_rate=rate(foe_aim, decisions),
            other_aim_rate=rate(other, decisions), self_single=self_single, foe_single=foe_single,
            self_all=self_all, foe_all=foe_all, ally_companion=own_companion,
            foe_companion=foe_companion, objects=objects, guessed=guesses, declined=declined,
            offensive_aim=offensive, offensive_aim_per_bout=offensive / bouts,
            choices={str(i): int(values[i]) for i in np.flatnonzero(values)},
        )

    return {"all_windows": counts(choices), "foe_legal_windows": counts(with_foe)}


def run_probe(policy, opponents=DEFAULT_OPPONENTS, replicates=4, levels=(1, 2, 3, 4, 5)):
    if ARENA_ABI != 2002:
        raise ValueError("target probe requires arena ABI 2002")
    if replicates < 1 or not levels or any(level not in range(1, 6) for level in levels):
        raise ValueError("provide positive replicates and levels in 1–5")
    if not opponents or any(opponent not in OPPONENTS for opponent in opponents):
        raise ValueError(f"choose opponents from {', '.join(OPPONENTS)}")
    policy.require_arena(ARENA_ABI, FEATURES, ACTIONS)
    classes, level_pairs, seeds = fixtures(levels, replicates)
    return {
        "abi": ARENA_ABI, "namespace": NAMESPACE, "paired_fixtures": len(seeds),
        "levels": list(levels), "replicates": replicates,
        "opponents": {
            opponent: {
                seat: play(policy, opponent, seat, classes, level_pairs, seeds) for seat in (0, 1)
            } for opponent in opponents
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checkpoints", type=Path, nargs="+")
    parser.add_argument("--opponents", default=",".join(DEFAULT_OPPONENTS))
    parser.add_argument("--replicates", type=int, default=4)
    parser.add_argument("--levels", type=int, nargs="+", default=(1, 2, 3, 4, 5))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = {
        str(path): run_probe(Policy.load(path), args.opponents.split(","), args.replicates, args.levels)
        for path in args.checkpoints
    }
    output = json.dumps(report, sort_keys=True, indent=2)
    if args.output:
        args.output.write_text(output + "\n")
    else:
        print(output)


if __name__ == "__main__":
    main()
