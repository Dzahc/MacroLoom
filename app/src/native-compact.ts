import { useEffect, useState, useSyncExternalStore } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { CompactWindowController } from './compact-window-controller';
import {
  IDLE_PRESENTATION,
  PRESENTATION_EVENT,
  WINDOW_COMMAND,
  WINDOW_VIEW_EVENT,
  type SessionPresentation,
  type WindowViewResult,
} from './compact-contract';

const SUBSCRIPTION_FAILURE = 'Could not subscribe to window presentation';

/** @returns Revisioned backend presentation; subscribes before loading and releases late listeners on cleanup. */
export function useSessionPresentation(): SessionPresentation {
  const [session, setSession] = useState(IDLE_PRESENTATION);
  useEffect(
    /** Connects controlled/native status with listener-first startup and cancellation-safe promise handling. */
    () => {
      let disposed = false;
      let unlisten: (() => void) | undefined;
      /** @param snapshot Revisioned status. @returns Nothing; late results cannot rewind the visible phase. */
      function receive(snapshot: SessionPresentation) {
        if (!disposed)
          setSession(
            /** @param current Latest applied state. @returns The newer state without mutating it. */
            (current) =>
              snapshot.revision >= current.revision ? snapshot : current,
          );
      }
      void listen<SessionPresentation>(
        PRESENTATION_EVENT,
        /** @param event Typed native status delivery. @returns Nothing. */
        (event) => receive(event.payload),
      )
        .then(
          /** @param cleanup Listener release. @returns Snapshot loading unless the owner has already disconnected. */
          async (cleanup) => {
            if (disposed) {
              cleanup();
              return;
            }
            unlisten = cleanup;
            receive(await invoke<SessionPresentation>(WINDOW_COMMAND.snapshot));
          },
        )
        .catch(
          /** @param reason Native subscription failure. @returns Nothing; status remains safe idle and diagnostics are logged. */
          (reason: unknown) => {
            if (!disposed) console.error(SUBSCRIPTION_FAILURE, reason);
          },
        );
      /** Disconnects current or pending listeners without starting/stopping a session. */
      return () => {
        disposed = true;
        if (unlisten) unlisten();
      };
    },
    [],
  );
  return session;
}

/** @returns One mounted native transition owner and its actual-layout store; queue timers disconnect on cleanup. */
export function useCompactWindow() {
  const [controller] = useState(
    /** Creates a serialized main-window native boundary without activating it. */
    () =>
      new CompactWindowController(
        /** @param request Logical measured content. @returns Native actual mode and optional transition error. */
        (request) => invoke<WindowViewResult>(WINDOW_COMMAND.view, { request }),
      ),
  );
  const view = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  useEffect(
    /** Reconnects the owner across StrictMode and releases pending callbacks/timers on cleanup. */
    () => {
      controller.activate();
      return () => controller.dispose();
    },
    [controller],
  );
  useEffect(
    /** Subscribes native movement/DPI results without allowing late listener registration to survive cleanup. */
    () => {
      let disposed = false;
      let unlisten: (() => void) | undefined;
      void listen<WindowViewResult>(
        WINDOW_VIEW_EVENT,
        /** @param event Actual native layout correction. @returns Nothing. */
        (event) => {
          if (!disposed) controller.receive(event.payload);
        },
      )
        .then(
          /** @param cleanup Native subscription cleanup. @returns Nothing; releases immediately after an abandoned registration. */
          (cleanup) => {
            if (disposed) cleanup();
            else unlisten = cleanup;
          },
        )
        .catch(
          /** @param reason Native event subscription failure. @returns Nothing; reports a timed failure while mounted. */
          (reason: unknown) => {
            if (!disposed) controller.report(String(reason));
          },
        );
      /** Releases current and late native-view listeners independently of progress subscriptions. */
      return () => {
        disposed = true;
        if (unlisten) unlisten();
      };
    },
    [controller],
  );
  return { controller, view };
}
