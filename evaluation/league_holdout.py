"""Evaluate a frozen league checkpoint and its initial policy on fresh paired bouts."""

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, replace
import gzip
import hashlib
from io import BytesIO
import json
from pathlib import Path
from shlex import join
import subprocess
import sys
import tempfile
import time

import numpy as np

from arena import native
from arena.policy import Policy
from paired_stats import cluster_interval, record
from schedule import BoutPair, arena_for_seed


ROOT = Path(__file__).resolve().parents[1]
ANCHORS = {
    "champion": ("models/arena_policy.npz",
                 "d81965d6d1554a3df34c6265551e637b243224988b7c7efe7bbec679c719353e"),
    "abi2002-control": ("training/runs/viewer_relative/control.npz",
                        "99b99391f0d80b090ae74d93ce25d7011956da836bdd2f5ad64293b1ff5f3f00"),
    "relative-update-90": ("training/runs/viewer_relative/target-aware-update-0090.npz",
                           "c9be34b39f4793e84bd051a1e6d66f697a6880051ed86f543f5d6df4adb6b480"),
}
REASONS = ("ongoing", "defeat", "simultaneous_ko", "turn_cap", "decision_cap")


def seed_in_domain(domain, *keys):
    prefix = int.from_bytes(domain.encode("ascii"), "big")
    payload = ",".join(map(str, keys)).encode("ascii")
    suffix = int.from_bytes(hashlib.blake2b(payload, digest_size=5).digest(), "big")
    return (prefix << 40) | suffix


def fixture_schedule(classes, levels, domain):
    rows = []
    seeds = set()
    for left in classes:
        for right in classes:
            if right < left:
                continue
            for level in levels:
                for arena in range(4):
                    nonce = 0
                    seed = seed_in_domain(domain, left, right, level, arena, nonce)
                    while arena_for_seed(seed) != arena:
                        nonce += 1
                        seed = seed_in_domain(domain, left, right, level, arena, nonce)
                    if seed in seeds:
                        raise RuntimeError("duplicate holdout seed")
                    seeds.add(seed)
                    row = BoutPair(left, right, level, arena, seed)
                    rows.append(row)
                    if left != right:
                        rows.append(replace(row, focal=right, opponent=left))
    return rows


def policy_from_bytes(data):
    return Policy.load(BytesIO(data)).require_arena(native.ARENA_ABI, native.FEATURES, native.ACTIONS)


def checkpoint(path, expected_sha, revision=None):
    payload = path.read_bytes() if path.exists() else b""
    if revision and hashlib.sha256(payload).hexdigest() != expected_sha:
        relative = path.relative_to(ROOT)
        payload = subprocess.check_output(["git", "show", f"{revision}:{relative}"], cwd=ROOT)
    if hashlib.sha256(payload).hexdigest() != expected_sha:
        raise ValueError(f"SHA-256 mismatch for {path}")
    return policy_from_bytes(payload)


def measure(outcomes, reasons):
    counts = Counter(reasons.ravel())
    return {
        **record(outcomes),
        "points": float(((outcomes + 1) / 2).sum()),
        "raw_win": float((outcomes == 1).mean()),
        "reasons": {name: counts[i] for i, name in enumerate(REASONS) if i},
        "seats": [record(outcomes[..., seat]) for seat in (0, 1)],
    }


def paired_gain(rows, new, old):
    # Seats, class directions and repeated opponent evaluations share their seed.
    # Off-diagonal class pairs have twice the weight of same-class matches.
    fixture = [row.seed for row in rows]
    _, indices = np.unique(fixture, return_inverse=True)
    sizes = np.bincount(indices)
    differences = (new - old).reshape(-1, len(rows), 2).mean(axis=(0, 2)) / 2
    cluster_means = np.bincount(indices, weights=differences) / sizes
    strata = np.empty((len(sizes), 3), dtype=np.int32)
    for row, index in zip(rows, indices):
        strata[index] = (row.level, row.arena, row.focal == row.opponent)
    # Bootstrap within each fixed cluster-size stratum, then recombine with the
    # original ordered-bout weights.
    weighted = cluster_means * sizes / sizes.mean()
    return {"gain": float(differences.mean()),
            "ci95": cluster_interval(weighted, strata=strata)}


