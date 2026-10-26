import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { before, test } from 'node:test';
import { pathToFileURL } from 'node:url';
import { build } from 'esbuild';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { INITIAL_SESSION, SESSION_TEXT } from '../src/session-contract.ts';
import { SessionController } from '../src/session-controller.ts';
import { PHASE } from '../src/library-model.ts';

const OUTPUT_DIRECTORY = '.quality-output/session-render';
const FAILURE = 'Injected release failed; Retry cleanup.';
const BANNER_OPEN = '<section class="library-banner"';
const BANNER_CLOSE = '</section>';
const CONTROLS_ENTRY = 'src/session-controls.tsx';
const VIEW_ENTRY = 'src/library-view.tsx';
const EXPECTED_PLAYBACK_GUIDANCE = 'Playback uses your cursor and keyboard';
const SELECTED_MACRO = {
  id: '68833c94-3d18-4fb2-8a59-0cc72382b616',
  name: 'Selected macro',
  durationMs: 1000,
  createdAt: '2026-10-25T00:00:00Z',
};
const PLAY_CONFLICT_TOOLTIP = 'Play (F10 unavailable)';
let controls: typeof import('../src/session-controls.tsx');
let view: typeof import('../src/library-view.tsx');

/** Play remains a usable button during an F10 conflict, but its tooltip and idle hint tell the truth. */
void test('Play-only conflict hides the F10 hint and explains the button shortcut', () => {
  const markup = renderToStaticMarkup(
    createElement(view.LibraryView, {
      snapshot: {
        macros: [SELECTED_MACRO],
        selectedId: SELECTED_MACRO.id,
        phase: PHASE.idle,
        message: 'Ready',
        shortcuts: { record: true, play: false, stop: true },
      },
      /** Rendering has no native selection side effects. */ onSelect: () => {},
      /** Rendering starts no input session. */ onAction: () => {},
      /** Rendering deletes no data. */ onDeleteConfirmed: () => {},
    }),
  );
  const banner = markup.slice(
    markup.indexOf(BANNER_OPEN),
    markup.indexOf(BANNER_CLOSE),
  );
  assert.match(banner, /Record: F9/);
  assert.doesNotMatch(banner, /Play: F10|Stop: F8/);
  assert.ok(markup.includes(`title="${PLAY_CONFLICT_TOOLTIP}"`));
  assert.doesNotMatch(markup, /data-action="play"[^>]*disabled/);
});

/** Idle presents the owner's brief shared-input reminder without the former paragraph of recording advice. */
void test('idle playback guidance contains only the brief shared-input reminder', () => {
  const snapshot = {
    ...INITIAL_SESSION,
    shortcuts: { record: true, play: true, stop: true },
  };
  const controller = new SessionController({
    /** @returns An inert listener cleanup; rendering owns no native resources. */
    listen: () =>
      Promise.resolve(/** Releases no fixture resources. */ () => {}),
    /** @returns The supplied idle snapshot without starting a session. */
    invoke: () => Promise.resolve(snapshot),
  });
  assert.equal(
    renderToStaticMarkup(
      createElement(controls.SessionControls, { snapshot, controller }),
    ),
    `<div class="session-controls"><p>${EXPECTED_PLAYBACK_GUIDANCE}</p></div>`,
  );
});

/** Bundles the real presentation modules; no native session effects execute during server rendering. */
before(async () => {
  const output = join(process.cwd(), OUTPUT_DIRECTORY);
  await mkdir(output, { recursive: true });
  await build({
    absWorkingDir: process.cwd(),
    entryPoints: [
      join(process.cwd(), CONTROLS_ENTRY),
      join(process.cwd(), VIEW_ENTRY),
    ],
    outdir: output,
    outExtension: { '.js': '.mjs' },
    bundle: true,
    platform: 'node',
    format: 'esm',
    packages: 'external',
    jsx: 'automatic',
    tsconfig: join(process.cwd(), 'tsconfig.json'),
  });
  controls = (await import(
    pathToFileURL(join(output, 'session-controls.mjs')).href
  )) as typeof controls;
  view = (await import(
    pathToFileURL(join(output, 'library-view.mjs')).href
  )) as typeof view;
});

