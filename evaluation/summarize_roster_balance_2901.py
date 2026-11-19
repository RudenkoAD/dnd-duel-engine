"""Summarize paired policy mirrors by build, class, and physical bout."""

import argparse
import csv
from itertools import combinations_with_replacement
import json
from pathlib import Path

import numpy as np

from arena.native import CLASS_NAMES, VARIANTS_PER_CLASS, roster

ARMS = ("initial", "final")
REASONS = ("ongoing", "defeat", "double_ko", "turn_limit", "decision_limit")
BOOTSTRAP_SEED = 2901975


def encoded(raw):
    winner, reason, hp, max_hp = np.moveaxis(raw, -1, 0)
    seat = np.arange(2)
    used = winner >= 0
    score = np.where(winner == 2, .5, winner == seat)
    knockout = reason == 1
    blowout = knockout & (4 * hp >= 3 * max_hp)
    return used, score, knockout, blowout


def physical(raw):
    used, _, ko, blowout = encoded(raw)
    winner = raw[..., 0][used]
    return {
        "bouts": int(used.sum()),
        "seat0_wins": int((winner == 0).sum()),
        "seat1_wins": int((winner == 1).sum()),
        "draws": int((winner == 2).sum()),
        "knockouts": int(ko.sum()),
        "ko_at_75_percent_hp": int(blowout.sum()),
        "one_sided_per_bout": float(blowout.sum() / used.sum()),
        "one_sided_per_ko": float(blowout.sum() / ko.sum()),
        "termination": dict(zip(REASONS, np.bincount(raw[..., 1][used], minlength=len(REASONS)).tolist())),
    }


def interval(samples):
    return np.quantile(samples, [.025, .975], axis=0).T.tolist()


def bootstrap(data, class_map, class_weights, build_primary, class_primary, samples):
    # Columns are scores from the lower build's perspective, plus physical
    # terminal-event counts. Seat assignments share a common seed block.
    common = []
    for raw in data:
        used, score, ko, blowout = encoded(raw)
        block_score = np.sum(score * used, axis=-1) / used.sum(axis=-1)
        block_score[~build_primary] = .5
        common.append(np.stack((block_score, ko.sum(axis=-1), blowout.sum(axis=-1)), axis=-1))
    arms = np.stack(common, axis=-1)
    rows, arenas, repeats = arms.shape[:3]
    rng = np.random.default_rng(BOOTSTRAP_SEED)
    scores = np.empty((samples, rows, 2))
    classes = np.empty((samples, len(class_weights), 2))
    rates = np.empty((samples, 2, 2))
    bouts = sum((2 if distinct else 1) * arenas * repeats for distinct in build_primary)
    for sample in range(samples):
        indices = rng.integers(repeats, size=(rows, arenas, repeats, 1, 1))
        boot = np.take_along_axis(arms, indices, axis=2)
        scores[sample] = boot[..., 0, :].mean(axis=(1, 2))
        for arm in range(2):
            classes[sample, :, arm] = np.bincount(
                class_map, weights=scores[sample, :, arm], minlength=len(class_weights)
            ) / class_weights
        classes[sample, ~class_primary] = .5
        totals = boot[..., 1:, :].sum(axis=(0, 1, 2))
        rates[sample] = totals[1] / bouts, totals[1] / totals[0]
    return scores, classes, rates


