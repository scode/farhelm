// The terminal's link opening and plain-text URL policy, shared by its two
// link adapters: xterm's built-in OSC 8 provider (a program emitting a real
// hyperlink escape) and the vendored WebLinks addon (a bare http(s) URL
// printed as plain text). Split out on its own for the same reason
// copy-on-select.js is — `node --test` runs the EXACT functions
// terminal.js calls, not hand-copied doubles that could silently drift
// from what ships — and terminal.js treats this file's global as a mount
// precondition, so no link can activate half-loaded.
//
// ## The opener is shared; the policy is not
//
// `openTerminalUrl` is deliberately dumb: it opens whatever URI its caller
// hands it, through whichever branch the page needs (main-webview
// navigation under `dioxus:`, where Dioxus intercepts the navigation into
// the system browser, else `window.open` with `_blank` + `noopener`). It
// does NO scheme filtering of its own. Each adapter owns its own input
// boundary instead: OSC 8 relies on xterm's `OscLinkProvider`, which
// already rejects non-HTTP(S) URIs unless `allowNonHttpProtocols` is set
// (it is not, and this file is not the place to change that), while the
// plain-text adapter — which turns ARBITRARY printed bytes into links —
// gets the explicit allowlist in `isPlainWebUrl`. Putting the policy in
// the opener would silently re-filter OSC 8 targets through a second,
// divergent rule; putting it in the plain-link adapter keeps each path's
// contract in exactly one place.
//
// ## What this does NOT do
//
// No fetching, no confirmation dialog, no async lookup, no detection
// service: opening stays inside the activation call stack. The `confirm()`
// xterm shows by default without a `linkHandler` is exactly what the tests
// pin against (see e2e/tests/terminal-links.spec.ts). And this file never
// DECIDES what text is a link — the WebLinks addon owns detection
// (its default matcher, whose http(s)-only boundaries were verified
// against the vendored bundle before wiring — lowercase or UPPERCASE
// schemes only, since the upstream regex carries no `i` flag);
// `isPlainWebUrl` is the second boundary at activation time, rejecting
// anything that is not an absolute http(s) URL with a host even if
// detection ever handed it over.
(function () {
  /**
   * Open a terminal link target in the page's system browser.
   *
   * Under the desktop webview (`dioxus:` origin) this navigates the page
   * itself, which Dioxus intercepts into a system-browser open — `window.open`
   * is a silent no-op there (verified by hand on the macOS build when the
   * OSC 8 handler was written). Everywhere else it opens a new tab with
   * `noopener`, so the target cannot reach back into this page.
   *
   * Synchronous and unconditional: callers must have validated `uri`
   * already (see the module header for which adapter owns which rule).
   * Never confirmation-gated — the absence of a dialog is asserted
   * coverage, not an oversight.
   *
   * @param {string} uri the already-validated link target
   */
  function openTerminalUrl(uri) {
    if (window.location.protocol === 'dioxus:') {
      window.location.assign(uri);
    } else {
      window.open(uri, '_blank', 'noopener');
    }
  }

  /**
   * Whether a plain-text link candidate may be opened as a web URL.
   *
   * Four checks, each closing a different hole, in an order that matters:
   *
   * 1. An explicit case-insensitive `http://`/`https://` prefix. Strips
   *    nothing, rewrites nothing: `javascript:`, `data:`, `file:`,
   *    `mailto:`, `ftp:`, protocol-relative `//host/path`, and malformed
   *    `http:` text all fail here, before parsing can be generous.
   * 2. No raw whitespace or control characters. The URL parser silently
   *    trims leading/trailing C0 controls and spaces and strips tabs and
   *    newlines ANYWHERE in the input, so this runs BEFORE parsing —
   *    otherwise a candidate the terminal visibly shows with a gap in it
   *    would open as a URL with the gap edited out.
   * 3. Absolute-URL parse with NO base URL. A base would let relative and
   *    protocol-relative strings acquire the app's own origin.
   * 4. A parsed `http:`/`https:` scheme AND a non-empty host. The scheme
   *    check is belt-and-braces beside (1) — parsing normalizes case and
   *    spelling, so re-checking after parse catches what the prefix regex
   *    reads differently than the parser — while the host check rejects
   *    `http:foo` and bare `http://` shapes the parser accepts as URLs.
   *
   * Anything failing any check is not clickable: terminal.js consults
   * this before opening, and rejected schemes must never be decorated
   * either (the addon's own http(s)-only matcher is the first boundary;
   * this is the second). The original string is preserved for opening —
   * this validates, never rewrites.
   *
   * @param {string} uri the link text the detector handed over
   * @returns {boolean} true to open `uri` unchanged via `openTerminalUrl`
   */
  function isPlainWebUrl(uri) {
    if (!/^https?:\/\//i.test(uri)) return false;
    if (/[\s\x00-\x1f\x7f]/.test(uri)) return false;
    let parsed;
    try {
      parsed = new URL(uri);
    } catch {
      return false;
    }
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return false;
    if (!parsed.host) return false;
    return true;
  }

  const api = { openTerminalUrl, isPlainWebUrl };
  if (typeof window !== 'undefined') window.farhelmTerminalLinks = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
