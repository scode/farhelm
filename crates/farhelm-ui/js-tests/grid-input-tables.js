// Renders the checked-in tables that pin new terminals' keyboard and
// Alt-click behavior, from the grid client's own code (grid-input.js).
//
// New terminals type the way terminals of the xterm family do, byte for
// byte, because shells and agents read keys that way, and the page leaves
// to the browser's text input exactly the keys whose character only the
// browser knows (Option on a Mac, dead keys, AltGr). That behavior is
// Farhelm's contract now, and these tables are where it is written down:
//
// - `crates/farhelm-supervisor/tests/fixtures/grid-keys.jsonl`: the bytes
//   each keydown sends, for every key code below 256, modifier set,
//   platform and cursor-key mode. The supervisor's encoder is held to it by
//   its own Rust test, and the page's half (`keyboardResult`) by
//   `grid-input-tables.test.js`, so the two cannot drift apart.
// - `fixtures/input/keydown-actions.jsonl`: what the page does with each
//   keydown (send it as a `key` message, leave it to text input, scroll,
//   select all, ignore it), by platform, AltGraph and dead-key state.
// - `fixtures/input/move-to-cell.txt`: the arrow keys an Alt-click types to
//   move a shell's cursor, over seeded random screens.
//
// The key table was first generated from the vendored xterm.js 6.0.0
// bundle's own functions, and the other two from grid-input.js while its
// tests held it to that bundle, so when they were frozen every row matched
// what an old terminal does. The tables are now the reference, and nothing
// here reads xterm.js. A deliberate change to one of these behaviors
// regenerates the tables with
// `node crates/farhelm-ui/js-tests/grid-input-tables.js --write` (from the
// repository root) and reviews the diff: every changed row is a key or a
// click that a program will now read differently. Without `--write` it
// prints the key table. The mouse table beside the key table has no
// renderer: the page does not encode mouse reports, so it is edited by hand
// together with the supervisor's encoder.
//
// Not named `*.test.js`, so `node --test`'s default discovery does not run
// it as a test file.
"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const input = require("../assets/grid-input.js");
const { shiftEnterKeyAction, mergeArmedPrefix } = require("../assets/shift-enter-key.js");

const KEY_TABLE_PATH = path.join(__dirname, "../../farhelm-supervisor/tests/fixtures/grid-keys.jsonl");
const KEYDOWN_TABLE_PATH = path.join(__dirname, "fixtures/input/keydown-actions.jsonl");
const MOVE_TABLE_PATH = path.join(__dirname, "fixtures/input/move-to-cell.txt");

// ## The key table's inputs

// The key and code a US layout reports for the key codes the keyboard rules
// treat specially, so each key code is tried with a realistic value as
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

// Values every key code is tried with, for the rules that read `key` or
// `code` rather than the key code: a dead key under Alt (which encodes the
// letter in `code`), Ctrl with `_` or `@`, and an empty `key` (falsy in the
// rules' checks).
const EVERY_KEY_CODE = [["Dead", "KeyE"], ["Dead", "Quote"], ["_", "Minus"], ["@", "Digit2"], ["", ""]];

// Rarer shapes, tried on a few key codes each: a multi-character `key`, a
// non-ASCII character, one outside the Basic Multilingual Plane (two
// UTF-16 units, so not "one character" to the keyboard rules), a dead key
// whose code is "Key" alone or ends in such a character, a numpad Enter,
// Enter during composition, and key codes past the byte range.
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

/** Every `{kc, key, code}` the key table covers, in table order, without repeats. */
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

/** A keydown event with the modifier bit set `mods` (1 Shift, 2 Alt, 4 Ctrl, 8 Meta). */
function keydown(kc, key, code, mods, altGraph = false) {
  return {
    type: "keydown",
    key,
    code,
    keyCode: kc,
    shiftKey: !!(mods & 1),
    altKey: !!(mods & 2),
    ctrlKey: !!(mods & 4),
    metaKey: !!(mods & 8),
    isComposing: false,
    getModifierState: (name) => name === "AltGraph" && altGraph,
  };
}

