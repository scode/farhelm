// The grid terminal: draws a grid stream's model into the page and turns
// the user's keyboard, mouse and focus into input messages. The DOM half
// of the grid client; the decisions it acts on are in grid-model.js,
// grid-style.js, grid-input.js and grid-buffer.js, where `node --test`
// covers them.
//
// terminal.js decides per attachment which component a terminal gets
// (xterm.js for byte streams, this for grid streams) and owns everything
// about the connection: the socket, catch-up, heartbeat, reconnect,
// banners, and what a clipboard write or a link click is allowed to do.
// This file only needs a container element, a way to send input messages,
// and to be fed the data frames.
//
// ## Rendering (the approach the renderer benchmark chose)
//
// - One element per visible row, in normal block flow, reused for the life
//   of the terminal. A scroll (`frame.shift`) moves row elements from the
//   top to the bottom instead of redrawing every row.
// - Each row's runs are spans, reused in place: a repaint compares the
//   cached style string and the text before touching either.
// - Plain ASCII runs flow inline at the bundled font's advance, which is
//   the cell width, and so do non-ASCII characters the font draws at exactly
//   that width (box drawing, block elements, braille: `flows`). Every other
//   character goes in a box of exactly its cell width, so a glyph drawn from
//   a fallback font, whatever its advance, cannot push later columns out of
//   place. Adjacent flowing text of one style shares a span. Measured (the
//   flood benchmark, 200 x 60, every row changing every frame): a box for
//   every non-ASCII cell put a box-drawing screen at 2,801 elements and
//   made every frame long in Chromium, where xterm.js (815) and flowing
//   (811) kept about 40% of them on time (on a loaded machine, so the
//   comparison is the finding, not the numbers); braille, color-per-cell
//   bars and dense ASCII cost about the same either way, and in WebKit the
//   grid was no slower than xterm.js in any of them.
// - Styles are inline declarations cached per style (grid-style.js's
//   header says why not stylesheet rules). The structural rules, which do
//   not vary by style, live in app.css under `.grid-terminal`.
// - Every span reads its characters in a left-to-right override: cells
//   are already in visual order, so bidirectional text must not be
//   reordered by the browser (and a program cannot use bidi controls to
//   make the screen show text in an order it was not written in).
// - Cell text is assigned with `textContent`, never parsed as markup.
// - Painting is coalesced to one animation frame however many frames
//   arrive. A terminal hidden by `visibility` (the catch-up, an unselected
//   tab) still paints, invisibly; in a background browser tab animation
//   frames do not run, damage accumulates, and the reveal's `flush()` or
//   the first frame after the tab returns draws it.
//
// ## Scrollback: a virtualized view over history and screen
//
// The rows on the page are only the visible ones. What they show is a
// window onto one buffer of lines (grid-buffer.js): the history the model
// has applied, then the screen, with a notice line on top when recovery
// lost some history (`state.history_trimmed`). Following the bottom, the
// rows are the screen; scrolled back, the window is anchored to the
// absolute number of its top line, so it stays on the same text while
// output continues and while the oldest history is dropped, as xterm.js's
// viewport does. Wheel, Shift+PageUp/Down and a scrollbar move it with
// xterm.js 6.0's amounts; any input sent to the program, a mouse report
// included, returns it to the bottom (`scrollOnUserInput`). History the
// client trims after falling behind keeps its line numbers, so a view
// anchored there holds at the oldest line rather than jumping (only a
// `history_reset`, a resize or the alternate screen let go of the anchor).
// On the alternate screen there is no history to scroll, as in xterm.js's
// alternate buffer.
//
// History is applied in slices (grid-model.js, "The HISTORY QUEUE"): a
// message that arrives with nothing queued gets one slice at once (so live
// output's history is never behind), and a backlog runs in `DRAIN_SLICE_MS`
// slices, one per animation frame while the page is visible (so it paints
// in between) and on message-channel tasks while it is hidden (which a
// background tab does not throttle the way it does timers; a slice waiting
// for a frame when the page is hidden moves to a task). The screen is drawn
// and usable throughout; scrollback fills in oldest first.
//
// ## Selection and copy (the local half of terminal.js's two mechanisms)
//
// A primary-button drag selects when the program has not turned mouse
// reporting on; with it on, the drag goes to the program unless Shift
// (Option on a Mac) forces a local selection. A double click selects a
// word, a triple click the soft-wrapped line, Shift-click extends, Alt-drag
// off a Mac selects a column (block), Cmd+A on a Mac selects everything,
// and a drag past the top or bottom scrolls. A quick Alt-click (Option on a
// Mac) moves the program's cursor there with arrow keys, as xterm.js's
// `altClickMovesCursor` does.
// A selection is anchored to line numbers like the view, so it spans
// history and screen and stays on its text while output arrives; a
// resize, the alternate screen switching, a history reset, or input sent to
// the program clears it. Every gesture that ends with a selection copies it
// (`onCopy`, through copy-on-select.js's decision), even when it is the
// same text again. The drag-copy notice is not built for grid terminals.
//
// ## Links
//
// Links come from a provider per line, like xterm.js's link providers:
// terminal.js's gives the program's hyperlinks, printed URLs and file paths
// with today's hover displays, checks and downloads (without a provider
// there are none). The pointer over a link underlines it and, when the link
// says so, shows a hand. A primary press and release on the same link, with
// no more than xterm.js's 4px of jitter between them, is a click, and the
// link's `activate` gets it (`isLinkClick` names the admitted release).
// Under mouse reporting `linkPress` decides whether the press is withheld
// from the program (the link-click setting); a withheld press, its drag and
// its release are never reported. A hover rests on the row objects its
// `hover()` read; once one is replaced, the hover is left and asked for
// afresh, which is how a redraw withdraws a stale link (see "## Links" in
// `create`).
//
// ## Size: exactly xterm.js's
//
// A pane must give the program the same rows and columns whichever
// component it holds: the socket URL carries xterm.js's size (measured
// before the stream's kind is known), and a grid that fitted differently
// would resize the program again at once, redrawing it and, for a grid
// stream, rebuilding its history. So the cell metrics and the fit are
// xterm.js 6.0's, step for step (the vendored bundle's `CharSizeService`,
// `DomRenderer._updateDimensions` and the fit addon's
// `proposeDimensions`):
//
// - the character is measured with an offscreen canvas's `measureText("W")`
//   (width, and the font bounding box for height), or a 32-W span where the
//   canvas lacks those metrics;
// - the cell is derived through device pixels, rounding the height up per
//   device pixel and the whole canvas to CSS pixels at the CURRENT size,
//   so the cell width can differ from the character width by rounding and
//   the result depends on the size being fitted from;
// - the fit subtracts this element's 4px padding (xterm.js's element
//   carries the same `.terminal` padding) and the 14px the addon reserves
//   for a scrollbar, which is where this component's scrollbar sits.
//
// A browser spec mounts both components in identical containers and holds
// them to the same fitted size across text sizes and pixel ratios. The
// fitted size goes to the program through the existing resize control
// message; what is drawn is always the size the supervisor last committed
// in a frame.
//
// ## Not built
//
// Showing an input method's composition at the cursor (see the composition
// handlers); its committed text is sent.
(function (root) {
  "use strict";

  // The fit addon reserves this for xterm.js's scrollbar whenever there is
  // scrollback; matching it keeps a pane's column count the same.
  const SCROLLBAR_WIDTH = 14;
  const MAX_DIMENSION = 1000;
  const MEASURE_TEXT = "W".repeat(32);
  // xterm.js's size before its first fit.
  const DEFAULT_SIZE = Object.freeze({ cols: 80, rows: 24 });
  // How long one slice of history application may run before it yields
  // (see "Scrollback" in this file's header): half a 60 Hz frame, so input
  // and painting keep their turn while a large history is applied.
  const DRAIN_SLICE_MS = 8;
  // xterm.js's selection color (its theme default, which Farhelm's theme
  // does not override), drawn over the text.
  const SELECTION_COLOR = "rgba(255, 255, 255, 0.3)";
  // A drag that leaves the screen scrolls it by up to this many lines per
  // tick, faster the farther out (xterm.js's DRAG_SCROLL_MAX_SPEED and
  // DRAG_SCROLL_MAX_THRESHOLD, at its 50 ms interval).
  const DRAG_SCROLL_MAX_LINES = 15;
  const DRAG_SCROLL_MAX_DISTANCE = 50;
  const DRAG_SCROLL_INTERVAL_MS = 50;
  // VS Code's scrollbar (xterm.js 6.0's): the slider never shrinks below
  // this, and it fades out this long after the last scroll once the pointer
  // has left the terminal.
  const SLIDER_MIN_HEIGHT = 20;
  const SCROLLBAR_FADE_MS = 800;
  // A press and release this close together are a click, not a drag
  // (terminal-links.js's `createLinkGesture`).
  const CLICK_JITTER_PX = 4;
  // What the notice line above the history says when recovery lost some
  // of it (`state.history_trimmed`).
  const TRIMMED_TEXT = "Some earlier output could not be recovered";

  /**
   * xterm.js's preferred character measurement: a 2D context whose
   * `measureText` reports the font bounding box, or null where it does not
   * (then the 32-W span is used, as xterm.js falls back to it). One
   * context serves every grid terminal on the page.
   */
  let measureContext;
  function canvasMeasurer() {
    if (measureContext !== undefined) return measureContext;
    measureContext = null;
    try {
      const context = new OffscreenCanvas(100, 100).getContext("2d");
      const metrics = context.measureText("W");
      if ("width" in metrics && "fontBoundingBoxAscent" in metrics && "fontBoundingBoxDescent" in metrics) {
        measureContext = context;
      }
    } catch (_error) {
      // No OffscreenCanvas: the span fallback below.
    }
    return measureContext;
  }

  /**
   * The CSS cell size xterm.js's DOM renderer uses for a character of
   * `charWidth` x `charHeight` at `cols` x `rows` and pixel ratio `dpr`
   * (`DomRenderer._updateDimensions`, with no letter spacing and a line
   * height of 1, as Farhelm configures xterm.js).
   */
  function cellFor(charWidth, charHeight, cols, rows, dpr) {
    const deviceWidth = charWidth * dpr;
    const deviceHeight = Math.floor(Math.ceil(charHeight * dpr));
    return {
      width: Math.round((deviceWidth * cols) / dpr) / cols,
      height: Math.round((deviceHeight * rows) / dpr) / rows,
      // `consumeWheelEvent`'s row height: the device cell over the ratio.
      wheel: deviceHeight / dpr,
    };
  }

  /**
   * The fit addon's `proposeDimensions`: the size that fits `available`
   * (the container's computed width and height, minus this element's
   * padding) at the cell metrics of the current size. Never below 2 x 1,
   * and never past the format's 1000 x 1000.
   */
  function proposeSize(availableWidth, availableHeight, cell) {
    return {
      cols: Math.min(MAX_DIMENSION, Math.max(2, Math.floor((availableWidth - SCROLLBAR_WIDTH) / cell.width))),
      rows: Math.min(MAX_DIMENSION, Math.max(1, Math.floor(availableHeight / cell.height))),
    };
  }

  /**
   * The exit line shown over a terminal whose program has ended, outside
   * the emulator's content (there is no tmux "Pane is dead" banner).
   */
  function exitText(exit) {
    if (!exit) return "";
    if ("signal" in exit) return `Process ended by signal ${exit.signal}`;
    return `Process exited with status ${exit.code}`;
  }

  /**
   * How many pixels one wheel event scrolls the view, as xterm.js 6.0's
   * scrollable element computes it (VS Code's `StandardWheelEvent` and
   * `_onMouseWheel`): the event normalized to notches (`wheelDeltaY` / 120
   * where the engine has it, else lines or pixels / 40), 50 pixels a notch,
   * five times that with Alt. Positive scrolls toward older lines. Shift
   * turns the wheel horizontal off a Mac, which scrolls nothing here.
   */
  function wheelPixels(ev, isMac) {
    if (ev.shiftKey && !isMac) return 0;
    let notches;
    if (typeof ev.wheelDeltaY === "number") notches = ev.wheelDeltaY / 120;
    else if (ev.deltaMode === 1) notches = -ev.deltaY;
    else if (ev.deltaMode === 2) notches = -ev.deltaY * 3;
    else notches = -ev.deltaY / 40;
    let pixels = 50 * notches;
    if (ev.altKey) pixels *= 5;
    return pixels < 0 ? Math.floor(pixels) : Math.ceil(pixels);
  }

  /**
   * Mount a grid terminal into `container`.
   *
   * `options`: `fontSize` and `fontFamily` (the same as the xterm.js
   * terminals use), `theme` (terminal-theme.js), `platform`
   * (`navigator.platform`), `send(message)` for input messages,
   * `onResize(cols, rows)` when the fitted size changes, and `size`, the
   * size the program already has (the one the socket URL carried; xterm.js's
   * initial 80 x 24 when omitted), which a fit reports only if it differs,
   * `onClipboard(text, target)` for a program's completed clipboard write
   * ("clipboard" or "primary"), `onCopy(text)` for a selection the user
   * completed, `links` (a link provider, see "## Links" inside; no links
   * without one), `linkPress(event, link)` to ask whether a press on a link is withheld
   * from a program with mouse reporting on (see "Links"),
   * `onProtocolError(error)` for a format violation found in history while
   * it is applied after `receive` returned, `sanitizePaste(text)` applied to
   * text this component pastes from a paste event itself, `onDispose()`
   * when it is disposed (for what a caller hosts on it), and
   * `flowCovered: false` to box every non-ASCII cell (for the renderer
   * benchmark's comparison only).
   *
   * Feed data frames to `receive`, which throws
   * `farhelmGridModel.GridProtocolError` on a frame that violates the
   * format (the caller ends the attachment). Call `dispose` when the
   * attachment ends; it removes everything this mounted, including the
   * document listeners a mouse gesture installs. `freeze` keeps the last
   * screen on display without input (a takeover's snapshot).
   */
  function create(container, options) {
    const gridModel = root.farhelmGridModel;
    const gridStyle = root.farhelmGridStyle;
    const gridInput = root.farhelmGridInput;
    const gridBuffer = root.farhelmGridBuffer;
    const copyOnSelect = root.farhelmCopyOnSelect;
    const model = new gridModel.GridModel();
    model.deferHistory = true;
    const styles = gridStyle.createStyleMapper(options.theme);
    const platform = options.platform || "";
    // Mac-ness as xterm.js reads it (grid-input.js's `platformOf`).
    const isMac = gridInput.platformOf(platform).isMac;
    const isLinux = /Linux/.test(platform);
    let send = options.send;
    let onResize = options.onResize;
    let onClipboard = options.onClipboard;
    let onCopy = options.onCopy;
    let linkPress = options.linkPress;
    let onProtocolError = options.onProtocolError;
    let onDispose = options.onDispose;
    const sanitizePaste = options.sanitizePaste || ((text) => text);
    // On unless a caller (the renderer benchmark) asks for a box per
    // non-ASCII cell; see `flows`.
    const flowCovered = options.flowCovered !== false;
    let disposed = false;
    // A frozen terminal (a takeover's snapshot) keeps its screen, its
    // scrollback, selection and links, and sends nothing.
    let frozen = false;

    // ## DOM

    const element = document.createElement("div");
    element.className = "grid-terminal";
    const screen = document.createElement("div");
    screen.className = "grid-terminal-screen";
    // Decorative to assistive technology for now, as xterm.js's rows are;
    // an accessible reading of the grid is not part of today's terminal.
    screen.setAttribute("aria-hidden", "true");
    // The selection and the hovered link are drawn over the rows, under the
    // cursor; both ignore the pointer.
    const selectionLayer = document.createElement("div");
    selectionLayer.className = "grid-terminal-selection";
    const linkLayer = document.createElement("div");
    linkLayer.className = "grid-terminal-link-underline";
    const cursorEl = document.createElement("div");
    cursorEl.className = "grid-terminal-cursor";
    screen.append(selectionLayer, linkLayer, cursorEl);
    const scrollbar = document.createElement("div");
    scrollbar.className = "grid-terminal-scrollbar";
    const slider = document.createElement("div");
    slider.className = "grid-terminal-slider";
    scrollbar.appendChild(slider);
    const exitEl = document.createElement("div");
    exitEl.className = "grid-terminal-exit";
    // The keyboard's target: a hidden text field, as xterm.js uses, so the
    // browser's own text input (dead keys, Option characters, input
    // methods) has somewhere to deliver characters.
    const input = document.createElement("textarea");
    input.className = "grid-terminal-input";
    input.setAttribute("aria-label", "Terminal input");
    input.setAttribute("autocorrect", "off");
    input.setAttribute("autocapitalize", "off");
    input.setAttribute("spellcheck", "false");
    input.setAttribute("tabindex", "0");
    // WebKit's inline predictions would otherwise offer completions for
    // what the field holds (see terminal.js's note on xterm.js's own
    // textarea); this field is emptied after every input anyway.
    input.setAttribute("writingsuggestions", "false");
    const measure = document.createElement("span");
    measure.className = "grid-terminal-measure";
    measure.setAttribute("aria-hidden", "true");
    measure.textContent = MEASURE_TEXT;
    // A second hidden span for `flows`' per-character advances, styled as
    // the row spans are (bidi override, kerning off) so a character
    // measures as it will draw.
    const glyphMeasure = document.createElement("span");
    glyphMeasure.className = "grid-terminal-measure grid-terminal-glyph-measure";
    glyphMeasure.setAttribute("aria-hidden", "true");
    element.append(screen, scrollbar, exitEl, input, measure, glyphMeasure);
    container.appendChild(element);

    element.style.fontFamily = options.fontFamily;
    element.style.fontSize = `${options.fontSize}px`;

    // ## Metrics and fit

    let fontSize = options.fontSize;
    // The measured character (CSS pixels), 0 until a measurement succeeds.
    let charWidth = 0;
    let charHeight = 0;
    // The cell metrics the screen is drawn with (see `currentCell`).
    let cellWidth = 0;
    let cellHeight = 0;
    let wheelRowHeight = 0;
    // The size the program was last told about: the size the socket URL
    // carried (`options.size`, xterm.js's fit before the swap) until this
    // component fits differently. Like xterm.js's buffer size, it is also
    // the size the next fit's cell metrics are computed at.
    let fitted = { ...(options.size || DEFAULT_SIZE) };

    /**
     * Measure the character as xterm.js's `CharSizeService` does. Returns
     * whether the measurement changed; a font with no metrics yet (no
     * layout box for the span fallback) leaves the previous one.
     */
    function measureChar() {
      let width;
      let height;
      const context = canvasMeasurer();
      if (context) {
        context.font = `${fontSize}px ${options.fontFamily}`;
        const metrics = context.measureText("W");
        width = metrics.width;
        height = metrics.fontBoundingBoxAscent + metrics.fontBoundingBoxDescent;
      } else {
        width = measure.offsetWidth / MEASURE_TEXT.length;
        height = measure.offsetHeight;
      }
      if (!(width > 0) || !(height > 0)) return false;
      if (width === charWidth && height === charHeight) return false;
      charWidth = width;
      charHeight = height;
      return true;
    }

    function pixelRatio() {
      return root.devicePixelRatio || 1;
    }

    /**
     * Bring the drawing metrics up to date for the size on screen (the
     * model's, or the fitted size before the first frame), as xterm.js's
     * renderer recomputes them on every resize. Returns whether they
     * changed, in which case every row must be laid out again.
     */
    function currentCell() {
      if (!charWidth && !measureChar()) return false;
      const cols = model.cols || fitted.cols;
      const rows = model.rows || fitted.rows;
      const cell = cellFor(charWidth, charHeight, cols, rows, pixelRatio());
      wheelRowHeight = cell.wheel;
      if (cell.width === cellWidth && cell.height === cellHeight) return false;
      cellWidth = cell.width;
      cellHeight = cell.height;
      return true;
    }

    /**
     * Fit the grid to its container exactly as xterm.js's fit addon fits
     * xterm.js (see "Size" in this file's header) and report a size that
     * differs from the last one reported or carried through `onResize`. A
     * container with no layout box, or no font metrics yet, proposes
     * nothing.
     */
    function fit() {
      if (!charWidth) {
        if (!measureChar()) return;
        // The first metrics arrived here (the container had no layout box
        // until now): lay the rows out, which a paint does not do by itself
        // when the fitted size turns out unchanged.
        if (currentCell()) repaintAll = true;
        schedulePaint();
      }
      const outer = root.getComputedStyle(container);
      const inner = root.getComputedStyle(element);
      const height = parseInt(outer.getPropertyValue("height"), 10) -
        (parseInt(inner.getPropertyValue("padding-top"), 10) + parseInt(inner.getPropertyValue("padding-bottom"), 10));
      const width = Math.max(0, parseInt(outer.getPropertyValue("width"), 10)) -
        (parseInt(inner.getPropertyValue("padding-right"), 10) + parseInt(inner.getPropertyValue("padding-left"), 10));
      if (!Number.isFinite(height) || !Number.isFinite(width)) return;
      const cell = cellFor(charWidth, charHeight, fitted.cols, fitted.rows, pixelRatio());
      const size = proposeSize(width, height, cell);
      if (size.cols === fitted.cols && size.rows === fitted.rows) return;
      fitted = size;
      if (onResize) onResize(size.cols, size.rows);
    }

    /** Re-measure after a font or pixel-ratio change, redraw, and refit. */
    function remeasure() {
      measureChar();
      advanceCache = new Map();
      referenceCache = new Map();
      if (currentCell()) repaintAll = true;
      paint();
      fit();
    }

    // ## The view over history and screen
    //
    // Lines are numbered as grid-buffer.js numbers them. The view is the
    // combined list [notice?, applied history..., screen rows...] (see
    // "Scrollback" in this file's header), indexed from 0; `viewAbs` anchors
    // its top to a line number, or is null while following the bottom.

    let viewAbs = null;
    // The top index the rows were last painted from, and whether the view
    // was following then: a change in either redraws every row.
    let paintedTop = -1;
    let paintedFollowing = true;

    function historyShown() {
      return !model.state.altScreen;
    }

    function noticeShown() {
      return model.state.historyTrimmed && historyShown();
    }

    /** The view's shape: the notice and history counts, total and the bottom top. */
    function viewShape() {
      const notice = noticeShown() ? 1 : 0;
      const history = historyShown() ? model.historyLength : 0;
      const total = notice + history + model.rows;
      return { notice, history, total, bottom: total - model.rows };
    }

    /** The line number the view's index `i` shows; the notice is the line before the oldest. */
    function absAt(i, shape) {
      if (i < shape.notice) return model.historyFirst - 1;
      if (i < shape.notice + shape.history) return model.historyFirst + (i - shape.notice);
      return model.screenBase + (i - shape.notice - shape.history);
    }

    /** The view index of line `abs`, clamped into the lines that are there. */
    function indexOf(abs, shape) {
      if (shape.history > 0 && abs < model.historyEnd) {
        if (abs < model.historyFirst) return 0;
        return shape.notice + (abs - model.historyFirst);
      }
      if (abs < model.screenBase) return abs < model.historyFirst ? 0 : shape.notice + shape.history;
      return Math.min(shape.total - 1, shape.notice + shape.history + (abs - model.screenBase));
    }

    /**
     * The index of the top visible line now. An anchor is kept even while
     * it clamps (history being refilled after a trim, or its line dropped at
     * the cap shows the oldest line, as xterm.js holds its viewport there);
     * only scrolling to the bottom or something that voids line numbers
     * (`noteDamage`) lets go of it.
     */
    function topIndex(shape = viewShape()) {
      if (viewAbs === null) return shape.bottom;
      return Math.min(shape.bottom, Math.max(0, indexOf(viewAbs, shape)));
    }

    function following() {
      return viewAbs === null;
    }

    /**
     * Move the view's top to index `top` (clamped); the bottom resumes
     * following. Any part-line the wheel had gathered is dropped unless the
     * wheel itself is moving the view (`keepWheelRemainder`).
     */
    function scrollToIndex(top, keepWheelRemainder = false) {
      const shape = viewShape();
      const clamped = Math.min(shape.bottom, Math.max(0, Math.round(top)));
      viewAbs = clamped >= shape.bottom ? null : absAt(clamped, shape);
      if (!keepWheelRemainder) wheelRemainder = 0;
      showScrollbarActivity();
      schedulePaint();
    }

    function scrollLines(count) {
      if (count) scrollToIndex(topIndex() + count);
    }

    function scrollToBottom() {
      if (viewAbs === null) return;
      viewAbs = null;
      schedulePaint();
    }

    /** The buffer as grid-buffer.js reads it (history hidden on the alternate screen). */
    function source() {
      return gridBuffer.modelSource(model, historyShown());
    }

    // ## Painting

    const rowEls = [];
    let rafHandle = null;
    let repaintAll = true;
    let focused = false;

    function schedulePaint() {
      if (rafHandle !== null || disposed) return;
      rafHandle = root.requestAnimationFrame(() => {
        rafHandle = null;
        paint();
      });
    }

    /** Make the row elements match the model's row count and the metrics. */
    function layoutRows() {
      while (rowEls.length < model.rows) {
        const row = document.createElement("div");
        row.className = "grid-terminal-row";
        screen.insertBefore(row, selectionLayer);
        rowEls.push(row);
      }
      while (rowEls.length > model.rows) rowEls.pop().remove();
      const height = `${cellHeight}px`;
      for (const row of rowEls) {
        row.style.height = height;
        row.style.lineHeight = height;
      }
      screen.style.width = `${model.cols * cellWidth}px`;
      screen.style.height = `${model.rows * cellHeight}px`;
    }

    /** Move the top `shift` row elements to the bottom, keeping their content. */
    function rotateRows(shift) {
      const moved = rowEls.splice(0, shift);
      for (const row of moved) {
        screen.insertBefore(row, selectionLayer);
        rowEls.push(row);
      }
    }

    /**
     * Bring one span to the given style, box width and text, touching only
     * what differs from what it already shows.
     */
    function setSpan(span, className, css, text) {
      if (span.className !== className) span.className = className;
      if (span.gridCss !== css) {
        span.style.cssText = css;
        span.gridCss = css;
      }
      if (span.textContent !== text) span.textContent = text;
    }

    // Measured advances (see `flows`): one character's per weight and
    // slant, and the reference ("W") per weight and slant, kept apart so a
    // character can never be read as a reference. Both are dropped with the
    // metrics. The characters `flows` measures are a closed set of 416, so
    // with four weight-and-slant variants the cache cannot exceed 1,664
    // entries and needs no eviction.
    let advanceCache = new Map();
    let referenceCache = new Map();

    /**
     * Whether a single-cell character can flow inline like ASCII instead of
     * taking a box of its own.
     *
     * Only box drawing (U+2500-257F), block elements (U+2580-259F) and
     * braille (U+2800-28FF): the families the flood benchmark was about
     * (table and frame borders, progress bars, terminal graphs), where a box
     * per cell made a box-drawing screen 2,801 elements and every Chromium
     * frame long. Each is a single code point drawn on its own: no script
     * shapes it against its neighbors, so the width it measures alone is the
     * width it takes in a span. Every other character, and every shaping
     * script in particular, keeps an exact-width box, which is what stops a
     * glyph from moving the columns after it.
     *
     * And only when the character, measured alone in the rows' own context
     * (a hidden span with the row spans' bidi and font settings, in the run's
     * weight and slant), advances exactly as far as "W", the advance plain
     * ASCII flows at. A character the font lacks is drawn from a fallback at
     * some other advance and stays boxed. A zero advance (no layout box yet,
     * or an invisible character) counts as "box".
     */
    function flows(text, style) {
      if (!flowCovered || text.length !== 1) return false;
      const code = text.charCodeAt(0);
      if (!((code >= 0x2500 && code <= 0x259f) || (code >= 0x2800 && code <= 0x28ff))) return false;
      const bold = (style.flags & gridStyle.FLAGS.BOLD) !== 0;
      const italic = (style.flags & gridStyle.FLAGS.ITALIC) !== 0;
      const variant = `${bold ? 1 : 0}${italic ? 1 : 0}`;
      const key = variant + text;
      let result = advanceCache.get(key);
      if (result !== undefined) return result;
      glyphMeasure.style.fontWeight = bold ? "bold" : "";
      glyphMeasure.style.fontStyle = italic ? "italic" : "";
      const advance = (sample) => {
        glyphMeasure.textContent = sample;
        return glyphMeasure.getBoundingClientRect().width;
      };
      let reference = referenceCache.get(variant);
      if (reference === undefined) {
        reference = advance("W");
        // Without a layout box nothing is decided or remembered yet.
        if (!(reference > 0)) return false;
        referenceCache.set(variant, reference);
      }
      result = Math.abs(advance(text) - reference) < 0.01;
      advanceCache.set(key, result);
      return result;
    }

    /**
     * Draw `row` (a screen or history row) into visible row element `y`,
     * reusing its spans.
     *
     * Adjacent text that flows (printable ASCII, a gap of blank cells, and
     * with `flowCovered` the characters `flows` admits) and shares one style
     * is drawn as one span; every other cell is a box of exactly its width.
     */
    function paintRow(y, row) {
      const rowEl = rowEls[y];
      if (!rowEl) return;
      const spans = rowEl.children;
      let index = 0;
      const nextSpan = () => {
        let span = spans[index++];
        if (!span) {
          span = document.createElement("span");
          rowEl.appendChild(span);
        }
        return span;
      };
      if (!row) {
        // The notice line above history that recovery lost part of: in the
        // terminal's defaults reversed, like the exit line, so it reads as
        // the client's own line rather than the program's output.
        const defaults = styles.defaults();
        setSpan(nextSpan(), "grid-terminal-notice", `color:${defaults.bg};background-color:${defaults.fg};`, TRIMMED_TEXT);
        while (spans.length > index) rowEl.lastChild.remove();
        return;
      }
      // The inline text being gathered, written out when the style changes
      // or a box interrupts it.
      let flowCss = null;
      let flowText = "";
      const flush = () => {
        if (flowCss !== null) setSpan(nextSpan(), "", flowCss, flowText);
        flowCss = null;
        flowText = "";
      };
      const flow = (css, text) => {
        if (css !== flowCss) flush();
        flowCss = css;
        flowText += text;
      };
      let next = 0;
      for (const run of row.runs) {
        // A history line written wider than the screen is cut at its edge.
        if (run.col >= model.cols) break;
        if (run.col > next) flow("", " ".repeat(run.col - next));
        const css = styles.css(run.style);
        // Hidden text keeps its colors and lines but never enters the page.
        const hidden = (run.style.flags & gridStyle.FLAGS.HIDDEN) !== 0;
        const shown = gridModel.displayText(run.text);
        const pieces = gridModel.layoutRun(shown, run.width);
        // A cell whose drawn text is empty (only a bidi control) is still a
        // cell: it keeps its box so the columns after it stay put.
        if (pieces === null || hidden || (run.width === 1 && shown !== "" && flows(shown, run.style))) {
          const visible = Math.min(run.width, model.cols - run.col);
          flow(css, hidden ? " ".repeat(visible) : pieces === null ? shown.slice(0, visible) : shown);
        } else {
          flush();
          for (const [text, cells] of pieces) {
            setSpan(nextSpan(), "grid-terminal-box", `${css}width:${cells * cellWidth}px;`, text);
          }
        }
        next = run.col + run.width;
      }
      flush();
      while (spans.length > index) rowEl.lastChild.remove();
    }

    /**
     * Draw the cursor over its cell: a filled block showing the cell's
     * character in the accent color, a bar or an underline while the
     * terminal has focus, and an outline of the cell when it does not
     * (xterm.js's `cursorInactiveStyle`). Blinks only while focused. It
     * scrolls with the screen, so a view scrolled back far enough hides it.
     */
    function paintCursor(rowOffset) {
      const cursor = model.cursor;
      const y = cursor.y + rowOffset;
      if (!model.started || !cursor.visible || !cellWidth || y < 0 || y >= model.rows) {
        cursorEl.style.display = "none";
        // Forget the cached declarations so showing it again rewrites them.
        cursorEl.gridCss = null;
        return;
      }
      const cell = model.cellAt(cursor.x, cursor.y);
      const colors = styles.cursor();
      let css = `display:block;left:${cell.col * cellWidth}px;top:${y * cellHeight}px;` +
        `width:${cell.cells * cellWidth}px;height:${cellHeight}px;line-height:${cellHeight}px;`;
      let text = "";
      if (!focused) {
        css += `outline:1px solid ${colors.color};outline-offset:-1px;`;
      } else if (cursor.shape === "bar") {
        css += `box-shadow:1px 0 0 ${colors.color} inset;`;
      } else if (cursor.shape === "underline") {
        css += `border-bottom:1px solid ${colors.color};height:${cellHeight - 1}px;`;
      } else {
        css += `background-color:${colors.color};color:${colors.accent};`;
        const flags = cell.style ? cell.style.flags : 0;
        if (flags & gridStyle.FLAGS.BOLD) css += "font-weight:bold;";
        if (flags & gridStyle.FLAGS.ITALIC) css += "font-style:italic;";
        text = flags & gridStyle.FLAGS.HIDDEN ? " " : cell.text;
      }
      const className = focused && cursor.blink ? "grid-terminal-cursor grid-terminal-cursor-blink" : "grid-terminal-cursor";
      if (cursorEl.className !== className) cursorEl.className = className;
      if (cursorEl.gridCss !== css) {
        cursorEl.style.cssText = css;
        cursorEl.gridCss = css;
      }
      if (cursorEl.textContent !== text) cursorEl.textContent = text;
    }

    /** Apply the state that changes the whole terminal rather than a row. */
    function paintState() {
      const state = model.state;
      if (styles.setDefaults(state.defaultFg, state.defaultBg)) repaintAll = true;
      const defaults = styles.defaults();
      element.style.color = defaults.fg;
      element.style.backgroundColor = defaults.bg;
      // xterm.js's `enable-mouse-events`: an arrow pointer while the
      // program owns the mouse.
      element.classList.toggle("grid-terminal-mouse", state.mouse !== "off");
      // The exit line is in the terminal's defaults reversed, so it reads as
      // the client's own line rather than part of the program's output.
      const text = exitText(state.exit);
      if (exitEl.textContent !== text) exitEl.textContent = text;
      exitEl.style.color = defaults.bg;
      exitEl.style.backgroundColor = defaults.fg;
    }

    /**
     * Place the exit line directly below the program's last row, never over
     * one. When the space under the rows is less than a line, the screen
     * moves up by whole rows (so no row is left half visible) until the
     * line fits, which is what tmux's "Pane is dead" line did (it scrolled
     * the pane by a row). Re-run whenever the rows, the metrics or the
     * container change.
     */
    function layoutExit() {
      if (!exitEl.textContent || !cellHeight || !model.started) {
        exitEl.style.display = "none";
        if (screen.style.marginTop) screen.style.marginTop = "";
        if (screen.style.clipPath) screen.style.clipPath = "";
        return;
      }
      const style = root.getComputedStyle(element);
      const padTop = parseFloat(style.paddingTop) || 0;
      const padBottom = parseFloat(style.paddingBottom) || 0;
      const content = element.clientHeight - padTop - padBottom;
      const rowsHeight = model.rows * cellHeight;
      const shortfall = rowsHeight + cellHeight - content;
      const lift = shortfall > 0 ? Math.min(rowsHeight, Math.ceil(shortfall / cellHeight - 1e-6) * cellHeight) : 0;
      screen.style.marginTop = lift ? `${-lift}px` : "";
      // The element clips at its padding edge, so without this the bottom
      // of the last lifted row would show in the 4px top padding.
      screen.style.clipPath = lift ? `inset(${lift}px 0 0 0)` : "";
      exitEl.style.display = "block";
      exitEl.style.top = `${padTop + rowsHeight - lift}px`;
      exitEl.style.left = style.paddingLeft;
      exitEl.style.right = style.paddingRight;
      exitEl.style.height = `${cellHeight}px`;
      exitEl.style.lineHeight = `${cellHeight}px`;
    }

    /**
     * Paint whatever changed. Following the bottom, the screen's own damage
     * drives it (moved rows for a scroll, the changed rows); scrolled back,
     * any change redraws the visible rows, because a scroll or new history
     * moves what each of them shows.
     */
    function paint() {
      if (rafHandle !== null) {
        root.cancelAnimationFrame(rafHandle);
        rafHandle = null;
      }
      if (disposed) return;
      // Late metrics (a container that had no layout box when this was
      // created) or a new size lay every row out again.
      if (currentCell()) repaintAll = true;
      const damage = model.takeDamage();
      noteDamage(damage);
      if (damage.state || repaintAll) paintState();
      const shape = viewShape();
      const top = topIndex(shape);
      const follow = following();
      // Following the bottom, the rows show the screen however much history
      // there is, so only leaving or returning to the bottom, or a scrolled
      // view's top moving, counts as the view moving.
      const moved = follow !== paintedFollowing || (!follow && top !== paintedTop);
      if (repaintAll || damage.all || moved || (!follow && (damage.rows.size > 0 || damage.shift > 0 || damage.history))) {
        if (repaintAll || damage.all) layoutRows();
        repaintAll = false;
        for (let y = 0; y < model.rows; y++) paintRow(y, lineRow(top + y, shape));
      } else {
        if (damage.shift > 0) rotateRows(damage.shift);
        for (const y of damage.rows) paintRow(y, model.screen[y]);
      }
      paintedTop = top;
      paintedFollowing = follow;
      paintCursor(shape.notice + shape.history - top);
      paintSelection(top, shape);
      paintScrollbar(top, shape);
      layoutExit();
      refreshHover();
    }

    /** The row the view's index `i` shows, or null for the notice line. */
    function lineRow(i, shape) {
      if (i < shape.notice) return null;
      if (i < shape.notice + shape.history) return model.historyLine(i - shape.notice);
      return model.screen[i - shape.notice - shape.history];
    }

    /**
     * What the model's changes mean for the view and the selection: a new
     * size or a history reset voids anything anchored to line numbers, and
     * the alternate screen switching changes which lines exist.
     */
    let paintedAltScreen = false;
    function noteDamage(damage) {
      const altSwitched = model.state.altScreen !== paintedAltScreen;
      paintedAltScreen = model.state.altScreen;
      if (damage.size || damage.historyReset || altSwitched) {
        viewAbs = null;
        clearSelection();
      }
      if (selection && selection.start.abs < source().first) {
        // The selected lines were dropped at the history cap.
        if (selection.end.abs < source().first) clearSelection();
        // A column selection keeps its column, as xterm.js's trim does.
        else selection.start = { abs: source().first, col: selection.column ? selection.start.col : 0 };
      }
    }

    // ## Scrollbar
    //
    // VS Code's scrollbar as xterm.js 6.0 shows it: a slider in the 14px the
    // fit reserves, sized by the share of lines visible, shown while the
    // pointer is over the terminal and for a moment after a scroll, and
    // only when there is something to scroll.

    // The part of a line the wheel has moved past the view's top line, in
    // pixels at `wheelRemainderCell`, so a trackpad's small deltas add up to
    // lines (xterm.js rounds its scroll position to a line the same way).
    // Only the remainder is kept: the top itself is read from the anchor at
    // every event, so history dropped at the cap, or new history while
    // scrolled back, cannot make the next wheel step jump.
    let wheelRemainder = 0;
    let wheelRemainderCell = 0;
    let scrollbarFade = null;
    let pointerInside = false;

    function paintScrollbar(top, shape) {
      const scrollable = shape.total > model.rows && cellHeight > 0;
      scrollbar.classList.toggle("grid-terminal-scrollbar-needed", scrollable);
      if (!scrollable) return;
      const track = element.clientHeight;
      const height = Math.max(SLIDER_MIN_HEIGHT, Math.round((track * model.rows) / shape.total));
      const offset = shape.bottom > 0 ? Math.round(((track - height) * top) / shape.bottom) : 0;
      slider.style.height = `${height}px`;
      slider.style.top = `${offset}px`;
    }

    function showScrollbarActivity() {
      scrollbar.classList.add("grid-terminal-scrollbar-active");
      if (scrollbarFade !== null) clearTimeout(scrollbarFade);
      scrollbarFade = setTimeout(() => {
        scrollbarFade = null;
        if (!pointerInside && !sliderDrag) scrollbar.classList.remove("grid-terminal-scrollbar-active");
      }, SCROLLBAR_FADE_MS);
    }

    // A slider drag: the pointer's offset into the slider and the slider's
    // travel, set at the press.
    let sliderDrag = null;

    function sliderMove(ev) {
      if (!sliderDrag) return;
      const shape = viewShape();
      const travel = element.clientHeight - slider.offsetHeight;
      if (travel <= 0) return;
      const offset = ev.clientY - element.getBoundingClientRect().top - sliderDrag.grab;
      scrollToIndex((Math.min(travel, Math.max(0, offset)) / travel) * shape.bottom);
    }

    function sliderUp() {
      sliderDrag = null;
      document.removeEventListener("mousemove", sliderMove);
      document.removeEventListener("mouseup", sliderUp);
      showScrollbarActivity();
    }

    scrollbar.addEventListener("mousedown", (ev) => {
      // The scrollbar is the client's own: nothing here reaches the program
      // or starts a selection.
      ev.preventDefault();
      ev.stopPropagation();
      if (ev.button !== 0) return;
      const sliderRect = slider.getBoundingClientRect();
      const onSlider = ev.target === slider;
      // A press on the track centers the slider there, then drags it, as
      // VS Code's scrollbar does.
      const grab = onSlider ? ev.clientY - sliderRect.top : sliderRect.height / 2;
      sliderDrag = { grab };
      if (!onSlider) sliderMove(ev);
      document.addEventListener("mousemove", sliderMove);
      document.addEventListener("mouseup", sliderUp);
    });
    element.addEventListener("mouseenter", () => {
      pointerInside = true;
      scrollbar.classList.add("grid-terminal-scrollbar-active");
    });
    element.addEventListener("mouseleave", () => {
      pointerInside = false;
      showScrollbarActivity();
      lookup++;
      setHover(null, null);
    });

    // ## Selection

    // The current selection: `{start, end}` in line positions, ordered, or
    // null. `gesture` is the press being followed (selection drag, link
    // click, or both) until its release.
    let selection = null;
    let gesture = null;

    // A column (block) selection keeps its two corners' columns as dragged
    // (`start` is the corner on the first line); a character selection is
    // ordered.
    function hasSelection() {
      if (!selection) return false;
      if (selection.column) return selection.start.col !== selection.end.col;
      return gridBuffer.compare(selection.start, selection.end) < 0;
    }

    function selectionText() {
      if (!hasSelection()) return "";
      const newline = gridInput.platformOf(platform).isWindows ? "\r\n" : "\n";
      if (selection.column) return gridBuffer.columnText(source(), selection.start, selection.end, newline);
      return gridBuffer.selectionText(source(), selection.start, selection.end, newline);
    }

    /** Whether a gesture's moving end goes to the nearest cell boundary (else to the cell). */
    function roundsToBoundary(mode) {
      return mode === "char" || mode === "column";
    }

    function clearSelection() {
      if (!selection) return;
      selection = null;
      if (!disposed) paintSelection();
    }

    function selectAll() {
      const lines = source();
      selection = { start: { abs: lines.first, col: 0 }, end: { abs: lines.last, col: model.cols } };
      paintSelection();
    }

    /**
     * The line position under a pointer event: the cell (`round: false`)
     * or the nearest cell boundary (`round: true`, how a character
     * selection's ends move, as xterm.js's selection coordinates do).
     * Rows above or below the screen clamp to its first or last row.
     */
    function positionAt(ev, round) {
      const rect = screen.getBoundingClientRect();
      const x = ev.clientX - rect.left;
      const y = ev.clientY - rect.top;
      const shape = viewShape();
      const row = Math.min(model.rows - 1, Math.max(0, Math.floor(y / cellHeight)));
      let index = topIndex(shape) + row;
      // The notice line is not text: a press on it lands on the line below.
      if (index < shape.notice) index = shape.notice;
      const abs = absAt(Math.min(index, shape.total - 1), shape);
      const raw = x / cellWidth;
      const col = Math.min(model.cols, Math.max(0, round ? Math.round(raw) : Math.floor(raw)));
      return { abs, col: round ? col : Math.min(col, model.cols - 1) };
    }

    /** The selection a gesture makes with its far end at `pos`. */
    function extendTo(pos) {
      const lines = source();
      let start;
      let end;
      if (gesture.mode === "column") {
        const anchor = gesture.anchor;
        selection = anchor.abs <= pos.abs
          ? { start: anchor, end: pos, column: true }
          : { start: { abs: pos.abs, col: anchor.col }, end: { abs: anchor.abs, col: pos.col }, column: true };
        paintSelection();
        return;
      }
      if (gesture.mode === "char") {
        [start, end] = gridBuffer.ordered(gesture.anchor, pos);
      } else {
        const measure = gesture.mode === "word" ? gridBuffer.wordRange : gridBuffer.lineRange;
        const at = measure(lines, { abs: pos.abs, col: Math.min(pos.col, model.cols - 1) }) || { start: pos, end: pos };
        start = gridBuffer.compare(at.start, gesture.anchorRange.start) < 0 ? at.start : gesture.anchorRange.start;
        end = gridBuffer.compare(at.end, gesture.anchorRange.end) > 0 ? at.end : gesture.anchorRange.end;
      }
      selection = { start, end };
      paintSelection();
    }

    /**
     * Start a local selection at a press: a click starts an empty one at
     * the nearest boundary, a double click selects the word, a triple click
     * the soft-wrapped line, and Shift with a selection already there
     * extends it (xterm.js's `SelectionService` press handling).
     */
    function startSelection(ev) {
      const detail = ev.detail || 1;
      if (detail === 2 || detail === 3) {
        const cell = positionAt(ev, false);
        const range = (detail === 2 ? gridBuffer.wordRange : gridBuffer.lineRange)(source(), cell);
        if (!range) return;
        gesture.mode = detail === 2 ? "word" : "line";
        gesture.anchorRange = range;
        selection = { start: range.start, end: range.end };
      } else if (ev.altKey && !isMac) {
        // xterm.js's `shouldColumnSelect`: Alt starts a column selection
        // off a Mac (where Option forces a selection instead).
        gesture.mode = "column";
        gesture.anchor = positionAt(ev, true);
        selection = { start: gesture.anchor, end: gesture.anchor, column: true };
      } else if (ev.shiftKey && hasSelection() && !selection.column) {
        gesture.mode = "char";
        gesture.anchor = selection.start;
        extendTo(positionAt(ev, true));
        return;
      } else {
        gesture.mode = "char";
        gesture.anchor = positionAt(ev, true);
        selection = { start: gesture.anchor, end: gesture.anchor };
      }
      paintSelection();
    }

    /**
     * Draw the selection over the visible rows: one box per row it touches,
     * from its start column (or the row's start) to its end column (or the
     * screen's edge).
     */
    function paintSelection(top = topIndex(), shape = viewShape()) {
      const boxes = [];
      if (hasSelection() && cellWidth) {
        const { start, end } = selection;
        for (let y = 0; y < model.rows; y++) {
          const i = top + y;
          if (i < shape.notice) continue;
          const abs = absAt(i, shape);
          if (abs < start.abs || abs > end.abs) continue;
          const column = selection.column;
          const from = column ? Math.min(start.col, end.col) : abs === start.abs ? start.col : 0;
          const to = Math.min(model.cols, column ? Math.max(start.col, end.col) : abs === end.abs ? end.col : model.cols);
          if (to <= from) continue;
          boxes.push([from, to, y]);
        }
      }
      while (selectionLayer.children.length > boxes.length) selectionLayer.lastChild.remove();
      boxes.forEach(([from, to, y], i) => {
        const box = selectionLayer.children[i] || selectionLayer.appendChild(document.createElement("div"));
        box.style.cssText = `left:${from * cellWidth}px;top:${y * cellHeight}px;` +
          `width:${(to - from) * cellWidth}px;height:${cellHeight}px;background-color:${SELECTION_COLOR};`;
      });
    }

    // A drag held past the top or bottom of the screen scrolls the view
    // while it stays there, extending the selection with it.
    let dragScrollTimer = null;
    let dragScrollLines = 0;

    function updateDragScroll(ev) {
      const rect = screen.getBoundingClientRect();
      let distance = 0;
      if (ev.clientY < rect.top) distance = ev.clientY - rect.top;
      else if (ev.clientY > rect.bottom) distance = ev.clientY - rect.bottom;
      dragScrollLines = distance === 0
        ? 0
        : Math.sign(distance) * Math.max(1, Math.round((Math.min(Math.abs(distance), DRAG_SCROLL_MAX_DISTANCE) / DRAG_SCROLL_MAX_DISTANCE) * DRAG_SCROLL_MAX_LINES));
      if (dragScrollLines !== 0 && dragScrollTimer === null) {
        dragScrollTimer = setInterval(() => {
          if (!gesture || !gesture.selecting || dragScrollLines === 0) return stopDragScroll();
          scrollLines(dragScrollLines);
          paint();
          // The selection's moving end follows to the edge row it is held past.
          extendTo(positionAt(gesture.lastMove, roundsToBoundary(gesture.mode)));
        }, DRAG_SCROLL_INTERVAL_MS);
      } else if (dragScrollLines === 0) {
        stopDragScroll();
      }
    }

    function stopDragScroll() {
      if (dragScrollTimer !== null) clearInterval(dragScrollTimer);
      dragScrollTimer = null;
      dragScrollLines = 0;
    }

    // ## Links
    //
    // The links on a line come from a provider, as xterm.js's link
    // providers give them: `options.links.row(abs, done)` (terminal.js's
    // gives the program's hyperlinks, printed URLs and file paths, with
    // today's displays and checks; without a provider there are no links).
    // A link is `{kind, web, ranges, evidence?, hover(event), leave(),
    // activate(event), decorations}`; `decorations.underline` and
    // `decorations.pointerCursor` may change while it is hovered (a file
    // link shows the hand only once its check succeeds), so they are setters
    // here, as in xterm.js. A provider may answer later (a container's paths
    // are checked by its host first); only the answer to the latest
    // question, about rows still in place and the line still under the
    // pointer, counts (`answerLines`).
    //
    // A hover rests on the row objects it read, captured when its `hover()`
    // runs and nowhere else: the lines the link covers, plus an OSC 8 link's
    // `evidence` lines (the adjacent rows its text is joined from, and one
    // beyond each end). Rows are immutable and replaced only when the
    // supervisor sends that row, so "the hover is current" is one identity
    // check per line. Every path checks it before relying on the hover: a
    // pointer move, the paint after rows changed, and a release. A hover
    // that is not current is left first (withdrawing a stale target display
    // and a file link's permission) and the link under the pointer asked for
    // afresh, a new link object hovered afresh; the file addon reuses its
    // check when path, text and range are unchanged. That is xterm.js's
    // linkifier: a render of the hovered link's rows leaves it and asks
    // again; with nothing hovered a render does nothing, and a link that
    // appears under a still pointer is found on the next move. A release
    // activates only the link its press was on, still current.

    let hoverLink = null;
    // The row objects the hovered link's `hover()` read, by line number.
    let hoverRows = null;
    // The pointer's last event over the terminal.
    let lastPointer = null;
    // The links of one line (`{abs, links}`), dropped whenever the model
    // changes (`noteModelChange`), and the latest question asked.
    let rowCache = null;
    let lookup = 0;
    // The release a click on a link was admitted with (`isLinkClick`).
    let admittedClick = null;
    const linkProvider = options.links || null;

    function rowAt(abs) {
      const line = source().line(abs);
      return line ? line.row : null;
    }

    /** The lines a link's hover reads: the lines it covers and its evidence lines. */
    function linkLines(link) {
      const lines = new Set(link.evidence || []);
      for (const range of link.ranges) {
        for (let abs = range.start.abs; abs <= range.end.abs; abs++) lines.add(abs);
      }
      return lines;
    }

    /** Whether every row the hovered link read is still the row on its line. */
    function hoverCurrent() {
      if (!hoverRows) return true;
      for (const [abs, row] of hoverRows) if (rowAt(abs) !== row) return false;
      return true;
    }

    /** The model's lines changed (a received message, a history slice): cached answers are stale. */
    function noteModelChange() {
      rowCache = null;
    }

    /** The cell under a pointer event, or null off the screen and on the notice line. */
    function linkCell(ev) {
      if (!model.started || !cellWidth) return null;
      const rect = screen.getBoundingClientRect();
      if (ev.clientX < rect.left || ev.clientX >= rect.right || ev.clientY < rect.top || ev.clientY >= rect.bottom) {
        return null;
      }
      const shape = viewShape();
      if (topIndex(shape) + Math.floor((ev.clientY - rect.top) / cellHeight) < shape.notice) return null;
      return positionAt(ev, false);
    }

    /**
     * Find the link under `ev`: a stale hover is left first; a current one
     * that still covers the cell is kept; otherwise the line's links are
     * asked for (from the cache while the model has not changed).
     */
    function lookUpLink(ev) {
      if (hoverLink && !hoverCurrent()) {
        lookup++;
        setHover(null, ev);
      }
      const pos = linkProvider ? linkCell(ev) : null;
      if (!pos) {
        lookup++;
        setHover(null, ev);
        return;
      }
      if (hoverLink && gridBuffer.linkCovers(hoverLink, pos)) return;
      const token = ++lookup;
      const pick = (links, cell) => {
        if (token !== lookup || disposed || gesture) return;
        setHover(links.find((link) => gridBuffer.linkCovers(link, cell)) || null, ev);
      };
      if (rowCache && rowCache.abs === pos.abs) {
        pick(rowCache.links, pos);
        return;
      }
      // The rows the answer can depend on, as they are now: a late answer
      // (a container's paths are checked by its host first) is used only if
      // none of them was replaced meanwhile. Output elsewhere does not void
      // it, so links keep appearing while a program writes other rows.
      const depends = new Map();
      for (const abs of gridBuffer.answerLines(source(), pos.abs)) depends.set(abs, rowAt(abs));
      linkProvider.row(pos.abs, (links) => {
        if (token !== lookup || disposed) return;
        for (const [abs, row] of depends) if (rowAt(abs) !== row) return;
        // Nor is it used for a line that is no longer under the pointer (the
        // view scrolled while the answer was pending).
        const now = lastPointer && pointerInside ? linkCell(lastPointer) : null;
        if (!now || now.abs !== pos.abs) return;
        rowCache = { abs: pos.abs, links: links || [] };
        pick(rowCache.links, now);
      });
    }

    /** Give a hovered link xterm.js's decoration setters (see "## Links"). */
    function decorate(link) {
      const initial = link.decorations || {};
      let underline = initial.underline !== false;
      let pointerCursor = !!initial.pointerCursor;
      link.decorations = {
        get underline() {
          return underline;
        },
        set underline(value) {
          underline = !!value;
          if (hoverLink === link) paintLinkUnderline();
        },
        get pointerCursor() {
          return pointerCursor;
        },
        set pointerCursor(value) {
          pointerCursor = !!value;
          if (hoverLink === link) element.classList.toggle("grid-terminal-link-hover", pointerCursor);
        },
      };
    }

    /** The only place a hover starts or ends, and where its rows are captured. */
    function setHover(link, ev) {
      if (link === hoverLink) return;
      if (hoverLink) hoverLink.leave();
      hoverLink = link;
      hoverRows = null;
      if (link) {
        hoverRows = new Map();
        for (const abs of linkLines(link)) hoverRows.set(abs, rowAt(abs));
        decorate(link);
      }
      element.classList.toggle("grid-terminal-link-hover", !!link && link.decorations.pointerCursor);
      paintLinkUnderline();
      if (link) link.hover(ev);
    }

    /** Underline the hovered link's cells on the visible rows. */
    function paintLinkUnderline() {
      const boxes = [];
      if (hoverLink && hoverLink.decorations.underline && cellWidth) {
        const shape = viewShape();
        const top = topIndex(shape);
        for (const range of hoverLink.ranges) {
          for (let abs = range.start.abs; abs <= range.end.abs; abs++) {
            const i = indexOf(abs, shape);
            const y = i - top;
            if (y < 0 || y >= model.rows || absAt(i, shape) !== abs) continue;
            const from = abs === range.start.abs ? range.start.col : 0;
            const to = Math.min(model.cols, abs === range.end.abs ? range.end.col : model.cols);
            if (to > from) boxes.push([from, to, y]);
          }
        }
      }
      while (linkLayer.children.length > boxes.length) linkLayer.lastChild.remove();
      boxes.forEach(([from, to, y], i) => {
        const line = linkLayer.children[i] || linkLayer.appendChild(document.createElement("div"));
        line.style.cssText = `left:${from * cellWidth}px;top:${(y + 1) * cellHeight - 1}px;width:${(to - from) * cellWidth}px;`;
      });
    }

    /**
     * After a paint (xterm.js's `onRenderedViewportChange` for a hovered
     * link): a hover whose rows changed, or that is no longer under the
     * resting pointer because the view moved, is left and asked for afresh.
     * With nothing hovered nothing happens until the pointer moves.
     */
    function refreshHover() {
      if (hoverLink && lastPointer && pointerInside && !gesture) {
        const pos = linkCell(lastPointer);
        if (!hoverCurrent() || !pos || !gridBuffer.linkCovers(hoverLink, pos)) {
          lookup++;
          setHover(null, lastPointer);
          lookUpLink(lastPointer);
          return;
        }
      }
      paintLinkUnderline();
    }

    // ## Input

    let deadKeyPending = false;
    let composing = false;

    function sendMessage(message) {
      if (message && send && !frozen) send(message);
    }

    /**
     * Send input the user made (a key, typed text, a paste): like
     * xterm.js's `scrollOnUserInput`, it brings the view back to the bottom,
     * and like terminal.js's `dismissSelection` for xterm.js, it clears the
     * selection, which would otherwise sit over text the input changes.
     */
    function sendUserInput(messages) {
      if (frozen || !send) return false;
      for (const message of messages) sendMessage(message);
      scrollToBottom();
      clearSelection();
      return true;
    }

    function sendText(data) {
      deadKeyPending = false;
      sendUserInput(gridInput.textMessages(data));
    }

    /**
     * Paste text as the program's input: `paste` messages split at the
     * format's bounds (the supervisor adds bracketed-paste markers and
     * converts line endings as xterm.js does). The caller has sanitized it
     * (terminal.js's attachment interception, or the paste listener here).
     */
    function paste(text) {
      if (typeof text !== "string" || text === "") return false;
      return sendUserInput(gridInput.pasteMessages(text));
    }

    function cancel(ev) {
      ev.preventDefault();
      ev.stopPropagation();
    }

    // Page shortcuts (the quick switcher, templates, session actions, text
    // size) are window capture listeners that stop the event before it gets
    // here, so they never reach the program, exactly as with xterm.js.
    input.addEventListener("keydown", (ev) => {
      restoreInput();
      const decision = gridInput.keydownAction(ev, platform, deadKeyPending);
      deadKeyPending = decision.deadKeyPending;
      if (decision.action === "send") {
        sendUserInput([gridInput.keyMessage(ev, platform)]);
        cancel(ev);
        return;
      }
      // Shift+PageUp/Down scroll the view a page less a line (xterm.js's
      // `scrollPages`), and Cmd+A on a Mac selects everything; none of them
      // reach the program.
      if (decision.action === "page-up") scrollLines(-(model.rows - 1));
      else if (decision.action === "page-down") scrollLines(model.rows - 1);
      else if (decision.action === "select-all") selectAll();
      if (decision.cancel) cancel(ev);
    });
    input.addEventListener("input", (ev) => {
      // Composition text arrives at compositionend; a paste arrives as a
      // paste. Only typed text is sent from here.
      if (!composing && !ev.isComposing && ev.inputType === "insertText" && ev.data) sendText(ev.data);
      if (!composing) input.value = "";
    });
    input.addEventListener("compositionstart", () => {
      composing = true;
    });
    input.addEventListener("compositionend", (ev) => {
      composing = false;
      // The committed result of an input method or a dead-key sequence.
      // Showing the composition in progress at the cursor is not built
      // (it failed the client parity work's complexity gate; see the
      // working log), so the candidate window sits where the browser puts
      // it for a field it cannot see.
      if (ev.data) sendText(ev.data);
      input.value = "";
    });
    // A paste that terminal.js's attachment interception (a capture
    // listener on the pane) did not take: plain text without that policy.
    // It goes to the program as a paste, as xterm.js's own handler sends it.
    input.addEventListener("paste", (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      restoreInput();
      let text = "";
      try {
        text = (ev.clipboardData && ev.clipboardData.getData("text/plain")) || "";
      } catch (_error) {
        text = "";
      }
      if (text) paste(sanitizePaste(text));
    });
    // The browser's own Copy (a keyboard shortcut or the context menu) on
    // the input copies the selection, as xterm.js's copy handler does.
    input.addEventListener("copy", (ev) => {
      if (!hasSelection() || !ev.clipboardData) return;
      ev.clipboardData.setData("text/plain", selectionText());
      ev.preventDefault();
    });
    input.addEventListener("focus", () => setFocused(true));
    input.addEventListener("blur", () => {
      restoreInput();
      setFocused(false);
    });

    function setFocused(value) {
      if (focused === value) return;
      focused = value;
      element.classList.toggle("grid-terminal-focused", value);
      // Focus in/out reports are what the program asked for with mode 1004
      // (the report of the state at the moment it asks is in `receive`).
      if (model.state.focus) sendMessage({ t: "focus", in: value });
      paintCursor(viewRowOffset());
    }

    function viewRowOffset() {
      const shape = viewShape();
      return shape.notice + shape.history - topIndex(shape);
    }

    /**
     * xterm.js's `moveTextAreaUnderMouseCursor`: put the input under the
     * pointer for a context menu (or a Linux middle-button paste), so the
     * browser's menu offers Paste (and Copy, for the selection put in it)
     * and a paste lands in the terminal. `restoreInput` parks it again at
     * the next key, press, paste or blur.
     */
    function moveInputUnder(ev) {
      const rect = element.getBoundingClientRect();
      input.style.cssText = `left:${ev.clientX - rect.left - 10}px;top:${ev.clientY - rect.top - 10}px;` +
        "width:20px;height:20px;z-index:1000;";
      input.value = selectionText();
      input.select();
      focus();
    }

    function restoreInput() {
      if (!input.style.cssText) return;
      input.style.cssText = "";
      input.value = "";
    }

    // ## Mouse
    //
    // A press decides what the gesture is: a report to the program (mouse
    // reporting on and no forcing modifier), a local selection, and, on a
    // link, a candidate click. The gesture's moves and release are followed
    // on the document, as xterm.js does, so a release outside the terminal
    // still ends it.

    const mouseDedupe = gridInput.createMouseDeduper();
    const wheel = gridInput.createWheelAccumulator();

    /** The message for a pointer event, or null before the grid has a size. */
    function pointerMessage(kind, button, ev) {
      if (!model.started || !cellWidth) return null;
      const rect = screen.getBoundingClientRect();
      const cell = gridInput.mouseCell(ev.clientX - rect.left, ev.clientY - rect.top, cellWidth, cellHeight, model.cols, model.rows);
      return { t: "mouse", kind, button, x: cell.x, y: cell.y, mods: gridInput.modifiers(ev) };
    }

    /**
     * Report a mouse event the program's mode asks for. A report is input
     * from the user, so like a key it brings the view back to the bottom and
     * clears the selection (xterm.js sends SGR and UTF-8 reports through its
     * user-input path, which applies `scrollOnUserInput` and fires the
     * `onUserInput` its selection clears on; the client cannot see the
     * encoding, and programs use SGR).
     */
    function mouseMessage(kind, button, ev) {
      if (!gridInput.modeReports(model.state.mouse, kind, button)) return;
      const message = pointerMessage(kind, button, ev);
      if (!message || !mouseDedupe.shouldSend(message)) return;
      sendUserInput([message]);
    }

    function onDocumentMouseMove(ev) {
      if (!gesture) return;
      gesture.lastMove = ev;
      if (gesture.link && Math.hypot(ev.clientX - gesture.x, ev.clientY - gesture.y) > CLICK_JITTER_PX) {
        gesture.link = null;
      }
      if (gesture.reporting && ev.buttons && !(gesture.withheld && (ev.buttons & 1))) {
        mouseMessage("move", gridInput.moveButton(ev.buttons), ev);
      }
      if (gesture.selecting && gesture.mode) {
        extendTo(positionAt(ev, roundsToBoundary(gesture.mode)));
        updateDragScroll(ev);
      }
    }

    function onDocumentMouseUp(ev) {
      if (!gesture) return;
      const ending = gesture;
      const button = gridInput.pressButton(ev.button);
      if (ending.reporting && button !== null && !(ending.withheld && button === 0)) mouseMessage("up", button, ev);
      // A report gesture ends when every button is up, as xterm.js's
      // document listeners do; a selection or link gesture ends with the
      // button that started it.
      const done = ending.reporting ? !ev.buttons : ev.button === ending.button || !ev.buttons;
      if (!done) return;
      stopGesture();
      if (ending.selecting && ending.mode) {
        const text = selectionText();
        if (onCopy && copyOnSelect && copyOnSelect.copySelectionOnMouseUp({
          button: ending.button,
          hasSelection: hasSelection(),
          selectionText: text,
          trackingAtPress: ending.trackingAtPress,
          forced: ending.forced,
        })) onCopy(text);
        altClickMovesCursor(ending, ev, text);
      }
      // A click on a link: the press and release on the same link, with no
      // more than jitter between them.
      if (ending.link && ev.button === 0 && Math.hypot(ev.clientX - ending.x, ev.clientY - ending.y) <= CLICK_JITTER_PX) {
        const released = linkCell(ev);
        if (released && hoverLink === ending.link && hoverCurrent() && gridBuffer.linkCovers(ending.link, released)) {
          admittedClick = ev;
          ending.link.activate(ev);
        }
      }
      if (lastPointer) {
        lastPointer = ev;
        lookUpLink(ev);
      }
    }

    /**
     * xterm.js's `altClickMovesCursor` (on by default): a quick Alt-click
     * (Option on a Mac) that selected at most one character, with the view
     * at the bottom, moves the program's cursor to the clicked cell by
     * typing arrow keys (grid-input.js's `moveToCellSequence`, sent as `key`
     * messages so the supervisor encodes them in the program's cursor-key
     * mode). Returns whether it did.
     */
    function altClickMovesCursor(ending, ev, text) {
      if (!ev.altKey || ending.button !== 0 || text.length > 1 || ev.timeStamp - ending.pressedAt >= 500 || !following()) {
        return false;
      }
      const rect = screen.getBoundingClientRect();
      const target = gridInput.mouseCell(ev.clientX - rect.left, ev.clientY - rect.top, cellWidth, cellHeight, model.cols, model.rows);
      const lines = gridBuffer.xtermBuffer(gridBuffer.modelSource(model, false));
      const base = model.screenBase;
      const bufferService = {
        cols: model.cols,
        rows: model.rows,
        buffer: {
          x: model.cursor.x,
          y: model.cursor.y,
          hasScrollback: !model.state.altScreen,
          lines: { length: model.rows, get: (row) => lines.getLine(base + row) },
          translateBufferLineToString(row, trimRight, startCol, endCol) {
            const line = lines.getLine(base + row);
            return line ? line.translateToString(trimRight, startCol, endCol) : "";
          },
        },
      };
      const sequence = gridInput.moveToCellSequence(target.x, target.y, bufferService, false);
      if (!sequence) return false;
      sendUserInput(gridInput.arrowKeyMessages(sequence, platform));
      return true;
    }

    function startGesture(state) {
      if (gesture) stopGesture();
      gesture = state;
      document.addEventListener("mousemove", onDocumentMouseMove);
      document.addEventListener("mouseup", onDocumentMouseUp);
    }

    function stopGesture() {
      gesture = null;
      stopDragScroll();
      document.removeEventListener("mousemove", onDocumentMouseMove);
      document.removeEventListener("mouseup", onDocumentMouseUp);
    }

    element.addEventListener("mousedown", (ev) => {
      // Keep keyboard focus in the terminal and stop the browser's own
      // text selection, as xterm.js's press handler does.
      ev.preventDefault();
      restoreInput();
      focus();
      if (!model.started || !cellWidth) return;
      const forced = !!copyOnSelect && copyOnSelect.pressForcesSelection(ev, platform);
      const tracking = model.state.mouse !== "off";
      const button = gridInput.pressButton(ev.button);
      // The press is on a link when it lands on the hovered one (xterm.js's
      // linkifier keeps the link the pointer is over, not one found at the
      // press).
      const pressed = ev.button === 0 ? linkCell(ev) : null;
      const link = pressed && hoverLink && gridBuffer.linkCovers(hoverLink, pressed) ? hoverLink : null;
      // Under mouse reporting the link-click setting decides whether a
      // press on a web link is the program's or Farhelm's alone.
      const withheld = tracking && !forced && !!link && !!linkPress && linkPress(ev, link);
      const state = {
        button: ev.button,
        x: ev.clientX,
        y: ev.clientY,
        lastMove: ev,
        link,
        withheld,
        forced,
        trackingAtPress: tracking,
        reporting: tracking && !forced && button !== null,
        selecting: (!tracking || forced) && ev.button === 0,
        mode: null,
        pressedAt: ev.timeStamp,
      };
      if (state.reporting && !withheld) mouseMessage("down", button, ev);
      if (withheld) clearSelection();
      if (state.selecting) {
        gesture = state;
        startSelection(ev);
        gesture = null;
      } else if (ev.button === 2 && isMac && !inSelection(ev)) {
        // xterm.js's `rightClickSelectsWord`, on by default on a Mac: the
        // context menu's Copy then has the word.
        const range = gridBuffer.wordRange(source(), positionAt(ev, false));
        if (range) {
          selection = { start: range.start, end: range.end };
          paintSelection();
        }
      }
      if (state.reporting || state.selecting || state.link) startGesture(state);
    });
    element.addEventListener("mousemove", (ev) => {
      lastPointer = ev;
      // A move over the terminal is proof the pointer is in it, whether or
      // not this engine delivered `mouseenter` for how it got there.
      pointerInside = true;
      if (!ev.buttons) mouseMessage("move", -1, ev);
      if (!gesture) lookUpLink(ev);
    });
    element.addEventListener("wheel", (ev) => {
      if (gridInput.wheelGoesToProgram(model.state)) {
        if (ev.deltaY === 0) return;
        ev.preventDefault();
        // One message per event that scrolls at least a line, as xterm.js
        // sends one report (or one arrow key) per such event.
        if (wheel.consume(ev, wheelRowHeight, model.rows) === 0) return;
        const button = ev.deltaY < 0 ? 0 : 1;
        if (model.state.mouse === "off") {
          // The alternate screen without mouse reporting: the supervisor
          // turns the step into an arrow key, as xterm.js does.
          sendMessage(pointerMessage("wheel", button, ev));
          return;
        }
        mouseMessage("wheel", button, ev);
        return;
      }
      // The client's own scrollback.
      ev.preventDefault();
      const pixels = wheelPixels(ev, isMac);
      if (pixels === 0 || !cellHeight) return;
      const shape = viewShape();
      // A remainder gathered at another cell height (a text-size change)
      // does not carry over.
      const remainder = wheelRemainderCell === cellHeight ? wheelRemainder : 0;
      const current = topIndex(shape) * cellHeight + remainder;
      const next = Math.min(shape.bottom * cellHeight, Math.max(0, current - pixels));
      const top = Math.round(next / cellHeight);
      wheelRemainder = next - top * cellHeight;
      wheelRemainderCell = cellHeight;
      scrollToIndex(top, true);
    }, { passive: false });
    element.addEventListener("contextmenu", (ev) => {
      if (ev.target === scrollbar || ev.target === slider) return;
      moveInputUnder(ev);
    });
    if (isLinux) {
      // A middle-button paste of the primary selection lands in the input.
      element.addEventListener("auxclick", (ev) => {
        if (ev.button === 1) moveInputUnder(ev);
      });
    }

    /** Whether a pointer event is over the current selection. */
    function inSelection(ev) {
      if (!hasSelection()) return false;
      const pos = positionAt(ev, false);
      if (selection.column) {
        return pos.abs >= selection.start.abs && pos.abs <= selection.end.abs &&
          pos.col >= Math.min(selection.start.col, selection.end.col) && pos.col < Math.max(selection.start.col, selection.end.col);
      }
      return gridBuffer.compare(pos, selection.start) >= 0 && gridBuffer.compare(pos, selection.end) < 0;
    }

    function focus() {
      input.focus({ preventScroll: true });
    }

    // ## History application in slices (see "Scrollback")

    let drainScheduled = false;
    // The animation frame a scheduled slice waits for, if it went that way.
    let drainFrame = null;
    const drainChannel = typeof MessageChannel === "function" ? new MessageChannel() : null;

    /**
     * Apply queued history for at most `DRAIN_SLICE_MS`. Returns whether
     * any is left; throws the format violation it met.
     */
    function drainSlice() {
      const deadline = performance.now() + DRAIN_SLICE_MS;
      return model.drainHistory(() => performance.now() >= deadline);
    }

    function runDrain() {
      drainScheduled = false;
      drainFrame = null;
      if (disposed) return;
      let more;
      try {
        more = drainSlice();
      } catch (error) {
        if (onProtocolError) onProtocolError(error);
        return;
      }
      noteModelChange();
      schedulePaint();
      if (more) scheduleDrain();
    }

    /**
     * Run the next slice: in an animation frame while the page is visible,
     * so the page paints between slices (back-to-back message-channel
     * tasks starved Chromium's rendering for seconds in the measurement),
     * and on a message-channel task while it is hidden, where animation
     * frames do not run and nothing is painted anyway.
     */
    function scheduleDrain() {
      if (drainScheduled || disposed) return;
      drainScheduled = true;
      if (pageVisible()) drainFrame = root.requestAnimationFrame(runDrain);
      else postDrain();
    }

    function pageVisible() {
      return typeof document.visibilityState !== "string" || document.visibilityState === "visible";
    }

    function postDrain() {
      if (drainChannel) drainChannel.port2.postMessage(0);
      else setTimeout(runDrain, 0);
    }
    if (drainChannel) drainChannel.port1.onmessage = runDrain;

    // A slice waiting for an animation frame when the page is hidden would
    // wait until it is shown again (hidden pages run no frames); move it to
    // a task, so history keeps being applied in a background tab.
    function onVisibilityChange() {
      if (drainFrame === null || pageVisible()) return;
      root.cancelAnimationFrame(drainFrame);
      drainFrame = null;
      postDrain();
    }
    // On the window (the event bubbles there from the document), which keeps
    // the document's own listeners to the gestures that need them.
    root.addEventListener("visibilitychange", onVisibilityChange);

    // A window moved to a display with another pixel ratio changes the
    // rounded cell height; xterm.js re-measures then, and so does this.
    // The query names one ratio, so it is re-armed for the new one.
    let pixelRatioWatch = null;
    function watchPixelRatio() {
      if (pixelRatioWatch) pixelRatioWatch.query.removeEventListener("change", pixelRatioWatch.listener);
      pixelRatioWatch = null;
      if (typeof root.matchMedia !== "function") return;
      const query = root.matchMedia(`(resolution: ${root.devicePixelRatio || 1}dppx)`);
      const listener = () => {
        remeasure();
        watchPixelRatio();
      };
      query.addEventListener("change", listener);
      pixelRatioWatch = { query, listener };
    }

    measureChar();
    currentCell();
    watchPixelRatio();

    return {
      element,
      model,

      /**
       * Apply one data frame. Throws `GridProtocolError` on a bad one, or
       * on a bad history row found in this frame's slice of history.
       */
      receive(data) {
        const reporting = model.state.focus;
        const backlog = model.historyPending;
        const message = gridModel.decodeFrame(data);
        model.apply(message);
        noteModelChange();
        // A program turning focus reporting on hears the current state at
        // once, focused or not, as xterm.js answers DECSET 1004.
        if (!reporting && model.state.focus) sendMessage({ t: "focus", in: focused });
        // A program's completed clipboard write (OSC 52).
        for (const write of model.takeClipboard()) if (onClipboard) onClipboard(write.text, write.target);
        // The program's real size, at attach. If it is not the size this
        // pane fits (an attach-time resize that failed, a size the
        // supervisor clamped), ask once for the fitted size.
        if (message.t === "hello" && (model.cols !== fitted.cols || model.rows !== fitted.rows) && onResize) {
          onResize(fitted.cols, fitted.rows);
        }
        // A message that arrives with nothing queued gets its slice at once,
        // so ordinary output's history lines (a few per frame) are never
        // behind; behind a backlog it waits its turn in the scheduled slices.
        if (model.historyPending && (backlog || drainSlice())) scheduleDrain();
        schedulePaint();
      },

      /** Paint now rather than at the next animation frame (the reveal). */
      flush() {
        paint();
      },

      /**
       * Refit to the container (a window or pane resize). Also re-places
       * the exit line, whose room below the rows depends on the container.
       */
      fit() {
        fit();
        layoutExit();
      },

      /** The fitted size, the one to send as the terminal's size. */
      size() {
        return { cols: fitted.cols, rows: fitted.rows };
      },

      /**
       * Change the text size: re-measure, redraw at the new metrics and
       * refit, which reports the new rows and columns through `onResize`.
       */
      setFontSize(size) {
        fontSize = size;
        element.style.fontSize = `${size}px`;
        remeasure();
      },

      focus,

      /** Whether keyboard focus is in this terminal. */
      hasFocus() {
        return focused;
      },

      paste,

      /**
       * Whether `event` is the release a click on a link was admitted with,
       * for a link's `activate` to check (terminal-links.js's link gesture
       * plays this part for xterm.js).
       */
      isLinkClick(event) {
        return !!event && event === admittedClick;
      },

      /**
       * A fresh read-only adapter of xterm.js's buffer API over what the
       * terminal holds (grid-buffer.js's `xtermBuffer`), for today's link
       * helpers. History is left out on the alternate screen.
       */
      xtermBuffer() {
        return gridBuffer.xtermBuffer(source());
      },

      /** The program's hyperlinks and printed URLs touching line `abs` (grid-buffer.js's `rowLinks`). */
      rowLinks(abs) {
        return gridBuffer.rowLinks(source(), abs);
      },

      hasSelection,
      selectionText,
      clearSelection,
      selectAll,

      /** Scroll the view by `count` lines (negative is older), or to the bottom. */
      scrollLines,
      scrollToBottom,

      /**
       * Where the view is, for tests and the e2e helpers: the index of its
       * top line among `total`, and whether it follows the bottom.
       */
      viewport() {
        const shape = viewShape();
        return { top: topIndex(shape), total: shape.total, rows: model.rows, following: following() };
      },

      /**
       * Stop sending anything while keeping the screen, scrollback,
       * selection and links: the non-live snapshot a displaced client
       * keeps. Irreversible; a reattach is a new component.
       */
      freeze() {
        frozen = true;
        send = null;
        onResize = null;
        stopGesture();
      },

      /**
       * What the terminal shows, as plain data: for tests and the e2e
       * helpers, which read the model rather than the DOM.
       */
      snapshot() {
        const lines = model.screen.map((row) => gridBuffer.rowText(row, model.cols).replace(/ +$/, ""));
        const history = [];
        for (let i = 0; i < model.historyLength; i++) {
          const row = model.historyLine(i);
          history.push(gridBuffer.rowText(row, row.cols).replace(/ +$/, ""));
        }
        return {
          cols: model.cols,
          rows: model.rows,
          lines,
          history,
          historyPending: model.historyPending,
          cursor: { ...model.cursor },
          state: { ...model.state },
        };
      },

      dispose() {
        disposed = true;
        if (rafHandle !== null) root.cancelAnimationFrame(rafHandle);
        rafHandle = null;
        if (pixelRatioWatch) pixelRatioWatch.query.removeEventListener("change", pixelRatioWatch.listener);
        pixelRatioWatch = null;
        root.removeEventListener("visibilitychange", onVisibilityChange);
        if (drainFrame !== null) root.cancelAnimationFrame(drainFrame);
        drainFrame = null;
        if (drainChannel) {
          drainChannel.port1.onmessage = null;
          drainChannel.port1.close();
          drainChannel.port2.close();
        }
        stopGesture();
        // Released before the fade timer is cleared: letting go of the
        // slider arms that timer.
        sliderUp();
        if (scrollbarFade !== null) clearTimeout(scrollbarFade);
        scrollbarFade = null;
        lookup++;
        if (hoverLink) hoverLink.leave();
        hoverLink = null;
        send = null;
        onResize = null;
        onClipboard = null;
        onCopy = null;
        linkPress = null;
        onProtocolError = null;
        if (onDispose) onDispose();
        onDispose = null;
        element.remove();
      },
    };
  }

  root.farhelmGridTerminal = { create, exitText, wheelPixels };
})(typeof window === "object" ? window : globalThis);
