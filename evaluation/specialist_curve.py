"""Select equal-budget specialist and control checkpoints on the M1D development set."""

import argparse
from collections import Counter
from dataclasses import asdict, replace
import hashlib
import json
from pathlib import Path
import sys
import time

from arena import native
from league_holdout import (
    ANCHORS, ROOT, atomic_write, checkpoint, measure, play, policy_from_bytes, seed_in_domain,
)
from schedule import ARENAS, BoutPair, arena_for_seed


DOMAIN = "M1D"
BUDGETS = (600, 800, 1000, 1200)
LANES = 384
PRETRAINING = {"control": 0, "treatment": 400}
RUNS = Path("training/runs/specialist-leagues")
BASELINE = Path("training/runs/league/checkpoints/update-1300.npz")
BASELINE_SHA = "96f9bddabc0eeff4a70c111db443dea064490c6e576ff8bb569966024cb26e31"
LABELS = {"control": "Обычное обучение", "treatment": "Предобучение специалистов"}


def schedule():
    classes = range(len(native.CLASS_NAMES))
    rows = []
    used = set()
    for left in classes:
        for right in range(left, len(classes)):
            for level in range(1, 6):
                starting_arena = (left + right + level) % len(ARENAS)
                for offset in (0, 2):
                    arena = (starting_arena + offset) % len(ARENAS)
                    nonce = 0
                    while True:
                        seed = seed_in_domain(DOMAIN, left, right, level, arena, nonce)
                        if arena_for_seed(seed) == arena:
                            break
                        nonce += 1
                    if seed in used:
                        raise RuntimeError("duplicate development seed")
                    used.add(seed)
                    row = BoutPair(left, right, level, arena, seed)
                    rows.append(row)
                    if left != right:
                        rows.append(replace(row, focal=right, opponent=left))
    return rows


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save_json(path, value):
    atomic_write(path, (json.dumps(value, indent=2) + "\n").encode())


def score(path, data, rows, opponents, budget, main_updates, batch_size):
    started = time.monotonic()
    cpu_started = time.process_time()
    candidate = policy_from_bytes(data)
    results = {}
    for name, rival in opponents.items():
        raw = play(rows, candidate, rival, batch_size, DOMAIN)
        results[name] = measure(raw[..., 0], raw[..., 1])
    league = sum(results[name]["score"] for name in ANCHORS) / len(ANCHORS)
    league_win = sum(results[name]["raw_win"] for name in ANCHORS) / len(ANCHORS)
    expert = results["expert"]
    # Compare exact integer numerators. Each historical anchor has weight 1/6,
    # expert has weight 1/2, and all opponents use the same number of bouts.
    weighted_points_twice = sum(2 * value["wins"] + value["draws"]
                                for value in results.values())
    weighted_points_twice += 2 * (2 * expert["wins"] + expert["draws"])
    relative_path = path.relative_to(ROOT) if path.is_relative_to(ROOT) else path
    composite = weighted_points_twice / (24 * len(rows))
    cpu_seconds = round(time.process_time() - cpu_started, 3)
    wall_seconds = round(time.monotonic() - started, 3)
    print(f"{relative_path}  {composite:.2%}  ({wall_seconds:.1f}s wall, {cpu_seconds:.1f}s CPU)",
          flush=True)
    return dict(path=str(relative_path), checkpoint_sha256=digest(data), total_updates=budget,
                main_updates=main_updates, league=league, league_raw_win=league_win,
                expert=expert["score"], expert_raw_win=expert["raw_win"],
                composite=composite, weighted_points_twice=weighted_points_twice,
                cpu_seconds=cpu_seconds, wall_seconds=wall_seconds,
                opponents=results)


def selection(records):
    if set(map(int, records)) != set(BUDGETS):
        raise ValueError("selection requires every registered budget")
    return max(records.values(), key=lambda row: (row["weighted_points_twice"], -row["total_updates"]))


