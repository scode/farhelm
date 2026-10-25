// Independent WCAG contrast-floor coverage for app.css's shipped `:root`
// palette — the sibling to app-css-tokens.test.js, which pins the
// no-literals STRUCTURE of the token layer but says nothing about whether
// any given pair of tokens is actually legible together.
//
// Ordinary text, host identity and status colors belong on neutral surfaces.
// Solid selection instead needs inverse text, marks and keyboard focus; the
// selected running pulse must remain visible even at its minimum opacity.
// Bind both the numeric pairs and their actual shipped declarations so an
// unused accessible token cannot hide an unreadable use site.
// `cargo fmt`/`clippy` and `dprint` have no notion of a
// contrast ratio; the Playwright suite reads rendered pixels and DOM state,
// and a pair sitting at 4.3:1 instead of 4.5:1 looks identical in a
// screenshot to anyone not running an actual contrast checker over it —
// which is exactly how the palette shipped with a `--danger-strong`/`--bg-2`
// pairing under the floor in the first place (fixed alongside this test).
// This file computes the ratio itself, from first principles, so a future
// retune that silently drops a promised pair below its floor fails here
// instead of shipping unnoticed again.
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

// Text uses AA's 4.5:1 floor; focus and status shapes use 3:1. The ordinary
// quiet-text binding is the brightest neutral fill, not the selected fill:
// Steel deliberately uses a separate inverse vocabulary for selected objects.
const NEUTRAL_SURFACES = ["--bg-0", "--bg-1", "--bg-2", "--control-hover-bg", "--chip-bg", "--well"];
const SELECTED_SURFACES = ["--accent-fill", "--accent-fill-hover"];
const REQUIRED_BINDINGS = [
  ["--link", "--bg-2", 4.5],
  ["--warn", "--warn-bg", 4.5],
  ...["lavender", "orchid", "teal", "steel", "sand", "copper", "red"].flatMap((name) =>
    NEUTRAL_SURFACES.map((bg) => [`--host-icon-${name}`, bg, 4.5])
  ),
  ...["--fg-0", "--fg-1", "--fg-2", "--danger-strong", "--accent"].flatMap((fg) =>
    NEUTRAL_SURFACES.map((bg) => [fg, bg, 4.5])
  ),
  ...["--ok", "--danger", "--info"].flatMap((fg) => NEUTRAL_SURFACES.map((bg) => [fg, bg, 3])),
  ...NEUTRAL_SURFACES.map((bg) => ["--accent", bg, 3]),
  ...SELECTED_SURFACES.flatMap((bg) => [["--on-fill", bg, 4.5], ["--on-accent-dim", bg, 4.5]]),
  ["--on-fill", "--danger-bg", 4.5],
  ["--on-fill", "--danger-fill-hover", 4.5],
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

/** Collect declarations for an exact selector in this sheet's flat rules.
 * Grouped selectors count separately, and later declarations win. This is
 * deliberately not a cascade engine: browser tests own specificity and DOM
 * inheritance, while this check binds the named roles to shipped use sites. */
function declarationsForSelector(css, selector) {
  const declarations = new Map();
  for (const match of stripComments(css).matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (!match[1].split(",").some((candidate) => candidate.trim() === selector)) continue;
    for (const declaration of match[2].split(";")) {
      const colon = declaration.indexOf(":");
      if (colon < 0) continue;
      declarations.set(declaration.slice(0, colon).trim(), declaration.slice(colon + 1).trim());
    }
  }
  return declarations;
}

// These are rendered selection owners, rather than a list of new tokens.
// Pinning their fills and inverse roles catches an accessible but unused
// token, a forgotten metadata override and a resurrected accent edge stripe.
const SELECTED_OWNERS = [
  ".session-row.selected",
  ".tab.selected",
  ".templates-row[aria-pressed=\"true\"]",
  ".templates-segments button[aria-pressed=\"true\"]",
  ".app-settings-dialog[aria-label=\"settings\"] .app-settings-navigation button[aria-pressed=\"true\"]",
  ".launch-kind-tab.selected",
  ".launch-composer-folder-options button[aria-selected=\"true\"]",
  ".launch-composer-repository-options > button[aria-selected=\"true\"]",
  ".launch-composer-options > button.selected",
  ".launch-composer-segmented > button.selected",
  ".launch-composer-listbox > button.selected",
  ".launch-composer-search-group > button.selected",
  ".quick-switcher-row[aria-selected=\"true\"]",
  ".launcher-host-option[aria-selected=\"true\"]",
  ".host-icon-choice[aria-pressed=\"true\"]",
];
const SELECTED_PRIMARY = [
  ".session-row.selected .session-title",
  ".session-row.selected .session-repository",
  ".session-row.selected .sidebar-glyph",
  ".session-row.selected .status-badge",
  ".session-row.selected .status-badge .status-dot",
  ".session-row.selected .delete-progress",
  ".session-row.selected .action-error",
  ".session-row.selected .session-row-menu",
  ".session-row.selected .session-row-bell",
  ".session-row.selected .inline-link",
  ".session-row.selected a:any-link",
  ".quick-switcher-row[aria-selected=\"true\"] .status-badge",
  ".quick-switcher-row[aria-selected=\"true\"] .status-dot",
  ".quick-switcher-row[aria-selected=\"true\"] mark",
  ".launch-composer-listbox > button.selected:last-child",
  ".launch-composer-search-group > button.selected",
];
const SELECTED_SECONDARY = [
  ".session-row.stale.selected .session-title",
  ".session-row.selected .session-pin",
  ".session-row.selected .session-host",
  ".session-row.selected .session-host-separator",
  ".session-row.selected .session-cwd",
  ".session-row.selected .session-invocation",
  ".session-row.selected .status-time",
  ".session-row.selected .stale-badge",
  ".session-row.selected .session-detail-copy",
  ".session-row.selected .managed-checkout-mark",
  ".session-row.selected .compact-qualifier",
  ".quick-switcher-row[aria-selected=\"true\"] .quick-switcher-agent",
  ".quick-switcher-row[aria-selected=\"true\"] .quick-switcher-host",
  ".quick-switcher-row[aria-selected=\"true\"] .quick-switcher-cwd",
  ".quick-switcher-row[aria-selected=\"true\"] .quick-switcher-template-summary",
  ".quick-switcher-row[aria-selected=\"true\"] .quick-switcher-template-keys kbd",
  ".templates-row[aria-pressed=\"true\"] .templates-row-summary",
  ".launch-composer-folder-options button[aria-selected=\"true\"] .launch-composer-folder-meaning",
  ".launch-composer-search-group > button.selected .launch-composer-search-recent-selection",
];
const SELECTED_HOST_MARKS = [
  ".session-row.selected .host-kind-icon",
  ".quick-switcher-row[aria-selected=\"true\"] .host-kind-icon",
  ".launcher-host-option[aria-selected=\"true\"] .host-kind-icon",
  ".host-icon-choice[aria-pressed=\"true\"] .host-kind-icon",
];
const HIGHLIGHTED_COMMANDS = ["session-row-menu-item", "host-row-menu-item"].flatMap((name) =>
  ["hover", "focus"].map((state) => `.${name}:${state}:not([aria-disabled="true"])`)
);
const SHIPPED_ROLE_BINDINGS = [
  ...SELECTED_OWNERS.map((selector) => [selector, "background", "var(--accent-fill)"]),
  ...SELECTED_OWNERS.filter((selector) =>
    ![".session-row.selected", ".host-icon-choice[aria-pressed=\"true\"]"].includes(selector)
  )
    .map((selector) => [selector, "color", "var(--on-fill)"]),
  ...SELECTED_PRIMARY.map((selector) => [selector, "color", "var(--on-fill)"]),
  ...SELECTED_SECONDARY.map((selector) => [selector, "color", "var(--on-accent-dim)"]),
  ...SELECTED_HOST_MARKS.map((selector) => [selector, "color", "var(--on-fill) !important"]),
  ...HIGHLIGHTED_COMMANDS.flatMap((selector) => [
    [selector, "background", "var(--accent-fill)"],
    [selector, "color", "var(--on-fill)"],
    [`${selector} .session-row-menu-icon`, "color", "var(--on-fill)"],
    [`${selector} .session-row-menu-description`, "color", "var(--on-accent-dim)"],
    [
      selector.replace(":hover:", ":focus-visible:").replace(":focus:", ":focus-visible:"),
      "outline-color",
      "var(--on-fill)",
    ],
  ]),
  [".session-row.selected :focus-visible", "outline-color", "var(--on-fill)"],
  [".session-row.selected .session-row-open:disabled", "opacity", "1"],
  [".session-row.selected .session-row-open:disabled .session-title", "color", "var(--on-accent-dim)"],
  [".tab.selected:focus-visible", "outline-color", "var(--on-fill)"],
  [".btn-primary:not(.btn-outline):focus-visible", "outline-color", "var(--on-fill)"],
  [".btn-danger:not(.btn-outline):focus-visible", "outline-color", "var(--on-fill)"],
  [".btn-primary", "background", "var(--accent-fill)"],
  [".btn-primary", "color", "var(--on-fill)"],
  [".btn-primary:hover:not(:disabled)", "background", "var(--accent-fill-hover)"],
  [".btn-danger", "background", "var(--danger-bg)"],
  [".btn-danger", "color", "var(--on-fill)"],
  [".btn-neutral:hover:not(:disabled)", "background", "var(--control-hover-bg)"],
  [".launch-composer-actions .create-session-submit", "background", "var(--accent-fill)"],
  [".launch-composer-actions .create-session-submit", "color", "var(--on-fill)"],
  [".launch-composer-launch-context", "color", "var(--on-accent-dim)"],
  [".claude-trust-band", "background", "var(--bg-2)"],
  [".claude-trust-band-copy strong", "color", "var(--info)"],
];

/** Report missing or misrouted role declarations without trusting token names.
 * This guards the source contract only; runtime tests still have to establish
 * that the actual selected DOM receives these declarations after the cascade. */
function checkShippedRoles(css) {
  const errors = [];
  for (const [selector, property, expected] of SHIPPED_ROLE_BINDINGS) {
    const actual = declarationsForSelector(css, selector).get(property);
    if (actual !== expected) errors.push(`${selector}: ${property} is ${actual}, expected ${expected}`);
  }
  for (const selector of SELECTED_OWNERS) {
    const shadow = declarationsForSelector(css, selector).get("box-shadow") || "";
    if (shadow.includes("--accent")) errors.push(`${selector}: obsolete selected accent stripe ${shadow}`);
  }
  return errors;
}

/** Composite opacity in encoded sRGB, as CSS paints an ordinary opacity layer.
 * Contrast is computed after compositing, rather than multiplying the opaque
 * contrast ratio: the latter would miss the selected pulse's old 0.45 trough. */
function compositeHex(foreground, background, opacity) {
  const fg = expandHex(foreground);
  const bg = expandHex(background);
  const channels = [1, 3, 5].map((offset) => {
    const mixed = parseInt(fg.slice(offset, offset + 2), 16) * opacity +
      parseInt(bg.slice(offset, offset + 2), 16) * (1 - opacity);
    return Math.round(mixed).toString(16).padStart(2, "0");
  });
  return `#${channels.join("")}`;
}

// ---- Real-file regressions: arithmetic, use sites and selected motion.
// Browser checks complement these by resolving the actual DOM cascade. ----

test("the shipped app.css palette clears every AA/non-text binding its comments promise", () => {
  const css = fs.readFileSync(APP_CSS_PATH, "utf8");
  const result = checkContrastBindings(css, REQUIRED_BINDINGS);
  assert.deepEqual(result.errors, []);
  assert.equal(result.ok, true);
});

test("solid selection uses inverse roles at actual owners, metadata, marks and focus", () => {
  const css = fs.readFileSync(APP_CSS_PATH, "utf8");
  assert.deepEqual(checkShippedRoles(css), []);
});

test("selected running retains its shaped stepped pulse above the graphical floor", () => {
  const css = fs.readFileSync(APP_CSS_PATH, "utf8");
  const root = declarationsForSelector(css, ":root");
  const tokens = extractRootHexTokens(css);
  const minimum = Number(root.get("--selected-status-pulse-min"));
  assert.equal(minimum, 0.85);
  assert.equal(root.get("--status-pulse-min"), "0.60");
  for (const selector of [".session-row.selected", ".quick-switcher-row[aria-selected=\"true\"]"]) {
    assert.equal(declarationsForSelector(css, selector).get("--status-pulse-min"), "var(--selected-status-pulse-min)");
  }
  assert.equal(
    declarationsForSelector(css, ".status-badge.running .status-dot").get("animation"),
    "none",
    "the final reduced-motion rule must still stop the pulse",
  );
  const stripped = stripComments(css);
  assert.match(stripped, /\.status-badge\.running \.status-dot\s*\{\s*animation: farhelm-status-pulse 2s steps\(4\) infinite;/);
  assert.match(stripped, /@keyframes farhelm-status-pulse\s*\{[\s\S]*?50%\s*\{\s*opacity: var\(--status-pulse-min\);/);
  assert.equal(
    declarationsForSelector(css, ".status-badge.waiting .status-dot::before").get("clip-path"),
    "polygon(50% 0, 100% 100%, 0 100%)",
  );
  assert.equal(
    declarationsForSelector(css, ".status-badge.idle .status-dot::before").get("border"),
    "1.5px solid currentColor",
  );
  assert.equal(
    declarationsForSelector(css, ".status-badge.idle.unseen .status-dot::before").get("clip-path"),
    "polygon(50% 0, 100% 50%, 50% 100%, 0 50%)",
  );
  assert.equal(
    declarationsForSelector(css, ".session-row.stale .status-badge.running .status-dot").get("animation"),
    "none",
  );
  for (const surface of SELECTED_SURFACES) {
    const background = tokens.get(surface);
    const trough = compositeHex(tokens.get("--on-fill"), background, minimum);
    assert.ok(contrastRatio(trough, background) >= 3, `${surface}: selected pulse trough fell below 3:1`);
    assert.ok(
      contrastRatio(compositeHex(tokens.get("--on-fill"), background, 0.45), background) < 3,
      `${surface}: premise that the old pulse trough loses contrast no longer holds`,
    );
  }
  // Ordinary green also needs the opacity calculation. The quick switcher's
  // neutral hover is brighter than the sidebar/card, so it binds this floor.
  for (const surface of ["--bg-1", "--bg-2", "--control-hover-bg"]) {
    const background = tokens.get(surface);
    const trough = compositeHex(tokens.get("--ok"), background, Number(root.get("--status-pulse-min")));
    assert.ok(contrastRatio(trough, background) >= 3, `${surface}: ordinary running pulse trough fell below 3:1`);
    assert.ok(contrastRatio(compositeHex(tokens.get("--ok"), background, 0.45), background) < 3);
  }
});

test("an accessible unused inverse token cannot hide a selected inline host tint", () => {
  const css = fs.readFileSync(APP_CSS_PATH, "utf8");
  const broken = css.replace("color: var(--on-fill) !important;", "color: var(--host-icon-red) !important;");
  assert.notEqual(broken, css, "the mutation must actually change the host override");
  assert.ok(checkShippedRoles(broken).some((error) => error.includes(".host-kind-icon")));
});

test("opacity compositing preserves opaque, transparent and midpoint channels", () => {
  assert.equal(compositeHex("#fff", "#000", 1), "#ffffff");
  assert.equal(compositeHex("#fff", "#000", 0), "#000000");
  assert.equal(compositeHex("#fff", "#000", 0.5), "#808080");
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
