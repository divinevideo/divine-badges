import { DIVINE_RELAY } from "/app/nostr/constants.js?v=2026-04-14-3";
import { relayPublish } from "/app/nostr/relay.js?v=2026-04-14-3";
import {
  beginDivineOAuth,
  bootstrapSession,
  clearStoredSession,
} from "/app/auth/session.js?v=2026-04-14-3";
import { loadDivineProfile } from "/app/auth/profile.js?v=2026-04-14-3";
import {
  buildBadgeDefinitionEvent,
  coordinateFromBadgeDefinition,
} from "/app/nostr/badges.js?v=2026-04-14-3";
import { esc, replaceView, showStatus } from "/app/views/common.js?v=2026-04-14-3";

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

function formMarkup() {
  return `
    <div class="card">
      <h2>Badge definition</h2>
      <div class="row">
        <label for="slug">Identifier</label>
        <input id="slug" placeholder="diviner-of-the-day">
      </div>
      <div class="row">
        <label for="name">Name</label>
        <input id="name" placeholder="Diviner of the Day">
      </div>
      <div class="row">
        <label for="description">Description</label>
        <textarea id="description" placeholder="Whoever yesterday's loops loved most."></textarea>
      </div>
      <div class="row">
        <label for="image">Image URL</label>
        <input id="image" placeholder="https://...">
      </div>
      <div class="row">
        <label for="thumb">Thumbnail URL</label>
        <input id="thumb" placeholder="https://...">
      </div>
      <div class="row">
        <button id="create-submit" type="button">Publish badge</button>
      </div>
    </div>
    <div class="card">
      <h2>Preview</h2>
      <div class="preview" id="badge-preview">
        <div class="name">Untitled badge</div>
        <p>Add a name and description to preview the badge metadata.</p>
      </div>
    </div>
  `;
}

function wirePreview() {
  const fields = ["slug", "name", "description", "image", "thumb"];
  const preview = document.getElementById("badge-preview");
  const update = () => {
    const name = document.getElementById("name").value.trim() || "Untitled badge";
    const description = document.getElementById("description").value.trim();
    const image = document.getElementById("image").value.trim() || document.getElementById("thumb").value.trim();
    preview.innerHTML = `
      <div class="name">${esc(name)}</div>
      ${description ? `<p>${esc(description)}</p>` : "<p>No description yet.</p>"}
      ${image ? `<p><a href="${esc(image)}" target="_blank" rel="noreferrer">preview image</a></p>` : ""}
      <p><code>${esc(document.getElementById("slug").value.trim() || "badge-id")}</code></p>
    `;
  };
  fields.forEach((id) => {
    document.getElementById(id).addEventListener("input", update);
  });
  update();
}

async function publishBadge() {
  if (!signer || !signerPubkey) {
    showStatus(getView(), "err", "Log in first to create a badge.");
    return;
  }
  const slug = document.getElementById("slug").value.trim();
  const name = document.getElementById("name").value.trim();
  const description = document.getElementById("description").value.trim();
  const image = document.getElementById("image").value.trim();
  const thumb = document.getElementById("thumb").value.trim();
  if (!slug || !name) {
    showStatus(getView(), "err", "Identifier and name are required.");
    return;
  }
  const event = buildBadgeDefinitionEvent({
    pubkey: signerPubkey,
    slug,
    name,
    description,
    image,
    thumb,
    createdAt: Math.floor(Date.now() / 1000),
  });
  const signed = await signer.signEvent(event);
  await relayPublish(DIVINE_RELAY, signed);
  window.location.href = `/b/${encodeURIComponent(coordinateFromBadgeDefinition(signed))}`;
}

export async function mountNewBadgePage() {
  const root = getView();
  replaceView(root, '<p class="status info">Checking for a signer…</p>');
  await restoreOptionalSession();
  replaceView(
    root,
    signer
      ? formMarkup()
      : `<div class="card"><h2>Log in to create a badge</h2><p>Use the top-right login button, then come back here. This page will restore your signer and let you publish a kind 30009 badge definition.</p></div>`
  );
  if (!signer) {
    return;
  }
  wirePreview();
  document.getElementById("create-submit").onclick = async () => {
    try {
      await publishBadge();
    } catch (error) {
      showStatus(root, "err", `Could not create badge: ${error.message || error}`);
    }
  };
}
