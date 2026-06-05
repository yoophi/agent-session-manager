# agent-sessions

Rust CLI and reusable library for listing local coding-agent sessions.

Supported agents:

- Claude Code
- OpenAI Codex CLI
- Pi Coding Agent

## Usage

```sh
cargo run -- list --agent claude
cargo run -- list --agent codex --all
cargo run -- list --agent pi --path /path/to/project
```

`--all` is the default when `--path` is not provided.

## Architecture

The project uses hexagonal architecture:

- `domain`: shared agent/session model.
- `application`: use cases and ports.
- `outbound`: filesystem session repositories.
- `inbound`: CLI adapter.

The library crate is exposed as `agent_sessions`; the binary is `agent-sessions`.

## Development

```sh
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```
