# ML-47 — Configure placement and display recovery

Source: [ML-47](https://github.com/Dzahc/MacroLoom/issues/47), with owner decisions
confirmed in the implementation request after the design interview on October 26, 2026.

## Opening

- Recompute the destination on every opening from the main window's physical outer
  bounds. Choose the display with the largest intersection with those bounds.
- Among equal-overlap displays, prefer the one containing the main frame's top-left
  corner. Otherwise prefer the tied display nearest that corner; remaining ties use
  leftmost, then topmost geometry ordering. Enumeration order must not affect placement.
- If the main window has no overlap with any display, use its nearest display. If
  main-window bounds are unavailable, use the primary display.
- Center the decorated editor over the main frame, then constrain its entire frame
  to the destination's usable work area. Use signed physical pixels and the
  destination's DPI when converting logical content sizes and native decorations.
- Start with the existing 480×620 logical content size on each opening. Reduce it
  only as needed, preserving the existing 360×300 minimum, scrolling fields, and
  reachable Save/Cancel footer. Remembering user resizing is outside this change.
- If selection, sizing, or placement fails, try the main window's nearest available
  display, then the primary display. Refresh stale monitor information during an
  opening race. If no verified visible placement succeeds, fail opening with an
  actionable message, release the editor reservation, and leave the main enabled.

## Existing editor

- Moving the main window does not move an already-open editor. A user may move
  Configure independently. Opening the next editor reevaluates the main window.
- Display/work-area/DPI changes that leave Configure inaccessible recover the same
  editor onto a reachable remaining display. Preserve typed strings, validation,
  pending saves, and authoritative save receipts; never recreate its document.
- If recovery cannot establish visibility, retain the editor and draft, restore
  main-window access, and show persistent Retry and explicit Discard controls.
  Other operations remain blocked by backend ownership. Discard is unavailable
  while Save is pending or its outcome is uncertain; Retry never resubmits Save.
  Recovery status presents the authoritative Save receipt before permitting
  Discard, which must identify that observed attempt. Missing/stale receipts or
  a newer Save reject dismissal without changing the draft.
- Successful recovery restores normal editor ownership. Recovery must not activate
  a window while another application has focus. Preserve initial name selection,
  ordinary Save/Cancel/Escape/close behavior, and the one-editor restriction.

## Validation boundaries

Separate deterministic placement and recovery-policy tests from real Windows
window integration and manual observations. Cover primary/secondary placement,
ties, zero overlap, negative origins, mixed scaling, taskbar work areas, minimum
size failure, placement failures, display removal, retained edits, and pending-save
guards. Record only observations actually performed; do not infer hardware coverage
from synthetic layouts. Run the repository completion gate and record its result
in the issue or PR before declaring implementation complete.
