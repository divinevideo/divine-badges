import test from "node:test";
import assert from "node:assert/strict";

import {
  sanitizeReturnTo,
  saveReturnTo,
  consumeReturnTo,
} from "./return_to.js";

function fakeStorage() {
  const map = new Map();
  return {
    getItem: (key) => (map.has(key) ? map.get(key) : null),
    setItem: (key, value) => map.set(key, String(value)),
    removeItem: (key) => map.delete(key),
  };
}

test("sanitizeReturnTo accepts same-origin absolute paths", () => {
  assert.equal(sanitizeReturnTo("/me"), "/me");
  assert.equal(sanitizeReturnTo("/b/coord/edit"), "/b/coord/edit");
  assert.equal(sanitizeReturnTo("/p/npub1abc?tab=badges"), "/p/npub1abc?tab=badges");
});

test("sanitizeReturnTo rejects values that could leave the origin", () => {
  assert.equal(sanitizeReturnTo("https://evil.example/phish"), null);
  assert.equal(sanitizeReturnTo("//evil.example/phish"), null);
  assert.equal(sanitizeReturnTo("/\\evil.example"), null);
  assert.equal(sanitizeReturnTo("me"), null);
  assert.equal(sanitizeReturnTo(""), null);
  assert.equal(sanitizeReturnTo(null), null);
  assert.equal(sanitizeReturnTo(undefined), null);
});

test("saveReturnTo stores sanitized paths for consumeReturnTo", () => {
  const storage = fakeStorage();
  saveReturnTo("/b/coord/edit", storage);
  assert.equal(consumeReturnTo(storage), "/b/coord/edit");
});

test("saveReturnTo drops unsafe values instead of storing them", () => {
  const storage = fakeStorage();
  saveReturnTo("/me", storage);
  saveReturnTo("https://evil.example/phish", storage);
  assert.equal(consumeReturnTo(storage), "/me");
});

test("consumeReturnTo clears the stored value and falls back to /me", () => {
  const storage = fakeStorage();
  saveReturnTo("/relays", storage);
  assert.equal(consumeReturnTo(storage), "/relays");
  assert.equal(consumeReturnTo(storage), "/me");
});

test("consumeReturnTo ignores tampered storage values", () => {
  const storage = fakeStorage();
  storage.setItem("dbdg_return_to", "//evil.example/phish");
  assert.equal(consumeReturnTo(storage), "/me");
});
