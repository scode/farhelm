// Independent WCAG contrast-floor coverage for app.css's shipped `:root`
// palette — the sibling to app-css-tokens.test.js, which pins the
// no-literals STRUCTURE of the token layer but says nothing about whether
// any given pair of tokens is actually legible together.
//
// The palette's own `:root` comments make specific, numeric promises: "Each
// [foreground] clears WCAG AA (4.5:1) against every surface it actually
// lands on", "--danger-strong ... [c]alibrated to clear WCAG AA (4.5:1) on
// every row surface", "--accent ... spent on selection ... and
// `:focus-visible`" (a non-text UI indicator, so its own floor is the 3:1
// WCAG sets for those rather than 4.5:1). Nothing else in this repo's test
// suite checks that those promises hold. `cargo fmt`/`clippy` and `dprint`
// have no notion of a contrast ratio; the Playwright suite reads rendered
// pixels and DOM state, and a pair sitting at 4.3:1 instead of 4.5:1 looks
// identical in a screenshot to anyone not running an actual contrast
// checker over it — which is exactly how the palette shipped with a
// `--danger-strong`/`--bg-2` pairing under the floor in the first place
// (fixed alongside this test). This file computes the ratio itself, from
// first principles, so a future retune that silently drops a promised
// pair below its floor fails here instead of shipping unnoticed again.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const APP_CSS_PATH = path.join(__dirname, "../assets/app.css");

