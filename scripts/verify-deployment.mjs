import { readFileSync, writeFileSync, appendFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const worker = 'divine-badges';
const healthUrl = 'https://badges.divine.video/healthz';
const uuid = /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i;

export function deployedVersion(output) {
  const records = output.trim().split('\n').map(line => JSON.parse(line));
  const deployments = records.filter(record => record.type === 'deploy');
  if (deployments.length !== 1 || deployments[0].version !== 1 ||
      deployments[0].worker_name !== worker || !uuid.test(deployments[0].version_id)) {
    throw new Error('Expected one Wrangler deployment receipt for divine-badges.');
  }
  return deployments[0].version_id;
}

// Read only. Do not request bindings, settings, secret values or badge actions.
export async function verifyDeployment({ versionId, sourceSha, accountId, token, fetchImpl = fetch }) {
  if (!uuid.test(versionId) || !/^[a-f0-9]{40}$/.test(sourceSha) ||
      !/^[a-f0-9]{32}$/i.test(accountId) || !token) {
    throw new Error('Missing or malformed deployment verification inputs.');
  }
  const api = `https://api.cloudflare.com/client/v4/accounts/${accountId}/workers/scripts/${worker}`;
  const query = async path => {
    const response = await fetchImpl(`${api}/${path}`, {
      headers: { Authorization: `Bearer ${token}` },
      redirect: 'error', signal: AbortSignal.timeout(15_000),
    });
    if (!response.ok) throw new Error('Could not read Worker release metadata.');
    const body = await response.json();
    if (body.success !== true || !body.result) throw new Error('Invalid Worker release metadata.');
    return body.result;
  };
  const checkActive = async () => {
    const result = await query('deployments');
    const latest = result.deployments?.[0];
    if (!uuid.test(latest?.id) || latest.versions?.length !== 1 ||
        latest.versions[0].version_id !== versionId || latest.versions[0].percentage !== 100) {
      throw new Error('The released version is not the sole active Worker version.');
    }
    return latest.id;
  };
  const deploymentId = await checkActive();
  const version = await query(`versions/${versionId}`);
  if (version.id !== versionId || version.annotations?.['workers/tag'] !== sourceSha) {
    throw new Error('The active Worker version does not identify the checked source commit.');
  }
  const health = await fetchImpl(healthUrl, {
    redirect: 'error', cache: 'no-store', signal: AbortSignal.timeout(15_000),
  });
  if (health.status !== 200 || (await health.text()).trim() !== 'ok') {
    throw new Error('The public Worker health check failed.');
  }
  if (await checkActive() !== deploymentId) {
    throw new Error('The active deployment changed during health verification.');
  }
  return { version_id: versionId, deployment_id: deploymentId, health_url: healthUrl, health: 'passed' };
}

export async function recordRelease(env = process.env) {
  const receipt = {
    source_sha: env.GITHUB_SHA,
    environment: 'production', worker,
    run_url: `https://github.com/${env.GITHUB_REPOSITORY}/actions/runs/${env.GITHUB_RUN_ID}`,
    run_attempt: env.GITHUB_RUN_ATTEMPT,
    observed_at: new Date().toISOString(),
    result: 'failed',
  };
  let failed = false;
  try {
    if (env.DEPLOY_OUTCOME === 'success') {
      const versionId = deployedVersion(readFileSync(env.WRANGLER_OUTPUT_FILE_PATH, 'utf8'));
      receipt.version_id = versionId;
      Object.assign(receipt, await verifyDeployment({
        versionId, sourceSha: env.GITHUB_SHA,
        accountId: env.CLOUDFLARE_ACCOUNT_ID, token: env.CLOUDFLARE_API_TOKEN,
      }));
      receipt.result = 'verified';
    } else if (env.HEAD_CURRENT === 'false' && env.HEAD_OUTCOME === 'success' &&
        /^[a-f0-9]{40}$/.test(env.OBSERVED_MAIN_SHA) && env.OBSERVED_MAIN_SHA !== env.GITHUB_SHA &&
        env.NATIVE_OUTCOME === 'success' &&
        env.WASM_OUTCOME === 'success' && env.SECRETS_OUTCOME === 'success') {
      receipt.result = 'superseded';
      receipt.observed_main_sha = env.OBSERVED_MAIN_SHA;
    } else {
      throw new Error('Deployment did not complete; inspect the preceding failed or cancelled step.');
    }
  } catch {
    // Provider responses and exception text can contain private data. Persist
    // only bounded release metadata; never echo bodies, headers or credentials.
    failed = true;
    receipt.error = 'Deployment or verification failed; production success is not established.';
    console.error(`::error::${receipt.error}`);
  }
  writeFileSync('deployment-result.json', `${JSON.stringify(receipt, null, 2)}\n`);
  if (env.GITHUB_STEP_SUMMARY) {
    appendFileSync(env.GITHUB_STEP_SUMMARY,
      `## Production release\n\nResult: **${receipt.result}**\n\nSource: \`${receipt.source_sha}\`\n\nVersion: \`${receipt.version_id ?? 'not established'}\`\n`);
  }
  if (failed) process.exitCode = 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await recordRelease();
