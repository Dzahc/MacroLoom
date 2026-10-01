import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execute, summary, type CheckResult } from './process.ts';
import { frontendEnvironment, nativeEnvironment } from './prerequisites.ts';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const app = join(root, 'app');
const cargoArgs = ['--manifest-path', join(app, 'src-tauri/Cargo.toml')];

function baseRef(args: string[]): string {
  if (args.length === 0) return 'origin/develop';
  if (args.length === 2 && args[0] === '--base-ref' && args[1]) return args[1];
  throw new Error('Usage: npm --prefix app run verify -- [--base-ref <ref>]');
}

function pythonPath(): string {
  const executable =
    process.platform === 'win32' ? 'Scripts/python.exe' : 'bin/python';
  const python = join(root, '.venv', executable);
  if (!existsSync(python))
    throw new Error(
      'Missing .venv. Follow docs/agents/quality-gate.md setup first.',
    );
  return python;
}

function frontendChecks(): CheckResult[] {
  const checks: [string, string, string[]][] = [
    [
      'Prettier',
      'prettier/bin/prettier.cjs',
      ['--ignore-path', join(root, '.prettierignore'), '--check', root],
    ],
    ['TypeScript', 'typescript/bin/tsc', ['--noEmit']],
    ['ESLint', 'eslint/bin/eslint.js', ['.', '--max-warnings', '0']],
    ['Frontend build', 'vite/bin/vite.js', ['build']],
  ];
  return checks.map(([name, entry, args]) =>
    execute(
      name,
      process.execPath,
      [join(app, 'node_modules', entry), ...args],
      app,
    ),
  );
}

function nativeChecks(): CheckResult[] {
  return [
    execute('rustfmt', 'cargo', ['fmt', ...cargoArgs, '--', '--check'], root),
    execute(
      'Clippy',
      'cargo',
      [
        'clippy',
        ...cargoArgs,
        '--locked',
        '--offline',
        '--all-targets',
        '--',
        '-D',
        'warnings',
      ],
      root,
    ),
    execute(
      'Rust tests',
      'cargo',
      ['test', ...cargoArgs, '--locked', '--offline', '--all-targets'],
      root,
    ),
  ];
}

export function verify(base: string): boolean {
  const fetched = execute('Fetch origin', 'git', ['fetch', 'origin'], root);
  const frontend = frontendEnvironment(root, app);
  const native = nativeEnvironment(root);
  const results = [fetched, frontend, native];
  if (frontend.ok) {
    results.push(...frontendChecks());
    results.push(
      execute(
        'Frontend/tooling tests',
        process.execPath,
        ['--test', 'scripts/*.test.ts'],
        app,
      ),
    );
  }
  if (native.ok) results.push(...nativeChecks());
  try {
    const python = pythonPath();
    const environment = execute(
      'Python quality environment',
      python,
      ['-m', 'scripts.quality.environment'],
      root,
    );
    results.push(environment);
    if (environment.ok) {
      results.push(
        execute(
          'Complexity fixtures',
          python,
          ['-m', 'unittest', 'discover', '-s', 'scripts/tests', '-v'],
          root,
        ),
      );
      if (fetched.ok) {
        results.push(
          execute(
            'Changed-function complexity',
            python,
            ['-m', 'scripts.quality.complexity', '--base-ref', base],
            root,
          ),
        );
      } else {
        console.error(
          'SKIP Changed-function complexity: origin fetch failed; no stale-base result accepted.',
        );
      }
    }
  } catch (error) {
    console.error(String(error));
    results.push({ name: 'Python quality environment', ok: false });
  }
  return summary(results);
}

try {
  process.exitCode = verify(baseRef(process.argv.slice(2))) ? 0 : 1;
} catch (error) {
  console.error(String(error));
  process.exitCode = 1;
}
