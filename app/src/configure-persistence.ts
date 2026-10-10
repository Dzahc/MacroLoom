import {
  CONFIGURE_TEXT,
  type ConfigureDraft,
  type ConfigureResult,
  type ConfigureStatus,
} from './configure-contract.ts';

export const CONFIGURE_RECOVERY_DELAY_MS = 100;
const RECOVERY_ABORTED = 'Configure recovery canceled';
const UNKNOWN_RESULT = {
  ok: false,
  message: CONFIGURE_TEXT.uncertain,
  uncertain: true,
} as const;
export type ConfigurePersistence = {
  submit: (draft: ConfigureDraft) => Promise<ConfigureResult>;
  status: () => Promise<ConfigureStatus>;
  wait: () => Promise<void>;
};

/** @param draft Complete editor values. @param transport Native submission and read-only recovery. @param recovering Whether Retry must reconcile an earlier unknown outcome before any write. @returns Authoritative result; pending/unavailable recovery never unlocks edits or submits again. */
export async function persistConfigure(
  draft: ConfigureDraft,
  transport: ConfigurePersistence,
  recovering = false,
): Promise<ConfigureResult> {
  if (recovering) {
    try {
      const result = await recoveredResult(transport);
      if (result) return result;
    } catch {
      return UNKNOWN_RESULT;
    }
  }
  try {
    return await transport.submit(draft);
  } catch {
    try {
      return (await recoveredResult(transport)) ?? UNKNOWN_RESULT;
    } catch {
      return UNKNOWN_RESULT;
    }
  }
}

/** @param transport Native read-only boundary. @returns Finished attempt, or null if no attempt began; pending work is polled without resubmitting. */
async function recoveredResult(
  transport: ConfigurePersistence,
): Promise<ConfigureResult | null> {
  let status = await transport.status();
  while (status.pending) {
    await transport.wait();
    status = await transport.status();
  }
  return status.completion?.result ?? null;
}

/** @param signal Editor lifetime. @returns Completion after one recovery delay, or rejection on teardown; every timer/listener is released. */
export function waitForConfigure(signal: AbortSignal): Promise<void> {
  return new Promise(
    /** @param resolve Delay completion. @param reject Teardown rejection. @returns Nothing; installs an abortable timer. */
    (resolve, reject) => {
      if (signal.aborted) {
        reject(new Error(RECOVERY_ABORTED));
        return;
      }
      /** Cancels a pending recovery timer when the editor unmounts. */
      function aborted() {
        clearTimeout(timer);
        reject(new Error(RECOVERY_ABORTED));
      }
      const timer = setTimeout(
        /** Releases the abort registration before continuing the read-only status query. */
        () => {
          signal.removeEventListener('abort', aborted);
          resolve();
        },
        CONFIGURE_RECOVERY_DELAY_MS,
      );
      signal.addEventListener('abort', aborted, { once: true });
    },
  );
}
