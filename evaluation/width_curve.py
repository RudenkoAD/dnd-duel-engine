"""Select CPU-matched width checkpoints on the W1D development fixtures."""

import argparse
from collections import Counter
from dataclasses import asdict
import gzip
import json
from pathlib import Path
import sys

from arena import native
from league_holdout import ANCHORS, ROOT, atomic_write, checkpoint, fixture_schedule
from specialist_curve import BASELINE, BASELINE_SHA, digest, save_json, score


DOMAIN = "W1D"
BUDGET_SECONDS = 1576
PERCENTAGES = (25, 50, 75, 100)
ARMS = {"narrow": "2×128", "wide": "2×256"}
RUNS = Path("training/runs/width-league")


def schedule():
    rows = [
        row for row in fixture_schedule(range(len(native.CLASS_NAMES)), range(1, 6), DOMAIN)
        if (row.arena - row.focal - row.opponent - row.level) % 4 in (0, 2)
    ]
    coverage = Counter((row.focal, row.level, row.arena) for row in rows)
    if len(rows) != 1440 or len(coverage) != 240 or set(coverage.values()) != {6}:
        raise RuntimeError("unexpected width development fixture coverage")
    return rows


def training_receipts(run):
    path = run / "training.jsonl"
    compressed = path.with_suffix(".jsonl.gz")
    if path.exists():
        with path.open() as source:
            lines = source.readlines()
    elif compressed.exists():
        with gzip.open(compressed, "rt") as source:
            lines = source.readlines()
    else:
        return {}
    receipts = {}
    for line in lines:
        if not line.endswith("\n"):
            break
        row = json.loads(line)
        if row.get("event") == "checkpoint":
            percentage = 100 * row["budget_fraction"]
            if percentage not in PERCENTAGES:
                raise ValueError(f"unexpected CPU checkpoint at {percentage}% in {path}")
            percentage = int(percentage)
            if percentage in receipts:
                raise ValueError(f"duplicate {percentage}% checkpoint receipt in {path}")
            receipts[percentage] = row
    return receipts


def select(records):
    if set(records) != {str(percent) for percent in PERCENTAGES}:
        raise ValueError("selection requires all four registered CPU checkpoints")
    return max(records.values(), key=lambda row: (row["weighted_points_twice"], -row["budget_percent"]))


def evaluate(path, data, rows, opponents, percentage, updates, batch_size, training_cpu):
    record = score(path, data, rows, opponents, percentage, updates, batch_size)
    del record["total_updates"]
    record["budget_percent"] = percentage
    record["training_cpu_seconds"] = training_cpu
    return record


