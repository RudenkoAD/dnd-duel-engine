#!/usr/bin/env python3
"""Check coverage matrix structure, traceability, and release readiness."""

import argparse
from collections import Counter
import csv
from functools import lru_cache
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
COLUMNS = ("id", "build", "levels", "rule_source", "trigger", "cost", "choices",
           "implementation", "test", "status")
EXPECTED_GRANTS = {
    "barbarian": 11, "bard": 9, "cleric": 9, "druid": 10, "fighter": 11, "monk": 12,
    "paladin": 12, "ranger": 10, "rogue": 12, "sorcerer": 9, "warlock": 7, "wizard": 9,
}
EXPECTED_CATALOG = {"grant": 121, "prepared": 30, "common": 49, "creation": 12}
STATUS = {
    "verified": "verified", "pass": "verified",
    "implemented": "gap",
    "not_applicable": "not_applicable", "out_of_scope": "not_applicable", "out-of-scope": "not_applicable",
    "n/a": "not_applicable", "out_of_duel": "not_applicable",
    "unreachable": "not_applicable", "arena rule": "not_applicable",
    "creation_choice": "creation_choice",
    "pending": "gap", "pending_integration": "gap", "open": "gap",
    "owned by casters": "gap", "owned by casters/entities": "gap",
    "gap": "gap", "planned": "gap", "missing": "gap", "partial": "gap", "unverified": "gap",
}
LEVELS = re.compile(r"([1-5])(?:[-–]([1-5]))?\Z")
VALID_ID = re.compile(r"[a-z][a-z0-9_-]*(?:[.][a-z0-9_-]+)+\Z")
REFERENCE = re.compile(r"(?P<path>[A-Za-z0-9_./-]+\.(?:rs|py|md|tsv|toml))"
                       r"(?::(?P<line>[1-9][0-9]*)|#(?P<anchor>[A-Za-z0-9_:-]+))?")
RUST_QUALIFIED = re.compile(
    r"(?<![\w/.#])(?:crate::)?arena2::(?P<module>[a-z][a-z0-9_]*)(?:::[a-zA-Z_][a-zA-Z_0-9]*)+"
)
TEST_FUNCTION = re.compile(r"(?m)^(?:\s*#\[\w+[^(\]]*(?:\([^\n]*\))?\]\s*)*"
                           r"\s*(?:async\s+)?(?:def|fn)\s+([_a-zA-Z]\w*)\s*\(")


def normalize_status(value):
    state = value.split(":", 1)[0].strip()
    if state.startswith(("verified for ", "implemented for ")):
        return "gap"
    if state.startswith("inapplicable to "):
        return "not_applicable"
    return STATUS.get(state)


def load(path):
    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if tuple(reader.fieldnames or ()) != COLUMNS:
            raise ValueError(f"{path}: expected tab-separated fields {' '.join(COLUMNS)}")
        return [(number, row) for number, row in enumerate(reader, 2)]


@lru_cache(maxsize=None)
def source(path):
    resolved = (ROOT / path).resolve()
    if not resolved.is_relative_to(ROOT):
        raise ValueError(f"reference escapes the repository: {path}")
    return resolved.read_text(encoding="utf-8")


@lru_cache(maxsize=1)
def indexed_test_sources():
    matches = {}
    for root, suffix in ((ROOT / "src", "*.rs"), (ROOT / "python" / "tests", "*.py")):
        for path in root.rglob(suffix):
            relative = path.relative_to(ROOT).as_posix()
            body = source(relative)
            for symbol in TEST_FUNCTION.findall(body):
                if path.suffix == ".py" and not symbol.startswith("test_"):
                    continue
                if path.suffix == ".rs" and not re.search(
                    rf"#\[test\]\s*(?:#\[[^\n]+\]\s*)*fn\s+{re.escape(symbol)}\s*\(", body
                ):
                    continue
                matches.setdefault(symbol, set()).add(relative)
    return matches


def resolve_references(value, *, executable=False):
    def qualified_ref(match):
        module = match.group("module")
        symbol = match.group().rsplit("::", 1)[-1]
        return f"src/arena2/{module}.rs#{symbol}"

    value = RUST_QUALIFIED.sub(qualified_ref, value)
    value = re.sub(r"(\.(?:rs|py))::([A-Za-z_])", r"\1#\2", value)
    if executable:
        tokens = [token.strip() for token in value.split(";")]
        normalized = []
        for token in tokens:
            if re.fullmatch(r"[A-Za-z_]\w*", token):
                paths = indexed_test_sources().get(token, ())
                if len(paths) != 1:
                    raise ValueError(f"test name must resolve uniquely or include a path: {token}")
                normalized.append(f"{next(iter(paths))}#{token}")
            else:
                normalized.append(token)
        value = "; ".join(normalized)
    resolved = []
    for ref in REFERENCE.finditer(value):
        path, line, anchor = ref.group("path", "line", "anchor")
        body = source(path)
        if line and int(line) > len(body.splitlines()):
            raise ValueError(f"reference line does not exist: {path}:{line}")
        symbol = anchor.rsplit("::", 1)[-1] if anchor else None
        if executable:
            if not symbol or not path.endswith((".rs", ".py")):
                raise ValueError(f"test evidence needs an executable file#test_name: {value}")
            if symbol not in TEST_FUNCTION.findall(body):
                raise ValueError(f"test function does not exist: {path}#{symbol}")
            if path.endswith(".py") and not symbol.startswith("test_"):
                raise ValueError(f"Python evidence is not discoverable as a test: {path}#{symbol}")
            if path.endswith(".rs") and not re.search(
                rf"#\[test\]\s*(?:#\[[^\n]+\]\s*)*fn\s+{re.escape(symbol)}\s*\(", body
            ):
                raise ValueError(f"Rust evidence lacks #[test]: {path}#{symbol}")
        elif symbol and path.endswith((".rs", ".py")) and not re.search(rf"\b{re.escape(symbol)}\b", body):
            raise ValueError(f"implementation anchor does not exist: {path}#{symbol}")
        resolved.append((path, symbol))
    if not resolved:
        raise ValueError(f"no repository path referenced: {value}")
    return resolved


