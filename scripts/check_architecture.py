"""Check desktop -> core -> ports, including clipboard's arboard constraint."""
import json
from pathlib import Path
import re
import subprocess

metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"], text=True
))
packages = {package["name"]: package for package in metadata["packages"]}
ports = {"nooboard-clipboard", "nooboard-network", "nooboard-storage"}
expected = ports | {"nooboard-core", "nooboard-desktop"}
assert set(packages) == expected, "unexpected workspace crates"
for name, package in packages.items():
    local = {dep["name"] for dep in package["dependencies"] if dep.get("path")}
    allowed = ports if name == "nooboard-core" else {"nooboard-core"} if name == "nooboard-desktop" else set()
    assert local == allowed, (name, local)
    if name != "nooboard-desktop":
        assert not any(dep["name"].startswith("tauri") for dep in package["dependencies"]), name

tree = subprocess.check_output(
    ["cargo", "tree", "-p", "nooboard-clipboard", "--target", "all", "--edges", "normal,build", "--prefix", "none", "--locked"],
    text=True,
)
assert not any(line.startswith("arboard ") for line in tree.splitlines()), "clipboard depends on arboard"

# Native drivers own OS operations, not application requests or subscriptions.
clipboard = Path(__file__).resolve().parents[1] / "crates" / "clipboard" / "src"
for source in (clipboard / "backend").rglob("*.rs"):
    code = source.read_text(encoding="utf-8")
    assert "crate::runtime" not in code, f"backend imports runtime: {source}"
    assert not re.search(r"\b(?:mpsc|oneshot|watch)::", code), f"backend owns application channels: {source}"
for source in (clipboard / "formats").rglob("*.rs"):
    code = source.read_text(encoding="utf-8")
    assert "crate::runtime" not in code, f"format code imports runtime: {source}"
    assert "crate::backend" not in code, f"shared format code imports platform backend: {source}"
    assert not re.search(r"\b(?:mpsc|oneshot|watch)::", code), f"format code owns application channels: {source}"
print("Architecture constraints passed.")
