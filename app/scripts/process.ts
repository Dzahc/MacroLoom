import { spawnSync } from 'node:child_process';

export type CheckResult = { name: string; ok: boolean };

export function execute(
  name: string,
  command: string,
  args: string[],
  cwd: string,
): CheckResult {
  console.log(`\n--- ${name} ---`);
  const result = spawnSync(command, args, {
    cwd,
    stdio: 'inherit',
    shell: false,
  });
  if (result.error) console.error(`${command}: ${result.error.message}`);
  return { name, ok: !result.error && result.status === 0 };
}

export function summary(results: CheckResult[]): boolean {
  console.log('\nVerification results:');
  for (const result of results)
    console.log(`${result.ok ? 'PASS' : 'FAIL'} ${result.name}`);
  return results.every((result) => result.ok);
}
