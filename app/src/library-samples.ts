import {
  LABEL,
  PHASE,
  type LibraryPhase,
  type MacroSummary,
} from './library-model.ts';

export const SAMPLE_MACROS: readonly MacroSummary[] = [
  {
    id: 'sample-report',
    name: 'Update report',
    durationMs: 24000,
    createdAt: '2026-09-28T22:00:00Z',
  },
  {
    id: 'sample-form',
    name: 'Fill form',
    durationMs: 8000,
    createdAt: '2026-09-28T21:32:08Z',
  },
  {
    id: 'sample-long',
    name: 'Prepare the monthly report and transfer the completed figures to the shared planning workbook',
    durationMs: 3737000,
    createdAt: '2026-09-27T21:32:08Z',
  },
];
export const SCENARIO = {
  populated: 'populated',
  empty: 'empty',
  failedLoad: 'failed-load',
  saving: 'saving',
} as const;
export type ScenarioName = (typeof SCENARIO)[keyof typeof SCENARIO];
export type SampleScenario = {
  label: string;
  macros: readonly MacroSummary[];
  phase: LibraryPhase;
  message: string;
  failures: readonly string[];
};
const LOAD_FAILURE =
  'Could not load broken-macro.json: unsupported sample format. File left unchanged.';
export const SCENARIOS: Record<ScenarioName, SampleScenario> = {
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
