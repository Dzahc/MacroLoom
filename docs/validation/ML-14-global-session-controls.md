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
