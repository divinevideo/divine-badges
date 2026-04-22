import {
  DIVINE_RELAY,
  RELAY_LIST_METADATA,
} from "/app/nostr/constants.js?v=2026-04-20-1";
import {
  buildEffectiveRelays,
  buildRelayListMetadataEvent,
  discoverReadRelays,
  newestFirst,
  normalizeRelayUrl,
  relayQueryMany,
  relayQueryManyDetailed,
  relayUrlsFromRelayListEvent,
} from "/app/nostr/relay.js?v=2026-04-20-1";
import {
  publishSignedToWriteRelays,
  publishSucceeded,
  readLocalRelays,
  summarizePublishResult,
  writeLocalRelays,
} from "/app/nostr/publish.js?v=2026-04-20-1";
import {
  beginDivineOAuth,
  bootstrapSession,
  clearStoredSession,
} from "/app/auth/session.js?v=2026-04-14-3";
import { loadDivineProfile } from "/app/auth/profile.js?v=2026-04-14-3";
import {
  clearStatus,
  esc,
  replaceView,
  showStatus,
} from "/app/views/common.js?v=2026-04-14-4";

let signer = null;
let signerPubkey = null;
let state = {
  discovered: [],
  localRelays: [],
  publishedEvent: null,
  publishedListStatus: "unknown",
  checkResults: null,
};

function getView() {
  const root = document.getElementById("view");
  if (!root) {
    throw new Error("missing #view");
  }
  return root;
}

function getAuthChip() {
  const chip = document.getElementById("auth-chip");
  if (!(chip instanceof HTMLButtonElement)) {
    throw new Error("missing #auth-chip");
  }
  return chip;
}

function renderLoggedOutChip() {
  const chip = getAuthChip();
  chip.className = "auth-chip logged-out";
  chip.innerHTML = '<span class="name">Log in</span>';
  chip.onclick = async () => {
    try {
      window.location.href = await beginDivineOAuth();
    } catch (error) {
      showStatus(getView(), "err", `OAuth error: ${error.message || error}`);
    }
  };
}

function renderLoggedInChip(profile) {
  const chip = getAuthChip();
  chip.className = "auth-chip logged-in";
  const avatar = profile.avatarUrl
    ? `<span class="avatar"><img src="${esc(profile.avatarUrl)}" alt=""></span>`
    : `<span class="avatar">${esc(profile.initials)}</span>`;
  chip.innerHTML = `${avatar}<span class="name">${esc(profile.displayName)}</span>`;
  chip.onclick = () => {
    clearStoredSession();
    signer = null;
    signerPubkey = null;
    window.location.reload();
  };
}

async function restoreOptionalSession() {
  try {
    const restored = await bootstrapSession();
    if (!restored) {
      renderLoggedOutChip();
      return null;
    }
    signer = restored;
    signerPubkey = await restored.getPublicKey();
    renderLoggedInChip(await loadDivineProfile(signerPubkey));
    return restored;
  } catch {
    renderLoggedOutChip();
    return null;
  }
}

export function parseRelayListIntoPrefs(event) {
  const seen = new Map();
  if (!event || !Array.isArray(event.tags)) return [];
  for (const tag of event.tags) {
    if (tag[0] !== "r") continue;
    const url = normalizeRelayUrl(tag[1]);
    if (!url) continue;
    const marker = (tag[2] || "").trim().toLowerCase();
    const read = marker === "read" || marker === "";
    const write = marker === "write" || marker === "";
    const existing = seen.get(url);
    if (existing) {
      existing.read = existing.read || read;
      existing.write = existing.write || write;
    } else {
      seen.set(url, { url, read, write });
    }
  }
  return [...seen.values()];
}

