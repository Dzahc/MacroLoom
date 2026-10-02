export const ACTION = {
  record: 'record',
  play: 'play',
  stop: 'stop',
  configure: 'configure',
  delete: 'delete',
} as const;
export const PHASE = {
  idle: 'idle',
  saving: 'saving',
  recording: 'recording',
  playing: 'playing',
  stopping: 'stopping',
} as const;
export const SOURCE = {
  toolbar: 'toolbar',
  row: 'row',
  context: 'context-menu',
  doubleClick: 'double-click',
} as const;
export const TOOLBAR = [
  ACTION.record,
  ACTION.play,
  ACTION.stop,
  ACTION.configure,
  ACTION.delete,
] as const;
export const LABEL = {
  app: 'MacroLoom',
  controls: 'Macro actions',
  macros: 'Macros',
  empty: 'No macros yet. Record your first macro.',
  ready: 'Ready',
  saving: 'Saving…',
  recordShortcut: 'Record: F9',
  stopShortcut: 'Stop: F8',
  dismiss: 'Dismiss notification',
  record: 'Record',
  play: 'Play',
  stop: 'Stop',
  configure: 'Configure',
  delete: 'Delete',
} as const;
export const TOOLTIP = {
  record: 'Record (F9)',
  play: 'Play',
  stop: 'Stop (F8)',
  configure: 'Configure',
  delete: 'Delete',
} as const;
export const TIME = {
  millisecond: 1000,
  minute: 60,
  hour: 3600,
  pad: 2,
  toast: 4000,
} as const;

export type MacroSummary = Readonly<{
  id: string;
  name: string;
  durationMs: number;
  createdAt: string;
}>;
export type LibraryPhase = (typeof PHASE)[keyof typeof PHASE];
export type ActionName = (typeof ACTION)[keyof typeof ACTION];
export type ActionSource = (typeof SOURCE)[keyof typeof SOURCE];
export type LibraryAction =
  | { action: typeof ACTION.record | typeof ACTION.stop; source: ActionSource }
  | {
      action:
        | typeof ACTION.play
        | typeof ACTION.configure
        | typeof ACTION.delete;
      macroId: string;
      source: ActionSource;
    };
export type SelectionChange = {
  macroId: string;
  source: typeof SOURCE.row | typeof SOURCE.context | typeof SOURCE.doubleClick;
};
export type LibrarySnapshot = Readonly<{
  macros: readonly MacroSummary[];
  selectedId: string | null;
  phase: LibraryPhase;
  message: string;
}>;
export type LibraryCallbacks = {
  onSelect: (change: SelectionChange) => void;
  onAction: (request: LibraryAction) => void;
};
export type Outcome = Readonly<{
  id: number;
  kind: 'success' | 'failure';
  message: string;
}>;

export function selectedMacro(
  snapshot: LibrarySnapshot,
): MacroSummary | undefined {
  return snapshot.macros.find((macro) => macro.id === snapshot.selectedId);
}

export function canRequest(
  snapshot: LibrarySnapshot,
  action: ActionName,
  macroId: string | null = snapshot.selectedId,
): boolean {
  if (action === ACTION.stop)
    return [PHASE.recording, PHASE.playing, PHASE.stopping].some(
      (phase) => phase === snapshot.phase,
    );
  if (snapshot.phase !== PHASE.idle) return false;
  if (action === ACTION.record) return true;
  return snapshot.macros.some((macro) => macro.id === macroId);
}

export function requestAction(
  snapshot: LibrarySnapshot,
  action: ActionName,
  source: ActionSource,
  macroId: string | null = snapshot.selectedId,
): LibraryAction | null {
  if (!canRequest(snapshot, action, macroId)) return null;
  if (action === ACTION.record || action === ACTION.stop)
    return { action, source };
  if (macroId === null) return null;
  return { action, macroId, source };
}

export function reconcileSelection(
  macros: readonly MacroSummary[],
  selectedId: string | null,
): string | null {
  return macros.some((macro) => macro.id === selectedId) ? selectedId : null;
}

export function formatDuration(durationMs: number): string {
  const total = Math.max(0, Math.floor(durationMs / TIME.millisecond));
  const seconds = String(total % TIME.minute).padStart(TIME.pad, '0');
  const minutes = String(
    Math.floor(total / TIME.minute) % TIME.minute,
  ).padStart(TIME.pad, '0');
  if (total < TIME.hour) return `${minutes}:${seconds}`;
  return `${Math.floor(total / TIME.hour)}:${minutes}:${seconds}`;
}
