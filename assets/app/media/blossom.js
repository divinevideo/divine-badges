const CONTENT_TYPE_BY_EXTENSION = {
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  png: "image/png",
  gif: "image/gif",
  webp: "image/webp",
  heic: "image/heic",
  heif: "image/heif",
  avif: "image/avif",
  svg: "image/svg+xml",
};

function toBase64Utf8(value) {
  return btoa(unescape(encodeURIComponent(value)));
}

function bytesToHex(bytes) {
  let hex = "";
  for (const value of bytes) {
    hex += value.toString(16).padStart(2, "0");
  }
  return hex;
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

// iOS/Safari frequently reports an empty or non-image MIME type for photos
// taken from the camera roll (HEIC). Fall back to the file extension so the
// media server still receives a usable image content type.
export function uploadContentType(file) {
  const declared = (file?.type || "").trim();
  if (declared.toLowerCase().startsWith("image/")) {
    return declared;
  }
  const name = (file?.name || "").toLowerCase();
  const dot = name.lastIndexOf(".");
  const extension = dot >= 0 ? name.slice(dot + 1) : "";
  return CONTENT_TYPE_BY_EXTENSION[extension] || declared || "application/octet-stream";
}

// Read the selected file into memory exactly once. `Blob.prototype.arrayBuffer`
// is unavailable on older iOS Safari / WKWebView builds, so fall back to a
// FileReader there instead of throwing an opaque TypeError.
export async function readFileBytes(file) {
  if (file && typeof file.arrayBuffer === "function") {
    return new Uint8Array(await file.arrayBuffer());
  }
  if (typeof FileReader === "function") {
    return await new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(new Uint8Array(reader.result));
      reader.onerror = () =>
        reject(reader.error || new Error("Could not read the selected file."));
      reader.readAsArrayBuffer(file);
    });
  }
  throw new Error("This browser cannot read the selected file.");
}

export async function sha256HexFromBytes(bytes) {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) {
    throw new Error("Secure upload is unavailable in this browser context.");
  }
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  const digest = await subtle.digest("SHA-256", view);
  return bytesToHex(new Uint8Array(digest));
}

export async function sha256Hex(file) {
  return sha256HexFromBytes(await readFileBytes(file));
}

export async function resolveBlossomUploadEndpoint({ endpoint }) {
  const controlUrl = new URL("/upload", endpoint);
  let response;
  try {
    response = await fetch(controlUrl, {
      method: "HEAD",
    });
  } catch {
    return controlUrl.toString();
  }

  if (!response.ok) {
    return controlUrl.toString();
  }

  const dataHost = response.headers.get("x-divine-upload-data-host");
  if (!dataHost) {
    return controlUrl.toString();
  }

  return new URL("/upload", `https://${dataHost}`).toString();
}

async function putBlobToHost({ url, body, contentType, sha256, authorization }) {
  let response;
  try {
    response = await fetch(url, {
      method: "PUT",
      headers: {
        Authorization: authorization,
        "Content-Type": contentType,
        "X-Sha256": sha256,
      },
      body,
    });
  } catch (error) {
    const reason = error?.message || error;
    const wrapped = new Error(`Could not reach the media server (${reason}).`);
    wrapped.retryable = true;
    throw wrapped;
  }

  if (!response.ok) {
    const detail = (await response.text().catch(() => "")).trim();
    const error = new Error(
      `Media server returned ${response.status}${detail ? `: ${detail.slice(0, 200)}` : ""}.`
    );
    error.status = response.status;
    // Server-side faults can differ between the data host and the control
    // host, so they are worth retrying elsewhere; client faults are not.
    error.retryable = response.status >= 500;
    throw error;
  }

  let payload;
  try {
    payload = await response.json();
  } catch {
    throw new Error("The media server response could not be read.");
  }
  const descriptor = normalizeBlossomUpload(payload);
  if (!descriptor.url) {
    throw new Error("The media server did not return a media URL.");
  }
  return descriptor;
}

export async function uploadToBlossom({
  file,
  signer,
  pubkey,
  endpoint = "https://media.divine.video",
  expiresInSeconds = 300,
}) {
  if (!file) {
    throw new Error("Choose an image to upload.");
  }
  if (!signer || !pubkey) {
    throw new Error("Log in before uploading media.");
  }

  const bytes = await readFileBytes(file);
  if (!bytes.length) {
    throw new Error("The selected file is empty.");
  }
  const sha256 = await sha256HexFromBytes(bytes);
  const contentType = uploadContentType(file);

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
  const authorization = `Nostr ${toBase64Utf8(JSON.stringify(signedAuth))}`;

  const controlUrl = new URL("/upload", endpoint).toString();
  const dataUrl = await resolveBlossomUploadEndpoint({ endpoint });
  const targets = dataUrl === controlUrl ? [controlUrl] : [dataUrl, controlUrl];

  let lastError = null;
  for (let index = 0; index < targets.length; index += 1) {
    try {
      return await putBlobToHost({
        url: targets[index],
        body: new Blob([bytes], { type: contentType }),
        contentType,
        sha256,
        authorization,
      });
    } catch (error) {
      lastError = error;
      const moreHostsToTry = index < targets.length - 1;
      if (!moreHostsToTry || error?.retryable === false) {
        break;
      }
    }
  }
  throw lastError || new Error("Upload failed.");
}
