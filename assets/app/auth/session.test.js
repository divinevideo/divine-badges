import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const source = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "session.js"),
  "utf8"
);

function functionBody(name) {
  const start = source.indexOf(`export async function ${name}()`);
  assert.notEqual(start, -1, `${name} is exported`);
  const next = source.indexOf("\nexport ", start + 1);
  return source.slice(start, next === -1 ? source.length : next);
}

test("completeOAuthCallback surfaces provider error callbacks", () => {
  const body = functionBody("completeOAuthCallback");

  assert.match(body, /params\.get\("error"\)/);
  assert.match(body, /params\.get\("error_description"\)/);
  assert.match(body, /throw new Error/);
});

test("bootstrapSession does not exchange OAuth callback codes", () => {
  const body = functionBody("bootstrapSession");

  assert.doesNotMatch(body, /new URLSearchParams/);
  assert.doesNotMatch(body, /exchangeCode\(/);
});
