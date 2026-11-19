"""Run the registered held-out schedule and replay a reaction bout."""

import argparse
from collections import Counter
from dataclasses import asdict
import gzip
import hashlib
import json
from pathlib import Path

import numpy as np

from arena.native import ACTION_NAMES, CLASS_NAMES, Batch
import arena.native as native
from arena.policy import DEFAULT_CHECKPOINT, Policy

from schedule import ARENAS, held_out_schedule, seed_in_domain
from paired_stats import paired_difference, record, summary


MATCHUPS = (
    ("learned", "heuristic"),
    ("random", "heuristic"),
    ("heuristic", "heuristic"),
    ("learned", "random"),
)
SELF_PLAY_MATCHUPS = (
    *MATCHUPS,
    ("learned", "expert"),
    ("expert", "expert"),
    ("learned", "prior"),
    ("prior", "heuristic"),
)


CLASSES = tuple(name.lower() for name in CLASS_NAMES)
OPPORTUNITY = ACTION_NAMES.index("Opportunity")
REASON_LABELS = (
    "evaluator_decision_cap", "defeat", "simultaneous_ko", "arena_turn_cap", "arena_decision_cap",
    "legacy_arena_draw",
)
EXTENDED_REACTIONS = {
    "ready_release": (9, ACTION_NAMES.index("Ready")),
    "counterspell": (47, ACTION_NAMES.index("Defend")),
}
# ABI features 136–139 identify normal decisions, before any ground-target selection.
COMBAT_PHASES = slice(136, 140)
TARGET_PHASES = {5: "TargetRow", 6: "TargetColumn", 19: "TargetAltitude"}
TARGET_KINDS = {1: "burst", 2: "land_aid", 3: "moonbeam", 4: "caltrops"}
TARGET_COUNTERS = (
    "chosen_cells", "foe_only_available_on_board", "foe_only_when_available",
    "foe_only", "self_only", "both", "empty",
)

TACTICAL_CHECKS = (
    ("opportunity", (14,), (14,)),
    ("defend", (16,), (16,)),
    ("rider", (17,), (17,)),
    ("end_with_weapon_or_spell", (8,), (9, 10, *range(26, 32))),
    ("move_with_weapon_or_spell", tuple(range(8)), (9, 10, *range(26, 32))),
    ("weapon_with_spell_available", (9, 10), tuple(range(26, 32))),
    ("cast_when_available", tuple(range(26, 32)), tuple(range(26, 32))),
    *((name, (action,), (action,)) for name, (_, action) in EXTENDED_REACTIONS.items()),
)



def record_target_cells(metrics, lanes, kinds, footprints, board_foe_only):
    measured = (kinds >= 0) & (kinds < len(TARGET_KINDS))
    lanes, kinds, footprints = lanes[measured], kinds[measured], footprints[measured]
    enemy = footprints[:, 2] != 0
    self_hit = footprints[:, 3] != 0
    enemy_only = enemy & ~self_hit
    metrics[lanes, kinds] += np.column_stack((
        np.ones(len(lanes), dtype=np.int32), board_foe_only[lanes],
        board_foe_only[lanes] & enemy_only, enemy_only,
        self_hit & ~enemy, enemy & self_hit, ~enemy & ~self_hit,
    ))


