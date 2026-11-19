"""Replay one seeded fight or time a small NumPy policy through Gymnasium."""
import argparse
import time

import numpy as np

from duel_env import ACTIONS, DuelEnv, STATE_FIELDS


def make_policy(seed):
    rng = np.random.default_rng(seed)
    inputs = len(STATE_FIELDS)
    w1 = rng.normal(0, 1 / np.sqrt(inputs), (inputs, 32)).astype(np.float32)
    w2 = rng.normal(0, 1 / np.sqrt(32), (32, len(ACTIONS))).astype(np.float32)
    scale = np.array([1, 1, 2, 1, 32, 60, 30, 1, 1, 30] + [12, 60, 2, 20, 1, 1, 1] * 2,
                     dtype=np.float32)

    def choose(obs):
        x = obs["state"].astype(np.float32) / scale
        logits = np.tanh(x @ w1) @ w2
        legal = np.flatnonzero(obs["action_mask"])
        scores = logits[legal]
        probs = np.exp(scores - scores.max())
        return int(rng.choice(legal, p=probs / probs.sum()))

    return choose


def run(count, seed, policy):
    choose = make_policy(seed) if policy == "mlp" else None
    steps = opponent = capped = 0
    wins = [0, 0]
    start = time.perf_counter()
    with DuelEnv() as env:
        for k in range(count):
            obs, _ = env.reset(seed=(seed + k) % 2**64)
            while True:
                action = choose(obs) if choose else env.duel.bot_action()
                obs, reward, terminated, truncated, info = env.step(action)
                steps += 1
                opponent += info["opponent_decisions"]
                if terminated or truncated:
                    if terminated:
                        wins[info["winner"]] += 1
                    else:
                        capped += 1
                    if count == 1:
                        print(f"seat={env.seat} winner={info['winner']} reward={reward} "
                              f"terminated={terminated} truncated={truncated}")
                    break
    seconds = time.perf_counter() - start
    print(f"policy={policy} episodes={count} seed={seed} shield={wins[0]} archer={wins[1]} "
          f"capped={capped} agent_steps={steps} opponent_decisions={opponent} "
          f"seconds={seconds:.4f} agent_steps/s={steps/seconds:.0f}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--episodes", type=int, default=1)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--policy", choices=["bot", "mlp"], default="bot")
    args = parser.parse_args()
    if args.episodes < 1 or not 0 <= args.seed < 2**64:
        parser.error("episodes must be positive and seed must fit u64")
    run(args.episodes, args.seed, args.policy)
