// Session-host file links: recognition is pure, while `FileLinks` owns fresh
// hover evidence and explicit-click transfers for exactly one terminal.
// Nothing here consults the shell's current directory or opens a file URI.
//
// Both kinds of terminal share this code: it reads a terminal through a
// "view" (`element`, `lines()` giving terminal-links.js's link lines, and
// `hasSelection()`) and speaks link-lines ranges. A new terminal's grid
// hosts `FileLinks` directly (terminal.js's `createGridLinks`); an old
// terminal's xterm.js loads `XtermFileLinksAddon`, the adapter that registers
// an xterm.js link provider and converts its ranges. Native OSC 8 links keep
// xterm.js's ordinary provider; every supervisor owns its actual filesystem.
(function () {
  const MAX_BYTES = 100000000;

  /** Link admission requires the entire live disclosure to fit and be uncovered.
   * The noninteractive hover cannot be scrolled, so clipped details cannot count
   * as evidence the user could inspect. Read dimensions at click time to
   * cover a window resize after lookup, and fail closed for a hidden hover.
   */
  function disclosureFits(display, viewport, coveringRects = []) {
    if (!display || display.hidden || display.clientHeight <= 0 || display.clientWidth <= 0 ||
      display.scrollHeight > display.clientHeight || display.scrollWidth > display.clientWidth) return false;
    // A resize can move fixed-position details off-screen without changing
    // the terminal's cell grid or overflowing the disclosure's own content box.
    const rect = display.getBoundingClientRect();
    return rect.top >= 0 && rect.left >= 0 && rect.bottom <= viewport.innerHeight && rect.right <= viewport.innerWidth &&
      !coveringRects.some(cover => cover.right > cover.left && cover.bottom > cover.top &&
        rect.left < cover.right && rect.right > cover.left && rect.top < cover.bottom && rect.bottom > cover.top);
  }

  /** The attribute every surface drawing at or above a link's destination
   * preview carries. app.css's z-index ladder (at `.header-confirm`) states
   * the rule; this is the one place the cover check reads it from.
   */
  const LINK_COVER_SELECTOR = '[data-link-cover]';

  /** Live rectangles of every marked cover that is currently drawn.
   *
   * Covers are declared by the surfaces themselves rather than listed here:
   * a hand-kept selector list is how the quick-launch pill came to draw over
   * destination previews without ever being counted. A marked element that is
   * not rendered (`display: none`, such as the idle tooltip) or is
   * `visibility: hidden` hides nothing and is skipped. Each rectangle is read
   * at call time, never cached, because covers move and come and go between
   * hover and click. A cover that scrolls its own content (the approval
   * cards) contributes only its visible region's bounds.
   */
  function markedCoverRects(doc, view = doc.defaultView) {
    return Array.from(doc.querySelectorAll(LINK_COVER_SELECTOR))
      .filter(cover => cover.getClientRects().length && view.getComputedStyle(cover).visibility !== 'hidden')
      .map(cover => cover.getBoundingClientRect());
  }

  /** Decimal units agree with the transfer cap. Rounding never renders a
   * refused file as less than 100 MB; the refusal still names the exact limit.
   */
  function formatSize(bytes) {
    if (bytes < 1000) return `${bytes} B`;
    const unit = bytes < 1000000 ? 1000 : bytes < 1000000000 ? 1000000 : 1000000000;
    const suffix = unit === 1000 ? 'KB' : unit === 1000000 ? 'MB' : 'GB';
    return `${(bytes / unit).toFixed(1).replace(/\.0$/, '')} ${suffix}`;
  }

  /** Size controls the stripe, while a refusal always takes precedence.
   * The 100 MB boundary itself is allowed, matching the host's byte limit.
   */
  function sizeTier(info) {
    if (info.status !== 'ready') return 'danger';
    return info.size < 1000000 ? 'small' : info.size < 10000000 ? 'medium' : 'large';
  }

  /** Bundled Nerd Font symbols distinguish broad file kinds, not opening
   * permission. The helm alone supplies the native opening policy.
   */
  function fileIcon(path, status) {
    if (status === 'folder') return '\uf07b';
    const ext = path.split('/').pop().split('.').pop().toLowerCase();
    if (['pdf'].includes(ext)) return '\uf1c1';
    if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'avif'].includes(ext)) return '\uf1c5';
    if (['zip', 'gz', 'xz', 'bz2', 'tar', '7z', 'zst'].includes(ext)) return '\uf1c6';
    if (['txt', 'md', 'rs', 'js', 'ts', 'py', 'html', 'css', 'json', 'toml', 'yaml', 'yml', 'sh', 'c', 'h'].includes(ext)) return '\uf15c';
    return '\uf15b';
  }

  /** Version-like tokens (1.2.3, v1.5x, 1.2.3-rc.1+build.5) that link recognition skips.
   * Each dotted component starts with a required digit, so components cannot
   * repartition each other's digits. Hyphens belong only to the suffix: when
   * components could hold them too, `1.1` followed by thousands of hyphens and
   * a stray character backtracked quadratically, 50 to 80 ms for one
   * 8,192-character line in a browser. The suffix is `+build`, `-pre`, or
   * `-pre+build`. In that last form a dot before the `+` must be followed by a
   * digit, because the hyphenated text there stands for what the old pattern
   * matched as further dotted components; with that rule the three forms accept
   * exactly the tokens the overlapping pattern accepted.
   */
  const NUMERIC_VERSION = /^[+-]?v?\d+(?:\.\d[a-z0-9_]*)+(?:\+[\w.-]+|-[\w.-]*|-(?:[\w-]|\.\d)*\+[\w.-]+)?$/iu;

  /** Recognize conservative prose tokens without turning URL text into paths.
   * Quotes/brackets delimit tokens; spaces inside filenames need OSC 8. Source
   * diagnostic suffixes and sentence punctuation are outside the link range.
   */
  function pathTokens(text) {
    const found = [];
    // Reject the whole whitespace-delimited URI before splitting its internal
    // punctuation. Otherwise javascript:alert(document.domain) creates a
    // filename link for document.domain after losing the scheme-bearing prefix.
    for (const chunk of text.matchAll(/\S+/gu)) {
      const unwrapped = chunk[0].replace(/^["'`<>(){}\[\]]+/u, '').replace(/[.,;!?"'`<>(){}\[\]]+$/u, '').replace(/:\d+(?::\d+)?$/u, '');
      if (/^[a-z][a-z0-9+.-]*:/iu.test(unwrapped) || chunk[0].includes('://') || unwrapped.startsWith('//')) continue;
      for (const match of chunk[0].matchAll(/[^\s"'`<>(){}\[\]]+/gu)) {
        const path = match[0].replace(/[.,;!?]+$/u, '').replace(/:\d+(?::\d+)?$/u, '');
        if (!path || /[\x00-\x1f\x7f]/u.test(path)) continue;
        if (path.startsWith('//') || /^[a-z][a-z0-9+.-]*:/iu.test(path) || path.includes('://')) continue;
        // Numeric version-like tokens, including units such as 1.5x, are
        // not file evidence. A directory prefix still makes them paths.
        if (NUMERIC_VERSION.test(path)) continue;
        if (!(path.startsWith('/') || path.startsWith('~/') || path.includes('/') || /[^/.]+\.[a-z0-9][a-z0-9_-]*$/iu.test(path))) continue;
        const start = chunk.index + match.index;
        found.push({ path, start, end: start + path.length });
      }
    }
    return found;
  }

  /** OSC 8 has its own explicit allowlist because the terminal passes every scheme
   * (xterm.js does with `allowNonHttpProtocols`; the grid passes the program's target).
   * The URI authority never selects a host: files belong to the session host.
   * File targets must be absolute, decode once, and carry no query or fragment.
   */
  function oscTarget(uri) {
    try {
      if (['http:', 'https:'].includes(new URL(uri).protocol)) return { kind: 'web', uri };
    } catch {
      return null;
    }
    if (!/^file:\/\//iu.test(uri) || /[\x00-\x1f\x7f]/u.test(uri)) return null;
    try {
      const url = new URL(uri);
      // URL validates the authority, but its pathname removes dot segments.
      // Unix resolves those after following symlinks; preserve the original
      // path and let the session host establish its filesystem meaning.
      const raw = /^file:\/\/[^/?#]*(\/[^?#]*)$/iu.exec(uri);
      if (url.protocol !== 'file:' || !raw) return null;
      const path = decodeURIComponent(raw[1]);
      if (!path.startsWith('/') || /[\x00-\x1f\x7f]/u.test(path)) return null;
      return { kind: 'file', path };
    } catch {
      return null;
    }
  }

  /** Recognize one program-inserted break, never a chain or a terminal wrap.
   * This is a deliberate exception to ordinary whitespace boundaries: a TUI
   * can break a path while leaving spare columns for its right margin. Keep
   * cell coordinates separate from UTF-16 offsets for wide/combined glyphs.
   */
  function cutPathLink(lines, abs) {
    const head = lines.line(abs);
    const tail = lines.line(abs + 1);
    if (!head || !tail || head.wrapped || tail.wrapped || lines.line(abs + 2)?.wrapped) return null;
    /** Preserve each UTF-16 unit's owning cell, with the same text cap as
     * ordinary recognition; padding still counts against the work bound.
     */
    const read = (line, row) => {
      let text = '';
      const cells = [];
      for (let col = 0; col < line.length; col++) {
        const cell = line.cell(col);
        if (!cell?.width) continue;
        const chars = cell.chars || ' ';
        if (text.length + chars.length > 8192) return null;
        for (let i = 0; i < chars.length; i++) cells.push({ abs: row, col, end: col + cell.width });
        text += chars;
      }
      return { text, cells };
    };
    const a = read(head, abs);
    const b = read(tail, abs + 1);
    if (!a || !b) return null;
    const end = a.text.replace(/ +$/u, '').length;
    const runs = Array.from(a.text.slice(0, end).matchAll(/[^\s"'`<>(){}\[\]]+/gu));
    const last = runs.at(-1);
    const next = /^ *([^\s"'`<>(){}\[\]]+)/u.exec(b.text);
    if (!last || last.index + last[0].length !== end || !next || /^[,;!?:]/u.test(next[1])) return null;
    // The head must already look like a path. Otherwise ordinary prose
    // above an absolute path can satisfy the fit rule and replace that
    // real path with an invented target, including across a history resize.
    // Read the whole head so URI chunks keep the tokenizer's exclusions.
    if (!pathTokens(a.text.slice(0, end)).some(token => token.start === last.index)) return null;
    const indent = next[0].length - next[1].length;
    const tailEnd = indent + next[1].length;
    const remaining = head.length - a.cells[end - 1].end;
    const tailColumns = b.cells[tailEnd - 1].end - b.cells[indent].col;
    if (remaining - 4 >= tailColumns) return null;
    const text = a.text.slice(0, end) + b.text.slice(indent);
    if (text.length > 8192) return null;
    // Tokenize the whole joined line so a scheme-bearing chunk still cannot
    // donate a filename-looking suffix. Delimiters and diagnostic suffixes
    // remain the ordinary tokenizer's, rather than a second path grammar.
    const token = pathTokens(text).find(token => token.start === last.index && token.end > end);
    if (!token) return null;
    const start = a.cells[token.start];
    const finish = b.cells[indent + token.end - end - 1];
    return { path: token.path, range: { start: { abs, col: start.col }, end: { abs: abs + 1, col: finish.end } } };
  }

  /** Offer a cut path from either half, replacing its two misleading fragments.
   * Consecutive possible cuts are ambiguous and stay under the ordinary
   * rules; the exception must never grow into a three-row reconstruction.
   */
  function pathLinks(lines, abs) {
    const cuts = new Map();
    for (let y = Math.max(0, abs - 2); y <= abs + 1; y++) cuts.set(y, cutPathLink(lines, y));
    const joined = [abs - 1, abs].filter(y => cuts.get(y) && !cuts.get(y - 1) && !cuts.get(y + 1))
      .map(y => cuts.get(y));
    const compare = (a, b) => a.abs - b.abs || a.col - b.col;
    return softPathLinks(lines, abs).filter(link => !joined.some(cut =>
      compare(link.range.start, cut.range.end) < 0 && compare(cut.range.start, link.range.end) < 0))
      .concat(joined);
  }

  /** The printed paths touching line `abs` of `lines` (terminal-links.js's
   * link lines), each `{path, range}` with a link-lines range.
   * Map a logical wrapped line's UTF-16 offsets back to cells.
   * Wide cells have a zero-width continuation; combining characters share a
   * cell. Neither may be counted as one column per JS character. A wide glyph
   * wrapping from the last column leaves a padding cell, which is not text.
   * Work is bounded; an overlong logical line yields no links rather than a
   * truncated token whose apparent start changes the recognition policy.
   */
  function softPathLinks(lines, abs) {
    let first = abs;
    while (first > 0 && lines.line(first)?.wrapped) {
      if (abs - first > 128) return [];
      first--;
    }
    let text = '';
    const cells = [];
    for (let y = first; ; y++) {
      const line = lines.line(y);
      if (!line || (y > first && !line.wrapped)) break;
      if (y - first > 128 || text.length > 8192) return [];
      for (let x = 0; x < line.length; x++) {
        const cell = line.cell(x);
        if (!cell || !cell.width) continue;
        const chars = cell.chars;
        if (!chars && x === line.length - 1 && lines.line(y + 1)?.wrapped && lines.line(y + 1).cell(0)?.width === 2) continue;
        const value = chars || ' ';
        for (let i = 0; i < value.length; i++) cells.push({ col: x, abs: y, end: x + cell.width });
        text += value;
      }
    }
    if (text.length > 8192) return [];
    return pathTokens(text).map((token) => {
      const start = cells[token.start];
      const end = cells[token.end - 1];
      return { path: token.path, range: { start: { abs: start.abs, col: start.col }, end: { abs: end.abs, col: end.end } } };
    }).filter((link) => link.range.start.abs <= abs && link.range.end.abs >= abs);
  }

  /** Whether two link-lines ranges name the same cells (both present). */
  function sameRange(a, b) {
    const fields = range => [range?.start?.abs, range?.start?.col, range?.end?.abs, range?.end?.col];
    // A range that is not a link-lines one (an xterm.js range, a missing
    // one) fails closed rather than comparing equal on undefined fields.
    if (![...fields(a), ...fields(b)].every(Number.isInteger)) return false;
    return a.start.abs === b.start.abs && a.start.col === b.start.col &&
      a.end.abs === b.end.abs && a.end.col === b.end.col;
  }

  /** Buffer only complete, bounded responses. A body error or growth past the
   * cap rejects the save; no object URL exists until successful EOF.
   */
  async function boundedBlob(response) {
    const reader = response.body.getReader();
    const chunks = [];
    let size = 0;
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) return new Blob(chunks, { type: 'application/octet-stream' });
        size += value.byteLength;
        if (size > MAX_BYTES) throw new Error('Too large to download (100 MB limit)');
        chunks.push(value);
      }
    } finally {
      await reader.cancel().catch(() => {});
      reader.releaseLock();
    }
  }

  /** File links for one terminal, read through its `view` (see this file's
   * header), which the host sets before any link is asked for: the grid
   * passes it at construction, and the old terminal's add-on sets it on
   * activation (see this file's header).
   * This has the terminal's lifetime, including retained reconnects.
   * Lookup evidence belongs to a single hover, never to a path cache. A click
   * reopens the disclosed canonical path; that still is not a file snapshot.
   */
  class FileLinks {
    constructor({ endpoint, secret, native, isMac = false, isClick = () => false, coveringRects = () => [], view = null,
      // Browser timers require their global receiver. Wrappers keep the
      // defaults out of this object's method-call context while allowing clocks.
      setTimeout: schedule = (callback, delay) => setTimeout(callback, delay),
      clearTimeout: cancel = id => clearTimeout(id) }) {
      this.view = view;
      this.endpoint = endpoint;
      this.secret = secret;
      this.native = native;
      this.isMac = isMac;
      // The terminal owns primary-gesture admission across all link providers.
      // A `FileLinks` without that evidence cannot authorize a transfer.
      this.isClick = isClick;
      // The shell supplies its marked covers' rectangles at activation time
      // (`markedCoverRects`); this knows rectangles, not the shell's DOM
      // or cached geometry.
      this.coveringRects = coveringRects;
      this.hovered = null;
      this.transfers = new Set();
      this.schedule = schedule;
      this.cancel = cancel;
      this.messageTimer = null;
      this.failureMessage = false;
      this.dismissFailure = () => {
        if (this.failureMessage) this.hideMessage();
      };
    }

    /** A text-only disclosure stays inside its terminal; remote values cannot
     * become markup or reorder hidden format characters in the HTML tooltip.
     */
    show(event, lines, checking = false, tier = 'neutral') {
      let display = this.view.element.querySelector(':scope > .terminal-file-target');
      if (!display) {
        display = document.createElement('div');
        display.className = 'terminal-link-target terminal-file-target';
        display.setAttribute('role', 'tooltip');
        display.dir = 'ltr';
        this.view.element.appendChild(display);
      }
      display.hidden = false;
      display.dataset.tier = tier;
      display.replaceChildren(...lines.map((value) => {
        const part = typeof value === 'string' ? { text: value } : value;
        const line = document.createElement('div');
        line.className = `peer-value ${part.className || ''}`;
        if (part.icon) {
          const icon = document.createElement('span');
          // Nerd Font glyphs must keep their face even beside Inter hints.
          icon.className = 'terminal-file-icon';
          icon.textContent = part.icon + ' ';
          icon.setAttribute('aria-hidden', 'true');
          line.appendChild(icon);
        }
        const text = document.createElement('span');
        text.textContent = window.farhelmTerminalLinks.visibleFormatCharacters(String(part.text));
        line.appendChild(text);
        if (part.suffix) {
          const suffix = document.createElement('span');
          suffix.className = 'terminal-file-hint';
          suffix.textContent = part.suffix;
          line.appendChild(suffix);
        }
        return line;
      }));
      display.classList.toggle('terminal-file-checking', checking);
      display.style.left = '0px';
      display.style.top = '0px';
      const box = display.getBoundingClientRect();
      display.style.left = `${Math.max(4, Math.min(event.clientX + 12, window.innerWidth - box.width - 4))}px`;
      display.style.top = `${Math.max(4, event.clientY + 16 + box.height > window.innerHeight ? event.clientY - box.height - 8 : event.clientY + 16)}px`;
    }

    /** Redraws synchronously leave and re-hover the same cells, in both
     * terminals. Reclaim that one hover without restarting its lookup or
     * disclosure. Changed displayed text, path/range or a completed departure
     * checks afresh; no evidence survives across hovers. Unknown text/ranges
     * cannot establish identity, including OSC labels independent of their
     * target. `range` is a link-lines range, by default the link's own;
     * a range that is not one (missing or malformed fields) never matches,
     * so it cannot reclaim a hover. `link`, when given, is the surface's
     * link object, whose `decorations.pointerCursor` this sets.
     */
    async hover(event, path, link = null, range = link?.range, text = path) {
      const previous = this.hovered;
      if (previous?.leaving && previous.path === path && text !== null && previous.text === text && sameRange(range, previous.range)) {
        this.cancel(previous.leaveTimer);
        previous.leaveTimer = null;
        previous.leaving = false;
        previous.link = link;
        if (link) {
          // The terminal snapshots decorations before hover(), then replaces
          // them with cursor setters after it returns (xterm.js does; the
          // grid's link decorations imitate it). Wait for that installation;
          // a retired link's setter would target whatever link is current.
          await Promise.resolve();
          if (this.hovered === previous && !previous.leaving && previous.link === link) {
            link.decorations.pointerCursor = previous.info?.status === 'ready';
          }
        }
        return;
      }
      this.clearHover();
      const state = {
        path, text, link, range: range ? { start: { ...range.start }, end: { ...range.end } } : null,
        screenText: window.farhelmTerminalLinks.linkRangeText(this.view.lines(), range),
        controller: new AbortController(), info: null, leaving: false, leaveTimer: null,
      };
      this.hovered = state;
      this.show(event, ['Checking…', path], true);
      try {
        const response = await this.request('stat', path, state.controller.signal);
        const info = await response.json();
        if (this.hovered !== state || state.leaving) return;
        state.info = info;
        // Re-linkification may have replaced the original link while stat
        // was pending. Its old cursor setter now targets a different link.
        if (state.link) state.link.decorations.pointerCursor = info.status === 'ready';
        const status = {
          ready: formatSize(info.size),
          not_found: 'Not found',
          folder: 'Folder: not downloadable',
          too_large: `Too large to download: ${formatSize(info.size)} (100 MB limit)`,
          not_readable: 'Not readable',
          not_regular: 'Not a regular file: not downloadable',
        }[info.status] || 'Not downloadable';
        // Split only for presentation. Both pieces retain the full disclosed
        // path, and each remote fragment still passes through visible escapes.
        const split = info.path.lastIndexOf('/') + 1;
        const hint = info.status === 'ready'
          ? ' · click to download' + (this.native && info.can_open ? ` · ${this.isMac ? '⌘' : 'Ctrl'}-click to open` : '')
          : '';
        this.show(event, [
          { text: info.path.slice(split), icon: fileIcon(info.path, info.status), className: 'terminal-file-name' },
          { text: `${info.host}:${info.path.slice(0, split)}`, className: 'terminal-file-directory' },
          { text: status, suffix: hint, className: info.status === 'ready' ? 'terminal-file-size' : 'terminal-file-refusal' },
        ], false, sizeTier(info));
      } catch (error) {
        if (this.hovered === state && !state.leaving && !state.controller.signal.aborted) this.show(event, [path, error.message], false, 'danger');
      }
    }

    /** Withdraw click/cursor admission now, but allow the terminal's
     * synchronous redraw re-hover (xterm.js's, which the grid imitates) to
     * reclaim the lookup before the next task tears it down. Cursor setters
     * must run here: the terminal redirects them to its current link,
     * so a deferred write could switch off an unrelated link's hand cursor.
     */
    leave() {
      const state = this.hovered;
      if (!state || state.leaving) return;
      if (state.link) state.link.decorations.pointerCursor = false;
      state.leaving = true;
      state.leaveTimer = this.schedule(() => {
        if (this.hovered === state) this.clearHover();
      }, 0);
    }

    /** End the current lookup immediately on replacement or disposal. This
     * teardown owns timers, abort and disclosure only, never cursor setters.
     */
    clearHover() {
      const state = this.hovered;
      if (state) {
        if (state.leaveTimer !== null) this.cancel(state.leaveTimer);
        state.controller.abort();
      }
      this.hovered = null;
      const display = this.view?.element?.querySelector(':scope > .terminal-file-target');
      if (display) display.hidden = true;
      this.lowerRefusal();
    }

    /** A departed file or web hover must not leave its notice above later cards
     * or disclosures. Keep the explanation, but restore ordinary failure stacking.
     */
    lowerRefusal() {
      this.refusalRaised = false;
      const message = this.view?.element?.querySelector(':scope > .terminal-file-message');
      if (message?.dataset.outcome === 'disclosure-refusal') message.dataset.outcome = 'failure';
    }

    /** Share the click-time disclosure gate and explanation across file and web
     * activation, so both always measure the same covers. Empty/hidden
     * disclosures fail closed; the shell supplies the covering rectangles.
     */
    admitDisclosure(display, web = false) {
      const fits = disclosureFits(display, window);
      if (fits && disclosureFits(display, window, this.coveringRects())) return true;
      const reason = fits ? 'are covered by another Farhelm surface; move the pointer' : 'are not visible; enlarge the window';
      this.refuse(`cannot ${web ? 'open' : 'download'}: full ${web ? 'link' : 'file'} details ${reason} and hover again`);
      return false;
    }

    /** Activation runs on mouseup, before the click that dismisses failures.
     * Deferring both paths through the owned timer keeps a refusal from erasing
     * itself; replacement or disposal cancels the pending explanation as usual.
     */
    refuse(text) {
      this.hideMessage();
      this.refusalRaised = true;
      this.messageTimer = this.schedule(() => {
        this.messageTimer = null;
        // A departure can happen before this task runs. It withdraws the
        // raised layer for the pending explanation as well as a shown one.
        this.message(text, this.refusalRaised ? 'disclosure-refusal' : 'failure');
      }, 0);
    }

    /** Paths travel as JSON data with the current device credential, never as
     * URL tickets. HTTP refusals remain in-page text rather than navigation.
     */
    async request(operation, path, signal, open = false) {
      const response = await fetch(`${this.endpoint}/${operation}`, {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${this.secret()}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ path, save_to_downloads: operation === 'download' && this.native, open }),
        signal,
      });
      if (!response.ok) throw new Error((await response.text()).slice(0, 4096).trim() || 'Download refused');
      return response;
    }

    /** Retire the previous deadline before clearing or replacing its notice.
     * Otherwise an earlier success could hide a later transfer or failure.
     */
    hideMessage() {
      if (this.messageTimer !== null) this.cancel(this.messageTimer);
      this.messageTimer = null;
      this.failureMessage = false;
      this.refusalRaised = false;
      const message = this.view?.element?.querySelector(':scope > .terminal-file-message');
      if (message) {
        message.classList.remove('showing');
        message.textContent = '';
        message.setAttribute('aria-hidden', 'true');
      }
    }

    /** Outcomes survive leaving the hover: success fades after five seconds,
     * failure remains until the next terminal click or admitted download.
     * Agent values use textContent and peer-value isolation. Injected timers
     * let tests exercise replacement and teardown without wall-clock delays.
     */
    message(text, outcome = 'progress') {
      this.hideMessage();
      let message = this.view.element.querySelector(':scope > .terminal-file-message');
      if (!message) {
        message = document.createElement('div');
        message.className = 'terminal-file-message peer-value';
        message.setAttribute('role', 'status');
        this.view.element.appendChild(message);
        // A terminal can hand over its view before its DOM exists (xterm.js
        // activates add-ons before open() creates it). The first
        // notice is necessarily later, so listener ownership starts here.
        this.view.element.addEventListener('click', this.dismissFailure);
      }
      message.textContent = window.farhelmTerminalLinks.visibleFormatCharacters(text);
      message.dataset.outcome = outcome;
      message.setAttribute('aria-hidden', 'false');
      message.classList.add('showing');
      this.failureMessage = outcome === 'failure' || outcome === 'disclosure-refusal';
      if (outcome === 'success') {
        this.messageTimer = this.schedule(() => {
          this.messageTimer = null;
          // Keep the text for the opacity transition, but retire its announcement.
          message.classList.remove('showing');
          message.setAttribute('aria-hidden', 'true');
        }, 5000);
      }
    }

    /** Re-recognize a printed path's target and endpoints from the live buffer.
     * The grid uses this after neighboring evidence changes, without renewing
     * host permission for an unchanged hover. Transfers independently repeat
     * it so a token growing outside its cached rectangle cannot reuse a target.
     */
    pathCurrent(path, range) {
      return pathLinks(this.view.lines(), range.start.abs).some(current =>
        current.path === path && sameRange(current.range, range));
    }

    /** Only a successful hover over unchanged text can start a transfer.
     * A terminal may retain a link after a partial redraw; its cached target must
     * not outlive the cells that established this hover. Native opening also
     * requires that hover's opening offer. Native failure never falls back to
     * a webview Blob save.
     */
    async download(path, event = {}) {
      const state = this.hovered;
      if (!this.isClick(event) || !state || state.leaving || state.path !== path || state.info?.status !== 'ready' || this.view.hasSelection()) return;
      const lines = this.view.lines();
      if (state.screenText === null || state.screenText !== window.farhelmTerminalLinks.linkRangeText(lines, state.range)) return;
      // A printed token can grow beyond its cached endpoint without changing
      // any old cell. Reuse recognition so suffixes, prefixes and diagnostic
      // punctuation obey the same boundaries as the original link provider.
      if (state.link && !this.pathCurrent(path, state.range)) return;
      if (!this.admitDisclosure(this.view.element.querySelector(':scope > .terminal-file-target'))) return;
      const controller = new AbortController();
      this.transfers.add(controller);
      this.message('Downloading…');
      try {
        // The current policy can become more permissive after hover. Its
        // click-time check may refuse an advertised opening, but cannot add
        // an opening that this hover never offered to the user.
        const requestedOpen = this.native && Boolean(this.isMac ? event.metaKey : event.ctrlKey);
        const open = requestedOpen && Boolean(state.info.can_open);
        const response = await this.request('download', state.info.path, controller.signal, open);
        if (this.native) {
          const result = await response.json();
          if (controller.signal.aborted) return;
          const outcome = open ? result.opened ? ' · opened' : ` · not opened: ${result.open_error}` :
            requestedOpen ? ' · not opened: opening was not offered' : '';
          this.message(`Saved to ${result.path}${outcome}`, 'success');
        } else {
          const blob = await boundedBlob(response);
          if (controller.signal.aborted) return;
          const url = URL.createObjectURL(blob);
          const anchor = document.createElement('a');
          anchor.href = url;
          anchor.download = state.info.path.split('/').pop();
          this.view.element.appendChild(anchor);
          anchor.click();
          anchor.remove();
          // Retain until the browser has consumed the click. Disposal releases
          // these too; there is no transfer payload beyond the bounded Blob.
          this.urls ||= new Set();
          this.urls.add(url);
          setTimeout(() => {
            URL.revokeObjectURL(url);
            this.urls.delete(url);
          }, 0);
          this.message(`Downloaded ${anchor.download}`, 'success');
        }
      } catch (error) {
        if (!controller.signal.aborted) this.message(`Download failed: ${error.message}`, 'failure');
      } finally {
        this.transfers.delete(controller);
      }
    }

    /** Disposing the terminal cancels owned work and releases completed Blobs. */
    dispose() {
      this.leave();
      this.clearHover();
      this.hideMessage();
      this.view?.element?.removeEventListener('click', this.dismissFailure);
      for (const controller of this.transfers) controller.abort();
      for (const url of this.urls || []) URL.revokeObjectURL(url);
    }
  }

  // ------------------------------------------------------------------
  // Old terminals: the xterm.js add-on
  // ------------------------------------------------------------------

  /** `FileLinks` as an xterm.js add-on, for old terminals only: activation
   * reads the terminal through terminal-links.js's `xtermLines` and registers
   * a link provider below OSC 8 and web links, preserving their existing
   * precedence, with xterm.js's 1-based ranges converted at this boundary.
   */
  class XtermFileLinksAddon extends FileLinks {
    activate(term) {
      this.term = term;
      this.view = {
        get element() {
          return term.element;
        },
        lines: () => window.farhelmTerminalLinks.xtermLines(term),
        hasSelection: () => !!term.getSelection(),
      };
      this.provider = term.registerLinkProvider({
        provideLinks: (row, callback) => {
          const abs = row - 1;
          callback(pathLinks(this.view.lines(), abs).map(candidate => this.fileLink(candidate)));
        },
      });
    }

    /** An xterm.js link for a printed path: xterm.js's range, with the usual
     * fresh-hover and click checks after provider admission.
     */
    fileLink({ path, range }) {
      const link = {
        text: path,
        range: window.farhelmTerminalLinks.toXtermRange(range),
        decorations: { underline: true, pointerCursor: false },
        hover: event => this.hover(event, path, link, range),
        leave: () => this.leave(),
        activate: event => this.download(path, event),
      };
      return link;
    }

    dispose() {
      super.dispose();
      this.provider?.dispose();
    }
  }

  const api = {
    pathTokens, oscTarget, pathLinks, boundedBlob, formatSize, sizeTier, fileIcon, disclosureFits, markedCoverRects,
    LINK_COVER_SELECTOR, FileLinks, XtermFileLinksAddon, MAX_BYTES, NUMERIC_VERSION,
  };
  if (typeof window !== 'undefined') window.farhelmTerminalFiles = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