// ## The key table

/**
 * The bytes one keydown sends, before the page's filters decide whether it
 * is sent at all (a third-level shift, a lone capital, the keydown after a
 * dead key: those are the keydown table's), as lowercase hex ("" for
 * none): the sequence with Option not acting as Meta on a Mac, Shift+Enter's
 * ESC merged on as terminal.js merges it, encoded as UTF-8 the way the
 * page's `TextEncoder` sends it (a lone surrogate becomes U+FFFD).
 */
function sentBytes(ev, appCursor, isMac) {
  const result = input.keyboardResult(ev, appCursor, isMac, false);
  if (!result.key) return "";
  let data = result.key;
  if (shiftEnterKeyAction(ev) === "arm") data = mergeArmedPrefix(true, data).send;
  return Buffer.from(data, "utf8").toString("hex");
}

/**
 * The key table as text: a comment header, then one JSON array per input,
 * `[kc, key, code, sent]`. `sent` has one entry per modifier set, indexed by
 * the grid format's `mods` bits. An entry is the hex bytes sent ("" for
 * none) when they are the same on every platform and cursor-key mode, which
 * they mostly are; otherwise it is `[other, otherAppCursor, mac,
 * macAppCursor]`: on a non-Mac and on a Mac, with application cursor keys
 * off and on. One line per input keeps a change reviewable in a diff.
 */
function keyTable() {
  const lines = [
    "# The bytes a new terminal's program reads for each keydown: Farhelm's key contract, which the supervisor's",
    "# encoder (grid_input.rs) and the page's keyboard rules (grid-input.js) are both held to. Change it only on",
    "# purpose: regenerate with `node crates/farhelm-ui/js-tests/grid-input-tables.js --write` and review the diff.",
    "# Which keydowns are sent at all is the page's (js-tests/fixtures/input/keydown-actions.jsonl).",
    "# [keyCode, key, code, sent]: sent[mods] (1 Shift, 2 Alt, 4 Ctrl, 8 Meta) is the hex bytes,",
    "# or [non-Mac, non-Mac with application cursor keys, Mac, Mac with application cursor keys].",
  ];
  for (const { kc, key, code } of tableInputs()) {
    const sent = [];
    for (let mods = 0; mods < 16; mods++) {
      const ev = keydown(kc, key, code, mods);
      const variants = [];
      for (const isMac of [false, true]) {
        for (const appCursor of [false, true]) variants.push(sentBytes(ev, appCursor, isMac));
      }
      sent.push(variants.every((bytes) => bytes === variants[0]) ? variants[0] : variants);
    }
    lines.push(JSON.stringify([kc, key, code, sent]));
  }
  return lines.join("\n") + "\n";
}

// ## The keydown table

const PLATFORMS = ["Linux x86_64", "MacIntel", "Win32"];

// One letter per outcome of `keydownAction`: its action, whether it cancels
// the event, and whether a dead key is pending after it.
const OUTCOMES = [
  ["pass", false, false],
  ["pass", false, true],
  ["send", true, false],
  ["pass", true, false],
  ["page-up", true, false],
  ["page-up", true, true],
  ["page-down", true, false],
  ["page-down", true, true],
  ["ignore", false, false],
  ["ignore", false, true],
  ["select-all", false, false],
  ["select-all", false, true],
];
const OUTCOME_LETTERS = "abcdefghijkl";

/** Plausible `key`/`code` pairs for a key code, including the odd ones the rules branch on. */
function candidateKeys(keyCode) {
  const keys = [{ key: "Unidentified", code: "" }, { key: "Dead", code: "KeyE" }, { key: "Dead", code: "Quote" }];
  if (keyCode === 0) {
    for (const key of ["UIKeyInputUpArrow", "UIKeyInputDownArrow", "UIKeyInputLeftArrow", "UIKeyInputRightArrow"]) {
      keys.push({ key, code: "" });
    }
  }
  if (keyCode >= 32 && keyCode < 127) {
    const ch = String.fromCharCode(keyCode);
    keys.push({ key: ch.toLowerCase(), code: `Key${ch.toUpperCase()}` }, { key: ch.toUpperCase(), code: "" });
  }
  keys.push({ key: "_", code: "Minus" }, { key: "@", code: "Digit2" }, { key: "ab", code: "" });
  return keys;
}

