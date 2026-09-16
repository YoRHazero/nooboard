"""Check desktop -> core -> ports, including clipboard's arboard constraint."""
import json
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
print("Architecture constraints passed.")
