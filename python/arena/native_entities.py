"""Read viewer-safe summoned creatures from the arena ABI."""
import ctypes as ct

import numpy as np


_FIELDS = (
    "present", "id", "side", "kind", "spirit", "x", "y", "altitude", "hp", "max_hp",
    "ac", "initiative", "reaction", "dodging", "spirit_power", "mounted", "active",
    "movement_left", "action_available", "bonus_available", "flyby", "exhaustion", "temp_hp",
)


def configure_entity_library(lib):
    """Bind the entity projections before the arena library is used."""
    ptr, size = ct.c_void_p, ct.c_size_t
    lib.arena_entities.argtypes = [ptr, size, size, ptr, size]
    lib.arena_entities.restype = ct.c_bool
    lib.arena_entity_name.argtypes = [ptr, size, size, size, ptr, size]
    lib.arena_entity_name.restype = ct.c_bool


def read_entities(lib, handle, index, viewer):
    """Return entities visible to an explicit seat, own side first."""
    raw = np.empty((2, len(_FIELDS)), dtype=np.int32)
    if not lib.arena_entities(handle, index, viewer, raw.ctypes.data, raw.size):
        raise RuntimeError("failed reading arena entities")
    entities = []
    for row in raw:
        if not row[0]:
            continue
        entity = dict(zip(_FIELDS, row.tolist()))
        identifier = entity["id"]
        label = ct.create_string_buffer(64)
        if not lib.arena_entity_name(handle, index, viewer, identifier, label, len(label)):
            raise RuntimeError("failed reading arena entity name")
        entity["name"] = label.value.decode()
        if entity.pop("present") == 2:
            contact = {key: entity[key] for key in ("id", "side", "spirit", "x", "y", "name")}
            entities.append({**contact, "detected": True})
            continue
        for key in ("movement_left", "action_available", "bonus_available"):
            if entity[key] < 0:
                entity[key] = None
        entities.append(entity)
    return entities
