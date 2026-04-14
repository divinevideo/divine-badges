import {
  BADGE_AWARD,
  BADGE_DEFINITION,
  DIVINE_RELAY,
} from "/app/nostr/constants.js?v=2026-04-14-3";
import {
  relayPublish,
  relayQuery,
} from "/app/nostr/relay.js?v=2026-04-14-3";
import {
  beginDivineOAuth,
  bootstrapSession,
  clearStoredSession,
} from "/app/auth/session.js?v=2026-04-14-3";
import { loadDivineProfile } from "/app/auth/profile.js?v=2026-04-14-3";
import {
  buildBadgeAwardEvent,
  coordinateFromBadgeDefinition,
  findTag,
} from "/app/nostr/badges.js?v=2026-04-14-3";
import {
  parseBadgeCoordinate,
  resolveProfileId,
} from "/app/nostr/identity.js?v=2026-04-14-3";
import {
  esc,
  replaceView,
  shorten,
  showStatus,
} from "/app/views/common.js?v=2026-04-14-4";

const BADGE_META = {
  "diviner-of-the-day": { mark: "D", variant: "day", kind: "Daily badge" },
  "diviner-of-the-week": { mark: "W", variant: "week", kind: "Weekly badge" },
  "diviner-of-the-month": { mark: "M", variant: "month", kind: "Monthly badge" },
};

let signer = null;
let signerPubkey = null;

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

function routeCoordinate() {
  return decodeURIComponent(window.location.pathname.replace(/^\/b\//, ""));
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
    clearStoredSession();
    signer = null;
    signerPubkey = null;
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
    renderLoggedInChip(await loadDivineProfile(signerPubkey));
    return restored;
  } catch {
    renderLoggedOutChip();
    return null;
  }
}

async function loadBadgePageState() {
  const coordinate = parseBadgeCoordinate(routeCoordinate());
  const coordinateValue = `${coordinate.kind}:${coordinate.pubkey}:${coordinate.identifier}`;
  const [definitions, awards, issuer] = await Promise.all([
    relayQuery(DIVINE_RELAY, [
      {
        kinds: [BADGE_DEFINITION],
        authors: [coordinate.pubkey],
        "#d": [coordinate.identifier],
      },
    ]),
    relayQuery(DIVINE_RELAY, [{ kinds: [BADGE_AWARD], "#a": [coordinateValue] }]),
    loadDivineProfile(coordinate.pubkey),
  ]);
  const badge = definitions[0];
  if (!badge) {
    throw new Error("badge not found");
  }
  const recipientPubkeys = [
    ...new Set(
      awards.flatMap((award) =>
        award.tags.filter((tag) => tag[0] === "p").map((tag) => tag[1])
      )
    ),
  ];
  const awardees = await Promise.all(
    recipientPubkeys.map(async (pubkey) => ({
      pubkey,
      profile: await loadDivineProfile(pubkey),
    }))
  );
  return {
    coordinate,
    coordinateValue,
    badge,
    awards,
    awardees,
    issuer,
  };
}

function badgePresentation(state) {
  const name = findTag(state.badge.tags, "name") || state.coordinate.identifier;
  const key = state.coordinate.identifier;
  const base = BADGE_META[key] || {
    mark: name.trim().charAt(0).toUpperCase() || "B",
    variant: "day",
    kind: "Divine badge",
  };
  return {
    ...base,
    name,
    description: findTag(state.badge.tags, "description") || "A badge issued on Divine and pinned on Nostr profiles.",
    image: findTag(state.badge.tags, "image") || findTag(state.badge.tags, "thumb"),
  };
}

function heroMetaPill(label, value) {
  return `<span class="meta-pill"><span>${esc(label)}</span><strong>${esc(value)}</strong></span>`;
}

function awardeeCardMarkup(entry) {
  const { pubkey, profile } = entry;
  const avatar = profile.avatarUrl
    ? `<img src="${esc(profile.avatarUrl)}" alt="">`
    : esc(profile.initials);
  const subline = profile.handle || profile.nip05 || "Divine creator";
  return `
    <a class="awardee-card" href="/p/${encodeURIComponent(pubkey)}">
      <div class="awardee-avatar">${avatar}</div>
      <div>
        <div class="awardee-name">${esc(profile.displayName)}</div>
        <div class="awardee-sub">${esc(subline)}</div>
      </div>
    </a>
  `;
}

function badgePageMarkup(state, canAward) {
  const badge = badgePresentation(state);
  const awardCount = state.awardees.length;
  const medal = badge.image
    ? `<img src="${esc(badge.image)}" alt="">`
    : `<span class="medal-letter">${esc(badge.mark)}</span>`;
  return `
    <div class="page">
      <section class="hero-card">
        <div class="hero-copy">
          <div>
            <div class="eyebrow">${esc(badge.kind)}</div>
            <h2>${esc(badge.name)}</h2>
          </div>
          <p>${esc(badge.description)}</p>
          <div class="meta-row">
            ${heroMetaPill("Issuer", state.issuer.displayName)}
            ${heroMetaPill("Awarded", `${awardCount} ${awardCount === 1 ? "time" : "times"}`)}
            ${heroMetaPill("Kind", String(state.coordinate.kind))}
          </div>
        </div>
        <div class="medal ${esc(badge.variant)}">${medal}</div>
      </section>

      <div class="layout">
        <section class="section">
          <h3>Awardees</h3>
          <p class="kicker">${
            awardCount
              ? `People who already earned this badge on Divine.`
              : `This badge is ready to award when the first winner lands.`
          }</p>
          ${
            awardCount
              ? `<div class="awardee-grid">${state.awardees
                  .slice(0, 24)
                  .map((entry) => awardeeCardMarkup(entry))
                  .join("")}</div>`
              : '<div class="empty">No one has claimed this trophy yet.</div>'
          }
        </section>

        <div style="display:grid;gap:18px">
          <details class="technical">
            <summary>Technical details</summary>
            <div class="technical-body">
              <div class="details-list">
                <div class="detail-row">
                  <div class="detail-label">Badge coordinate</div>
                  <div class="detail-value"><code>${esc(state.coordinateValue)}</code></div>
                </div>
                <div class="detail-row">
                  <div class="detail-label">Issuer pubkey</div>
                  <div class="detail-value"><code>${esc(state.coordinate.pubkey)}</code></div>
                </div>
                <div class="detail-row">
                  <div class="detail-label">Definition identifier</div>
                  <div class="detail-value">${esc(state.coordinate.identifier)}</div>
                </div>
                ${
                  badge.image
                    ? `<div class="detail-row">
                         <div class="detail-label">Badge media</div>
                         <div class="detail-value"><a href="${esc(
                           badge.image
                         )}" target="_blank" rel="noreferrer">Open image</a></div>
                       </div>`
                    : ""
                }
              </div>
            </div>
          </details>

          ${
            canAward
              ? `<section class="section">
                   <h3>Award This Badge</h3>
                   <p class="kicker">Paste one or many recipients as npub, hex pubkeys, or NIP-05 handles.</p>
                   <div class="award-form">
                     <textarea id="recipient-input" placeholder="npub1...\ncreator@divine.video\nhexpubkey..."></textarea>
                     <div class="action-row">
                       <button class="primary" id="award-submit" type="button">Award badge</button>
                       <span style="color:rgba(208,251,203,.72);font-size:.92rem">One kind 8 event with one <code>a</code> tag and one <code>p</code> tag per recipient.</span>
                     </div>
                   </div>
                 </section>`
              : `<div class="owner-note">Only the badge issuer can award this badge.</div>`
          }
        </div>
      </div>
    </div>
  `;
}

