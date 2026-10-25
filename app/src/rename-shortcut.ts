import { useEffect } from 'react';
import {
  ACTION,
  canRequest,
  selectedMacro,
  type LibrarySnapshot,
} from './library-model';
import { RENAME_KEY, type RenameBinding } from './rename-contract';

const KEY_DOWN = 'keydown';

/** @param snapshot Current selected row and backend availability. @param rename Optional editor owner. @returns Nothing; releases the document shortcut when its owner changes. */
export function useRenameShortcut(
  snapshot: LibrarySnapshot,
  rename?: RenameBinding,
): void {
  const open = rename?.open;
  const editing = Boolean(rename?.snapshot.draft);
  useEffect(
    /** Registers selected-row F2 without changing existing row click or playback gestures. */ () => {
      /** @param event Document keyboard gesture. @returns Nothing; text editing and existing dialogs keep ownership. */
      function key(event: KeyboardEvent) {
        if (event.key !== RENAME_KEY.open || event.repeat || editing || !open)
          return;
        if (!canRequest(snapshot, ACTION.rename)) return;
        const macro = selectedMacro(snapshot);
        if (macro) {
          event.preventDefault();
          open(macro);
        }
      }
      document.addEventListener(KEY_DOWN, key);
      /** Removes this selected-row shortcut listener. */
      return () => document.removeEventListener(KEY_DOWN, key);
    },
    [snapshot, open, editing],
  );
}
