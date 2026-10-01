## Agent skills

### Issue tracker

Engineering issues and specs live in GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the five canonical triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout: a root glossary and repo-wide ADRs. See `docs/agents/domain.md`.

## Stack and boundaries

- `app/src/`: React 19 + strict TypeScript owns presentation and UI state. Derive display state from backend snapshots; keep effects for external subscriptions, cancel timers/listeners on cleanup, and handle asynchronous failures. Keep rendering pure and preserve React hook dependencies.
- `app/src-tauri/`: Rust owns native input, session scheduling/cancellation, persistence, and platform adapters. Tauri 2 provides the desktop shell, typed command/event boundary, window integration, and least-privilege capabilities (`capabilities/`). Vite (`app/vite.config.ts`) builds and serves the frontend.
- Keep command inputs/results/events explicitly typed on both sides of `invoke`/`listen`; coordinate changes to TS contracts and Rust serde shapes. Validate external payloads and file data in the backend before using them. UI availability never replaces backend preconditions. Keep Win32 APIs and physical-coordinate conversion in `src-tauri/src/windows.rs`; the UI uses commands/events rather than platform calls.

## Code and Windows safety

- Use explicit Rust `Result`/`Option` handling at fallible boundaries. Explain invariants behind panic paths; keep owned session data and synchronization lifetimes clear. Put `unsafe` around the smallest necessary operations and document pointer/handle validity, initialization, and lifetime assumptions.
- Keep input callbacks short; run storage and long work away from the UI/native event loops. Allow one active session; recording and playback never overlap. Preserve monotonic deadlines and equal-timestamp ordering. Stop stays effective through waits, holds, drags, and repeat intervals; completion, failure, cancellation, and exit release session-owned input. Injection failure stops playback; resume never bursts overdue input.
- Preserve target focus on compact-view transitions and status updates. Exclude control shortcuts from recorded input. Check actual compact-window bounds before mouse injection; use signed physical screen pixels and compatible DPI handling.
- Validate schema versions, timestamps, ranges, and drag state. Persist atomically, retaining the previous valid file on failure. Failed recording saves retain recoverable data for Retry/Discard; discard needs the user's explicit action. Remove list entries only after disk deletion succeeds; failed property writes preserve persisted values and editable drafts. Read `docs/agents/PRD.md` sections 6–11 before changing session, window, input, timing, or persistence behavior.
- Add behavior-focused tests for changed logic. Keep deterministic tests distinct from Windows integration and manual target-application evidence. Preserve the glossary/ADR workflow above; read applicable domain documents before code exploration.

## Setup, commands, and completion

Read `docs/agents/quality-gate.md` before setup, changing quality tooling, investigating a gate failure, or requesting a complexity exception. It defines exact pinned setup commands, score semantics, unsupported syntax, and the owner-approval process.

From the repository root: `npm --prefix app ci`; create a Python 3.12.14 `.venv` and install `scripts/requirements-quality.txt`; install Rust 1.98.1 with rustfmt/Clippy; run `cargo fetch --locked --manifest-path app/src-tauri/Cargo.toml`. Windows also requires MSVC C++ Build Tools and WebView2. The linked setup provides exact Windows/Unix commands.

| Task                 | From root                                                                                               | From `app/` or `app/src-tauri/`                                                    |
| -------------------- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Frontend setup       | `npm --prefix app ci`                                                                                   | `npm ci` in `app/`                                                                 |
| Format/fix           | `npm --prefix app run format`                                                                           | `npm run format` in `app/`                                                         |
| Format checks        | `npm --prefix app run format:check`                                                                     | `npm run format:check` in `app/`                                                   |
| Type check           | `npm --prefix app run typecheck`                                                                        | `npm run typecheck` in `app/`                                                      |
| Frontend lint/fix    | `npm --prefix app run lint` / `npm --prefix app run lint:fix`                                           | `npm run lint` / `npm run lint:fix` in `app/`                                      |
| Frontend build/tests | `npm --prefix app run build` / `npm --prefix app test`                                                  | `npm run build` / `npm test` in `app/`                                             |
| Rust format/check    | `cargo fmt --manifest-path app/src-tauri/Cargo.toml` / append `-- --check`                              | `cargo fmt` / `cargo fmt -- --check` in `app/src-tauri/`                           |
| Clippy               | `cargo clippy --manifest-path app/src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings` | `cargo clippy --locked --offline --all-targets -- -D warnings` in `app/src-tauri/` |
| Rust tests           | `cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline --all-targets`                  | `cargo test --locked --offline --all-targets` in `app/src-tauri/`                  |
| Completion           | `npm --prefix app run verify`                                                                           | `npm run verify` in `app/`                                                         |

On PowerShell use `npm.cmd` when execution policy blocks `npm.ps1`. **Before declaring a code task complete, pass `npm --prefix app run verify` and record its command/result in the issue or PR.** It fetches `origin`, then checks formatting, static analysis, production frontend build, every maintained test suite, and cyclomatic complexity ≤10 for added/touched `.ts`/`.tsx`/`.rs` functions, methods, and closures. Nested definitions have independent scores; their edits also recheck enclosing functions. Formatting edits, moves, and renames count as touches. Default base: merge base with refreshed `origin/develop`; override with `npm --prefix app run verify -- --base-ref <ref>`. Index/worktree and untracked files are covered. Exceptions require the owner's explicit approval and a narrowly keyed entry. Checks leave source unchanged and fail clearly on missing prerequisites or unsupported mapping. Hooks and CI are outside ML-25.
