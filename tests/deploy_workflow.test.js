import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const workflow = readFileSync(new URL('../.github/workflows/deploy.yml', import.meta.url), 'utf8');

test('production deploy requires a successful read-only D1 migration check', () => {
  const step = workflow.match(/      - name: Check remote D1 migrations\n([\s\S]*?)(?=\n      - name:)/)?.[1];
  assert.ok(step, 'missing remote migration gate');
  assert.ok(workflow.indexOf('Check remote D1 migrations') < workflow.indexOf('      - name: Deploy Worker'));
  assert.match(step, /if: steps.secrets.outputs.configured == 'true' && steps.head.outputs.current == 'true'/);
  assert.match(step, /CLOUDFLARE_ACCOUNT_ID: \$\{\{ secrets.CLOUDFLARE_ACCOUNT_ID \}\}/);
  assert.match(step, /CLOUDFLARE_API_TOKEN: \$\{\{ secrets.CLOUDFLARE_API_TOKEN \}\}/);
  const script = step.split('        run: |\n')[1].split('\n').map(line => line.replace(/^          /, '')).join('\n');
  const directory = mkdtempSync(join(tmpdir(), 'badges-deploy-test-'));
  try {
    writeFileSync(join(directory, 'npx'), '#!/bin/bash\n[[ "$*" == "wrangler d1 migrations list divine-badges --remote" ]] || exit 90\nprintf "%s\\n" "$MIGRATION_OUTPUT"\nexit "$MIGRATION_EXIT"\n', { mode: 0o755 });
    for (const [name, output, status, allowed] of [
      ['applied', '✅ No migrations to apply!', 0, true],
      ['pending', 'Migrations to be applied:\n0007_push_notified_at.sql', 0, false],
      ['unrecognized', 'Unexpected output', 0, false],
      ['empty', '', 0, false],
      ['failed read', 'Authentication error', 1, false],
      ['failed despite success text', 'No migrations to apply!', 1, false],
    ]) {
      const result = spawnSync('bash', ['-e', '-c', script], {
        encoding: 'utf8',
        env: { PATH: `${directory}:${process.env.PATH}`, MIGRATION_OUTPUT: output, MIGRATION_EXIT: String(status) },
      });
      assert.equal(result.status === 0, allowed, `${name}: ${result.stdout} ${result.stderr}`);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
