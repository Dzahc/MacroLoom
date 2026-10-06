import assert from 'node:assert/strict';
import { test, type TestContext } from 'node:test';
import {
  LibraryController,
  type LibraryTransport,
} from '../src/library-controller.ts';
import { DeleteConfirmation } from '../src/delete-confirmation.ts';
import {
  ACTION,
  LABEL,
  SOURCE,
  TIME,
  canRequest,
} from '../src/library-model.ts';
import type {
  BackendLibraryState,
  MacroDocument,
  StoredMacroSummary,
} from '../src/macro-contract.ts';

const TARGET_ID = '68833c94-3d18-4fb2-8a59-0cc72382b616';
const OTHER_ID = '78833c94-3d18-4fb2-8a59-0cc72382b616';
const TARGET_NAME = 'Fill form';
const DATE = '2026-09-28T21:32:08Z';
const FAILURE = 'Access denied';
const INITIAL_REVISION = 1;
const PENDING_REVISION = INITIAL_REVISION + 1;
const FINAL_REVISION = PENDING_REVISION + 1;
const TARGET: StoredMacroSummary = {
  id: TARGET_ID,
  name: TARGET_NAME,
  createdAt: DATE,
  durationMs: TIME.millisecond,
  playback: { speed: 1, repeatMode: 'once', totalRuns: 1, intervalMs: 0 },
};
const INITIAL: BackendLibraryState = {
  revision: INITIAL_REVISION,
  loading: false,
  writable: true,
  deleting: null,
  macros: [TARGET, { ...TARGET, id: OTHER_ID }],
  failures: [],
};
const FINAL: BackendLibraryState = {
  ...INITIAL,
  revision: FINAL_REVISION,
  macros: [INITIAL.macros[1]],
};

/** @returns An async turn after subscription/discovery promise callbacks settle. */
async function settle(): Promise<void> {
  await new Promise<void>(
    /** @param resolve Completes the turn. */ (resolve) =>
      setImmediate(resolve),
  );
}

/**
 * Creates a controller at the public transport boundary with a controlled pending deletion.
 * @param context Owns timer mocks and connection cleanup.
 * @returns Controller, native event delivery, request list and explicit completion functions.
 */
async function setup(context: TestContext) {
  context.mock.timers.enable({ apis: ['setTimeout'] });
  let receive: (
    state: BackendLibraryState,
  ) => void = /** Initial event sink. */ () => {};
  let succeed: (
    state: BackendLibraryState,
  ) => void = /** Initial completion sink. */ () => {};
  let fail: (reason: Error) => void = /** Initial failure sink. */ () => {};
  const requests: string[] = [];
  const pending = new Promise<BackendLibraryState>(
    /** @param resolve Success completion. @param reject Disk failure completion. */
    (resolve, reject) => {
      succeed = resolve;
      fail = reject;
    },
  );
  const transport: LibraryTransport = {
    /** @param callback Event subscriber. @returns Listener cleanup. */
    listen: (callback) => {
      receive = callback;
      return Promise.resolve(/** Releases this fake listener. */ () => {});
    },
    /** @returns Loaded metadata only. */
    load: () => Promise.resolve(INITIAL),
    /** @returns Rejection; deletion must never request document contents. */
    read: () => Promise.reject<MacroDocument>(new Error(FAILURE)),
    /** @param id Confirmed identity. @returns Pending disk operation. */
    delete: (id) => {
      requests.push(id);
      return pending;
    },
  };
  const controller = new LibraryController(transport);
  const disconnect = controller.connect();
  context.after(
    /** Releases subscription and notification timers. */ () => {
      disconnect();
      controller.dispose();
    },
  );
  await settle();
  controller.select({ macroId: TARGET_ID, source: SOURCE.row });
  return {
    controller,
    requests,
    succeed,
    fail,
    disconnect,
    /** @param state Native metadata delivered through the listener. */
    emit: (state: BackendLibraryState) => receive(state),
  };
}

