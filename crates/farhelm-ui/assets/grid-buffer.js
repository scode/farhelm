// The grid client's scrollback as one buffer of lines, and the text-level
// questions asked of it: what a selection copies, where a word or a line
// starts and ends, and which link is under a cell. Pure, no DOM:
// grid-terminal.js draws and gestures, this decides, and `node --test`
// covers it.
//
// ## Lines and their numbers
//
// A line is a history line or a screen row, named by the absolute line
// number grid-model.js keeps: history lines by their own numbers, screen
// row `y` as `model.screenBase + y`. Those numbers do not change when output
// scrolls (the line that was screen row 0 is history line `screenBase`
// once it scrolls off) or when the oldest history is dropped at the cap, so
// a scrolled-back view and a selection anchored to them stay on the same
// text while output continues, as xterm.js's do. A position is
// `{abs, col}`, where `col` is a cell boundary (0 to the line's width), and
// a range is `[start, end)` between two positions.
//
// ## Text as today's terminal copies it
//
// `selectionText` follows xterm.js's `SelectionService.selectionText`: rows
// joined with a newline, except that a row the emulator soft-wrapped (`w`)
// continues on the next one with nothing between them; a line that ends is
// trimmed on the right, and a wrapped one keeps its trailing blanks up to
// its own width (history lines keep the width they were written at, so this
// survives a resize), except the padding cell the emulator leaves before a
// wide character that wrapped (`padsWideWrap`). A wide character counts
// once, for the cell it starts in. Non-breaking spaces copy as spaces, and
// Windows gets CRLF. A column selection (`columnText`) copies the same
// columns of every line, one per line.
//
// The emulator cannot tell a space the program printed from a cell nothing
// touched, so trimming drops trailing spaces where xterm.js dropped only
// untouched cells: a line that ends in printed spaces copies without them.
//
// ## Links
//
// Two kinds, with xterm.js's precedence: a program's OSC 8 hyperlink (runs
// carrying a `link`), and an http(s) URL printed as text, found with the
// vendored web-links addon's own pattern over the soft-wrapped line around
// the cell (its `_getWindowedLineStrings` window: back while rows continue
// and do not start with a space, forward while they continue, stopping at a
// row with a space in it, 2048 characters either way). A text URL that
// overlaps an OSC 8 run is dropped. Targets are data: what may be opened is
// decided at the click by terminal-links.js's rules, never here.
// Generated OSC ids also continue directly adjacent same-target pieces,
// across soft wraps. SPEC_impl.md explains the spelling dependency and why
// the mismatch check may use this chain as one hyperlink.
(function (root) {
  "use strict";

  // xterm.js 6.0's default `wordSeparator`, which Farhelm leaves as is.
  const WORD_SEPARATORS = " ()[]{}',\"`";
  // The vendored @xterm/addon-web-links's default `urlRegex`.
  const URL_PATTERN = /(https?|HTTPS?):[/]{2}[^\s"'!*(){}|\\^<>`]*[^\s"':,.!?{}|\\^~[\]`()<>]/g;
  // How far the URL window reaches either way, in characters (the addon's),
  // and how far a double click's word search reaches.
  const URL_WINDOW = 2048;
  const WORD_WINDOW = 2048;
  const PRINTABLE_ASCII = /^[\x20-\x7e]*$/;
  const NBSP = /\u00a0/g;

  /**
   * One line's text over the cell boundaries `[from, to)`: blanks as
   * spaces, a wide character once (for the cell it starts in, so a range
   * starting on its second cell leaves it out and one ending there takes
   * it), combining characters with their base. Not trimmed.
   */
  function rowText(row, width, from = 0, to = width) {
    let out = "";
    let col = from;
    for (const run of row.runs) {
      const end = run.col + run.width;
      if (end <= from) continue;
      if (run.col >= to) break;
      if (run.col > col) {
        out += " ".repeat(run.col - col);
        col = run.col;
      }
      if (run.text.length === run.width && PRINTABLE_ASCII.test(run.text)) {
        const a = Math.max(from, run.col);
        const b = Math.min(to, end);
        out += run.text.slice(a - run.col, b - run.col);
        col = b;
      } else {
        if (run.col >= from) out += run.text;
        col = Math.min(to, end);
      }
    }
    if (col < to) out += " ".repeat(to - col);
    return out;
  }

  /**
   * Whether `line` (number `abs`) ends in the emulator's padding before a
   * wide character that did not fit: it wraps, its last cell is one no run
   * covers, and the next line starts with a two-cell character (a run that
   * is not printable ASCII and two cells wide: a printable ASCII run of two
   * cells is two characters). That cell
   * is not text: xterm.js leaves it out of copies (and terminal-files.js's
   * `pathLinks` out of paths), so the copy and the word and URL searches
   * here do too. A blank the program printed is covered by a run and stays.
   */
  function padsWideWrap(source, abs, line) {
    if (!line.row.wrapped || line.row.end > line.width - 1) return false;
    const next = source.line(abs + 1);
    const first = next && next.row.runs[0];
    return !!first && first.col === 0 && first.width === 2 && !PRINTABLE_ASCII.test(first.text);
  }

  /** Order two positions. */
  function compare(a, b) {
    return a.abs !== b.abs ? a.abs - b.abs : a.col - b.col;
  }

  function ordered(a, b) {
    return compare(a, b) <= 0 ? [a, b] : [b, a];
  }

  /**
   * The text of the range `[start, end)` (see "Text as today's terminal
   * copies it"). `source.line(abs)` gives `{row, width}` or null for a line
   * that is not there (dropped, or history not yet applied), which
   * contributes nothing. `newline` is "\n", or "\r\n" on Windows.
   */
  function selectionText(source, start, end, newline = "\n") {
    [start, end] = ordered(start, end);
    const lines = [];
    let joinNext = false;
    for (let abs = start.abs; abs <= end.abs; abs++) {
      const line = source.line(abs);
      if (!line) {
        joinNext = false;
        continue;
      }
      const from = abs === start.abs ? Math.min(start.col, line.width) : 0;
      const to = abs === end.abs ? Math.min(end.col, line.width) : line.width;
      let text = from < to ? rowText(line.row, line.width, from, to) : "";
      // A row that continues on the next keeps its blanks up to its width:
      // they are part of the logical line.
      const continues = line.row.wrapped && abs !== end.abs;
      if (!continues) text = text.replace(/ +$/, "");
      else if (to === line.width && padsWideWrap(source, abs, line)) text = text.slice(0, -1);
      if (joinNext && lines.length > 0) lines[lines.length - 1] += text;
      else lines.push(text);
      joinNext = continues;
    }
    return lines.join(newline).replace(NBSP, " ");
  }

  /**
   * The soft-wrapped line around `abs`: the rows from the first one whose
   * predecessor does not continue onto it to the last that does not
   * continue, bounded by `limit` rows and by `cells` cells (the sum of the
   * rows' widths, which may differ between history lines) either way.
   * `{first, last}` in absolute numbers.
   */
  function wrappedSpan(source, abs, limit = 1000, cells = Infinity) {
    let first = abs;
    let budget = cells;
    for (let n = 0; n < limit; n++) {
      const previous = source.line(first - 1);
      if (!previous || !previous.row.wrapped || budget < previous.width) break;
      budget -= previous.width;
      first--;
    }
    let last = abs;
    budget = cells;
    for (let n = 0; n < limit; n++) {
      const line = source.line(last);
      const next = line && line.row.wrapped ? source.line(last + 1) : null;
      if (!next || budget < next.width) break;
      budget -= next.width;
      last++;
    }
    return { first, last };
  }

  /**
   * The text of rows `first..last` joined, with the cell each character
   * came from: `chars[i]` is `{abs, col, width}` for `text[i]` (both code
   * units of a surrogate pair, and combining characters, share their
   * cell). Every row is taken at its full width, so blanks are there too.
   */
  function mappedText(source, first, last) {
    let text = "";
    const chars = [];
    for (let abs = first; abs <= last; abs++) {
      const line = source.line(abs);
      if (!line) continue;
      let col = 0;
      const blank = (count) => {
        for (let i = 0; i < count; i++) {
          text += " ";
          chars.push({ abs, col: col + i, width: 1 });
        }
        col += count;
      };
      for (const run of line.row.runs) {
        if (run.col > col) blank(run.col - col);
        if (run.text.length === run.width && PRINTABLE_ASCII.test(run.text)) {
          for (let i = 0; i < run.width; i++) chars.push({ abs, col: run.col + i, width: 1 });
          text += run.text;
        } else {
          for (let i = 0; i < run.text.length; i++) chars.push({ abs, col: run.col, width: run.width });
          text += run.text;
        }
        col = run.col + run.width;
      }
      const end = line.width - (padsWideWrap(source, abs, line) ? 1 : 0);
      if (col < end) blank(end - col);
    }
    return { text, chars };
  }

  /** The index in `chars` of the character drawn in cell `col` of line `abs`. */
  function charIndex(chars, abs, col) {
    for (let i = 0; i < chars.length; i++) {
      const c = chars[i];
      if (c.abs === abs && col >= c.col && col < c.col + c.width) return i;
    }
    return -1;
  }

  /** The range a character index range `[a, b)` of `mappedText` covers. */
  function charsRange(chars, a, b) {
    const first = chars[a];
    const last = chars[b - 1];
    return { start: { abs: first.abs, col: first.col }, end: { abs: last.abs, col: last.col + last.width } };
  }

  /**
   * The word a double click at `pos` selects, as xterm.js's
   * `_getWordAt` finds it: on a space, the run of spaces; otherwise from the
   * clicked character outwards until a word separator, across soft wraps.
   * The clicked character itself may be a separator, which then starts the
   * word (a double click on "(" in "(foo)" selects "(foo"). Null when there
   * is no line there.
   */
  function wordRange(source, pos, separators = WORD_SEPARATORS) {
    if (!source.line(pos.abs)) return null;
    // Only the rows around the click within `WORD_WINDOW` cells either way:
    // a program can print one soft-wrapped line of thousands of rows, and
    // the search builds a cell record per character.
    const span = wrappedSpan(source, pos.abs, Infinity, WORD_WINDOW);
    const { text, chars } = mappedText(source, span.first, span.last);
    let at = charIndex(chars, pos.abs, pos.col);
    if (at < 0) return null;
    // Step back to the first code unit of the clicked cell.
    while (at > 0 && chars[at - 1].abs === chars[at].abs && chars[at - 1].col === chars[at].col) at--;
    const cellStart = (i) => {
      while (i > 0 && chars[i - 1].abs === chars[i].abs && chars[i - 1].col === chars[i].col) i--;
      return i;
    };
    const cellEnd = (i) => {
      while (i + 1 < chars.length && chars[i + 1].abs === chars[i].abs && chars[i + 1].col === chars[i].col) i++;
      return i + 1;
    };
    let start = at;
    let end = cellEnd(at);
    if (text[at] === " ") {
      while (start > 0 && text[start - 1] === " ") start--;
      while (end < text.length && text[end] === " ") end++;
    } else {
      const separator = (i) => separators.includes(text[i]);
      while (start > 0 && !separator(cellStart(start - 1))) start = cellStart(start - 1);
      while (end < text.length && !separator(end)) end = cellEnd(end);
    }
    return charsRange(chars, start, end);
  }

  /** The whole soft-wrapped line through `pos`, as a triple click selects it. */
  function lineRange(source, pos) {
    if (!source.line(pos.abs)) return null;
    const span = wrappedSpan(source, pos.abs);
    const last = source.line(span.last);
    return { start: { abs: span.first, col: 0 }, end: { abs: span.last, col: last.width } };
  }

  /** The OSC 8 link of the run covering cell `col` of `row`, or null. */
  function oscLinkAt(row, col) {
    for (const run of row.runs) {
      if (col < run.col) break;
      if (col < run.col + run.width) return run.link || null;
    }
    return null;
  }

  /**
   * Resolve a run's link to its chain's first piece, within one question.
   * Only numeric emulator-generated ids may inherit a neighbour's identity;
   * explicit ids keep their ordinary id/target identity. A row boundary
   * continues only at a soft wrap with no intervening cell of real text.
   *
   * Resolve whole rows forwards after looking back at most 128 lines. The
   * per-question cache makes one-cell restarts linear in the runs examined,
   * rather than walking the chain again for every run or evidence row.
   */
  function linkIdentities(source) {
    const rows = new Map();
    const generated = (link) => !!link && /^\d+_alacritty$/.test(link.id);
    const joins = (a, b) => generated(a) && generated(b) && a.uri === b.uri;
    const previousPiece = (abs, line) => {
      const first = line.row.runs[0];
      if (!first || first.col !== 0 || !generated(first.link)) return null;
      const previous = source.line(abs - 1);
      const last = previous && previous.row.runs[previous.row.runs.length - 1];
      if (!previous || !previous.row.wrapped || !last || !joins(last.link, first.link)) return null;
      const end = last.col + last.width;
      return end === previous.width || (end === previous.width - 1 && padsWideWrap(source, abs - 1, previous))
        ? previous
        : null;
    };
    return (abs) => {
      const pending = [];
      let at = abs;
      for (let n = 0; n <= 128 && !rows.has(at); n++, at--) {
        const line = source.line(at);
        if (!line) break;
        pending.push([at, line]);
        if (!previousPiece(at, line)) break;
      }
      for (let i = pending.length - 1; i >= 0; i--) {
        const [number, line] = pending[i];
        const keys = [];
        const previous = previousPiece(number, line);
        const above = previous && rows.get(number - 1);
        for (const [index, run] of line.row.runs.entries()) {
          const before = line.row.runs[index - 1];
          const contiguous = before && before.col + before.width === run.col && joins(before.link, run.link);
          keys.push(contiguous ? keys[index - 1] : index === 0 && above ? above[above.length - 1] : run.link);
        }
        rows.set(number, keys);
      }
      return rows.get(abs) || [];
    };
  }

  /** The stable id/target pair, including a generated chain's first id. */
  function linkKey(link) {
    return link ? JSON.stringify([link.id, link.uri]) : null;
  }

  /**
   * Whether two resolved links are the same id/target pair. The hover's
   * per-run checks use this rather than comparing `linkKey`s: a key is as long
   * as the target, and a screen any program can print (128 links per row with
   * 2 KB targets, compared across 257 rows) made building one per run cost
   * seconds per hover lookup.
   */
  function sameLink(a, b) {
    return !!a && !!b && a.id === b.id && a.uri === b.uri;
  }

  /**
   * Every link touching line `abs`, in xterm.js's precedence order (OSC 8
   * links first, then URLs printed as text): `{kind, uri, ranges, evidence}`
   * (an OSC 8 link also has its `id`) where
   * `kind` is "osc" (a program's hyperlink) or "text", and `ranges` are the
   * `[start, end)` cell ranges it covers, one per line, for the hover
   * underline. An OSC 8 link is one contiguous stretch of runs of the same
   * link on this line (xterm.js's OSC provider makes one link per such
   * stretch), including a generated-id continuation chain. A chain's tag
   * replaces later pieces' tags, so a later piece continued by cursor
   * movement onto an unwrapped row no longer joins that row by its own id.
   * A text URL's ranges may cross soft wraps. Its evidence includes the
   * search window and its boundaries: an edit outside the matched cells
   * can extend or remove the address while leaving those cells unchanged.
   */
  function rowLinks(source, abs) {
    const line = source.line(abs);
    if (!line) return [];
    const identities = linkIdentities(source);
    const keys = identities(abs);
    const found = [];
    let current = null;
    for (const [index, run] of line.row.runs.entries()) {
      const link = keys[index];
      if (current && sameLink(link, current.link) && current.end === run.col) {
        current.end = run.col + run.width;
        continue;
      }
      if (current) found.push(current);
      current = link ? { link, start: run.col, end: run.col + run.width } : null;
    }
    if (current) found.push(current);
    const links = found.map(({ link, start, end }) => ({
      kind: "osc",
      id: link.id,
      uri: link.uri,
      ranges: [{ start: { abs, col: start }, end: { abs, col: end } }],
      // The lines its hover text is joined from (see `taggedLines`).
      evidence: taggedLines(source, abs, link.id, link.uri, identities),
    }));
    const window = urlWindow(source, abs);
    const { text, chars } = mappedText(source, window.first, window.last);
    const evidence = [];
    for (let at = window.first - 1; at <= window.last + 1; at++) evidence.push(at);
    URL_PATTERN.lastIndex = 0;
    for (let match; (match = URL_PATTERN.exec(text));) {
      const a = match.index;
      const b = a + match[0].length;
      const cells = chars.slice(a, b);
      if (!cells.some((c) => c.abs === abs)) continue;
      if (!hostPrefixed(match[0])) continue;
      // An OSC 8 hyperlink keeps the cells it covers.
      if (cells.some((c) => { const l = source.line(c.abs); return l && oscLinkAt(l.row, c.col); })) continue;
      links.push({ kind: "text", uri: match[0], ranges: splitByLine(charsRange(chars, a, b), source), evidence });
    }
    return links;
  }

  /**
   * The lines an OSC 8 link's hover text depends on: the run of adjacent
   * lines carrying the same resolved link (id and target), as terminal-links.js's
   * `linkHoverText` joins them (at most 128 either way), and the line just
   * beyond each end, whose gaining the link would extend the join.
   */
  function taggedLines(source, abs, id, uri, identities = linkIdentities(source)) {
    const wanted = { id, uri };
    const carries = (at) => identities(at).some((link) => sameLink(link, wanted));
    let first = abs;
    while (abs - first < 128 && carries(first - 1)) first--;
    let last = abs;
    while (last - abs < 128 && carries(last + 1)) last++;
    const lines = [];
    for (let at = first - 1; at <= last + 1; at++) lines.push(at);
    return lines;
  }

  /**
   * The text of a column (block) selection, as xterm.js copies one: every
   * line from `start` to `end` cut to the same column span, trimmed on the
   * right, one per line with no joining of soft wraps.
   */
  function columnText(source, start, end, newline = "\n") {
    const from = Math.min(start.col, end.col);
    const to = Math.max(start.col, end.col);
    if (from === to) return "";
    const lines = [];
    for (let abs = Math.min(start.abs, end.abs); abs <= Math.max(start.abs, end.abs); abs++) {
      const line = source.line(abs);
      if (!line) continue;
      const a = Math.min(from, line.width);
      const b = Math.min(to, line.width);
      lines.push(a < b ? rowText(line.row, line.width, a, b).replace(/ +$/, "") : "");
    }
    return lines.join(newline).replace(NBSP, " ");
  }

  /**
   * The lines a provider's answer about line `abs` can depend on: its
   * soft-wrapped line (within the 128 rows either way today's path detector
   * reads) and the line before it, whose wrap decides where it starts; the
   * rows the printed-URL search reads (`urlWindow`, 2,048 characters either
   * way, which on narrow rows reaches past the path detector's bound) with
   * the row beyond each end it looked at; and, for each OSC 8 link on the
   * line, the rows its hover text is joined from with one beyond each end
   * (`taggedLines`). A late answer is still about the screen exactly when
   * none of these rows was replaced.
   */
  function answerLines(source, abs) {
    const lines = new Set();
    const span = wrappedSpan(source, abs, 128);
    for (let at = span.first - 1; at <= span.last; at++) lines.add(at);
    const line = source.line(abs);
    if (line) {
      const window = urlWindow(source, abs);
      for (let at = Math.min(window.first, abs) - 1; at <= window.last + 1; at++) lines.add(at);
    }
    const seen = new Set();
    const identities = linkIdentities(source);
    for (const link of identities(abs)) {
      if (!link) continue;
      const key = linkKey(link);
      if (seen.has(key)) continue;
      seen.add(key);
      for (const at of taggedLines(source, abs, link.id, link.uri, identities)) lines.add(at);
    }
    return lines;
  }

  /** Whether `link` covers cell `pos` (its ranges may each cross lines). */
  function linkCovers(link, pos) {
    return link.ranges.some((range) => compare(pos, range.start) >= 0 && compare(pos, range.end) < 0);
  }

  /** The link under cell `pos` (the first of `rowLinks` covering it), or null. */
  function linkAt(source, pos) {
    return rowLinks(source, pos.abs).find((link) => linkCovers(link, pos)) || null;
  }

  /**
   * The addon's second check on a match: the text must start with what the
   * URL parser makes of its scheme, credentials and host, so a match the
   * parser would rewrite is not offered.
   */
  function hostPrefixed(text) {
    try {
      const url = new URL(text);
      const prefix = url.password && url.username
        ? `${url.protocol}//${url.username}:${url.password}@${url.host}`
        : url.username ? `${url.protocol}//${url.username}@${url.host}` : `${url.protocol}//${url.host}`;
      return text.toLocaleLowerCase().startsWith(prefix.toLocaleLowerCase());
    } catch (_error) {
      return false;
    }
  }

  /** The rows the web-links addon would search around `abs` (see "Links"). */
  function urlWindow(source, abs) {
    const line = source.line(abs);
    let first = abs;
    let last = abs;
    const own = rowText(line.row, line.width);
    const previous = source.line(abs - 1);
    if (previous && previous.row.wrapped && own[0] !== " ") {
      let length = 0;
      for (let at = abs - 1; ; at--) {
        const above = source.line(at);
        if (!above || !above.row.wrapped || length >= URL_WINDOW) break;
        const text = rowText(above.row, above.width);
        first = at;
        length += text.length;
        if (text.includes(" ")) break;
      }
    }
    let length = 0;
    for (let at = abs; ; at++) {
      const current = source.line(at);
      const below = source.line(at + 1);
      if (!current || !current.row.wrapped || !below || length >= URL_WINDOW) break;
      const text = rowText(below.row, below.width);
      last = at + 1;
      length += text.length;
      if (text.includes(" ")) break;
    }
    return { first, last };
  }

  /** Split a range that may cross lines into one range per line. */
  function splitByLine(range, source) {
    const out = [];
    for (let abs = range.start.abs; abs <= range.end.abs; abs++) {
      const line = source.line(abs);
      if (!line) continue;
      out.push({
        start: { abs, col: abs === range.start.abs ? range.start.col : 0 },
        end: { abs, col: abs === range.end.abs ? range.end.col : line.width },
      });
    }
    return out;
  }

  /**
   * A line source over a grid model: `line(abs)` for any history line that
   * has been applied (when `history` is true) and every screen row, with the
   * first and last numbers there are. On the alternate screen the view
   * passes `history: false`, as xterm.js's alternate buffer has none.
   */
  function modelSource(model, history = true) {
    const base = model.screenBase;
    return {
      first: history && model.historyLength > 0 ? model.historyFirst : base,
      last: base + model.rows - 1,
      line(abs) {
        if (abs >= base && abs < base + model.rows) return { row: model.screen[abs - base], width: model.cols };
        if (!history || abs < model.historyFirst || abs >= model.historyEnd) return null;
        const row = model.historyLine(abs - model.historyFirst);
        return row ? { row, width: row.cols } : null;
      },
    };
  }

  // ## The link code's lines
  //
  // terminal-links.js and terminal-files.js read a terminal as "link lines"
  // (terminal-links.js's header, "Link lines"): `line(abs)` with `length`,
  // `wrapped`, `cell(x)` (`chars`, `width`, `tag`) and `text`. `linkLines`
  // is that over a line source, so the hyperlink text a hover compares, the
  // printed-path detector and the file hover's redraw check run on the
  // grid's own lines; an old terminal's xterm.js buffer gets an adapter to
  // the same shape on its side (terminal-links.js's `xtermLines`).
  //
  // Line `abs` is grid-buffer.js's line number, and ranges are this file's
  // `[start, end)` line ranges. A cell no run covers reads as empty chars
  // with width 1, a wide character's second cell as empty chars with width
  // 0, and only cells a run covers count toward the trimmed length. Hyperlink
  // tags are small integers, equal for runs whose links resolve to the same
  // id and target, including direct generated-id continuations (see
  // SPEC_impl.md), and 0 for no link, assigned per `linkLines` call: callers
  // ask for fresh lines per question, so the table never outlives one hover.

  /**
   * Present one grid row as a link line. `tagOf(index)` resolves the run at
   * its row position: a generated id's tag may come from an earlier piece,
   * rather than from the raw wire id.
   */
  function lineView(line, previous, tagOf) {
    const { row, width } = line;
    const cells = new Array(width);
    let trimmed = 0;
    for (const [index, run] of row.runs.entries()) {
      const tag = run.link ? tagOf(index) : 0;
      if (run.text.length === run.width && PRINTABLE_ASCII.test(run.text)) {
        for (let i = 0; i < run.width && run.col + i < width; i++) cells[run.col + i] = { chars: run.text[i], width: 1, tag };
      } else {
        if (run.col < width) cells[run.col] = { chars: run.text, width: run.width, tag };
        if (run.width === 2 && run.col + 1 < width) cells[run.col + 1] = { chars: "", width: 0, tag };
      }
      trimmed = Math.min(width, run.col + run.width);
    }
    const blank = { chars: "", width: 1, tag: 0 };
    const at = (x) => cells[x] || blank;
    return {
      length: width,
      wrapped: !!(previous && previous.row.wrapped),
      cell(x) {
        if (!Number.isInteger(x) || x < 0 || x >= width) return undefined;
        const cell = at(x);
        return { chars: cell.chars, width: cell.width, tag: cell.tag };
      },
      // A cell's characters (a space for an empty one), stepping over a wide
      // character's second cell.
      text(startCol = 0, endCol = width, trimRight = false) {
        const end = Math.min(width, trimRight ? Math.min(endCol, trimmed) : endCol);
        let out = "";
        for (let x = Math.max(0, startCol); x < end;) {
          const cell = at(x);
          out += cell.chars || " ";
          x += cell.width || 1;
        }
        return out;
      },
    };
  }

  /**
   * The link lines over `source` (see "The link code's lines"): `line(abs)`,
   * undefined for a line that is not there.
   */
  function linkLines(source) {
    const tags = new Map();
    const identities = linkIdentities(source);
    const tagOf = (link) => {
      const key = linkKey(link);
      let tag = tags.get(key);
      if (tag === undefined) {
        tag = tags.size + 1;
        tags.set(key, tag);
      }
      return tag;
    };
    return {
      line(abs) {
        if (!Number.isSafeInteger(abs)) return undefined;
        const line = source.line(abs);
        const keys = line && identities(abs);
        return line ? lineView(line, source.line(abs - 1), (index) => tagOf(keys[index])) : undefined;
      },
    };
  }

  const api = {
    URL_PATTERN,
    WORD_SEPARATORS,
    answerLines,
    columnText,
    compare,
    lineRange,
    linkAt,
    linkCovers,
    linkLines,
    modelSource,
    ordered,
    rowLinks,
    rowText,
    selectionText,
    taggedLines,
    wordRange,
    wrappedSpan,
  };
  root.farhelmGridBuffer = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(typeof window === "object" ? window : globalThis);
