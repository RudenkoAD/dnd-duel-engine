# Arena policy training

## Level-five roster self-play

New runs of both `training/league_play.py` and `training/self_play.py` use all 36 builds at level 5. Each training sweep covers every ordered build pair once. Each fixture is played with the same arena and seed in both seat orientations. This gives each build equal representation in each seat. The self-play development schedule rotates arena assignments across sweeps.

Use a separate output directory with an environment that has NumPy and PyTorch. The ordinary prioritized league supports wide and narrow frozen opponents together. This example starts the learner from the 256-unit final wide policy and uses two independent historical anchors.

```sh
make release
TRAIN_PYTHON=python3.10
PYTHONPATH=python "$TRAIN_PYTHON" training/league_play.py \
  --start training/runs/long-width/checkpoints/update-2901.npz \
  --anchor training/runs/league/checkpoints/update-1300.npz \
  --anchor training/runs/viewer_relative/control.npz \
  --run-id 1 --run-dir training/runs/roster-league
```

Warm starts and anchors from before the roster update zero their new observation input weights in memory. These inputs can learn in the new policy. Existing checkpoint files remain unchanged.

The current launchers accept `--legacy-schedule` to use the historical level 1–5 fixtures. For exact reproduction, run the archived commands in their recorded source revisions, where those fixtures are the default.

## Longer training for the 256-unit league policy

The previous width comparison gave the 128-unit policy 371,328 training bouts and the 256-unit policy 290,688 bouts. This continuation starts with the wide policy from update 757. It compares a fixed trajectory at 371,328, 742,656 and 1,113,984 cumulative bouts, corresponding to updates 967, 1934 and 2901. These counts cover training since the width experiment began, excluding the shared champion's earlier history. Only the wide learner receives additional training. The final threefold budget adds 823,296 bouts and 2,144 updates.

The main comparisons use the fixed terminal checkpoint regardless of development performance. Its fresh matchups against the historical league and the enhanced bot measure gains over the previous wide policy, the previous narrow policy and the starting champion 1300. The onefold checkpoint tests widths at equal numbers of bouts. The twofold checkpoint and the best of the three checkpoints on development seeds show how the trajectory evolved. The budget is measured in bouts, with CPU time reported separately.

The terminal checkpoint scored 51.72% head-to-head against the narrow policy, 51.62% against the original wide policy, and 53.15% against champion 1300. The onefold and twofold checkpoints scored 50.10% and 49.62% against the narrow policy. The terminal policy scored 55.84% against the three historical opponents and 71.76% against the enhanced bot. Paired score gains over champion 1300 were positive for both groups. The intervals for gains over the narrow policy and the earlier wide policy included zero on these shared opponents. The continuation used 4,193.46 process CPU seconds and 62 minutes 18 seconds of wall time. The old Adam state was unavailable at the start of the continuation. The final weights, optimizer state and frozen league are available for further training. [Training and recovery](runs/long-width/README.md) · [Development curve](../evaluation/long-width-dev/README.md) · [Independent evaluation](../evaluation/long-width-results/README.md)

To play against the terminal policy, run the following command in the repository root.

```sh
make play PLAY_ARGS="--checkpoint training/runs/long-width/checkpoints/update-2901.npz"
```

## Hidden width at a fixed CPU budget

The comparison starts two ordinary league learners from checkpoint 1300. The narrow network uses two 128-unit hidden layers. The wide network starts with two 256-unit layers by duplicating hidden activations and splitting outgoing weights. The historical opponents, game rules, observations, and action masks are common to both arms. The experiment gives each learner 1,576 process CPU seconds and uses four development checkpoints per architecture. The CPU measurement includes time on all training threads. The number of training bouts and PPO updates can differ between widths.

Development selection weights the mean score against the three historical policies and the score against the enhanced bot equally. Models and training-time endpoints are frozen before testing on new W1F fixtures. The comparisons estimate wide minus narrow at equal training cost, and each finalist minus checkpoint 1300. We use the previous plateau criterion of 0.5 percentage points to interpret gains for each opponent group. A separate endpoint comparison charges the four development evaluations to each arm as well. A single paired training run estimates performance for the resulting policies; the reported combat intervals describe uncertainty across the matched test fixtures.

