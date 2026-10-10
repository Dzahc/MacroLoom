import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  CONFIGURE_DEFAULT_PLAYBACK,
  CONFIGURE_FIELD_KEY,
  CONFIGURE_TEXT,
  type ConfigureCompletion,
  type ConfigureDraft,
  type ConfigureStatus,
} from '../src/configure-contract.ts';
import {
  persistConfigure,
  waitForConfigure,
} from '../src/configure-persistence.ts';
import { ConfigureController } from '../src/configure-controller.ts';
import {
  LibraryController,
  type LibraryTransport,
} from '../src/library-controller.ts';
import { SOURCE } from '../src/library-model.ts';
import { SAMPLE_MACROS } from '../src/sample-macros.ts';

const FIRST = 0;
const FIRST_ATTEMPT = 1;
const SECOND_ATTEMPT = 2;
const ONE_SUBMISSION = 1;
const ONE_OUTCOME = 1;
const NO_OUTCOMES = 0;
const UPDATED_NAME = 'Updated form';
const LOST_ACKNOWLEDGEMENT = 'Acknowledgement unavailable';
const STORAGE_FAILURE =
  'Access denied. Close the locking application and Retry.';
const FIRST_REVISION = 1;
const SECOND_REVISION = 2;
const ORIGINAL = {
  macroId: SAMPLE_MACROS[FIRST].id,
  name: SAMPLE_MACROS[FIRST].name,
  playback: { ...CONFIGURE_DEFAULT_PLAYBACK },
} satisfies ConfigureDraft;
const DRAFT = { ...ORIGINAL, name: UPDATED_NAME };
const SAVED = { ok: true, changed: true, name: UPDATED_NAME } as const;
const COMPLETION: ConfigureCompletion = {
  attemptId: FIRST_ATTEMPT,
  result: SAVED,
  library: null,
};
const DONE: ConfigureStatus = { pending: false, completion: COMPLETION };

void test('lost acknowledgement polls native completion without submitting again' /** @returns Completion proving a committed result is recovered using only read-only queries. */, async () => {
  let submissions = 0;
  let ready = false;
  const result = await persistConfigure(DRAFT, {
    /** @returns Lost response after a single native submission. */
    submit: () => {
      submissions += ONE_SUBMISSION;
      return Promise.reject(new Error(LOST_ACKNOWLEDGEMENT));
    },
    /** @returns Pending state until the simulated worker finishes. */
    status: () =>
      Promise.resolve(ready ? DONE : { pending: true, completion: null }),
    /** @returns Completion of the controllable recovery wait. */
    wait: () => {
      ready = true;
      return Promise.resolve();
    },
  });
  assert.deepEqual(result, SAVED);
  assert.equal(submissions, ONE_SUBMISSION);
});

void test('unknown outcome preserves the submitted draft until Retry reconciles it' /** @returns Completion checking that later edits cannot be silently discarded by a recovered old receipt. */, async () => {
  let reachable = false;
  const controller = new ConfigureController(
    ORIGINAL,
    /** @param draft Submitted properties. @returns An uncertain result or the subsequently reachable native receipt. */
    (draft, recovering) =>
      persistConfigure(
        draft,
        {
          /** @returns A lost acknowledgement until recovery is reachable. */
          submit: () =>
            reachable
              ? Promise.resolve(SAVED)
              : Promise.reject(new Error(LOST_ACKNOWLEDGEMENT)),
          /** @returns A failed recovery query. */
          status: () =>
            reachable
              ? Promise.resolve(DONE)
              : Promise.reject(new Error(LOST_ACKNOWLEDGEMENT)),
          /** @returns No delay; status query failure ends this attempt. */
          wait: () => Promise.resolve(),
        },
        recovering,
      ),
  );
  controller.edit(CONFIGURE_FIELD_KEY.name, UPDATED_NAME);
  await controller.submit();
  assert.equal(controller.getSnapshot().uncertain, true);
  assert.equal(controller.getSnapshot().message, CONFIGURE_TEXT.uncertain);
  assert.equal(controller.getSnapshot().closed, false);
  assert.equal(controller.cancel(), false);
  controller.edit(CONFIGURE_FIELD_KEY.name, ORIGINAL.name);
  assert.equal(controller.getSnapshot().input.name, UPDATED_NAME);
  reachable = true;
  await controller.submit();
  assert.equal(controller.getSnapshot().closed, true);
});

