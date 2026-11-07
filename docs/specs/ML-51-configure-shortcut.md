# ML-51 — Selected-row Configure shortcut

Issue: [#51](https://github.com/Dzahc/MacroLoom/issues/51). These acceptance criteria
record the owner's confirmed interview decisions.

- Add the existing Configure icon immediately after the pencil and before duration.
  Show both icons only for the selected macro while its row is hovered or has
  keyboard focus, using the pencil's existing reveal behavior.
- Reserve both icon slots in every normal row, including unselected rows. Preserve
  name truncation and duration alignment as selection and hover change.
- Open the same Configure window through the existing guarded action and native
  lifecycle. Use Configure toolbar availability, including allowing property
  inspection with read-only storage. Disable the shortcut during excluded work.
- Use a labelled button with an icon tooltip and normal Tab order when enabled.
  Activating it must not change selection or trigger double-click playback.
- Hide both icons while that row is replaced by the inline rename editor. Preserve
  its existing dismissal and save behavior.
- After Configure dismissal, return focus to the originating Configure icon when
  its macro remains selected and its control is available. Preserve the foreground
  focus safeguard and the existing toolbar/context-menu restoration behavior.
  A skipped restoration must not replay after later state changes.

## Validation

Use the previously approved public library/editor interaction seam. Cover rendered
selection, icon order and reserved slots, rename exclusion, toolbar availability,
and real browser activation/focus with only native IPC and the OS foreground
boundary simulated. Browser fixtures create no native editor, saved files, or
desktop input; they do not establish physical Windows focus behavior.

Review base: `faabd5b6e6b2b0677d866aa52a219053303ef917` (the starting commit).