async function discoverPublishedRelayList() {
  if (!signerPubkey) return { status: "unknown", event: null };
  const readRelays = await discoverReadRelays({
    pubkeys: [signerPubkey],
    seedRelays: [DIVINE_RELAY],
    relayListKind: RELAY_LIST_METADATA,
  });
  const detailed = await relayQueryManyDetailed(readRelays, [
    {
      kinds: [RELAY_LIST_METADATA],
      authors: [signerPubkey],
      limit: 1,
    },
  ]);
  const anyOk = detailed.relays.some((relay) => relay.status === "ok");
  if (!anyOk) {
    return { status: "unknown", event: null };
  }
  const newest = newestFirst(detailed.events)[0] || null;
  if (!newest) {
    return { status: "empty", event: null };
  }
  return { status: "loaded", event: newest };
}

function renderPage() {
  clearStatus();
  const root = getView();
  const discoveredRelays = state.discovered;
  const localRelays = state.localRelays;

  const divineCard = `
    <div class="card section">
      <h2>Divine fallback relay</h2>
      <p>Divine Badges always publishes to this relay so the issuer can find your accepted badges.</p>
      <ul class="relay-list">
        <li class="relay locked">
          <div>
            <div class="url">${esc(DIVINE_RELAY)}</div>
            <div class="markers">Read · Write · Required</div>
          </div>
          <div class="controls"><span class="relay-status ok">always on</span></div>
        </li>
      </ul>
    </div>
  `;

  const discoveredList = discoveredRelays.length
    ? `<ul class="relay-list">${discoveredRelays
        .map((pref) => {
          const markers = [];
          if (pref.read) markers.push("read");
          if (pref.write) markers.push("write");
          return `
            <li class="relay locked">
              <div>
                <div class="url">${esc(pref.url)}</div>
                <div class="markers">${esc(markers.join(" · ") || "—")}</div>
              </div>
              <div class="controls"><span class="relay-status ok">published</span></div>
            </li>
          `;
        })
        .join("")}</ul>`
    : `<p>We didn't find a published <code>kind:10002</code> relay list for your pubkey yet. Add local overrides below and click publish when you're ready.</p>`;

  const discoveredCard = `
    <div class="card section">
      <h2>Your published relay list</h2>
      <p>These are the relays announced via your current <code>kind:10002</code> event.</p>
      ${discoveredList}
    </div>
  `;

  const localList = localRelays.length
    ? `<ul class="relay-list" id="local-relay-list">${localRelays
        .map(
          (pref, idx) => `
            <li class="relay" data-idx="${idx}">
              <div>
                <div class="url">${esc(pref.url)}</div>
              </div>
              <div class="controls">
                <label><input type="checkbox" data-field="read" data-idx="${idx}" ${pref.read ? "checked" : ""}> Read</label>
                <label><input type="checkbox" data-field="write" data-idx="${idx}" ${pref.write ? "checked" : ""}> Write</label>
                <button class="danger" data-action="remove" data-idx="${idx}">Remove</button>
              </div>
            </li>
          `
        )
        .join("")}</ul>`
    : `<p>No local overrides yet. Add one below.</p>`;

  const localCard = `
    <div class="card section">
      <h2>Your local overrides</h2>
      <p>These preferences live in this browser only. Publish a <code>kind:10002</code> event to share them with other Nostr clients.</p>
      ${localList}
      <div class="row">
        <input type="url" id="new-relay-url" placeholder="wss://relay.example" autocomplete="off" spellcheck="false">
        <label><input type="checkbox" id="new-relay-read" checked> Read</label>
        <label><input type="checkbox" id="new-relay-write" checked> Write</label>
        <button class="primary" id="add-relay-btn">Add relay</button>
      </div>
    </div>
  `;

  const publishDisabled = state.publishedListStatus === "unknown";
  const publishButtonMarkup = signer
    ? `<button class="primary" id="publish-relay-list-btn"${
        publishDisabled ? " disabled" : ""
      }>Publish relay list (kind:10002)</button>${
        publishDisabled
          ? '<span class="markers">Your current published relay list could not be loaded from any relay, so publishing now could wipe it. Check your relays and reload.</span>'
          : ""
      }`
    : '<span class="markers">Log in to publish a relay list.</span>';

  const actionsCard = `
    <div class="card section">
      <h2>Actions</h2>
      <p>Check whether the effective relay set is reachable, or publish your local preferences to Nostr.</p>
      <div class="row">
        <button class="secondary" id="check-relays-btn">Check relays</button>
        ${publishButtonMarkup}
      </div>
      <div id="check-results"></div>
    </div>
  `;

  replaceView(root, divineCard + discoveredCard + localCard + actionsCard);

  wireHandlers();
  renderCheckResults();
}

