import { deriveBadgeSlug } from "../nostr/badges.js";

export function applyNameInput(state, value) {
  state.name = value;
  if (!state.identifierTouched) {
    state.identifier = deriveBadgeSlug(value);
  }
}

export function applyIdentifierInput(state, value) {
  state.identifierTouched = true;
  state.identifier = value;
}

export function applyDescriptionInput(state, value) {
  state.description = value;
}

export function applyUploadError(state, value) {
  state.uploadError = value;
}

export function wireTextFieldHandlers({
  nameInput,
  identifierInput,
  descriptionInput,
  state,
  onStateChange,
}) {
  nameInput.addEventListener("input", (event) => {
    applyNameInput(state, event.target.value);
    onStateChange();
  });

  identifierInput.addEventListener("input", (event) => {
    applyIdentifierInput(state, event.target.value);
    onStateChange();
  });

  descriptionInput.addEventListener("input", (event) => {
    applyDescriptionInput(state, event.target.value);
    onStateChange();
  });
}
