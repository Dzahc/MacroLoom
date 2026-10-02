import assert from 'node:assert/strict';
import { test, type TestContext } from 'node:test';
import { TIME, type Outcome } from '../src/library-model.ts';
import { OUTCOME_EXAMPLES } from '../src/library-samples.ts';
import { OutcomeQueue, PAUSE } from '../src/outcome-queue.ts';
import {
  BROWSER_VISIBILITY,
  FULLY_VISIBLE,
  observeToastVisibility,
  type ToastVisibilityEnvironment,
} from '../src/toast-visibility.ts';

const FIRST: Outcome = { ...OUTCOME_EXAMPLES[0], id: 1 };
const SECOND: Outcome = { ...OUTCOME_EXAMPLES[1], id: 2 };
const HALF_LIFETIME = TIME.toast / 2;
const LONG_WAIT = TIME.toast * 3;
const LAST_MILLISECOND = 1;
const ELEMENT = {} as Element;
const PARTLY_VISIBLE = FULLY_VISIBLE / 2;
const NOT_VISIBLE = 0;
const VISIBILITY_SUBSCRIPTIONS = 2;
const OBSERVER_GLOBAL = 'IntersectionObserver';

/**
 * Creates controlled host visibility sources and a real queue with a fake monotonic clock.
 * @param context Test context owning timer mocks and subscription cleanup.
 * @returns Controls for time, intersection, document state, and cleanup inspection.
 */
function setup(context: TestContext) {
  let now = 0;
  let hidden = false;
  let documentListener: (() => void) | null = null;
  let elementListener: ((visible: boolean) => void) | null = null;
  context.mock.timers.enable({ apis: ['setTimeout'] });
  const queue = new OutcomeQueue({
    /** Returns the controlled monotonic timestamp in milliseconds. */
    now: () => now,
    /** Schedules the callback after delay milliseconds using mocked timers. */
    schedule: (callback, delay) => setTimeout(callback, delay),
    /** Cancels the supplied mocked timer handle. */
    cancel: (timer) => clearTimeout(timer),
  });
  const environment: ToastVisibilityEnvironment = {
    /** Returns the host's controlled hidden state. */
    isDocumentHidden: () => hidden,
    /** Stores the document callback and returns cleanup that releases it. */
    subscribeDocument: (listener) => {
      documentListener = listener;
      /** Releases the document listener; subsequent host changes do nothing. */
      return () => {
        documentListener = null;
      };
    },
    /** Stores the intersection callback for the supplied toast and returns cleanup. */
    observeElement: (_element, listener) => {
      elementListener = listener;
      /** Releases the element observer callback. */
      return () => {
        elementListener = null;
      };
    },
  };
  queue.enqueue(FIRST);
  const cleanup = observeToastVisibility(queue, ELEMENT, environment);
  /** Releases host callbacks and queue timers after the test. */
  context.after(() => {
    cleanup();
    queue.dispose();
  });
  return {
    queue,
    cleanup,
    /** Advances monotonic time and executes due timers by the supplied milliseconds. */
    advance(milliseconds: number) {
      now += milliseconds;
      context.mock.timers.tick(milliseconds);
    },
    /** Emits whether the full toast is visible in its scrollable ancestors and viewport. */
    visible(value: boolean) {
      if (elementListener !== null) elementListener(value);
    },
    /** Emits a document visibility change without changing element intersection. */
    hidden(value: boolean) {
      hidden = value;
      if (documentListener !== null) documentListener();
    },
    /** Reports how many external callbacks remain subscribed. */
    subscriptions() {
      return (
        Number(documentListener !== null) + Number(elementListener !== null)
      );
    },
  };
}

