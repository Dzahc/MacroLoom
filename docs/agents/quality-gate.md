# Local quality gate

Run from the repository root before declaring a code task complete:

```sh
npm --prefix app run verify
npm --prefix app run verify -- --base-ref origin/develop
```

PowerShell users can spell npm as `npm.cmd` if their execution policy blocks `npm.ps1`.
The runner uses argument arrays and native executables, with no shell-specific pipeline.
It fetches `origin` before resolving the comparison base. A failed fetch fails verification;
there is no fallback to an older remote ref. Fetch updates Git metadata. All remaining
checks leave maintained source unchanged. Caches, frontend build output, Rust targets,
and test output are permitted. Verification does not install packages or apply fixes.

## Reproducible setup

Install Node **24.21.0**, npm **12.1.0**, Python **3.12.14**, Git, and Rust via rustup.
Version selectors are `.nvmrc`, `.python-version`, `package.json`, and
`rust-toolchain.toml`. npm dependencies and their transitives are locked in
`app/package-lock.json`; Cargo dependencies are locked in `app/src-tauri/Cargo.lock`.
Python packages, including transitives, have exact versions in
`scripts/requirements-quality.txt`. Verification checks the installed frontend and
Python versions, and verifies that Rust/rustfmt/Clippy are already installed.

On Windows, also install the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/):
MSVC C++ Build Tools and WebView2. Keep `%USERPROFILE%\.cargo\bin` on PATH.
The verification tooling is portable. The current native application uses Windows
APIs and its complete gate is validated on Windows; this ticket does not port it.

From the repository root on Windows:

```powershell
npm.cmd --prefix app ci
py -3.12 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r scripts/requirements-quality.txt
rustup toolchain install 1.98.1 --profile minimal --component rustfmt --component clippy
cargo fetch --locked --manifest-path app/src-tauri/Cargo.toml
```

Confirm `py -3.12 --version` is 3.12.14 first. If Python is not registered with `py`,
use the installed Python 3.12.14 executable for the venv command. On macOS/Linux,
use `python3.12 -m venv .venv` and `.venv/bin/python -m pip install -r scripts/requirements-quality.txt`;
the npm, rustup, and Cargo commands are the same.
Dependency downloads belong to setup; Cargo checks use `--locked --offline`.

## Commands and coverage

| Operation                 | Repository root                                                                                         | Within `app/` or `app/src-tauri/`                                                  |
| ------------------------- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Install frontend          | `npm --prefix app ci`                                                                                   | `npm ci` in `app/`                                                                 |
| Development               | `npm --prefix app run tauri -- dev`                                                                     | `npm run tauri -- dev` in `app/`                                                   |
| Format/fix                | `npm --prefix app run format`                                                                           | `npm run format` in `app/`                                                         |
| Format check              | `npm --prefix app run format:check`                                                                     | `npm run format:check` in `app/`                                                   |
| Strict type check         | `npm --prefix app run typecheck`                                                                        | `npm run typecheck` in `app/`                                                      |
| Frontend lint / fix       | `npm --prefix app run lint` / `npm --prefix app run lint:fix`                                           | `npm run lint` / `npm run lint:fix` in `app/`                                      |
| Frontend production build | `npm --prefix app run build`                                                                            | `npm run build` in `app/`                                                          |
| Frontend/tooling tests    | `npm --prefix app test`                                                                                 | `npm test` in `app/`                                                               |
| Rust format / check       | `cargo fmt --manifest-path app/src-tauri/Cargo.toml` / append `-- --check`                              | `cargo fmt` / `cargo fmt -- --check` in `app/src-tauri/`                           |
| Rust static analysis      | `cargo clippy --manifest-path app/src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings` | `cargo clippy --locked --offline --all-targets -- -D warnings` in `app/src-tauri/` |
| Rust tests                | `cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline --all-targets`                  | `cargo test --locked --offline --all-targets` in `app/src-tauri/`                  |
| Complexity fixtures       | `.venv/Scripts/python.exe -m unittest discover -s scripts/tests -v`                                     | Run from root; Unix uses `.venv/bin/python`                                        |
| Complexity only           | `.venv/Scripts/python.exe -m scripts.quality.complexity --base-ref origin/develop`                      | Run from root; does not fetch itself                                               |
| Completion gate           | `npm --prefix app run verify`                                                                           | `npm run verify` in `app/`                                                         |

Prettier checks maintained frontend and supported text formats across the repository;
rustfmt covers the crate. TypeScript strict checking includes presentation, Vite
configuration, and task scripts. ESLint uses typed TypeScript and React hook rules.
Clippy warnings fail. The gate runs every maintained test suite, including Python
analyzer/Git fixtures and Node's built-in frontend/tooling regression tests. No
additional frontend test framework is introduced. Tauri installer packaging remains
a separate release command. Independent failures are collected into a final summary;
dependent checks are omitted only after their prerequisite has failed visibly.

## Changed functions and cyclomatic scores

