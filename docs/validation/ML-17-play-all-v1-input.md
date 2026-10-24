# ML-17 saved-library playback validation

Scope: [reviewed specification](../specs/ML-17-play-all-v1-input.md).

## Evidence boundaries

Deterministic scheduler tests use controlled monotonic time to establish initial
and trailing waits, equal-time ordering, repeated downs, processing overhead and
cancellation/failure behavior. Session tests use real isolated repository files
and substitute only desktop effects. They establish selected-ID validation,
snapshot stability, unchanged properties, startup interlocks, cancellation,
environment interruption and owned-input cleanup/retry. Frontend tests exercise
the ID-only command boundary and Enter/Space release activation.

These checks do not establish physical desktop injection, target focus, actual
sleep/display-power behavior, or mixed-DPI replay fidelity. The owner confirmed
ML-16's outstanding capture checks; that confirmation does not claim ML-17 replay.
Do not test or interact with Google Play Games.

## Ordinary Windows replay checklist

Use scratch files and windows. Keep target placement and display layout unchanged
from recording; place the compact toolbar away from the captured path. Launch
with `npm.cmd --prefix app run tauri -- dev`.

1. Record a Notepad field click followed by text, modifiers, a held/repeated key,
   initial and trailing waits. Stop/save, clear the scratch text, then replay with
   Play and separately with row double click. Confirm the intended instance gains
   focus and receives text while the compact toolbar stays topmost without taking
   focus. Check multiple Notepad instances.
2. Record/replay signed vertical wheel input, two-pair double clicks, Explorer
   scratch-file drag-and-drop, curved/paused/modifier-assisted text selection, a
   Settings slider, window move/resize and a supported right-button drag. Restore
   changed scratch settings and window placement before each repeat.
3. Replay a recorded physical 175%/100% display crossing under matching metadata.
   Check endpoint alignment and negative screen coordinates where available.
4. Stop with F8 during an initial wait, held key, active drag and trailing wait.
   Confirm no later ordinary input and no held-input residue. An interrupted drag
   releases at its last replayed position. Repeat with a recording ending held.
5. Overlap a later target with the toolbar. Playback has no upfront scan; it must
   stop before that colliding event and explain how to retry. Confirm cleanup.
6. Hold Shift or a mouse button while requesting playback: expect refusal, no
   injection and explicit retry guidance. Release, then deliberately retry. Enter
   and Space on Play should start once after release, without a countdown.
7. Change recorded speed/repetition settings: ML-17 still runs once at 1× without
   an extra limitation notice and leaves the settings unchanged.
8. Check screen-off continuation with an unchanged desktop separately from an
   actual monitor removal/layout change. Actual layout change and computer sleep
   interrupt; wake never bursts overdue events. Do not claim these checks from
   simulated environment observations.

Report build/commit, Windows/application versions, monitor origins/scales, macro
filename, step, expected/actual outcome and repeatability. Measure dispatch
lateness separately from capture resolution; subjective observation does not
establish the numerical Stop-response budget.

## Automated results

October 23, 2026, implementation commit `34db661`:

`npm.cmd --prefix app run verify` — **PASS**, exit 0, against refreshed
`origin/develop` at `37c945e`. Formatting, strict TypeScript, ESLint, the production
frontend build, rustfmt, Clippy, all maintained tests and complexity checks passed.
The test totals were 78 frontend/tooling tests, 89 Rust library tests plus 21
repository integration tests, and 33 Python fixtures. Two existing interactive
Windows tests remain ignored. Complexity checked 932 function/closure snapshots
across index/worktree, with zero failures and no exceptions.
Full local output: `app/.quality-output/ML-17-verify-final.log`.

`cargo check --manifest-path app/src-tauri/Cargo.toml --locked --offline --all-targets --features native-checks,input-prototype`
— **PASS**, exit 0. This establishes optional-feature build coverage without
injecting input or changing the owner's desktop.
Full local output: `app/.quality-output/ML-17-native-check.log`.

Ordinary-target replay observations remain pending until explicitly exercised and
reported. No Google Play Games testing or interaction occurred.

## Standards

Independent review against starting commit `37c945e`: no documented-standard
violations or actionable baseline smells found. Reviewed ownership, cancellation,
cleanup, validated-file loading, environment checks, typed command contracts,
keyboard activation and behavior-focused tests. Native input and timing remain
in Rust, and the existing runtime compact-window guard is preserved.

Standards findings: **0**.

## Spec

Independent review found no actionable code defects or scope additions. The
implementation follows selected-ID snapshot ownership, silent once-at-1× replay,
held-input rejection, release-based keyboard activation, one post-preparation
clock, original event ordering/waits, runtime collision checks, semantic environment
checks, sleep cancellation and recoverable owned-input cleanup.

Spec findings: **0 code defects**, **0 scope additions**, **1 documented validation
limitation**: ordinary-application/mixed-DPI replay and real display-power/sleep
observations remain pending. Deterministic tests do not establish that evidence.
