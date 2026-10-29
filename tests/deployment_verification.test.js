import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { deployedVersion, verifyDeployment } from '../scripts/verify-deployment.mjs';

const versionId = '11111111-1111-4111-8111-111111111111';
const deploymentId = '22222222-2222-4222-8222-222222222222';
const sourceSha = 'a'.repeat(40);
const inputs = { versionId, sourceSha, accountId: 'b'.repeat(32), token: 'synthetic-test-token' };
const output = { type: 'deploy', version: 1, worker_name: 'divine-badges', version_id: versionId };
const active = () => ({ deployments: [{ id: deploymentId, versions: [{ version_id: versionId, percentage: 100 }] }] });
const version = () => ({ id: versionId, annotations: { 'workers/tag': sourceSha } });

function mock({ deployment = active(), metadata = version(), health = 'ok', status = 200, apiStatus = 200,
  apiSuccess = true, afterHealth = deployment } = {}) {
  const calls = [];
  const fetchImpl = async (url, options) => {
    calls.push({ url, options });
    if (url === 'https://badges.divine.video/healthz') return new Response(health, { status });
    assert.ok(url.startsWith(`https://api.cloudflare.com/client/v4/accounts/${inputs.accountId}/workers/scripts/divine-badges/`));
    assert.equal(options.headers.Authorization, 'Bearer synthetic-test-token');
    assert.equal(options.redirect, 'error');
    const result = url.endsWith('/deployments') ? (calls.length > 3 ? afterHealth : deployment) : metadata;
    return Response.json({ success: apiSuccess, result }, { status: apiStatus });
  };
  return { fetchImpl, calls };
}

test('the deployment receipt identifies the version actually uploaded by this run', () => {
  assert.equal(deployedVersion(JSON.stringify(output)), versionId);
  for (const invalid of ['', '{}', 'invalid', JSON.stringify({ ...output, worker_name: 'other' }),
    JSON.stringify({ ...output, version_id: null }), JSON.stringify({ ...output, version: 2 }),
    `${JSON.stringify(output)}\n${JSON.stringify(output)}`]) {
    assert.throws(() => deployedVersion(invalid));
  }
});

test('verifies the exact sole active version, source annotation and safe health route, then rechecks activation', async () => {
  const { fetchImpl, calls } = mock();
  assert.deepEqual(await verifyDeployment({ ...inputs, fetchImpl }), {
    version_id: versionId, deployment_id: deploymentId, health_url: 'https://badges.divine.video/healthz', health: 'passed',
  });
  assert.equal(calls.length, 4);
  assert.equal(calls[2].options.headers, undefined, 'no provider credential on the public health request');
  assert.equal(calls[2].options.cache, 'no-store');
  assert.equal(calls[2].options.redirect, 'error');
});

test('unverifiable activation, source, provider reads and public health all fail closed', async () => {
  for (const options of [
    { deployment: {} }, { deployment: { deployments: [] } },
    { deployment: { deployments: [{ id: deploymentId, versions: [{ version_id: deploymentId, percentage: 100 }] }] } },
    { deployment: { deployments: [{ id: deploymentId, versions: [{ version_id: versionId, percentage: 50 }] }] } },
    { deployment: { deployments: [{ id: deploymentId, versions: [...active().deployments[0].versions, { version_id: deploymentId, percentage: 0 }] }] } },
    { metadata: { id: versionId } }, { metadata: { ...version(), id: deploymentId } },
    { metadata: { ...version(), annotations: { 'workers/tag': 'c'.repeat(40) } } },
    { apiStatus: 403 }, { apiSuccess: false }, { health: 'not ok' }, { status: 503 },
    { afterHealth: { deployments: [{ ...active().deployments[0], id: versionId }] } },
  ]) {
    await assert.rejects(verifyDeployment({ ...inputs, fetchImpl: mock(options).fetchImpl }));
  }
  await assert.rejects(verifyDeployment({ ...inputs, fetchImpl: async () => { throw new Error('network failure'); } }));
  await assert.rejects(verifyDeployment({ ...inputs, fetchImpl: async () => ({ ok: true, json: async () => { throw new Error('invalid JSON'); } }) }));
});

