import { PHASE } from './library-model.ts';
import type { BackendLibraryState } from './macro-contract.ts';
import {
  IDLE_PRESENTATION,
  PLAYBACK_STAGE,
  type PlaybackStage,
} from './compact-contract.ts';

export const SESSION_EVENT = 'session-state';
export const SESSION_COMMAND = {
  snapshot: 'session_snapshot',
  record: 'start_recording',
  stop: 'stop',
  replay: 'play',
  playMacro: 'play_macro',
  retry: 'retry_shortcuts',
  confirmation: 'session_confirmation',
  keepOpen: 'session_keep_open',
  discardExit: 'session_discard_exit',
  retryRecording: 'retry_recording',
  discardRecording: 'discard_recording',
} as const;
export const SESSION_TEXT = {
  retry: 'Retry shortcuts',
  cleanup: 'Retry cleanup',
  replay: 'Replay temporary recording',
  retained: 'Recording retained in memory; not saved.',
  close:
    'This recording is not saved. Retry and exit, discard and exit, or keep MacroLoom open.',
  keepOpen: 'Keep open',
  discard: 'Discard and exit',
  starting: 'Checking global shortcuts…',
  failure: 'Session control failed',
  f9Conflict:
    'F9 Record is unavailable; use the Record button or Retry shortcuts.',
  f8Conflict:
    'F8 Stop is unavailable. Close the application using it, then Retry shortcuts.',
  playbackGuidance: 'Playback uses your cursor and keyboard',
  retryRecording: 'Retry save',
  retryExit: 'Retry and exit',
  discardRecording: 'Discard recording',
} as const;

/** Native command/event contract; capture data and injected held-input ownership never enter React state. */
export type SessionSnapshot = Readonly<{
  revision: number;
  renameSaving: boolean;
  deferredStart: boolean;
  startSequence: number;
  phase:
    | typeof PHASE.idle
    | typeof PHASE.recording
    | typeof PHASE.playing
    | typeof PHASE.interval
    | typeof PHASE.saving
    | typeof PHASE.stopping;
  elapsedMs: number;
  eventCount: number;
  durationMs: number;
  dragMoveCount: number;
  lastRelease: readonly [number, number] | null;
  message: string;
  shortcuts: Readonly<{ record: boolean; stop: boolean }>;
  canRecord: boolean;
  canPlay: boolean;
  macroName: string | null;
  playbackStage: PlaybackStage;
  run: number;
  totalRuns: number | null;
  remainingMs: number;
  cleanupFailed: boolean;
  closeRequested: boolean;
  unsaved: boolean;
  saveFailed: boolean;
  recordingOutcome: RecordingOutcome | null;
}>;
/** Durable backend receipt; a library entry and automatic selection require committed persistence. */
export type RecordingOutcome = Readonly<{
  sequence: number;
  id: string;
  name: string;
  success: boolean;
  message: string;
  library: BackendLibraryState | null;
}>;
export const INITIAL_SESSION: SessionSnapshot = {
  revision: -1,
  renameSaving: false,
  deferredStart: false,
  startSequence: 0,
  phase: PHASE.idle,
  elapsedMs: 0,
  eventCount: 0,
  durationMs: 0,
  dragMoveCount: 0,
  lastRelease: null,
  message: SESSION_TEXT.starting,
  shortcuts: { record: false, stop: false },
  canRecord: false,
  canPlay: false,
  macroName: null,
  playbackStage: PLAYBACK_STAGE.running,
  run: IDLE_PRESENTATION.run,
  totalRuns: IDLE_PRESENTATION.totalRuns,
  remainingMs: IDLE_PRESENTATION.remainingMs,
  cleanupFailed: false,
  closeRequested: false,
  unsaved: false,
  saveFailed: false,
  recordingOutcome: null,
};
/** Commands whose input is empty and whose result is a revisioned session snapshot. */
export type SnapshotCommand = Exclude<
  (typeof SESSION_COMMAND)[keyof typeof SESSION_COMMAND],
  | typeof SESSION_COMMAND.confirmation
  | typeof SESSION_COMMAND.discardExit
  | typeof SESSION_COMMAND.playMacro
>;
/** Validated native confirmation claim/release; each mounted owner uses a distinct identity. */
export type ConfirmationRequest = Readonly<{ owner: string; blocked: boolean }>;

/** Native subscription and command boundaries; no timer or session lifecycle is implemented in the frontend. */
export type SessionTransport = {
  listen: (receive: (snapshot: SessionSnapshot) => void) => Promise<() => void>;
  invoke: (command: SnapshotCommand) => Promise<SessionSnapshot>;
  play?: (macroId: string) => Promise<SessionSnapshot>;
  confirmation?: (request: ConfirmationRequest) => Promise<SessionSnapshot>;
};