The default comparison is the merge base of `HEAD` and refreshed `origin/develop`.
An explicit `--base-ref` changes that reference, without skipping the fetch.
Both index and final working-tree snapshots are checked against the base. This
includes committed branch changes, staged/unstaged edits, and new untracked source.
A staged violation cannot be hidden by an unstaged fix: restage the fix before
completion. Conflicts, unavailable refs, decoding failures, parse errors, and
ambiguous function mappings fail with diagnostics.

Maintained `.ts`, `.tsx`, and `.rs` files are in scope, including config, build,
test, and task code. Dependencies, `dist`, Rust `target`, `.venv`, and generated
Tauri schemas are excluded. Artificial complexity fixtures are strings in tests,
so they cannot become accidental application-source exceptions.

A changed line intersecting a function's signature, attached Rust attributes,
or body brings the function into scope, including comments and formatting edits.
Nested definitions are scored independently; editing one also selects all its
enclosing functions. Calling another function adds no complexity. Untouched legacy
functions above 10 are exempt. Renames and moves trigger rechecking even when code
is unchanged. Fully deleted functions have no current score. Removed lines within
a surviving function still select it.

The limit is **10 inclusive**. Scores start at 1. Add one for each `if`, loop,
catch, conditional expression, short-circuit `&&`/`||`/`??` (including logical
assignment), optional-chain operation, Rust `?`, Rust `let ... else`, and guarded
match arm. TypeScript `switch` adds one per nondefault case. Rust `match` adds
`max(0, number of arms - 1)`; alternatives sharing one arm count as one outcome.
Calls, ordinary `else`, types/generic constraints, literals/comments, JSX markup,
and nested function bodies add nothing to their enclosing score. Executable
parameter-default expressions contribute to the function's score.

The analyzer is **Lizard 1.24.0**, with pinned Tree-sitter TS/TSX/Rust grammars.
Syntax trees establish source spans and discover declarations, methods, generators,
arrows, async functions, extern hooks, and expression/block/unsafe/move closures.
Executable bodies are projected into Lizard-compatible tokens: nonexecuted text,
types, and nested definitions are removed; nullish/optional operators and Rust
`loop` are normalized. TS/TSX projections use Lizard's C++ decision reader to avoid
its TypeScript declaration heuristics; Rust uses its Rust reader with the documented
match-arm adjustment. Every score is reconciled independently against the syntax
tree's branching policy. A disagreement fails rather than accepting either result.
This is cyclomatic complexity, not cognitive complexity.

Rust expression-macro arguments are parsed as calls while preserving byte offsets,
including nested macros. Macro-expanded/generated implementations are outside
source analysis. Macro definitions and token trees that cannot be parsed as
expression arguments are unsupported and fail when a changed file contains them.
New unsupported syntax requires demonstrated parser/analyzer support before passing.
This boundary is deliberate; there are no blanket syntax exemptions.

## Owner-approved exceptions

`scripts/complexity-allowlist.json` starts empty. Request the owner's explicit
approval in the issue or PR before adding an entry. Record the exact relative
path, fully qualified function name from a diagnostic, approved maximum score,
specific reason, and URL of the approval comment:

```json
{
  "path": "app/src-tauri/src/example.rs",
  "function": "Example::method",
  "max_score": 11,
  "reason": "Explain why this exact function needs an exception",
  "approval": "https://github.com/Dzahc/MacroLoom/issues/25#issuecomment-123"
}
```

The example is illustrative and is not an approval. Patterns, directory/file
exemptions, duplicates, missing reasons/approval links, and limits at or below 10
are rejected. Further growth fails and requires renewed approval. Missing, renamed,
or resolved functions make entries stale and fail validation. Anonymous callback
identities include their owning scope, ordinal, and source hash so an exception
cannot silently transfer to another callback; changing its definition invalidates
that identity. Approval links are review evidence, not a cryptographic verification
of the approver. The owner reviews the actual allowlist change.

## Validation evidence

Before refactoring, raw Lizard reported the React `App` at 49, `Engine::play` at 17,
and `start_hooks` at 12; it produced no separate Rust worker closure records.
The adapted policy reported `App` at 40, playback's closure at 14, hook startup's
closure at 11, and `keyboard_hook` at 11. It separately recognized the TSX effect,
promise, cleanup, JSX-handler callbacks and Rust setup/worker closures. These are
different policies; raw output is recorded to show why an unadapted reader cannot
enforce this repository's contract.

The fixture suite proves 10 passes/11 fails in TS, TSX React components and arrows,
Rust functions, methods, extern hooks and closures; nested scores and parent scope;
operator/match rules; literal/type exclusion; all Git work states; touched/untouched
legacy code; moves, renames, deletions; exact exceptions, growth and staleness;
and clear rejection of unsupported syntax. Record the final gate command and
output in the implementation issue or PR. Manual Windows focus/input/timing and
packaging validation remain governed by PRD section 11.
