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
const RETRY_AND_EXIT = 'Retry and exit';
let controls: typeof import('../src/session-controls.tsx');
let view: typeof import('../src/library-view.tsx');

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
  assert.match(markup, new RegExp(RETRY_AND_EXIT));
  assert.match(markup, new RegExp(SESSION_TEXT.keepOpen));
  assert.match(markup, new RegExp(SESSION_TEXT.discard));
  assert.doesNotMatch(markup, new RegExp(SESSION_TEXT.discardRecording));
  assert.doesNotMatch(markup, new RegExp(SESSION_TEXT.retryRecording));
});
