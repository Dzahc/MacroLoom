# ML-14 — Global session controls validation

## Scope and evidence

The reviewed scope is [ML-14](../specs/ML-14-global-session-controls.md).
Automated checks exercise the backend session command/snapshot boundary, native
shortcut sequence policy, and frontend listener/action reconciliation. A native
reservation check is separate from deterministic tests. Target-application
observations below are pending the owner's report, rather than inferred from
automated results.

ML-14 retains recordings in memory. It does not create or update macro JSON files.
Automatic recording saving belongs to ML-15; saved-macro playback to ML-17;
recording-on-sleep automatic saving is specified in ML-22.

## Launch

From the repository root:

```powershell
npm.cmd --prefix app run tauri -- dev
```

Use the normal library build, without `input-prototype` or `native-checks`.
Use a blank Notepad document first. Keep its window at the same size and position
through recording/replay. Place MacroLoom away from the intended mouse targets.

## Record, repeats, focus, and Stop

1. Focus Notepad. Hold F9 for **two seconds**, then release it. Expect one recording
   to start, the recording timer to run continuously, and MacroLoom to become a
   compact topmost toolbar without taking focus.
2. Click into the blank document and type `alpha`. Hold F9 again for **two seconds**,
   release it, and type `beta`. Expect no timer reset, restart, or second recording.
   Both text fragments must appear in Notepad.
3. Hold F8 for **two seconds**, release it, and type `gamma` without clicking
   anywhere. Expect recording to stop promptly, the full library to return without
   taking focus, and `gamma` to appear in Notepad outside the stopped recording.
   The retained recording must be identified as unsaved; no new library row/file
   is expected in ML-14.
4. Start a new recording with the toolbar button. Expect the previous temporary
   capture to be replaced without a discard prompt. Stop it with the toolbar.
   Verify both button and shortcut paths show the same lifecycle/status.

## Temporary playback and cancellation

1. Open the separate development controls from the main window. Start a recording
   with F9 from Notepad, wait **five seconds**, click into the document, type
   `wait-test`, and press F8. Clear the text without moving the target window.
2. In development controls, choose **Replay temporary recording**. Press F8 during
   the initial five-second wait. Expect no later click or typing, prompt stopping,
   and full-window restoration. Starting a new recording must then work.
3. Record a target click, press and hold Shift for **five seconds**, release Shift,
   and stop. Replay and press F8 during that hold. Expect later ordinary playback
   to cease and Shift to be released; typing a lowercase letter afterwards must
   not act as though Shift remains injected down.
4. Repeat with a **five-second left-button drag/hold**. Stop playback midway. Expect
   the injected button to release at its current location, without jumping to an
   unexecuted destination or leaving a stuck drag.
5. Move the compact MacroLoom window over a recorded mouse target and replay.
   Expect playback to fail before injecting into MacroLoom, with a message to move
   the window. Move it away and retry.

Saved-library Play remains disabled. Temporary replay controls exist only in
development builds. There is no global Play shortcut.

## Controlled registration conflicts

Run the helper in a second PowerShell terminal; press Enter in that terminal to
release its reservation. Do not leave it running after the test.

For a Record conflict, close MacroLoom first, then run:

```powershell
cargo run --manifest-path app/src-tauri/Cargo.toml --locked --offline --bin shortcut-conflict-check -- record
```

Launch MacroLoom. Expect an explanation that F9 is unavailable. Button recording
must still work because F8 remains available. Stop, release the helper with Enter,
then choose **Retry shortcuts**. Expect F9 to work without restarting MacroLoom.

For a Stop conflict, leave MacroLoom idle and run:

```powershell
cargo run --manifest-path app/src-tauri/Cargo.toml --locked --offline --bin shortcut-conflict-check -- stop
```

Attempt Record. Expect rejection before any capture starts, with clear F8 conflict
feedback; the previous temporary capture must remain intact. Release the helper
and choose **Retry shortcuts**. Expect recording to become usable again.

