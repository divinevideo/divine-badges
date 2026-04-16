export function buildMeEmptyStateMarkup(tabName) {
  if (tabName === "accepted") {
    return '<div class="empty">No accepted badges yet. Accept badges from the Awarded tab when you want them on your profile.</div>';
  }

  if (tabName === "created") {
    return `
      <div class="empty">
        <div>No badges created from this account yet.</div>
        <a class="empty-cta" href="/new">Create your first badge</a>
      </div>
    `;
  }

  return `
    <div class="empty">
      <div>No badges awarded here yet. Keep looping - we check every UTC morning.</div>
      <a class="empty-cta" href="/new">Create a badge</a>
    </div>
  `;
}
