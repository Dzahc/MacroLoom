import { ActionIcon } from './action-icon';
import {
  ACTION,
  LABEL,
  type LibraryCallbacks,
  type LibrarySnapshot,
} from './library-model';
import { pencilRenameRequest, renamePencilReason } from './rename-pencil-model';

/** @param props Selected row, current availability, and the existing guarded action receiver. @returns A separate pencil button whose clicks cannot bubble into row playback. */
export function RenamePencil({
  snapshot,
  macroId,
  editorAvailable,
  onAction,
}: {
  snapshot: LibrarySnapshot;
  macroId: string;
  editorAvailable: boolean;
  onAction: LibraryCallbacks['onAction'];
}) {
  const reason = renamePencilReason(snapshot, macroId, editorAvailable);
  const disabled = Boolean(reason);
  return (
    <span className="macro-rename-pencil" title={reason || undefined}>
      <button
        type="button"
        className="macro-rename-button"
        data-action={ACTION.rename}
        aria-label={LABEL.rename}
        disabled={disabled}
        onClick={
          /** @param event Pencil activation. @returns Nothing; dispatches the explicit row ID without a selection or playback gesture. */
          (event) => {
            event.stopPropagation();
            if (disabled) return;
            const request = pencilRenameRequest(snapshot, macroId);
            if (request) onAction(request);
          }
        }
      >
        <ActionIcon action={ACTION.rename} />
      </button>
    </span>
  );
}
