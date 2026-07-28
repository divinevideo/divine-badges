import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflowPath = ".github/workflows/pr-preview.yml";

// Returns the body of a top-level job so assertions can distinguish which job
// carries the same-repo gate.
function jobBlock(workflow, jobName) {
  const lines = workflow.split("\n");
  const start = lines.indexOf(`  ${jobName}:`);
  assert.notEqual(start, -1, `job "${jobName}" is missing from ${workflowPath}`);

  const body = lines.slice(start + 1);
  const end = body.findIndex((line) => /^ {2}\S/.test(line));

  return (end === -1 ? body : body.slice(0, end)).join("\n");
}

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
  assert.match(workflow, /cargo install worker-build --version 0\.8\.1 --locked/);
  assert.doesNotMatch(workflow, /worker-build --version 0\.6\.6/);
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

test("validation runs for fork pull requests while the preview upload stays gated", () => {
  const workflow = readFileSync(workflowPath, "utf8");
  const checks = jobBlock(workflow, "checks");
  const preview = jobBlock(workflow, "preview");

  assert.doesNotMatch(checks, /head\.repo\.full_name/);
  assert.match(checks, /npm run test:js/);
  assert.match(checks, /npm run check\b/);
  assert.match(checks, /npm run check:wasm/);

  assert.match(
    preview,
    /if:\s*github\.event\.pull_request\.head\.repo\.full_name == github\.repository/
  );
  assert.match(preview, /needs:\s*checks/);
});

test("the JS test command covers the asset tests and gates npm run check", () => {
  const { scripts } = JSON.parse(readFileSync("package.json", "utf8"));

  assert.match(scripts["test:js"], /node --test/);
  assert.ok(
    scripts["test:js"].includes('"tests/**/*.test.js"'),
    "test:js must cover tests/"
  );
  assert.ok(
    scripts["test:js"].includes('"assets/**/*.test.js"'),
    "test:js must cover assets/"
  );
  assert.match(scripts.check, /npm run test:js/);
});
