# TUI application plan

## Goal

Add a ratatui-based terminal UI for browsing agent sessions while preserving the
current script-friendly output path.

Target behavior:

```sh
agent-sessions --print
agent-sessions list --print
agent-sessions --print --output json
agent-sessions
agent-sessions list
```

- `--print` keeps the current behavior: render `text`, `csv`, or `json` to
  stdout and exit.
- Without `--print`, the `list` command opens a TUI session list.
- Pressing Enter on a selected session opens a detail view.
- Omitting the command still defaults to `list`.
- Omitting `--path` still defaults to the current working directory.
- The `rm` command remains non-interactive for the first TUI implementation.

## Dependencies

Add these runtime dependencies:

```toml
ratatui = "0.30.1"
crossterm = "0.29.0"
```

Use ratatui for widgets, layout, styling, and terminal rendering. Use crossterm
for terminal setup, raw mode, alternate screen handling, and event polling.

## CLI contract

### List command

Extend list options with a boolean flag:

```text
--print
```

The flag should be available both at the root list-default level and on the
explicit `list` subcommand:

```sh
agent-sessions --print
agent-sessions list --print
agent-sessions --agent codex --print
agent-sessions list --agent codex --print
```

`--output` remains valid and keeps its existing default of `text`, but it only
controls stdout formatting in `--print` mode. In TUI mode, output format is not
used.

### Default mode

Change the list default from print mode to TUI mode:

| Command | Behavior |
| --- | --- |
| `agent-sessions` | Open TUI for current directory and all agents |
| `agent-sessions list` | Open TUI for current directory and all agents |
| `agent-sessions --print` | Print text list and exit |
| `agent-sessions --print --output json` | Print JSON and exit |
| `agent-sessions rm ...` | Keep current remove behavior |

This is an intentional behavior change for interactive terminal usage. Scripts
should use `--print` after this change.

## Architecture

Keep the hexagonal architecture boundary:

- `domain`: no ratatui or terminal concepts.
- `application`: keep list/remove use cases unchanged unless detail-specific
  query helpers become necessary.
- `outbound`: keep filesystem repositories unchanged.
- `inbound::cli`: parse flags, load sessions, and route to either print or TUI.
- `inbound::tui`: own terminal setup, app state, rendering, and event handling.

Proposed module layout:

```text
src/inbound/cli.rs
src/inbound/tui.rs
```

If `tui.rs` grows too large, split it later:

```text
src/inbound/tui/app.rs
src/inbound/tui/render.rs
src/inbound/tui/events.rs
```

Do not introduce that split until the single module becomes hard to review.

## Data flow

1. Parse CLI args.
2. Resolve list scope:
   - `--path <path>` when provided.
   - current working directory when omitted.
3. Load sessions using `ListSessionsService`.
4. If `--print` is set:
   - call the existing print formatter.
   - exit.
5. If `--print` is not set:
   - create TUI app state from the loaded `Vec<AgentSession>`.
   - enter alternate screen and raw mode.
   - run the event loop.
   - restore terminal state before returning.

Terminal restoration must happen even if rendering or event handling returns an
error. Prefer a small guard type that disables raw mode and leaves the alternate
screen in `Drop`.

## TUI views

### Session list view

Initial screen:

- Header: current scope path and active agent filter.
- Main list/table: sessions sorted by `updated_at` descending.
- Footer: key hints.

Recommended columns:

| Column | Source |
| --- | --- |
| Agent | `AgentSession.agent` |
| Updated | `AgentSession.updated_at` |
| Messages | `AgentSession.message_count` |
| Title | `AgentSession.title` fallback `-` |
| CWD | `AgentSession.cwd` fallback `-` |

Behavior:

- Up/Down or `k`/`j`: move selection.
- PageUp/PageDown: move by page.
- Home/End: jump to first or last row.
- Enter: open selected session detail.
- `q` or Esc: quit.

Empty state:

- Render a clear empty message with the resolved path and selected agent scope.
- `q` and Esc still quit.

### Session detail view

Open when Enter is pressed on a selected session.

Content:

- Session id.
- Agent.
- Title.
- Working directory.
- Transcript file path.
- Message count.
- Created and updated timestamps.
- Model, branch, source.
- Subsession metadata.

Behavior:

- Up/Down or `k`/`j`: scroll detail content.
- PageUp/PageDown: scroll by page.
- Backspace, Left, Esc, or `b`: return to list.
- `q`: quit application.

First implementation should show normalized metadata only. Transcript message
preview can be added later after deciding a safe parser/view model for each
agent transcript format.

## State model

Use a small explicit app state:

```rust
enum View {
    List,
    Detail { selected_index: usize, scroll: u16 },
}

struct TuiApp {
    sessions: Vec<AgentSession>,
    selected_index: usize,
    view: View,
    scope_path: PathBuf,
    agent_filter: Option<AgentKind>,
    should_quit: bool,
}
```

Selection should be clamped whenever the session list is empty or the selected
index would exceed the last row.

## Rendering guidelines

- Keep styling restrained and readable in light and dark terminal themes.
- Use borders for the main list and detail panel.
- Use reversed style or a distinct modifier for the selected row.
- Avoid color-only state indicators.
- Do not wrap long file paths in the table; truncate there and show the full
  value in the detail view.
- Keep all text ASCII unless existing data contains Unicode.

## Error handling

- Print-mode errors continue to return through `anyhow::Result`.
- TUI setup errors should fail before entering the event loop.
- Runtime errors should restore the terminal and then return the error.
- If session loading fails, do not enter the TUI.

## Test plan

Unit tests:

- List option parsing accepts `--print` at root and explicit `list` levels.
- Root command without `--print` selects TUI mode.
- Root command with `--print` selects print mode.
- Explicit `list --print --output csv` preserves CSV output.
- Selection movement clamps at list bounds.
- Detail view opens only when a selected session exists.

Integration tests:

- Existing print tests should add `--print`.
- Add a test for `agent-sessions --print`.
- Add a test for `agent-sessions list --print`.
- Keep `rm` tests unchanged.

Manual verification:

```sh
cargo fmt
cargo clippy --all-targets --all-features
cargo test
cargo run --
cargo run -- list
cargo run -- --print
cargo run -- --print --output json
```

For manual TUI checks:

- Open with sessions present.
- Open with no sessions for the selected path.
- Navigate list.
- Open detail with Enter.
- Return to list.
- Quit from both list and detail.
- Confirm the terminal is restored after normal quit and after Ctrl+C.

## Implementation phases

### Phase 1: CLI routing and dependency setup

- Add `ratatui` and `crossterm` dependencies.
- Add `--print` to shared list args.
- Route list requests to print mode only when `--print` is set.
- Keep current formatter code intact.
- Update CLI integration tests to use `--print`.

### Phase 2: Minimal TUI list

- Add `inbound::tui` module.
- Implement terminal guard.
- Implement list app state.
- Render session table.
- Implement basic navigation and quit.
- Verify empty state.

### Phase 3: Detail view

- Add Enter handling from list.
- Add detail rendering from `AgentSession`.
- Add scroll and back navigation.
- Add state unit tests where practical.

### Phase 4: Polish and documentation

- Update README usage examples.
- Document `--print` for scripting.
- Add a short TUI keybinding section.
- Run full verification.

## Non-goals for first implementation

- Editing or deleting sessions from the TUI.
- Searching/filtering inside the TUI.
- Transcript message preview.
- Mouse support.
- Configurable themes.
- Async filesystem scanning.

These can be added after the list/detail interaction is stable.
