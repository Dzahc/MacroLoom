import type { ConfigureCompletion } from './configure-contract.ts';
import type { MacroSummary } from './library-model.ts';

export const RENAME_COMMAND = {
  submit: 'rename_macro',
  status: 'rename_status',
} as const;
export const RENAME_KEY = {
  open: 'F2',
  save: 'Enter',
  cancel: 'Escape',
} as const;
export const RENAME_TEXT = {
  updated: 'Renamed',
  name: 'Macro name',
  save: 'Save name',
  cancel: 'Cancel rename',
  retry: 'Check save result',
  failure: 'Could not rename macro',
  uncertain: 'Save result unavailable. Check the result before editing again.',
} as const;
export const RENAME_INITIAL_ATTEMPT = 0;
export const RENAME_ATTEMPT_INCREMENT = 1;
export const RENAME_POLL_MS = 100;
export type RenameInput = Readonly<{
  attemptId: number;
  macroId: string;
  name: string;
}>;
export type RenameCompletion = ConfigureCompletion &
  Readonly<{ discarded: boolean }>;
export type RenameStatus = Readonly<{
  attemptId: number;
  pending: boolean;
  completion: RenameCompletion | null;
}>;
export type RenameTransport = {
  status: () => Promise<RenameStatus>;
  submit: (input: RenameInput) => Promise<RenameCompletion>;
};
export type RenameSnapshot = Readonly<{
  draft: Readonly<{ macroId: string; name: string }> | null;
  pending: boolean;
  uncertain: boolean;
  attempted: boolean;
  error: string;
  focusId: string | null;
}>;
export type RenameBinding = {
  snapshot: RenameSnapshot;
  open: (macro: MacroSummary) => void;
  change: (name: string) => void;
  cancel: (restoreFocus?: boolean) => void;
  save: () => Promise<void>;
};
