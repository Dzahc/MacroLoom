# ML-26 — Rename macros from the library

Issue: [#38](https://github.com/Dzahc/MacroLoom/issues/38). These acceptance criteria record the user's confirmed interview decisions.

- Right-click a macro and choose Rename, or press F2 with a macro selected. Select the entire original name. Single-click selection and double-click Play keep their existing behavior.
- Replace that row's name with an inline text field followed by boxed checkmark Save and X Cancel buttons. Hide duration while editing; errors appear beneath the field and may increase row height.
- Enter saves, Escape cancels, and Tab traverses field → Save → Cancel → normal document order. IME composition does not submit. Buttons have accessible names and visible focus.
- Clicking outside the editor within MacroLoom cancels an unsaved draft and performs the clicked action. Moving focus outside the editor through Tab follows the same rule. Switching applications retains the draft.
- Reuse Configure's Rust-compatible Unicode trimming, 1–120 Unicode scalar limit, and control-character prohibition. Validation appears after attempting Save and updates during subsequent edits. Duplicate display names remain allowed.
- Only confirmed backend saves update the library. Preserve ID, filename, creation time, recording, events, playback settings, and extension metadata. Preserve existing atomic-write, conflict, no-op, and outcome-feedback behavior.
- Ordinary save failures retain the editable draft and actionable error. Lost acknowledgements reconcile an authoritative receipt before any new write. Pending/unknown writes cannot be edited or cancelled; Retry checks unknown status without submitting again.
- Rename is unavailable during an active session. An accepted F9 Record or GUI Play cancels an unsaved rename without warnings and proceeds, including when later session preflight fails. No global Play shortcut or paused session is introduced.
- Once Save is submitted, Record/Play waits for its backend outcome and then executes automatically. At most one deferred start exists: first request wins, and Play retains its original macro ID.
- During that wait F8 and toolbar Stop are available. Stop immediately cancels the deferred start without cancelling the accepted disk write. If Save fails while a start still waits, discard the rename and prioritize that start using persisted data; suppress rename-failure warnings for that explicitly authorized discard.
- Closing discards an unsaved draft. Closing during Save cancels any deferred start, waits for Save, exits on success, and cancels exit on failure while retaining the draft and error. No deferred session starts after closing.
- Explicit Save/Cancel returns focus to the originating row only while the main window remains focused and idle. Outside-click dismissal and session starts preserve the destination's focus.

## Selected-row pencil entry point

The user confirmed this extension after the original implementation:

- Only the selected macro shows a pencil, and only while its row is hovered or has keyboard focus. The enabled pencil is a labelled button in the normal Tab order. Unselected rows never show a pencil.
- Keep the pencil in a fixed slot at the end of the name area, before duration. Reserve that width in every normal row even when the pencil is hidden, so hover and selection do not move text or durations.
- Clicking the pencil opens the existing editor with the full name selected. It does not change selection or trigger Play. Keep existing row selection, double-click playback, F2, context-menu Rename, and editor/session rules.
- Show a gray disabled pencil when loading, an active session, pending saves, confirmations, deletion, or unavailable writable storage prevents Rename. Disabled pencils cannot activate and are excluded from the Tab order.
- Explain a disabled pencil with a reason tooltip only when hovering directly over its icon area. Row hover and keyboard focus reveal the pencil but do not show that tooltip. Enabled pencils use the standard Rename icon tooltip on the same hover target.

## Test seams

The user approved public library/editor interactions, backend rename/session controls, and repository save behavior as test seams. The review base is commit `30a66737ac7dd9459aec43db12a51caeb055d84d`.
