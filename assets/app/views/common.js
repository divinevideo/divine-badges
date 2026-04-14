export const esc = (value) =>
  String(value).replace(
    /[&<>"']/g,
    (character) =>
      ({
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&#39;",
      })[character]
  );

export const shorten = (hex) =>
  hex ? `${hex.slice(0, 8)}…${hex.slice(-6)}` : "";

export function replaceView(root, html) {
  root.innerHTML = html;
}

export function showStatus(root, kind, message) {
  let element = document.getElementById("status");
  if (!element) {
    element = document.createElement("p");
    element.id = "status";
    root.appendChild(element);
  }
  element.className = `status ${kind}`;
  element.textContent = message;
  element.style.display = "block";
  return element;
}

export function clearStatus() {
  const element = document.getElementById("status");
  if (element) {
    element.remove();
  }
}

export function renderEmptyState(root, message) {
  root.innerHTML = `<div class="empty">${esc(message)}</div>`;
}

export function replaceWithEmptyState(element, message) {
  element.outerHTML = `<div class="empty">${esc(message)}</div>`;
}
