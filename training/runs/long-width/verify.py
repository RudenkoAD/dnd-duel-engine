"""Check completed training and milestone receipts against the run contract."""

import hashlib
import json
import math
from pathlib import Path
import sys

import numpy as np

ROOT = Path(__file__).resolve().parent
MILESTONES = (967, 1934, 2901)
ORIGIN = 757
DUELS_PER_UPDATE = 384
ORIGIN_SHA = "1c8cfe14f7a18f5455e022dfcc27376ecb273ae276a1a493914680c4d7fc51b5"


def require(condition, detail):
    if not condition:
        raise ValueError(detail)


def verify_resume(path, policy_path, receipt):
    import torch

    state = torch.load(path, map_location="cpu", weights_only=False)
    require(state["receipt"] == {key: value for key, value in receipt.items()
                                 if key not in {"path", "sha256"}},
            "recovery receipt differs from the terminal milestone")
    with np.load(policy_path, allow_pickle=False) as archive:
        metadata = json.loads(archive["metadata"].item())
        require(state["league"] == metadata["league"], "recovery league differs from checkpoint")
        layers = {"first": "1", "second": "2", "policy": "_policy", "value": "_value"}
        for layer, key in layers.items():
            require(np.array_equal(state["model"][f"{layer}.weight"].numpy().T,
                                   archive["w" + key]), f"recovery {layer} weight differs")
            require(np.array_equal(state["model"][f"{layer}.bias"].numpy(),
                                   archive["b" + key]), f"recovery {layer} bias differs")
    optimizer = state["optimizer"]
    require(len(optimizer["state"]) == len(state["model"]), "incomplete recovery optimizer")
    for entry in optimizer["state"].values():
        require(int(entry["step"]) == receipt["optimizer_minibatches"] - 64608,
                "incorrect recovery Adam step")
        require(bool(torch.isfinite(entry["exp_avg"]).all())
                and bool(torch.isfinite(entry["exp_avg_sq"]).all()),
                "non-finite recovery Adam state")
    require(state["schedule"]["seed"] == 0x4232540000000000
            and state["schedule"]["pairs_started"] == receipt["new_bouts"] // 2,
            "incorrect recovery fixture offset")
    return dict(path=path.name, sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def verify(root):
    with (root / "training.jsonl").open() as handle:
        events = [json.loads(line) for line in handle]
    start = events[0]
    require(start["event"] == "start" and start["base"]["update"] == ORIGIN
            and start["warm_sha256"] == ORIGIN_SHA, "unexpected origin")
    require(start["seed_namespace"] == "4232540000000000", "unexpected seed namespace")
    require(len(start["league"]) == 15, "incomplete origin league")
    updates = [event for event in events if event["event"] == "update"]
    actual = [event["update"] for event in updates]
    require(actual == list(range(ORIGIN + 1, ORIGIN + 1 + len(actual))),
            "missing, duplicated or unordered PPO update")
    previous = dict(cpu_s=0, elapsed_s=0, **{
        key: start["base"][key] for key in
        ("learner_decisions", "frozen_decisions", "optimizer_minibatches")
    })
    for event in updates:
        update = event["update"]
        expected = update * DUELS_PER_UPDATE
        new_duels = (update - ORIGIN) * DUELS_PER_UPDATE
        require(event["bouts"] == expected and event["new_bouts"] == new_duels,
                f"inconsistent duel counts at update {update}")
        require(event["live_vs_live_bouts"] == event["unequal_level_bouts"] == expected // 4,
                f"incorrect live or mixed-level allocation at update {update}")
        require(0 < event["coverage_cells"] <= 120 and event["min_games_per_cell"] > 0,
                f"incomplete learner coverage at update {update}")
        require(event["learning_rate"] == 4e-5, f"unexpected LR at update {update}")
        require(sum(event["termination"].values()) == new_duels,
                f"termination mismatch at update {update}")
        for key in previous:
            require(event[key] > previous[key], f"non-increasing {key} at update {update}")
        for key in ("kl", "entropy", "policy_loss", "value_loss", "projectile_target_loss"):
            require(math.isfinite(event[key]), f"non-finite {key} at update {update}")
        previous = {key: event[key] for key in previous}

    receipts = []
    complete_through = actual[-1] if actual else ORIGIN
    checkpoint_events = [event for event in events if event["event"] == "checkpoint"]
    require([e["update"] for e in checkpoint_events] ==
            [update for update in MILESTONES if update <= complete_through],
            "unexpected checkpoint events")
    for checkpoint in checkpoint_events:
        update = checkpoint["update"]
        require(checkpoint["coverage_cells"] == 120, f"incomplete coverage at milestone {update}")
        path = root / "checkpoints" / f"update-{update:04d}.npz"
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        receipt = json.loads(path.with_suffix(".json").read_text())
        require(receipt["sha256"] == checkpoint["sha256"] == digest, f"bad digest for {path}")
        event = updates[update - ORIGIN - 1]
        for key in receipt.keys() - {"sha256", "path"}:
            require(receipt[key] == checkpoint[key] == event[key],
                    f"inconsistent {key} for {path}")
        with np.load(path, allow_pickle=False) as archive:
            metadata = json.loads(archive["metadata"].item())
            require(archive["b1"].shape == archive["b2"].shape == (256,), f"bad width for {path}")
            for key in receipt.keys() - {"sha256", "path"}:
                require(metadata[key] == receipt[key], f"bad metadata {key} for {path}")
        require(metadata["arena_abi"] == 2002
                and metadata["native_sha256"] == start["native_sha256"],
                f"inconsistent arena for {path}")
        growth = [u for u in range(ORIGIN + 1, update + 1) if u % 64 == 0]
        require(len(metadata["league"]) == 15 + len(growth), f"incorrect league size for {path}")
        for snapshot_update in growth:
            label = f"update-{snapshot_update:04d}"
            snapshot = root / "history" / f"{label}.npz"
            digest = hashlib.sha256(snapshot.read_bytes()).hexdigest()
            require(metadata["league"][label]["sha256"] == digest, f"bad digest for {snapshot}")
        require(sum(member["bouts"] for member in metadata["league"].values()) ==
                receipt["bouts"] - receipt["live_vs_live_bouts"],
                f"league bout count mismatch for {path}")
        receipts.append(receipt)

    result = {"complete_through": complete_through, "verified_checkpoints": receipts}
    if complete_through == MILESTONES[-1]:
        require(events[-1]["event"] == "stop" and events[-1]["reason"] == "exact_update_budget",
                "complete update budget without a terminal receipt")
        result["verified_resume"] = verify_resume(
            root / "resume.pt", root / "checkpoints" / "update-2901.npz", receipts[-1],
        )
    else:
        require(complete_through < MILESTONES[-1], "training went past the planned budget")
    return result


if __name__ == "__main__":
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT
    print(json.dumps(verify(root), indent=2, sort_keys=True))
