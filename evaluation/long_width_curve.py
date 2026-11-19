"""Reevaluate fixed width references and rank the three extended wide checkpoints on B2D."""

import argparse
from collections import Counter
from dataclasses import asdict
import json
from pathlib import Path
import sys

from arena import native
from league_holdout import ANCHORS, ROOT, atomic_write, checkpoint, fixture_schedule
from specialist_curve import BASELINE, BASELINE_SHA, LANES, digest, save_json, score


DOMAIN = "B2D"
START_UPDATES = 1300
CHECKPOINTS = (967, 1934, 2901)
RUNS = Path("training/runs/long-width")
WIDTH = Path("training/runs/width-league")
REFERENCES = {
    "baseline": (BASELINE, BASELINE_SHA, 0),
    "old-wide": (WIDTH / "wide/checkpoints/cpu-100.npz",
                 "1c8cfe14f7a18f5455e022dfcc27376ecb273ae276a1a493914680c4d7fc51b5", 757),
    "old-narrow": (WIDTH / "narrow/checkpoints/cpu-100.npz",
                   "4f4bb5c76eb175e1028b18761df01b5206fb379930a9fbca9b19c82336edc346", 967),
}
LABELS = {
    "baseline": "Общий старт",
    "old-wide": "2×256, прежний чекпойнт",
    "old-narrow": "2×128, прежний чекпойнт",
}


def schedule():
    rows = [
        row for row in fixture_schedule(range(len(native.CLASS_NAMES)), range(1, 6), DOMAIN)
        if (row.arena - row.focal - row.opponent - row.level) % 4 in (0, 2)
    ]
    coverage = Counter((row.focal, row.level, row.arena) for row in rows)
    if len(rows) != 1440 or len(coverage) != 240 or set(coverage.values()) != {6}:
        raise RuntimeError("unexpected B2D coverage")
    return rows


def render_plot(curve, output):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.lines import Line2D
    from matplotlib.ticker import FuncFormatter, MaxNLocator

    records = curve["records"]
    wide = [records["old-wide"], *(records[str(n)] for n in CHECKPOINTS)]
    ticks = [records["baseline"]["training_duels_after_start"], *(row["training_duels_after_start"] for row in wide)]
    color = "#176569"
    fig, panels = plt.subplots(1, 3, figsize=(12.6, 4.6), layout="constrained")
    metrics = ("league", "expert", "composite")
    titles = ("Историческая лига", "Усиленный бот", "Итоговый балл")
    for ax, metric, title in zip(panels, metrics, titles):
        ax.plot([row["training_duels_after_start"] for row in wide], [row[metric] for row in wide],
                color=color, marker="o", ms=5)
        ax.plot([row["training_duels_after_start"] for row in wide],
                [row[f"{metric}_raw_win"] for row in wide], color=color, linestyle="--", alpha=.75)
        for label, marker, tint in (("baseline", "s", "#5e6a72"), ("old-narrow", "^", "#a34123")):
            point = records[label]
            ax.scatter(point["training_duels_after_start"], point[metric], marker=marker, color=tint,
                       s=58, zorder=4, edgecolor="white", linewidth=.65)
            ax.scatter(point["training_duels_after_start"], point[f"{metric}_raw_win"], marker=marker,
                       facecolors="none", edgecolors=tint, s=43)
        best = records[str(curve["best_update"])]
        ax.scatter(best["training_duels_after_start"], best[metric], color=color, marker="D", s=65,
                   edgecolor="white", zorder=4)
        ax.set_title(title, loc="left", weight="bold")
        ax.set_xticks(ticks, [f"{n / 1000:.0f}" for n in ticks], rotation=32)
        ax.yaxis.set_major_locator(MaxNLocator(nbins=6))
        ax.yaxis.set_major_formatter(FuncFormatter(lambda value, _: f"{100 * value:.1f}%".replace(".", ",")))
        ax.grid(axis="y", alpha=.2)
        ax.spines[["top", "right"]].set_visible(False)
    panels[0].set_ylabel("Результат B2D")
    panels[1].set_xlabel("Обучающие бои после общего старта, тыс.", fontsize=10)
    fig.legend(handles=[
        Line2D([], [], color=color, marker="o", label="2×256, включая прежний чекпойнт"),
        Line2D([], [], color="#5e6a72", marker="s", linestyle="none", label=LABELS["baseline"]),
        Line2D([], [], color="#a34123", marker="^", linestyle="none", label=LABELS["old-narrow"]),
        Line2D([], [], color="#333333", linestyle="--", label="Доля побед"),
        Line2D([], [], color=color, marker="D", linestyle="none", label="Лучший новый на B2D"),
    ], loc="outside lower center", ncols=3, frameon=False, fontsize=9)
    fig.savefig(output, dpi=180, facecolor="white")
    plt.close(fig)


