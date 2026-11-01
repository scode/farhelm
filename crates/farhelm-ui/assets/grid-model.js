// The grid client's wire decoder and screen/history model: pure, no DOM.
//
// New terminals (the ones a terminal host runs) reach the page as a grid of
// styled runs that the supervisor's emulator computed, not as bytes for
// xterm.js. The supervisor-to-client messages (`hello`, `state`, `rows`,
// `frame`, `history`, `history_reset`, `clipboard`) are specified in the grid stream
// format (`farhelm-proto`'s grid types and the format notes they came
// from). This file turns one data frame into a validated message and
// applies messages to a model of what the terminal shows; grid-terminal.js
// draws that model, and grid-input.js turns the user's input back into
// messages.
//
// ## Everything from the wire is hostile until validated
//
// Under `--ssh` the supervisor is another machine, and a program running in
// the terminal controls the cell text, the colors and the link targets
// that arrive here. So every message is checked against the format's
// bounds BEFORE anything is applied: sizes and positions are integers in
// range, colors are one of the three encodings, text is within its per-cell
// byte budget, runs are ordered and inside the row. A violation throws
// `GridProtocolError`; the caller treats that as the format says (report a
// protocol error and reconnect), and the model is left as it was before the
// offending message, because a commit validates everything it will touch
// before it mutates anything. Nothing here ever builds markup or CSS from a
// wire string: text stays text, styles stay integers (grid-style.js maps
// them to CSS from fixed tables).
//
// ## The model
//
// - The SCREEN is `rows` rows of runs. A screen update is any number of
//   `rows` messages followed by one `frame`, which commits them: nothing in
//   `rows` is visible until its frame arrives, so a frame is never
//   half-applied, and nothing else may come between them.
// - HISTORY is the lines above the screen, oldest first, numbered by an
//   absolute line number that only grows, each with the width it was
//   written at. Appends must be contiguous; the oldest lines past
//   `history_cap` are dropped. grid-terminal.js shows it as scrollback.
// - The HISTORY QUEUE holds `history` and `history_reset` messages that
//   have been accepted but not yet applied. An attach sends up to the whole
//   history (12,000 lines by default, in many messages) right after the
//   screen, and decoding all of it at once held the page for seconds on a
//   loaded machine. With `deferHistory` set, `apply` only checks a history
//   message's framing and queues it, and `drainHistory` applies queued rows
//   until its caller says to yield, so the renderer can spread the work
//   across tasks. Without it (the unit tests, small callers), `apply`
//   drains at once and nothing is ever left queued.
// - CLIPBOARD writes a program makes (OSC 52) arrive in contiguous parts
//   and are handed out whole (`takeClipboard`).
// - STATE is the small set of program modes the client acts on (mouse and
//   focus reporting, the alternate screen, redefined default colors, the
//   exit status).
// - DAMAGE records what changed since the renderer last drew, so a paint
//   touches only changed rows and can move rows for a scroll instead of
//   redrawing them.
(function (root) {
  "use strict";

  // ## Bounds (the format's table; both sides enforce what they receive)

  const GRID_MAX_MESSAGE = 1024 * 1024;
  const MAX_DIMENSION = 1000;
  const MAX_HISTORY_CAP = 100000;
  const MAX_LINKS = 256;
  const MAX_LINK_ID_BYTES = 250;
  const MAX_LINK_URI_BYTES = 2083;
  // A character plus up to 16 combining characters, at most 4 UTF-8 bytes
  // each, per cell.
  const MAX_TEXT_BYTES_PER_CELL = 4 * (1 + 16);
  const MAX_CLIPBOARD_PART_BYTES = 512 * 1024;
  const MAX_CLIPBOARD_BYTES = 8 * 1024 * 1024;
  // Small clipboard parts are joined into chunks of at least this much
  // (see `applyClipboard`), so a write holds at most 128 chunks plus the
  // parts of one chunk being gathered.
  const CLIPBOARD_CHUNK_BYTES = 64 * 1024;
  const DIRECT_COLOR_BASE = 0x1000000;
  const DIRECT_COLOR_MAX = 0x1ffffff;
  // The flag bits the format defines (1 bold through 2048 blink). Unknown
  // higher bits are dropped rather than refused, like unknown fields.
  const KNOWN_FLAGS = 0xfff;
  const MOUSE_MODES = new Set(["off", "click", "drag", "motion"]);
  const CURSOR_SHAPES = new Set(["block", "underline", "bar"]);
  const INT32_MIN = -0x80000000;
  const INT32_MAX = 0x7fffffff;

  /**
   * A message that violates the grid stream format. The message text is
   * this file's own wording, never peer text, so callers may show it.
   */
  class GridProtocolError extends Error {
    constructor(message) {
      super(message);
      this.name = "GridProtocolError";
    }
  }

  function fail(message) {
    throw new GridProtocolError(message);
  }

  function isInt(value, lo, hi) {
    return Number.isInteger(value) && value >= lo && value <= hi;
  }

  function isPlainObject(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }

  /**
   * The UTF-8 length of a JS string, which is what the format's byte
   * bounds count. A lone surrogate counts as the three bytes of the
   * U+FFFD that an encoder would write in its place.
   */
  function utf8Length(text) {
    let bytes = 0;
    for (let i = 0; i < text.length; i++) {
      const unit = text.charCodeAt(i);
      if (unit < 0x80) bytes += 1;
      else if (unit < 0x800) bytes += 2;
      else if (unit >= 0xd800 && unit <= 0xdbff && i + 1 < text.length) {
        const next = text.charCodeAt(i + 1);
        if (next >= 0xdc00 && next <= 0xdfff) {
          bytes += 4;
          i++;
        } else {
          bytes += 3;
        }
      } else bytes += 3;
    }
    return bytes;
  }

  // ## Decoding one data frame

  const utf8Decoder = typeof TextDecoder === "function" ? new TextDecoder("utf-8", { fatal: true }) : null;

  /**
   * Turn one supervisor-to-client data frame into a message object, or
   * throw `GridProtocolError`.
   *
   * Only the framing is checked here (size, UTF-8, JSON, an object with a
   * string `t`); each message's fields are validated when it is applied,
   * because some bounds (a row's width) depend on the model's state.
   *
   * @param {ArrayBuffer|Uint8Array|string} data
   */
  function decodeFrame(data) {
    let text;
    if (typeof data === "string") {
      if (utf8Length(data) > GRID_MAX_MESSAGE) fail("message larger than 1 MiB");
      text = data;
    } else {
      const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
      if (bytes.byteLength > GRID_MAX_MESSAGE) fail("message larger than 1 MiB");
      try {
        text = utf8Decoder.decode(bytes);
      } catch (_error) {
        fail("message is not UTF-8");
      }
    }
    let message;
    try {
      message = JSON.parse(text);
    } catch (_error) {
      fail("message is not JSON");
    }
    if (!isPlainObject(message) || typeof message.t !== "string") fail("message has no type");
    return message;
  }

  // ## Field validation

  function decodeColor(value, what) {
    if (value === -1 || isInt(value, 0, 255) || isInt(value, DIRECT_COLOR_BASE, DIRECT_COLOR_MAX)) {
      return value;
    }
    return fail(`bad ${what} color`);
  }

  /**
   * Validate a `[flags, fg, bg]` or `[flags, fg, bg, underline_color]`
   * style into `{flags, fg, bg, ul, key}`. `key` identifies the style for
   * grid-style.js's CSS cache; it is built from validated integers only.
   */
  function decodeStyle(raw) {
    if (!Array.isArray(raw) || (raw.length !== 3 && raw.length !== 4)) fail("bad style");
    if (!isInt(raw[0], 0, INT32_MAX)) fail("bad style flags");
    const flags = raw[0] & KNOWN_FLAGS;
    const fg = decodeColor(raw[1], "foreground");
    const bg = decodeColor(raw[2], "background");
    const ul = raw.length === 4 ? decodeColor(raw[3], "underline") : -1;
    return { flags, fg, bg, ul, key: `${flags},${fg},${bg},${ul}` };
  }

  /**
   * Validate a message's optional `links` table into `{id, uri}` objects.
   * The targets are data: nothing here interprets a URI, and whatever
   * later opens one goes through terminal-links.js's scheme rules.
   */
  function decodeLinks(raw) {
    if (raw === undefined || raw === null) return [];
    if (!Array.isArray(raw) || raw.length > MAX_LINKS) fail("bad links table");
    return raw.map((entry) => {
      if (!Array.isArray(entry) || entry.length !== 2) fail("bad link");
      const [id, uri] = entry;
      if (typeof id !== "string" || utf8Length(id) > MAX_LINK_ID_BYTES) fail("bad link id");
      if (typeof uri !== "string" || utf8Length(uri) > MAX_LINK_URI_BYTES) fail("bad link target");
      return Object.freeze({ id, uri });
    });
  }

  /**
   * Validate one row (`{"w":bool,"r":[run...]}`) against a width of
   * `maxCols` into `{wrapped, runs, end}`, where `end` is the first column
   * after the last run.
   *
   * Runs come out as `{col, width, text, style, link}` in increasing,
   * non-overlapping column order; columns no run covers are blank in the
   * default style. The text budget is per cell, so a run cannot carry more
   * than its cells could legitimately hold. A run of printable ASCII has one
   * character per cell; any other run holds exactly one cell (width 1, or 2
   * for a wide character), because only the emulator knows how characters
   * group into cells and the format never asks the browser to guess.
   */
  function decodeRow(raw, maxCols, links, styles) {
    if (!isPlainObject(raw)) fail("bad row");
    if (typeof raw.w !== "boolean") fail("bad row wrap flag");
    if (!Array.isArray(raw.r) || raw.r.length > maxCols) fail("bad row runs");
    const runs = [];
    let end = 0;
    for (const run of raw.r) {
      if (!Array.isArray(run) || (run.length !== 4 && run.length !== 5)) fail("bad run");
      const [col, width, text, style] = run;
      if (!isInt(col, end, maxCols - 1)) fail("run column out of order or past the row");
      if (!isInt(width, 1, maxCols - col)) fail("run width past the row");
      if (typeof text !== "string" || utf8Length(text) > MAX_TEXT_BYTES_PER_CELL * width) fail("bad run text");
      if (PRINTABLE_ASCII.test(text)) {
        if (text.length !== width) fail("an ASCII run's width is not its length");
      } else if (width > 2) {
        fail("a non-ASCII run holds more than one cell");
      }
      let link = null;
      if (run.length === 5) {
        if (!isInt(run[4], 0, links.length - 1)) fail("run link out of range");
        link = links[run[4]];
      }
      runs.push({ col, width, text, style: internStyle(decodeStyle(style), styles), link });
      end = col + width;
    }
    return { wrapped: raw.w, runs, end };
  }

  // How many distinct styles a model shares before it starts over. A
  // program cycling through true colors would otherwise grow the table
  // without limit; starting over only costs sharing, never correctness.
  const STYLE_TABLE_LIMIT = 65536;

  /**
   * The model's one object for a style, so the many runs in history that
   * share a handful of styles share their objects too (history is kept up
   * to `history_cap` lines whether or not it is drawn).
   */
  function internStyle(style, styles) {
    if (!styles) return style;
    const known = styles.get(style.key);
    if (known) return known;
    if (styles.size >= STYLE_TABLE_LIMIT) styles.clear();
    styles.set(style.key, style);
    return style;
  }

  /** A row with nothing on it, shared because rows are never mutated. */
  const BLANK_ROW = Object.freeze({ wrapped: false, runs: Object.freeze([]), end: 0 });

  function blankScreen(rows) {
    return new Array(rows).fill(BLANK_ROW);
  }

  function decodeCursor(raw) {
    if (!isPlainObject(raw)) fail("bad cursor");
    if (!isInt(raw.x, 0, MAX_DIMENSION) || !isInt(raw.y, 0, MAX_DIMENSION - 1)) fail("bad cursor position");
    if (typeof raw.visible !== "boolean" || typeof raw.blink !== "boolean") fail("bad cursor flags");
    if (!CURSOR_SHAPES.has(raw.shape)) fail("bad cursor shape");
    return { x: raw.x, y: raw.y, visible: raw.visible, shape: raw.shape, blink: raw.blink };
  }

  function decodeDefaultColor(value, what) {
    if (value === null) return null;
    if (!isInt(value, DIRECT_COLOR_BASE, DIRECT_COLOR_MAX)) fail(`bad default ${what}`);
    return value;
  }

  function decodeExit(value) {
    if (value === null) return null;
    if (!isPlainObject(value)) fail("bad exit");
    if ("code" in value && isInt(value.code, INT32_MIN, INT32_MAX)) return { code: value.code };
    if ("signal" in value && isInt(value.signal, 1, INT32_MAX)) return { signal: value.signal };
    return fail("bad exit");
  }

  function decodeState(message) {
    if (!MOUSE_MODES.has(message.mouse)) fail("bad mouse mode");
    for (const field of ["focus", "alt_screen", "history_trimmed"]) {
      if (typeof message[field] !== "boolean") fail(`bad ${field}`);
    }
    return {
      mouse: message.mouse,
      focus: message.focus,
      altScreen: message.alt_screen,
      defaultFg: decodeDefaultColor(message.default_fg, "foreground"),
      defaultBg: decodeDefaultColor(message.default_bg, "background"),
      historyTrimmed: message.history_trimmed,
      exit: decodeExit(message.exit),
    };
  }

  // ## History storage

  /**
   * The history lines, oldest first, as an array with a moving start so
   * dropping the oldest lines at the cap is not a copy per append. The
   * array is compacted once the dead prefix outgrows the live part.
   *
   * A dropped line's slot is cleared as it is dropped, not at compaction:
   * the rows are the expensive part (their runs and text), and holding a
   * cap's worth of evicted rows until the next compaction doubled the
   * history's memory at cap turnover (measured: 168 MiB instead of 84 MiB
   * for 12,000 per-cell-styled lines at 200 columns).
   */
  class LineStore {
    constructor() {
      this.lines = [];
      this.start = 0;
    }

    get length() {
      return this.lines.length - this.start;
    }

    push(row) {
      this.lines.push(row);
    }

    dropOldest(count) {
      const end = this.start + Math.min(count, this.length);
      for (let i = this.start; i < end; i++) this.lines[i] = undefined;
      this.start = end;
      if (this.start > 1024 && this.start * 2 > this.lines.length) {
        this.lines = this.lines.slice(this.start);
        this.start = 0;
      }
    }

    get(index) {
      return index >= 0 && index < this.length ? this.lines[this.start + index] : undefined;
    }

    clear() {
      this.lines = [];
      this.start = 0;
    }

    /** How many rows the backing array still holds (for tests). */
    get retained() {
      let held = 0;
      for (const line of this.lines) if (line !== undefined) held++;
      return held;
    }
  }

  // ## The model

  function freshDamage() {
    return {
      all: false,
      size: false,
      shift: 0,
      rows: new Set(),
      cursor: false,
      state: false,
      history: false,
      // History was cleared (a `history_reset` was applied): line numbers
      // start over, so anything anchored to one (a scrolled-back view, a
      // selection) is void.
      historyReset: false,
    };
  }

  /**
   * One attachment's view of a grid terminal. Feed it messages with
   * `apply`; read what to draw from `screen`, `cursor` and `state`, and
   * what changed from `takeDamage()`.
   *
   * A model belongs to exactly one attachment: the format starts every
   * attachment with a `hello`, and a reconnect builds a new model rather
   * than reusing this one.
   */
  class GridModel {
    constructor() {
      this.started = false;
      this.cols = 0;
      this.rows = 0;
      this.historyCap = 0;
      this.screen = [];
      // Rows received for the next frame, by screen row. Keys are below
      // MAX_DIMENSION, so this holds at most that many rows however many
      // `rows` messages arrive before their frame.
      this.pending = new Map();
      // Decoded styles by key (see `internStyle`).
      this.styles = new Map();
      // Whether a screen update has begun (a `rows` message) and not been
      // committed, and the clipboard write whose parts are still arriving:
      // both must be contiguous (see `apply`).
      this.updateOpen = false;
      this.clipboardPending = null;
      this.clipboardWrites = [];
      this.rev = -1;
      this.cursor = { x: 0, y: 0, visible: false, shape: "block", blink: false };
      this.history = new LineStore();
      // Absolute line number of the oldest kept history line, and of the
      // line after the newest applied one (`null` after a hello or reset,
      // when the first append may start anywhere).
      this.historyFirst = 0;
      this.historyNext = null;
      // Where the next `history` message must start, counting the queued
      // ones (`null` when the next may start anywhere): the contiguity rule
      // is checked when a message arrives, not when it is applied.
      this.historyExpected = null;
      // See "The HISTORY QUEUE" in this file's header. Entries are
      // `{reset: true}` (`trim: true` when `trimQueue` made it) or `{first, cols, rows, links, offset}` with `rows`
      // and `links` still raw; `queuedRows` counts the rows not yet applied.
      this.deferHistory = false;
      this.historyQueue = [];
      this.queuedRows = 0;
      this.state = {
        mouse: "off",
        focus: false,
        altScreen: false,
        defaultFg: null,
        defaultBg: null,
        historyTrimmed: false,
        exit: null,
      };
      this.damage = freshDamage();
    }

    /**
     * Validate and apply one decoded message. Throws `GridProtocolError`
     * without changing the screen, cursor or state when the message
     * violates the format. History is the exception: its rows are checked
     * as they are applied (see "The HISTORY QUEUE"), so a bad history row
     * can leave the rows before it applied. Every caller ends the
     * attachment on a protocol error, so that history is never shown.
     */
    apply(message) {
      if (!this.started && message.t !== "hello") fail("first message is not hello");
      // The format's ordering: a screen update (its `rows` and the `frame`
      // that commits it) and the parts of one clipboard write are each
      // contiguous; nothing comes between their messages.
      if (this.updateOpen && message.t !== "rows" && message.t !== "frame") fail("a screen update was interrupted");
      if (this.clipboardPending && message.t !== "clipboard") fail("a clipboard write was interrupted");
      switch (message.t) {
        case "hello":
          return this.applyHello(message);
        case "state":
          return this.applyState(message);
        case "rows":
          return this.applyRows(message);
        case "frame":
          return this.applyFrame(message);
        case "history":
          return this.applyHistory(message);
        case "history_reset":
          return this.applyHistoryReset();
        case "clipboard":
          return this.applyClipboard(message);
        default:
          return fail("unknown message type");
      }
    }

    applyHello(message) {
      if (this.started) fail("second hello");
      if (!isInt(message.cols, 1, MAX_DIMENSION) || !isInt(message.rows, 1, MAX_DIMENSION)) fail("bad hello size");
      if (!isInt(message.history_cap, 0, MAX_HISTORY_CAP)) fail("bad history cap");
      this.started = true;
      this.cols = message.cols;
      this.rows = message.rows;
      this.historyCap = message.history_cap;
      this.screen = blankScreen(this.rows);
      this.damage.all = true;
      this.damage.size = true;
    }

    applyState(message) {
      this.state = decodeState(message);
      this.damage.state = true;
    }

    applyRows(message) {
      if (!Array.isArray(message.rows)) fail("bad rows");
      const links = decodeLinks(message.links);
      // Decoded in full before anything is buffered, so a bad entry leaves
      // the pending update exactly as it was.
      const decoded = message.rows.map((entry) => {
        if (!Array.isArray(entry) || entry.length !== 2) fail("bad rows entry");
        if (!isInt(entry[0], 0, MAX_DIMENSION - 1)) fail("row index out of range");
        return [entry[0], decodeRow(entry[1], MAX_DIMENSION, links, this.styles)];
      });
      for (const [y, row] of decoded) this.pending.set(y, row);
      this.updateOpen = true;
    }

    applyFrame(message) {
      if (!Number.isSafeInteger(message.rev) || message.rev <= this.rev) fail("frame revision did not increase");
      if (!isInt(message.cols, 1, MAX_DIMENSION) || !isInt(message.rows, 1, MAX_DIMENSION)) fail("bad frame size");
      const cols = message.cols;
      const rows = message.rows;
      const shift = message.shift === undefined ? 0 : message.shift;
      if (!isInt(shift, 0, rows)) fail("bad frame shift");
      const cursor = decodeCursor(message.cursor);
      // Every buffered row must fit the committing frame; checked before
      // the screen is touched.
      for (const [y, row] of this.pending) {
        if (y >= rows) fail("row index past the screen");
        if (row.end > cols) fail("run past the last column");
      }

      const damage = this.damage;
      if (cols !== this.cols || rows !== this.rows) {
        // A resized screen keeps nothing: rows this frame does not update
        // are blank (the format's commit order: resize, shift, rows).
        this.cols = cols;
        this.rows = rows;
        this.screen = blankScreen(rows);
        damage.all = true;
        damage.size = true;
      } else if (shift > 0) {
        this.screen.splice(0, shift);
        for (let i = 0; i < shift; i++) this.screen.push(BLANK_ROW);
        noteShift(damage, shift, rows);
      }
      for (const [y, row] of this.pending) {
        this.screen[y] = row;
        damage.rows.add(y);
      }
      this.pending.clear();
      this.updateOpen = false;
      // Clamped rather than refused: a cursor one past the last column is
      // a pending-wrap state some emulators report, and drawing it at the
      // edge is harmless.
      this.cursor = {
        ...cursor,
        x: Math.min(cursor.x, cols - 1),
        y: Math.min(cursor.y, rows - 1),
      };
      this.rev = message.rev;
      damage.cursor = true;
    }

    /**
     * Accept a `history` message: check what can be checked without
     * decoding its rows (the line numbers, contiguity, the width, the links
     * table) and queue it. The rows are validated as they are applied
     * (`drainHistory`), so a bad row is reported then.
     */
    applyHistory(message) {
      if (!Number.isSafeInteger(message.first) || message.first < 0) fail("bad history line number");
      if (this.historyExpected !== null && message.first !== this.historyExpected) fail("history is not contiguous");
      if (!Array.isArray(message.rows)) fail("bad history rows");
      // Compared by subtraction: the sum itself can round back into range.
      if (message.rows.length > 0 && message.rows.length - 1 > Number.MAX_SAFE_INTEGER - message.first) {
        fail("history line numbers past the largest safe integer");
      }
      // Each history line keeps the width it was written at (its message's
      // `cols`), which need not be the screen's: joining soft-wrapped lines
      // for selection needs it to keep their trailing blanks.
      if (!isInt(message.cols, 1, MAX_DIMENSION)) fail("bad history width");
      const links = decodeLinks(message.links);
      this.historyExpected = message.first + message.rows.length;
      this.historyQueue.push({ first: message.first, cols: message.cols, rows: message.rows, links, offset: 0 });
      this.queuedRows += message.rows.length;
      this.trimQueue();
      if (!this.deferHistory) this.drainHistory();
    }

    applyHistoryReset() {
      // Nothing queued before a reset can matter any more.
      this.historyQueue = [{ reset: true }];
      this.queuedRows = 0;
      this.historyExpected = null;
      if (!this.deferHistory) this.drainHistory();
    }

    /**
     * Keep the queue within what applying it could leave in history: once
     * more rows are queued than `history_cap`, the oldest queued rows would
     * only be applied to be dropped again, together with every line already
     * applied. So they are dropped now, unread, behind a reset that clears
     * the applied lines when it is reached. This bounds what the queue holds
     * by the cap however far applying falls behind the socket, and a row
     * dropped this way is never decoded (its bounds are never checked, which
     * is harmless: nothing ever reads it).
     */
    trimQueue() {
      let excess = this.queuedRows - this.historyCap;
      if (excess <= 0) return;
      // A trim, not a `history_reset`: the kept rows keep their line numbers,
      // so nothing anchored to them is void (`damage.historyReset` stays
      // false; see `drainHistory`). Unless the queue held a real reset not
      // yet applied: that one must still void the anchors.
      const queueHadRealReset = this.historyQueue.some((entry) => entry.reset && !entry.trim);
      const kept = [{ reset: true, trim: !queueHadRealReset }];
      for (const entry of this.historyQueue) {
        if (entry.reset) continue;
        const left = entry.rows.length - entry.offset;
        // An empty append drops nothing, but still settles where history
        // starts, so it stays.
        if (left > 0 && excess >= left) {
          excess -= left;
          this.queuedRows -= left;
          continue;
        }
        if (excess > 0) {
          entry.offset += excess;
          this.queuedRows -= excess;
          excess = 0;
        }
        kept.push(entry);
      }
      this.historyQueue = kept;
    }

    /** Whether accepted history is still waiting to be applied. */
    get historyPending() {
      return this.historyQueue.length > 0;
    }

    /**
     * Apply queued history, oldest first, until the queue is empty or
     * `shouldYield()` (checked between rows) says to stop. Returns whether
     * anything is still queued. Throws `GridProtocolError` on a row that
     * breaks the format; rows before it in the same message stay applied
     * (the caller ends the attachment on a protocol error, so the partial
     * history is never read).
     */
    drainHistory(shouldYield) {
      while (this.historyQueue.length > 0) {
        const entry = this.historyQueue[0];
        if (entry.reset) {
          this.history.clear();
          this.historyNext = null;
          this.historyFirst = 0;
          this.damage.history = true;
          if (!entry.trim) this.damage.historyReset = true;
          this.historyQueue.shift();
          continue;
        }
        if (this.historyNext === null) this.historyFirst = entry.first + entry.offset;
        while (entry.offset < entry.rows.length) {
          const row = decodeRow(entry.rows[entry.offset], entry.cols, entry.links, this.styles);
          row.cols = entry.cols;
          this.history.push(row);
          entry.offset++;
          this.queuedRows--;
          this.historyNext = entry.first + entry.offset;
          this.damage.history = true;
          if (this.history.length > this.historyCap) {
            this.history.dropOldest(1);
            this.historyFirst++;
          }
          if (shouldYield && shouldYield()) {
            if (entry.offset >= entry.rows.length) this.historyQueue.shift();
            return this.historyQueue.length > 0;
          }
        }
        // An empty append still settles where history starts.
        if (this.historyNext === null) this.historyNext = entry.first;
        this.historyQueue.shift();
      }
      return false;
    }

    /**
     * The absolute line number of screen row 0: the line after the last
     * history line accepted, queued or not. Screen rows keep their numbers
     * while queued history is applied, so a view or selection anchored to a
     * line number does not move under the user as the queue drains.
     */
    get screenBase() {
      if (this.historyExpected !== null) return this.historyExpected;
      if (this.historyNext !== null) return this.historyNext;
      return this.historyFirst + this.history.length;
    }

    /** The absolute line number after the newest APPLIED history line. */
    get historyEnd() {
      return this.historyFirst + this.history.length;
    }

    /**
     * Collect one part of a program's clipboard write (OSC 52, live output
     * only). Parts are contiguous and share a target; the write is complete
     * at the part marked `last`, and `takeClipboard` hands it out. Bounded
     * as the format bounds it: 512 KiB of text per part, 8 MiB per write,
     * counted in UTF-8 bytes.
     *
     * What is held is bounded by those bytes, not by how many parts a peer
     * sends: an empty part adds nothing, and small parts are gathered into
     * chunks of at least 64 KiB, so a write held open with endless empty or
     * one-character parts holds memory in proportion to its bytes, never to
     * its part count.
     */
    applyClipboard(message) {
      if (message.target !== "clipboard" && message.target !== "primary") fail("bad clipboard target");
      if (typeof message.text !== "string" || typeof message.last !== "boolean") fail("bad clipboard part");
      const bytes = utf8Length(message.text);
      if (bytes > MAX_CLIPBOARD_PART_BYTES) fail("clipboard part larger than 512 KiB");
      const pending = this.clipboardPending;
      if (pending && pending.target !== message.target) fail("clipboard target changed mid-write");
      const total = (pending ? pending.bytes : 0) + bytes;
      if (total > MAX_CLIPBOARD_BYTES) fail("clipboard write larger than 8 MiB");
      const held = pending || { target: message.target, chunks: [], small: [], smallBytes: 0, bytes: 0 };
      held.bytes = total;
      if (bytes > 0) {
        held.small.push(message.text);
        held.smallBytes += bytes;
        if (held.smallBytes >= CLIPBOARD_CHUNK_BYTES) {
          held.chunks.push(held.small.join(""));
          held.small = [];
          held.smallBytes = 0;
        }
      }
      if (message.last) {
        this.clipboardPending = null;
        this.clipboardWrites.push({ target: message.target, text: held.chunks.join("") + held.small.join("") });
      } else {
        this.clipboardPending = held;
      }
    }

    /** How many strings an unfinished clipboard write holds (for tests). */
    get clipboardHeldStrings() {
      const held = this.clipboardPending;
      return held ? held.chunks.length + held.small.length : 0;
    }

    /** Completed clipboard writes since the last call, oldest first. */
    takeClipboard() {
      const writes = this.clipboardWrites;
      this.clipboardWrites = [];
      return writes;
    }

    /** What changed since the last call, and start a fresh record. */
    takeDamage() {
      const damage = this.damage;
      this.damage = freshDamage();
      return damage;
    }

    /** History line `index` (0 = the oldest kept), or undefined. */
    historyLine(index) {
      return this.history.get(index);
    }

    get historyLength() {
      return this.history.length;
    }

    /**
     * The cell at a screen position, for drawing the cursor over it:
     * `{text, cells, style}` with `cells` 2 for a wide character, or a
     * blank default cell where no run covers the position.
     */
    cellAt(x, y) {
      const row = this.screen[y];
      if (!row) return { text: " ", cells: 1, style: null, col: x };
      for (const run of row.runs) {
        if (x < run.col) break;
        if (x >= run.col + run.width) continue;
        const text = displayText(run.text);
        // A printable ASCII run has a character per cell; any other run is
        // one cell (two for a wide character), which the cursor covers whole.
        if (layoutRun(text, run.width) === null) return { text: text[x - run.col], cells: 1, style: run.style, col: x };
        return { text, cells: run.width, style: run.style, col: run.col };
      }
      return { text: " ", cells: 1, style: null, col: x };
    }
  }

  /**
   * Record a scroll of `shift` rows in `damage`: rows already marked move
   * up with their content, the uncovered bottom rows are new, and a
   * renderer can move its row elements by the accumulated shift instead of
   * redrawing them.
   */
  function noteShift(damage, shift, rows) {
    if (damage.all) return;
    damage.shift += shift;
    if (damage.shift >= rows) {
      damage.all = true;
      return;
    }
    const moved = new Set();
    for (const y of damage.rows) if (y - shift >= 0) moved.add(y - shift);
    for (let y = rows - shift; y < rows; y++) moved.add(y);
    damage.rows = moved;
  }

  // ## Run layout

  // Bidirectional formatting characters (ALM, LRM, RLM, the embeddings and
  // overrides, the isolates). They occupy no cell and draw nothing, but in
  // the page they would reorder the characters after them even inside the
  // renderer's left-to-right override, letting a program show text in an
  // order it did not write. Cells are already in visual order, so the
  // renderer drops them; the model keeps the text as received.
  const BIDI_CONTROL = /[\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/;
  const BIDI_CONTROLS = /[\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/g;

  /** A run's text as it is drawn: without bidirectional formatting characters. */
  function displayText(text) {
    return BIDI_CONTROL.test(text) ? text.replace(BIDI_CONTROLS, "") : text;
  }

  const PRINTABLE_ASCII = /^[\x20-\x7e]*$/;

  /**
   * How a run's text (as `displayText` gives it) is laid out in its cells.
   *
   * `null` means plain flow: the text is printable ASCII with one
   * character per cell, which the bundled monospace font draws at exactly
   * the cell width. Any other run is one cell's content (the format's
   * rule): `[[text, cells]]`, one box of exactly its width, so a glyph whose
   * advance is not the cell width (a fallback font's symbol, CJK or emoji,
   * a combining sequence) cannot push later columns out of place. The
   * renderer may still let a single-width character flow when it measures
   * the font drawing it at exactly the cell width (grid-terminal.js,
   * `flows`).
   */
  function layoutRun(text, width) {
    if (text.length === width && PRINTABLE_ASCII.test(text)) return null;
    return [[text, width]];
  }

  const api = {
    GRID_MAX_MESSAGE,
    MAX_DIMENSION,
    MAX_HISTORY_CAP,
    DIRECT_COLOR_BASE,
    BLANK_ROW,
    GridProtocolError,
    GridModel,
    decodeFrame,
    decodeStyle,
    displayText,
    layoutRun,
    utf8Length,
  };
  root.farhelmGridModel = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(typeof window === "object" ? window : globalThis);