def evaluate_group(rows, seat, matchup, policy, cap, trace=False, *, prior=None, prior_sample=False, domain="final"):
    count = len(rows)
    indices = np.arange(count, dtype=np.int32)
    classes = np.array([(row.focal, row.opponent) for row in rows], dtype=np.int32)
    levels = np.array([(row.level, row.level) for row in rows], dtype=np.int32)
    seeds = np.array([row.seed for row in rows], dtype=np.uint64)
    rngs = [np.random.default_rng(seed_in_domain(domain, row.seed, seat, 773)) for row in rows]
    arena = Batch(count)
    chosen = np.zeros((count, len(ACTION_NAMES)), dtype=np.int32)
    offered = np.zeros_like(chosen)
    diagnostics = np.zeros((count, len(TACTICAL_CHECKS), 2), dtype=np.int32)
    target_metrics = np.zeros((count, len(TARGET_KINDS), len(TARGET_COUNTERS)), dtype=np.int32)
    board_foe_only = np.zeros(count, dtype=bool)
    pending_columns = np.full(count, -1, dtype=np.int32)
    pending_footprints = np.zeros((count, 8), dtype=np.float32)
    decisions = []
    extended = len(ACTION_NAMES) > 32
    try:
        arena.reset(indices, seeds, classes[:, ::-1].copy() if seat else classes, levels)
        for _ in range(cap):
            x, masks, actors, results = arena.observe()
            active = actors >= 0
            if extended:
                altitude = (x[:, 275] == 1) & active
                direct = np.flatnonzero((pending_columns >= 0) & ~altitude)
                record_target_cells(target_metrics, direct, pending_columns[direct],
                                    pending_footprints[direct], board_foe_only)
                pending_columns.fill(-1)
            if not active.any():
                break
            actions = np.full(count, -1, dtype=np.int32)
            standard = arena.bot(expert=False) if "heuristic" in matchup else None
            expert = arena.bot(expert=True) if "expert" in matchup else None
            for actor, category in zip((seat, 1 - seat), matchup):
                subset = np.flatnonzero(actors == actor)
                if not len(subset):
                    continue
                if category == "learned":
                    actions[subset] = policy.choose(x[subset], masks[subset])
                elif category == "prior":
                    if prior_sample:
                        logits = prior.logits(x[subset])
                        for values, lane in zip(logits, subset):
                            values += rngs[lane].gumbel(size=len(ACTION_NAMES))
                        actions[subset] = np.where(masks[subset], logits, -np.inf).argmax(axis=1)
                    else:
                        actions[subset] = prior.choose(x[subset], masks[subset])
                elif category in {"heuristic", "expert"}:
                    actions[subset] = (standard if category == "heuristic" else expert)[subset]
                else:
                    actions[subset] = [
                        rngs[i].choice(np.flatnonzero(masks[i])) for i in subset
                    ]
            legal_choices = actions[active]
            if (legal_choices < 0).any() or (legal_choices >= len(ACTION_NAMES)).any() or not masks[
                indices[active], legal_choices
            ].all():
                raise RuntimeError(f"illegal action in {matchup}, starting at {rows[0]}")
            combat_phase = x[:, COMBAT_PHASES].any(axis=1)
            if extended:
                combat_phase |= x[:, 271] == 1  # CutDamage reaction.
            choices = (actors == seat) & combat_phase & (masks.sum(axis=1) > 1)
            offered[choices] += masks[choices]
            chosen[indices[choices], actions[choices]] += 1
            for j, (name, selected_ids, offered_ids) in enumerate(TACTICAL_CHECKS):
                eligible = choices
                if name in {"end_with_weapon_or_spell", "move_with_weapon_or_spell",
                            "weapon_with_spell_available", "cast_when_available"}:
                    eligible = choices & (x[:, 136] == 1)
                if name in EXTENDED_REACTIONS:
                    phase, _ = EXTENDED_REACTIONS[name]
                    eligible = (
                        (actors == seat) & (masks.sum(axis=1) > 1) & (x[:, 256 + phase] == 1)
                        if extended else np.zeros(count, dtype=bool)
                    )
                eligible = eligible & masks[:, offered_ids].any(axis=1)
                diagnostics[eligible, j, 0] += 1
                diagnostics[eligible & np.isin(actions, selected_ids), j, 1] += 1
            # Current target geometry resolves columns with a single legal elevation
            # directly, and offers a separate altitude decision otherwise.
            target_rows = (x[:, 261] == 1) if extended else (x[:, 140] == 1) & (x[:, 157] == 0)
            target_columns = (x[:, 262] == 1) if extended else (x[:, 140] == 1) & (x[:, 157] == 1)
            target_altitudes = (x[:, 275] == 1) if extended else np.zeros(count, dtype=bool)
            known_foe = (x[:, 334] == 1) if extended else active
            placed_axis = (target_columns & (actions < 9)) | (target_altitudes & (actions < 7))
            target_lanes = np.flatnonzero(
                (actors == seat) & known_foe & (target_rows | placed_axis)
            )

            if len(target_lanes):
                target = x[target_lanes]
                candidates = np.concatenate(
                    (target[:, 160:224].reshape(-1, 8, 8), target[:, None, 146:154]),
                    axis=1,
                )
                aiming_row = target_rows[target_lanes]
                board_foe_only[target_lanes[aiming_row]] = (
                    (candidates[aiming_row, :, 4] != 0) & masks[target_lanes[aiming_row], :9]
                ).any(axis=1)

                select = ~aiming_row
                if extended:
                    pending = target_columns[target_lanes]
                    lanes = target_lanes[pending]
                    pending_columns[lanes] = x[lanes, 154].astype(np.int32) - 1
                    pending_footprints[lanes] = candidates[pending, actions[lanes]]
                    select &= target_altitudes[target_lanes]
                completed = target_lanes[select]
                record_target_cells(
                    target_metrics, completed, x[completed, 154].astype(np.int32) - 1,
                    candidates[select, actions[completed]], board_foe_only,
                )
            if trace:
                state = arena.state(0)
                event = {
                    "actor": int(actors[0]),
                    "action": int(actions[0]),
                    "action_name": TARGET_PHASES.get(state["phase"], ACTION_NAMES[actions[0]]),
                    "action_label": state["actions"][int(actions[0])],
                    "legal": np.flatnonzero(masks[0]).tolist(),
                    "state": state,
                }
                if state["phase"] == 5 and actions[0] < 7:
                    event["target"] = {"row": int(actions[0])}
                elif ((state["phase"] == 6 and actions[0] < 9)
                      or (state["phase"] == 19 and actions[0] < 7)):
                    choice = int(actions[0])
                    row = int(round(float(x[0, 156]) * 7)) - 1
                    altitude = state["phase"] == 19
                    column = int(round(float(x[0, 157]) * 9)) - 1 if altitude else choice
                    footprint = x[0, 146:154] if choice == 8 else x[0, 160 + choice * 8:168 + choice * 8]
                    kind = int(x[0, 154])
                    event["target"] = {
                        "kind": TARGET_KINDS.get(kind, str(kind)), "row": row, "column": column,
                        "covers_foe": bool(footprint[2]), "covers_self": bool(footprint[3]),
                    }
                    if altitude:
                        event["target"]["altitude"] = choice
                    elif extended:
                        event["target"]["projected_over_legal_altitudes"] = True
                decisions.append(event)
            arena.step(actions)
        _, _, actors, results = arena.observe()
        if extended:
            direct = np.flatnonzero((pending_columns >= 0) & (actors < 0))
            record_target_cells(target_metrics, direct, pending_columns[direct],
                                pending_footprints[direct], board_foe_only)
        reasons = (arena.result_reasons() if extended else
                   np.where(results < 0, 0, np.where(results == 2, 5, 1)))
        outcomes = np.where(results == seat, 1, np.where(results == 1 - seat, -1, 0))
        replay = None
        if trace:
            if decisions[0]["state"]["layout"] != rows[0].arena:
                raise RuntimeError("final schedule did not select the expected layout")
            replay = {
                "pair": asdict(rows[0]), "focal_seat": seat, "decisions": decisions,
                "result": int(results[0]), "reason": int(reasons[0]),
                "final_state": arena.state(0),
            }
            arena.reset(indices, seeds, classes[:, ::-1].copy() if seat else classes, levels)
            for i, decision in enumerate(decisions):
                _, masks, actors, _ = arena.observe()
                if (int(actors[0]) != decision["actor"]
                    or np.flatnonzero(masks[0]).tolist() != decision["legal"]
                    or dump(arena.state(0)) != dump(decision["state"])):
                    raise RuntimeError(f"replay diverged before decision {i}")
                arena.step(np.array([decision["action"]], dtype=np.int32))
            if dump(arena.state(0)) != dump(replay["final_state"]):
                raise RuntimeError("replay final state diverged")
        return outcomes, reasons, chosen, offered, diagnostics, target_metrics, replay
    finally:
        arena.close()


