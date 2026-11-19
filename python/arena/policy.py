"""NumPy inference for the masked arena policy exported by the trainer."""

import json
from pathlib import Path

import numpy as np

DEFAULT_CHECKPOINT = Path(__file__).resolve().parents[2] / "models" / "arena_policy.npz"
CHECKPOINT_VERSION = 1
WEIGHT_KEYS = ("w1", "b1", "w2", "b2", "w_policy", "b_policy", "w_value", "b_value")


class Policy:
    def __init__(self, weights, metadata):
        self.weights = weights
        self.metadata = metadata
        w1, b1, w2, b2, wp, bp, wv, bv = (weights[key] for key in WEIGHT_KEYS)
        valid = (
            w1.ndim == w2.ndim == wp.ndim == wv.ndim == 2
            and b1.shape == (w1.shape[1],)
            and w2.shape[0] == w1.shape[1]
            and b2.shape == (w2.shape[1],)
            and wp.shape[0] == w2.shape[1]
            and bp.shape == (wp.shape[1],)
            and wv.shape == (w2.shape[1], 1)
            and bv.shape == (1,)
        )
        if not valid or any(not np.isfinite(w).all() for w in weights.values()):
            raise ValueError("invalid policy weights")
        self.features = w1.shape[0]
        self.actions = wp.shape[1]

    @classmethod
    def load(cls, path=DEFAULT_CHECKPOINT):
        with np.load(path, allow_pickle=False) as archive:
            metadata = json.loads(archive["metadata"].item())
            if metadata.get("checkpoint_version") != CHECKPOINT_VERSION:
                raise ValueError("unsupported policy checkpoint version")
            weights = {key: archive[key].astype(np.float32) for key in WEIGHT_KEYS}
        return cls(weights, metadata)

    def require_arena(self, abi, features, actions):
        """Reject a checkpoint built against another observation or action contract."""
        checkpoint_abi = self.metadata.get("arena_abi")
        same_shape = (self.features, self.actions) == (features, actions)
        if checkpoint_abi != abi or not same_shape:
            migration = (
                ". Use training/migrate_abi2001.py to explicitly reinterpret the target coordinates"
                if same_shape and (checkpoint_abi, abi) == (2001, 2002) else ""
            )
            raise ValueError(
                f"checkpoint arena ABI {checkpoint_abi}, "
                f"{self.features} features and {self.actions} actions are incompatible with "
                f"arena ABI {abi}, {features} features and {actions} actions{migration}"
            )
        return self

    def logits(self, features):
        features = np.asarray(features, dtype=np.float32)
        if features.ndim == 0 or features.shape[-1] != self.features:
            raise ValueError(f"expected {self.features} arena features")
        w = self.weights
        hidden = np.tanh(np.einsum("...i,ij->...j", features, w["w1"]) + w["b1"])
        hidden = np.tanh(np.einsum("...i,ij->...j", hidden, w["w2"]) + w["b2"])
        return np.einsum("...i,ij->...j", hidden, w["w_policy"]) + w["b_policy"]

    def act_batch(self, features, action_mask, rng=None):
        logits = self.logits(features)
        mask = np.asarray(action_mask, dtype=bool)
        if logits.ndim != 2 or mask.shape != logits.shape or not np.all(mask.any(axis=1)):
            raise ValueError("expected one nonempty legal action mask per decision")
        masked = np.where(mask, logits, -np.inf)
        if rng is not None:
            masked = masked + rng.gumbel(size=masked.shape)
        return masked.argmax(axis=1).astype(np.int32)

    def act(self, features, action_mask, rng=None):
        return int(self.act_batch(np.asarray(features)[None, :],
                                  np.asarray(action_mask)[None, :], rng)[0])

    def choose(self, features, action_mask):
        return self.act_batch(features, action_mask)
