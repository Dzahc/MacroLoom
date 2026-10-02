# ML-9 — Main library window GUI

## Reviewed scope

The October 1, 2026 grilling session settled the following changes to the original
issue: icon-only toolbar with tooltips and accessible names; no dedicated GUI
keyboard-navigation work; fixed F9/F8 assignments for later live integration;
failed macros omitted with logging-only load feedback; no persistent operation
error/recovery demonstrations. The user's final scope confirmation preceded implementation.

Subsequent UI review removed the sample-data label from the main window. The
development pane retains its explanation of controlled sample scenarios.

The sample library is the default application. A separate development pane
controls scenarios and operation toasts and displays callback/failure logs.
Sample actions do not simulate recording, playback, saving, configuration, or
deletion. Configure/Delete dialogs, native compact transitions, real loading,
recovery, and global shortcut integration remain in subsequent stories.

## Implementation boundary

- `LibraryView` consumes a typed snapshot and separate selection/action callbacks.
  Requests identify stable macro IDs and their toolbar, row, context-menu, or
  double-click source. Unavailable requests are rejected by the UI contract.
- The sample adapter starts without selection, retains it by ID, and clears it
  when that ID disappears. Right-click selects before opening Configure/Delete;
  double-click emits one Play request for the clicked ID.
- Operation toasts are serialized in arrival order with four seconds of visible
  time, explicit dismissal, and accessible announcements. Hover, focus, and
  document invisibility or a partially/fully offscreen toast pause their remaining lifetime. Timer/listener cleanup
  runs on unmount. Toasts occupy space below the banner and cannot cover Stop.
- The backend exposes typed `app_mode` metadata. Only the explicit debug
  `input-prototype` feature starts live hooks and F9/F8 registration; live commands
  reject calls in normal library mode. Release builds cannot enable the prototype.
- The default native window is 600 × 480 logical pixels and resizable, with a
  360 × 230 minimum. No new native window-transition behavior is introduced.

## Recorded checks

October 1, 2026, Windows development host, Codex in-app browser against the local
Vite server. These are presentation checks, not native input or storage evidence.

- Initial state: no selected row; Record enabled; Play/Stop/Configure/Delete disabled.
- Selecting a row enables Play/Configure/Delete while Stop remains disabled.
- Play emits the selected ID without changing the supplied sample state.
- Right-clicking a different row selects it and exposes Configure/Delete.
  Configure emits that row's ID and closes the menu.
- Double-clicking a row emits one Play request with the double-click source.
  Ordinary click selection notifications also occur, following browser event order.
- Empty scenario shows the prescribed empty text and clears selection. Returning
  to populated data preserves no stale selection.
- Load-failure scenario retains valid rows, adds no failed row or product error,
  and writes the sample failure to the development log and console.
- Saving scenario disables the toolbar and row actions and displays Saving status.
- Success-toast scenario announces text, provides a labeled dismiss button,
  preserves focus on its initiating development control, and leaves the toolbar visible.
- At 360 × 480 and 600 × 480 CSS-pixel viewports, the library has no horizontal
  overflow; action buttons measure 44 CSS pixels at default text size. Long names
  truncate while their full text remains in accessible content and the tooltip.
- Light styling was visually inspected. System dark, reduced-motion, and forced-color
  rules are present; actual Windows theme/text-scaling and screen-reader checks are
  recorded separately from browser evidence and must not be inferred from it.

Automated coverage includes toolbar gating in every supplied phase, stale/explicit
macro ID requests, selection reconciliation, duration boundaries, rendered toolbar
order/names/tooltips/selection, empty/load-failure examples, and deterministic toast
queue lifetime, overlapping pause reasons, dismissal, duplication, and cleanup.
Native tests cover live-command mode gating alongside the existing prototype tests.

## Completion gate

The required command is `npm.cmd --prefix app run verify`. The final result is
recorded in GitHub issue #9 after the checks complete.

## PR review follow-up — October 2, 2026

- Added JSDoc for the ML-9 components, internal helpers, callbacks, and queue
  lifetime/subscription methods, plus Rust documentation for mode metadata and
  command preconditions.
- Record and Stop use the same F9/F8-aware text for accessible names and tooltips.
- Toast presentation observes full intersection with the viewport and scrolling
  ancestors. Expiration stays paused until visibility is confirmed, pauses when
  clipped/offscreen, and resumes the remaining time on re-entry. Replacing or
  unmounting a toast disconnects the observer and document listener.
- Focused regressions failed before the fixes: shortcut names omitted F9/F8,
  and document-only visibility allowed an offscreen toast to expire unseen.
  Deterministic tests cover initial offscreen state, scrolling out/back in,
  overlapping pause reasons, successor dismissal, host observer thresholds, and
  subscription cleanup.
- Browser automation exposed no available browser in this session. The reported
  600 × 480 interaction was therefore checked through controlled intersection
  reports and the real browser adapter, rather than a new visual walkthrough.

`npm.cmd --prefix app run verify` **PASS** on October 2, 2026: all formatting,
static analysis, production build, and pinned-environment checks passed; 21
frontend/tooling tests, 7 Rust tests, and 33 Python quality fixtures passed.
Complexity checked 369 functions across index/worktree against refreshed
`origin/develop`, with limit 10 and zero failures or exceptions.
