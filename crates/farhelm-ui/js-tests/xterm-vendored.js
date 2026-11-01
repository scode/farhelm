// Lifting functions out of the vendored, minified xterm.js bundle, for the
// generators of the supervisor's golden input tables
// (`xterm-key-table.js`, `xterm-mouse-table.js`).
//
// The tables are only worth having if they come from the shipped code
// rather than a second hand-written copy, so the generators evaluate the
// bundle's own functions. Every lookup here throws when the bundle no
// longer has the shape it relies on, so re-vendoring xterm.js fails loudly
// instead of producing a table from nothing.
//
// Not named `*.test.js`, so `node --test`'s default discovery does not run
// it as a test file.
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const XTERM_PATH = path.join(__dirname, "../assets/vendor/xterm.js");

/** The vendored bundle's source. */
function bundleSource() {
  return fs.readFileSync(XTERM_PATH, "utf8");
}

/**
 * The text of the balanced `{...}` block that opens at `open`, skipping
 * braces inside string and template literals. Enough for the minified
 * bundle's object literals and function bodies, which contain no regex
 * literals or comments in the regions extracted here.
 */
function balancedBlock(source, open) {
  if (source[open] !== "{") throw new Error("extraction must start at a brace");
  let depth = 0;
  let quote = null;
  for (let i = open; i < source.length; i++) {
    const ch = source[i];
    if (quote) {
      if (ch === "\\") i++;
      else if (ch === quote) quote = null;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === "`") quote = ch;
    else if (ch === "{") depth++;
    else if (ch === "}" && --depth === 0) return source.slice(open, i + 1);
  }
  throw new Error("unbalanced block");
}

/**
 * The block that follows the first occurrence of `marker` at or after
 * `from` (the marker must end just before the block's opening brace), or a
 * thrown error naming `what`. A negative `from` (an `indexOf` that found
 * nothing) is an error too.
 */
function blockAfter(source, marker, what, from = 0) {
  if (from < 0) throw new Error(`vendored xterm.js no longer has ${what}`);
  const at = source.indexOf(marker, from);
  if (at === -1) throw new Error(`vendored xterm.js no longer has ${what} (looked for ${JSON.stringify(marker)})`);
  return { at, block: balancedBlock(source, at + marker.length) };
}

/**
 * The C0 control characters by xterm.js's names, for the functions that
 * read them as `s.C0.X`. A name not listed throws instead of reading as
 * `undefined`, which would otherwise put the text "undefined" into a table
 * row and show up as a baffling encoder mismatch.
 */
const C0 = new Proxy(
  { NUL: "\x00", HT: "\t", CR: "\r", ESC: "\x1b", FS: "\x1c", GS: "\x1d", US: "\x1f", DEL: "\x7f" },
  {
    get(names, name) {
      if (!Object.prototype.hasOwnProperty.call(names, name)) {
        throw new Error(`vendored xterm.js reads C0.${String(name)}, which the generator does not define`);
      }
      return names[name];
    },
  },
);

module.exports = { C0, XTERM_PATH, balancedBlock, blockAfter, bundleSource };