## Close, sleep, and operation exclusion

- Close during a nonempty recording. Expect capture to stop before the unsaved
  decision. **Keep open** retains input; **Discard and exit** closes only after
  cleanup. No success/save message is expected in this interim story.
- Put the **computer to sleep** while recording, then wake it. Expect the session
  ended, temporary capture retained, and an interruption message. It must not
  continue recording after wake. Repeat during playback: no overdue input burst
  may occur. Display blanking alone is not this test.
- Open Configure, a Delete confirmation, and a pending deletion in turn. Try F9
  and the Record button. Expect no overlapping session. Cancel/finish the operation
  and check availability returns. Stop stays usable during active session cleanup.
- Cleanup failure/retry is covered with controllable native-effect failures in
  automated tests; manual tests need not deliberately strand desktop input.

Repeat the relevant flows in the intended target application, including Google
Play Games if applicable. Check focus, shortcut interference, repeat handling,
and cancellation rather than assuming Notepad evidence transfers.

## Report format

Report: build/commit, application and version, Windows version, display scaling,
step, expected result, actual result, and whether it repeats. Note any unexpected
focus change, timer reset, second session, delayed Stop, subsequent input, or held
key/button. A screen recording helps investigate timing, but manual perception
does not establish the numerical 100 ms response target.

## Automated result

October 12, 2026: `npm.cmd --prefix app run verify` — **PASS**, exit 0 against
refreshed `origin/develop` at `1ef6578`. Formatting, type checking, ESLint,
production frontend build, rustfmt, Clippy, 72 frontend/tooling tests, 58 Rust
tests (two additional interactive tests ignored), 33 Python quality fixtures,
and 978 function/closure complexity checks across index/worktree passed, with
no exceptions. Implementation reviewed through `ffb01aa`.
Local full output: `app/.quality-output/ML-14-verify-final.log`.

`cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline native_shortcut_conflict_recovers_and_releases_stop -- --ignored --test-threads=1 --nocapture`
— **PASS** using actual Windows global registration. This confirms conflict,
retry, and release/handoff; it does not establish physical keyboard handling or
target-application focus/Stop response.

`cargo run --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --bin compact-preview-check`
— **PASS**: real WebViews rendered the main and development controls, applied the
preview state, and closed both windows normally. This is a presentation smoke
check; native input hooks are disabled in this check build.

Independent Standards and Spec reviews against starting commit `1ef6578` found
no remaining actionable findings through `ffb01aa`. The Standards review retained
an optional Data Clumps suggestion for the existing Windows key tuple; it is not
a documented standards violation and does not affect acceptance.

The owner-run target-application checklist remains pending.

## Owner feedback and development subscription fix

October 12, 2026: the owner reported **Record, repeats, focus, and Stop passed**.
Temporary playback/cancellation was blocked when opening development controls:
`Session control failed: event.listen not allowed on window "compact-development"`.
Playback/cancellation acceptance remains pending; this report establishes no
successful playback evidence.

The real-WebView check was strengthened to require the development pane's
listener-first session snapshot, and to fail immediately on its visible session
transport error. The same command above reproduced the exact denial with exit 1.
Adding a separate capability for `compact-development`, granting only
`core:event:allow-listen` and `core:event:allow-unlisten`, made the check pass
with exit 0. The main window's broader capability remains scoped to `main`.

Restart the native application before retrying **Temporary playback and
cancellation**, because capabilities are embedded in the native build. The
previously reported Record/Stop result remains recorded; repeat it only if a
regression appears.

Follow-up verification: `npm.cmd --prefix app run verify` — **PASS**, exit 0,
with `CARGO_TARGET_DIR` set to the workspace-local
`app/.quality-output/ML-14-verification-target`. The initial default-directory
attempt was prevented by Windows locking the owner's running `macroloom.exe`;
the separate cache allowed verification while preserving that process.
All gate checks passed: 72 frontend/tooling tests, 58 Rust tests (two additional
interactive tests ignored), 33 Python fixtures, and 988 function/closure
complexity checks across index/worktree, with no exceptions. Full output:
`app/.quality-output/ML-14-permission-verify.log`.

