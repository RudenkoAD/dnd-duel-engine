"""Build an evaluation-only Rust library from an exact git revision."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def build(revision, output, abi):
    revision = subprocess.check_output(
        ["git", "rev-parse", "--verify", f"{revision}^{{commit}}"], cwd=ROOT, text=True
    ).strip()
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output) as source:
        source = Path(source)
        archive = subprocess.check_output(
            ["git", "archive", revision, "src", "Cargo.toml", "Cargo.lock", "Makefile",
             "models/arena_policy.npz"], cwd=ROOT
        )
        subprocess.run(["tar", "-xf", "-", "-C", source], input=archive, check=True)
        shim = Path(__file__).with_name("relative_random.rs").read_text()
        shim = shim.replace("RAW_MASK", "battle.mask()" if abi == 2001 else "battle.raw_action_mask()")
        shim = shim.replace("PUBLIC_ID", "raw_id" if abi == 2001 else "battle.target_permutation().encode(raw_id)")
        bridge = source / "src/arena2/bridge.rs"
        with bridge.open("a") as handle:
            handle.write("\n" + shim)
        toolchain = ROOT / ".toolchain"
        env = os.environ | {"CARGO_TARGET_DIR": str(output / "target")}
        if (toolchain / "cargo/bin/cargo").exists():
            env |= {"CARGO_HOME": str(toolchain / "cargo"), "RUSTUP_HOME": str(toolchain / "rustup")}
            cargo = toolchain / "cargo/bin/cargo"
        else:
            cargo = "cargo"
        subprocess.run([cargo, "build", "--locked", "--offline", "--release", "--lib"],
                       cwd=source, env=env, check=True)
        (output / "arena_policy.npz").write_bytes((source / "models/arena_policy.npz").read_bytes())
        (output / "revision").write_text(revision + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("revision")
    parser.add_argument("output", type=Path)
    parser.add_argument("abi", type=int, choices=(2001, 2002))
    args = parser.parse_args()
    build(args.revision, args.output, args.abi)
