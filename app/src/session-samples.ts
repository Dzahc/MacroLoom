import {
  IDLE_PRESENTATION,
  type SessionPresentation,
} from './compact-contract.ts';
import { PHASE } from './library-model.ts';
import { SAMPLE_MACROS } from './sample-macros.ts';

export const SESSION_SCENARIO = {
  idle: 'idle',
  recording: 'recording',
  playing: 'playing',
  infinite: 'infinite',
  interval: 'interval',
  stopping: 'stopping',
  saving: 'saving',
} as const;
export type SessionScenario =
  (typeof SESSION_SCENARIO)[keyof typeof SESSION_SCENARIO];
const SAMPLE_ELAPSED = 18_000;
const SAMPLE_RUN = 2;
const SAMPLE_RUNS = 5;
const SAMPLE_INTERVAL = 3000;
const SAMPLE_INDEX = { report: 0, form: 1, monthlyReport: 2 } as const;
export const SESSION_SAMPLES: Record<
  SessionScenario,
  { label: string; status: SessionPresentation }
> = {
  idle: { label: 'Idle / restore full view', status: IDLE_PRESENTATION },
  recording: {
    label: 'Recording (no name)',
    status: {
      ...IDLE_PRESENTATION,
      phase: PHASE.recording,
      elapsedMs: SAMPLE_ELAPSED,
    },
  },
  playing: {
    label: 'Playback / long name',
    status: {
      ...IDLE_PRESENTATION,
      phase: PHASE.playing,
      macroName: SAMPLE_MACROS[SAMPLE_INDEX.monthlyReport].name,
      elapsedMs: SAMPLE_ELAPSED,
      run: SAMPLE_RUN,
      totalRuns: SAMPLE_RUNS,
    },
  },
  infinite: {
    label: 'Indefinite playback',
    status: {
      ...IDLE_PRESENTATION,
      phase: PHASE.playing,
      macroName: SAMPLE_MACROS[SAMPLE_INDEX.form].name,
      elapsedMs: SAMPLE_ELAPSED,
      run: SAMPLE_RUN,
      totalRuns: null,
    },
  },
  interval: {
    label: 'Between runs',
    status: {
      ...IDLE_PRESENTATION,
      phase: PHASE.interval,
      macroName: SAMPLE_MACROS[SAMPLE_INDEX.report].name,
      elapsedMs: SAMPLE_ELAPSED,
      run: SAMPLE_RUN,
      totalRuns: SAMPLE_RUNS,
      remainingMs: SAMPLE_INTERVAL,
    },
  },
  stopping: {
    label: 'Stopping / cleanup',
    status: { ...IDLE_PRESENTATION, phase: PHASE.stopping },
  },
  saving: {
    label: 'Saving / full view',
    status: { ...IDLE_PRESENTATION, phase: PHASE.saving },
  },
};

/** @param status Controlled seed. @param elapsed Monotonic demonstration elapsed milliseconds. @returns Progress-only status with no automatic session operations. */
export function advanceSample(
  status: SessionPresentation,
  elapsed: number,
): SessionPresentation {
  return {
    ...status,
    elapsedMs: status.elapsedMs + Math.floor(elapsed),
    remainingMs: Math.max(0, status.remainingMs - Math.floor(elapsed)),
  };
}

/** @param status Controlled UI status. @returns Only typed mutable backend fields; revisions remain backend-owned. */
export function previewStatus(
  status: SessionPresentation,
): Omit<SessionPresentation, 'revision'> {
  return {
    phase: status.phase,
    macroName: status.macroName,
    elapsedMs: status.elapsedMs,
    run: status.run,
    totalRuns: status.totalRuns,
    remainingMs: status.remainingMs,
  };
}
