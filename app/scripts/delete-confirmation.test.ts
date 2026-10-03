import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DeleteConfirmation } from '../src/delete-confirmation.ts';
import {
  ACTION,
  LABEL,
  PHASE,
  SOURCE,
  TOOLBAR,
  requestAction,
  type LibrarySnapshot,
} from '../src/library-model.ts';
import { SAMPLE_MACROS } from '../src/library-samples.ts';

const TARGET = SAMPLE_MACROS[0];
const OTHER = SAMPLE_MACROS[1];
const IDLE: LibrarySnapshot = {
  macros: SAMPLE_MACROS,
  selectedId: TARGET.id,
  phase: PHASE.idle,
  message: LABEL.ready,
};
const RENAMED = 'Renamed report';

/** Verifies deliberate confirmation addresses the original ID even after selection changes. */
void test('confirmation emits the frozen macro ID once and preserves library data', () => {
  const attempt = new DeleteConfirmation(TARGET, SOURCE.toolbar);
  const emitted: string[] = [];
  /** @param id Confirmed stable identity, observed without executing deletion. */
  function receive(id: string) {
    emitted.push(id);
  }
  const changedSelection = { ...IDLE, selectedId: OTHER.id };
  assert.equal(attempt.confirm(changedSelection, receive), true);
  assert.equal(attempt.confirm(changedSelection, receive), false);
  assert.deepEqual(emitted, [TARGET.id]);
  assert.equal(changedSelection.macros, SAMPLE_MACROS);
});

/** Verifies a consumer cannot confirm the same attempt reentrantly. */
void test('confirmation is already finished when its consumer runs', () => {
  const attempt = new DeleteConfirmation(TARGET, SOURCE.toolbar);
  const emitted: string[] = [];
  /** @param id Frozen ID emitted to a consumer that tries to submit again. */
  function receive(id: string) {
    emitted.push(id);
    assert.equal(attempt.confirm(IDLE, receive), false);
  }
  assert.equal(attempt.confirm(IDLE, receive), true);
  assert.deepEqual(emitted, [TARGET.id]);
});

/** Verifies a throwing consumer cannot leave the destructive confirmation reusable. */
void test('consumer failure does not permit another confirmation', () => {
  const attempt = new DeleteConfirmation(TARGET, SOURCE.toolbar);
  const failure = new Error(RENAMED);
  assert.throws(
    /** Attempts confirmation through a failing consumer. */
    () =>
      attempt.confirm(
        IDLE,
        /** Throws the consumer's failure after receiving confirmation. */ () => {
          throw failure;
        },
      ),
    failure,
  );
  const emitted: string[] = [];
  assert.equal(
    attempt.confirm(
      IDLE,
      /** @param id Unexpected second confirmation. */ (id) => {
        emitted.push(id);
      },
    ),
    false,
  );
  assert.deepEqual(emitted, []);
});

/** Verifies unrelated selection/order updates do not silently retarget or cancel the displayed macro. */
void test('stable target remains confirmable after selection clears and rows reorder', () => {
  const attempt = new DeleteConfirmation(TARGET, SOURCE.context);
  const state = {
    ...IDLE,
    selectedId: null,
    macros: [...SAMPLE_MACROS].reverse(),
  };
  const emitted: string[] = [];
  assert.equal(
    attempt.confirm(
      state,
      /** @param id Stable confirmation output. */ (id) => {
        emitted.push(id);
      },
    ),
    true,
  );
  assert.deepEqual(emitted, [TARGET.id]);
});

/** Verifies the presentation request boundary excludes new operations while confirmation is open. */
void test('an open confirmation blocks library action requests while active Stop remains available', () => {
  const state = { ...IDLE, confirmationOpen: true };
  for (const action of TOOLBAR)
    assert.equal(requestAction(state, action, SOURCE.toolbar), null);
  assert.deepEqual(
    requestAction(
      { ...state, phase: PHASE.playing },
      ACTION.stop,
      SOURCE.toolbar,
    ),
    { action: ACTION.stop, source: SOURCE.toolbar },
  );
});

/** Verifies cancellation finishes only its own attempt and emits no confirmation. */
void test('cancel emits nothing and a reopened dialog can be confirmed independently', () => {
  const attempt = new DeleteConfirmation(TARGET, SOURCE.context);
  const emitted: string[] = [];
  /** @param id Confirmation callback output. */
  function receive(id: string) {
    emitted.push(id);
  }
  attempt.cancel();
  assert.equal(attempt.confirm(IDLE, receive), false);
  assert.deepEqual(emitted, []);
  const reopened = new DeleteConfirmation(TARGET, SOURCE.context);
  assert.equal(reopened.confirm(IDLE, receive), true);
  assert.deepEqual(emitted, [TARGET.id]);
});

/** Verifies known removal, rename, and nonidle updates invalidate the original target. */
void test('changed or unavailable targets cannot confirm even if selection is valid', () => {
  const invalid: readonly LibrarySnapshot[] = [
    { ...IDLE, macros: [OTHER], selectedId: OTHER.id },
    { ...IDLE, macros: [{ ...TARGET, name: RENAMED }, OTHER] },
    ...[PHASE.saving, PHASE.recording, PHASE.playing, PHASE.stopping].map(
      /** @param phase Nonidle phase. @returns A valid library with a session taking precedence. */
      (phase) => ({ ...IDLE, phase }),
    ),
    { ...IDLE, writable: false },
  ];
  for (const state of invalid) {
    const attempt = new DeleteConfirmation(TARGET, SOURCE.toolbar);
    const emitted: string[] = [];
    assert.equal(attempt.isAvailable(state), false);
    assert.equal(
      attempt.confirm(
        state,
        /** @param id Unexpected confirmation output. */ (id) => {
          emitted.push(id);
        },
      ),
      false,
    );
    assert.equal(
      attempt.confirm(
        IDLE,
        /** @param id Unexpected output after invalidation. */ (id) => {
          emitted.push(id);
        },
      ),
      false,
    );
    assert.deepEqual(emitted, []);
  }
});
