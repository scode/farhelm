// The grid terminal: draws a grid stream's model into the page and turns
// the user's keyboard, mouse and focus into input messages. The DOM half
// of the grid client; the decisions it acts on are in grid-model.js,
// grid-style.js and grid-input.js, where `node --test` covers them.
//
// terminal.js decides per attachment which component a terminal gets
// (xterm.js for byte streams, this for grid streams) and owns everything
// about the connection: the socket, catch-up, heartbeat, reconnect,
// banners. This file only needs a container element, a way to send input
// messages, and to be fed the data frames.
//
// ## Rendering (the approach the renderer benchmark chose)
//
// - One element per screen row, in normal block flow, reused for the life
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
//   for a scrollbar.
//
// A browser spec mounts both components in identical containers and holds
// them to the same fitted size across text sizes and pixel ratios. The
// fitted size goes to the program through the existing resize control
// message; what is drawn is always the size the supervisor last committed
// in a frame.
//
// ## Seams for later tiers
//
// Selection and copy, paste (and its attachment classification), local
// scrollback viewing, links and their hover disclosures are not built
// here yet. The places they attach are marked "Tier 2" or "Tier 3" below;
// until then those gestures do nothing rather than reaching the program
// in a form today's terminal would not send.
(function (root) {
  "use strict";

  // The fit addon reserves this for xterm.js's scrollbar whenever there is
  // scrollback; matching it keeps a pane's column count the same.
  const SCROLLBAR_WIDTH = 14;
  const MAX_DIMENSION = 1000;
  const MEASURE_TEXT = "W".repeat(32);
  // xterm.js's size before its first fit.
  const DEFAULT_SIZE = Object.freeze({ cols: 80, rows: 24 });

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
   * Mount a grid terminal into `container`.
   *
   * `options`: `fontSize` and `fontFamily` (the same as the xterm.js
   * terminals use), `theme` (terminal-theme.js), `platform`
   * (`navigator.platform`), `send(message)` for input messages,
   * `onResize(cols, rows)` when the fitted size changes, and `size`, the
   * size the program already has (the one the socket URL carried; xterm.js's
   * initial 80 x 24 when omitted), which a fit reports only if it differs,
   * `onClipboard(text, target)` for a program's completed clipboard write
   * ("clipboard" or "primary"), and `flowCovered: false` to box every
   * non-ASCII cell (for the renderer benchmark's comparison only).
   *
   * Feed data frames to `receive`, which throws
   * `farhelmGridModel.GridProtocolError` on a frame that violates the
   * format (the caller ends the attachment). Call `dispose` when the
   * attachment ends; it removes everything this mounted, including the
   * document listeners a mouse drag installs.
   */
  function create(container, options) {
    const gridModel = root.farhelmGridModel;
    const gridStyle = root.farhelmGridStyle;
    const gridInput = root.farhelmGridInput;
    const model = new gridModel.GridModel();
    const styles = gridStyle.createStyleMapper(options.theme);
    const platform = options.platform || "";
    let send = options.send;
    let onResize = options.onResize;
    let onClipboard = options.onClipboard;
    // On unless a caller (the renderer benchmark) asks for a box per
    // non-ASCII cell; see `flows`.
    const flowCovered = options.flowCovered !== false;

    // ## DOM

    const element = document.createElement("div");
    element.className = "grid-terminal";
    const screen = document.createElement("div");
    screen.className = "grid-terminal-screen";
    // Decorative to assistive technology for now, as xterm.js's rows are;
    // an accessible reading of the grid is not part of today's terminal.
    screen.setAttribute("aria-hidden", "true");
    const cursorEl = document.createElement("div");
    cursorEl.className = "grid-terminal-cursor";
    screen.appendChild(cursorEl);
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
    element.append(screen, exitEl, input, measure, glyphMeasure);
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

    // ## Painting

    const rowEls = [];
    let rafHandle = null;
    let repaintAll = true;
    let focused = false;

    function schedulePaint() {
      if (rafHandle !== null) return;
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
        screen.insertBefore(row, cursorEl);
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
        screen.insertBefore(row, cursorEl);
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
     * Draw screen row `y` into its element, reusing its spans.
     *
     * Adjacent text that flows (printable ASCII, a gap of blank cells, and
     * with `flowCovered` the characters `flows` admits) and shares one style
     * is drawn as one span; every other cell is a box of exactly its width.
     */
    function paintRow(y) {
      const rowEl = rowEls[y];
      const row = model.screen[y];
      if (!rowEl || !row) return;
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
        if (run.col > next) flow("", " ".repeat(run.col - next));
        const css = styles.css(run.style);
        // Hidden text keeps its colors and lines but never enters the page.
        const hidden = (run.style.flags & gridStyle.FLAGS.HIDDEN) !== 0;
        const shown = gridModel.displayText(run.text);
        const pieces = gridModel.layoutRun(shown, run.width);
        // A cell whose drawn text is empty (only a bidi control) is still a
        // cell: it keeps its box so the columns after it stay put.
        if (pieces === null || hidden || (run.width === 1 && shown !== "" && flows(shown, run.style))) {
          flow(css, hidden ? " ".repeat(run.width) : shown);
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
     * (xterm.js's `cursorInactiveStyle`). Blinks only while focused.
     */
    function paintCursor() {
      const cursor = model.cursor;
      if (!model.started || !cursor.visible || !cellWidth) {
        cursorEl.style.display = "none";
        // Forget the cached declarations so showing it again rewrites them.
        cursorEl.gridCss = null;
        return;
      }
      const cell = model.cellAt(cursor.x, cursor.y);
      const colors = styles.cursor();
      let css = `display:block;left:${cell.col * cellWidth}px;top:${cursor.y * cellHeight}px;` +
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

    function paint() {
      if (rafHandle !== null) {
        root.cancelAnimationFrame(rafHandle);
        rafHandle = null;
      }
      // Late metrics (a container that had no layout box when this was
      // created) or a new size lay every row out again.
      if (currentCell()) repaintAll = true;
      const damage = model.takeDamage();
      if (damage.state || repaintAll) paintState();
      if (repaintAll || damage.all) {
        repaintAll = false;
        layoutRows();
        for (let y = 0; y < model.rows; y++) paintRow(y);
      } else {
        if (damage.shift > 0) rotateRows(damage.shift);
        for (const y of damage.rows) paintRow(y);
      }
      paintCursor();
      layoutExit();
    }

    // ## Input

    let deadKeyPending = false;
    let composing = false;

    function sendMessage(message) {
      if (message && send) send(message);
    }

    function sendText(data) {
      deadKeyPending = false;
      for (const message of gridInput.textMessages(data)) sendMessage(message);
    }

    function cancel(ev) {
      ev.preventDefault();
      ev.stopPropagation();
    }

    // Page shortcuts (the quick switcher, templates, session actions, text
    // size) are window capture listeners that stop the event before it gets
    // here, so they never reach the program, exactly as with xterm.js.
    input.addEventListener("keydown", (ev) => {
      const decision = gridInput.keydownAction(ev, platform, deadKeyPending);
      deadKeyPending = decision.deadKeyPending;
      if (decision.action === "send") {
        sendMessage(gridInput.keyMessage(ev));
        cancel(ev);
        return;
      }
      // Shift+PageUp/Down and Cmd+A act on the local view (scrollback and
      // selection, Tier 2) and never reach the program.
      if (decision.cancel) cancel(ev);
    });
    input.addEventListener("input", (ev) => {
      // Composition text arrives at compositionend; a paste arrives as a
      // paste (Tier 2). Only typed text is sent from here.
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
      // (an input-method gate for the client parity work).
      if (ev.data) sendText(ev.data);
      input.value = "";
    });
    // Tier 2: paste, with today's attachment classification for pasted
    // content. Until then a paste must not reach the program as typed text.
    input.addEventListener("paste", (ev) => ev.preventDefault());
    input.addEventListener("focus", () => setFocused(true));
    input.addEventListener("blur", () => setFocused(false));

    function setFocused(value) {
      if (focused === value) return;
      focused = value;
      element.classList.toggle("grid-terminal-focused", value);
      // Focus in/out reports are what the program asked for with mode 1004
      // (the report of the state at the moment it asks is in `receive`).
      if (model.state.focus) sendMessage({ t: "focus", in: value });
      paintCursor();
    }

    // Mouse reporting. The press handler is on the screen; a drag's moves
    // and release are followed on the document, as xterm.js does, so a
    // release outside the terminal still reaches the program.
    const mouseDedupe = gridInput.createMouseDeduper();
    const wheel = gridInput.createWheelAccumulator();
    let dragging = false;

    /** The message for a pointer event, or null before the grid has a size. */
    function pointerMessage(kind, button, ev) {
      if (!model.started || !cellWidth) return null;
      const rect = screen.getBoundingClientRect();
      const cell = gridInput.mouseCell(ev.clientX - rect.left, ev.clientY - rect.top, cellWidth, cellHeight, model.cols, model.rows);
      return { t: "mouse", kind, button, x: cell.x, y: cell.y, mods: gridInput.modifiers(ev) };
    }

    function mouseMessage(kind, button, ev) {
      if (!gridInput.modeReports(model.state.mouse, kind, button)) return;
      const message = pointerMessage(kind, button, ev);
      if (message && mouseDedupe.shouldSend(message)) sendMessage(message);
    }

    function onDocumentMouseMove(ev) {
      if (ev.buttons) mouseMessage("move", gridInput.moveButton(ev.buttons), ev);
    }

    function onDocumentMouseUp(ev) {
      const button = gridInput.pressButton(ev.button);
      if (button !== null) mouseMessage("up", button, ev);
      if (!ev.buttons) stopDrag();
    }

    function stopDrag() {
      if (!dragging) return;
      dragging = false;
      document.removeEventListener("mousemove", onDocumentMouseMove);
      document.removeEventListener("mouseup", onDocumentMouseUp);
    }

    element.addEventListener("mousedown", (ev) => {
      // Keep keyboard focus in the terminal and stop the browser's own
      // text selection, as xterm.js's press handler does.
      ev.preventDefault();
      focus();
      // A Shift-press (Option on a Mac) forces a local selection even under
      // mouse reporting; selection itself is Tier 2.
      if (root.farhelmCopyOnSelect && root.farhelmCopyOnSelect.pressForcesSelection(ev, platform)) return;
      const button = gridInput.pressButton(ev.button);
      if (button === null || model.state.mouse === "off") return;
      mouseMessage("down", button, ev);
      if (!dragging) {
        dragging = true;
        document.addEventListener("mousemove", onDocumentMouseMove);
        document.addEventListener("mouseup", onDocumentMouseUp);
      }
    });
    element.addEventListener("mousemove", (ev) => {
      if (!ev.buttons) mouseMessage("move", -1, ev);
    });
    element.addEventListener("wheel", (ev) => {
      if (!gridInput.wheelGoesToProgram(model.state)) return; // Tier 2: local scrollback.
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
    }, { passive: false });
    // No context-menu handling: xterm.js leaves the browser's menu alone
    // under mouse reporting too (the right button is still reported). Its
    // trick of moving the text field under the pointer, so the menu offers
    // Paste, comes with paste (Tier 2).

    function focus() {
      input.focus({ preventScroll: true });
    }

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

      /** Apply one data frame. Throws `GridProtocolError` on a bad one. */
      receive(data) {
        const reporting = model.state.focus;
        const message = gridModel.decodeFrame(data);
        model.apply(message);
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

      /**
       * What the terminal shows, as plain data: for tests and the e2e
       * helpers, which read the model rather than the DOM.
       */
      snapshot() {
        const lines = model.screen.map((row) => {
          let text = "";
          let col = 0;
          for (const run of row.runs) {
            text += " ".repeat(run.col - col) + run.text;
            col = run.col + run.width;
          }
          return text;
        });
        return { cols: model.cols, rows: model.rows, lines, cursor: { ...model.cursor }, state: { ...model.state } };
      },

      dispose() {
        if (rafHandle !== null) root.cancelAnimationFrame(rafHandle);
        rafHandle = null;
        if (pixelRatioWatch) pixelRatioWatch.query.removeEventListener("change", pixelRatioWatch.listener);
        pixelRatioWatch = null;
        stopDrag();
        send = null;
        onResize = null;
        onClipboard = null;
        element.remove();
      },
    };
  }

  root.farhelmGridTerminal = { create, exitText };
})(typeof window === "object" ? window : globalThis);
