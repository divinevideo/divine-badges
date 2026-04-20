import {
  beginDivineOAuth,
  bootstrapSession,
  clearStoredSession,
} from "/app/auth/session.js?v=2026-04-14-3";
import { loadDivineProfile } from "/app/auth/profile.js?v=2026-04-14-3";
import {
  buildBadgeDefinitionEvent,
  buildNewBadgePreviewModel,
  coordinatePathFromBadge,
} from "/app/nostr/badges.js?v=2026-04-20-1";
import {
  publishSignedToWriteRelays,
  publishSucceeded,
  summarizePublishResult,
} from "/app/nostr/publish.js?v=2026-04-20-1";
import { uploadToBlossom } from "/app/media/blossom.js?v=2026-04-16-1";
import { clearStatus, esc, replaceView, showStatus } from "/app/views/common.js?v=2026-04-14-3";
import {
  applyUploadError,
  wireTextFieldHandlers,
} from "/app/views/new_text_fields.js?v=2026-04-16-3";

const BLOSSOM_ENDPOINT = "https://media.divine.video";

let signer = null;
let signerPubkey = null;
let navProfile = null;

const state = {
  name: "",
  description: "",
  identifier: "",
  identifierTouched: false,
  imageUrl: null,
  thumbUrl: null,
  imageSha256: null,
  thumbSha256: null,
  customThumbEnabled: false,
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

function formMarkup() {
  const preview = buildNewBadgePreviewModel({
    name: state.name.trim(),
    description: state.description.trim(),
    identifier: state.identifier.trim(),
    imageUrl: state.imageUrl,
    thumbUrl: state.thumbUrl,
  });
  const publisherLine = navProfile
    ? `<div class="publisher-line">Publishing as <strong>${esc(navProfile.displayName)}</strong>${navProfile.handle ? ` <span>${esc(navProfile.handle)}</span>` : ""}</div>`
    : "";

  return `
    <div class="studio-grid">
      <section class="studio-preview panel">
        <div class="panel-kicker">Live preview</div>
          <div class="badge-stage">
          <div class="badge-art ${preview.imageUrl ? "has-art" : ""}">
            ${
              preview.imageUrl
                ? `<img src="${esc(preview.imageUrl)}" alt="">`
                : '<div class="badge-art-placeholder">Upload artwork</div>'
            }
          </div>
          <div class="preview-copy">
            <div class="preview-name" id="preview-name">${esc(preview.name)}</div>
            <div class="preview-id" id="preview-id">${esc(preview.identifier || "badge-id")}</div>
            <p class="preview-description" id="preview-description">${esc(
              preview.description || "Add a short description so recipients know what this badge means."
            )}</p>
          </div>
        </div>
        <div class="thumb-preview-row">
          <div class="thumb-preview-label">Thumbnail</div>
          <div class="thumb-preview ${preview.thumbUrl ? "has-thumb" : ""}">
            ${
              preview.thumbUrl
                ? `<img src="${esc(preview.thumbUrl)}" alt="">`
                : '<span>Uses the primary image by default</span>'
            }
          </div>
        </div>
      </section>

      <section class="studio-form panel">
        <div class="panel-kicker">Badge definition</div>
        <div class="field-grid">
          <label class="field">
            <span>Badge name</span>
            <input id="name" value="${esc(state.name)}" placeholder="Scene Stealer">
          </label>
          <label class="field">
            <span>Identifier</span>
            <input id="identifier" value="${esc(state.identifier)}" placeholder="scene-stealer">
          </label>
          <label class="field field-full">
            <span>Description</span>
            <textarea id="description" placeholder="For the creator who steals the scroll with one perfect loop.">${esc(
              state.description
            )}</textarea>
          </label>
        </div>

        <div class="upload-grid">
          <label class="upload-card">
            <span class="upload-label">Primary image</span>
            <input id="image-file" type="file" accept="image/*">
            <span class="upload-help">${
              state.imageUrl
                ? `Uploaded to Blossom${state.imageSha256 ? ` · ${esc(state.imageSha256.slice(0, 12))}…` : ""}`
                : "Used for both image and thumb unless you override the thumbnail."
            }</span>
          </label>

          <div class="upload-card">
            <label class="toggle-row">
              <input id="custom-thumb-toggle" type="checkbox" ${state.customThumbEnabled ? "checked" : ""}>
              <span>Use custom thumbnail</span>
            </label>
            ${
              state.customThumbEnabled
                ? `
                  <label class="thumb-upload">
                    <input id="thumb-file" type="file" accept="image/*">
                    <span class="upload-help">${
                      state.thumbUrl
                        ? `Custom thumb uploaded${state.thumbSha256 ? ` · ${esc(state.thumbSha256.slice(0, 12))}…` : ""}`
                        : "Upload a separate cropped image for list and card views."
                    }</span>
                  </label>
                `
                : '<span class="upload-help">Primary image will be reused automatically.</span>'
            }
          </div>
        </div>

        <div class="publish-bar">
          <div>
            <div class="publish-title">Publish badge</div>
            <div class="publish-copy">Create the kind 30009 definition, then jump straight into awarding on the badge page.</div>
            ${publisherLine}
          </div>
          <button id="publish-badge" type="button" ${state.publishing ? "disabled" : ""}>
            ${state.publishing ? "Publishing…" : "Publish badge"}
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
        <h2>Log in to create a badge.</h2>
        <p>This studio publishes a signed kind 30009 badge definition under your own pubkey. Use the top-right button, then come back here.</p>
      </section>
    `
  );
}

