import test from "node:test";
import assert from "node:assert/strict";

import { DIVINE_RELAY } from "./constants.js";
import {
  LOCAL_RELAYS_STORAGE_KEY,
  publishSignedToWriteRelays,
  publishSucceeded,
  readLocalRelays,
  summarizePublishResult,
  writeLocalRelays,
} from "./publish.js";

function buildMockStorage(initial = {}) {
  const store = new Map(Object.entries(initial));
  return {
    getItem(key) {
      return store.has(key) ? store.get(key) : null;
    },
    setItem(key, value) {
      store.set(key, String(value));
    },
    removeItem(key) {
      store.delete(key);
    },
    _store: store,
  };
}

function buildSignerStub(signed) {
  const calls = [];
  return {
    calls,
    async signEvent(event) {
      calls.push(event);
      return signed;
    },
  };
}

test("publishSignedToWriteRelays signs exactly once", async () => {
  const signed = { id: "signed-1", sig: "sig", pubkey: "pk", kind: 1, tags: [], content: "", created_at: 1 };
  const signer = buildSignerStub(signed);
  const unsigned = { kind: 1, tags: [], content: "", created_at: 1 };

  await publishSignedToWriteRelays({
    pubkey: "pk",
    unsignedEvent: unsigned,
    signer,
    seedRelays: ["wss://seed.example"],
    discoverFn: async () => ["wss://seed.example"],
    publishManyFn: async () => ({ ok: ["wss://seed.example"], failed: [] }),
  });

  assert.equal(signer.calls.length, 1);
  assert.deepEqual(signer.calls[0], unsigned);
});

test("publishSignedToWriteRelays discovers write relays using the signer pubkey", async () => {
  const discoverCalls = [];
  const publishCalls = [];
  const signed = { id: "signed-2" };
  const signer = buildSignerStub(signed);

  const outcome = await publishSignedToWriteRelays({
    pubkey: "pk-42",
    unsignedEvent: { kind: 1 },
    signer,
    seedRelays: ["wss://seed.one"],
    discoverFn: async ({ pubkeys, seedRelays }) => {
      discoverCalls.push({ pubkeys, seedRelays });
      return ["wss://discovered.one", "wss://discovered.two"];
    },
    publishManyFn: async (relays, event) => {
      publishCalls.push({ relays, event });
      return { ok: relays, failed: [] };
    },
  });

  assert.equal(discoverCalls.length, 1);
  assert.deepEqual(discoverCalls[0].pubkeys, ["pk-42"]);
  assert.deepEqual(discoverCalls[0].seedRelays, ["wss://seed.one"]);
  // Effective relay list = seed relays + discovered relays (deduped, normalized)
  assert.deepEqual(outcome.relays, [
    "wss://seed.one",
    "wss://discovered.one",
    "wss://discovered.two",
  ]);
  assert.deepEqual(publishCalls[0].relays, [
    "wss://seed.one",
    "wss://discovered.one",
    "wss://discovered.two",
  ]);
});

test("publishSignedToWriteRelays defaults seed relays to include DIVINE_RELAY", async () => {
  let observedSeeds = null;
  const signer = buildSignerStub({ id: "signed-3" });

  await publishSignedToWriteRelays({
    pubkey: "pk",
    unsignedEvent: { kind: 1 },
    signer,
    discoverFn: async ({ seedRelays }) => {
      observedSeeds = seedRelays;
      return seedRelays;
    },
    publishManyFn: async () => ({ ok: [], failed: [] }),
  });

  assert.ok(Array.isArray(observedSeeds));
  assert.ok(
    observedSeeds.includes(DIVINE_RELAY),
    `expected default seed relays to include ${DIVINE_RELAY}, got ${JSON.stringify(observedSeeds)}`
  );
});

test("publishSignedToWriteRelays publishes the signed event to each deduped relay", async () => {
  const signed = { id: "signed-4", sig: "sig" };
  const signer = buildSignerStub(signed);
  let passedRelays = null;
  let passedEvent = null;

  await publishSignedToWriteRelays({
    pubkey: "pk",
    unsignedEvent: { kind: 1 },
    signer,
    seedRelays: ["wss://seed.one"],
    discoverFn: async () => [
      "wss://relay.one",
      "wss://relay.one",
      "wss://relay.two",
    ],
    publishManyFn: async (relays, event) => {
      passedRelays = relays;
      passedEvent = event;
      return { ok: ["wss://relay.one", "wss://relay.two"], failed: [] };
    },
  });

  // publishManyFn receives the (possibly duplicated) relay list — relayPublishMany
  // itself dedupes, which is what we rely on. But the helper must pass the exact
  // signed event through.
  assert.equal(passedEvent, signed);
  assert.ok(Array.isArray(passedRelays));
  assert.ok(passedRelays.includes("wss://relay.one"));
  assert.ok(passedRelays.includes("wss://relay.two"));
});

test("publishSignedToWriteRelays returns partial-failure result when some relays accept and some fail", async () => {
  const signer = buildSignerStub({ id: "signed-5" });

  const outcome = await publishSignedToWriteRelays({
    pubkey: "pk",
    unsignedEvent: { kind: 1 },
    signer,
    seedRelays: ["wss://seed.one"],
    discoverFn: async () => ["wss://relay.one", "wss://relay.two"],
    publishManyFn: async () => ({
      ok: ["wss://relay.one"],
      failed: [{ relayUrl: "wss://relay.two", error: "boom" }],
    }),
  });

  assert.deepEqual(outcome.result.ok, ["wss://relay.one"]);
  assert.equal(outcome.result.failed.length, 1);
  assert.equal(outcome.result.failed[0].relayUrl, "wss://relay.two");
});

