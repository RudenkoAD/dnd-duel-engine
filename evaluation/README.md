# Held-out arena evaluation

For the current 36-build arena, [the level-five roster comparison](roster-parity-results/README.md) contains matched fixtures, results and the reproduction command. The historical evaluations below use their original class and level schedules.

The final seed domain has the high bytes `FIN`. Training uses `TRA` and checkpoint selection uses `DEV`. The remaining 40 bits of each final combat seed come from a stable BLAKE2b digest of the focal class, opponent class, level, arena and repeat. The evaluator checks combat seeds for collisions.

Every focal-class and level combination faces all 12 classes, including mirrors. Each class matchup at each level is tested on all four arena layouts. Rejection sampling over hashed seed candidates selects the requested layout using the Rust map selector. The default schedule has 2,880 two-seat pairs and 5,760 bouts per matchup. A pair plays both seat assignments with the same combat seed. The policy receives the legal action mask on its own normal turns and during reactions, according to the actual actor reported by Rust. RNG-driven choices for the masked random policy use separate per-seat streams.

The primary comparison estimates the change in score from replacing a uniformly random legal-action policy with the frozen learned policy against the same heuristic. Score is `(wins + 0.5 × draws) / bouts`. Each matchup reporting that score uses two-seat pairs as the bootstrap unit. The 95% percentile intervals stratify by focal class, level and arena, preserving both seats and both policies for the paired difference. The intervals reflect sampling variation on this schedule and checkpoint. Group intervals are descriptive and have no multiple-comparison adjustment.

The report includes class, level, arena and class-by-level breakdowns, the raw paired scores, and three replay candidates selected in schedule order when available. One trace shows a decision window for an opportunity attack, defense or rider against the heuristic. The second shows an opportunity-attack decision against masked random. The third shows a Burst or Moonbeam aiming decision against the heuristic. The evaluator reruns that bout from its seed with the recorded actions and checks its legal mask and state throughout. Tactical tables count combat actions the focal policy chose and what it was offered at decisions with multiple legal alternatives. Ground-target row and column picks use their own replay labels. They also report reaction take-up and casting, movement and end-turn choices when relevant attack alternatives were legal. Ground-target results count the selected footprint and whether any legal foe-only center was available before the row decision. Selection and availability counts depend on the game states each policy visits.

Run the evaluation from the repository root once the release library and frozen checkpoint are ready.

```sh
PYTHONPATH=python .venv/bin/python evaluation/evaluate.py --output evaluation/results
```

The four matchup rows compare learned decisions against the standard scripted heuristic and against uniformly random legal choices. The report includes matched comparisons against both random-versus-heuristic and heuristic-versus-heuristic controls. The self-control helps distinguish improvements in policy from effects of class matchups. Terminal draws and decision caps both contribute half a point. The report gives separate counts for knockouts, simultaneous knockouts, arena turn and decision caps, and the evaluator cap.

## Self-play arena ABI

The self-play holdout uses `S2F`, a separate seed domain from the original reports, the two training/DEV domains and the earlier holdout. The script requires the frozen SHA-256 and matching ABI, compares against the standard and expert scripts and can use the migrated previous policy as a frozen benchmark. With the sampled prior adapter, each bout uses its own deterministic random stream. Use a separate output directory to preserve the original results.

```sh
PYTHONPATH=python .venv/bin/python evaluation/evaluate.py \
  --seed-domain s2-final \
  --checkpoint models/your_frozen_checkpoint.npz --expected-sha256 YOUR_SHA256 \
  --prior /path/to/prior_with_matching_abi.npz --prior-sample \
  --output /tmp/duel-evaluation
.venv/bin/python evaluation/render_report.py /tmp/duel-evaluation
```

In the expanded arena, the target trace records row, column and altitude when the engine offers a height choice. Placement diagnostics count resolved footprints where the primary opponent's location was known. They measure footprint geometry and availability before rolls, with secondary creatures outside the tally.

The report binds it to the exact learned checkpoint, optional prior checkpoint and native library by SHA-256. Movement and weapon/slot-spell counters use the original action families for comparability. The report also measures Ready release and Counterspell acceptance when each reaction is optional.

The archived `evaluation/self-play-results` report and traces were produced on ABI 2001 at mechanics revision `70408a3`. ABI 2002 uses viewer-relative target IDs. For new comparisons use matching checkpoints and a fresh seed domain if the previous holdout informed model changes.

## Relative target IDs, ABI 2002

The [S3F report](relative-results/README.md) compares a frozen ABI 2002
checkpoint against masked random, standard and expert scripts, and the
previous checkpoint adapted to ABI 2002. It also reruns the exact ABI 2001
checkpoint against the same controls. S3F uses 2,880 seed pairs with both
seat assignments, and the random opponents sample physical legal actions
from corresponding per-seat streams across both ABIs. The phase-80 audit
reports self-target selections when the opposite duelist is legal.

From the repository root, build isolated evaluation libraries and use the
checkpoint hash from the report manifest:

```sh
.venv/bin/python evaluation/build_relative_control.py 690fe54 /tmp/duel-abi2001 2001
.venv/bin/python evaluation/build_relative_control.py 600436d /tmp/duel-abi2002 2002
PYTHONPATH=python .venv/bin/python evaluation/evaluate_relative.py \
  --candidate models/arena_policy.npz --expected-sha256 FROZEN_SHA256 \
  --abi2001 /tmp/duel-abi2001 --abi2002 /tmp/duel-abi2002 \
  --output /tmp/duel-s3f-reproduction
```

Use `--smoke` with a separate output directory for a 72-bout test per
matchup in the S3X seed domain.

## Level-five roster parity

The roster parity benchmark uses the same expert controller for both builds in each bout. It measures how much class-pair score gaps change when training fixtures move from the historical level mix to canonical level-5 builds and then to the full level-5 roster. Shared combat seeds, the four original arenas and mirrored seat assignments keep the comparison paired. Pass an explicit checkpoint to use the same greedy learned controller on both sides.

```sh
make release
PYTHONPATH=python .venv/bin/python evaluation/roster_parity.py \
  --output evaluation/roster-parity-results
```

The report includes class-pair and build-pair scores, draw rates, terminal reasons, and winner hit points for each knockout. Class-pair scores average over builds. The held-out seeds are separate from training and development. [Expert results](roster-parity-results/README.md) · [Wide-policy sensitivity](roster-wide-parity-results/README.md)
