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
const {
  isPlainWebUrl,
  linkTargetParts,
  shortenedTarget,
  displayedTarget,
  linkTextMismatch,
  linkRowText,
  visibleFormatCharacters,
} = require("../assets/terminal-links.js");

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

// Why this matters: an OSC 8 link's underlined text is whatever a program
// printed, so the hover display is the only place a user sees where a click
// really goes. Spec: the parts come from the parsed URL, the host a browser
// would contact is the emphasized part (credentials before `@` are not the
// host), an internationalized host shows as punycode, and an unparsable
// target is shown whole without emphasis.
test("linkTargetParts emphasizes the host a browser would actually contact", () => {
  const host = (uri) => linkTargetParts(uri).filter((part) => part.host).map((part) => part.text);
  const joined = (uri) => linkTargetParts(uri).map((part) => part.text).join("");

  assert.deepEqual(host("https://github.com/scode/farhelm?x=1#y"), ["github.com"]);
  assert.equal(joined("https://github.com/scode/farhelm?x=1#y"), "https://github.com/scode/farhelm?x=1#y");
  assert.deepEqual(host("https://github.com@evil.example/login"), ["evil.example"]);
  assert.equal(joined("https://github.com@evil.example/login"), "https://github.com@evil.example/login");
  assert.deepEqual(host("https://ex\u0430mple.com/"), ["xn--exmple-4nf.com"]);
  assert.deepEqual(linkTargetParts("not a url"), [{ text: "not a url", host: false }]);
  assert.equal(joined("https://:secret@example.com/path"), "https://:secret@example.com/path");
  assert.deepEqual(host("https://:secret@example.com/path"), ["example.com"]);
});

// Why this matters: a target thousands of characters long (credentials
// padding before `@host`, say) must not push what a click contacts out of
// the hover display. Spec: short targets are shown whole; long ones keep
// their start and end around a marker that says how many characters were
// left out, and the display stays bounded (the host has its own line).
test("shortenedTarget keeps short targets whole and bounds long ones honestly", () => {
  assert.equal(shortenedTarget("https://example.com/a"), "https://example.com/a");
  const long = `https://${"a".repeat(5000)}@evil.example/path`;
  const shown = shortenedTarget(long);
  assert.ok(shown.length < 400, shown.length);
  assert.ok(shown.startsWith("https://aaaa"));
  assert.ok(shown.endsWith("@evil.example/path"));
  assert.match(shown, /…\[\d+ characters\]…/);
});

// Why this matters: the hover line promises the exact target, and an empty
// query or fragment is part of it (it can change what a server does). Spec:
// the displayed target keeps a trailing `?`, `#` or `?#`, and shows the
// host in the punycode form a browser contacts.
test("displayedTarget keeps empty query and fragment delimiters", () => {
  assert.equal(displayedTarget("https://example.com/path?"), "https://example.com/path?");
  assert.equal(displayedTarget("https://example.com/path#"), "https://example.com/path#");
  assert.equal(displayedTarget("https://example.com/path?#"), "https://example.com/path?#");
  assert.equal(displayedTarget("https://ex\u0430mple.com/"), "https://xn--exmple-4nf.com/");
  assert.equal(displayedTarget("not a url"), "not a url");
});

