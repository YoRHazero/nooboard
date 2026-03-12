# nooboard

In the AI era, copy and paste are used more frequently than ever. Sometimes a loooooooong prompt that took half an hour to refine can disappear because I selected the wrong text and hit `Ctrl+C` at the wrong moment. And when I want to copy text on one device and paste it on another device, the flow is still more troublesome than it should be. That is why this project exists.

`nooboard` is an experimental local-first clipboard and transfer board built in Rust. It is designed around a single app service and a desktop client that works with:

- clipboard history and rebroadcast
- peer discovery and manual peer setup in LAN
- file transfer workflows (Implemented but I haven't fully tested it yet)
- local-first settings and config bootstrap

Note: this project is developed with heavy AI assistance and supervised by a noob Rustacean. The GPUI part in particular is currently verified more by "does it work?" than by full source-level understanding.

This project supports both macOS and Windows. Linux support is not planned for now, because it's hard for me to imagine a Linux user who needs this kind of app, but I might add it in the future if there is demand.

The project is still under active development, but the current desktop app already exposes the main clipboard, peers, transfers, and settings flows through a unified backend contract. (I hope)

## Quick start

Requirements:

- Rust stable
- `cargo`

Run the desktop app:

```bash
cargo run -p nooboard-desktop
```

If the default config file does not exist yet, the app opens a bootstrap chooser and helps you create or select one.

## Configuration

Default config path:

- macOS: `~/.nooboard/nooboard.toml`
- Windows: `%USERPROFILE%\.nooboard\nooboard.toml`

You can also launch with an explicit config file:

```bash
cargo run -p nooboard-desktop -- --config /absolute/path/to/nooboard.toml
```

Or force the chooser:

```bash
cargo run -p nooboard-desktop -- --choose-config
```

## Repository structure

- `crates/nooboard-config`
  - config schema, bootstrap resolution, template generation, and config CLI
- `crates/nooboard-app`
  - the app service and runtime state/event contract
- `crates/nooboard-desktop`
  - the GPUI desktop frontend
- `crates/nooboard-sync`
  - peer sync and transport primitives
- `crates/nooboard-storage`
  - local persistence

## Development

Development-specific commands, bootstrap modes, and config generation details live in [`DEVELOPMENT.md`](./DEVELOPMENT.md).

## License

This project is licensed under the MIT License. See [`LICENSE`](./LICENSE).
