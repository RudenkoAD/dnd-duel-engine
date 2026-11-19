"""Replay an independent DEV schedule and audit tactical decisions by arena role.

A reversal is a return to the previous position within one turn. This includes
intentional retracing. Revisits also include longer cycles. Inactive turns
can still contain support actions.
Full-HP Second Wind can pay for Tactical Shift at level 5. Report how often an
immediate movement decision follows it and whether the fighter moves at all.
"""

import argparse
from collections import Counter
import hashlib
from io import BytesIO
import json
from pathlib import Path
from random import Random

from arena.native import ACTIONS, ARENA_ABI, CLASS_NAMES, FEATURES, Batch
from arena.policy import DEFAULT_CHECKPOINT, Policy

MELEE = {"Barbarian", "Fighter", "Monk", "Paladin"}
MELEE_OFFENSE = {
    "Melee attack", "Reckless Attack", "Bonus Unarmed Strike", "Flurry of Blows", "Sacred Weapon",
}
OFFENSE = MELEE_OFFENSE | {
    "Ranged attack", "Land's Aid", "Divine Spark", "Strike with Spiritual Weapon",
    "Move Moonbeam", "Heightened Fireball", "Quickened Fire Bolt", "Vicious Mockery",
    "Dissonant Whispers", "Shatter", "Starry Wisp", "Sacred Flame", "Guiding Bolt",
    "Spiritual Weapon", "Spirit Guardians", "Thunderwave", "Moonbeam", "Fireball",
    "Burning Hands", "Fire Bolt", "Chromatic Orb", "Magic Missile", "Scorching Ray",
    "Ray of Frost", "Eldritch Blast",
}
COUNTS = (
    "bouts", "wins", "losses", "draws", "turns", "moves", "reversals",
    "turns_with_reversal", "revisits", "turns_with_revisit", "turns_with_offense_available",
    "turns_without_move_or_weapon_spell", "inactive_turns_with_offense_available",
    "turns_with_offense_available_but_unused",
    "end_with_offense_offered", "end_with_offense_and_no_offensive_choice",
    "second_wind", "full_hp_second_wind", "full_hp_second_wind_without_move",
    "full_hp_second_wind_without_immediate_move", "full_hp_wind_before_level_five",
    "opportunity_offered", "opportunity_chosen", "defend_offered", "defend_chosen",
    "rider_offered", "rider_chosen", "already_lethal_rider_offered", "already_lethal_rider_chosen",
    "shield_tradeoff_offered", "shield_tradeoff_opportunity_chosen",
    "low_hp_shield_tradeoff_offered", "low_hp_shield_tradeoff_opportunity_chosen",
    "targeted_bursts_or_moonbeams", "targeted_area_foe_affected", "targeted_area_self_affected",
    "targeted_area_safe_center_available", "targeted_area_safe_row_selected",
    "targeted_area_safe_center_selected", "lands_aid_placements",
    "lands_aid_damage_and_healing_available", "lands_aid_damage_and_healing_selected",
    "lands_aid_damage_and_healing_row_selected", "caltrops_placements",
    "caltrops_placed_on_own_square", "caltrops_placed_on_foe_square",
    "target_row_decisions", "target_column_decisions",
    *(f"{kind}_{metric}" for kind in ("burst", "moonbeam") for metric in (
        "placements", "safe_center_available", "safe_row_selected", "safe_center_selected",
        "foe_affected", "self_affected",
    )),
)
# The actor magic-state block starts at 84, with the slot-used flag at offset 14.
ACTOR_SLOT_USED_THIS_TURN = 98
PENDING_RIDER_DAMAGE = 151
RESPONSES = {1: ("opportunity", 14), 2: ("defend", 16), 3: ("rider", 17)}


def dev_seed(*keys):
    """Domain-match the registered DEV prefix with an independent probe key."""
    payload = ("tactical-probe-v1:" + ",".join(map(str, keys))).encode()
    return int.from_bytes(b"DEV" + hashlib.blake2b(payload, digest_size=5).digest(), "big")


def coordinate(fighter):
    return [fighter["x"], fighter["y"]]