function renderStudio() {
  replaceView(getView(), formMarkup());
  wireStudioEvents();
}

function syncPreviewText() {
  const preview = buildNewBadgePreviewModel({
    name: state.name.trim(),
    description: state.description.trim(),
    identifier: state.identifier.trim(),
    imageUrl: state.imageUrl,
    thumbUrl: state.thumbUrl,
  });
  const previewName = document.getElementById("preview-name");
  const previewId = document.getElementById("preview-id");
  const previewDescription = document.getElementById("preview-description");
  const identifierInput = document.getElementById("identifier");

  if (previewName) {
    previewName.textContent = preview.name;
  }
  if (previewId) {
    previewId.textContent = preview.identifier || "badge-id";
  }
  if (previewDescription) {
    previewDescription.textContent =
      preview.description || "Add a short description so recipients know what this badge means.";
  }
  if (identifierInput instanceof HTMLInputElement && identifierInput.value !== state.identifier) {
    identifierInput.value = state.identifier;
  }
}

function wireTextFields() {
  wireTextFieldHandlers({
    nameInput: document.getElementById("name"),
    identifierInput: document.getElementById("identifier"),
    descriptionInput: document.getElementById("description"),
    state,
    onStateChange: syncPreviewText,
  });
}

async function handleFileUpload(kind, file) {
  if (!file) {
    return;
  }
  applyUploadError(state, "");
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
      if (!state.customThumbEnabled || !state.thumbUrl) {
        state.thumbUrl = null;
        state.thumbSha256 = null;
      }
    } else {
      state.thumbUrl = uploaded.url;
      state.thumbSha256 = uploaded.sha256;
    }
  } catch (error) {
    applyUploadError(state, `Upload failed: ${error.message || error}`);
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
  document.getElementById("image-file").addEventListener("change", async (event) => {
    await handleFileUpload("image", event.target.files?.[0] || null);
  });
  document.getElementById("custom-thumb-toggle").addEventListener("change", (event) => {
    state.customThumbEnabled = event.target.checked;
    if (!state.customThumbEnabled) {
      state.thumbUrl = null;
      state.thumbSha256 = null;
    }
    renderStudio();
  });
  const thumbInput = document.getElementById("thumb-file");
  if (thumbInput) {
    thumbInput.addEventListener("change", async (event) => {
      await handleFileUpload("thumb", event.target.files?.[0] || null);
    });
  }
}

async function publishBadge() {
  if (!signer || !signerPubkey) {
    showStatus(getView(), "err", "Log in first to create a badge.");
    return;
  }

  const identifier = state.identifier.trim();
  const name = state.name.trim();
  const description = state.description.trim();
  const imageUrl = state.imageUrl;
  const thumbUrl = state.customThumbEnabled ? state.thumbUrl : state.imageUrl;

  if (!name || !identifier) {
    showStatus(getView(), "err", "Badge name and identifier are required.");
    return;
  }
  if (!imageUrl) {
    showStatus(getView(), "err", "Upload a primary image before publishing.");
    return;
  }
  if (state.customThumbEnabled && !thumbUrl) {
    showStatus(getView(), "err", "Upload a custom thumbnail or turn that option off.");
    return;
  }

  state.publishing = true;
  renderStudio();
  try {
    const event = buildBadgeDefinitionEvent({
      pubkey: signerPubkey,
      identifier,
      name,
      description,
      imageUrl,
      thumbUrl,
      createdAt: Math.floor(Date.now() / 1000),
    });
    const outcome = await publishSignedToWriteRelays({
      pubkey: signerPubkey,
      unsignedEvent: event,
      signer,
    });
    if (!publishSucceeded(outcome)) {
      state.publishing = false;
      renderStudio();
      showStatus(
        getView(),
        "err",
        `Could not create badge: ${summarizePublishResult(outcome)}`
      );
      return;
    }
    if (outcome.result.failed.length > 0) {
      console.warn(`Badge definition publish partial: ${summarizePublishResult(outcome)}`);
    }
    window.location.href = `${coordinatePathFromBadge(outcome.signed)}?award=1`;
  } catch (error) {
    state.publishing = false;
    renderStudio();
    showStatus(getView(), "err", `Could not create badge: ${error.message || error}`);
  }
}

function wireStudioEvents() {
  wireTextFields();
  wireUploads();
  document.getElementById("publish-badge").onclick = async () => {
    await publishBadge();
  };
  if (state.uploadingImage) {
    showStatus(getView(), "info", "Uploading primary image to Blossom…");
  } else if (state.uploadingThumb) {
    showStatus(getView(), "info", "Uploading custom thumbnail to Blossom…");
  }
}

export async function mountNewBadgePage() {
  const root = getView();
  replaceView(root, '<p class="status info">Checking for a signer…</p>');
  await restoreOptionalSession();
  if (!signer) {
    renderLoggedOutState();
    return;
  }
  renderStudio();
}
