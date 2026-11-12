// Unit coverage for paste-sanitizer.js's paste sanitizing, run with node's
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
// matching paste-sanitizer.js's own `module.exports` branch, which is what most
// of the tests below exercise. The browser `window` branch gets its own
// coverage further down, via `node:vm`: nothing short of a direct check
// pins that the global assignment itself still exists.
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const fs = require("node:fs");
const path = require("node:path");
const { sanitizePastedText } = require("../assets/paste-sanitizer.js");

// Paste contents cannot close the bracketed-paste frame around them. In
// particular, deleting one marker must not join its neighbours into a new
// marker; ESC is stripped more broadly to keep that invariant without
// repeated rescanning.
test("pasted end markers and ESC fragments cannot become terminal framing", () => {
  assert.equal(sanitizePastedText("safe\x1b[201~evil\n"), "safeevil\n");
  assert.equal(sanitizePastedText("\x1b[201~a\x1b[201~b\x1b[201~"), "ab");
  assert.equal(sanitizePastedText("\x1b[20\x1b[201~1~"), "[201~");
  assert.equal(sanitizePastedText("\x1b[31mred\x1b[0m"), "[31mred[0m");
});

// Unicode, spaces and line breaks remain paste contents; the supervisor's
// paste encoding still decides newline normalization and whether the pane
// needs bracketed-paste framing.
test("ordinary pasted text is unchanged", () => {
  for (const text of ["", "/etc/hosts", "hello 世界 ❤️", "line\nnext\r\n\tindented"]) {
    assert.equal(sanitizePastedText(text), text);
  }
});

test("browser-global branch: window.farhelmPasteSanitizer exists with no module present", () => {
  // The tests above `require()` paste-sanitizer.js, which only ever exercises the
  // `module.exports` branch — `module` is always defined under `require()`.
  // `node:vm` fabricates the one environment shape that runs the `window`
  // branch: a fresh context with a `window` object and no `module` at all.
  // Running the file's actual source (read from disk, not re-typed here) is
  // what makes this a regression test rather than a restatement.
  const source = fs.readFileSync(path.join(__dirname, "../assets/paste-sanitizer.js"), "utf8");
  const sandbox = { window: {} };
  vm.createContext(sandbox);
  vm.runInContext(source, sandbox);

  assert.equal(typeof sandbox.window.farhelmPasteSanitizer.sanitizePastedText, "function");
  assert.equal(sandbox.window.farhelmPasteSanitizer.sanitizePastedText("a\x1b[201~b"), "ab");
});
