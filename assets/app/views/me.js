import { DIVINE_RELAY } from "/app/nostr/constants.js";
import { newestFirst, relayPublish, relayQuery } from "/app/nostr/relay.js";
import {
  beginDivineOAuth,
  bootstrapSession,
  clearStoredSession,
  loginWithBunker,
  loginWithExtension,
  loginWithNsec,
  markSessionActive,
} from "/app/auth/session.js";
import {
  clearStatus,
  esc,
  renderEmptyState,
  replaceView,
  shorten,
  showStatus,
} from "/app/views/common.js";

const ISSUER = "e21369e63b98f58de8aa171ec9794006eb0118891ae70895106d44525b718d2b";
const BADGE_META = {
  "diviner-of-the-day": { name: "Diviner of the Day", emoji: "D", variant: "day" },
  "diviner-of-the-week": { name: "Diviner of the Week", emoji: "W", variant: "week" },
  "diviner-of-the-month": {
    name: "Diviner of the Month",
    emoji: "M",
    variant: "month",
  },
};

let signer = null;

function getView() {
  const view = document.getElementById("view");
  if (!view) {
    throw new Error("missing #view mount");
  }
  return view;
}

function renderLogin(root) {
  replaceView(
    root,
    `
      <div class="card">
        <h2>Log in with Divine</h2>
        <p>Use your Divine OAuth account. We ask for permission to read your pubkey and sign the badge-acceptance event on your behalf.</p>
        <div class="row"><button class="primary" id="oauth-btn">Continue with Divine</button></div>
      </div>
      <div class="card">
        <h2>Browser extension</h2>
        <p>Works with Alby, nos2x, Flamingo, Soapbox Signer, or any NIP-07 extension. Keys never leave the extension.</p>
        <div class="row"><button class="secondary" id="ext-btn">Log in with extension</button></div>
      </div>
      <div class="card">
        <h2>Remote signer</h2>
        <p>Paste a <code>bunker://</code> URL from Amber, nsec.app, or another NIP-46 signer.</p>
        <div class="row">
          <input id="bunker-input" placeholder="bunker://…" autocomplete="off" spellcheck="false">
          <button class="secondary" id="bunker-btn">Connect</button>
        </div>
      </div>
      <div class="card">
        <h2>Paste an nsec</h2>
        <p>Use this only in a browser you trust. The nsec stays in this tab — nothing is uploaded.</p>
        <div class="row">
          <input id="nsec-input" type="password" placeholder="nsec1…" autocomplete="off" spellcheck="false">
          <button class="secondary" id="nsec-btn">Log in</button>
        </div>
      </div>
    `
  );

  document.getElementById("oauth-btn").onclick = async () => {
    try {
      window.location.href = await beginDivineOAuth();
    } catch (error) {
      showStatus(root, "err", `OAuth error: ${error.message || error}`);
    }
  };

  document.getElementById("ext-btn").onclick = async () => {
    try {
      await onLogin(await loginWithExtension());
    } catch (error) {
      showStatus(root, "err", `Extension error: ${error.message || error}`);
    }
  };

  document.getElementById("bunker-btn").onclick = async () => {
    const bunkerInput = document.getElementById("bunker-input");
    const bunkerUrl = bunkerInput.value.trim();
    if (!bunkerUrl) {
      return;
    }
    showStatus(root, "info", "Connecting to bunker…");
    try {
      await onLogin(await loginWithBunker(bunkerUrl));
    } catch (error) {
      showStatus(root, "err", `Bunker error: ${error.message || error}`);
    }
  };

  document.getElementById("nsec-btn").onclick = async () => {
    const nsecInput = document.getElementById("nsec-input");
    const nsec = nsecInput.value.trim();
    if (!nsec) {
      return;
    }
    try {
      await onLogin(await loginWithNsec(nsec));
    } catch (error) {
      showStatus(root, "err", `nsec error: ${error.message || error}`);
    }
  };
}

function renderLoaded(root, pubkey) {
  replaceView(
    root,
    `
      <div class="you">
        <div class="avatar">${esc(pubkey.charAt(0).toUpperCase())}</div>
        <div class="who">
          <strong>Logged in</strong>
          <code>${esc(shorten(pubkey))}</code>
        </div>
        <span class="spacer"></span>
        <button class="danger" id="logout-btn">Log out</button>
      </div>
      <p id="status" class="status info">Looking up your badges on the Divine relay…</p>
      <ul class="badges" id="badges"></ul>
    `
  );
  document.getElementById("logout-btn").onclick = () => {
    signer = null;
    clearStoredSession();
    renderLogin(root);
  };
}

