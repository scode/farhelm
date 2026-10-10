// Session-host file links: recognition is pure, while the addon owns fresh
// hover evidence and explicit-click transfers for exactly one terminal.
// Nothing here consults the shell's current directory or opens a file URI.
(function () {
  const MAX_BYTES = 100000000;

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
        if (/^v?\d+(?:\.\d+)+(?:[-+][\w.-]+)?$/iu.test(path)) continue;
        if (!(path.startsWith('/') || path.startsWith('~/') || path.includes('/') || /[^/.]+\.[a-z][a-z0-9_-]*$/iu.test(path))) continue;
        const start = chunk.index + match.index;
        found.push({ path, start, end: start + path.length });
      }
    }
    return found;
  }

  /** OSC 8 has its own explicit allowlist because xterm now passes all schemes.
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

  /** Map a logical wrapped line's UTF-16 offsets back to public xterm cells.
   * Wide cells have a zero-width continuation; combining characters share a
   * cell. Neither may be counted as one column per JS character. A wide glyph
   * wrapping from the last column leaves a padding cell, which is not text.
   * Work is bounded; an overlong logical line yields no links rather than a
   * truncated token whose apparent start changes the recognition policy.
   */
  function bufferLinks(term, row) {
    const buffer = term.buffer.active;
    let first = row - 1;
    while (first > 0 && buffer.getLine(first)?.isWrapped) {
      if (row - first > 128) return [];
      first--;
    }
    let text = '';
    const cells = [];
    for (let y = first; ; y++) {
      const line = buffer.getLine(y);
      if (!line || (y > first && !line.isWrapped)) break;
      if (y - first > 128 || text.length > 8192) return [];
      for (let x = 0; x < line.length; x++) {
        const cell = line.getCell(x);
        if (!cell || !cell.getWidth()) continue;
        const chars = cell.getChars();
        if (!chars && x === line.length - 1 && buffer.getLine(y + 1)?.isWrapped && buffer.getLine(y + 1).getCell(0)?.getWidth() === 2) continue;
        const value = chars || ' ';
        for (let i = 0; i < value.length; i++) cells.push({ x: x + 1, y: y + 1, endX: x + cell.getWidth() });
        text += value;
      }
    }
    if (text.length > 8192) return [];
    return pathTokens(text).map((token) => {
      const start = cells[token.start];
      const end = cells[token.end - 1];
      return { path: token.path, range: { start: { x: start.x, y: start.y }, end: { x: end.endX, y: end.y } } };
    }).filter((link) => link.range.start.y <= row && link.range.end.y >= row);
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
        if (size > MAX_BYTES) throw new Error('too large to download (100 MB limit)');
        chunks.push(value);
      }
    } finally {
      await reader.cancel().catch(() => {});
      reader.releaseLock();
    }
  }

  /** This addon has the terminal's lifetime, including retained reconnects.
   * Lookup evidence belongs to a single hover, never to a path cache. A click
   * reopens the disclosed canonical path; that still is not a file snapshot.
   */
  class FileLinksAddon {
    constructor({ endpoint, secret, native,
      // Browser timers require their global receiver. Wrappers keep the
      // defaults out of the addon's method-call context while allowing clocks.
      setTimeout: schedule = (callback, delay) => setTimeout(callback, delay),
      clearTimeout: cancel = id => clearTimeout(id) }) {
      this.endpoint = endpoint;
      this.secret = secret;
      this.native = native;
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

    /** Register below OSC 8 and web links, preserving their existing precedence. */
    activate(term) {
      this.term = term;
      this.provider = term.registerLinkProvider({
        provideLinks: (row, callback) => callback(bufferLinks(term, row).map(({ path, range }) => {
          const link = {
            text: path,
            range,
            decorations: { underline: true, pointerCursor: false },
            hover: (event) => this.hover(event, path, link),
            leave: () => this.leave(),
            activate: () => this.download(path),
          };
          return link;
        })),
      });
    }

    /** A text-only disclosure stays inside its terminal; remote values cannot
     * become markup or reorder hidden format characters in the HTML tooltip.
     */
    show(event, lines, checking = false) {
      let display = this.term.element.querySelector(':scope > .terminal-file-target');
      if (!display) {
        display = document.createElement('div');
        display.className = 'terminal-link-target terminal-file-target';
        display.setAttribute('role', 'tooltip');
        display.dir = 'ltr';
        this.term.element.appendChild(display);
      }
      display.hidden = false;
      display.replaceChildren(...lines.map((text) => {
        const line = document.createElement('div');
        line.className = 'peer-value';
        line.textContent = window.farhelmTerminalLinks.visibleFormatCharacters(String(text));
        return line;
      }));
      display.classList.toggle('terminal-file-checking', checking);
      display.style.left = '0px';
      display.style.top = '0px';
      const box = display.getBoundingClientRect();
      display.style.left = `${Math.max(4, Math.min(event.clientX + 12, window.innerWidth - box.width - 4))}px`;
      display.style.top = `${Math.max(4, event.clientY + 16 + box.height > window.innerHeight ? event.clientY - box.height - 8 : event.clientY + 16)}px`;
    }

    /** A new hover immediately withdraws any old click admission. Leaving
     * aborts the request; the identity check also rejects late raced replies.
     */
    async hover(event, path, link = null) {
      this.leave();
      const state = { path, link, controller: new AbortController(), info: null };
      this.hovered = state;
      this.show(event, ['checking…', path], true);
      try {
        const response = await this.request('stat', path, state.controller.signal);
        const info = await response.json();
        if (this.hovered !== state) return;
        state.info = info;
        if (link) link.decorations.pointerCursor = info.status === 'ready';
        const status = {
          ready: `${info.size} bytes — click to download`,
          not_found: 'not found',
          folder: 'folder: not downloadable',
          too_large: `too large to download: ${info.size} bytes (100 MB limit)`,
          not_readable: 'not readable',
          not_regular: 'not a regular file: not downloadable',
        }[info.status] || 'not downloadable';
        this.show(event, [info.host, info.path, status]);
      } catch (error) {
        if (this.hovered === state && !state.controller.signal.aborted) this.show(event, [path, error.message]);
      }
    }

    /** Cancel lookup and remove pointer admission even when a queued reply lands. */
    leave() {
      if (this.hovered) {
        this.hovered.controller.abort();
        if (this.hovered.link) this.hovered.link.decorations.pointerCursor = false;
      }
      this.hovered = null;
      const display = this.term?.element?.querySelector(':scope > .terminal-file-target');
      if (display) display.hidden = true;
    }

    /** Paths travel as JSON data with the current device credential, never as
     * URL tickets. HTTP refusals remain in-page text rather than navigation.
     */
    async request(operation, path, signal) {
      const response = await fetch(`${this.endpoint}/${operation}`, {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${this.secret()}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ path, save_to_downloads: operation === 'download' && this.native }),
        signal,
      });
      if (!response.ok) throw new Error((await response.text()).slice(0, 4096).trim() || 'download refused');
      return response;
    }

    /** Retire the previous deadline before clearing or replacing its notice.
     * Otherwise an earlier success could hide a later transfer or failure.
     */
    hideMessage() {
      if (this.messageTimer !== null) this.cancel(this.messageTimer);
      this.messageTimer = null;
      this.failureMessage = false;
      const message = this.term?.element?.querySelector(':scope > .terminal-file-message');
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
      let message = this.term.element.querySelector(':scope > .terminal-file-message');
      if (!message) {
        message = document.createElement('div');
        message.className = 'terminal-file-message peer-value';
        message.setAttribute('role', 'status');
        this.term.element.appendChild(message);
        // xterm activates addons before open() creates its DOM. The first
        // notice is necessarily later, so listener ownership starts here.
        this.term.element.addEventListener('click', this.dismissFailure);
      }
      message.textContent = window.farhelmTerminalLinks.visibleFormatCharacters(text);
      message.setAttribute('aria-hidden', 'false');
      message.classList.add('showing');
      this.failureMessage = outcome === 'failure';
      if (outcome === 'success') {
        this.messageTimer = this.schedule(() => {
          this.messageTimer = null;
          // Keep the text for the opacity transition, but retire its announcement.
          message.classList.remove('showing');
          message.setAttribute('aria-hidden', 'true');
        }, 5000);
      }
    }

    /** Only a successful current hover can start a transfer. Native mode is
     * explicit: its failure never falls back to a webview Blob save.
     */
    async download(path) {
      const state = this.hovered;
      if (!state || state.path !== path || state.info?.status !== 'ready' || this.term.getSelection()) return;
      const controller = new AbortController();
      this.transfers.add(controller);
      this.message('downloading…');
      try {
        const response = await this.request('download', state.info.path, controller.signal);
        if (this.native) {
          const result = await response.json();
          if (controller.signal.aborted) return;
          this.message(`saved to ${result.path}`, 'success');
        } else {
          const blob = await boundedBlob(response);
          if (controller.signal.aborted) return;
          const url = URL.createObjectURL(blob);
          const anchor = document.createElement('a');
          anchor.href = url;
          anchor.download = state.info.path.split('/').pop();
          this.term.element.appendChild(anchor);
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
          this.message(`downloaded ${anchor.download}`, 'success');
        }
      } catch (error) {
        if (!controller.signal.aborted) this.message(`download failed: ${error.message}`, 'failure');
      } finally {
        this.transfers.delete(controller);
      }
    }

    /** Disposing the terminal cancels owned work and releases completed Blobs. */
    dispose() {
      this.leave();
      this.hideMessage();
      this.term?.element?.removeEventListener('click', this.dismissFailure);
      this.provider?.dispose();
      for (const controller of this.transfers) controller.abort();
      for (const url of this.urls || []) URL.revokeObjectURL(url);
    }
  }

  const api = { pathTokens, oscTarget, bufferLinks, boundedBlob, FileLinksAddon, MAX_BYTES };
  if (typeof window !== 'undefined') window.farhelmTerminalFiles = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
