"""Regression checks for widening and loading exported policies."""

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import numpy as np
import torch

from arena.policy import CHECKPOINT_VERSION, Policy
from train_arena import ActorCritic
from widen_policy import widen_checkpoint


class WidthInitializationTests(unittest.TestCase):
    def test_checkpoint_equivalence_and_trainable_duplicates(self):
        torch.manual_seed(4)
        narrow = ActorCritic(17, 9, hidden=16)
        x = torch.randn(512, 17)
        mask = torch.rand(512, 9) > 0.3
        mask[:, 0] = True

        with TemporaryDirectory() as temp:
            source = Path(temp) / "narrow.npz"
            target = Path(temp) / "wide.npz"
            narrow.export(source, {"checkpoint_version": CHECKPOINT_VERSION, "arena_abi": 2002})
            widen_checkpoint(source, target)
            with self.assertRaises(FileExistsError):
                widen_checkpoint(source, target)
            wide = ActorCritic.from_checkpoint(target)
            legacy = ActorCritic.from_checkpoint(source)
            self.assertEqual(legacy.first.out_features, 16)
            self.assertEqual(wide.first.out_features, 32)
            original_policy = Policy.load(source)
            expanded_policy = Policy.load(target)
            np.testing.assert_allclose(original_policy.logits(x.numpy()),
                                       expanded_policy.logits(x.numpy()), atol=1e-6)
            np.testing.assert_array_equal(original_policy.act_batch(x.numpy(), mask.numpy()),
                                          expanded_policy.act_batch(x.numpy(), mask.numpy()))

        old_logits, old_values = legacy(x, mask)
        new_logits, new_values = wide(x, mask)
        torch.testing.assert_close(old_logits, new_logits, atol=1e-6, rtol=0)
        torch.testing.assert_close(old_values, new_values, atol=1e-6, rtol=0)
        old_actions = old_logits.argmax(dim=1)
        torch.testing.assert_close(old_actions, new_logits.argmax(dim=1))

        optimizer = torch.optim.SGD(wide.parameters(), lr=0.01)
        loss = -new_logits.log_softmax(dim=-1).gather(1, old_actions[:, None]).mean()
        (loss + new_values.square().mean()).backward()
        for layer in (wide.first, wide.second):
            leading, trailing = layer.weight.grad.chunk(2)
            self.assertGreater((leading - trailing).abs().max().item(), 1e-5)
        optimizer.step()
        for layer in (wide.first, wide.second):
            leading, trailing = layer.weight.detach().chunk(2)
            self.assertGreater((leading - trailing).abs().max().item(), 1e-7)