async function publishAward(state) {
  const input = document.getElementById("recipient-input");
  const tokens = [...new Set(input.value.split(/[\s,]+/).map((value) => value.trim()).filter(Boolean))];
  if (!tokens.length) {
    showStatus(getView(), "err", "Add at least one recipient.");
    return;
  }
  const resolved = [];
  const invalid = [];
  for (const token of tokens) {
    try {
      resolved.push(await resolveProfileId(token));
    } catch {
      invalid.push(token);
    }
  }
  if (!resolved.length) {
    showStatus(getView(), "err", "No valid recipients found.");
    return;
  }
  const event = buildBadgeAwardEvent({
    pubkey: signerPubkey,
    badgeCoordinate: state.coordinateValue,
    recipients: resolved,
    createdAt: Math.floor(Date.now() / 1000),
  });
  const signed = await signer.signEvent(event);
  await relayPublish(DIVINE_RELAY, signed);
  showStatus(
    getView(),
    "info",
    invalid.length
      ? `Awarded to ${resolved.length}. Ignored ${invalid.length} invalid recipient(s).`
      : `Awarded to ${resolved.length} recipient(s).`
  );
  window.setTimeout(() => window.location.reload(), 800);
}

export async function mountBadgePage() {
  const root = getView();
  replaceView(root, '<p class="status info">Loading badge…</p>');
  await restoreOptionalSession();

  try {
    const state = await loadBadgePageState();
    const canAward = signerPubkey === state.coordinate.pubkey;
    replaceView(root, badgePageMarkup(state, canAward));
    if (canAward) {
      document.getElementById("award-submit").onclick = async () => {
        try {
          await publishAward(state);
        } catch (error) {
          showStatus(root, "err", `Could not award badge: ${error.message || error}`);
        }
      };
    }
  } catch (error) {
    replaceView(root, '<div class="empty">Could not load this badge.</div>');
    showStatus(root, "err", error.message || String(error));
  }
}
