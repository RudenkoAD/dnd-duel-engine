"""Summarize strength arms with paired, stratified fixture-level inference."""

import json
from math import sqrt
from pathlib import Path
from statistics import NormalDist

import numpy as np

from arena.native import ARENA_NAMES, CLASS_NAMES, VARIANTS_PER_CLASS
from fixtures import DOMAIN, MANIFEST


HERE = Path(__file__).resolve().parent
ARMS = {
    "initial-expert": "Neutralized wide 2901 vs expert",
    "trained-expert": "Fine-tuned vs expert",
    "trained-initial": "Fine-tuned vs neutralized wide 2901",
    "trained-original": "Fine-tuned vs literal wide 2901",
    "original-expert": "Literal wide 2901 vs expert",
}
REASONS = ("ongoing", "defeat", "double knockout", "turn limit", "decision limit")


def interval(blocks, arena=None):
    """Estimate combat-seed uncertainty across fixed build pairs and arenas."""
    if arena is not None:
        blocks = blocks[:, arena:arena + 1, :]
    point = blocks.mean()
    variance = (blocks.var(axis=2, ddof=1) / blocks.shape[2]).sum()
    standard_error = sqrt(variance) / (blocks.shape[0] * blocks.shape[1])
    radius = NormalDist().inv_cdf(.975) * standard_error
    return {
        "estimate": float(point),
        "standard_error": standard_error,
        "95_percent_ci": [float(point - radius), float(point + radius)],
    }


def summarize(arm, outcomes, reasons, valid):
    games = outcomes[valid]
    if not np.isin(games, [-1, 0, 1]).all() or not (outcomes[~valid] == 9).all():
        raise ValueError(f"Invalid outcome array for {arm}")
    if not np.isin(reasons[valid], [1, 2, 3, 4]).all() or reasons[~valid].any():
        raise ValueError(f"Invalid termination array for {arm}")
    return {
        "wins": int((games == 1).sum()),
        "draws": int((games == 0).sum()),
        "losses": int((games == -1).sum()),
        "score": float((1 + games.mean()) / 2),
        "termination": dict(zip(REASONS, np.bincount(reasons[valid], minlength=len(REASONS)).tolist())),
    }


def weighted_blocks(outcomes, distinct):
    # Within each seed block aggregate all assignments and seats. Rescale
    # both mirror and distinct strata to equal weight per ordered build pair.
    first = (1 + outcomes[..., :2].mean(axis=-1)) / 2
    second = (1 + outcomes[distinct, ..., 2:].mean(axis=-1)) / 2
    first[distinct] += second
    return first * (len(distinct) / (len(distinct) + int(distinct.sum())))


def class_blocks(pairs, outcomes, class_id):
    """Weight the focal class's ordered builds equally within each seed."""
    focal_in_class = pairs // VARIANTS_PER_CLASS == class_id
    focal_in_class[:, 1] &= pairs[:, 0] != pairs[:, 1]
    selected = focal_in_class.any(axis=1)
    focal_in_class = focal_in_class[selected]
    selected_outcomes = outcomes[selected]
    seat_pairs = selected_outcomes.reshape(*selected_outcomes.shape[:-1], 2, 2)
    orientation_scores = (1 + seat_pairs.mean(axis=-1)) / 2
    numerator = np.where(focal_in_class[:, None, None, :], orientation_scores, 0).sum(axis=-1)
    return numerator * (selected.sum() / focal_in_class.sum())


