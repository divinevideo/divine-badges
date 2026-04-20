import { loadNostrProfileMetadata } from "../nostr/profile_metadata.js";

function shortenPubkey(pubkey) {
  return pubkey ? `${pubkey.slice(0, 8)}…${pubkey.slice(-6)}` : "";
}

function initialFor(value) {
  const first = (value || "").trim().charAt(0);
  return (first || "?").toUpperCase();
}

function compactCount(value) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return "0";
  }
  return new Intl.NumberFormat("en-US", {
    notation: "compact",
    maximumFractionDigits: numeric >= 1000 ? 1 : 0,
  }).format(numeric);
}

function usernameFromNip05(nip05) {
  const [name, domain] = (nip05 || "").split("@");
  const local = name?.trim() || "";
  const host = domain?.trim().toLowerCase() || "";
  if (!local && !host) {
    return null;
  }
  if (local && local !== "_") {
    return local;
  }
  if (host.endsWith(".divine.video")) {
    return host.replace(/\.divine\.video$/i, "");
  }
  return local || null;
}

export async function loadDivineProfile(pubkey, apiBase = "https://api.divine.video") {
  try {
    const response = await fetch(`${apiBase}/api/users/${pubkey}`);
    if (!response.ok) {
      throw new Error(`profile request failed with ${response.status}`);
    }
    return buildNavProfile({
      pubkey,
      payload: await response.json(),
    });
  } catch {
    return buildNavProfile({
      pubkey,
      payload: null,
    });
  }
}

export function buildNavProfile({ pubkey, payload }) {
  const profile = payload?.profile || null;
  const nip05 = profile?.nip05?.trim() || null;
  const username = usernameFromNip05(nip05);
  const displayName =
    profile?.display_name?.trim() ||
    profile?.name?.trim() ||
    shortenPubkey(pubkey);

  return {
    displayName,
    avatarUrl: profile?.picture?.trim() || null,
    initials: initialFor(displayName || pubkey),
    handle: username ? `@${username}` : null,
    username,
    nip05,
    about: profile?.about?.trim() || null,
    pubkey,
  };
}

async function fetchDivine(pubkey, apiBase) {
  try {
    const response = await fetch(`${apiBase}/api/users/${pubkey}`);
    if (!response.ok) return null;
    return await response.json();
  } catch {
    return null;
  }
}

export async function loadCreatorProfile(
  pubkey,
  {
    relays = [],
    apiBase = "https://api.divine.video",
    fetchDivineFn = fetchDivine,
    loadNostrFn = loadNostrProfileMetadata,
  } = {}
) {
  const [divinePayload, nostr] = await Promise.all([
    Promise.resolve().then(() => fetchDivineFn(pubkey, apiBase)).catch(() => null),
    Promise.resolve().then(() => loadNostrFn(pubkey, relays)).catch(() => null),
  ]);
  const base = buildNavProfile({ pubkey, payload: divinePayload });
  if (!nostr) return base;
  const shortened = shortenPubkey(pubkey);
  const displayName =
    base.displayName && base.displayName !== shortened
      ? base.displayName
      : nostr.displayName || base.displayName;
  const handle = base.handle || (nostr.handle ? `@${nostr.handle}` : null);
  return {
    ...base,
    displayName,
    avatarUrl: base.avatarUrl || nostr.avatarUrl,
    nip05: base.nip05 || nostr.nip05,
    handle,
    username: base.username || nostr.handle,
    about: base.about || nostr.about,
    initials: (displayName || pubkey || "?").trim().charAt(0).toUpperCase() || "?",
  };
}

export function buildPublicCreatorSummary({ pubkey, payload, badgeState }) {
  const profile = buildNavProfile({ pubkey, payload });
  const stats = payload?.stats || {};
  const social = payload?.social || {};
  const engagement = payload?.engagement || {};

  return {
    ...profile,
    kicker: "Divine creator",
    subline: profile.handle || "Nostr creator",
    stats: [
      { label: "Videos", value: compactCount(stats.video_count) },
      { label: "Followers", value: compactCount(social.follower_count) },
      { label: "Loops", value: compactCount(engagement.total_loops) },
      { label: "Views", value: compactCount(engagement.total_views) },
      { label: "Awarded", value: compactCount(badgeState?.awarded?.length) },
      { label: "Pinned", value: compactCount(badgeState?.accepted?.length) },
    ],
  };
}
