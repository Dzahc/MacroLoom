import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile } from 'node:fs/promises';
import {
  CONFIGURE_LIMIT,
  CONFIGURE_TEXT,
  CONFIGURE_WINDOW,
  REPEAT_MODE,
  SPEED_PRESETS,
  type ConfigureDraft,
  type ConfigureResult,
} from '../src/configure-contract.ts';
import {
  deliverConfigureSubmission,
  type ConfigureDelivery,
} from '../src/configure-delivery.ts';
import { SAMPLE_MACROS } from '../src/sample-macros.ts';

const SAMPLE_INDEX = 0;
const SPEED_INDEX = 2;
const ATTEMPT = CONFIGURE_LIMIT.firstRun;
const FORGED_ATTEMPT = ATTEMPT + CONFIGURE_LIMIT.firstRun;
const NO_CALLS = 0;
const ONE_CALL = 1;
const TWO_CALLS = 2;
const CLAIM_FAILED = 'No matching unclaimed native draft';
const RESOLVE_FAILED = 'Malformed consumer acknowledgement';
const FORGED_NAME = 'Forged name';
const CAPABILITY_PATH = 'src-tauri/capabilities/configure.json';
const EVENT_PERMISSIONS = [
  'core:default',
  'core:event:default',
  'core:event:allow-emit',
  'core:event:allow-emit-to',
] as const;
const NATIVE_DRAFT: ConfigureDraft = {
  macroId: SAMPLE_MACROS[SAMPLE_INDEX].id,
  name: SAMPLE_MACROS[SAMPLE_INDEX].name,
  playback: {
    speed: SPEED_PRESETS[SPEED_INDEX],
    repeatMode: REPEAT_MODE.once,
    totalRuns: CONFIGURE_LIMIT.firstRun,
    intervalMs: NO_CALLS,
  },
};
const SUCCESS: ConfigureResult = { ok: true };
const FAILURE: ConfigureResult = { ok: false, message: CONFIGURE_TEXT.failure };

/** @returns A native-state stand-in that authorizes one attempt and captures callback acknowledgements independently of event payloads. */
function deliveryHarness() {
  let available = true;
  const results: ConfigureResult[] = [];
  const errors: unknown[] = [];
  const delivery: ConfigureDelivery = {
    /** @param attemptId Event identity. @returns One authoritative draft; forged/replayed notifications reject before consumer work. */
    claim: (attemptId) => {
      if (attemptId !== ATTEMPT || !available)
        return Promise.reject(new Error(CLAIM_FAILED));
      available = false;
      return Promise.resolve(NATIVE_DRAFT);
    },
    /** @param attemptId Claimed identity. @param result Consumer result. @returns Acknowledgement completion. */
    resolve: (attemptId, result) => {
      assert.equal(attemptId, ATTEMPT);
      results.push(result);
      return Promise.resolve();
    },
    /** @param reason Boundary rejection. @returns Nothing; retains diagnostics for assertions. */
    report: (reason) => errors.push(reason),
  };
  return { delivery, results, errors };
}

/** Editor capabilities must not authorize frontend-generated callback notifications. */
void test('editor cannot emit Save notifications through event permissions', async () => {
  const capability = JSON.parse(await readFile(CAPABILITY_PATH, 'utf8')) as {
    windows: string[];
    permissions: string[];
  };
  assert.deepEqual(capability.windows, [CONFIGURE_WINDOW]);
  for (const permission of EVENT_PERMISSIONS)
    assert.ok(!capability.permissions.includes(permission));
});

