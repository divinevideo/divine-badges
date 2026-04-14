import {
  BADGE_AWARD,
  BADGE_DEFINITION,
  DIVINE_RELAY,
  PROFILE_BADGES,
  PROFILE_BADGES_D,
} from "/app/nostr/constants.js?v=2026-04-14-3";
import {
  newestFirst,
  relayPublish,
  relayQuery,
} from "/app/nostr/relay.js?v=2026-04-14-3";
import {
  beginDivineOAuth,
  bootstrapSession,
  clearStoredSession,
  loginWithBunker,
  loginWithExtension,
  loginWithNsec,
  markSessionActive,
} from "/app/auth/session.js?v=2026-04-14-3";
import { buildNavProfile, loadDivineProfile } from "/app/auth/profile.js?v=2026-04-14-3";
import {
  buildAcceptedBadgeRecords,
  buildAcceptProfileBadgesEvent,
  buildAwardedBadgeRecords,
  buildHideProfileBadgesEvent,
  coordinateFromBadgeDefinition,
  extractProfileBadgePairs,
  findTag,
} from "/app/nostr/badges.js?v=2026-04-14-3";
import {
  clearStatus,
  esc,
  replaceView,
  shorten,
  showStatus,
} from "/app/views/common.js?v=2026-04-14-3";

const DIVINE_API_BASE = "https://api.divine.video";
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

function getAuthChip() {
  const button = document.getElementById("auth-chip");
  if (!(button instanceof HTMLButtonElement)) {
    throw new Error("missing #auth-chip");
  }
  return button;
}

async function beginPrimaryLogin() {
  window.location.href = await beginDivineOAuth();
}

function renderAuthChipLoggedOut() {
  const chip = getAuthChip();
  chip.className = "auth-chip logged-out";
  chip.disabled = false;
  chip.innerHTML = '<span class="name">Log in</span>';
  chip.onclick = async () => {
    try {
      await beginPrimaryLogin();
    } catch (error) {
      showStatus(getView(), "err", `OAuth error: ${error.message || error}`);
    }
  };
}

function renderAuthChipProfile(profile, isLoading = false) {
  const chip = getAuthChip();
  chip.className = "auth-chip logged-in";
  chip.disabled = isLoading;
  const avatar = profile.avatarUrl
    ? `<span class="avatar"><img src="${esc(profile.avatarUrl)}" alt=""></span>`
    : `<span class="avatar">${esc(profile.initials)}</span>`;
  chip.innerHTML = `${avatar}<span class="name">${esc(profile.displayName)}</span>`;
  chip.title = "Log out";
  chip.onclick = () => {
    signer = null;
    clearStoredSession();
    renderAuthChipLoggedOut();
    renderLogin(getView());
    clearStatus();
  };
}

