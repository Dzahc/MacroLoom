# ML-16 live Windows capture evidence

Session date: October 15, 2026, America/Los_Angeles. Capture identities use UTC,
so these recordings are named October 16. Implementation under test: `e45118c`
(including implementation commit `8373885`).

## Reference environment and method

- Windows kernel `10.0.26200.0`, x64 desktop.
- Notepad `11.2607.14.0`, Explorer `10.0.26100.8117`, and Settings
  `10.0.26100.9549`.
- Primary display: physical rectangle `(0, 0, 3840, 2160)`, scale `1.75`.
  Secondary: `(3840, 0, 1920, 1080)`, scale `1.0`.
- Build command: `npm.cmd --prefix app run tauri -- build --debug --no-bundle`
  passed. Launch the resulting `app/src-tauri/target/debug/macroloom.exe`.
- Record/Stop toolbar controls started/stopped the automated desktop captures.
  Inputs were delivered through the Computer Use Windows desktop API into actual
  applications, rather than calling MacroLoom's conversion/capture helpers.
  The production hook/session/save path produced the inspected files.
- The desktop API generates single-jump left drags. These observations establish
  actual application effects, hook delivery, and endpoint coordinates. They do
  **not** establish high-rate physical-mouse sampling, curved/paused drags,
  modifier transitions during holds, right drags, or mid-drag Stop.
- Evidence JSON copies preserve event values and identities; whitespace may be
  formatted for review. Original captures remain beside the debug executable.
  Existing user recordings and documents were not edited.

## Automated desktop observations

### Notepad selection, wheel, and double click — pass for the exercised gestures

Evidence: [saved recording](evidence/ML-16/notepad-selection-wheel-double-click.json),
ID `d15fbdd0-68fb-4188-aa6f-bbedb9760be1`, duration `53396 ms`, nine events.

The scratch `selection.txt` contained 80 numbered lines. Dragging selected the
first six lines (384 characters); a subsequent double click selected `select`.
Scrolling down/up visibly changed/restored the text position.

- Selection: left press at `14609 ms`, `(1013, 427)`; movement at `14611 ms`,
  `(1771, 579)`; release at `14629 ms`, `(1771, 579)`.
- Wheel: `-600` at `25359 ms` and `+600` at `29875 ms`, both at `(2121, 693)`.
- Double click: down/up/down/up at `41076`, `41093`, `41225`, and `41242 ms`,
  all at `(1267, 427)`, with no synthetic action.
- The window's observed physical origin was `(984, 256)` on the 175% display.
  Requested logical drag offsets `(17, 98)` to `(450, 185)` map to the retained
  physical pixels within one pixel using `origin + offset * 1.75`.
  Wheel offsets `(650, 250)` likewise map to `(2121, 693)`.
- Moving freely between completed gestures added no path events. Stop's control
  click is absent. Trailing duration remains independent of the final event.

### Explorer drag-and-drop — pass for the exercised left drag

Evidence: [saved recording](evidence/ML-16/explorer-drag-and-drop.json),
ID `5a057226-681d-4959-a698-45164c036d65`, duration `29730 ms`, three events.

Dragging the disposable `drag-source.txt` onto the `drop-target` folder moved the
file: it disappeared from the parent listing, was present inside the destination,
and its contents remained the scratch fixture.

- Left press: `16558 ms`, `(1129, 832)`.
- Movement: `16560 ms`, `(1138, 781)`.
- Release: `16578 ms`, `(1138, 781)`.
- Explorer's observed physical origin `(622, 465)` and logical offsets
  `(290, 210)` to `(295, 181)` map to those pixels within one pixel at 175%.

### Settings slider — pass for the exercised preview drags

Evidence: [saved recording](evidence/ML-16/settings-text-size-slider.json),
ID `9765002a-0897-42fb-8e74-99f6cd891384`, duration `30398 ms`, six events.

In Accessibility → Text size, dragging the preview slider changed its preview
from the original value to 191%; the return drag restored the original value and
disabled Apply. Apply was never activated; no system text-size change was made.