/**
 * The 64 outcomes of one keydown on one platform, as a string of
 * `OUTCOME_LETTERS`: position `mods * 4 + altGraph * 2 + deadBefore`.
 */
function keydownPattern(kc, key, code, platform) {
  let pattern = "";
  for (let mods = 0; mods < 16; mods++) {
    for (const altGraph of [false, true]) {
      for (const deadBefore of [false, true]) {
        const got = input.keydownAction(keydown(kc, key, code, mods, altGraph), platform, deadBefore);
        const at = OUTCOMES.findIndex(([action, cancel, dead]) =>
          action === got.action && cancel === got.cancel && dead === got.deadKeyPending
        );
        if (at === -1) throw new Error(`an outcome the table has no letter for: ${JSON.stringify(got)}`);
        pattern += OUTCOME_LETTERS[at];
      }
    }
  }
  return pattern;
}

/** A pattern's id: the first 8 hex digits of its SHA-1, so it depends on the pattern alone. */
function patternId(pattern) {
  return crypto.createHash("sha1").update(pattern).digest("hex").slice(0, 8);
}

/**
 * The keydown table as text: a header naming the letters, the distinct
 * 64-outcome patterns as `["p", id, pattern]` lines in id order, then one
 * `[kc, key, code, linux, mac, windows]` line per input, each platform's
 * entry the id of its pattern. Patterns are shared because only a few dozen
 * distinct ones occur across thousands of inputs. Ids come from a pattern's
 * content, not from where it is first seen, so a changed rule changes only
 * the rows whose outcome changed (plus the pattern lines it adds or
 * removes) rather than renumbering the rest of the file.
 */
function keydownTable() {
  const patterns = new Map();
  const rows = [];
  for (let kc = 0; kc < 256; kc++) {
    for (const { key, code } of candidateKeys(kc)) {
      const ids = PLATFORMS.map((platform) => {
        const pattern = keydownPattern(kc, key, code, platform);
        const id = patternId(pattern);
        const known = patterns.get(id);
        if (known !== undefined && known !== pattern) throw new Error(`two patterns share the id ${id}`);
        patterns.set(id, pattern);
        return id;
      });
      rows.push([kc, key, code, ...ids]);
    }
  }
  const legend = OUTCOMES.map(([action, cancel, dead], i) =>
    `${OUTCOME_LETTERS[i]} ${action}${cancel ? " cancelled" : ""}${dead ? ", dead key pending" : ""}`
  );
  const lines = [
    "# What a new terminal's page does with each keydown (grid-input.js's keydownAction): Farhelm's contract for",
    "# which keys reach the program at keydown and which are left to the browser's text input. Change it only on",
    "# purpose: regenerate with `node crates/farhelm-ui/js-tests/grid-input-tables.js --write` and review the diff.",
    "# [\"p\", id, pattern] lines name each pattern by an id taken from its SHA-1. A pattern's position",
    "# mods * 4 + altGraph * 2 + deadKeyBefore (mods: 1 Shift, 2 Alt, 4 Ctrl, 8 Meta) holds one letter:",
    ...legend.map((line) => `#   ${line}`),
    `# [keyCode, key, code, pattern id on ${PLATFORMS.join(", pattern id on ")}]`,
    ...[...patterns].sort(([a], [b]) => (a < b ? -1 : 1)).map(([id, pattern]) => JSON.stringify(["p", id, pattern])),
    ...rows.map((row) => JSON.stringify(row)),
  ];
  return lines.join("\n") + "\n";
}

// ## The Alt-click table