`cargo clippy --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --all-targets -- -D warnings`
also passed for the strengthened native regression. Follow-up Standards review:
no actionable findings. Follow-up Spec review: no actionable findings.

## Further owner acceptance and controlled conflicts

October 12, 2026: the owner confirmed **Temporary playback and cancellation**
passed. The owner delegated **Controlled registration conflicts** to the agent
and reported physical F8/F9 were ineffective with either the main window or
development pane focused. The subsequent trace and real-delivery regression below
identified the focused-window runtime issue.

The agent ran:

```powershell
cargo run --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --bin session-control-check
```

**PASS** using competing real Windows F9/F8 registrations and actual React
controls in real WebViews: visible conflict text and Retry shortcuts, Record
button fallback with F9 unavailable, F8 conflict rejecting startup while retaining
the previous capture, and retry recovery after releasing each competing key.
The check also focused the real main and development windows and exercised
non-injected callback fixtures through the native shortcut handler: F9 started,
F8 stopped, and own-window ordinary typing stayed excluded. Callback fixtures
do not establish that physical hardware events arrive correctly on the owner's
desktop. No keyboard input was sent to another application.

Final follow-up `npm.cmd --prefix app run verify` — **PASS**, exit 0: 72
frontend/tooling tests, 58 Rust tests (two additional interactive tests ignored),
33 Python fixtures, and 1046 function/closure complexity checks across
index/worktree. Formatting, lint, type checking, production build, and all other
gate checks passed. Full output: `app/.quality-output/ML-14-controls-verify.log`.
The final native control check also passed with opt-in tracing enabled; output:
`app/.quality-output/ML-14-native-controls.log`. Clippy with `--features native-checks
--all-targets -- -D warnings` passed. Follow-up Standards and Spec reviews found
no remaining actionable findings. The owner subsequently supplied the diagnostic
trace summarized below.

## Focused-window hook delivery regression and fix

October 12, 2026: the owner's trace showed real non-injected F9/F8 callbacks and
successful Record/Stop with `app_focused=false`, and no callbacks at all with
either MacroLoom window focused. Direct callback fixtures had skipped the failing
Windows-to-hook delivery path.

The real-WebView `compact-preview-check` now focuses each owned test window,
installs a genuine low-level observer with its own message pump, and sends tagged
F9 release through Windows `SendInput`. It requires the OS-delivered callback;
the injected tag is excluded from session capture and control actions. The initial
down/up version reproduced the missing-delivery symptom before the fix: observed
mask 0, expected mask 3, exit 1. Output:
`app/.quality-output/ML-14-focused-hook-red.log`.

