"""Sensitivity of mean matchup gaps to conditioning on a decisive outcome."""

import json
from pathlib import Path

import numpy as np


HERE = Path(__file__).resolve().parent


def load_counts():
    counts = []
    for arm in ("initial", "final"):
        with np.load(HERE / f"balance-{arm}.npz") as archive:
            if not counts:
                pairs, seeds = archive["pairs"], archive["seeds"]
            elif not (np.array_equal(pairs, archive["pairs"]) and np.array_equal(seeds, archive["seeds"])):
                raise ValueError("Both arms must use the same fixtures")
            winners = archive["raw"][..., 0]
            first_won = (winners == np.arange(2)).sum(axis=-1)
            decisive = np.isin(winners, (0, 1)).sum(axis=-1)
            counts.append(np.stack((first_won, decisive), axis=-1))
    distinct = pairs[:, 0] != pairs[:, 1]
    return pairs[distinct], np.stack(counts, axis=-1)[distinct]


def mean_gaps(totals, class_index, cross_class):
    def gap(won, decisive):
        if not (decisive > 0).all():
            raise ValueError("An evaluated matchup has no decisive games")
        return np.abs(won / decisive - 0.5).mean(axis=0)

    build_gap = gap(totals[:, 0], totals[:, 1])
    class_totals = np.zeros((66, 2, 2))
    np.add.at(class_totals, class_index, totals[cross_class])
    return np.stack((gap(class_totals[:, 0], class_totals[:, 1]), build_gap))


def main():
    pairs, counts = load_counts()
    classes = pairs // 3
    cross_class = classes[:, 0] != classes[:, 1]
    _, class_index = np.unique(classes[cross_class], axis=0, return_inverse=True)
    point = mean_gaps(counts.sum(axis=(1, 2)), class_index, cross_class)
    rng = np.random.default_rng(6122901)
    changes = []
    for _ in range(2000):
        indices = rng.integers(counts.shape[2], size=(*counts.shape[:3], 1, 1))
        draw = np.take_along_axis(counts, indices, axis=2).sum(axis=(1, 2))
        estimates = mean_gaps(draw, class_index, cross_class)
        changes.append(estimates[:, 1] - estimates[:, 0])
    intervals = np.quantile(changes, [.025, .975], axis=0).T
    result = {key: {"initial": x, "final": y, "paired_delta_95": interval.tolist()}
              for key, (x, y), interval in zip(("class", "build"), point, intervals)}
    (HERE / "decisive-balance.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
