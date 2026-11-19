"""Compare the frozen ABI 2002 policy on paired seats, including ABI 2001 controls."""
import argparse
from collections import Counter, defaultdict
import ctypes as ct
from dataclasses import asdict, replace
import gzip
import hashlib
from io import BytesIO
import json
from pathlib import Path
import sys
import tempfile

import numpy as np

from arena.native import ACTIONS, CLASS_NAMES, FEATURES, Batch
from arena.policy import Policy, WEIGHT_KEYS
from schedule import held_out_schedule, seed_in_domain


MATCHUPS = (
    ("candidate", "random", 2002), ("candidate", "standard", 2002),
    ("candidate", "expert", 2002), ("candidate", "adapted", 2002),
    ("adapted", "random", 2002), ("adapted", "standard", 2002),
    ("adapted", "expert", 2002), ("historical", "random", 2001),
    ("historical", "standard", 2001), ("historical", "expert", 2001),
)
PROJECTILE_PHASE = 616 + 80 - 74
MAIN_PHASE = 256
PROJECTILE_KIND = {
    0.125: "Shocking Grasp", 0.25: "other", 0.375: "Poison Spray",
    0.5: "Eldritch Blast", 0.625: "True Strike", 0.75: "Chromatic Orb",
    0.875: "True Strike (ranged)", 0.9375: "True Strike (thrown)", 1.0: "Magic Missile",
}
TACTICAL_FIELDS = (
    "main", "aim", "foe_legal", "both_legal", "self_when_foe", "foe_selected",
    "aim_bouts", "foe_legal_bouts", "self_bouts",
)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class EvaluationBatch(Batch):
    def __init__(self, count, lib):
        self.n = count
        self._lib = lib
        self._handle = lib.arena_new(count)

    def random_choices(self, ranks, actions):
        self._lib.arena_evaluation_random(
            self._open(), ranks.ctypes.data, actions.ctypes.data
        )


def evaluator_library(path, abi):
    lib = ct.CDLL(str(path.resolve()))
    ptr, size, integer = ct.c_void_p, ct.c_size_t, ct.c_int32
    for name, args, result in (
        ("arena_schema", [], ct.c_uint32), ("arena_features", [], size),
        ("arena_actions", [], size), ("arena_new", [size], ptr), ("arena_free", [ptr], None),
        ("arena_reset", [ptr, size, ptr, ptr, ptr, ptr], ct.c_bool),
        ("arena_observe", [ptr, ptr, ptr, ptr, ptr], None),
        ("arena_choices", [ptr, ct.c_bool, ptr], None), ("arena_step", [ptr, ptr], integer),
        ("arena_results", [ptr, ptr], None),
        ("arena_text", [ptr, size, integer, integer, ptr, size], ct.c_bool),
        ("arena_evaluation_random", [ptr, ptr, ptr], None),
    ):
        getattr(lib, name).argtypes = args
        getattr(lib, name).restype = result
    if (lib.arena_schema(), lib.arena_features(), lib.arena_actions()) != (abi, FEATURES, ACTIONS):
        raise ValueError(f"ABI or dimensions mismatch for {path}")
    return lib