// Why this matters: the loud hover warning exists to catch a link whose
// underlined text is a web address while the link goes somewhere else; if
// it fired on ordinary link text or on trailing-slash differences, people
// would learn to ignore it. Spec: only URL-like text (a scheme, `www.`, or a
// dotted host followed by `/`) is judged; it matches when the parsed URLs
// are equal ignoring one trailing slash, with scheme-less text borrowing the
// target's scheme; a prefix of the target, a different path, and URL-like
// text that does not parse all mismatch; file names and plain words never
// do.
test("linkTextMismatch flags only URL-like text that names another place", () => {
  // Matches, including the trailing-slash and normalization cases.
  assert.equal(linkTextMismatch("http://127.0.0.1:6080", "http://127.0.0.1:6080/"), false);
  assert.equal(linkTextMismatch("https://example.com/docs", "https://example.com/docs/"), false);
  assert.equal(linkTextMismatch("HTTPS://Example.COM/x", "https://example.com/x"), false);
  assert.equal(linkTextMismatch("example.com/x", "https://example.com/x"), false);
  assert.equal(linkTextMismatch("www.example.com", "https://www.example.com/"), false);
  assert.equal(linkTextMismatch("  https://example.com/x  ", "https://example.com/x"), false);
  // Mismatches.
  assert.equal(linkTextMismatch("https://github.com", "https://evil.example/"), true);
  assert.equal(linkTextMismatch("https://good.example", "https://good.example.evil.test/"), true);
  assert.equal(linkTextMismatch("https://example.com/a", "https://example.com/b"), true);
  assert.equal(linkTextMismatch("docs.rs/foo", "https://evil.example/foo"), true);
  assert.equal(linkTextMismatch("http://example.com/x", "https://example.com/x"), true);
  assert.equal(linkTextMismatch("https://exa mple.com/", "https://example.com/"), true);
  // Lookalike schemes and invisible leading characters claim an address but
  // do not parse as one, so they warn rather than slipping past the gate.
  assert.equal(linkTextMismatch("htt\u0440s://github.com/login", "https://evil.example/login"), true);
  assert.equal(linkTextMismatch("\u2800https://github.com/login", "https://evil.example/login"), true);
  assert.equal(linkTextMismatch("\u3164https://github.com/login", "https://evil.example/login"), true);
  // Lookalike separators and blank-drawing letters are read through for the
  // decision to judge, then strict parsing warns.
  for (const text of [
    "https\u02D0//github.com/login",
    "https\uFF1A//github.com/login",
    "https:\u2215\u2215github.com",
    "github\u3002com/login",
    "github.com\u2215login",
    "\u3164 https://github.com/login",
    "\uFFA0 https://github.com/login",
  ]) {
    assert.equal(linkTextMismatch(text, "https://evil.example/login"), true, text);
  }
  // Percent-escape case alone is not a different place.
  assert.equal(linkTextMismatch("https://example.com/%c3%bc", "https://example.com/%C3%BC"), false);
  // The trailing-slash allowance covers the path only.
  assert.equal(linkTextMismatch("https://example.com/?next=/", "https://example.com/?next="), true);
  assert.equal(linkTextMismatch("https://example.com/a#/", "https://example.com/a#"), true);
  // Not URL-like: never judged, whatever the target.
  for (const text of [
    "click here",
    "#123",
    "main.rs",
    "setup.py",
    "src/main.rs",
    "src/main.rs:12",
    "./foo.d/x",
    "changelog.d/x.md",
    "v1.2/CHANGELOG.md",
    "1.2.3/",
    "",
    null,
    undefined,
  ]) {
    assert.equal(linkTextMismatch(text, "https://evil.example/"), false, String(text));
  }
});

// Why this matters: the comparison is only as good as the text it is given.
// Spec: the hovered row's cells from the range's start to its end column
// (1-based, inclusive) are returned; a range that runs past the row is cut at
// the row's end (the provider builds one link per row); no range, or a row
// that no longer exists, gives null so the caller keeps the quiet display.
test("linkRowText reads the underlined cells of the hovered row", () => {
  const rows = ["0123 https://example.com/x tail"];
  const term = {
    buffer: {
      active: {
        getLine(y) {
          const text = rows[y];
          if (text === undefined) return undefined;
          return {
            length: text.length,
            translateToString(_trimRight, start, end) {
              return text.slice(start, end);
            },
          };
        },
      },
    },
  };
  const range = { start: { x: 6, y: 1 }, end: { x: 26, y: 1 } };
  assert.equal(linkRowText(term, range), "https://example.com/x");
  // Guard only: the vendored provider never reports a range spanning rows.
  assert.equal(linkRowText(term, { start: { x: 6, y: 1 }, end: { x: 3, y: 2 } }), "https://example.com/x tail");
  assert.equal(linkRowText(term, undefined), null);
  assert.equal(linkRowText(term, { start: { x: 1, y: 9 }, end: { x: 4, y: 9 } }), null);
});

// Why this matters: the warning asks the user to compare the link's text
// with its target, and the display is HTML, where a right-to-left override
// or other format character the terminal stored but did not apply would
// reorder or hide part of the text being compared. Spec: every Unicode
// format character (general category Cf) becomes a visible <U+XXXX> escape;
// everything else is left as it is.
test("visibleFormatCharacters makes bidi controls and zero-width characters visible", () => {
  assert.equal(visibleFormatCharacters("https://a.example/"), "https://a.example/");
  assert.equal(visibleFormatCharacters("abc\u202Edef"), "abc<U+202E>def");
  assert.equal(visibleFormatCharacters("a\u200Bb\u2066c"), "a<U+200B>b<U+2066>c");
  assert.equal(visibleFormatCharacters("b\u00fccher.de"), "b\u00fccher.de");
});
