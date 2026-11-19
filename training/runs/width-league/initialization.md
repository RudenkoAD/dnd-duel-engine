# Width warm start

The ABI 2002 champion at update 1300 has 164,353 parameters. Its width-256
warm start has 361,345 parameters, 2.20 times as many. Both have the same
observation vector, action set, legality mask, activation and output heads.

The transform duplicates layer 1's incoming weights and biases. It divides
each original unit's outgoing weights across the two copies, using a
seeded fraction of 0.25, 0.375, 0.625 or 0.75 for the first copy and the
residual for the second. Both copies of each layer 2 unit initially receive
the same preactivation. The policy and value heads share an independent
layer 2 split. In exact arithmetic each pair contributes the same weighted
sum as the original unit. In float32, changing the summation order can
affect logits at a near tie. The unequal splits break the gradient symmetry
of the copies.

| Checkpoint | SHA-256 |
|---|---|
| Champion update 1300 | `96f9bddabc0eeff4a70c111db443dea064490c6e576ff8bb569966024cb26e31` |
| Width-256 warm start | `8757c7dec788c354152f7546f36b3cf3051f6fd5eab62faf28308300c2f5a86b` |

The audit used 4,096 synthetic observations (including 32 all-zero vectors)
and 81,375 actor-visible observations from 1,152 complete seeded arena
bouts. It sampled 40,888 seat-0 decisions in 69 phases and 40,487 seat-1
decisions in 68 phases. Bouts followed the expert, greedy champion or
Gumbel-sampled champion for both players, covering same- and mixed-level
fixtures across the 12 classes. The simulator library SHA-256 and full
phase counts are recorded in `initialization.json`.

| Maximum absolute discrepancy | Synthetic | Arena |
|---|---:|---:|
| NumPy full logits | 1.70e-5 | 7.63e-6 |
| PyTorch full logits | 1.72e-5 | 1.72e-5 |
| PyTorch legal logits | 1.53e-5 | 1.63e-5 |
| PyTorch value | 9.54e-7 | 8.95e-7 |

The audit compared float32 outputs with a 5e-4 absolute tolerance. Every
legal greedy action agreed in both NumPy inference and Torch training. The
audit found zero exact ties for the top legal action and two Torch arena
near ties within twice the maximum discrepancy in their batches. A single
SGD update on a masked policy loss plus squared value loss changed the
paired incoming weights by up to 4.48e-5 for layer 1 and 1.54e-4 for layer 2.
This verifies the wider network can learn distinct incoming weights for
each copy immediately.

The measured one-shot widening and compressed serialization cost was
0.033384 process CPU seconds, with a median of 0.022539 seconds across nine
in-memory repetitions. Each repetition produced the recorded output hash.
The measurement starts after loading and hashing the narrow archive.

From the repository root, use the built ABI 2002 native library and a
Python environment with NumPy and Torch. The widening command requires a
fresh destination.

```sh
export PYTHONPATH=python:training
python training/widen_policy.py \
  training/runs/league/checkpoints/update-1300.npz /tmp/width-warm-reproduction.npz
shasum -a 256 /tmp/width-warm-reproduction.npz
python training/runs/width-league/initialization.py
python -m unittest discover -s training/tests -p test_widen_policy.py -v
```
