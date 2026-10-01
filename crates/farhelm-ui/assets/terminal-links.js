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

  // Underlined text that claims to be a web address: a scheme followed by
  // `://`, a `www.` prefix, or a dotted host whose last label is two or more
  // letters followed by a slash (`docs.rs/foo`). Deliberately not "anything
  // shaped like a domain": file names such as `main.rs`, `setup.py` or
  // `notes.md` are valid domain names under real country-code TLDs, and
  // agents print exactly that kind of link text, so a looser test would put
  // the loud warning on ordinary file links. The letters-only last label
  // keeps `v1.2/CHANGELOG.md`, `changelog.d/x.md` and `1.2.3/` quiet too; the
  // cost is that scheme-less IP text such as `127.0.0.1:6080/x` is not
  // judged (with a scheme it is).
  //
  // This test only decides whether the text CLAIMS to be an address;
  // whether the claim holds is decided by strict parsing afterwards, which
  // fails (and so warns) on anything that is not really one. So the test is
  // generous where a forger would be creative: its scheme part accepts any
  // characters (a Cyrillic `httрs`), it looks past leading non-ASCII
  // characters that are not letters or digits and past the Hangul fillers,
  // which count as letters but draw as blanks (a Braille blank, U+3164), and
  // it reads lookalike separators as their ASCII originals (`ː` for `:`,
  // `∕` for `/`, `。` for `.`; see `GATE_LOOKALIKES`). An ASCII-only test
  // let all of these through quietly. Only the start of the text is
  // examined: link text that is prose containing an address
  // ("Visit https://...") is not judged, and the quiet display, which names
  // the real host, is what remains for it.
  const URL_LIKE_TEXT = /^(?:[^\s/:]+:\/\/|www\.|[^\s/]+\.\p{L}{2,}(?::\d+)?\/)/iu;
  const LEADING_BLANKS = /^(?:[^\p{L}\p{N}\x00-\x7F]|[\u115F\u1160\u3164\uFFA0]|\s)+/u;
  const GATE_LOOKALIKES = new Map([
    ...['\u02D0', '\u02F8', '\u2236', '\uA789', '\uFF1A', '\uFE13', '\uFE55'].map((c) => [c, ':']),
    ...['\u2215', '\u2044', '\u29F8', '\uFF0F', '\u2571'].map((c) => [c, '/']),
    ...['\u3002', '\uFF0E', '\uFF61', '\u2024'].map((c) => [c, '.']),
  ]);

  /**
   * Whether `text` claims to be a web address, per `URL_LIKE_TEXT` and the
   * generosity rules above it. Used only to decide whether to judge the
   * text; never to parse it.
   *
   * @param {string} text
   * @returns {boolean}
   */
  function claimsAddress(text) {
    const gated = Array.from(text.replace(LEADING_BLANKS, ''), (c) => GATE_LOOKALIKES.get(c) ?? c).join('');
    return URL_LIKE_TEXT.test(gated);
  }

  /**
   * A URL's serialization with one trailing slash removed from its PATH, so
   * `/docs` and `/docs/` (or a bare origin and the `/` the parser adds to
   * it) compare equal. That difference is common and almost never matters,
   * and flagging it would teach people to ignore the warning. A slash at the
   * end of a query or fragment is left alone: `?next=/` is not `?next=`.
   * Percent escapes are compared case-insensitively (`%c3%bc` is `%C3%BC`),
   * since the parser keeps them as written. An empty query or fragment
   * (`https://a.example?`) compares equal to none, unlike in
   * `displayedTarget`, which keeps the delimiter because it shows the exact
   * target; here only the place matters.
   *
   * @param {URL} url
   * @returns {string}
   */
  function comparableHref(url) {
    const path = url.pathname.endsWith('/') ? url.pathname.slice(0, -1) : url.pathname;
    const href = `${url.protocol}//${url.username}:${url.password}@${url.host}${path}${url.search}${url.hash}`;
    return href.replace(/%[0-9a-f]{2}/gi, (escape) => escape.toUpperCase());
  }

  /**
   * Whether an OSC 8 link's underlined `text` names a different place than
   * its target `uri`, in which case the hover display turns into a loud
   * warning (SPEC.md, Terminal experience).
   *
   * Only URL-like text is judged (see `URL_LIKE_TEXT`): "click here",
   * "#123" or a file name never matches a URL, but it is not the lookalike
   * trick the warning exists for, and such links keep the quiet display.
   * URL-like text is parsed and compared with the parsed target, ignoring
   * only a trailing slash; text without a scheme borrows the target's, so
   * `example.com/x` matches `https://example.com/x`. Text that is merely a
   * prefix of the target (`https://good.example` over
   * `https://good.example.evil.test`) is a mismatch, as is URL-like text
   * that does not parse.
   *
   * `text` is the hovered row's part of the link only: a link whose text
   * wraps onto another row is judged on a fragment. A URL-like fragment
   * never equals the whole target, so an honest wrapped URL gets a false
   * warning. That is intentional. Joining the rows would need xterm's
   * private internals (its public buffer API does not say which cells
   * belong to a link), and agent TUIs wrap with cursor movement, so the next
   * row is not even marked as a continuation. Erring loud keeps a fragment
   * from ever being judged a MATCH. It does not make the check complete: a
   * fragment that is not URL-like on its own (`https:/` on one row,
   * `/github.com/login` on the next) is not judged at all, so a program that
   * chooses its own line breaks can keep every row of a lookalike on the
   * quiet display. That display still names the real host, which is the
   * safeguard this warning adds to rather than replaces.
   *
   * @param {string | null | undefined} text
   * @param {string} uri
   * @returns {boolean}
   */
  function linkTextMismatch(text, uri) {
    const claimed = (text || '').trim();
    if (!claimsAddress(claimed)) return false;
    let target;
    try {
      target = new URL(uri);
    } catch {
      return true;
    }
    let named;
    try {
      named = /^[^\s/:]+:\/\//u.test(claimed)
        ? new URL(claimed)
        : new URL(`${target.protocol}//${claimed}`);
    } catch {
      return true;
    }
    return comparableHref(named) !== comparableHref(target);
  }

  /**
   * `text` with Unicode format characters (bidi controls, zero-width
   * characters and the like) replaced by visible `<U+XXXX>` escapes.
   *
   * The terminal stores these characters without applying them, but the
   * hover display is ordinary HTML, where a right-to-left override would
   * reorder the very text the user is asked to compare. Showing them as
   * escapes makes the hidden characters themselves visible. Right-to-left
   * letters would still reorder the line by themselves; the stylesheet
   * forces that line left to right (`.terminal-link-target-text`), the
   * order the terminal draws it in.
   *
   * @param {string} text
   * @returns {string}
   */
  function visibleFormatCharacters(text) {
    return text.replace(/\p{Cf}/gu, (ch) =>
      `<U+${ch.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')}>`,
    );
  }

  /**
   * The text an OSC 8 link underlines on the hovered row.
   *
   * `range` is what xterm's OSC 8 link provider passes as the third hover
   * argument: 1-based cell coordinates of the link on one buffer row (the
   * provider builds one link per row; see `linkTextMismatch` for what that
   * means for wrapped links). Returns null when there is no range or the
   * row is gone, so the caller falls back to the quiet display.
   *
   * @param {{buffer: {active: {getLine(y: number): any}}}} term
   * @param {{start: {x: number, y: number}, end: {x: number, y: number}} | undefined} range
   * @returns {string | null}
   */
  function linkRowText(term, range) {
    if (!range || !range.start || !range.end) return null;
    const line = term.buffer.active.getLine(range.start.y - 1);
    if (!line) return null;
    // The vendored provider always reports one row; a range that ends on a
    // later row is cut at this row's end, a guard against a future
    // provider that spans rows rather than something seen today.
    const end = range.end.y === range.start.y ? range.end.x : line.length;
    return line.translateToString(true, range.start.x - 1, end);
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
   * When `text`, the link's underlined text on the hovered row, is URL-like
   * and names somewhere else (`linkTextMismatch`), the display becomes a
   * loud warning instead: danger-styled and larger, led by a line saying
   * the text does not match where the link goes, and showing the text
   * beside the real target so the difference is visible before a click.
   *
   * @param {MouseEvent} event
   * @param {string} uri
   * @param {HTMLElement} owner
   * @param {string | null} [text]
   */
  function showLinkTarget(event, uri, owner, text) {
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
    const mismatch = linkTextMismatch(text, uri);
    display.classList.toggle('terminal-link-target-mismatch', mismatch);
    const lines = [];
    if (mismatch) {
      const warning = document.createElement('div');
      warning.className = 'terminal-link-target-warning';
      warning.textContent = 'link text does not match where it goes';
      lines.push(warning);
      const claimed = document.createElement('div');
      claimed.className = 'terminal-link-target-text';
      claimed.textContent = `text says: ${shortenedTarget(visibleFormatCharacters(text.trim()))}`;
      lines.push(claimed);
      const goes = document.createElement('div');
      goes.textContent = 'actually goes to:';
      lines.push(goes);
    }
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
    linkTextMismatch,
    linkRowText,
    visibleFormatCharacters,
    shortenedTarget,
    displayedTarget,
    showLinkTarget,
    hideLinkTarget,
  };
  if (typeof window !== 'undefined') window.farhelmTerminalLinks = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