Both branches selected their last checkpoint. The 128-unit network completed 371,328 training duels and scored 55.30% against the three historical opponents and 71.29% against the enhanced bot. The 256-unit network completed 290,688 duels and scored 55.18% and 71.16%, respectively. The two finalists split their head-to-head match almost evenly (50.02% to the wider network). Historical matchups yielded 1,177 draws for the wide finalist and 834 for the narrow finalist. Under the additional common budget for both training and model selection, the wide-minus-narrow score differences were −0.88 and +0.10 percentage points. [Warm-start validation](runs/width-league/initialization.md) · [Training budget and steps](runs/width-league/README.md) · [Development curve](../evaluation/width-dev/README.md) · [Independent final evaluation](../evaluation/width-results/README.md)

## Two specialists against the league champion

Both specialists train against the same frozen opponent, league checkpoint 1300, using all 12 classes and all four arenas. One starts from the champion's weights and one from the independent ABI 2002 control. The main comparison gives each league 460,800 training duels. The specialist league spends one third of its budget creating opponents and the remaining two thirds on its main policy. The ordinary league spends its entire budget on its main policy. [Training protocol](runs/specialist-leagues/protocol.md)

Before the final evaluation, each branch selects the highest development score at four equal cumulative budget landmarks. The score gives equal weight to the mean against three historical policies and the result against the native expert. Final comparisons use fresh paired fixtures for each opponent. The two questions are whether each branch improves on checkpoint 1300 and whether the specialist league improves on ordinary continuation. We report both raw wins and half-draw scores. A gain of 0.5 percentage points uses the scale of the earlier plateau rule. Paired intervals measure variation across combat fixtures for the policies produced in this experiment. A single training seed per branch limits inference about the training method across independent runs.

The selected specialist-league policy (main update 200) scored 54.01% against the historical opponents and 70.67% against the expert. Ordinary continuation (main update 1000) scored 54.69% and 70.93%, and the starting champion scored 54.04% and 69.97%. All simultaneous 95% intervals for the differences included zero. The secondary comparison at a common CPU budget also found no clear advantage for either league. [Specialist training](runs/specialists/README.md) · [Main-policy selection](../evaluation/specialist-dev/README.md) · [Final results and reproduction commands](../evaluation/specialist-results/README.md) · [CPU accounting](runs/specialist-leagues/resources.json)

## Historical-opponent league on arena ABI 2002

Run 302 starts from the trained relative-target policy (run 102, update 180). Three frozen policies stay in the league throughout the run: the starting policy, its update-90 predecessor and the independently trained control from run 101. Every 100 PPO updates another frozen version of the learner joins the pool. About a quarter of games use the current learner on both sides. After the first snapshot joins, the remaining fixtures split evenly in expectation between the three original opponents and new snapshots. The sampler favours opponents against which the learner has scored poorly and retains a uniform sampling component. Both seats use learned policies throughout training. The native expert participates only in evaluation.

The [development protocol and curve](../evaluation/league-dev/README.md) use the same three opponents throughout, with equal weight for each. Development selection gives half the weight to the neural league and half to the native expert. The final comparison uses separately generated combat seeds. All evaluations compare deterministic play using the highest-scoring legal action. Training samples legal actions from the policies.

To reproduce training from revision `0260a49` of the starting weights, use the installed training dependencies (NumPy and PyTorch), build the Rust engine with `make release`, and choose a fresh run directory.

```sh
PYTHONPATH=python python3.10 training/league_play.py \
  --start models/arena_policy.npz \
  --anchor training/runs/viewer_relative/control.npz \
  --anchor training/runs/viewer_relative/target-aware-update-0090.npz \
  --run-id 302 --updates 5000 --run-dir /tmp/arena-league-reproduction
```

The run writes a checkpoint every 100 updates and records all training metrics in `training.jsonl`. Creating an empty `STOP` file in its run directory stops training after the current update. The reproduction limit can be set to the exact desired checkpoint update.

