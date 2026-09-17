// Unit coverage for terminal-links.js's opener and plain-text URL filter,
// run with node's built-in test runner (the same shape as the other files
// in this directory: `node --test` loads the exact file the page ships,
// so these pin shipped behavior rather than a restatement of it).
//
// Two independent seams, not one restated twice:
//
// - `isPlainWebUrl` is the plain-TEXT adapter's activation boundary — the
//   allowlist that decides whether detector output may open. Every reject
//   below closes a real navigation hole (parser trimming, origin
//   acquisition, scheme smuggling), and each accept pins a shape that must
//   keep working; a regression here either opens something hostile or
//   silently un-links legitimate output, and neither failure would show up
//   in any other unit test.
// - `openTerminalUrl` is the shared opener both adapters call. Its web
//   branch is also covered by the browser suite, but its `dioxus:` branch
//   is unreachable there — no browser run can present that origin — so
//   these vm cases are the ONLY automated coverage of the desktop open
//   path and its exact call shape. They run through a node:vm fabricated
//   window (client-log-shim.test.js's pattern), because `require()` alone
//   cannot provide the `window` the shipped function reads.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { isPlainWebUrl } = require("../assets/terminal-links.js");

const SHIPPED_SOURCE = fs.readFileSync(
  path.join(__dirname, "..", "assets", "terminal-links.js"),
  "utf8",
);

// ---------------------------------------------------------------------------
// The plain-text filter: what may open.
// ---------------------------------------------------------------------------

test("plain http and https URLs open", () => {
  assert.equal(isPlainWebUrl("http://example.com/"), true);
  assert.equal(isPlainWebUrl("https://example.com/farhelm-e2e/123"), true);
});

test("uppercase scheme opens (prefix is case-insensitive, parser normalizes)", () => {
  assert.equal(isPlainWebUrl("HTTP://EXAMPLE.COM/X"), true);
  assert.equal(isPlainWebUrl("Https://Example.Com/Mixed"), true);
});

test("userinfo, ports, paths, queries, and fragments open unchanged", () => {
  assert.equal(isPlainWebUrl("https://user:pw@h.example/q?a=b#c"), true);
  assert.equal(isPlainWebUrl("http://127.0.0.1:7433/api/sessions"), true);
  assert.equal(isPlainWebUrl("http://localhost:8080/"), true);
  assert.equal(isPlainWebUrl("https://example.com/a_b-c~d"), true);
  assert.equal(isPlainWebUrl("https://example.com/a%20b"), true);
});

test("forbidden schemes never open, even with valid-looking hosts", () => {
  assert.equal(isPlainWebUrl("javascript:alert(1)"), false);
  assert.equal(isPlainWebUrl("data:text/plain,hi"), false);
  assert.equal(isPlainWebUrl("file:///etc/passwd"), false);
  assert.equal(isPlainWebUrl("mailto:a@b.c"), false);
  assert.equal(isPlainWebUrl("ftp://h.example/f"), false);
  // Case-smuggled variants fail the same prefix check.
  assert.equal(isPlainWebUrl("JaVaScRiPt:alert(1)"), false);
  assert.equal(isPlainWebUrl("DATA:text/html,<h1>x</h1>"), false);
});

test("protocol-relative and relative strings never open (no base to acquire)", () => {
  assert.equal(isPlainWebUrl("//host.example/path"), false);
  assert.equal(isPlainWebUrl("/just/a/path"), false);
  assert.equal(isPlainWebUrl("example.com/no-scheme"), false);
});

test("malformed http text never opens (no scheme, no host, or no slashes)", () => {
  assert.equal(isPlainWebUrl("http:foo"), false);
  assert.equal(isPlainWebUrl("http://"), false);
  assert.equal(isPlainWebUrl("https://"), false);
  assert.equal(isPlainWebUrl("http:/single-slash.example/"), false);
  assert.equal(isPlainWebUrl("http:example.com"), false);
  assert.equal(isPlainWebUrl(""), false);
  assert.equal(isPlainWebUrl("not a url at all"), false);
});

