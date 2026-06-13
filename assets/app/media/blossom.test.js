import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

import {
  buildBlossomAuthorizationEvent,
  resolveBlossomUploadEndpoint,
  normalizeBlossomUpload,
  readFileBytes,
  sha256HexFromBytes,
  uploadContentType,
  uploadToBlossom,
} from "./blossom.js";

function nodeSha256Hex(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fakeSigner() {
  return {
    async signEvent(event) {
      return { ...event, id: "signed", sig: "sig" };
    },
  };
}

// A File-like object whose bytes can only be read through arrayBuffer(), with a
// call counter so tests can assert the upload reads the file exactly once.
function countingFile(contents, { name = "photo.png", type = "image/png" } = {}) {
  const data = new TextEncoder().encode(contents);
  const handle = {
    name,
    type,
    size: data.byteLength,
    reads: 0,
    async arrayBuffer() {
      handle.reads += 1;
      return data.buffer.slice(0);
    },
  };
  return handle;
}

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

test("uploadContentType keeps a declared image type", () => {
  assert.equal(uploadContentType({ type: "image/webp", name: "x.bin" }), "image/webp");
});

test("uploadContentType infers from the extension when the type is empty", () => {
  assert.equal(uploadContentType({ type: "", name: "IMG_0001.HEIC" }), "image/heic");
  assert.equal(uploadContentType({ type: "", name: "snap.jpg" }), "image/jpeg");
});

test("uploadContentType falls back to octet-stream for unknown files", () => {
  assert.equal(uploadContentType({ type: "", name: "mystery" }), "application/octet-stream");
});

test("readFileBytes uses arrayBuffer when available", async () => {
  const bytes = await readFileBytes(countingFile("hello"));
  assert.equal(new TextDecoder().decode(bytes), "hello");
});

test("readFileBytes falls back to FileReader on older Safari", async () => {
  const previous = globalThis.FileReader;
  class FakeFileReader {
    readAsArrayBuffer(file) {
      this.result = file.data;
      queueMicrotask(() => this.onload());
    }
  }
  globalThis.FileReader = FakeFileReader;
  try {
    const data = new TextEncoder().encode("from-reader").buffer;
    const bytes = await readFileBytes({ name: "a.png", type: "image/png", data });
    assert.equal(new TextDecoder().decode(bytes), "from-reader");
  } finally {
    if (previous === undefined) {
      delete globalThis.FileReader;
    } else {
      globalThis.FileReader = previous;
    }
  }
});

test("readFileBytes throws clearly when the file cannot be read", async () => {
  const previous = globalThis.FileReader;
  delete globalThis.FileReader;
  try {
    await assert.rejects(
      () => readFileBytes({ name: "a.png", type: "image/png" }),
      /cannot read the selected file/
    );
  } finally {
    if (previous !== undefined) {
      globalThis.FileReader = previous;
    }
  }
});

test("sha256HexFromBytes matches a known digest", async () => {
  const bytes = new TextEncoder().encode("hello");
  assert.equal(await sha256HexFromBytes(bytes), nodeSha256Hex("hello"));
});

test("resolveBlossomUploadEndpoint prefers the advertised data host", async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options = {}) => {
    calls.push({ url: String(url), options });
    return new Response(null, {
      status: 200,
      headers: {
        "x-divine-upload-data-host": "upload.divine.video",
      },
    });
  };

  try {
    const endpoint = await resolveBlossomUploadEndpoint({
      endpoint: "https://media.divine.video",
    });

    assert.equal(endpoint, "https://upload.divine.video/upload");
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, "https://media.divine.video/upload");
    assert.equal(calls[0].options.method, "HEAD");
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("resolveBlossomUploadEndpoint falls back to the control host when HEAD fails", async () => {
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async () => new Response(null, { status: 405 });
  try {
    const endpoint = await resolveBlossomUploadEndpoint({
      endpoint: "https://media.divine.video",
    });
    assert.equal(endpoint, "https://media.divine.video/upload");
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("uploadToBlossom reads the file once and uploads the hashed bytes", async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  const file = countingFile("hello", { name: "photo.png", type: "image/png" });
  globalThis.fetch = async (url, options = {}) => {
    calls.push({ url: String(url), options });
    if (options.method === "HEAD") {
      return new Response(null, {
        status: 200,
        headers: { "x-divine-upload-data-host": "upload.divine.video" },
      });
    }
    return Response.json({
      url: "https://media.divine.video/blob.png",
      sha256: nodeSha256Hex("hello"),
      type: "image/png",
    });
  };

  try {
    const result = await uploadToBlossom({
      file,
      signer: fakeSigner(),
      pubkey: "f".repeat(64),
      endpoint: "https://media.divine.video",
    });

    assert.equal(result.url, "https://media.divine.video/blob.png");
    assert.equal(file.reads, 1, "file must be read exactly once");
    assert.equal(calls.length, 2);

    const put = calls[1];
    assert.equal(put.url, "https://upload.divine.video/upload");
    assert.equal(put.options.method, "PUT");
    // The bytes that were hashed must be exactly the bytes that were uploaded.
    const sentBytes = new Uint8Array(await put.options.body.arrayBuffer());
    assert.equal(new TextDecoder().decode(sentBytes), "hello");
    assert.equal(put.options.headers["X-Sha256"], nodeSha256Hex("hello"));
    assert.equal(put.options.headers["Content-Type"], "image/png");
    assert.match(put.options.headers.Authorization, /^Nostr /);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("uploadToBlossom infers a content type for typeless iOS photos", async () => {
  let putContentType = null;
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options = {}) => {
    if (options.method === "HEAD") {
      return new Response(null, { status: 405 });
    }
    putContentType = options.headers["Content-Type"];
    return Response.json({ url: "https://media.divine.video/blob.heic" });
  };
  try {
    await uploadToBlossom({
      file: countingFile("x", { name: "IMG_2.HEIC", type: "" }),
      signer: fakeSigner(),
      pubkey: "f".repeat(64),
    });
    assert.equal(putContentType, "image/heic");
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("uploadToBlossom falls back to the control host on a transport failure", async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options = {}) => {
    const target = String(url);
    calls.push({ url: target, method: options.method });
    if (options.method === "HEAD") {
      return new Response(null, {
        status: 200,
        headers: { "x-divine-upload-data-host": "data.example" },
      });
    }
    if (target.startsWith("https://data.example")) {
      throw new TypeError("Load failed");
    }
    return Response.json({ url: "https://media.divine.video/ok.png" });
  };

  try {
    const result = await uploadToBlossom({
      file: countingFile("hello"),
      signer: fakeSigner(),
      pubkey: "f".repeat(64),
      endpoint: "https://media.divine.video",
    });
    assert.equal(result.url, "https://media.divine.video/ok.png");
    assert.deepEqual(
      calls.map((call) => call.url),
      [
        "https://media.divine.video/upload",
        "https://data.example/upload",
        "https://media.divine.video/upload",
      ]
    );
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("uploadToBlossom falls back to the control host on a 5xx", async () => {
  const puts = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options = {}) => {
    const target = String(url);
    if (options.method === "HEAD") {
      return new Response(null, {
        status: 200,
        headers: { "x-divine-upload-data-host": "data.example" },
      });
    }
    puts.push(target);
    if (target.startsWith("https://data.example")) {
      return new Response("upstream down", { status: 503 });
    }
    return Response.json({ url: "https://media.divine.video/ok.png" });
  };
  try {
    const result = await uploadToBlossom({
      file: countingFile("hello"),
      signer: fakeSigner(),
      pubkey: "f".repeat(64),
    });
    assert.equal(result.url, "https://media.divine.video/ok.png");
    assert.equal(puts.length, 2);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("uploadToBlossom does not retry a client rejection and surfaces detail", async () => {
  const puts = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options = {}) => {
    if (options.method === "HEAD") {
      return new Response(null, {
        status: 200,
        headers: { "x-divine-upload-data-host": "data.example" },
      });
    }
    puts.push(String(url));
    return new Response("sha256 mismatch", { status: 400 });
  };
  try {
    await assert.rejects(
      () =>
        uploadToBlossom({
          file: countingFile("hello"),
          signer: fakeSigner(),
          pubkey: "f".repeat(64),
        }),
      /Media server returned 400: sha256 mismatch/
    );
    assert.equal(puts.length, 1, "client errors must not retry the other host");
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("uploadToBlossom reports when no media URL is returned", async () => {
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options = {}) => {
    if (options.method === "HEAD") {
      return new Response(null, { status: 405 });
    }
    return Response.json({ sha256: "abc" });
  };
  try {
    await assert.rejects(
      () =>
        uploadToBlossom({
          file: countingFile("hello"),
          signer: fakeSigner(),
          pubkey: "f".repeat(64),
        }),
      /did not return a media URL/
    );
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("uploadToBlossom guards missing inputs", async () => {
  await assert.rejects(
    () => uploadToBlossom({ file: null, signer: fakeSigner(), pubkey: "f".repeat(64) }),
    /Choose an image/
  );
  await assert.rejects(
    () => uploadToBlossom({ file: countingFile("x"), signer: null, pubkey: null }),
    /Log in before uploading/
  );
});

test("uploadToBlossom rejects an empty file before signing", async () => {
  let signed = false;
  const signer = {
    async signEvent(event) {
      signed = true;
      return { ...event, id: "signed" };
    },
  };
  await assert.rejects(
    () => uploadToBlossom({ file: countingFile(""), signer, pubkey: "f".repeat(64) }),
    /selected file is empty/
  );
  assert.equal(signed, false);
});
