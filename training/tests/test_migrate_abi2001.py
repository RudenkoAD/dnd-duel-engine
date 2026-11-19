"""Verify the viewer-relative migration against the archived absolute checkpoint."""

import hashlib
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

import numpy as np

from arena.policy import DEFAULT_CHECKPOINT, Policy, WEIGHT_KEYS
from training.migrate_abi2001 import migrate


class MigrationTests(unittest.TestCase):
    def test_migration_preserves_all_weights_and_provenance(self):
        training = Path(__file__).resolve().parents[1]
        source = training / "runs" / "self_play" / "continuation.npz"
        digest = "fd8a81371247b42ba186a9f88e318fe2afab0ddfd131aad2b793a6fffd85cb6b"
        self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), digest)

        with TemporaryDirectory() as temp:
            output = Path(temp) / "converted.npz"
            migrate(source, output)
            generated = Policy.load(output).require_arena(2002, 1024, 128)
            self.assertEqual(generated.metadata["source_sha256"], digest)
            self.assertEqual(generated.metadata["source_abi"], 2001)
            self.assertEqual(generated.metadata["stage"], "reinterpreted_warm_start")
            with np.load(source) as original, np.load(output) as converted:
                for key in WEIGHT_KEYS:
                    self.assertEqual(original[key].dtype, converted[key].dtype)
                    self.assertEqual(original[key].tobytes(), converted[key].tobytes())

            with self.assertRaisesRegex(ValueError, "ABI 2001.*incompatible.*ABI 2002"):
                Policy.load(source).require_arena(2002, 1024, 128)
            with self.assertRaisesRegex(ValueError, "ABI 2002.*incompatible.*ABI 2001"):
                migrate(output, Path(temp) / "invalid.npz")
            with self.assertRaisesRegex(ValueError, "preserve the source"):
                migrate(source, source)

    def test_default_matches_relative_arena(self):
        Policy.load(DEFAULT_CHECKPOINT).require_arena(2002, 1024, 128)


if __name__ == "__main__":
    unittest.main()
