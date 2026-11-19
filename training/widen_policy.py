"""Double an arena policy's hidden width with function-preserving weight splits."""

import argparse
import hashlib
import json
from pathlib import Path

import numpy as np

from arena.policy import Policy

SPLIT_SEED = 0x57494445


def double_width(policy, seed=SPLIT_SEED):
    """Duplicate hidden activations and split each unit's outgoing weights."""
    weights = policy.weights
    rng = np.random.default_rng(seed)

    def split_rows(matrix):
        # Asymmetry lets incoming gradients diverge across each duplicate pair.
        # Binary fractions also keep the rounding error small in float32.
        fractions = rng.choice(np.array([0.25, 0.375, 0.625, 0.75], np.float32),
                               size=(matrix.shape[0], 1))
        primary = matrix * fractions
        return np.concatenate((primary, matrix - primary), axis=0)

    second = split_rows(weights["w2"])
    # Policy and value share layer 2, so use the same split for both heads.
    heads = split_rows(np.concatenate((weights["w_policy"], weights["w_value"]), axis=1))
    return {
        **weights,
        "w1": np.tile(weights["w1"], (1, 2)),
        "b1": np.tile(weights["b1"], 2),
        "w2": np.tile(second, (1, 2)),
        "b2": np.tile(weights["b2"], 2),
        "w_policy": heads[:, :policy.actions],
        "w_value": heads[:, policy.actions:],
    }


def widen_checkpoint(source, destination, seed=SPLIT_SEED):
    policy = Policy.load(source)
    weights = double_width(policy, seed)
    metadata = {
        **policy.metadata,
        "hidden_width": weights["w1"].shape[1],
        "width_initialization": {
            "method": "unequal_outgoing_weight_split",
            "seed": seed,
            "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
            "source_width": policy.weights["w1"].shape[1],
        },
    }
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("xb") as handle:
        np.savez_compressed(handle, metadata=json.dumps(metadata, sort_keys=True), **weights)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    arguments = parser.parse_args()
    widen_checkpoint(arguments.source, arguments.destination)
