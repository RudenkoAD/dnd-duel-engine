"""Plot scores from matched expert and neural roster tournaments."""
import argparse
import csv
import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np


def panel(ax, data, label, names):
    n = len(names)
    cells = np.full((n, n), np.nan)
    for (i, j), value in zip(data["class_pairs"], data["statistics"]["roster"]["class_scores"]):
        cells[i, j], cells[j, i] = value, 1 - value
    colors = plt.get_cmap("RdBu").copy()
    colors.set_bad("#e4e7e8")
    image = ax.imshow(cells, cmap=colors, vmin=0, vmax=1)
    ax.set_xticks(range(n), labels=names, rotation=56, ha="right")
    ax.set_yticks(range(n), labels=names)
    ax.set_title(label, pad=10)
    ax.tick_params(length=0, pad=3, labelsize=8)
    ax.set_xticks(np.arange(-.5, n, 1), minor=True)
    ax.set_yticks(np.arange(-.5, n, 1), minor=True)
    ax.grid(which="minor", color="white", linewidth=.45)
    ax.tick_params(which="minor", length=0)
    for i, j in zip(*np.where(np.isfinite(cells))):
        value = cells[i, j]
        ax.text(j, i, f"{100 * value:.0f}", color="white" if abs(value - .5) > .29 else "#19242b",
                ha="center", va="center", fontsize=7)
    for spine in ax.spines.values():
        spine.set_visible(False)
    return image


def write_build_table(path, runs):
    from arena.native import roster

    entries = roster()
    with path.open("w", newline="", encoding="utf-8") as output:
        writer = csv.writer(output)
        writer.writerow(["controller", "class_A", "subclass_A", "class_B", "subclass_B",
                         "games", "wins_A", "draws", "wins_B", "score_A_pct", "one_sided_KOs"])
        for report, summary in runs:
            with np.load(report.with_name("bouts.npz"), allow_pickle=False) as data:
                roster_bouts = data["roster"]
                for pair, (one, two) in enumerate(summary["class_pairs"]):
                    for variant, (a, b) in enumerate(summary["build_variants"]):
                        rows = roster_bouts[pair, variant].reshape(-1, len(summary["raw_fields"]))
                        outcome, reason, hp, maximum = rows.T.astype(np.int32)
                        wins = int((outcome == 1).sum())
                        draws = int((outcome == 0).sum())
                        dominant = int(((reason == 1) & (4 * hp >= 3 * maximum)).sum())
                        writer.writerow([summary["controller"], summary["classes"][one],
                                         entries[3 * one + a]["name"], summary["classes"][two],
                                         entries[3 * two + b]["name"], len(rows), wins, draws,
                                         len(rows) - wins - draws, round(100 * (wins + .5 * draws) / len(rows), 3),
                                         dominant])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("expert", type=Path)
    parser.add_argument("policy", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--csv", type=Path, help="also export counts for every tested subclass matchup")
    args = parser.parse_args()
    expert = json.loads(args.expert.read_text())
    policy = json.loads(args.policy.read_text())
    if expert["classes"] != policy["classes"] or expert["native_sha256"] != policy["native_sha256"]:
        raise ValueError("Mismatched roster or native build")
    names = ["Варвар", "Бард", "Жрец", "Друид", "Воин", "Монах", "Паладин",
             "Следопыт", "Плут", "Чародей", "Колдун", "Волшебник"]
    fig, axes = plt.subplots(1, 2, figsize=(13, 6.5), constrained_layout=True)
    fig.suptitle("Сколько очков набрал класс из строки против класса из столбца, %", fontsize=14)
    left = panel(axes[0], expert, "Усиленный бот играет за обе стороны", names)
    panel(axes[1], policy, "Сеть 2×256 играет за обе стороны", names)
    legend = fig.colorbar(left, ax=axes, shrink=.66, pad=.02, ticks=np.linspace(0, 1, 5))
    legend.set_ticklabels(["0", "25", "50", "75", "100"])
    fig.savefig(args.out, dpi=160)
    if args.csv:
        write_build_table(args.csv, [(args.expert, expert), (args.policy, policy)])
    plt.close(fig)


if __name__ == "__main__":
    main()
