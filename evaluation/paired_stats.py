"""Summaries whose sampling unit is the complete two-seat bout pair."""

import numpy as np


def record(outcomes):
    outcomes = np.asarray(outcomes)
    wins, draws, losses = (int((outcomes == score).sum()) for score in (1, 0, -1))
    return {
        "wins": wins,
        "draws": draws,
        "losses": losses,
        "score": (wins + draws / 2) / outcomes.size,
    }


def cluster_interval(pair_values, seed=77192, samples=4000, *, strata=None):
    """Return a percentile interval for the mean, resampling pairs within strata."""
    pair_values = np.asarray(pair_values, dtype=np.float64)
    rng = np.random.default_rng(seed)
    if strata is None:
        strata = np.zeros(len(pair_values), dtype=np.int32)
    _, indices = np.unique(strata, axis=0, return_inverse=True)
    totals = np.zeros(samples, dtype=np.float64)
    for group in range(indices.max() + 1):
        values = pair_values[indices == group]
        for start in range(0, samples, 256):
            count = min(256, samples - start)
            totals[start:start + count] += rng.choice(values, (count, len(values))).sum(axis=1)
    return [float(value) for value in np.quantile(totals / len(pair_values), (.025, .975))]


def paired_difference(outcomes, control, strata):
    control = np.asarray(control)
    difference = (np.asarray(outcomes) - control).mean(axis=1) / 2
    return {
        "matched_control": record(control),
        "paired_score_gain": float(difference.mean()),
        "paired_score_gain_ci95": cluster_interval(difference, strata=strata),
    }


def summary(outcomes, strata, control=None):
    outcomes = np.asarray(outcomes)
    result = record(outcomes)
    result["score_ci95"] = cluster_interval((1 + outcomes.mean(axis=1)) / 2, strata=strata)
    if control is not None:
        result.update(paired_difference(outcomes, control, strata))
    return result
