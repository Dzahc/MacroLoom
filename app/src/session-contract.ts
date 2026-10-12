import { PHASE } from './library-model.ts';

export const SESSION_EVENT = 'session-state';
export const SESSION_COMMAND = {
  snapshot: 'session_snapshot',
  record: 'start_recording',
  stop: 'stop',
  replay: 'play',
  retry: 'retry_shortcuts',
  confirmation: 'session_confirmation',
  keepOpen: 'session_keep_open',
  discardExit: 'session_discard_exit',
} as const;
export const SESSION_TEXT = {
  retry: 'Retry shortcuts',
  cleanup: 'Retry cleanup',
  replay: 'Replay temporary recording',
  retained: 'Recording retained in memory; not saved.',
  close:
    'This recording is not saved. Keep MacroLoom open or discard it and exit.',
  keepOpen: 'Keep open',
  discard: 'Discard and exit',
  starting: 'Checking global shortcuts…',
  failure: 'Session control failed',
  f9Conflict:
    'F9 Record is unavailable; use the Record button or Retry shortcuts.',
  f8Conflict:
    'F8 Stop is unavailable. Close the application using it, then Retry shortcuts.',
  playbackDeferred: 'Saved-macro playback arrives in ML-17.',
} as const;

/** Native command/event contract; capture data and injected held-input ownership never enter React state. */
export type SessionSnapshot = Readonly<{
  revision: number;
  phase:
    | typeof PHASE.idle
    | typeof PHASE.recording
    | typeof PHASE.playing
    | typeof PHASE.stopping;
  elapsedMs: number;
  eventCount: number;
  durationMs: number;
  dragMoveCount: number;
  lastRelease: readonly [number, number] | null;
  message: string;
  shortcuts: Readonly<{ record: boolean; stop: boolean }>;
  canRecord: boolean;
  cleanupFailed: boolean;
  closeRequested: boolean;
  unsaved: boolean;
}>;
export const INITIAL_SESSION: SessionSnapshot = {
  revision: -1,
  phase: PHASE.idle,
  elapsedMs: 0,
  eventCount: 0,
  durationMs: 0,
  dragMoveCount: 0,
  lastRelease: null,
  message: SESSION_TEXT.starting,
  shortcuts: { record: false, stop: false },
  canRecord: false,
  cleanupFailed: false,
  closeRequested: false,
  unsaved: false,
};

/** Native subscription and command boundaries; no timer or session lifecycle is implemented in the frontend. */
export type SessionTransport = {
  listen: (receive: (snapshot: SessionSnapshot) => void) => Promise<() => void>;
  invoke: (
    command: string,
    args?: Record<string, unknown>,
  ) => Promise<SessionSnapshot>;
};
