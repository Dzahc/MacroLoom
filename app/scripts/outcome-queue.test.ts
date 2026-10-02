import assert from 'node:assert/strict';
import { test, type TestContext } from 'node:test';
import { TIME, type Outcome } from '../src/library-model.ts';
import { OUTCOME_EXAMPLES } from '../src/library-samples.ts';
import { OutcomeQueue, PAUSE } from '../src/outcome-queue.ts';

const FIRST: Outcome = { ...OUTCOME_EXAMPLES[0], id: 1 };
const SECOND: Outcome = { ...OUTCOME_EXAMPLES[1], id: 2 };
const HALF_LIFETIME = TIME.toast / 2;
const LONG_WAIT = TIME.toast * 3;

function setup(context: TestContext) {
  let now = 0;
  context.mock.timers.enable({ apis: ['setTimeout'] });
  const queue = new OutcomeQueue({
    now: () => now,
    schedule: (callback, delay) => setTimeout(callback, delay),
    cancel: (timer) => clearTimeout(timer),
  });
  function advance(milliseconds: number) {
    now += milliseconds;
    context.mock.timers.tick(milliseconds);
  }
  context.after(() => queue.dispose());
  return { queue, advance };
}

void test('outcomes are shown in arrival order and each receives a full visible lifetime', (context) => {
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

void test('hover and focus independently pause the remaining lifetime', (context) => {
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

void test('hidden documents preserve notifications, including subsequent manual dismissals', (context) => {
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

void test('dismissal cancels the old timer and duplicate IDs cannot duplicate notifications', (context) => {
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

void test('unsubscribing and disposal release listeners and timers', (context) => {
  const { queue, advance } = setup(context);
  let changes = 0;
  const off = queue.subscribe(() => {
    changes += 1;
  });
  queue.enqueue(FIRST);
  assert.equal(changes, 1);
  off();
  queue.enqueue(SECOND);
  queue.dispose();
  advance(LONG_WAIT);
  assert.equal(changes, 1);
  assert.equal(queue.getSnapshot().length, 0);
});
