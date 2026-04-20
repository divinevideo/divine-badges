import {
  BADGE_AWARD,
  BADGE_DEFINITION,
  DIVINE_RELAY,
  RELAY_LIST_METADATA,
} from "/app/nostr/constants.js?v=2026-04-14-3";
import {
  discoverReadRelays,
  relayQueryMany,
} from "/app/nostr/relay.js?v=2026-04-14-3";
import {
  beginDivineOAuth,
  bootstrapSession,
  clearStoredSession,
} from "/app/auth/session.js?v=2026-04-14-3";
import { loadDivineProfile } from "/app/auth/profile.js?v=2026-04-14-3";
import {
  buildBadgeAwardEvent,
  canAwardBadge,
  coordinateFromBadgeDefinition,
  findTag,
  parseRecipientInput,
  shouldOpenAwardPanel,
} from "/app/nostr/badges.js?v=2026-04-14-3";
import {
  publishSignedToWriteRelays,
  publishSucceeded,
  summarizePublishResult,
} from "/app/nostr/publish.js?v=2026-04-20-1";
import {
  parseBadgeCoordinate,
  resolveProfileId,
} from "/app/nostr/identity.js?v=2026-04-14-3";
import {
  clearStatus,
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
  const badgeReadRelays = await discoverReadRelays({
    pubkeys: [coordinate.pubkey],
    seedRelays: [DIVINE_RELAY],
    relayListKind: RELAY_LIST_METADATA,
  });
  const [definitions, awards, issuer] = await Promise.all([
    relayQueryMany(badgeReadRelays, [
      {
        kinds: [BADGE_DEFINITION],
        authors: [coordinate.pubkey],
        "#d": [coordinate.identifier],
      },
    ]),
    relayQueryMany(badgeReadRelays, [{ kinds: [BADGE_AWARD], "#a": [coordinateValue] }]),
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
    description:
      findTag(state.badge.tags, "description") ||
      "A badge issued on Divine and pinned on Nostr profiles.",
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

function resolvedRecipientMarkup(entry) {
  const avatar = entry.profile.avatarUrl
    ? `<img src="${esc(entry.profile.avatarUrl)}" alt="">`
    : esc(entry.profile.initials);
  return `
    <div class="resolved-recipient">
      <div class="awardee-avatar small">${avatar}</div>
      <div>
        <div class="awardee-name compact">${esc(entry.profile.displayName)}</div>
        <div class="awardee-sub">${esc(entry.profile.handle || shorten(entry.pubkey))}</div>
      </div>
    </div>
  `;
}

function awardPanelMarkup(awardState) {
  const resolvedCount = awardState.resolved.length;
  const invalidCount = awardState.invalid.length;
  return `
    <details class="award-panel" ${awardState.open ? "open" : ""}>
      <summary>Award this badge</summary>
      <div class="award-panel-body">
        <p class="kicker">Resolve one NIP-05 handle or paste bulk recipients as <code>npub</code>, hex pubkeys, or NIP-05 values.</p>
        <div class="inline-row">
          <label class="inline-field">
            <span>Quick resolve</span>
            <input id="nip05-input" value="${esc(awardState.nip05Input)}" placeholder="creator@domain.com">
          </label>
          <button class="secondary" id="add-handle" type="button" ${awardState.resolving ? "disabled" : ""}>Add handle</button>
        </div>
        <label class="stack-field">
          <span>Bulk recipients</span>
          <textarea id="recipient-input" placeholder="npub1...\nhexpubkey...\ncreator@divine.video">${esc(
            awardState.bulkInput
          )}</textarea>
        </label>
        <div class="action-row">
          <button class="secondary" id="resolve-recipients" type="button" ${
            awardState.resolving ? "disabled" : ""
          }>${awardState.resolving ? "Resolving…" : "Review recipients"}</button>
          <button class="primary" id="award-submit" type="button" ${
            awardState.publishing || !resolvedCount ? "disabled" : ""
          }>${awardState.publishing ? "Publishing…" : "Publish award"}</button>
          <span class="helper-copy">One kind 8 event with one <code>a</code> tag and one <code>p</code> tag per recipient.</span>
        </div>
        <div class="recipient-results">
          <div class="result-block">
            <div class="result-title">Ready to publish${resolvedCount ? ` · ${resolvedCount}` : ""}</div>
            ${
              resolvedCount
                ? `<div class="resolved-list">${awardState.resolved
                    .map((entry) => resolvedRecipientMarkup(entry))
                    .join("")}</div>`
                : '<div class="empty compact">Resolve recipients to preview the final award list.</div>'
            }
          </div>
          ${
            invalidCount
              ? `<div class="result-block">
                   <div class="result-title">Couldn’t resolve · ${invalidCount}</div>
                   <div class="invalid-list">${awardState.invalid
                     .map((value) => `<span class="invalid-pill">${esc(value)}</span>`)
                     .join("")}</div>
                 </div>`
              : ""
          }
        </div>
      </div>
    </details>
  `;
}

function badgePageMarkup(state, canAward, awardState) {
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
              ? awardPanelMarkup(awardState)
              : `<div class="owner-note">Only the badge issuer can award this badge.</div>`
          }
        </div>
      </div>
    </div>
  `;
}

async function resolveAwardRecipients(awardState) {
  const combined = [awardState.nip05Input, awardState.bulkInput]
    .filter(Boolean)
    .join("\n");
  const tokens = parseRecipientInput(combined);
  if (!tokens.length) {
    awardState.resolved = [];
    awardState.invalid = [];
    throw new Error("Add at least one recipient.");
  }

  const resolved = [];
  const invalid = [];
  const seenPubkeys = new Set();

  for (const token of tokens) {
    try {
      const pubkey = await resolveProfileId(token);
      if (seenPubkeys.has(pubkey)) {
        continue;
      }
      seenPubkeys.add(pubkey);
      resolved.push({
        token,
        pubkey,
        profile: await loadDivineProfile(pubkey),
      });
    } catch {
      invalid.push(token);
    }
  }

  awardState.resolved = resolved;
  awardState.invalid = invalid;
}

async function publishAward(state, awardState) {
  if (!awardState.resolved.length) {
    throw new Error("Resolve recipients before publishing.");
  }
  const event = buildBadgeAwardEvent({
    pubkey: signerPubkey,
    badgeCoordinate: state.coordinateValue,
    recipients: awardState.resolved.map((entry) => entry.pubkey),
    createdAt: Math.floor(Date.now() / 1000),
  });
  return publishSignedToWriteRelays({
    pubkey: signerPubkey,
    unsignedEvent: event,
    signer,
  });
}

function readAwardInputs(awardState) {
  const nip05Input = document.getElementById("nip05-input");
  const recipientInput = document.getElementById("recipient-input");
  awardState.nip05Input = nip05Input?.value || "";
  awardState.bulkInput = recipientInput?.value || "";
}

function bindAwardControls(state, awardState, rerender) {
  const details = document.querySelector(".award-panel");
  if (details) {
    details.addEventListener("toggle", () => {
      awardState.open = details.open;
    });
  }

  const addHandle = document.getElementById("add-handle");
  if (addHandle) {
    addHandle.onclick = async () => {
      readAwardInputs(awardState);
      awardState.open = true;
      awardState.resolving = true;
      clearStatus();
      rerender();
      try {
        await resolveAwardRecipients(awardState);
        if (awardState.nip05Input.trim()) {
          const addition = awardState.nip05Input.trim();
          awardState.bulkInput = [awardState.bulkInput, addition].filter(Boolean).join("\n");
          awardState.nip05Input = "";
        }
      } catch (error) {
        showStatus(getView(), "err", error.message || String(error));
      } finally {
        awardState.resolving = false;
        rerender();
      }
    };
  }

  const reviewButton = document.getElementById("resolve-recipients");
  if (reviewButton) {
    reviewButton.onclick = async () => {
      readAwardInputs(awardState);
      awardState.open = true;
      awardState.resolving = true;
      clearStatus();
      rerender();
      try {
        await resolveAwardRecipients(awardState);
        if (!awardState.resolved.length) {
          showStatus(getView(), "err", "No valid recipients found.");
        }
      } catch (error) {
        showStatus(getView(), "err", error.message || String(error));
      } finally {
        awardState.resolving = false;
        rerender();
      }
    };
  }

  const publishButton = document.getElementById("award-submit");
  if (publishButton) {
    publishButton.onclick = async () => {
      readAwardInputs(awardState);
      awardState.open = true;
      awardState.publishing = true;
      clearStatus();
      rerender();
      try {
        const outcome = await publishAward(state, awardState);
        if (!publishSucceeded(outcome)) {
          awardState.publishing = false;
          rerender();
          showStatus(
            getView(),
            "err",
            `Could not award badge: ${summarizePublishResult(outcome)}`
          );
          return;
        }
        const recipientCount = awardState.resolved.length;
        const baseMsg = `Awarded to ${recipientCount} recipient(s).`;
        if (outcome.result.failed.length > 0) {
          showStatus(
            getView(),
            "info",
            `${baseMsg} ${summarizePublishResult(outcome)}`
          );
        } else {
          showStatus(getView(), "info", baseMsg);
        }
        window.setTimeout(() => window.location.reload(), 800);
      } catch (error) {
        awardState.publishing = false;
        rerender();
        showStatus(getView(), "err", `Could not award badge: ${error.message || error}`);
      }
    };
  }
}

export async function mountBadgePage() {
  const root = getView();
  replaceView(root, '<p class="status info">Loading badge…</p>');
  await restoreOptionalSession();

  try {
    const state = await loadBadgePageState();
    const canAward = canAwardBadge({
      signerPubkey,
      badgeAuthorPubkey: state.coordinate.pubkey,
    });
    const awardState = {
      nip05Input: "",
      bulkInput: "",
      resolved: [],
      invalid: [],
      open: shouldOpenAwardPanel(window.location.search),
      resolving: false,
      publishing: false,
    };

    const rerender = () => {
      replaceView(root, badgePageMarkup(state, canAward, awardState));
      if (canAward) {
        bindAwardControls(state, awardState, rerender);
      }
    };

    rerender();
  } catch (error) {
    replaceView(root, '<div class="empty">Could not load this badge.</div>');
    showStatus(root, "err", error.message || String(error));
  }
}
