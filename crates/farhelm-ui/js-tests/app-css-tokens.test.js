// Static invariant coverage for app.css's design-token layer (see that
// file's own header and `:root` signpost comment for the rule this test
// pins: colors, font stacks, and non-zero corner radii live ONCE in
// `:root` as named custom properties, and every use site carries
// `var(--token)` rather than a literal).
//
// Why this file exists rather than trusting review: the token layer is a
// contract the rest of the UI refresh is queued to build on (TODO.md's
// surface-layering and light-theme items both assume every color already
// has a name). None of the other test suites would catch a regression
// here — `cargo fmt`/`dprint` don't know a hex code from a token, the
// Playwright suite only sees rendered pixels (a stray literal that
// happens to match its token's value looks identical on screen), and a
// misspelled `var(--toekn)` fails silently in CSS (the browser just
// ignores the malformed declaration) rather than erroring anywhere a
// human would see. A parse-and-assert check over the literal file text is
// the only thing that would have caught either mistake before a screenshot
// review did.
//
// The checker below is a pure function over a CSS string (`checkAppCss`)
// specifically so the negative-path assertions can feed it small synthetic
// snippets instead of mutating the real stylesheet — see the "synthetic
// cases" tests. It is a pragmatic regex/brace-matching scanner, not a full
// CSS parser: good enough for this file's actual shape (no nested at-rules
// besides `@font-face`, no custom properties whose values contain braces),
// not a general-purpose CSS validator.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const APP_CSS_PATH = path.join(__dirname, "../assets/app.css");

// `--menu-top` is the one token this file legitimately references without
// declaring: `menu_panel.rs` sets it inline via a `style` attribute the
// panel's measured position computes at runtime (search that file for
// `--menu-top:` to confirm), so there is no fixed value for `:root` to
// hold. Every other custom property app.css touches has to be declared.
const RUNTIME_ALLOWLISTED_VARS = new Set(["--menu-top"]);