def dump(value, **kwargs):
    def encode(obj):
        return obj.tolist() if hasattr(obj, "tolist") else asdict(obj)

    return json.dumps(value, default=encode, **kwargs)


def key(matchup):
    return "_vs_".join(matchup)


def summarize(outcomes, rows, mask, matchups):
    strata = np.array([(row.focal, row.level, row.arena) for row in rows])[mask]
    comparisons = {
        key(matchup): summary(
            outcomes[key(matchup)][mask], strata,
            outcomes["random_vs_heuristic"][mask] if matchup == MATCHUPS[0] else None,
        )
        for matchup in matchups
    }
    comparisons["learned_vs_heuristic"]["vs_heuristic_self_control"] = paired_difference(
        outcomes["learned_vs_heuristic"][mask], outcomes["heuristic_vs_heuristic"][mask], strata
    )
    if "learned_vs_expert" in comparisons:
        comparisons["learned_vs_expert"]["vs_expert_self_control"] = paired_difference(
            outcomes["learned_vs_expert"][mask], outcomes["expert_vs_expert"][mask], strata
        )
        if "prior_vs_heuristic" in outcomes:
            comparisons["learned_vs_heuristic"]["vs_prior_control"] = paired_difference(
                outcomes["learned_vs_heuristic"][mask], outcomes["prior_vs_heuristic"][mask], strata
            )
    return comparisons


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, default=DEFAULT_CHECKPOINT)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seed-domain", choices=["final", "s2-final"], default="final")
    parser.add_argument("--prior", type=Path, help="migrated frozen prior for the self-play evaluation")
    parser.add_argument("--expected-sha256", help="require the exact frozen checkpoint hash")
    parser.add_argument("--prior-sample", action="store_true",
                        help="sample the migrated prior with a reproducible per-bout stream")
    parser.add_argument("--repeats", type=int, default=len(ARENAS))
    parser.add_argument("--max-decisions", type=int, default=800)
    args = parser.parse_args()
    if args.repeats % len(ARENAS) or args.repeats < len(ARENAS) or args.max_decisions < 1:
        parser.error("repeats must be a positive multiple of four and max decisions must be positive")
    if args.seed_domain == "s2-final" and not args.expected_sha256:
        parser.error("--seed-domain s2-final requires the exact --expected-sha256")
    digest = hashlib.sha256(args.checkpoint.read_bytes()).hexdigest()
    if args.expected_sha256 and digest != args.expected_sha256:
        parser.error("checkpoint does not match the required frozen SHA-256")
    policy = Policy.load(args.checkpoint).require_arena(native.ARENA_ABI, native.FEATURES, native.ACTIONS)
    native_library = Path(native._lib()._name)
    native_digest = hashlib.sha256(native_library.read_bytes()).hexdigest()
    prior = None
    prior_digest = None
    matchups = MATCHUPS
    if args.seed_domain == "s2-final":
        matchups = SELF_PLAY_MATCHUPS
        if args.prior:
            prior_digest = hashlib.sha256(args.prior.read_bytes()).hexdigest()
            prior = Policy.load(args.prior).require_arena(native.ARENA_ABI, native.FEATURES, native.ACTIONS)
        else:
            matchups = tuple(matchup for matchup in matchups if "prior" not in matchup)
    classes, levels, arenas = CLASSES, tuple(range(1, 6)), ARENAS
    rows = list(held_out_schedule(len(classes), levels, args.repeats, domain=args.seed_domain))
    outcomes = {key(matchup): np.empty((len(rows), 2), dtype=np.int8) for matchup in matchups}
    endings = {name: np.empty_like(scores) for name, scores in outcomes.items()}
    costs = {key(matchup): Counter() for matchup in matchups}
    tactical = {
        key(matchup): {
            cls: {
                "chosen_when_choice": Counter(),
                "offered_when_choice": Counter(),
                "conditional_decisions": {metric: Counter() for metric, _, _ in TACTICAL_CHECKS},
                "ground_targets": {kind: Counter() for kind in TARGET_KINDS.values()},
            }
            for cls in classes
        }
        for matchup in matchups
    }
    replays = {}
    replay_order = {}
    replay_files = {
        "combat": "replay.json",
        "opportunity": "replay-opportunity.json",
        "target": "replay-target.json",
    }
    for start in range(0, len(rows), 256):
        group = rows[start:start + 256]
        for seat in (0, 1):
            for matchup in matchups:
                name = key(matchup)
                results, reasons, chosen, offered, diagnostics, ground_targets, _ = evaluate_group(
                    group, seat, matchup, policy, args.max_decisions, prior=prior, prior_sample=args.prior_sample, domain=args.seed_domain
                )
                outcomes[name][start:start + len(group), seat] = results
                endings[name][start:start + len(group), seat] = reasons
                costs[name].update({
                    label: int((reasons == code).sum())
                    for code, label in enumerate(REASON_LABELS)
                })
                for i, pair in enumerate(group):
                    table = tactical[name][classes[pair.focal]]
                    for counts, field in zip((chosen, offered), ("chosen_when_choice", "offered_when_choice")):
                        table[field].update({
                            ACTION_NAMES[j]: int(counts[i, j]) for j in np.flatnonzero(counts[i])
                        })
                    for j, (metric, _, _) in enumerate(TACTICAL_CHECKS):
                        table["conditional_decisions"][metric].update({
                            "eligible": int(diagnostics[i, j, 0]),
                            "chosen": int(diagnostics[i, j, 1]),
                        })
                    for j, kind in enumerate(TARGET_KINDS.values()):
                        table["ground_targets"][kind].update({
                            field: int(value) for field, value in zip(TARGET_COUNTERS, ground_targets[i, j])
                        })
                if matchup == MATCHUPS[0]:
                    replay_candidates = {
                        "combat": offered[:, (14, 16, 17)].any(axis=1),
                        "target": ground_targets[:, (0, 2), 0].any(axis=1),
                    }
                elif matchup == MATCHUPS[3]:
                    replay_candidates = {"opportunity": offered[:, OPPORTUNITY] > 0}
                else:
                    replay_candidates = {}
                for replay_kind, qualifies in replay_candidates.items():
                    if not qualifies.any():
                        continue
                    i = int(np.flatnonzero(qualifies)[0])
                    order = (start + i, seat)
                    if replay_kind in replay_order and order >= replay_order[replay_kind]:
                        continue
                    replay_order[replay_kind] = order
                    result, _, _, _, _, _, replay = evaluate_group(
                        [group[i]], seat, matchup, policy, args.max_decisions, trace=True,
                        prior=prior, prior_sample=args.prior_sample, domain=args.seed_domain
                    )
                    if result[0] != results[i]:
                        raise RuntimeError("policy replay outcome diverged")
                    replays[replay_kind] = {
                        "matchup": matchup,
                        "focal_class": CLASSES[group[i].focal],
                        "opponent_class": CLASSES[group[i].opponent],
                        "checkpoint_sha256": digest,
                        **replay,
                    }
        print(f"{start + len(group)}/{len(rows)} pairs", flush=True)
    if hashlib.sha256(args.checkpoint.read_bytes()).hexdigest() != digest:
        raise RuntimeError("checkpoint changed during evaluation")
    if prior_digest and hashlib.sha256(args.prior.read_bytes()).hexdigest() != prior_digest:
        raise RuntimeError("prior checkpoint changed during evaluation")
    if hashlib.sha256(native_library.read_bytes()).hexdigest() != native_digest:
        raise RuntimeError("native library changed during evaluation")
    args.output.mkdir(parents=True, exist_ok=True)
    classes_col = np.array([row.focal for row in rows])
    arenas_col = np.array([row.arena for row in rows])
    levels_col = np.array([row.level for row in rows])
    report = {
        "checkpoint_sha256": digest,
        "native_library_sha256": native_digest,
        "arena_abi": native.ARENA_ABI,
        "arena_features": native.FEATURES,
        "arena_actions": native.ACTIONS,
        "prior_checkpoint_sha256": prior_digest,
        "prior_action_mode": "sampled" if args.prior_sample else "greedy",
        "seed_domain": args.seed_domain,
        "training_metadata": policy.metadata,
        "pairs": len(rows),
        "bouts_per_matchup": 2 * len(rows),
        "repeats": args.repeats,
        "max_decisions": args.max_decisions,
        "class_ids": classes,
        "arena_ids": arenas,
        "overall": summarize(outcomes, rows, np.ones(len(rows), dtype=bool), matchups),
        "classes": {cls: summarize(outcomes, rows, classes_col == i, matchups) for i, cls in enumerate(classes)},
        "arenas": {arena: summarize(outcomes, rows, arenas_col == arena, matchups) for arena in arenas},
        "levels": {level: summarize(outcomes, rows, levels_col == level, matchups) for level in levels},
        "class_levels": {
            cls: {level: summarize(outcomes, rows, (classes_col == i) & (levels_col == level), matchups)
                  for level in levels}
            for i, cls in enumerate(classes)
        },
        "seats": {
            seat: {name: record(values[:, seat]) for name, values in outcomes.items()}
            for seat in (0, 1)
        },
        "costs": costs,
        "draw_definition": "terminal arena draws and evaluator decision caps",
        "termination_reason_codes": dict(enumerate(REASON_LABELS)),
        "target_footprints": (
            "Primary-opponent and self footprints on decisions where the primary opponent's "
            "position is known. Does not count secondary targets or damage after saving throws."
        ),
        "tactical": tactical,
    }
    (args.output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    for name, replay in replays.items():
        filename = replay_files[name]
        (args.output / filename).write_text(dump(replay, indent=2) + "\n")
    with gzip.open(args.output / "paired-outcomes.jsonl.gz", "wt") as stream:
        for i, pair in enumerate(rows):
            stream.write(json.dumps({
                **asdict(pair), **{name: values[i].tolist() for name, values in outcomes.items()},
                "termination_reasons": {name: values[i].tolist() for name, values in endings.items()},
            }) + "\n")


if __name__ == "__main__":
    main()
