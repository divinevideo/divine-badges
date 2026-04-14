const BECH32_ALPHABET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32_REV = Object.fromEntries(
  [...BECH32_ALPHABET].map((character, index) => [character, index])
);

function convertBits(data, fromBits, toBits, pad = true) {
  let accumulator = 0;
  let bits = 0;
  const result = [];
  const maxValue = (1 << toBits) - 1;
  for (const value of data) {
    if (value < 0 || value >> fromBits !== 0) {
      throw new Error("invalid bech32 value");
    }
    accumulator = (accumulator << fromBits) | value;
    bits += fromBits;
    while (bits >= toBits) {
      bits -= toBits;
      result.push((accumulator >> bits) & maxValue);
    }
  }
  if (pad) {
    if (bits > 0) {
      result.push((accumulator << (toBits - bits)) & maxValue);
    }
  } else if (bits >= fromBits || ((accumulator << (toBits - bits)) & maxValue) !== 0) {
    throw new Error("invalid bech32 padding");
  }
  return result;
}

function decodeBech32Words(value) {
  const normalized = value.toLowerCase();
  const separatorIndex = normalized.lastIndexOf("1");
  if (separatorIndex <= 0 || separatorIndex === normalized.length - 1) {
    throw new Error("invalid bech32 string");
  }
  const words = [...normalized.slice(separatorIndex + 1)].map((character) => {
    const decoded = BECH32_REV[character];
    if (decoded === undefined) {
      throw new Error("invalid bech32 character");
    }
    return decoded;
  });
  if (words.length < 6) {
    throw new Error("missing bech32 checksum");
  }
  return {
    prefix: normalized.slice(0, separatorIndex),
    words: words.slice(0, -6),
  };
}

function bytesToHex(bytes) {
  return bytes.map((value) => value.toString(16).padStart(2, "0")).join("");
}

function decodeBech32Bytes(value, expectedPrefix) {
  const { prefix, words } = decodeBech32Words(value);
  if (prefix !== expectedPrefix) {
    throw new Error(`expected ${expectedPrefix}`);
  }
  return convertBits(words, 5, 8, false);
}

export function decodeNpub(npub) {
  return bytesToHex(decodeBech32Bytes(npub, "npub"));
}

export function normalizeProfileId(value) {
  const normalized = value.trim();
  if (/^[0-9a-f]{64}$/i.test(normalized)) {
    return { type: "pubkey", value: normalized.toLowerCase() };
  }
  if (normalized.startsWith("npub1")) {
    return { type: "pubkey", value: decodeNpub(normalized) };
  }
  if (normalized.endsWith(".divine.video")) {
    return {
      type: "nip05",
      value: `${normalized.replace(/\.divine\.video$/i, "").toLowerCase()}@divine.video`,
    };
  }
  if (normalized.includes("@")) {
    return { type: "nip05", value: normalized.toLowerCase() };
  }
  throw new Error("unsupported profile identifier");
}

export async function resolveNip05(nip05) {
  const [name, domain] = nip05.trim().toLowerCase().split("@");
  if (!name || !domain) {
    throw new Error("invalid nip05");
  }
  const response = await fetch(
    `https://${domain}/.well-known/nostr.json?name=${encodeURIComponent(name)}`
  );
  if (!response.ok) {
    throw new Error(`nip05 request failed with ${response.status}`);
  }
  const payload = await response.json();
  const pubkey = payload?.names?.[name];
  if (!pubkey || !/^[0-9a-f]{64}$/i.test(pubkey)) {
    throw new Error("nip05 not found");
  }
  return pubkey.toLowerCase();
}

export async function resolveProfileId(value) {
  const normalized = normalizeProfileId(value);
  if (normalized.type === "pubkey") {
    return normalized.value;
  }
  return resolveNip05(normalized.value);
}

export function parseBadgeCoordinate(value) {
  const trimmed = value.trim();
  if (trimmed.startsWith("naddr1")) {
    return parseNaddr(trimmed);
  }
  const parts = trimmed.split(":");
  if (parts.length !== 3) {
    throw new Error("unsupported badge coordinate");
  }
  const kind = Number(parts[0]);
  if (!Number.isInteger(kind)) {
    throw new Error("invalid badge kind");
  }
  return {
    kind,
    pubkey: parts[1],
    identifier: parts[2],
    raw: trimmed,
  };
}

export function parseNaddr(value) {
  const bytes = decodeBech32Bytes(value.trim(), "naddr");
  let index = 0;
  let identifier = null;
  let pubkey = null;
  let kind = null;

  while (index < bytes.length) {
    const type = bytes[index];
    const length = bytes[index + 1];
    if (length === undefined) {
      throw new Error("invalid naddr tlv");
    }
    const chunk = bytes.slice(index + 2, index + 2 + length);
    if (chunk.length !== length) {
      throw new Error("truncated naddr tlv");
    }
    if (type === 0) {
      identifier = new TextDecoder().decode(Uint8Array.from(chunk));
    } else if (type === 2) {
      pubkey = bytesToHex(chunk);
    } else if (type === 3) {
      if (chunk.length !== 4) {
        throw new Error("invalid naddr kind");
      }
      kind = (chunk[0] << 24) | (chunk[1] << 16) | (chunk[2] << 8) | chunk[3];
    }
    index += 2 + length;
  }

  if (!identifier || !pubkey || !Number.isInteger(kind)) {
    throw new Error("incomplete naddr");
  }

  return {
    kind,
    pubkey,
    identifier,
    raw: `${kind}:${pubkey}:${identifier}`,
  };
}