The registered plateau rule selected update 1,300 after 499,200 training games. The original trainer completed 64 further updates while the concurrent development evaluation finished, stopping after 523,776 games in 23 minutes 15 seconds. Only the 100-update milestones entered model selection. The [training ledger](runs/league/training.jsonl.gz) records all 1,364 updates. The [development curve](../evaluation/league-dev/curve.png) and the [independent final comparison](../evaluation/league-results/README.md) report both raw wins and scores with half credit for draws. To play against the selected checkpoint, run the following command in the repository root.

```sh
make play PLAY_ARGS="--checkpoint training/runs/league/checkpoints/update-1300.npz"
```

## Viewer-relative self-play on arena ABI 2002

The ABI 2002 warm start comes from the archived ABI 2001 continuation. The 1,024 input features and 128 action IDs keep their dimensions. Target-linked inputs now use viewer-relative ordering, including object and creature footprint slots, target masks and stored target IDs. Migration reinterprets the action head and preserves every weight and bias. It records `stage=reinterpreted_warm_start` and the original checkpoint's ABI and SHA-256. The warm-start SHA-256 is `6f46d8839f5406b6b399cdb227dc3c2d08ba1b29508a89966427c9be77da845d`. An ABI 2002 arena rejects ABI 2001 checkpoints until they are explicitly reinterpreted.

Two independent PPO runs started from this same warm start. Both trained exclusively on games between the current policy and other learned policies, including frozen snapshots and the warm start. The control kept the existing repeated-choice and timeout penalties. The second run added a 0.04-weighted auxiliary loss in phase 80 when both adventurers were legal projectile targets. The loss computes the negative log probability of selecting the foe conditional on selecting either adventurer, pooling single-projectile and all-darts IDs. PPO rollouts and inference use the original legal choices. Scripted opponents are used for development evaluation. The two runs use different training seeds, and the branch comparison combines objective and seed effects.

Reproduce the warm start and both branches with the ABI 2002 simulator from revision `600436d`. Use a Python environment with NumPy and PyTorch. The original runs used Python 3.10, NumPy 2.2.6 and PyTorch 2.8.0 on macOS arm64.

```sh
make release
TRAIN_PYTHON=python3.10
RUN_DIR=training/runs/viewer_relative_reproduction
WARM="$RUN_DIR/warm.npz"
export PYTHONPATH="$PWD/python"
"$TRAIN_PYTHON" training/migrate_abi2001.py \
  training/runs/self_play/continuation.npz "$WARM"

"$TRAIN_PYTHON" training/self_play.py --warm "$WARM" --run-id 101 \
  --updates 180 --batch 384 --epochs 3 --eval-every 30 --add-every 24 \
  --dev-batches 1 --lr 0.00016 --final-lr 0.00006 --seconds 420 \
  --repeat-cost 0.02 --timeout-penalty 0.35 \
  --log "$RUN_DIR/training.jsonl" --output "$RUN_DIR/control.npz"

"$TRAIN_PYTHON" training/self_play.py --warm "$WARM" --run-id 102 \
  --updates 180 --batch 384 --epochs 3 --eval-every 30 --add-every 24 \
  --dev-batches 1 --lr 0.00016 --final-lr 0.00006 --seconds 420 \
  --repeat-cost 0.02 --timeout-penalty 0.35 --projectile-target-weight 0.04 \
  --log "$RUN_DIR/training.jsonl" --output "$RUN_DIR/target-aware.npz" \
  --checkpoint-dir "$RUN_DIR/target-aware-checkpoints"
```

For subsequent self-play, use the reconstructed ABI 2002 warm start as `--warm` and the trained default as `--resume`. Write the new checkpoint to a separate output.

In a matched development check against a uniform legal-action opponent, the east-seat policy aimed at itself on 547 of 3,334 eligible projectile decisions on ABI 2001 and 2,238 of 3,139 after the weight transfer to ABI 2002. Each denominator counts phase 80 decisions where the opposing adventurer was a legal target. The east-seat record went from 2,804 wins, 39 losses and 37 draws to 2,215 wins, 632 losses and 33 draws. The west-seat record stayed at 2,814 wins, 15 losses and 51 draws, with self-aim counts of 52 of 4,446 and 51 of 4,441. Both versions used the same `S2D` fixtures and sampled the opponent's legal actions in physical action-ID order. The counts measure aim selections.

