const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

/**
 * Exercise the shipped painter before and after its optional script loads.
 * Reconciliation can explain a withheld attachment before any terminal mounts;
 * peer notices arrive only after mounting has established the helper dependency.
 */
test("terminal banners tolerate the local pre-mount notice and escape peer text once", () => {
  const source = fs.readFileSync(path.join(__dirname, "../assets/terminal.js"), "utf8");
  const start = source.indexOf("  function paintBanner(");
  const end = source.indexOf("  function browserHandlesDrop(", start);
  assert.ok(start >= 0 && end > start, "the test must execute the shipped banner painter");
  const banner = { textContent: "", style: {}, appendChild() {} };
  const window = {};
  const document = {
    getElementById: () => banner,
    createElement: () => ({ dataset: {}, addEventListener() {} }),
  };
  const paint = new Function("window", "document", `${source.slice(start, end)}; return paintBanner;`)(window, document);
  paint("banner", "Detached: automatic attachment unavailable", true);
  assert.equal(banner.textContent, "Detached: automatic attachment unavailable");
  assert.equal(banner.style.display, "block");
  window.farhelmTerminalLinks = require("../assets/terminal-links.js");
  const reason = "Detached: a\u202Eb\u200B <U+202E>";
  for (let repaint = 0; repaint < 2; repaint++) {
    paint("banner", reason, true);
    assert.equal(banner.textContent, "Detached: a<U+202E>b<U+200B> <U+003C>U+202E>");
  }
});
