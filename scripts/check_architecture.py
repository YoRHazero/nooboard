"""Check crate boundaries and the clipboard/storage backend constraints."""
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

# Storage's public models and message runtime are independent of database drivers.
storage = clipboard.parent.parent / "storage" / "src"
for source in storage.rglob("*.rs"):
    relative = source.relative_to(storage)
    code = source.read_text(encoding="utf-8")
    if relative.parts[:2] != ("backend", "sqlite"):
        assert not re.search(r"\brusqlite\b|\bsha2\b", code), f"SQLite detail outside adapter: {source}"
    if relative.parts[0] in {"model", "runtime"}:
        assert not re.search(r"\b(?:Sqlite|rusqlite)\w*\b|backend::sqlite", code), f"backend detail in shared layer: {source}"
    if relative.parts[0] == "model":
        assert "crate::backend" not in code and "crate::runtime" not in code, f"model imports implementation: {source}"
        assert not re.search(r"\b(?:mpsc|oneshot|watch)::", code), f"model owns channels: {source}"
    if relative.parts[0] == "backend":
        assert "crate::runtime" not in code, f"storage backend imports runtime: {source}"
        assert not re.search(r"\b(?:mpsc|oneshot|watch)::", code), f"storage backend owns application channels: {source}"
assert not (storage / "files.rs").exists(), "storage owns transfer files"
assert not (storage / "secrets.rs").exists(), "storage owns credentials"
storage_deps = packages["nooboard-storage"]["dependencies"]
assert all(dep["optional"] for dep in storage_deps if dep["name"] in {"rusqlite", "sha2"}), "SQLite dependencies must be optional"
assert not any(dep["name"] == "keyring" for dep in storage_deps), "storage depends on keyring"
driverless_tree = subprocess.check_output(
    ["cargo", "tree", "-p", "nooboard-storage", "--no-default-features", "--edges", "normal,build", "--prefix", "none", "--locked"],
    text=True,
)
assert not any(line.startswith(("rusqlite ", "libsqlite3-sys ", "sha2 ", "keyring ")) for line in driverless_tree.splitlines()), "driverless storage includes a database driver"
print("Architecture constraints passed.")
