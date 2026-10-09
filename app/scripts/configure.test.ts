import assert from 'node:assert/strict';
import { before, test } from 'node:test';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { build } from 'esbuild';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { ConfigureController } from '../src/configure-controller.ts';
import {
  CONFIGURE_LIMIT,
  CONFIGURE_DEFAULT_PLAYBACK,
  CONFIGURE_FIELD_KEY,
  CONFIGURE_INPUT_ID,
  CONFIGURE_TEXT,
  REPEAT_MODE,
  SPEED_PRESETS,
  type ConfigureDraft,
  type ConfigureResult,
  type ConfigureSnapshot,
} from '../src/configure-contract.ts';
import {
  CONFIGURE_ERROR,
  configureInputs,
  parseInterval,
  parseTotalRuns,
  trimConfigureName,
} from '../src/configure-validation.ts';

const TARGET_ID = '68833c94-3d18-4fb2-8a59-0cc72382b616';
const NAME = 'Fill form';
const NEW_NAME = 'Updated form';
const EMOJI = '🧶';
const UNSUPPORTED_SPEED = 3;
const VALID_COUNT = 5;
const VALID_INTERVAL = '1.001';
const ONE_MILLISECOND = 1;
const EXPECTED_INTERVAL = CONFIGURE_LIMIT.milliseconds + ONE_MILLISECOND;
const FAILURE = 'Storage is read-only. Check permissions and try again.';
const INVALID = 'unfinished';
const FIRST_SUBMISSION_INDEX = 0;
const NO_SUBMISSIONS = 0;
const ONE_SUBMISSION = 1;
const RETRY_SUBMISSIONS = 2;
const ONE_EXTRA_CHARACTER = 1;
const SAFE_INTEGER_INCREMENT = 1n;
const EMPTY_INPUT = '';
const ZERO_COUNT = '0';
const EXCESS_PRECISION_INTERVAL = '0.0005';
const HALF_SECOND = '.5';
const HALF_SECOND_MILLISECONDS = CONFIGURE_LIMIT.milliseconds / 2;
const UNICODE_NEL = '\u0085';
const UNICODE_BOM = '\ufeff';
const INTERNAL_CONTROL = '\u0001';
const INVALID_INTERVALS = [
  EXCESS_PRECISION_INTERVAL,
  '1e3',
  '-1',
  'Infinity',
  'NaN',
  EMPTY_INPUT,
  '9007199254740.992',
] as const;
const INVALID_COUNTS = [
  ZERO_COUNT,
  '-1',
  '1.5',
  '1e3',
  String(BigInt(CONFIGURE_LIMIT.max) + SAFE_INTEGER_INCREMENT),
] as const;
const FORM_BUNDLE = {
  directory: '.quality-output',
  file: 'configure-form.mjs',
  entry: 'src/configure-form.tsx',
  platform: 'node',
  format: 'esm',
  packages: 'external',
  jsx: 'automatic',
} as const;
const MARKUP_PATTERN = {
  nameLabel: new RegExp(`for="${CONFIGURE_INPUT_ID.name}"`),
  nameInput: new RegExp(`<input[^>]*id="${CONFIGURE_INPUT_ID.name}"`),
  maxLength: /maxLength/i,
  disabledCount: new RegExp(
    `id="${CONFIGURE_INPUT_ID.totalRuns}"[^>]*disabled`,
  ),
  disabledInterval: new RegExp(
    `id="${CONFIGURE_INPUT_ID.interval}"[^>]*disabled`,
  ),
  busy: /aria-busy="true"/,
  disabledSubmit: /type="submit"[^>]*disabled/,
} as const;
const BASELINE: ConfigureSnapshot = {
  macroId: TARGET_ID,
  name: NAME,
  playback: { ...CONFIGURE_DEFAULT_PLAYBACK },
};
const SUCCESS: ConfigureResult = { ok: true };
const FAILURE_RESULT: ConfigureResult = { ok: false, message: FAILURE };
let view: typeof import('../src/configure-form.tsx');

before(
  /** Bundles the maintained form for real SSR accessibility checks without adding a test framework. */ async () => {
    const output = join(process.cwd(), FORM_BUNDLE.directory, FORM_BUNDLE.file);
    await mkdir(join(process.cwd(), FORM_BUNDLE.directory), {
      recursive: true,
    });
    await build({
      entryPoints: [join(process.cwd(), FORM_BUNDLE.entry)],
      outfile: output,
      bundle: true,
      platform: FORM_BUNDLE.platform,
      format: FORM_BUNDLE.format,
      packages: FORM_BUNDLE.packages,
      jsx: FORM_BUNDLE.jsx,
    });
    view = (await import(pathToFileURL(output).href)) as typeof view;
  },
);

