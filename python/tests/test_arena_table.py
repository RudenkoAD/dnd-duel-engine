"""Regression coverage for the seat-relative, mouse-operated table."""

import unittest
from collections import OrderedDict
from types import SimpleNamespace

from arena.policy import Policy
from arena_table import Duel
from arena_web import Handler


class TableTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.policy = Policy.load()

    def game(self, **kwargs):
        d = Duel(self.policy, **dict({"seed": 11}, **kwargs))
        self.addCleanup(d.batch.close)
        return d

    def test_all_level_five_builds_expose_only_the_fixed_seat_mask(self):
        for player in range(12):
            for variant in range(3):
                d = self.game(player=player, player_variant=variant, seat=variant % 2)
                me = d.state["fighters"][d.seat]
                self.assertEqual((me["build_id"], me["level"]), (3 * player + variant, 5))
                v = d.view()
                self.assertEqual(
                    {a["id"] for a in v["choices"]}, set(d.state["actions"])
                )
                if v["actor"] != v["seat"]:
                    self.assertFalse(v["columns"] or v["book"] or v["routes"])
                self.assertNotIn("slots", v["pieces"][1 - d.seat])

    def test_catalog_and_east_seat_keep_the_selected_subclass(self):
        response = []
        Handler.do_GET(SimpleNamespace(path="/api/roster", reply=response.append))
        catalog, = response
        self.assertEqual([b["id"] for b in catalog], list(range(36)))
        self.assertEqual({b["level"] for b in catalog}, {5})
        self.assertEqual(len({b["title"] for b in catalog}), 36)
        duel = self.game(player=11, player_variant=2, opponent=4, opponent_variant=1, seat=1)
        west, east = duel.view()["pieces"][:2]
        self.assertEqual((west["subclass"], east["subclass"]), (catalog[13]["title"], catalog[35]["title"]))

    def test_new_duels_require_level_five_and_valid_subclasses(self):
        for fields in (dict(level=4), dict(enemy_level=2), dict(player_variant=3), dict(opponent_variant=-1)):
            with self.subTest(fields=fields), self.assertRaises(ValueError):
                self.game(**fields)

    def test_clicking_ground_matches_two_axis_choices_and_rejects_invalid_cells(self):
        mouse, axes = self.game(), self.game()
        for d in (mouse, axes):
            d.play(30)
        row, col = 3, 8
        self.assertTrue(mouse.view()["columns"][row] & (1 << col))
        with self.assertRaises(ValueError):
            mouse.play(row, 9)
        self.assertEqual(mouse.rev, 1)
        mouse.play(row, col)
        axes.play(row)
        axes.play(col)
        self.assertEqual(mouse.state, axes.state)
        self.assertEqual(mouse.view()["center"], [col, row])
        self.assertEqual(mouse.view()["phase"], 19)
        self.assertTrue(int(mouse.view()["slices"][0], 16) & (1 << (9 * row + col)))
        mouse.play(0)
        self.assertEqual(mouse.view()["pieces"][0]["slots"][2], 1)

    def test_darts_and_east_seat_use_relative_targets(self):
        for seat in (0, 1):
            d = self.game(seat=seat)
            if d.state["actor"] != seat:
                self.pass_bot(d)
            d.play(27)
            d.play(0)
            v = d.view()
            self.assertEqual(v["phase"], 80)
            variants = {a["id"]: a["targets"] for a in v["choices"]}
            self.assertEqual(variants[1], [1])
            self.assertEqual(variants[17], [1])
            self.assertEqual(v["pieces"][1 - seat]["target_id"], 1)
            d.play(1)
            self.assertEqual(d.state["phase"], 80)

    @staticmethod
    def pass_bot(d):
        d.step(8, bot_main=True)

    def test_familiar_spawn_flight_and_activation(self):
        d = self.game(player=3, seed=1)
        self.pass_bot(d)

        def choose(label):
            d.play(next(a for a, name in d.state["actions"].items() if name == label))

        choose("Stow melee weapon (free interaction)")
        choose("Wild Companion (Wild Shape)")
        choose("Owl familiar")
        d.play(3, 1)
        for _ in range(8):
            if d.state["phase"] == 53:
                break
            if d.state["actor"] == d.seat:
                choose("End turn")
            else:
                self.pass_bot(d)
        v = d.view()
        owl = next(p for p in v["pieces"] if p["kind"] == "companion")
        self.assertEqual((owl["x"], owl["y"]), (1, 3))
        self.assertTrue(owl["active"])
        choose("Companion ascend")
        self.assertEqual(d.state["entities"][0]["altitude"], 1)

    def test_hidden_paths_are_indistinguishable(self):
        visible = []
        for direction in (1, 3):
            d = self.game(player=4, opponent=8, seat=1, seed=308)
            for action in (109, 111, 88, direction, 8):
                d.step(action, bot_main=True)
            visible.append(d.view())
        self.assertEqual(*visible)
        self.assertFalse(visible[0]["cover"])
        self.assertFalse(visible[0]["pieces"][0]["known"])
        self.assertEqual(visible[0]["pieces"][0]["x"], -1)

    def test_route_dispatch_rejects_duplicate_revision(self):
        d = self.game()
        handler = SimpleNamespace(
            path="/api/walk",
            server=SimpleNamespace(games=OrderedDict(game=d), model="test"),
        )
        dest, path = next(iter(d.view()["routes"].items()))
        body = {"game": "game", "rev": 0, "destination": dest}
        data, code = Handler.dispatch(handler, body)
        self.assertEqual(code, 200)
        self.assertEqual(d.rev, len(path["cells"]))
        self.assertEqual(data["view"]["budget"]["move"], 30 - path["feet"])
        old = d.rev
        self.assertEqual(Handler.dispatch(handler, body)[1], 409)
        self.assertEqual(d.rev, old)
