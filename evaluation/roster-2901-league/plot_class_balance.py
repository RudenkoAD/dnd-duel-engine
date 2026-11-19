"""Render class scores directly from two matched self-play outcome archives."""

import argparse
from pathlib import Path
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from plot_roster_balance import panel


CLASSES = ["Варвар", "Бард", "Жрец", "Друид", "Воин", "Монах", "Паладин",
           "Следопыт", "Плут", "Чародей", "Колдун", "Волшебник"]


def scores(archive):
    winners = archive["raw"][..., 0]
    focal_seat = np.arange(winners.shape[-1])
    points = np.where(winners == 2, 0.5, winners == focal_seat)
    classes = archive["pairs"] // 3
    class_pairs = []
    results = []
    for first in range(len(CLASSES)):
        for second in range(first + 1, len(CLASSES)):
            cells = (classes == (first, second)).all(axis=1)
            if cells.sum() != 9 or not np.isin(winners[cells], (0, 1, 2)).all():
                raise ValueError("Expected complete results for all nine subclass pairings")
            class_pairs.append((first, second))
            results.append(points[cells].mean())
    return {"class_pairs": class_pairs, "statistics": {"roster": {"class_scores": results}}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("initial", type=Path)
    parser.add_argument("trained", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    with np.load(args.initial) as initial, np.load(args.trained) as trained:
        if any(not np.array_equal(initial[key], trained[key]) for key in ("pairs", "seeds")):
            raise ValueError("Initial and trained policies must use identical fixtures")
        initial_scores, trained_scores = scores(initial), scores(trained)

    fig, axes = plt.subplots(1, 2, figsize=(14.0, 7.0), constrained_layout=True)
    fig.suptitle("Баланс классов в дуэлях пятого уровня", fontsize=17)
    image = panel(axes[0], initial_scores, "До обучения · версия 2901", CLASSES)
    panel(axes[1], trained_scores, "После 384 000 боёв обычной лиги", CLASSES)
    colorbar = fig.colorbar(image, ax=axes, shrink=0.72, pad=0.02, ticks=np.linspace(0, 1, 5))
    colorbar.set_ticklabels(["0", "25", "50", "75", "100"])
    colorbar.set_label("Очки класса из строки, %")
    for ax in axes:
        ax.set_xlabel("Класс соперника")
    fig.savefig(args.output, dpi=155, facecolor="white")
    plt.close(fig)


if __name__ == "__main__":
    main()