/** @param controller Local editor owner. @returns Actual form markup, with a no-op dismissal observer. */
function render(controller: ConfigureController): string {
  return renderToStaticMarkup(
    createElement(view.ConfigureForm, {
      controller,
      onClosed: /** Observes no disk/window effects during SSR. */ () => {},
    }),
  );
}

/** @returns A controller, observed draft list, and externally controlled asynchronous acknowledgement. */
function pendingEditor() {
  const emitted: ConfigureDraft[] = [];
  let complete: (
    result: ConfigureResult,
  ) => void = /** Initial no-op until the Promise executor captures completion. */ () => {};
  const promise = new Promise<ConfigureResult>(
    /** @param resolve Future acknowledgement. @returns Nothing. */ (
      resolve,
    ) => {
      complete = resolve;
    },
  );
  const controller = new ConfigureController(
    BASELINE,
    /** @param draft Complete callback value. @returns Controlled pending result. */ (
      draft,
    ) => {
      emitted.push(draft);
      return promise;
    },
  );
  return { controller, emitted, complete };
}

/** Exact decimal parsing preserves milliseconds at safe-range boundaries without rounding or exponential syntax. */
void test('decimal interval parsing is exact, bounded and rejects extra precision', () => {
  assert.equal(parseInterval(VALID_INTERVAL), EXPECTED_INTERVAL);
  const maximum = configureInputs({
    ...BASELINE,
    playback: { ...BASELINE.playback, intervalMs: CONFIGURE_LIMIT.max },
  }).interval;
  assert.equal(parseInterval(maximum), CONFIGURE_LIMIT.max);
  for (const input of INVALID_INTERVALS)
    assert.equal(parseInterval(input), null);
  assert.equal(parseInterval(HALF_SECOND), HALF_SECOND_MILLISECONDS);
  assert.equal(
    parseInterval(ZERO_COUNT),
    CONFIGURE_DEFAULT_PLAYBACK.intervalMs,
  );
  assert.equal(
    parseTotalRuns(String(CONFIGURE_LIMIT.max)),
    CONFIGURE_LIMIT.max,
  );
  for (const input of INVALID_COUNTS) assert.equal(parseTotalRuns(input), null);
});

/** Invalid inactive drafts remain editable but Save uses baseline values and never mutates the source snapshot. */
void test('repeat switching retains raw drafts and falls back only for invalid inactive fields', async () => {
  const editor = pendingEditor();
  editor.controller.edit(CONFIGURE_FIELD_KEY.repeatMode, REPEAT_MODE.fixed);
  editor.controller.edit(CONFIGURE_FIELD_KEY.totalRuns, String(VALID_COUNT));
  editor.controller.edit(CONFIGURE_FIELD_KEY.interval, VALID_INTERVAL);
  editor.controller.edit(CONFIGURE_FIELD_KEY.repeatMode, REPEAT_MODE.once);
  editor.controller.edit(CONFIGURE_FIELD_KEY.repeatMode, REPEAT_MODE.fixed);
  assert.equal(editor.controller.getSnapshot().input.interval, VALID_INTERVAL);
  assert.equal(
    editor.controller.getSnapshot().input.totalRuns,
    String(VALID_COUNT),
  );
  editor.controller.edit(CONFIGURE_FIELD_KEY.totalRuns, INVALID);
  editor.controller.edit(CONFIGURE_FIELD_KEY.interval, INVALID);
  editor.controller.edit(CONFIGURE_FIELD_KEY.repeatMode, REPEAT_MODE.once);
  const submitted = editor.controller.submit();
  assert.deepEqual(
    editor.emitted[FIRST_SUBMISSION_INDEX].playback,
    BASELINE.playback,
  );
  assert.equal(editor.controller.getSnapshot().input.interval, INVALID);
  editor.complete(SUCCESS);
  await submitted;
  assert.equal(BASELINE.name, NAME);
});

