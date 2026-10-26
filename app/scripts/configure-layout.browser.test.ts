import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test } from 'node:test';
import { pathToFileURL } from 'node:url';
import { build } from 'esbuild';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { ConfigureController } from '../src/configure-controller.ts';
import {
  CONFIGURE_DEFAULT_PLAYBACK,
  CONFIGURE_TEXT,
} from '../src/configure-contract.ts';

const OUTPUT_DIRECTORY = '.quality-output/configure-layout-browser';
const FORM_ENTRY = 'src/configure-form.tsx';
const PROBE_ENTRY = 'scripts/configure-layout-probe.ts';
const FORM_FILE = 'configure-form.mjs';
const PROBE_GLOBAL = 'ConfigureLayoutProbe';
const REPORT_PATTERN = /<pre id="layout-result">([^<]+)<\/pre>/;
const BROWSER_TIMEOUT_MS = 30_000;
const VIRTUAL_TIME_MS = 1000;
const BUFFER_BYTES = 4 * 1024 * 1024;
const BROWSER_ENVIRONMENT = 'MACROLOOM_BROWSER_PATH';
const BROWSER_CANDIDATES = [
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
  '/usr/bin/google-chrome',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
] as const;
const VIEWPORTS = [
  { width: 360, height: 300 },
  { width: 480, height: 620 },
] as const;
const TEXT_SIZES = [100, 200] as const;
const SAVE_STATES = [false, true] as const;
const BASELINE = {
  macroId: '68833c94-3d18-4fb2-8a59-0cc72382b616',
  name: 'Fill form',
  playback: CONFIGURE_DEFAULT_PLAYBACK,
};
const NO_BROWSER =
  'Browser integration requires installed Chromium or MACROLOOM_BROWSER_PATH; deterministic Configure tests still run separately.';
const browser =
  process.env[BROWSER_ENVIRONMENT] ??
  BROWSER_CANDIDATES.find(
    /** @param candidate Known local Chromium executable. @returns Whether this browser integration can run without installing software. */
    (candidate) => existsSync(candidate),
  );

/** @param html Complete iframe document. @returns Attribute-safe markup retaining the original CSS and component output. */
function escapeAttribute(html: string): string {
  return html.replaceAll('&', '&amp;').replaceAll('"', '&quot;');
}

/** @param form Bundled production form. @param pending Whether persistence is awaiting acknowledgement. @returns Actual idle/pending markup; executes no native or disk operation. */
function renderForm(
  form: typeof import('../src/configure-form.tsx'),
  pending: boolean,
): string {
  const controller = new ConfigureController(
    BASELINE,
    /** @returns An intentionally outstanding save acknowledgement for rendering progress. */
    () =>
      new Promise(
        /** Leaves acknowledgement pending so production Save progress remains rendered. */
        () => {},
      ),
  );
  if (pending) void controller.submit();
  return renderToStaticMarkup(
    createElement(form.ConfigureForm, {
      controller,
      /** Rendering cannot close a native window. */
      onClosed: () => {},
    }),
  );
}

/** @param css Current production styles. @param markup Actual controlled form. @param viewport Logical native size. @param textSize Root text percentage. @param pending Expected save state. @returns One isolated document with realistic layout and the exact status text. */
function frame(
  css: string,
  markup: string,
  viewport: (typeof VIEWPORTS)[number],
  textSize: number,
  pending: boolean,
): string {
  const document = `<!doctype html><meta charset="utf-8"><style>${css}html{font-size:${textSize}%}</style><div class="app-layout"><div class="product-column">${markup}</div></div>`;
  return `<iframe title="${viewport.width}px/${textSize}%/${pending ? 'pending' : 'idle'}" data-pending="${pending}" style="width:${viewport.width}px;height:${viewport.height}px" srcdoc="${escapeAttribute(document)}"></iframe>`;
}

/** @param output Isolated artifact directory. @returns Current-source fixtures and browser-side range measurement code; bundle failures fail the regression. */
async function fixturePage(output: string): Promise<string> {
  await build({
    entryPoints: [FORM_ENTRY],
    outfile: join(output, FORM_FILE),
    bundle: true,
    platform: 'node',
    format: 'esm',
    packages: 'external',
    jsx: 'automatic',
  });
  const form = (await import(
    pathToFileURL(join(output, FORM_FILE)).href
  )) as typeof import('../src/configure-form.tsx');
  const probe = await build({
    entryPoints: [PROBE_ENTRY],
    bundle: true,
    write: false,
    platform: 'browser',
    format: 'iife',
    globalName: PROBE_GLOBAL,
  });
  const css = `${await readFile('src/style.css', 'utf8')}\n${await readFile('src/configure.css', 'utf8')}`;
  const frames: string[] = [];
  for (const viewport of VIEWPORTS)
    for (const textSize of TEXT_SIZES)
      for (const pending of SAVE_STATES)
        frames.push(
          frame(css, renderForm(form, pending), viewport, textSize, pending),
        );
  return `<!doctype html><meta charset="utf-8">${frames.join('')}<pre id="layout-result"></pre><script>${probe.outputFiles[0].text}</script><script>window.addEventListener('load',()=>{document.getElementById('layout-result').textContent=JSON.stringify(${PROBE_GLOBAL}.checkConfigureFooters(${JSON.stringify(CONFIGURE_TEXT.saving)}));});</script>`;
}

/** The installed-browser integration measures actual painted progress text; element boxes alone cannot detect overflow into Cancel. */
void test(
  'Configure footer progress text never overlaps actions at minimum size and enlarged text',
  { skip: browser ? false : NO_BROWSER },
  async () => {
    assert.ok(browser);
    assert.ok(
      existsSync(browser),
      `${BROWSER_ENVIRONMENT} does not name an installed browser`,
    );
    const output = join(process.cwd(), OUTPUT_DIRECTORY);
    await mkdir(output, { recursive: true });
    const page = join(output, 'configure-layout.html');
    await writeFile(page, await fixturePage(output));
    const result = spawnSync(
      browser,
      [
        '--headless',
        '--disable-gpu',
        '--no-first-run',
        '--no-default-browser-check',
        '--disable-background-networking',
        `--user-data-dir=${join(output, 'profile')}`,
        '--force-device-scale-factor=1',
        `--virtual-time-budget=${VIRTUAL_TIME_MS}`,
        '--dump-dom',
        pathToFileURL(page).href,
      ],
      {
        encoding: 'utf8',
        windowsHide: true,
        timeout: BROWSER_TIMEOUT_MS,
        maxBuffer: BUFFER_BYTES,
      },
    );
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    const report = result.stdout.match(REPORT_PATTERN);
    assert.ok(report, 'Chromium did not produce footer geometry measurements');
    const measurements: unknown = JSON.parse(report[1]);
    await writeFile(
      join(output, 'result.json'),
      `${JSON.stringify(measurements, null, 2)}\n`,
    );
    assert.deepEqual(measurements, {
      cases: VIEWPORTS.length * TEXT_SIZES.length * SAVE_STATES.length,
      failures: [],
    });
  },
);