test('failed and superseded runs retain bounded metadata without contacting production', () => {
  const directory = mkdtempSync(join(tmpdir(), 'badges-release-test-'));
  try {
    const script = new URL('../scripts/verify-deployment.mjs', import.meta.url).pathname;
    for (const [env, expected, exit] of [
      [{ DEPLOY_OUTCOME: 'failure' }, 'failed', 1],
      [{ DEPLOY_OUTCOME: 'skipped' }, 'failed', 1],
      [{ DEPLOY_OUTCOME: 'success', WRANGLER_OUTPUT_FILE_PATH: join(directory, 'missing') }, 'failed', 1],
      [{ DEPLOY_OUTCOME: 'skipped', HEAD_CURRENT: 'false', HEAD_OUTCOME: 'success', OBSERVED_MAIN_SHA: 'c'.repeat(40), NATIVE_OUTCOME: 'success', WASM_OUTCOME: 'success', SECRETS_OUTCOME: 'success' }, 'superseded', 0],
      [{ DEPLOY_OUTCOME: 'skipped', HEAD_CURRENT: 'false', HEAD_OUTCOME: 'failure', NATIVE_OUTCOME: 'success', WASM_OUTCOME: 'success', SECRETS_OUTCOME: 'success' }, 'failed', 1],
      [{ DEPLOY_OUTCOME: 'skipped', HEAD_CURRENT: 'false', HEAD_OUTCOME: 'success', OBSERVED_MAIN_SHA: sourceSha, NATIVE_OUTCOME: 'success', WASM_OUTCOME: 'success', SECRETS_OUTCOME: 'success' }, 'failed', 1],
      [{ DEPLOY_OUTCOME: 'skipped', HEAD_CURRENT: 'false', NATIVE_OUTCOME: 'failure', WASM_OUTCOME: 'success', SECRETS_OUTCOME: 'success' }, 'failed', 1],
    ]) {
      const result = spawnSync(process.execPath, [script], { cwd: directory, encoding: 'utf8', env: {
        GITHUB_SHA: sourceSha, GITHUB_REPOSITORY: 'example/synthetic', GITHUB_RUN_ID: '123', GITHUB_RUN_ATTEMPT: '1',
        ...env,
      } });
      assert.equal(result.status, exit, result.stderr);
      const receipt = JSON.parse(readFileSync(join(directory, 'deployment-result.json'), 'utf8'));
      assert.equal(receipt.result, expected);
      assert.equal(receipt.source_sha, sourceSha);
      assert.equal(receipt.environment, 'production');
      assert.equal(receipt.worker, 'divine-badges');
      assert.equal('token' in receipt, false);
      assert.equal('accountId' in receipt, false);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test('main-head lookup fails closed on failed, empty or malformed remote responses', () => {
  const workflow = readFileSync(new URL('../.github/workflows/deploy.yml', import.meta.url), 'utf8');
  const step = workflow.match(/      - name: Check commit is still the head of main\n([\s\S]*?)(?=\n      - name:)/)[1];
  assert.match(step, /shell: bash/);
  const script = step.split('run: |\n')[1].split('\n').map(line => line.replace(/^          /, '')).join('\n');
  const directory = mkdtempSync(join(tmpdir(), 'badges-head-test-'));
  try {
    writeFileSync(join(directory, 'git'), `#!/usr/bin/env node
if (JSON.stringify(process.argv.slice(2)) !== JSON.stringify(['ls-remote', '--exit-code', 'origin', 'refs/heads/main'])) process.exit(90);
process.stdout.write(process.env.MOCK_HEAD_OUTPUT);
process.exit(Number(process.env.MOCK_HEAD_EXIT));
`, { mode: 0o755 });
    for (const [output, exit, expected] of [
      ['', 1, null], ['', 0, null], ['not-a-sha\\trefs/heads/main\n', 0, null],
      [`${sourceSha}\trefs/heads/main\n`, 1, null],
      [`${sourceSha}\trefs/heads/main\n`, 0, 'true'],
      [`${'c'.repeat(40)}\trefs/heads/main\n`, 0, 'false'],
    ]) {
      const outputs = join(directory, 'outputs');
      writeFileSync(outputs, '');
      const run = spawnSync('bash', ['--noprofile', '--norc', '-e', '-o', 'pipefail', '-c', script], {
        encoding: 'utf8', env: { PATH: `${directory}:${process.env.PATH}`, MOCK_HEAD_OUTPUT: output,
          MOCK_HEAD_EXIT: String(exit), GITHUB_SHA: sourceSha, GITHUB_OUTPUT: outputs },
      });
      assert.equal(run.status === 0, expected !== null, run.stderr);
      const written = readFileSync(outputs, 'utf8');
      if (expected === null) assert.doesNotMatch(written, /current=/);
      else assert.match(written, new RegExp(`current=${expected}`));
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test('workflow verifies after deploy and retains receipts even on failures without widening triggers', () => {
  const workflow = readFileSync(new URL('../.github/workflows/deploy.yml', import.meta.url), 'utf8');
  assert.match(workflow, /push:\n    branches: \[main\]\n  workflow_dispatch:/);
  assert.doesNotMatch(workflow, /pull_request|continue-on-error/);
  assert.match(workflow, /if: github.ref == 'refs\/heads\/main'/);
  assert.match(workflow, /group: deploy-main\n  cancel-in-progress: false/);
  assert.match(workflow, /command: deploy --tag \$\{\{ github.sha \}\}/);
  assert.match(workflow, /wranglerVersion: '4\.81\.1'/);
  assert.match(workflow, /name: Verify and record production release\n        if: always\(\)/);
  assert.match(workflow, /DEPLOY_OUTCOME: \$\{\{ steps.deploy.outcome \}\}/);
  assert.match(workflow, /name: Upload production release receipt\n        if: always\(\)/);
  assert.match(workflow, /if-no-files-found: error/);
  assert.ok(workflow.indexOf('Run native checks') < workflow.indexOf('      - name: Deploy Worker'));
  assert.ok(workflow.indexOf('Run wasm check') < workflow.indexOf('      - name: Deploy Worker'));
  assert.ok(workflow.indexOf('      - name: Deploy Worker') < workflow.indexOf('Verify and record production release'));
  for (const action of workflow.matchAll(/uses: (\S+)/g)) assert.match(action[1], /@[a-f0-9]{40}$/);
});
