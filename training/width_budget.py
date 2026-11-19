"""Freeze endpoints at a common training plus model-selection CPU allowance."""

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import shutil


def read_log(arm_dir):
    path = arm_dir / "training.jsonl"
    handle = path.open() if path.exists() else gzip.open(path.with_suffix(".jsonl.gz"), "rt")
    with handle:
        events = [json.loads(line) for line in handle]
    if events[-1].get("reason") != "cpu_cap":
        raise ValueError(f"incomplete training log in {arm_dir}")
    return events


def pin_endpoint(arm_dir, receipt):
    source = Path(receipt["path"])
    path = arm_dir / ("checkpoints/" + source.name if receipt["event"] == "checkpoint"
                      else "cpu-matched.npz")
    if not path.exists():
        shutil.copyfile(source, path)
    if hashlib.sha256(path.read_bytes()).hexdigest() != receipt["sha256"]:
        raise ValueError(f"checkpoint hash mismatch for {path}")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", type=Path, default=Path("training/runs/width-league"))
    parser.add_argument("--curve", type=Path, default=Path("evaluation/width-dev/curve.json"))
    args = parser.parse_args()
    curve = json.loads(args.curve.read_text())
    logs = {arm: read_log(args.run_dir / arm) for arm in ("narrow", "wide")}
    costs = {}
    for arm in logs:
        records = curve["branches"][arm]
        if set(records) != {"25", "50", "75", "100"}:
            raise ValueError(f"incomplete development evaluations for {arm}")
        costs[arm] = sum(r["cpu_seconds"] for r in records.values())
    cap = min(events[-1]["cpu_s"] + costs[arm] for arm, events in logs.items())
    report = {"common_cpu_seconds": cap, "arms": {}}
    for arm, events in logs.items():
        candidates = [e for e in events if e["event"] in {"checkpoint", "audit_checkpoint"}]
        eligible = [e for e in candidates if e["cpu_s"] + costs[arm] <= cap]
        chosen = max(eligible, key=lambda e: e["cpu_s"])
        path = pin_endpoint(args.run_dir / arm, chosen)
        report["arms"][arm] = {
            "training_cpu_seconds": events[-1]["cpu_s"],
            "selection_cpu_seconds": costs[arm],
            "all_in_cpu_seconds": events[-1]["cpu_s"] + costs[arm],
            "endpoint": str(path), "sha256": chosen["sha256"],
            "endpoint_training_cpu_seconds": chosen["cpu_s"],
            "endpoint_all_in_cpu_seconds": chosen["cpu_s"] + costs[arm],
            "endpoint_update": chosen["update"], "endpoint_bouts": chosen["bouts"],
            "final_training_update": events[-1]["last_completed"]["update"],
            "final_training_bouts": events[-1]["last_completed"]["bouts"],
        }
    (args.run_dir / "cpu-comparison.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
