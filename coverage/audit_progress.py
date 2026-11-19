#!/usr/bin/env python3
"""Group the independent master checklist by concrete domain dependencies."""

import argparse
from collections import Counter, defaultdict
import csv
from pathlib import Path
import sys

from check_matrix import ROOT, load, normalize_status, read_test_outcomes, resolve_references

MATRIX_DIR = ROOT / "docs" / "coverage"
LINKS = ROOT / "coverage" / "links.tsv"
OPEN_IMPLEMENTATION = {"missing", "partial", "unsupported"}
OPEN_EVIDENCE = {"pending", "pending_integration", "unverified", "open", "planned", "gap", "implemented"}


def build_audit(matrix_dir=MATRIX_DIR, links_path=LINKS):
    catalog = {row["id"]: row for _, row in load(matrix_dir / "catalog.tsv")}
    with links_path.open(encoding="utf-8", newline="") as stream:
        link_rows = list(csv.DictReader(stream, delimiter="\t"))
    links = {row["master"]: (row["owner"], tuple(row["dependencies"].split()))
             for row in link_rows}
    if len(links) != len(link_rows) or set(links) != set(catalog):
        raise ValueError("dependency map must contain exactly one entry per master catalog row")

    domain = {}
    for path in sorted(matrix_dir.glob("*.tsv")):
        if path.name == "catalog.tsv":
            continue
        for _, row in load(path):
            identifier = row["id"]
            if identifier in domain or identifier in catalog:
                raise ValueError(f"duplicate coverage ID {identifier} in {path}")
            domain[identifier] = row
    return catalog, domain, links


def domain_state(row, outcomes=None):
    if row is None:
        return "unmapped"
    status = row["status"].split(":", 1)[0].strip()
    normalized = normalize_status(row["status"])
    if normalized == "verified":
        try:
            resolve_references(row["implementation"])
            refs = resolve_references(row["test"], executable=True)
            if outcomes is not None:
                for _, symbol in refs:
                    seen = outcomes.get(symbol)
                    if seen is None:
                        return "test-not-observed"
                    if seen != {"ok"}:
                        return "failing-test"
        except (OSError, ValueError):
            return "missing-anchor"
        return "verified"
    if normalized in {"creation_choice", "not_applicable"}:
        return normalized
    if status in OPEN_IMPLEMENTATION or status.startswith("verified for "):
        return "open-mechanic"
    if status in OPEN_EVIDENCE or status.startswith("implemented for "):
        return "open-evidence"
    return "unclassified-status"


def audit(catalog, domain, links, outcomes=None):
    graph = domain | catalog

    def evaluate(identifier, visiting):
        if identifier in visiting:
            raise ValueError(f"dependency cycle through {identifier}")
        visiting = visiting | {identifier}
        row = graph.get(identifier)
        status = domain_state(row, outcomes)
        if identifier not in links:
            return status, [] if status in {"verified", "creation_choice", "not_applicable"} else [identifier]
        dependencies = () if links[identifier][1] == ("-",) else links[identifier][1]
        child_results = [(child, *evaluate(child, visiting)) for child in dependencies]
        blockers = []
        for child, reason, downstream in child_results:
            if reason == "domain-blocked":
                blockers.extend(downstream)
            elif reason not in {"verified", "creation_choice", "not_applicable", "reviewable"}:
                blockers.append(f"{child} [{reason}]")
        blockers = list(dict.fromkeys(blockers))
        if blockers:
            return "domain-blocked", blockers
        pending_reviews = [child for child, reason, _ in child_results if reason == "reviewable"]
        if pending_reviews:
            return "master-review", pending_reviews
        if status in {"verified", "creation_choice", "not_applicable"}:
            return status, []
        if dependencies:
            return "reviewable", []
        return "no-linked-evidence", []

    results = {}
    for identifier in catalog:
        results[identifier] = evaluate(identifier, set())
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--owner")
    parser.add_argument("--test-log", type=Path, help="use actual Cargo/unittest execution evidence")
    parser.add_argument("--strict", action="store_true", help="fail unless every matched master row is closed")
    parser.add_argument("--details", action="store_true", help="list open master rows and immediate blockers")
    parser.add_argument("--blockers", action="store_true", help="rank outstanding leaf IDs by the number of master rows affected")
    args = parser.parse_args()
    try:
        catalog, domain, links = build_audit()
        outcomes = None if args.test_log is None else read_test_outcomes(args.test_log)
        report = audit(catalog, domain, links, outcomes)
    except (OSError, ValueError) as error:
        return parser.exit(1, f"{error}\n")
    totals = defaultdict(Counter)
    unresolved = 0
    for record, (state, blockers) in report.items():
        owner = links[record][0]
        if args.owner and args.owner != owner:
            continue
        totals[owner][state] += 1
        unresolved += state not in {"verified", "creation_choice", "not_applicable"}
        if args.details and state not in {"verified", "creation_choice", "not_applicable"}:
            detail = ", ".join(blockers)
            print(f"{owner:10} {state:18} {record}{f' — {detail}' if detail else ''}")
    if args.blockers:
        blocker_counts = Counter()
        for record, (_, immediate) in report.items():
            if args.owner and args.owner != links[record][0]:
                continue
            blocker_counts.update(block for block in immediate if "[" in block)
        for blocker, affected in blocker_counts.most_common():
            print(f"{affected:3} {blocker}")
    for owner, counts in sorted(totals.items()):
        print(f"{owner:10} {sum(counts.values()):3} "
              + ", ".join(f"{state}={value}" for state, value in sorted(counts.items())))
    return int(args.strict and unresolved > 0)


if __name__ == "__main__":
    sys.exit(main())
