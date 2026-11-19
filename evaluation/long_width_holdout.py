"""Evaluate the frozen extended wide training run on independent B2F bouts."""

import argparse
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict
from functools import partial
import json
from pathlib import Path
from shlex import join
import sys

import numpy as np

from arena import native
from league_holdout import (
    ANCHORS, ROOT, REASONS, SmokePolicy, atomic_write, checkpoint, fixture_schedule, measure,
)
from specialist_holdout import (
    BOOTSTRAP_SAMPLES, BOOTSTRAP_SEED, CHAMPION_1300_SHA, NATIVE_SHA,
    format_interval, paired_bands, shared_opponent_bands,
)
from width_holdout import OPPONENTS, PRACTICAL, contrast_table, evaluate_match, hash_file, plateau_result

START = "champion-1300"
NARROW = "narrow-cpu100"
OLD_WIDE = "wide-cpu100"
CURVE = ("wide-967", "wide-1934", "wide-2901")
TERMINAL = CURVE[-1]
BASELINES = (OLD_WIDE, NARROW, START)
FOCALS = (*BASELINES, *CURVE)
PRIMARY = tuple((TERMINAL, baseline) for baseline in BASELINES)
DIRECT = PRIMARY
CONTEXTUAL_DIRECT = tuple((focal, NARROW) for focal in CURVE[:-1])
FROZEN = {
    START: CHAMPION_1300_SHA,
    NARROW: "4f4bb5c76eb175e1028b18761df01b5206fb379930a9fbca9b19c82336edc346",
    OLD_WIDE: "1c8cfe14f7a18f5455e022dfcc27376ecb273ae276a1a493914680c4d7fc51b5",
}


def load_policies(args):
    if args.smoke:
        return {name: SmokePolicy(i) for i, name in enumerate((*ANCHORS, *FOCALS))}, {}
    entries = {name: (Path(path), sha) for name, path, sha in args.policy}
    if set(entries) != set(FOCALS) or len(entries) != len(args.policy):
        raise ValueError(f"supply one --policy per name in {FOCALS}")
    for name, expected_sha in FROZEN.items():
        if entries[name][1] != expected_sha:
            raise ValueError(f"{name} must hash to {expected_sha}")
    configs = {name: (ROOT / path, sha) for name, (path, sha) in ANCHORS.items()}
    configs.update(entries)
    policies = {
        name: checkpoint(path, sha, "0260a49" if name == "champion" else None)
        for name, (path, sha) in configs.items()
    }
    return policies, {name: sha for name, (_, sha) in configs.items()}


def contextual_pairs(dev_best):
    pairs = [(CURVE[0], OLD_WIDE), (CURVE[1], CURVE[0]), (TERMINAL, CURVE[1])]
    if dev_best != TERMINAL:
        pairs.extend((dev_best, baseline) for baseline in BASELINES)
    return tuple(dict.fromkeys(pairs))


def direct_bands(rows, raw, matches):
    scores = {
        f"{focal}:{foe}:score_minus_half": raw[focal, foe][..., 0].mean(axis=1) / 2
        for focal, foe in matches
    }
    bands = paired_bands(rows, scores)
    for key, result in list(bands.items()):
        bands[key.replace("score_minus_half", "raw_win_minus_raw_loss")] = {
            "difference": 2 * result["difference"],
            "ci95_joint": [2 * x for x in result["ci95_joint"]],
            "ci95_pointwise": [2 * x for x in result["ci95_pointwise"]],
        }
    return bands


def summarize(rows, raw, dev_best):
    historical = {
        focal: np.stack([raw[focal, foe] for foe in ANCHORS], axis=1)
        for focal in FOCALS
    }
    records = {**raw, **{(focal, "historical"): bouts for focal, bouts in historical.items()}}
    levels = np.array([row.level for row in rows])
    arenas = np.array([row.arena for row in rows])

    def rates(data):
        result = measure(data[..., 0], data[..., 1])
        for seat, by_seat in enumerate(result["seats"]):
            by_seat["raw_win"] = float((data[..., seat, 0] == 1).mean())
        return result

    def breakdown(data):
        return {
            **rates(data),
            "levels": {str(level): rates(data[levels == level]) for level in np.unique(levels)},
            "arenas": {str(arena): rates(data[arenas == arena]) for arena in np.unique(arenas)},
        }

    return {
        "matchups": {f"{focal}:{foe}": breakdown(data) for (focal, foe), data in records.items()},
        "primary": shared_opponent_bands(rows, raw, PRIMARY, historical),
        "contextual": shared_opponent_bands(rows, raw, contextual_pairs(dev_best), historical),
        "direct": direct_bands(rows, raw, DIRECT),
        "contextual_direct": direct_bands(rows, raw, CONTEXTUAL_DIRECT),
    }


