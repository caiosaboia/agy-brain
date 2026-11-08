# Spec: Claude Code session provider

Status: Draft -> Implemented
Owner: agy-brain core team
Related: `src/core.rs` (Antigravity provider), `src/cli.rs`, `src/main.rs`, `src/tui.rs`

## 1. Problem

`agy-brain` currently understands exactly one session backend: **Antigravity CLI**.
It reads a SQLite database (`conversation_summaries.db`) plus a `brain/<uuid>/`
artifact tree, and exposes navigation (`go`), listing (`list`), inspection
(`info`, `artifacts`), pinning (`pin`/`unpin`), lifecycle management
(`rename`, `delete`, `prune`) and health checks (`doctor`) on top of that model.

Developers who use **Claude Code** (Anthropic's official CLI) instead of (or
in addition to) Antigravity CLI get no value from `agy-brain`, even though
Claude Code keeps an on-disk session history that is conceptually equivalent
(a list of past conversations, each tied to a project working directory, that
a developer wants to browse, resume, pin and clean up).

## 2. Goal

Add a second, first-class session backend ("provider") for Claude Code, so
that every existing `agy-brain` command behaves consistently regardless of
which tool produced the sessions. The user-facing command surface
(`agy-brain list/info/artifacts/go/open/edit/nvim/pin/unpin/rename/delete/
prune/resolve/doctor/tui`) stays identical; only the data source changes.

Non-goals:
- We do not change Antigravity's behavior, file formats, or default provider
  selection when nothing suggests Claude Code is in use (full backward
  compatibility with the existing test suite is required).
- We never write into Claude Code's own data files
  (`~/.claude/projects/**/*.jsonl`, `~/.claude/todos/**`). Any customization
  agy-brain needs (pins, renamed titles) is stored in sidecar files that
  agy-brain owns exclusively, the same way `workspace_pins.json` already is
  an agy-brain-owned file living next to Antigravity's own database.
- We do not attempt to reverse Claude Code's project-directory name encoding
  to recover a workspace path; we always read the authoritative `cwd` field
  recorded inside the transcript instead (see 3.1).

## 3. Claude Code on-disk model (ground truth)

Verified directly against a real `~/.claude` directory on this machine
(`C--...`/`D--...` encoded project folders under `~/.claude/projects/`), and
cross-checked against the publicly documented Claude Code CLI transcript
format used by the wider community (`ccusage`, `claude-code-log`, etc.):

```
~/.claude/
  projects/
    <encoded-cwd>/
      <session-uuid>.jsonl      # one file per resumable session
  todos/
    <session-uuid>-agent-<n>.json   # optional per-session todo snapshots
```

- `<encoded-cwd>` is the session's working directory with path separators and
  `:` replaced by `-` (e.g. `D:\projetos\caio` -> `D--projetos-caio`,
  `/home/dev/app` -> `-home-dev-app`). This encoding is lossy/ambiguous, so
  agy-brain uses it only as a fast lookup path, never as a source of truth.
- Each `.jsonl` file is a newline-delimited stream of JSON objects
  ("entries"). Entries agy-brain understands:
  - `{"type":"summary","summary":"<title>","leafUuid":"<uuid>"}` - an
    optional human-readable title Claude Code itself writes for its resume
    picker.
  - `{"type":"user","message":{"role":"user","content":<string|blocks>},
    "cwd":"...","sessionId":"...","uuid":"...","timestamp":"...",
    "isSidechain":bool}` - a user turn. `content` is either a plain string or
    an array of content blocks (`{"type":"text","text":"..."}` among others).
  - `{"type":"assistant","message":{"role":"assistant","content":[...]},
    "cwd":"...","timestamp":"...","isSidechain":bool}` - an assistant turn.
  - Any other `type` (tool results, etc.) is ignored; unknown fields/types
    must never cause a parse failure for the whole file (resilience to
    format drift).
  - `isSidechain: true` marks a Task-tool sub-conversation branch that lives
    **inside the same file**. Unlike Antigravity, Claude Code has no separate
    listable "subagent session" - sidechains are not independently resumable
    or deletable. agy-brain therefore reports them only as an informational
    `subagent_count` on the parent session; `-s/--subagents` is accepted for
    CLI parity but is a no-op for this provider (there is nothing extra to
    reveal).

## 4. Data mapping (Antigravity concept -> Claude Code concept)

All of this reuses the existing `SessionSummary` / `BrainStats` /
`ArtifactInfo` / `PrunedSession` / `DeleteResult` structs from `src/core.rs`
unchanged, so `src/ui.rs` and `src/tui.rs` rendering code needs no format-
specific branching.

