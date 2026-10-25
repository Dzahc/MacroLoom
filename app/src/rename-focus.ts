import { PHASE, type LibraryPhase } from './library-model.ts';

/** Browser focus boundary; tests substitute focus ownership without replacing editor or session state. */
export type RenameFocusTarget = {
  hasFocus: () => boolean;
  focusRow: (macroId: string) => void;
};

/** @param takeFocus Consumes the pending origin, if an editor owner exists. @param phase Current session phase. @param target Browser focus adapter. @returns Nothing; consumes even skipped requests so later session transitions cannot replay them. */
export function restoreRenameFocus(
  takeFocus: (() => string | null) | undefined,
  phase: LibraryPhase,
  target: RenameFocusTarget,
): void {
  if (!takeFocus) return;
  const macroId = takeFocus();
  if (!macroId || phase !== PHASE.idle || !target.hasFocus()) return;
  target.focusRow(macroId);
}
