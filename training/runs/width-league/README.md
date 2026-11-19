# CPU-matched width league

Both arms start from the learned league champion at update 1300. The learner and its growing history use width 128 in the narrow arm and width 256 in the wide arm. Their four native-width 128 anchors are update 1300, the original trained policy, the independent control, and target-aware update 90.

The arms share mirrored fixture streams, rollout and PPO seed streams, and a separate per-update matchmaking stream. History selection uses its own per-update stream. Opponent probabilities track each arm's results. Each PPO update uses 384 duels, minibatches of at most 768 decisions, and three epochs. Each process uses two Torch compute threads and one interop thread. The learning rate decays linearly from 0.00008 to 0.00004 across the first half of the budget.

Each arm has a cap of 1,576 process CPU seconds, measured as user plus system time across threads. Common initialization and final serialization are outside this measurement. The wide arm pays 0.033384 CPU seconds for the deterministic widening and serialization. The CPU deadline is checked between rollout steps and before PPO minibatches. A terminal partial update can spend CPU without contributing a checkpoint or its cumulative work counters. Interim serialization, matchmaking, logging, and copies of completed model states are charged. The final receipt distinguishes the last eligible checkpoint, total CPU to stop, measured overshoot, and terminal serialization cost.

A growing snapshot joins the training history every 1/12 of the cap. Independent DEV selection uses four checkpoints at 25%, 50%, 75%, and 100%, with the last fully completed update at or below each boundary. CPU audit checkpoints are spaced 1/96 of the cap, about 16.42 CPU seconds apart, including the four DEV candidates. Only checkpoints at growth boundaries can join the league. The temporary audit archives are under /tmp/dnd-width-training-1811. The two compressed training logs record each checkpoint digest, its completed update's CPU and wall times, duel and decision counts, and PPO optimizer minibatches.

Run both commands from the repository root. The original runs used a frozen ABI 2002 native package with dylib SHA-256 `621964e798659ba534fab6c8494664c8e7f9d91abb9a4e28ccb0ed9f40888e3f`.

```sh
export PYTHONPATH=/tmp/dnd-spec-frozen-621964e7/python
python_bin=python3.10
champion=training/runs/league/checkpoints/update-1300.npz
output_root=$(mktemp -d /tmp/dnd-width-reproduction.XXXXXX)
mkdir "$output_root"/{runs,audits}
anchors=(--anchor "$champion" --anchor models/arena_policy.npz
         --anchor training/runs/viewer_relative/control.npz
         --anchor training/runs/viewer_relative/target-aware-update-0090.npz)

"$python_bin" training/width_league.py --hidden 128 --run-id 1811 \
  --warm "$champion" "${anchors[@]}" \
  --run-dir "$output_root/runs/narrow" \
  --audit-dir "$output_root/audits/narrow"

"$python_bin" training/width_league.py --hidden 256 --run-id 1811 \
  --warm training/runs/width-league/wide-init.npz --init-cpu 0.033384 "${anchors[@]}" \
  --run-dir "$output_root/runs/wide" \
  --audit-dir "$output_root/audits/wide"
```

## Training results

| Arm | Last completed update | CPU to checkpoint | CPU to stop | Wall time to stop | Completed duels | Learner decisions | Frozen decisions | PPO minibatches |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 128 | 967 | 1,575.775 s | 1,576.003784 s | 1,455.748 s | 371,328 | 19,589,103 | 11,508,452 | 77,958 |
| 256 | 757 | 1,573.871 s | 1,576.007320 s | 1,416.307 s | 290,688 | 16,252,228 | 9,402,319 | 64,608 |

The narrow arm's final rollout stopped 3.784 ms past the CPU deadline. Its terminal checkpoint publication took 22.538 ms of CPU. The wide arm's final PPO update stopped 7.320 ms past the deadline. Its terminal checkpoint publication took 50.190 ms of CPU.

Verified the weights, metadata, SHA-256, and last-completed-update rule for all 192 snapshots in [the checkpoint receipt log](checkpoint-receipts.jsonl). Both arms covered all 120 class, level, and seat combinations.

The original logs are preserved in [width 128](narrow/training.jsonl.gz) and [width 256](wide/training.jsonl.gz). The decompressed SHA-256 digests are `5e8d4704aaa9950505c79c490ce639cc95ae4f05dd9ded41eedf68fb645878a0` and `a26b4f836d8312e0f40eefb5dd6dc83a2de37ff84a35459fa7cd7e261751ca8a`.

The training-plus-DEV CPU comparison uses the [width 128 final checkpoint](narrow/checkpoints/cpu-100.npz) and the [width 256 audit checkpoint](wide/cpu-matched.npz) at update 719. The latter has the same SHA-256 as `audit-091.npz` in the receipt log. The audit grid leaves a 9.914 CPU-second difference between the two retained endpoints after adding the four DEV evaluations per arm.