def read_test_outcomes(path):
    """Read Cargo/libtest and unittest -v outcomes from a completed log."""
    observed = {}
    rust = re.compile(r"^test ([\w:]+) \.\.\. (ok|FAILED|ignored)")
    python = re.compile(r"^(test_\w+) \(.+?\) \.\.\. (ok|FAIL|ERROR|skipped)")
    for line in path.read_text(encoding="utf-8").splitlines():
        match = rust.match(line) or python.match(line)
        if match:
            symbol = match[1].rsplit("::", 1)[-1]
            observed.setdefault(symbol, set()).add(match[2])
    if not observed:
        raise ValueError("no Cargo or unittest results in selected log")
    return observed


def check(paths, strict, outcomes=None):
    inventory = {}
    totals = Counter()
    errors = []
    for path in paths:
        for number, row in load(path.resolve()):
            location = f"{path.relative_to(ROOT) if path.is_relative_to(ROOT) else path}:{number}"
            try:
                if row.get(None) is not None or any(not (row.get(key) or "").strip() for key in COLUMNS):
                    raise ValueError("wrong number of fields or empty field")
                first_last = LEVELS.fullmatch(row["levels"])
                if first_last is None or (first_last[2] and first_last[1] >= first_last[2]):
                    raise ValueError(f"invalid supported level range {row['levels']!r}")
                identifier = row["id"]
                if not VALID_ID.fullmatch(identifier) or identifier in inventory:
                    raise ValueError(f"invalid or duplicate ID {identifier}")
                inventory[identifier] = row
                status = normalize_status(row["status"])
                if status is None:
                    raise ValueError(f"unknown status {row['status']!r}")
                totals[status] += 1
                if row["rule_source"] == "-":
                    raise ValueError("missing rule source")
                if status == "verified":
                    resolve_references(row["implementation"])
                    evidence = resolve_references(row["test"], executable=True)
                    if outcomes is not None:
                        for _, test_name in evidence:
                            if outcomes.get(test_name) != {"ok"}:
                                observed = ", ".join(sorted(outcomes.get(test_name, ()))) or "not seen"
                                raise ValueError(f"execution evidence for {test_name}: {observed}")
                elif status == "creation_choice":
                    if len(row["choices"].strip()) < 3 or row["choices"] == "Per source":
                        raise ValueError("creation choice must record the selected option")
                    resolve_references(row["implementation"])
                elif status == "not_applicable":
                    if len(row["trigger"].strip()) < 3 or row["choices"] in {"-", "Per source"}:
                        raise ValueError("not-applicable row needs concrete scope rationale")
                elif strict:
                    raise ValueError(f"unresolved coverage: {identifier}")
            except (OSError, ValueError) as error:
                errors.append(f"{location}: {error}")

    counts = Counter(row["build"] for key, row in inventory.items() if key.startswith("grant."))
    for name, expected in EXPECTED_GRANTS.items():
        if counts[name] != expected:
            errors.append(f"{name} has {counts[name]} class grant rows, expected {expected}")
    for prefix, expected in EXPECTED_CATALOG.items():
        count = sum(key.startswith(f"{prefix}.") for key in inventory)
        if count < expected:
            errors.append(f"{prefix} has {count} rows, expected at least {expected}")
    return totals, errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("matrices", nargs="*", type=Path, help="defaults to docs/coverage/*.tsv")
    parser.add_argument("--strict", action="store_true", help="fail on an unresolved coverage row")
    parser.add_argument("--test-log", type=Path,
                        help="require every verified test to have passed in a Cargo/unittest log")
    args = parser.parse_args()
    try:
        paths = args.matrices or sorted((ROOT / "docs" / "coverage").glob("*.tsv"))
        outcomes = None if args.test_log is None else read_test_outcomes(args.test_log)
        totals, errors = check(paths, args.strict, outcomes)
    except (OSError, ValueError) as error:
        return parser.exit(1, f"{error}\n")
    for error in errors:
        print(error, file=sys.stderr)
    print(f"{sum(totals.values())} rows across {len(paths)} matrices "
          f"({', '.join(f'{state}={count}' for state, count in sorted(totals.items()))})")
    return int(bool(errors))


if __name__ == "__main__":
    sys.exit(main())
