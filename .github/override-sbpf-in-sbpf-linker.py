#!/usr/bin/env python3
"""Override the sbpf crates in sbpf-linker with the given sbpf checkout.

Usage: override-sbpf-in-sbpf-linker.py <sbpf checkout> <path to write the config to>
"""

import json
import subprocess
import sys
from pathlib import Path


def metadata(*args):
    out = subprocess.run(["cargo", "metadata", "--format-version", "1", *args],
                         check=True, capture_output=True, text=True).stdout
    return json.loads(out)


if len(sys.argv) != 3:
    sys.exit(f"usage: {sys.argv[0]} <sbpf checkout> <path to write the config to>")
sbpf, override = map(Path, sys.argv[1:])

needed = {d["name"]
          for p in metadata("--no-deps")["packages"] if p["name"] == "sbpf-linker"
          for d in p["dependencies"] if d["name"].startswith("sbpf")}
if not needed:
    sys.exit("the linker declares no sbpf dependency to override")

supplied = {p["name"]: Path(p["manifest_path"]).parent
            for p in metadata("--manifest-path", sbpf / "Cargo.toml", "--no-deps")["packages"]
            if p.get("publish") is None}

absent = needed - supplied.keys()
if absent:
    sys.exit(f"{' '.join(sorted(absent))} is a dependency this repository does not supply")

override.write_text("".join(
    ["[patch.crates-io]\n"] + [f'{n} = {{ path = "{supplied[n]}" }}\n' for n in sorted(needed)]))
print(override.read_text(), end="")

subprocess.run(["cargo", "--config", str(override), "update",
                *[f"-p={n}" for n in sorted(needed)]],
               check=False, capture_output=True, text=True)

for p in metadata("--config", str(override))["packages"]:
    if p["name"] in needed and (p["source"] or Path(p["manifest_path"]).parent != supplied[p["name"]]):
        sys.exit(f"{p['name']} did not resolve to this checkout")
