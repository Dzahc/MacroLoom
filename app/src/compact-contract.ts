import { PHASE, type LibraryPhase } from './library-model.ts';

export const WINDOW_COMMAND = {
  view: 'set_window_view',
  snapshot: 'presentation_snapshot',
  preview: 'preview_presentation',
  openPreview: 'open_presentation_preview',
} as const;
export const PRESENTATION_EVENT = 'presentation-state';
export const WINDOW_VIEW_EVENT = 'window-view-state';
export const PREVIEW_QUERY = 'compact-development';
export const COMPACT = {
  minimumWidth: 360,
  progressInterval: 100,
  applyDelay: 3000,
} as const;

/** Backend-owned presentation data; elapsed includes waits and intervals, never macro duration. */
export type SessionPresentation = Readonly<{
  revision: number;
  phase: LibraryPhase;
  macroName: string | null;
  elapsedMs: number;
  run: number;
  totalRuns: number | null;
  remainingMs: number;
}>;

/** Content measurements in logical CSS pixels; the Windows adapter owns physical conversion. */
export type WindowViewRequest = Readonly<{
  compact: boolean;
  width: number;
  height: number;
}>;

/** Actual window mode after a transition, including an optional timed-toast failure message. */
export type WindowViewResult = Readonly<{
  compact: boolean;
  error: string | null;
}>;

export const IDLE_PRESENTATION: SessionPresentation = {
  revision: 0,
  phase: PHASE.idle,
  macroName: null,
  elapsedMs: 0,
  run: 1,
  totalRuns: 1,
  remainingMs: 0,
};
