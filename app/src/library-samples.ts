import {
  LABEL,
  PHASE,
  type LibraryPhase,
  type MacroSummary,
} from './library-model.ts';

import { SAMPLE_MACROS } from './sample-macros.ts';
import { SESSION_SAMPLES, type SessionScenario } from './session-samples.ts';
import type { SessionPresentation } from './compact-contract.ts';
export { SAMPLE_MACROS } from './sample-macros.ts';
export const SCENARIO = {
  populated: 'populated',
  empty: 'empty',
  failedLoad: 'failed-load',
  saving: 'saving',
  recording: 'recording',
  playing: 'playing',
  infinite: 'infinite',
  interval: 'interval',
  stopping: 'stopping',
} as const;
export type ScenarioName = (typeof SCENARIO)[keyof typeof SCENARIO];
export type SampleScenario = {
  label: string;
  macros: readonly MacroSummary[];
  phase: LibraryPhase;
  message: string;
  failures: readonly string[];
  session?: SessionPresentation;
};
const LOAD_FAILURE =
  'Could not load broken-macro.json: unsupported sample format. File left unchanged.';
/** @param scenario Controlled session key. @returns Presentation-only sample sharing the stable library and selection identities. */
function activeSample(scenario: SessionScenario): SampleScenario {
  const sample = SESSION_SAMPLES[scenario];
  return {
    label: sample.label,
    phase: sample.status.phase,
    session: sample.status,
    macros: SAMPLE_MACROS,
    message: LABEL.ready,
    failures: [],
  };
}
export const SCENARIOS: Record<ScenarioName, SampleScenario> = {
  recording: activeSample('recording'),
  playing: activeSample('playing'),
  infinite: activeSample('infinite'),
  interval: activeSample('interval'),
  stopping: activeSample('stopping'),
  populated: {
    label: 'Populated library',
    macros: SAMPLE_MACROS,
    phase: PHASE.idle,
    message: LABEL.ready,
    failures: [],
  },
  empty: {
    label: 'Empty library',
    macros: [],
    phase: PHASE.idle,
    message: LABEL.ready,
    failures: [],
  },
  'failed-load': {
    label: 'Load failure (log only)',
    macros: SAMPLE_MACROS,
    phase: PHASE.idle,
    message: LABEL.ready,
    failures: [LOAD_FAILURE],
  },
  saving: {
    label: 'Saving (actions unavailable)',
    macros: SAMPLE_MACROS,
    phase: PHASE.saving,
    message: LABEL.saving,
    failures: [],
  },
};
export const OUTCOME_EXAMPLES = [
  {
    label: 'Recording saved',
    kind: 'success',
    message: 'Saved recording “Update report”.',
  },
  {
    label: 'Recording save failed',
    kind: 'failure',
    message: 'Could not save recording “Update report”.',
  },
  {
    label: 'Properties saved',
    kind: 'success',
    message: 'Saved properties for “Fill form”.',
  },
  {
    label: 'Property save failed',
    kind: 'failure',
    message: 'Could not save properties for “Fill form”.',
  },
  { label: 'Macro deleted', kind: 'success', message: 'Deleted “Fill form”.' },
  {
    label: 'Delete failed',
    kind: 'failure',
    message: 'Could not delete “Fill form”.',
  },
] as const;
export const DEV_TEXT = {
  title: 'Development preview',
  hint: 'Sample data only. Actions are logged; no input or files are changed.',
  scenarios: 'Library scenario',
  toasts: 'Example notifications',
  log: 'Action and failure log',
  emptyLog: 'Interact with the library to inspect its callbacks.',
  clear: 'Clear log',
  open: 'Open development pane',
  close: 'Close development pane',
  maxLog: 30,
} as const;