### Frozen ABI 2002 policy

`models/arena_policy.npz` contains run 102 update 180, SHA-256 `d81965d6d1554a3df34c6265551e637b243224988b7c7efe7bbec679c719353e`. Both runs reached all 120 class, level and seat combinations.

| Run | Updates | Bouts | Learner decisions | Frozen policy decisions | Minimum games per cell | Elapsed seconds |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 101, control | 180 | 69,120 | 4,006,436 | 1,848,816 | 655 | 208.6916 |
| 102, projectile loss | 180 | 69,120 | 3,520,051 | 1,692,058 | 628 | 191.4212 |

The independent `DEV/target-probe-v1` comparison uses 2,880 fixtures, each played in both seats against each opponent. The selected model scored 70.15% against the scripted baseline and 69.24% against the tactical expert, counting draws as half a point. The control scored 69.09% and 67.85%. Both trained models made zero self-aim selections when the opposing adventurer was legal across these opponents and uniform random play. The selection favored run 102 for its higher scripted-opponent scores and uniform-opponent win rates.

| Policy | Uniform W–L–D west | Uniform W–L–D east | Self-aim with legal foe west / east | Offensive aims per bout west / east |
| --- | ---: | ---: | ---: | ---: |
| Transferred warm start | 2,815–20–45 | 2,228–629–23 | 49/4,267 · 2,205/3,110 | 1.48 / 0.32 |
| Control | 2,820–9–51 | 2,850–7–23 | 0/4,628 · 0/3,801 | 1.66 / 1.33 |
| Selected | 2,836–8–36 | 2,855–3–22 | 0/4,461 · 0/3,936 | 1.57 / 1.38 |

The selected model still made five west-seat self-aim choices against uniform random when the opposing adventurer was unavailable. East made zero self-aim choices in all projectile windows against this opponent.

The [control checkpoint](runs/viewer_relative/control.npz), [shared training ledger](runs/viewer_relative/training.jsonl.gz) and [development measurements](runs/viewer_relative/development.json) preserve the comparison. To rerun the ABI 2002 development probe, use the warm-start path produced above.

```sh
PYTHONPATH=python "$TRAIN_PYTHON" training/target_probe.py \
  "$WARM" training/runs/viewer_relative/control.npz models/arena_policy.npz \
  --opponents uniform,heuristic,expert,self --replicates 4
```

## Historical two-sided self-play on arena ABI 2001

The previous 1003 policy is archived as `training/checkpoints/arena_policy_abi1003.npz`. The ABI 2001 warm start has 1,024 inputs and 128 outputs. It preserves the first 256 input weights and first 32 output logits, adds zero weights for new observations, and initializes new output logits at −1.25. This is a parameter transfer. Expanded arena mechanics still change what the observations and masks can contain. A migrated warm start has `stage=migrated_warm_start` and is used as a reference opponent and initialization for self-play; a trained checkpoint has `stage=self_play_ppo`.

Both sides in self-play act from masked learned policies, including attack targets, reactions, movement and any further choices. In a portion of bouts the current actor-critic plays both seats and both trajectories feed PPO. In the remaining bouts one seat uses the current policy and the other uses a frozen policy from the checkpoint pool. A pool entry stays frozen while it is in use. The original migrated reference remains in the pool throughout. Each actor's value training follows that seat's own decision sequence even when another actor interrupts, and receives that seat's own terminal reward.

The training schedule gives three out of four pairs to equal-level bouts and cycles through all 390 same-level matchups, including mirrors. The remaining quarter cycles through unequal-level matchups. Across training, all 25 choices of the two seat levels are represented. Each paired fixture uses the same fight seed, classes and levels for both legs and assigns the current policy to both physical seats against the same frozen snapshot where applicable. Development and final model comparisons use equal-level fights. Randomized learned actions use a run-specific seed. The independent seed namespaces are `S2T` for training, `S2D` for development and `S2F` for final evaluation. The separate final namespace is reserved until the selected model has been frozen. Development selection evaluates equal-size sets against the scripted baseline, tactical expert and migrated reference, weighted 40%, 30% and 30% respectively. The migrated reference samples legal actions from a seeded distribution. The current model is evaluated greedily and in a separate same-policy check that penalizes native decision-limit or turn-limit endings. Optional `--repeat-cost 0.02` trains against identical public state and action repetitions by the same seat within a turn. Match reports continue to count native wins, losses and draws.