Setting `.device_event_filter(tauri::DeviceEventFilter::Always)` on the desktop
builder made the identical check pass for both main and development windows,
exit 0. `Always` filters Tao's raw device events; the pinned Tao implementation
removes its raw keyboard/mouse registration. MacroLoom continues using its native
hooks. This matches the [upstream focused-hook report](https://github.com/tauri-apps/wry/issues/1761)
and its reported setting. No DOM shortcut fallback or duplicate action path was
added. Output: `app/.quality-output/ML-14-focused-hook-green.log`.

Temporary shortcut tracing and its environment switch were removed after the
cause was reproduced. To confirm on the owner's physical keyboard, restart the
normal development app with the Launch command above, then:

1. Wait for library loading to finish. With the **main window** focused, tap F9.
   Expect recording and the compact view. Tap F8 without changing focus; expect
   recording to stop and the full view to return.
2. Repeat with the **development pane** focused. Expect the same lifecycle.
3. Repeat with **Notepad** focused to check the existing global path. F8 only
   stops active work; no phase change is expected from F8 while idle.

October 12, 2026: the owner explicitly confirmed **F8/F9 work now with MacroLoom
focused**. Physical-keyboard acceptance for the focused-window fix is **PASS**.
The earlier Record/repeats/focus/Stop and temporary playback/cancellation passes
remain recorded. Close, sleep, and operation-exclusion manual observations remain
pending; this confirmation does not establish a numerical Stop response time.

The final review required safer native test failure handling: abandon and release
an observer when readiness delivery fails, and send only a release so partial
injection cannot leave a held key. Those changes are applied. The check also waits
for the asynchronous focus request to settle through `document.hasFocus()` before
probing, and verifies foreground ownership after delivery.

After the usage-limit interruption, the final release-only check passed in both
windows, exit 0. Output: `app/.quality-output/ML-14-focused-hook-final.log`.
Native-feature Clippy passed. Standards review found no remaining fixture or
standards findings; Spec review found no actionable findings.

Final `npm.cmd --prefix app run verify` — **PASS**, exit 0, with the separate
workspace-local Cargo target directory: 72 frontend/tooling tests, 58 Rust tests
(two additional interactive tests ignored), 33 Python fixtures, and 1094
function/closure complexity checks across index/worktree, with no exceptions.
Formatting, type checking, lint, production build, and all other gate checks
passed. Full output: `app/.quality-output/ML-14-focused-verify.log`.

## Startup queue and cancellation regressions

The owner identified two startup races: a second F9 followed by F8 during compact
entry could execute after cleanup and replace the capture, and Stop or sleep
during F8 reservation could be lost before recording/playback became active.

Deterministic tests exercise the production native control arrival/execution
boundary and substitute only desktop effects. Before the fix:

- `cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline startup --lib`
  failed: stale F9 left zero events instead of the two retained events. Output:
  `app/.quality-output/ML-14-startup-red.log`.
- `cargo test --manifest-path app/src-tauri/Cargo.toml --locked --offline cancellation_during_reservation --lib`
  failed: recording started after Stop; playback injected two events instead of
  zero. Output: `app/.quality-output/ML-14-reservation-red.log`.

Record/Play requests now carry their arrival generation. Startup claims
cancellation ownership before waiting for a presentation transition or acquiring
native reservations. Stop and sleep signal immediately; their queued cleanup
belongs to the original generation. Cancelled preactivation startup retains the
previous temporary capture and dispatches no playback input. An F9 received
during startup cannot become a replacement recording after cleanup.

Regression coverage also checks two starts received before worker dispatch,
idle Stop/sleep invalidating queued Record/Play, delayed cleanup preserving a new
recording, and Stop/sleep while startup waits behind a preview transition.
These tests do not claim new physical-keyboard or actual computer-sleep evidence.
The previously recorded owner acceptance and pending manual lifecycle observations
remain applicable.

Review found and corrected a further ownership gap: redundant unguarded idle
shortcut reconciliation could release a newer session's F8. Terminal cleanup now
performs that reconciliation once under the transition guard, and both startup
and playback completion capture cleanup generation before relinquishing ownership.

Final `npm.cmd --prefix app run verify` — **PASS**, exit 0, using
`CARGO_TARGET_DIR=.quality-output/ML-14-startup-target`: 72 frontend/tooling tests,
65 Rust tests (two additional interactive tests ignored), 33 Python quality
fixtures, and 1121 function/closure complexity checks across index/worktree,
with no exceptions. All formatting, type checking, lint, production build, and
other gate checks passed. Output: `app/.quality-output/ML-14-startup-verify.log`.
The session suite includes seven new startup/queue regression tests.

`cargo clippy --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --all-targets -- -D warnings`
also passed. Output: `app/.quality-output/ML-14-startup-native-clippy.log`.
Follow-up Standards review: no remaining actionable findings. Follow-up Spec
review: no remaining actionable findings.
