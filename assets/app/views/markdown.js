// Small safe-subset markdown renderer for badge descriptions.
// Supports: paragraphs, line breaks, **bold**, `inline code`, safe links.
// Everything else is HTML-escaped and rendered as text.

const HTML_ESCAPE_MAP = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
};

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => HTML_ESCAPE_MAP[character]);
}

// Placeholder sentinel: \x00LINK<index>\x00 — NUL byte never appears in user
// input passed through our event pipeline, so it won't collide.
const LINK_PLACEHOLDER_PREFIX = "\x00LINK";
const LINK_PLACEHOLDER_SUFFIX = "\x00";
const LINK_PATTERN = /\[([^\]]*)\]\(([^)]*)\)/g;
const SAFE_URL_PATTERN = /^https?:\/\//i;

function extractLinks(source) {
  const links = [];
  const replaced = source.replace(LINK_PATTERN, (match, label, url) => {
    const index = links.length;
    links.push({ label, url, raw: match });
    return `${LINK_PLACEHOLDER_PREFIX}${index}${LINK_PLACEHOLDER_SUFFIX}`;
  });
  return { replaced, links };
}

function reinsertLinks(escapedSource, links) {
  if (links.length === 0) {
    return escapedSource;
  }
  const pattern = new RegExp(
    `${LINK_PLACEHOLDER_PREFIX}(\\d+)${LINK_PLACEHOLDER_SUFFIX}`,
    "g"
  );
  return escapedSource.replace(pattern, (_match, indexText) => {
    const index = Number.parseInt(indexText, 10);
    const link = links[index];
    if (!link) {
      return "";
    }
    const escapedLabel = escapeHtml(link.label);
    if (SAFE_URL_PATTERN.test(link.url)) {
      const escapedUrl = escapeHtml(link.url);
      return `<a href="${escapedUrl}" target="_blank" rel="noreferrer noopener">${escapedLabel}</a>`;
    }
    // Unsafe scheme (javascript:, data:, etc): render label as plain escaped
    // text, dropping the URL entirely to avoid leaking the scheme.
    return escapedLabel;
  });
}

function applyInlineFormatting(escapedText) {
  // Inline code: `x` → <code>x</code>. Content is already HTML-escaped.
  let result = escapedText.replace(/`([^`]+)`/g, (_match, inner) => `<code>${inner}</code>`);
  // Bold: **x** → <strong>x</strong>. Content is already HTML-escaped.
  result = result.replace(/\*\*([^*]+)\*\*/g, (_match, inner) => `<strong>${inner}</strong>`);
  return result;
}

function renderParagraph(paragraph) {
  // Single newlines become <br>. Paragraph string already has links
  // reinserted and inline formatting applied.
  return `<p>${paragraph.replace(/\n/g, "<br>")}</p>`;
}

export function renderSafeMarkdown(value) {
  if (value === null || value === undefined) {
    return "";
  }
  const source = String(value);
  if (source.length === 0) {
    return "";
  }

  // Step 1: pull all [label](url) matches out of the source so their
  // contents don't participate in further escaping / inline-formatting.
  const { replaced, links } = extractLinks(source);

  // Step 2: escape the remaining HTML-meaningful characters.
  const escaped = escapeHtml(replaced);

  // Step 3: apply inline formatting (**bold**, `code`).
  const formatted = applyInlineFormatting(escaped);

  // Step 4: re-insert links as anchors (safe) or escaped labels (unsafe).
  const withLinks = reinsertLinks(formatted, links);

  // Step 5: split on blank lines → paragraphs; convert remaining \n to <br>.
  const paragraphs = withLinks.split(/\n{2,}/).filter((paragraph) => paragraph.length > 0);
  if (paragraphs.length === 0) {
    return "";
  }
  return paragraphs.map(renderParagraph).join("");
}
