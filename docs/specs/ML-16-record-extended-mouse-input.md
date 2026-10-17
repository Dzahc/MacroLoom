# ML-16 — Record wheel input, double clicks, and drags

Source: [ML-16](https://github.com/Dzahc/MacroLoom/issues/16).
Reviewed decisions: October 15, 2026.

## Capture contract

- Store vertical wheel input as the exact signed native amount, cursor position,
  and observed time. Preserve partial-notch amounts. Horizontal scrolling and
  middle/extra buttons remain outside v1.
- A double click is two ordinary ordered press/release pairs with their observed
  positions and times. Add no synthetic double-click action.
- Store movement only while a recorded left/right button remains held. Track each
  button independently and record releases at their actual physical positions.
  Initial held inputs retain the PRD's zero-time snapshot ordering.
- Sample dense consecutive movement at approximately one retained position per
  8 ms, preserving the first and final observations at their original times.
  Preserve the endpoint before a pause. Flush the pending endpoint before any
  retained key, button, or wheel transition; equal-time ordering stays observed.
- Stop preserves the last observed endpoint and trailing duration, with no
  invented movement or release. End-held streams remain valid; playback owns
  cleanup at the last successfully replayed position.
- Pending movement counts against both storage limits before it enters the
  capture. Replacement refunds its previous byte charge. Rejection retains the
  previous valid prefix and uses the existing automatic stop/save path.

## MacroLoom interaction policy

Each supported button gesture keeps its press origin through its release.
Exclude complete control-origin gestures, including release outside the window.
An externally started drag retains its movement and release while crossing or
ending over MacroLoom. A separate control-origin button gesture does not end the
other button's external drag.

Include vertical wheel events positioned over MacroLoom while an external drag
is active. Without an external held button, wheel input over MacroLoom remains
excluded. Actual Record/Stop activation gestures and reserved shortcuts remain
excluded regardless of another held button.

For a right-button drag followed by a left click on Stop, retain the observed
approach movement and exclude the left Stop gesture. Stop does not retrospectively
erase the approach. Keyboard Stop avoids introducing an approach path.

These capture rules do not waive playback collision checks against the compact
window's current actual bounds. Move the toolbar away before replaying a captured
path through its former location. Collision failure must clean up owned holds.

## Evidence boundary

ML-16 establishes capture, inspected saved events, deterministic ordering,
coalescing, and storage-limit correctness. Saved-library playback and end-to-end
replay acceptance belong to ML-17. Native injection support for the new wheel
payload keeps the existing temporary playback adapter exhaustive; it does not
connect saved-library playback.

The ordinary Windows capture baseline is Explorer drag-and-drop, Notepad text
selection, a Settings slider, and standard window movement/resizing. Include
curved movement, a pause, modifier changes, a supported right-button drag, and
crossing the reference 175%/100% display boundary. Validate the sampling density
against these cases; tune the initial target if needed. Precision drawing remains
deferred, and substantial extra work requires revisiting release scope.

See [validation and reporting instructions](../validation/ML-16-record-extended-mouse-input.md).