def render_plot(curve, output):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.lines import Line2D
    from matplotlib.ticker import FuncFormatter, MaxNLocator

    colors = {"control": "#176569", "treatment": "#a34123"}
    fig, panels = plt.subplots(1, 3, figsize=(12.6, 4.4), layout="constrained")
    for ax, key, title in zip(panels, ("league", "expert", "composite"),
                              ("Историческая лига", "Усиленный бот", "Итоговый балл")):
        for branch, color in colors.items():
            points = [curve["baseline"], *(curve["branches"][branch][str(b)] for b in BUDGETS)]
            budgets = [row["total_updates"] for row in points]
            ax.plot(budgets, [row[key] for row in points], color=color, marker="o", ms=4)
            if key != "composite":
                ax.plot(budgets, [row[f"{key}_raw_win"] for row in points], color=color,
                        linestyle="--", alpha=.75)
            best = curve["selection"][branch]
            ax.scatter(best["total_updates"], best[key], color=color, marker="D", s=52,
                       edgecolor="white", linewidth=.8, zorder=4)
        ax.set_title(title, loc="left", weight="bold")
        ax.set_xticks((0, *BUDGETS))
        ax.set_xlabel("Обновления PPO после общего старта")
        ax.yaxis.set_major_locator(MaxNLocator(nbins=6, steps=(1, 2, 5, 10)))
        ax.yaxis.set_major_formatter(FuncFormatter(
            lambda value, _: f"{100 * value:.1f}%".replace(".", ",")))
        ax.grid(axis="y", alpha=.2)
        ax.spines[["top", "right"]].set_visible(False)
    panels[0].set_ylabel("Результат")
    fig.legend(handles=[
        *(Line2D([], [], color=color, label=LABELS[branch]) for branch, color in colors.items()),
        Line2D([], [], color="#333333", linestyle="--", label="Доля побед"),
        Line2D([], [], color="#333333", marker="D", linestyle="none", label="Выбранный чекпойнт"),
    ], loc="outside lower center", ncols=4, frameon=False, fontsize=9)
    fig.savefig(output, dpi=160, facecolor="white")
    plt.close(fig)