def match_row(focal, foe, metrics):
    wdl = "/".join(str(metrics[key]) for key in ("wins", "draws", "losses"))
    return f"| {focal} | {foe} | {wdl} | {metrics['raw_win']:.2%} | {metrics['score']:.2%} |"


def render(metadata, report, args):
    count = 2 * len(metadata["fixtures"])
    primary = report["primary"]
    lines = [
        "# Широкая сеть после трёх бюджетов боёв", "",
        f"Домен `{metadata['domain']}`. {count} боёв на пару политик. "
        "Полное расписание включает 12×12 упорядоченных пар классов, уровни 1–5, четыре арены и обе рассадки.",
        "В historical три прежние политики имеют равный вес. Expert — встроенный усиленный бот. "
        "Победа, ничья и поражение дают 1, ½ и 0 очков соответственно.",
        "Широкие чекпойнты 967, 1934 и 2901 накопили 371 328, 742 656 и 1 113 984 боёв. "
        "Это 1×, 2× и 3× числа боёв узкой сети на cpu100.",
        "Продолжение началось с wide-cpu100 после 757 обновлений, с восстановленной лигой "
        "и новой инициализацией Adam. Сравнение описывает эту траекторию обучения.", "",
        "## Основные сравнения", "",
        "Срез 2901 зафиксирован заранее. Интервалы 95% совместны для 12 разностей. "
        "Парный бутстрэп объединяет обе рассадки, оба направления пары классов, все политики "
        "и общих соперников с одним сидом. Страты заданы уровнем, ареной и совпадением классов.", "",
    ]
    for baseline in BASELINES:
        for foe in ("historical", "expert"):
            score = primary[f"{TERMINAL}:{baseline}:{foe}:score"]
            lines.append(f"- {baseline}, {foe}. Очки {format_interval(score, 'ci95_joint')} п. п. "
                         f"Практический эффект {plateau_result(score['ci95_joint'])}.")
    lines.extend([
        "", *contrast_table(primary), "",
        "## Промежуточные срезы и выбор DEV", "",
        f"До запуска был выбран `{metadata['dev_best']}` на DEV. "
        "В этой таблице для всех разностей используется отдельное совместное семейство интервалов.", "",
        *contrast_table(report["contextual"]), "",
        "## Очные встречи", "",
        "Совместные 95% интервалы для очков используют отдельное семейство. "
        "Разность долей побед и поражений равна удвоенному отклонению очков от 50%.", "",
        "| Политика | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] |",
        "|---|---|---|---:|",
    ])
    for index, group in enumerate(("direct", "contextual_direct")):
        if index:
            lines.extend([
                "", "### Срезы 1× и 2× против узкой сети", "",
                "Эти описательные интервалы образуют собственное семейство.", "",
                "| Политика | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] |",
                "|---|---|---|---:|",
            ])
        for key, result in report[group].items():
            focal, foe, metric = key.split(":")
            lines.append(f"| {focal} | {foe} | {metric} | {format_interval(result, 'ci95_joint')} |")
    lines.extend([
        "", "## Исходы боёв", "",
        "| Политика | Соперник | В/Н/П | Победы | Очки |",
        "|---|---|---:|---:|---:|",
    ])
    for focal in FOCALS:
        for foe in ("historical", *OPPONENTS):
            lines.append(match_row(focal, foe, report["matchups"][f"{focal}:{foe}"]))
    for focal, foe in (*CONTEXTUAL_DIRECT, *DIRECT, (START, START)):
        lines.append(match_row(focal, foe, report["matchups"][f"{focal}:{foe}"]))
    lines.extend([
        "", "`manifest.json` хранит расписание и SHA-256 входов. "
        "`summary.json` содержит результаты по уровню, арене и рассадкам. "
        "Сжатые файлы содержат исходы отдельных боёв и причины окончания.", "",
    ])
    if not args.smoke:
        command = [
            ["OPENBLAS_NUM_THREADS=1", "VECLIB_MAXIMUM_THREADS=1", "PYTHONPATH=" + str(native.ROOT / "python"),
             ".venv/bin/python", "evaluation/long_width_holdout.py", "--release"],
            *[["--policy", *entry] for entry in args.policy],
            ["--dev-best", args.dev_best, "--batch-size", str(args.batch_size), "--jobs", str(args.jobs)],
            ["--output", str(args.output.with_name(args.output.name + "-reproduced"))],
        ]
        continuation = " " + chr(92) + "\n  "
        lines.extend(["## Воспроизведение", "", "```bash", continuation.join(map(join, command)), "```", ""])
    return "\n".join(lines)


