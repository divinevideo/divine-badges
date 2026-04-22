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

function hexToBytes(hex) {
  if (typeof hex !== "string" || hex.length % 2 !== 0) {
    throw new Error("invalid hex string");
  }
  const result = [];
  for (let index = 0; index < hex.length; index += 2) {
    const byte = Number.parseInt(hex.slice(index, index + 2), 16);
    if (Number.isNaN(byte)) {
      throw new Error("invalid hex character");
    }
    result.push(byte);
  }
  return result;
}

function bech32Polymod(values) {
  const generators = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
  let checksum = 1;
  for (const value of values) {
    const high = checksum >> 25;
    checksum = ((checksum & 0x1ffffff) << 5) ^ value;
    for (let index = 0; index < generators.length; index += 1) {
      if ((high >> index) & 1) {
        checksum ^= generators[index];
      }
    }
  }
  return checksum;
}

function hrpExpand(prefix) {
  const high = [...prefix].map((character) => character.charCodeAt(0) >> 5);
  const low = [...prefix].map((character) => character.charCodeAt(0) & 31);
  return [...high, 0, ...low];
}

function encodeBech32(prefix, words) {
  const checksumInput = [...hrpExpand(prefix), ...words, 0, 0, 0, 0, 0, 0];
  const polymod = bech32Polymod(checksumInput) ^ 1;
  const checksum = Array.from({ length: 6 }, (_, index) =>
    (polymod >> (5 * (5 - index))) & 31
  );
  return `${prefix}1${[...words, ...checksum]
    .map((word) => BECH32_ALPHABET[word])
    .join("")}`;
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
  const normalized = value.trim().toLowerCase();
  const strippedAt = normalized.replace(/^@+/, "");
  if (/^[0-9a-f]{64}$/i.test(normalized)) {
    return { type: "pubkey", value: normalized.toLowerCase() };
  }
  if (normalized.startsWith("npub1")) {
    return { type: "pubkey", value: decodeNpub(normalized) };
  }
  if (strippedAt.endsWith(".divine.video")) {
    return {
      type: "nip05",
      value: `${strippedAt.replace(/\.divine\.video$/i, "")}@divine.video`,
    };
  }
  if (/^[a-z0-9][a-z0-9._-]*$/i.test(strippedAt) && !strippedAt.includes("@")) {
    return {
      type: "nip05",
      value: `${strippedAt}@divine.video`,
    };
  }
  if (strippedAt.includes("@")) {
    return { type: "nip05", value: strippedAt };
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
  const relays = [];

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
    } else if (type === 1) {
      relays.push(new TextDecoder().decode(Uint8Array.from(chunk)));
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
    relays,
    raw: `${kind}:${pubkey}:${identifier}`,
  };
}

export function encodeNaddr({ kind, pubkey, identifier, relays = [] }) {
  if (!Number.isInteger(kind) || kind < 0 || kind > 0xffffffff) {
    throw new Error("invalid naddr kind");
  }
  if (typeof pubkey !== "string" || !/^[0-9a-f]{64}$/i.test(pubkey)) {
    throw new Error("invalid naddr pubkey");
  }
  if (typeof identifier !== "string") {
    throw new Error("invalid naddr identifier");
  }
  if (!Array.isArray(relays)) {
    throw new Error("invalid naddr relays");
  }

  const identifierBytes = [...new TextEncoder().encode(identifier)];
  if (identifierBytes.length > 255) {
    throw new Error("naddr identifier too long");
  }
  const pubkeyBytes = hexToBytes(pubkey.toLowerCase());
  const kindBytes = [
    (kind >>> 24) & 0xff,
    (kind >>> 16) & 0xff,
    (kind >>> 8) & 0xff,
    kind & 0xff,
  ];

  const payload = [];
  // kind (type 3)
  payload.push(3, 4, ...kindBytes);
  // author (type 2)
  payload.push(2, 32, ...pubkeyBytes);
  // relays (type 1) — one TLV per URL
  for (const relay of relays) {
    if (typeof relay !== "string") {
      throw new Error("invalid naddr relay");
    }
    const relayBytes = [...new TextEncoder().encode(relay)];
    if (relayBytes.length > 255) {
      throw new Error("naddr relay url too long");
    }
    payload.push(1, relayBytes.length, ...relayBytes);
  }
  // identifier (type 0)
  payload.push(0, identifierBytes.length, ...identifierBytes);

  const words = convertBits(payload, 8, 5, true);
  return encodeBech32("naddr", words);
}

export function canonicalBadgePath(coordinate) {
  const { kind, pubkey, identifier, relays = [] } = coordinate;
  return `/b/${encodeURIComponent(
    encodeNaddr({ kind, pubkey, identifier, relays })
  )}`;
}
