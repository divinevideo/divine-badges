export function mountEditBadgePage() {
  const root = document.getElementById("view");
  if (!root) return;
  root.innerHTML = '<p class="status info">Loading edit badge…</p>';
}
