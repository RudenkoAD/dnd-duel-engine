"""Evaluate immutable specialist-league finalists on the independent X1F fixtures."""

import argparse
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict
from functools import partial
import gzip
import hashlib
import json
from pathlib import Path
from shlex import join
import sys
import time

import numpy as np

from arena import native
from league_holdout import (
    ANCHORS, ROOT, REASONS, SmokePolicy, atomic_write, checkpoint, fixture_schedule, measure, play,
)

FOCALS = ("champion-1300", "ordinary", "treatment")
SPECIALISTS = ("specialist-401", "specialist-402")
HISTORICAL = tuple(ANCHORS)
CHECKPOINTS = FOCALS + SPECIALISTS
ENDPOINTS = ("ordinary-endpoint", "treatment-endpoint")
OPPONENTS = HISTORICAL + ("expert", FOCALS[0]) + SPECIALISTS
MATCHES = tuple((focal, foe) for focal in FOCALS for foe in OPPONENTS) + (
    ("treatment", "ordinary"),
)
BOOTSTRAP_SAMPLES = 8000
BOOTSTRAP_SEED = 58727
NATIVE_SHA = "621964e798659ba534fab6c8494664c8e7f9d91abb9a4e28ccb0ed9f40888e3f"
CHAMPION_1300_SHA = "96f9bddabc0eeff4a70c111db443dea064490c6e576ff8bb569966024cb26e31"


def paired_bands(rows, estimands):
    """Resample shared seeds in strata and use the maximum centered deviation."""
    _, indices = np.unique([row.seed for row in rows], return_inverse=True)
    sizes = np.bincount(indices)
    strata = np.empty((len(sizes), 3), dtype=np.int32)
    for row, index in zip(rows, indices):
        strata[index] = (row.level, row.arena, row.focal == row.opponent)

    values = np.asarray(list(estimands.values()))
    means = values.mean(axis=1)
    # Totals per seed preserve the equal weight of each ordered bout. Seed
    # clusters span either one or two rows, with a fixed size in each stratum.
    totals = np.stack([np.bincount(indices, weights=value) for value in values], axis=1)
    _, groups = np.unique(strata, axis=0, return_inverse=True)
    rng = np.random.default_rng(BOOTSTRAP_SEED)
    replicate = np.zeros((BOOTSTRAP_SAMPLES, len(estimands)))
    for group in np.unique(groups):
        population = totals[groups == group]
        for start in range(0, BOOTSTRAP_SAMPLES, 256):
            end = min(start + 256, BOOTSTRAP_SAMPLES)
            draws = rng.integers(len(population), size=(end - start, len(population)))
            replicate[start:end] += population[draws].sum(axis=1)
    replicate /= len(rows)
    deviations = np.abs(replicate - means)
    radius = np.quantile(deviations.max(axis=1), .95)
    return {
        name: {
            "difference": float(value),
            "ci95_joint": [float(value - radius), float(value + radius)],
            "ci95_pointwise": np.quantile(replicate[:, i], [.025, .975]).tolist(),
        }
        for i, (name, value) in enumerate(zip(estimands, means))
    }


def shared_opponent_bands(rows, raw, comparisons, ensemble):
    estimands = {}
    policies = dict.fromkeys(policy for pair in comparisons for policy in pair)
    for name, bouts in (
        ("historical", ensemble),
        ("expert", {focal: raw[focal, "expert"] for focal in policies}),
    ):
        for candidate, baseline in comparisons:
            for metric, transform in (
                ("score", lambda outcomes: (outcomes + 1) / 2),
                ("raw_win", lambda outcomes: (outcomes == 1).astype(float)),
            ):
                diff = transform(bouts[candidate][..., 0]) - transform(bouts[baseline][..., 0])
                estimands[f"{candidate}:{baseline}:{name}:{metric}"] = diff.reshape(len(rows), -1).mean(axis=1)
    return paired_bands(rows, estimands)