def render_plot(curve, output):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.lines import Line2D
    from matplotlib.ticker import PercentFormatter

    colors = {"narrow": "#176569", "wide": "#a34123"}
    fig, axes = plt.subplots(1, 3, figsize=(11.4, 4.1), layout="constrained")
    for ax, metric, title in zip(axes, ("league", "expert", "composite"),
                                 ("Историческая лига", "Усиленный бот", "Итоговый балл")):
        for arm, label in ARMS.items():
            baseline = curve["baseline"] if arm == "narrow" else curve["wide_initial"]
            points = [baseline, *(curve["branches"][arm][str(p)] for p in PERCENTAGES)]
            budgets = [point["training_cpu_seconds"] for point in points]
            ax.plot(budgets, [point[metric] for point in points], "o-", color=colors[arm],
                    markersize=4, label=label)
            chosen = curve["selection"][arm]
            ax.scatter(chosen["training_cpu_seconds"], chosen[metric], marker="D", s=54,
                       color=colors[arm], edgecolor="white", zorder=3)
        ax.set_title(title, loc="left", fontweight="bold")
        ax.set_xticks([0, *(BUDGET_SECONDS * p // 100 for p in PERCENTAGES)])
        ax.set_xlabel("Процессорное время обучения, с")
        ax.yaxis.set_major_formatter(PercentFormatter(xmax=1, decimals=1))
        ax.grid(axis="y", alpha=.2)
        ax.spines[["top", "right"]].set_visible(False)
    axes[0].set_ylabel("Очки W1D")
    handles, labels = axes[0].get_legend_handles_labels()
    handles.append(Line2D([], [], marker="D", color="#333333", linestyle="none"))
    labels.append("Выбранный чекпойнт")
    fig.legend(handles, labels, ncols=3, loc="outside lower center", frameon=False)
    fig.savefig(output, dpi=160, facecolor="white")
    plt.close(fig)


def render_report(curve, plot):
    lines = [
        "# Ширина сети на W1D", "",
        "Сравнение 2×128 и 2×256 при одинаковом лимите 1576 процессорных секунд обучения. "
        "Обе сети наследуют политику после 1300 обновлений. Широкая сеть получает её через "
        "расширение скрытых слоёв. Начальные политики показаны как ориентир.", "",
        "2880 боёв на соперника покрывают 12×12 упорядоченных пар классов, уровни 1–5, "
        "две арены на пару и уровень и обе рассадки. Оценки используют одни и те же сиды W1D. "
        "Нейросетевые политики жадно выбирают разрешённые действия. Победа приносит очко, "
        "ничья пол-очка. Итоговый балл поровну учитывает среднее по трём историческим "
        "сетям и очки против усиленного бота.", "",
        "Для каждой ширины выбирается лучший чекпойнт из 25%, 50%, 75% и 100% лимита. "
        "При точном равенстве выигрывает более ранняя точка. Таблица выделяет выбранные точки.", "",
        "| Сеть | Лимит | Процессорное время обучения, с | Обновления | Лига | Бот | Итог | DEV CPU, с |",
        "|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for arm, label in ARMS.items():
        starting = curve["baseline"] if arm == "narrow" else curve["wide_initial"]
        records = curve["branches"][arm]
        for row in [starting, *(records[str(p)] for p in PERCENTAGES)]:
            name = f"**{label}**" if row == curve["selection"][arm] else label
            scores = " | ".join(f"{row[key]:.2%}".replace(".", ",")
                                for key in ("league", "expert", "composite"))
            lines.append(
                f"| {name} | {row['budget_percent']}% | {row['training_cpu_seconds']:.2f} | "
                f"{row['main_updates']} | {scores} | {row['cpu_seconds']:.2f} |"
            )
    totals = curve["selection_cpu_seconds"]
    initial_parity = curve["baseline"]["opponents"] == curve["wide_initial"]["opponents"]
    initial_text = (
        "У начальных сетей совпали сводки по победам, ничьим, поражениям и причинам завершения "
        "для каждого соперника."
        if initial_parity else
        "Итоги начальных сетей разошлись как минимум у одного соперника."
    )
    lines += [
        "", f"Четыре DEV-оценки заняли {totals['narrow']:.2f} с CPU для 2×128 и "
        f"{totals['wide']:.2f} с CPU для 2×256. Эти расходы и оценки начальных политик "
        "учтены отдельно от лимита обучения. Время включает симуляцию боёв и инференс.", "",
        initial_text, "",
    ]
    if plot:
        lines += ["![Кривые обучения на W1D](curve.png)", ""]
    lines += [
        "Хэши чекпойнтов и результаты находятся в `curve.json`, расписание и хэш библиотеки в "
        "`manifest.json`. Для воспроизведения запустите из корня репозитория с обученными "
        "чекпойнтами, собранной release-библиотекой и Matplotlib.", "",
        "```sh",
        "export PYTHONPATH=python OPENBLAS_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1",
        ".venv/bin/python evaluation/width_curve.py --output /tmp/width-dev --plot",
        "```", "",
        "Для замороженной сборки подставьте её каталог `python` в `PYTHONPATH` и сверяйте "
        "SHA-256 библиотеки с манифестом.", "",
    ]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=Path, default=RUNS)
    parser.add_argument("--output", type=Path, default=Path("evaluation/width-dev"))
    parser.add_argument("--batch-size", type=int, default=512)
    parser.add_argument("--partial", action="store_true", help="Score the checkpoints available so far")
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
        baseline_sha256=BASELINE_SHA, budget_seconds=BUDGET_SECONDS, budgets_percent=list(PERCENTAGES),
        opponent_sha256={key: value[1] for key, value in ANCHORS.items()},
        bouts_per_opponent=2 * len(rows), batch_size=args.batch_size,
        fixtures=[asdict(row) for row in rows],
    )
    args.output.mkdir(parents=True, exist_ok=True)
    manifest_path = args.output / "manifest.json"
    if manifest_path.exists() and json.loads(manifest_path.read_text()) != manifest:
        raise ValueError("output belongs to a different development protocol")
    save_json(manifest_path, manifest)
    curve_path = args.output / "curve.json"
    curve = json.loads(curve_path.read_text()) if curve_path.exists() else {
        "baseline": None, "wide_initial": None, "branches": {arm: {} for arm in ARMS},
    }
    opponents = {name: checkpoint(ROOT / path, sha, "0260a49")
                 for name, (path, sha) in ANCHORS.items()}
    opponents["expert"] = "expert"

    def evaluate_once(records, key, path, expected_sha=None, percentage=0, updates=0, cpu=0):
        path = path.resolve()
        data = path.read_bytes()
        sha = digest(data)
        if expected_sha is not None and sha != expected_sha:
            raise ValueError(f"checkpoint SHA-256 mismatch for {path}")
        if records.get(key) is None:
            records[key] = evaluate(path, data, rows, opponents, percentage, updates, args.batch_size, cpu)
            save_json(curve_path, curve)
        elif (records[key]["checkpoint_sha256"], records[key]["main_updates"],
              records[key]["training_cpu_seconds"]) != (sha, updates, cpu):
            raise ValueError(f"cached evaluation SHA-256 mismatch for {path}")

    evaluate_once(curve, "baseline", ROOT / BASELINE, BASELINE_SHA)
    missing = []
    wide_initial = args.runs / "wide-init.npz"
    if wide_initial.is_file():
        evaluate_once(curve, "wide_initial", wide_initial)
    else:
        missing.append(wide_initial)
    receipts = {arm: training_receipts(args.runs / arm) for arm in ARMS}
    for percentage in PERCENTAGES:
        for arm in ARMS:
            path = args.runs / arm / "checkpoints" / f"cpu-{percentage:03}.npz"
            receipt = receipts[arm].get(percentage)
            if receipt is None or not path.is_file():
                missing.append(path)
                continue
            evaluate_once(curve["branches"][arm], str(percentage), path, receipt["sha256"],
                          percentage, receipt["update"], receipt["cpu_s"])
    if missing:
        if not args.partial:
            parser.error("missing checkpoints or receipts: " + ", ".join(map(str, missing)))
        return
    curve["selection"] = {arm: select(records) for arm, records in curve["branches"].items()}
    curve["selection_cpu_seconds"] = {
        arm: round(sum(row["cpu_seconds"] for row in records.values()), 3)
        for arm, records in curve["branches"].items()
    }
    save_json(curve_path, curve)
    if args.plot:
        render_plot(curve, args.output / "curve.png")
    atomic_write(args.output / "README.md", render_report(curve, args.plot).encode())
    for arm, row in curve["selection"].items():
        print(arm, row["budget_percent"], row["path"], row["checkpoint_sha256"], flush=True)


if __name__ == "__main__":
    main()
