# ML-15 recording and automatic save

The approved recording boundary snapshots held keys and supported buttons at
`0 ms`, excluding controls. Initial modifiers precede other keys and then buttons,
with deterministic ordering inside each group. Subsequent repeats, releases and
equal-time events retain their observed order. Recording Stop never injects a
release or appends an invented release; playback releases its owned remaining
holds at the recorded end.

Nonempty captures commit automatically after Stop and after detected storage
limits, keyboard/display changes, sleep or capture interruption. Interrupted
saves explain why capture ended. The full view is restored before Saving status
and persistence. Empty captures create no file or success toast. Failed commits
retain the frozen capture for Retry/Discard and keep other operations excluded.
Retry preserves identity, timestamps, events and duration; collisions may change
a generated name or filename. Commits stage and sync bytes before a Windows
atomic move without replacement. Library insertion and selection require commit
success. Other applications' generated input is eligible; MacroLoom's tagged
injection and reserved controls are excluded.

## Automated evidence

- Session tests cover initial held keys/buttons, repeats, trailing duration,
  no synthetic end releases, empty recording, storage preflight, failed saves,
  explicit Discard, Retry identity, sleep/close, environment change, layout change
  before a key event, and the event-count limit.
- Format tests cover serialized-size accounting, millisecond tie ordering,
  native key metadata and preserved duration.
- Real filesystem tests cover idempotent commits, generated-name collisions,
  existing-file preservation, failure after staging and directory recovery.
- Native snapshot policy tests use a controlled held-state reader and perform
  no desktop input injection.
- Barrier-controlled regression coverage keeps recording B active when recording
  A's delayed environment failure returns after A has stopped. Environment failures
  revalidate the captured generation and recording phase under the state lock.
- Initial key ownership regression coverage retains a snapshotted Ctrl release
  before later typing, preserves repeats while MacroLoom is focused, keeps control
  activation gestures excluded and replaces ownership at the next snapshot.
- Startup focus-change regressions exclude control-origin Enter/Space presses,
  repeats and releases, while retaining valid initial modifiers and externally
  held Enter/Space. A new external press becomes recordable after the activation
  gesture's real release.
- Frontend tests cover selection after commit, stale/duplicate outcome delivery,
  timed accessible outcomes and persistent save recovery controls.
- Completion command: `npm.cmd --prefix app run verify`. Its final result is
  recorded on issue #15. Sandbox restrictions require escalation for Git metadata,
  Windows atomic moves and temporary Git repositories used by the quality suite.

## Remaining interactive evidence

Ordinary target-application recording, external focus preservation, live mixed-DPI
and negative-coordinate capture, actual keyboard-layout/display changes, and
physical held-input behavior require an interactive Windows desktop check. The
deterministic tests do not claim that manual evidence. Wheel capture and complete
drag sampling validation remain ML-16; saved-macro playback remains ML-17. The
existing temporary playback path retains end-of-run owned-input cleanup.