def summarize(rows, raw):
    endpoints = ENDPOINTS if (ENDPOINTS[0], "expert") in raw else ()
    ensemble = {
        focal: np.stack([raw[focal, opponent] for opponent in HISTORICAL], axis=1)
        for focal in FOCALS + endpoints
    }
    by_class = {}
    index = {(row.focal, row.opponent, row.seed): i for i, row in enumerate(rows)}
    order = [index[row.opponent, row.focal, row.seed] for row in rows]
    focal_classes = np.array([row.focal for row in rows])
    class_masks = {native.CLASS_NAMES[cls]: focal_classes == cls for cls in np.unique(focal_classes)}
    for specialist in SPECIALISTS:
        # Swapping class roles and focal seat yields the opposite perspective of
        # the same physical duel, including the same terminal reason.
        result = raw[FOCALS[0], specialist][order, ::-1].copy()
        result[..., 0] *= -1
        by_class[specialist] = {
            "overall": measure(result[..., 0], result[..., 1]),
            "focal_class": {
                name: measure(result[mask, :, 0], result[mask, :, 1])
                for name, mask in class_masks.items()
            },
        }
    direct = {
        f"{focal}:{opponent}": measure(data[..., 0], data[..., 1])
        for (focal, opponent), data in raw.items()
    }
    gaps = {
        f"{focal}:{opponent}": paired_bands(
            rows, {"score_minus_half": data[..., 0].mean(axis=1) / 2},
        )["score_minus_half"]
        for (focal, opponent), data in raw.items()
        if (focal, opponent) in {
            ("treatment", "ordinary"), ("treatment", FOCALS[0]), ("ordinary", FOCALS[0]),
            ENDPOINTS[::-1],
        }
    }
    return {
        "matchups": direct,
        "historical": {focal: measure(data[..., 0], data[..., 1])
                       for focal, data in ensemble.items()},
        "matched_differences": shared_opponent_bands(
            rows, raw, (("treatment", "champion-1300"), ("treatment", "ordinary"),
                        ("ordinary", "champion-1300")), ensemble,
        ),
        "endpoint_differences": shared_opponent_bands(rows, raw, (ENDPOINTS[::-1],), ensemble)
        if endpoints else {},
        "head_to_head_score_minus_half": gaps,
        "specialists_against_champion": by_class,
    }


def render(metadata, report, args):
    header = "| Политика | Соперник | В/Н/П | Победы | Очки за бой |"
    separator = "|---|---|---:|---:|---:|"
    lines = [
        "# Независимая проверка лиги со специалистами", "",
        f"Домен `{metadata['domain']}`. На одну пару политик — "
        f"{2 * len(metadata['fixtures']):,}".replace(",", " ") + " "
        "боёв. У политик совпадают сиды, классы, уровни, карты и рассадки. "
        "Баллы за победу, ничью и поражение равны 1, ½ и 0 соответственно.", "",
        "Группа historical поровну учитывает три замороженных исторических чекпойнта. "
        "Имя champion в этой группе обозначает d819 из прежнего теста, а champion-1300 — "
        "стартовую политику нового эксперимента.", "",
    ]
    if not args.smoke:
        lines.extend(["## Итоги по очкам", ""])
        for candidate, baseline in (("treatment", "champion-1300"),
                                    ("treatment", "ordinary"),
                                    ("ordinary", "champion-1300")):
            outcomes = []
            for foe in ("historical", "expert"):
                value = report["matched_differences"][f"{candidate}:{baseline}:{foe}:score"]
                low, high = value["ci95_joint"]
                direction = "выше" if low > 0 else "ниже" if high < 0 else "интервал включает ноль"
                outcomes.append(f"{foe} — {direction}, {format_interval(value, 'ci95_joint')} п. п.")
            lines.append(f"- {candidate} против {baseline}. " + " ".join(outcomes))
        lines.extend(["", "Квадратные скобки показывают совместные 95% интервалы.", ""])
    lines.extend([header, separator])
    for focal, result in report["historical"].items():
        lines.append(format_match(focal, "historical", result))
    for key, result in report["matchups"].items():
        lines.append(format_match(*key.split(":"), result))
    lines.extend([
        "", "## Парные разности на общих соперниках", "",
        "Интервалы совместно покрывают двенадцать сравнений при номинальном уровне 95%. "
        "Стратифицированный бутстрэп сохраняет вместе оба порядка классов, обе рассадки "
        "и результаты всех политик и соперников на одном сиде. Страты заданы уровнем, "
        "ареной и совпадением классов.", "",
        "| Политика | База сравнения | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] |",
        "|---|---|---|---|---:|",
    ])
    for key, value in report["matched_differences"].items():
        candidate, baseline, opponent, metric = key.split(":")
        lines.append(f"| {candidate} | {baseline} | {opponent} | {metric} | {format_interval(value, 'ci95_joint')} |")
    if report["endpoint_differences"]:
        lines.extend([
            "", "## Вторичная проверка при общем лимите CPU", "",
            "Совместный интервал покрывает четыре разности в этой таблице.", "",
            "| База сравнения | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] |",
            "|---|---|---|---:|",
        ])
        for key, value in report["endpoint_differences"].items():
            _, baseline, opponent, metric = key.split(":")
            lines.append(f"| {baseline} | {opponent} | {metric} | {format_interval(value, 'ci95_joint')} |")
    lines.extend(["", "## Очные матчи финалистов", "",
                  "Отклонение очков за бой от 50%, с отдельным парным 95% интервалом.", ""])
    for key, value in report["head_to_head_score_minus_half"].items():
        focal, foe = key.split(":")
        lines.append(f"- {focal} против {foe} — {format_interval(value, 'ci95_pointwise')} п. п.")
    lines.extend(["", "## Специалисты против champion-1300", "", header, separator])
    for specialist, values in report["specialists_against_champion"].items():
        lines.append(format_match(specialist, "champion-1300", values["overall"]))
    for specialist, values in report["specialists_against_champion"].items():
        lines.extend(["", f"### {specialist} по классу специалиста", "", header, separator])
        for cls, value in values["focal_class"].items():
            lines.append(format_match(cls, "champion-1300", value))
    lines.extend(["", "Манифест хранит хэши и расписание, а сжатые файлы содержат результаты отдельных боёв.", ""])
    if not args.smoke:
        args_out = args.output.with_name(args.output.name + "-reproduced")
        commands = [
            ["OPENBLAS_NUM_THREADS=1", "VECLIB_MAXIMUM_THREADS=1", "PYTHONPATH=" + str(native.ROOT / "python"),
             ".venv/bin/python", "evaluation/specialist_holdout.py"],
            *[["--policy", *entry] for entry in args.policy],
            ["--expected-native-sha", metadata["native_library_sha256"]],
            ["--batch-size", str(args.batch_size), "--jobs", str(args.jobs), "--output", str(args_out)],
        ]
        continuation = " \\" + "\n  "
        lines.extend(["## Воспроизведение", "",
                      "Команда ниже использует зафиксированную локальную сборку. Для проверки "
                      "на другой машине соберите совместимую библиотеку, замените `PYTHONPATH` "
                      "и передайте SHA-256 своей сборки в `--expected-native-sha`.", "",
                      "```bash", continuation.join(map(join, commands)), "```", ""])
    return "\n".join(lines)


