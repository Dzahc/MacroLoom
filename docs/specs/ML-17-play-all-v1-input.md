# ML-17 — Play all v1 input types

Source: [ML-17](https://github.com/Dzahc/MacroLoom/issues/17).
Owner-reviewed decisions: October 23, 2026.

Play and list double click pass the selected ID to one backend session workflow.
The backend reserves operation ownership and F8, reads the validated trusted file,
and owns a snapshot of its events, environment, name and playback properties.
External changes after that snapshot cannot alter the active run. A later start
still checks the repository fingerprint.

Run once at 1×, silently, preserving all saved speed/repetition settings. Do not
add a startup countdown. Finish preconditions and compact entry before starting
one monotonic clock shared by dispatch and elapsed status. Preserve initial wait,
all event offsets, equal-time order, observed repeats, holds and trailing duration.
Retained drag points are replayed without interpolation or another sampling pass.

Refuse startup while physical keys/buttons remain held, with release-and-retry
guidance. Keyboard Play completes on Enter/Space release; suppress early/repeated
native activation. This startup snapshot does not lock physical input or promise
protection against later manual interference. Show shared-cursor/focus guidance
and tell users to record a target-window click before typing.

Validate the recorded keyboard layout and physical monitor geometry/scales before
dispatch and observe relevant changes during the run. Display power alone does
not interrupt compatible playback: dark screens with unchanged desktop geometry
can continue. Actual layout changes and system sleep interrupt the run; never
replay an overdue backlog on resume. Environment extension metadata does not
affect compatibility. Target-window recognition/remapping remains deferred.

Keep the existing per-event collision check against the compact window's current
actual physical bounds. Do not add an upfront collision scan or automatically
reposition the toolbar. Failure explains that the toolbar must be moved.

Stop, completion and injection rejection cancel subsequent ordinary dispatch and
release only successfully injected owned holds. Positioned mouse actions move
before their button/wheel transition; partial failure does not claim the rejected
transition succeeded. Cleanup releases at the current position without moving to
an unexecuted target. Cleanup failure retains ownership and Retry/F8 recovery.
Restore the full view after cleanup, preserving focus, placement, selection and
scroll. Distinguish failure/cancellation from completion.

The owner confirmed the remaining ML-16 physical-input checks during this review;
they do not need revalidation to begin ML-17. ML-17 still owns its own end-to-end
replay acceptance in ordinary Windows applications and mixed DPI. Google Play
Games is excluded from testing and must not be interacted with: the owner is
actively using it.