def plausible_shield_tradeoff(state, seat, slot_used_this_turn):
    if state["phase"] != 1 or not state["action_available"] or slot_used_this_turn:
        return False
    me, enemy = state["fighters"][seat], state["fighters"][1 - seat]
    class_name = CLASS_NAMES[me["class_id"]]
    # Both presets have a dagger and Dexterity +2. Require enough HP to
    # survive even a maximum critical hit so the reaction cannot end the duel.
    max_critical = 2 * me["melee_die"] + 2
    return (
        class_name in {"Sorcerer", "Wizard"}
        and me["hp"] < me["max_hp"] and me["reaction"] and me["shield"] == 0
        and any(me["slots"]) and enemy["hp"] + enemy["temp"] > max_critical
    )


def target_preview(features, choice):
    # The arena ABI packs the ninth candidate in the otherwise idle pending
    # hit block. The other candidates occupy the normal movement preview.
    offset = 146 if choice == 8 else 160 + 8 * choice
    return features[offset:offset + 8]


def record_target(features, mask, action, active, state, seat, counts):
    kind = int(features[154])
    column_phase = bool(features[157])
    options = [target_preview(features, index) for index in mask.nonzero()[0]]
    choice = target_preview(features, action)
    if not column_phase:
        counts["target_row_decisions"] += 1
        placement = {
            "safe_available": any(option[4] for option in options),
            "safe_row": bool(choice[4]),
            "both_available": any(option[5] for option in options),
            "both_row": bool(choice[5]),
            "kind": kind,
            "row": action,
        }
        if active is not None:
            placement["ability"] = active["actions"][-1]
            active["pending_placement"] = placement
            active["actions"].append(f"Target row {action + 1}")
        return
    counts["target_column_decisions"] += 1
    placement = active.pop("pending_placement", {}) if active is not None else {}
    if active is not None:
        active["actions"].append(f"Target column {action + 1}")
        active["placements"].append({
            **placement, "column": action,
            "affects_foe": bool(choice[2]), "affects_self": bool(choice[3]),
        })
    if kind in (1, 3):
        area = "burst" if kind == 1 else "moonbeam"
        counts[f"{area}_placements"] += 1
        counts[f"{area}_foe_affected"] += bool(choice[2])
        counts[f"{area}_self_affected"] += bool(choice[3])
        counts["targeted_bursts_or_moonbeams"] += 1
        counts["targeted_area_foe_affected"] += bool(choice[2])
        counts["targeted_area_self_affected"] += bool(choice[3])
        if placement.get("safe_available"):
            counts["targeted_area_safe_center_available"] += 1
            counts["targeted_area_safe_row_selected"] += placement["safe_row"]
            counts["targeted_area_safe_center_selected"] += bool(choice[4])
            counts[f"{area}_safe_center_available"] += 1
            counts[f"{area}_safe_row_selected"] += placement["safe_row"]
            counts[f"{area}_safe_center_selected"] += bool(choice[4])
    elif kind == 2:
        counts["lands_aid_placements"] += 1
        me = state["fighters"][seat]
        if me["hp"] < me["max_hp"] and placement.get("both_available"):
            counts["lands_aid_damage_and_healing_available"] += 1
            counts["lands_aid_damage_and_healing_selected"] += bool(choice[5])
            counts["lands_aid_damage_and_healing_row_selected"] += placement["both_row"]
    elif kind == 4:
        counts["caltrops_placements"] += 1
        center = [action, round(float(features[156]) * 7) - 1]
        counts["caltrops_placed_on_own_square"] += center == coordinate(state["fighters"][seat])
        counts["caltrops_placed_on_foe_square"] += center == coordinate(state["fighters"][1 - seat])


def turn_record(state, seat):
    me, enemy = state["fighters"][seat], state["fighters"][1 - seat]
    return {
        "turn": state["turns"], "start_hp": me["hp"], "enemy_start_hp": enemy["hp"],
        "positions": [coordinate(me)], "enemy_position": coordinate(enemy), "actions": [],
        "placements": [],
        "offense": 0, "offense_available": False, "reversals": 0,
        "full_hp_wind_positions": [], "wind_without_move": 0,
        "skipped_offense": False, "enemy_end_hp": enemy["hp"],
    }