def format_match(focal, opponent, result):
    record = "/".join(str(result[key]) for key in ("wins", "draws", "losses"))
    return f"| {focal} | {opponent} | {record} | {result['raw_win']:.2%} | {result['score']:.2%} |"


def format_interval(value, interval_key):
    low, high = value[interval_key]
    return f"{value['difference'] * 100:+.2f} [{low * 100:+.2f}, {high * 100:+.2f}]"


def load_policies(args):
    if args.smoke:
        return {key: SmokePolicy(i) for i, key in enumerate(HISTORICAL + CHECKPOINTS)}, {}
    entries = {name: (path, sha) for name, path, sha in args.policy}
    required = set(CHECKPOINTS)
    if set(entries) not in (required, required | set(ENDPOINTS)) or len(args.policy) != len(entries):
        raise ValueError(f"supply --policy once for each of {CHECKPOINTS}, with optional pair {ENDPOINTS}")
    if entries[FOCALS[0]][1] != CHAMPION_1300_SHA:
        raise ValueError("champion-1300 must be the frozen winner of the previous league")
    configs = {
        **{key: (ROOT / path, sha) for key, (path, sha) in ANCHORS.items()},
        **{key: (Path(path), sha) for key, (path, sha) in entries.items()},
    }
    policies = {
        key: checkpoint(path, sha, "0260a49" if key == "champion" else None)
        for key, (path, sha) in configs.items()
    }
    return policies, {key: sha for key, (_, sha) in configs.items()}


def evaluate_match(match, *, args, rows, policies, domain):
    focal, opponent = match
    path = args.output / f"{focal}-vs-{opponent}.json.gz"
    if path.exists():
        raw = np.array(json.loads(gzip.decompress(path.read_bytes())), dtype=np.int8)
    else:
        started = time.monotonic()
        raw = play(rows, policies[focal], policies.get(opponent, opponent), args.batch_size, domain)
        atomic_write(path, gzip.compress(json.dumps(raw.tolist(), separators=(",", ":")).encode(), mtime=0))
        print(f"{focal} vs {opponent}: {time.monotonic() - started:.1f}s", flush=True)
    if raw.shape != (len(rows), 2, 2) or not (
        np.isin(raw[..., 0], [-1, 0, 1]).all() and np.isin(raw[..., 1], range(1, len(REASONS))).all()
    ):
        raise ValueError(f"invalid raw outcomes in {path}")
    return match, raw


