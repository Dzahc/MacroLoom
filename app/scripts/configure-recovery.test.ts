import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  ConfigureRecoveryController,
  type ConfigureRecoveryStatus,
} from '../src/configure-recovery.ts';

const RETAINED: ConfigureRecoveryStatus = {
  occupied: true,
  needed: true,
  pending: false,
  message: 'Reconnect a display and Retry.',
  completion: null,
};
const RECOVERED: ConfigureRecoveryStatus = {
  occupied: true,
  needed: false,
  pending: false,
  message: '',
  completion: null,
};
const UNAVAILABLE = 'Native status unavailable';
const NO_DISCARDS = 0;
const ONE_DISCARD = 1;
const DEFERRED_NOT_READY = 'Deferred response not registered';
const NO_DELIVERIES = 0;
const ONE_DELIVERY = 1;
const SAVE_ATTEMPT = 1;

void test('pending and unavailable status cannot authorize draft Discard' /** @returns Completion checking native-status-driven availability without changing the retained draft. */, async () => {
  let status = { ...RETAINED, pending: true };
  let unavailable = false;
  let discards = NO_DISCARDS;
  const controller = new ConfigureRecoveryController({
    /** @returns Current authoritative state or transport failure. */
    status: () =>
      unavailable
        ? Promise.reject(new Error(UNAVAILABLE))
        : Promise.resolve(status),
    /** @returns Same editor recovered without a Save submission. */
    retry: () => Promise.resolve(RECOVERED),
    /** @returns Explicit dismissal receipt. */
    discard: () => {
      discards += ONE_DISCARD;
      return Promise.resolve({ ...RETAINED, occupied: false, needed: false });
    },
  });
  await controller.refresh();
  await controller.discard();
  assert.equal(discards, NO_DISCARDS);
  status = RETAINED;
  unavailable = true;
  await controller.refresh();
  await controller.discard();
  assert.equal(discards, NO_DISCARDS);
  assert.equal(controller.getSnapshot().needed, true);
  assert.match(controller.getSnapshot().error, /Native status unavailable/);
});

void test('disconnect ignores late native responses and reconnect can recover availability' /** @returns Completion checking owned subscription/timer cleanup and remount behavior. */, async () => {
  /** @param _status Native deferred response. @returns Never before registration. */
  let deliver: (status: ConfigureRecoveryStatus) => void = (): void => {
    throw new Error(DEFERRED_NOT_READY);
  };
  const deferred = new Promise<ConfigureRecoveryStatus>(
    /** @param resolve Deferred query response. */ (resolve) => {
      deliver = resolve;
    },
  );
  let late = true;
  let deliveries = NO_DELIVERIES;
  const controller = new ConfigureRecoveryController({
    /** @returns A late query or current native recovery state. */
    status: () => (late ? deferred : Promise.resolve(RETAINED)),
    /** @returns Current successful placement. */
    retry: () => Promise.resolve(RECOVERED),
    /** @returns Explicit dismissal. */
    discard: () => Promise.resolve({ ...RECOVERED, occupied: false }),
  });
  controller.subscribe(
    /** Records mounted view deliveries. */ () => {
      deliveries += ONE_DELIVERY;
    },
  );
  const disconnect = controller.connect();
  disconnect();
  deliver(RETAINED);
  await deferred;
  assert.equal(deliveries, NO_DELIVERIES);
  late = false;
  const reconnect = controller.connect();
  await controller.refresh();
  assert.equal(controller.getSnapshot().needed, true);
  reconnect();
});

void test('Retry keeps the same editor and ignores an older failed-placement status' /** @returns Completion checking response ordering through the public recovery transport. */, async () => {
  let delayed = false;
  /** @param _status Native deferred response. @returns Never before registration. */
  let deliver: (status: ConfigureRecoveryStatus) => void = (): void => {
    throw new Error(DEFERRED_NOT_READY);
  };
  const oldStatus = new Promise<ConfigureRecoveryStatus>(
    /** @param resolve Deferred native query. */ (resolve) => {
      deliver = resolve;
    },
  );
  const controller = new ConfigureRecoveryController({
    /** @returns An old in-flight status after the initial recovery message. */
    status: () => (delayed ? oldStatus : Promise.resolve(RETAINED)),
    /** @returns Successful placement with singleton ownership still active. */
    retry: () => Promise.resolve(RECOVERED),
    /** @returns Explicit dismissal; never used by Retry. */
    discard: () => Promise.reject(new Error(UNAVAILABLE)),
  });
  await controller.refresh();
  delayed = true;
  const reading = controller.refresh();
  await controller.retry();
  deliver(RETAINED);
  await reading;
  assert.equal(controller.getSnapshot().needed, false);
  assert.equal(controller.getSnapshot().occupied, true);
});

void test('Discard carries the exact authoritative save receipt presented to the recovering owner' /** @returns Completion verifying that a previously unknown outcome is reconciled before dismissal. */, async () => {
  const completion = {
    attemptId: SAVE_ATTEMPT,
    result: { ok: false, message: UNAVAILABLE },
  } as const;
  let observed: number | null = null;
  const controller = new ConfigureRecoveryController({
    /** @returns Native completion after the editor lost its save reply. */
    status: () => Promise.resolve({ ...RETAINED, completion }),
    /** @returns Same retained editor. */
    retry: () => Promise.resolve(RECOVERED),
    /** @param attemptId Receipt the recovering main view has observed. @returns Explicit idle dismissal. */
    discard: (attemptId) => {
      observed = attemptId;
      return Promise.resolve({ ...RECOVERED, occupied: false, completion });
    },
  });
  await controller.refresh();
  assert.deepEqual(controller.getSnapshot().completion, completion);
  await controller.discard();
  assert.equal(observed, SAVE_ATTEMPT);
  assert.equal(controller.getSnapshot().occupied, false);
});
