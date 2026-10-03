# ML-12 Delete confirmation GUI

## Accepted behavior

The dialog is an in-app HTML modal with a named heading and warning, an inert
background, and initial visible focus on Cancel. It follows the system theme.
The complete macro name wraps, and the dialog scrolls within the viewport.
Cancel, Escape, and the named Close button dismiss without confirmation; backdrop
clicks leave it open. Enter and Space activate the focused button. Repeated
activation keydowns are suppressed in the dialog and library, including after
focus returns to the toolbar.

Each attempt freezes the requested macro ID and display name. Confirmation
finishes the attempt before emitting that ID once and immediately dismisses the
dialog. Reopening creates an independent attempt. No file or sample-library data
is deleted, and no deletion-success toast is emitted. Consumer exceptions and
promise rejections are handled and logged; the future deletion consumer owns
disk validation, progress, and outcome feedback.

Known removal, rename, loss of write availability, or departure from idle cancels
the attempt. Selection changes and row reordering do not retarget it. No filesystem
watcher is added. The owner restores idle focus to the toolbar Delete button or
the originating context-menu row, falling back to an available row or the library
section. A background document or active session suppresses explicit restoration.

## Integration boundary

`LibraryView.onDeleteConfirmed` supplies the stable ID separately from ordinary
action requests. `LibraryApp` and `LiveLibraryApp` expose an optional consumer for
later deletion integration; the default native host performs no deletion.
Browser examples log confirmation and cancellation and preserve their rows.

`LibrarySnapshot.confirmationOpen` excludes idle action requests while the dialog
is open. Native global shortcuts do not run in the current default library mode.
Future shortcut/session integration must enforce this exclusion independently
in the backend; presentation availability is not a backend precondition.

## Automated validation

The Node tests at the public confirmation boundary cover stable-ID emission,
duplicate/reentrant activation, consumer failure, cancellation, reopening,
selection/reordering, and known invalidation. Request-boundary tests cover modal
exclusion while retaining active-session Stop. Existing static library tests remain
part of the maintained suite.

Completion command: `npm.cmd --prefix app run verify` from the repository root.
Result on October 3, 2026: **PASS**, exit code 0. All 14 gate checks passed,
including the refreshed-origin comparison, formatting, TypeScript, ESLint,
production frontend build, Clippy, 31 frontend/tooling tests, 19 Rust tests,
33 Python quality tests, and 126 changed-file function checks at complexity ≤10.
The command/result is also recorded in the ML-12 issue.

## Interactive validation status

On October 3, 2026, the connected browser inventory exposed no browser. The
Windows computer-use attempt stopped because it could not determine the browser's
current URL with enough confidence to enforce policy. No interactive accessibility,
focus, keyboard, layout, or WebView2 check is claimed as passed.

Use the browser preview and development callback log to check:

1. Open through toolbar and context menu; verify the complete selected name and
   permanent-file warning, accessible dialog name/description, and focus on Cancel.
2. Verify Tab and Shift+Tab remain in the dialog and focus indicators remain visible.
3. Verify initial Enter and Space cancel. Reopen, focus Delete, and activate it;
   exactly one `delete-confirmed` entry names the correct stable ID.
4. Verify Cancel, Escape, and Close log cancellation, while backdrop clicks leave
   the dialog open and background Record/Play remain unavailable.
5. Reopen repeatedly and double-click Delete; sample rows remain unchanged and
   each attempt reports at most one confirmation. Hold Enter to check repeats.
6. Verify toolbar/context launches restore their respective controls. Check the
   long-name sample at narrow sizes, scalable text, and system light/dark themes.
7. In an integration host supplying changing snapshots, remove/rename the target
   or leave idle while open; verify cancellation, no confirmation, and no explicit
   focus restoration during an active session. Repeat in the Tauri WebView2 host.
