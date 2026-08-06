// Unit coverage for term-bytes.js's `binaryStringToBytes`, run with node's
// built-in test runner (PLAN_M6_5.md item 1 — see the plan's "Testing
// decisions" for why `node --test` over vitest/jest: node is already a CI
// requirement for Playwright, and one small function does not earn a
// dependency tree or a bundler config).
//
// Deliberately OUTSIDE assets/: source/test separation, not a bundling
// mechanism — Dioxus bundles only what `asset!` registers (see
// crates/farhelm-ui/src/lib.rs), so a stray file beside terminal.js would
// not be served either; keeping tests out of assets/ just keeps the served
// directory honest about what it is.
//
// `require`, not `import`: the repo has no root `package.json` setting
// `"type": "module"`, so plain `.js` here is CommonJS by node's default —
// matching term-bytes.js's own `module.exports` branch, which is what most
// of the tests below exercise. The browser `window` branch gets its own
// coverage further down, via `node:vm`, rather than being left to the
// mouse-mode e2e alone: that e2e proves the WIRING (the global gets called
// correctly once mounted), but nothing short of a direct check pins that
// the global assignment itself still exists — deleting it would still pass
// every e2e that never runs against a helper-less page.
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const fs = require("node:fs");
const path = require("node:path");
const { binaryStringToBytes } = require("../assets/term-bytes.js");

// The contract under test is "every UTF-16 code unit becomes its low
// byte" — code UNIT, because `charCodeAt` and `length` operate on code
// units — and not any particular masking operator the implementation
// happens to use, per PLAN_M6_5.md item 1. Each case below is chosen to
// pin a byte-domain boundary or a shape the real caller (xterm.js's
// `onBinary`) actually produces, rather than to exercise arithmetic in
// the abstract.

test("empty input yields an empty byte array", () => {
  assert.deepEqual(binaryStringToBytes(""), new Uint8Array(0));
});

test("0x00 survives with both its byte and the output length intact", () => {
  // Nothing in this path has terminator semantics; the useful pin is
  // simply that a zero byte is preserved and contributes to the length.
  assert.deepEqual(binaryStringToBytes("\x00"), new Uint8Array([0x00]));
});

test("0x7f is the top of the ASCII range, converted unchanged", () => {
  assert.deepEqual(binaryStringToBytes("\x7f"), new Uint8Array([0x7f]));
});

test("0x80 is the bottom of the high-byte range mouse reports rely on", () => {
  assert.deepEqual(binaryStringToBytes("\x80"), new Uint8Array([0x80]));
});

test("0xff is the top of the single-byte domain", () => {
  assert.deepEqual(binaryStringToBytes("\xff"), new Uint8Array([0xff]));
});

test("a mouse-report-shaped sequence converts byte-for-byte in order", () => {
  // Shape of a legacy (non-SGR) X10 mouse report: ESC [ M then three bytes
  // encoding button state, column, and row, each offset by 32 — the exact
  // path this helper exists for (PLAN_M6_5.md item 1's motivation), and the
  // reason `onBinary` sees high bytes at all instead of plain ASCII text.
  const report = "\x1b[M" + String.fromCharCode(0x20, 0xff, 0x80);
  assert.deepEqual(
    binaryStringToBytes(report),
    new Uint8Array([0x1b, 0x5b, 0x4d, 0x20, 0xff, 0x80]),
  );
});

test("browser-global branch: window.farhelmTermBytes exists with no module present", () => {
  // Every test above `require()`s term-bytes.js, which only ever exercises
  // the `module.exports` branch — `module` is always defined under
  // `require()`, so the `if (typeof window !== "undefined")` branch never
  // runs there no matter what this file asserts. `node:vm` fabricates the
  // one environment shape that DOES run it: a fresh context with a
  // `window` object and no `module` at all, matching a real browser tab
  // closely enough for this purpose. Running the file's actual source
  // (read from disk, not re-typed here) is what makes this a regression
  // test rather than a restatement — deleting term-bytes.js's `window`
  // assignment fails only this test, not the six above it.
  const source = fs.readFileSync(path.join(__dirname, "../assets/term-bytes.js"), "utf8");
  const sandbox = { window: {} };
  vm.createContext(sandbox);
  vm.runInContext(source, sandbox);

  assert.equal(typeof sandbox.window.farhelmTermBytes.binaryStringToBytes, "function");
  // `Array.from(...)`, not a direct `Uint8Array` comparison: the sandbox's
  // `Uint8Array` constructor is a DIFFERENT one from this test's own realm
  // (each `vm` context gets its own global built-ins), so `assert/strict`'s
  // reference-equality check on the prototype chain would fail even for
  // byte-identical arrays — a realm mismatch, not a bug in the code under
  // test. Flattening to a plain array sidesteps that entirely.
  assert.deepEqual(
    Array.from(sandbox.window.farhelmTermBytes.binaryStringToBytes("\x00\x7f\x80\xff")),
    [0x00, 0x7f, 0x80, 0xff],
  );
});
