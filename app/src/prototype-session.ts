import type { SessionSnapshot } from './session-contract.ts';
import type { Snapshot } from './prototype-view.tsx';

/** @param session Authoritative native lifecycle. @returns Compatibility presentation for the optional ML-8 view; unavailable timing metrics remain explicitly absent. */
export function prototypeSnapshot(session: SessionSnapshot): Snapshot {
  return {
    phase: session.phase,
    eventCount: session.eventCount,
    dragMoveCount: session.dragMoveCount,
    lastRelease: session.lastRelease ? [...session.lastRelease] : null,
    lastReplayRelease: null,
    durationMs:
      session.phase === 'recording' ? session.elapsedMs : session.durationMs,
    message: session.message,
    captureGapMinMs: null,
    dispatchLatenessP95Ms: null,
    dispatchLatenessMaxMs: null,
    stopResponseMs: null,
    f9Available: session.shortcuts.record,
    f8Available: session.shortcuts.stop,
  };
}