/** Valid disabled settings are preserved instead of silently canonicalizing modes to defaults. */
void test('valid inactive values and every speed preset reach a complete draft', async () => {
  for (const speed of SPEED_PRESETS) {
    const emitted: ConfigureDraft[] = [];
    const editor = new ConfigureController(
      BASELINE,
      /** @param draft Valid properties. @returns Successful acknowledgement. */ (
        draft,
      ) => {
        emitted.push(draft);
        return Promise.resolve(SUCCESS);
      },
    );
    editor.edit(CONFIGURE_FIELD_KEY.speed, speed);
    editor.edit(CONFIGURE_FIELD_KEY.totalRuns, String(VALID_COUNT));
    editor.edit(CONFIGURE_FIELD_KEY.interval, VALID_INTERVAL);
    await editor.submit();
    assert.deepEqual(emitted, [
      {
        macroId: TARGET_ID,
        name: NAME,
        playback: {
          speed,
          repeatMode: REPEAT_MODE.once,
          totalRuns: VALID_COUNT,
          intervalMs: EXPECTED_INTERVAL,
        },
      },
    ]);
  }
});

/** Errors are deferred until blur/submit, remain field-specific and block callback delivery. */
void test('invalid active inputs cannot emit and focus the first invalid input', async () => {
  const editor = pendingEditor();
  editor.controller.edit(CONFIGURE_FIELD_KEY.name, EMPTY_INPUT);
  assert.deepEqual(editor.controller.getSnapshot().errors, {});
  editor.controller.blur(CONFIGURE_FIELD_KEY.name);
  assert.equal(
    editor.controller.getSnapshot().errors.name,
    CONFIGURE_ERROR.name,
  );
  editor.controller.edit(CONFIGURE_FIELD_KEY.speed, UNSUPPORTED_SPEED);
  editor.controller.edit(CONFIGURE_FIELD_KEY.repeatMode, REPEAT_MODE.fixed);
  editor.controller.edit(CONFIGURE_FIELD_KEY.totalRuns, ZERO_COUNT);
  editor.controller.edit(
    CONFIGURE_FIELD_KEY.interval,
    EXCESS_PRECISION_INTERVAL,
  );
  await editor.controller.submit();
  const state = editor.controller.getSnapshot();
  assert.equal(state.focus, CONFIGURE_FIELD_KEY.name);
  assert.equal(state.errors.speed, CONFIGURE_ERROR.speed);
  assert.equal(state.errors.totalRuns, CONFIGURE_ERROR.runs);
  assert.equal(state.errors.interval, CONFIGURE_ERROR.interval);
  assert.equal(editor.emitted.length, NO_SUBMISSIONS);
  assert.equal(state.pending, false);
});

/** Native-compatible trimming and scalar counting accept 120 emoji and reject internal controls without UTF-16 maxlength truncation. */
void test('name trimming and character limits agree with Rust Unicode semantics', async () => {
  assert.equal(trimConfigureName(`${UNICODE_NEL}${NAME}${UNICODE_NEL}`), NAME);
  assert.equal(
    trimConfigureName(`${UNICODE_BOM}${NAME}${UNICODE_BOM}`),
    `${UNICODE_BOM}${NAME}${UNICODE_BOM}`,
  );
  const editor = pendingEditor();
  editor.controller.edit(
    CONFIGURE_FIELD_KEY.name,
    EMOJI.repeat(CONFIGURE_LIMIT.name),
  );
  const submitted = editor.controller.submit();
  assert.equal(editor.emitted.length, ONE_SUBMISSION);
  editor.complete(SUCCESS);
  await submitted;
  const invalid = pendingEditor();
  invalid.controller.edit(
    CONFIGURE_FIELD_KEY.name,
    EMOJI.repeat(CONFIGURE_LIMIT.name + ONE_EXTRA_CHARACTER),
  );
  await invalid.controller.submit();
  assert.equal(invalid.emitted.length, NO_SUBMISSIONS);
  invalid.controller.edit(
    CONFIGURE_FIELD_KEY.name,
    `${NAME}${INTERNAL_CONTROL}`,
  );
  await invalid.controller.submit();
  assert.equal(invalid.emitted.length, NO_SUBMISSIONS);
});

