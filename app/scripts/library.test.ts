import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { before, test } from 'node:test';
import { pathToFileURL } from 'node:url';
import { build } from 'esbuild';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import {
  ACTION,
  LABEL,
  PHASE,
  SOURCE,
  TOOLBAR,
  TIME,
  canRequest,
  formatDuration,
  reconcileSelection,
  requestAction,
  type LibrarySnapshot,
} from '../src/library-model.ts';
import { SAMPLE_MACROS, SCENARIOS, SCENARIO } from '../src/library-samples.ts';

let view: typeof import('../src/library-view.tsx');
const SELECTED = SAMPLE_MACROS[0];
const OTHER = SAMPLE_MACROS[1];
const MISSING_ID = 'missing-macro';
const DURATION_CASES = [
  [0, '00:00'],
  [TIME.millisecond - 1, '00:00'],
  [TIME.minute * TIME.millisecond, '01:00'],
  [TIME.hour * TIME.millisecond, '1:00:00'],
  [SAMPLE_MACROS[2].durationMs, '1:02:17'],
] as const;

before(async () => {
  const output = join(process.cwd(), '.quality-output/library-view.mjs');
  await mkdir(join(process.cwd(), '.quality-output'), { recursive: true });
  await build({
    entryPoints: [join(process.cwd(), 'src/library-view.tsx')],
    outfile: output,
    bundle: true,
    platform: 'node',
    format: 'esm',
    packages: 'external',
    jsx: 'automatic',
  });
  view = (await import(pathToFileURL(output).href)) as typeof view;
});

function snapshot(overrides: Partial<LibrarySnapshot> = {}): LibrarySnapshot {
  return {
    macros: SAMPLE_MACROS,
    selectedId: null,
    phase: PHASE.idle,
    message: LABEL.ready,
    ...overrides,
  };
}

function render(state: LibrarySnapshot): string {
  return renderToStaticMarkup(
    createElement(view.LibraryView, {
      snapshot: state,
      onSelect: () => {},
      onAction: () => {},
    }),
  );
}

void test('idle toolbar availability depends on valid selection; Stop is unavailable', () => {
  const emptySelection = snapshot();
  assert.equal(canRequest(emptySelection, ACTION.record), true);
  for (const action of TOOLBAR.filter((name) => name !== ACTION.record))
    assert.equal(canRequest(emptySelection, action), false);
  const selected = snapshot({ selectedId: SELECTED.id });
  for (const action of [
    ACTION.record,
    ACTION.play,
    ACTION.configure,
    ACTION.delete,
  ])
    assert.equal(canRequest(selected, action), true);
  assert.equal(canRequest(selected, ACTION.stop), false);
  assert.equal(
    canRequest(snapshot({ selectedId: MISSING_ID }), ACTION.play),
    false,
  );
});

void test('busy snapshots cannot emit mutating requests even with a selection', () => {
  for (const phase of [
    PHASE.saving,
    PHASE.recording,
    PHASE.playing,
    PHASE.stopping,
  ]) {
    const busy = snapshot({ phase, selectedId: SELECTED.id });
    for (const action of [
      ACTION.record,
      ACTION.play,
      ACTION.configure,
      ACTION.delete,
    ]) {
      assert.equal(requestAction(busy, action, SOURCE.toolbar), null);
    }
    assert.equal(canRequest(busy, ACTION.stop), phase !== PHASE.saving);
  }
});

void test('double click and context actions address the clicked ID without waiting for selection updates', () => {
  const oldSelection = snapshot({ selectedId: SELECTED.id });
  assert.deepEqual(
    requestAction(oldSelection, ACTION.play, SOURCE.doubleClick, OTHER.id),
    { action: ACTION.play, macroId: OTHER.id, source: SOURCE.doubleClick },
  );
  assert.deepEqual(
    requestAction(oldSelection, ACTION.delete, SOURCE.context, OTHER.id),
    { action: ACTION.delete, macroId: OTHER.id, source: SOURCE.context },
  );
  assert.equal(
    requestAction(oldSelection, ACTION.configure, SOURCE.context, MISSING_ID),
    null,
  );
});

void test('selection follows stable identity and clears on removal instead of choosing a successor', () => {
  assert.equal(reconcileSelection(SAMPLE_MACROS, null), null);
  assert.equal(
    reconcileSelection([...SAMPLE_MACROS].reverse(), SELECTED.id),
    SELECTED.id,
  );
  assert.equal(reconcileSelection([OTHER], SELECTED.id), null);
  assert.equal(reconcileSelection([], SELECTED.id), null);
});

void test('duration formatting handles subsecond values, whole minutes and hours', () => {
  DURATION_CASES.forEach(([duration, expected]) =>
    assert.equal(formatDuration(duration), expected),
  );
});

void test('view puts icon-only actions above status and list with accessible names, titles and selection', () => {
  const markup = render(snapshot({ selectedId: SELECTED.id }));
  assert.ok(markup.indexOf('<nav') < markup.indexOf('role="status"'));
  assert.ok(markup.indexOf('role="status"') < markup.indexOf('<ul'));
  const toolbar = markup.slice(
    markup.indexOf('<nav'),
    markup.indexOf('</nav>'),
  );
  const labels = [...toolbar.matchAll(/aria-label="([^"]+)" title=/g)].map(
    (match) => match[1],
  );
  assert.deepEqual(
    labels,
    TOOLBAR.map((action) => LABEL[action]),
  );
  assert.doesNotMatch(toolbar, /<span/);
  assert.match(markup, /aria-pressed="true"/);
  assert.ok(markup.includes(SELECTED.name));
  assert.ok(markup.includes(formatDuration(SELECTED.durationMs)));
});

void test('empty and failed-load samples omit failed rows without adding a product error', () => {
  assert.ok(render(snapshot({ macros: [] })).includes(LABEL.empty));
  const sample = SCENARIOS[SCENARIO.failedLoad];
  assert.ok(sample.failures.length > 0);
  const markup = render(snapshot({ ...sample }));
  sample.failures.forEach((failure) =>
    assert.equal(markup.includes(failure), false),
  );
  assert.ok(markup.includes(SELECTED.name));
});
