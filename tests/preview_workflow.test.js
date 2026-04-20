import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflowPath = ".github/workflows/pr-preview.yml";

test("wrangler enables Cloudflare Worker preview URLs", () => {
  const wranglerConfig = readFileSync("wrangler.toml", "utf8");

  assert.match(wranglerConfig, /^preview_urls\s*=\s*true$/m);
});

test("PR preview workflow uploads aliased Worker versions without production deploys", () => {
  const workflow = readFileSync(workflowPath, "utf8");

  assert.match(workflow, /pull_request:/);
  assert.match(workflow, /branches:\s+\[main\]/);
  assert.match(workflow, /issues:\s+write/);
  assert.match(workflow, /pull-requests:\s+write/);
  assert.match(workflow, /npm run check/);
  assert.match(workflow, /npm run check:wasm/);
  assert.match(workflow, /Check Cloudflare deployment secrets/);
  assert.match(workflow, /Comment missing preview configuration/);
  assert.match(workflow, /CLOUDFLARE_WORKERS_SUBDOMAIN/);
  assert.match(workflow, /cloudflare\/wrangler-action@v3/);
  assert.match(
    workflow,
    /versions upload --preview-alias pr-\$\{\{ github\.event\.pull_request\.number \}\}/
  );
  assert.match(workflow, /steps\.secrets\.outputs\.configured == 'true'/);
  assert.match(workflow, /steps\.secrets\.outputs\.configured != 'true'/);
  assert.match(workflow, /Extract preview URL/);
  assert.match(workflow, /\$\{alias\}-\$\{worker\}\.\$\{CLOUDFLARE_WORKERS_SUBDOMAIN\}\.workers\.dev/);
  assert.ok(workflow.includes('text.match(/^name\\s*=\\s*"([^"]+)"/m)'));
  assert.doesNotMatch(workflow, /name\\\\s/);
  assert.match(workflow, /actions\/github-script@v7/);
  assert.match(workflow, /badges-preview-url/);
  assert.doesNotMatch(workflow, /npm run deploy/);
  assert.doesNotMatch(workflow, /command:\s*deploy\b/);
});
