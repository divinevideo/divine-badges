import { DIVINE_RELAY } from "./constants.js";
import {
  buildEffectiveRelays,
  discoverWriteRelays,
  hasAnyRelayPublishSuccess,
  normalizeRelayUrl,
  relayPublishMany,
} from "./relay.js";

export const LOCAL_RELAYS_STORAGE_KEY = "divine-badges.local-relays.v1";

export async function publishSignedToWriteRelays({
  pubkey,
  unsignedEvent,
  signer,
  seedRelays = [DIVINE_RELAY],
  localRelays = [],
  discoverFn = discoverWriteRelays,
  publishManyFn = relayPublishMany,
}) {
  const discovered = await discoverFn({ pubkeys: [pubkey], seedRelays });
  const relays = buildEffectiveRelays({
    divineRelays: seedRelays,
    discoveredRelays: discovered,
    localRelays,
    mode: "write",
  });
  const signed = await signer.signEvent(unsignedEvent);
  const result = await publishManyFn(relays, signed);
  return { signed, relays, result };
}

export function publishSucceeded(outcome) {
  return hasAnyRelayPublishSuccess(outcome?.result);
}

export function summarizePublishResult(outcome) {
  const ok = outcome?.result?.ok?.length || 0;
  const failed = outcome?.result?.failed?.length || 0;
  if (!ok && !failed) return "No relays attempted";
  if (!failed) return `Published to ${ok} relay${ok === 1 ? "" : "s"}`;
  if (!ok) return `Failed to publish on ${failed} relay${failed === 1 ? "" : "s"}`;
  return `Published to ${ok} relay${ok === 1 ? "" : "s"}, ${failed} failed`;
}

function resolveStorage(storage) {
  if (storage === undefined) {
    return typeof globalThis !== "undefined" ? globalThis.localStorage : null;
  }
  return storage;
}

export function readLocalRelays(storage) {
  const resolved = resolveStorage(storage);
  if (!resolved) return [];
  try {
    const raw = resolved.getItem(LOCAL_RELAYS_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .map((entry) => ({
        url: normalizeRelayUrl(entry?.url),
        read: Boolean(entry?.read),
        write: Boolean(entry?.write),
      }))
      .filter((entry) => entry.url);
  } catch {
    return [];
  }
}

export function writeLocalRelays(relays, storage) {
  const resolved = resolveStorage(storage);
  if (!resolved) return;
  const cleaned = (relays || [])
    .map((entry) => ({
      url: normalizeRelayUrl(entry?.url),
      read: Boolean(entry?.read),
      write: Boolean(entry?.write),
    }))
    .filter((entry) => entry.url);
  resolved.setItem(LOCAL_RELAYS_STORAGE_KEY, JSON.stringify(cleaned));
}
