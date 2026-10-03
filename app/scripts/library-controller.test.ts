import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  LibraryController,
  type LibraryTransport,
} from '../src/library-controller.ts';
import {
  ACTION,
  SOURCE,
  LABEL,
  type LibraryAction,
} from '../src/library-model.ts';
import type {
  BackendLibraryState,
  MacroDocument,
  StoredMacroSummary,
} from '../src/macro-contract.ts';

const ID = '68833c94-3d18-4fb2-8a59-0cc72382b616';
const OTHER_ID = '78833c94-3d18-4fb2-8a59-0cc72382b616';
const DATE = '2026-09-28T21:32:08Z';
const SUMMARY: StoredMacroSummary = {
  id: ID,
  name: 'Fill form',
  durationMs: 1600,
  createdAt: DATE,
  playback: { speed: 1, repeatMode: 'once', totalRuns: 1, intervalMs: 0 },
};
const FAILURE = {
  file: 'broken.json',
  field: 'events[0].atMs',
  message: 'must be within durationMs',
};
const LOADING: BackendLibraryState = {
  revision: 1,
  loading: true,
  writable: true,
  macros: [SUMMARY],
  failures: [],
};
const COMPLETE: BackendLibraryState = {
  ...LOADING,
  revision: 2,
  loading: false,
  failures: [FAILURE],
};
const DOCUMENT = {
  ...SUMMARY,
  schemaVersion: 1,
  updatedAt: DATE,
  recording: {
    platform: 'windows',
    coordinateSpace: 'screen_physical_pixels',
    keyboardLayout: '00000409',
    displays: [],
  },
  events: [],
} satisfies MacroDocument;

void test('progressive snapshots preserve selection, deduplicate persistent failures and reject stale updates' /** @returns A promise checking public controller updates and cleanup at the native transport boundary. */, async () => {
  let receive: (
    state: BackendLibraryState,
  ) => void = /** Initial inactive event sink. */ () => {};
  let disconnected = false;
  const transport: LibraryTransport = {
    /** @param callback Backend event subscriber. @returns Native listener cleanup. */
    listen: (callback) => {
      receive = callback;
      return Promise.resolve(
        /** Releases the fake native listener. */ () => {
          disconnected = true;
        },
      );
    },
    /** @returns Final command result, deliberately older than an emitted event. */
    load: () => {
      receive(LOADING);
      return Promise.resolve(LOADING);
    },
    /** @param id Requested stable ID. @returns Its document; records identity through assertion. */
    read: (id) => {
      assert.equal(id, ID);
      return Promise.resolve(DOCUMENT);
    },
  };
  const controller = new LibraryController(transport);
  const disconnect = controller.connect();
  await new Promise<void>(
    /** @param resolve Microtask turn completion. */ (resolve) =>
      queueMicrotask(resolve),
  );
  controller.select({ macroId: ID, source: SOURCE.row });
  receive(COMPLETE);
  receive(LOADING);
  assert.equal(controller.getSnapshot().selectedId, ID);
  assert.equal(controller.getSnapshot().loading, false);
  assert.equal(controller.queue.getSnapshot().length, 1);
  assert.equal(controller.queue.getSnapshot()[0].persistent, true);
  assert.ok(controller.queue.getSnapshot()[0].message.includes(FAILURE.field));
  receive(COMPLETE);
  assert.equal(controller.queue.getSnapshot().length, 1);
  const action: LibraryAction = {
    action: ACTION.play,
    macroId: ID,
    source: SOURCE.doubleClick,
  };
  const prepared = await controller.prepare(action);
  assert.equal(prepared?.snapshot.id, ID);
  assert.equal(
    await controller.prepare({ ...action, macroId: OTHER_ID }),
    null,
  );
  disconnect();
  controller.dispose();
  receive({ ...COMPLETE, revision: 3, macros: [] });
  assert.equal(controller.getSnapshot().selectedId, ID);
  assert.equal(disconnected, true);
  assert.notEqual(controller.getSnapshot().message, LABEL.loading);
});
