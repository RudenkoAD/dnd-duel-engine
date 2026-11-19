"""In-process batch of independent Rust fights. One driver per batch."""
import ctypes as ct
from functools import lru_cache
from pathlib import Path
import sys

import numpy as np

from .native_entities import configure_entity_library, read_entities

ARENA_ABI = 2002
FEATURES = 1024
ACTIONS = 128
CLASS_NAMES = ("Barbarian", "Bard", "Cleric", "Druid", "Fighter", "Monk", "Paladin",
               "Ranger", "Rogue", "Sorcerer", "Warlock", "Wizard")
ARENA_NAMES = ("Мшистая цитадель", "Янтарная застава", "Серебряный бастион",
               "Туманные врата", "Тёмный бастион")
VARIANTS_PER_CLASS = 3
ROSTER_SIZE = len(CLASS_NAMES) * VARIANTS_PER_CLASS
ROSTER_LEVEL = 5
ROSTER_NEW_INPUTS = (*range(108, 132), 599, *range(916, 920))
ACTION_NAMES = ("N", "NE", "E", "SE", "S", "SW", "W", "NW", "End", "Melee", "Ranged",
                "Dash", "Disengage", "Dodge", "Opportunity", "Pass", "Defend", "Rider",
                *(f"Special{i}" for i in range(1, 9)), *(f"Spell{i}" for i in range(6)))
ACTION_NAMES += (
    "Hide", "Search", "Ready", "Grapple", "ShovePush", "ShoveProne", "Stand", "EscapeAthletics",
    "EscapeAcrobatics", "ReleaseGrapple", "DropProne", "Utilize", "Study", "Help", "Influence",
    "Interact", "JumpLong", "JumpHigh", "Climb", "DropConcentration", "Unarmed", "Drag", "HoldBreath",
    *(f"OtherCommon{i}" for i in range(1, 10)), *(f"Magic{i}" for i in range(1, 25)),
    *(f"Martial{i}" for i in range(1, 9)), *(f"Entity{i}" for i in range(1, 13)),
    *(f"Gear{i}" for i in range(1, 13)), *(f"Other{i}" for i in range(1, 9)),
)
ROOT = Path(__file__).resolve().parents[2]
_HEADER = ("actor", "owner", "turns", "phase", "winner", "movement_left", "action_available",
           "bonus_available", "attacks_left", "moved", "disengaged", "layout")
_FIELDS = ("class_id", "level", "hp", "max_hp", "temp", "ac", "x", "y", "resource", "secondary",
           "slot1", "slot2", "slot3", "reaction", "dodge", "shield", "sap", "slow", "stunned",
           "stun_slow", "stun_exposed", "advantage", "disadvantage", "speed_zero", "raging",
           "reckless", "concentrating", "speed", "melee_die", "ranged_die", "proficiency", "casting",
           "arrows", "caltrops_x", "caltrops_y")


@lru_cache(maxsize=1)
def _lib():
    fname = {"darwin": "libdnd_duel_engine.dylib", "linux": "libdnd_duel_engine.so",
             "win32": "dnd_duel_engine.dll"}[sys.platform]
    lib = ct.CDLL(str(ROOT / "target" / "release" / fname))
    ptr, size, integer = ct.c_void_p, ct.c_size_t, ct.c_int32
    lib.arena_schema.restype = ct.c_uint32
    if lib.arena_schema() != ARENA_ABI:
        raise RuntimeError("Rebuild native arena with make release")
    if not hasattr(lib, "arena_features") or (lib.arena_features(),lib.arena_actions()) != (FEATURES,ACTIONS):
        raise RuntimeError("Native observation dimensions differ. Rebuild the arena with make release.")
    for name, args, ret in (
        ("arena_schema", [], ct.c_uint32), ("arena_new", [size], ptr), ("arena_free", [ptr], None),
        ("arena_reset", [ptr, size, ptr, ptr, ptr, ptr], ct.c_bool),
        ("arena_observe", [ptr, ptr, ptr, ptr, ptr], None), ("arena_step", [ptr, ptr], integer),
        ("arena_advance", [ptr, ptr, ct.c_bool], ct.c_bool),
        ("arena_choices", [ptr, ct.c_bool, ptr], None), ("arena_raw", [ptr, size, integer, ptr, ptr], ct.c_bool),
        ("arena_text", [ptr, size, integer, integer, ptr, size], ct.c_bool),
        ("arena_view", [ptr, size, integer, ptr, ptr], ct.c_bool),
        ("arena_results", [ptr,ptr], None),
    ):
        func = getattr(lib, name)
        func.argtypes, func.restype = args, ret
    configure_entity_library(lib)
    for name, args, ret in (
        ("arena_roster_size", [], size),
        ("arena_roster_text", [size, ct.c_bool, ptr, size], ct.c_bool),
        ("arena_reset_builds", [ptr, size, ptr, ptr, ptr, ptr], ct.c_bool),
        ("arena_layout_count", [], size),
    ):
        func = getattr(lib, name, None)
        if func is not None:
            func.argtypes, func.restype = args, ret
    return lib