| Field                 | Antigravity source                          | Claude Code source |
|------------------------|----------------------------------------------|---------------------|
| `id`                  | `conversation_id` column                     | jsonl file stem (session uuid) |
| `title` / `title_source` | `title` -> `preview` -> `raw_summary` -> transcript cascade | title-override sidecar -> `summary` entry -> first root `user` message text -> `"(sem titulo)"` |
| `steps`               | `step_count` column                          | count of root-level (`isSidechain != true`) `user`/`assistant` entries |
| `mtime`               | `last_modified_time` column                  | `timestamp` of last root-level entry, else file mtime |
| `uris` / `primary_workspace` | `workspace_uris` JSON column            | `cwd` field recorded on entries (normalized with the existing `normalize_workspace_path`) |
| `status`              | `status` column                              | synthetic: last root entry `type=="user"` -> `"WAITING"`, `type=="assistant"` -> `"IDLE"`, empty file -> `"IDLE"` |
| `pinned`              | `.brain` / `workspace_pins.json`             | `.claude-brain` / `claude_workspace_pins.json` (separate files, no collision with Antigravity's) |
| `is_subagent`         | `nesting_depth`/`parent_conversation_id`     | always `false` (see 3.1) |
| `agent_name`, `parent_id`, `parent_title`, `nesting_depth` | DB columns | always empty/0 (not applicable) |
| `subagent_count`      | count of child conversations                 | count of distinct sidechain branches inside the file |

Ghost-session filtering (used by `list` and `prune`) is the same philosophy
on both providers: a session with `steps == 0` and an empty resolved title,
and no on-disk artifacts, is a ghost unless it is pinned.

### 4.1 `get_brain_stats` / `artifacts`

Antigravity's `brain/<uuid>/` holds freely generated markdown plans,
screenshots and scratch scripts. Claude Code has no equivalent scratch
folder - the only real per-session artifacts it produces are the transcript
itself and, optionally, todo-list snapshots under `~/.claude/todos/`. The
Claude Code provider reports those honestly instead of inventing synthetic
markdown/png counts:

- `recent_artifacts` contains the transcript file (`category: "transcript"`)
  plus any matching `~/.claude/todos/<id>*.json` files (`category: "todo"`).
- `md_count`, `png_count`, `scratch_count` are always `0` for this provider.
- `total_files` counts the transcript + todo files found.

### 4.2 `go` / `open` / `edit` / `nvim` target directory

Antigravity's "brain directory" is a space *outside* the real project,
holding generated artifacts, so `go`/`open`/`edit`/`nvim` jump there.
Claude Code has no such out-of-repo folder: all real work happens directly
in the project's own working directory. Per the explicit instruction to
adapt to what Claude Code actually uses rather than force Antigravity's
layout onto it, this provider resolves those commands to **the session's
recorded `cwd`** (the real project directory), falling back to the internal
`~/.claude/projects/<encoded-cwd>/` bookkeeping folder only if the recorded
`cwd` no longer exists on disk.

### 4.3 Rename

Claude Code's own `summary` entry is written by Claude Code itself and is
never mutated by agy-brain. `agy-brain rename` instead writes to a sidecar
file `agy_titles.json` (`{"<session-id>": "<custom title>"}`) inside the
Claude Code app dir. Title resolution checks this file first, exactly like
Antigravity's literal `title` column takes priority over `preview`.

### 4.4 Delete

Deletes the session's `.jsonl` transcript and any matching
`~/.claude/todos/<id>*.json` files, and cleans up any pin / title-override
entries that reference the id. `DeleteResult.db_deleted` means "transcript
file removed"; `DeleteResult.folder_deleted` means "todo snapshot files
removed" for this provider (field names are reused as-is to avoid widening
the struct for a cosmetic distinction; CLI messages are phrased per
provider, see `src/main.rs`).

## 5. Provider selection

New global flag: `--provider <auto|antigravity|claude-code>` (also settable
via the `AGY_PROVIDER` environment variable; flag wins over env var).
Default is `auto`:

1. If `--app-dir`/`AGY_APP_DIR` is explicit: a dir containing
   `conversation_summaries.db` selects Antigravity; a dir containing a
   `projects/` subdirectory selects Claude Code; otherwise Antigravity is
   kept as the default (preserves all existing behavior/tests that pass an
   empty/fresh app dir expecting Antigravity semantics).
2. Otherwise: if the default Antigravity database file exists, select
   Antigravity (preserves today's out-of-the-box behavior for existing
   users); else if the default Claude Code `~/.claude/projects` directory
   exists, select Claude Code; else default to Antigravity.

`agy-brain doctor` reports which provider was selected and why, in addition
to its existing per-provider health checks.

## 6. CLI/TUI parity matrix

| Command            | Antigravity | Claude Code |
|---------------------|:-----------:|:------------:|
| `list` / `ls`       | yes | yes |
| `info` / `show`     | yes | yes |
| `artifacts` / `art` | yes | yes (see 4.1) |
| `go`, `resolve`     | yes | yes (see 4.2) |
| `open`, `edit`, `nvim` | yes | yes (see 4.2) |
| `pin` / `unpin`     | yes | yes (separate sidecar files, 4) |
| `rename` / `mv`     | yes (SQLite) | yes (sidecar, 4.3) |
| `delete`            | yes | yes (4.4) |
| `prune`             | yes | yes |
| `doctor`            | yes | yes |
| `tui`               | yes | yes |
| `-s/--subagents`    | hides/shows child sessions | accepted, no-op (3.1) |

## 7. Test plan (TDD)

Implementation must be written test-first, mirroring the existing suite
file-for-file so both providers are verified to the same depth:

- `tests/claude_code_tests.rs` (unit-level, parallel to `core_tests.rs`):
  title cascade (summary / first user message / empty), step counting,
  mtime, strict workspace isolation via recorded `cwd`, ghost filtering,
  pin/unpin round-trip, rename via sidecar, delete (transcript + todos +
  pin/title cleanup), prune (dry-run and real), `resolve_target` (index,
  uuid, `default`, prefix match).
- `tests/cli_tests.rs` gains Claude-Code-flavored end-to-end scenarios
  (`--provider claude-code --app-dir <fixture>`) covering the same command
  sequences already exercised for Antigravity: list (local/global), go/
  resolve/info/artifacts, pin/unpin/rename/prune/delete, doctor.
- `tests/cli_tests.rs` gains a provider auto-detection test: an
  Antigravity-shaped fixture dir and a Claude-Code-shaped fixture dir must
  each auto-select the right provider with no explicit `--provider` flag.
- `tests/tui_tests.rs` gains headless coverage proving the TUI drives the
  Claude Code provider (pin toggle, delete, navigate) the same way it
  already does for Antigravity.

All new/changed code must keep `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo test --all-targets`
green, per `.github/workflows/ci.yml`.
