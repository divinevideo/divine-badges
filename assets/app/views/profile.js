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
} from "/app/auth/session.js?v=2026-04-14-3";
import {
  buildPublicCreatorSummary,
  loadDivineProfile,
} from "/app/auth/profile.js?v=2026-04-14-3";
import {
  buildAcceptedBadgeRecords,
  buildAcceptProfileBadgesEvent,
  buildAwardedBadgeRecords,
  buildHideProfileBadgesEvent,
  coordinateFromBadgeDefinition,
  findTag,
} from "/app/nostr/badges.js?v=2026-04-14-3";
import { resolveProfileId } from "/app/nostr/identity.js?v=2026-04-14-3";
import {
  clearStatus,
  esc,
  replaceView,
  showStatus,
} from "/app/views/common.js?v=2026-04-14-3";

const DIVINE_API_BASE = "https://api.divine.video";
const BADGE_META = {
  "diviner-of-the-day": { name: "Diviner of the Day", emoji: "D" },
  "diviner-of-the-week": { name: "Diviner of the Week", emoji: "W" },
  "diviner-of-the-month": { name: "Diviner of the Month", emoji: "M" },
};

let signer = null;
let signerPubkey = null;

function getView() {
  const view = document.getElementById("view");
  if (!view) {
    throw new Error("missing #view");
  }
  return view;
}

function getAuthChip() {
  const chip = document.getElementById("auth-chip");
  if (!(chip instanceof HTMLButtonElement)) {
    throw new Error("missing #auth-chip");
  }
  return chip;
}

