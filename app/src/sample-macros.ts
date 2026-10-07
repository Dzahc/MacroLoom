import type { MacroSummary } from './library-model.ts';

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
