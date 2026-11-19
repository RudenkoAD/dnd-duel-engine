"""Plot scored and raw win rates for the checkpoint series in league_curve.py."""

import argparse
import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("curve", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    curve = json.loads(args.curve.read_text())
    records = curve["checkpoints"]
    anchors = curve["protocol"]["opponent_sha256"]
    updates = [row["update"] for row in records]
    best = curve["selection"]["balanced"]["best_update"]
    tick_step = max(100, 100 * ((updates[-1] + 499) // 500))
    ticks = [x for x in range(0, updates[-1], tick_step) if updates[-1] - x >= tick_step / 2]
    ticks.append(updates[-1])
    fig, panels = plt.subplots(1, 2, figsize=(10, 3.8), sharey=True, layout="constrained")
    for ax, metric, title, color in zip(
        panels, ("league", "expert"), ("Три прошлые сети", "Усиленный бот"),
        ("#12605a", "#9a4313"),
    ):
        def wins(row):
            if metric == "expert":
                return row["opponents"]["expert"]["raw_win"]
            return sum(row["opponents"][name]["raw_win"] for name in anchors) / len(anchors)

        score = [100 * row[metric] for row in records]
        raw = [100 * wins(row) for row in records]
        ax.plot(updates, score, color=color, linewidth=2.4, label="Очки")
        ax.plot(updates, raw, color=color, linewidth=1.7, linestyle="--", label="Победы")
        ax.axvline(best, color="#69747b", linewidth=0.9, linestyle=":")
        ax.scatter(best, score[updates.index(best)], color=color, edgecolor="white",
                   linewidth=0.8, s=52, zorder=4, label="Выбранная версия")
        ax.grid(axis="y", alpha=0.20)
        ax.spines[["top", "right"]].set_visible(False)
        ax.set_xticks(ticks)
        ax.set_xlabel("Обновления PPO")
        ax.set_title(title, loc="left", weight="bold")
    panels[0].set_ylabel("Результат, %")
    panels[0].legend(frameon=False, loc="best", fontsize=9)
    fig.savefig(args.output, dpi=165, facecolor="white")
    plt.close(fig)


if __name__ == "__main__":
    main()
