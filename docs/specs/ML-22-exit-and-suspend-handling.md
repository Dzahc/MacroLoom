# ML-22 — Exit and suspend handling

Source: [ML-22](https://github.com/Dzahc/MacroLoom/issues/22).
Owner-reviewed decisions: October 24, 2026.

Cover ordinary main-window close, computer sleep and hibernation. Lock alone and
display blanking leave compatible sessions running. Sign-out, restart, shutdown,
forced termination, crash recovery and power-loss durability are separate scope.

Close stops recording and automatically saves its nonempty capture once, then
exits after success. Empty recordings create no file. Suspend freezes capture at
interruption and attempts the same automatic save. Windows may pause execution
before cleanup or persistence finishes; complete it when execution resumes, never
restart a session. Preserve the frozen identity, events and duration on failure.

During an existing recording save, Close joins that attempt and exits on success.
After a failed save, Close does not automatically retry. Close recovery offers
**Retry and exit**, **Discard and exit**, and **Keep open**. Keep open cancels exit
and retains capture with ordinary Retry/Discard controls. Repeated Close or suspend
notifications never discard, create duplicate saves, or retry failed saves.

Close or suspend cancels playback during startup, event waits, held keys, dragging
and repeat intervals. No overdue ordinary input is dispatched after wake. Release
session-owned holds at the current position and acknowledge hook/shortcut teardown
before exit. Cleanup failure retains ownership and pending exit, blocks new work,
and offers Retry/F8; successful recovery finishes the pending exit. No ordinary
force-exit action bypasses incomplete cleanup. Successful exit never waits for a
toast to expire.

Retain a visible sleep/hibernation explanation and outcome across wake until the
next deliberate operation. Recording save results include the interruption reason;
playback reports interruption rather than ordinary Stop. Actionable cleanup/save
failures remain visible independently of timed toasts. Sleep while already saving
or stopping does not start new work or replace the original operation's reason.

Preserve Configure's approved policy: idle drafts are discarded on close; pending
Save delays exit, success honors it, and failure retains edits and cancels exit.
Wait for a pending deletion: success honors exit, failure retains the entry and
ordinary deletion error and cancels exit. Close cancels an unconfirmed deletion
and exits. Library discovery can be cancelled on close; next launch scans again.

Validate session commands/snapshots and Configure/library operation boundaries,
rendered recovery controls, and native hook/shortcut shutdown separately. Cover
startup/close/suspend races, failed and pending saves, repeated notifications,
held-input cleanup and repeat intervals deterministically. Native checks establish
teardown; owner-run sleep **and hibernation** checks establish actual recording
saves, playback interruption, and no automatic restart or overdue input after wake.
Keep those evidence categories separate. Do not interact with Google Play Games.
