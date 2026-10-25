import assert from 'node:assert/strict';
import { test } from 'node:test';
import { RenameController } from '../src/rename-controller.ts';
import { CONFIGURE_ERROR } from '../src/configure-validation.ts';
import type { RenameCompletion, RenameStatus } from '../src/rename-contract.ts';
import { INITIAL_SESSION } from '../src/session-contract.ts';
import { ACTION, canRequest, PHASE, SOURCE } from '../src/library-model.ts';
import {
  CONFIGURE_DEFAULT_PLAYBACK,
  CONFIGURE_LIMIT,
} from '../src/configure-contract.ts';
import { LibraryController } from '../src/library-controller.ts';
import type { BackendLibraryState } from '../src/macro-contract.ts';
import { RENAME_TEXT } from '../src/rename-contract.ts';

const TARGET = {
  id: 'rename-target',
  name: 'Original',
  durationMs: 0,
  createdAt: '',
};
const INVALID_NAME = '';
const NO_SAVE = 0;
const FIRST_SAVE = 1;
const NEW_NAME = 'Renamed';
const LOST_ACK = 'Reply lost after commit';
const SAVED: RenameCompletion = {
  attemptId: FIRST_SAVE,
  result: { ok: true, changed: true, name: NEW_NAME },
  library: null,
  discarded: false,
};
const FAILED: RenameCompletion = {
  ...SAVED,
  result: { ok: false, message: LOST_ACK },
};
const EMOJI = '\u{1f9f6}';
const CONTROL = '\u0001';
const OVER_LIMIT = CONFIGURE_LIMIT.name + FIRST_SAVE;
const SECOND_SAVE = FIRST_SAVE + FIRST_SAVE;

/** @param completion Native receipt returned by this isolated transport. @returns An editor connected only at the IPC boundary. */
function editor(completion: RenameCompletion): RenameController {
  return new RenameController({
    /** @returns An idle receipt sequence. */
    status: () =>
      Promise.resolve({
        attemptId: NO_SAVE,
        pending: false,
        completion: null,
      }),
    /** @returns The specified native outcome. */
    submit: () => Promise.resolve(completion),
  });
}

void test('rename validates only after Save and keeps an invalid draft editable', /** @returns Verification through the approved public editor or command seam. */ async () => {
  let writes = NO_SAVE;
  const controller = new RenameController({
    /** @returns An idle native status. */
    status: (): Promise<RenameStatus> =>
      Promise.resolve({
        attemptId: NO_SAVE,
        pending: false,
        completion: null,
      }),
    /** @returns An unused receipt; validation must prevent this disk boundary. */
    submit: (): Promise<RenameCompletion> => {
      writes++;
      return Promise.reject(new Error(LOST_ACK));
    },
  });
  controller.open(TARGET);
  controller.change(INVALID_NAME);
  assert.equal(controller.getSnapshot().error, INVALID_NAME);
  await controller.save();
  assert.equal(controller.getSnapshot().error, CONFIGURE_ERROR.name);
  assert.equal(controller.getSnapshot().draft?.name, INVALID_NAME);
  assert.equal(writes, NO_SAVE);
});

void test('ordinary failure retains the editable name, while a waiting start discards it', /** @returns Verification through the approved public editor or command seam. */ async () => {
  for (const discarded of [false, true]) {
    const controller = editor({ ...FAILED, discarded });
    controller.open(TARGET);
    controller.change(NEW_NAME);
    await controller.save();
    assert.equal(
      controller.getSnapshot().draft?.name ?? null,
      discarded ? null : NEW_NAME,
    );
    assert.equal(controller.getSnapshot().pending, false);
  }
});

void test('global start cancels a draft even when session preflight remains idle', /** @returns Verification through the approved public editor or command seam. */ () => {
  const controller = editor(SAVED);
  controller.open(TARGET);
  controller.change(NEW_NAME);
  controller.observe({ ...INITIAL_SESSION, startSequence: FIRST_SAVE });
  assert.equal(controller.getSnapshot().draft, null);
  assert.equal(controller.getSnapshot().focusId, null);
});

void test('name validation uses Unicode scalar limits and existing control restrictions', /** @returns Verification through the approved public editor or command seam. */ async () => {
  const controller = editor(SAVED);
  controller.open(TARGET);
  controller.change(EMOJI.repeat(CONFIGURE_LIMIT.name));
  await controller.save();
  assert.equal(controller.getSnapshot().draft, null);
  for (const invalid of [EMOJI.repeat(OVER_LIMIT), CONTROL]) {
    controller.open(TARGET);
    controller.change(invalid);
    await controller.save();
    assert.equal(controller.getSnapshot().error, CONFIGURE_ERROR.name);
  }
});

void test('a pending start exposes Stop while name-save availability preserves Record and Play', /** @returns Verification through the approved public editor or command seam. */ () => {
  const snapshot = {
    macros: [TARGET],
    selectedId: TARGET.id,
    phase: PHASE.idle,
    message: '',
    renameSaving: true,
    deferredStart: true,
  };
  assert.ok(canRequest(snapshot, ACTION.stop));
  assert.ok(canRequest(snapshot, ACTION.record));
  assert.ok(canRequest(snapshot, ACTION.play));
  assert.ok(!canRequest(snapshot, ACTION.rename));
  assert.ok(!canRequest(snapshot, ACTION.configure));
  assert.ok(!canRequest(snapshot, ACTION.delete));
});