/** A forged notification never reaches Save or acknowledges an unrelated active callback; event-supplied drafts are ignored. */
void test('forged events cannot supply callback drafts or settle native attempts', async () => {
  const harness = deliveryHarness();
  const received: ConfigureDraft[] = [];
  /** @param draft Claimed native properties. @returns Successful callback while recording the actual delivered draft. */
  const save = (draft: ConfigureDraft) => {
    received.push(draft);
    return Promise.resolve(SUCCESS);
  };
  await deliverConfigureSubmission(
    { attemptId: FORGED_ATTEMPT },
    save,
    harness.delivery,
  );
  assert.equal(received.length, NO_CALLS);
  assert.equal(harness.results.length, NO_CALLS);
  const forged = {
    attemptId: ATTEMPT,
    draft: { ...NATIVE_DRAFT, name: FORGED_NAME },
  };
  await deliverConfigureSubmission(forged, save, harness.delivery);
  assert.deepEqual(received, [NATIVE_DRAFT]);
  assert.deepEqual(harness.results, [SUCCESS]);
});

/** Duplicate notification delivery cannot repeat consumer work or prematurely settle a pending Save. */
void test('replayed events cannot duplicate a pending or completed Save', async () => {
  const harness = deliveryHarness();
  let complete!: (result: ConfigureResult) => void;
  const pending = new Promise<ConfigureResult>(
    /** @param resolve Controlled callback completion. @returns Nothing. */ (
      resolve,
    ) => {
      complete = resolve;
    },
  );
  const received: ConfigureDraft[] = [];
  /** @param draft Claimed draft. @returns Pending Save completion. */
  const save = (draft: ConfigureDraft) => {
    received.push(draft);
    return pending;
  };
  const notification = { attemptId: ATTEMPT };
  const first = deliverConfigureSubmission(
    notification,
    save,
    harness.delivery,
  );
  await deliverConfigureSubmission(notification, save, harness.delivery);
  assert.equal(received.length, ONE_CALL);
  assert.equal(harness.results.length, NO_CALLS);
  complete(SUCCESS);
  await first;
  await deliverConfigureSubmission(notification, save, harness.delivery);
  assert.equal(received.length, ONE_CALL);
  assert.deepEqual(harness.results, [SUCCESS]);
  assert.equal(harness.errors.length, TWO_CALLS);
});

/** Consumer rejections remain recoverable native failures after claim authentication. */
void test('rejected consumers receive a retained failure acknowledgement', async () => {
  const harness = deliveryHarness();
  await deliverConfigureSubmission(
    { attemptId: ATTEMPT },
    /** @returns A rejected consumer outcome. */ () =>
      Promise.reject(new Error(CONFIGURE_TEXT.failure)),
    harness.delivery,
  );
  assert.deepEqual(harness.results, [FAILURE]);
});

/** Invalid acknowledgements fall back once to an actionable failure and disconnected bridges never leak a rejected promise. */
void test('invalid consumer results fall back to failure and handle bridge disconnection', async () => {
  const harness = deliveryHarness();
  let calls = NO_CALLS;
  harness.delivery.resolve =
    /** @param attemptId Active identity. @param result Consumer acknowledgement. @returns Rejection of the first result followed by accepted fallback. */ (
      attemptId,
      result,
    ) => {
      assert.equal(attemptId, ATTEMPT);
      calls++;
      if (calls === ONE_CALL) return Promise.reject(new Error(RESOLVE_FAILED));
      harness.results.push(result);
      return Promise.resolve();
    };
  await deliverConfigureSubmission(
    { attemptId: ATTEMPT },
    /** @returns Consumer success. */ () => Promise.resolve(SUCCESS),
    harness.delivery,
  );
  assert.deepEqual(harness.results, [FAILURE]);
  assert.equal(calls, TWO_CALLS);
  const disconnected = deliveryHarness();
  disconnected.delivery.resolve =
    /** @returns Disconnected acknowledgement rejection. */ () =>
      Promise.reject(new Error(RESOLVE_FAILED));
  await deliverConfigureSubmission(
    { attemptId: ATTEMPT },
    /** @returns Consumer success. */ () => Promise.resolve(SUCCESS),
    disconnected.delivery,
  );
  assert.equal(disconnected.errors.length, TWO_CALLS);
});
