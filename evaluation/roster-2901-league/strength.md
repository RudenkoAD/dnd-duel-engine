# Final strength on the 36-build level-five roster

The selected endpoint is update 1000 after 384,000 training bouts. Every arm uses the same 26,640 seed blocks from R36-L2901-FINAL-v1. The schedule enumerates 666 unordered build pairs, five explicit arenas and eight seeds per pair and arena. Each non-mirror block rotates both builds through the focal controller and plays both seats for each assignment. Mirror blocks play both seats. This gives each of the 1,296 ordered build assignments equal weight.

Score gives one point for a win, half a point for a draw and zero for a loss. The 95% intervals keep shared seed blocks intact and estimate seed variation within each fixed build-pair and arena stratum. The expert contrast subtracts the two arms within the same seed blocks. Intervals are pointwise.

| Focal vs opponent | Wins | Draws | Losses | Score | 95% CI |
| --- | ---: | ---: | ---: | ---: | ---: |
| Neutralized wide 2901 vs expert | 63,385 | 5,711 | 34,584 | 63.89% | [63.68, 64.10]% |
| Fine-tuned vs expert | 69,484 | 3,505 | 30,691 | 68.71% | [68.50, 68.91]% |
| Fine-tuned vs neutralized wide 2901 | 55,226 | 6,486 | 41,968 | 56.39% | [56.19, 56.59]% |
| Fine-tuned vs literal wide 2901 | 55,697 | 6,871 | 41,112 | 57.03% | [56.83, 57.23]% |
| Literal wide 2901 vs expert | 62,068 | 6,168 | 35,444 | 62.84% | [62.63, 63.05]% |

Against expert, fine-tuning changed score by **+4.82 percentage points** (paired 95% CI [4.57, 5.06] pp).
Relative to literal wide 2901, fine-tuning changed expert score by **+5.87 percentage points** (paired 95% CI [5.62, 6.12] pp).

| Arena | Start vs expert | Fine-tuned vs expert | Paired change, pp (95% CI) | Fine-tuned vs start | Fine-tuned vs literal 2901 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Мшистая цитадель | 65.01% | 69.97% | +4.96 [4.40, 5.51] | 57.07% | 57.47% |
| Янтарная застава | 64.54% | 67.75% | +3.22 [2.67, 3.77] | 55.86% | 56.84% |
| Серебряный бастион | 62.98% | 67.56% | +4.58 [4.05, 5.11] | 55.64% | 56.02% |
| Туманные врата | 63.91% | 69.32% | +5.41 [4.86, 5.96] | 56.82% | 57.61% |
| Тёмный бастион | 63.01% | 68.94% | +5.93 [5.38, 6.48] | 56.58% | 57.22% |

| Focal class | Start vs expert | Fine-tuned vs expert | Paired change, pp (95% CI) |
| --- | ---: | ---: | ---: |
| Barbarian | 77.09% | 81.12% | +4.03 [3.49, 4.57] |
| Bard | 48.32% | 53.41% | +5.09 [4.09, 6.09] |
| Cleric | 58.18% | 61.62% | +3.44 [2.38, 4.49] |
| Druid | 59.16% | 67.81% | +8.65 [7.68, 9.62] |
| Fighter | 72.20% | 77.19% | +4.99 [4.26, 5.71] |
| Monk | 80.35% | 86.86% | +6.50 [5.83, 7.18] |
| Paladin | 77.79% | 86.35% | +8.56 [7.68, 9.45] |
| Ranger | 65.06% | 68.48% | +3.41 [2.50, 4.33] |
| Rogue | 50.58% | 54.20% | +3.61 [2.87, 4.35] |
| Sorcerer | 68.94% | 73.26% | +4.32 [3.46, 5.17] |
| Warlock | 44.62% | 49.75% | +5.13 [4.18, 6.08] |
| Wizard | 64.36% | 64.44% | +0.08 [-0.60, 0.77] |

| Focal vs opponent | Double knockouts | Turn-cap draws | Decision-cap draws |
| --- | ---: | ---: | ---: |
| Neutralized wide 2901 vs expert | 10 | 862 | 4,839 |
| Fine-tuned vs expert | 33 | 974 | 2,498 |
| Fine-tuned vs neutralized wide 2901 | 55 | 1,150 | 5,281 |
| Fine-tuned vs literal wide 2901 | 52 | 1,433 | 5,386 |
| Literal wide 2901 vs expert | 11 | 1,191 | 4,966 |

The primary comparator applies the trainer's roster warm-start rule to update 2901, zeroing the 29 reserved observation inputs at the first layer. The literal comparison uses its original weights, including 6,144 nonzero weights on those inputs. Neutralization alone changed expert score by +1.05 pp (paired 95% CI [0.89, 1.21] pp).

The estimates apply to greedy masked policies, this native engine and expert, and uniform weighting over level-five builds and arenas. The intervals quantify combat RNG uncertainty conditional on these checkpoints. There is one training run and no estimate of variability across independent retraining runs. Each exact build pair and arena has only eight seeds.

Input paths, SHA-256 digests and the full intervals are in strength.json. Each arm stores outcomes and terminal reasons as pair × arena × repeat × orientation arrays in its matching NPZ file. Orientations order the focal build first and seat second. For identical builds, only orientations 0 and 1 are played.