function renderLogin(root) {
  renderAuthChipLoggedOut();
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
      await beginPrimaryLogin();
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
      <p id="status" class="status info">Looking up your badges on the Divine relay…</p>
      <div id="badges"></div>
    `
  );
}

async function onLogin(nextSigner) {
  signer = nextSigner;
  markSessionActive();
  const pubkey = await signer.getPublicKey();
  const root = getView();
  renderAuthChipProfile(
    buildNavProfile({
      pubkey,
      payload: null,
    }),
    true
  );
  renderLoaded(root, pubkey);
  renderAuthChipProfile(await loadDivineProfile(pubkey, DIVINE_API_BASE));
  await loadBadges(pubkey);
}

async function loadBadges(pubkey) {
  const root = getView();
  try {
    const [awardEvents, profileBadges, createdBadges] = await Promise.all([
      relayQuery(DIVINE_RELAY, [{ kinds: [BADGE_AWARD], "#p": [pubkey] }]),
      relayQuery(DIVINE_RELAY, [
        {
          kinds: [PROFILE_BADGES],
          authors: [pubkey],
          "#d": [PROFILE_BADGES_D],
          limit: 1,
        },
      ]),
      relayQuery(DIVINE_RELAY, [{ kinds: [BADGE_DEFINITION], authors: [pubkey] }]),
    ]);
    const profileEvent = newestFirst(profileBadges)[0] || null;
    const coordinates = new Set(
      awardEvents.map((award) => findTag(award.tags, "a")).filter(Boolean)
    );
    for (const pair of extractProfileBadgePairs(profileEvent)) {
      if (pair.a) {
        coordinates.add(pair.a);
      }
    }
    const definitionAuthors = [...new Set([...coordinates].map((coordinate) => coordinate.split(":")[1]))];
    const definitionIds = [...new Set([...coordinates].map((coordinate) => coordinate.split(":")[2]))];
    const awardedBadgeDefinitions =
      definitionAuthors.length && definitionIds.length
        ? await relayQuery(DIVINE_RELAY, [
            {
              kinds: [BADGE_DEFINITION],
              authors: definitionAuthors,
              "#d": definitionIds,
            },
          ])
        : [];
    const definitionsByCoordinate = new Map();
    for (const badge of newestFirst([...createdBadges, ...awardedBadgeDefinitions])) {
      definitionsByCoordinate.set(coordinateFromBadgeDefinition(badge), badge);
    }
    const badgeDefinitions = [...definitionsByCoordinate.values()];
    renderBadgeTabs(pubkey, {
      profileEvent,
      awarded: buildAwardedBadgeRecords(awardEvents, badgeDefinitions),
      accepted: buildAcceptedBadgeRecords(profileEvent, awardEvents, badgeDefinitions),
      created: newestFirst(createdBadges),
    });
  } catch (error) {
    showStatus(root, "err", `Could not load badges: ${error.message || error}`);
  }
}

function getBadgeMeta(record) {
  const fallbackCoordinate = record.coordinate || coordinateFromBadgeDefinition(record.badge);
  const badgeId = fallbackCoordinate.split(":")[2] || "";
  return (
    BADGE_META[badgeId] || {
      name: findTag(record.badge.tags, "name") || badgeId || "Badge",
      emoji: "★",
      variant: "day",
    }
  );
}

function getAcceptedAwardIds(records) {
  return new Set(records.map((record) => record.award.id));
}

function badgeCardMarkup(record, actions = "") {
  const meta = getBadgeMeta(record);
  const period = findTag(record.award?.tags || [], "period");
  const issuer = record.badge?.pubkey ? shorten(record.badge.pubkey) : "";
  const description = findTag(record.badge?.tags || [], "description");
  return `
    <li class="badge ${meta.variant}${record.accepted ? " accepted" : ""}">
      <div class="med">${esc(meta.emoji)}</div>
      <div class="info">
        <div class="name">${esc(meta.name)}</div>
        <div class="period">${esc(period || "—")}</div>
        ${issuer ? `<div class="issuer">Issuer: ${esc(issuer)}</div>` : ""}
        ${description ? `<div class="description">${esc(description)}</div>` : ""}
      </div>
      <div class="actions">${actions}</div>
    </li>
  `;
}

function renderBadgeTabs(pubkey, state) {
  clearStatus();
  const root = getView();
  const panel = document.getElementById("badges");
  if (!panel) {
    return;
  }
  const acceptedAwardIds = getAcceptedAwardIds(state.accepted);
  replaceView(
    panel,
    `
      <div class="tabs">
        <div class="tab-buttons">
          <button class="tab-button active" data-tab="accepted">Accepted</button>
          <button class="tab-button" data-tab="awarded">Awarded</button>
          <button class="tab-button" data-tab="created">Created</button>
        </div>
        <div class="tab-panel" id="badge-tab-panel"></div>
      </div>
    `
  );

  const tabPanel = document.getElementById("badge-tab-panel");
  const buttons = [...root.querySelectorAll(".tab-button")];

  const renderTab = (tabName) => {
    buttons.forEach((button) => {
      button.classList.toggle("active", button.dataset.tab === tabName);
    });
    if (tabName === "accepted") {
      if (!state.accepted.length) {
        tabPanel.innerHTML =
          '<div class="empty">No accepted badges yet. Accept badges from the Awarded tab when you want them on your profile.</div>';
        return;
      }
      tabPanel.innerHTML = `<ul class="badges">${state.accepted
        .map((record) => badgeCardMarkup({ ...record, accepted: true }))
        .join("")}</ul>`;
      return;
    }

    if (tabName === "created") {
      if (!state.created.length) {
        tabPanel.innerHTML =
          '<div class="empty">No badges created from this account yet.</div>';
        return;
      }
      tabPanel.innerHTML = `<ul class="badges">${state.created
        .map((badge) =>
          badgeCardMarkup({
            badge,
            coordinate: coordinateFromBadgeDefinition(badge),
            award: null,
          })
        )
        .join("")}</ul>`;
      return;
    }

    if (!state.awarded.length) {
      tabPanel.innerHTML =
        '<div class="empty">No badges awarded here yet. Keep looping — we check every UTC morning.</div>';
      return;
    }
    tabPanel.innerHTML = `<ul class="badges">${state.awarded
      .map((record) => {
        const accepted = acceptedAwardIds.has(record.award.id);
        return badgeCardMarkup(
          record,
          `
            <button class="primary" data-action="accept" data-award-id="${esc(
              record.award.id
            )}" data-coordinate="${esc(record.coordinate)}" ${accepted ? "disabled" : ""}>Accept</button>
            <button class="secondary" data-action="hide" data-award-id="${esc(
              record.award.id
            )}" ${accepted ? "" : "disabled"}>Hide</button>
          `
        );
      })
      .join("")}</ul>`;
    tabPanel.querySelectorAll("button[data-action]").forEach((button) => {
      button.onclick = () =>
        handleAwardAction(pubkey, state, button.dataset.action, button);
    });
  };

  buttons.forEach((button) => {
    button.onclick = () => renderTab(button.dataset.tab);
  });
  renderTab("accepted");
}

async function handleAwardAction(pubkey, state, action, button) {
  button.disabled = true;
  button.textContent = action === "accept" ? "Signing…" : "Updating…";
  try {
    const createdAt = Math.floor(Date.now() / 1000);
    const event =
      action === "accept"
        ? buildAcceptProfileBadgesEvent({
            pubkey,
            profileEvent: state.profileEvent,
            badgeCoordinate: button.dataset.coordinate,
            awardId: button.dataset.awardId,
            relayUrl: DIVINE_RELAY,
            createdAt,
          })
        : buildHideProfileBadgesEvent({
            pubkey,
            profileEvent: state.profileEvent,
            awardId: button.dataset.awardId,
            createdAt,
          });
    const signed = await signer.signEvent(event);
    await relayPublish(DIVINE_RELAY, signed);
    await loadBadges(pubkey);
  } catch (error) {
    showStatus(getView(), "err", `Could not update badges: ${error.message || error}`);
    button.disabled = false;
    button.textContent = action === "accept" ? "Accept" : "Hide";
  }
}

export async function mountMePage() {
  const root = getView();
  renderLogin(root);
  showStatus(root, "info", "Checking for an existing session…");
  try {
    const restoredSigner = await bootstrapSession();
    if (restoredSigner) {
      await onLogin(restoredSigner);
      return;
    }
    clearStatus();
  } catch (error) {
    renderLogin(root);
    showStatus(root, "err", `Startup error: ${error.message || error}`);
  }
}
