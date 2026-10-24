import type { SessionSnapshot } from './session-contract.ts';
import type { Snapshot } from './prototype-view.tsx';
import { PHASE } from './library-model.ts';

/** @param session Authoritative native lifecycle. @returns Compatibility presentation for the optional ML-8 view; unavailable timing metrics remain explicitly absent. */
export function prototypeSnapshot(session: SessionSnapshot): Snapshot {
  return {
    phase: prototypePhase(session.phase),
    eventCount: session.eventCount,
    dragMoveCount: session.dragMoveCount,
    lastRelease: session.lastRelease ? [...session.lastRelease] : null,
    lastReplayRelease: null,
    durationMs:
      session.phase === PHASE.recording
        ? session.elapsedMs
        : session.durationMs,
    message: session.message,
    captureGapMinMs: null,
    dispatchLatenessP95Ms: null,
    dispatchLatenessMaxMs: null,
    stopResponseMs: null,
    f9Available: session.shortcuts.record,
    f8Available: session.shortcuts.stop,
  };
}

/** @param phase Native lifecycle. @returns The corresponding phase supported by the original optional prototype view. */
function prototypePhase(phase: SessionSnapshot['phase']): Snapshot['phase'] {
  if (phase === PHASE.saving) return PHASE.idle;
  if (phase === PHASE.interval) return PHASE.playing;
  return phase;
}