def render_report(curve, plot_exists):
    def percent(value):
        return f"{100 * value:.2f}%".replace(".", ",")

    lines = [
        "# Выбор чекпойнтов на M1D", "",
        "Обе ветки начинают с чекпойнта исторической лиги после 1300 обновлений. "
        "Обычная ветка выполняет 1200 основных обновлений. Ветка со специалистами "
        "тратит 400 обновлений на предобучение специалистов и 800 на основное обучение. "
        "Каждое обновление использует 384 боя.", "",
        "Расписание покрывает все 12×12 упорядоченных пар классов и уровни 1–5. "
        "На каждую пару и уровень приходятся две арены и обе рассадки. "
        "Четыре арены равномерно представлены для каждого класса на каждом уровне. "
        "На одного соперника приходится 2880 боёв и 780 разных сидов. "
        "На ничью приходится пол-очка. Итоговый балл поровну учитывает средние очки "
        "против трёх исторических сетей и очки против усиленного бота.", "",
        "У каждой ветки четыре точки выбора с одинаковым суммарным бюджетом. "
        "При равенстве баллов выбирается более ранняя точка.", "",
        "| Ветка | Всего обновлений | Основных обновлений | Лига | Усиленный бот | Итоговый балл |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    entries = [("Общий старт", curve["baseline"])]
    for budget in BUDGETS:
        for branch, label in LABELS.items():
            row = curve["branches"][branch][str(budget)]
            chosen = row == curve["selection"][branch]
            entries.append((f"**{label}**" if chosen else label, row))
    for label, row in entries:
        values = " | ".join(percent(row[key]) for key in ("league", "expert", "composite"))
        lines.append(f"| {label} | {row['total_updates']} | {row['main_updates']} | {values} |")
    regular = curve["branches"]["control"]["800"]["composite"]
    specialist = curve["branches"]["treatment"]["1200"]["composite"]
    gain = f"{100 * (specialist - regular):+.2f}".replace(".", ",")
    lines += [
        "", "При одинаковых 800 обновлениях основного обучения ветка со специалистами дала "
        f"{percent(specialist)} против {percent(regular)} у обычной ветки ({gain} п. п.). "
        "Это дополнительное сравнение на M1D. Ветка со специалистами здесь израсходовала "
        "на 400 обновлений больше.", "",
        "Для веток учитывается процессорное время четырёх DEV-оценок. "
        "Оценка общего старта выполнена один раз.", "",
        "| Оценка | Процессорное время, с |",
        "|---|---:|",
    ]
    for key, label in LABELS.items():
        cpu_seconds = sum(row["cpu_seconds"] for row in curve["branches"][key].values())
        lines.append(f"| {label} | {cpu_seconds:.2f} |".replace(".", ","))
    lines.append(f"| Общий старт | {curve['baseline']['cpu_seconds']:.2f} |".replace(".", ","))
    lines.append("")
    if plot_exists:
        lines.extend(["![Кривые обучения на M1D](curve.png)", ""])
    lines += [
        "Полное расписание находится в `manifest.json`, а хэши и результаты чекпойнтов в `curve.json`. "
        "Для воспроизведения из корня репозитория после `make release` выполните", "",
        "```sh",
        "export PYTHONPATH=python OPENBLAS_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1",
        ".venv/bin/python evaluation/specialist_curve.py --output /tmp/specialist-dev",
        "```", "",
        "Если используете замороженную сборку, укажите её каталог `python` в `PYTHONPATH`. "
        "Для изображения добавьте `--plot` и установите Matplotlib в используемое окружение.", "",
    ]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("evaluation/specialist-dev"))
    parser.add_argument("--runs", type=Path, default=RUNS)
    parser.add_argument("--batch-size", type=int, default=512)
    parser.add_argument("--partial", action="store_true", help="Score the checkpoints available so far")
    parser.add_argument("--plot", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.batch_size <= 65536:
        parser.error("batch size must be in 1..65536")

    rows = schedule()
    coverage = Counter((row.focal, row.level, row.arena) for row in rows)
    if len(rows) != 1440 or len(coverage) != 240 or set(coverage.values()) != {6}:
        raise RuntimeError("unexpected development schedule coverage")
    library_name = {"darwin": "libdnd_duel_engine.dylib", "linux": "libdnd_duel_engine.so",
                    "win32": "dnd_duel_engine.dll"}[sys.platform]
    args.output.mkdir(parents=True, exist_ok=True)
    manifest = dict(domain=DOMAIN, arena_abi=native.ARENA_ABI, candidates=list(BUDGETS),
                    lanes=LANES, pretraining_updates=PRETRAINING,
                    baseline_sha256=BASELINE_SHA, bouts_per_opponent=2 * len(rows),
                    opponent_sha256={key: value[1] for key, value in ANCHORS.items()},
                    native_library_sha256=digest((native.ROOT / "target" / "release" / library_name).read_bytes()),
                    fixtures=[asdict(row) for row in rows])
    manifest_path = args.output / "manifest.json"
    if manifest_path.exists() and json.loads(manifest_path.read_text()) != manifest:
        raise ValueError("output belongs to a different development protocol")
    save_json(manifest_path, manifest)
    curve_path = args.output / "curve.json"
    curve = json.loads(curve_path.read_text()) if curve_path.exists() else {
        "baseline": None, "branches": {branch: {} for branch in PRETRAINING},
    }
    rivals = {name: checkpoint(ROOT / path, sha, "0260a49")
              for name, (path, sha) in ANCHORS.items()}
    opponents = {**rivals, "expert": "expert"}
    baseline = ROOT / BASELINE
    data = baseline.read_bytes()
    if digest(data) != BASELINE_SHA:
        raise ValueError("starting checkpoint SHA-256 mismatch")
    if curve["baseline"] is None:
        curve["baseline"] = score(baseline, data, rows, opponents, 0, 0, args.batch_size)
        save_json(curve_path, curve)
    elif curve["baseline"]["checkpoint_sha256"] != BASELINE_SHA:
        raise ValueError("cached starting checkpoint SHA-256 mismatch")

    missing = []
    for budget in BUDGETS:
        for branch, pretraining in PRETRAINING.items():
            main_updates = budget - pretraining
            path = (args.runs / branch / "checkpoints" / f"update-{main_updates:04d}.npz").resolve()
            key = str(budget)
            records = curve["branches"][branch]
            if not path.is_file():
                missing.append(path)
                continue
            data = path.read_bytes()
            if key in records:
                if digest(data) != records[key]["checkpoint_sha256"]:
                    raise ValueError(f"cached evaluation SHA-256 mismatch for {path}")
                continue
            records[key] = score(path, data, rows, opponents, budget, main_updates, args.batch_size)
            save_json(curve_path, curve)
    if missing:
        if not args.partial:
            parser.error("missing checkpoints: " + ", ".join(str(path) for path in missing))
        return
    curve["selection"] = {branch: selection(records) for branch, records in curve["branches"].items()}
    save_json(curve_path, curve)
    plot_path = args.output / "curve.png"
    if args.plot:
        render_plot(curve, plot_path)
    atomic_write(args.output / "README.md", render_report(curve, plot_path.exists()).encode())
    for branch, row in curve["selection"].items():
        print(branch, row["total_updates"], row["path"], row["checkpoint_sha256"], flush=True)


if __name__ == "__main__":
    main()
