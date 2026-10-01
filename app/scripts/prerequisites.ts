import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { CheckResult } from './process.ts';

function packageVersion(path: string): string {
  const manifest: unknown = JSON.parse(readFileSync(path, 'utf8'));
  if (
    typeof manifest !== 'object' ||
    manifest === null ||
    !('version' in manifest)
  )
    throw new Error(`Missing package version: ${path}`);
  if (typeof manifest.version !== 'string')
    throw new Error(`Invalid package version: ${path}`);
  return manifest.version;
}

export function frontendEnvironment(root: string, app: string): CheckResult {
  try {
    const expected = readFileSync(join(root, '.nvmrc'), 'utf8').trim();
    if (process.version !== `v${expected}`)
      throw new Error(`Node: expected ${expected}, found ${process.version}`);
    const manifest = JSON.parse(
      readFileSync(join(app, 'package.json'), 'utf8'),
    ) as {
      dependencies: Record<string, string>;
      devDependencies: Record<string, string>;
    };
    for (const [name, expectedVersion] of Object.entries({
      ...manifest.dependencies,
      ...manifest.devDependencies,
    })) {
      const actual = packageVersion(
        join(app, 'node_modules', name, 'package.json'),
      );
      const agent = process.env.npm_config_user_agent;
      if (agent && !agent.startsWith('npm/12.1.0 '))
        throw new Error(`npm: expected 12.1.0, found ${agent}`);
      if (actual !== expectedVersion)
        throw new Error(
          `${name}: expected ${expectedVersion}, found ${actual}`,
        );
    }
    return { name: 'Frontend environment', ok: true };
  } catch (error) {
    console.error(
      `${String(error)}. Follow quality-gate setup and run npm ci in app/.`,
    );
    return { name: 'Frontend environment', ok: false };
  }
}

export function nativeEnvironment(root: string): CheckResult {
  const tools = spawnSync('rustup', ['toolchain', 'list'], {
    cwd: root,
    encoding: 'utf8',
  });
  const installed = tools.stdout
    ?.split('\n')
    .some((line) => line.startsWith('1.98.1-'));
  if (tools.error || tools.status !== 0 || !installed) {
    console.error(
      'Rust 1.98.1 is missing. Run rustup toolchain install 1.98.1 --profile minimal --component rustfmt --component clippy.',
    );
    return { name: 'Rust environment', ok: false };
  }
  const components = spawnSync(
    'rustup',
    ['component', 'list', '--toolchain', '1.98.1', '--installed'],
    { cwd: root, encoding: 'utf8' },
  );
  const available =
    /rustfmt/.test(components.stdout) && /clippy/.test(components.stdout);
  if (!available)
    console.error(
      'Missing rustfmt/Clippy: follow docs/agents/quality-gate.md setup.',
    );
  return {
    name: 'Rust environment',
    ok: !components.error && components.status === 0 && available,
  };
}
