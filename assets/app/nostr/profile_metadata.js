import { relayQueryMany } from "./relay.js";

const PROFILE_METADATA_KIND = 0;

function deriveHandleFromNip05(nip05) {
  const [local, domain] = (nip05 || "").split("@");
  const l = local?.trim() || "";
  const d = domain?.trim().toLowerCase() || "";
  if (!l && !d) return null;
  if (l && l !== "_") return l;
  if (d.endsWith(".divine.video")) return d.replace(/\.divine\.video$/i, "");
  return l || null;
}

export function parseProfileMetadataEvent(event) {
  if (!event || event.kind !== PROFILE_METADATA_KIND) return null;
  let parsed;
  try {
    parsed = JSON.parse(event.content || "");
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object") return null;
  const nip05 =
    typeof parsed.nip05 === "string" ? parsed.nip05.trim() || null : null;
  const displayName =
    (typeof parsed.display_name === "string" && parsed.display_name.trim()) ||
    (typeof parsed.name === "string" && parsed.name.trim()) ||
    null;
  return {
    pubkey: event.pubkey,
    displayName: displayName || null,
    avatarUrl:
      typeof parsed.picture === "string" ? parsed.picture.trim() || null : null,
    nip05,
    handle: deriveHandleFromNip05(nip05),
    about: typeof parsed.about === "string" ? parsed.about.trim() || null : null,
    raw: parsed,
    createdAt: event.created_at,
  };
}

export function newestProfileMetadata(events) {
  if (!events || !events.length) return null;
  const sorted = [...events].sort(
    (a, b) => (b?.created_at || 0) - (a?.created_at || 0)
  );
  for (const event of sorted) {
    const parsed = parseProfileMetadataEvent(event);
    if (parsed) return parsed;
  }
  return null;
}

export async function loadNostrProfileMetadata(
  pubkey,
  relays,
  queryFn = relayQueryMany
) {
  if (!pubkey || !relays?.length) return null;
  const events = await queryFn(relays, [
    { kinds: [PROFILE_METADATA_KIND], authors: [pubkey], limit: 1 },
  ]);
  return newestProfileMetadata(events);
}
