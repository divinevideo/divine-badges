import test from "node:test";
import assert from "node:assert/strict";

import {
  canonicalBadgePath,
  decodeNpub,
  encodeNaddr,
  normalizeProfileId,
  parseBadgeCoordinate,
  parseNaddr,
} from "./identity.js";

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

test("normalizeProfileId accepts a bare Divine handle", () => {
  assert.deepEqual(normalizeProfileId("kirstenswasey"), {
    type: "nip05",
    value: "kirstenswasey@divine.video",
  });
});

test("normalizeProfileId accepts a leading-at Divine handle", () => {
  assert.deepEqual(normalizeProfileId("@kirstenswasey"), {
    type: "nip05",
    value: "kirstenswasey@divine.video",
  });
});

test("normalizeProfileId accepts an at-prefixed Divine subdomain", () => {
  assert.deepEqual(normalizeProfileId("@kirstenswasey.divine.video"), {
    type: "nip05",
    value: "kirstenswasey@divine.video",
  });
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
      relays: [],
      raw:
        "30009:e21369e63b98f58de8aa171ec9794006eb0118891ae70895106d44525b718d2b:diviner-of-the-day",
    }
  );
});

test("encodeNaddr + parseNaddr round-trip preserves identifier", () => {
  const naddr = encodeNaddr({
    kind: 30009,
    pubkey: "0".repeat(64),
    identifier: "scene-stealer",
  });
  assert.equal(parseNaddr(naddr).identifier, "scene-stealer");
});

test("canonicalBadgePath returns /b/<url-encoded-naddr>", () => {
  const coordinate = {
    kind: 30009,
    pubkey: "0".repeat(64),
    identifier: "scene-stealer",
  };
  const naddr = encodeNaddr(coordinate);
  assert.equal(
    canonicalBadgePath(coordinate),
    `/b/${encodeURIComponent(naddr)}`
  );
});

test("encodeNaddr matches the nostr-tools fixture for relays hint", () => {
  const fixture =
    "naddr1qvzqqqr48ypzqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqyv8wumn8ghj7un9d3shjtnyd9mxjmn99emxjer9duqq6umrv4hx2ttnw3jkzmr9wgcf7h3d";
  assert.equal(
    encodeNaddr({
      kind: 30009,
      pubkey: "0000000000000000000000000000000000000000000000000000000000000000",
      identifier: "scene-stealer",
      relays: ["wss://relay.divine.video"],
    }),
    fixture
  );
  const parsed = parseNaddr(fixture);
  assert.equal(parsed.kind, 30009);
  assert.equal(
    parsed.pubkey,
    "0000000000000000000000000000000000000000000000000000000000000000"
  );
  assert.equal(parsed.identifier, "scene-stealer");
  assert.deepEqual(parsed.relays, ["wss://relay.divine.video"]);
});

test("encodeNaddr + parseNaddr round-trip with empty relays array", () => {
  const naddr = encodeNaddr({
    kind: 30009,
    pubkey: "0".repeat(64),
    identifier: "scene-stealer",
    relays: [],
  });
  const parsed = parseNaddr(naddr);
  assert.equal(parsed.kind, 30009);
  assert.equal(parsed.pubkey, "0".repeat(64));
  assert.equal(parsed.identifier, "scene-stealer");
  assert.deepEqual(parsed.relays, []);
});

test("parseNaddr returns empty relays array when no relay TLVs are present", () => {
  const naddr = encodeNaddr({
    kind: 30009,
    pubkey: "e21369e63b98f58de8aa171ec9794006eb0118891ae70895106d44525b718d2b",
    identifier: "diviner-of-the-day",
  });
  assert.deepEqual(parseNaddr(naddr).relays, []);
});

test("canonicalBadgePath includes relay hints when provided", () => {
  const coordinate = {
    kind: 30009,
    pubkey: "0".repeat(64),
    identifier: "scene-stealer",
    relays: ["wss://relay.divine.video"],
  };
  const expectedNaddr = encodeNaddr(coordinate);
  assert.equal(
    canonicalBadgePath(coordinate),
    `/b/${encodeURIComponent(expectedNaddr)}`
  );
});
