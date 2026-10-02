import assert from 'node:assert/strict';
import { test, type TestContext } from 'node:test';
import { TIME, type Outcome } from '../src/library-model.ts';
import { OUTCOME_EXAMPLES } from '../src/library-samples.ts';
import { OutcomeQueue, PAUSE } from '../src/outcome-queue.ts';
import {
  DOCUMENT_VISIBILITY_EVENT,
  subscribeDocumentVisibility,
} from '../src/document-visibility.ts';

const FIRST: Outcome = { ...OUTCOME_EXAMPLES[0], id: 1 };
const HALF_LIFETIME = TIME.toast / 2;
const LONG_WAIT = TIME.toast * 3;
const LAST_MILLISECOND = 1;

/**
 * Creates a real queue, controlled document events, and deterministic timer advancement.
 * @param context Test context owning timer mocks and cleanup.
 * @param hidden Initial document state to exercise startup while hidden.
 * @returns Queue, host document, cleanup, visibility changes, and time advancement.
 */
function setup(context: TestContext, hidden = false) {
  let now = 0;
  context.mock.timers.enable({ apis: ['setTimeout'] });
  const queue = new OutcomeQueue({
    /** Returns controlled monotonic milliseconds. */
    now: () => now,
    /** Schedules callback after delay milliseconds through mocked timers. */
    schedule: (callback, delay) => setTimeout(callback, delay),
    /** Cancels the supplied mocked timer handle. */
    cancel: (timer) => clearTimeout(timer),
  });
  const events = new EventTarget();
  const source = {
    hidden,
    /** Registers document visibility listeners with the controlled event target. */
    addEventListener: events.addEventListener.bind(events),
    /** Removes the supplied document visibility listener. */
    removeEventListener: events.removeEventListener.bind(events),
  };
  const cleanup = subscribeDocumentVisibility(queue, source);
  queue.enqueue(FIRST);
  /** Releases the subscription and queued timers after the test, including assertion failures. */
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
    /** Emits the supplied document visibility state without changing hover/focus. */
    hidden(value: boolean) {
      source.hidden = value;
      events.dispatchEvent(new Event(DOCUMENT_VISIBILITY_EVENT));
    },
  };
}

/** Checks an overlay created while hidden receives its complete lifetime after the app becomes visible. */
void test('initially hidden overlays wait for document visibility before expiring', (context) => {
  const host = setup(context, true);
  host.advance(LONG_WAIT);
  assert.equal(host.queue.getSnapshot()[0], FIRST);
  host.hidden(false);
  host.advance(TIME.toast - LAST_MILLISECOND);
  assert.equal(host.queue.getSnapshot()[0], FIRST);
  host.advance(LAST_MILLISECOND);
  assert.equal(host.queue.getSnapshot().length, 0);
});

/** Checks hiding the app preserves remaining time while hover and focus remain independent. */
void test('document changes preserve the remaining overlay lifetime across overlapping pauses', (context) => {
  const host = setup(context);
  host.advance(HALF_LIFETIME);
  host.hidden(true);
  host.queue.pause(PAUSE.hover);
  host.queue.pause(PAUSE.focus);
  host.advance(LONG_WAIT);
  host.hidden(false);
  host.queue.resume(PAUSE.hover);
  host.advance(LONG_WAIT);
  assert.equal(host.queue.getSnapshot()[0], FIRST);
  host.queue.resume(PAUSE.focus);
  host.advance(HALF_LIFETIME);
  assert.equal(host.queue.getSnapshot().length, 0);
});

/** Checks cleanup removes the external listener so later document events cannot affect expiration. */
void test('unsubscribed document events cannot pause an overlay timer', (context) => {
  const host = setup(context);
  host.cleanup();
  host.hidden(true);
  host.advance(TIME.toast);
  assert.equal(host.queue.getSnapshot().length, 0);
});