def synthesize(rows, matchups):
    combined = dict(matchups)
    for name, components in (
        ("ensemble", tuple(ANCHORS)),
        ("composite", tuple(ANCHORS) + ("expert",) * len(ANCHORS)),
    ):
        # Repeated expert observations encode the fixed one-half weight.
        combined[name] = np.stack([matchups[component] for component in components], axis=1)
    result = {}
    for name, data in combined.items():
        is_composite = name == "composite"
        new, old = data
        comparison = paired_gain(rows, new[..., 0], old[..., 0])
        if is_composite:
            result[name] = {"new_score": float((new[..., 0].mean() + 1) / 2),
                            "initial_score": float((old[..., 0].mean() + 1) / 2),
                            **comparison}
        else:
            result[name] = {"new": measure(new[..., 0], new[..., 1]),
                            "initial": measure(old[..., 0], old[..., 1]),
                            **comparison}
    return result


def render(metadata, results, args):
    lines = [
        "# Финальная парная проверка лиги", "",
        f"Домен **{metadata['domain']}**, {2 * len(metadata['fixtures']):,} боёв на каждую "
        "пару политик. Расписание покрывает упорядоченные пары классов, одинаковые уровни "
        "и все четыре арены. Для каждой пары бойцов сыграны обе рассадки. "
        "Один сид используется также для обратного порядка классов.", "",
        "Очки за бой равны 1 за победу, ½ за ничью и 0 за поражение. "
        "Итоговый балл поровну учитывает средние очки против трёх нейросетевых соперников "
        "и очки против встроенного усиленного бота. Оба чекпойнта действуют жадно "
        "по разрешённым действиям. Случайный соперник равномерно выбирает разрешённый индекс. "
        "Строки league-update отражают дополнительные бои с ранними чекпойнтами "
        "этого запуска и имеют нулевой вес в итоговом балле.", "",
        "| Соперник | Новая В/Н/П | Очки | Лимит ходов / решений | Исходная В/Н/П | Очки | Лимит ходов / решений | Δ, п. п. [95% ДИ] |",
        "|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for name, row in results.items():
        low, high = row["ci95"]
        gain = f"{row['gain'] * 100:+.2f} [{low * 100:+.2f}, {high * 100:+.2f}]"
        if name == "composite":
            cells = ["—", f"{row['new_score']:.2%}", "—",
                     "—", f"{row['initial_score']:.2%}", "—"]
        else:
            cells = []
            for policy in ("new", "initial"):
                value = row[policy]
                reasons = value["reasons"]
                cells.extend((
                    f"{value['wins']}/{value['draws']}/{value['losses']}",
                    f"{value['score']:.2%}",
                    f"{reasons['turn_cap']}/{reasons['decision_cap']}",
                ))
        lines.append(f"| {name} | {' | '.join(cells)} | {gain} |")
    lines.extend([
        "", "Для 95%-х интервалов бутстрэп сохраняет вместе обе рассадки, оба порядка "
        "классов и результаты против всех соперников на одном сиде. Страты заданы "
        "уровнем, ареной и совпадением классов. Это описательные интервалы.",
        "", "Хэши чекпойнтов и полное расписание находятся в `manifest.json`. "
        "В сжатых файлах перечислены исходы и причины завершения для каждой рассадки.", "",
    ])
    if not args.smoke:
        output = args.output.with_name(args.output.name + "-reproduced")
        command = [
            ["PYTHONPATH=python", ".venv/bin/python", "evaluation/league_holdout.py"],
            ["--checkpoint", str(args.checkpoint)],
            ["--sha256", args.sha256],
            *[["--league-anchor", *anchor] for anchor in args.league_anchor],
            ["--batch-size", str(args.batch_size), "--jobs", str(args.jobs)],
            ["--random", "--output", str(output)] if args.random else ["--output", str(output)],
        ]
        continuation = " \\" + "\n  "
        lines.extend([
            "## Воспроизведение", "",
            "Запустите из корня репозитория после сборки release-библиотеки (`make release`). "
            "Для повторного расчёта используйте пустой каталог вывода.", "",
            "```bash", continuation.join(map(join, command)), "```", "",
        ])
    return "\n".join(lines)


def play(rows, focal, opponent, batch_size, domain):
    outcomes = np.empty((len(rows) * 2, 2), dtype=np.int8)
    trials = [(seat, row) for row in rows for seat in (0, 1)]
    for start in range(0, len(trials), batch_size):
        chunk = trials[start:start + batch_size]
        lanes = np.arange(len(chunk), dtype=np.int32)
        seats = np.array([seat for seat, row in chunk], dtype=np.int32)
        seeds = np.array([row.seed for seat, row in chunk], dtype=np.uint64)
        classes = np.array([(row.focal, row.opponent)[::1 - 2 * seat] for seat, row in chunk],
                           dtype=np.int32)
        levels = np.array([(row.level, row.level) for seat, row in chunk], dtype=np.int32)
        actions = np.empty(len(chunk), dtype=np.int32)
        generators = [
            np.random.default_rng(seed_in_domain(domain, row.seed, 1 - seat, 773))
            for seat, row in chunk
        ] if opponent == "random" else []
        with native.Batch(len(chunk)) as arena:
            arena.reset(lanes, seeds, classes, levels)
            for _ in range(801):
                if opponent == "expert":
                    arena.advance(seats, expert=True)
                features, masks, actors, winners = arena.observe()
                if (actors < 0).all():
                    break
                actions.fill(-1)
                focal_lanes = lanes[actors >= 0] if focal is opponent else lanes[actors == seats]
                if len(focal_lanes):
                    actions[focal_lanes] = focal.choose(features[focal_lanes], masks[focal_lanes])
                if focal is not opponent:
                    opposing_lanes = lanes[(actors >= 0) & (actors != seats)]
                    if len(opposing_lanes):
                        if opponent == "random":
                            actions[opposing_lanes] = [
                                generators[lane].choice(np.flatnonzero(masks[lane]))
                                for lane in opposing_lanes
                            ]
                        elif opponent == "expert":
                            raise RuntimeError("advance left an expert decision unresolved")
                        else:
                            actions[opposing_lanes] = opponent.choose(
                                features[opposing_lanes], masks[opposing_lanes])
                arena.step(actions)
            else:
                raise RuntimeError("native decision cap did not settle all matches")
            reasons = arena.result_reasons()
        if (reasons < 1).any() or (reasons >= len(REASONS)).any():
            raise RuntimeError("invalid terminal reason")
        if not np.isin(winners, [0, 1, 2]).all():
            raise RuntimeError("invalid terminal winner")
        outcomes[start:start + len(chunk)] = np.column_stack((
            (winners == seats).astype(np.int8) - (winners == 1 - seats), reasons))
    return outcomes.reshape(len(rows), 2, 2)


def atomic_write(path, data):
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as pending:
        temporary = Path(pending.name)
        pending.write(data)
    try:
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


class SmokePolicy:
    def __init__(self, seed):
        self.ranks = np.random.default_rng(seed).permutation(native.ACTIONS)

    def choose(self, features, masks):
        return np.where(masks, self.ranks, -1).argmax(axis=1).astype(np.int32)


def run(args):
    if args.smoke:
        domain = "L1X"
        rows = fixture_schedule((2, 9, 11), (5,), domain)
        policies = {name: SmokePolicy(i) for i, name in enumerate(ANCHORS)}
        candidate = policies["relative-update-90"]
    else:
        domain = "L1F"
        candidate = checkpoint(args.checkpoint, args.sha256)
        policies = {
            name: checkpoint(ROOT / path, expected, "0260a49" if name == "champion" else None)
            for name, (path, expected) in ANCHORS.items()
        }
        rows = fixture_schedule(tuple(range(len(native.CLASS_NAMES))), tuple(range(1, 6)), domain)
        if len(rows) != 2880 or len({row.seed for row in rows}) != 1560:
            raise RuntimeError("unexpected holdout schedule size")
    initial = policies["champion"]
    opponents = {**policies, "expert": "expert"}
    if args.random:
        opponents["random"] = "random"
    league_sha = {}
    for update, expected, path in args.league_anchor:
        name = f"league-update-{int(update):04d}"
        if name in league_sha:
            raise ValueError(f"duplicate secondary opponent {name}")
        opponents[name] = checkpoint(Path(path), expected)
        league_sha[name] = expected
    suffix = {"darwin": "dylib", "linux": "so"}[sys.platform]
    library = ROOT / "target" / "release" / f"libdnd_duel_engine.{suffix}"
    metadata = {
        "domain": domain,
        "arena_abi": native.ARENA_ABI,
        "native_library_sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
        "candidate_sha256": args.sha256,
        "initial_sha256": ANCHORS["champion"][1] if not args.smoke else None,
        "anchor_sha256": {key: value[1] for key, value in ANCHORS.items()} if not args.smoke else {},
        "league_anchor_sha256": league_sha,
        "opponents": list(opponents),
        "raw_layout": ["policy (new, initial)", "ordered fixture", "focal seat",
                       "field (outcome -1/0/1, terminal reason code)"],
        "terminal_reasons": REASONS,
        "fixtures": [asdict(row) for row in rows],
    }
    args.output.mkdir(parents=True, exist_ok=True)
    manifest = args.output / "manifest.json"
    canonical = json.dumps(metadata, indent=2) + "\n"
    if manifest.exists() and manifest.read_text() != canonical:
        raise ValueError("output belongs to a different frozen protocol")
    atomic_write(manifest, canonical.encode())
    def match(name, opponent):
        path = args.output / f"{name}.json.gz"
        if path.exists():
            raw = np.array(json.loads(gzip.decompress(path.read_bytes())), dtype=np.int8)
        else:
            started = time.monotonic()
            raw = np.stack(tuple(play(rows, focal, opponent, args.batch_size, domain)
                                 for focal in (candidate, initial)))
            print(f"{name}: {time.monotonic() - started:.1f}s", flush=True)
            data = json.dumps(raw.tolist(), separators=(",", ":")).encode()
            atomic_write(path, gzip.compress(data, mtime=0))
        if raw.shape != (2, len(rows), 2, 2):
            raise ValueError(f"unexpected outcome shape for {name}: {raw.shape}")
        return name, raw

    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        results = dict(executor.map(match, opponents, opponents.values()))
    if results["champion"][1, ..., 0].sum() != 0:
        raise RuntimeError("initial champion self-match failed seat/class symmetry")
    report = synthesize(rows, results)
    atomic_write(args.output / "summary.json", (json.dumps(report, indent=2) + "\n").encode())
    atomic_write(args.output / "README.md", render(metadata, report, args).encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--sha256")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--batch-size", type=int, default=512)
    parser.add_argument("--jobs", type=int, default=1)
    parser.add_argument("--league-anchor", nargs=3, action="append", default=[],
                        metavar=("UPDATE", "SHA256", "PATH"))
    parser.add_argument("--random", action="store_true")
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    if args.batch_size < 1 or args.batch_size > 65536:
        parser.error("batch size must be in 1..65536")
    if args.jobs < 1:
        parser.error("jobs must be positive")
    if args.smoke:
        if args.checkpoint or args.sha256 or args.league_anchor:
            parser.error("smoke runs use synthetic policies")
    elif not args.checkpoint or not args.sha256:
        parser.error("specify both the frozen checkpoint and its SHA-256")
    run(args)


if __name__ == "__main__":
    main()