def run(args):
    domain = "B2X" if args.smoke else "B2F"
    classes = (0, 2, 11) if args.smoke else range(len(native.CLASS_NAMES))
    levels = (5,) if args.smoke else range(1, 6)
    rows = fixture_schedule(classes, levels, domain)
    if not args.smoke and (len(rows) != 2880 or len({row.seed for row in rows}) != 1560):
        raise RuntimeError("unexpected B2F schedule size")
    policies, hashes = load_policies(args)
    suffix = {"darwin": "dylib", "linux": "so"}[sys.platform]
    library = native.ROOT / "target" / "release" / f"libdnd_duel_engine.{suffix}"
    library_hash = hash_file(library)
    if library_hash != NATIVE_SHA:
        raise ValueError("native library differs from the frozen release")
    with native.Batch(1):
        pass
    matches = tuple((focal, foe) for focal in FOCALS for foe in OPPONENTS)
    matches += DIRECT + CONTEXTUAL_DIRECT + ((START, START),)
    metadata = {
        "domain": domain,
        "arena_abi": native.ARENA_ABI,
        "native_library_sha256": library_hash,
        "python_version": sys.version.split()[0],
        "numpy_version": np.__version__,
        "batch_size": args.batch_size,
        "checkpoint_sha256": hashes,
        "dev_best": args.dev_best,
        "primary_comparisons": PRIMARY,
        "contextual_comparisons": contextual_pairs(args.dev_best),
        "direct_comparisons": DIRECT,
        "contextual_direct_comparisons": CONTEXTUAL_DIRECT,
        "historical_anchors": tuple(ANCHORS),
        "matches": {f"{focal}:{foe}": f"{focal}-vs-{foe}.json.gz" for focal, foe in matches},
        "raw_layout": ["ordered fixture", "focal seat", "field (outcome -1/0/1, terminal reason code)"],
        "terminal_reasons": REASONS,
        "bootstrap_samples": BOOTSTRAP_SAMPLES,
        "bootstrap_seed": BOOTSTRAP_SEED,
        "practical_difference": PRACTICAL,
        "fixtures": [asdict(row) for row in rows],
    }
    args.output.mkdir(parents=True, exist_ok=True)
    manifest = args.output / "manifest.json"
    frozen = json.dumps(metadata, indent=2) + "\n"
    if manifest.exists():
        if manifest.read_text() != frozen:
            raise ValueError("output belongs to a different holdout manifest")
    elif any(args.output.iterdir()):
        raise ValueError("output must be empty or contain the matching manifest")
    else:
        atomic_write(manifest, frozen.encode())
    summary = args.output / "summary.json"
    if summary.exists():
        previous = json.loads(summary.read_bytes())["raw_sha256"]
        if set(previous) != set(metadata["matches"].values()) or any(
            hash_file(args.output / name) != digest for name, digest in previous.items()
        ):
            raise ValueError("cached bout results differ from the completed report")
    match = partial(evaluate_match, args=args, rows=rows, policies=policies, domain=domain)
    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        raw = dict(executor.map(match, matches))
    if hash_file(library) != library_hash:
        raise RuntimeError("native library changed during evaluation")
    swapped = {(row.opponent, row.focal, row.seed): i for i, row in enumerate(rows)}
    reverse = [swapped[row.focal, row.opponent, row.seed] for row in rows]
    self_play = raw[START, START]
    mirror = self_play[reverse, ::-1]
    if not (np.array_equal(self_play[..., 0], -mirror[..., 0])
            and np.array_equal(self_play[..., 1], mirror[..., 1])):
        raise RuntimeError("champion-1300 self-match failed seat/class symmetry")
    report = summarize(rows, raw, args.dev_best)
    report["raw_sha256"] = {name: hash_file(args.output / name)
                            for name in sorted(metadata["matches"].values())}
    atomic_write(summary, (json.dumps(report, indent=2) + "\n").encode())
    atomic_write(args.output / "README.md", render(metadata, report, args).encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", nargs=3, action="append", default=[], metavar=("NAME", "PATH", "SHA256"))
    parser.add_argument("--dev-best", choices=CURVE)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--batch-size", type=int, default=512)
    parser.add_argument("--jobs", type=int, default=1)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--smoke", action="store_true")
    modes.add_argument("--release", action="store_true", help="Run B2F with the frozen checkpoint hashes and DEV selection")
    args = parser.parse_args()
    if not 1 <= args.batch_size <= 65536 or args.jobs < 1:
        parser.error("batch size must be in 1..65536 and jobs must be positive")
    if args.smoke and args.policy:
        parser.error("smoke uses only synthetic policies")
    if args.smoke and args.dev_best is None:
        args.dev_best = CURVE[0]
    if args.release and args.dev_best is None:
        parser.error("freeze --dev-best before running B2F")
    run(args)


if __name__ == "__main__":
    main()
