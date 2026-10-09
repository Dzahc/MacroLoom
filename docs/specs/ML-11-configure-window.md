# ML-11 Configure window

The accepted design comes from the issue #11 grilling session, confirmed before
implementation. This story delivers the editor and asynchronous draft callback.
Durable property storage and outcome toasts remain the later integration story.

- Configure opens one separate, resizable native window owned by the main window.
  The main window and conflicting operations remain unavailable while it opens
  or is active. Native operation guards also exclude deletion and prototype
  Record/Play; the global Record path uses that same native guard.
- The editor copies a validated selected snapshot into local fields. Names use
  Rust-compatible trimming, Unicode scalar counting, the 1–120-character limit,
  and control-character rejection. Speed choices are 0.25×, 0.5×, 1×, 2× and 4×.
- Repeat modes are Once, Fixed count and Indefinitely. Total runs is available
  only for Fixed count; the interval is available for Fixed count/Indefinitely.
  Switching modes preserves typed strings. A complete Save preserves valid
  inactive values and substitutes original snapshot values for invalid inactive
  strings. Active invalid fields block callback delivery.
- Counts accept positive decimal safe integers. Intervals accept ordinary
  nonnegative decimal seconds with at most three fractional digits. Exponents,
  signs and excess precision are rejected. Decimal-to-millisecond conversion is
  exact through the maximum safe integer; values are never silently rounded.
- Field validation appears on blur or a Save attempt. Save remains available
  to trigger validation; invalid submission targets the first invalid input.
  Initial focus belongs to the actual name input with its text selected.
- Enter submits from inputs; buttons retain their normal keyboard behavior.
  Tab/Shift+Tab stay within the form. Escape, Cancel and native close discard
  idle local edits. Closing restores the opening control when the main document
  is active. Animation-frame work and event subscriptions disconnect on cleanup.
- Valid Save emits the complete `{macroId, name, playback}` draft once per
  attempt, even without changes, through `ConfigureSave`. While awaiting the
  callback, fields and buttons are disabled, Escape is ignored and native close
  requests are blocked. Application exit is deferred. Success closes the editor
  and honors queued application exit; failure retains strings and actionable
  form/field errors, cancels deferred exit and permits deliberate Retry.
- The form scrolls independently of its persistent Save/Cancel footer. Labels,
  hints, errors, visible focus and progress status have explicit accessible
  associations. Styling uses the existing system-theme design tokens.

## Integration boundary

`LiveLibraryApp.onConfigureSave` supplies the async consumer. The native bridge
validates drafts independently, freezes the macro identity and correlates each
submission with a safe integer attempt ID. Only the editor may submit/close;
only the main window may acknowledge a matching callback. Unknown control
payload fields, changed IDs, invalid settings and duplicate acknowledgements
are rejected. No Configure command writes a file or changes library metadata.

Without a storage consumer the native host returns an actionable unavailable
failure and retains edits. It does not claim a successful save. The browser
route `/?configure` supplies controlled success/failure callbacks and logs
validated drafts while retaining the sample library unchanged.
