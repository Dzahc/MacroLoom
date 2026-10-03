import assert from 'node:assert/strict';
import { test, type TestContext } from 'node:test';
import { TIME, type Outcome } from '../src/library-model.ts';
import { OUTCOME_EXAMPLES } from '../src/library-samples.ts';
import { OutcomeQueue, PAUSE } from '../src/outcome-queue.ts';

const FIRST: Outcome = { ...OUTCOME_EXAMPLES[0], id: 1 };
const SECOND: Outcome = { ...OUTCOME_EXAMPLES[1], id: 2 };
const HALF_LIFETIME = TIME.toast / 2;
const LONG_WAIT = TIME.toast * 3;
const LOAD_FAILURE: Outcome = { ...SECOND, persistent: true };

/**
 * Creates a real queue backed by deterministic monotonic time and mocked timers.
 * @param context Test owner registering disposal cleanup.
 * @returns Queue and a millisecond clock/timer advancement helper.
 */
function setup(context: TestContext) {
  let now = 0;
  context.mock.timers.enable({ apis: ['setTimeout'] });
  const queue = new OutcomeQueue({
    /** Returns controlled monotonic milliseconds. */
    now: () => now,
    /** Schedules callback after delay milliseconds through the test's mocked timer. */
    schedule: (callback, delay) => setTimeout(callback, delay),
    /** Cancels the supplied mocked timer handle. */
    cancel: (timer) => clearTimeout(timer),
  });
  /** @param milliseconds Elapsed monotonic time; runs due mocked timers. @returns Nothing. */
  function advance(milliseconds: number) {
    now += milliseconds;
    context.mock.timers.tick(milliseconds);
  }
  context.after(
    /** Disposes queued work after each test, including assertion failures. */ () =>
      queue.dispose(),
  );
  return { queue, advance };
}

void test('outcomes are shown in arrival order and each receives a full visible lifetime' /** @param context Timer-mock owner. @returns Nothing; checks both outcomes' expiration boundaries. */, (context) => {
  const { queue, advance } = setup(context);
  queue.enqueue(FIRST);
  queue.enqueue(SECOND);
  advance(TIME.toast - 1);
  assert.equal(queue.getSnapshot()[0], FIRST);
  advance(1);
  assert.equal(queue.getSnapshot()[0], SECOND);
  advance(TIME.toast);
  assert.equal(queue.getSnapshot().length, 0);
});

void test('load failures remain until dismissed and their successor gets its normal lifetime' /** @param context Timer owner. @returns Nothing; checks persistent failure and subsequent expiration. */, (context) => {
  const { queue, advance } = setup(context);
  queue.enqueue(LOAD_FAILURE);
  queue.enqueue(FIRST);
  advance(LONG_WAIT);
  assert.equal(queue.getSnapshot()[0], LOAD_FAILURE);
  queue.dismiss();
  advance(TIME.toast);
  assert.equal(queue.getSnapshot().length, 0);
});

void test('hover and focus independently pause the remaining lifetime' /** @param context Timer-mock owner. @returns Nothing; checks accumulated time across overlapping pauses. */, (context) => {
  const { queue, advance } = setup(context);
  queue.enqueue(FIRST);
  advance(HALF_LIFETIME);
  queue.pause(PAUSE.hover);
  queue.pause(PAUSE.focus);
  advance(LONG_WAIT);
  queue.resume(PAUSE.hover);
  advance(LONG_WAIT);
  assert.equal(queue.getSnapshot()[0], FIRST);
  queue.resume(PAUSE.focus);
  advance(HALF_LIFETIME - 1);
  assert.equal(queue.getSnapshot()[0], FIRST);
  advance(1);
  assert.equal(queue.getSnapshot().length, 0);
});

void test('hidden documents preserve notifications, including subsequent manual dismissals' /** @param context Timer-mock owner. @returns Nothing; checks hidden state carries over to the next outcome. */, (context) => {
  const { queue, advance } = setup(context);
  queue.enqueue(FIRST);
  queue.enqueue(SECOND);
  queue.pause(PAUSE.hidden);
  queue.dismiss();
  advance(LONG_WAIT);
  assert.equal(queue.getSnapshot()[0], SECOND);
  queue.resume(PAUSE.hidden);
  advance(TIME.toast);
  assert.equal(queue.getSnapshot().length, 0);
});

void test('dismissal cancels the old timer and duplicate IDs cannot duplicate notifications' /** @param context Timer-mock owner. @returns Nothing; checks duplicate suppression and stale timer cancellation. */, (context) => {
  const { queue, advance } = setup(context);
  queue.enqueue(FIRST);
  queue.enqueue(FIRST);
  assert.equal(queue.getSnapshot().length, 1);
  advance(HALF_LIFETIME);
  queue.enqueue(SECOND);
  queue.dismiss();
  advance(HALF_LIFETIME);
  assert.equal(queue.getSnapshot()[0], SECOND);
  advance(HALF_LIFETIME);
  assert.equal(queue.getSnapshot().length, 0);
});

void test('unsubscribing and disposal release listeners and timers' /** @param context Timer-mock owner. @returns Nothing; checks detached listeners and disposed timers stay inert. */, (context) => {
  const { queue, advance } = setup(context);
  let changes = 0;
  const off = queue.subscribe(
    /** Counts synchronous notifications while subscribed. */
    () => {
      changes += 1;
    },
  );
  queue.enqueue(FIRST);
  assert.equal(changes, 1);
  off();
  queue.enqueue(SECOND);
  queue.dispose();
  advance(LONG_WAIT);
  assert.equal(changes, 1);
  assert.equal(queue.getSnapshot().length, 0);
});