test("publishSignedToWriteRelays returns all-failed result when no relay accepts", async () => {
  const signer = buildSignerStub({ id: "signed-6" });

  const outcome = await publishSignedToWriteRelays({
    pubkey: "pk",
    unsignedEvent: { kind: 1 },
    signer,
    seedRelays: ["wss://seed.one"],
    discoverFn: async () => ["wss://relay.one", "wss://relay.two"],
    publishManyFn: async () => ({
      ok: [],
      failed: [
        { relayUrl: "wss://relay.one", error: "nope" },
        { relayUrl: "wss://relay.two", error: "nope" },
      ],
    }),
  });

  assert.deepEqual(outcome.result.ok, []);
  assert.equal(outcome.result.failed.length, 2);
});

test("publishSucceeded returns true iff result.ok has entries", () => {
  assert.equal(
    publishSucceeded({ result: { ok: ["wss://relay.one"], failed: [] } }),
    true
  );
  assert.equal(
    publishSucceeded({ result: { ok: [], failed: [{ relayUrl: "x", error: "e" }] } }),
    false
  );
  assert.equal(publishSucceeded(undefined), false);
  assert.equal(publishSucceeded({ result: undefined }), false);
});

test("publishSignedToWriteRelays includes write-enabled local relays in the published relay list", async () => {
  let passedRelays = null;
  const signer = buildSignerStub({ id: "signed-local" });

  await publishSignedToWriteRelays({
    pubkey: "pk",
    unsignedEvent: { kind: 1 },
    signer,
    seedRelays: ["wss://seed.one"],
    localRelays: [
      { url: "wss://local.write", read: false, write: true },
      { url: "wss://local.read-only", read: true, write: false },
    ],
    discoverFn: async () => ["wss://discovered.one"],
    publishManyFn: async (relays, event) => {
      passedRelays = relays;
      return { ok: relays, failed: [] };
    },
  });

  assert.ok(Array.isArray(passedRelays));
  assert.ok(passedRelays.includes("wss://seed.one"));
  assert.ok(passedRelays.includes("wss://discovered.one"));
  assert.ok(passedRelays.includes("wss://local.write"));
  assert.ok(!passedRelays.includes("wss://local.read-only"));
});

test("readLocalRelays returns normalized entries from storage", () => {
  const storage = buildMockStorage({
    [LOCAL_RELAYS_STORAGE_KEY]: JSON.stringify([
      { url: "WSS://Relay.One", read: true, write: false },
      { url: "wss://relay.two", read: false, write: true },
      { url: "not-a-url", read: true, write: true },
    ]),
  });

  const relays = readLocalRelays(storage);
  assert.deepEqual(relays, [
    { url: "wss://relay.one", read: true, write: false },
    { url: "wss://relay.two", read: false, write: true },
  ]);
});

test("readLocalRelays returns [] when storage value is invalid JSON", () => {
  const storage = buildMockStorage({
    [LOCAL_RELAYS_STORAGE_KEY]: "not json",
  });
  assert.deepEqual(readLocalRelays(storage), []);
});

test("readLocalRelays returns [] when storage value is not an array", () => {
  const storage = buildMockStorage({
    [LOCAL_RELAYS_STORAGE_KEY]: JSON.stringify({ nope: true }),
  });
  assert.deepEqual(readLocalRelays(storage), []);
});

test("readLocalRelays returns [] when key is absent", () => {
  const storage = buildMockStorage({});
  assert.deepEqual(readLocalRelays(storage), []);
});

test("readLocalRelays returns [] when storage is undefined/null", () => {
  assert.deepEqual(readLocalRelays(undefined), []);
  assert.deepEqual(readLocalRelays(null), []);
});

test("writeLocalRelays round-trips normalized entries through readLocalRelays", () => {
  const storage = buildMockStorage();
  writeLocalRelays(
    [
      { url: "WSS://Relay.One", read: true, write: false },
      { url: "   wss://Relay.Two   ", read: true, write: true },
      { url: "bogus", read: true, write: true },
    ],
    storage
  );
  assert.deepEqual(readLocalRelays(storage), [
    { url: "wss://relay.one", read: true, write: false },
    { url: "wss://relay.two", read: true, write: true },
  ]);
});

test("summarizePublishResult formats ok-only, failed-only, mixed, and empty cases", () => {
  assert.equal(
    summarizePublishResult({ result: { ok: ["wss://a", "wss://b"], failed: [] } }),
    "Published to 2 relays"
  );
  assert.equal(
    summarizePublishResult({ result: { ok: ["wss://a"], failed: [] } }),
    "Published to 1 relay"
  );
  assert.equal(
    summarizePublishResult({
      result: {
        ok: ["wss://a", "wss://b"],
        failed: [{ relayUrl: "wss://c", error: "x" }],
      },
    }),
    "Published to 2 relays, 1 failed"
  );
  assert.equal(
    summarizePublishResult({
      result: {
        ok: [],
        failed: [
          { relayUrl: "wss://a", error: "x" },
          { relayUrl: "wss://b", error: "y" },
        ],
      },
    }),
    "Failed to publish on 2 relays"
  );
  assert.equal(
    summarizePublishResult({ result: { ok: [], failed: [] } }),
    "No relays attempted"
  );
});