/** The seeded random screens and moves the Alt-click table covers. */
function moveCases() {
  let seed = 11;
  const random = (n) => (seed = (seed * 1103515245 + 12345) & 0x7fffffff) % n;
  const cases = [];
  for (let round = 0; round < 400; round++) {
    const cols = 2 + random(9);
    const rows = 1 + random(6);
    const lines = Array.from({ length: rows }, () => Array.from({ length: cols }, () => (random(4) ? "x" : " ")).join(""));
    const wrapped = Array.from({ length: rows }, (_, row) => row > 0 && random(3) === 0);
    for (const hasScrollback of [false, true]) {
      for (const app of [false, true]) {
        const cursor = { x: random(cols), y: random(rows) };
        const target = { x: random(cols), y: random(rows) };
        cases.push({ lines, wrapped, cols, cursor, target, hasScrollback, app });
      }
    }
  }
  return cases;
}

/** A screen of `lines` (strings of `cols` characters, `wrapped[i]`: row i continues row i-1). */
function screenService(lines, wrapped, cols, cursor, hasScrollback) {
  return {
    cols,
    rows: lines.length,
    buffer: {
      x: cursor.x,
      y: cursor.y,
      hasScrollback,
      lines: { length: lines.length, get: (row) => (row >= 0 && row < lines.length ? { isWrapped: wrapped[row] } : undefined) },
      translateBufferLineToString: (row, trimRight, start, end) => {
        const text = lines[row] === undefined ? "" : lines[row].slice(start, end);
        return trimRight ? text.replace(/ +$/, "") : text;
      },
    },
  };
}

/**
 * One case's arrows, run-length coded: "3D1A" is three lefts, then one up
 * ("A" up, "B" down, "C" right, "D" left; "-" for no move). The sequences
 * are made with normal or application cursor keys as the case says, and
 * the code records which, so a change of prefix shows too.
 */
function moveArrows(sequence) {
  if (sequence === "") return "-";
  let out = "";
  let i = 0;
  while (i < sequence.length) {
    const arrow = sequence.slice(i, i + 3);
    let count = 0;
    while (sequence.slice(i, i + 3) === arrow) {
      count++;
      i += 3;
    }
    out += `${count}${arrow[1] === "O" ? arrow[2].toLowerCase() : arrow[2]}`;
  }
  return out;
}

/**
 * The Alt-click table as text: one line per case of `moveCases`, in order,
 * holding `moveArrows` of `moveToCellSequence`'s result (a lowercase letter
 * is an application cursor key).
 */
function moveTable() {
  const lines = [
    "# The arrow keys an Alt-click types to move a shell's cursor (grid-input.js's moveToCellSequence), one line per",
    "# case of grid-input-tables.js's seeded moveCases, in order: Farhelm's Alt-click contract. Change it only on",
    "# purpose: regenerate with `node crates/farhelm-ui/js-tests/grid-input-tables.js --write` and review the diff.",
    "# \"3D1A\" is three lefts then one up (A up, B down, C right, D left; lowercase with application cursor keys);",
    "# \"-\" is no move.",
  ];
  for (const c of moveCases()) {
    const service = screenService(c.lines, c.wrapped, c.cols, c.cursor, c.hasScrollback);
    lines.push(moveArrows(input.moveToCellSequence(c.target.x, c.target.y, service, c.app)));
  }
  return lines.join("\n") + "\n";
}

if (require.main === module) {
  if (process.argv.includes("--write")) {
    fs.writeFileSync(KEY_TABLE_PATH, keyTable());
    fs.mkdirSync(path.dirname(KEYDOWN_TABLE_PATH), { recursive: true });
    fs.writeFileSync(KEYDOWN_TABLE_PATH, keydownTable());
    fs.writeFileSync(MOVE_TABLE_PATH, moveTable());
  } else {
    process.stdout.write(keyTable());
  }
}

module.exports = {
  KEYDOWN_TABLE_PATH,
  KEY_TABLE_PATH,
  MOVE_TABLE_PATH,
  keyTable,
  keydownTable,
  moveCases,
  moveTable,
  tableInputs,
};
