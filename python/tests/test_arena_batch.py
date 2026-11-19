import unittest
import numpy as np
from arena.native import ACTIONS, ARENA_NAMES, FEATURES, Batch, arena_count, roster


class ArenaBatchTests(unittest.TestCase):
    def test_roster_reset_preserves_canonical_and_encodes_both_variants(self):
        with Batch(2) as builds, Batch(1) as canonical:
            seed = 2**64 - 1
            builds.reset_builds([seed, seed], [[12, 33], [13, 35]])
            canonical.reset([0], [seed], [[4, 11]], [[5, 5]])
            for a, b in zip(builds.observe(), canonical.observe()):
                np.testing.assert_array_equal(a[:1], b)
            features, masks, actors, _ = builds.observe()
            expected = [1, 0, 0, 1] if actors[1] == 0 else [0, 1, 1, 0]
            np.testing.assert_array_equal(features[1, 916:920], expected)
            np.testing.assert_array_equal(features[:, 640:640 + ACTIONS].astype(bool), masks)
            self.assertEqual([f["build_id"] for f in builds.state(1)["fighters"]], [13, 35])
            self.assertEqual([f["level"] for f in builds.state(1)["fighters"]], [5, 5])
            for _ in range(801):
                for a, b in zip(builds.observe(), canonical.observe()):
                    np.testing.assert_array_equal(a[:1], b)
                action = canonical.bot(expert=True)[0]
                if action < 0:
                    break
                canonical.step([action])
                builds.step([action, -1])
            else:
                self.fail("canonical fixture did not finish")

    def test_all_roster_entries_and_explicit_layouts(self):
        entries = roster()
        self.assertEqual(len(entries), 36)
        self.assertEqual(len({(e["class_id"], e["name"]) for e in entries}), 36)
        self.assertTrue(all(e["description"] and e["level"] == 5 for e in entries))
        layouts = arena_count()
        self.assertEqual(layouts, len(ARENA_NAMES))
        with Batch(len(entries)) as batch:
            ids = np.arange(len(entries))
            batch.reset_builds(ids, np.stack((ids, ids[::-1]), axis=1), ids % layouts)
            for i, entry in enumerate(entries):
                state = batch.state(i)
                self.assertEqual(state["layout"], i % layouts)
                self.assertEqual(state["layout_name"], ARENA_NAMES[i % layouts])
                self.assertEqual(state["fighters"][0]["build_id"], entry["id"])
                self.assertEqual(state["fighters"][0]["class_id"], entry["class_id"])
                self.assertEqual(state["fighters"][0]["variant"], entry["variant"])
            old = batch.observe()
            with self.assertRaises(ValueError):
                batch.reset_builds([1, 2], [[0, 0], [0, 0]], [0, layouts], indices=[0, 1])
            for actual, expected in zip(batch.observe(), old):
                np.testing.assert_array_equal(actual, expected)
            for step in range(801):
                features, masks, actors, _ = batch.observe()
                actions = batch.bot(expert=bool(step % 2))
                active = np.flatnonzero(actors >= 0)
                if not len(active):
                    break
                self.assertTrue(np.isfinite(features).all())
                self.assertTrue((actions[active] >= 0).all())
                self.assertTrue(masks[active, actions[active]].all())
                batch.step(actions)
            else:
                self.fail("all roster fixtures should finish within the decision limit")

    def test_native_bots_handle_responses_over_suspended_entity_decisions(self):
        actions = (109, 96, 0, 6, 4, 0, 1, 1, 2, 2, 2, 8, 2, 2, 1, 0, 9, 1)
        for expert in (False, True):
            with self.subTest(expert=expert), Batch(1) as fast, Batch(1) as manual:
                for batch in (fast, manual):
                    batch.reset_builds([201615], [[20, 14]], [0])
                    for action in actions:
                        batch.step([action])
                _, mask, actor, _ = manual.observe()
                self.assertEqual(manual.state()["phase"], 195)
                self.assertEqual(actor[0], 1)
                choice = manual.bot(expert=expert)[0]
                self.assertGreaterEqual(choice, 0)
                self.assertTrue(mask[0, choice])
                while manual.observe()[2][0] == 1:
                    manual.step(manual.bot(expert=expert))
                fast.advance([0], expert=expert)
                for actual, expected in zip(fast.observe(), manual.observe()):
                    np.testing.assert_array_equal(actual, expected)

    def test_atomic_rejection_and_accelerated_opponent_yield(self):
        with Batch(2) as a, Batch(2) as b:
            setup = ([0, 1], [2**64-1, 31], [[1, 11], [5, 2]], [[5, 5], [5, 5]])
            for board in (a, b):
                board.reset(*setup)
            with self.assertRaises(ValueError):
                a.step([a.bot()[0], 15])  # invalid main-turn pass in the second lane
            for actual, expected in zip(a.observe(), b.observe()):
                np.testing.assert_array_equal(actual, expected)
            for _ in range(800):
                a.advance([0, 1])
                while True:
                    _, _, actors, _ = b.observe()
                    opponents = (actors >= 0) & (actors != [0, 1])
                    if not opponents.any():
                        break
                    b.step(np.where(opponents, b.bot(), -1))
                for actual, expected in zip(a.observe(), b.observe()):
                    np.testing.assert_array_equal(actual, expected)
                if np.all(a.observe()[2] < 0):
                    return
                actions = a.bot(expert=True)
                a.step(actions)
                b.step(actions)
            self.fail("two complete fights expected")

    def test_snapshot_layout_and_all_loadouts(self):
        with Batch(60) as b:
            ids = np.arange(60)
            classes = ids % 12
            levels = ids // 12 + 1
            b.reset(ids, ids + 9001, np.stack([classes, 11 - classes], 1),
                    np.stack([levels, levels], 1))
            x, masks, actors, results = b.observe()
            self.assertTrue(np.isfinite(x).all())
            self.assertEqual(x.shape, (60, FEATURES))
            np.testing.assert_array_equal(x[:, 640:640 + ACTIONS].astype(bool), masks)
            self.assertTrue((results == -1).all())
            for i in range(60):
                state = b.state(i)
                self.assertEqual(state["actor"], actors[i])
                self.assertEqual(state["fighters"][0]["level"], levels[i])
                self.assertEqual(state["fighters"][0]["class_id"], classes[i])
                self.assertEqual(state["caltrops"], [None, None])
                self.assertEqual(sorted(state["actions"]), np.flatnonzero(masks[i]).tolist())
                own = b.state(i, viewer=0)
                self.assertEqual(own["fighters"][0]["arrows"], 20 if classes[i] in (1, 7, 8) else 0)
                self.assertNotIn("arrows", own["fighters"][1])
                self.assertNotIn("slots", own["fighters"][1])
                if actors[i] != 0:
                    self.assertFalse(own["actions"])


    def test_target_features_match_public_legal_coordinates(self):
        with Batch(1) as b:
            for seed in range(64):
                b.reset([0], [seed], [[11, 4]], [[5, 5]])
                if b.state()["actor"] == 0 and b.observe()[0][0, 29]:
                    break
            fireball = 30
            before = b.state()["fighters"][0]["slots"][:]
            b.step([fireball])
            row = b.state()
            _, mask, _, _ = b.observe()
            self.assertEqual((row["phase"], row["target_action"], row["target_row"]), (5, fireball, None))
            self.assertEqual(set(row["target_effects"]), set(np.flatnonzero(mask[0])))
            y = next(k for k, e in row["target_effects"].items() if e["foe_only"])
            b.step([y])
            col = b.state()
            self.assertEqual((col["phase"], col["target_row"]), (6, y))
            self.assertEqual(col["fighters"][0]["slots"], before)
            selected = next(k for k, e in col["target_effects"].items() if e["foe_only"])
            self.assertEqual(col["actions"][selected], f"Cell {chr(65 + selected)}{y + 1}")
            old = [f["hp"] for f in col["fighters"]]
            b.step([selected])
            height = b.state(viewer=0)
            if height["phase"] == 19:
                self.assertEqual(height["target_cell"], [selected, y])
                self.assertEqual(height["fighters"][0]["slots"], before)
                b.step([next(k for k, e in height["target_effects"].items() if e["foe_only"])])
            resolved = b.state(viewer=0)
            self.assertEqual(resolved["fighters"][0]["hp"], old[0])
            self.assertLess(resolved["fighters"][1]["hp"], old[1])
            self.assertEqual(resolved["fighters"][0]["slots"][2], before[2]-1)
