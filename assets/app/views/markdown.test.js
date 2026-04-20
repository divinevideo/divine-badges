import test from "node:test";
import assert from "node:assert/strict";

import { renderSafeMarkdown } from "./markdown.js";

test("returns empty string for null or undefined", () => {
  assert.equal(renderSafeMarkdown(null), "");
  assert.equal(renderSafeMarkdown(undefined), "");
});

test("escapes raw HTML, stripping script tags", () => {
  const html = renderSafeMarkdown("hello <script>alert('x')</script>");
  assert.ok(!html.includes("<script>"), "must not contain raw <script>");
  assert.ok(!html.includes("</script>"), "must not contain raw </script>");
  assert.ok(html.includes("&lt;script&gt;"), "must contain escaped <script>");
  assert.ok(html.includes("&lt;/script&gt;"), "must contain escaped </script>");
});

test("splits double newlines into separate <p> blocks", () => {
  const html = renderSafeMarkdown("one\n\ntwo");
  assert.match(html, /<p>one<\/p>/);
  assert.match(html, /<p>two<\/p>/);
});

test("wraps inline content in a single <p>", () => {
  const html = renderSafeMarkdown("just a line");
  assert.equal(html, "<p>just a line</p>");
});

test("converts single newlines inside a paragraph into <br>", () => {
  const html = renderSafeMarkdown("line one\nline two");
  assert.match(html, /line one<br>line two/);
});

test("converts `inline code` to <code> and escapes contents", () => {
  const html = renderSafeMarkdown("use `a<b>` here");
  assert.match(html, /<code>a&lt;b&gt;<\/code>/);
});

test("converts **bold** to <strong> and escapes contents", () => {
  const html = renderSafeMarkdown("**bo<ld**");
  assert.match(html, /<strong>bo&lt;ld<\/strong>/);
});

test("renders https links with safe anchor attributes", () => {
  const html = renderSafeMarkdown("[hi](https://example.com)");
  assert.match(
    html,
    /<a href="https:\/\/example\.com" target="_blank" rel="noreferrer noopener">hi<\/a>/
  );
});

test("renders http links with safe anchor attributes", () => {
  const html = renderSafeMarkdown("[hi](http://example.com)");
  assert.match(
    html,
    /<a href="http:\/\/example\.com" target="_blank" rel="noreferrer noopener">hi<\/a>/
  );
});

test("escapes anchor label text", () => {
  const html = renderSafeMarkdown("[<b>bad</b>](https://example.com)");
  assert.ok(!html.includes("<b>bad</b>"), "label must be escaped");
  assert.match(html, /&lt;b&gt;bad&lt;\/b&gt;/);
  assert.match(html, /href="https:\/\/example\.com"/);
});

test("external links always include target=_blank and rel=noreferrer noopener", () => {
  const html = renderSafeMarkdown("[docs](https://example.com/docs)");
  assert.match(html, /target="_blank"/);
  assert.match(html, /rel="noreferrer noopener"/);
});

test("rejects javascript: links — renders label as plain text, no anchor", () => {
  const html = renderSafeMarkdown("[click](javascript:alert(1))");
  assert.ok(!html.includes("<a "), "must not emit anchor tag");
  assert.ok(!html.toLowerCase().includes("javascript:"), "must not contain javascript: scheme");
  assert.match(html, /click/);
});

test("rejects data: URIs — renders label as plain text, no anchor", () => {
  const html = renderSafeMarkdown("[sneak](data:text/html,<script>1</script>)");
  assert.ok(!html.includes("<a "), "must not emit anchor tag");
  assert.ok(!html.toLowerCase().includes("data:"), "must not contain data: scheme");
  assert.match(html, /sneak/);
});
