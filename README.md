# NoteHub

NoteHub is a Rust CLI that treats GitHub issues as a lightweight note system.

## Current Capabilities
- Parse command-line actions via `clap` (subcommands: `sync`, `issue`, `note`, `init`).
- `notehub init --token <PAT> --repo <owner/name>` stores your token and default repository in the per-user config directory (e.g. `~/Library/Application Support/com.LexicalMathical.NoteHub/config.toml` on macOS).
- `notehub sync` pulls open **and closed** issues from GitHub and persists them in an on-disk SQLite cache (`~/Library/Application Support/com.LexicalMathical.NoteHub/notehub.db`).
- `notehub issue list` / `notehub issue view <num>` read from the local cache; viewing an uncached issue will fetch and store it on demand.

## Limitations (MVP)
- Only a **single repository** is tracked per config; multi-repo support and vault switching are planned.
- Local note storage and semantic search are not implemented yet; commands still print placeholders for note operations.

## Roadmap
- Persist local-only notes (likely via SQLite) and add semantic search.
- Extend configuration to multiple repo vaults and editor integrations.
- Add offline caching and background sync jobs.

## Getting Started
```bash
cargo run -- init --token <your_personal_access_token> --repo owner/name
cargo run -- sync
cargo run -- issue list
cargo run -- issue view <number>
```

Ensure your shell sources `~/.cargo/env` so `cargo` is on the PATH after installing Rust with `rustup`.
