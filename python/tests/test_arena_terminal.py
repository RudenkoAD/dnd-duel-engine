"""Exercise the identical Rust decisions through native, Gym and the terminal."""
from collections import deque
from contextlib import redirect_stdout
from dataclasses import dataclass
from io import StringIO
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import numpy as np

from arena.gym_env import LegacyArenaEnv
from arena.native import ACTIONS, ARENA_ABI, FEATURES, CLASS_NAMES, Batch
from arena.policy import CHECKPOINT_VERSION
from arena_play import main, pending_description, result_message, run_duel, state_screen


@dataclass
class Decision:
    actor: int
    features: np.ndarray
    mask: np.ndarray
    action: int
    canonical: str
    views: tuple

    @property
    def legal(self):
        return np.flatnonzero(self.mask).tolist()

    @property
    def ordinal(self):
        return str(self.legal.index(self.action) + 1)


class ScriptOpponent:
    """A test policy that verifies its own input before returning a fixed choice."""

    def __init__(self, decisions):
        self.pending = deque(decisions)

    def __call__(self, features, mask):
        if not self.pending:
            raise AssertionError("opponent asked for an unrecorded decision")
        expected = self.pending.popleft()
        np.testing.assert_array_equal(features, expected.features)
        np.testing.assert_array_equal(mask, expected.mask)
        return expected.action

    def act(self, features, mask, rng=None):
        return self(features, mask)


def class_ids(names):
    return [CLASS_NAMES.index(name) for name in names]


def native_trace(seed, classes, levels, choose):
    """Generate ground truth through direct, checked applications of actions."""
    history = []
    probed = set()
    with Batch(1) as battle:
        battle.reset([0], [seed], [classes], [levels])
        while True:
            features, masks, actors, results = battle.observe()
            if results[0] != -1:
                return history, tuple(battle.state(viewer=seat) for seat in (0, 1))
            actor = int(actors[0])
            views = tuple(battle.state(viewer=seat) for seat in (0, 1))
            mask = masks[0]
            options = views[actor]["actions"]
            assert set(options) == set(np.flatnonzero(mask)), views[actor]
            assert not views[1 - actor]["actions"], views[1 - actor]
            phase_key = actor, views[actor]["phase"]
            if phase_key not in probed:
                probed.add(phase_key)
                invalid = int(np.flatnonzero(~mask)[0])
                with np.testing.assert_raises(ValueError):
                    battle.step([invalid])
                for before, after in zip((features, masks, actors, results), battle.observe()):
                    np.testing.assert_array_equal(before, after)
            action = int(choose(battle, views[actor], options, history))
            assert action in options, (action, views[actor])
            history.append(Decision(actor, features[0], mask, action, options[action], views))
            battle.step([action])


def baseline_action(battle, state, actions, history):
    return battle.bot(expert=True)[0]


def scripted_actions(*labels):
    def follow(battle, state, actions, history):
        if len(history) >= len(labels):
            return baseline_action(battle, state, actions, history)
        expected = labels[len(history)]
        for action, label in actions.items():
            if label == expected:
                return action
        raise AssertionError(f"expected {expected!r} at phase {state['phase']}, choices: {actions}")

    return follow


ROGUE_SCRIPT = scripted_actions(
    "Stow melee weapon (free interaction)", "Draw or pick up bow (Utilize action)",
    "Cunning Action: Hide", "End turn", "Search", "Pass", "End turn",
    "North-east", "North-east", "Ranged attack", "Sneak Attack (full damage)",
)
MONK_SCRIPT = scripted_actions(
    "Stow melee weapon (free interaction)", "Draw or pick up bow (Utilize action)", "End turn",
    "North-east", "East", "East", "South-east", "North-east", "East", "East", "Flurry of Blows",
    "Decline", "Open Hand Technique: Topple", "Decline", "Open Hand Technique: Topple", "End turn",
    "Stand", "Ranged attack",
)
ELEVATED_SUGGESTION_SCRIPT = scripted_actions(
    "Fly", "Pass", "End turn", "East", "East", "Suggestion",
    "Suggest moving to a safe cell", "Row 4", "Cell I4", "5 ft above the floor",
    "Pass", "End turn", "Compelled move to elevation 5 ft", "Finish compelled movement",
)

