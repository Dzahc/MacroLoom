import {
  IDLE_PRESENTATION,
  PLAYBACK_STAGE,
  type SessionPresentation,
} from './compact-contract.ts';
import {
  LABEL,
  PHASE,
  formatDuration,
  type LibraryPhase,
  type LibrarySnapshot,
} from './library-model.ts';

export const SESSION_TEXT = {
  recording: 'Recording',
  playing: 'Playing',
  interval: 'Between runs',
  stopping: 'Stopping…',
  cleanup: 'Releasing input and resources',
  run: 'Run',
  complete: 'complete',
  next: 'Next run in',
  infinity: '∞',
  busy: 'busy',
  starting: 'Starting',
  finishing: 'Finishing run',
} as const;
const COUNTDOWN_DIGITS = 1;
const COUNTDOWN_TENTH_MS = 100;
const TENTHS_PER_SECOND = 10;

/** @param phase Backend phase. @returns Whether its presentation occupies the compact window, including cleanup. */
export function isCompactPhase(phase: LibraryPhase): boolean {
  return [PHASE.recording, PHASE.playing, PHASE.interval, PHASE.stopping].some(
    /** @param active Compact phase. @returns Whether it matches the supplied state. */
    (active) => active === phase,
  );
}

/** @param snapshot Library state with an optional native failure override. @returns The actual presented layout. */
export function isCompactView(snapshot: LibrarySnapshot): boolean {
  return snapshot.compact ?? isCompactPhase(snapshot.phase);
}

/** @param phase Requested backend phase. @param actual Actual native compact mode. @param failed Whether the transition failed. @returns Safe toolbar/status availability until restoration completes. */
function nativePhase(
  phase: LibraryPhase,
  actual: boolean | null,
  failed: boolean,
): LibraryPhase {
  if (actual && !isCompactPhase(phase)) return PHASE.stopping;
  if (failed && !actual) return PHASE.idle;
  return phase;
}

/** @param actual Actual native compact mode. @param failed Whether native work failed. @returns A retained actual-layout override, or undefined to measure the requested entry layout. */
function nativeCompact(
  actual: boolean | null,
  failed: boolean,
): boolean | undefined {
  if (failed) return actual ?? false;
  return actual === true ? true : undefined;
}

/** @param snapshot Persisted library state. @param session Revisioned backend presentation. @param view Actual native layout result. @returns Derived presentation preserving library identities, selection, and failed-transition controls. */
export function presentedLibrary(
  snapshot: LibrarySnapshot,
  session: SessionPresentation,
  view: Readonly<{ compact: boolean | null; failed: boolean }>,
): LibrarySnapshot {
  if (session.revision === 0) return snapshot;
  const phase = nativePhase(session.phase, view.compact, view.failed);
  return {
    ...snapshot,
    phase,
    session,
    message: phase === PHASE.saving ? LABEL.saving : snapshot.message,
    compact: nativeCompact(view.compact, view.failed),
  };
}

/** @param snapshot Supplied backend/controlled status. @returns Two readable banner lines; recording never invents a name. */
export function sessionBanner(snapshot: LibrarySnapshot): {
  title: string;
  detail: string;
  indicator: string;
} {
  const session = snapshot.session ?? IDLE_PRESENTATION;
  const elapsed = formatDuration(session.elapsedMs);
  const runs = `${SESSION_TEXT.run} ${session.run}/${session.totalRuns ?? SESSION_TEXT.infinity}`;
  switch (snapshot.phase) {
    case PHASE.recording:
      return {
        title: SESSION_TEXT.recording,
        detail: elapsed,
        indicator: PHASE.recording,
      };
    case PHASE.playing:
      return playbackBanner(session, elapsed, runs);
    case PHASE.interval:
      return {
        title: `${runs} ${SESSION_TEXT.complete}`,
        detail: `${SESSION_TEXT.next} ${(Math.ceil(session.remainingMs / COUNTDOWN_TENTH_MS) / TENTHS_PER_SECOND).toFixed(COUNTDOWN_DIGITS)} s · ${elapsed}`,
        indicator: SESSION_TEXT.busy,
      };
    case PHASE.stopping:
      return {
        title: SESSION_TEXT.stopping,
        detail: SESSION_TEXT.cleanup,
        indicator: SESSION_TEXT.busy,
      };
    default:
      return {
        title: snapshot.message,
        detail: `${LABEL.recordShortcut} · ${LABEL.stopShortcut}`,
        indicator:
          snapshot.phase !== PHASE.idle || snapshot.deleting
            ? SESSION_TEXT.busy
            : '',
      };
  }
}

/** @param session Authoritative playback progress. @param elapsed Formatted total session time. @param runs Current run label. @returns Distinct preparation/release/input feedback without frontend scheduling. */
function playbackBanner(
  session: SessionPresentation,
  elapsed: string,
  runs: string,
): ReturnType<typeof sessionBanner> {
  if (session.playbackStage === PLAYBACK_STAGE.starting) {
    return {
      title: session.macroName
        ? `${SESSION_TEXT.starting} “${session.macroName}”`
        : SESSION_TEXT.starting,
      detail: elapsed,
      indicator: SESSION_TEXT.busy,
    };
  }
  if (session.playbackStage === PLAYBACK_STAGE.finishing) {
    return {
      title: `${SESSION_TEXT.finishing} ${session.run}/${session.totalRuns ?? SESSION_TEXT.infinity}`,
      detail: elapsed,
      indicator: SESSION_TEXT.busy,
    };
  }
  return {
    title: session.macroName ?? SESSION_TEXT.playing,
    detail: `${runs} · ${elapsed}`,
    indicator: SESSION_TEXT.busy,
  };
}
