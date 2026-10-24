# ML-18 — Safe playback targeting

Source: [ML-18](https://github.com/Dzahc/MacroLoom/issues/18).
Owner-reviewed decisions: October 23, 2026.

Retain ML-17's full-set display geometry/scale and keyboard-layout compatibility
checks before dispatch and during playback. Enumeration order and extension
metadata do not affect compatibility. Actual environment changes interrupt the
session; screen power alone with unchanged desktop geometry does not.

Before any input dispatch, check every positioned mouse event against the union
of its recording's physical monitor surfaces. Include presses, releases, retained
drag movement and wheel targets. Negative pixels are valid; monitor gaps and
points outside every recorded monitor are not. Reject playback with an actionable
event/coordinate explanation, preserving the structurally valid library entry and
saved file. Never clamp, skip or remap an invalid destination.

Keep collision checks immediately before each mouse action against the compact
window's current actual physical bounds. Do not scan future compact collisions,
reserve the hidden library's former bounds, or reposition the toolbar
automatically. Collision failure prevents that action, cancels subsequent input,
cleans session-owned holds at the current position, and restores the library after
cleanup. The user moves MacroLoom and deliberately starts playback again.

Manual compact title-bar dragging preserves the external target's keyboard focus
during both recording and playback. This supersedes ML-10's allowance for native
title-bar dragging to activate MacroLoom. Ordinary toolbar controls retain their
normal interaction. Moving the toolbar does not stop, pause or reschedule playback;
shared-cursor interference remains possible. Control-origin recording gestures
remain excluded. Remember compact position until process exit and restore the
independent full-window placement as specified by ML-10.

Keep signed physical-pixel conversion in the Windows adapter, with compatible
capture/replay DPI handling and virtual-desktop injection. No target-window
selection, automatic focus restoration, coordinate remapping or pause/resume is
introduced. Preserve cancellation, held-input ownership and recoverable cleanup.

Validate through the saved-library session boundary and the native compact-window
adapter. Separate deterministic checks from native Windows observations and
ordinary-application evidence. Record physical targeting at 100%, 125% and 150%,
negative-coordinate monitors, layout changes, moved-toolbar collisions, hidden
library bounds, and manual title-bar focus preservation. Unexercised environments
remain explicitly pending. Do not test or interact with Google Play Games.
