# Development

This file collects developer-oriented workflow notes that do not belong on the GitHub landing page.

## Workspace

- `crates/nooboard-config`
  - config schema, bootstrap resolution, template generation, and the `nooboard-config` CLI
- `crates/nooboard-app`
  - the app service, runtime state, events, settings patch flow, clipboard logic, and transfer orchestration
- `crates/nooboard-desktop`
  - the GPUI desktop frontend
- `crates/nooboard-sync`
  - peer sync and transport primitives
- `crates/nooboard-storage`
  - local persistence
- `crates/nooboard-platform`
  - platform integration abstractions

## Desktop bootstrap

Desktop bootstrap resolution currently follows this order:

1. `--choose-config`
2. `--config /path/to/nooboard.toml`
3. `--dev`
4. `NOOBOARD_CONFIG=/path/to/nooboard.toml`
5. default config path
6. bootstrap chooser if the default config file does not exist

Useful launch modes:

```bash
cargo run -p nooboard-desktop
cargo run -p nooboard-desktop -- --choose-config
cargo run -p nooboard-desktop -- --config /absolute/path/to/nooboard.toml
cargo run -p nooboard-desktop -- --dev
```

## Config generation

The repository includes a small config CLI in the `nooboard-config` package.

Create a production config:

```bash
cargo run -p nooboard-config --bin nooboard-config -- init --profile production
```

Create a development config in a custom location:

```bash
cargo run -p nooboard-config --bin nooboard-config -- init --profile development --output .dev-data
```

`--output` accepts either:

- a directory, which will receive `nooboard.toml`
- a file path, which will be written directly

If `--output` is omitted, the CLI targets `./nooboard.toml` and asks for confirmation before writing or overwriting.

## Local development setup

Repository-local development setup uses:

- config: `<repo>/.dev-data/nooboard.toml`
- device id: `nooboard-dev`
- token: `token-for-sync`

Launch the desktop app against the local development setup:

```bash
cargo run -p nooboard-desktop -- --dev
```

If the repository-local development config does not exist yet, it is created automatically. If it exists but is invalid, startup fails explicitly instead of silently rewriting it.

## Checks

Run the most common checks with:

```bash
cargo check -p nooboard-config -p nooboard-app -p nooboard-desktop
cargo test -p nooboard-config -p nooboard-app -p nooboard-desktop
```
