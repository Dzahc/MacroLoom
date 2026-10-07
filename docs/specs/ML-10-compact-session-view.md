# ML-10 — Compact session view GUI

Reviewed with the owner on October 6, 2026. This scope supplements GitHub issue
[#10](https://github.com/Dzahc/MacroLoom/issues/10) and supersedes conflicting
defaults in the PRD for this story.

- Demonstrate recording, playback, repeat intervals, stopping, saving, and idle
  through direct development-state switches. Do not implement session actions or
  register native input hooks or shortcuts for these demonstrations.
- Shrink the same native main window to its frame, fixed-order icon toolbar, and
  readable two-line banner. Hide the library without unmounting it. Preserve its
  selected identity and scroll position.
- Compact width fits all toolbar buttons; height fits the toolbar and banner.
  Keep a consistent banner height across active states. Compact mode is movable,
  non-resizable, and topmost through recording, playback, intervals, and stopping.
  Saving and idle use the full window.
- Show a red recording indicator and elapsed time. Playback shows its available
  macro name, current/total runs (including indefinite repetition), and elapsed
  session time. Intervals show the completed run and next-run countdown; stopping
  has distinct text. Recording need not have a macro name. Truncate long names
  visually with full-name tooltips and accessible text.
- Remember compact position until the process exits, initially using the full
  window's upper-left corner. Keep the entire compact frame within the applicable
  display work area. Restore full-window size, position, and maximized state
  independently of compact movement.
- Automated transitions and progress updates preserve external target focus.
  Deliberate clicks or native title-bar dragging may activate MacroLoom normally.
- Hide ordinary toasts and preserve their remaining visible lifetime while
  compact. Resume them in full view. Window-transition failure toasts bypass this
  suppression and appear below the toolbar with Stop accessible.
- On failed compact entry, attempt rollback to the full view. On failed
  restoration, keep the actual visible view and its controls usable. Report
  transition failures through timed toasts only: no persistent error or Retry.
- Preserve typed backend preconditions. Production omits development controls;
  live session wiring belongs to later stories.

Validation separates deterministic presentation/toast/transition tests from
actual Windows bounds, topmost, focus, maximized restoration, and display-scaling
evidence. Unexercised native configurations must be identified explicitly.
