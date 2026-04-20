import test from "node:test";
import assert from "node:assert/strict";

import {
  loadNostrProfileMetadata,
  newestProfileMetadata,
  parseProfileMetadataEvent,
} from "./profile_metadata.js";

test("parseProfileMetadataEvent normalizes kind:0 content with display_name + picture + nip05", () => {
  const parsed = parseProfileMetadataEvent({
    kind: 0,
    pubkey: "deadbeef",
    created_at: 1700000000,
    content: JSON.stringify({
      name: "kingbach",
      display_name: "KingBach",
      picture: "https://example.com/a.png",
      nip05: "_@kingbach.divine.video",
      about: "welcome",
    }),
  });

  assert.equal(parsed.pubkey, "deadbeef");
  assert.equal(parsed.displayName, "KingBach");
  assert.equal(parsed.avatarUrl, "https://example.com/a.png");
  assert.equal(parsed.nip05, "_@kingbach.divine.video");
  assert.equal(parsed.handle, "kingbach");
  assert.equal(parsed.about, "welcome");
  assert.equal(parsed.createdAt, 1700000000);
  assert.deepEqual(parsed.raw, {
    name: "kingbach",
    display_name: "KingBach",
    picture: "https://example.com/a.png",
    nip05: "_@kingbach.divine.video",
    about: "welcome",
  });
});

test("parseProfileMetadataEvent falls back to name when display_name is missing", () => {
  const parsed = parseProfileMetadataEvent({
    kind: 0,
    pubkey: "abc",
    created_at: 1,
    content: JSON.stringify({ name: "rabble" }),
  });
  assert.equal(parsed.displayName, "rabble");
  assert.equal(parsed.avatarUrl, null);
  assert.equal(parsed.nip05, null);
  assert.equal(parsed.about, null);
  assert.equal(parsed.handle, null);
});

test("parseProfileMetadataEvent returns null for invalid JSON", () => {
  assert.equal(
    parseProfileMetadataEvent({ kind: 0, pubkey: "abc", content: "{not json" }),
    null
  );
});

test("parseProfileMetadataEvent returns null for wrong kind or missing event", () => {
  assert.equal(parseProfileMetadataEvent(null), null);
  assert.equal(
    parseProfileMetadataEvent({ kind: 1, pubkey: "abc", content: "{}" }),
    null
  );
});

test("parseProfileMetadataEvent returns null when content is not an object", () => {
  assert.equal(
    parseProfileMetadataEvent({ kind: 0, pubkey: "abc", content: JSON.stringify("string") }),
    null
  );
});

test("parseProfileMetadataEvent derives handle from local-part nip05", () => {
  const parsed = parseProfileMetadataEvent({
    kind: 0,
    pubkey: "abc",
    created_at: 1,
    content: JSON.stringify({ name: "x", nip05: "alice@example.com" }),
  });
  assert.equal(parsed.handle, "alice");
});

test("parseProfileMetadataEvent derives handle from _@handle.divine.video", () => {
  const parsed = parseProfileMetadataEvent({
    kind: 0,
    pubkey: "abc",
    created_at: 1,
    content: JSON.stringify({ nip05: "_@handle.divine.video" }),
  });
  assert.equal(parsed.handle, "handle");
});

test("newestProfileMetadata returns null for empty input", () => {
  assert.equal(newestProfileMetadata([]), null);
  assert.equal(newestProfileMetadata(null), null);
});

test("newestProfileMetadata picks the event with the highest created_at", () => {
  const older = {
    kind: 0,
    pubkey: "abc",
    created_at: 100,
    content: JSON.stringify({ name: "old" }),
  };
  const newer = {
    kind: 0,
    pubkey: "abc",
    created_at: 200,
    content: JSON.stringify({ name: "new" }),
  };
  const parsed = newestProfileMetadata([older, newer]);
  assert.equal(parsed.displayName, "new");
});

test("newestProfileMetadata skips unparseable events and returns the next best", () => {
  const bad = { kind: 0, pubkey: "abc", created_at: 300, content: "garbage" };
  const good = {
    kind: 0,
    pubkey: "abc",
    created_at: 200,
    content: JSON.stringify({ name: "good" }),
  };
  const parsed = newestProfileMetadata([bad, good]);
  assert.equal(parsed.displayName, "good");
});

test("loadNostrProfileMetadata calls injectable queryFn with kind:0 filter and returns newest", async () => {
  const calls = [];
  const queryFn = async (relays, filters) => {
    calls.push({ relays, filters });
    return [
      {
        kind: 0,
        pubkey: "abc",
        created_at: 10,
        content: JSON.stringify({ name: "old" }),
      },
      {
        kind: 0,
        pubkey: "abc",
        created_at: 20,
        content: JSON.stringify({ name: "new" }),
      },
    ];
  };
  const parsed = await loadNostrProfileMetadata(
    "abc",
    ["wss://relay"],
    queryFn
  );
  assert.equal(parsed.displayName, "new");
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].relays, ["wss://relay"]);
  assert.equal(calls[0].filters[0].kinds[0], 0);
  assert.deepEqual(calls[0].filters[0].authors, ["abc"]);
});

test("loadNostrProfileMetadata returns null when no pubkey or no relays", async () => {
  const queryFn = async () => {
    throw new Error("should not be called");
  };
  assert.equal(await loadNostrProfileMetadata(null, ["wss://r"], queryFn), null);
  assert.equal(await loadNostrProfileMetadata("abc", [], queryFn), null);
});
