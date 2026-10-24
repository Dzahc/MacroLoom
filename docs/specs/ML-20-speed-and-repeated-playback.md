# ML-20 — Speed and repeated playback

The canonical story is [ML-20](https://github.com/Dzahc/MacroLoom/issues/20).
PRD §§4.4–4.5, 6–7, 9 and 11 apply. The following decisions were confirmed in
the implementation discussion on October 24, 2026.

- Own one immutable saved-document/property snapshot throughout playback.
  Support 0.25×, 0.5×, 1×, 2× and 4×, including initial waits, event spacing,
  held durations and recorded trailing duration. Retain submillisecond deadlines
  internally and original equal-timestamp ordering.
- Once means one run; fixed counts include the first run; indefinite playback
  continues until cancellation or failure. Inactive Once settings do not add runs
  or a final interval.
- Use fresh monotonic run-start anchors. Dispatch overhead does not accumulate
  within a run. Late completion does not shorten intervals or create catch-up
  repetitions: begin the full unscaled interval after actual completion and
  successful release of that run's remaining input. Omit the final finite interval.
- Permit zero-duration, zero-interval repetition. Check cancellation throughout
  dispatch and run transitions and yield the worker between repetitions without
  an artificial minimum interval.
- Retain compact placement, Stop reservation and operation exclusion through all
  runs, per-run cleanup, intervals and terminal cleanup. Failed release prevents
  another run; retained failed releases remain available to terminal cleanup/Retry.
- Continue environment compatibility checks during intervals. A detected mismatch
  cancels playback; suspend cancels rather than replaying overdue input on resume.
- Start session elapsed time when the backend accepts playback ownership, including
  startup preparation, recorded waits, cleanup and intervals. Establish the first
  event schedule only when preparation is complete. Show `Starting “Name”` with
  elapsed time before run 1, then the active run/total or infinity. Show
  `Finishing run 2/5` with elapsed time during per-run release. Between runs, show
  the completed run number, elapsed time and the next-run countdown; increment
  the run number only when the next run starts.
- Round a positive countdown upward to tenths of a second, displaying at least
  `0.1 s` until expiry. Display rounding never changes the backend deadline.
- Manual target-application acceptance uses Notepad only. Do not infer Google
  Play Games compatibility from these checks.

Tests use the agreed public seams: playback scheduling with controlled clock/input
effects; session commands/snapshots with controlled desktop effects; and frontend
presentation derived from backend snapshots. The review base is `7bd8822`.