const HEX_DECL_RE = /(--[a-zA-Z0-9-]+)\s*:\s*(#[0-9a-fA-F]{3,8})\s*[;)]/g;

/** Removes every `/* ... *\/` comment, so a token-looking string inside a
 * prose comment (the `:root` block is full of them) can never be mistaken
 * for a declaration. Mirrors app-css-tokens.test.js's own first pass. */
function stripComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/**
 * Every `--token: #hex` custom property declared in the FIRST `:root`
 * block of `css`. Comments are stripped before scanning. Non-hex
 * declarations (`rgba()`, `calc()`, font stacks, `--radius-*`) are
 * silently skipped — contrast math only ever needs the hex-valued color
 * tokens. A light theme's SECOND `:root` block (app.css's own header
 * describes that arrangement) is out of scope for this check, same
 * carve-out the sibling token-usage test makes.
 */
function extractRootHexTokens(css) {
  const stripped = stripComments(css);
  const start = stripped.indexOf(":root");
  if (start === -1) return new Map();
  const openBrace = stripped.indexOf("{", start);
  let depth = 1;
  let i = openBrace + 1;
  while (i < stripped.length && depth > 0) {
    if (stripped[i] === "{") depth++;
    else if (stripped[i] === "}") depth--;
    i++;
  }
  const body = stripped.slice(openBrace + 1, i - 1);
  const tokens = new Map();
  for (const match of body.matchAll(HEX_DECL_RE)) {
    tokens.set(match[1], match[2]);
  }
  return tokens;
}

/** `#rgb`/`#rrggbb`/`#rrggbbaa` -> always `#rrggbb`: drops a shorthand or an
 * alpha channel so `relativeLuminance` only ever has one format to slice.
 * (Nothing in this palette actually uses alpha on a color that lands in a
 * contrast pair — `--scrim` is the one alpha token and is never one of
 * REQUIRED_BINDINGS' foregrounds or backgrounds — but a synthetic test
 * case is free to try, so this normalizes rather than assuming.) */
function expandHex(hex) {
  if (hex.length === 4) {
    const [, r, g, b] = hex;
    return `#${r}${r}${g}${g}${b}${b}`;
  }
  return hex.slice(0, 7);
}

/** sRGB -> linear-light channel, the WCAG 2.x transform a contrast ratio is
 * computed from (relative luminance is a linear-light quantity; the 8-bit
 * channel values in a hex color are gamma-encoded and cannot be averaged
 * directly). */
function linearizeChannel(channel255) {
  const c = channel255 / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** WCAG relative luminance of a hex color: the weighted sum of its
 * linearized channels, per https://www.w3.org/TR/WCAG21/#dfn-relative-luminance. */
function relativeLuminance(hex) {
  const normalized = expandHex(hex);
  const r = parseInt(normalized.slice(1, 3), 16);
  const g = parseInt(normalized.slice(3, 5), 16);
  const b = parseInt(normalized.slice(5, 7), 16);
  return 0.2126 * linearizeChannel(r) + 0.7152 * linearizeChannel(g) + 0.0722 * linearizeChannel(b);
}

/** WCAG contrast ratio between two colors: `(L1 + 0.05) / (L2 + 0.05)`
 * with the LIGHTER luminance always in the numerator, per the spec's own
 * formula — order of arguments must not matter, and a synthetic test
 * below pins exactly that. */
function contrastRatio(hexA, hexB) {
  const la = relativeLuminance(hexA);
  const lb = relativeLuminance(hexB);
  const lighter = Math.max(la, lb);
  const darker = Math.min(la, lb);
  return (lighter + 0.05) / (darker + 0.05);
}

// The bindings the palette's `:root` comments promise, as
// [foreground token, background token, minimum ratio]. Text pairs use
// WCAG AA's 4.5:1. `--accent` on `--bg-1` is the one non-text entry — the
// selected row's inset bar and every `:focus-visible` ring are graphical
// indicators, not text, so WCAG's lower 3:1 floor for those applies
// instead of the text floor.
const ROW_SURFACES = ["--bg-1", "--bg-2", "--accent-fill", "--accent-fill-hover"];
const REQUIRED_BINDINGS = [
  ...["--bg-0", ...ROW_SURFACES].map((bg) => ["--fg-0", bg, 4.5]),
  ...["--bg-0", ...ROW_SURFACES].map((bg) => ["--fg-1", bg, 4.5]),
  ["--fg-2", "--bg-0", 4.5],
  ...ROW_SURFACES.map((bg) => ["--danger-strong", bg, 4.5]),
  ["--accent", "--bg-1", 3],
];

/**
 * Checks every `[fg, bg, min]` triple in `bindings` against the hex values
 * `css` declares in `:root`, returning `{ ok, errors, ratios }` — never
 * throws, so both the real-file regression and the synthetic negative
 * cases below can assert on the error list's shape rather than catching.
 * A binding naming a token `css` never declared is itself an error (not a
 * thrown exception), so a synthetic snippet can exercise that path
 * directly too.
 */
function checkContrastBindings(css, bindings) {
  const tokens = extractRootHexTokens(css);
  const errors = [];
  const ratios = [];
  for (const [fg, bg, min] of bindings) {
    const fgHex = tokens.get(fg);
    const bgHex = tokens.get(bg);
    if (!fgHex || !bgHex) {
      errors.push(`${fg} on ${bg}: one or both tokens not declared in :root`);
      continue;
    }
    const ratio = contrastRatio(fgHex, bgHex);
    ratios.push({ fg, bg, ratio });
    if (ratio < min) {
      errors.push(`${fg} (${fgHex}) on ${bg} (${bgHex}): ${ratio.toFixed(3)}:1 is below the ${min}:1 floor`);
    }
  }
  return { ok: errors.length === 0, errors, ratios };
}

// ---- Real-file regression: the invariant this file exists to protect.
// Everything below this point exercises the checker in isolation against
// synthetic snippets, which pin the checker's OWN behavior; this is the
// one test that pins app.css itself. ----

test("the shipped app.css palette clears every AA/non-text binding its comments promise", () => {
  const css = fs.readFileSync(APP_CSS_PATH, "utf8");
  const result = checkContrastBindings(css, REQUIRED_BINDINGS);
  assert.deepEqual(result.errors, []);
  assert.equal(result.ok, true);
});

// ---- Synthetic cases: each pins one piece of the checker's own behavior,
// so a change to the math or the parsing that quietly stops catching a
// real failure fails here even on a real app.css that has nothing wrong
// with it. ----

test("contrastRatio matches the WCAG worked example (black on white is 21:1)", () => {
  assert.ok(Math.abs(contrastRatio("#000000", "#ffffff") - 21) < 0.001);
});

test("contrastRatio does not depend on argument order", () => {
  assert.equal(contrastRatio("#9199a7", "#1a1d23"), contrastRatio("#1a1d23", "#9199a7"));
});

test("expandHex normalizes a #rgb shorthand to the same ratio as its #rrggbb equivalent", () => {
  assert.equal(contrastRatio("#fff", "#000"), contrastRatio("#ffffff", "#000000"));
});

test("a synthetic pair that clears its floor passes", () => {
  const css = `:root {\n  --fg-0: #ffffff;\n  --bg-0: #000000;\n}`;
  const result = checkContrastBindings(css, [["--fg-0", "--bg-0", 4.5]]);
  assert.deepEqual(result.errors, []);
});

test("a synthetic pair that crosses the AA floor fails", () => {
  // #777777 on #666666: close enough in lightness that eyeballing a
  // screenshot would not catch it, which is the whole reason this file
  // computes the ratio instead of trusting review.
  const css = `:root {\n  --fg-1: #777777;\n  --bg-1: #666666;\n}`;
  const result = checkContrastBindings(css, [["--fg-1", "--bg-1", 4.5]]);
  assert.equal(result.ok, false);
  assert.ok(result.errors.some((e) => e.includes("below the 4.5:1 floor")), result.errors.join("\n"));
});

test("a pair that clears the 3:1 non-text floor but not the 4.5:1 text floor fails only the text check", () => {
  // #787878 on #1a1a1a sits at ~3.94:1 — deliberately a synthetic pair
  // rather than a real token pairing, chosen to land in the gap between
  // the two floors WCAG actually distinguishes (3:1 for graphical/UI
  // indicators, 4.5:1 for text), which the real palette's own --accent
  // on --bg-1 binding (~5.58:1) clears with room to spare and so cannot
  // exercise on its own.
  const css = `:root {\n  --accent: #787878;\n  --bg-1: #1a1a1a;\n}`;
  const nonText = checkContrastBindings(css, [["--accent", "--bg-1", 3]]);
  assert.equal(nonText.ok, true);
  const text = checkContrastBindings(css, [["--accent", "--bg-1", 4.5]]);
  assert.equal(text.ok, false);
});

test("a binding naming a token the snippet never declares is reported, not thrown", () => {
  const css = `:root { --fg-0: #ffffff; }`;
  const result = checkContrastBindings(css, [["--fg-0", "--bg-does-not-exist", 4.5]]);
  assert.equal(result.ok, false);
  assert.ok(result.errors.some((e) => e.includes("not declared")), result.errors.join("\n"));
});

test("a hex color mentioned only inside a comment is not read as a declaration", () => {
  // Regression for the comment-stripping pass: the real :root block is
  // full of prose that mentions token names and colors together (this
  // very file, `#e47a72`'s own comment, is an example) — without
  // stripping first, a naive regex could misattribute a value that only
  // ever appears in explanatory text.
  const css = `:root {\n  /* --bg-0: #ff0000 in prose, not a declaration */\n  --bg-0: #000000;\n}`;
  const tokens = extractRootHexTokens(css);
  assert.equal(tokens.get("--bg-0"), "#000000");
});
