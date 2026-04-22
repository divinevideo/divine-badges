import test from "node:test";
import assert from "node:assert/strict";

import {
  buildEffectiveRelays,
  buildRelayListMetadataEvent,
  discoverReadRelays,
  discoverWriteRelays,
  hasAnyRelayPublishSuccess,
  mergeRelayEvents,
  normalizeRelayUrl,
  RELAY_LIST_KIND,
  relayPublishMany,
  relayUrlsFromRelayListEvent,
  relayQueryMany,
  relayQueryManyDetailed,
} from "./relay.js";

test("relayUrlsFromRelayListEvent keeps read and neutral relays", () => {
  const urls = relayUrlsFromRelayListEvent({
    tags: [
      ["r", "wss://relay.one"],
      ["r", "wss://relay.two", "read"],
      ["r", "wss://relay.three", "write"],
      ["r", "https://not-a-websocket.example"],
      ["p", "ignored"],
    ],
  });

  assert.deepEqual(urls, ["wss://relay.one", "wss://relay.two"]);
});

test("relayUrlsFromRelayListEvent returns marker-aware read and write relays", () => {
  const relayList = {
    tags: [
      ["r", "wss://neutral.example"],
      ["r", "wss://read.example", "read"],
      ["r", "wss://write.example", "write"],
      ["r", "https://not-a-websocket.example"],
      ["p", "ignored"],
    ],
  };

  assert.deepEqual(
    relayUrlsFromRelayListEvent(relayList, "read"),
    ["wss://neutral.example", "wss://read.example"]
  );
  assert.deepEqual(
    relayUrlsFromRelayListEvent(relayList, "write"),
    ["wss://neutral.example", "wss://write.example"]
  );
});

test("mergeRelayEvents deduplicates by event id", () => {
  const merged = mergeRelayEvents([
    [{ id: "one", created_at: 1 }, { id: "two", created_at: 2 }],
    [{ id: "two", created_at: 2 }, { id: "three", created_at: 3 }],
  ]);

  assert.deepEqual(
    merged.map((event) => event.id),
    ["one", "two", "three"]
  );
});

test("relayQueryMany merges successful relay results and ignores relay failures", async () => {
  const calls = [];
  const events = await relayQueryMany(
    ["wss://relay.one", "wss://relay.two"],
    [{ kinds: [1] }],
    500,
    async (relayUrl) => {
      calls.push(relayUrl);
      if (relayUrl === "wss://relay.two") {
        throw new Error("relay error");
      }
      return [{ id: "one", created_at: 1 }];
    }
  );

  assert.deepEqual(calls, ["wss://relay.one", "wss://relay.two"]);
  assert.deepEqual(events, [{ id: "one", created_at: 1 }]);
});

test("discoverReadRelays combines seed and published relay list", async () => {
  const relays = await discoverReadRelays(
    {
      pubkeys: ["f".repeat(64)],
      seedRelays: ["wss://relay.divine.video"],
    },
    async () => [
      {
        id: "relay-list",
        created_at: 10,
        tags: [
          ["r", "wss://relay.one"],
          ["r", "wss://relay.two", "read"],
          ["r", "wss://relay.write-only", "write"],
        ],
      },
    ]
  );

  assert.deepEqual(relays, [
    "wss://relay.divine.video",
    "wss://relay.one",
    "wss://relay.two",
  ]);
});

test("discoverWriteRelays combines seed relays with write and neutral relays", async () => {
  const relays = await discoverWriteRelays(
    {
      pubkeys: ["f".repeat(64)],
      seedRelays: ["wss://relay.divine.video"],
    },
    async () => [
      {
        id: "relay-list",
        created_at: 10,
        tags: [
          ["r", "wss://relay.neutral"],
          ["r", "wss://relay.read-only", "read"],
          ["r", "wss://relay.write-only", "write"],
        ],
      },
    ]
  );

  assert.deepEqual(relays, [
    "wss://relay.divine.video",
    "wss://relay.neutral",
    "wss://relay.write-only",
  ]);
});

