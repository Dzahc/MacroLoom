import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  LibraryController,
  type LibraryTransport,
  type PreparedAction,
} from './library-controller';
import { LibraryView } from './library-view';
import { OutcomeToasts } from './outcome-toasts';
import {
  LIBRARY_COMMAND,
  LIBRARY_EVENT,
  type BackendLibraryState,
  type MacroDocument,
} from './macro-contract';
import type { LibraryAction, SelectionChange } from './library-model';

const NATIVE_TRANSPORT: LibraryTransport = {
  /** @param receive Revisioned state subscriber. @returns Native event listener cleanup. */
  listen: (receive) =>
    listen<BackendLibraryState>(
      LIBRARY_EVENT,
      /** @param event Typed backend snapshot. */ (event) =>
        receive(event.payload),
    ),
  /** @returns Final discovery snapshot; progress is delivered through the subscribed event. */
  load: () => invoke<BackendLibraryState>(LIBRARY_COMMAND.load),
  /** @param id Loaded stable identity. @returns Validated action-owned document, or native rejection. */
  read: (id) =>
    invoke<MacroDocument>(LIBRARY_COMMAND.snapshot, { macroId: id }),
};
const ACTION_CALLBACK_FAILURE = 'Library action callback failed';

/**
 * Presents the executable-relative saved library with worker-driven progress and persistent error toasts.
 * @param props Optional later-story consumer for prepared Configure/Delete/Play snapshots.
 * @returns The library view; native listeners and queue timers disconnect on unmount.
 */
export function LiveLibraryApp({
  onPreparedAction,
}: {
  onPreparedAction?: (action: PreparedAction) => void;
}) {
  const [controller] = useState(
    /** @returns One controller for this mounted application. */ () =>
      new LibraryController(NATIVE_TRANSPORT),
  );
  const snapshot = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  const mounted = useRef(false);
  useEffect(
    /** Connects native progress and returns owner cleanup. */ () => {
      mounted.current = true;
      const disconnect = controller.connect();
      /** Disconnects pending callbacks before releasing owned timers. */
      return () => {
        mounted.current = false;
        disconnect();
        controller.dispose();
      };
    },
    [controller],
  );

  /** @param change Clicked stable ID/source. @returns Nothing; applies valid selection. */
  function select(change: SelectionChange) {
    controller.select(change);
  }
  /** @param request Available operation. @returns Nothing; asynchronously supplies data to later-story consumer. */
  function action(request: LibraryAction) {
    void controller
      .prepare(request)
      .then(
        /** @param prepared Validated ID/document, or null after a reported failure. */
        (prepared) => {
          if (mounted.current && prepared && onPreparedAction)
            onPreparedAction(prepared);
        },
      )
      .catch(
        /** @param reason Consumer failure; report without leaving an unhandled promise rejection. */
        (reason: unknown) => {
          if (mounted.current) console.error(ACTION_CALLBACK_FAILURE, reason);
        },
      );
  }
  return (
    <LibraryView
      snapshot={snapshot}
      onSelect={select}
      onAction={action}
      notifications={<OutcomeToasts queue={controller.queue} />}
    />
  );
}
