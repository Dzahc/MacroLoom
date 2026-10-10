import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ConfigureForm } from './configure-form';
import { ConfigureController } from './configure-controller';
import {
  CONFIGURE_COMMAND,
  CONFIGURE_TEXT,
  CONFIGURE_OUTCOME,
  type ConfigureDraft,
  type ConfigureResult,
  type ConfigureSnapshot,
  type ConfigureStatus,
} from './configure-contract';
import { persistConfigure, waitForConfigure } from './configure-persistence';
import { OutcomeQueue } from './outcome-queue';
import { OutcomeToasts } from './outcome-toasts';

/** @param draft Complete locally validated properties. @param signal Editor lifetime. @param queue Owned failure notifications. @param recovering Whether to query the previous unknown result before submitting. @returns Native commit result or recovered receipt; only precommit failures are reported as failed saves. */
async function submit(
  draft: ConfigureDraft,
  signal: AbortSignal,
  queue: OutcomeQueue,
  recovering = false,
): Promise<ConfigureResult> {
  const result = await persistConfigure(
    draft,
    {
      /** @param value Complete draft. @returns Backend-owned disk outcome. */
      submit: (value) =>
        invoke<ConfigureResult>(CONFIGURE_COMMAND.submit, { draft: value }),
      /** @returns Read-only native completion, independent of acknowledgement delivery. */
      status: () => invoke<ConfigureStatus>(CONFIGURE_COMMAND.status),
      /** @returns An abortable delay while the worker remains pending. */
      wait: () => waitForConfigure(signal),
    },
    recovering,
  );
  if (!signal.aborted && !result.ok) {
    queue.enqueue({
      id: performance.now(),
      kind: CONFIGURE_OUTCOME.failure,
      message: `${result.uncertain ? CONFIGURE_TEXT.saveStatus : CONFIGURE_TEXT.updateFailed} “${draft.name}”: ${result.message}`,
    });
  }
  return result;
}

/** @returns Native editor loaded from its backend-owned snapshot, with errors retained and late results ignored after teardown. */
export function NativeConfigureApp() {
  const [controller, setController] = useState<ConfigureController | null>(
    null,
  );
  const [error, setError] = useState('');
  const [queue] = useState(
    /** @returns One timer-owning toast queue for this editor. */ () =>
      new OutcomeQueue(),
  );
  useEffect(
    /** Loads only this editor's trusted snapshot and returns cleanup suppressing late UI updates. */ () => {
      let disposed = false;
      const lifetime = new AbortController();
      void invoke<ConfigureSnapshot>(CONFIGURE_COMMAND.snapshot)
        .then(
          /** @param snapshot Supplied native baseline. @returns Nothing. */ (
            snapshot,
          ) => {
            if (!disposed)
              setController(
                new ConfigureController(
                  snapshot,
                  /** @param draft Editable values. @param recovering Whether a previous outcome must be queried first. @returns Native save/recovery completion for this editor lifetime. */
                  (draft, recovering) =>
                    submit(draft, lifetime.signal, queue, recovering),
                ),
              );
          },
        )
        .catch(
          /** @param reason Failed native load. @returns Nothing. */ (
            reason: unknown,
          ) => {
            if (!disposed) setError(String(reason));
          },
        );
      /** Ignores results delivered after the owned window unmounts. */
      return () => {
        disposed = true;
        lifetime.abort();
        queue.dispose();
      };
    },
    [queue],
  );
  const close = useCallback(
    /** Closes only the owned editor; native lifecycle restores the main window and rejects pending closure. */ () => {
      void invoke<void>(CONFIGURE_COMMAND.close).catch(
        /** @param reason Closure failure. @returns Nothing; exposes the native error. */ (
          reason: unknown,
        ) => setError(String(reason)),
      );
    },
    [],
  );
  if (error && !controller)
    return (
      <main className="configure-sample">
        <p role="alert">{error}</p>
        <button type="button" onClick={close}>
          {CONFIGURE_TEXT.cancel}
        </button>
      </main>
    );
  return controller ? (
    <>
      {error && (
        <p className="configure-error" role="alert">
          {error}
        </p>
      )}
      <ConfigureForm controller={controller} onClosed={close} />
      <OutcomeToasts queue={queue} />
    </>
  ) : (
    <p role="status">{CONFIGURE_TEXT.loading}</p>
  );
}
