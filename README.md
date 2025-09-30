# NoteHub

NoteHub is a Rust CLI that treats GitHub issues as a lightweight note system.

## Current Capabilities
- Parse command-line actions via `clap` (subcommands: `sync`, `issue`, `note`, `init`).
- `notehub init --token <PAT> --repo <owner/name>` stores your token and default repository in the per-user config directory (e.g. `~/Library/Application Support/NoteHub/config.toml` on macOS).
- Shared runtime context loads config once and reuses it across commands.

## Limitations (MVP)
- Only a **single repository** is tracked per config; multi-repo support and vault switching are planned.
- Commands currently output placeholders until GitHub fetch and local note storage are implemented.

## Roadmap
- Integrate `octocrab` and implement live issue listing/viewing.
- Persist local-only notes (likely via SQLite) and add semantic search.
- Introduce vaults for switching between repo sets and support external editor integration.

## Getting Started
```bash
cargo run -- init --token <your_personal_access_token> --repo owner/name
cargo run -- sync
```

Ensure your shell sources `~/.cargo/env` so `cargo` is on the PATH after installing Rust with `rustup`.
