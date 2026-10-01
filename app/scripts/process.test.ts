import assert from 'node:assert/strict';
import { test } from 'node:test';
import { execute, summary } from './process.ts';

void test('child failures and missing tools produce failed check results', (context) => {
  context.mock.method(console, 'log', () => undefined);
  context.mock.method(console, 'error', () => undefined);
  assert.equal(
    execute(
      'fixture failure',
      process.execPath,
      ['-e', 'process.exit(3)'],
      process.cwd(),
    ).ok,
    false,
  );
  assert.equal(
    execute(
      'missing fixture tool',
      'macroloom-nonexistent-tool',
      [],
      process.cwd(),
    ).ok,
    false,
  );
});

void test('summary retains all independent failures and successes', (context) => {
  context.mock.method(console, 'log', () => undefined);
  assert.equal(
    summary([
      { name: 'format', ok: false },
      { name: 'tests', ok: true },
      { name: 'complexity', ok: false },
    ]),
    false,
  );
  assert.equal(summary([{ name: 'all checks', ok: true }]), true);
});
