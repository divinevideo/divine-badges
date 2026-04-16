function toBase64Utf8(value) {
  return btoa(unescape(encodeURIComponent(value)));
}

export function buildBlossomAuthorizationEvent({
  pubkey,
  host,
  sha256,
  createdAt,
  expiresAt,
}) {
  return {
    kind: 24242,
    pubkey,
    created_at: createdAt,
    content: "Authorize upload",
    tags: [
      ["t", "upload"],
      ["x", sha256],
      ["expiration", String(expiresAt)],
      ["server", host],
    ],
  };
}

export function normalizeBlossomUpload(descriptor) {
  return {
    url: descriptor?.url || descriptor?.download_url || descriptor?.blob?.url || null,
    sha256: descriptor?.sha256 || descriptor?.sha256_hex || descriptor?.blob?.sha256 || null,
    type: descriptor?.type || descriptor?.mime || descriptor?.blob?.type || null,
  };
}

export async function sha256Hex(file) {
  const buffer = await file.arrayBuffer();
  const digest = await crypto.subtle.digest("SHA-256", buffer);
  return [...new Uint8Array(digest)]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");
}

export async function uploadToBlossom({
  file,
  signer,
  pubkey,
  endpoint = "https://media.divine.video",
  expiresInSeconds = 300,
}) {
  if (!file) {
    throw new Error("missing file");
  }
  if (!signer || !pubkey) {
    throw new Error("missing signer");
  }

  const sha256 = await sha256Hex(file);
  const createdAt = Math.floor(Date.now() / 1000);
  const expiresAt = createdAt + expiresInSeconds;
  const host = new URL(endpoint).host;
  const authEvent = buildBlossomAuthorizationEvent({
    pubkey,
    host,
    sha256,
    createdAt,
    expiresAt,
  });
  const signedAuth = await signer.signEvent(authEvent);
  const response = await fetch(new URL("/upload", endpoint), {
    method: "PUT",
    headers: {
      Authorization: `Nostr ${toBase64Utf8(JSON.stringify(signedAuth))}`,
      "Content-Type": file.type || "application/octet-stream",
      "X-Sha256": sha256,
    },
    body: file,
  });

  if (!response.ok) {
    throw new Error(`upload failed with ${response.status}`);
  }

  const descriptor = normalizeBlossomUpload(await response.json());
  if (!descriptor.url) {
    throw new Error("upload response missing url");
  }
  return descriptor;
}
