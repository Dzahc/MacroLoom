# ML-22 — Exit and suspend handling validation

Reviewed scope: [ML-22 lifecycle contract](../specs/ML-22-exit-and-suspend-handling.md).

## Evidence boundaries

Deterministic session/operation tests substitute desktop effects. Native teardown
checks use real Windows hooks and reservations. Neither establishes actual sleep
or hibernation behavior. Owner-run observations below remain pending until reported.
Do not interact with Google Play Games.

## Owner-run close and recovery

Use the normal development application and a blank Notepad document. Keep the
compact window away from the recorded targets. Report build/commit, Windows
version, target application/version, display layout/scaling, expected/actual
behavior, and whether each observation repeats.

1. Close during a nonempty recording. Expect one saved file, then exit. Relaunch
   and replay the macro. Repeat with an empty capture: expect no new file.
2. Cause a recoverable recording save failure using an isolated test installation
   and storage folder. Close: expect Retry and exit, Discard and exit, Keep open.
   Repeated Close must neither retry nor discard. Restore writable storage and
   Retry: expect one committed macro, then exit.
3. Repeat failure and choose Keep open. Expect retained capture, ordinary Retry
   save/Discard recording, and no later automatic exit. Discard only by explicit
   action. A successful ordinary Retry keeps the app open.
4. Close during playback's initial wait, key hold, drag, and repeat interval.
   Expect no later ordinary input and released session-owned holds before exit.
5. Close while a confirmed deletion is pending: success exits; failure keeps the
   app open with its entry retained and ordinary deletion error. Closing an
   unconfirmed Delete dialog cancels it and exits. Check existing Configure
   Save/close success/failure behavior remains intact.

## Owner-run sleep and hibernation

Run these checks separately for actual computer **sleep** and **hibernation**.
Locking or blanking the display does not exercise this requirement. Save unrelated
work first. Use ordinary Notepad targets and the normal library build.

1. Interrupt a nonempty recording. Wake: recording has ended; one macro is saved,
   or saving completes after wake. The banner retains the interruption reason and
   outcome. Empty interrupted recording creates no file. Capture never restarts.
2. Repeat with a recoverable save failure: capture and Retry/Discard survive wake.
   Repeated sleep/wake must not retry saving without explicit Retry.
3. Interrupt playback during its initial wait, an owned key hold, a drag, and a
   repeat interval. Wake: no overdue burst, next run, or automatic restart. Owned
   holds are released; the persistent banner identifies the interruption.
4. If cleanup or saving was paused by suspension, allow it to finish after wake.
   Failure stays actionable. An existing pending application exit is honored only
   after successful saving and resource teardown.
5. Lock/unlock and blank/unblank the display separately with an unchanged desktop
   layout. These alone must not cancel a compatible session.

## Automated result

Initial implementation (`99af403`), October 24, 2026:
`npm.cmd --prefix app run verify` — **PASS**, exit 0,
against refreshed `origin/develop` at `15a6c95`. Formatting, type checking, ESLint,
production build, rustfmt, Clippy, 81 frontend/tooling tests, 110 Rust unit tests
(six additional interactive tests ignored), 21 Rust repository integration tests,
33 Python quality fixtures, and 586 function/closure complexity checks passed.
No complexity exceptions. Local output: `app/.quality-output/ML-22-verify.log`.

The behavior coverage includes failed-close save recovery and frozen identity,
repeated Close/suspend, Close joining a pending save, empty/nonempty recording
interruption feedback, interruption feedback across shortcut conflicts, deletion
success/failure exit coordination, preservation of committed deletion outcomes,
editor exclusion, and Close during held playback and repeat intervals. Failed
held-input cleanup remains pending across refresh/repeated Close/suspend until
explicit Retry; final native shutdown failures also require explicit Retry/F8.

`cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline --lib native_hook_shutdown_acknowledges -- --ignored --test-threads=1 --nocapture`
— **PASS** with real hooks and F8/F9 registrations, acknowledged teardown, and
actual shortcut handoff. This does not inject target-application input.

