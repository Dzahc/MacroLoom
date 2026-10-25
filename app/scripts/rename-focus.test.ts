import assert from 'node:assert/strict';
import { test } from 'node:test';
import { RenameController } from '../src/rename-controller.ts';
import { restoreRenameFocus } from '../src/rename-focus.ts';
import { PHASE } from '../src/library-model.ts';
import { SAMPLE_MACROS } from '../src/sample-macros.ts';
import {
  RENAME_INITIAL_ATTEMPT,
  RENAME_ATTEMPT_INCREMENT,
} from '../src/rename-contract.ts';

const FIRST = 0;
const SECOND = 1;
const ORIGIN = SAMPLE_MACROS[FIRST];
const OTHER = SAMPLE_MACROS[SECOND];
const FOCUS = {
  editor: 'rename-editor',
  play: 'toolbar-play',
  record: 'toolbar-record',
  stop: 'toolbar-stop',
} as const;
const SESSION_STARTS = [
  [PHASE.playing, FOCUS.play],
  [PHASE.recording, FOCUS.record],
] as const;
const SKIPPED_REQUESTS = [
  { phase: PHASE.idle, foreground: false },
  { phase: PHASE.playing, foreground: true },
] as const;

/** @returns An editor backed by an inert native transport; focus requests come from its real public lifecycle. */
function editor(): RenameController {
  return new RenameController({
    /** @returns The idle native sequence without performing a write. */
    status: () =>
      Promise.resolve({
        attemptId: RENAME_INITIAL_ATTEMPT,
        pending: false,
        completion: null,
      }),
    /** @returns A successful native acknowledgement for focus lifecycle checks. */
    submit: () =>
      Promise.resolve({
        attemptId: RENAME_INITIAL_ATTEMPT + RENAME_ATTEMPT_INCREMENT,
        result: { ok: true },
        library: null,
        discarded: false,
      }),
  });
}

/** A completed rename must never override a later selected row or the toolbar after playback stops. */
void test('Save/Cancel restores its row once and later Play/Record/Stop keeps toolbar focus', async () => {
  for (const save of [false, true]) {
    const rename = editor();
    let focus: string = FOCUS.editor;
    const target = {
      /** @returns A foreground library document. */
      hasFocus: () => true,
      /** @param macroId Row receiving browser focus. @returns Nothing; represents only the DOM effect. */
      focusRow: (macroId: string) => {
        focus = macroId;
      },
    };
    rename.open(ORIGIN);
    if (save) await rename.save();
    else rename.cancel();
    restoreRenameFocus(rename.takeFocus, PHASE.idle, target);
    assert.equal(focus, ORIGIN.id);
    for (const [phase, destination] of SESSION_STARTS) {
      target.focusRow(OTHER.id);
      focus = destination;
      restoreRenameFocus(rename.takeFocus, phase, target);
      assert.equal(focus, destination);
      focus = FOCUS.stop;
      restoreRenameFocus(rename.takeFocus, PHASE.stopping, target);
      restoreRenameFocus(rename.takeFocus, PHASE.idle, target);
      assert.equal(focus, FOCUS.stop);
    }
  }
});

/** Requests skipped for external focus or a running session must expire instead of replaying on a later Idle transition. */
void test('skipped rename restoration cannot steal focus later', () => {
  for (const skipped of SKIPPED_REQUESTS) {
    const rename = editor();
    const focused: string[] = [];
    let foreground: boolean = skipped.foreground;
    const target = {
      /** @returns The current foreground document boundary. */
      hasFocus: () => foreground,
      /** @param macroId Row receiving an unexpected restoration. @returns Nothing. */
      focusRow: (macroId: string) => {
        focused.push(macroId);
      },
    };
    rename.open(ORIGIN);
    rename.cancel();
    restoreRenameFocus(rename.takeFocus, skipped.phase, target);
    assert.equal(rename.getSnapshot().focusId, null);
    foreground = true;
    restoreRenameFocus(rename.takeFocus, PHASE.idle, target);
    assert.deepEqual(focused, []);
  }
});

/** Consuming one request must not prevent a later Save/Cancel on the same macro from restoring normally. */
void test('each new rename dismissal can restore the same row once', () => {
  const rename = editor();
  const focused: string[] = [];
  const target = {
    /** @returns A foreground library document. */
    hasFocus: () => true,
    /** @param macroId Restored row. @returns Nothing. */
    focusRow: (macroId: string) => {
      focused.push(macroId);
    },
  };
  for (const macro of [ORIGIN, ORIGIN]) {
    rename.open(macro);
    rename.cancel();
    restoreRenameFocus(rename.takeFocus, PHASE.idle, target);
    restoreRenameFocus(rename.takeFocus, PHASE.idle, target);
  }
  assert.deepEqual(focused, [ORIGIN.id, ORIGIN.id]);
});
