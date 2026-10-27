import { useEffect, useState, useSyncExternalStore } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { CONFIGURE_COMMAND, CONFIGURE_TEXT } from './configure-contract';
import {
  ConfigureRecoveryController,
  type ConfigureRecoveryStatus,
  type ConfigureRecoveryReceipt,
} from './configure-recovery';

/** @returns Authoritative native recovery state and actions; the polling timer disconnects on unmount. */
export function useConfigureRecovery() {
  const [controller] = useState(
    /** Creates one native transport for this mounted main window. */ () =>
      new ConfigureRecoveryController({
        /** @returns Native visibility-recovery ownership and pending-save guard. */
        status: () =>
          invoke<ConfigureRecoveryStatus>(CONFIGURE_COMMAND.recoveryStatus),
        /** @returns Placement result for the same editor; never submits Save. */
        retry: () =>
          invoke<ConfigureRecoveryStatus>(CONFIGURE_COMMAND.retryPlacement),
        /** @returns Backend-guarded explicit draft dismissal. */
        discard: (observedAttemptId) =>
          invoke<ConfigureRecoveryStatus>(CONFIGURE_COMMAND.discardPlacement, {
            observedAttemptId,
          }),
      }),
  );
  const snapshot = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  useEffect(
    /** Connects read-only polling and returns timer/late-response cleanup. */ () =>
      controller.connect(),
    [controller],
  );
  return { controller, snapshot };
}

/** @param completion Authoritative receipt returned to the recovering main window. @returns Explicit saved/failed outcome before Discard can carry its attempt identity. */
function saveOutcome(completion: ConfigureRecoveryReceipt): string {
  const result = completion.result;
  return result.ok
    ? `${CONFIGURE_TEXT.placementSaved} ${result.warning ?? ''}`
    : `${CONFIGURE_TEXT.placementNotSaved} ${result.message}`;
}

/** @param props Main-window native recovery controller. @returns Persistent reachable recovery actions while the original editor retains its draft. */
export function ConfigureRecoveryControls({
  controller,
}: {
  controller: ConfigureRecoveryController;
}) {
  const state = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  if (!state.needed) return null;
  return (
    <section
      className="configure-placement-recovery"
      aria-label={CONFIGURE_TEXT.placementRecovery}
    >
      <p role="alert">
        {state.message} {CONFIGURE_TEXT.placementRetained}
      </p>
      {state.error && <p role="alert">{state.error}</p>}
      {state.pending && <p role="status">{CONFIGURE_TEXT.placementPending}</p>}
      {state.completion && <p role="status">{saveOutcome(state.completion)}</p>}
      <button
        type="button"
        disabled={state.working}
        onClick={
          /** Retries native placement without saving or replacing the draft. */ () => {
            void controller.retry();
          }
        }
      >
        {CONFIGURE_TEXT.placementRetry}
      </button>
      <button
        type="button"
        disabled={state.working || state.pending || Boolean(state.error)}
        onClick={
          /** Explicitly dismisses only an idle failed-placement editor. */ () => {
            void controller.discard();
          }
        }
      >
        {CONFIGURE_TEXT.placementDiscard}
      </button>
    </section>
  );
}