function wireHandlers() {
  const addBtn = document.getElementById("add-relay-btn");
  if (addBtn) {
    addBtn.onclick = () => handleAddRelay();
  }

  const list = document.getElementById("local-relay-list");
  if (list) {
    list.querySelectorAll('input[type="checkbox"]').forEach((input) => {
      input.onchange = (event) => handleToggle(event.currentTarget);
    });
    list.querySelectorAll('button[data-action="remove"]').forEach((btn) => {
      btn.onclick = () => handleRemove(Number(btn.dataset.idx));
    });
  }

  const checkBtn = document.getElementById("check-relays-btn");
  if (checkBtn) {
    checkBtn.onclick = () => handleCheckRelays();
  }

  const publishBtn = document.getElementById("publish-relay-list-btn");
  if (publishBtn) {
    publishBtn.onclick = () => handlePublishRelayList();
  }
}

function handleAddRelay() {
  const urlInput = document.getElementById("new-relay-url");
  const readInput = document.getElementById("new-relay-read");
  const writeInput = document.getElementById("new-relay-write");
  if (!(urlInput instanceof HTMLInputElement)) return;
  const url = normalizeRelayUrl(urlInput.value);
  if (!url) {
    showStatus(getView(), "err", "Enter a wss:// or ws:// relay URL.");
    return;
  }
  const read = readInput instanceof HTMLInputElement ? readInput.checked : true;
  const write =
    writeInput instanceof HTMLInputElement ? writeInput.checked : true;
  if (!read && !write) {
    showStatus(getView(), "err", "Pick at least one of read or write.");
    return;
  }
  const next = [...state.localRelays];
  const existingIdx = next.findIndex((pref) => pref.url === url);
  if (existingIdx >= 0) {
    next[existingIdx] = { url, read, write };
  } else {
    next.push({ url, read, write });
  }
  state.localRelays = next;
  writeLocalRelays(next);
  renderPage();
}

function handleToggle(input) {
  const idx = Number(input.dataset.idx);
  const field = input.dataset.field;
  if (!Number.isInteger(idx) || (field !== "read" && field !== "write")) return;
  const next = state.localRelays.map((pref, i) =>
    i === idx ? { ...pref, [field]: Boolean(input.checked) } : pref
  );
  state.localRelays = next;
  writeLocalRelays(next);
}

function handleRemove(idx) {
  if (!Number.isInteger(idx)) return;
  const next = state.localRelays.filter((_, i) => i !== idx);
  state.localRelays = next;
  writeLocalRelays(next);
  renderPage();
}

async function handleCheckRelays() {
  const btn = document.getElementById("check-relays-btn");
  if (btn instanceof HTMLButtonElement) {
    btn.disabled = true;
    btn.textContent = "Checking…";
  }
  const relays = effectiveRelayUrls("read");
  try {
    const result = await relayQueryManyDetailed(
      relays,
      [{ kinds: [1], limit: 1 }],
      2000
    );
    state.checkResults = result.relays;
  } catch (error) {
    showStatus(getView(), "err", `Check failed: ${error.message || error}`);
  } finally {
    if (btn instanceof HTMLButtonElement) {
      btn.disabled = false;
      btn.textContent = "Check relays";
    }
    renderCheckResults();
  }
}

