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
  reconcileSelection,
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
import { useNativeSession } from './native-session';
import { SessionControls } from './session-controls';
import { SESSION_COMMAND } from './session-contract';
import { useNativeRename } from './native-rename';
import {
  ConfigureRecoveryControls,
  useConfigureRecovery,
} from './configure-recovery-controls';

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
  const nativeSession = useNativeSession();
  const [controller] = useState(
    /** @returns One controller for this mounted application. */ () =>
      new LibraryController({
        ...NATIVE_TRANSPORT,
        select: nativeSession.controller.select,
        /** @param id Explicitly confirmed macro. @returns Deletion result with native confirmation ownership transferred atomically. */
        delete: (id) =>
          invoke<BackendLibraryState>(LIBRARY_COMMAND.delete, {
            macroId: id,
            confirmationOwner: nativeSession.controller.confirmationOwner,
          }),
      }),
  );
  const snapshot = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  const rename = useNativeRename(controller, nativeSession.snapshot);
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
  const recovery = useConfigureRecovery();
  const configureBusy = configure.busy || recovery.snapshot.occupied;
  const desiredCompact = isCompactPhase(session.phase);
  const presentation = {
    ...presentedLibrary(snapshot, session, view),
    selectedId: reconcileSelection(
      snapshot.macros,
      nativeSession.snapshot.selectedMacroId,
    ),
    shortcuts: nativeSession.snapshot.shortcuts,
    playGesture: nativeSession.snapshot.playGesture,
    message: snapshot.loading
      ? snapshot.message
      : nativeSession.snapshot.message,
    renameSaving:
      nativeSession.snapshot.renameSaving || rename.snapshot.pending,
    deferredStart: nativeSession.snapshot.deferredStart,
    recordingAvailable: nativeSession.snapshot.canRecord,
    playbackAvailable: nativeSession.snapshot.canPlay,
  };
  const compact = isCompactView(presentation);
  const layout = useCallback(
    /** @param request Measured logical content. @returns Nothing; the serialized controller converts native rejections to timed toasts. */
    (request: WindowViewRequest) => {
      void windowController.update({ ...request, compact: desiredCompact });
    },
    [windowController, desiredCompact],
  );
  useEffect(
    /** Reconciles the durable commit receipt before selecting its entry; failures preserve existing selection. */
    () => controller.recorded(nativeSession.snapshot.recordingOutcome),
    [controller, nativeSession.snapshot.recordingOutcome],
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
    const owner = nativeSession.controller.confirmationOwner;
    return controller.deleteConfirmed(macroId).finally(
      /** Retires the original dialog owner after worker completion; a newer dialog has a distinct token. */
      () => nativeSession.controller.confirmationDone(owner),
    );
  }
  const openDelete = useCallback(
    /** Claims the native confirmation interlock before the dialog appears. @returns Whether confirmation may open. */
    () => nativeSession.controller.setConfirmation(true),
    [nativeSession.controller],
  );
  /** Releases the native reservation on Cancel or dialog-owner cleanup; errors stay visible. */
  const cancelDelete = useCallback(
    /** Releases this owner's native confirmation on Cancel or dialog unmount; late owners cannot release a newer claim. */
    () => {
      void nativeSession.controller.setConfirmation(false);
    },
    [nativeSession.controller],
  );
  /** @param request Available operation. @returns Nothing; asynchronously supplies data to later-story consumer. */
  function action(request: LibraryAction) {
    if (request.action === ACTION.record || request.action === ACTION.stop) {
      const command =
        request.action === ACTION.record
          ? SESSION_COMMAND.record
          : SESSION_COMMAND.stop;
      void nativeSession.controller.request(command);
      return;
    }
    if (configureBusy) return;
    if (request.action === ACTION.play) {
      void nativeSession.controller.play(request.macroId);
      return;
    }
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
        <ConfigureRecoveryControls controller={recovery.controller} />
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
          snapshot={{ ...presentation, confirmationOpen: configureBusy }}
          rename={rename}
          onSelect={select}
          onAction={action}
          onDeleteConfirmed={confirmDelete}
          onDeleteOpening={openDelete}
          onDeleteCancelled={cancelDelete}
          onDeleteDisposed={cancelDelete}
          sessionControls={
            <SessionControls
              snapshot={nativeSession.snapshot}
              controller={nativeSession.controller}
            />
          }
          onLayout={layout}
          notifications={
            <>
              <OutcomeToasts queue={controller.queue} compact={compact} />
              <OutcomeToasts
                queue={nativeSession.controller.queue}
                compact={compact}
              />
              <OutcomeToasts queue={windowController.failures} />
            </>
          }
        />
      </div>
    </div>
  );
}
