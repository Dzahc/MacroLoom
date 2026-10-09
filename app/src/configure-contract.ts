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
export const SPEED_PRESETS = [0.25, 0.5, 1, 2, 4] as const;
export const REPEAT_MODE = {
  once: 'once',
  fixed: 'fixed',
  indefinite: 'indefinite',
} as const;
export const CONFIGURE_FIELD = [
  'name',
  'speed',
  'repeatMode',
  'totalRuns',
  'interval',
] as const;
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
