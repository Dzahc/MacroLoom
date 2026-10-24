import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  PLAYBACK_KEY,
  playbackKeyDown,
  playbackKeyUp,
} from '../src/playback-activation.ts';
import { SessionController } from '../src/session-controller.ts';
import {
  INITIAL_SESSION,
  type SessionSnapshot,
  type SessionTransport,
} from '../src/session-contract.ts';
import { ACTION, PHASE, SOURCE, requestAction } from '../src/library-model.ts';

const MACRO_ID = '68833c94-3d18-4fb2-8a59-0cc72382b616';
const MACRO_NAME = 'Recorded target click';
const LATER_REVISION = 3;
const START_REVISION = 2;
const FAILURE = 'Release held keys and mouse buttons';
const UNRELATED_KEY = 'Escape';
const LIBRARY = {
  macros: [{ id: MACRO_ID, name: MACRO_NAME, durationMs: 0, createdAt: '' }],
  selectedId: MACRO_ID,
  phase: PHASE.idle,
  message: '',
  playbackAvailable: true,
} as const;

/** Enter and Space activate exactly once after key release; native repeats never start playback early. */
void test('keyboard Play completes on release without a countdown', () => {
  for (const key of Object.values(PLAYBACK_KEY)) {
    let prevented = false;
    let starts = 0;
    const event = {
      key,
      repeat: false,
      /** Suppresses the browser's earlier native click. */
      preventDefault: () => {
        prevented = true;
      },
    };
    playbackKeyDown(event);
    playbackKeyDown({ ...event, repeat: true });
    assert.ok(prevented);
    assert.equal(starts, 0);
    playbackKeyUp(
      event,
      /** Records one completed keyboard activation. */ () => {
        starts += 1;
      },
    );
    assert.equal(starts, 1);
    playbackKeyUp(
      { ...event, key: UNRELATED_KEY },
      /** An unrelated release must not activate Play. */ () => {
        starts += 1;
      },
    );
    assert.equal(starts, 1);
  }
});

/** Toolbar and clicked-row Play both pass the same explicit ID to the backend session boundary. */
void test('Play sources use an ID-only command and preserve newer cancellation status', async () => {
  const ids: string[] = [];
  let resolve: ((snapshot: SessionSnapshot) => void) | undefined;
  const transport: SessionTransport = {
    /** Owns no native subscriptions in this command-boundary fixture. */
    listen: () => Promise.resolve(/** No subscription to release. */ () => {}),
    /** Returns an idle startup query without loading any macro events. */
    invoke: () => Promise.resolve(INITIAL_SESSION),
    /** Captures only the selected identity; input data stays entirely in the backend. */
    play: (id) => {
      ids.push(id);
      return new Promise(
        /** Captures a delayed native response. */ (complete) => {
          resolve = complete;
        },
      );
    },
  };
  const controller = new SessionController(transport);
  for (const source of [SOURCE.toolbar, SOURCE.doubleClick]) {
    const request = requestAction(LIBRARY, ACTION.play, source, MACRO_ID);
    assert.ok(request && 'macroId' in request);
    const pending = controller.play(request.macroId);
    controller.receive({
      ...INITIAL_SESSION,
      revision: LATER_REVISION,
      phase: PHASE.stopping,
    });
    assert.ok(resolve);
    resolve({
      ...INITIAL_SESSION,
      revision: START_REVISION,
      phase: PHASE.playing,
      macroName: MACRO_NAME,
    });
    await pending;
    assert.equal(controller.getSnapshot().phase, PHASE.stopping);
  }
  assert.deepEqual(ids, [MACRO_ID, MACRO_ID]);
  assert.equal(
    requestAction(
      { ...LIBRARY, playbackAvailable: false },
      ACTION.play,
      SOURCE.toolbar,
    ),
    null,
  );
});

/** A backend startup refusal preserves selection and exposes recovery guidance without automatic retry. */
void test('held-input startup rejection is actionable and does not retry', async () => {
  let attempts = 0;
  const controller = new SessionController({
    /** Provides no physical input listeners in this fixture. */
    listen: () => Promise.resolve(/** Inert listener cleanup. */ () => {}),
    /** Supplies initial backend state. */ invoke: () =>
      Promise.resolve(INITIAL_SESSION),
    /** Models one backend physical-held precondition failure. */
    play: () => {
      attempts += 1;
      return Promise.reject(new Error(FAILURE));
    },
  });
  await controller.play(MACRO_ID);
  assert.match(controller.getSnapshot().message, new RegExp(FAILURE));
  assert.equal(controller.getSnapshot().phase, PHASE.idle);
  assert.equal(attempts, 1);
});
