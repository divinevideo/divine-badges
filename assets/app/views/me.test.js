import test from "node:test";
import assert from "node:assert/strict";

import { buildMeEmptyStateMarkup } from "./me_empty_state.js";

test("created empty state links to badge creation", () => {
  const markup = buildMeEmptyStateMarkup("created");

  assert.match(markup, /Create your first badge/);
  assert.match(markup, /href="\/new"/);
});

test("awarded empty state links to badge creation", () => {
  const markup = buildMeEmptyStateMarkup("awarded");

  assert.match(markup, /Create a badge/);
  assert.match(markup, /href="\/new"/);
});
