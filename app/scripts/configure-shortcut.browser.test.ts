import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test } from 'node:test';
import { pathToFileURL } from 'node:url';
import { build } from 'esbuild';

const OUTPUT_DIRECTORY = '.quality-output/configure-shortcut-browser';
const REPORT_PATTERN = /<pre id="shortcut-result">([^<]+)<\/pre>/;
const BROWSER_TIMEOUT_MS = 30_000;
const VIRTUAL_TIME_MS = 2000;
const BUFFER_BYTES = 4 * 1024 * 1024;
const FIRST = 0;
const SUCCESS = 0;
const BROWSER_CANDIDATES = [
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
  '/usr/bin/google-chrome',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
] as const;
const browser =
  process.env.MACROLOOM_BROWSER_PATH ??
  BROWSER_CANDIDATES.find(
    /** @param candidate Known browser installation. @returns Whether integration can run without installing software. */
    (candidate) => existsSync(candidate),
  );

/** @param output Artifact directory. @returns A bundled production-library fixture using only mocked native IPC. */
async function fixturePage(output: string): Promise<string> {
  const probe = await build({
    entryPoints: ['scripts/configure-shortcut-probe.ts'],
    bundle: true,
    write: false,
    platform: 'browser',
    format: 'iife',
    globalName: 'ConfigureShortcutProbe',
    jsx: 'automatic',
    define: { 'process.env.NODE_ENV': '"production"' },
  });
  const css = `${await readFile('src/style.css', 'utf8')}\n${await readFile('src/rename.css', 'utf8')}`;
  const page = join(output, 'shortcut.html');
  await writeFile(
    page,
    `<!doctype html><meta charset="utf-8"><style>${css}</style><button id="outside">Outside</button><div id="root"></div><pre id="shortcut-result">pending</pre><script>${probe.outputFiles[FIRST].text}</script><script>ConfigureShortcutProbe.run().catch(error=>{document.getElementById('shortcut-result').textContent=JSON.stringify({failures:[String(error)]});});</script>`,
  );
  return page;
}

/** Real browser focus and React events verify the agreed public library/editor seam without touching saved macros or desktop input. */
void test(
  'row Configure opens the existing editor and restores its own focus after dismissal',
  {
    skip: browser
      ? false
      : 'Requires installed Chromium or MACROLOOM_BROWSER_PATH.',
  },
  async () => {
    assert.ok(browser);
    const output = join(process.cwd(), OUTPUT_DIRECTORY);
    await mkdir(output, { recursive: true });
    const page = await fixturePage(output);
    const result = spawnSync(
      browser,
      [
        '--headless',
        '--disable-gpu',
        '--no-first-run',
        '--no-default-browser-check',
        '--disable-background-networking',
        `--user-data-dir=${join(output, 'profile')}`,
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
    assert.equal(result.status, SUCCESS, result.stderr);
    const report = result.stdout.match(REPORT_PATTERN);
    assert.ok(report, 'Browser did not report Configure shortcut observations');
    const measurements: unknown = JSON.parse(report[1]);
    await writeFile(
      join(output, 'result.json'),
      `${JSON.stringify(measurements, null, 2)}\n`,
    );
    assert.deepEqual(measurements, { failures: [] });
  },
);
