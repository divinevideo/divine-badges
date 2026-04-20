import {
  beginDivineOAuth,
  bootstrapSession,
  clearStoredSession,
} from "/app/auth/session.js?v=2026-04-14-3";
import { loadDivineProfile } from "/app/auth/profile.js?v=2026-04-14-3";
import { parseBadgeCoordinate } from "/app/nostr/identity.js?v=2026-04-14-3";
import {
  discoverReadRelays,
  relayQueryMany,
} from "/app/nostr/relay.js?v=2026-04-14-3";
import {
  BADGE_DEFINITION,
  DIVINE_RELAY,
} from "/app/nostr/constants.js?v=2026-04-20-1";
import {
  buildEditedBadgeDefinitionEvent,
  coordinatePathFromBadge,
  findTag,
} from "/app/nostr/badges.js?v=2026-04-20-1";
import {
  publishSignedToWriteRelays,
  publishSucceeded,
  readLocalRelays,
  summarizePublishResult,
} from "/app/nostr/publish.js?v=2026-04-20-1";
import { uploadToBlossom } from "/app/media/blossom.js?v=2026-04-16-1";
import {
  clearStatus,
  esc,
  replaceView,
  showStatus,
} from "/app/views/common.js?v=2026-04-14-4";

const BLOSSOM_ENDPOINT = "https://media.divine.video";

let signer = null;
let signerPubkey = null;
let navProfile = null;

const state = {
  coordinate: null,
  existingEvent: null,
  identifier: "",
  name: "",
  description: "",
  imageUrl: "",
  thumbUrl: "",
  imageSha256: null,
  thumbSha256: null,
  uploadingImage: false,
  uploadingThumb: false,
  publishing: false,
  uploadError: "",
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
    navProfile = null;
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
    navProfile = await loadDivineProfile(signerPubkey);
    renderLoggedInChip(navProfile);
    return restored;
  } catch {
    renderLoggedOutChip();
    return null;
  }
}

function routeCoordinate() {
  const match = window.location.pathname.match(/^\/b\/([^/]+)\/edit\/?$/);
  if (!match) {
    throw new Error("invalid edit route");
  }
  return parseBadgeCoordinate(decodeURIComponent(match[1]));
}

async function loadExistingDefinition(coordinate) {
  const relays = await discoverReadRelays({
    pubkeys: [coordinate.pubkey],
    seedRelays: [DIVINE_RELAY],
  });
  const events = await relayQueryMany(relays, [
    {
      kinds: [BADGE_DEFINITION],
      authors: [coordinate.pubkey],
      "#d": [coordinate.identifier],
    },
  ]);
  if (!events.length) {
    return null;
  }
  return events.reduce((latest, candidate) => {
    if (!latest) return candidate;
    return candidate.created_at > latest.created_at ? candidate : latest;
  }, null);
}

