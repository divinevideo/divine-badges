export const RELAY_LIST_KIND = 10002;

export function normalizeRelayUrl(value) {
  if (value === undefined || value === null) return null;
  const trimmed = String(value).trim().toLowerCase();
  if (!/^wss?:\/\//.test(trimmed)) return null;
  return trimmed;
}

export function buildEffectiveRelays({
  divineRelays = [],
  discoveredRelays = [],
  localRelays = [],
  mode = "read",
} = {}) {
  const out = [];
  const seen = new Set();
  const addUrl = (raw) => {
    const normalized = normalizeRelayUrl(raw);
    if (!normalized || seen.has(normalized)) return;
    seen.add(normalized);
    out.push(normalized);
  };
  (divineRelays || []).forEach((url) => addUrl(url));
  (discoveredRelays || []).forEach((url) => addUrl(url));
  (localRelays || []).forEach((pref) => {
    if (!pref?.url) return;
    if (mode === "read" && pref.read) addUrl(pref.url);
    if (mode === "write" && pref.write) addUrl(pref.url);
  });
  return out;
}

export function buildRelayListMetadataEvent({ pubkey, relays, createdAt } = {}) {
  const tags = [];
  for (const pref of relays || []) {
    const url = normalizeRelayUrl(pref?.url);
    if (!url) continue;
    const r = Boolean(pref.read);
    const w = Boolean(pref.write);
    if (r && w) tags.push(["r", url]);
    else if (r) tags.push(["r", url, "read"]);
    else if (w) tags.push(["r", url, "write"]);
  }
  return {
    kind: RELAY_LIST_KIND,
    pubkey,
    content: "",
    tags,
    created_at: createdAt,
  };
}

export function newestFirst(events) {
  return [...events].sort((left, right) => right.created_at - left.created_at);
}

export function mergeRelayEvents(resultSets) {
  const eventsById = new Map();
  for (const resultSet of resultSets) {
    for (const event of resultSet || []) {
      if (!event?.id || eventsById.has(event.id)) {
        continue;
      }
      eventsById.set(event.id, event);
    }
  }
  return [...eventsById.values()];
}

export function relayUrlsFromRelayListEvent(relayListEvent, mode = "read") {
  const opposite = mode === "write" ? "read" : "write";
  const urls = [];
  for (const tag of relayListEvent?.tags || []) {
    if (tag[0] !== "r") {
      continue;
    }
    const url = tag[1]?.trim();
    const marker = tag[2]?.trim().toLowerCase();
    if (!url || !/^wss?:\/\//i.test(url) || marker === opposite) {
      continue;
    }
    urls.push(url);
  }
  return [...new Set(urls)];
}

export function relayQuery(relayUrl, filters, timeoutMs = 6000) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(relayUrl);
    const subId = "r" + Math.random().toString(36).slice(2, 10);
    const events = [];
    const done = (value) => {
      clearTimeout(timer);
      try {
        ws.close();
      } catch {}
      resolve(value);
    };
    const timer = setTimeout(() => done(events), timeoutMs);
    ws.onopen = () => ws.send(JSON.stringify(["REQ", subId, ...filters]));
    ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        if (message[0] === "EVENT" && message[1] === subId) {
          events.push(message[2]);
        } else if (message[0] === "EOSE" && message[1] === subId) {
          done(events);
        }
      } catch {}
    };
    ws.onerror = () => {
      clearTimeout(timer);
      reject(new Error("relay error"));
    };
  });
}

