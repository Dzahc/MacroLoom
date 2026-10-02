import { OutcomeQueue, PAUSE } from './outcome-queue.ts';

export type ToastVisibilityEnvironment = {
  isDocumentHidden: () => boolean;
  subscribeDocument: (listener: () => void) => () => void;
  observeElement: (
    element: Element,
    listener: (visible: boolean) => void,
  ) => () => void;
};

export const FULLY_VISIBLE = 1;
const VISIBILITY_EVENT = 'visibilitychange';
export const BROWSER_VISIBILITY: ToastVisibilityEnvironment = {
  /** Returns whether the browser has hidden the document. */
  isDocumentHidden: () => document.hidden,
  /** Subscribes listener to document visibility changes; returns listener cleanup. */
  subscribeDocument: (listener) => {
    document.addEventListener(VISIBILITY_EVENT, listener);
    /** Removes the supplied listener without modifying document state. */
    return () => document.removeEventListener(VISIBILITY_EVENT, listener);
  },
  /**
   * Observes the full toast, accounting for viewport and scrolling ancestor clipping.
   * @param element Toast element to observe.
   * @param listener Receives true only when the entire toast is intersecting.
   * @returns Cleanup that disconnects the observer.
   */
  observeElement: (element, listener) => {
    const observer = new IntersectionObserver(
      /** Reports each entry's full visibility; partial intersections remain paused. */
      (entries) => {
        for (const entry of entries)
          listener(
            entry.isIntersecting && entry.intersectionRatio >= FULLY_VISIBLE,
          );
      },
      { threshold: FULLY_VISIBLE },
    );
    observer.observe(element);
    /** Disconnects the element observer, preventing further visibility reports. */
    return () => observer.disconnect();
  },
};

/**
 * Binds document and element visibility to a queue. Lifetime starts paused until
 * the observer confirms full visibility; hover/focus remain independent.
 * @param queue Queue whose lifetime counts only fully visible time.
 * @param element Toast element being displayed.
 * @param environment Visibility sources supplied by the host.
 * @returns Cleanup that disconnects both sources and releases their pause reasons.
 */
export function observeToastVisibility(
  queue: OutcomeQueue,
  element: Element,
  environment: ToastVisibilityEnvironment,
): () => void {
  queue.pause(PAUSE.offscreen);
  /** Updates the document pause reason without affecting hover or focus. */
  function updateDocument(): void {
    if (environment.isDocumentHidden()) queue.pause(PAUSE.hidden);
    else queue.resume(PAUSE.hidden);
  }
  updateDocument();
  const unsubscribe = environment.subscribeDocument(updateDocument);
  const unobserve = environment.observeElement(
    element,
    /** Updates the intersection pause reason without changing other pause reasons. */
    (visible) => {
      if (visible) queue.resume(PAUSE.offscreen);
      else queue.pause(PAUSE.offscreen);
    },
  );
  /** Releases host subscriptions and pauses when the toast is replaced or unmounted. */
  return () => {
    unsubscribe();
    unobserve();
    queue.resume(PAUSE.hidden);
    queue.resume(PAUSE.offscreen);
  };
}
