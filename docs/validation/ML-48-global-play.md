# ML-48 — Global Play validation

Scope: [issue #48](https://github.com/Dzahc/MacroLoom/issues/48) and the
[owner-confirmed specification](../specs/ML-48-global-play.md).

## Automated evidence

Deterministic tests use the public session commands/snapshots, native shortcut
sequence policy, and library/editor/controller interactions. Desktop effects are
substituted; repository-backed playback retains real file validation. Coverage
includes release-time selection, selecting while held, selection retained during
rename Save, Stop cancellation, intervening Configure, physical-held startup
rejection, no-selection toast deduplication, busy exclusion, conflicts/retry,
recording-save selection, cleanup reservations, and truthful tooltip/banner
rendering. Reserved, modified, unreserved, repeated, and trailing-release key
sequences are checked independently, including controls held across hook startup.

The real-WebView `session-control-check` additionally exercises competing Windows
F9/F10/F8 registrations, visible conflict/retry and GUI fallback, F10 recording
exclusion, no-selection toast, selected-macro Play/F8 cancellation during an
initial wait, and real tagged F9/F10 release delivery while owned windows are
focused, and with an isolated external WinForms process focused. The external
process has an empty window and hidden PowerShell host; it is created and
terminated by this check, without reusing a user application. Playback/Stop
fixtures also run across that process boundary and verify retained focus.
Tagged releases test the OS hook delivery path without activating
controls. Non-injected callback fixtures separately exercise activation; they
are not physical-keyboard evidence.

October 25, 2026:

- `cargo run --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --bin session-control-check`
  — **PASS**, exit 0. Real F9/F10/F8 conflicts, GUI fallback, recording exclusion,
  no-selection toast, Play/Stop, owned/external OS key delivery, focus
  preservation, retry, and shutdown passed. Log:
  `app/.quality-output/ML-48-native-controls.log`. WebView2 emitted a class
  unregistration diagnostic after successful fixture assertions and normal exit.
- `npm.cmd --prefix app run verify` — **PASS**, exit 0 against refreshed
  `origin/develop` at `7a013f3`. Formatting, strict TypeScript, ESLint, production
  build, rustfmt, Clippy, 108 frontend/tooling tests, 158 Rust tests (seven
  interactive tests ignored), 33 Python fixtures, and 1,830 function/closure
  complexity checks across index/worktree passed with no exceptions. Final log:
  `app/.quality-output/ML-48-verify-final.log`. This rerun covers the reviewed
  startup-held gesture fix at `d13b927`.
- `cargo clippy --manifest-path app/src-tauri/Cargo.toml --locked --offline --features native-checks --all-targets -- -D warnings`
  also passed, covering the opt-in native fixture code.

Physical-keyboard acceptance in Notepad/the intended target and numerical Stop
timing remain pending owner observations.

## Standards

Review against `7a013f3` through `d13b927`: no remaining actionable findings.
Named constants now cover native-check process arguments/errors and the render
fixture status. The grouped F9/F10 reservation parameter is named `start_keys`.
The follow-up review found no additional baseline smells.

## Spec

Review against the confirmed ML-48 specification: no remaining findings.
The startup-held F10 finding is resolved: physical controls already held when
hooks start are seeded as passed before registration/pumping. Their repeats and
release cannot activate Play. A fresh press can. A new regression covers this
case. The clarified distinction between an already-pending rename Save and a
newly submitted Save during a held gesture also has regression coverage.

Remaining review findings: Standards **0**, Spec **0**.

## Owner target-application checklist

Launch the normal library build with
`npm.cmd --prefix app run tauri -- dev`. Keep the target at its recorded position
and size; include a focus click before recorded typing. Use a harmless Notepad
recording first.

1. Select a saved macro. With MacroLoom focused, press and hold F10: no playback
   starts until release. Release once: one session starts with saved playback
   settings. F8 stops during the initial wait, key hold, drag, and repeat interval.
2. Repeat with Notepad focused. Compact entry, status updates, and full restoration
   preserve target focus. Repeat in the intended target application; do not infer
   its behavior from Notepad.
3. Hold F10 over selection A, select B, and release: B plays. Start with no
   selection, select B while held, and release: B plays. Releasing with nothing
   selected shows exactly `Select a macro to play` as a timed toast.
4. While F10 is held, press F8, start Record, or open/close Configure. Releasing
   afterward must not start playback. A fresh F10 press is required. Holding or
   auto-repeat never restarts a completed/stopped macro or queues another run.
5. Ctrl/Alt/Shift/Win+F10 remains ordinary input. Releasing the modifier before
   F10 does not turn the gesture into Play. Reserved bare F10 during recording
   is consumed, ignored, and absent from the saved events.
6. Start an unsaved inline rename and use F10: discard the draft and play. During
   submitted rename Save, accept one deferred Play; changing selection afterward
   does not change its ID. F8 cancels Play while the disk write finishes. Verify
   retained failure/recovery behavior if Save fails after cancellation.
7. Close MacroLoom, reserve F10 in a second terminal using
   `cargo run --manifest-path app/src-tauri/Cargo.toml --locked --offline --bin shortcut-conflict-check -- play`,
   then relaunch. Expect F10 conflict feedback, no idle Play hint, and usable
   toolbar Play when F8 is free. Release the helper with Enter and choose Retry
   shortcuts. Repeat with combined conflicts; F8 feedback takes priority.

Record build/commit, target/version, Windows version, display scaling, expected
and actual behavior. Physical-keyboard/external-target acceptance and numerical
Stop timing remain separate from deterministic and native-fixture results.