def evaluate(rows, seat, matchup, policies, lib, domain):
    left, right, abi = matchup
    count = len(rows)
    lanes = np.arange(count, dtype=np.int32)
    seeds = np.array([row.seed for row in rows], dtype=np.uint64)
    classes = np.array([(row.focal, row.opponent)[::1 - 2 * seat] for row in rows], dtype=np.int32)
    levels = np.array([(row.level, row.level) for row in rows], dtype=np.int32)
    random_actor = seat if left == "random" else 1 - seat
    random_rng = [
        np.random.default_rng(seed_in_domain(domain, row.seed, random_actor, 773))
        for row in rows
    ] if "random" in matchup else []
    actions = np.empty(count, dtype=np.int32)
    ranks = np.empty(count, dtype=np.int32)
    stats = np.zeros((count, len(TACTICAL_FIELDS)), dtype=np.int32)
    usage = Counter()
    kind_checks = set()
    self_id, foe_id = (0, 1) if abi == 2002 else (seat, 1 - seat)
    with EvaluationBatch(count, lib) as arena:
        arena.reset(lanes, seeds, classes, levels)
        for _ in range(801):
            x, masks, actors, winners = arena.observe()
            if (actors < 0).all():
                break
            actions.fill(-1)
            for actor, category in ((seat, left), (1 - seat, right)):
                acted = actors == actor
                active = lanes[acted]
                if not len(active):
                    continue
                if category in policies:
                    actions[active] = policies[category].choose(x[active], masks[active])
                elif category in {"standard", "expert"}:
                    actions[active] = arena.bot(expert=category == "expert")[active]
                else:
                    ranks.fill(-1)
                    sizes = masks[active].sum(axis=1)
                    ranks[active] = [random_rng[lane].integers(size)
                                     for lane, size in zip(active, sizes)]
                    arena.random_choices(ranks, actions)
            focal = actors == seat
            stats[focal & (x[:, MAIN_PHASE] != 0), 0] += 1
            aim = focal & (x[:, PROJECTILE_PHASE] != 0)
            focal_lanes = lanes[aim]
            legal_foe = masks[:, (foe_id, 16 + foe_id)].any(axis=1)
            legal_self = masks[:, (self_id, 16 + self_id)].any(axis=1)
            self_pick = np.isin(actions, (self_id, 16 + self_id))
            foe_pick = np.isin(actions, (foe_id, 16 + foe_id))
            stats[aim, 1] += 1
            stats[aim & legal_foe, 2] += 1
            stats[aim & legal_foe & legal_self, 3] += 1
            stats[aim & legal_foe & self_pick, 4] += 1
            stats[aim & legal_foe & foe_pick, 5] += 1
            for code in np.unique(x[focal_lanes, 545]):
                kind = PROJECTILE_KIND[float(code)]
                these = aim & (x[:, 545] == code)
                usage[kind] += int(these.sum())
                if kind not in kind_checks:
                    direct = lanes[these & masks[:, foe_id]]
                    if len(direct):
                        label = arena._text(int(direct[0]), seat, foe_id).lower()
                        if "opposing adventurer" not in label:
                            raise RuntimeError(f"phase-80 fixture mismatch: {abi=} {kind=} {label=}")
                        kind_checks.add(kind)
            arena.step(actions)
        else:
            _, _, actors, winners = arena.observe()
            if (actors >= 0).any():
                raise RuntimeError("native decision cap did not settle all matches")
        reasons = arena.result_reasons()
    stats[:, 6:] = stats[:, (1, 2, 4)] > 0
    return [{
        "seat": seat, "result": "win" if w == seat else "loss" if w == 1 - seat else "draw",
        "reason": int(reason), "tactical": stat.tolist(),
    } for w, reason, stat in zip(winners, reasons, stats)], dict(usage)


def rate(numerator, denominator):
    return "—" if denominator == 0 else f"{100 * numerator / denominator:.2f}%"


def score(bouts):
    return np.array([{"win": 1, "draw": 0.5, "loss": 0}[b["result"]] for b in bouts])


def wdl_counts(records):
    counts = Counter(record["result"] for record in records)
    return f"{counts['win']}/{counts['draw']}/{counts['loss']}"


