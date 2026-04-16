import test from "node:test";
import assert from "node:assert/strict";

import {
  buildBlossomAuthorizationEvent,
  normalizeBlossomUpload,
} from "./blossom.js";

test("buildBlossomAuthorizationEvent shapes a kind 24242 upload token", () => {
  const event = buildBlossomAuthorizationEvent({
    pubkey: "pubkey123",
    host: "media.divine.video",
    sha256: "a".repeat(64),
    createdAt: 1700000000,
    expiresAt: 1700000300,
  });

  assert.deepEqual(event, {
    kind: 24242,
    pubkey: "pubkey123",
    created_at: 1700000000,
    content: "Authorize upload",
    tags: [
      ["t", "upload"],
      ["x", "a".repeat(64)],
      ["expiration", "1700000300"],
      ["server", "media.divine.video"],
    ],
  });
});

test("normalizeBlossomUpload returns the served URL from a descriptor", () => {
  assert.deepEqual(
    normalizeBlossomUpload({
      url: "https://media.divine.video/abc123.webp",
      sha256: "abc123",
      type: "image/webp",
    }),
    {
      url: "https://media.divine.video/abc123.webp",
      sha256: "abc123",
      type: "image/webp",
    }
  );
});