function formMarkup() {
  const publisherLine = navProfile
    ? `<div class="publisher-line">Editing as <strong>${esc(navProfile.displayName)}</strong>${navProfile.handle ? ` <span>${esc(navProfile.handle)}</span>` : ""}</div>`
    : "";
  const previewImage = state.imageUrl;
  const previewThumb = state.thumbUrl || state.imageUrl;
  return `
    <div class="studio-grid">
      <section class="studio-preview panel">
        <div class="panel-kicker">Live preview</div>
        <div class="badge-stage">
          <div class="badge-art ${previewImage ? "has-art" : ""}">
            ${
              previewImage
                ? `<img src="${esc(previewImage)}" alt="">`
                : '<div class="badge-art-placeholder">Upload artwork</div>'
            }
          </div>
          <div class="preview-copy">
            <div class="preview-name" id="preview-name">${esc(state.name || "Untitled badge")}</div>
            <div class="preview-id" id="preview-id">${esc(state.identifier)}</div>
            <p class="preview-description" id="preview-description">${esc(
              state.description ||
                "Add a short description so recipients know what this badge means."
            )}</p>
          </div>
        </div>
        <div class="thumb-preview-row">
          <div class="thumb-preview-label">Thumbnail</div>
          <div class="thumb-preview ${previewThumb ? "has-thumb" : ""}">
            ${
              previewThumb
                ? `<img src="${esc(previewThumb)}" alt="">`
                : "<span>Uses the primary image by default</span>"
            }
          </div>
        </div>
      </section>

      <section class="studio-form panel">
        <div class="panel-kicker">Edit badge</div>
        <div class="field-grid">
          <label class="field">
            <span>Badge name</span>
            <input id="name" value="${esc(state.name)}" placeholder="Scene Stealer">
          </label>
          <label class="field">
            <span>Identifier</span>
            <input id="identifier" value="${esc(state.identifier)}" readonly>
          </label>
          <label class="field field-full">
            <span>Description</span>
            <textarea id="description" placeholder="What does this badge celebrate?">${esc(
              state.description
            )}</textarea>
          </label>
          <label class="field field-full">
            <span>Primary image URL</span>
            <input id="image-url" value="${esc(state.imageUrl)}" placeholder="https://…">
          </label>
          <div class="upload-card">
            <span class="upload-label">Replace primary image</span>
            <input id="image-file" type="file" accept="image/*">
            <span class="upload-help">${
              state.uploadingImage
                ? "Uploading primary image to Blossom…"
                : state.imageSha256
                  ? `New image uploaded · ${esc(state.imageSha256.slice(0, 12))}…`
                  : "Upload a new image or edit the URL above."
            }</span>
          </div>
          <label class="field field-full">
            <span>Thumbnail URL (optional)</span>
            <input id="thumb-url" value="${esc(state.thumbUrl)}" placeholder="https://…">
          </label>
          <div class="upload-card">
            <span class="upload-label">Replace thumbnail</span>
            <input id="thumb-file" type="file" accept="image/*">
            <span class="upload-help">${
              state.uploadingThumb
                ? "Uploading thumbnail to Blossom…"
                : state.thumbSha256
                  ? `New thumbnail uploaded · ${esc(state.thumbSha256.slice(0, 12))}…`
                  : "Leave blank to reuse the primary image."
            }</span>
          </div>
        </div>

        <div class="publish-bar">
          <div>
            <div class="publish-title">Save changes</div>
            <div class="publish-copy">Publishes a new kind 30009 definition with the same identifier, replacing the previous one.</div>
            ${publisherLine}
          </div>
          <button id="save-badge" type="button" ${state.publishing ? "disabled" : ""}>
            ${state.publishing ? "Publishing…" : "Save badge"}
          </button>
        </div>
      </section>
    </div>
    ${state.uploadError ? `<p class="status err">${esc(state.uploadError)}</p>` : ""}
  `;
}

function renderLoggedOutState() {
  replaceView(
    getView(),
    `
      <section class="panel logged-out-panel">
        <div class="panel-kicker">Signer required</div>
        <h2>Log in to edit badges.</h2>
        <p>This page updates your own kind 30009 badge definition. Use the top-right button to log in, then reload.</p>
      </section>
    `
  );
}

function renderUnauthorizedState() {
  replaceView(
    getView(),
    `
      <section class="panel logged-out-panel">
        <div class="panel-kicker">Not the author</div>
        <h2>Only the badge author can edit this badge.</h2>
        <p>If you issued this badge under a different pubkey, switch signers and try again.</p>
      </section>
    `
  );
}

function renderNotFoundState() {
  replaceView(
    getView(),
    `
      <section class="panel logged-out-panel">
        <div class="panel-kicker">Badge missing</div>
        <h2>Could not find the badge to edit.</h2>
        <p>No kind 30009 event was returned for this coordinate on the author's read relays.</p>
      </section>
    `
  );
}

function renderStudio() {
  replaceView(getView(), formMarkup());
  wireStudioEvents();
}

function syncPreviewText() {
  const previewName = document.getElementById("preview-name");
  const previewId = document.getElementById("preview-id");
  const previewDescription = document.getElementById("preview-description");

  if (previewName) {
    previewName.textContent = state.name || "Untitled badge";
  }
  if (previewId) {
    previewId.textContent = state.identifier;
  }
  if (previewDescription) {
    previewDescription.textContent =
      state.description ||
      "Add a short description so recipients know what this badge means.";
  }
}

function wireTextFields() {
  const nameInput = document.getElementById("name");
  const descriptionInput = document.getElementById("description");
  const imageUrlInput = document.getElementById("image-url");
  const thumbUrlInput = document.getElementById("thumb-url");

  if (nameInput) {
    nameInput.addEventListener("input", (event) => {
      state.name = event.target.value;
      syncPreviewText();
    });
  }
  if (descriptionInput) {
    descriptionInput.addEventListener("input", (event) => {
      state.description = event.target.value;
      syncPreviewText();
    });
  }
  if (imageUrlInput) {
    imageUrlInput.addEventListener("input", (event) => {
      state.imageUrl = event.target.value;
    });
  }
  if (thumbUrlInput) {
    thumbUrlInput.addEventListener("input", (event) => {
      state.thumbUrl = event.target.value;
    });
  }
}

