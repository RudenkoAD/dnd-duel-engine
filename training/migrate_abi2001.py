"""Reinterpret an ABI 2001 checkpoint as ABI 2002 without updating weights."""

import argparse
import hashlib
from io import BytesIO
import json
from pathlib import Path
from tempfile import NamedTemporaryFile

import numpy as np

from arena.policy import CHECKPOINT_VERSION, Policy, WEIGHT_KEYS

SOURCE_ABI = 2001
TARGET_ABI = 2002
FEATURES = 1024
ACTIONS = 128


def migrate(source, output):
    source = Path(source)
    output = Path(output)
    if source.resolve() == output.resolve():
        raise ValueError("migration must preserve the source checkpoint")

    source_bytes = source.read_bytes()
    policy = Policy.load(BytesIO(source_bytes)).require_arena(SOURCE_ABI, FEATURES, ACTIONS)
    metadata = {
        "checkpoint_version": CHECKPOINT_VERSION,
        "arena_abi": TARGET_ABI,
        "observation_features": FEATURES,
        "action_count": ACTIONS,
        "stage": "reinterpreted_warm_start",
        "target_coordinates": "viewer_relative",
        "source_abi": SOURCE_ABI,
        "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
    }
    for field in ("stage", "training_run_id", "training_update"):
        if field in policy.metadata:
            metadata["source_" + field] = policy.metadata[field]

    # Copy the arrays from the archive to preserve their exact values and dtypes.
    with np.load(BytesIO(source_bytes), allow_pickle=False) as archive:
        weights = {key: archive[key] for key in WEIGHT_KEYS}
    output.parent.mkdir(parents=True, exist_ok=True)
    with NamedTemporaryFile(dir=output.parent, suffix=".npz", delete=False) as temp:
        path = Path(temp.name)
        try:
            np.savez_compressed(temp, metadata=json.dumps(metadata, sort_keys=True), **weights)
            temp.close()
            path.replace(output)
        finally:
            path.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    migrate(args.source, args.output)


if __name__ == "__main__":
    main()
