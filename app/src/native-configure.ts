import {
  useCallback,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  CONFIGURE_COMMAND,
  CONFIGURE_EVENT,
  type ConfigureCompletion,
  type ConfigureStatus,
} from './configure-contract';
import { ACTION, SOURCE, type LibraryAction } from './library-model';

const FOCUS_TARGET = {
  toolbar: `[data-action="${ACTION.configure}"]`,
  rows: '[data-macro-id]',
  fallback: '.macro-library',
} as const;
const LISTENER_FAILED = 'Configure dismissal listener registration failed';

/** @param request Original opening control. @returns Nothing; restores actual control focus after the native owner is enabled. */
function restoreFocus(request: LibraryAction | null): void {
  if (!request || !document.hasFocus()) return;
  let target = document.querySelector<HTMLElement>(FOCUS_TARGET.toolbar);
  if ('macroId' in request && request.source !== SOURCE.toolbar) {
    target =
      Array.from(
        document.querySelectorAll<HTMLElement>(FOCUS_TARGET.rows),
      ).find(
        /** @param row Mounted library row. @returns Whether its stable identity opened Configure. */ (
          row,
        ) => row.dataset.macroId === request.macroId,
      ) ?? null;
  }
  (target ?? document.querySelector<HTMLElement>(FOCUS_TARGET.fallback))?.focus(
    { preventScroll: true },
  );
}

/** @param completed Native authoritative completion subscriber. @param failed Recovery failure reporter. @returns Singleton opening callback and modal availability; listeners disconnect on teardown. */
export function useNativeConfigure(
  completed: (completion: ConfigureCompletion) => void,
  failed: (reason: unknown) => void,
) {
  const [busy, setBusy] = useState(false);
  const ready = useRef<Promise<void> | null>(null);
  const origin = useRef<LibraryAction | null>(null);
  const opening = useRef(false);
  const accept = useEffectEvent(completed);
  const report = useEffectEvent(failed);
  useEffect(
    /** Subscribes before opening an editor and cleans up registrations that complete after teardown. */ () => {
      let disposed = false;
      let focusFrame: number | undefined;
      const cleanups: (() => void)[] = [];
      /** @returns Native completion after dismissal; disconnected owners ignore late delivery, and querying never submits a save. */
      async function reconcile() {
        const status = await invoke<ConfigureStatus>(CONFIGURE_COMMAND.status);
        if (!disposed && status.completion) accept(status.completion);
      }
      /** @param cleanup Registered listener. @returns Nothing; disconnected owners release late registrations immediately. */
      function registered(cleanup: () => void) {
        if (disposed) cleanup();
        else cleanups.push(cleanup);
      }
      const registration = Promise.all([
        listen<void>(
          CONFIGURE_EVENT.closed,
          /** Releases the view interlock and restores the initiating actual control. */ () => {
            if (disposed) return;
            const dismissed = origin.current;
            void reconcile()
              .catch(
                /** @param reason Read-only result retrieval failure. @returns Nothing; records uncertainty without claiming a failed commit. */
                (reason: unknown) => {
                  if (!disposed) report(reason);
                },
              )
              .finally(
                /** Reenables controls only after result retrieval, preventing a new editor from replacing the receipt mid-query. */
                () => {
                  if (disposed) return;
                  opening.current = false;
                  setBusy(false);
                  if (focusFrame !== undefined)
                    cancelAnimationFrame(focusFrame);
                  focusFrame = requestAnimationFrame(
                    /** Restores the initiating control after disabled attributes update, without stealing external focus. */
                    () => {
                      if (!disposed && !opening.current)
                        restoreFocus(dismissed);
                    },
                  );
                },
              );
          },
        ).then(registered),
      ]).then(
        /** Completes dismissal listener registration before native editor creation. */ () => {},
      );
      ready.current = registration;
      void registration.catch(
        /** @param reason Listener registration failure. @returns Nothing; opening remains unavailable. */ (
          reason: unknown,
        ) => console.error(LISTENER_FAILED, reason),
      );
      /** Disconnects subscribers and prevents late receipt delivery; the native lifecycle retains ownership during Save. */
      return () => {
        disposed = true;
        if (focusFrame !== undefined) cancelAnimationFrame(focusFrame);
        cleanups.forEach(
          /** @param cleanup Native listener release. @returns Nothing. */ (
            cleanup,
          ) => cleanup(),
        );
      };
    },
    [],
  );
  const open = useCallback(
    /** @param request Available Configure request. @returns Completion of native creation; failures release presentation availability and propagate for feedback. */ async (
      request: LibraryAction,
    ) => {
      if (!('macroId' in request) || opening.current) return;
      opening.current = true;
      origin.current = request;
      setBusy(true);
      try {
        await ready.current;
        await invoke<void>(CONFIGURE_COMMAND.open, {
          macroId: request.macroId,
        });
      } catch (reason) {
        opening.current = false;
        setBusy(false);
        throw reason;
      }
    },
    [],
  );
  return { busy, open };
}