async function onLogin(nextSigner) {
  signer = nextSigner;
  markSessionActive();
  const pubkey = await signer.getPublicKey();
  const root = getView();
  renderLoaded(root, pubkey);
  await loadBadges(pubkey);
}

async function loadBadges(pubkey) {
  const root = getView();
  try {
    const [awards, profileBadges] = await Promise.all([
      relayQuery(DIVINE_RELAY, [{ kinds: [8], authors: [ISSUER], "#p": [pubkey] }]),
      relayQuery(DIVINE_RELAY, [
        { kinds: [30008], authors: [pubkey], "#d": ["profile_badges"], limit: 1 },
      ]),
    ]);
    const profileEvent = newestFirst(profileBadges)[0] || null;
    const acceptedIds = new Set();
    if (profileEvent) {
      for (const tag of profileEvent.tags) {
        if (tag[0] === "e") {
          acceptedIds.add(tag[1]);
        }
      }
    }
    renderBadges(pubkey, awards, acceptedIds, profileEvent);
  } catch (error) {
    showStatus(root, "err", `Could not load badges: ${error.message || error}`);
  }
}

function renderBadges(pubkey, awards, acceptedIds, profileEvent) {
  clearStatus();
  const list = document.getElementById("badges");
  if (!list) {
    return;
  }
  if (!awards.length) {
    renderEmptyState(list.parentElement, "No Diviner badges here yet. Keep looping — we check every UTC morning.");
    return;
  }
  list.innerHTML = "";
  for (const award of newestFirst(awards)) {
    const badgeRef = award.tags.find((tag) => tag[0] === "a");
    const period = award.tags.find((tag) => tag[0] === "period");
    const coord = badgeRef ? badgeRef[1] : "";
    const badgeId = coord.split(":")[2] || "";
    const meta = BADGE_META[badgeId] || {
      name: badgeId || "Diviner",
      emoji: "★",
      variant: "day",
    };
    const accepted = acceptedIds.has(award.id);
    const item = document.createElement("li");
    item.className = `badge ${meta.variant}${accepted ? " accepted" : ""}`;
    item.innerHTML = `
      <div class="med">${esc(meta.emoji)}</div>
      <div class="info">
        <div class="name">${esc(meta.name)}</div>
        <div class="period">${esc(period ? period[1] : "—")}</div>
      </div>
      <div class="actions">
        ${
          accepted
            ? "<button disabled>Pinned ✓</button>"
            : `<button class="primary" data-id="${esc(award.id)}" data-coord="${esc(coord)}">Pin to profile</button>`
        }
      </div>
    `;
    list.appendChild(item);
  }
  list.querySelectorAll("button[data-id]").forEach((button) => {
    button.onclick = () =>
      acceptBadge(
        pubkey,
        button.dataset.id,
        button.dataset.coord,
        acceptedIds,
        profileEvent,
        button
      );
  });
}

async function acceptBadge(pubkey, awardId, coord, acceptedIds, profileEvent, button) {
  button.disabled = true;
  button.textContent = "Signing…";
  try {
    const pairs = [];
    if (profileEvent) {
      let current = null;
      for (const tag of profileEvent.tags) {
        if (tag[0] === "a") {
          if (current) {
            pairs.push(current);
          }
          current = { a: tag[1], relay: tag[2] };
        } else if (tag[0] === "e" && current) {
          current.e = tag[1];
          current.eRelay = tag[2];
        }
      }
      if (current) {
        pairs.push(current);
      }
    }
    pairs.push({ a: coord, e: awardId, eRelay: DIVINE_RELAY });
    const tags = [["d", "profile_badges"]];
    for (const pair of pairs) {
      if (pair.a) {
        tags.push(pair.relay ? ["a", pair.a, pair.relay] : ["a", pair.a]);
      }
      if (pair.e) {
        tags.push(pair.eRelay ? ["e", pair.e, pair.eRelay] : ["e", pair.e]);
      }
    }
    const event = await signer.signEvent({
      kind: 30008,
      content: "",
      tags,
      created_at: Math.floor(Date.now() / 1000),
    });
    button.textContent = "Publishing…";
    await relayPublish(DIVINE_RELAY, event);
    button.textContent = "Pinned ✓";
    button.classList.remove("primary");
    button.closest(".badge").classList.add("accepted");
    acceptedIds.add(awardId);
  } catch (error) {
    button.disabled = false;
    button.textContent = "Pin to profile";
    alert(`Could not pin badge: ${error.message || error}`);
  }
}

export async function mountMePage() {
  const root = getView();
  try {
    const restoredSigner = await bootstrapSession();
    if (restoredSigner) {
      await onLogin(restoredSigner);
      return;
    }
    renderLogin(root);
  } catch (error) {
    renderLogin(root);
    showStatus(root, "err", `Startup error: ${error.message || error}`);
  }
}
