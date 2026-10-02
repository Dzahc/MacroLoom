import { OutcomeQueue, PAUSE } from './outcome-queue.ts';

export const DOCUMENT_VISIBILITY_EVENT = 'visibilitychange';
type VisibilityDocument = Pick<
  Document,
  'hidden' | 'addEventListener' | 'removeEventListener'
>;

/**
 * Suspends overlay expiration while its document is hidden, independently of hover/focus.
 * @param queue Queue whose remaining lifetime is preserved while hidden.
 * @param source Document supplying hidden state and visibility events.
 * @returns Cleanup that removes the listener and releases this subscription's pause.
 */
export function subscribeDocumentVisibility(
  queue: OutcomeQueue,
  source: VisibilityDocument,
): () => void {
  /** Synchronizes the hidden-document pause without changing outcomes or other pause reasons. */
  function update(): void {
    if (source.hidden) queue.pause(PAUSE.hidden);
    else queue.resume(PAUSE.hidden);
  }
  update();
  source.addEventListener(DOCUMENT_VISIBILITY_EVENT, update);
  /** Removes the host listener and releases its pause when the queue owner changes or unmounts. */
  return () => {
    source.removeEventListener(DOCUMENT_VISIBILITY_EVENT, update);
    queue.resume(PAUSE.hidden);
  };
}
