import assert from 'node:assert/strict';
import { test } from 'node:test';
import { SessionController } from '../src/session-controller.ts';
import {
  INITIAL_SESSION,
  SESSION_COMMAND,
  type SessionSnapshot,
  type SessionTransport,
  type ConfirmationRequest,
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

/** A pending successful native claim is released after unmount, and its old owner cannot release the remounted owner's claim. */
void test('late confirmation claim is released with its original owner', async () => {
  let reply: ((value: SessionSnapshot) => void) | undefined;
  const releases: ConfirmationRequest[] = [];
  const controller = new SessionController({
    /** @returns An immediately installed listener cleanup. */
    listen: () =>
      Promise.resolve(/** Fixture owns no native event resources. */ () => {}),
    /** @returns Startup state for a current owner. */
    invoke: () => Promise.resolve(snapshot(EARLY_REVISION)),
    /** @param request Native owner claim/release. @returns One delayed acquisition and immediate cleanup results. */
    confirmation: (request) => {
      if (!request.blocked) {
        releases.push(request);
        return Promise.resolve(snapshot(EARLY_REVISION));
      }
      return new Promise(
        /** @param resolve Delayed claim receipt. @returns Nothing. */
        (resolve) => {
          reply = resolve;
        },
      );
    },
  });
  const disconnect = controller.connect();
  const claim = controller.setConfirmation(true);
  const owner = controller.confirmationOwner;
  disconnect();
  const disconnectNext = controller.connect();
  assert.notEqual(controller.confirmationOwner, owner);
  assert.ok(reply);
  reply(snapshot(LATER_REVISION));
  assert.equal(await claim, false);
  await settle();
  assert.ok(releases.length > 0);
  assert.ok(
    releases.every(
      /** @param release Abandoned native claim release. @returns Whether it targets only the original owner. */
      (release) => release.owner === owner,
    ),
  );
  disconnectNext();
});

/** Sequential dialogs in one mounted window have distinct owners, so delayed Cancel acknowledgement cannot clear the newer claim. */
void test('a new dialog gets a distinct owner while old Cancel acknowledgement is delayed', async () => {
  let oldOwner: string | undefined;
  let oldReply: ((value: SessionSnapshot) => void) | undefined;
  const released: string[] = [];
  const controller = new SessionController({
    /** @returns Inert fixture event cleanup. */ listen: () =>
      Promise.resolve(
        /** No native subscriptions are acquired by this command fixture. */ () => {},
      ),
    /** @returns Current native snapshot. */ invoke: () =>
      Promise.resolve(snapshot(EARLY_REVISION)),
    /** @param request Owner-specific claim/release. @returns A delayed first Cancel acknowledgement with newer requests completing normally. */
    confirmation: (request) => {
      if (request.blocked) {
        oldOwner ??= request.owner;
        return Promise.resolve(snapshot(EARLY_REVISION));
      }
      released.push(request.owner);
      if (request.owner !== oldOwner)
        return Promise.resolve(snapshot(LATER_REVISION));
      return new Promise(
        /** @param resolve Old Cancel acknowledgement. @returns Nothing. */
        (resolve) => {
          oldReply = resolve;
        },
      );
    },
  });
  assert.equal(await controller.setConfirmation(true), true);
  const firstOwner = controller.confirmationOwner;
  const cancel = controller.setConfirmation(false);
  assert.equal(await controller.setConfirmation(true), true);
  const nextOwner = controller.confirmationOwner;
  assert.notEqual(nextOwner, firstOwner);
  assert.ok(oldReply);
  oldReply(snapshot(EARLY_REVISION));
  await cancel;
  assert.equal(await controller.setConfirmation(false), true);
  assert.ok(released.includes(nextOwner));
});