### Verify migration and the sampler

```sh
TRAIN_PYTHON=python3.10
PYTHONPATH=python:training "$TRAIN_PYTHON" -m unittest discover -s training/tests -v
```

The tests perform forward inference and exercise a mock two-sided fight. They use no optimizer, training duels or held-out seeds.

### Reproduce the two-sided training

Use the ABI 2001 checkout at revision `690fe54` for these historical commands. Build its native library first. Use a fresh output directory for a reproduction and retain one shared JSONL ledger across its runs. The ledger caps this round at five hours of elapsed local training and reports the historical arena training separately. The original jobs used Python 3.10.15, PyTorch 2.9.1 and NumPy 2.2.6. Different hardware and library versions may change the resulting weights when sampling or floating-point reductions differ.

```sh
make release
export PYTHONPATH="$PWD/python"
TRAIN_PYTHON=python3.10
RUN_DIR=training/runs/self_play_reproduction
WARM=training/checkpoints/arena_policy_migrated_abi2001.npz
"$TRAIN_PYTHON" training/migrate_abi1003.py \
  training/checkpoints/arena_policy_abi1003.npz "$WARM"

"$TRAIN_PYTHON" training/self_play.py \
  --warm "$WARM" --run-id 1 --updates 120 --batch 384 --epochs 3 \
  --eval-every 20 --add-every 20 --dev-batches 1 \
  --lr 0.0002 --final-lr 0.0001 --seconds 720 --repeat-cost 0 \
  --log "$RUN_DIR/training.jsonl" --output "$RUN_DIR/ablation_unshaped.npz"

"$TRAIN_PYTHON" training/self_play.py \
  --warm "$WARM" --run-id 2 --updates 120 --batch 384 --epochs 3 \
  --eval-every 20 --add-every 20 --dev-batches 1 \
  --lr 0.0002 --final-lr 0.0001 --seconds 720 --repeat-cost 0.02 \
  --log "$RUN_DIR/training.jsonl" --output "$RUN_DIR/ablation_shaped.npz"

"$TRAIN_PYTHON" training/self_play.py \
  --warm "$WARM" --resume "$RUN_DIR/ablation_shaped.npz" \
  --run-id 3 --updates 650 --batch 384 --epochs 3 \
  --eval-every 50 --add-every 40 --dev-batches 2 --pool-size 8 \
  --lr 0.00015 --final-lr 0.00004 --seconds 2400 \
  --repeat-cost 0.02 --timeout-penalty 0.35 \
  --log "$RUN_DIR/training.jsonl" --output "$RUN_DIR/continuation.npz"
```

The first two trained candidates use independent training seeds and identical development fixtures. In the expanded 1,980-game development comparison per opponent, the shaped candidate won 1,210 games against the standard baseline with 34 draws. The unshaped candidate won 1,136 with 337 draws. The shared selection criterion accounts for draws as half points and penalizes native engine limits in the independent greedy mirror. The higher-scoring shaped candidate continued into run 3 with a larger snapshot pool and a stronger symmetric penalty for exhausting a native match limit.

### Frozen policy and evidence

The archived `training/runs/self_play/continuation.npz` contains the selected self-play run 3 update 650. Its SHA-256 is `fd8a81371247b42ba186a9f88e318fe2afab0ddfd131aad2b793a6fffd85cb6b`. It uses ABI 2001 with 1,024 observation features and 128 action IDs. The underlying simulator was frozen at revision `70408a3` before this training round. The initial ABI 1003 policy remains available at `training/checkpoints/arena_policy_abi1003.npz`, SHA-256 `23233928f6a50071222f6aaddc3ce632d944e31f6bad4a94847ec3c749371511`.

