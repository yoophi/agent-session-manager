# agent-sessions

Rust CLI and reusable library for listing local coding-agent sessions.

Supported agents:

- Claude Code
- OpenAI Codex CLI
- Pi Coding Agent

## Usage

```sh
cargo run -- list
cargo run -- list --all
cargo run -- list --agent claude
cargo run -- list --path /path/to/project
cargo run -- list --agent pi --path /path/to/project
cargo run -- list --agent claude --output json
cargo run -- list --agent codex --output csv
```

`--all` means all agents and is the default when `--agent` is omitted.
`--all` cannot be used with `--agent`.
`--output` supports `text`, `csv`, and `json`; `text` is the default.

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
