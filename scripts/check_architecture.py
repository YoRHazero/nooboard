"""Check crate boundaries and the clipboard/storage/network/core implementation constraints."""
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
# Network exposes one facade; feature runtimes and transport drivers remain private.
network = clipboard.parent.parent / "network" / "src"
root_code = (network / "lib.rs").read_text(encoding="utf-8")
assert not re.search(r"^pub(?:\([^)]*\))? mod\b", root_code, re.M), "network exports an implementation module"
assert re.findall(r"^pub use (.*);$", root_code, re.M) == ["api::*"], "network bypasses its API facade"
api_code = (network / "api.rs").read_text(encoding="utf-8")
for public_type in ("NetworkService", "Network", "NetworkEvents"):
    assert re.search(rf"^pub struct {public_type}\s*\{{", api_code, re.M), f"{public_type} must be defined at the public boundary"
assert not re.search(r"\b(?:rustls|keyring|mdns_sd|if_addrs)::|(?:std|tokio)::fs", api_code), "network API performs driver work"
assert not re.search(r"\b(?:Connection|TlsConfig|Message|Endpoint|Control)\b", api_code), "network API exposes internal handles or packets"
for source in network.rglob("*.rs"):
    relative = source.relative_to(network)
    code = source.read_text(encoding="utf-8")
    if "keyring::" in code:
        assert relative.parts[:2] == ("identity", "credentials"), f"credentials outside identity adapter: {source}"
    if source.name == "model.rs":
        assert not re.search(r"\b(?:mpsc|oneshot|watch)::|\bruntime::|\bprotocol::", code), f"network model owns runtime/protocol implementation: {source}"
        assert not re.search(r"(?:std|tokio)::fs|\b(?:rustls|keyring|mdns_sd|if_addrs)::", code), f"network model performs driver work: {source}"
    if relative.parts[:2] == ("transfer", "files"):
        assert not re.search(r"\b(?:mpsc|oneshot|watch)::|crate::(?:runtime|connections)", code), f"file operations own network scheduling: {source}"
network_deps = packages["nooboard-network"]["dependencies"]
assert all(dep["optional"] for dep in network_deps if dep["name"] == "keyring"), "native credentials must be optional"
# Core owns application workflows, never a second transport or database implementation.
core = clipboard.parent.parent / "core" / "src"
core_root = (core / "lib.rs").read_text(encoding="utf-8")
assert not re.search(r"^pub(?:\([^)]*\))? mod\b", core_root, re.M), "core exports an implementation module"
assert re.findall(r"^pub use (.*);$", core_root, re.M) == ["api::*"], "core bypasses API facade"
for obsolete in ("app.rs", "bootstrap.rs", "link", "content_transfer", "transfers.rs"):
    assert not any((core / obsolete).rglob("*.rs")) if (core / obsolete).is_dir() else not (core / obsolete).exists(), f"obsolete core implementation: {obsolete}"
for source in core.rglob("*.rs"):
    relative = source.relative_to(core)
    code = source.read_text(encoding="utf-8")
    assert not re.search(r"nooboard_network::(?:pairing|discovery|transport|protocol|identity|local)::", code), f"core imports network implementation: {source}"
    assert not re.search(r"\b(?:rusqlite|rustls|keyring|mdns_sd|if_addrs)::", code), f"driver work in core: {source}"
    if relative.parts[0] != "tests" and source.name != "diagnostics.rs":
        assert not re.search(r"\b(?:TcpListener|TcpStream|TlsConfig|SecretStore|Database)\s*::", code), f"transport/database ownership in core: {source}"
    if source.name == "model.rs":
        assert not re.search(r"\b(?:mpsc|oneshot|watch)::|crate::runtime", code), f"model owns runtime channels: {source}"
    if "self.clipboard.write_content(" in code:
        assert relative.as_posix() == "sync/runtime/apply.rs", f"clipboard write outside application queue: {source}"
print("Architecture constraints passed.")
