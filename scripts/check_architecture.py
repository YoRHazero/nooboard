"""Check the four-crate dependency direction and clipboard's arboard constraint."""
import json
import subprocess

metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"], text=True
))
packages = {package["name"]: package for package in metadata["packages"]}
expected = {"nooboard-core", "nooboard-clipboard", "nooboard-network", "nooboard-storage"}
assert set(packages) == expected, "unexpected workspace crates"
for name, package in packages.items():
    local = {dep["name"] for dep in package["dependencies"] if dep.get("path")}
    assert local == (expected - {name} if name == "nooboard-core" else set()), (name, local)
    assert not any(dep["name"].startswith("tauri") for dep in package["dependencies"]), name

tree = subprocess.check_output(
    ["cargo", "tree", "-p", "nooboard-clipboard", "--edges", "normal,build", "--prefix", "none", "--locked"],
    text=True,
)
assert not any(line.startswith("arboard ") for line in tree.splitlines()), "clipboard depends on arboard"
print("Architecture constraints passed.")