/** Pending Save prevents duplicate callbacks, edits and cancellation; failure preserves strings for correction and deliberate Retry. */
void test('async pending guard survives reentrant submission and cancellation, then unlocks retry', async () => {
  const editor = pendingEditor();
  editor.controller.edit(CONFIGURE_FIELD_KEY.name, NEW_NAME);
  const first = editor.controller.submit();
  await editor.controller.submit();
  assert.equal(editor.emitted.length, ONE_SUBMISSION);
  assert.equal(editor.controller.cancel(), false);
  editor.controller.edit(CONFIGURE_FIELD_KEY.name, NAME);
  assert.equal(editor.controller.getSnapshot().input.name, NEW_NAME);
  editor.complete(FAILURE_RESULT);
  await first;
  assert.equal(editor.controller.getSnapshot().message, FAILURE);
  assert.equal(editor.controller.getSnapshot().input.name, NEW_NAME);
  assert.equal(editor.controller.getSnapshot().closed, false);
  await editor.controller.submit();
  assert.equal(editor.emitted.length, RETRY_SUBMISSIONS);
  assert.equal(editor.controller.cancel(), true);
});

/** Rejections are handled without closing; valid unchanged Save emits once and acknowledged success closes permanently. */
void test('throwing consumers retain drafts and success closes even unchanged input', async () => {
  const rejected = new ConfigureController(
    BASELINE,
    /** @returns Rejected async consumer. */ () =>
      Promise.reject(new Error(FAILURE)),
  );
  await rejected.submit();
  assert.equal(rejected.getSnapshot().message, CONFIGURE_TEXT.failure);
  assert.equal(rejected.getSnapshot().closed, false);
  const editor = pendingEditor();
  const submitted = editor.controller.submit();
  editor.complete(SUCCESS);
  await submitted;
  await editor.controller.submit();
  assert.equal(editor.emitted.length, ONE_SUBMISSION);
  assert.equal(editor.controller.getSnapshot().closed, true);
});

/** Consumer field errors are retained, announced, and focusable for correction. */
void test('consumer field errors survive acknowledgement and clear on corrected input', async () => {
  const editor = pendingEditor();
  const submitted = editor.controller.submit();
  editor.complete({
    ok: false,
    message: FAILURE,
    fields: { name: CONFIGURE_ERROR.name },
  });
  await submitted;
  assert.equal(editor.controller.getSnapshot().focus, CONFIGURE_FIELD_KEY.name);
  assert.equal(
    editor.controller.getSnapshot().errors.name,
    CONFIGURE_ERROR.name,
  );
  editor.controller.edit(CONFIGURE_FIELD_KEY.name, NEW_NAME);
  assert.equal(editor.controller.getSnapshot().errors.name, undefined);
});

/** Cancellation and closed owners never emit callbacks, including after previously changed drafts. */
void test('cancel discards local edits without callback delivery', async () => {
  const editor = pendingEditor();
  editor.controller.edit(CONFIGURE_FIELD_KEY.name, NEW_NAME);
  assert.equal(editor.controller.cancel(), true);
  await editor.controller.submit();
  assert.equal(editor.emitted.length, NO_SUBMISSIONS);
});

/** Rendered controls have actual input-label associations, mode-sensitive availability, units, errors and pending-state actions. */
void test('form markup contains accessible labels and conditional field availability', async () => {
  const editor = pendingEditor();
  const initial = render(editor.controller);
  assert.match(initial, MARKUP_PATTERN.nameLabel);
  assert.match(initial, MARKUP_PATTERN.nameInput);
  assert.doesNotMatch(initial, MARKUP_PATTERN.maxLength);
  assert.match(initial, MARKUP_PATTERN.disabledCount);
  assert.match(initial, MARKUP_PATTERN.disabledInterval);
  assert.ok(initial.includes(CONFIGURE_TEXT.interval));
  editor.controller.edit(
    CONFIGURE_FIELD_KEY.repeatMode,
    REPEAT_MODE.indefinite,
  );
  const indefinite = render(editor.controller);
  assert.match(indefinite, MARKUP_PATTERN.disabledCount);
  assert.doesNotMatch(indefinite, MARKUP_PATTERN.disabledInterval);
  editor.controller.edit(CONFIGURE_FIELD_KEY.repeatMode, REPEAT_MODE.fixed);
  assert.doesNotMatch(render(editor.controller), MARKUP_PATTERN.disabledCount);
  const submitted = editor.controller.submit();
  const pending = render(editor.controller);
  assert.match(pending, MARKUP_PATTERN.busy);
  assert.match(pending, MARKUP_PATTERN.disabledSubmit);
  editor.complete(FAILURE_RESULT);
  await submitted;
  assert.ok(render(editor.controller).includes(FAILURE));
});
