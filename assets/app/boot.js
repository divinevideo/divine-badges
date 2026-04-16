export function mountPlaceholder(name, root = document.getElementById("view")) {
  if (!root) {
    throw new Error(`missing mount root for ${name}`);
  }
  root.innerHTML = `<p>Loading ${name}...</p>`;
}