const HEX_COLOR_RE = /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b/;
const COLOR_FUNCTION_RE = /\b(?:rgb|rgba|hsl|hsla)\(/i;
// A literal font stack almost always ends in one of CSS's generic family
// keywords (a *token* stack does too, inside its own `:root` declaration —
// that's why this check only ever runs on the "outside :root" slice of the
// file, never on the whole thing).
const FONT_LIST_LITERAL_RE = /,\s*(?:monospace|sans-serif|serif|cursive|fantasy|system-ui)\b/i;
const VAR_REF_RE = /var\(\s*(--[a-zA-Z0-9-]+)/g;
const VAR_DECL_RE = /(--[a-zA-Z0-9-]+)\s*:/g;

/** Removes every `/* ... *\/` comment so the checks below never trip on
 * prose that happens to mention a hex code or a font name. Must run before
 * any other pass — the block-finding functions below assume comments are
 * already gone, otherwise a brace inside a comment could desync their depth
 * counting. */
function stripComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/**
 * Locates every top-level `{ ... }` block whose header matches `keyword`
 * (e.g. `:root`, `@font-face`), returning each as a `{ blockStart,
 * blockEnd, bodyStart, bodyEnd }` character-offset span.
 *
 * Brace-depth counting rather than a "match to the next `}`" regex,
 * because a block's own body legitimately nests parens-in-braces-adjacent
 * constructs (`var(--x)`, `rgba(...)`, `calc(...)`) that a naive first-
 * closing-brace match does not need to worry about, but a NESTED at-rule
 * would — this stays correct if one is ever added inside `:root`.
 */
function findBlocks(css, keyword) {
  const blocks = [];
  let searchFrom = 0;
  for (;;) {
    const headerIdx = css.indexOf(keyword, searchFrom);
    if (headerIdx === -1) break;
    const openBrace = css.indexOf("{", headerIdx);
    if (openBrace === -1) break;
    let depth = 1;
    let i = openBrace + 1;
    while (i < css.length && depth > 0) {
      if (css[i] === "{") depth++;
      else if (css[i] === "}") depth--;
      i++;
    }
    blocks.push({ blockStart: headerIdx, blockEnd: i, bodyStart: openBrace + 1, bodyEnd: i - 1 });
    searchFrom = i;
  }
  return blocks;
}

/** The set of `--name` custom properties declared inside any `:root` block
 * in `css` (comments already stripped). A light theme's SECOND `:root`
 * block — see app.css's own header — would union in here too, which is
 * deliberate: this checker's job is "was a token declared somewhere legal",
 * not "was it declared exactly once". */
function declaredRootTokens(css) {
  const declared = new Set();
  for (const block of findBlocks(css, ":root")) {
    const body = css.slice(block.bodyStart, block.bodyEnd);
    for (const match of body.matchAll(VAR_DECL_RE)) declared.add(match[1]);
  }
  return declared;
}

/** Returns `css` with the given blocks' full text (header through closing
 * brace) blanked to spaces of the same length, so remaining offsets still
 * line up with the original string for anyone debugging a failure, while
 * none of the blanked text can match a literal-value regex. */
function blankBlocks(css, blocks) {
  let out = css;
  // Blank back-to-front so each replacement's indices stay valid for the
  // ones still queued.
  for (const block of [...blocks].sort((a, b) => b.blockStart - a.blockStart)) {
    const blankLen = block.blockEnd - block.blockStart;
    out = out.slice(0, block.blockStart) + " ".repeat(blankLen) + out.slice(block.blockEnd);
  }
  return out;
}

/**
 * Checks one CSS source string against the token-layer invariants and
 * returns `{ ok, errors }` — never throws, so a caller can assert on the
 * error list's shape rather than catching.
 *
 * Invariants enforced (see this file's header for why each matters):
 *   (a) outside `:root` and `@font-face`, no hex color, no
 *       rgb()/rgba()/hsl()/hsla() call, and no literal font-family list —
 *       every such value must instead be a `var(--token)` reference.
 *   (b) every `var(--x)` referenced anywhere in the source names a token
 *       declared in some `:root` block, or is in `RUNTIME_ALLOWLISTED_VARS`.
 */
function checkAppCss(css) {
  const errors = [];
  const stripped = stripComments(css);

  const rootBlocks = findBlocks(stripped, ":root");
  const fontFaceBlocks = findBlocks(stripped, "@font-face");
  const exemptFromLiteralCheck = blankBlocks(stripped, [...rootBlocks, ...fontFaceBlocks]);

  const hexMatch = exemptFromLiteralCheck.match(HEX_COLOR_RE);
  if (hexMatch) {
    errors.push(`hex color literal "${hexMatch[0]}" found outside :root/@font-face`);
  }
  const colorFnMatch = exemptFromLiteralCheck.match(COLOR_FUNCTION_RE);
  if (colorFnMatch) {
    errors.push(`color function literal "${colorFnMatch[0]}" found outside :root/@font-face`);
  }
  const fontListMatch = exemptFromLiteralCheck.match(FONT_LIST_LITERAL_RE);
  if (fontListMatch) {
    errors.push(`font-family list literal found outside :root/@font-face (matched "${fontListMatch[0]}")`);
  }

  const declared = declaredRootTokens(stripped);
  const referenced = new Set([...stripped.matchAll(VAR_REF_RE)].map((m) => m[1]));
  for (const name of referenced) {
    if (!declared.has(name) && !RUNTIME_ALLOWLISTED_VARS.has(name)) {
      errors.push(`var(${name}) referenced but not declared in :root and not runtime-allowlisted`);
    }
  }

  return { ok: errors.length === 0, errors };
}

// ---- Real-file regression: the invariant the rest of this file exists to
// protect. Everything below this point exercises `checkAppCss` in
// isolation against synthetic snippets, which pin the checker's OWN
// behavior; this is the one test that pins app.css itself. ----

test("the shipped app.css holds no bare literals and every var() resolves", () => {
  const css = fs.readFileSync(APP_CSS_PATH, "utf8");
  const result = checkAppCss(css);
  assert.deepEqual(result.errors, []);
  assert.equal(result.ok, true);
});

// ---- Synthetic cases: each pins one failure mode in isolation, so a
// change to the checker's regexes that quietly stops catching a category
// fails here even on a real app.css that has nothing wrong with it. ----

test("rejects a hex color literal outside :root", () => {
  const css = `:root { --bg-0: #14161a; }\n.thing { color: #ff00ff; }`;
  const result = checkAppCss(css);
  assert.equal(result.ok, false);
  assert.ok(result.errors.some((e) => e.includes("hex color literal")), result.errors.join("\n"));
});

test("rejects an rgb()/rgba()/hsl() literal outside :root", () => {
  const css = `:root { --bg-0: #14161a; }\n.thing { background: rgba(0, 0, 0, 0.5); }`;
  const result = checkAppCss(css);
  assert.equal(result.ok, false);
  assert.ok(result.errors.some((e) => e.includes("color function literal")), result.errors.join("\n"));
});

test("rejects a literal font-family list outside :root", () => {
  const css = `:root { --font-ui: system-ui, sans-serif; }\n.thing { font: 14px Arial, sans-serif; }`;
  const result = checkAppCss(css);
  assert.equal(result.ok, false);
  assert.ok(result.errors.some((e) => e.includes("font-family list literal")), result.errors.join("\n"));
});

test("rejects a var() reference to an undeclared, non-allowlisted token", () => {
  const css = `:root { --bg-0: #14161a; }\n.thing { color: var(--totally-made-up); }`;
  const result = checkAppCss(css);
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((e) => e.includes("--totally-made-up")),
    result.errors.join("\n"),
  );
});

test("@font-face's own font-family literal is exempt, per invariant (c)", () => {
  const css = `:root { --bg-0: #14161a; }\n@font-face { font-family: "JetBrains Mono"; src: url("/x.ttf"); }`;
  const result = checkAppCss(css);
  assert.deepEqual(result.errors, []);
});

test("var(--menu-top) is accepted though never declared in :root", () => {
  // Mirrors the real shape: menu_panel.rs writes this custom property
  // inline at runtime, so app.css only ever CONSUMES it via `var()`, in a
  // `calc()` expression exactly like this one — see this file's own header
  // comment on RUNTIME_ALLOWLISTED_VARS.
  const css = `:root { --bg-0: #14161a; }\n.panel { max-height: calc(100vh - var(--menu-top) - 8px); }`;
  const result = checkAppCss(css);
  assert.deepEqual(result.errors, []);
});

test("a clean stylesheet with only var() use sites passes", () => {
  const css = `:root {\n  --bg-0: #14161a;\n  --font-ui: system-ui, sans-serif;\n}\n` +
    `.thing {\n  color: var(--bg-0);\n  font-family: var(--font-ui);\n}`;
  const result = checkAppCss(css);
  assert.deepEqual(result.errors, []);
});
