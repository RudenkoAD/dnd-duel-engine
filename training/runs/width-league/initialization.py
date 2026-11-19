"""Reproduce the width warm-start audit against live, actor-visible observations."""

import hashlib
import json
from collections import Counter
from pathlib import Path

import numpy as np
import torch

from arena.native import ACTIONS, ARENA_ABI, FEATURES, Batch, _lib
from arena.policy import Policy
from self_play import Schedule, phase_ids
from train_arena import ActorCritic

ROOT = Path(__file__).resolve().parents[2]
NARROW = ROOT / "runs/league/checkpoints/update-1300.npz"
WIDE = Path(__file__).with_name("wide-init.npz")
SEED = 0x5749445600000000
ATOL = 5e-4


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class Audit:
    def __init__(self, numpy_models, torch_models):
        self.numpy_models = numpy_models
        self.torch_models = torch_models
        self.stats = Counter()
        self.phases = Counter()

    def _compare(self, label, old, new, mask):
        error = float(np.max(np.abs(old - new)))
        self.stats[label + "_max_abs_error"] = max(
            self.stats[label + "_max_abs_error"], error,
        )
        if error > ATOL:
            raise AssertionError((label, error))
        before = np.where(mask, old, -np.inf)
        after = np.where(mask, new, -np.inf)
        best = before.argmax(axis=1)
        changed = best != after.argmax(axis=1)
        # The old top-two gap explains any greedy changes from rounding.
        top_two = np.partition(before, -2, axis=1)[:, -2:]
        margin = top_two[:, 1] - top_two[:, 0]
        self.stats[label + "_exact_ties"] += int((margin == 0).sum())
        self.stats[label + "_near_ties"] += int(((margin > 0) & (margin <= 2 * error)).sum())
        self.stats[label + "_greedy_changes"] += int(changed.sum())
        largest_change_margin = float(margin[changed].max(initial=0))
        self.stats[label + "_largest_changed_margin"] = max(
            self.stats[label + "_largest_changed_margin"], largest_change_margin,
        )
        if largest_change_margin > 2 * error:
            raise AssertionError((label, largest_change_margin, error))

    @torch.no_grad()
    def verify(self, features, masks, actors=None):
        self.stats["observations"] += len(features)
        if actors is not None:
            for seat, phase in zip(actors.tolist(), phase_ids(features).tolist()):
                self.phases[f"{seat}:{phase}"] += 1
        numpy_logits = [policy.logits(features) for policy in self.numpy_models]
        self._compare("numpy_logits", *numpy_logits, masks)
        features = torch.from_numpy(features)
        tensor_mask = torch.from_numpy(masks)
        old, new = [model(features, tensor_mask) for model in self.torch_models]
        self._compare("torch_legal_logits", old[0].numpy(), new[0].numpy(), masks)
        self._compare("torch_all_logits",
                      *(model(features, torch.ones_like(tensor_mask))[0].numpy()
                        for model in self.torch_models),
                      masks)
        error = float((old[1] - new[1]).abs().max())
        if error > ATOL:
            raise AssertionError(("value", error))
        self.stats["value_max_abs_error"] = max(self.stats["value_max_abs_error"], error)
        return numpy_logits[0]

    def report(self):
        return {**self.stats, "observations_by_seat_and_phase": dict(sorted(self.phases.items()))}


def run():
    torch.set_num_threads(2)
    torch.set_num_interop_threads(1)
    rng = np.random.default_rng(SEED)
    paths = (NARROW, WIDE)
    deployed = [Policy.load(path).require_arena(ARENA_ABI, FEATURES, ACTIONS) for path in paths]
    models = [ActorCritic.from_checkpoint(path).eval() for path in paths]

    synthetic = Audit(deployed, models)
    x = rng.standard_normal((4096, FEATURES), dtype=np.float32)
    x[:32] = 0
    mask = rng.random((len(x), ACTIONS)) >= 0.5
    mask[:, 0] = True
    synthetic.verify(x, mask)

    live = Audit(deployed, models)
    scenario_reports = {}
    # Three disjoint fixture sets cover expert play, learner trajectories and exploration.
    for offset, controller in enumerate(("expert", "champion_greedy", "champion_sample"), start=1):
        count = 384
        fixture_schedule = Schedule(SEED + (offset << 24), training=True)
        result_counts = Counter()
        started = live.stats["observations"]
        with Batch(count) as arena:
            seeds, classes, levels, _ = fixture_schedule.draw(count)
            arena.reset(np.arange(count), seeds, classes, levels)
            for _ in range(4096):
                features, masks, actors, results = arena.observe()
                active = (results == -1) & masks.any(axis=1)
                if not active.any():
                    result_counts.update(results.tolist())
                    break
                logits = live.verify(features[active], masks[active], actors[active])
                if controller == "expert":
                    actions = arena.bot(expert=True)
                    actions[~active] = -1
                else:
                    actions = np.full(count, -1, dtype=np.int32)
                    if controller == "champion_sample":
                        logits = logits + rng.gumbel(size=logits.shape)
                    actions[active] = np.where(masks[active], logits, -np.inf).argmax(axis=1)
                arena.step(actions)
            else:
                raise AssertionError("fixture batch did not finish")
        scenario_reports[controller] = {
            "duels": count,
            "results": dict(result_counts),
            "decisions": live.stats["observations"] - started,
        }

    optimizer = torch.optim.SGD(models[1].parameters(), lr=0.01)
    logits, values = models[1](torch.from_numpy(x), torch.from_numpy(mask))
    loss = values.square().mean() - logits.log_softmax(dim=-1).gather(
        1, logits.argmax(dim=-1, keepdim=True),
    ).mean()
    loss.backward()
    layers = (models[1].first, models[1].second)
    gradient_differences = [float((a - b).abs().max()) for a, b in
                            (layer.weight.grad.chunk(2) for layer in layers)]
    optimizer.step()
    weight_differences = [float((a - b).abs().max()) for a, b in
                          (layer.weight.detach().chunk(2) for layer in layers)]
    if min(*gradient_differences, *weight_differences) <= 0:
        raise AssertionError("duplicates must diverge on the first gradient update")
    return {
        "checkpoints": [{"sha256": sha256(path),
                         "parameters": sum(p.numel() for p in model.parameters())}
                        for path, model in zip(paths, models)],
        "arena_abi": ARENA_ABI,
        "native_sha256": sha256(_lib()._name),
        "seed": SEED,
        "absolute_tolerance": ATOL,
        "synthetic": synthetic.report(),
        "arena": live.report(),
        "arena_controllers": scenario_reports,
        "max_gradient_difference_by_hidden_layer": gradient_differences,
        "max_weight_difference_by_hidden_layer_after_one_sgd_step": weight_differences,
    }


if __name__ == "__main__":
    print(json.dumps(run(), indent=2, sort_keys=True))
