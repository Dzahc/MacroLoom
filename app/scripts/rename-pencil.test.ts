import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  ACTION,
  PHASE,
  SOURCE,
  type LibrarySnapshot,
} from '../src/library-model.ts';
import { SAMPLE_MACROS } from '../src/sample-macros.ts';
import {
  RENAME_PENCIL_REASON,
  pencilRenameRequest,
  renamePencilReason,
} from '../src/rename-pencil-model.ts';

const FIRST = 0;
const SELECTED = SAMPLE_MACROS[FIRST];
const SECOND = 1;
const OTHER = SAMPLE_MACROS[SECOND];
const SNAPSHOT: LibrarySnapshot = {
  macros: SAMPLE_MACROS,
  selectedId: SELECTED.id,
  phase: PHASE.idle,
  message: '',
};
const DISABLED_CASES: readonly [Partial<LibrarySnapshot>, string][] = [
  [{ loading: true }, RENAME_PENCIL_REASON.loading],
  [{ phase: PHASE.recording }, RENAME_PENCIL_REASON.session],
  [{ renameSaving: true }, RENAME_PENCIL_REASON.saving],
  [{ confirmationOpen: true }, RENAME_PENCIL_REASON.confirmation],
  [{ deleting: OTHER.id }, RENAME_PENCIL_REASON.deleting],
];

/** Disabled pencils explain the current storage constraint rather than exposing an inert enabled button. */
void test('read-only storage produces a disabled-pencil explanation', () => {
  assert.equal(
    renamePencilReason({ ...SNAPSHOT, writable: false }, SELECTED.id, true),
    RENAME_PENCIL_REASON.storage,
  );
  assert.equal(renamePencilReason(SNAPSHOT, SELECTED.id, true), '');
});

/** Pencil activation always addresses the selected row and never turns into selection or Play. */
void test('pencil activation rejects unselected and loading rows', () => {
  assert.deepEqual(pencilRenameRequest(SNAPSHOT, SELECTED.id), {
    action: ACTION.rename,
    macroId: SELECTED.id,
    source: SOURCE.row,
  });
  assert.equal(pencilRenameRequest(SNAPSHOT, OTHER.id), null);
  assert.equal(
    pencilRenameRequest({ ...SNAPSHOT, loading: true }, SELECTED.id),
    null,
  );
});

/** Current native/library interlocks each produce an actionable disabled reason without changing their existing commands. */
void test('unavailable pencils explain loading, sessions, saves, dialogs, and deletion', () => {
  for (const [changes, reason] of DISABLED_CASES) {
    const state = { ...SNAPSHOT, ...changes };
    assert.equal(renamePencilReason(state, SELECTED.id, true), reason);
    assert.equal(pencilRenameRequest(state, SELECTED.id), null);
  }
  assert.equal(
    renamePencilReason(SNAPSHOT, SELECTED.id, false),
    RENAME_PENCIL_REASON.unavailable,
  );
});