function routeProfileId() {
  return decodeURIComponent(window.location.pathname.replace(/^\/p\//, ""));
}

function renderLoggedOutChip() {
  const chip = getAuthChip();
  chip.innerHTML = '<span class="name">Log in</span>';
  chip.onclick = async () => {
    window.location.href = await beginDivineOAuth();
  };
}

function renderLoggedInChip(profile) {
  const chip = getAuthChip();
  const avatar = profile.avatarUrl
    ? `<span class="avatar"><img src="${esc(profile.avatarUrl)}" alt=""></span>`
    : `<span class="avatar">${esc(profile.initials)}</span>`;
  chip.innerHTML = `${avatar}<span class="name">${esc(profile.displayName)}</span>`;
  chip.onclick = () => {
    signer = null;
    signerPubkey = null;
    clearStoredSession();
    renderLoggedOutChip();
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
    renderLoggedInChip(await loadDivineProfile(signerPubkey, DIVINE_API_BASE));
    return restored;
  } catch {
    renderLoggedOutChip();
    return null;
  }
}

async function loadPublicProfile(pubkey) {
  try {
    const response = await fetch(`${DIVINE_API_BASE}/api/users/${pubkey}`);
    if (!response.ok) {
      throw new Error(`profile request failed with ${response.status}`);
    }
    return await response.json();
  } catch {
    return {
      pubkey,
      profile: null,
    };
  }
}

async function loadBadgeState(pubkey) {
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
  const coordinates = new Set(awardEvents.map((award) => findTag(award.tags, "a")).filter(Boolean));
  const acceptedCoordinates = profileEvent?.tags
    ?.filter((tag) => tag[0] === "a")
    .map((tag) => tag[1]) || [];
  acceptedCoordinates.forEach((coordinate) => coordinates.add(coordinate));
  const authors = [...new Set([...coordinates].map((coordinate) => coordinate.split(":")[1]))];
  const identifiers = [...new Set([...coordinates].map((coordinate) => coordinate.split(":")[2]))];
  const referencedDefinitions =
    authors.length && identifiers.length
      ? await relayQuery(DIVINE_RELAY, [
          {
            kinds: [BADGE_DEFINITION],
            authors,
            "#d": identifiers,
          },
        ])
      : [];
  const definitionsByCoordinate = new Map();
  for (const badge of newestFirst([...createdBadges, ...referencedDefinitions])) {
    definitionsByCoordinate.set(coordinateFromBadgeDefinition(badge), badge);
  }
  const badgeDefinitions = [...definitionsByCoordinate.values()];
  return {
    pubkey,
    profileEvent,
    awarded: buildAwardedBadgeRecords(awardEvents, badgeDefinitions),
    accepted: buildAcceptedBadgeRecords(profileEvent, awardEvents, badgeDefinitions),
    created: newestFirst(createdBadges),
  };
}

function badgeCardMarkup(record, actions = "") {
  const coordinate = record.coordinate || coordinateFromBadgeDefinition(record.badge);
  const badgeId = coordinate.split(":")[2];
  const meta = BADGE_META[badgeId] || {
    name: findTag(record.badge.tags, "name") || badgeId || "Badge",
    emoji: "★",
  };
  const description = findTag(record.badge.tags, "description");
  const period = findTag(record.award?.tags || [], "period");
  const issuer = findTag(record.badge.tags, "name") ? "Divine badge" : "Badge";
  return `
    <li class="badge">
      <div class="med">${esc(meta.emoji)}</div>
      <div class="info">
        <a class="name" href="/b/${encodeURIComponent(coordinate)}">${esc(meta.name)}</a>
        ${period ? `<div class="period">${esc(period)}</div>` : ""}
        <div class="issuer">${esc(issuer)}</div>
        ${description ? `<div class="description">${esc(description)}</div>` : ""}
      </div>
      <div class="actions">${actions}</div>
    </li>
  `;
}

function renderProfileHeader(payload, pubkey, badgeState, isOwner) {
  const summary = buildPublicCreatorSummary({ pubkey, payload, badgeState });
  const avatar = summary.avatarUrl
    ? `<img src="${esc(summary.avatarUrl)}" alt="">`
    : esc(summary.initials);
  const subItems = [
    summary.subline,
    badgeState.created.length ? `${badgeState.created.length} created` : null,
    isOwner ? "Your badge profile" : null,
  ].filter(Boolean);
  return `
    <section class="profile-hero">
      <div class="identity-row">
        <div class="profile-avatar">${avatar}</div>
        <div>
          <div class="profile-kicker">${esc(summary.kicker)}</div>
          <div class="profile-name">${esc(summary.displayName)}</div>
          ${subItems.length ? `<div class="profile-sub">${subItems.map((item) => `<span>${esc(item)}</span>`).join("")}</div>` : ""}
        </div>
      </div>
      <p class="profile-bio">${esc(summary.about || "Public badge history and Divine creator stats, pulled into one place.")}</p>
      <div class="stats-grid">
        ${summary.stats
          .map(
            (stat) => `
              <div class="stat-card">
                <div class="stat-label">${esc(stat.label)}</div>
                <div class="stat-value">${esc(stat.value)}</div>
              </div>
            `
          )
          .join("")}
      </div>
    </section>
  `;
}

function renderTabs(state, canEdit) {
  return `
    <div class="tabs">
      <div class="tab-buttons">
        <button class="tab-button active" data-tab="accepted">Accepted (${state.accepted.length})</button>
        <button class="tab-button" data-tab="awarded">Awarded (${state.awarded.length})</button>
        <button class="tab-button" data-tab="created">Created (${state.created.length})</button>
      </div>
      <div class="tab-panel" id="tab-panel"></div>
    </div>
  `;
}

function mountTabInteractions(pubkey, state, canEdit) {
  const panel = document.getElementById("tab-panel");
  const buttons = [...document.querySelectorAll(".tab-button")];
  const acceptedIds = new Set(state.accepted.map((record) => record.award.id));

  const renderTab = (tab) => {
    buttons.forEach((button) => {
      button.classList.toggle("active", button.dataset.tab === tab);
    });
    if (tab === "accepted") {
      panel.innerHTML = state.accepted.length
        ? `<ul class="badges">${state.accepted.map((record) => badgeCardMarkup(record)).join("")}</ul>`
        : '<div class="empty">No accepted badges on this profile yet.</div>';
      return;
    }
    if (tab === "created") {
      panel.innerHTML = state.created.length
        ? `<ul class="badges">${state.created
            .map((badge) =>
              badgeCardMarkup({
                badge,
                coordinate: coordinateFromBadgeDefinition(badge),
                award: null,
              })
            )
            .join("")}</ul>`
        : '<div class="empty">No created badges here yet.</div>';
      return;
    }
    panel.innerHTML = state.awarded.length
      ? `<ul class="badges">${state.awarded
          .map((record) => {
            if (!canEdit) {
              return badgeCardMarkup(record);
            }
            const accepted = acceptedIds.has(record.award.id);
            return badgeCardMarkup(
              record,
              `
                <button data-action="accept" data-coordinate="${esc(record.coordinate)}" data-award-id="${esc(
                  record.award.id
                )}" ${accepted ? "disabled" : ""}>Accept</button>
                <button data-action="hide" data-award-id="${esc(record.award.id)}" ${
                  accepted ? "" : "disabled"
                }>Hide</button>
              `
            );
          })
          .join("")}</ul>`
      : '<div class="empty">No awarded badges here yet.</div>';

    panel.querySelectorAll("button[data-action]").forEach((button) => {
      button.onclick = async () => {
        button.disabled = true;
        try {
          const createdAt = Math.floor(Date.now() / 1000);
          const event =
            button.dataset.action === "accept"
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
          window.location.reload();
        } catch (error) {
          showStatus(getView(), "err", `Could not update badges: ${error.message || error}`);
          button.disabled = false;
        }
      };
    });
  };

  buttons.forEach((button) => {
    button.onclick = () => renderTab(button.dataset.tab);
  });
  renderTab("accepted");
}

export async function mountProfilePage() {
  const root = getView();
  replaceView(root, '<p class="status info">Loading profile…</p>');
  await restoreOptionalSession();

  try {
    const routeId = routeProfileId();
    const pubkey = await resolveProfileId(routeId);
    const [profilePayload, badgeState] = await Promise.all([
      loadPublicProfile(pubkey),
      loadBadgeState(pubkey),
    ]);
    const isOwner = signerPubkey === pubkey;
    replaceView(
      root,
      `${renderProfileHeader(profilePayload, pubkey, badgeState, isOwner)}
       ${renderTabs(badgeState, isOwner)}`
    );
    mountTabInteractions(pubkey, badgeState, isOwner);
    clearStatus();
  } catch (error) {
    replaceView(
      root,
      `<div class="empty">Could not load this badge profile.</div>`
    );
    showStatus(root, "err", error.message || String(error));
  }
}
