"""Locked release build with normalized source paths and a portable Windows CRT."""
import argparse
import os
from pathlib import Path
import subprocess

from release import ROOT

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target")
parser.add_argument("--probe", action="store_true")
args = parser.parse_args()
env = os.environ.copy()
# Rust applies the last matching prefix. Keep the checkout rule most specific.
# Unit-separated flags work even when a checkout/home path contains spaces.
flags = [f"--remap-path-prefix={Path.home()}=/build-home",
         f"--remap-path-prefix={Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))}=/cargo",
         f"--remap-path-prefix={ROOT}=."]
if os.name == "nt":
    flags.extend(["-C", "target-feature=+crt-static"])
env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
command = ["cargo", "build", "--locked", "--release"]
command += ["--features", "test-support", "--example", "package_probe"] if args.probe else ["--bin", "codeconvoy"]
if args.target:
    command += ["--target", args.target]
subprocess.run(command, cwd=ROOT, env=env, check=True)