async function handleFileUpload(kind, file) {
  if (!file) return;
  state.uploadError = "";
  clearStatus();
  if (kind === "image") {
    state.uploadingImage = true;
  } else {
    state.uploadingThumb = true;
  }
  renderStudio();
  try {
    const uploaded = await uploadToBlossom({
      file,
      signer,
      pubkey: signerPubkey,
      endpoint: BLOSSOM_ENDPOINT,
    });
    if (kind === "image") {
      state.imageUrl = uploaded.url;
      state.imageSha256 = uploaded.sha256;
    } else {
      state.thumbUrl = uploaded.url;
      state.thumbSha256 = uploaded.sha256;
    }
  } catch (error) {
    state.uploadError = `Upload failed: ${error.message || error}`;
  } finally {
    if (kind === "image") {
      state.uploadingImage = false;
    } else {
      state.uploadingThumb = false;
    }
    renderStudio();
  }
}

function wireUploads() {
  const imageFile = document.getElementById("image-file");
  if (imageFile) {
    imageFile.addEventListener("change", async (event) => {
      await handleFileUpload("image", event.target.files?.[0] || null);
    });
  }
  const thumbFile = document.getElementById("thumb-file");
  if (thumbFile) {
    thumbFile.addEventListener("change", async (event) => {
      await handleFileUpload("thumb", event.target.files?.[0] || null);
    });
  }
}

async function saveBadge() {
  if (!signer || !signerPubkey) {
    showStatus(getView(), "err", "Log in first to edit this badge.");
    return;
  }
  if (!state.existingEvent) {
    showStatus(getView(), "err", "Cannot edit a badge that has not loaded.");
    return;
  }
  const name = (state.name || "").trim();
  const description = (state.description || "").trim();
  const imageUrl = (state.imageUrl || "").trim();
  const thumbUrl = (state.thumbUrl || "").trim();
  if (!name) {
    showStatus(getView(), "err", "Badge name is required.");
    return;
  }
  if (!imageUrl) {
    showStatus(getView(), "err", "A primary image URL is required.");
    return;
  }

  state.publishing = true;
  renderStudio();
  try {
    const event = buildEditedBadgeDefinitionEvent({
      existingEvent: state.existingEvent,
      pubkey: signerPubkey,
      name,
      description,
      imageUrl,
      thumbUrl: thumbUrl || imageUrl,
      createdAt: Math.floor(Date.now() / 1000),
    });
    const outcome = await publishSignedToWriteRelays({
      pubkey: signerPubkey,
      unsignedEvent: event,
      signer,
      localRelays: readLocalRelays(),
    });
    if (!publishSucceeded(outcome)) {
      state.publishing = false;
      renderStudio();
      showStatus(
        getView(),
        "err",
        `Could not save badge: ${summarizePublishResult(outcome)}`
      );
      return;
    }
    if (outcome.result.failed.length > 0) {
      console.warn(`Badge edit publish partial: ${summarizePublishResult(outcome)}`);
    }
    window.location.href = coordinatePathFromBadge(outcome.signed);
  } catch (error) {
    state.publishing = false;
    renderStudio();
    showStatus(getView(), "err", `Could not save badge: ${error.message || error}`);
  }
}

function wireStudioEvents() {
  wireTextFields();
  wireUploads();
  const saveButton = document.getElementById("save-badge");
  if (saveButton) {
    saveButton.onclick = async () => {
      await saveBadge();
    };
  }
  if (state.uploadingImage) {
    showStatus(getView(), "info", "Uploading primary image to Blossom…");
  } else if (state.uploadingThumb) {
    showStatus(getView(), "info", "Uploading custom thumbnail to Blossom…");
  }
}

function prefillStateFromEvent(existingEvent) {
  state.existingEvent = existingEvent;
  state.identifier = findTag(existingEvent.tags || [], "d") || "";
  state.name = findTag(existingEvent.tags || [], "name") || "";
  state.description = findTag(existingEvent.tags || [], "description") || "";
  state.imageUrl = findTag(existingEvent.tags || [], "image") || "";
  state.thumbUrl = findTag(existingEvent.tags || [], "thumb") || "";
}

export async function mountEditBadgePage() {
  const root = getView();
  replaceView(root, '<p class="status info">Loading badge…</p>');
  await restoreOptionalSession();
  if (!signer || !signerPubkey) {
    renderLoggedOutState();
    return;
  }
  let coordinate;
  try {
    coordinate = routeCoordinate();
  } catch (error) {
    replaceView(root, '<div class="empty">Invalid badge URL.</div>');
    showStatus(root, "err", error.message || String(error));
    return;
  }
  state.coordinate = coordinate;
  if (signerPubkey !== coordinate.pubkey) {
    renderUnauthorizedState();
    return;
  }
  let existing;
  try {
    existing = await loadExistingDefinition(coordinate);
  } catch (error) {
    replaceView(root, '<div class="empty">Could not load this badge.</div>');
    showStatus(root, "err", error.message || String(error));
    return;
  }
  if (!existing) {
    renderNotFoundState();
    return;
  }
  prefillStateFromEvent(existing);
  renderStudio();
}