/** Cleanup failures and their retry control render inside the measured native compact banner, rather than below its viewport. */
void test('persistent cleanup recovery is inside the compact banner', () => {
  const snapshot = {
    ...INITIAL_SESSION,
    phase: PHASE.stopping,
    cleanupFailed: true,
    message: FAILURE,
  } as const;
  const controller = new SessionController({
    /** @returns A fixture listener cleanup. */ listen: () =>
      Promise.resolve(
        /** Owns no native subscriptions in this render-only fixture. */ () => {},
      ),
    /** @returns Current backend-derived lifecycle. */ invoke: () =>
      Promise.resolve(snapshot),
  });
  const markup = renderToStaticMarkup(
    createElement(view.LibraryView, {
      snapshot: {
        macros: [],
        selectedId: null,
        phase: PHASE.stopping,
        message: FAILURE,
      },
      sessionControls: createElement(controls.SessionControls, {
        snapshot,
        controller,
      }),
      /** Selection does no native work in the render-only fixture. */ onSelect:
        () => {},
      /** Toolbar callbacks are not executed by static rendering. */ onAction:
        () => {},
      /** Deletion is unavailable in Stopping. */ onDeleteConfirmed: () => {},
    }),
  );
  const start = markup.indexOf(BANNER_OPEN);
  assert.ok(start >= 0);
  const end = markup.indexOf(BANNER_CLOSE, start);
  assert.ok(end > start);
  const banner = markup.slice(start, end);
  assert.match(banner, new RegExp(FAILURE));
  assert.match(banner, new RegExp(SESSION_TEXT.cleanup));
});

/** A failed recording save leaves accessible Retry/Discard controls in the restored full view after its toast expires. */
void test('recording save recovery remains actionable in the full view', () => {
  const snapshot = {
    ...INITIAL_SESSION,
    phase: PHASE.saving,
    saveFailed: true,
    unsaved: true,
    message: FAILURE,
  } as const;
  const controller = new SessionController({
    /** @returns An inert native listener. */
    listen: () => Promise.resolve(/** Owns no native subscription. */ () => {}),
    /** @returns Current retained recording state without claiming persistence. */
    invoke: () => Promise.resolve(snapshot),
  });
  const markup = renderToStaticMarkup(
    createElement(controls.SessionControls, { snapshot, controller }),
  );
  assert.match(markup, /role="alert"/);
  assert.match(markup, new RegExp(SESSION_TEXT.retryRecording));
  assert.match(markup, new RegExp(SESSION_TEXT.discardRecording));
});

/** A pending exit exposes exactly the close-specific recovery choices, avoiding an ordinary discard that also exits. */
void test('close save failure offers explicit exit recovery choices', () => {
  const snapshot = {
    ...INITIAL_SESSION,
    phase: PHASE.saving,
    saveFailed: true,
    unsaved: true,
    closeRequested: true,
    message: FAILURE,
  } as const;
  const controller = new SessionController({
    /** @returns An inert subscription disposer. */
    listen: () => Promise.resolve(/** Owns no native resources. */ () => {}),
    /** @returns The retained failed recording. */
    invoke: () => Promise.resolve(snapshot),
  });
  const markup = renderToStaticMarkup(
    createElement(controls.SessionControls, { snapshot, controller }),
  );
  assert.match(markup, new RegExp(SESSION_TEXT.retryExit));
  assert.match(markup, new RegExp(SESSION_TEXT.keepOpen));
  assert.match(markup, new RegExp(SESSION_TEXT.discard));
  assert.doesNotMatch(markup, new RegExp(SESSION_TEXT.discardRecording));
  assert.doesNotMatch(markup, new RegExp(SESSION_TEXT.retryRecording));
});