- Outward press/move/release: `10876`, `10877`, `10894 ms`, from `(5369, 482)`
  to `(5457, 482)`, released at the destination.
- Return press/move/release: `18043`, `18044`, `18063 ms`, from `(5457, 482)`
  to `(5369, 482)`.
- The Settings window's observed physical origin was `(5057, 83)` on the 100%
  display. Logical offsets `(312, 399)` and `(400, 399)` map exactly to the
  retained physical pixels.

### Window movement and resizing — pass for the exercised gestures

Evidence: [saved recording](evidence/ML-16/window-move-and-resize.json),
ID `71defe16-d701-4688-b6c8-8f7c204732ab`, duration `203614 ms`, 21 events.

- A title-bar drag at `11078`/`11079`/`11096 ms` moved the scratch Notepad window
  from physical origin `(984, 256)` to `(1089, 344)`. Recorded pointer positions
  `(2453, 280)` to `(2558, 368)` have the same `(105, 88)` displacement.
- An initial corner attempt at `18428`–`18448 ms` did not change the window bounds;
  its observed input remains in the capture and is not claimed as a resize pass.
- The right-edge drag at `34844`/`34846`/`34863 ms`, `(2994, 1044)` to
  `(2838, 1044)`, reduced observed logical width from 1090 to 1001, preserving
  the actual release location.
- Four subsequent title-bar drags moved the window toward the display boundary.
  The final recorded release was `(3711, 368)`, still on the primary display.
  The desktop API rejected endpoints outside the cropped visible window. This
  capture therefore does **not** establish a cross-monitor drag.

## Owner-operated physical capture

Evidence: [saved recording](evidence/ML-16/physical-selection-mixed-dpi.json),
ID `2d89e675-68a1-48ab-a50f-a762afb3f055`, duration `35821 ms`, 399 events.
This capture contains 353 retained movement points, three left-button pairs,
39 observed Shift presses/repeats, and one Shift release.

The owner performed the guided curved/paused Shift-assisted selection,
cross-monitor window move/resize, and attempted F8 during a final drag, then
reported: “Done—all three worked.” Desktop automation did not issue input while
the owner performed these gestures. Inspection establishes:

- The first held gesture runs from `2283` to `15333 ms` with 163 retained points.
  Its position `(1187, 596)` at `3401 ms` remains before a `1605 ms` pause; the
  next point is `(1186, 596)` at `5006 ms`. A further pause lasts `3480 ms` before
  `(1187, 596)` at `8486 ms`. The pause endpoints have not been erased.
- Shift-down at `14317 ms` and its observed repeats occur in stream order while
  the first drag remains held; Shift-up at `16069 ms` follows its mouse release.
  This does not claim both modifier transitions occurred before mouse release.
- The cross-monitor gesture runs from `(2230, 298)` at `23238 ms` to
  `(5048, 147)` at `25781 ms`, with 137 retained points. Around the physical
  `x = 3840` boundary, points are `(3821, 58)` at `24198 ms` and `(3843, 62)` at
  `24206 ms`. The eight-millisecond step crosses the 175%/100% boundary without
  a logical-coordinate scaling jump. The owner reported the window move/resize
  worked visually.
- The final gesture has 53 retained points. Its real mouse release is at
  `35734 ms`, `(4643, 457)`, which is **87 ms before** the recording end. Despite
  the reported attempt, this file does not demonstrate a button held at Stop.

The observed selection and cross-monitor behavior support sampling suitability
for these exercised physical gestures. They do not establish replay fidelity or
precision drawing support.

## Remaining physical-input evidence

Physical right-button drags, toolbar crossing with held-wheel input, and a left
Stop click while an external right drag remains held still require a recorded
physical pass. A recording that actually ends with a held button also remains
unverified. Computer Use was stopped by the owner's Escape key before these
additional cases could be run. No further desktop input was issued afterward.

P2 remains open for these cases; the five saved captures are partial acceptance
evidence, not a claim that the entire interactive checklist is complete.

Saved-library replay remains ML-17 and was not exercised.
