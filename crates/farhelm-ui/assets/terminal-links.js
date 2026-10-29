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
// `openTerminalUrl` opens its caller-validated URI through main-webview
// navigation under `dioxus:`, else window.open with noopener. It performs
// no scheme filtering. terminal.js's OSC handler explicitly permits only
// http(s) here and routes file targets to terminal-files.js; every other
// scheme is ignored. The plain WebLinks adapter has its own stricter
// isPlainWebUrl boundary. File lookup/transfer never reaches this opener.
//
// ## What this does NOT do
//
// Ordinary opening does no fetching, confirmation or async lookup and stays
// inside the activation stack. Native remote Meta-click instead hands an
// explicit tunnel intent to Rust, which owns consent and the async request.
// This asset never sends tunnel HTTP requests. The `confirm()`
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

  /** The tunnel's narrow loopback target, after the same URL normalization as Rust.
   * No DNS lookup and no LAN/local-network guess: only localhost names, 127/8,
   * ::1 and the two wildcard bind addresses qualify. Zero is not a service port.
   * Ordinary terminal opening deliberately retains its existing broader URL rule.
   */
  function tunnelTarget(uri) {
    if (typeof uri !== 'string' || !isPlainWebUrl(uri) || new TextEncoder().encode(uri).length > 4096) return null;
    const parsed = new URL(uri);
    const port = parsed.port === '' ? (parsed.protocol === 'https:' ? 443 : 80) : Number(parsed.port);
    if (!port) return null;
    const host = parsed.hostname.toLowerCase().replace(/\.+$/, '');
    const loopback = host === 'localhost' || host.endsWith('.localhost') ||
      /^127\.\d+\.\d+\.\d+$/.test(host) || host === '0.0.0.0' ||
      host === '[::1]' || host === '[::]';
    return loopback ? { port } : null;
  }

  /** Admit the one new gesture after the caller's actual click/selection guards.
   * Extra modifiers retain their established selection/alternate-click meanings.
   */
  function shouldTunnel({ uri, native, remote, button, metaKey, ctrlKey, shiftKey, altKey, click, selection, mode }) {
    return !!(native && remote && button === 0 && metaKey && !ctrlKey && !shiftKey && !altKey &&
      click && !selection && mode !== 'program' && tunnelTarget(uri));
  }

  /** Both web-link providers route through this decision, so a tunnel can never
   * fall through to raw opening. The injected bridge reports a missing native
   * channel through the terminal's existing status surface. `validated` is
   * reserved for OSC's caller-owned http(s) policy; prose always uses the
   * stricter plain-text predicate. Tunneling never waives that strict rule.
   */
  function activateWebLink(options, open, tunnel) {
    if (!options.click || options.selection || options.mode === 'program' ||
      (!options.validated && !isPlainWebUrl(options.uri))) return 'ignored';
    if (shouldTunnel(options)) { tunnel(options.uri); return 'tunnel'; }
    open(options.uri);
    return 'raw';
  }

  /** Admit only the release of a primary press on this terminal's screen.
   * Mouse-reporting programs suppress local selection, so selection alone
   * cannot distinguish a click from a drag. Four CSS pixels tolerate hand
   * jitter; crossing that distance retires the gesture even if it returns
   * to the press. Document capture observes the whole gesture before xterm
   * activates a link, including releases outside the terminal. The exact
   * release event is the admission token, never a sticky permission.
   *
   * The drag check is only as good as the observer's coverage: a press is
   * judged by the movement this object saw, so a press must not outlive the
   * listeners that feed it. Whoever removes those listeners calls `reset()`.
   * Reconnect is the case that needs it: a recovery that ends by restoring
   * the old screen hands this same object to new listeners, and a press
   * kept across the gap would let a drag away and back during the gap
   * release as a click. `reset()` clears state in place rather than handing
   * out a new object, because the terminal's link providers captured this
   * one at mount and keep asking it `isClick`.
   */
  function createLinkGesture() {
    let press = null;
    let click = null;
    return {
      observe(event, onScreen) {
        if (event.type === 'mousedown' && event.button === 0) {
          click = null;
          press = onScreen ? { x: event.clientX, y: event.clientY, dragged: false } : null;
        }
        if (!press) return;
        if (event.type === 'mousemove' || (event.type === 'mouseup' && event.button === 0)) {
          press.dragged ||= Math.hypot(event.clientX - press.x, event.clientY - press.y) > 4;
        }
        if (event.type === 'mouseup' && event.button === 0) {
          click = press.dragged ? null : event;
          press = null;
        }
      },
      isClick(event) { return event === click; },
      /** Forget any held press and any admitted release; the next click
       * has to start with a press observed from here on. */
      reset() {
        press = null;
        click = null;
      },
    };
  }

  /** Decide whether this press belongs to Farhelm's web link gesture.
   * Forced local selection keeps xterm's own path. File links never qualify:
   * their broad path detection must not steal ordinary transcript input.
   * The press is withheld as a whole, including any drag and release it owns;
   * no later motion replays a press the program never received.
   */
  function interceptLinkPress({ button, tracking, forced, webLink, mode }) {
    return button === 0 && tracking && !forced && webLink && mode === 'intercept';
  }

  /** Suppress only a complete mouse report emitted during its owned DOM event.
   * xterm emits one report synchronously through its public output callbacks.
   * A captured event remains in dispatch until eventPhase becomes NONE (0);
   * unlike a microtask, that boundary cannot expire between native listeners.
   * Stale events, focus reports, keyboard input and pasted lookalikes pass.
   */
  function suppressMouseReport(data, dispatch) {
    return !!(dispatch && dispatch.withheld && dispatch.event.eventPhase !== 0 &&
      (/^\x1b\[M[\s\S]{3}$/.test(data) || /^\x1b\[<\d+;\d+;\d+[Mm]$/.test(data)));
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
   * Web hover normally supplies the complete tagged text via linkHoverText.
   * If xterm's private tag is unavailable, hover falls back to the row-only
   * comparison. An incomplete join is handled separately by showLinkTarget:
   * a fragment that happens to equal the target must not authorize quietness.
   * Untagged text around a link is outside this comparison; the quiet display
   * still names the real host and never certifies all surrounding output.
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
   * Show presentation-unsafe characters as `<U+XXXX>` and escape the `<`
   * of literal `<U+` text, so a filename cannot impersonate that notation.
   *
   * The terminal stores these characters without applying them, but the
   * hover display is ordinary HTML, where a right-to-left override would
   * reorder the very text the user is asked to compare. Showing them as
   * escapes makes the hidden characters themselves visible. The unsafe set
   * mirrors `farhelm-proto/src/text.rs::is_presentation_unsafe`, plus all Cf
   * characters already covered here. Keep both policies in sync; variation
   * selectors remain readable. Right-to-left
   * letters would still reorder the line by themselves; the stylesheet
   * forces that line left to right (`.terminal-link-target-text`), the
   * order the terminal draws it in.
   *
   * @param {string} text
   * @returns {string}
   */
  function visibleFormatCharacters(text) {
    return text.replace(/<(?=U\+)|[\p{Cc}\p{Cf}\u034F\u115F\u1160\u17B4-\u17B5\u2028\u2029\u3164\uFFA0\u{E0000}-\u{E007F}]/gu, (ch) =>
      `<U+${ch.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')}>`,
    );
  }

  /**
   * The text an OSC 8 link underlines on the hovered row.
   *
   * `range` is what xterm's OSC 8 link provider passes as the third hover
   * argument: 1-based cell coordinates of the link on one buffer row (the
   * provider builds one link per row). File hover uses this only as an identity
   * snapshot; web hover uses linkHoverText to join tagged rows. Returns null
   * when there is no range or the row is gone, so the caller falls back to the
   * quiet display.
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

  /** Join consecutive rows carrying this OSC 8 tag, including agent-drawn wraps.
   * Agents often move the cursor instead of letting xterm wrap, so isWrapped
   * cannot establish the link's extent. The public getCell returns an internal
   * cell whose undocumented extended.urlId is read-only here. A real-bundle
   * browser test pins it; losing the field restores the old row comparison.
   *
   * Only one run per row may join. Bounds or ambiguous runs produce a fragment
   * only when an adjacent row carries the tag: repeated links on a single row
   * must retain their ordinary hover behavior. Spaces within each tagged run
   * survive joining, since a space at a row edge can change the named target.
   * Only xterm's empty pad before a terminal-wrapped wide character is omitted.
   * No text outside the tagged cells is evidence for a match.
   * Adjacent complete addresses may reuse an id. Compare the hovered run alone
   * only when every run independently names the target within the same bounds.
   * @param {{buffer: {active: {getLine(y: number): any}}}} term
   * @param {{start: {x: number, y: number}, end: {x: number, y: number}} | undefined} range
   * @param {string | undefined} uri Target for independent-row comparison.
   * @returns {{kind: 'whole' | 'fragment' | 'unknown', text: string | null}}
   */
  function linkHoverText(term, range, uri) {
    const fragment = linkRowText(term, range);
    const unknown = { kind: 'unknown', text: fragment };
    if (fragment === null) return unknown;
    const buffer = term.buffer.active;
    const y = range.start.y - 1;
    const line = buffer.getLine(y);
    const cell = line?.getCell?.(range.start.x - 1);
    const tag = cell?.extended?.urlId;
    if (!Number.isInteger(tag) || tag <= 0) return unknown;

    // xterm only replaces extended when a cell has extended attributes. Reusing
    // a cell would carry the preceding link's tag into unlinked indentation.
    // Fresh snapshots avoid that stale private value without writing attributes
    // that may be owned by the buffer itself.
    // A repeated id within one row is legal OSC 8 output, but cannot establish
    // which run continues. Count all runs before extracting any joined text.
    const readRow = (row) => {
      const source = buffer.getLine(row);
      if (!source) return { runs: 0, text: '' };
      let runs = 0;
      let from = -1;
      let to = 0;
      let inside = false;
      for (let x = 0; x < source.length; x++) {
        const tagged = source.getCell(x)?.extended?.urlId === tag;
        if (tagged && !inside) {
          runs++;
          if (from < 0) from = x;
        }
        if (tagged) to = x + 1;
        inside = tagged;
      }
      // A wide character that cannot fit at the edge moves to a wrapped row.
      // xterm tags its empty padding cell too. Drop only that synthetic pad;
      // an actual printed space has getChars() === ' ' and must survive.
      const next = buffer.getLine(row + 1);
      if (runs === 1 && to === source.length && next?.isWrapped &&
          source.getCell(to - 1)?.getChars?.() === '' && next.getCell(0)?.getWidth?.() === 2) to--;
      return { runs, text: runs === 1 ? source.translateToString(false, from, to) : '' };
    };
    const above = readRow(y - 1);
    const below = readRow(y + 1);
    if (!above.runs && !below.runs) return { kind: 'whole', text: fragment };
    const incomplete = { kind: 'fragment', text: fragment };
    const center = readRow(y);
    if (center.runs !== 1) return incomplete;
    const rows = [center.text];
    let characters = center.text.length;
    if (characters > 8192) return incomplete;
    for (const direction of [-1, 1]) {
      for (let row = y + direction; row >= 0; row += direction) {
        const run = readRow(row);
        if (!run.runs) break;
        if (run.runs !== 1 || rows.length >= 128) return incomplete;
        characters += run.text.length;
        if (characters > 8192) return incomplete;
        if (direction < 0) rows.unshift(run.text);
        else rows.push(run.text);
      }
    }
    // A matching piece alone cannot excuse a split-host or suffix forgery.
    const repeated = uri && rows.every(text => claimsAddress(text.trim()) && !linkTextMismatch(text, uri));
    return { kind: 'whole', text: repeated ? fragment : rows.join('').trim() };
  }

  /** Snapshot every cell of a file link, including its wrapped continuation.
   * A partial repaint can leave xterm's cached link active. Reading through
   * the public buffer API at hover and click makes that cache insufficient
   * authority for a transfer. Keep whitespace and cell boundaries intact;
   * missing or overlong ranges cannot establish that the text is unchanged.
   * Bounds match the printed-path reader's 128-row span and 8192 characters.
   */
  function linkRangeText(term, range) {
    if (!range?.start || !range?.end) return null;
    const { start, end } = range;
    if (![start.x, start.y, end.x, end.y].every(Number.isInteger) ||
        start.x < 1 || start.y < 1 || end.x < 1 || end.y < start.y || end.y - start.y > 128) return null;
    let text = '';
    for (let y = start.y; y <= end.y; y++) {
      const line = term.buffer?.active?.getLine(y - 1);
      if (!line) return null;
      const from = y === start.y ? start.x - 1 : 0;
      const to = y === end.y ? end.x : line.length;
      if (to > line.length || to <= from) return null;
      text += line.translateToString(false, from, to);
      if (text.length > 8192) return null;
    }
    return text;
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
   * When `text`, the link's tagged text, is URL-like
   * and names somewhere else (`linkTextMismatch`), the display becomes a
   * loud warning instead: danger-styled and larger, led by a line saying
   * the text does not match where the link goes, and showing the text
   * beside the real target so the difference is visible before a click.
   *
   * @param {MouseEvent} event
   * @param {string} uri
   * @param {HTMLElement} owner
   * @param {string | null} [text]
   * @param {boolean} [fragment] an adjacent tagged row could not be joined;
   * URL-like pieces always warn, even if that piece alone equals the target.
   */
  function showLinkTarget(event, uri, owner, text, fragment = false, tunnel = null) {
    // Files share visual CSS only; their async evidence owns another element.
    let display = owner.querySelector(':scope > .terminal-link-target:not(.terminal-file-target)');
    if (!display) {
      display = document.createElement('div');
      display.className = 'terminal-link-target';
      display.setAttribute('role', 'tooltip');
      display.dir = 'ltr';
      owner.appendChild(display);
    }
    const parts = linkTargetParts(uri);
    const hostPart = parts.find((part) => part.host);
    const mismatch = fragment ? claimsAddress((text || '').trim()) : linkTextMismatch(text, uri);
    display.classList.toggle('terminal-link-target-mismatch', mismatch);
    const lines = [];
    if (mismatch) {
      const warning = document.createElement('div');
      warning.className = 'terminal-link-target-warning';
      warning.textContent = 'Link text does not match where it goes';
      lines.push(warning);
      const claimed = document.createElement('div');
      claimed.className = 'terminal-link-target-text';
      claimed.textContent = `Text says: ${shortenedTarget(visibleFormatCharacters(text.trim()))}`;
      lines.push(claimed);
      const goes = document.createElement('div');
      goes.textContent = 'Actually goes to:';
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
    if (tunnel) {
      const hint = document.createElement('div');
      hint.textContent = `This is port ${tunnel.port} on ${visibleFormatCharacters(tunnel.host)}, not on this Mac. ⌘-click to open it through a tunnel. A plain click opens it on this Mac.`;
      lines.push(hint);
    }
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
    const display = owner.querySelector(':scope > .terminal-link-target:not(.terminal-file-target)');
    if (display) display.hidden = true;
  }

  const api = {
    openTerminalUrl,
    isPlainWebUrl,
    tunnelTarget,
    shouldTunnel,
    activateWebLink,
    createLinkGesture,
    interceptLinkPress,
    suppressMouseReport,
    linkTargetParts,
    linkTextMismatch,
    linkRowText,
    linkHoverText,
    linkRangeText,
    visibleFormatCharacters,
    shortenedTarget,
    displayedTarget,
    showLinkTarget,
    hideLinkTarget,
  };
  if (typeof window !== 'undefined') window.farhelmTerminalLinks = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
