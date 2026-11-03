// Generates the golden key table that pins the supervisor's key encoder to
// today's browser terminal.
//
// Terminals in terminal hosts get their keys from the supervisor: the grid
// client reports a keydown (`key`, `code`, `keyCode`, modifiers, whether it
// runs on a Mac) and the supervisor turns it into bytes
// (`crates/farhelm-supervisor/src/grid_input.rs`). Programs must see the
// bytes today's xterm.js terminal sends, so the table is computed by the
// vendored xterm.js bundle itself: its own `evaluateKeyboardEvent`, lifted
// out of the minified file, run over every key code, every modifier set,
// both platforms that function tells apart (Mac and everything else) and
// both cursor-key modes, with Farhelm's Shift+Enter merge
// (`assets/shift-enter-key.js`, the exact functions the page loads) applied
// on top. The supervisor's Rust test compares its encoder with every row,
// and `xterm-key-table.test.js` regenerates the table and compares it with
// the checked-in file, so re-vendoring xterm.js without regenerating fails a
// test instead of drifting.
//
// The table is that function's result for every keydown, including ones
// xterm.js's `_keyDown` would drop or leave to text input (a third-level
// shift, a lone capital, the keydown after a dead key). Those filters are
// the grid client's, which sends only the keydowns that survive them, and
// its own tests pin them; a row here does not mean today's terminal sends
// its bytes for that keydown.
//
// Regenerate with `node crates/farhelm-ui/js-tests/xterm-key-table.js --write`
// (from the repository root); without `--write` it prints the table.
//
// Not named `*.test.js`, so `node --test`'s default discovery does not run
// it as a test file.
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { shiftEnterKeyAction, mergeArmedPrefix } = require("../assets/shift-enter-key.js");
const { C0, blockAfter, bundleSource } = require("./xterm-vendored.js");

const TABLE_PATH = path.join(__dirname, "../../farhelm-supervisor/tests/fixtures/xterm-keys.jsonl");

// ## Lifting the function out of the bundle

/**
 * The vendored bundle's own `evaluateKeyboardEvent(ev, applicationCursorMode,
 * isMac, macOptionIsMeta)`, as a callable, with the Alt key mapping table it
 * reads beside it.
 */
function vendoredEvaluate() {
  const source = bundleSource();
  const { at, block: body } = blockAfter(source, "evaluateKeyboardEvent=function(e,t,i,n)", "evaluateKeyboardEvent");
  const { block: mapping } = blockAfter(source, "r=", "the Alt key mapping table next to evaluateKeyboardEvent", at);
  if (!mapping.startsWith('{48:["0",")"]')) throw new Error("the Alt key mapping table changed shape");
  // Evaluation of the trusted vendored file, so the table comes from the
  // shipped code rather than a second hand-written copy.
  return new Function("s", "r", `return function(e,t,i,n)${body};`)({ C0 }, new Function(`return ${mapping};`)());
}

// ## The inputs

// The key and code a US layout reports for the key codes the function
// treats specially, so each key code is tried with a realistic value as
// well as the odd ones below.
const NAMED_KEYS = {
  8: ["Backspace", "Backspace"],
  9: ["Tab", "Tab"],
  13: ["Enter", "Enter"],
  16: ["Shift", "ShiftLeft"],
  17: ["Control", "ControlLeft"],
  18: ["Alt", "AltLeft"],
  20: ["CapsLock", "CapsLock"],
  27: ["Escape", "Escape"],
  32: [" ", "Space"],
  33: ["PageUp", "PageUp"],
  34: ["PageDown", "PageDown"],
  35: ["End", "End"],
  36: ["Home", "Home"],
  37: ["ArrowLeft", "ArrowLeft"],
  38: ["ArrowUp", "ArrowUp"],
  39: ["ArrowRight", "ArrowRight"],
  40: ["ArrowDown", "ArrowDown"],
  45: ["Insert", "Insert"],
  46: ["Delete", "Delete"],
  91: ["Meta", "MetaLeft"],
  106: ["*", "NumpadMultiply"],
  107: ["+", "NumpadAdd"],
  109: ["-", "NumpadSubtract"],
  110: [".", "NumpadDecimal"],
  111: ["/", "NumpadDivide"],
  186: [";", "Semicolon"],
  187: ["=", "Equal"],
  188: [",", "Comma"],
  189: ["-", "Minus"],
  190: [".", "Period"],
  191: ["/", "Slash"],
  192: ["`", "Backquote"],
  219: ["[", "BracketLeft"],
  220: ["\\", "Backslash"],
  221: ["]", "BracketRight"],
  222: ["'", "Quote"],
  229: ["Process", ""],
};

/** The realistic `[key, code]` pairs for one key code. */
function naturalKeys(keyCode) {
  if (keyCode === 0) {
    return ["UIKeyInputUpArrow", "UIKeyInputDownArrow", "UIKeyInputLeftArrow", "UIKeyInputRightArrow", "Unidentified"]
      .map((key) => [key, ""]);
  }
  if (keyCode >= 48 && keyCode <= 57) {
    const digit = String(keyCode - 48);
    return [[digit, `Digit${digit}`]];
  }
  if (keyCode >= 65 && keyCode <= 90) {
    const letter = String.fromCharCode(keyCode);
    return [[letter.toLowerCase(), `Key${letter}`], [letter, `Key${letter}`]];
  }
  if (keyCode >= 96 && keyCode <= 105) {
    const digit = String(keyCode - 96);
    return [[digit, `Numpad${digit}`]];
  }
  if (keyCode >= 112 && keyCode <= 123) return [[`F${keyCode - 111}`, `F${keyCode - 111}`]];
  if (NAMED_KEYS[keyCode]) return [NAMED_KEYS[keyCode]];
  return [["Unidentified", ""]];
}