The strongest continuation scored 67.85% against the standard baseline, 66.67% against the tactical expert and 73.03% against the stochastic migrated reference in the expanded development comparison, with 1,980 games per opponent. Its independent equal-level check against the standard baseline yielded 868 wins, 424 losses and 28 draws under greedy play, and 828 wins, 474 losses and 18 draws under sampling, each over 1,320 games. On the separate 660 unequal-level games, its greedy record was 374–279–7 and the sampled record was 377–275–8. The equal-level independent greedy policy recorded more wins than the corresponding baseline in each of the twelve classes.

In the independent 660-game learned mirror, greedy play reached the decision limit four times and the ordinary turn limit twenty times. The repeated public choices in the four decision-limit games were modal cancellations around Barbarian styles or Bard Font of Inspiration. Sampling reached no decision limits and seventeen turn limits. These diagnostics used development seeds. Final held-out results were not used to choose or change the model.

| Run | Selected checkpoint | Updates | Bouts | Paired fixtures | Unequal-level bouts | Learner decisions | Snapshot decisions | Elapsed seconds |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | `ablation_unshaped.npz` | 120 | 46,080 | 23,040 | 11,520 | 3,280,779 | 1,618,876 | 188.2958 |
| 2 | `ablation_shaped.npz` | 120 | 46,080 | 23,040 | 11,520 | 1,942,584 | 1,323,368 | 129.1306 |
| 3 | `continuation.npz` | 650 | 249,600 | 124,800 | 62,400 | 11,944,928 | 6,476,871 | 729.8906 |
| **Total** | | **890** | **341,760** | **170,880** | **85,440** | **17,168,291** | **9,419,115** | **1,047.3170** |

The round used **17 minutes 27.317 seconds** of local training time, including every executed update and in-run development evaluation. Across the round, both seats used the current network in 119,748 bouts. The other bouts paired the learner with frozen learned snapshots. Run 3 completed at least 2,412 bouts with learner decisions per class, level and seat cell. The training ledger and each branch winner are in `training/runs/self_play`, along with the expanded comparison, independent development assays and separate greedy and sampled loop reports.

## Historical training on arena ABI 1002 and 1003

The archived policy uses 256 public-state features and masks its 32 action logits before choosing an action. Two 128-unit tanh layers feed policy and value heads. Training uses PyTorch on CPU. Playing and evaluating the exported NumPy checkpoint only requires NumPy and the arena library.

The first run clones choices from the Rust tactical expert. Its learner gradually takes over decisions while the expert continues to label the encountered states. Clipped PPO then learns from wins, losses and draws. Training cycles through all same-level matchups from levels 1–5, including mirrors, with the learner playing both seats. Opening knockouts without a learner decision are recorded separately. Training rewards use an HP potential and, in the mixed run, small costs for a movement action or a same-turn tile revisit.

The mixed runs use 50% standard baseline, 20% tactical expert, 10% uniform legal actions and 20% stochastic decisions from an earlier frozen checkpoint. The final continuation anneals the learning rate and adds two small costs. It penalizes Second Wind at full HP before Tactical Shift is available and paying for a rider when the displayed post-resistance damage is already enough for a knockout. The latter excludes Monk and Rogue defenders because their damage reductions can still be pending. These costs apply only during training. At play time the network uses the native move and reaction mask.

### Runtime and checkpoint lineage

The recorded runtime was Python 3.10.15, PyTorch 2.9.1 and NumPy 2.2.6 on macOS. The first four runs used arena ABI 1002, before selectable ground-target decisions. Their selected warm start is archived at `training/checkpoints/arena_policy_pre_target.npz`. Its run ID is 3, its update is 2,300, and its SHA-256 is `db47570f970142411098a4649eebaf4ec720fbe548ab7aec3f7bd7e98a6ebf30`.

### Historical pre-target recipe

Run this recipe against the pre-target arena ABI 1002. The commands write into `training/runs/reproduction`. Choose a fresh output directory for repeat experiments because the JSONL log is append-only.

