import {
  ACTION,
  PHASE,
  SOURCE,
  requestAction,
  type LibraryAction,
  type LibrarySnapshot,
} from './library-model.ts';

export const RENAME_PENCIL_REASON = {
  loading: 'Wait for macros to finish loading.',
  session: 'Finish the current session before renaming.',
  saving: 'Wait for the current save to finish.',
  confirmation: 'Close the current dialog before renaming.',
  deleting: 'Wait for deletion to finish.',
  storage: 'Macro storage is read-only or unavailable.',
  unavailable: 'Rename is currently unavailable.',
} as const;

/** @param snapshot Current library state. @param macroId Pencil's explicit target. @returns An ordinary rename intent, independently validated at activation. */
export function pencilRenameRequest(
  snapshot: LibrarySnapshot,
  macroId: string,
): LibraryAction | null {
  if (snapshot.selectedId !== macroId || snapshot.loading) return null;
  return requestAction(snapshot, ACTION.rename, SOURCE.row, macroId);
}

/** @param snapshot Current library state. @param macroId Pencil's explicit target. @param editorAvailable Whether a host owns the existing editor. @returns A hover explanation, or an empty string for an enabled pencil. */
export function renamePencilReason(
  snapshot: LibrarySnapshot,
  macroId: string,
  editorAvailable: boolean,
): string {
  if (snapshot.loading) return RENAME_PENCIL_REASON.loading;
  if (snapshot.phase !== PHASE.idle) return RENAME_PENCIL_REASON.session;
  if (snapshot.renameSaving) return RENAME_PENCIL_REASON.saving;
  if (snapshot.confirmationOpen) return RENAME_PENCIL_REASON.confirmation;
  if (snapshot.deleting) return RENAME_PENCIL_REASON.deleting;
  if (snapshot.writable === false) return RENAME_PENCIL_REASON.storage;
  if (!editorAvailable) return RENAME_PENCIL_REASON.unavailable;
  return pencilRenameRequest(snapshot, macroId)
    ? ''
    : RENAME_PENCIL_REASON.unavailable;
}
