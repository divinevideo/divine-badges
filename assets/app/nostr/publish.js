import { DIVINE_RELAY } from "./constants.js";
import {
  discoverWriteRelays,
  hasAnyRelayPublishSuccess,
  relayPublishMany,
} from "./relay.js";

export async function publishSignedToWriteRelays({
  pubkey,
  unsignedEvent,
  signer,
  seedRelays = [DIVINE_RELAY],
  discoverFn = discoverWriteRelays,
  publishManyFn = relayPublishMany,
}) {
  const relays = await discoverFn({ pubkeys: [pubkey], seedRelays });
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