def caster_script(battle, state, options, history):
    if not any(decision.canonical == "Shatter" for decision in history):
        label = "Shatter" if state["actor"] == 0 else "End turn"
        return next(action for action, canonical in options.items() if canonical == label)
    if state["phase"] == 0 or any(d.views[0]["phase"] == 0 and d.canonical != "Shatter"
                                  for d in history[2:]):
        return baseline_action(battle, state, options, history)
    if state["phase"] == 38:
        return next(action for action, label in options.items() if label == "Cast Shatter with level 3 slot")
    if state["phase"] in (5, 6, 19):
        return next(action for action, effect in state["target_effects"].items()
                    if effect["foe_only"] and (state["phase"] != 19 or action > 0))
    return baseline_action(battle, state, options, history)


def companion_script(battle, state, options, history):
    if any(d.canonical == "End companion turn" for d in history):
        return baseline_action(battle, state, options, history)
    labels = {label: action for action, label in options.items()}
    phase = state["phase"]
    choices = {50: "Owl familiar", 51: "Choose row 4", 52: "Choose column 2"}
    if phase in choices:
        return labels[choices[phase]]
    if phase == 53:
        choice = "End companion turn" if any(d.canonical == "Companion ascend" for d in history) else "Companion ascend"
        return labels[choice]
    if not any(d.canonical == "Wild Companion (Wild Shape)" for d in history) and state["actor"] == 0:
        if "Wild Companion (Wild Shape)" in labels:
            return labels["Wild Companion (Wild Shape)"]
        return labels["Stow melee weapon (free interaction)"]
    return labels["End turn"]


def split_missile_script(battle, state, options, history):
    labels = {label: action for action, label in options.items()}
    phase = state["phase"]
    if not history:
        return labels["Magic Missile"]
    if phase == 38 and len(history) == 1:
        return labels["Cast Magic Missile with level 1 slot"]
    if phase == 80 and len(history) < 5:
        return labels["Aim one projectile at yourself" if len(history) == 3 else "Aim one projectile at the opposing adventurer"]
    if phase == 81 and not any(d.canonical.startswith("Cast Shield using") for d in history):
        return labels["Cast Shield using a level 1 slot"]
    return baseline_action(battle, state, options, history)


def poison_guess_script(battle, state, options, history):
    labels = {label: action for action, label in options.items()}
    if not any(d.canonical == "Poison Spray" for d in history):
        return labels["Poison Spray" if state["actor"] == 0 else "End turn"]
    if state["phase"] == 80:
        return labels["Aim at a guessed location"]
    if state["phase"] in (82, 83, 84):
        return min(options)
    return baseline_action(battle, state, options, history)