def _roster_library():
    lib = _lib()
    if not hasattr(lib, "arena_roster_size") or lib.arena_roster_size() != ROSTER_SIZE:
        raise RuntimeError("Rebuild native arena with make release to access the roster")
    return lib


def arena_count():
    """Number of layouts available for explicit roster fixtures."""
    return _roster_library().arena_layout_count()


@lru_cache(maxsize=1)
def _roster_text():
    lib = _roster_library()
    buf = ct.create_string_buffer(4096)
    rows = []
    for build_id in range(ROSTER_SIZE):
        values = []
        for description in (False, True):
            if not lib.arena_roster_text(build_id, description, buf, len(buf)):
                raise RuntimeError(f"failed reading roster entry {build_id}")
            values.append(buf.value.decode())
        rows.append(tuple(values))
    return tuple(rows)


def roster():
    """Return level-five preset metadata in stable build-ID order."""
    entries = []
    for build_id, (name, description) in enumerate(_roster_text()):
        class_id, variant = divmod(build_id, VARIANTS_PER_CLASS)
        entries.append(dict(id=build_id, class_id=class_id, variant=variant,
                            class_name=CLASS_NAMES[class_id], level=ROSTER_LEVEL,
                            name=name, description=description))
    return entries


def _integers(values, shape, low, high, dtype):
    # Preserve large u64 seeds in heterogeneous Python-int lists. The default
    # NumPy promotion may otherwise round them through a float64.
    if dtype == np.uint64 and not isinstance(values, np.ndarray):
        if any(not isinstance(v, (int, np.integer)) or not 0 <= int(v) < 2**64 for v in values):
            raise ValueError("seed must fit u64")
        array = np.asarray(values, dtype=dtype)
    else:
        array = np.asarray(values)
    if array.shape != shape or array.dtype.kind not in "iu":
        raise ValueError(f"expected integer array shaped {shape}, got {array.shape}, {array.dtype}")
    if array.size and (int(array.min()) < low or int(array.max()) > high):
        raise ValueError(f"value outside [{low}, {high}]")
    return np.ascontiguousarray(array, dtype=dtype)