void test('uncertain Retry waits for pending work without resubmission or dismissal' /** @returns Completion verifying edits cannot unlock before the outstanding worker outcome is known. */, async () => {
  let reachable = false;
  let finished = false;
  let submissions = NO_OUTCOMES;
  let release: () => void =
    /** Initial inactive completion release. */ () => {};
  const waiting = new Promise<void>(
    /** @param resolve Worker completion release. @returns Nothing; retains the controllable wait. */ (
      resolve,
    ) => {
      release = resolve;
    },
  );
  const controller = new ConfigureController(
    ORIGINAL,
    /** @param draft Retained values. @param recovering Whether Retry must query the unknown outcome. @returns Completion after authoritative recovery. */
    (draft, recovering) =>
      persistConfigure(
        draft,
        {
          /** @returns The first lost acknowledgement; retries must not reach this boundary. */
          submit: () => {
            submissions += ONE_SUBMISSION;
            return Promise.reject(new Error(LOST_ACKNOWLEDGEMENT));
          },
          /** @returns Unknown, pending or completed state according to worker progress. */
          status: () =>
            reachable
              ? Promise.resolve(
                  finished ? DONE : { pending: true, completion: null },
                )
              : Promise.reject(new Error(LOST_ACKNOWLEDGEMENT)),
          /** @returns A controllable delay while the worker remains active. */
          wait: () => waiting,
        },
        recovering,
      ),
  );
  controller.edit(CONFIGURE_FIELD_KEY.name, UPDATED_NAME);
  await controller.submit();
  reachable = true;
  const retry = controller.submit();
  assert.equal(controller.getSnapshot().pending, true);
  assert.equal(controller.cancel(), false);
  controller.edit(CONFIGURE_FIELD_KEY.name, ORIGINAL.name);
  assert.equal(controller.getSnapshot().input.name, UPDATED_NAME);
  finished = true;
  release();
  await retry;
  assert.equal(submissions, ONE_SUBMISSION);
  assert.equal(controller.getSnapshot().closed, true);
});

void test('lost acknowledgement of a precommit failure retains actionable errors for Retry' /** @returns Completion checking that recovery preserves backend field diagnostics and the current draft. */, async () => {
  const failure = {
    ok: false,
    message: STORAGE_FAILURE,
    fields: { name: STORAGE_FAILURE },
  } as const;
  const result = await persistConfigure(DRAFT, {
    /** @returns A response delivery failure. */
    submit: () => Promise.reject(new Error(LOST_ACKNOWLEDGEMENT)),
    /** @returns The native failed attempt without issuing another write. */
    status: () =>
      Promise.resolve({
        pending: false,
        completion: { ...COMPLETION, result: failure },
      }),
    /** @returns No delay for an already completed failure. */
    wait: () => Promise.resolve(),
  });
  assert.deepEqual(result, failure);
});

void test('editor teardown cancels recovery timers' /** @returns Completion verifying an outstanding recovery delay rejects promptly on teardown. */, async () => {
  const lifetime = new AbortController();
  const waiting = waitForConfigure(lifetime.signal);
  lifetime.abort();
  await assert.rejects(waiting);
});

/** @returns A controller with an inert transport; completed receipts are the public reconciliation boundary under test. */
function libraryController(): LibraryController {
  const library = {
    revision: FIRST_REVISION,
    loading: false,
    writable: true,
    deleting: null,
    macros: [{ ...SAMPLE_MACROS[FIRST], playback: ORIGINAL.playback }],
    failures: [],
  };
  const transport: LibraryTransport = {
    /** @returns An inert subscription cleanup. */
    listen: () => Promise.resolve(/** Releases the inert listener. */ () => {}),
    /** @returns Initial metadata only. */
    load: () => Promise.resolve(library),
    /** @returns An unused action rejection. */
    read: () => Promise.reject(new Error(LOST_ACKNOWLEDGEMENT)),
    /** @returns Unchanged metadata for an unused delete boundary. */
    delete: () => Promise.resolve(library),
  };
  const controller = new LibraryController(transport);
  controller.configured({
    attemptId: FIRST_ATTEMPT,
    result: { ok: true, changed: false },
    library,
  });
  controller.select({ macroId: ORIGINAL.macroId, source: SOURCE.row });
  return controller;
}

void test('committed metadata precedes a single named success toast and keeps selection' /** Checks public library state and outcomes after receipt replay. */, () => {
  const controller = libraryController();
  try {
    assert.equal(controller.queue.getSnapshot().length, NO_OUTCOMES);
    const completion: ConfigureCompletion = {
      attemptId: SECOND_ATTEMPT,
      result: SAVED,
      library: {
        revision: SECOND_REVISION,
        loading: false,
        writable: true,
        deleting: null,
        macros: [
          {
            ...SAMPLE_MACROS[FIRST],
            name: UPDATED_NAME,
            playback: ORIGINAL.playback,
          },
        ],
        failures: [],
      },
    };
    controller.configured(completion);
    controller.configured(completion);
    assert.equal(controller.getSnapshot().selectedId, ORIGINAL.macroId);
    assert.equal(controller.getSnapshot().macros[FIRST].name, UPDATED_NAME);
    assert.equal(controller.queue.getSnapshot().length, ONE_OUTCOME);
    assert.equal(
      controller.queue.getSnapshot()[FIRST].message,
      `${CONFIGURE_TEXT.updated} “${UPDATED_NAME}”`,
    );
    assert.notEqual(controller.queue.getSnapshot()[FIRST].persistent, true);
  } finally {
    controller.dispose();
  }
});

void test('postcommit refresh trouble remains visible after its toast is dismissed' /** Checks that committed success cannot be misreported as preserved prior data. */, () => {
  const controller = libraryController();
  try {
    controller.configured({
      attemptId: SECOND_ATTEMPT,
      result: { ...SAVED, warning: CONFIGURE_TEXT.savedRefreshFailed },
      library: null,
    });
    const outcome = controller.queue.getSnapshot()[FIRST];
    assert.ok(outcome.message.includes(CONFIGURE_TEXT.savedRefreshFailed));
    controller.queue.dismiss();
    assert.equal(
      controller.getSnapshot().message,
      CONFIGURE_TEXT.savedRefreshFailed,
    );
  } finally {
    controller.dispose();
  }
});
