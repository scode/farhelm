// Generates the golden mouse table that pins the supervisor's mouse report
// encoder to today's browser terminal.
//
// For terminals in terminal hosts the grid client sends mouse events in cell
// coordinates and the supervisor encodes them
// (`crates/farhelm-supervisor/src/grid_input.rs`). The table is computed by
// the vendored xterm.js bundle's own mouse service: its protocols' `restrict`
// rules, its event-code function, its DEFAULT and SGR encodings, and
// `triggerMouseEvent`'s own checks, lifted out of the minified file and run
// over every kind of event the grid format has, every Shift, Alt and Ctrl
// combination (xterm.js does not report Meta), each reporting protocol the
// supervisor's emulator tracks (none, 1000, 1002, 1003), both encodings, and
// coordinates at the screen's edges and the default encoding's byte limit.
// The supervisor's Rust test compares its encoder with every row, and
// `xterm-mouse-table.test.js` keeps the table in step with the bundle.
//
// Not covered here, and tested by hand on the Rust side: UTF-8 coordinates
// (1005), which xterm.js does not implement; the wheel on the alternate
// screen without reporting, which xterm.js turns into arrow keys outside the
// mouse service; and motion de-duplication (`_lastEvent`), which is the grid
// client's job, so every row starts with no previous event.
//
// Regenerate with `node crates/farhelm-ui/js-tests/xterm-mouse-table.js --write`
// (from the repository root); without `--write` it prints the table.
//
// Not named `*.test.js`, so `node --test`'s default discovery does not run
// it as a test file.
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { blockAfter, bundleSource } = require("./xterm-vendored.js");

const TABLE_PATH = path.join(__dirname, "../../farhelm-supervisor/tests/fixtures/xterm-mouse.jsonl");

// ## Lifting the mouse service out of the bundle

/**
 * A callable `report(event, protocol, encoding, cols, rows)` built from the
 * bundle's own `CoreMouseService` pieces: what `triggerMouseEvent` sends for
 * an xterm.js mouse event (`{col, row, button, action, ctrl, alt, shift}`,
 * 0-based cell) as a string of byte values, or "" for nothing. Each call
 * starts with no previous event.
 */
function vendoredReport() {
  const source = bundleSource();
  const { block: protocols } = blockAfter(source, "l=", "the mouse protocols (NONE, X10, VT200, DRAG, ANY)",
    source.indexOf("l={NONE:{events:0"));
  // The bundle has other one-letter functions called `h`; this one starts
  // by collecting the modifier bits.
  const { block: code } = blockAfter(source, "function h(e,t)", "the mouse event-code function",
    source.indexOf("function h(e,t){let i=(e.ctrl?16:0)"));
  const { block: encodings } = blockAfter(source, "d=", "the mouse encodings (DEFAULT, SGR)",
    source.indexOf("d={DEFAULT:"));
  const { block: trigger } = blockAfter(source, "triggerMouseEvent(e)", "triggerMouseEvent");
  const { block: equal } = blockAfter(source, "_equalEvents(e,t,i)", "the mouse event comparison");
  // Evaluation of the trusted vendored file, so the table comes from the
  // shipped code rather than a second hand-written copy.
  const h = new Function(`return function(e,t)${code};`)();
  const service = {
    _protocols: new Function(`return ${protocols};`)(),
    _encodings: new Function("h", "c", `return ${encodings};`)(h, String.fromCharCode),
    triggerMouseEvent: new Function(`return function(e)${trigger};`)(),
    _equalEvents: new Function(`return function(e,t,i)${equal};`)(),
  };
  for (const name of ["NONE", "VT200", "DRAG", "ANY"]) {
    if (!service._protocols[name]) throw new Error(`the mouse protocols no longer include ${name}`);
  }
  for (const name of ["DEFAULT", "SGR"]) {
    if (!service._encodings[name]) throw new Error(`the mouse encodings no longer include ${name}`);
  }
  // A lifted function of the wrong shape can still return something that
  // looks like a report, so check one event code whose parts all differ:
  // a right-button press (2) with Ctrl (16) and Alt (8).
  const probe = h({ button: 2, action: 1, ctrl: true, alt: true, shift: false }, true);
  if (probe !== 26) throw new Error(`the mouse event-code function gave ${probe} for a known event, not 26`);
  return (event, protocol, encoding, cols, rows) => {
    let sent = "";
    const instance = Object.assign(Object.create(service), {
      _bufferService: { cols, rows },
      _activeProtocol: protocol,
      _activeEncoding: encoding,
      _lastEvent: null,
      _coreService: {
        // The default encoding's bytes go out as binary (one byte per
        // character), the SGR encoding's as text; both are ASCII or bytes
        // below 256 here, so one conversion serves both.
        triggerBinaryEvent: (data) => (sent = data),
        triggerDataEvent: (data) => (sent = data),
      },
    });
    instance.triggerMouseEvent({ ...event });
    return Buffer.from(sent, "latin1").toString("hex");
  };
}