class Batch:
    def __init__(self, n):
        if not isinstance(n, (int, np.integer)) or not 1 <= n <= 65536:
            raise ValueError("n must be in 1..65536")
        self.n = int(n)
        self._lib = _lib()
        self._handle = self._lib.arena_new(self.n)

    def _open(self):
        if not self._handle:
            raise RuntimeError("arena batch is closed")
        return self._handle

    def reset(self, indices, seeds, classes, levels):
        k = len(indices)
        if not k:
            return
        idx = _integers(indices, (k,), 0, self.n - 1, np.int32)
        seeds = _integers(seeds, (k,), 0, 2**64 - 1, np.uint64)
        classes = _integers(classes, (k, 2), 0, 11, np.int32)
        levels = _integers(levels, (k, 2), 1, 5, np.int32)
        if not self._lib.arena_reset(self._open(), k, idx.ctypes.data, seeds.ctypes.data,
                                     classes.ctypes.data, levels.ctypes.data):
            raise ValueError("invalid reset parameters")

    def reset_builds(self, seeds, builds, arenas=None, *, indices=None):
        """Reset roster fights in seat order, optionally selecting lanes and layouts.

        By default, reset every lane and let the seed choose its layout.
        Supply an arena ID per selected lane to choose layouts independently
        of the combat RNG seed.
        """
        if indices is None:
            indices = np.arange(self.n, dtype=np.int32)
        k = len(indices)
        if not k:
            return
        idx = _integers(indices, (k,), 0, self.n - 1, np.int32)
        seeds = _integers(seeds, (k,), 0, 2**64 - 1, np.uint64)
        builds = _integers(builds, (k, 2), 0, ROSTER_SIZE - 1, np.uint8)
        lib = _roster_library()
        if arenas is not None:
            arenas = _integers(arenas, (k,), 0, lib.arena_layout_count() - 1, np.int32)
        if not lib.arena_reset_builds(self._open(), k, idx.ctypes.data, seeds.ctypes.data,
                                      builds.ctypes.data, None if arenas is None else arenas.ctypes.data):
            raise ValueError("invalid roster reset parameters")

    def observe(self):
        x = np.empty((self.n, FEATURES), np.float32)
        mask = np.empty((self.n, ACTIONS), bool)
        actors = np.empty(self.n, np.int32)
        results = np.empty(self.n, np.int32)
        self._lib.arena_observe(self._open(), x.ctypes.data, mask.ctypes.data,
                                actors.ctypes.data, results.ctypes.data)
        return x, mask, actors, results

    def step(self, actions):
        a = _integers(actions, (self.n,), -1, ACTIONS - 1, np.int32)
        invalid = self._lib.arena_step(self._open(), a.ctypes.data)
        if invalid != -1:
            raise ValueError(f"illegal action {a[invalid]} in lane {invalid}")

    def advance(self, seats, expert=False):
        """Run baseline opponents until the designated seats decide. -1 skips a lane."""
        seats = _integers(seats, (self.n,), -1, 1, np.int32)
        if not self._lib.arena_advance(self._open(), seats.ctypes.data, bool(expert)):
            raise ValueError("invalid seats")

    def bot(self, expert=False):
        out = np.empty(self.n, np.int32)
        self._lib.arena_choices(self._open(), bool(expert), out.ctypes.data)
        return out

    def result_reasons(self):
        """0 ongoing, 1 defeat, 2 simultaneous knockout, 3 turn cap, 4 decision cap."""
        out = np.empty(self.n, np.int32)
        self._lib.arena_results(self._open(), out.ctypes.data)
        return out

    def _text(self, i, viewer, key):
        buf = ct.create_string_buffer(4096)
        if not self._lib.arena_text(self._open(), i, viewer, key, buf, len(buf)):
            raise RuntimeError("failed reading arena label")
        return buf.value.decode()

    def state(self, index=0, viewer=None):
        """Project the fight as seen by a seat. The default viewer is the current decision maker."""
        if not isinstance(index, (int, np.integer)) or not 0 <= index < self.n:
            raise IndexError(index)
        if viewer is not None and (not isinstance(viewer,(int,np.integer)) or viewer not in (0,1)):
            raise ValueError("viewer must be seat 0 or 1")
        native_viewer = -1 if viewer is None else int(viewer)
        raw = np.empty(224, np.int32)
        terrain = np.empty((7, 9), np.uint8)
        if not self._lib.arena_raw(self._open(), int(index), native_viewer, raw.ctypes.data, terrain.ctypes.data):
            raise IndexError(index)
        state = dict(zip(_HEADER, raw[:12].tolist()))
        state["layout_name"] = ARENA_NAMES[state["layout"]]
        if viewer is None:
            viewer = state["actor"] if state["actor"] >= 0 else state["owner"]
        viewer = int(viewer)
        fighters = []
        for i in range(2):
            f = dict(zip(_FIELDS, raw[12+i*35:47+i*35].tolist()))
            f["variant"] = int(raw[190 + i])
            f["build_id"] = f["class_id"] * VARIANTS_PER_CLASS + f["variant"]
            known, visible, altitude, prone, grappled, hidden = raw[85+6*i:91+6*i].tolist()
            f.update(known=bool(known), visible=bool(visible))
            if known:
                f["altitude"] = altitude
            if visible:
                f.update(prone=bool(prone), grappled=bool(grappled),
                         armor_state=raw[100+4*i:104+4*i].tolist(),body_armor=self._text(index,viewer,133+i))
            else:
                for field in ("hp", "temp", "ac", *_FIELDS[14:27]):
                    f.pop(field, None)
            slots = [f.pop(f"slot{k}") for k in (1, 2, 3)]
            if i == viewer:
                f.update(slots=slots, hidden=bool(hidden), stonecunning_uses=int(raw[108]), tremorsense=bool(raw[109]))
            else:
                for field in ("resource", "secondary", "reaction", "arrows"):
                    f.pop(field)
            f["equipment"] = self._text(index, viewer, 131+i)
            f["summary"] = self._text(index, viewer, 128+i)
            fighters.append(f)
        x = np.empty(FEATURES, np.float32)
        mask = np.empty(ACTIONS, bool)
        if not self._lib.arena_view(self._open(), index, viewer, x.ctypes.data, mask.ctypes.data):
            raise IndexError(index)
        state["target_action"] = int(raw[82]) if raw[82] >= 0 else None
        state["target_row"] = int(raw[83]) if raw[83] >= 0 else None
        state["target_cell"] = None
        if raw[97] >= 0:
            state.update(target_action=int(raw[97]), target_cell=raw[98:100].tolist(), target_row=int(raw[99]))
        effects = {}
        if state["target_action"] is not None:
            for axis in np.flatnonzero(mask):
                if axis >= 9:
                    continue
                offset = 160+8*axis if axis < 8 else 146
                coverage = x[offset+2:offset+6]
                effects[int(axis)] = dict(zip(("foe", "self", "foe_only", "both"), map(bool, coverage)))
        state["target_effects"] = effects
        hazards = [[f["caltrops_x"], f["caltrops_y"]] if f["caltrops_x"] >= 0 else None for f in fighters]
        reason = int(raw[84])
        objects = []
        for slot in range(10):
            present,xpos,ypos,zpos,ac,hp,burning,flammable = raw[110+slot*8:118+slot*8].tolist()
            if present:
                identity = 4 + slot
                objects.append(dict(id=identity, target_id=identity ^ viewer,
                                    name=self._text(index, viewer, 140 + slot), x=xpos, y=ypos,
                                    altitude=zpos, ac=ac, hp=hp, burning=bool(burning), flammable=bool(flammable)))
        entities=read_entities(self._lib, self._open(), index, viewer)
        visible_combatants = [*enumerate(fighters), *((e["id"], e) for e in entities)]
        for identity, combatant in visible_combatants:
            target_id = identity ^ viewer
            combatant["target_id"] = target_id
            offset = 952 + 6 * target_id
            footprint = x[offset:offset+6]
            if footprint[0]:
                combatant.update(footprint=int(round(2*float(footprint[0]))), size=int(round(5*float(footprint[1]))),
                                 squeezing=bool(footprint[2]), starry_wisp=bool(footprint[3]),
                                 guiding_bolt=bool(footprint[4]), frost_slow_feet=int(round(10*float(footprint[5]))))
        spell_fields = []
        for base in (819, 843):
            active, xf, yf, zf, extent, height = x[base:base+6]
            if active:
                spell_fields.append({
                    "x": int(round(8 * float(xf))),
                    "y": int(round(6 * float(yf))),
                    "altitude": int(round(6 * float(zf))),
                    "extent_feet": int(round(40 * float(extent))),
                    "height_feet": int(round(40 * float(height))),
                })
        state.update(viewer=viewer,objects=objects,spell_fields=spell_fields,
                     identified_spells=self._text(index,viewer,150),
                      result_reason=("ongoing","defeat","double_ko","timeout","timeout")[reason],
                     limit_reason={3:"turn_limit",4:"decision_limit"}.get(reason),
                     fighters=fighters, terrain=terrain.tolist(), caltrops=hazards,
                     entities=entities,
                     actions={int(a): self._text(index, viewer, int(a)) for a in np.flatnonzero(mask)},
                     pending=self._text(index, viewer, 130))
        return state

    def tabletop(self, viewer, index=0):
        """Optional fixed-seat geometry and spell metadata for the graphical table."""
        if viewer not in (0, 1) or not 0 <= index < self.n:
            raise ValueError("invalid viewer or lane")
        book, cover, columns, costs, slices, edges = (
            self._text(index, viewer, code) for code in range(151, 157)
        )
        return dict(book=[row.split("\t") for row in book.splitlines()], cover=cover,
                    columns=list(map(int, columns.split())), costs=list(map(int, costs.split())),
                    slices=slices.split(), edges=bytes.fromhex(edges))

    def close(self):
        if getattr(self, "_handle", None):
            self._lib.arena_free(self._handle)
            self._handle = None

    def __enter__(self):
        self._open()
        return self

    def __exit__(self, *args):
        self.close()

    def __del__(self):
        self.close()

    def __reduce__(self):
        raise TypeError("Batch owns its Rust handles")
