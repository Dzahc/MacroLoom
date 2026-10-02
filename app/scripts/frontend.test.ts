import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { before, test } from 'node:test';
import { pathToFileURL } from 'node:url';
import { build } from 'esbuild';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import type { Snapshot } from '../src/prototype-view.tsx';

let view: typeof import('../src/prototype-view.tsx');
before(async () => {
  const output = join(process.cwd(), '.quality-output/prototype-view.mjs');
  await mkdir(join(process.cwd(), '.quality-output'), { recursive: true });
  await build({
    absWorkingDir: process.cwd(),
    entryPoints: [join(process.cwd(), 'src/prototype-view.tsx')],
    outfile: output,
    bundle: true,
    platform: 'node',
    format: 'esm',
    packages: 'external',
    jsx: 'automatic',
    tsconfig: join(process.cwd(), 'tsconfig.json'),
  });
  view = (await import(pathToFileURL(output).href)) as typeof view;
});

function snapshot(overrides: Partial<Snapshot> = {}): Snapshot {
  return {
    phase: 'idle',
    eventCount: 8,
    dragMoveCount: 2,
    lastRelease: [-20, 100],
    lastReplayRelease: [-20, 100],
    durationMs: 1234,
    message: 'Ready for playback',
    captureGapMinMs: 0,
    dispatchLatenessP95Ms: 3,
    dispatchLatenessMaxMs: 5,
    stopResponseMs: 2,
    f9Available: true,
    f8Available: true,
    ...overrides,
  };
}

function render(state: Snapshot | null, error = ''): string {
  return renderToStaticMarkup(
    createElement(view.PrototypeView, {
      state,
      error,
      uiResponse: 0,
      onAction: async () => {},
    }),
  );
}

function button(markup: string, label: string): string {
  const match = markup.match(
    new RegExp(`<button[^>]*>[^]*?<span>${label}</span>[^]*?</button>`),
  );
  assert.ok(match);
  return match[0].slice(match[0].lastIndexOf('<button'));
}

void test('idle view retains recording/play controls, signed release pixels, and zero metrics', () => {
  const markup = render(snapshot());
  assert.doesNotMatch(button(markup, 'Record'), /disabled/);
  assert.doesNotMatch(button(markup, 'Play'), /disabled/);
  assert.match(button(markup, 'Stop'), /disabled/);
  assert.match(markup, /8 events · 1.2s/);
  assert.match(markup, /-20, 100 \/ -20, 100/);
  assert.match(markup, /Capture minimum gap<\/span><strong>0 ms/);
  assert.match(markup, /Last UI response<\/span><strong>0 ms/);
});

void test('recording and playback retain compact active controls and hide idle content', () => {
  for (const phase of ['recording', 'playing'] as const) {
    const markup = render(snapshot({ phase }));
    assert.match(button(markup, 'Record'), /disabled/);
    assert.match(button(markup, 'Play'), /disabled/);
    assert.doesNotMatch(button(markup, 'Stop'), /disabled/);
    assert.doesNotMatch(markup, /In-memory take/);
    assert.match(markup, /<time>1.2s<\/time>/);
  }
});

void test('unavailable Stop, empty take, startup and errors preserve their feedback', () => {
  assert.match(
    button(render(snapshot({ f8Available: false })), 'Record'),
    /disabled/,
  );
  assert.match(button(render(snapshot({ eventCount: 0 })), 'Play'), /disabled/);
  assert.match(render(null), /Starting…/);
  assert.match(render(snapshot(), 'Command failed'), /Command failed/);
});
