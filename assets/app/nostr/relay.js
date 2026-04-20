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

export async function relayQueryMany(
  relayUrls,
  filters,
  timeoutMs = 6000,
  queryFn = relayQuery
) {
  const uniqueRelayUrls = [...new Set((relayUrls || []).filter(Boolean))];
  if (!uniqueRelayUrls.length) {
    return [];
  }
  const settled = await Promise.allSettled(
    uniqueRelayUrls.map((relayUrl) => queryFn(relayUrl, filters, timeoutMs))
  );
  return mergeRelayEvents(
    settled
      .filter((result) => result.status === "fulfilled")
      .map((result) => result.value)
  );
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
