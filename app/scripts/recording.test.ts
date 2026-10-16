import assert from 'node:assert/strict';
import { test } from 'node:test';
import { SessionController } from '../src/session-controller.ts';
import { LibraryController } from '../src/library-controller.ts';
import {
  INITIAL_SESSION,
  SESSION_COMMAND,
  type RecordingOutcome,
} from '../src/session-contract.ts';
import { PHASE, SOURCE, canRequest, ACTION } from '../src/library-model.ts';
import type { BackendLibraryState } from '../src/macro-contract.ts';

const ID = '68833c94-3d18-4fb2-8a59-0cc72382b616';
const OTHER_ID = '78833c94-3d18-4fb2-8a59-0cc72382b616';
const NAME = 'New recording';
const DATE = '2026-10-14T00:00:00Z';
const FAILURE = 'Could not save recording; Retry or Discard.';
const SUCCESS = `Saved “${NAME}”`;
const FAILURE_SEQUENCE = 1;
const SUCCESS_SEQUENCE = 2;
const FIRST_REVISION = 1;
const NEXT_REVISION = 2;
const SUCCESS_REVISION = 3;
const LATER_REVISION = 4;
const ONE_OUTCOME = 1;
const TWO_OUTCOMES = 2;
const DURATION = 100;
const REPROBE_MESSAGE =
  'Known storage failure; repaired permissions may be retried.';
const LOADED: BackendLibraryState = {
  revision: FIRST_REVISION,
  loading: false,
  writable: true,
  deleting: null,
  failures: [],
  macros: [
    {
      id: OTHER_ID,
      name: NAME,
      createdAt: DATE,
      durationMs: DURATION,
      playback: { speed: 1, repeatMode: 'once', totalRuns: 1, intervalMs: 0 },
    },
  ],
};
const COMMITTED: BackendLibraryState = {
  ...LOADED,
  revision: NEXT_REVISION,
  macros: [...LOADED.macros, { ...LOADED.macros[0], id: ID }],
};
const FAILED: RecordingOutcome = {
  sequence: FAILURE_SEQUENCE,
  id: ID,
  name: NAME,
  success: false,
  message: FAILURE,
  library: null,
};
const SAVED: RecordingOutcome = {
  ...FAILED,
  sequence: SUCCESS_SEQUENCE,
  success: true,
  message: SUCCESS,
  library: COMMITTED,
};

/** A failed save remains actionable after its timed toast; only a new committed receipt can announce success. */
void test('save outcomes are timed once per attempt and Retry does not announce premature success', async () => {
  const failed = {
    ...INITIAL_SESSION,
    revision: FIRST_REVISION,
    phase: PHASE.saving,
    saveFailed: true,
    unsaved: true,
    recordingOutcome: FAILED,
    message: FAILURE,
  };
  const controller = new SessionController({
    /** @returns An inert native subscription cleanup. */
    listen: () =>
      Promise.resolve(/** No native resources in this fixture. */ () => {}),
    /** @param command Explicit retry. @returns The same backend-owned committed capture. */
    invoke: (command) => {
      assert.equal(command, SESSION_COMMAND.retryRecording);
      return Promise.resolve({
        ...INITIAL_SESSION,
        revision: SUCCESS_REVISION,
        recordingOutcome: SAVED,
        message: SUCCESS,
      });
    },
  });
  controller.receive(failed);
  controller.receive({ ...failed, revision: NEXT_REVISION });
  assert.equal(controller.queue.getSnapshot().length, ONE_OUTCOME);
  controller.queue.dismiss();
  assert.equal(controller.getSnapshot().message, FAILURE);
  assert.ok(controller.getSnapshot().saveFailed);
  await controller.request(SESSION_COMMAND.retryRecording);
  assert.equal(controller.queue.getSnapshot().length, ONE_OUTCOME);
  assert.equal(controller.queue.getSnapshot()[0]?.kind, 'success');
  assert.equal(controller.getSnapshot().unsaved, false);
  controller.receive({
    ...INITIAL_SESSION,
    revision: LATER_REVISION,
    recordingOutcome: SAVED,
  });
  assert.notEqual(controller.queue.getSnapshot().length, TWO_OUTCOMES);
  controller.queue.dispose();
});

/** Failed writes retain prior selection; commit selects once, and duplicate/stale receipts cannot override later user selection. */
void test('recordings enter the library and become selected only after a committed receipt', async () => {
  const controller = new LibraryController({
    /** @returns An inert native subscription. */
    listen: () => Promise.resolve(/** Owns no native listener. */ () => {}),
    /** @returns The existing library, before the recording commits. */
    load: () => Promise.resolve(LOADED),
    /** Reading full events is not needed to publish a commit receipt. */
    read: () => Promise.reject(new Error(FAILURE)),
    /** @returns Unchanged library; unused by this save test. */
    delete: () => Promise.resolve(LOADED),
  });
  const disconnect = controller.connect();
  await new Promise<void>(
    /** @param resolve Advances subscription and discovery promises. */ (
      resolve,
    ) => setImmediate(resolve),
  );
  controller.select({ macroId: OTHER_ID, source: SOURCE.row });
  controller.recorded(FAILED);
  assert.equal(controller.getSnapshot().selectedId, OTHER_ID);
  controller.recorded(SAVED);
  assert.equal(controller.getSnapshot().selectedId, ID);
  controller.select({ macroId: OTHER_ID, source: SOURCE.row });
  controller.recorded(SAVED);
  assert.equal(controller.getSnapshot().selectedId, OTHER_ID);
  assert.equal(
    canRequest(
      {
        ...controller.getSnapshot(),
        writable: false,
        recordingAvailable: true,
        message: REPROBE_MESSAGE,
      },
      ACTION.record,
    ),
    true,
  );
  assert.equal(
    canRequest(
      { ...controller.getSnapshot(), phase: PHASE.saving },
      ACTION.configure,
    ),
    false,
  );
  disconnect();
  controller.dispose();
});
