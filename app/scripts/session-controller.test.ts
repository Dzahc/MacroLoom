import assert from 'node:assert/strict';
import { test } from 'node:test';
import { SessionController } from '../src/session-controller.ts';
import {
  INITIAL_SESSION,
  SESSION_COMMAND,
  type SessionSnapshot,
  type SessionTransport,
} from '../src/session-contract.ts';
import { PHASE, ACTION, canRequest } from '../src/library-model.ts';

const EARLY_REVISION = 1;
const LATER_REVISION = 2;
const FAILED_COMMAND = 'Native control unavailable';
const LIBRARY = {
  macros: [],
  selectedId: null,
  phase: PHASE.idle,
  message: '',
  writable: true,
} as const;

/** @param revision Known backend revision. @param phase Observable lifecycle phase. @returns One contract-valid test snapshot. */
function snapshot(
  revision: number,
  phase: SessionSnapshot['phase'] = PHASE.idle,
): SessionSnapshot {
  return { ...INITIAL_SESSION, revision, phase };
}
/** @returns A bounded async turn for listener registration and startup queries. */
async function settle() {
  await new Promise<void>(
    /** @param resolve Async-turn completion. @returns Nothing. */
    (resolve) => setImmediate(resolve),
  );
}

/** Newer native events win over a slow command reply, preserving active Stop availability. */
void test('late Record response cannot replace a newer Stopping event', async () => {
  let reply: ((value: SessionSnapshot) => void) | undefined;
  const transport: SessionTransport = {
    /** @returns An inert listener cleanup for this command-only case. */
    listen: () =>
      Promise.resolve(
        /** Releases no system resources in the fixture. */ () => {},
      ),
    /** @returns One deliberately pending native command. */
    invoke: () =>
      new Promise<SessionSnapshot>(
        /** @param resolve Pending command completion. @returns Nothing. */
        (resolve) => {
          reply = resolve;
        },
      ),
  };
  const controller = new SessionController(transport);
  const pending = controller.request(SESSION_COMMAND.record);
  controller.receive(snapshot(LATER_REVISION, PHASE.stopping));
  assert.ok(reply);
  reply(snapshot(EARLY_REVISION, PHASE.recording));
  await pending;
  assert.equal(controller.getSnapshot().phase, PHASE.stopping);
});

/** Abandoned listener registration and command replies cannot mutate a disconnected owner. */
void test('disconnect releases a late listener and ignores later native events', async () => {
  let finish: ((cleanup: () => void) => void) | undefined;
  let receive: ((value: SessionSnapshot) => void) | undefined;
  let released = false;
  const controller = new SessionController({
    /** @param callback Event subscriber. @returns Pending event registration. */
    listen: (callback) => {
      receive = callback;
      return new Promise(
        /** @param resolve Listener registration completion. @returns Nothing. */
        (resolve) => {
          finish = resolve;
        },
      );
    },
    /** @returns Initial native state if startup remains mounted. */
    invoke: () => Promise.resolve(snapshot(EARLY_REVISION)),
  });
  const disconnect = controller.connect();
  disconnect();
  assert.ok(finish);
  finish(
    /** Records actual listener cleanup after abandoned registration. */
    () => {
      released = true;
    },
  );
  await settle();
  assert.ok(receive);
  receive(snapshot(LATER_REVISION, PHASE.recording));
  assert.ok(released);
  assert.equal(controller.getSnapshot(), INITIAL_SESSION);
});

/** A failed native control leaves lifecycle ownership intact while reporting the failure. */
void test('command rejection preserves Stopping and actionable feedback', async () => {
  const controller = new SessionController({
    /** @returns Inert unsubscription. */ listen: () =>
      Promise.resolve(
        /** Fixture cleanup owns no native resources. */ () => {},
      ),
    /** @returns A rejected native control command. */ invoke: () =>
      Promise.reject(new Error(FAILED_COMMAND)),
  });
  controller.receive(snapshot(EARLY_REVISION, PHASE.stopping));
  await controller.request(SESSION_COMMAND.stop);
  assert.equal(controller.getSnapshot().phase, PHASE.stopping);
  assert.match(controller.getSnapshot().message, new RegExp(FAILED_COMMAND));
});

/** The real library must not enable Record without Stop or imply saved-library playback exists in ML-14. */
void test('toolbar availability respects native recording and deferred playback', () => {
  assert.equal(
    canRequest({ ...LIBRARY, recordingAvailable: false }, ACTION.record),
    false,
  );
  assert.equal(
    canRequest({ ...LIBRARY, recordingAvailable: true }, ACTION.record),
    true,
  );
  assert.equal(
    canRequest({ ...LIBRARY, playbackAvailable: false }, ACTION.play),
    false,
  );
});
