import type { PlaybackProperties } from './macro-contract.ts';

export const CONFIGURE_QUERY = 'configure';
export const CONFIGURE_WINDOW = 'configure';
export const CONFIGURE_COMMAND = {
  open: 'open_configure',
  snapshot: 'configure_snapshot',
  submit: 'configure_submit',
  claim: 'configure_claim',
  resolve: 'configure_resolve',
  close: 'close_configure',
} as const;
export const CONFIGURE_EVENT = {
  submit: 'configure-submit',
  closed: 'configure-closed',
} as const;
export const CONFIGURE_LIMIT = {
  name: 120,
  milliseconds: 1000,
  precision: 3,
  firstRun: 1,
  max: Number.MAX_SAFE_INTEGER,
} as const;
export const PLAYBACK_SPEED = {
  quarter: 0.25,
  half: 0.5,
  normal: 1,
  double: 2,
  quadruple: 4,
} as const;
export const SPEED_PRESETS = Object.freeze(Object.values(PLAYBACK_SPEED));
export const REPEAT_MODE = {
  once: 'once',
  fixed: 'fixed',
  indefinite: 'indefinite',
} as const;
export const CONFIGURE_DEFAULT_PLAYBACK = {
  speed: PLAYBACK_SPEED.normal,
  repeatMode: REPEAT_MODE.once,
  totalRuns: CONFIGURE_LIMIT.firstRun,
  intervalMs: 0,
} as const satisfies Readonly<PlaybackProperties>;
export const CONFIGURE_FIELD_KEY = {
  name: 'name',
  speed: 'speed',
  repeatMode: 'repeatMode',
  totalRuns: 'totalRuns',
  interval: 'interval',
} as const;
export const CONFIGURE_FIELD = Object.freeze(
  Object.values(CONFIGURE_FIELD_KEY),
);
export const CONFIGURE_INPUT_ID = {
  name: 'configure-name',
  speed: 'configure-speed',
  repeatMode: 'configure-repeatMode',
  totalRuns: 'configure-totalRuns',
  interval: 'configure-interval',
} as const;
export type ConfigureField = (typeof CONFIGURE_FIELD)[number];
export type ConfigureSnapshot = Readonly<{
  macroId: string;
  name: string;
  playback: PlaybackProperties;
}>;
export type ConfigureDraft = Readonly<{
  macroId: string;
  name: string;
  playback: PlaybackProperties;
}>;
export type ConfigureErrors = Partial<Record<ConfigureField, string>>;
export type ConfigureResult =
  | { ok: true }
  | { ok: false; message: string; fields?: ConfigureErrors };
export type ConfigureSave = (draft: ConfigureDraft) => Promise<ConfigureResult>;
export type ConfigureSubmission = Readonly<{
  attemptId: number;
}>;
export const CONFIGURE_TEXT = {
  title: 'Configure macro',
  name: 'Name',
  speed: 'Playback speed',
  repeatMode: 'Repeat mode',
  totalRuns: 'Total runs',
  interval: 'Interval between runs (seconds)',
  once: 'Once',
  fixed: 'Fixed count',
  indefinite: 'Indefinitely',
  runsHint: 'Includes the first run.',
  intervalHint:
    'Up to three decimal places. The interval is not scaled by playback speed.',
  save: 'Save',
  saving: 'Saving…',
  cancel: 'Cancel',
  unavailable:
    'Property saving is currently unavailable. Your edits are retained; try again or Cancel.',
  failure:
    'Could not save changes. Your edits are retained; try again or Cancel.',
  loading: 'Loading macro properties…',
  openFailure: 'Could not open Configure',
} as const;