export async function relayQueryManyDetailed(
  relayUrls,
  filters,
  timeoutMs = 6000,
  queryFn = relayQuery
) {
  const uniqueRelayUrls = [...new Set((relayUrls || []).filter(Boolean))];
  if (!uniqueRelayUrls.length) {
    return { events: [], relays: [] };
  }
  const settled = await Promise.allSettled(
    uniqueRelayUrls.map((relayUrl) => queryFn(relayUrl, filters, timeoutMs))
  );
  const relays = [];
  const successful = [];
  settled.forEach((result, index) => {
    const relayUrl = uniqueRelayUrls[index];
    if (result.status === "fulfilled") {
      const value = Array.isArray(result.value) ? result.value : [];
      successful.push(value);
      relays.push({
        relayUrl,
        status: "ok",
        eventCount: value.length,
      });
    } else {
      const err = result.reason;
      relays.push({
        relayUrl,
        status: "error",
        eventCount: 0,
        error: err?.message || String(err),
      });
    }
  });
  return {
    events: mergeRelayEvents(successful),
    relays,
  };
}

export async function relayQueryMany(
  relayUrls,
  filters,
  timeoutMs = 6000,
  queryFn = relayQuery
) {
  const { events } = await relayQueryManyDetailed(
    relayUrls,
    filters,
    timeoutMs,
    queryFn
  );
  return events;
}

export async function discoverReadRelays(
  { pubkeys, seedRelays, relayListKind = 10002 },
  queryFn = relayQueryMany
) {
  const uniquePubkeys = [...new Set((pubkeys || []).filter(Boolean))];
  const baseRelays = [...new Set((seedRelays || []).filter(Boolean))];
  if (!uniquePubkeys.length) {
    return baseRelays;
  }
  const relayListEvents = await queryFn(baseRelays, [
    {
      kinds: [relayListKind],
      authors: uniquePubkeys,
    },
  ]);
  const discovered = relayListEvents.flatMap((event) =>
    relayUrlsFromRelayListEvent(event)
  );
  return [...new Set([...baseRelays, ...discovered])];
}

export async function discoverWriteRelays(
  { pubkeys, seedRelays, relayListKind = 10002 },
  queryFn = relayQueryMany
) {
  const uniquePubkeys = [...new Set((pubkeys || []).filter(Boolean))];
  const baseRelays = [...new Set((seedRelays || []).filter(Boolean))];
  if (!uniquePubkeys.length) {
    return baseRelays;
  }
  const relayListEvents = await queryFn(baseRelays, [
    {
      kinds: [relayListKind],
      authors: uniquePubkeys,
    },
  ]);
  const discovered = relayListEvents.flatMap((event) =>
    relayUrlsFromRelayListEvent(event, "write")
  );
  return [...new Set([...baseRelays, ...discovered])];
}

export function relayPublish(relayUrl, nostrEvent, timeoutMs = 8000) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(relayUrl);
    const timer = setTimeout(() => {
      try {
        ws.close();
      } catch {}
      reject(new Error("relay timeout"));
    }, timeoutMs);
    ws.onopen = () => ws.send(JSON.stringify(["EVENT", nostrEvent]));
    ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        if (message[0] === "OK" && message[1] === nostrEvent.id) {
          clearTimeout(timer);
          try {
            ws.close();
          } catch {}
          if (message[2]) {
            resolve();
          } else {
            reject(new Error(message[3] || "relay rejected event"));
          }
        }
      } catch {}
    };
    ws.onerror = () => {
      clearTimeout(timer);
      reject(new Error("relay error"));
    };
  });
}

export async function relayPublishMany(
  relayUrls,
  nostrEvent,
  timeoutMs = 8000,
  publishFn = relayPublish
) {
  const uniqueRelayUrls = [...new Set((relayUrls || []).filter(Boolean))];
  if (!uniqueRelayUrls.length) {
    return { ok: [], failed: [] };
  }
  const settled = await Promise.allSettled(
    uniqueRelayUrls.map((relayUrl) => publishFn(relayUrl, nostrEvent, timeoutMs))
  );
  const ok = [];
  const failed = [];
  settled.forEach((result, index) => {
    const relayUrl = uniqueRelayUrls[index];
    if (result.status === "fulfilled") {
      ok.push(relayUrl);
    } else {
      const err = result.reason;
      failed.push({ relayUrl, error: err?.message || String(err) });
    }
  });
  return { ok, failed };
}

export function hasAnyRelayPublishSuccess(result) {
  return Boolean(result?.ok?.length);
}
