# ML-14 — Global Record and Stop actions

Type: Story
Release: v1
PRD references: §§4.1, 4.4, 4.7, 6–11
Parent Epic: #3
Blocked by: #10 (closed)
Implementation branch: `feature/ml-14-global-record-and-stop-actions`

## Reviewed scope — October 12, 2026

Connect the real library toolbar and compact view to one backend-owned native
session lifecycle, reusing the existing in-memory input prototype. F9 starts
recording and F8 globally stops recording or playback while another application
has focus. No global Play shortcut or automatic target activation is added.

Automatic recording persistence belongs to ML-15, saved-library playback to
ML-17, and final exit/suspend integration to ML-22. Until then, recordings are
retained in memory and clearly marked unsaved. Development controls replay the
temporary capture; the saved-library Play action must not pretend to play it.
Starting another recording replaces the previous temporary capture without a
discard prompt, only after start validation succeeds. Closing with unsaved input
stops active work and requires explicit Discard and exit or Keep open.

## Acceptance criteria

- Record button and F9 use the same backend start validation and session owner.
  Repeated/held Record never restarts or toggles capture; recording and playback
  cannot overlap or overlap Configure, deletion, or library initialization.
- Reserve F9 while idle/recording, releasing it in other unavailable states.
  Reserve F8 before committing any session start and keep it through recording,
  playback, waits, and cleanup. Release it after terminal cleanup.
- A reserved shortcut consumes its complete down/repeat/up sequence, excludes
  that sequence from capture, and finishes consuming its release even if its
  reservation ends first. Unreserved keys pass through normally.
- F9 registration failure is explained, but button recording remains usable
  when F8 is available. F8 registration failure prevents session start. Provide
  Retry shortcuts; each start must independently acquire F8 successfully.
- Stop reaches the backend directly, independent of frontend event delivery.
  Preserve cancellation during event waits and held keys/buttons. Keep Stopping
  ownership until injected input and native window restoration are complete.
- Failed releases remain tracked. Show persistent failure, reject new sessions,
  and provide Retry cleanup and F8 to retry only outstanding cleanup. Never
  replay input as part of retry or claim successful cleanup after failure.
- Live status and elapsed time reach the real compact toolbar/banner. Entry,
  progress, restoration, and shortcut actions preserve external focus. Use
  actual compact bounds to reject mouse playback into MacroLoom controls.
- Computer sleep ends capture and retains its temporary data; playback is
  cancelled without overdue input on resume. The final recording-on-sleep
  automatic-save workflow is added to ML-22 and uses ML-15 persistence.
- Production builds omit development replay controls. Controlled presentation
  previews cannot overwrite real active-session ownership or status.

## Validation

Test backend session commands/snapshots, Windows shortcut adapter behavior, and
frontend listener/action reconciliation. Separate deterministic tests, native
Windows checks, and owner-reported target-application observations. Manual
instructions must give exact actions, expected results, and a reporting format.
Hold F9 for two seconds both from idle and again during recording.

Record the required `npm.cmd --prefix app run verify` command/result here or in
the issue/PR after implementation. Manual acceptance remains pending until the
owner reports the supplied flow's results.
