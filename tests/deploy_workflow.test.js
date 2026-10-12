import assert from 'node:assert/strict';
import { readFileSync, readdirSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const workflow = readFileSync(new URL('../.github/workflows/deploy.yml', import.meta.url), 'utf8');
const names = readdirSync(new URL('../migrations/', import.meta.url)).filter(name => name.endsWith('.sql'));
const response = names => JSON.stringify([{ success: true, results: names.map(name => ({ name })) }]);

test('production deploy requires a successful SELECT-only D1 migration check', () => {
  const step = workflow.match(/      - name: Check remote D1 migrations\n([\s\S]*?)(?=\n      - name:)/)?.[1];
  assert.ok(step, 'missing remote migration gate');
  assert.ok(workflow.indexOf('Check remote D1 migrations') < workflow.indexOf('      - name: Deploy Worker'));
  assert.match(step, /if: steps.secrets.outputs.configured == 'true' && steps.head.outputs.current == 'true'/);
  assert.match(step, /CLOUDFLARE_ACCOUNT_ID: \$\{\{ secrets.CLOUDFLARE_ACCOUNT_ID \}\}/);
  assert.match(step, /CLOUDFLARE_API_TOKEN: \$\{\{ secrets.CLOUDFLARE_API_TOKEN \}\}/);
  assert.match(step, /run: node scripts\/check-remote-migrations.mjs/);
  const directory = mkdtempSync(join(tmpdir(), 'badges-deploy-test-'));
  try {
    writeFileSync(join(directory, 'npx'), `#!/usr/bin/env node
const expected = ['wrangler', 'd1', 'execute', 'divine-badges', '--remote', '--json', '--command', 'SELECT name FROM d1_migrations'];
if (JSON.stringify(process.argv.slice(2)) !== JSON.stringify(expected)) process.exit(90);
process.stdout.write(process.env.MIGRATION_OUTPUT);
process.exit(Number(process.env.MIGRATION_EXIT));
`, { mode: 0o755 });
    for (const [name, output, status, allowed] of [
      ['applied', response(names), 0, true],
      ['additional applied migration', response([...names, '9999_later.sql']), 0, true],
      ['pending', response(names.slice(1)), 0, false],
      ['no applied migrations', response([]), 0, false],
      ['unrecognized', 'Unexpected output', 0, false],
      ['empty', '', 0, false],
      ['wrong shape', '{}', 0, false],
      ['no results', '[{"success":true}]', 0, false],
      ['failed response', '[{"success":false,"results":[]}]', 0, false],
      ['invalid name', '[{"success":true,"results":[{"name":null}]}]', 0, false],
      ['extra result', JSON.stringify([...JSON.parse(response(names)), { success: false }]), 0, false],
      ['missing table or failed read', 'no such table: d1_migrations', 1, false],
      ['failed despite valid response', response(names), 1, false],
    ]) {
      const result = spawnSync(process.execPath, ['scripts/check-remote-migrations.mjs'], {
        encoding: 'utf8',
        env: { PATH: `${directory}:${process.env.PATH}`, MIGRATION_OUTPUT: output, MIGRATION_EXIT: String(status) },
      });
      assert.equal(result.status === 0, allowed, `${name}: ${result.stdout} ${result.stderr}`);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
