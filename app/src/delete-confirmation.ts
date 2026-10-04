import {
  ACTION,
  canRequest,
  type ActionSource,
  type LibrarySnapshot,
  type MacroSummary,
} from './library-model.ts';

export type DeleteTarget = Readonly<Pick<MacroSummary, 'id' | 'name'>>;
export type DeleteConfirmed = (macroId: string) => void | Promise<void>;
export const DELETE_TEXT = {
  title: 'Delete macro?',
  cancel: 'Cancel',
  confirm: 'Delete',
  close: 'Close delete confirmation',
  closeSymbol: '×',
  warningStart: 'Permanently delete “',
  warningEnd: '” and its saved file? This cannot be undone.',
  callbackFailure: 'Delete confirmation callback failed',
} as const;

/** @param reason Consumer failure reported without implying a disk outcome. @returns Nothing. */
function reportCallbackFailure(reason: unknown): void {
  console.error(DELETE_TEXT.callbackFailure, reason);
}

/**
 * Delivers a UI callback while handling both synchronous and asynchronous consumer failures.
 * @param receive Confirmation or cancellation observer; actual deletion outcomes remain consumer-owned.
 * @param macroId Frozen identity being reported.
 * @returns Nothing; failures reach the console rather than an unhandled event or rejection.
 */
export function notifyDelete(receive: DeleteConfirmed, macroId: string): void {
  try {
    void Promise.resolve(receive(macroId)).catch(reportCallbackFailure);
  } catch (reason) {
    reportCallbackFailure(reason);
  }
}

/** Owns one deliberate confirmation attempt; never performs file or library mutations. */
export class DeleteConfirmation {
  readonly target: DeleteTarget;
  readonly source: ActionSource;
  private finished = false;

  /**
   * Freezes the displayed identity/name for this attempt.
   * @param target Macro shown when the dialog opens.
   * @param source Origin used to restore focus after dismissal.
   */
  constructor(target: DeleteTarget, source: ActionSource) {
    this.target = Object.freeze({ id: target.id, name: target.name });
    this.source = source;
  }

  /** Cancels this attempt without invoking a consumer. @returns Nothing; repeated cancellation is harmless. */
  cancel(): void {
    this.finished = true;
  }

  /**
   * Derives availability without mutating the attempt or the supplied snapshot.
   * @param snapshot Latest known library state; disk monitoring is outside this module.
   * @returns Whether the original target still has its displayed name and Delete is available.
   */
  isAvailable(snapshot: LibrarySnapshot): boolean {
    if (this.finished || !canRequest(snapshot, ACTION.delete, this.target.id))
      return false;
    return snapshot.macros.some(
      /** @param macro Current library entry. @returns Whether it is the displayed target. */
      (macro) => macro.id === this.target.id && macro.name === this.target.name,
    );
  }

  /**
   * Finishes before calling the consumer, preventing duplicate or reentrant submissions.
   * @param snapshot Latest library state used to reject stale or unavailable targets.
   * @param receive Consumer of the stable ID; synchronous failures propagate after the attempt is finished.
   * @returns Whether this activation emitted confirmation; asynchronous failures are handled and logged.
   */
  confirm(snapshot: LibrarySnapshot, receive: DeleteConfirmed): boolean {
    const available = this.isAvailable(snapshot);
    this.finished = true;
    if (!available) return false;
    void Promise.resolve(receive(this.target.id)).catch(reportCallbackFailure);
    return true;
  }
}