```sh
make release
export PYTHONPATH="$PWD/python"
TRAIN_PYTHON=python3.10
TRAIN_DIR=training/runs/reproduction
mkdir -p "$TRAIN_DIR"

"$TRAIN_PYTHON" training/train_arena.py \
  --seconds 1200 --updates 150 --expert-dev \
  --output "$TRAIN_DIR/initial.npz" --log "$TRAIN_DIR/training.jsonl"

"$TRAIN_PYTHON" training/train_arena.py \
  --seconds 900 --updates 925 --cloning-updates 0 --run-id 1 \
  --eval-every 25 --imitation 0.01 --entropy 0.012 \
  --resume "$TRAIN_DIR/initial.npz" --output "$TRAIN_DIR/frozen_opponent.npz" \
  --log "$TRAIN_DIR/frozen_prefix.jsonl"

"$TRAIN_PYTHON" training/train_arena.py \
  --seconds 900 --updates 1800 --cloning-updates 0 --run-id 1 \
  --eval-every 25 --imitation 0.01 --entropy 0.012 \
  --resume "$TRAIN_DIR/initial.npz" --output "$TRAIN_DIR/baseline_ppo.npz" \
  --log "$TRAIN_DIR/training.jsonl"

"$TRAIN_PYTHON" training/train_arena.py \
  --seconds 1500 --updates 1000 --cloning-updates 0 --run-id 2 \
  --dev-repeats 3 --eval-every 25 --imitation 0.01 --entropy 0.01 \
  --move-cost 0.008 --revisit-cost 0.06 --select-revisit-weight 0.20 \
  --resume "$TRAIN_DIR/baseline_ppo.npz" --frozen-opponent "$TRAIN_DIR/frozen_opponent.npz" \
  --output "$TRAIN_DIR/mixed_ppo.npz" --log "$TRAIN_DIR/training.jsonl"

"$TRAIN_PYTHON" training/train_arena.py \
  --seconds 1800 --updates 2400 --cloning-updates 0 --run-id 3 \
  --dev-repeats 3 --eval-every 50 --lr 0.00015 --final-lr 0.00006 \
  --imitation 0.006 --entropy 0.008 --move-cost 0.008 --revisit-cost 0.06 \
  --select-revisit-weight 0.20 --full-hp-wind-cost 0.015 --lethal-rider-cost 0.04 \
  --resume "$TRAIN_DIR/mixed_ppo.npz" --frozen-opponent "$TRAIN_DIR/mixed_ppo.npz" \
  --output "$TRAIN_DIR/finetuned_ppo.npz" --log "$TRAIN_DIR/training.jsonl"
```

The original run copied the frozen opponent during baseline training at update 925. The reconstruction above replays that prefix separately to recover the snapshot without timing a live copy. The baseline branch resumed the best update from the initial run. The first mixed branch resumed the best update of the baseline branch. The final branch used the first mixed branch's selected model both as its initialization and as its new frozen opponent. Different hardware or library versions can change stochastic decisions and checkpoint selection.

Checkpoint selection uses development seeds disjoint from both training and the final evaluation. The mixed branch evaluates 1,980 games against each scripted opponent style and weights the standard and expert results 70% and 30%, respectively. It subtracts 0.20 times the same-turn revisit fraction for each style. Match score counts a win as 1 and a draw as 0.5. To use a reproduced candidate, pass its path to the play or evaluation CLI. To replace the repository checkpoint deliberately, copy the selected reproduction checkpoint to `models/arena_policy.npz`.

### Ground-target continuation

Use the historical arena ABI 1003 and the archived pre-target checkpoint to reproduce the continuation. The network still outputs 32 action logits, with a native mask that gives axis choices during target selection. The learner receives extra probability credit for placing a damaging area at a legal row or column with an effect equivalent to the tactical expert's choice. Identical enemy and self contact is sufficient when the enemy can be affected. Otherwise the eligible alternatives also match the expert's distance from the enemy. Caltrops use the expert's tactical placement directly. A second supervised signal teaches the learner to Pass when an optional on-hit rider would be redundant on an already-lethal hit. Neither signal runs during play.

