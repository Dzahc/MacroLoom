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
  TOOLTIP,
  TIME,
  canRequest,
  formatDuration,
  reconcileSelection,
  requestAction,
  type LibrarySnapshot,
} from '../src/library-model.ts';
import {
  SAMPLE_MACROS,
  SCENARIOS,
  SCENARIO,
  OUTCOME_EXAMPLES,
} from '../src/library-samples.ts';
import { OutcomeQueue } from '../src/outcome-queue.ts';
import { IDLE_PRESENTATION } from '../src/compact-contract.ts';
import { presentedLibrary } from '../src/compact-presentation.ts';
import { SESSION_SAMPLES, SESSION_SCENARIO } from '../src/session-samples.ts';

let view: typeof import('../src/library-view.tsx');
let toastView: typeof import('../src/outcome-toasts.tsx');
const SELECTED = SAMPLE_MACROS[0];
const OTHER = SAMPLE_MACROS[1];
const MISSING_ID = 'missing-macro';
const OUTCOME_ID = { first: 1, second: 2 } as const;
const READ_ONLY = false;
const DURATION_CASES = [
  [0, '00:00'],
  [TIME.millisecond - 1, '00:00'],
  [TIME.minute * TIME.millisecond, '01:00'],
  [TIME.hour * TIME.millisecond, '1:00:00'],
  [SAMPLE_MACROS[2].durationMs, '1:02:17'],
] as const;

before(
  /** Bundles the real TSX view for Node rendering; build/import failures fail the test setup. */
  async () => {
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
    const toastOutput = join(
      process.cwd(),
      '.quality-output/outcome-toasts.mjs',
    );
    await build({
      entryPoints: [join(process.cwd(), 'src/outcome-toasts.tsx')],
      outfile: toastOutput,
      bundle: true,
      platform: 'node',
      format: 'esm',
      packages: 'external',
      jsx: 'automatic',
    });
    toastView = (await import(
      pathToFileURL(toastOutput).href
    )) as typeof toastView;
  },
);

/** @param overrides State fields replacing the idle sample fixture. @returns A typed test snapshot. */
function snapshot(overrides: Partial<LibrarySnapshot> = {}): LibrarySnapshot {
  return {
    macros: SAMPLE_MACROS,
    selectedId: null,
    phase: PHASE.idle,
    message: LABEL.ready,
    ...overrides,
  };
}

/** @param state Supplied library state. @returns Static markup with inert host callbacks. */
function render(state: LibrarySnapshot): string {
  return renderToStaticMarkup(
    createElement(view.LibraryView, {
      snapshot: state,
      onSelect:
        /** Ignores selection requests during static rendering. */ () => {},
      onAction:
        /** Ignores action requests during static rendering. */ () => {},
      onDeleteConfirmed:
        /** Ignores confirmation during static rendering; no dialog is open. */ () => {},
    }),
  );
}

/** @param queue Owner-managed outcome state. @returns Static overlay markup with real queue-head selection. */
function renderToast(queue: OutcomeQueue, compact = false): string {
  return renderToStaticMarkup(
    createElement(toastView.OutcomeToasts, { queue, compact }),
  );
}

/** Active recording retains library nodes and selection while hiding them, and displays its own elapsed clock. */
void test('recording compact view hides the mounted selected library and shows elapsed time without a name', () => {
  const state = {
    ...snapshot({ phase: PHASE.recording, selectedId: SELECTED.id }),
    session: {
      ...IDLE_PRESENTATION,
      phase: PHASE.recording,
      elapsedMs: SELECTED.durationMs,
    },
  };
  const markup = render(state);
  assert.match(markup, /class="library-window compact"/);
  assert.match(markup, /class="macro-library"[^>]*hidden/);
  assert.match(markup, /aria-pressed="true"/);
  assert.match(markup, /status-dot recording/);
  assert.ok(markup.includes('Recording'));
  assert.ok(markup.includes(formatDuration(SELECTED.durationMs)));
  assert.match(markup, /data-action="stop"(?![^>]*disabled)/);
});

/** Native retained compact mode keeps cleanup/Stop presentation even after backend status has already returned idle. */
void test('actual retained compact bounds stay usable until native restoration completes', () => {
  const session = { ...IDLE_PRESENTATION, revision: 1 };
  const retained = presentedLibrary(snapshot(), session, {
    compact: true,
    failed: false,
  });
  assert.equal(retained.phase, PHASE.stopping);
  const markup = render(retained);
  assert.match(markup, /class="library-window compact"/);
  assert.match(markup, /data-action="stop"(?![^>]*disabled)/);
  const restored = presentedLibrary(snapshot(), session, {
    compact: false,
    failed: false,
  });
  assert.equal(restored.phase, PHASE.idle);
  assert.doesNotMatch(render(restored), /class="library-window compact"/);
});