`cargo run --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --bin session-control-check`
— **PASS**, exit 0, using isolated real WebViews, real shortcut conflicts, and
callback fixtures through the native control handler. Normal main-window close
returned only after hook teardown; another owner acquired both F9 and F8. Output:
`app/.quality-output/ML-22-native-controls.log`. The initial run hit the check's
immediate foreground assertion; waiting for `document.hasFocus()` after the
asynchronous focus request made the check pass. No real keyboard or computer
suspend observation is inferred from callback fixtures.

`cargo clippy --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --all-targets -- -D warnings`
— **PASS** for the additional native-check feature code.

## Review corrections and final verification

Independent review found and corrected two lifecycle edges: property submissions
after accepted main Close now use the same atomic session gate as editor creation;
every native shutdown-job failure attempts F8 recovery independently of failed F9
release. Recovery text names F8 only when its reservation is available. The shared
Configure deferred-exit rule now lives on ConfigureState, and the rendering test
uses the source-owned recovery label. Deterministic and real native regressions
reproduced the corresponding failures before their fixes.

Through `e9f5c46`, `npm.cmd --prefix app run verify` — **PASS**, exit 0, against
refreshed `origin/develop` at `15a6c95`: 81 frontend/tooling tests, 111 Rust unit
tests (seven interactive tests ignored), 21 Rust repository integration tests,
33 Python fixtures, and 1,244 function/closure checks across index/worktree.
All formatting, static analysis, production build, and environment checks passed,
with no complexity exceptions. Output: `app/.quality-output/ML-22-verify-final.log`.

`cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline --lib native_shutdown_reservation_failure_restores_stop -- --ignored --test-threads=1 --nocapture`
— **PASS**, verifying actual F8 recovery and rejection of a competing owner after
an intentionally lost F9 reservation makes shutdown fail. No target input is sent.

The real-WebView `session-control-check` above passed again after corrections,
including acknowledged close and shortcut handoff. Output:
`app/.quality-output/ML-22-native-controls-final.log`. Native-feature Clippy also
passed again with `--features native-checks --all-targets -- -D warnings`.

Follow-up Standards review: **0 remaining actionable findings**.
Follow-up Spec review: **0 remaining actionable findings**.

## Shortcut-error feedback regression

October 24, 2026, through `d04149f`: automatic native shortcut refresh errors
are retained separately from session feedback. Previously, `Session::report`
replaced the interruption message, and successful refresh recognized only conflict
text, leaving the native error visible indefinitely. The snapshot now composes
interruption context and current native errors; recovery removes only that
transient error. Unrelated actionable feedback remains visible, and repeated
reports do not duplicate text. Failed cleanup uses a reason-only sleep/hibernation
explanation until held input is actually released.

`cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline --lib wake_shortcut_error_preserves_the_interruption_outcome -- --nocapture`
reproduced the reported failure before the fix: recovered feedback was
`Injected native failure` instead of the retained interruption outcome. The
regression now covers interrupted recording and playback. Additional tests cover
a real committed recording outcome, unrelated errors, duplicate reporting, and
truthful interruption feedback during failed held-input cleanup. All four
`wake_shortcut` tests and all 35 `session::tests::` tests passed.

Final `npm.cmd --prefix app run verify` — **PASS**, exit 0: 81 frontend/tooling
tests, 115 Rust unit tests (seven interactive tests ignored), 21 Rust repository
integration tests, 33 Python fixtures, and 1,260 function/closure checks across
index/worktree. Formatting, type checking, lint, production build, Clippy and all
other checks passed against refreshed `origin/develop` at `15a6c95`, with no
complexity exceptions. Output:
`app/.quality-output/ML-22-shortcut-feedback-verify-final.log`.

Follow-up Standards and Spec reviews: **0 remaining actionable findings**.
These are deterministic regressions; actual sleep/hibernation evidence below
remains pending.

## Owner evidence

Actual sleep and hibernation checks: **pending**.
