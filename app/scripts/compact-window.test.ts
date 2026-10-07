import assert from 'node:assert/strict';
import { test } from 'node:test';
import { COMPACT, type WindowViewRequest } from '../src/compact-contract.ts';
import { CompactWindowController } from '../src/compact-window-controller.ts';

const CONTENT_HEIGHT = 156;
const ENTER: WindowViewRequest = {
  compact: true,
  width: COMPACT.minimumWidth,
  height: CONTENT_HEIGHT,
};
const EXIT: WindowViewRequest = { ...ENTER, compact: false };
const RESTORE_FAILURE = 'Could not restore window';
const MEASURED_HEIGHT_CHANGE = 1;

/** Requests are serialized at the native boundary; restoring after an in-flight entry wins without losing the full-window snapshot. */
void test('window transitions serialize entry followed by restoration', async () => {
  let active = 0;
  const controller = new CompactWindowController(
    /** @param request Requested native layout. @returns Actual mode after an asynchronous boundary. */
    async (request) => {
      active += 1;
      assert.equal(active, 1);
      await Promise.resolve();
      active -= 1;
      return { compact: request.compact, error: null };
    },
  );
  try {
    await Promise.all([controller.update(ENTER), controller.update(EXIT)]);
    assert.equal(controller.getSnapshot().compact, false);
  } finally {
    controller.dispose();
  }
});

/** Entry failure blocks already queued remeasurements, but an explicit full/compact cycle starts a fresh transition. */
void test('entry failure does not retry queued size changes and a later view cycle can enter again', async () => {
  let rejectEntry = true;
  const controller = new CompactWindowController(
    /** @param request Requested native mode. @returns One rejected entry followed by successful explicit transitions. */
    (request) => {
      const error = request.compact && rejectEntry ? RESTORE_FAILURE : null;
      rejectEntry = false;
      return Promise.resolve({
        compact: request.compact && error === null,
        error,
      });
    },
  );
  try {
    await Promise.all([
      controller.update(ENTER),
      controller.update({
        ...ENTER,
        height: CONTENT_HEIGHT + MEASURED_HEIGHT_CHANGE,
      }),
    ]);
    assert.deepEqual(controller.getSnapshot(), {
      compact: false,
      failed: true,
    });
    assert.equal(controller.failures.getSnapshot().length, 1);
    await controller.update(EXIT);
    await controller.update(ENTER);
    assert.deepEqual(controller.getSnapshot(), {
      compact: true,
      failed: false,
    });
  } finally {
    controller.dispose();
  }
});

/** Failed restoration retains the actual compact layout and reports one timed failure without automatic retries. */
void test('failed restoration remains compact and emits a timed toast once', async () => {
  const controller = new CompactWindowController(
    /** @param request Desired mode. @returns Entry success or a restoration failure retaining actual compact bounds. */
    (request) =>
      Promise.resolve({
        compact: true,
        error: request.compact ? null : RESTORE_FAILURE,
      }),
  );
  try {
    await controller.update(ENTER);
    await controller.update(EXIT);
    await controller.update(EXIT);
    assert.deepEqual(controller.getSnapshot(), { compact: true, failed: true });
    const failures = controller.failures.getSnapshot();
    assert.equal(failures.length, 1);
    assert.equal(failures[0].message, RESTORE_FAILURE);
    assert.notEqual(failures[0].persistent, true);
  } finally {
    controller.dispose();
  }
});