/** Finite/infinite playback, repeat waits and cleanup remain compact while saving restores the mounted library. */
void test('session banners show backend run semantics and saving uses the full view', () => {
  const scenarios = [
    SESSION_SCENARIO.playing,
    SESSION_SCENARIO.infinite,
    SESSION_SCENARIO.interval,
    SESSION_SCENARIO.stopping,
  ] as const;
  for (const name of scenarios) {
    const session = SESSION_SAMPLES[name].status;
    const markup = render({ ...snapshot({ phase: session.phase }), session });
    assert.match(markup, /class="library-window compact"/);
    assert.match(markup, /class="macro-library"[^>]*hidden/);
    assert.match(markup, /data-action="stop"(?![^>]*disabled)/);
    if (session.phase === PHASE.playing) {
      assert.ok(markup.includes(session.macroName ?? 'Playing'));
      assert.ok(
        markup.includes(`Run ${session.run}/${session.totalRuns ?? '∞'}`),
      );
      assert.ok(markup.includes(formatDuration(session.elapsedMs)));
    }
  }
  const interval = SESSION_SAMPLES.interval.status;
  assert.ok(
    render({
      ...snapshot({ phase: PHASE.interval }),
      session: interval,
    }).includes('Next run in 3.0 s'),
  );
  const saving = render(
    snapshot({ phase: PHASE.saving, message: LABEL.saving }),
  );
  assert.doesNotMatch(saving, /class="library-window compact"/);
  assert.doesNotMatch(saving, /class="macro-library"[^>]*hidden/);
});

/** Compact sessions hide both visual ordinary toasts and their live announcement without removing queued data. */
void test('ordinary toast presentation is suppressed while compact', () => {
  const queue = new OutcomeQueue();
  try {
    const outcome = { ...OUTCOME_EXAMPLES[0], id: OUTCOME_ID.first };
    queue.enqueue(outcome);
    const markup = renderToast(queue, true);
    assert.equal(markup.includes(outcome.message), false);
    assert.doesNotMatch(markup, /<aside/);
    assert.equal(queue.getSnapshot()[0], outcome);
  } finally {
    queue.dispose();
  }
});

/** Pending disk deletion retains enabled rows, disables toolbar actions, and announces progress. */
void test('pending deletion keeps rows selectable while actions are disabled', () => {
  const pending = snapshot({
    selectedId: SELECTED.id,
    deleting: SELECTED.id,
    message: LABEL.delete,
  });
  const markup = render(pending);
  const toolbar = markup.slice(
    markup.indexOf('<nav'),
    markup.indexOf('</nav>'),
  );
  const rows = markup.slice(markup.indexOf('<ul'), markup.indexOf('</ul>'));
  for (const action of TOOLBAR) {
    assert.match(toolbar, new RegExp(`data-action="${action}"[^>]*disabled`));
  }
  assert.doesNotMatch(rows, /disabled/);
  assert.ok(rows.includes(SELECTED.id));
  assert.match(markup, /status-dot busy/);
  assert.match(markup, /role="status" aria-live="polite"/);
});

/** Checks the fixed-overlay presentation retains one polite announcement, labeled dismissal, and outcome styling. */
void test('toast overlay presents only the queue head and retains accessible dismissal and announcements', () => {
  const queue = new OutcomeQueue();
  try {
    const empty = renderToast(queue);
    assert.match(empty, /class="toast-overlay"/);
    assert.match(empty, /role="status" aria-live="polite" aria-atomic="true"/);
    assert.doesNotMatch(empty, /<aside/);
    const success = OUTCOME_EXAMPLES[0];
    const failure = OUTCOME_EXAMPLES[1];
    queue.enqueue({ ...success, id: OUTCOME_ID.first });
    queue.enqueue({ ...failure, id: OUTCOME_ID.second });
    const first = renderToast(queue);
    assert.match(first, /class="outcome-toast success"/);
    assert.ok(first.includes(success.message));
    assert.equal(first.includes(failure.message), false);
    assert.ok(first.includes(`aria-label="${LABEL.dismiss}"`));
    assert.doesNotMatch(first, /aria-modal/);
    queue.dismiss();
    const next = renderToast(queue);
    assert.match(next, /class="outcome-toast failure"/);
    assert.ok(next.includes(failure.message));
    assert.equal(next.includes(success.message), false);
  } finally {
    queue.dispose();
  }
});