def render(output, rows, metadata):
    sections = [
        "# Независимая проверка политики ABI 2002", "",
        f"Домен сидов **{metadata['domain']}**. На каждую строку приходится {2 * len(rows)} боёв. "
        "Перебраны упорядоченные пары классов, одинаковый уровень обоих бойцов и все четыре арены. "
        "Каждая пара использует один боевой сид в двух рассадках.", "",
        "Кандидат и адаптированный чекпойнт выбирают действие с максимальным логитом среди разрешённых. "
        "Исторический контроль использует исходный чекпойнт и исходное наблюдение ABI 2001. "
        "Random равномерно выбирает индекс в списке допустимых физических действий. "
        "В одинаковых боях обе версии получают одинаковый поток случайных чисел для соответствующего места. "
        "Функция оценки добавлена только во временную сборку каждой версии.", "",
        "| Политика | Соперник | ABI | Победы | Ничьи | Поражения | Очки | Место 0, В/Н/П | Место 1, В/Н/П |",
        "|---|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    stored = {}
    for left, right, abi in MATCHUPS:
        key = f"{left}-{right}"
        with gzip.open(output / f"{key}.json.gz", "rt") as handle:
            data = json.load(handle)
        stored[left, right] = data
        bouts = [bout for both in data["bouts"] for bout in both]
        wdl = Counter(bout["result"] for bout in bouts)
        sections.append(f"| {left} | {right} | {abi} | {wdl['win']} | {wdl['draw']} | {wdl['loss']} "
                        f"| {rate(wdl['win'] + wdl['draw'] / 2, len(bouts))} | "
                        f"{wdl_counts(pair[0] for pair in data['bouts'])} | "
                        f"{wdl_counts(pair[1] for pair in data['bouts'])} |")
    random_candidate = [bout for pair in stored["candidate", "random"]["bouts"] for bout in pair]
    random_historical = [bout for pair in stored["historical", "random"]["bouts"] for bout in pair]
    direct = [bout for pair in stored["candidate", "adapted"]["bouts"] for bout in pair]
    all_candidate = [bout for key, data in stored.items() if key[0] == "candidate"
                     for pair in data["bouts"] for bout in pair]
    aimed = sum(bout["tactical"][1] for bout in all_candidate)
    eligible = sum(bout["tactical"][2] for bout in all_candidate)
    self_picks = sum(bout["tactical"][4] for bout in all_candidate)
    sections[2:2] = [
        f"Кандидат набрал **{score(random_candidate).mean():.2%}** очков против random "
        f"при **{score(random_historical).mean():.2%}** у исходного чекпойнта ABI 2001 "
        f"и **{score(direct).mean():.2%}** в очном матче с адаптированным чекпойнтом. "
        f"В фазе 80 он выбрал себя {self_picks} раз из {eligible} случаев, когда был доступен "
        f"противоположный дуэлянт. Всего зафиксировано {aimed} решений фазы 80.", "",
    ]
    sections.extend(["", "Очки равны доле побед плюс половина доли ничьих.", "",
                     "## Разница очков на парных сидах", "",
                     "| Соперник | Кандидат − исходная политика | Кандидат − адаптированная политика |",
                     "|---|---:|---:|"])
    for right in ("random", "standard", "expert"):
        candidate = score([bout for pair in stored["candidate", right]["bouts"] for bout in pair])
        delta = []
        for baseline in ("historical", "adapted"):
            old = score([bout for pair in stored[baseline, right]["bouts"] for bout in pair])
            diff = (candidate - old).reshape(-1, 2).mean(axis=1)
            strata = defaultdict(list)
            for i, row in enumerate(rows):
                strata[row.focal, row.level, row.arena].append(i)
            rng = np.random.default_rng(20261118)
            samples = np.zeros(2000)
            for group in strata.values():
                samples += diff[rng.choice(group, size=(len(samples), len(group)))].sum(axis=1)
            low, high = np.quantile(100 * samples / len(rows), (0.025, 0.975))
            delta.append(f"{100 * diff.mean():+.2f} п. п. [{low:+.2f}, {high:+.2f}]")
        sections.append(f"| {right} | {' | '.join(delta)} |")
    sections.extend([
        "", "95%-е бутстрэп-интервалы описательные. Пара рассадок остаётся вместе, "
        "выборка стратифицирована по классу, уровню и арене.", "",
        "## Выбор собственной цели в фазе 80", "",
        "В знаменатель входят решения, где противоположный дуэлянт доступен как цель. "
        "Обычный выбор и отправка всех оставшихся снарядов Magic Missile учтены одинаково. "
        "Столбцы с числом решений показывают, как часто политика открывала прицеливание. "
        "Доля прицеливаний нормирована на решения в основной фазе. "
        "Победы в боях с самоцелью указаны описательно.", "",
        "| Политика | Соперник | Место | Прицеливания / решения | Бои с прицеливанием | Противник доступен | Обе цели доступны | В себя | Доля в себя | Бои с самоцелью | Победы в них |",
        "|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ])
    for left, right, _ in MATCHUPS:
        bouts = stored[left, right]["bouts"]
        for seat in (0, 1):
            records = [pair[seat] for pair in bouts]
            main, aim, foe, both, own, _, aim_bouts, _, self_bouts = np.sum(
                [x["tactical"] for x in records], axis=0)
            own_wins = sum(x["tactical"][-1] and x["result"] == "win" for x in records)
            sections.append(f"| {left} | {right} | {seat} | {aim}/{main} ({rate(aim, main)}) "
                            f"| {aim_bouts} | {foe} | {both} | {own} | {rate(own, foe)} | {self_bouts} | {own_wins} |")
    sections.extend([
        "", "Решение отправить снаряд в себя считается один раз независимо от числа снарядов. "
        "Число прицеливаний зависит от посещённых состояний и может включать несколько решений одного заклинания.", "",
        "| Политика | Соперник | Место 0 — решения по заклинаниям | Место 1 — решения по заклинаниям |",
        "|---|---|---|---|",
    ])
    for left, right, _ in MATCHUPS:
        counts = [", ".join(f"{name} {n}" for name, n in sorted(usage.items())) or "—"
                  for usage in stored[left, right]["projectile_usage"]]
        sections.append(f"| {left} | {right} | {' | '.join(counts)} |")
    sections.extend(["", "Хэши, схема расписания и исходные результаты по боям находятся рядом с этим отчётом.", ""])
    (output / "README.md").write_text("\n".join(sections), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--expected-sha256", required=True)
    parser.add_argument("--abi2001", type=Path, required=True)
    parser.add_argument("--abi2002", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--matchup", choices=[f"{a}-{b}" for a, b, _ in MATCHUPS])
    args = parser.parse_args()
    candidate_bytes = args.candidate.read_bytes()
    candidate_sha = hashlib.sha256(candidate_bytes).hexdigest()
    if candidate_sha != args.expected_sha256:
        parser.error("candidate SHA-256 mismatch")
    domain = "s3-smoke" if args.smoke else "s3-final"
    if args.smoke:
        smoke_classes = (2, 9, 11)
        rows = [replace(row, focal=smoke_classes[row.focal], opponent=smoke_classes[row.opponent])
                for row in held_out_schedule(len(smoke_classes), (5,), 4, domain=domain)]
    else:
        rows = list(held_out_schedule(len(CLASS_NAMES), tuple(range(1, 6)), 4, domain=domain))
    outputs = {2001: args.abi2001, 2002: args.abi2002}
    fname = {"darwin": "libdnd_duel_engine.dylib", "linux": "libdnd_duel_engine.so"}[sys.platform]
    libs = {abi: output / "target/release" / fname for abi, output in outputs.items()}
    checkpoints = dict(candidate=args.candidate, adapted=args.abi2002 / "arena_policy.npz",
                       historical=args.abi2001 / "arena_policy.npz")
    policies = {
        key: Policy.load(BytesIO(candidate_bytes) if key == "candidate" else path).require_arena(
            2001 if key == "historical" else 2002, FEATURES, ACTIONS
        )
        for key, path in checkpoints.items()
    }
    if any(not np.array_equal(policies["historical"].weights[key], policies["adapted"].weights[key])
           for key in WEIGHT_KEYS):
        parser.error("adapted benchmark differs in weights from the historical model")
    metadata = {
        "domain": domain,
        "checkpoint_sha256": {key: candidate_sha if key == "candidate" else sha(path)
                              for key, path in checkpoints.items()},
        "native": {abi: {"revision": (directory / "revision").read_text().strip(),
                         "sha256": sha(libs[abi])} for abi, directory in outputs.items()},
        "seeds": [asdict(row) for row in rows],
    }
    args.output.mkdir(exist_ok=True, parents=True)
    manifest = args.output / "manifest.json"
    canonical = json.dumps(metadata, indent=2)
    with tempfile.NamedTemporaryFile(dir=args.output, mode="w", delete=False) as pending:
        pending.write(canonical)
        pending_path = Path(pending.name)
    try:
        manifest.hardlink_to(pending_path)
    except FileExistsError:
        if manifest.read_text() != canonical:
            parser.error("output refers to a different schedule or checkpoint")
    finally:
        pending_path.unlink()
    loaded = {}
    for left, right, abi in MATCHUPS:
        key = f"{left}-{right}"
        if args.matchup and args.matchup != key:
            continue
        if abi not in loaded:
            loaded[abi] = evaluator_library(libs[abi], abi)
        destination = args.output / f"{key}.json.gz"
        if destination.exists():
            continue
        data = [evaluate(rows, seat, (left, right, abi), policies, loaded[abi], domain) for seat in (0, 1)]
        with tempfile.NamedTemporaryFile(dir=args.output, delete=False) as pending:
            pending_path = Path(pending.name)
        try:
            with gzip.open(pending_path, "wt") as handle:
                json.dump({"fields": TACTICAL_FIELDS,
                           "bouts": list(zip(data[0][0], data[1][0])),
                           "projectile_usage": [data[0][1], data[1][1]]}, handle)
            pending_path.replace(destination)
        finally:
            pending_path.unlink(missing_ok=True)
        print(key, Counter(bout["result"] for records, _ in data for bout in records), flush=True)
    if all((args.output / f"{a}-{b}.json.gz").exists() for a, b, _ in MATCHUPS):
        render(args.output, rows, metadata)


if __name__ == "__main__":
    main()
