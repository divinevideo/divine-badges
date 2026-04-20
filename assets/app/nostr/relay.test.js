import test from "node:test";
import assert from "node:assert/strict";

import {
  discoverReadRelays,
  mergeRelayEvents,
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
