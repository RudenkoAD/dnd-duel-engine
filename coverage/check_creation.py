#!/usr/bin/env python3
"""Check that the fixed prepared-spell ledger fills every SRD slot."""

from pathlib import Path
import re
import sys

LEDGER = Path(__file__).resolve().parents[1] / "docs" / "coverage" / "creation-ledger.md"
FULL = (4, 5, 6, 7, 9)
HALF = (2, 3, 4, 5, 6)
QUOTAS = {
    "Bard": ((2, 2, 2, 3, 3), FULL),
    "Cleric": ((3, 3, 3, 4, 4), FULL),
    "Druid": ((2, 2, 2, 3, 3), FULL),
    "Paladin": ((0, 0, 0, 0, 0), HALF),
    "Ranger": ((0, 0, 0, 0, 0), HALF),
    "Sorcerer": ((4, 4, 4, 5, 5), (2, 4, 6, 7, 9)),
    "Warlock": ((2, 2, 2, 3, 3), HALF),
    "Wizard": ((3, 3, 3, 4, 4), FULL),
}
PROMOTIONS = {
    ("Cleric", 3): {"Cure Wounds", "Bless"},
    ("Paladin", 2): {"Divine Smite"},
    ("Sorcerer", 3): {"Chromatic Orb"},
}


def markdown_table(doc, heading):
    part = doc.split(f"### {heading}\n", 1)[1].split("\n##", 1)[0]
    rows = {}
    for line in part.splitlines():
        if not line.startswith("|"):
            continue
        name, *cells = (cell.strip() for cell in line.strip("|").split("|"))
        if name in QUOTAS:
            if name in rows:
                raise ValueError(f"{heading}: duplicate {name} row")
            rows[name] = cells
    return rows


def add_names(cumulative, additions, name, level):
    picks = [pick.strip() for pick in additions.split(",")]
    if not all(picks) or len(set(picks)) != len(picks):
        raise ValueError(f"{name} at {level}: duplicate or empty choices {picks}")
    if cumulative.intersection(picks):
        raise ValueError(f"{name} at {level}: choice already prepared")
    cumulative.update(picks)


def check(doc):
    prepared = markdown_table(doc, "Base prepared progression")
    cantrips = markdown_table(doc, "Cantrip selections")
    if set(prepared) != set(QUOTAS) or set(cantrips) != set(QUOTAS) - {"Paladin", "Ranger"}:
        raise ValueError("prepared class or cantrip roster differs from the expected SRD classes")
    spells = 0
    for class_name, (cantrip_count, prepared_count) in QUOTAS.items():
        chosen_cantrips = set()
        chosen_spells = set()
        for level, cell in enumerate(prepared[class_name], 1):
            if class_name in cantrips and level in (1, 4):
                cantrip_cell = cantrips[class_name][int(level == 4)]
                add_names(chosen_cantrips, cantrip_cell, class_name, level)
            promotion = PROMOTIONS.get((class_name, level), set())
            if not promotion <= chosen_spells:
                raise ValueError(f"{class_name} at {level}: promoted spell missing in preceding level")
            chosen_spells -= promotion
            if level > 1:
                added = re.search(r"(?:^|\. )\+(.+)$", cell)
                if not added:
                    raise ValueError(f"{class_name} at {level}: missing addition marker")
                cell = added.group(1)
            add_names(chosen_spells, cell, class_name, level)
            if len(chosen_cantrips) != cantrip_count[level - 1]:
                raise ValueError(f"{class_name} at {level}: wrong cantrip count")
            if len(chosen_spells) != prepared_count[level - 1]:
                raise ValueError(f"{class_name} at {level}: wrong prepared spell count")
        if len(prepared[class_name]) != 5:
            raise ValueError(f"{class_name}: expected one choice cell per level")
        spells += len(chosen_spells)
    return spells


if __name__ == "__main__":
    try:
        count = check(LEDGER.read_text(encoding="utf-8"))
    except (IndexError, OSError, ValueError) as error:
        print(f"{LEDGER}: {error}", file=sys.stderr)
        sys.exit(1)
    print(f"Cantrip/leveled-spell quotas verified across 40 profiles; {count} base choices at level 5.")
