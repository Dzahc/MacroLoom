import { useEffect, useState, useSyncExternalStore } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { RenameController } from './rename-controller';
import {
  RENAME_COMMAND,
  type RenameCompletion,
  type RenameStatus,
} from './rename-contract';
import type { LibraryController } from './library-controller';
import type { SessionSnapshot } from './session-contract';

/** @param library Committed library/outcome owner. @param session Backend lifecycle. @returns Disposable inline rename state and typed native actions. */
export function useNativeRename(
  library: LibraryController,
  session: SessionSnapshot,
) {
  const [controller] = useState(
    /** Binds one persistence owner; no native work starts during rendering. */ () =>
      new RenameController(
        {
          /** @returns Recoverable native status without any disk mutation. */
          status: () => invoke<RenameStatus>(RENAME_COMMAND.status),
          /** @param input Name-only draft and monotonic attempt identity. @returns Authoritative disk receipt. */
          submit: (input) =>
            invoke<RenameCompletion>(RENAME_COMMAND.submit, { input }),
        },
        /** @param completion Native outcome, reconciled independently of Configure's attempt sequence. */ (
          completion,
        ) => library.renamed(completion),
      ),
  );
  const snapshot = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  useEffect(
    /** Applies external start intent even when native startup subsequently fails. */ () =>
      controller.observe(session),
    [controller, session],
  );
  useEffect(
    /** Returns unmount cleanup for pending status polls and IPC deliveries. */ () =>
      /** Releases this mounted editor's resources. */ () =>
        controller.dispose(),
    [controller],
  );
  return {
    snapshot,
    open: controller.open,
    change: controller.change,
    save: controller.save,
    cancel: controller.cancel,
  };
}
