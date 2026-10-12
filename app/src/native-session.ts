import { useEffect, useState, useSyncExternalStore } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { SessionController } from './session-controller';
import { SESSION_EVENT, type SessionSnapshot } from './session-contract';

/** @returns A mounted native session store and controls; StrictMode/unmount cleans current and pending subscriptions. */
export function useNativeSession() {
  const [controller] = useState(
    /** Creates one listener/action reconciler for this owner. */
    () =>
      new SessionController({
        /** @param receive Native subscriber. @returns Event unsubscription. */
        listen: (receive) =>
          listen<SessionSnapshot>(
            SESSION_EVENT,
            /** @param event Backend state. @returns Nothing; forwards the typed snapshot. */
            (event) => receive(event.payload),
          ),
        /** @param command Native control. @param args Typed command payload. @returns Authoritative state. */
        invoke: (command, args) => invoke<SessionSnapshot>(command, args),
      }),
  );
  const snapshot = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  useEffect(
    /** Connects native lifecycle before querying the initial state; cleanup starts/stops no native session. */
    () => controller.connect(),
    [controller],
  );
  return { controller, snapshot };
}
