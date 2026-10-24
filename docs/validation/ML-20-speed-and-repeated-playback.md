# ML-20 — Speed and repeated playback validation

## Scope

[ML-20](https://github.com/Dzahc/MacroLoom/issues/20) and the
[confirmed decisions](../specs/ML-20-speed-and-repeated-playback.md).
Implementation starts at `7bd8822` on the owner's current
`feature/ml-20-speed-and-repeated-playback` branch.

## Automated evidence

October 24, 2026: focused checks pass for:

- Every saved speed preset, including exact 0.25 ms deadlines at 4×.
- Ordered equal-time events, absolute deadlines and preserved trailing waits.
- Three exact runs with fresh run epochs, simulated dispatch/cleanup overhead,
  full unscaled intervals and no interval after the final run.
- Indefinite cancellation during an interval and zero-duration/zero-interval
  indefinite playback without an artificial timing floor.
- Saved-property snapshots, Once/fixed-one semantics, safe-integer totals and
  infinity, per-run held-input release, failed-release exclusion and Retry.
- Stop, suspend and detected environment changes during repeat intervals; existing
  startup, initial-wait, active-drag and native-injection cancellation regressions.
- Startup elapsed time without early dispatch, idle progress reset, preparation
  and finishing banners, and upward-rounded positive countdowns.

The scheduler checks use a controllable clock and controlled input effects.
Session checks run on Windows with controlled desktop effects and the real
repository. They send no input to Notepad or another target application and do
not establish physical-shortcut behavior or a numerical Stop-response budget.

`npm.cmd --prefix app run verify` — **PASS**, exit 0, against refreshed
`origin/develop` at `7bd8822`. Formatting, type checking, ESLint, production
frontend build, rustfmt, Clippy, all 80 frontend/tooling tests, 121 Rust tests
(five interactive tests ignored), 33 Python quality fixtures, and 308
function/closure complexity checks passed without exceptions. Local output:
`app/.quality-output/ML-20-verify.log`.

`cargo clippy --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --all-targets -- -D warnings`
also passed, exit 0. Independent Standards and Spec reviews follow this gate.

## Notepad acceptance — pending

Launch the normal application with `npm.cmd --prefix app run tauri -- dev`.
Keep Notepad at its recorded position/size and place the compact toolbar away
from mouse targets. Include a click into the blank document before typing.

1. Record a short line with an initial wait, a held key and a trailing wait.
   Replay Once at each of 0.25×, 0.5×, 1×, 2× and 4×. Check relative speed and
   held/trailing durations; target recognition need not be identical at every speed.
2. Configure fixed count 3 and an interval of 2 seconds. Expect exactly three
   lines, a completed-run countdown between runs, and immediate completion after
   the third run's cleanup. Repeat at 0.5× and 2×; the interval remains 2 seconds.
3. Configure Indefinitely. Press physical F8 during a run, then in a separate
   attempt during the interval. Expect prompt stopping, no later run, released
   injected input, full-view restoration, preserved selection and target focus.
4. Record a drag/text-selection hold. Repeat it and stop partway through the hold;
   subsequent typing/mouse movement must not act as though input remains held.
5. During an interval, change the keyboard or display layout. Expect interruption
   without another run. Restore the environment before retrying. Actual computer
   sleep during an interval must also cancel with no burst after wake.
6. Check that Starting/Finishing feedback and progress updates preserve focus,
   the compact view persists between runs, and Record/Play/Configure/Delete remain
   unavailable until terminal cleanup completes.

Report commit/build, Windows and Notepad versions, display scaling, step,
expected/actual outcome and repeatability. Manual perception does not establish
the proposed 100 ms Stop target. No Notepad acceptance has been claimed yet.
