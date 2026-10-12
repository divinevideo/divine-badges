import { readdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';

// Wrangler migrations list initializes its bookkeeping table. Execute only a
// SELECT instead, so a missing table blocks deployment without creating it.
try {
  const required = readdirSync(new URL('../migrations/', import.meta.url))
    .filter(name => name.endsWith('.sql'));
  if (required.length === 0) throw new Error('No local migrations found.');
  const query = spawnSync('npx', [
    'wrangler', 'd1', 'execute', 'divine-badges', '--remote', '--json',
    '--command', 'SELECT name FROM d1_migrations',
  ], { encoding: 'utf8', timeout: 60_000 });
  if (query.error || query.status !== 0) {
    throw new Error('Could not read applied D1 migrations. Check access and migration-table existence.');
  }
  let result;
  try {
    result = JSON.parse(query.stdout);
  } catch {
    throw new Error('Invalid JSON in D1 migration response.');
  }
  if (!Array.isArray(result) || result.length !== 1 || result[0]?.success !== true ||
      !Array.isArray(result[0].results) ||
      !result[0].results.every(row => typeof row?.name === 'string' && row.name.length > 0)) {
    throw new Error('Unexpected D1 migration response.');
  }
  const applied = new Set(result[0].results.map(row => row.name));
  const pending = required.filter(name => !applied.has(name));
  if (pending.length > 0) {
    throw new Error(`${pending.length} local D1 migration(s) remain unapplied. Apply migrations separately before deploying.`);
  }
  console.log(`Verified all ${required.length} local D1 migrations are applied.`);
} catch (error) {
  console.error(`::error::${error.message}`);
  process.exitCode = 1;
}
