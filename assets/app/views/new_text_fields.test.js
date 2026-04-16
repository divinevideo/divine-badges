import test from "node:test";
import assert from "node:assert/strict";

import {
  applyDescriptionInput,
  applyIdentifierInput,
  applyNameInput,
  applyUploadError,
  wireTextFieldHandlers,
} from "./new_text_fields.js";

function createInputStub() {
  const listeners = new Map();
  return {
    addEventListener(type, handler) {
      listeners.set(type, handler);
    },
    dispatch(value) {
      const handler = listeners.get("input");
      if (!handler) {
        throw new Error("missing input handler");
      }
      handler({ target: { value } });
    },
  };
}

test("name input derives the identifier until the user edits it", () => {
  const state = {
    name: "",
    description: "",
    identifier: "",
    identifierTouched: false,
  };

  applyNameInput(state, "Scene Stealer");
  assert.equal(state.name, "Scene Stealer");
  assert.equal(state.identifier, "scene-stealer");

  applyIdentifierInput(state, "my-custom-slug");
  applyNameInput(state, "Loop Oracle");
  assert.equal(state.identifier, "my-custom-slug");
});

test("description input updates the draft in place", () => {
  const state = {
    name: "",
    description: "",
    identifier: "",
    identifierTouched: false,
  };

  applyDescriptionInput(state, "For one perfect loop.");

  assert.equal(state.description, "For one perfect loop.");
});

test("name input keeps syncing the untouched identifier", () => {
  const state = {
    name: "",
    description: "",
    identifier: "",
    identifierTouched: false,
  };
  const nameInput = createInputStub();
  const identifierInput = createInputStub();
  const descriptionInput = createInputStub();

  wireTextFieldHandlers({
    nameInput,
    identifierInput,
    descriptionInput,
    state,
    onStateChange: () => {},
  });

  nameInput.dispatch("Scene Stealer");

  assert.equal(state.identifier, "scene-stealer");
});

test("text field handlers notify a sync callback for each edit", () => {
  const state = {
    name: "",
    description: "",
    identifier: "",
    identifierTouched: false,
  };
  const nameInput = createInputStub();
  const identifierInput = createInputStub();
  const descriptionInput = createInputStub();
  let updates = 0;

  wireTextFieldHandlers({
    nameInput,
    identifierInput,
    descriptionInput,
    state,
    onStateChange: () => {
      updates += 1;
    },
  });

  nameInput.dispatch("Scene Stealer");
  identifierInput.dispatch("scene-stealer");
  descriptionInput.dispatch("For one perfect loop.");

  assert.equal(updates, 3);
  assert.equal(state.name, "Scene Stealer");
  assert.equal(state.identifier, "scene-stealer");
  assert.equal(state.description, "For one perfect loop.");
});

test("upload errors are stored for persistent rendering", () => {
  const state = {
    uploadError: "",
  };

  applyUploadError(state, "upload failed with 401");

  assert.equal(state.uploadError, "upload failed with 401");
});
