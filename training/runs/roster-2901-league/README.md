# Level-five roster league from update 2901

The learner starts from the 2×256 [update 2901](../long-width/checkpoints/update-2901.npz) policy. The frozen anchors are the starting policy and the two earlier milestones in the same training lineage, [update 967](../long-width/checkpoints/update-0967.npz) and [update 1934](../long-width/checkpoints/update-1934.npz). All three use masked stochastic inference in training. The loader zeros the 29 new observation inputs in the first layer of each pre-roster network. The learner can update those weights, and the historical files stay intact. The run starts fresh Adam at the previous learning-rate floor, 0.00004.

The fixed budget is 1,000 new PPO updates, each with 384 complete bouts. The fixed terminal checkpoint is `checkpoints/update-1000.npz`. The run uses level five throughout. Each sweep covers all 1,296 ordered build pairs, plays both seat orientations with a shared seed and arena, and cycles through all five layouts. Each update has 96 live–live bouts. Until the first new snapshot, all 288 frozen bouts per update go to the anchors. From update 65, half of those frozen bouts go to new snapshots. A snapshot joins every 64 updates. Within each pool the ordinary league prioritizes opponents against which the learner scores poorly. There are no specialist opponents.

## Reproduce

From the repository root, activate the Python environment described in the [quickstart](../../../README.md) and install PyTorch. The recorded run used Python 3.10.15, NumPy 2.2.6 and CPU PyTorch 2.9.1. Build the native ABI 2002 library for your platform, then run the league into a fresh directory.

```sh
make release
PYTHONPATH=python python training/league_play.py \
  --start training/runs/long-width/checkpoints/update-2901.npz \
  --anchor training/runs/long-width/checkpoints/update-0967.npz \
  --anchor training/runs/long-width/checkpoints/update-1934.npz \
  --run-id 2901 --run-dir training/runs/my-roster-reproduction \
  --updates 1000 --seconds 18000 --checkpoint-every 64 \
  --batch 384 --minibatch 768 --epochs 3 --threads 2 \
  --lr 0.00004 --final-lr 0.00004 --decay-updates 600 \
  --entropy 0.012 --repeat-cost 0.02 --timeout-penalty 0.35 \
  --projectile-target-weight 0.04 --live-fraction 0.25 --matched
```

The [training log](training.jsonl) records the run configuration, input checkpoint hashes, neutralized features, arena library hash, fixtures' seed namespace, cumulative optimizer and coverage counts, league scores, and published checkpoint hashes. The count of appearances in a build × seat cell includes bouts in which the learner made at least one decision. The checkpoint's `cumulative` counters cover this run only.

After completion, verify the training log, checkpoint files, and deterministic build × seat × layout schedule using your release build. To additionally check byte identity with the original macOS library, pass `--require-original-native`.

```sh
PYTHONPATH=python:training python \
  training/runs/roster-2901-league/verify.py
```

## Completed run

The run ended at the fixed update limit after 43 minutes 56 seconds of wall time and 2,900.08 process CPU seconds. It completed 384,000 bouts and 108,219 optimizer minibatches, with 27,315,868 learner decisions and 16,927,995 frozen-policy decisions. The 288,000 frozen-opponent bouts include 153,216 against the three fixed anchors and 134,784 against 15 new snapshots. There were 381,542 defeats, 412 double knockouts, 1,229 turn-limit draws, and 817 decision-limit draws.

The [verification receipt](verification.json) confirms at least 6,531 learner appearances in each of the 72 build × seat cells. Replaying the frozen fixture schedule yields exactly 76,800 bouts on each of the five maps and identical focal-seat counts for each build. Every ordered build pair visits every map in both seat orientations. The audit verifies all 16 checkpoint hashes and checks that the terminal learner updated both its newly initialized inputs and other model weights.

The fixed [update 1000 checkpoint](checkpoints/update-1000.npz) has SHA-256 `608facd39535f804f9304783093f2590c74ed66966654ff51bb667c480ceda17`. The checkpoint is an inference export. This launcher does not save its Adam state.