// Values every key code is tried with, for the branches that read `key` or
// `code` rather than the key code: a dead key under Alt (which encodes the
// letter in `code`), Ctrl with `_` or `@`, and an empty `key` (falsy in the
// function's checks).
const EVERY_KEY_CODE = [["Dead", "KeyE"], ["Dead", "Quote"], ["_", "Minus"], ["@", "Digit2"], ["", ""]];

// Rarer shapes, tried on a few key codes each: a multi-character `key`, a
// non-ASCII character, one outside the Basic Multilingual Plane (two
// UTF-16 units, so not "one character" to xterm.js), a dead key whose code
// is "Key" alone or ends in such a character, a numpad Enter, Enter during
// composition, and key codes past the byte range.
const EXTRA_INPUTS = [
  [65, "ab", ""],
  [0, "é", ""],
  [50, "é", "Digit2"],
  [222, "é", "Quote"],
  [65, "😀", "KeyA"],
  [222, "Dead", "Key"],
  [222, "Dead", "Key😀"],
  [222, "Dead", "Keyé"],
  [13, "Enter", "NumpadEnter"],
  [229, "Enter", "Enter"],
  [256, "a", "KeyA"],
  [1000, "ß", "Minus"],
];

/** Every `{kc, key, code}` the table covers, in table order, without repeats. */
function tableInputs() {
  const inputs = [];
  const seen = new Set();
  const add = (kc, key, code) => {
    const id = JSON.stringify([kc, key, code]);
    if (seen.has(id)) return;
    seen.add(id);
    inputs.push({ kc, key, code });
  };
  for (let kc = 0; kc < 256; kc++) {
    for (const [key, code] of naturalKeys(kc)) add(kc, key, code);
    for (const [key, code] of EVERY_KEY_CODE) add(kc, key, code);
  }
  for (const [kc, key, code] of EXTRA_INPUTS) add(kc, key, code);
  return inputs;
}

// ## What today's terminal sends

/**
 * The bytes `evaluateKeyboardEvent` produces for one keydown, before
 * `_keyDown`'s filters decide whether it is sent at all (see the file
 * header), as lowercase hex ("" for none): the sequence with Farhelm's
 * options (`macOptionIsMeta` off, as terminal.js leaves it), Shift+Enter's
 * ESC merged on as terminal.js merges it, encoded as UTF-8 the way the
 * page's `TextEncoder` sends it (a lone surrogate becomes U+FFFD).
 */
function sentBytes(evaluate, ev, appCursor, isMac) {
  const result = evaluate(ev, appCursor, isMac, false);
  if (!result.key) return "";
  let data = result.key;
  if (shiftEnterKeyAction(ev) === "arm") data = mergeArmedPrefix(true, data).send;
  return Buffer.from(data, "utf8").toString("hex");
}

/**
 * The whole table as text: a comment header, then one JSON array per input,
 * `[kc, key, code, sent]`. `sent` has one entry per modifier set, indexed by
 * the grid format's `mods` bits (1 Shift, 2 Alt, 4 Ctrl, 8 Meta). An entry
 * is the hex bytes sent ("" for none) when they are the same on every
 * platform and cursor-key mode, which they mostly are; otherwise it is
 * `[other, otherAppCursor, mac, macAppCursor]`: on a non-Mac and on a Mac,
 * with application cursor keys off and on. One line per input keeps the
 * file small enough to review in a diff when xterm.js is re-vendored.
 */
function generateTable() {
  const evaluate = vendoredEvaluate();
  const lines = [
    "# Generated by crates/farhelm-ui/js-tests/xterm-key-table.js from the vendored xterm.js; do not edit.",
    "# Regenerate: node crates/farhelm-ui/js-tests/xterm-key-table.js --write",
    "# evaluateKeyboardEvent's sequence (with the Shift+Enter merge) for each keydown; which keydowns are sent is the client's.",
    "# [keyCode, key, code, sent]: sent[mods] (1 Shift, 2 Alt, 4 Ctrl, 8 Meta) is the hex bytes,",
    "# or [non-Mac, non-Mac with application cursor keys, Mac, Mac with application cursor keys].",
  ];
  for (const { kc, key, code } of tableInputs()) {
    const sent = [];
    for (let mods = 0; mods < 16; mods++) {
      const ev = {
        type: "keydown",
        key,
        code,
        keyCode: kc,
        shiftKey: !!(mods & 1),
        altKey: !!(mods & 2),
        ctrlKey: !!(mods & 4),
        metaKey: !!(mods & 8),
        isComposing: false,
        getModifierState: () => false,
      };
      const variants = [];
      for (const isMac of [false, true]) {
        for (const appCursor of [false, true]) variants.push(sentBytes(evaluate, ev, appCursor, isMac));
      }
      sent.push(variants.every((bytes) => bytes === variants[0]) ? variants[0] : variants);
    }
    lines.push(JSON.stringify([kc, key, code, sent]));
  }
  return lines.join("\n") + "\n";
}

if (require.main === module) {
  const table = generateTable();
  if (process.argv.includes("--write")) fs.writeFileSync(TABLE_PATH, table);
  else process.stdout.write(table);
}

module.exports = { TABLE_PATH, generateTable, tableInputs, vendoredEvaluate };