class ArenaTerminalParityTests(unittest.TestCase):
    def test_draw_message_explains_which_native_limit_was_reached(self):
        for limit, expected in [("turn_limit", "60 индивидуальных ходов"),
                                ("decision_limit", "800 решений")]:
            state = {"winner": 2, "result_reason": "timeout", "limit_reason": limit}
            self.assertIn(expected, result_message(state, 0))

    def assert_gym_parity(self, seed, classes, levels, history, final, seat):
        own = [decision for decision in history if decision.actor == seat]
        policy = ScriptOpponent(decision for decision in history if decision.actor != seat)
        probed = set()
        with LegacyArenaEnv(max_steps=2 * len(history) + 1, opponent_policy=policy) as env:
            obs, _ = env.reset(seed=seed, options={"classes": classes, "levels": levels, "seat": seat})
            for index, decision in enumerate(own):
                self.assertEqual(obs["seat"], seat)
                np.testing.assert_array_equal(obs["state"], decision.features)
                np.testing.assert_array_equal(obs["action_mask"], decision.mask)
                self.assertEqual(env.batch.state(viewer=seat), decision.views[seat])
                phase = decision.views[seat]["phase"]
                if phase not in probed:
                    probed.add(phase)
                    invalid = int(np.flatnonzero(~decision.mask)[0])
                    unchanged, reward, terminated, truncated, info = env.step(invalid)
                    self.assertTrue(info["invalid_action"])
                    self.assertFalse(terminated or truncated)
                    self.assertEqual(reward, -0.05)
                    np.testing.assert_array_equal(unchanged["state"], obs["state"])
                    np.testing.assert_array_equal(unchanged["action_mask"], obs["action_mask"])
                    self.assertEqual(env.batch.state(viewer=seat), decision.views[seat])
                obs, _, terminated, truncated, info = env.step(decision.action)
                self.assertFalse(info["invalid_action"])
                self.assertEqual(terminated or truncated, index == len(own) - 1)
            self.assertFalse(policy.pending)
            self.assertFalse(obs["state"].any())
            self.assertFalse(obs["action_mask"].any())
            self.assertEqual(env.batch.state(viewer=seat), final[seat])

    def assert_terminal_parity(self, seed, classes, levels, history, final, seat):
        own = [decision for decision in history if decision.actor == seat]
        policy = ScriptOpponent(decision for decision in history if decision.actor != seat)
        # The real numbered menu refuses the invalid entry and accepts the same
        # next decision without applying an action while the prompt is repeated.
        inputs = []
        probed = set()
        for decision in own:
            phase = decision.views[seat]["phase"]
            if phase not in probed:
                inputs.append(str(len(decision.legal) + 1))
                probed.add(phase)
            inputs.append(decision.ordinal)
        output = StringIO()
        with Batch(1) as battle, patch("sys.stdin", StringIO("\n".join(inputs) + "\n")), redirect_stdout(output):
            battle.reset([0], [seed], [classes], [levels])
            record = run_duel(battle, policy, seat, "Контрольная дуэль")
            self.assertEqual(battle.state(viewer=seat), final[seat])
        self.assertFalse(policy.pending)
        self.assertFalse(record["interrupted"])
        self.assertEqual(record["winner"], final[seat]["winner"])
        self.assertIn("Выберите один из указанных пунктов.", output.getvalue())
        human_frames = [frame for frame in record["frames"] if frame["actor"] == seat]
        self.assertEqual(len(human_frames), len(own))
        for decision, frame in zip(own, human_frames):
            self.assertEqual(frame["action"], decision.action)
            self.assertEqual(frame["legal"], decision.legal)
            self.assertEqual(frame["canonical"], decision.canonical)
            self.assertEqual(frame["before"], decision.views[seat])
        for frame in record["frames"]:
            self.assertEqual(frame["before"]["viewer"], seat)
            self.assertEqual(frame["after"]["viewer"], seat)
            if frame["actor"] != seat:
                self.assertNotIn("action", frame)
                self.assertNotIn("legal", frame)
                self.assertNotIn("canonical", frame)
        return record, output.getvalue()

    def assert_native_rejects_masked(self, seed, classes, levels):
        with Batch(1) as battle:
            battle.reset([0], [seed], [classes], [levels])
            obs = battle.observe()
            invalid = int(np.flatnonzero(~obs[1][0])[0])
            with self.assertRaisesRegex(ValueError, "illegal action"):
                battle.step([invalid])
            for previous, current in zip(obs, battle.observe()):
                np.testing.assert_array_equal(previous, current)

    def assert_path(self, seed, character_names, levels, choose=baseline_action):
        classes = class_ids(character_names)
        history, final = native_trace(seed, classes, levels, choose)
        self.assert_native_rejects_masked(seed, classes, levels)
        for seat in (0, 1):
            with self.subTest(seat=seat):
                self.assert_gym_parity(seed, classes, levels, history, final, seat)
                self.assert_terminal_parity(seed, classes, levels, history, final, seat)
        return history, final

    def test_rogue_hide_failed_search_and_ambush_from_cover(self):
        history, _ = self.assert_path(308, ("Rogue", "Fighter"), [5, 5], ROGUE_SCRIPT)
        index = next(i for i, d in enumerate(history) if d.actor == 1 and d.canonical == "Search")
        search = history[index]
        hidden = search.views[1]["fighters"][0]
        self.assertFalse(hidden["known"] or hidden["visible"])
        self.assertTrue(all(hidden.get(key) is None for key in ("hp", "ac", "temp", "hidden", "slots", "arrows", "resource")))
        self.assertEqual((hidden["x"], hidden["y"]), (-1, -1))
        tactical = history[index + 1]
        self.assertEqual((tactical.actor, tactical.views[1]["phase"], tactical.canonical), (1, 11, "Pass"))
        ambush_index = next(index for index, d in enumerate(history) if d.actor == 0 and d.canonical == "Ranged attack")
        ambush, followup, resolved = history[ambush_index:ambush_index+3]
        self.assertTrue(ambush.views[0]["fighters"][0]["hidden"])
        self.assertFalse(ambush.views[1]["fighters"][0]["known"])
        self.assertFalse(followup.views[0]["fighters"][0]["hidden"])
        self.assertEqual(followup.canonical, "Sneak Attack (full damage)")
        self.assertLess(resolved.views[1]["fighters"][1]["hp"], followup.views[1]["fighters"][1]["hp"])
        self.assertFalse(followup.views[1]["actions"])

    def test_public_spell_field_appears_in_both_views_with_unseen_caster(self):
        for caster in (0, 1):
            with self.subTest(caster=caster), Batch(1) as battle:
                names = ["Fighter", "Fighter"]
                names[caster] = "Warlock"
                battle.reset([0], [1], [class_ids(names)], [[5, 5]])
                if battle.state()["actor"] != caster:
                    battle.step([8])  # End chance-selected opening turn.
                opening = battle.state()
                origin = opening["fighters"][caster]
                x = origin["x"] + (1 if caster == 0 else -1)
                y = origin["y"]
                spells = {name: action for action, name in opening["actions"].items()}
                battle.step([spells["Stinking Cloud"]])
                for _ in range(4):
                    phase = battle.state()["phase"]
                    if phase == 0:
                        break
                    battle.step([{5: y, 6: x, 19: 0, 38: 2}[phase]])
                expected = [{
                    "x": x, "y": y, "altitude": 0, "extent_feet": 20, "height_feet": 0,
                }]
                for seat in (0, 1):
                    snapshot = battle.state(viewer=seat)
                    self.assertEqual(snapshot["spell_fields"], expected)
                    if seat != caster:
                        self.assertFalse(snapshot["fighters"][caster]["visible"])
                        self.assertNotIn("hp", snapshot["fighters"][caster])
                    screen = state_screen(snapshot, seat)
                    self.assertIn(
                        f"Видимый эффект · {chr(ord('A') + x)}{y + 1} · высота точки 0 фт · от центра 20 фт",
                        screen,
                    )
                battle.step([8])
                features, _, actors, _ = battle.observe()
                self.assertEqual(actors[0], 1 - caster)
                np.testing.assert_allclose(
                    features[0, 840:849],
                    [0, 0, 0, 1, x / 8, y / 6, 0, 0.5, 0],
                )

    def test_hidden_movement_leaves_the_same_foe_observation_mask_and_screen(self):
        classes = class_ids(("Rogue", "Fighter"))
        with Batch(2) as battle:
            battle.reset([0, 1], [308, 308], [classes, classes], [[5, 5], [5, 5]])
            for action in (109, 111, 88):  # Stow the rapier, draw the bow, hide.
                battle.step([action, action])
            battle.step([1, 3])  # Different concealed cells.
            battle.step([8, 8])
            own = [battle.state(i, viewer=0)["fighters"][0] for i in range(2)]
            self.assertTrue(all(f["hidden"] for f in own))
            self.assertNotEqual((own[0]["x"], own[0]["y"]), (own[1]["x"], own[1]["y"]))
            states, masks, actors, _ = battle.observe()
            self.assertEqual(actors.tolist(), [1, 1])
            np.testing.assert_array_equal(states[0], states[1])
            np.testing.assert_array_equal(masks[0], masks[1])
            left, right = (battle.state(i, viewer=1) for i in range(2))
            self.assertEqual(left, right)
            screen = state_screen(left, 1)
            self.assertEqual(screen, state_screen(right, 1))
            self.assertIn("Плут · скрыт", screen)

    def test_monk_topple_standing_consumes_movement_and_ranged_attack_uses_ammo(self):
        history, _ = self.assert_path(1, ("Monk", "Ranger"), [5, 5], MONK_SCRIPT)
        toppling = next(index for index, d in enumerate(history) if d.canonical == "Open Hand Technique: Topple")
        self.assertFalse(history[toppling].views[1]["fighters"][1]["prone"])
        self.assertTrue(history[toppling+1].views[1]["fighters"][1]["prone"])
        standing = next(index for index, d in enumerate(history) if d.actor == 1 and d.canonical == "Stand")
        stand, shoot, after_shot = (history[i].views[1] for i in range(standing, standing+3))
        self.assertEqual(history[standing+1].canonical, "Ranged attack")
        self.assertTrue(stand["fighters"][1]["prone"])
        self.assertFalse(shoot["fighters"][1]["prone"])
        self.assertEqual(shoot["movement_left"], stand["movement_left"] - 3)
        self.assertEqual(after_shot["fighters"][1]["arrows"], shoot["fighters"][1]["arrows"] - 1)

    def test_caster_chooses_slot_rank_and_an_elevated_cell(self):
        history, _ = self.assert_path(1, ("Wizard", "Fighter"), [5, 5], caster_script)
        slot = next(index for index, d in enumerate(history) if d.canonical == "Cast Shatter with level 3 slot")
        altitude = next(index for index, d in enumerate(history) if d.views[0]["phase"] == 19)
        aimed = history[altitude].views[0]
        before, resolved = (history[i].views[0] for i in range(max(slot, altitude), max(slot, altitude) + 2))
        self.assertEqual(len(aimed["target_cell"]), 2)
        self.assertGreater(history[altitude].action, 0)
        self.assertTrue(aimed["target_effects"][history[altitude].action]["foe_only"])
        self.assertEqual(before["fighters"][0]["hp"], resolved["fighters"][0]["hp"])
        self.assertEqual(resolved["fighters"][0]["slots"][2], before["fighters"][0]["slots"][2] - 1)
        self.assertLess(resolved["fighters"][1]["hp"], before["fighters"][1]["hp"])

    def test_independent_companion_form_cell_and_flight_have_the_same_decisions(self):
        history, _ = self.assert_path(1, ("Druid", "Fighter"), [5, 5], companion_script)
        placement = next(d for d in history if d.canonical == "Choose column 2")
        self.assertEqual(placement.views[0]["target_row"], 3)
        flight = next(index for index, d in enumerate(history) if d.canonical == "Companion ascend")
        before, after = (history[i].views[0]["entities"][0] for i in range(flight, flight+2))
        self.assertEqual(before["name"], "Owl familiar")
        self.assertEqual((before["x"], before["y"], before["altitude"]), (1, 3, 0))
        self.assertEqual(after["altitude"], 1)
        for viewer in (0, 1):
            owl = history[flight + 1].views[viewer]["entities"][0]
            self.assertEqual((owl["id"], owl["target_id"], owl["footprint"]), (2, 2 + viewer, 1))
        self.assertIn("Сова-фамильяр", state_screen(history[flight+1].views[0], 0))

    def test_split_magic_missile_shield_preserves_the_other_recipient_damage(self):
        history, _ = self.assert_path(1, ("Wizard", "Wizard"), [3, 3], split_missile_script)
        source = history[0].actor
        receiver = 1 - source
        protection = next(i for i, d in enumerate(history) if d.canonical == "Cast Shield using a level 1 slot")
        before = history[protection].views[receiver]
        after = history[protection+1].views[receiver]
        self.assertEqual(before["phase"], 81)
        self.assertIn("летит снарядов 2", pending_description(before))
        self.assertEqual(after["fighters"][receiver]["hp"], before["fighters"][receiver]["hp"])
        self.assertEqual(after["fighters"][receiver]["slots"][0], before["fighters"][receiver]["slots"][0] - 1)
        self.assertLess(after["fighters"][source]["hp"], before["fighters"][source]["hp"])

    def test_magic_missile_numeric_target_ids_are_relative_for_both_seats(self):
        seed = 1
        classes = class_ids(("Wizard", "Wizard"))
        levels = [3, 3]
        aim_menu = {
            0: "Aim one projectile at yourself",
            1: "Aim one projectile at the opposing adventurer",
            16: "Aim all remaining darts at yourself",
            17: "Aim all remaining darts at the opposing adventurer",
        }
        localized_menu = (
            "Один снаряд в себя", "Один снаряд в соперника",
            "Все оставшиеся снаряды в себя", "Все оставшиеся снаряды в соперника",
        )
        for caster in (0, 1):
            for aims, dart_counts in (((0, 17), (1, 2)), ((1, 16), (2, 1))):
                with self.subTest(caster=caster, aims=aims):
                    def choose(battle, state, options, history):
                        phase = state["phase"]
                        previous = sum(d.views[d.actor]["phase"] == phase for d in history)
                        if phase == 80 and previous < len(aims):
                            return aims[previous]
                        if phase == 81 and previous == 0:
                            return next(a for a, label in options.items() if label == "Let the darts strike")
                        if not any(d.canonical == "Magic Missile" for d in history):
                            label = "Magic Missile" if state["actor"] == caster else "End turn"
                            return next(a for a, name in options.items() if name == label)
                        if phase == 38 and history[-1].canonical == "Magic Missile":
                            return next(a for a, label in options.items() if label == "Cast Magic Missile with level 1 slot")
                        return baseline_action(battle, state, options, history)

                    history, final = native_trace(seed, classes, levels, choose)
                    self.assert_gym_parity(seed, classes, levels, history, final, caster)
                    record, output = self.assert_terminal_parity(seed, classes, levels, history, final, caster)
                    cast = next(d for d in history if d.canonical == "Magic Missile")
                    targeting = [d for d in history if d.views[caster]["phase"] == 80][:2]
                    self.assertEqual([d.action for d in targeting], list(aims))
                    for decision in targeting:
                        fighters = decision.views[caster]["fighters"]
                        self.assertEqual(fighters[caster]["target_id"], 0)
                        self.assertEqual(fighters[1 - caster]["target_id"], 1)
                        for action, label in aim_menu.items():
                            self.assertTrue(decision.mask[action])
                            self.assertEqual(decision.views[caster]["actions"][action], label)
                        self.assertFalse(decision.mask[2])  # There is no companion to target.
                    for ordinal, label in enumerate(localized_menu, 1):
                        self.assertIn(f"  {ordinal:>2}  {label}", output)
                    self.assertIn(f"хиты 23 → {23 - 4 * dart_counts[0]}", output)
                    self.assertIn("ячейки 1 ур. 4 → 3", output)

                    aimed = [f for f in record["frames"] if f["before"]["phase"] == 80][:2]
                    self.assertEqual([f["action"] for f in aimed], list(aims))
                    shield = next(i for i, d in enumerate(history) if d.views[d.actor]["phase"] == 81)
                    self.assertEqual(history[shield].actor, 1 - caster)
                    for target, count in zip((caster, 1 - caster), dart_counts):
                        before = cast.views[target]["fighters"][target]
                        after = history[shield + 1].views[target]["fighters"][target]
                        self.assertEqual((before["hp"], after["hp"]), (23, 23 - 4 * count))
                        self.assertEqual(before["slots"], [4, 2, 0])
                        self.assertEqual(after["slots"], [3 if target == caster else 4, 2, 0])

    def test_guessed_poison_spray_choices_match_across_native_gym_and_terminal(self):
        history, _ = self.assert_path(1, ("Warlock", "Fighter"), [3, 3], poison_guess_script)
        row = next(index for index, decision in enumerate(history) if decision.views[0]["phase"] == 82)
        guess = history[row:row + 3]
        self.assertEqual([decision.views[0]["phase"] for decision in guess], [82, 83, 84])
        self.assertEqual(guess[1].views[0]["target_row"], guess[0].action)
        self.assertIn("Предполагаемая цель · клетка", pending_description(guess[2].views[0]))

    def test_suggestion_sets_a_destination_height_and_flies_the_target_on_its_turn(self):
        history, _ = self.assert_path(1, ("Warlock", "Sorcerer"), [5, 5], ELEVATED_SUGGESTION_SCRIPT)
        height = next(i for i, decision in enumerate(history) if decision.views[0]["phase"] == 117)
        self.assertEqual(history[height].action, 1)
        self.assertEqual(pending_description(history[height].views[0]), "Внушение · клетка I4")
        self.assertFalse(history[height].views[1]["actions"])
        self.assertEqual(history[height + 1].views[0]["fighters"][0]["slots"][2],
                         history[height].views[0]["fighters"][0]["slots"][2] - 1)
        ascent = next(i for i, decision in enumerate(history) if decision.canonical == "Compelled move to elevation 5 ft")
        self.assertEqual(history[ascent].actor, 1)
        self.assertEqual(history[ascent].views[1]["fighters"][1]["altitude"], 0)
        self.assertEqual(history[ascent + 1].views[1]["fighters"][1]["altitude"], 1)
        self.assertEqual(history[ascent + 1].canonical, "Finish compelled movement")

    def test_existing_public_replay_plays_without_an_engine_or_model(self):
        root = Path(__file__).resolve().parents[2]
        for name, impact in (("wizard-vs-policy.json", "клетка D3"),
                             ("evoker-vs-trained-policy.json", "клетка F2")):
            with self.subTest(recording=name):
                output = StringIO()
                with redirect_stdout(output):
                    self.assertEqual(main(["--replay", str(root / "examples/replays" / name)]), 0)
                self.assertIn(f"Огненный шар → {impact}", output.getvalue())
                self.assertIn("Победитель", output.getvalue())

    def test_visible_objects_and_projectile_prompts_use_native_data(self):
        with Batch(1) as battle:
            fighters = class_ids(("Fighter", "Fighter"))
            battle.reset([0], [1], [fighters], [[3, 3]])
            state = battle.state()
            actor = state["actor"]
            drop = next(action for action, label in state["actions"].items() if label.startswith("Drop melee weapon"))
            battle.step([drop])
            visible = battle.state(viewer=actor)
            dropped = next(obj for obj in visible["objects"] if obj["name"] == "Longsword")
            self.assertEqual(dropped["target_id"], dropped["id"] ^ actor)
            screen = state_screen(visible, actor)
            self.assertIn("Предмет · длинный меч", screen)
            self.assertIn("▪ предмет", screen)

            wizards = class_ids(("Wizard", "Wizard"))
            battle.reset([0], [1], [wizards], [[3, 3]])
            state = battle.state()
            shooter = state["actor"]
            target = 1 - shooter
            missile = next(action for action, label in state["actions"].items() if label == "Magic Missile")
            battle.step([missile])
            tier = battle.state(viewer=shooter)
            self.assertEqual(tier["phase"], 38)
            rank_one = next(action for action, label in tier["actions"].items() if label.endswith("level 1 slot"))
            battle.step([rank_one])
            for remaining in (3, 2, 1):
                aim = battle.state(viewer=shooter)
                self.assertEqual(aim["phase"], 80)
                self.assertIn(f"осталось снарядов {remaining}", pending_description(aim))
                battle.step([1])  # Projectile aim selects the foe relative to the shooter.
            ward = battle.state(viewer=target)
            self.assertEqual(ward["phase"], 81)
            self.assertIn("летит снарядов 3", pending_description(ward))
            self.assertFalse(battle.state(viewer=shooter)["actions"])

    def test_checkpoint_fixture_and_user_replay_entry_points(self):
        classes, levels, seed = class_ids(("Fighter", "Fighter")), [5, 5], 3
        with Batch(1) as battle:
            battle.reset([0], [seed], [classes], [levels])
            human = 1 - int(battle.observe()[2][0])
        with TemporaryDirectory() as directory:
            checkpoint = Path(directory) / "deterministic-test-weights.npz"
            recording = Path(directory) / "test-fight.json"
            metadata = json.dumps({"checkpoint_version": CHECKPOINT_VERSION, "arena_abi": ARENA_ABI,
                                   "stage": "mechanics_fixture"})
            action_bias = np.zeros(ACTIONS, np.float32)
            action_bias[8] = 1.0  # The test opponent ends its turn when legal.
            np.savez(checkpoint, metadata=metadata,
                     w1=np.zeros((FEATURES, 1)), b1=np.zeros(1),
                     w2=np.zeros((1, 1)), b2=np.zeros(1),
                     w_policy=np.zeros((1, ACTIONS)), b_policy=action_bias,
                     w_value=np.zeros((1, 1)), b_value=np.zeros(1))
            output = StringIO()
            with patch("sys.stdin", StringIO("1\nq\n")), redirect_stdout(output):
                status = main(["--class", "fighter", "--opponent", "fighter", "--subclass", "1", "--opponent-subclass", "1",
                               "--seat", ("west", "east")[human], "--seed", str(seed),
                               "--checkpoint", str(checkpoint), "--save", str(recording)])
            self.assertEqual(status, 0)
            record = json.loads(recording.read_text())
            self.assertTrue(record["interrupted"])
            self.assertEqual(record["selection"]["builds"], [12, 12])
            self.assertEqual(record["arena_abi"], ARENA_ABI)
            self.assertEqual(record["checkpoint"]["arena_abi"], ARENA_ABI)
            self.assertTrue(any(frame["actor"] != human for frame in record["frames"]))
            self.assertEqual([frame["actor"] for frame in record["frames"] if "action" in frame], [human])
            displayed = StringIO()
            with redirect_stdout(displayed):
                self.assertEqual(main(["--replay", str(recording)]), 0)
            self.assertIn(record["result"], displayed.getvalue())
