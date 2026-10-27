# ML-47 — Configure placement and display recovery validation

Spec: [ML-47](https://github.com/Dzahc/MacroLoom/issues/47) and
[owner-approved placement/recovery decisions](../specs/ML-47-configure-placement.md).

## Deterministic coverage

- Physical display selection follows fresh main-window bounds, chooses largest
  overlap, and resolves ties using the main frame's top-left corner, distance,
  then leftmost/topmost geometry. Tests include reversed enumeration, a corner
  on a minority third display, zero overlap, missing main bounds, and a primary
  fallback.
- Frame placement centers before clamping, uses signed negative coordinates,
  honors taskbar/top-edge work areas, shrinks only down to the decorated minimum,
  and fails visibly when that minimum cannot fit. Synthetic scaled physical
  frames cover negative origins without claiming physical monitor observations.
- A failing destination tries a reachable primary; failure on every destination
  returns an actionable error after bounded rediscovery rather than successful
  invisible placement.
- Native recovery Discard requires failed placement and no pending Save. Its
  atomic dismissal reservation blocks a concurrent new Save and keeps session
  and library-operation guards held until native destruction.
- Review caught a settled-but-unknown Save outcome bypassing the editor's uncertain
  dismissal restriction. Recovery now presents the authoritative saved/failed
  receipt in the main window and requires its exact observed attempt ID for
  Discard. Tests reject absent/stale receipts and a newer Save, and check that
  the transport carries the receipt actually presented to the recovering owner.
- Main-window transport tests cover pending/unknown status blocking Discard,
  retained recovery feedback, successful Retry without saving, ignored stale
  responses, polling cleanup, and reconnection after effect teardown.
- A live editor spanning adjacent healthy work areas is left in place. A coverage
  test distinguishes adjacent displays from offscreen regions or gaps; initial
  placement still fits the complete frame onto the one selected display.
- Existing Configure form and persistence tests remain applicable to validation,
  scrolling, retained strings, uncertain save outcomes, and Cancel/Save behavior.

## Real Windows/WebView check

Command (with the local Vite server running):

```powershell
cargo run --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --bin configure-placement-check
```

Passed on October 26, 2026, exit 0. This opt-in executable uses an isolated
fixture library under ignored `.quality-output/ml-47-native/`, starts no native
input hooks, and closes its own windows. Observed connected displays:

| Display   | Physical full bounds      | Usable work area          | Editor scaling |
| --------- | ------------------------- | ------------------------- | -------------- |
| Secondary | (3840, 0) to (5760, 1080) | (3840, 0) to (5760, 1032) | 100%           |
| Primary   | (0, 0) to (3840, 2160)    | (0, 0) to (3840, 2076)    | 175%           |

For each display, the check moved the main window, invoked the actual opening
command, observed Configure on that display with its full native frame inside a
usable work area, and verified the actual selected Name input. Moving the main
window afterward left the editor in place. It edited the actual React input,
moved that same editor offscreen, and sent a synchronous work-area notification
through the installed native observer. The editor became reachable with its
unsaved text intact. A separate disposable native window kept foreground focus
through recovery. Cancel from the actual form destroyed Configure and reenabled
the owner. Reopening on the next display followed the new main-window position.

The initial local-server isolation timeout and a test-only asynchronous
`WM_SETTINGCHANGE` failure were diagnosed and corrected. Windows requires
synchronous delivery for that notification; the product observer posts its own
pointer-free private message after native processing. A separate blank-WebView
focus fixture timed out; replacing it with a disposable native window let the
expanded focus check pass. WebView2 printed a shutdown class-unregistration
diagnostic after the successful run; the executable returned 0.

This is automated real-Windows integration evidence. It does not establish
physical hot-unplug behavior, a negative-coordinate hardware arrangement, a
100%/150% hardware arrangement, or assistive-technology announcements. Those
manual observations remain outstanding; no display settings were changed.

## Completion gate and review

Final completion command: `npm.cmd --prefix app run verify`.
Result on October 26, 2026: **PASS**, exit 0, all 14 checks passed. This includes
112 frontend/tooling tests, 147 Rust unit tests plus 22 repository tests, 33
Python quality fixtures, and 938 index/worktree function checks with no scores
above 10. Seven preexisting interactive native unit tests remain deliberately
ignored in the unattended gate; the ML-47 real-WebView check above ran separately.
The local gate log is `.quality-output/ML-47-verify.log`.

Additional opt-in static check:
`cargo clippy --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --all-targets -- -D warnings`
passed. The agreed review baseline was starting commit
`102ac55347fc43db3d4f4f947564beb3baee1c40` on the existing feature branch.
Independent Standards review: zero findings. Spec review: one P2 uncertain-save
dismissal finding, fixed with the displayed authoritative receipt and native
attempt check; follow-up found zero remaining specification findings. The final
completion gate ran after that fix. Verification is also recorded in ML-47.