// ## The inputs

// Every event the grid format can carry, `[kind, button]`, and the xterm.js
// button and action its DOM handler would have made for the same browser
// event: a press or release keeps its button (0 to 2), motion carries the
// held button or 3 for none, and the wheel is button 4 with action 0 (up)
// or 1 (down).
const EVENTS = [
  ["down", 0], ["down", 1], ["down", 2],
  ["up", 0], ["up", 1], ["up", 2],
  ["move", -1], ["move", 0], ["move", 1], ["move", 2],
  ["wheel", 0], ["wheel", 1],
];

function xtermButtonAction(kind, button) {
  switch (kind) {
    case "down":
      return [button, 1];
    case "up":
      return [button, 0];
    case "move":
      return [button === -1 ? 3 : button, 32];
    case "wheel":
      return [4, button === 0 ? 0 : 1];
  }
  throw new Error(`unknown kind ${kind}`);
}

// `[x, y, cols, rows]`: the corners of an 80x24 screen and just past it,
// and on the largest screen the default encoding's last cell (222), one
// past it in each direction, and the far corner.
const PLACES = [
  [0, 0, 80, 24],
  [79, 23, 80, 24],
  [80, 0, 80, 24],
  [0, 24, 80, 24],
  [222, 222, 1000, 1000],
  [223, 5, 1000, 1000],
  [5, 223, 1000, 1000],
  [999, 999, 1000, 1000],
];

const PROTOCOLS = ["NONE", "VT200", "DRAG", "ANY"];
const ENCODINGS = ["DEFAULT", "SGR"];

/**
 * The whole table as text: a comment header, then one JSON array per event,
 * place, protocol and encoding, `[kind, button, x, y, cols, rows, protocol,
 * encoding, sent]`. `sent` is the hex bytes sent ("" for none) when they are
 * the same under every modifier set, else one entry per set indexed by the
 * grid format's bits (1 Shift, 2 Alt, 4 Ctrl).
 */
function generateTable() {
  const report = vendoredReport();
  const lines = [
    "# Generated by crates/farhelm-ui/js-tests/xterm-mouse-table.js from the vendored xterm.js; do not edit.",
    "# Regenerate: node crates/farhelm-ui/js-tests/xterm-mouse-table.js --write",
    "# [kind, button, x, y, cols, rows, protocol, encoding, sent]: sent is the hex bytes sent, the same",
    "# for every modifier set, or sent[mods] for mods 0 to 7 (1 Shift, 2 Alt, 4 Ctrl).",
  ];
  for (const [kind, gridButton] of EVENTS) {
    const [button, action] = xtermButtonAction(kind, gridButton);
    for (const [x, y, cols, rows] of PLACES) {
      for (const protocol of PROTOCOLS) {
        for (const encoding of ENCODINGS) {
          const sent = [];
          for (let mods = 0; mods < 8; mods++) {
            const event = { col: x, row: y, x: 0, y: 0, button, action, shift: !!(mods & 1), alt: !!(mods & 2), ctrl: !!(mods & 4) };
            sent.push(report(event, protocol, encoding, cols, rows));
          }
          const same = sent.every((bytes) => bytes === sent[0]);
          lines.push(JSON.stringify([kind, gridButton, x, y, cols, rows, protocol, encoding, same ? sent[0] : sent]));
        }
      }
    }
  }
  return lines.join("\n") + "\n";
}

if (require.main === module) {
  const table = generateTable();
  if (process.argv.includes("--write")) fs.writeFileSync(TABLE_PATH, table);
  else process.stdout.write(table);
}

module.exports = { EVENTS, PLACES, PROTOCOLS, TABLE_PATH, generateTable };