def effects(points, boot, primary, blocks, arena_repeats):
    scores = points[primary]
    gap = np.abs(scores - .5)
    bootstrap_gap = np.abs(boot[:, primary] - .5).mean(axis=1)
    bootstrap_p90 = np.quantile(np.abs(boot[:, primary] - .5), .90, axis=1)
    gap_mean = gap.mean(axis=0)
    gap_p90 = np.quantile(gap, .90, axis=0)
    count = (gap >= .30 - 1e-12).sum(axis=0)
    sampling_sensitive = np.abs(boot[:, primary] - .5) >= .30 - 1e-12
    bootstrap_count = sampling_sensitive.sum(axis=1)
    # Union-bound Hoeffding intervals for both arms over the fixed family of
    # distinct matchup cells. Within a seat block score is bounded by [0,1].
    threshold_radius = np.sqrt(np.log(4 * len(scores) / .05) / (2 * blocks[primary]))
    # A difference of two matched scores lives in [-1, 1].
    shift_radius = np.sqrt(2 * np.log(2 * len(scores) / .05) / blocks[primary])
    delta = scores[:, 1] - scores[:, 0]
    paired_boot = boot[:, primary, 1] - boot[:, primary, 0]
    paired_max_radius = float(np.quantile(np.max(np.abs(paired_boot - delta), axis=1), .95))
    stats = {
        "denominator": int(primary.sum()),
        "all_cells_including_structural_mirrors": int(primary.size),
        "physical_bouts_per_cell": [int(2 * blocks[primary].min()), int(2 * blocks[primary].max())],
        "independent_seed_blocks_per_cell": [int(blocks[primary].min()), int(blocks[primary].max())],
        "seeds_per_arena_per_build_pair": arena_repeats,
        "arms": {
            arm: {
                "mean_gap": float(gap_mean[i]),
                "mean_gap_including_structural_mirrors": float(gap[:, i].sum() / primary.size),
                "bias_corrected_mean_gap_sensitivity": float(2 * gap_mean[i] - bootstrap_gap[:, i].mean()),
                "p90_gap": float(gap_p90[i]),
                "observed_80_20_count": int(count[i]),
                "conservative_95_familywise_80_20_count_bounds": [
                    int((gap[:, i] - threshold_radius >= .30 - 1e-12).sum()),
                    int((gap[:, i] + threshold_radius >= .30 - 1e-12).sum()),
                ],
            }
            for i, arm in enumerate(ARMS)
        },
        "paired_delta_mean_gap": float(gap_mean[1] - gap_mean[0]),
        "paired_delta_mean_gap_95": interval(bootstrap_gap[:, 1] - bootstrap_gap[:, 0]),
        "paired_delta_p90_gap": float(gap_p90[1] - gap_p90[0]),
        "paired_delta_p90_gap_95": interval(bootstrap_p90[:, 1] - bootstrap_p90[:, 0]),
        "paired_delta_observed_80_20_count": int(count[1] - count[0]),
        "paired_delta_80_20_count_bootstrap_sensitivity": interval(bootstrap_count[:, 1] - bootstrap_count[:, 0]),
        "simultaneously_resolved_shift_count": int((np.abs(delta) > shift_radius).sum()),
        "bootstrap_simultaneously_resolved_shift_count": int((np.abs(delta) > paired_max_radius).sum()),
    }
    full_radius = np.zeros(len(primary))
    full_radius[primary] = shift_radius
    return stats, full_radius, np.where(primary, paired_max_radius, 0)


def scores_counts(raw, pairs):
    used, score, _, _ = encoded(raw)
    axes = (1, 2, 3)
    wins = ((score == 1) & used).sum(axis=axes)
    draws = ((score == .5) & used).sum(axis=axes)
    losses = ((score == 0) & used).sum(axis=axes)
    mirrors = pairs[:, 0] == pairs[:, 1]
    points = (wins + .5 * draws) / (wins + draws + losses)
    points[mirrors] = .5
    return points, np.column_stack((wins, draws, losses))


