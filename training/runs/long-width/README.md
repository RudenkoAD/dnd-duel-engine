# Extended width-256 league

This run starts from the previous [wide endpoint](../width-league/wide/checkpoints/cpu-100.npz) at 757 completed updates and 290,688 duels. It uses its weights and restores the four anchors plus all 11 historical league members, including their bout counts and learner-score EMAs. The eight snapshots held in [origin-history](origin-history) are bit-for-bit copies of the temporary archives. The three other snapshots are the original committed 25%, 50%, and 75% candidates. The runner checks each digest against both the previous training event log and the origin checkpoint.

The original Adam state was unavailable. This run uses fresh Adam with epsilon 1e-5 at the previous learning-rate floor of 0.00004. It retains the paired ordinary league and update recipe, with 384 duels, three PPO epochs, minibatches of 768, exactly 96 all-live duels and exactly 96 unequal-level duels per update. Growth occurs at every 64th cumulative update. The first new snapshot joins at update 768. Fixture, rollout, PPO, matchmaking and history draw streams use the fresh B2T namespace `4232540000000000`.

| Checkpoint | Cumulative updates | Cumulative duels | New updates | New duels |
| --- | ---: | ---: | ---: | ---: |
| 1× | 967 | 371,328 | 210 | 80,640 |
| 2× | 1,934 | 742,656 | 1,177 | 451,968 |
| 3× | 2,901 | 1,113,984 | 2,144 | 823,296 |

Run from the repository root with the original frozen arena ABI 2002 native binary. Its SHA-256 is `621964e798659ba534fab6c8494664c8e7f9d91abb9a4e28ccb0ed9f40888e3f`. The original run used the temporary archive path as its source. The copied archives provide the same inputs for reproduction.

```sh
export PYTHONPATH=/tmp/dnd-spec-frozen-621964e7/python
python_bin=python3.10
output_root=$(mktemp -d /tmp/dnd-long-width-reproduction.XXXXXX)
"$python_bin" training/long_width_league.py \
  --run-dir "$output_root/run" \
  --archive training/runs/long-width/origin-history
```

For a new run with a compatible build on another system, supply its library digest with `--native-sha256`. The runner verifies ABI 2002 and records the actual digest in the start log and each checkpoint. A resumed run requires the same native library used to start it.

Each completed update atomically replaces `resume.pt` with the model, Adam state, fixture RNG and permutations, league counters, coverage counters, and JSONL offset. The Torch and matchmaking seeds are derived from the absolute update number. After an interruption, pass `--resume` with the same arguments and use the same Python and Torch environment. The runner restores the last committed update and republishes its log entry. The `STOP` file allows a clean pause at the next completed update. Remove it before resuming.

The primary checkpoints appear in `checkpoints/` with a sibling JSON receipt published after the weight file. The receipt's `cpu_s` and `elapsed_s` measure continuation time through PPO, excluding initialization and publication of that same update. The terminal log event includes end-of-run publication time. Duel and decision counters include the origin run, while `new_bouts`, class-level-seat coverage and termination counts cover the continuation.

Run the training audit from the repository root. It verifies the exact update sequence, allocation and termination counts, finite PPO metrics, milestone digests and metadata, and snapshot hashes and bout totals at each published checkpoint.

```sh
"$python_bin" training/runs/long-width/verify.py
```

## Completed run

| Milestone | Continuation CPU s | Continuation wall s | Cumulative learner decisions | Cumulative frozen decisions | Cumulative PPO minibatches |
| --- | ---: | ---: | ---: | ---: | ---: |
| [1×](checkpoints/update-0967.json) | 442.283 | 398.263 | 20,869,386 | 12,067,038 | 82,977 |
| [2×](checkpoints/update-1934.json) | 2,352.697 | 2,103.224 | 43,481,762 | 24,551,010 | 172,728 |
| [3×](checkpoints/update-2901.json) | 4,193.406 | 3,737.684 | 66,179,405 | 37,026,272 | 262,839 |

The run ended at the exact budget without restarting. Including final publication it used 4,193.464 process CPU seconds and 3,737.742 wall seconds. The continuation covered all 120 class × level × seat cells, with at least 6,993 learner appearances in each cell at 3×. The final league contains the four anchors, 11 origin snapshots and 34 new history snapshots. There were 278,496 all-live duels and 278,496 mixed-level duels across both training runs, each exactly 25% of all completed duels.

The [verification manifest](verification.json) records all three milestone hashes and the final [recovery state](resume.pt) hash. The audit checked that the recovery state's weights and league match the 3× weights and metadata, its fixture generator is at the exact continuation offset, and its Adam state contains finite first and second moments for all eight parameter tensors after 198,231 new minibatches. To preserve those moments for a later training budget, keep this file together with [new history](history), [origin history](origin-history), the previous published checkpoints and the anchor files.
