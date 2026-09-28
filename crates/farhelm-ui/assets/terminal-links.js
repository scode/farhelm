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

  /**
   * The parts of an OSC 8 link target to show on hover, host flagged for
   * emphasis.
   *
   * An OSC 8 hyperlink's underlined text is whatever the program printed and
   * need not resemble its target, so hovering shows the target itself (see
   * SPEC.md, Terminal experience); the click still opens directly, with no
   * confirmation. The parts come from the PARSED URL rather than the raw
   * string, so what is emphasized is the host a browser would actually
   * contact: `https://github.com@evil.example/` shows `evil.example`, and an
   * internationalized host shows its punycode form, which cannot pose as a
   * lookalike of an ASCII name. A target that does not parse is shown whole
   * and unemphasized.
   *
   * @param {string} uri
   * @returns {{text: string, host: boolean}[]}
   */
  function linkTargetParts(uri) {
    let parsed;
    try {
      parsed = new URL(uri);
    } catch {
      return [{ text: uri, host: false }];
    }
    if (!parsed.host) return [{ text: uri, host: false }];
    // Either half of the userinfo keeps the `@` form, so a password-only
    // target (`https://:secret@host/`) is shown exactly, not silently
    // shortened.
    const credentials =
      parsed.username || parsed.password
        ? `${parsed.username}${parsed.password ? `:${parsed.password}` : ''}@`
        : '';
    return [
      { text: `${parsed.protocol}//${credentials}`, host: false },
      { text: parsed.host, host: true },
      { text: `${parsed.pathname}${parsed.search}${parsed.hash}`, host: false },
    ];
  }

  /**
   * The full target line: the URL's own serialization when it parses (so an
   * empty trailing `?` or `#`, which the component getters drop, is still
   * shown, and the host appears in the punycode form it is contacted by),
   * else the raw string.
   *
   * @param {string} uri
   * @returns {string}
   */
  function displayedTarget(uri) {
    try {
      return new URL(uri).href;
    } catch {
      return uri;
    }
  }

  // Targets longer than this are shown shortened in the middle, with the
  // number of omitted characters said out loud. The host is always shown in
  // full on its own line, so what a click would contact is never the part
  // that gets cut.
  const TARGET_DISPLAY_LIMIT = 300;

  /**
   * The target text to show under the host line: the whole target when it
   * is short, otherwise its start and end around an explicit marker naming
   * how much was left out.
   *
   * @param {string} text
   * @returns {string}
   */
  function shortenedTarget(text) {
    if (text.length <= TARGET_DISPLAY_LIMIT) return text;
    const keep = Math.floor((TARGET_DISPLAY_LIMIT - 20) / 2);
    const omitted = text.length - 2 * keep;
    return `${text.slice(0, keep)} …[${omitted} characters]… ${text.slice(-keep)}`;
  }

  /**
   * Show `uri` (an OSC 8 link's target) near the pointer while it hovers the
   * link. See `linkTargetParts` for what is shown and why.
   *
   * The display lives INSIDE `owner`, the hovered terminal's own root
   * element: it is hidden or removed together with that terminal (tab
   * switch, reconnect, restored snapshot, disposal) without any teardown
   * hook, and one terminal's lifecycle can never touch another terminal's
   * display. Built with `textContent` only, so program output never
   * becomes markup. The first line is the host alone (or the whole target
   * when it has no host); the second is the target, shortened in the middle
   * when it is very long.
   *
   * @param {MouseEvent} event
   * @param {string} uri
   * @param {HTMLElement} owner
   */
  function showLinkTarget(event, uri, owner) {
    let display = owner.querySelector(':scope > .terminal-link-target');
    if (!display) {
      display = document.createElement('div');
      display.className = 'terminal-link-target';
      display.setAttribute('role', 'tooltip');
      display.dir = 'ltr';
      owner.appendChild(display);
    }
    const parts = linkTargetParts(uri);
    const hostPart = parts.find((part) => part.host);
    const lines = [];
    if (hostPart) {
      const hostLine = document.createElement('div');
      const strong = document.createElement('strong');
      strong.textContent = hostPart.text;
      hostLine.appendChild(strong);
      lines.push(hostLine);
    }
    const targetLine = document.createElement('div');
    targetLine.className = 'terminal-link-target-url';
    targetLine.textContent = shortenedTarget(displayedTarget(uri));
    lines.push(targetLine);
    display.replaceChildren(...lines);
    // Place it below-right of the pointer, then pull it back inside the
    // viewport: a link on the last rows or near the right edge would
    // otherwise put the target where the page cannot scroll to it. Flips
    // above the pointer when there is no room below.
    display.hidden = false;
    display.style.left = '0px';
    display.style.top = '0px';
    const box = display.getBoundingClientRect();
    const margin = 4;
    let left = event.clientX + 12;
    let top = event.clientY + 16;
    if (left + box.width > window.innerWidth - margin) {
      left = window.innerWidth - margin - box.width;
    }
    if (top + box.height > window.innerHeight - margin) {
      top = event.clientY - 8 - box.height;
    }
    display.style.left = `${Math.max(margin, left)}px`;
    display.style.top = `${Math.max(margin, top)}px`;
  }

  /**
   * Hide `owner`'s hover display when the pointer leaves the link.
   *
   * @param {HTMLElement} owner
   */
  function hideLinkTarget(owner) {
    const display = owner.querySelector(':scope > .terminal-link-target');
    if (display) display.hidden = true;
  }

  const api = {
    openTerminalUrl,
    isPlainWebUrl,
    linkTargetParts,
    shortenedTarget,
    displayedTarget,
    showLinkTarget,
    hideLinkTarget,
  };
  if (typeof window !== 'undefined') window.farhelmTerminalLinks = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
