"""Shape-safe warm start from the archived ABI 1003 policy to arena ABI 2001."""

import argparse
import hashlib
import json
from pathlib import Path

import numpy as np

from arena.policy import CHECKPOINT_VERSION, Policy

LEGACY_FEATURES = 256
LEGACY_ACTIONS = 32
NEW_FEATURES = 1024
NEW_ACTIONS = 128


def migrate(source, output):
    source = Path(source)
    output = Path(output)
    if source.resolve() == output.resolve():
        raise ValueError("migration must preserve the source checkpoint")
    original = Policy.load(source)
    if (original.features, original.actions, original.metadata.get("arena_abi")) != (
            LEGACY_FEATURES, LEGACY_ACTIONS, 1003):
        raise ValueError("expected an archived 256-feature, 32-action ABI 1003 checkpoint")
    weights = original.weights.copy()
    input_weights = original.weights["w1"]
    action_weights = original.weights["w_policy"]
    weights["w1"] = np.zeros((NEW_FEATURES, input_weights.shape[1]), np.float32)
    weights["w1"][:LEGACY_FEATURES] = input_weights
    # ABI 2001 keeps the prefix used by the old checkpoint. Its expanded
    # action mask is also present at 640:768 for learning.
    weights["w_policy"] = np.zeros((action_weights.shape[0], NEW_ACTIONS), np.float32)
    weights["w_policy"][:, :LEGACY_ACTIONS] = action_weights
    weights["b_policy"] = np.full((NEW_ACTIONS,), -1.25, np.float32)
    weights["b_policy"][:LEGACY_ACTIONS] = original.weights["b_policy"]
    metadata = {
        "checkpoint_version": CHECKPOINT_VERSION,
        "arena_abi": 2001,
        "observation_features": NEW_FEATURES,
        "action_count": NEW_ACTIONS,
        "stage": "migrated_warm_start",
        "source_abi": 1003,
        "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "new_actions_logit_prior": -1.25,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    temp = output.with_suffix(".tmp.npz")
    np.savez_compressed(temp, metadata=json.dumps(metadata, sort_keys=True), **weights)
    temp.replace(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    migrate(args.source, args.output)


if __name__ == "__main__":
    main()
