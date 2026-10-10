import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from 'react';
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
import {
  ACTION,
  type LibraryAction,
  type SelectionChange,
} from './library-model';
import { useNativeConfigure } from './native-configure';
import { CONFIGURE_TEXT, type ConfigureCompletion } from './configure-contract';
import { WINDOW_COMMAND, type WindowViewRequest } from './compact-contract';
import {
  isCompactPhase,
  isCompactView,
  presentedLibrary,
} from './compact-presentation';
import { useCompactWindow, useSessionPresentation } from './native-compact';
import { DEV_TEXT } from './library-samples';

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
  /** @param id Confirmed stable identity. @returns Committed library metadata, or a disk/mode rejection. */
  delete: (id) =>
    invoke<BackendLibraryState>(LIBRARY_COMMAND.delete, { macroId: id }),
};
const ACTION_CALLBACK_FAILURE = 'Library action callback failed';

/**
 * Presents the executable-relative saved library with worker-driven progress and persistent error toasts.
 * @param props Optional playback snapshot consumer; property saves are backend-owned.
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
  const session = useSessionPresentation();
  const { controller: windowController, view } = useCompactWindow();
  const configure = useNativeConfigure(
    /** @param completion Authoritative native save receipt. @returns Nothing; applies metadata before announcing success. */
    (completion: ConfigureCompletion) => controller.configured(completion),
    /** @param reason Failed result query. @returns Nothing; reports uncertainty without claiming that disk persistence failed. */
    (reason: unknown) =>
      windowController.report(`${CONFIGURE_TEXT.uncertain} ${String(reason)}`),
  );
  const desiredCompact = isCompactPhase(session.phase);
  const presentation = presentedLibrary(snapshot, session, view);
  const compact = isCompactView(presentation);
  const layout = useCallback(
    /** @param request Measured logical content. @returns Nothing; the serialized controller converts native rejections to timed toasts. */
    (request: WindowViewRequest) => {
      void windowController.update({ ...request, compact: desiredCompact });
    },
    [windowController, desiredCompact],
  );
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
  /** @param macroId Deliberately confirmed stable ID. @returns Worker deletion completion; outcomes are controller-owned. */
  function confirmDelete(macroId: string) {
    return controller.deleteConfirmed(macroId);
  }
  /** @param request Available operation. @returns Nothing; asynchronously supplies data to later-story consumer. */
  function action(request: LibraryAction) {
    if (configure.busy) return;
    if (request.action === ACTION.configure) {
      void configure.open(request).catch(
        /** @param reason Snapshot/window creation failure. @returns Nothing; reports actionable feedback without losing selection. */
        (reason: unknown) =>
          windowController.report(
            `${CONFIGURE_TEXT.openFailure}: ${String(reason)}`,
          ),
      );
      return;
    }
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
    <div className="app-layout">
      <div className="product-column">
        {import.meta.env.DEV && !compact && (
          <div className="development-toggle">
            <button
              type="button"
              onClick={
                /** Opens separate native controls; development-command failures are reported through the transition toast queue. */
                () => {
                  void invoke<void>(WINDOW_COMMAND.openPreview).catch(
                    /** @param reason Preview opening failure. @returns Nothing; reports it without persistent recovery controls. */
                    (reason: unknown) =>
                      windowController.report(String(reason)),
                  );
                }
              }
            >
              {DEV_TEXT.open}
            </button>
          </div>
        )}
        <LibraryView
          snapshot={{ ...presentation, confirmationOpen: configure.busy }}
          onSelect={select}
          onAction={action}
          onDeleteConfirmed={confirmDelete}
          onLayout={layout}
          notifications={
            <>
              <OutcomeToasts queue={controller.queue} compact={compact} />
              <OutcomeToasts queue={windowController.failures} />
            </>
          }
        />
      </div>
    </div>
  );
}