void test('a lost save reply reconciles the committed receipt without writing again', /** @returns Verification through the approved public editor or command seam. */ async () => {
  let writes = NO_SAVE;
  const controller = new RenameController({
    /** @returns The authoritative receipt after the write, even though its acknowledgement failed. */
    status: () =>
      Promise.resolve({
        attemptId: writes,
        pending: false,
        completion: writes ? SAVED : null,
      }),
    /** @returns A failed IPC acknowledgement after the disk boundary committed. */
    submit: () => {
      writes++;
      return Promise.reject(new Error(LOST_ACK));
    },
  });
  controller.open(TARGET);
  controller.change(NEW_NAME);
  await controller.save();
  assert.equal(controller.getSnapshot().draft, null);
  assert.equal(writes, FIRST_SAVE);
});

void test('unknown outcomes freeze edits and Retry reconciles a failure before a new save', /** @returns Verification through the approved public editor or command seam. */ async () => {
  let reachable = true;
  let writes = NO_SAVE;
  const controller = new RenameController({
    /** @returns Native status or transport uncertainty after accepting the first write. */
    status: () =>
      reachable
        ? Promise.resolve({
            attemptId: writes,
            pending: false,
            completion: writes ? FAILED : null,
          })
        : Promise.reject(new Error(LOST_ACK)),
    /** @returns A lost reply first, then a successful deliberate retry. */
    submit: () => {
      writes++;
      if (writes === FIRST_SAVE) {
        reachable = false;
        return Promise.reject(new Error(LOST_ACK));
      }
      return Promise.resolve({ ...SAVED, attemptId: SECOND_SAVE });
    },
  });
  controller.open(TARGET);
  controller.change(NEW_NAME);
  await controller.save();
  assert.ok(controller.getSnapshot().uncertain);
  controller.cancel();
  controller.change(TARGET.name);
  assert.equal(controller.getSnapshot().draft?.name, NEW_NAME);
  reachable = true;
  await controller.save();
  assert.equal(writes, FIRST_SAVE);
  assert.equal(controller.getSnapshot().uncertain, false);
  assert.equal(controller.getSnapshot().pending, false);
  await controller.save();
  assert.equal(writes, SECOND_SAVE);
  assert.equal(controller.getSnapshot().draft, null);
});

void test('Stop cancels a deferred start without discarding an eventual failed-save draft', /** @returns Verification through the approved public editor or command seam. */ async () => {
  let settle: (completion: RenameCompletion) => void = ignoreReceipt;
  const waiting = new Promise<RenameCompletion>(
    /** @param resolve Native worker receipt release. */ (resolve) => {
      settle = resolve;
    },
  );
  const controller = new RenameController({
    /** @returns Available native submission identity. */
    status: () =>
      Promise.resolve({ attemptId: NO_SAVE, pending: false, completion: null }),
    /** @returns A controlled in-flight native write. */
    submit: () => waiting,
  });
  controller.open(TARGET);
  controller.change(NEW_NAME);
  const saving = controller.save();
  await Promise.resolve();
  controller.observe({
    ...INITIAL_SESSION,
    startSequence: FIRST_SAVE,
    renameSaving: true,
    deferredStart: true,
  });
  controller.observe({
    ...INITIAL_SESSION,
    startSequence: FIRST_SAVE,
    renameSaving: true,
    deferredStart: false,
  });
  settle(FAILED);
  await saving;
  assert.equal(controller.getSnapshot().draft?.name, NEW_NAME);
  assert.equal(controller.getSnapshot().error, LOST_ACK);
});

/** @returns Nothing before the controlled receipt promise installs its resolver. */
function ignoreReceipt(): void {}

void test('rename receipts update confirmed metadata once independently of Configure receipts', /** @returns Public library and toast evidence without optimistic draft updates. */ () => {
  const initial: BackendLibraryState = {
    revision: FIRST_SAVE,
    loading: false,
    writable: true,
    deleting: null,
    macros: [{ ...TARGET, playback: CONFIGURE_DEFAULT_PLAYBACK }],
    failures: [],
  };
  const library = new LibraryController({
    /** @returns An inert backend subscription cleanup. */
    listen: () => Promise.resolve(ignoreReceipt),
    /** @returns Initial backend metadata. */
    load: () => Promise.resolve(initial),
    /** @returns An unused native read rejection. */
    read: () => Promise.reject(new Error(LOST_ACK)),
    /** @returns Unchanged metadata for an unused deletion. */
    delete: () => Promise.resolve(initial),
  });
  try {
    library.configured({
      ...SAVED,
      result: { ok: true, changed: false },
      library: initial,
    });
    library.select({ macroId: TARGET.id, source: SOURCE.row });
    const completion = {
      ...SAVED,
      library: {
        ...initial,
        revision: SECOND_SAVE,
        macros: [{ ...initial.macros[NO_SAVE], name: NEW_NAME }],
      },
    };
    library.renamed(completion);
    library.renamed(completion);
    assert.equal(library.getSnapshot().macros[NO_SAVE].name, NEW_NAME);
    assert.equal(library.getSnapshot().selectedId, TARGET.id);
    assert.equal(library.queue.getSnapshot().length, FIRST_SAVE);
    assert.equal(
      library.queue.getSnapshot()[NO_SAVE].message,
      `${RENAME_TEXT.updated} “${NEW_NAME}”`,
    );
    library.renamed({ ...FAILED, attemptId: SECOND_SAVE, discarded: true });
    assert.equal(library.queue.getSnapshot().length, FIRST_SAVE);
  } finally {
    library.dispose();
  }
});