def run(args):
    domain = "X1X" if args.smoke else "X1F"
    classes = (0, 2, 11) if args.smoke else range(len(native.CLASS_NAMES))
    levels = (5,) if args.smoke else range(1, 6)
    rows = fixture_schedule(classes, levels, domain)
    if not args.smoke and (len(rows) != 2880 or len({row.seed for row in rows}) != 1560):
        raise RuntimeError("unexpected holdout schedule size")
    policies, hashes = load_policies(args)
    suffix = {"darwin": "dylib", "linux": "so"}[sys.platform]
    library = native.ROOT / "target" / "release" / f"libdnd_duel_engine.{suffix}"
    library_hash = hashlib.sha256(library.read_bytes()).hexdigest()
    if not args.smoke and library_hash != args.expected_native_sha:
        raise ValueError("native library SHA-256 differs from the frozen release")
    with native.Batch(1):
        pass
    matches = MATCHES
    if ENDPOINTS[0] in policies:
        matches += tuple((focal, foe) for focal in ENDPOINTS for foe in HISTORICAL + ("expert",))
        matches += (ENDPOINTS[::-1],)
    # Endpoint and selected checkpoint can be byte-identical.
    unique, aliases = {}, {}
    for focal, foe in matches:
        identity = (hashes.get(focal, focal), hashes.get(foe, foe))
        aliases[focal, foe] = unique.setdefault(identity, (focal, foe))
    metadata = {
        "domain": domain,
        "arena_abi": native.ARENA_ABI,
        "native_library_sha256": library_hash,
        "python_version": sys.version.split()[0],
        "numpy_version": np.__version__,
        "batch_size": args.batch_size,
        "checkpoint_sha256": hashes,
        "matches": {f"{focal}:{foe}": f"{source[0]}-vs-{source[1]}.json.gz"
                    for (focal, foe), source in aliases.items()},
        "historical_anchors": HISTORICAL,
        "raw_layout": ["ordered fixture", "focal seat", "field (outcome -1/0/1, terminal reason code)"],
        "terminal_reasons": REASONS,
        "bootstrap_samples": BOOTSTRAP_SAMPLES,
        "bootstrap_seed": BOOTSTRAP_SEED,
        "fixtures": [asdict(row) for row in rows],
    }
    args.output.mkdir(parents=True, exist_ok=True)
    manifest = args.output / "manifest.json"
    canonical = json.dumps(metadata, indent=2) + "\n"
    if manifest.exists() and manifest.read_text() != canonical:
        raise ValueError("output belongs to a different frozen protocol")
    if not manifest.exists() and any(args.output.iterdir()):
        raise ValueError("output must be empty or contain the matching manifest")
    atomic_write(manifest, canonical.encode())
    match = partial(evaluate_match, args=args, rows=rows, policies=policies, domain=domain)
    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        unique_raw = dict(executor.map(match, unique.values()))
    raw = {key: unique_raw[source] for key, source in aliases.items()}
    if hashlib.sha256(library.read_bytes()).hexdigest() != library_hash:
        raise RuntimeError("native library changed during evaluation")
    if raw[FOCALS[0], FOCALS[0]][..., 0].sum() != 0:
        raise RuntimeError("champion-1300 self-match failed seat/class symmetry")
    report = summarize(rows, raw)
    atomic_write(args.output / "summary.json", (json.dumps(report, indent=2) + "\n").encode())
    atomic_write(args.output / "README.md", render(metadata, report, args).encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", nargs=3, action="append", default=[], metavar=("NAME", "PATH", "SHA256"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--batch-size", type=int, default=512)
    parser.add_argument("--jobs", type=int, default=1)
    parser.add_argument("--expected-native-sha", default=NATIVE_SHA,
                        help="SHA-256 of the chosen ABI-compatible release library")
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.batch_size <= 65536 or args.jobs < 1:
        parser.error("batch size must be in 1..65536 and jobs must be positive")
    if args.smoke and args.policy:
        parser.error("smoke runs use synthetic policies")
    run(args)


if __name__ == "__main__":
    main()
