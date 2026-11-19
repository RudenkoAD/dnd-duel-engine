# Matched frozen-specialist league experiment

Both main learners start from ABI 2002 checkpoint `training/runs/league/checkpoints/update-1300.npz`
(SHA-256 `96f9bddabc0eeff4a70c111db443dea064490c6e576ff8bb569966024cb26e31`) and retain it as an anchor.
Both also use the following historical anchors, verified against the start event of league run 302:

| Path | SHA-256 |
| --- | --- |
| `models/arena_policy.npz` | `d81965d6d1554a3df34c6265551e637b243224988b7c7efe7bbec679c719353e` |
| `training/runs/viewer_relative/control.npz` | `99b99391f0d80b090ae74d93ce25d7011956da836bdd2f5ad64293b1ff5f3f00` |
| `training/runs/viewer_relative/target-aware-update-0090.npz` | `c9be34b39f4793e84bd051a1e6d66f697a6880051ed86f543f5d6df4adb6b480` |

The primary budget is 460,800 training duels per arm, with 384 duels per update.
Control uses 1,200 main updates. Treatment uses 800 main updates and 200 updates
for each of two specialists. All specialist training is charged to treatment regardless
of which specialist checkpoints their DEV protocol selects.

Both main branches use run ID 414 and share paired fixtures and fixture seeds
over their common prefix, with the same per-update Torch seeds for each rollout
and optimization. Actions and minibatches can diverge with the trajectories.
Both use two Torch worker threads, 768-example minibatches, three PPO epochs,
and a common linear learning rate schedule from 8e-5 to 4e-5 over 600 main updates.

Live self-play covers exactly 25% of fixtures. From update 101, 37.5% of
fixtures in each arm use growing snapshots. Treatment assigns exactly 25% of total
fixtures to specialists, with equal shares, replacing historical-anchor fixtures.
Specialist opponents use the deployed greedy NumPy Policy.choose inference used for their DEV selection.
The learner, historical anchors and growing snapshots use sampled actions.
Matchmaking prioritizes difficult anchors and growing snapshots.
Regular checkpoints publish atomically every 100 updates and join the growing pool.
Audit checkpoints publish atomically at the other multiples of 10.

The secondary endpoints match process CPU time at the smaller of the full control
and treatment charges. Treatment includes both specialist training and their
selection DEV evaluations. Each endpoint is the latest checkpoint whose logged
cumulative process CPU charge is at or below the cap, with the specialist charge
added to treatment's checkpoint times. Selection uses CPU logs before endpoint
scores. Logs also record wall time, learner and frozen decisions, and optimizer
minibatches.

The original league schedule gives 25% of main duels unequal levels. Specialist
preparation instead samples only equal-level pairs across all four maps against
the frozen champion. The primary endpoint equalizes completed training duels
across those curricula. The CPU endpoint measures their different workloads.
Main-branch DEV curve evaluations are reported separately from the CPU cap.

The pinned native library SHA-256 was
`621964e798659ba534fab6c8494664c8e7f9d91abb9a4e28ccb0ed9f40888e3f`.
Both runs use Python 3.10.15 with NumPy 2.2.6 and Torch 2.9.1.

The treatment uses the specialist checkpoints selected before training its main learner:

| Path | SHA-256 |
| --- | --- |
| `training/runs/specialists/champion-start/update-0150.npz` | `50833cbb06ea875b724c008eb0d255b1a4da93a3313c78e5520b918efe000923` |
| `training/runs/specialists/control-start/update-0200.npz` | `6f78550df56d6c030d0624373ddeb72e85d60f11fdcfabb99c7e473675bb77da` |

The exact training command for each branch can be reconstructed from the start
event in its log. Run from the repository root with `PYTHONPATH=python` and the
pinned native build under `target/release`. When using the current league
launcher, add `--legacy-schedule`. For control at the historical revision, use:

```sh
python3.10 training/league_play.py --matched --audit-every 10 \
  --start training/runs/league/checkpoints/update-1300.npz \
  --anchor models/arena_policy.npz \
  --anchor training/runs/viewer_relative/control.npz \
  --anchor training/runs/viewer_relative/target-aware-update-0090.npz \
  --run-dir /tmp/arena-control-414 --run-id 414 --updates 1200 \
  --batch 384 --threads 2 --checkpoint-every 100
```

For treatment, use a fresh run directory, set `--updates 800`, and add each
specialist path above as a separate `--specialist` argument in table order.

The archive keeps regular checkpoints and the selected treatment CPU checkpoint
under their original paths. `resources.json` records the charged totals and both
CPU endpoints. `checkpoint-receipts.jsonl` records the verified SHA-256 and
location for every regular or audit checkpoint. Unselected audit binaries
and plaintext logs are available locally under `/private/tmp/dnd-414-league-archive`.
Each arm keeps its full `training.jsonl.gz` in the repository.
