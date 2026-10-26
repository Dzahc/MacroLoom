# ML-48 — Global Play of the selected macro

Issue: [#48](https://github.com/Dzahc/MacroLoom/issues/48). The owner confirmed
these interview decisions on October 25, 2026 and authorized implementation.

- Bare F10 starts Play on release. Capture the selected saved macro ID at that
  release, not at key-down or worker execution. No selection at key-down does
  not prevent selecting a macro before release. Freeze the accepted ID through
  any rename-save wait; never substitute another macro.
- Reserve F10 with F9: idle (including no selection), recording, and pending
  rename Save. Release it for playback, cleanup, recording saving/recovery,
  loading, Configure, deletion, and exit. Reservation failures leave ordinary
  input available to other applications.
- Decide gesture ownership on the first physical down. A reserved bare F10's
  complete down/repeat/up sequence is consumed, including after unregistration,
  and excluded from capture. Modified and unreserved gestures pass through
  completely; dropping modifiers while F10 remains held never activates Play.
  Injected input never activates controls.
- An unfinished gesture does not block other operations. F8, suspend/close,
  another accepted start, Configure, deletion, or an intervening blocked state
  invalidates it. Returning to idle or repeating the held key cannot revive it.
- Use existing backend Play guards and session ownership, held-input validation,
  absolute physical coordinates, playback properties, cancellation, and cleanup.
  Preserve external focus; add no countdown or automatic target activation.
- An accepted F10 release cancels an unsaved inline-name draft and proceeds,
  including when later preflight fails. A submitted Save finishes first, then
  playback starts automatically. First deferred start wins. F8 cancels the
  waiting playback without cancelling the disk write. Failed Save yields to a
  still-waiting start using persisted data and suppresses the discarded draft's
  warning; cancellation retains the existing rename failure/recovery behavior.
- Release without selection produces one timed toast: `Select a macro to play`.
  Recording, Configure, and other busy states silently ignore F10. Repeats
  produce no additional requests or messages.
- Explain all unavailable shortcuts together, prioritizing missing F8 Stop.
  Retry shortcuts does not start a session. A Play-only conflict preserves GUI
  Play when its normal guards, including global Stop, pass.
- Play's tooltip includes F10. Idle hints advertise only usable Record/Play
  shortcuts. Active and pending-start controls prioritize Stop. Accessible
  names and disabled explanations remain clear.
- Update current PRD/help exclusions; retain historical completed-Story scope.

## Test seams

The owner confirmed public library/editor interactions, backend session commands
and snapshots, and native shortcut delivery as test seams. Deterministic tests
substitute desktop effects. Real Windows registration/delivery and manual target
application observations are recorded separately. Final review base:
`7a013f31318db0a42b6763c760d9ba8412f5782c`.
