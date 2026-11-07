import { ActionIcon } from './action-icon';
import {
  ACTION,
  LABEL,
  SOURCE,
  canRequest,
  requestAction,
  type LibraryCallbacks,
  type LibrarySnapshot,
} from './library-model';

/** @param props Selected macro, current toolbar availability, and guarded action receiver. @returns An independent Configure button; activation never selects or plays a row. */
export function ConfigureShortcut({
  snapshot,
  macroId,
  onAction,
}: {
  snapshot: LibrarySnapshot;
  macroId: string;
  onAction: LibraryCallbacks['onAction'];
}) {
  return (
    <span className="macro-configure-shortcut" title={LABEL.configure}>
      <button
        type="button"
        className="macro-configure-button"
        data-action={ACTION.configure}
        data-configure-id={macroId}
        aria-label={LABEL.configure}
        disabled={!canRequest(snapshot, ACTION.configure, macroId)}
        onClick={
          /** @param event Configure activation. @returns Nothing; forwards the existing explicit-ID request and isolates the row gesture. */
          (event) => {
            event.stopPropagation();
            const request = requestAction(
              snapshot,
              ACTION.configure,
              SOURCE.row,
              macroId,
            );
            if (request) onAction(request);
          }
        }
      >
        <ActionIcon action={ACTION.configure} />
      </button>
    </span>
  );
}
