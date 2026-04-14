import test from "node:test";
import assert from "node:assert/strict";

import {
  decodeNpub,
  normalizeProfileId,
  parseBadgeCoordinate,
  parseNaddr,
} from "./identity.js";

const BECH32_ALPHABET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

function hexToBytes(hex) {
  const result = [];
  for (let index = 0; index < hex.length; index += 2) {
    result.push(Number.parseInt(hex.slice(index, index + 2), 16));
  }
  return result;
}

function convertBits(data, fromBits, toBits, pad = true) {
  let accumulator = 0;
  let bits = 0;
  const result = [];
  const maxValue = (1 << toBits) - 1;
  for (const value of data) {
    accumulator = (accumulator << fromBits) | value;
    bits += fromBits;
    while (bits >= toBits) {
      bits -= toBits;
      result.push((accumulator >> bits) & maxValue);
    }
  }
  if (pad && bits > 0) {
    result.push((accumulator << (toBits - bits)) & maxValue);
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

function encodeNaddr({ identifier, pubkey, kind }) {
  const identifierBytes = [...new TextEncoder().encode(identifier)];
  const kindBytes = [(kind >> 24) & 0xff, (kind >> 16) & 0xff, (kind >> 8) & 0xff, kind & 0xff];
  const payload = [
    0,
    identifierBytes.length,
    ...identifierBytes,
    2,
    32,
    ...hexToBytes(pubkey),
    3,
    4,
    ...kindBytes,
  ];
  const prefix = "naddr";
  const words = convertBits(payload, 8, 5, true);
  const prefixValues = [...prefix].map((character) => character.charCodeAt(0) & 31);
  const checksumInput = [...prefixValues, 0, ...words, 0, 0, 0, 0, 0, 0];
  const polymod = bech32Polymod(checksumInput) ^ 1;
  const checksum = Array.from({ length: 6 }, (_, index) =>
    (polymod >> (5 * (5 - index))) & 31
  );
  return `${prefix}1${[...words, ...checksum].map((word) => BECH32_ALPHABET[word]).join("")}`;
}

test("normalizeProfileId accepts hex pubkeys directly", () => {
  const hex = "f".repeat(64);
  assert.deepEqual(normalizeProfileId(hex), {
    type: "pubkey",
    value: hex,
  });
});

test("decodeNpub converts npub to a hex pubkey", () => {
  const npub =
    "npub180cvv07tjdjn8tdv5s7vfsn2hm8xdx0mmdx4n4wvpjal88dltt8q7xk6my";
  const hex = decodeNpub(npub);

  assert.match(hex, /^[0-9a-f]{64}$/);
  assert.equal(normalizeProfileId(npub).value, hex);
});

test("parseBadgeCoordinate reads canonical coordinates", () => {
  assert.deepEqual(parseBadgeCoordinate("30009:issuer-pubkey:diviner-of-the-day"), {
    kind: 30009,
    pubkey: "issuer-pubkey",
    identifier: "diviner-of-the-day",
    raw: "30009:issuer-pubkey:diviner-of-the-day",
  });
});

test("parseNaddr reads badge coordinates from naddr values", () => {
  const naddr = encodeNaddr({
    identifier: "diviner-of-the-day",
    pubkey: "e21369e63b98f58de8aa171ec9794006eb0118891ae70895106d44525b718d2b",
    kind: 30009,
  });

  assert.deepEqual(
    parseNaddr(naddr),
    {
      kind: 30009,
      pubkey:
        "e21369e63b98f58de8aa171ec9794006eb0118891ae70895106d44525b718d2b",
      identifier: "diviner-of-the-day",
      raw:
        "30009:e21369e63b98f58de8aa171ec9794006eb0118891ae70895106d44525b718d2b:diviner-of-the-day",
    }
  );
});
