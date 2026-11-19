"""Evaluate frozen narrow and wide league policies on independent W1F bouts."""

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
    ANCHORS, REASONS, ROOT, SmokePolicy, atomic_write, checkpoint, fixture_schedule, measure, play,
)
from specialist_holdout import (
    BOOTSTRAP_SAMPLES, BOOTSTRAP_SEED, CHAMPION_1300_SHA, NATIVE_SHA, format_interval,
    paired_bands, shared_opponent_bands,
)

START = "champion-1300"
NARROW = "narrow"
WIDE = "wide"
FOCALS = (START, NARROW, WIDE)
ENDPOINTS = ("narrow-matched", "wide-matched")
OPPONENTS = (*ANCHORS, "expert")
DIRECT = ((WIDE, NARROW), (WIDE, START), (NARROW, START))
PRACTICAL = 0.005


def hash_file(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_policies(args):
    entries = {name: (Path(path), sha) for name, path, sha in args.policy}
    if args.smoke:
        names = (*ANCHORS, *FOCALS, *ENDPOINTS)
        return {name: SmokePolicy(i) for i, name in enumerate(names)}, {}
    required = set(FOCALS)
    if set(entries) not in (required, required | set(ENDPOINTS)) or len(entries) != len(args.policy):
        raise ValueError(f"supply --policy for {FOCALS}, with optional pair {ENDPOINTS}")
    if entries[START][1] != CHAMPION_1300_SHA:
        raise ValueError("champion-1300 must hash to the frozen league winner")
    configs = {name: (ROOT / path, sha) for name, (path, sha) in ANCHORS.items()}
    configs.update(entries)
    policies = {
        name: checkpoint(path, sha, "0260a49" if name == "champion" else None)
        for name, (path, sha) in configs.items()
    }
    return policies, {name: sha for name, (_, sha) in configs.items()}


def validate_raw(raw, row_count, path):
    if (raw.shape != (row_count, 2, 2)
        or raw.dtype.kind not in "iu"
        or not np.isin(raw[..., 0], (-1, 0, 1)).all()
        or not np.isin(raw[..., 1], range(1, len(REASONS))).all()):
        raise ValueError(f"invalid bout records in {path}")


def evaluate_match(match, *, args, rows, policies, domain):
    focal, foe = match
    path = args.output / f"{focal}-vs-{foe}.json.gz"
    if path.exists():
        raw = np.asarray(json.loads(gzip.decompress(path.read_bytes())))
        validate_raw(raw, len(rows), path)
        return match, raw.astype(np.int8)
    started = time.monotonic()
    raw = play(rows, policies[focal], policies.get(foe, foe), args.batch_size, domain)
    validate_raw(raw, len(rows), path)
    atomic_write(path, gzip.compress(json.dumps(raw.tolist(), separators=(",", ":")).encode(), mtime=0))
    print(f"{focal} vs {foe}: {time.monotonic() - started:.1f}s", flush=True)
    return match, raw


def plateau_result(interval):
    low, high = interval
    if low > PRACTICAL:
        return "превышает +0,5 п. п."
    if high < -PRACTICAL:
        return "ниже −0,5 п. п."
    if low >= -PRACTICAL and high <= PRACTICAL:
        return "целиком в пределах ±0,5 п. п."
    if high <= PRACTICAL:
        return "прирост свыше +0,5 п. п. исключён"
    return "порог +0,5 п. п. остаётся неопределённым"


def summarize(rows, raw, endpoints):
    historical = {
        focal: np.stack([raw[focal, foe] for foe in ANCHORS], axis=1)
        for focal in FOCALS + endpoints
    }
    primary = shared_opponent_bands(rows, raw, DIRECT, historical)
    direct = {}
    for focal, foe in DIRECT:
        outcomes = raw[focal, foe][..., 0]
        direct[f"{focal}:{foe}:score_minus_half"] = outcomes.mean(axis=1) / 2
    direct_bands = paired_bands(rows, direct)
    # The net win rate is exactly twice the score advantage over 50%.
    for key, estimate in list(direct_bands.items()):
        direct_bands[key.replace("score_minus_half", "raw_win_minus_raw_loss")] = {
            "difference": 2 * estimate["difference"],
            "ci95_joint": [2 * limit for limit in estimate["ci95_joint"]],
            "ci95_pointwise": [2 * limit for limit in estimate["ci95_pointwise"]],
        }
    return {
        "matchups": {f"{focal}:{foe}": measure(data[..., 0], data[..., 1])
                     for (focal, foe), data in raw.items()},
        "historical": {focal: measure(data[..., 0], data[..., 1])
                       for focal, data in historical.items()},
        "matched_differences": primary,
        "direct_differences": direct_bands,
        "matched_compute_differences": shared_opponent_bands(rows, raw, (ENDPOINTS[::-1],), historical)
        if endpoints else {},
    }


def contrast_table(bands):
    lines = [
        "| Политика | База | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] | Порог 0,5 п. п. |",
        "|---|---|---|---|---:|---|",
    ]
    for key, estimate in bands.items():
        focal, base, foe, metric = key.split(":")
        interval = format_interval(estimate, "ci95_joint")
        threshold = plateau_result(estimate["ci95_joint"])
        lines.append(f"| {focal} | {base} | {foe} | {metric} | {interval} | {threshold} |")
    return lines


def render(metadata, report, args):
    count = f"{2 * len(metadata['fixtures']):,}".replace(",", " ")
    lines = [
        "# Независимая проверка ширины сети", "",
        f"Домен `{metadata['domain']}`, {count} боёв на пару политик.",
        "В основном тесте 12×12 упорядоченных пар классов, уровни 1–5, четыре арены и обе рассадки.",
        "В historical поровну учитываются d819 (`champion`), исходный ABI2002 control "
        "и target-aware-0090. Expert — встроенный усиленный бот. "
        "Победа, ничья и поражение дают соответственно 1, ½ и 0 очков.", "",
    ]
    if not args.smoke:
        lines.extend(["## Широкая сеть против узкой", ""])
        for group in ("historical", "expert"):
            score = report["matched_differences"][f"{WIDE}:{NARROW}:{group}:score"]
            win = report["matched_differences"][f"{WIDE}:{NARROW}:{group}:raw_win"]
            lines.append(
                f"- {group}. Очки {format_interval(score, 'ci95_joint')} п. п. "
                f"Победы {format_interval(win, 'ci95_joint')} п. п. "
                f"Для очков {plateau_result(score['ci95_joint'])}."
            )
        lines.append("")
    lines.extend([
        "## Сравнения на общих соперниках", "",
        "Совместные 95% интервалы покрывают все 12 разностей в таблице. "
        "Оба направления пары классов, обе рассадки и все политики и соперники "
        "на одном сиде входят в один кластер. Бутстрэп стратифицирован по уровню, "
        "арене и совпадению классов.", "",
        *contrast_table(report["matched_differences"]), "",
        "## Очные встречи", "",
        "Совместные 95% интервалы образуют отдельное семейство. Доля побед сравнивается "
        "с долей поражений, а средние очки — с 50%.", "",
        "| Политика | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] |",
        "|---|---|---|---:|",
    ])
    for key, estimate in report["direct_differences"].items():
        focal, foe, metric = key.split(":")
        lines.append(f"| {focal} | {foe} | {metric} | {format_interval(estimate, 'ci95_joint')} |")
    if report["matched_compute_differences"]:
        lines.extend([
            "", "## Проверка при сопоставимой стоимости обучения", "",
            "Для четырёх разностей срезов по бюджету рассчитано отдельное семейство интервалов.", "",
            *contrast_table(report["matched_compute_differences"]),
        ])
    lines.extend([
        "", "## Исходы боёв", "",
        "| Политика | Соперник | В/Н/П | Победы | Очки | Лимит ходов / решений |",
        "|---|---|---:|---:|---:|---:|",
    ])
    records = [(focal, "historical", v) for focal, v in report["historical"].items()]
    records.extend((*key.split(":"), v) for key, v in report["matchups"].items())
    for focal, foe, value in records:
        wdl = "/".join(str(value[key]) for key in ("wins", "draws", "losses"))
        caps = "/".join(str(value["reasons"][key]) for key in ("turn_cap", "decision_cap"))
        lines.append(f"| {focal} | {foe} | {wdl} | {value['raw_win']:.2%} | {value['score']:.2%} | {caps} |")
    lines.extend(["", "`manifest.json` хранит расписание и SHA-256 входов. "
                  "Сжатые файлы содержат исходы отдельных боёв и причины окончания.", ""])
    if not args.smoke:
        args_out = args.output.with_name(args.output.name + "-reproduced")
        command = [
            ["OPENBLAS_NUM_THREADS=1", "VECLIB_MAXIMUM_THREADS=1",
             "PYTHONPATH=" + str(native.ROOT / "python"), ".venv/bin/python", "evaluation/width_holdout.py"],
            *[["--policy", *entry] for entry in args.policy],
            ["--batch-size", str(args.batch_size), "--jobs", str(args.jobs), "--output", str(args_out)],
        ]
        continuation = " " + chr(92) + "\n  "
        lines.extend(["## Воспроизведение", "", "```bash", continuation.join(map(join, command)), "```", ""])
    return "\n".join(lines)


def run(args):
    domain = "W1X" if args.smoke else "W1F"
    classes = (0, 2, 11) if args.smoke else range(len(native.CLASS_NAMES))
    levels = (5,) if args.smoke else range(1, 6)
    rows = fixture_schedule(classes, levels, domain)
    if not args.smoke and (len(rows) != 2880 or len({row.seed for row in rows}) != 1560):
        raise RuntimeError("unexpected full holdout fixture count")
    policies, hashes = load_policies(args)
    suffix = {"darwin": "dylib", "linux": "so"}[sys.platform]
    library = native.ROOT / "target" / "release" / f"libdnd_duel_engine.{suffix}"
    library_hash = hash_file(library)
    if not args.smoke and library_hash != NATIVE_SHA:
        raise ValueError("native library differs from the frozen release")
    with native.Batch(1):
        pass
    endpoints = ENDPOINTS if ENDPOINTS[0] in policies else ()
    matches = tuple((focal, foe) for focal in FOCALS + endpoints for foe in OPPONENTS)
    matches += DIRECT + ((START, START),)
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
        "historical_anchors": tuple(ANCHORS),
        "raw_layout": ["ordered fixture", "focal seat", "field (outcome -1/0/1, terminal reason code)"],
        "terminal_reasons": REASONS,
        "bootstrap_samples": BOOTSTRAP_SAMPLES,
        "bootstrap_seed": BOOTSTRAP_SEED,
        "practical_difference": PRACTICAL,
        "fixtures": [asdict(row) for row in rows],
    }
    args.output.mkdir(parents=True, exist_ok=True)
    manifest = args.output / "manifest.json"
    canonical = json.dumps(metadata, indent=2) + "\n"
    if manifest.exists():
        if manifest.read_text() != canonical:
            raise ValueError("output belongs to a different holdout manifest")
    elif any(args.output.iterdir()):
        raise ValueError("output must be empty or contain the matching manifest")
    else:
        atomic_write(manifest, canonical.encode())
    summary = args.output / "summary.json"
    if summary.exists():
        previous = json.loads(summary.read_bytes())["raw_sha256"]
        if set(previous) != set(metadata["matches"].values()) or any(
            hash_file(args.output / name) != digest for name, digest in previous.items()
        ):
            raise ValueError("cached bout results differ from the completed report")
    match = partial(evaluate_match, args=args, rows=rows, policies=policies, domain=domain)
    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        unique_raw = dict(executor.map(match, unique.values()))
    raw = {key: unique_raw[source] for key, source in aliases.items()}
    if hash_file(library) != library_hash:
        raise RuntimeError("native library changed during evaluation")
    swapped = {(row.opponent, row.focal, row.seed): i for i, row in enumerate(rows)}
    reverse = [swapped[row.focal, row.opponent, row.seed] for row in rows]
    mirror = raw[START, START][reverse, ::-1]
    self_play = raw[START, START]
    if not (np.array_equal(self_play[..., 0], -mirror[..., 0])
            and np.array_equal(self_play[..., 1], mirror[..., 1])):
        raise RuntimeError("champion-1300 self-match failed seat/class symmetry")
    report = summarize(rows, raw, endpoints)
    report["raw_sha256"] = {name: hash_file(args.output / name)
                            for name in sorted(set(metadata["matches"].values()))}
    atomic_write(summary, (json.dumps(report, indent=2) + "\n").encode())
    atomic_write(args.output / "README.md", render(metadata, report, args).encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", nargs=3, action="append", default=[], metavar=("NAME", "PATH", "SHA256"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--batch-size", type=int, default=512)
    parser.add_argument("--jobs", type=int, default=1)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.batch_size <= 65536 or args.jobs < 1:
        parser.error("batch size must be in 1..65536 and jobs must be positive")
    if args.smoke and args.policy:
        parser.error("smoke uses only synthetic policies")
    run(args)


if __name__ == "__main__":
    main()
