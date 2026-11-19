"""Audit learned-versus-learned bouts for loops, time limits, and decision coverage."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import time

import numpy as np

from arena.native import ACTIONS, ARENA_ABI, FEATURES, Batch
from arena.policy import Policy
from self_play import DEV_DOMAIN, Schedule, phase_ids, termination

AUDIT_DOMAIN = DEV_DOMAIN + 0x20000000


def summarize(values):
    return {
        "median": float(np.median(values)),
        "p95": float(np.percentile(values, 95)),
        "max": int(max(values)),
    }


def audit(focal, other, n, stochastic):
    seed_rng = np.random.default_rng(AUDIT_DOMAIN)
    seeds, classes, levels, seats = Schedule(AUDIT_DOMAIN, training=False).draw(n)
    lanes = np.arange(n, dtype=np.int32)
    action_counts = Counter()
    phase_counts = Counter()
    phase_repeats = Counter()
    choice_repeats = Counter()
    decisions_per_lane = np.zeros(n, dtype=np.int32)
    gear_per_lane = np.zeros(n, dtype=np.int32)
    repeat_per_lane = np.zeros(n, dtype=np.int32)
    turn_id = np.full(n, -1, dtype=np.int32)
    turn_choices = np.zeros(n, dtype=np.int32)
    turn_main_choices = np.zeros(n, dtype=np.int32)
    finished_turns = []
    finished_main_choices = []
    fingerprints = [[set(), set()] for _ in lanes]
    with Batch(n) as batch:
        batch.reset(lanes, seeds, classes, levels)
        start = time.monotonic()
        for _ in range(2048):
            x, mask, actors, results = batch.observe()
            active = lanes[actors >= 0]
            if not len(active):
                break
            actions = np.full(n, -1, np.int32)
            for policy, selected in ((focal, active[actors[active] == seats[active]]),
                                     (other, active[actors[active] != seats[active]])):
                if len(selected):
                    actions[selected] = policy.act_batch(
                        x[selected], mask[selected], seed_rng if stochastic else None,
                    )
            turns = np.rint(60 * x[active, 30]).astype(np.int32)
            phases = phase_ids(x[active])
            for lane, seat, turn, phase, action in zip(
                    active, actors[active], turns, phases, actions[active]):
                if turn != turn_id[lane]:
                    if turn_choices[lane]:
                        finished_turns.append(turn_choices[lane])
                        finished_main_choices.append(turn_main_choices[lane])
                    turn_id[lane] = turn
                    turn_choices[lane] = turn_main_choices[lane] = 0
                    fingerprints[lane][0].clear()
                    fingerprints[lane][1].clear()
                action_counts[f"{phase}:{action}"] += 1
                phase_counts[int(phase)] += 1
                decisions_per_lane[lane] += 1
                turn_choices[lane] += 1
                turn_main_choices[lane] += phase == 0
                gear = (phase == 0 and 108 <= action <= 119) or 66 <= phase <= 72
                gear_per_lane[lane] += gear
                fingerprint = (x[lane].tobytes(), int(action))
                history = fingerprints[lane][seat]
                if fingerprint in history:
                    phase_repeats[int(phase)] += 1
                    choice_repeats[f"{phase}:{action}"] += 1
                    repeat_per_lane[lane] += 1
                history.add(fingerprint)
            batch.step(actions)
        else:
            raise RuntimeError("audit exceeded native decision count")
        reasons = termination(batch)

    finished_turns.extend(turn_choices[turn_choices > 0])
    finished_main_choices.extend(turn_main_choices[turn_choices > 0])
    wins = results == seats
    draws = results == 2
    return {
        "seed_namespace": "S2D / independent self-play audit",
        "stochastic": stochastic,
        "bouts": n,
        "wins": int(wins.sum()),
        "draws": int(draws.sum()),
        "losses": int((~wins & ~draws).sum()),
        "elapsed_s": round(time.monotonic() - start, 3),
        "termination": reasons,
        "decisions_per_bout": summarize(decisions_per_lane),
        "gear_decisions_per_bout": summarize(gear_per_lane),
        "decisions_per_turn": summarize(finished_turns),
        "main_decisions_per_turn": summarize(finished_main_choices),
        "exact_repeat_bouts": int((repeat_per_lane > 0).sum()),
        "exact_repeated_public_state_action": int(repeat_per_lane.sum()),
        "exact_repeat_by_phase": phase_repeats,
        "exact_repeat_by_phase_action": choice_repeats,
        "phase_counts": phase_counts,
        "phase_action_counts": action_counts,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checkpoint", type=Path)
    parser.add_argument("--other", type=Path,
                        help="another learned opponent; defaults to the same policy")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--repeats", type=int, default=2)
    parser.add_argument("--stochastic", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.repeats <= 10:
        parser.error("--repeats must be between 1 and 10")
    other = args.other or args.checkpoint
    result = audit(
        Policy.load(args.checkpoint).require_arena(ARENA_ABI, FEATURES, ACTIONS),
        Policy.load(other).require_arena(ARENA_ABI, FEATURES, ACTIONS),
        660 * args.repeats, args.stochastic,
    )
    result.update({
        "policy_sha256": hashlib.sha256(args.checkpoint.read_bytes()).hexdigest(),
        "other_sha256": hashlib.sha256(other.read_bytes()).hexdigest(),
    })
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered)
    else:
        print(rendered, end="")


if __name__ == "__main__":
    main()
