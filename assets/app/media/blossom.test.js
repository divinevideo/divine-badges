import test from "node:test";
import assert from "node:assert/strict";

import {
  buildBlossomAuthorizationEvent,
  blossomUploadUrl,
  normalizeBlossomUpload,
  uploadToBlossom,
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

test("blossomUploadUrl targets the control host", () => {
  assert.equal(
    blossomUploadUrl("https://media.divine.video"),
    "https://media.divine.video/upload"
  );
});

test("uploadToBlossom uploads to the control host, never the data host", async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options = {}) => {
    calls.push({ url: String(url), options });
    return Response.json({
      url: "https://media.divine.video/blob.png",
      sha256: "b".repeat(64),
      type: "image/png",
    });
  };

  try {
    const file = new File(["hello"], "hello.png", { type: "image/png" });
    const result = await uploadToBlossom({
      file,
      signer: {
        async signEvent(event) {
          return { ...event, id: "signed" };
        },
      },
      pubkey: "f".repeat(64),
      endpoint: "https://media.divine.video",
    });

    assert.equal(result.url, "https://media.divine.video/blob.png");
    // A single PUT to the control host: no HEAD probe, no data-host redirect.
    // Uploading to upload.divine.video skips blob-metadata publication, which
    // makes the returned URL 404 and the badge preview render as a broken image.
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, "https://media.divine.video/upload");
    assert.equal(calls[0].options.method, "PUT");
  } finally {
    globalThis.fetch = previousFetch;
  }
});
