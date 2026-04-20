import test from "node:test";
import assert from "node:assert/strict";

import {
  discoverReadRelays,
  discoverWriteRelays,
  hasAnyRelayPublishSuccess,
  mergeRelayEvents,
  relayPublishMany,
  relayUrlsFromRelayListEvent,
  relayQueryMany,
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

test("hasAnyRelayPublishSuccess reflects ok array length", () => {
  assert.equal(hasAnyRelayPublishSuccess({ ok: [], failed: [] }), false);
  assert.equal(
    hasAnyRelayPublishSuccess({ ok: ["wss://relay.one"], failed: [] }),
    true
  );
  assert.equal(hasAnyRelayPublishSuccess(undefined), false);
  assert.equal(hasAnyRelayPublishSuccess(null), false);
});