def render_report(curve, plot_exists):
    lines = [
        "# Длительное обучение 2×256 на B2D", "",
        "Общий стартовый чекпойнт получен после 1300 обновлений PPO по 384 боя. Новый прогон продолжил "
        "обучение 2×256 с чекпойнта на 757-м обновлении. Точка 967 совпадает по числу обучающих боёв с "
        "прежним 2×128. Отсчёт боёв начинается с развилки ширин, после общего этапа обучения. "
        "Продолжение использовало сохранённые веса и состав лиги прежнего 2×256 "
        "и заново инициализировало Adam.", "",
        "На одного соперника приходится 2880 боёв. Расписание покрывает все 12×12 "
        "упорядоченных пар классов, уровни 1–5, две арены и обе рассадки. Нейросетевые политики "
        "жадно выбирают разрешённые действия. Балл учитывает половину очка за ничью. "
        "Итог даёт равный вес среднему по трём историческим политикам и усиленному боту. "
        "Доля побед вычисляется с теми же весами.", "",
        "В рейтинге новых точек порядок задаёт точный итоговый балл B2D, при равенстве "
        "ранняя точка выше. Чекпойнт 2901 зафиксирован заранее как конечная точка. "
        "Точки 967, 1934 и 2901 соответствуют одному, двум и трём бюджетам дуэлей прежней узкой сети.", "",
        "| Чекпойнт | Обновления после старта | Боёв после старта | Лига, балл / победы | Бот, балл / победы | Итог, балл / победы |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for key in (*REFERENCES, *map(str, CHECKPOINTS)):
        row = curve["records"][key]
        title = LABELS.get(key, f"2×256, {key} обновлений")
        metrics = " | ".join(
            f"{row[metric]:.2%} / {row[f'{metric}_raw_win']:.2%}".replace(".", ",")
            for metric in ("league", "expert", "composite")
        )
        bouts = f"{row['training_duels_after_start']:,}".replace(",", " ")
        lines.append(f"| {title} | {row['main_updates']} | {bouts} | {metrics} |")
    ranking = " → ".join(map(str, curve["ranking"]))
    lines += ["", f"Порядок новых чекпойнтов по B2D от лучшего к худшему — {ranking}.", ""]
    if plot_exists:
        lines += ["![Кривые B2D](curve.png)", ""]
    lines += [
        "Расписание, хэши библиотеки и соперников сохранены в `manifest.json`. "
        "Хэши чекпойнтов и результаты по каждому сопернику и рассадке сохранены в `curve.json`. "
        "Для построения графика нужен Matplotlib.", "",
        "```sh",
        "export PYTHONPATH=/tmp/dnd-spec-frozen-621964e7/python OPENBLAS_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1",
        ".venv/bin/python evaluation/long_width_curve.py --plot",
        "```", "",
    ]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=Path, default=RUNS)
    parser.add_argument("--output", type=Path, default=Path("evaluation/long-width-dev"))
    parser.add_argument("--batch-size", type=int, default=512)
    parser.add_argument("--partial", action="store_true")
    parser.add_argument("--plot", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.batch_size <= 65536:
        parser.error("batch size must be in 1..65536")

    rows = schedule()
    library_name = {"darwin": "libdnd_duel_engine.dylib", "linux": "libdnd_duel_engine.so",
                    "win32": "dnd_duel_engine.dll"}[sys.platform]
    library = native.ROOT / "target" / "release" / library_name
    manifest = dict(
        domain=DOMAIN, arena_abi=native.ARENA_ABI, native_library_sha256=digest(library.read_bytes()),
        references={key: dict(path=str(path), sha256=sha, updates=updates)
                    for key, (path, sha, updates) in REFERENCES.items()},
        candidate_updates=list(CHECKPOINTS), fixed_terminal_update=CHECKPOINTS[-1],
        ranking_rule="Descending exact weighted points; ascending updates on ties",
        starting_updates=START_UPDATES, training_duels_per_update=LANES,
        opponent_sha256={key: value[1] for key, value in ANCHORS.items()},
        opponent_weights={**{key: "1/6" for key in ANCHORS}, "expert": "1/2"},
        bouts_per_opponent=2 * len(rows), batch_size=args.batch_size,
        fixtures=[asdict(row) for row in rows],
    )
    args.output.mkdir(parents=True, exist_ok=True)
    manifest_path = args.output / "manifest.json"
    if manifest_path.exists():
        if json.loads(manifest_path.read_text()) != manifest:
            raise ValueError("output belongs to a different development protocol")
    else:
        save_json(manifest_path, manifest)
    curve_path = args.output / "curve.json"
    curve = json.loads(curve_path.read_text()) if curve_path.exists() else {"records": {}}
    opponents = {name: checkpoint(ROOT / path, sha, "0260a49")
                 for name, (path, sha) in ANCHORS.items()}
    opponents["expert"] = "expert"

    def evaluate_once(key, path, expected_sha, updates):
        path = (ROOT / path).resolve()
        data = path.read_bytes()
        if digest(data) != expected_sha:
            raise ValueError(f"checkpoint SHA-256 mismatch for {path}")
        previous = curve["records"].get(key)
        if previous:
            if (previous["checkpoint_sha256"], previous["main_updates"]) != (expected_sha, updates):
                raise ValueError(f"cached evaluation mismatch for {path}")
            return
        row = score(path, data, rows, opponents, updates, updates, args.batch_size)
        row["training_duels_after_start"] = updates * LANES
        row["composite_raw_win"] = (row["league_raw_win"] + row["expert_raw_win"]) / 2
        curve["records"][key] = row
        save_json(curve_path, curve)

    for key, (path, sha, updates) in REFERENCES.items():
        evaluate_once(key, path, sha, updates)
    missing = []
    for updates in CHECKPOINTS:
        path = ROOT / args.runs / "checkpoints" / f"update-{updates:04d}.npz"
        receipt_path = path.with_suffix(".json")
        if not receipt_path.is_file():
            missing.append(receipt_path)
            continue
        receipt = json.loads(receipt_path.read_text())
        continued = updates - REFERENCES["old-wide"][2]
        if (receipt["update"], receipt["bouts"], receipt["new_updates"], receipt["new_bouts"]) != (
            updates, updates * LANES, continued, continued * LANES,
        ):
            raise ValueError(f"unexpected training totals in {receipt_path}")
        evaluate_once(str(updates), path, receipt["sha256"], updates)
    if missing:
        if not args.partial:
            parser.error("missing checkpoint receipts: " + ", ".join(map(str, missing)))
        return

    curve["ranking"] = sorted(CHECKPOINTS, key=lambda n: (-curve["records"][str(n)]["weighted_points_twice"], n))
    curve["best_update"] = curve["ranking"][0]
    curve["fixed_terminal_update"] = CHECKPOINTS[-1]
    save_json(curve_path, curve)
    plot_path = args.output / "curve.png"
    if args.plot:
        render_plot(curve, plot_path)
    atomic_write(args.output / "README.md", render_report(curve, plot_path.is_file()).encode())
    for label in ("best_update", "fixed_terminal_update"):
        row = curve["records"][str(curve[label])]
        print(label, row["main_updates"], row["path"], row["checkpoint_sha256"], flush=True)


if __name__ == "__main__":
    main()