```sh
make release
export PYTHONPATH="$PWD/python"
TRAIN_PYTHON=python3.10
TRAIN_DIR=training/runs/reproduction
mkdir -p "$TRAIN_DIR"
WARM_START=training/checkpoints/arena_policy_pre_target.npz

"$TRAIN_PYTHON" training/train_arena.py \
  --seconds 1800 --updates 1600 --cloning-updates 0 --run-id 4 \
  --dev-repeats 3 --eval-every 50 --lr 0.0002 --final-lr 0.00007 \
  --imitation 0.012 --entropy 0.01 --move-cost 0.008 --revisit-cost 0.06 \
  --select-revisit-weight 0.20 --full-hp-wind-cost 0.015 --lethal-rider-cost 0.04 \
  --lethal-rider-supervision 0.025 --target-supervision 0.08 --select-target-weight 0.035 \
  --resume "$WARM_START" --frozen-opponent "$WARM_START" \
  --output "$TRAIN_DIR/targeted_ppo.npz" --log "$TRAIN_DIR/targeted.jsonl"

"$TRAIN_PYTHON" training/train_arena.py \
  --seconds 900 --updates 800 --cloning-updates 0 --run-id 5 \
  --dev-repeats 3 --eval-every 25 --lr 0.00011 --final-lr 0.00004 \
  --imitation 0.008 --entropy 0.008 --move-cost 0.008 --revisit-cost 0.06 \
  --select-revisit-weight 0.20 --full-hp-wind-cost 0.015 --lethal-rider-cost 0.04 \
  --lethal-rider-supervision 0.016 --other-rider-supervision 0.025 \
  --target-supervision 0.035 --select-target-weight 0.035 --select-rider-weight 0.035 \
  --resume "$TRAIN_DIR/targeted_ppo.npz" --frozen-opponent "$TRAIN_DIR/targeted_ppo.npz" \
  --output "$TRAIN_DIR/balanced_ppo.npz" --log "$TRAIN_DIR/targeted.jsonl"
```

The placement-aware checkpoint score adds 0.035 times the share of targeting decisions equivalent to the expert's. Paired match results and movement quality still make up the rest of the selection score. The evaluation seed prefix is independent of the development schedule used here.

The last run also teaches the expert's rider action in other on-hit windows, with an independently weighted loss. This counteracts the first targeting run's tendency to Pass too often. Its selection score adds 0.035 times the mean of the clearly redundant Pass rate and the other rider spend rate.

### Archived checkpoint and elapsed training

The archived `training/checkpoints/arena_policy_abi1003.npz` is run 5, update 375, built with arena ABI 1003. SHA-256 `23233928f6a50071222f6aaddc3ce632d944e31f6bad4a94847ec3c749371511`. It was frozen using development results before the held-out evaluation. Its development match score was 63.94% against the standard baseline and 61.97% against the tactical expert, with 1,980 games against each. Against the standard baseline, 843 of 850 ground-target decisions matched the expert or had an equivalent effect. There were zero same-turn tile revisits across 7,060 movement decisions. The selected update had completed at least 1,757 training games in each of the 120 class × level × seat cells in its final run alone.

| Run | Stage | Executed updates | Learner decisions | Completed games with learner decisions | Elapsed (s) |
| ---: | --- | ---: | ---: | ---: | ---: |
| 0 | Expert cloning and initial PPO | 150 | 1,843,200 | 109,847 | 18.39 |
| 1 | Baseline-opponent PPO | 1,800 | 22,118,400 | 860,058 | 215.65 |
| 2 | Mixed-opponent PPO, movement costs | 1,000 | 12,288,000 | 681,543 | 257.75 |
| 3 | Pre-target fine-tuning | 2,400 | 29,491,200 | 1,612,417 | 515.66 |
| 4 | Ground-target training | 1,600 | 19,660,800 | 979,389 | 355.31 |
| 5 | Ground-target and resource refinement | 800 | 9,830,400 | 503,943 | 187.22 |
| **Total** | | **7,750** | **95,232,000** | **4,747,197** | **1,549.98** |

Elapsed time totals 25 minutes 49.98 seconds of sequential local run time. It includes training-time development checks and every executed update, including updates whose weights were discarded. The per-update record is in `models/arena_policy.jsonl`.
