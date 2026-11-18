#!/usr/bin/env python3
"""Packs `mod/` into dist/vasttweaks-<version>.hmod with the official packer (`cargo horizon-mod build`, which needs
the pinned build's SDK)."""
import hashlib
import shutil
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MOD = ROOT / "mod"


def main():
    # The mod has no registry dependencies: a lock from a local `cargo check` is not part of the package.
    (MOD / "Cargo.lock").unlink(missing_ok=True)
    shutil.copy(ROOT / "LICENSE", MOD / "LICENSE")
    subprocess.run(["cargo", "horizon-mod", "build"], cwd=MOD, check=True)
    version = tomllib.loads((MOD / "horizon-mod.toml").read_text())["version"]
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    out = dist / f"vasttweaks-{version}.hmod"
    shutil.copy(MOD / "target/horizon-mod/vasttweaks.hmod", out)
    print(f"{out}  {hashlib.sha256(out.read_bytes()).hexdigest()}")


if __name__ == "__main__":
    main()