def finish_turn(record, state, seat, counts):
    me, enemy = state["fighters"][seat], state["fighters"][1 - seat]
    final = coordinate(me)
    if final != record["positions"][-1]:
        record["positions"].append(final)
    positions = record["positions"]
    record["enemy_end_hp"] = enemy["hp"]
    record["reversals"] = sum(a == c for a, _, c in zip(positions, positions[1:], positions[2:]))
    record["revisits"] = len(positions) - len({tuple(position) for position in positions})
    moves = len(positions) - 1
    winds = record.pop("full_hp_wind_positions")
    record["full_hp_wind"] = len(winds)
    record["wind_without_move"] = sum(place == moves for place in winds)
    counts["turns"] += 1
    counts["moves"] += moves
    counts["reversals"] += record["reversals"]
    counts["turns_with_reversal"] += record["reversals"] > 0
    counts["revisits"] += record["revisits"]
    counts["turns_with_revisit"] += record["revisits"] > 0
    counts["turns_with_offense_available"] += record["offense_available"]
    inactive = moves == 0 and record["offense"] == 0
    counts["turns_without_move_or_weapon_spell"] += inactive
    counts["inactive_turns_with_offense_available"] += inactive and record["offense_available"]
    counts["turns_with_offense_available_but_unused"] += (
        record["offense"] == 0 and record["offense_available"]
    )
    counts["full_hp_second_wind_without_move"] += record["wind_without_move"]
    return record


def run_bout(batch, policy, controller, opponent, seed, classes, level, seat, counts):
    batch.reset([0], [seed], [classes], [[level, level]])
    rng = Random(dev_seed(seed, seat, 1))
    trace = []
    active = None
    pending_wind = None
    for _ in range(800):
        x, masks, actors, results = batch.observe()
        actor = int(actors[0])
        if actor != seat:
            if active is not None:
                state = batch.state()
                if state["turns"] != active["turn"] or actor < 0:
                    trace.append(finish_turn(active, state, seat, counts))
                    active = None
            if actor < 0:
                break
            if opponent == "random":
                action = int(rng.choice(masks[0].nonzero()[0]))
            else:
                action = int(batch.bot(expert=opponent == "expert")[0])
            batch.step([action])
            continue
        state = batch.state()
        action = (policy.act(x[0], masks[0]) if controller == "learned"
                  else int(batch.bot(expert=controller == "expert")[0]))
        name = state["actions"][action]
        phase = state["phase"]
        if phase in RESPONSES:
            response, action_id = RESPONSES[phase]
            counts[f"{response}_offered"] += 1
            counts[f"{response}_chosen"] += action == action_id
            if phase == 3:
                enemy = state["fighters"][1 - seat]
                damage = round(float(x[0, PENDING_RIDER_DAMAGE]) * 40)
                # A pending Monk or Rogue reduction is private. Exclude those
                # matchups to avoid marking potentially useful riders redundant.
                if (CLASS_NAMES[enemy["class_id"]] not in {"Monk", "Rogue"}
                        and damage >= enemy["hp"] + enemy["temp"]):
                    counts["already_lethal_rider_offered"] += 1
                    counts["already_lethal_rider_chosen"] += action == action_id
            if plausible_shield_tradeoff(state, seat, x[0, ACTOR_SLOT_USED_THIS_TURN]):
                counts["shield_tradeoff_offered"] += 1
                counts["shield_tradeoff_opportunity_chosen"] += action == action_id
                me = state["fighters"][seat]
                if me["hp"] * 2 <= me["max_hp"]:
                    counts["low_hp_shield_tradeoff_offered"] += 1
                    counts["low_hp_shield_tradeoff_opportunity_chosen"] += action == action_id
        if phase in (5, 6):
            record_target(x[0], masks[0], action, active, state, seat, counts)
        if phase == 0:
            if pending_wind is not None:
                previous_turn, previous_position = pending_wind
                counts["full_hp_second_wind_without_immediate_move"] += (
                    previous_turn != state["turns"] or action >= 8
                    or previous_position != coordinate(state["fighters"][seat])
                )
                pending_wind = None
            me = state["fighters"][seat]
            if active is None or active["turn"] != state["turns"]:
                if active is not None:
                    trace.append(finish_turn(active, state, seat, counts))
                active = turn_record(state, seat)
            position = coordinate(me)
            if position != active["positions"][-1]:
                active["positions"].append(position)
            options = sorted(set(state["actions"].values()) & OFFENSE)
            active["offense_available"] |= bool(options)
            active["actions"].append(name)
            active["offense"] += name in OFFENSE
            if name == "Second Wind":
                counts["second_wind"] += 1
                if me["hp"] == me["max_hp"]:
                    counts["full_hp_second_wind"] += 1
                    counts["full_hp_wind_before_level_five"] += me["level"] < 5
                    active["full_hp_wind_positions"].append(len(active["positions"]) - 1)
                    pending_wind = (state["turns"], position)
            if action == 8 and options:
                counts["end_with_offense_offered"] += 1
                active["skipped_offense"] = True
                active["skipped_options"] = options
                counts["end_with_offense_and_no_offensive_choice"] += active["offense"] == 0
        batch.step([action])
    _, _, actors, results = batch.observe()
    if actors[0] >= 0:
        raise RuntimeError(f"unfinished after 800 decisions on dev seed {seed}")
    if active is not None:
        trace.append(finish_turn(active, batch.state(), seat, counts))
    if pending_wind is not None:
        counts["full_hp_second_wind_without_immediate_move"] += 1
    winner = int(results[0])
    counts["bouts"] += 1
    counts["wins" if winner == seat else "losses" if winner == 1 - seat else "draws"] += 1
    return trace