test("discoverWriteRelays returns seed relays unchanged when no pubkeys supplied", async () => {
  let queryCalls = 0;
  const relays = await discoverWriteRelays(
    {
      pubkeys: [],
      seedRelays: ["wss://relay.divine.video", "wss://relay.divine.video", ""],
    },
    async () => {
      queryCalls += 1;
      return [];
    }
  );

  assert.equal(queryCalls, 0);
  assert.deepEqual(relays, ["wss://relay.divine.video"]);
});

test("relayPublishMany dedupes URLs and returns ok/failed split", async () => {
  const calls = [];
  const result = await relayPublishMany(
    [
      "wss://relay.one",
      "wss://relay.one",
      "wss://relay.two",
      "wss://relay.three",
    ],
    { id: "evt-1" },
    1234,
    async (relayUrl, nostrEvent, timeoutMs) => {
      calls.push({ relayUrl, nostrEvent, timeoutMs });
      if (relayUrl === "wss://relay.two") {
        throw new Error("relay timeout");
      }
    }
  );

  assert.deepEqual(
    calls.map((c) => c.relayUrl),
    ["wss://relay.one", "wss://relay.two", "wss://relay.three"]
  );
  assert.equal(calls[0].timeoutMs, 1234);
  assert.deepEqual(calls[0].nostrEvent, { id: "evt-1" });
  assert.deepEqual(result.ok, ["wss://relay.one", "wss://relay.three"]);
  assert.deepEqual(result.failed, [
    { relayUrl: "wss://relay.two", error: "relay timeout" },
  ]);
});

test("relayPublishMany returns empty ok when all relays fail", async () => {
  const result = await relayPublishMany(
    ["wss://relay.one", "wss://relay.two"],
    { id: "evt-2" },
    500,
    async (relayUrl) => {
      throw new Error(`no ${relayUrl}`);
    }
  );

  assert.deepEqual(result.ok, []);
  assert.deepEqual(result.failed, [
    { relayUrl: "wss://relay.one", error: "no wss://relay.one" },
    { relayUrl: "wss://relay.two", error: "no wss://relay.two" },
  ]);
});

test("relayQueryManyDetailed returns per-relay diagnostics and merged events", async () => {
  const result = await relayQueryManyDetailed(
    ["wss://one", "wss://two"],
    [{ kinds: [1] }],
    500,
    async (relayUrl) => {
      if (relayUrl === "wss://one") return [{ id: "event-1", created_at: 1 }];
      throw new Error("relay exploded");
    }
  );

  assert.deepEqual(result.events.map((e) => e.id), ["event-1"]);
  assert.equal(result.relays.length, 2);
  const one = result.relays.find((entry) => entry.relayUrl === "wss://one");
  const two = result.relays.find((entry) => entry.relayUrl === "wss://two");
  assert.equal(one.status, "ok");
  assert.equal(one.eventCount, 1);
  assert.equal(two.status, "error");
  assert.equal(two.error, "relay exploded");
});

test("relayQueryManyDetailed returns empty shape for empty input", async () => {
  const result = await relayQueryManyDetailed([]);
  assert.deepEqual(result, { events: [], relays: [] });
});

test("relayQueryMany still returns merged events array (backwards compatible)", async () => {
  const events = await relayQueryMany(
    ["wss://relay.one", "wss://relay.two"],
    [{ kinds: [1] }],
    500,
    async (relayUrl) => {
      if (relayUrl === "wss://relay.two") {
        throw new Error("relay error");
      }
      return [{ id: "compat-event", created_at: 2 }];
    }
  );

  assert.ok(Array.isArray(events));
  assert.deepEqual(events, [{ id: "compat-event", created_at: 2 }]);
});

test("hasAnyRelayPublishSuccess reflects ok array length", () => {
  assert.equal(hasAnyRelayPublishSuccess({ ok: [], failed: [] }), false);
  assert.equal(
    hasAnyRelayPublishSuccess({ ok: ["wss://relay.one"], failed: [] }),
    true
  );
  assert.equal(hasAnyRelayPublishSuccess(undefined), false);
  assert.equal(hasAnyRelayPublishSuccess(null), false);
});

test("normalizeRelayUrl lowercases, trims whitespace, and preserves trailing slash", () => {
  assert.equal(normalizeRelayUrl("wss://Relay.Example/"), "wss://relay.example/");
  assert.equal(normalizeRelayUrl("  WSS://relay.example  "), "wss://relay.example");
  assert.equal(normalizeRelayUrl("ws://local.test"), "ws://local.test");
});