test("raw whitespace anywhere rejects, including where the parser would trim", () => {
  // The URL parser strips tabs/newlines anywhere and trims C0/space at the
  // edges, so these must fail BEFORE parsing — otherwise visibly-gapped
  // text would open with the gap silently edited out.
  assert.equal(isPlainWebUrl("https://example.com/a b"), false);
  assert.equal(isPlainWebUrl("https://example.com/a\tb"), false);
  assert.equal(isPlainWebUrl("https://example.com/a\nb"), false);
  assert.equal(isPlainWebUrl(" https://example.com/a"), false);
  assert.equal(isPlainWebUrl("https://example.com/a "), false);
  assert.equal(isPlainWebUrl("https://example.com/\u00a0nbsp"), false);
});

test("raw control characters reject", () => {
  assert.equal(isPlainWebUrl("https://example.com/a\x00b"), false);
  assert.equal(isPlainWebUrl("https://example.com/a\x1fb"), false);
  assert.equal(isPlainWebUrl("https://example.com/a\x7fb"), false);
  assert.equal(isPlainWebUrl("\x00https://example.com/a"), false);
});

// ---------------------------------------------------------------------------
// The shared opener: which call, on which origin.
// ---------------------------------------------------------------------------

/**
 * A fresh `node:vm` context shaped like just enough page for the shipped
 * opener: a `window` with a scripted `location` (protocol + assign
 * recorder) and a recording `open`. Nothing else the function touches.
 */
function createWindow(protocol) {
  const openCalls = [];
  const assignCalls = [];
  const sandbox = {
    window: {
      location: {
        protocol: protocol,
        assign: function (uri) {
          assignCalls.push(uri);
        },
      },
      open: function (uri, target, features) {
        openCalls.push({ uri: uri, target: target, features: features });
        return null;
      },
    },
  };
  sandbox.window.window = sandbox.window;
  vm.createContext(sandbox);
  vm.runInContext(SHIPPED_SOURCE, sandbox);
  return {
    links: sandbox.window.farhelmTerminalLinks,
    openCalls: openCalls,
    assignCalls: assignCalls,
  };
}

test("the shipped file installs its global on a fabricated window", () => {
  const box = createWindow("https:");
  assert.equal(typeof box.links.openTerminalUrl, "function");
  assert.equal(typeof box.links.isPlainWebUrl, "function");
});

test("on the web the opener uses window.open with _blank and noopener", () => {
  for (const protocol of ["https:", "http:"]) {
    const box = createWindow(protocol);
    box.links.openTerminalUrl("https://example.com/x?y=1#z");
    assert.deepEqual(box.openCalls, [
      { uri: "https://example.com/x?y=1#z", target: "_blank", features: "noopener" },
    ]);
    assert.deepEqual(box.assignCalls, []);
  }
});

test("under dioxus: the opener navigates the page itself for interception", () => {
  // `window.open` is a silent no-op in the desktop webview (no new-window
  // handler), so this branch MUST NOT call it — navigating is the whole
  // desktop path, and Dioxus's navigation handler refuses the navigation
  // itself after handing the URL to the system browser.
  const box = createWindow("dioxus:");
  box.links.openTerminalUrl("https://example.com/desktop-path");
  assert.deepEqual(box.assignCalls, ["https://example.com/desktop-path"]);
  assert.deepEqual(box.openCalls, []);
});

test("the opener passes the URI through unrewritten on both branches", () => {
  const uri = "HTTP://EXAMPLE.COM:8080/Mixed?b=C#D";
  const web = createWindow("https:");
  web.links.openTerminalUrl(uri);
  assert.equal(web.openCalls[0].uri, uri);
  const desktop = createWindow("dioxus:");
  desktop.links.openTerminalUrl(uri);
  assert.equal(desktop.assignCalls[0], uri);
});
