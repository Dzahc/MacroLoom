# ML-11 Configure window validation

## Automated coverage

The Node suite exercises exact decimal conversion and safe-range boundaries;
all speed presets; inactive-field preservation/fallback; blur/submit field
errors; Rust-compatible Unicode names; complete unchanged submission; pending,
duplicate and reentrant guards; cancellation; success, failure, rejection and
Retry; consumer field errors; and actual form markup with accessible labels,
units, errors and mode-sensitive disabled fields.

Rust tests exercise external draft validation against the existing file schema,
stable identity, Unicode/range rejection, operation/submission exclusion,
retained failure/Retry, successful acknowledgement and deferred-exit state, and
typed camelCase boundary/error payloads. The backend reuses the storage name
and playback validators rather than duplicating those rules.

## Interactive checks

A native development build compiled successfully. The initial native preview
could not reach the sandboxed Vite server. The preview server was restarted
outside the sandbox. The user then requested that PC control and cursor-focus
testing be skipped because of token cost; no interactive focus or 200% text-size
checks are claimed. Those behaviors have been implemented, with deterministic
state and rendered-markup coverage kept separate from manual Windows evidence.

## Completion gate

Command from the repository root: `npm.cmd --prefix app run verify`.
Result on October 9, 2026: **PASS**, exit code 0. All 14 checks passed,
including refreshed-origin comparison, formatting, strict TypeScript, ESLint,
production frontend build, Clippy, 56 frontend/tooling tests, maintained Rust
test suites, 33 Python quality fixtures and 258 changed-file function checks at
complexity ≤10. The desktop-only interactive Rust check remains intentionally
ignored in the deterministic suite.

The initial sandbox gate failed on access restrictions. Its unrestricted rerun
passed every check except Rust tests because the development executable was
still running. Closing that test instance released the binary; the final complete
gate then passed. No complexity exceptions or quality-tooling changes were added.
This command/result is also recorded in GitHub issue #11.

## PR #35 review fixes

The Configure editor has no frontend event permissions. Submit events now carry
only an attempt notification; the main window claims the validated native-held
draft exactly once before invoking the Save consumer. Acknowledgement requires
that claim, and invalid/replayed notifications leave the legitimate pending
callback unchanged.

Five additional Node regression tests cover event permissions, forged draft
payloads, unknown attempts, replay during/after Save, rejected consumers,
malformed acknowledgements and bridge disconnection. Three additional Rust tests
cover matching once-only claims, native trimming, invalid identity rejection,
stale attempts after Retry and acknowledgement ordering/replay.

The native close path reenables the owner before accepted editor closure and
does not request foreground activation after destruction. Windows handles normal
owned-window activation; the frontend restores the opening control only when
the main document is active. Pending Save continues to block native closure.
Interactive foreground-focus and text-scaling checks were not rerun; the
previously documented user-requested omission remains in effect. Review-fix
verification results are recorded in GitHub issue #11.