void test('idle toolbar availability depends on valid selection; Stop is unavailable' /** Checks allowed idle actions for absent, valid, and stale selections. */, () => {
  const emptySelection = snapshot();
  assert.equal(canRequest(emptySelection, ACTION.record), true);
  for (const action of TOOLBAR.filter(
    /** @param name Ordered action. @returns Whether it requires selection or active-session state. */
    (name) => name !== ACTION.record,
  ))
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

void test('busy snapshots cannot emit mutating requests even with a selection' /** Checks busy-phase requests and Stop availability across every supplied phase. */, () => {
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

void test('loading uses progress text and read-only storage preserves selection, playback and property inspection', /** Checks the real view and request boundary while storage writes are unavailable. */ () => {
  const loading = snapshot({ macros: [], loading: true, writable: READ_ONLY });
  assert.ok(render(loading).includes(LABEL.loading));
  assert.equal(render(loading).includes(LABEL.empty), false);
  const readable = snapshot({
    selectedId: SELECTED.id,
    loading: true,
    writable: READ_ONLY,
  });
  assert.equal(canRequest(readable, ACTION.record), false);
  assert.equal(canRequest(readable, ACTION.delete), false);
  assert.equal(canRequest(readable, ACTION.play), true);
  assert.equal(canRequest(readable, ACTION.configure), true);
  assert.ok(
    render(snapshot({ macros: [], loading: false })).includes(LABEL.empty),
  );
});

void test('double click and context actions address the clicked ID without waiting for selection updates' /** Checks explicit clicked IDs take precedence over previous selection while missing IDs are rejected. */, () => {
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

void test('selection follows stable identity and clears on removal instead of choosing a successor' /** Checks reorder/removal/empty transitions preserve only an existing stable ID. */, () => {
  assert.equal(reconcileSelection(SAMPLE_MACROS, null), null);
  assert.equal(
    reconcileSelection([...SAMPLE_MACROS].reverse(), SELECTED.id),
    SELECTED.id,
  );
  assert.equal(reconcileSelection([OTHER], SELECTED.id), null);
  assert.equal(reconcileSelection([], SELECTED.id), null);
});

void test('duration formatting handles subsecond values, whole minutes and hours' /** Checks representative boundaries without rounding partial seconds upward. */, () => {
  DURATION_CASES.forEach(
    /** @param pair Duration milliseconds and expected display text; asserts the formatter result. @returns Nothing. */
    ([duration, expected]) => assert.equal(formatDuration(duration), expected),
  );
});

void test('view puts icon-only actions above status and list with accessible names, titles and selection' /** Checks rendered action ordering, shortcut-aware accessible names, icon-only content, and selection text. */, () => {
  const markup = render(snapshot({ selectedId: SELECTED.id }));
  assert.ok(markup.indexOf('<nav') < markup.indexOf('role="status"'));
  assert.ok(markup.indexOf('role="status"') < markup.indexOf('<ul'));
  const toolbar = markup.slice(
    markup.indexOf('<nav'),
    markup.indexOf('</nav>'),
  );
  const labels = [...toolbar.matchAll(/aria-label="([^"]+)" title=/g)].map(
    /** @param match Rendered label capture. @returns Its accessible-name text. */
    (match) => match[1],
  );
  assert.deepEqual(
    labels,
    TOOLBAR.map(
      /** @param action Ordered action. @returns Its source-defined name including any shortcut hint. */
      (action) => TOOLTIP[action],
    ),
  );
  assert.doesNotMatch(toolbar, /<span/);
  assert.match(markup, /aria-pressed="true"/);
  assert.ok(markup.includes(SELECTED.name));
  assert.ok(markup.includes(formatDuration(SELECTED.durationMs)));
});

void test('empty and failed-load samples omit failed rows without adding a product error' /** Checks empty-state text and logging-only failure scenarios retain valid rows without displaying diagnostics. */, () => {
  assert.ok(render(snapshot({ macros: [] })).includes(LABEL.empty));
  const sample = SCENARIOS[SCENARIO.failedLoad];
  assert.ok(sample.failures.length > 0);
  const markup = render(snapshot({ ...sample }));
  sample.failures.forEach(
    /** @param failure Log-only diagnostic; asserts it is absent from product markup. @returns Nothing. */
    (failure) => assert.equal(markup.includes(failure), false),
  );
  assert.ok(markup.includes(SELECTED.name));
});
