import {
  CONFIGURE_TEXT,
  type ConfigureDraft,
  type ConfigureResult,
  type ConfigureSave,
  type ConfigureSubmission,
} from './configure-contract.ts';

export type ConfigureDelivery = {
  claim: (attemptId: number) => Promise<ConfigureDraft>;
  resolve: (attemptId: number, result: ConfigureResult) => Promise<void>;
  report: (reason: unknown) => void;
};

/** @param submission Untrusted attempt notification. @param save Async draft consumer. @param delivery Native claim/acknowledgement boundary. @returns Completion; only a once-claimed native draft reaches Save, and stale/replayed notifications cannot settle another delivery. */
export async function deliverConfigureSubmission(
  submission: ConfigureSubmission,
  save: ConfigureSave,
  delivery: ConfigureDelivery,
): Promise<void> {
  let draft: ConfigureDraft;
  try {
    draft = await delivery.claim(submission.attemptId);
  } catch (reason) {
    delivery.report(reason);
    return;
  }
  let result: ConfigureResult;
  try {
    result = await save(draft);
  } catch {
    result = { ok: false, message: CONFIGURE_TEXT.failure };
  }
  try {
    await delivery.resolve(submission.attemptId, result);
  } catch (reason) {
    delivery.report(reason);
    // Malformed consumer results retain a recoverable failure, without releasing a replayed notification.
    await delivery
      .resolve(submission.attemptId, {
        ok: false,
        message: CONFIGURE_TEXT.failure,
      })
      .catch(delivery.report);
  }
}
