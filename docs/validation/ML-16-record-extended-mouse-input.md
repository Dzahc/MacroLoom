# ML-16 extended mouse recording validation

The [reviewed specification](../specs/ML-16-record-extended-mouse-input.md) captures
the owner's decisions. ML-16 validates capture and saved data; ML-17 validates
saved-library replay and final held-input cleanup in target applications.

## Automated evidence

The capture/format, native adapter, and session/repository regressions check:

- Signed vertical wheel high-word conversion, partial-notch amounts, physical
  positions, horizontal-wheel exclusion, and native injection payload encoding.
- Exactly two normal button pairs for a double click, with observed timing and
  no free movement between completed gestures.
- First/final dense movement, the source sampling interval, endpoint retention
  before a pause, modifier ordering, equal-time key/wheel ordering, independent
  left/right holds, and release at its actual physical position.
- External drags crossing/releasing over MacroLoom, wheel inclusion during those
  drags, full exclusion of a separate control-origin gesture, initial held-button
  origin handoff, and stale ownership removal at the next recording boundary.
- Storage-cap replacement at the event limit, refunded serialized bytes, rejected
  growth preserving the previous valid endpoint, and session Stop committing wheel
  input and an end-held drag through the real repository.

Native adapter fixtures use controlled Windows messages and held-state readers;
they do not inject desktop input. They do not establish ordinary-application or
live mixed-DPI evidence.

Completion gate: `npm.cmd --prefix app run verify` **PASS**, October 15, 2026.
All gate checks passed: refreshed origin, pinned environments, formatting,
TypeScript, ESLint, production frontend build, 75 frontend/tooling tests,
77 Rust library tests and 21 repository integration tests, 33 Python fixtures,
and 205 complexity checks with zero failures and no exceptions. Two existing
interactive Windows tests remain ignored by the maintained suite.

Optional build coverage also passed:
`cargo check --manifest-path app/src-tauri/Cargo.toml --locked --offline --all-targets --features native-checks,input-prototype`.

## Standards

Implementation review found no documented-standard violations or baseline smells
warranting changes. Mouse-origin locks are released before session state is
acquired; provisional movement is budgeted before mutation. Added helpers and
unsafe reads are documented, and behavior tests reuse source constants.

## Spec

Implementation review found no code defects or scope creep. One acceptance item
remains incomplete: ordinary-application capture evidence, including the live
mixed-DPI baseline. The checklist below is pending, so the automated results do
not establish those cases. Saved-library replay remains ML-17.

Review totals: Standards **0 findings**; Spec **1 pending validation item**,
**0 implementation defects**. Issue #16 remains open for interactive evidence.

## Interactive Windows capture checklist

Status: **pending interactive evidence**. No target-application pass is claimed.
Use scratch files/windows and stop with F8 unless a step specifically tests the
toolbar. Inspect each resulting JSON file; do not use the current saved-library
Play control as evidence of replay. Keep original captures for ML-17 replay checks.

| Case                   | Actions                                                                                                                                            | Expected saved evidence                                                                                                                                         |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Wheel and double click | In an ordinary scrollable window, record upward/downward scrolling and a double click; stop. Use a high-resolution wheel when available.           | Signed `mouse_wheel` amounts, position/time for each event, and two ordinary button pairs without an extra action.                                              |
| Explorer drag-and-drop | Drag a scratch file between scratch folders. Repeat with a curved route, a pause, a modifier change, and a right-button drag where supported.      | First/final movement points, position before the pause, key transitions in order, independent button holds, and release at the actual drop pixel.               |
| Notepad selection      | Drag to select scratch text; pause, change direction, then release. Move freely after release.                                                     | Timed held movement and actual release; no path events from the subsequent free movement.                                                                       |
| Settings slider        | Record changing a slider, including a pause before releasing. Restore the setting afterward.                                                       | The paused endpoint and release location remain present; dense movement is sampled rather than recorded at device rate.                                         |
| Window move/resize     | Move and resize a scratch window while recording.                                                                                                  | Left-button holds, timed movement, and actual release pixels.                                                                                                   |
| Mixed DPI              | Under unchanged display metadata, drag a scratch window across the 175%/100% boundary; include a negative-coordinate monitor if available.         | Signed physical pixels without logical-coordinate scaling jumps; recorded display metadata matches the setup.                                                   |
| Toolbar crossing       | Start a drag outside MacroLoom, cross it, scroll while held over it, and release over it. Then move freely and scroll over MacroLoom with no drag. | External drag movement/wheel/release remains; no later free path or idle wheel over MacroLoom.                                                                  |
| Stop during drag       | Stop a held drag with F8. Separately hold an external right drag, move toward the toolbar, and left-click Stop.                                    | Final observed drag endpoint and total duration; no synthetic release or cursor warp. Second recording retains the approach but excludes the left Stop gesture. |

Report each case as: Windows/app versions; display origins/scales; gesture and Stop
method; capture filename; inspected timestamps/positions/deltas; pass/fail and
observed difference. Recording resolution and target behavior are separate from
playback timing. ML-17 must additionally verify replay, collision rejection against
the toolbar's current bounds, and cleanup at the last successfully replayed point.
