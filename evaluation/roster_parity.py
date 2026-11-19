"""Measure fixture balance with the same controller on both sides."""

import argparse
from dataclasses import dataclass
import hashlib
from itertools import combinations, product
import json
from pathlib import Path
import shlex
import sys

import numpy as np

from arena.native import (
    ACTIONS, ARENA_ABI, CLASS_NAMES, FEATURES, ROSTER_LEVEL, ROSTER_NEW_INPUTS,
    VARIANTS_PER_CLASS, Batch, _lib,
)
from arena.policy import Policy
from schedule import ARENAS, arena_for_seed, seed_in_domain


ARMS = ("legacy", "level5", "roster")
LABELS = ("Legacy levels", "Level 5, canonical", "Level 5, full roster")
REASONS = ("ongoing", "defeat", "double_ko", "turn_limit", "decision_limit")
CLASSES = tuple(combinations(range(len(CLASS_NAMES)), 2))
VARIANTS = tuple(product(range(VARIANTS_PER_CLASS), repeat=2))
DOMAIN = "roster-parity"
BOOTSTRAP_SEED = 0x523550
BOOTSTRAP_SAMPLES = 2000


@dataclass(frozen=True)
class Fixtures:
    seeds: np.ndarray
    legacy_levels: np.ndarray


def fixtures(repeats):
    """Use identical seed and arena blocks across arms and build assignments."""
    seeds = np.empty((len(CLASSES), len(ARENAS), repeats), dtype=np.uint64)
    levels = np.empty((*seeds.shape, 2), dtype=np.int32)
    for pair, (left, right) in enumerate(CLASSES):
        for arena, _ in enumerate(ARENAS):
            for repeat in range(repeats):
                nonce = 0
                while True:
                    seed = seed_in_domain(DOMAIN, left, right, arena, repeat, nonce)
                    if arena_for_seed(seed) == arena:
                        break
                    nonce += 1
                seeds[pair, arena, repeat] = seed
                # Match the archived schedule's 3:1 split and rotate level IDs.
                if repeat % 4 == 3:
                    block = repeat // 8
                    first = (pair + arena // 2 + block) % 5
                    second = (first + 1 + (pair // 5 + arena // 2 + block) % 4) % 5
                    assignment = (first + 1, second + 1)
                    reverse = (arena % 2 == 1) != (repeat % 8 == 7)
                    levels[pair, arena, repeat] = assignment[::-1] if reverse else assignment
                else:
                    level = (pair + arena + repeat - repeat // 4) % 5 + 1
                    levels[pair, arena, repeat] = level
    if np.unique(seeds).size != seeds.size:
        raise RuntimeError("duplicate parity fixture seed")
    return Fixtures(seeds, levels)


def run_arm(plan, arm, batch_size, policy=None):
    repeats = plan.seeds.shape[-1]
    variants = VARIANTS if arm == "roster" else ((0, 0),)
    keys = list(product(range(len(CLASSES)), range(len(variants)), range(len(ARENAS)),
                        range(repeats), range(2)))
    raw = np.zeros((len(keys), 4), dtype=np.int16)
    for start in range(0, len(keys), batch_size):
        chunk = keys[start:start + batch_size]
        pair, variant, arenas, repeat, seats = np.asarray(chunk, dtype=np.int32).T
        classes = np.asarray(CLASSES, dtype=np.int32)[pair]
        seeds = plan.seeds[pair, arenas, repeat]
        levels = plan.legacy_levels[pair, arenas, repeat]
        builds = classes * VARIANTS_PER_CLASS + np.asarray(variants, dtype=np.int32)[variant]
        swapped = seats == 1
        classes[swapped] = classes[swapped, ::-1]
        levels[swapped] = levels[swapped, ::-1]
        builds[swapped] = builds[swapped, ::-1]
        indices = np.arange(len(chunk), dtype=np.int32)
        with Batch(len(chunk)) as batch:
            if arm == "roster":
                batch.reset_builds(seeds, builds, arenas=arenas)
            else:
                if arm == "level5":
                    levels.fill(ROSTER_LEVEL)
                batch.reset(indices, seeds, classes, levels)
            for _ in range(2048):
                if policy is None:
                    actions = batch.bot(expert=True)
                    if not (actions >= 0).any():
                        break
                else:
                    state, legal, actors, _ = batch.observe()
                    active = actors >= 0
                    if not active.any():
                        break
                    actions = np.full(len(chunk), -1, dtype=np.int32)
                    actions[active] = policy.choose(state[active], legal[active])
                try:
                    batch.step(actions)
                except ValueError as error:
                    raise RuntimeError(f"{arm} fixture batch begins at row {start}: {error}") from error
            else:
                raise RuntimeError("parity fights did not finish")
            _, _, _, winners = batch.observe()
            reasons = batch.result_reasons()
            if not np.isin(winners, [0, 1, 2]).all() or not np.isin(reasons, [1, 2, 3, 4]).all():
                raise RuntimeError("parity fight has an invalid terminal result")
            for lane in np.flatnonzero(reasons == 1):
                winner = int(winners[lane])
                fighter = batch.state(int(lane), viewer=winner)["fighters"][winner]
                raw[start + lane, 2:] = fighter["hp"], fighter["max_hp"]
        outcomes = np.where(winners == 2, 0, np.where(winners == seats, 1, -1))
        raw[start:start + len(chunk), 0] = outcomes
        raw[start:start + len(chunk), 1] = reasons
    return raw.reshape(len(CLASSES), len(variants), len(ARENAS), repeats, 2, 4)


def statistics(raw):
    outcomes = raw[..., 0]
    reasons = raw[..., 1]
    by_build = outcomes.mean(axis=(2, 3, 4)) / 2
    by_class = by_build.mean(axis=1)
    gaps = np.abs(by_class)
    knockouts = int((reasons == 1).sum())
    one_sided = int(((reasons == 1) & (4 * raw[..., 2] >= 3 * raw[..., 3])).sum())
    return {
        "bouts": int(outcomes.size),
        "mean_class_gap": float(gaps.mean()),
        "p90_class_gap": float(np.quantile(gaps, .90)),
        "lopsided_70_30": int((gaps >= .20 - 1e-10).sum()),
        "lopsided_80_20": int((gaps >= .30 - 1e-10).sum()),
        "mean_build_gap": float(np.abs(by_build).mean()),
        "draw_rate": float((outcomes == 0).mean()),
        "knockouts": knockouts,
        "knockouts_at_75_percent_hp": one_sided,
        "share_of_knockouts_at_75_percent_hp": one_sided / knockouts if knockouts else None,
        "share_of_bouts_at_75_percent_hp": one_sided / outcomes.size,
        "termination": dict(zip(REASONS, np.bincount(reasons.ravel(), minlength=len(REASONS)).tolist())),
        "class_scores": (by_class + .5).tolist(),
        "build_scores": (by_build + .5).tolist(),
    }


def paired_intervals(raw, samples=BOOTSTRAP_SAMPLES):
    """Resample a common set of repeats per class pair, arena and mixture stratum."""
    scores = [raw[arm][..., 0].mean(axis=(1, 4)) / 2 for arm in ARMS]
    one_sided = []
    for arm in ARMS:
        data = raw[arm]
        blowouts = (data[..., 1] == 1) & (4 * data[..., 2] >= 3 * data[..., 3])
        one_sided.append(blowouts.mean(axis=(1, 4)))
    pair_count, arena_count, repeats = scores[0].shape
    equal_slots = np.flatnonzero(np.arange(repeats) % 4 != 3)
    mixed_slots = np.flatnonzero(np.arange(repeats) % 4 == 3)
    rng = np.random.default_rng(BOOTSTRAP_SEED)
    gaps = np.empty((samples, 3))
    blowout_rates = np.empty((samples, 3))
    for sample in range(samples):
        indices = np.empty((pair_count, arena_count, repeats), dtype=np.int32)
        for slots in (equal_slots, mixed_slots):
            indices[..., slots] = rng.choice(slots, size=(pair_count, arena_count, len(slots)))
        legacy, canonical, roster = [
            np.abs(np.take_along_axis(outcomes, indices, axis=2).mean(axis=(1, 2))).mean()
            for outcomes in scores
        ]
        gaps[sample] = canonical - legacy, roster - canonical, roster - legacy
        legacy, canonical, roster = [
            np.take_along_axis(blowouts, indices, axis=2).mean() for blowouts in one_sided
        ]
        blowout_rates[sample] = canonical - legacy, roster - canonical, roster - legacy
    return {
        "class_gap": np.quantile(gaps, [.025, .975], axis=0).T.tolist(),
        "one_sided_bouts": np.quantile(blowout_rates, [.025, .975], axis=0).T.tolist(),
    }


def render(report):
    stats = report["statistics"]

    def percent(value):
        return f"{100 * value:.1f}%"

    lines = [
        "# Level-five roster matchup parity", "",
        f"Both seats used **{report['controller']}**. Each fixture was played twice, swapping seats.",
        f"The schedule covers 66 cross-class pairings across four shared arenas, with "
        f"{report['repeats_per_arena']} seeds per pair and arena.",
        "The legacy arm assigns different levels in a quarter of fixtures and cycles levels 1–5 "
        "in the remaining fixtures. Each unequal-level arrangement also occurs with the level "
        "assignments reversed. The level-5 arms use one canonical build or all nine "
        "build combinations for each class pair.",
        "Each seed and arena assignment is reused across arms and all nine build combinations.", "",
    ]
    if report["neutralized_roster_inputs"]:
        lines += ["This checkpoint predates the roster. The benchmark zeroed its weights on the "
                  "new roster input features, matching the PPO warm start.", ""]
    lines += [
        "A win counts as one point and a draw as half a point. Class-pair scores give equal weight "
        "to each build combination. Gaps are absolute deviations from a 50% score.", "",
        "| Arm | Bouts | Mean class gap | 90th-percentile class gap | ≥70–30 | ≥80–20 | Mean build gap | Draws |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for arm, label in zip(ARMS, LABELS):
        row = stats[arm]
        lines.append(
            f"| {label} | {row['bouts']:,} | {percent(row['mean_class_gap'])} "
            f"| {percent(row['p90_class_gap'])} | {row['lopsided_70_30']}/66 "
            f"| {row['lopsided_80_20']}/66 | {percent(row['mean_build_gap'])} "
            f"| {percent(row['draw_rate'])} |"
        )
    lines += [
        "",
        "### One-sided finishes",
        "",
        "The winner must have at least 75% of their maximum hit points at knockout. "
        "Temporary hit points do not count. The all-bout denominator includes draws.", "",
        "| Arm | KOs at ≥75% HP | Share of KOs | Share of all bouts |",
        "| --- | ---: | ---: | ---: |",
    ]
    for arm, label in zip(ARMS, LABELS):
        row = stats[arm]
        ko_share = row["share_of_knockouts_at_75_percent_hp"]
        lines.append(
            f"| {label} | {row['knockouts_at_75_percent_hp']:,}/{row['knockouts']:,} "
            f"| {percent(ko_share) if ko_share is not None else '—'} "
            f"| {percent(row['share_of_bouts_at_75_percent_hp'])} |"
        )
    comparisons = ((1, 0), (2, 1), (2, 0))
    for title, metric, statistic in (
        ("Change in one-sided finishes among all bouts", "one_sided_bouts",
         "share_of_bouts_at_75_percent_hp"),
        ("Change in mean class gap", "class_gap", "mean_class_gap"),
    ):
        lines += ["", f"| {title} | Percentage points | Paired 95% interval |",
                  "| --- | ---: | ---: |"]
        for (new, old), (lower, upper) in zip(comparisons, report["paired_intervals"][metric]):
            delta = stats[ARMS[new]][statistic] - stats[ARMS[old]][statistic]
            lines.append(f"| {LABELS[new]} vs. {LABELS[old]} | {100 * delta:+.2f} "
                         f"| [{100 * lower:+.2f}, {100 * upper:+.2f}] |")
    lines += [
        "", "Intervals resample matched seed blocks within each arena, class pair, and "
        "level-mixture stratum. The lopsided-pair counts use observed scores and are "
        "sensitive to sampling noise near the thresholds. The average absolute gaps can "
        "also increase from finite-sample noise, especially in arms with fewer bouts per class pair.", "",
    ]
    if report["repeats_per_arena"] == 4:
        lines += ["The four-seed run has one mixed-level repeat per class pair and arena. "
                  "Its bootstrap intervals hold those repeats fixed.", ""]
    scores = stats["roster"]["class_scores"]
    largest = sorted(range(len(scores)), key=lambda i: -abs(scores[i] - .5))[:3]
    lines += ["### Largest observed class-pair gaps in the full roster", ""]
    for pair in largest:
        left, right = report["class_pairs"][pair]
        left, right = report["classes"][left], report["classes"][right]
        score = scores[pair]
        if score < .5:
            left, right = right, left
            score = 1 - score
        lines.append(f"- {left} scored {percent(score)} against {right}.")
    command = ["PYTHONPATH=python", ".venv/bin/python", "evaluation/roster_parity.py"]
    if report["checkpoint"]:
        command += ["--checkpoint", report["checkpoint"]]
    if report["neutralized_roster_inputs"]:
        command.append("--neutralize-roster-inputs")
    if report["repeats_per_arena"] != 8:
        command += ["--repeats", str(report["repeats_per_arena"])]
    if report["batch_size"] != 512:
        command += ["--batch", str(report["batch_size"])]
    output = "evaluation/roster-wide-parity-results" if report["checkpoint"] else "evaluation/roster-parity-results"
    command += ["--output", output]
    lines += [
        "",
        f"The roster's canonical combination differed from the legacy level-5 preset in "
        f"{report['canonical_fixture_differences']:,} matched bouts across outcomes, "
        "terminal reasons, and winner hit points.",
        "",
        "These results describe the fixed controller under these arena rules. "
        "The experiment did not train a new policy.", "",
        "[Fixture statistics and provenance](summary.json) · [Per-bout outcomes and seeds](bouts.npz)", "",
        "Rebuild with `make release` and run:", "",
        "    " + shlex.join(command), "",
    ]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeats", type=int, default=8, help="seeds per class pair per arena")
    parser.add_argument("--batch", type=int, default=512)
    parser.add_argument("--checkpoint", type=Path, help="use the same greedy policy in both seats")
    parser.add_argument("--neutralize-roster-inputs", action="store_true",
                        help="zero weights on new roster features in a pre-roster checkpoint")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.repeats < 4 or args.repeats % 4 or not 1 <= args.batch <= 65536:
        parser.error("--repeats must be a positive multiple of four and --batch must be in 1..65536")
    if args.neutralize_roster_inputs and args.checkpoint is None:
        parser.error("--neutralize-roster-inputs requires --checkpoint")

    native_path = Path(_lib()._name)
    native_digest = hashlib.sha256(native_path.read_bytes()).hexdigest()
    policy = None
    digest = None
    if args.checkpoint:
        digest = hashlib.sha256(args.checkpoint.read_bytes()).hexdigest()
        policy = Policy.load(args.checkpoint).require_arena(ARENA_ABI, FEATURES, ACTIONS)
        if args.neutralize_roster_inputs:
            if "build_ids" in policy.metadata:
                parser.error("--neutralize-roster-inputs requires a pre-roster checkpoint")
            policy.weights["w1"][list(ROSTER_NEW_INPUTS)] = 0
    plan = fixtures(args.repeats)
    raw = {}
    for arm in ARMS:
        raw[arm] = run_arm(plan, arm, args.batch, policy)
        print(f"{arm}: {raw[arm][..., 0].size:,} bouts", flush=True)
    if hashlib.sha256(native_path.read_bytes()).hexdigest() != native_digest:
        raise RuntimeError("native library changed during the benchmark")
    if args.checkpoint and hashlib.sha256(args.checkpoint.read_bytes()).hexdigest() != digest:
        raise RuntimeError("checkpoint changed during the benchmark")
    report = {
        "controller": ("Expert bot" if policy is None else f"Greedy policy {args.checkpoint.name}"),
        "checkpoint": str(args.checkpoint) if args.checkpoint else None,
        "checkpoint_sha256": digest,
        "policy_inference": "arena.policy.Policy.choose" if policy is not None else None,
        "neutralized_roster_inputs": list(ROSTER_NEW_INPUTS) if args.neutralize_roster_inputs else [],
        "native_sha256": native_digest,
        "arena_abi": ARENA_ABI,
        "python_version": sys.version.split()[0],
        "numpy_version": np.__version__,
        "seed_domain": DOMAIN,
        "bootstrap_seed": BOOTSTRAP_SEED,
        "bootstrap_samples": BOOTSTRAP_SAMPLES,
        "repeats_per_arena": args.repeats,
        "batch_size": args.batch,
        "classes": CLASS_NAMES,
        "class_pairs": CLASSES,
        "build_variants": VARIANTS,
        "arena_ids": ARENAS,
        "raw_axes": ("class_pair", "build_variant_pair", "arena", "repeat", "focal_seat",
                     "field"),
        "raw_fields": ("focal outcome", "terminal reason ID", "KO winner HP", "KO winner maximum HP"),
        "canonical_fixture_differences": int(
            (raw["level5"][:, 0] != raw["roster"][:, 0]).any(axis=-1).sum(),
        ),
        "statistics": {arm: statistics(result) for arm, result in raw.items()},
        "paired_intervals": paired_intervals(raw),
    }
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    np.savez_compressed(args.output / "bouts.npz", seeds=plan.seeds, legacy_levels=plan.legacy_levels, **raw)
    (args.output / "README.md").write_text(render(report))


if __name__ == "__main__":
    main()
