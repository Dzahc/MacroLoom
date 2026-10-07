import { useEffect, useLayoutEffect, useSyncExternalStore } from 'react';
import { ActionIcon } from './action-icon';
import { LABEL } from './library-model';
import { OutcomeQueue, PAUSE } from './outcome-queue';
import { subscribeDocumentVisibility } from './document-visibility';

/**
 * Presents only the queue head and announces each identity through a polite live region.
 * @param props Owner-managed queue; document/hover/focus pause expiration.
 * @returns A fixed bottom overlay with accessible dismissal and no layout displacement.
 * Document subscriptions disconnect on unmount; the owner disposes the queue.
 */
export function OutcomeToasts({
  queue,
  compact = false,
}: {
  queue: OutcomeQueue;
  compact?: boolean;
}) {
  const outcomes = useSyncExternalStore(
    queue.subscribe,
    queue.getSnapshot,
    queue.getSnapshot,
  );
  const current = compact ? undefined : outcomes[0];
  useLayoutEffect(
    /** Suspends ordinary toast lifetime while hidden by compact mode; element pauses cannot outlive hidden controls. */
    () => {
      if (compact) {
        queue.pause(PAUSE.compact);
        queue.resume(PAUSE.hover);
        queue.resume(PAUSE.focus);
      } else queue.resume(PAUSE.compact);
      /** Releases this view's compact pause on unmount without touching document visibility. */
      return () => queue.resume(PAUSE.compact);
    },
    [queue, compact],
  );
  useEffect(
    /** Subscribes hidden-document pauses independently of scroll position; returns cleanup. */
    () => subscribeDocumentVisibility(queue, document),
    [queue],
  );
  return (
    <div className="toast-overlay">
      <div
        className="sr-only"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        {current && <span key={current.id}>{current.message}</span>}
      </div>
      {current && (
        <aside
          key={current.id}
          className={`outcome-toast ${current.kind}`}
          onMouseEnter={
            /** Preserves visible lifetime while hovered. */ () =>
              queue.pause(PAUSE.hover)
          }
          onMouseLeave={
            /** Releases only the hover pause. */ () =>
              queue.resume(PAUSE.hover)
          }
          onFocusCapture={
            /** Preserves visible lifetime while a toast control has focus. */ () =>
              queue.pause(PAUSE.focus)
          }
          onBlurCapture={
            /** @param event Focus transition; releases the pause only when focus leaves the toast. @returns Nothing. */
            (event) => {
              if (!event.currentTarget.contains(event.relatedTarget))
                queue.resume(PAUSE.focus);
            }
          }
        >
          <ActionIcon action={current.kind} />
          <p>{current.message}</p>
          <button
            type="button"
            className="icon-button"
            aria-label={LABEL.dismiss}
            title={LABEL.dismiss}
            onClick={queue.dismiss}
          >
            <ActionIcon action="close" />
          </button>
        </aside>
      )}
    </div>
  );
}