def render(report):
    def pct(value):
        return f"{100 * value:.2f}%"

    def bounds(row, suffix="%"):
        lo, hi = row["95_percent_ci"]
        return f"[{100 * lo:.2f}, {100 * hi:.2f}]{suffix}"

    lines = [
        "# Final strength on the 36-build level-five roster",
        "",
        "The selected endpoint is update 1000 after 384,000 training bouts. "
        "Every arm uses the same 26,640 seed blocks from R36-L2901-FINAL-v1. "
        "The schedule enumerates 666 unordered build pairs, five explicit arenas and eight "
        "seeds per pair and arena. Each non-mirror block rotates both builds through the "
        "focal controller and plays both seats for each assignment. Mirror blocks play both "
        "seats. This gives each of the 1,296 ordered build assignments equal weight.",
        "",
        "Score gives one point for a win, half a point for a draw and zero for a loss. "
        "The 95% intervals keep shared seed blocks intact and estimate seed variation "
        "within each fixed build-pair and arena stratum. The expert contrast subtracts "
        "the two arms within the same seed blocks. Intervals are pointwise.",
        "",
        "| Focal vs opponent | Wins | Draws | Losses | Score | 95% CI |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for arm, name in ARMS.items():
        row = report["arms"][arm]
        lines.append(f"| {name} | {row['wins']:,} | {row['draws']:,} | {row['losses']:,} "
                     f"| {pct(row['score'])} | {bounds(row['score_inference'])} |")
    delta = report["trained_minus_initial_vs_expert"]
    literal_delta = report["trained_minus_original_vs_expert"]
    neutralization = report["initial_minus_original_vs_expert"]
    lines += [
        "",
        f"Against expert, fine-tuning changed score by **{100 * delta['estimate']:+.2f} percentage points** "
        f"(paired 95% CI {bounds(delta, ' pp')}).",
        f"Relative to literal wide 2901, fine-tuning changed expert score by "
        f"**{100 * literal_delta['estimate']:+.2f} percentage points** "
        f"(paired 95% CI {bounds(literal_delta, ' pp')}).",
        "",
        "| Arena | Start vs expert | Fine-tuned vs expert | Paired change, pp (95% CI) | Fine-tuned vs start | Fine-tuned vs literal 2901 |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for arena in ARENA_NAMES:
        estimates = [pct(report["arms"][arm]["arena_score"][arena]["estimate"]) for arm in ARMS]
        change = report["arena_trained_minus_initial_vs_expert"][arena]
        lines.append(f"| {arena} | {estimates[0]} | {estimates[1]} "
                     f"| {100 * change['estimate']:+.2f} {bounds(change, '')} "
                     f"| {estimates[2]} | {estimates[3]} |")
    lines += [
        "",
        "| Focal class | Start vs expert | Fine-tuned vs expert | Paired change, pp (95% CI) |",
        "| --- | ---: | ---: | ---: |",
    ]
    for label, row in report["expert_by_focal_class"].items():
        change = row["difference"]
        lines.append(f"| {label} | {pct(row['initial']['estimate'])} | {pct(row['trained']['estimate'])} "
                     f"| {100 * change['estimate']:+.2f} {bounds(change, '')} |")
    lines += [
        "",
        "| Focal vs opponent | Double knockouts | Turn-cap draws | Decision-cap draws |",
        "| --- | ---: | ---: | ---: |",
    ]
    for arm, name in ARMS.items():
        terminal = report["arms"][arm]["termination"]
        lines.append(f"| {name} | {terminal['double knockout']:,} "
                     f"| {terminal['turn limit']:,} | {terminal['decision limit']:,} |")
    lines += [
        "",
        "The primary comparator applies the trainer's roster warm-start rule to update 2901, "
        "zeroing the 29 reserved observation inputs at the first layer. The literal comparison "
        "uses its original weights, including 6,144 nonzero weights on those inputs. "
        f"Neutralization alone changed expert score by {100 * neutralization['estimate']:+.2f} pp "
        f"(paired 95% CI {bounds(neutralization, ' pp')}).",
        "",
        "The estimates apply to greedy masked policies, this native engine and expert, "
        "and uniform weighting over level-five builds and arenas. The intervals quantify "
        "combat RNG uncertainty conditional on these checkpoints. There is one training run "
        "and no estimate of variability across independent retraining runs. Each exact build "
        "pair and arena has only eight seeds.",
        "",
        "Input paths, SHA-256 digests and the full intervals are in strength.json. "
        "Each arm stores outcomes and terminal reasons as pair × arena × repeat × orientation "
        "arrays in its matching NPZ file. Orientations order the focal build first and seat second. "
        "For identical builds, only orientations 0 and 1 are played.",
        "",
    ]
    return "\n".join(lines)


def main():
    with np.load(MANIFEST, allow_pickle=False) as manifest:
        pairs, seeds = manifest["pairs"], manifest["seeds"]
    distinct = pairs[:, 0] != pairs[:, 1]
    valid = np.ones((*seeds.shape, 4), dtype=bool)
    valid[~distinct, ..., 2:] = False
    report = {"fixture_domain": DOMAIN, "seed_blocks_per_arm": seeds.size, "arms": {}}
    scores = {}
    expert_class_scores = {}
    for arm in ARMS:
        with np.load(HERE / f"{arm}.npz", allow_pickle=False) as archive:
            outcomes, reasons = archive["outcomes"], archive["reasons"]
            metadata = json.loads(archive["metadata"].item())
        row = summarize(arm, outcomes, reasons, valid)
        blocks = weighted_blocks(outcomes, distinct)
        scores[arm] = blocks
        if arm.endswith("-expert"):
            expert_class_scores[arm] = [
                class_blocks(pairs, outcomes, class_id) for class_id in range(len(CLASS_NAMES))
            ]
        row["score_inference"] = interval(blocks)
        if not np.isclose(row["score"], row["score_inference"]["estimate"]):
            raise RuntimeError("Fixture weighting disagrees with the raw arm score")
        row["arena_score"] = {
            label: interval(blocks, arena=arena) for arena, label in enumerate(ARENA_NAMES)
        }
        row["metadata"] = metadata
        report["arms"][arm] = row
    for label in ("native", "fixtures"):
        hashes = {row["metadata"]["input_files"][label]["sha256"] for row in report["arms"].values()}
        if len(hashes) != 1:
            raise RuntimeError(f"Different {label} files used across arms")

    def checkpoint_hash(arm, label):
        return report["arms"][arm]["metadata"]["input_files"][label]["sha256"]

    if len({checkpoint_hash(arm, "focal") for arm in ARMS if arm.startswith("trained")}) != 1:
        raise RuntimeError("Different fine-tuned checkpoints used across arms")
    if len({checkpoint_hash("initial-expert", "focal"),
            checkpoint_hash("original-expert", "focal"),
            checkpoint_hash("trained-initial", "opponent"),
            checkpoint_hash("trained-original", "opponent")}) != 1:
        raise RuntimeError("Different starting checkpoints used across arms")
    delta = scores["trained-expert"] - scores["initial-expert"]
    report["trained_minus_initial_vs_expert"] = interval(delta)
    report["trained_minus_original_vs_expert"] = interval(scores["trained-expert"] - scores["original-expert"])
    report["initial_minus_original_vs_expert"] = interval(scores["initial-expert"] - scores["original-expert"])
    report["arena_trained_minus_initial_vs_expert"] = {
        label: interval(delta, arena=arena) for arena, label in enumerate(ARENA_NAMES)
    }
    report["expert_by_focal_class"] = {
        label: {"initial": interval(old), "trained": interval(new), "difference": interval(new - old)}
        for label, old, new in zip(
            CLASS_NAMES, expert_class_scores["initial-expert"], expert_class_scores["trained-expert"]
        )
    }
    for label, arm in (("initial", "initial-expert"), ("trained", "trained-expert")):
        class_scores = [row[label]["estimate"] for row in report["expert_by_focal_class"].values()]
        if not np.isclose(np.mean(class_scores), scores[arm].mean()):
            raise RuntimeError("Equal weighting by focal class disagrees with the all-class score")
    (HERE / "strength.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    (HERE / "strength.md").write_text(render(report))


if __name__ == "__main__":
    main()
