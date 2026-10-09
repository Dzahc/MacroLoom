import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ConfigureForm } from './configure-form';
import { ConfigureController } from './configure-controller';
import {
  CONFIGURE_COMMAND,
  CONFIGURE_TEXT,
  type ConfigureDraft,
  type ConfigureResult,
  type ConfigureSnapshot,
} from './configure-contract';

/** @param draft Complete locally validated properties. @returns Native-validated async consumer outcome; no persistence is performed here. */
function submit(draft: ConfigureDraft): Promise<ConfigureResult> {
  return invoke<ConfigureResult>(CONFIGURE_COMMAND.submit, { draft });
}

/** @returns Native editor loaded from its backend-owned snapshot, with errors retained and late results ignored after teardown. */
export function NativeConfigureApp() {
  const [controller, setController] = useState<ConfigureController | null>(
    null,
  );
  const [error, setError] = useState('');
  useEffect(
    /** Loads only this editor's trusted snapshot and returns cleanup suppressing late UI updates. */ () => {
      let disposed = false;
      void invoke<ConfigureSnapshot>(CONFIGURE_COMMAND.snapshot)
        .then(
          /** @param snapshot Supplied native baseline. @returns Nothing. */ (
            snapshot,
          ) => {
            if (!disposed)
              setController(new ConfigureController(snapshot, submit));
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
      };
    },
    [],
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
  if (error)
    return (
      <main className="configure-sample">
        <p role="alert">{error}</p>
        <button type="button" onClick={close}>
          {CONFIGURE_TEXT.cancel}
        </button>
      </main>
    );
  return controller ? (
    <ConfigureForm controller={controller} onClosed={close} />
  ) : (
    <p role="status">{CONFIGURE_TEXT.loading}</p>
  );
}