def write_csv(path, pairs, names, points, boot, physical_bouts, shifts, bootstrap_shifts, structural, stats, controls):
    marginal_delta = interval(boot[..., 1] - boot[..., 0])
    initial_interval = interval(boot[..., 0])
    final_interval = interval(boot[..., 1])
    wdl = ("focal_wins", "draws", "focal_losses")
    fields = [
        "left", "right", "physical_bouts_per_arm", "structural_50_50",
        "initial_score", "initial_95_low", "initial_95_high",
        "final_score", "final_95_low", "final_95_high",
        "score_delta", "paired_95_low", "paired_95_high",
        "simultaneous_95_low", "simultaneous_95_high",
        "bootstrap_simultaneous_95_low", "bootstrap_simultaneous_95_high",
        "initial_gap", "final_gap", "initial_80_20", "final_80_20",
    ]
    fields += [f"{arm}_{metric}" for arm in ARMS for metric in wdl]
    for control in controls:
        fields += [f"{control}_score", f"{control}_gap", f"{control}_80_20"]
    rows = []
    for i, (left, right) in enumerate(pairs):
        p, q = points[i]
        lo, hi = marginal_delta[i]
        a, b = initial_interval[i]
        c, d = final_interval[i]
        values = [
            names[left], names[right], int(physical_bouts[i]), bool(structural[i]),
            p, a, b, q, c, d, q-p, lo, hi, max(-1, q-p-shifts[i]), min(1, q-p+shifts[i]),
            max(-1, q-p-bootstrap_shifts[i]), min(1, q-p+bootstrap_shifts[i]),
            abs(p-.5), abs(q-.5), bool(abs(p-.5) >= .30-1e-12), bool(abs(q-.5) >= .30-1e-12),
        ]
        values.extend([""] * 6 if structural[i] else [int(v) for arm in stats for v in arm[i]])
        for control in controls.values():
            gap = abs(control[i] - .5)
            values.extend((control[i], gap, bool(gap >= .30 - 1e-12)))
        rows.append(dict(zip(fields, values)))
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--bootstrap-samples", type=int, default=2000)
    parser.add_argument("--expert", action="store_true", help="also summarize balance-expert.npz")
    parser.add_argument("--literal", action="store_true", help="also summarize balance-literal.npz")
    args = parser.parse_args()
    directory = args.directory
    with np.load(directory / "balance-initial.npz", allow_pickle=False) as initial, np.load(
        directory / "balance-final.npz", allow_pickle=False
    ) as final:
        pairs, seeds = initial["pairs"], initial["seeds"]
        if not np.array_equal(pairs, final["pairs"]) or not np.array_equal(seeds, final["seeds"]):
            raise ValueError("results used different fixtures")
        data = [initial["raw"], final["raw"]]
        provenance = {arm: json.loads(archive["metadata"].item()) for arm, archive in zip(ARMS, (initial, final))}
    for source in ("fixtures", "native"):
        if provenance["initial"]["sha256"][source] != provenance["final"]["sha256"][source]:
            raise ValueError(f"results used different {source}")
    if data[0].shape != data[1].shape or not np.array_equal(data[0][..., 0] >= 0, data[1][..., 0] >= 0):
        raise ValueError("results used different physical bouts")
    class_pairs = np.asarray(list(combinations_with_replacement(range(len(CLASS_NAMES)), 2)))
    class_index = {tuple(pair): n for n, pair in enumerate(class_pairs)}
    class_map = np.array([class_index[tuple(pair)] for pair in pairs // VARIANTS_PER_CLASS])
    build_primary = pairs[:, 0] != pairs[:, 1]
    class_primary = class_pairs[:, 0] != class_pairs[:, 1]
    weights = np.bincount(class_map, minlength=len(class_pairs))
    per_arm = [scores_counts(raw, pairs) for raw in data]
    scores = np.column_stack([row[0] for row in per_arm])
    results = [row[1] for row in per_arm]
    class_scores = np.column_stack([np.bincount(class_map, weights=col) / weights for col in scores.T])
    class_scores[~class_primary] = .5
    class_results = [
        np.column_stack([np.bincount(class_map, weights=col) for col in counts.T]) for counts in results
    ]
    boot, class_boot, rate_boot = bootstrap(data, class_map, weights, build_primary, class_primary, args.bootstrap_samples)
    blocks = np.full(len(pairs), seeds.shape[1] * seeds.shape[2])
    class_blocks = blocks[0] * weights
    build_stats, build_radius, build_boot_radius = effects(scores, boot, build_primary, blocks, seeds.shape[2])
    class_stats, class_radius, class_boot_radius = effects(class_scores, class_boot, class_primary, class_blocks, seeds.shape[2])
    bouts = blocks * (1 + build_primary)
    class_bouts = np.bincount(class_map, weights=bouts)
    build_names = [build["name"] for build in roster()]
    control_summaries, control_builds, control_classes = {}, {}, {}
    for control in ("expert", "literal"):
        if not getattr(args, control):
            continue
        with np.load(directory / f"balance-{control}.npz", allow_pickle=False) as archive:
            control_info = json.loads(archive["metadata"].item())
            control_data = archive["raw"]
            if not np.array_equal(pairs, archive["pairs"]) or not np.array_equal(seeds, archive["seeds"]):
                raise ValueError(f"{control} used different fixtures")
            if any(control_info["sha256"][key] != provenance["initial"]["sha256"][key]
                   for key in ("fixtures", "native")):
                raise ValueError(f"{control} used a different manifest or native library")
        if control_data.shape != data[0].shape or not np.array_equal(
            control_data[..., 0] >= 0, data[0][..., 0] >= 0
        ):
            raise ValueError(f"{control} used different physical bouts")
        build_points, _ = scores_counts(control_data, pairs)
        class_points = np.bincount(class_map, weights=build_points) / weights
        class_points[~class_primary] = .5
        control_builds[control] = build_points
        control_classes[control] = class_points
        stats = {"provenance": control_info, "physical_bouts": physical(control_data)}
        for level, points, primary in (
            ("build_pairs", build_points, build_primary), ("class_pairs", class_points, class_primary),
        ):
            gaps = np.abs(points[primary] - .5)
            stats[level] = {
                "mean_gap": float(gaps.mean()),
                "p90_gap": float(np.quantile(gaps, .90)),
                "observed_80_20_count": int((gaps >= .30 - 1e-12).sum()),
            }
        control_summaries[f"{control}_vs_{control}"] = stats
    build_rows = write_csv(directory / "balance-build-pairs.csv", pairs, build_names, scores, boot,
                           bouts, build_radius, build_boot_radius, ~build_primary, results, control_builds)
    class_rows = write_csv(directory / "balance-class-pairs.csv", class_pairs, CLASS_NAMES, class_scores,
                           class_boot, class_bouts, class_radius, class_boot_radius, ~class_primary,
                           class_results, control_classes)
    summary = {
        "provenance": provenance,
        "bootstrap": {"seed": BOOTSTRAP_SEED, "samples": args.bootstrap_samples},
        "physical_bouts": {arm: physical(raw) for arm, raw in zip(ARMS, data)},
        "exact_mirror_bouts": {arm: physical(raw[~build_primary]) for arm, raw in zip(ARMS, data)},
        "class_pairs": class_stats,
        "build_pairs": build_stats,
        "largest_class_shifts": largest_shifts(class_rows),
        "largest_build_shifts": largest_shifts(build_rows),
        **control_summaries,
    }
    for i, label in enumerate(("one_sided_per_bout", "one_sided_per_ko")):
        stats = summary["physical_bouts"]
        summary[f"paired_delta_{label}"] = stats["final"][label] - stats["initial"][label]
        summary[f"paired_delta_{label}_95"] = interval(rate_boot[:, i, 1] - rate_boot[:, i, 0])
    (directory / "balance-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (directory / "balance.md").write_text(render(summary))


def largest_shifts(rows):
    return sorted((row for row in rows if not row["structural_50_50"]),
                  key=lambda row: -abs(row["score_delta"]))[:10]


def render(summary):
    def pp(value):
        return f"{100 * value:.2f}"
    def ci(values):
        return f"[{pp(values[0])}, {pp(values[1])}]"
    rows = [
        "# Level-5 roster balance under greedy play", "",
        "The frozen warm start and final checkpoint each played itself on the same 36-build, five-arena fixture manifest.",
        "There were eight seeds per build pair and arena, with two seat assignments for distinct builds.",
        "Exact mirrors have one physical bout per seed. Including both focal labels would count the same fight twice.",
        "A win scores one, a draw scores one half, and a loss scores zero.",
        "An off-diagonal class score weights its nine build combinations equally.",
        "Same-class aggregate and exact-mirror build scores are 50% by symmetry.",
        "They remain in the CSVs and physical-bout totals. The gap and 80–20 summaries use 66 cross-class and 630 distinct-build cells.",
        "", "| Metric | Initial | Final | Final − initial | Paired 95% interval |",
        "| --- | ---: | ---: | ---: | ---: |",
    ]
    for level in ("class_pairs", "build_pairs"):
        stats = summary[level]
        arms = stats["arms"]
        label = level.split("_")[0].capitalize()
        rows.append(f"| {label}-pair mean gap (pp) | {pp(arms['initial']['mean_gap'])} | "
                    f"{pp(arms['final']['mean_gap'])} | {pp(stats['paired_delta_mean_gap'])} | "
                    f"{ci(stats['paired_delta_mean_gap_95'])} |")
        rows.append(f"| {label}-pair 90th-percentile gap (pp) | {pp(arms['initial']['p90_gap'])} | "
                    f"{pp(arms['final']['p90_gap'])} | {pp(stats['paired_delta_p90_gap'])} | "
                    f"{ci(stats['paired_delta_p90_gap_95'])} |")
    for label, metric in (
        ("KO with ≥75% HP, all bouts (%)", "one_sided_per_bout"),
        ("KO with ≥75% HP, among KOs (%)", "one_sided_per_ko"),
    ):
        stats = summary["physical_bouts"]
        rows.append(f"| {label} | {pp(stats['initial'][metric])} | {pp(stats['final'][metric])} | "
                    f"{pp(summary[f'paired_delta_{metric}'])} | {ci(summary[f'paired_delta_{metric}_95'])} |")
    rows += [
        "", "Winner HP uses actual hit points and maximum hit points at termination.",
        "", "| Checkpoint | Physical bouts | Seat 0 wins | Seat 1 wins | Draws | KO | Double KO | Turn cap | Decision cap | KO with ≥75% HP |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for arm in ARMS:
        stats = summary["physical_bouts"][arm]
        values = [stats[k] for k in ("bouts", "seat0_wins", "seat1_wins", "draws", "knockouts")]
        values.extend(stats["termination"][reason] for reason in REASONS[2:])
        values.append(stats["ko_at_75_percent_hp"])
        rows.append(f"| {arm.capitalize()} | " + " | ".join(f"{v:,}" for v in values) + " |")
    rows += [
        "", "| Cells | Bouts per cell | Initial observed ≥80–20 | Final observed ≥80–20 | Initial simultaneous 95% count bounds | Final simultaneous 95% count bounds |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for level in ("class_pairs", "build_pairs"):
        stats = summary[level]
        values = [stats["arms"][arm]["observed_80_20_count"] for arm in ARMS]
        bounds = [stats["arms"][arm]["conservative_95_familywise_80_20_count_bounds"] for arm in ARMS]
        rows.append(f"| {level.replace('_', ' ').capitalize()} | {stats['physical_bouts_per_cell'][0]} | "
                    f"{values[0]}/{stats['denominator']} | {values[1]}/{stats['denominator']} | "
                    f"{bounds[0][0]}–{bounds[0][1]} | {bounds[1][0]}–{bounds[1][1]} |")
    rows += [
        "",
        "The paired bootstrap resamples seeds separately within each build pair and arena while keeping both seat outcomes and both checkpoints together.",
        "Cells close to 80–20 can cross the threshold from sampling noise. Absolute gaps have finite-sample upward bias, especially for build pairs with 40 seed blocks.",
        "The summary includes a bootstrap bias-correction sensitivity estimate for mean gaps and conservative simultaneous Hoeffding bounds for the threshold counts.",
        "Threshold-count bootstrap ranges in the summary are descriptive sensitivity ranges for a discontinuous statistic.",
        "These intervals quantify combat-seed sampling under the fixed controller, roster, and arena set.",
    ]
    for key, label in (("expert_vs_expert", "Expert bot"), ("literal_vs_literal", "Original unmodified checkpoint")):
        if key not in summary:
            continue
        control = summary[key]
        classes, builds, physical = (control[key] for key in ("class_pairs", "build_pairs", "physical_bouts"))
        rows += [
            "", f"## {label} mirror", "",
            f"On the same {physical['bouts']:,} bouts, this controller scored mean gaps of "
            f"{pp(classes['mean_gap'])} pp by class and {pp(builds['mean_gap'])} pp by build.",
            f"The observed 80–20 counts were {classes['observed_80_20_count']}/66 class cells and "
            f"{builds['observed_80_20_count']}/630 build cells.",
            f"Winners retained at least 75% HP in {physical['ko_at_75_percent_hp']:,} KOs, "
            f"or {pp(physical['one_sided_per_bout'])}% of all bouts and "
            f"{pp(physical['one_sided_per_ko'])}% of KOs.",
        ]
    for level in ("class", "build"):
        rows += [
            "", f"## Largest {level}-pair score shifts", "",
            "Positive changes favor the first named side.",
            "The approximate simultaneous bootstrap bands use the largest centered error across all distinct cells of this type.",
            "The CSVs also give conservative simultaneous Hoeffding bounds.",
            "", "| First side | Second side | Initial score (%) | Final score (%) | Change (pp) | Simultaneous bootstrap 95% (pp) |",
            "| --- | --- | ---: | ---: | ---: | ---: |",
        ]
        for row in summary[f"largest_{level}_shifts"][:5]:
            rows.append(f"| {row['left']} | {row['right']} | {pp(row['initial_score'])} | {pp(row['final_score'])} | "
                        f"{pp(row['score_delta'])} | "
                        f"{ci((row['bootstrap_simultaneous_95_low'], row['bootstrap_simultaneous_95_high']))} |")
    rows += ["", "[Class-pair CSV](balance-class-pairs.csv) · [Build-pair CSV](balance-build-pairs.csv) · [Summary JSON](balance-summary.json)", ""]
    return "\n".join(rows)


if __name__ == "__main__":
    main()