/** Confirms once, permits selection while pending, reconciles disk success and expires its named toast. */
void test('confirmed deletion blocks actions and preserves a different selection on success', async (context) => {
  const { controller, requests, succeed, emit } = await setup(context);
  const attempt = new DeleteConfirmation(TARGET, SOURCE.toolbar);
  /** @param id Frozen confirmation identity. @returns Native deletion completion. */
  const confirm = (id: string) => controller.deleteConfirmed(id);
  assert.equal(attempt.confirm(controller.getSnapshot(), confirm), true);
  assert.equal(attempt.confirm(controller.getSnapshot(), confirm), false);
  await controller.deleteConfirmed(TARGET_ID);
  assert.deepEqual(requests, [TARGET_ID]);
  assert.equal(controller.getSnapshot().macros.length, INITIAL.macros.length);
  assert.match(controller.getSnapshot().message, new RegExp(TARGET_NAME));
  for (const action of Object.values(ACTION))
    assert.equal(canRequest(controller.getSnapshot(), action), false);
  controller.select({ macroId: OTHER_ID, source: SOURCE.row });
  emit({ ...INITIAL, revision: PENDING_REVISION, deleting: TARGET_ID });
  assert.equal(controller.getSnapshot().selectedId, OTHER_ID);
  emit({ ...FINAL, revision: FINAL_REVISION + 1 });
  assert.equal(canRequest(controller.getSnapshot(), ACTION.record), false);
  succeed(FINAL);
  await settle();
  assert.equal(controller.getSnapshot().selectedId, OTHER_ID);
  assert.equal(controller.getSnapshot().message, LABEL.ready);
  assert.deepEqual(controller.getSnapshot().macros, FINAL.macros);
  assert.equal(
    controller.queue.getSnapshot()[0].message,
    `Deleted “${TARGET_NAME}”`,
  );
  context.mock.timers.tick(TIME.toast);
  assert.equal(controller.queue.getSnapshot().length, 0);
});

/** A failed disk operation preserves metadata/selection and expires the ordinary error toast. */
void test('deletion failure retains selected entry and allows a fresh confirmation', async (context) => {
  const { controller, fail } = await setup(context);
  const deletion = controller.deleteConfirmed(TARGET_ID);
  fail(new Error(FAILURE));
  await deletion;
  assert.deepEqual(controller.getSnapshot().macros, INITIAL.macros);
  assert.equal(controller.getSnapshot().selectedId, TARGET_ID);
  assert.equal(controller.getSnapshot().message, LABEL.ready);
  assert.equal(canRequest(controller.getSnapshot(), ACTION.delete), true);
  assert.equal(controller.queue.getSnapshot()[0].kind, 'failure');
  assert.ok(controller.queue.getSnapshot()[0].message.includes(TARGET_NAME));
  assert.ok(controller.queue.getSnapshot()[0].message.includes(FAILURE));
  context.mock.timers.tick(TIME.toast);
  assert.equal(controller.queue.getSnapshot().length, 0);
});

/** Removing the selected identity clears selection, without picking its successor. */
void test('successful deletion clears only the deleted selection', async (context) => {
  const { controller, succeed } = await setup(context);
  const deletion = controller.deleteConfirmed(TARGET_ID);
  succeed(FINAL);
  await deletion;
  assert.equal(controller.getSnapshot().selectedId, null);
});

/** Cancelled confirmation never calls the deletion transport or produces an outcome. */
void test('cancel preserves the library and causes no native deletion', async (context) => {
  const { controller, requests } = await setup(context);
  const attempt = new DeleteConfirmation(TARGET, SOURCE.context);
  attempt.cancel();
  assert.equal(
    attempt.confirm(
      controller.getSnapshot(),
      /** @param id Confirmed ID, never called after cancellation. @returns Deletion completion. */
      (id) => controller.deleteConfirmed(id),
    ),
    false,
  );
  assert.deepEqual(requests, []);
  assert.deepEqual(controller.getSnapshot().macros, INITIAL.macros);
  assert.equal(controller.queue.getSnapshot().length, 0);
});

/** Reconnecting during pending deletion must not retain a stale local busy state or deliver an old toast. */
void test('disconnect suppresses late deletion outcomes and reconnect reconciles native completion', async (context) => {
  const { controller, succeed, disconnect, emit } = await setup(context);
  const deletion = controller.deleteConfirmed(TARGET_ID);
  disconnect();
  const reconnectCleanup = controller.connect();
  context.after(reconnectCleanup);
  await settle();
  emit(FINAL);
  succeed(FINAL);
  await deletion;
  assert.equal(controller.getSnapshot().deleting, null);
  assert.equal(controller.getSnapshot().selectedId, null);
  assert.equal(controller.queue.getSnapshot().length, 0);
});
