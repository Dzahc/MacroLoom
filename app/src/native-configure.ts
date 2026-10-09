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
  CONFIGURE_TEXT,
  type ConfigureSave,
  type ConfigureSubmission,
  type ConfigureResult,
} from './configure-contract';
import { ACTION, SOURCE, type LibraryAction } from './library-model';

const FOCUS_TARGET = {
  toolbar: `[data-action="${ACTION.configure}"]`,
  rows: '[data-macro-id]',
  fallback: '.macro-library',
} as const;
const BRIDGE_FAILED = 'Configure callback bridge failed';

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

/** @returns A retained failure until the storage story supplies its consumer; never reports false saved data. */
export const unavailableConfigureSave: ConfigureSave = () =>
  Promise.resolve({ ok: false, message: CONFIGURE_TEXT.unavailable });

/** @param save Supplied asynchronous consumer, independent of native window and form state. @returns Singleton opening callback and modal availability, with all native listeners released on teardown. */
export function useNativeConfigure(
  save: ConfigureSave = unavailableConfigureSave,
) {
  const [busy, setBusy] = useState(false);
  const ready = useRef<Promise<void> | null>(null);
  const origin = useRef<LibraryAction | null>(null);
  const opening = useRef(false);
  const consume = useEffectEvent(
    /** @param submission Native-validated, once-per-attempt draft. @returns Nothing; rejected consumers receive a retained failure acknowledgement. */ async (
      submission: ConfigureSubmission,
    ) => {
      let result: ConfigureResult;
      try {
        result = await save(submission.draft);
      } catch {
        result = { ok: false, message: CONFIGURE_TEXT.failure };
      }
      try {
        await invoke<void>(CONFIGURE_COMMAND.resolve, {
          attemptId: submission.attemptId,
          result,
        });
      } catch (reason) {
        console.error(BRIDGE_FAILED, reason);
        // Malformed consumer responses must leave a recoverable failure instead of indefinite progress.
        await invoke<void>(CONFIGURE_COMMAND.resolve, {
          attemptId: submission.attemptId,
          result: { ok: false, message: CONFIGURE_TEXT.failure },
        }).catch(
          /** @param failure Disconnected bridge rejection. @returns Nothing; reports it without an unhandled promise. */
          (failure: unknown) => console.error(BRIDGE_FAILED, failure),
        );
      }
    },
  );
  useEffect(
    /** Subscribes before opening an editor and cleans up registrations that complete after teardown. */ () => {
      let disposed = false;
      let focusFrame: number | undefined;
      const cleanups: (() => void)[] = [];
      /** @param cleanup Registered listener. @returns Nothing; disconnected owners release late registrations immediately. */
      function registered(cleanup: () => void) {
        if (disposed) cleanup();
        else cleanups.push(cleanup);
      }
      const registration = Promise.all([
        listen<ConfigureSubmission>(
          CONFIGURE_EVENT.submit,
          /** @param event Validated submission. @returns Nothing. */ (
            event,
          ) => {
            if (!disposed) void consume(event.payload);
          },
        ).then(registered),
        listen<void>(
          CONFIGURE_EVENT.closed,
          /** Releases the view interlock and restores the initiating actual control. */ () => {
            if (disposed) return;
            opening.current = false;
            setBusy(false);
            const dismissed = origin.current;
            if (focusFrame !== undefined) cancelAnimationFrame(focusFrame);
            focusFrame = requestAnimationFrame(
              /** Waits until disabled attributes update, suppressing restoration after reopening/teardown. */ () => {
                if (!disposed && !opening.current) restoreFocus(dismissed);
              },
            );
          },
        ).then(registered),
      ]).then(
        /** Completes both registrations before native editor creation. */ () => {},
      );
      ready.current = registration;
      void registration.catch(
        /** @param reason Listener registration failure. @returns Nothing; opening remains unavailable. */ (
          reason: unknown,
        ) => console.error(BRIDGE_FAILED, reason),
      );
      /** Disconnects subscribers and prevents late callback delivery; the native lifecycle retains ownership during Save. */
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