/** Reproduces a toast expiring unseen after its initiating dev control scrolls it offscreen. */
void test('offscreen toasts retain four seconds of visible lifetime, including before first intersection', (context) => {
  const host = setup(context);
  host.advance(LONG_WAIT);
  assert.equal(host.queue.getSnapshot()[0], FIRST);
  host.visible(true);
  host.advance(HALF_LIFETIME);
  host.visible(false);
  host.advance(LONG_WAIT);
  assert.equal(host.queue.getSnapshot()[0], FIRST);
  host.visible(true);
  host.advance(HALF_LIFETIME - LAST_MILLISECOND);
  assert.equal(host.queue.getSnapshot()[0], FIRST);
  host.advance(LAST_MILLISECOND);
  assert.equal(host.queue.getSnapshot().length, 0);
});

/** Confirms document, intersection, hover, and focus pauses remain independent. */
void test('offscreen state survives dismissal and overlaps document, hover and focus pauses', (context) => {
  const host = setup(context);
  host.queue.enqueue(SECOND);
  host.visible(false);
  host.hidden(true);
  host.queue.dismiss();
  host.visible(true);
  host.advance(LONG_WAIT);
  assert.equal(host.queue.getSnapshot()[0], SECOND);
  host.queue.pause(PAUSE.hover);
  host.queue.pause(PAUSE.focus);
  host.hidden(false);
  host.queue.resume(PAUSE.hover);
  host.advance(LONG_WAIT);
  assert.equal(host.queue.getSnapshot()[0], SECOND);
  host.queue.resume(PAUSE.focus);
  host.advance(TIME.toast);
  assert.equal(host.queue.getSnapshot().length, 0);
});

/** Checks that unmount cleanup releases both external subscriptions. */
void test('visibility cleanup disconnects the observer and document subscription', (context) => {
  const host = setup(context);
  assert.equal(host.subscriptions(), VISIBILITY_SUBSCRIPTIONS);
  host.cleanup();
  assert.equal(host.subscriptions(), 0);
});

/** Exercises the actual browser adapter's threshold, clipping reports, and disconnect path. */
void test('browser observer requires full intersection and disconnects on cleanup', (context) => {
  const original = Object.getOwnPropertyDescriptor(globalThis, OBSERVER_GLOBAL);
  let report: IntersectionObserverCallback;
  let disconnected = false;
  const observed: boolean[] = [];
  /** Host observer double delivering intersection entries without a browser dependency. */
  class Observer {
    /** @param callback Intersection receiver. @param options Host threshold configuration. */
    constructor(
      callback: IntersectionObserverCallback,
      options: IntersectionObserverInit,
    ) {
      report = callback;
      assert.equal(options.threshold, FULLY_VISIBLE);
    }
    /** @param element Toast being observed. @returns Nothing; verifies the adapter binds the requested node. */
    observe(element: Element) {
      assert.equal(element, ELEMENT);
    }
    /** Records that observer cleanup disconnected further host reports. */
    disconnect() {
      disconnected = true;
    }
  }
  Object.defineProperty(globalThis, OBSERVER_GLOBAL, {
    configurable: true,
    value: Observer,
  });
  /** Restores the host global even if assertions fail. */
  context.after(() => {
    if (original) Object.defineProperty(globalThis, OBSERVER_GLOBAL, original);
    else Reflect.deleteProperty(globalThis, OBSERVER_GLOBAL);
  });
  const cleanup = BROWSER_VISIBILITY.observeElement(
    ELEMENT,
    /** @param visible Actual adapter's full-intersection result. @returns Nothing; records the report. */
    (visible) => {
      observed.push(visible);
    },
  );
  for (const ratio of [NOT_VISIBLE, PARTLY_VISIBLE, FULLY_VISIBLE]) {
    report!(
      [
        {
          isIntersecting: ratio > NOT_VISIBLE,
          intersectionRatio: ratio,
        } as IntersectionObserverEntry,
      ],
      {} as IntersectionObserver,
    );
  }
  assert.deepEqual(observed, [false, false, true]);
  cleanup();
  assert.equal(disconnected, true);
});