function renderCheckResults() {
  const container = document.getElementById("check-results");
  if (!container) return;
  if (!state.checkResults) {
    container.innerHTML = "";
    return;
  }
  container.innerHTML = `<ul class="relay-list">${state.checkResults
    .map((entry) => {
      const cls = entry.status === "ok" ? "ok" : "err";
      const detail =
        entry.status === "ok"
          ? `${entry.eventCount} event${entry.eventCount === 1 ? "" : "s"}`
          : esc(entry.error || "error");
      return `
        <li class="relay">
          <div>
            <div class="url">${esc(entry.relayUrl)}</div>
          </div>
          <div class="controls"><span class="relay-status ${cls}">${esc(entry.status)}</span> <span class="markers">${detail}</span></div>
        </li>
      `;
    })
    .join("")}</ul>`;
}

function effectiveRelayUrls(mode) {
  return buildEffectiveRelays({
    divineRelays: [DIVINE_RELAY],
    discoveredRelays: state.discovered
      .filter((pref) => (mode === "read" ? pref.read : pref.write))
      .map((pref) => pref.url),
    localRelays: state.localRelays,
    mode,
  });
}

async function handlePublishRelayList() {
  if (!signer || !signerPubkey) {
    showStatus(getView(), "err", "Log in before publishing your relay list.");
    return;
  }
  if (state.publishedListStatus === "unknown") {
    showStatus(
      getView(),
      "err",
      "Your current published relay list could not be loaded from any relay. Publishing now could wipe it. Check your relays and reload."
    );
    return;
  }
  const btn = document.getElementById("publish-relay-list-btn");
  if (btn instanceof HTMLButtonElement) {
    btn.disabled = true;
    btn.textContent = "Publishing…";
  }
  try {
    const union = mergePrefsForPublish(state.discovered, state.localRelays);
    const event = buildRelayListMetadataEvent({
      pubkey: signerPubkey,
      relays: union,
      createdAt: Math.floor(Date.now() / 1000),
    });
    const outcome = await publishSignedToWriteRelays({
      pubkey: signerPubkey,
      unsignedEvent: event,
      signer,
      localRelays: state.localRelays,
    });
    if (publishSucceeded(outcome)) {
      showStatus(getView(), "ok", summarizePublishResult(outcome));
      // Refresh the discovered list so the published card reflects what was sent.
      state.discovered = union.filter((pref) => pref.read || pref.write);
      state.publishedListStatus = "loaded";
      state.publishedEvent = outcome.signed || state.publishedEvent;
      renderPage();
    } else {
      showStatus(
        getView(),
        "err",
        `Could not publish relay list: ${summarizePublishResult(outcome)}`
      );
    }
  } catch (error) {
    showStatus(
      getView(),
      "err",
      `Could not publish relay list: ${error.message || error}`
    );
  } finally {
    if (btn instanceof HTMLButtonElement) {
      btn.disabled = false;
      btn.textContent = "Publish relay list (kind:10002)";
    }
  }
}

export function mergePrefsForPublish(discovered, locals) {
  const map = new Map();
  for (const pref of discovered || []) {
    const url = normalizeRelayUrl(pref?.url);
    if (!url) continue;
    map.set(url, { url, read: Boolean(pref.read), write: Boolean(pref.write) });
  }
  for (const pref of locals || []) {
    const url = normalizeRelayUrl(pref?.url);
    if (!url) continue;
    // Local overrides replace the discovered marker.
    map.set(url, { url, read: Boolean(pref.read), write: Boolean(pref.write) });
  }
  return [...map.values()];
}

export async function mountRelaysPage() {
  const root = getView();
  showStatus(root, "info", "Loading your relay preferences…");
  await restoreOptionalSession();
  state.localRelays = readLocalRelays();
  try {
    const { status, event } = await discoverPublishedRelayList();
    state.publishedListStatus = status;
    state.publishedEvent = event;
    state.discovered = parseRelayListIntoPrefs(event);
  } catch (error) {
    // Non-fatal: render with whatever we have. Treat as unknown so we don't
    // allow publishing a list that could wipe an existing one.
    state.publishedListStatus = "unknown";
    state.publishedEvent = null;
    state.discovered = [];
    showStatus(
      root,
      "info",
      `Could not load your published relay list: ${error.message || error}`
    );
  }
  clearStatus();
  renderPage();
}