def update_examples(examples, role, context, records):
    for record in records:
        damage = record["enemy_start_hp"] - record["enemy_end_hp"]
        moves = len(record["positions"]) - 1
        role_offense = MELEE_OFFENSE if role == "melee" else OFFENSE - MELEE_OFFENSE
        offensive_actions = role_offense.intersection(record["actions"])
        strong = (damage if offensive_actions else 0, min(moves, 3), -len(record["actions"]))
        unsafe = sum(
            item["kind"] in (1, 3) and item.get("safe_available") and item["affects_self"]
            for item in record["placements"]
        )
        poor = (
            3 * record["wind_without_move"] + 3 * unsafe + record["reversals"]
            + 2 * record["skipped_offense"],
            -record["offense"],
            -damage,
        )
        for label, score in [("strong", strong), ("poor", poor)]:
            if score[0] <= 0:
                continue
            key = (role, label)
            if key not in examples or score > examples[key][0]:
                examples[key] = (score, {**context, **record})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, default=DEFAULT_CHECKPOINT)
    parser.add_argument("--controller", choices=["learned", "baseline", "expert"], default="learned")
    parser.add_argument("--opponent", choices=["baseline", "expert", "random"], default="baseline")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error("--repeats must be positive")
    checkpoint = args.checkpoint.read_bytes() if args.controller == "learned" else None
    policy = None
    if checkpoint is not None:
        policy = Policy.load(BytesIO(checkpoint)).require_arena(ARENA_ABI, FEATURES, ACTIONS)
    groups = {role: Counter({key: 0 for key in COUNTS}) for role in ("melee", "ranged")}
    per_class = {name: Counter({key: 0 for key in COUNTS}) for name in CLASS_NAMES}
    examples = {}
    inactive_actions = {role: Counter() for role in groups}
    with Batch(1) as batch:
        for focal, name in enumerate(CLASS_NAMES):
            role = "melee" if name in MELEE else "ranged"
            for level in range(1, 6):
                for repeat in range(args.repeats):
                    offset = 3 * repeat + repeat // 4
                    opponent = (focal + level + offset) % len(CLASS_NAMES)
                    seed = dev_seed(focal, opponent, level, repeat)
                    for seat in range(2):
                        classes = [focal, opponent] if seat == 0 else [opponent, focal]
                        counts = Counter({key: 0 for key in COUNTS})
                        records = run_bout(batch, policy, args.controller, args.opponent, seed, classes,
                                           level, seat, counts)
                        groups[role].update(counts)
                        per_class[name].update(counts)
                        for record in records:
                            if len(record["positions"]) == 1 and record["offense"] == 0:
                                non_end = [a for a in record["actions"] if a != "End turn"]
                                last_action = non_end[-1] if non_end else "End turn only"
                                inactive_actions[role][last_action] += 1
                        update_examples(examples, role, {
                            "class": name, "opponent": CLASS_NAMES[opponent], "level": level,
                            "seat": seat, "seed": seed,
                        }, records)
    report = {
        "controller": args.controller, "opponent": args.opponent,
        "checkpoint": policy.metadata if policy is not None else None,
        "checkpoint_sha256": hashlib.sha256(checkpoint).hexdigest() if checkpoint else None,
        "seed_domain": "DEV / tactical-probe-v1", "repeats": args.repeats,
        "targeting": (
            "Focal actor only, both seats. Globally feasible center measured "
            "from all legal rows; selected tile measured at column resolution."
        ),
        "groups": groups, "per_class": per_class,
        "inactive_turn_last_action_by_role": inactive_actions,
        "examples": {f"{role}_{label}": record
                     for (role, label), (_, record) in examples.items()},
    }
    result = json.dumps(report, indent=2)
    if args.output:
        args.output.write_text(result + "\n")
    else:
        print(result)


if __name__ == "__main__":
    main()