test("normalizeRelayUrl rejects non-websocket schemes and empty inputs", () => {
  assert.equal(normalizeRelayUrl("http://not-ws"), null);
  assert.equal(normalizeRelayUrl("https://not-ws"), null);
  assert.equal(normalizeRelayUrl(""), null);
  assert.equal(normalizeRelayUrl(null), null);
  assert.equal(normalizeRelayUrl(undefined), null);
});

test("buildEffectiveRelays dedupes across divine + discovered + local in read mode", () => {
  const relays = buildEffectiveRelays({
    divineRelays: ["wss://relay.divine.video"],
    discoveredRelays: ["wss://relay.divine.video", "wss://relay.one"],
    localRelays: [
      { url: "wss://relay.two", read: true, write: false },
      { url: "wss://relay.three", read: false, write: true },
      { url: "wss://relay.one", read: true, write: true },
    ],
    mode: "read",
  });

  assert.deepEqual(relays, [
    "wss://relay.divine.video",
    "wss://relay.one",
    "wss://relay.two",
  ]);
});

test("buildEffectiveRelays includes only write-enabled locals in write mode", () => {
  const relays = buildEffectiveRelays({
    divineRelays: ["wss://relay.divine.video"],
    discoveredRelays: ["wss://relay.discovered"],
    localRelays: [
      { url: "wss://relay.read-only", read: true, write: false },
      { url: "wss://relay.write-only", read: false, write: true },
      { url: "wss://relay.both", read: true, write: true },
    ],
    mode: "write",
  });

  assert.deepEqual(relays, [
    "wss://relay.divine.video",
    "wss://relay.discovered",
    "wss://relay.write-only",
    "wss://relay.both",
  ]);
});

test("buildEffectiveRelays normalizes URLs and ignores invalid entries", () => {
  const relays = buildEffectiveRelays({
    divineRelays: ["wss://Relay.Divine.Video"],
    discoveredRelays: ["https://not-ws", "wss://relay.one"],
    localRelays: [
      { url: "not-a-url", read: true, write: true },
      { url: "WSS://Relay.One", read: true, write: true },
    ],
    mode: "read",
  });

  assert.deepEqual(relays, ["wss://relay.divine.video", "wss://relay.one"]);
});

test("buildRelayListMetadataEvent emits kind:10002 with proper markers", () => {
  const event = buildRelayListMetadataEvent({
    pubkey: "pk-42",
    createdAt: 100,
    relays: [
      { url: "wss://relay.neutral", read: true, write: true },
      { url: "wss://relay.read-only", read: true, write: false },
      { url: "wss://relay.write-only", read: false, write: true },
      { url: "wss://relay.skipped", read: false, write: false },
    ],
  });

  assert.equal(event.kind, RELAY_LIST_KIND);
  assert.equal(event.kind, 10002);
  assert.equal(event.pubkey, "pk-42");
  assert.equal(event.created_at, 100);
  assert.equal(event.content, "");
  assert.deepEqual(event.tags, [
    ["r", "wss://relay.neutral"],
    ["r", "wss://relay.read-only", "read"],
    ["r", "wss://relay.write-only", "write"],
  ]);
});

test("buildRelayListMetadataEvent skips entries whose URL fails normalization", () => {
  const event = buildRelayListMetadataEvent({
    pubkey: "pk",
    createdAt: 1,
    relays: [
      { url: "http://not-ws", read: true, write: true },
      { url: "", read: true, write: true },
      { url: null, read: true, write: true },
      { url: "wss://keep.example", read: true, write: true },
    ],
  });

  assert.deepEqual(event.tags, [["r", "wss://keep.example"]]);
});

test("buildRelayListMetadataEvent tolerates empty relays arg", () => {
  const event = buildRelayListMetadataEvent({
    pubkey: "pk",
    createdAt: 1,
    relays: [],
  });
  assert.deepEqual(event.tags, []);
  const event2 = buildRelayListMetadataEvent({
    pubkey: "pk",
    createdAt: 1,
  });
  assert.deepEqual(event2.tags, []);
});
