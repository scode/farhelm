// The grid client held to the grid stream format's golden fixtures.
//
// `crates/farhelm-proto/grid-fixtures/` holds one JSON file per message,
// and `farhelm-proto`'s own tests decode every one of them with the Rust
// types the supervisor sends and receives with (`grid_tests.rs`). This file
// runs the same files through the browser's side: grid-model.js decodes
// what the supervisor sends, and grid-input.js builds what the client sends
// back. Two hand-maintained implementations of one format drift apart
// quietly (the client once clamped a cursor the Rust side refuses), and the
// fixtures are the one place both sides read, so they are the format's
// source of truth here: where the client and a fixture disagree, the client
// changes, unless the format document says the client is meant to be more
// lenient.
//
// What each directory means for the client:
//
// - `output/valid/` must be accepted and must come out of the model as it
//   went in (the same size, cursor, screen rows after a shift, runs, links,
//   history lines, clipboard write), since accepting a message and then
//   drawing something else is a disagreement too. Each is applied over a
//   state where its effect shows, asserted first: a frame or a `rows`
//   update over a committed screen whose rows are told apart (so a shift, a
//   row cleared and a row left alone all show), a state over one that
//   differs in every field, a history reset over existing history.
// - `output/tolerated/` must be accepted and must then hold what its
//   canonical form says (`TOLERATED_CANONICAL`: the unknown field dropped,
//   reserved style bits cleared), over the same observing prelude as a
//   valid fixture, so a client that ignored the whole message cannot pass
//   for one that ignored only the extension (except where the message
//   carries nothing to lose, such as `rows-empty-links`).
// - `output/invalid/` must be refused with `GridProtocolError`, for the
//   reason the fixture is named after. `INVALID_REASONS` pins the client's
//   own wording per fixture, so a fixture refused for some other reason
//   (missing context, a typo in it) fails instead of passing as "refused".
// - `input/valid/` is what the client's message builders produce for the
//   event the fixture describes (`INPUT_PRODUCERS`). Some are wire forms the
//   client never sends as written (an empty last paste part, a short part
//   that is not the last, a Shift+wheel step, an input method's key code);
//   their entries say so and compare what the client does build.
// - `input/tolerated/` and `input/invalid/` are the supervisor's to accept or
//   refuse, since the client decodes no input. What the client owes them is
//   never to build them: `INPUT_TOLERATED` checks that the client sends the
//   canonical form instead, and `INPUT_INVALID` checks the bounds the client
//   copies by hand (64-byte key fields, 16 KiB text, 32 KiB paste parts)
//   against the fixtures one past them, and states for every other file why
//   the client cannot build it.
//
// Every table must name exactly the files in its directory, both ways: a
// fixture added to `farhelm-proto` fails here until someone decides what the
// client does with it, and a removed one cannot leave a stale entry.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { isDeepStrictEqual } = require("node:util");
const grid = require("../assets/grid-model.js");
const input = require("../assets/grid-input.js");

const FIXTURES = path.join(__dirname, "../../farhelm-proto/grid-fixtures");

/** The fixture file names in one category directory (`"output/valid"`), sorted. */
function namesIn(category) {
  return fs
    .readdirSync(path.join(FIXTURES, category), { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .sort();
}

/** A fixture's raw bytes, as the client receives a data frame. */
function bytesOf(category, name) {
  return fs.readFileSync(path.join(FIXTURES, category, name));
}

function jsonOf(category, name) {
  return JSON.parse(bytesOf(category, name).toString("utf8"));
}

/** That `table` (a Map keyed by file name) names exactly the files in `category`. */
function assertNamesExactly(table, category, tableName) {
  assert.deepEqual([...table.keys()].sort(), namesIn(category), `${tableName} names exactly the files in ${category}/`);
}

// ## Applying one output fixture

const PLAIN = [0, -1, -1];
const HELLO = { t: "hello", cols: 80, rows: 24, history_cap: 100000 };
const STATE = {
  t: "state",
  mouse: "off",
  focus: false,
  alt_screen: false,
  default_fg: null,
  default_bg: null,
  history_trimmed: false,
  exit: null,
};

/**
 * Screen row `y` under a frame fixture: one cell whose foreground is a
 * direct color unique to `y`, so where a row sits after the fixture's shift
 * says which row it was.
 */
function seededRow(y) {
  return { w: false, r: [[0, 1, "x", [0, grid.DIRECT_COLOR_BASE + y, -1]]] };
}

/** The history a `history_reset` fixture has to clear: two lines from line 5. */
const SEEDED_HISTORY = { t: "history", first: 5, cols: 80, rows: [{ w: false, r: [[0, 3, "old", PLAIN]] }, { w: false, r: [] }] };

/** A frame that commits buffered rows anywhere on the largest screen. */
const COMMIT = { t: "frame", rev: 1, cols: 1000, rows: 1000, cursor: { x: 0, y: 0, visible: true, shape: "block", blink: true } };

/**
 * A committed screen of `cols` x `rows` holding `seededRow`s, at revision 0
 * (below every fixture frame's, which `prepare` asserts).
 */
function seededScreen(cols, rows) {
  return [
    { t: "rows", rows: Array.from({ length: rows }, (_, y) => [y, seededRow(y)]) },
    { t: "frame", rev: 0, cols, rows, cursor: { x: 0, y: 0, visible: false, shape: "block", blink: false } },
  ];
}

/**
 * The state a `state` fixture is applied over: every known field differs
 * from the fixture's, so a client that discarded the fixture (or one of its
 * fields) keeps a value the comparison catches.
 */
function oppositeState(message) {
  const modes = ["off", "click", "drag", "motion"];
  return {
    t: "state",
    mouse: modes[(modes.indexOf(message.mouse) + 1) % modes.length],
    focus: !message.focus,
    alt_screen: !message.alt_screen,
    default_fg: message.default_fg === null ? grid.DIRECT_COLOR_BASE + 1 : null,
    default_bg: message.default_bg === null ? grid.DIRECT_COLOR_BASE + 2 : null,
    history_trimmed: !message.history_trimmed,
    exit: message.exit === null ? { code: 3 } : null,
  };
}

/** A `state` message's fields as the model keeps them. */
function modelState(message) {
  return {
    mouse: message.mouse,
    focus: message.focus,
    altScreen: message.alt_screen,
    defaultFg: message.default_fg,
    defaultBg: message.default_bg,
    historyTrimmed: message.history_trimmed,
    exit: message.exit,
  };
}

/**
 * The messages applied before a fixture. The format's order comes first:
 * nothing before a `hello`, a `hello` before a `state`, and both before
 * anything else (the attach sequence). The prelude's size never limits a
 * fixture (rows are checked against the largest grid until a frame commits
 * them, and frames and history carry their own size), and its history cap is
 * the largest, so no fixture's history is dropped.
 *
 * With `observe`, the prelude also builds the state a fixture's effect needs
 * to be seen: a frame gets a committed screen of its own size holding
 * `seededRow`s, a `rows` update gets one at `COMMIT`'s size (so committing it
 * resizes nothing), a state gets `oppositeState`, and a history reset gets
 * `SEEDED_HISTORY` to clear. Refusal checks use the plain prelude, so a
 * refusal can only come from the fixture itself.
 */
function preludeFor(message, observe) {
  if (message.t === "hello") return [];
  if (message.t === "state") return observe ? [HELLO, oppositeState(message)] : [HELLO];
  const prelude = [HELLO, STATE];
  if (!observe) return prelude;
  if (message.t === "frame") prelude.push(...seededScreen(message.cols, message.rows));
  if (message.t === "rows") prelude.push(...seededScreen(COMMIT.cols, COMMIT.rows));
  if (message.t === "history_reset") prelude.push(SEEDED_HISTORY);
  return prelude;
}

/**
 * A fresh model with `message`'s prelude applied (`preludeFor`), the
 * observing prelude's premises asserted, and the prelude's damage taken,
 * ready for `message`. Premise failures throw from here, outside any check
 * of the fixture itself, so they read as what they are.
 */
function prepare(message, observe) {
  const model = new grid.GridModel();
  for (const before of preludeFor(message, observe)) model.apply(before);
  if (observe && (message.t === "frame" || message.t === "rows")) {
    if (message.t === "frame") assert.ok(message.rev > 0, "premise: the fixture's revision is above the seeded screen's 0");
    const seeded = Array.from({ length: model.rows }, (_, y) => fixtureRow(seededRow(y)));
    assert.deepEqual(model.screen.map(modelRow), seeded, "premise: the screen holds the seeded rows");
  }
  if (observe && message.t === "state") {
    const fixtureState = modelState(message);
    for (const [field, value] of Object.entries(model.state)) {
      assert.notDeepEqual(value, fixtureState[field], `premise: the state before differs in ${field}`);
    }
  }
  if (observe && message.t === "history_reset") {
    assert.deepEqual([model.historyFirst, model.historyLength], [5, 2], "premise: there is history to clear");
  }
  model.takeDamage();
  return model;
}

/** Decode a fixture's bytes and apply it after the plain prelude, as a refusal check does. */
function applyPlain(bytes) {
  const message = grid.decodeFrame(bytes);
  prepare(message, false).apply(message);
}

/**
 * Decode a fixture's bytes and apply it after the observing prelude,
 * asserting that both steps succeed. Returns the model.
 */
function acceptFixture(category, name) {
  let message;
  assert.doesNotThrow(() => (message = grid.decodeFrame(bytesOf(category, name))), `${name}: decodes`);
  const model = prepare(message, true);
  assert.doesNotThrow(() => model.apply(message), `${name}: is accepted`);
  return model;
}

// ## Comparing what the model holds with what the fixture says

/**
 * A run as `[col, width, text, [flags, fg, bg, ul], link]`, with an absent
 * underline color as -1 and the link resolved to `[id, uri]` (or null),
 * which is what both a fixture's run and the model's decoded run say once
 * the encoding's optional parts are filled in.
 */
function fixtureRun(run, links) {
  const [col, width, text, style] = run;
  const ul = style.length === 4 ? style[3] : -1;
  const link = run.length === 5 ? links[run[4]] : null;
  return [col, width, text, [style[0], style[1], style[2], ul], link];
}

function modelRun(run) {
  const { flags, fg, bg, ul } = run.style;
  return [run.col, run.width, run.text, [flags, fg, bg, ul], run.link ? [run.link.id, run.link.uri] : null];
}

function fixtureRow(row, links) {
  return { wrapped: row.w, runs: row.r.map((run) => fixtureRun(run, links || [])) };
}

function modelRow(row) {
  return { wrapped: row.wrapped, runs: row.runs.map(modelRun) };
}

const BLANK = { wrapped: false, runs: [] };


/**
 * Check that a fixture in its canonical form (a valid fixture as it is, a
 * tolerated one through `TOLERATED_CANONICAL`), applied with the observing
 * prelude, is what the model holds. One case per message type; an unknown
 * type fails, so a new kind of message cannot pass here unread. Some cases
 * apply a further message to show what the fixture left behind.
 */
function assertModelHolds(model, fixture, name) {
  switch (fixture.t) {
    case "hello":
      assert.deepEqual([model.cols, model.rows, model.historyCap], [fixture.cols, fixture.rows, fixture.history_cap], name);
      return;
    case "state":
      assert.deepEqual(model.state, modelState(fixture), name);
      return;
    case "frame": {
      assert.deepEqual([model.rev, model.cols, model.rows], [fixture.rev, fixture.cols, fixture.rows], name);
      assert.deepEqual(model.cursor, fixture.cursor, `${name}: the cursor is kept as sent`);
      // The seeded screen moves up by the shift (absent means 0), and the
      // rows it uncovers at the bottom are blank.
      const shift = fixture.shift === undefined ? 0 : fixture.shift;
      const expected = Array.from({ length: fixture.rows }, (_, y) => y + shift < fixture.rows ? fixtureRow(seededRow(y + shift)) : BLANK);
      assert.deepEqual(model.screen.map(modelRow), expected, `${name}: the screen after a shift of ${shift}`);
      return;
    }
    case "rows": {
      // Rows are invisible until a frame commits them. Over the seeded
      // screen, a named row is replaced (a blank one clears it) and every
      // other row keeps its seeded content.
      model.apply(COMMIT);
      const sent = new Map(fixture.rows.map(([y, row]) => [y, fixtureRow(row, fixture.links)]));
      for (const [y, row] of sent) assert.deepEqual(modelRow(model.screen[y]), row, `${name}: screen row ${y}`);
      const changed = model.screen.flatMap((row, y) => !sent.has(y) && !isDeepStrictEqual(modelRow(row), fixtureRow(seededRow(y))) ? [y] : []);
      assert.deepEqual(changed, [], `${name}: rows the message did not name keep their content`);
      return;
    }
    case "history": {
      const lines = [];
      for (let i = 0; i < model.historyLength; i++) {
        const line = model.historyLine(i);
        lines.push({ ...modelRow(line), cols: line.cols });
      }
      const expected = fixture.rows.map((row) => ({ ...fixtureRow(row, fixture.links), cols: fixture.cols }));
      assert.deepEqual(lines, expected, `${name}: history lines`);
      if (fixture.rows.length > 0) assert.equal(model.historyFirst, fixture.first, `${name}: first line number`);
      assert.equal(model.screenBase, fixture.first + fixture.rows.length, `${name}: the screen follows the history`);
      return;
    }
    case "history_reset":
      // The seeded history is gone, anything anchored to its line numbers
      // is told so, and history starts again at a line number that does not
      // continue the old one.
      assert.equal(model.takeDamage().historyReset, true, `${name}: the reset is reported as one`);
      assert.equal(model.historyLength, 0, `${name}: the seeded history is cleared`);
      model.apply({ t: "history", first: 100, cols: 80, rows: [{ w: false, r: [[0, 3, "new", PLAIN]] }] });
      assert.deepEqual([model.historyFirst, model.historyLength], [100, 1], `${name}: history starts again anywhere`);
      return;
    case "clipboard":
      // A part that is not the last writes nothing yet; an empty last part
      // of the same target completes it.
      if (!fixture.last) {
        assert.deepEqual(model.takeClipboard(), [], `${name}: nothing is written before the last part`);
        model.apply({ t: "clipboard", target: fixture.target, text: "", last: true });
      }
      assert.deepEqual(model.takeClipboard(), [{ target: fixture.target, text: fixture.text }], `${name}: the write`);
      return;
    default:
      assert.fail(`${name}: no check for message type ${fixture.t}`);
  }
}

// ## What each tolerated output fixture means

/** `object` without `key`. */
function without(object, key) {
  const { [key]: _dropped, ...rest } = object;
  return rest;
}

// The style flag bits the format defines (1 bold through 2048 blink); the
// client drops the rest rather than refusing them, like unknown fields.
const KNOWN_FLAGS = 0xfff;

/**
 * For each tolerated output fixture, the valid message it means, which
 * `assertModelHolds` then holds the model to: unknown fields dropped, an
 * empty `links` the same as none, reserved style bits cleared.
 */
const TOLERATED_CANONICAL = new Map([
  // An explicit zero shift is no shift: the revision and cursor still apply.
  ["frame-zero-shift.json", (f) => ({ ...f, cursor: without(f.cursor, "future") })],
  ["hello-unknown-field.json", (f) => without(f, "future")],
  ["row-unknown-field.json", (f) => ({ ...f, rows: f.rows.map(([y, row]) => [y, without(row, "future")]) })],
  ["rows-empty-links.json", (f) => without(f, "links")],
  [
    "rows-reserved-style-bits.json",
    (f) => {
      const flags = f.rows[0][1].r[0][3][0];
      assert.notEqual(flags & ~KNOWN_FLAGS, 0, "premise: the fixture sets reserved style bits");
      const canonical = {
        ...f,
        rows: f.rows.map(([y, row]) => [y, { ...row, r: row.r.map(([col, width, text, style, ...link]) => [col, width, text, [style[0] & KNOWN_FLAGS, ...style.slice(1)], ...link]) }]),
      };
      assert.equal(canonical.rows[0][1].r[0][3][0], 1, "premise: the known bit under the reserved ones is bold");
      return canonical;
    },
  ],
  ["state-unknown-field.json", (f) => without(f, "title")],
]);

// ## The refusals the client gives each invalid output fixture

/**
 * The client's own wording for each invalid fixture's refusal. The Rust
 * tests pin their wording separately (`INVALID_REASONS` in `grid_tests.rs`);
 * the two need not match, only both refuse for the rule the fixture names.
 */
const INVALID_REASONS = new Map([
  ["clipboard-missing-last.json", "bad clipboard part"],
  ["clipboard-unknown-target.json", "bad clipboard target"],
  ["frame-cursor-as-array.json", "bad cursor"],
  ["frame-cursor-past-cols.json", "cursor off the screen"],
  ["frame-cursor-past-rows.json", "cursor off the screen"],
  ["frame-rev-past-safe-integer.json", "bad frame revision"],
  ["frame-shift-past-rows.json", "bad frame shift"],
  ["frame-unknown-cursor-shape.json", "bad cursor shape"],
  ["frame-zero-rows.json", "bad frame size"],
  ["hello-history-cap-past-largest.json", "bad history cap"],
  ["hello-missing-history-cap.json", "bad history cap"],
  ["hello-rows-past-largest.json", "bad hello size"],
  ["hello-zero-cols.json", "bad hello size"],
  ["history-cols-past-largest.json", "bad history width"],
  ["history-empty-past-safe-integer.json", "bad history line number"],
  ["history-missing-cols.json", "bad history width"],
  ["history-past-safe-integer.json", "history line numbers past the largest safe integer"],
  ["history-run-overlap.json", "run column out of order or past the row"],
  ["history-run-past-cols.json", "run width past the row"],
  ["history-zero-cols.json", "bad history width"],
  ["missing-type.json", "message has no type"],
  ["not-an-object.json", "message has no type"],
  ["rows-ascii-width-mismatch.json", "an ASCII run's width is not its length"],
  ["rows-color-below-default.json", "bad background color"],
  ["rows-color-between-palette-and-direct.json", "bad foreground color"],
  ["rows-color-past-direct.json", "bad foreground color"],
  ["rows-empty-text.json", "empty run text"],
  ["rows-link-id-too-long.json", "bad link id"],
  ["rows-link-out-of-range.json", "run link out of range"],
  ["rows-link-uri-too-long.json", "bad link target"],
  ["rows-link-without-links.json", "run link out of range"],
  ["rows-non-ascii-run-wider-than-a-cell.json", "a non-ASCII run holds more than one cell"],
  ["rows-out-of-order.json", "run column out of order or past the row"],
  ["rows-overlap.json", "run column out of order or past the row"],
  ["rows-row-as-array.json", "bad row"],
  ["rows-row-missing-wrap.json", "bad row wrap flag"],
  ["rows-run-past-largest.json", "run width past the row"],
  ["rows-run-too-long.json", "bad run"],
  ["rows-run-too-short.json", "bad run"],
  ["rows-style-too-long.json", "bad style"],
  ["rows-style-too-short.json", "bad style"],
  ["rows-text-past-cell-budget.json", "bad run text"],
  ["rows-too-many-links.json", "bad links table"],
  ["rows-y-past-largest.json", "row index out of range"],
  ["rows-zero-width.json", "bad run width"],
  ["state-default-color-as-default-bg.json", "bad default background"],
  ["state-missing-default-fg.json", "bad default foreground"],
  ["state-missing-exit.json", "bad exit"],
  ["state-palette-default-fg.json", "bad default foreground"],
  ["state-unknown-exit.json", "bad exit"],
  ["state-unknown-mouse-mode.json", "bad mouse mode"],
  ["unknown-type.json", "unknown message type"],
]);
// ## The input fixtures and the events that produce them

const LINUX = "Linux x86_64";
const MAC = "MacIntel";

function keyEvent(fields) {
  return { key: "", code: "", keyCode: 0, shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, ...fields };
}

function pointerEvent(fields) {
  return { button: 0, buttons: 0, shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, ...fields };
}

// A grid at the format's largest size with 8x16-pixel cells, for turning a
// pointer position into the cell a mouse fixture names.
const CELL = { width: 8, height: 16, cols: 1000, rows: 1000 };

function cellAt(offsetX, offsetY) {
  return input.mouseCell(offsetX, offsetY, CELL.width, CELL.height, CELL.cols, CELL.rows);
}

/**
 * For each valid input fixture, the message the client builds for the event
 * it describes. Each producer goes through the builder the component calls
 * (keyMessage, textMessages, pasteMessages, mouseReport with mouseCell and
 * the button helpers, focusReport). A few fixtures are messages the client
 * never sends exactly as written (an empty last paste part, a short part
 * that is not the last, a wheel step with Shift held, a key an input method
 * owns); their comments say why, and their producers check what the client
 * does send.
 */
const INPUT_PRODUCERS = new Map([
  ["focus-in.json", () => input.focusReport(true)],
  ["focus-out.json", () => input.focusReport(false)],
  // Every modifier on a non-Mac viewer, so no `mac` field.
  [
    "key-all-modifiers.json",
    () => {
      const ev = keyEvent({ key: "Enter", code: "Enter", keyCode: 13, shiftKey: true, altKey: true, ctrlKey: true, metaKey: true });
      return input.keyMessage(ev, LINUX);
    },
  ],
  // The longest `key` and `code` the format allows (64 bytes) still make a
  // message; one byte more makes none (`INPUT_INVALID`). Key code 229 is an
  // input method's, which `keydownAction` never sends; the builder alone is
  // compared.
  [
    "key-longest.json",
    () => input.keyMessage(keyEvent({ key: "k".repeat(64), code: "c".repeat(64), keyCode: 229, shiftKey: true }), LINUX),
  ],
  // The Mac-only encoding the review found: Ctrl+Option+Shift+- on a Mac.
  // The client sends it (Option is not a third-level shift with Ctrl held)
  // and must say the viewer is a Mac, or the supervisor encodes it as
  // Alt-as-Meta (ESC _) where a Mac sends US.
  [
    "key-mac.json",
    () => {
      const ev = keyEvent({ key: "_", code: "Minus", keyCode: 189, shiftKey: true, altKey: true, ctrlKey: true });
      assert.equal(input.keydownAction(ev, MAC, false).action, "send", "the client sends this chord as a key");
      return input.keyMessage(ev, MAC);
    },
  ],
  ["key.json", () => input.keyMessage(keyEvent({ key: "ArrowUp", code: "ArrowUp", keyCode: 38 }), LINUX)],
  [
    "mouse-down.json",
    () => input.mouseReport("down", input.pressButton(0), cellAt(84, 88), pointerEvent({ button: 0, buttons: 1 })),
  ],
  ["mouse-move-no-button.json", () => input.mouseReport("move", input.moveButton(0), cellAt(0, 0), pointerEvent({}))],
  // Far past the grid's edge, which the grid's rounding clamps to the last cell.
  [
    "mouse-up-right.json",
    () => input.mouseReport("up", input.pressButton(2), cellAt(1e6, 1e6), pointerEvent({ button: 2, ctrlKey: true })),
  ],
  // A wheel step down (a positive `deltaY`). The fixture's Shift is a shape
  // the client never sends from a real wheel (Shift+wheel is horizontal
  // and `createWheelAccumulator` scrolls nothing for it), but the
  // message the builders make for it is still the format's.
  [
    "mouse-wheel-down.json",
    () => input.mouseReport("wheel", input.wheelButton({ deltaY: 120 }), cellAt(30, 70), pointerEvent({ shiftKey: true })),
  ],
  // The client never sends an empty last part: it knows where a paste ends,
  // so `last` rides on the final part with text, and an empty paste sends
  // nothing. So only the shape is compared: the builder's real last part
  // (its `t`, `last` and no other field), with the fixture's empty text.
  [
    "paste-last-empty.json",
    () => {
      assert.deepEqual(input.pasteMessages(""), [], "an empty paste sends nothing");
      return { ...input.pasteMessages("x")[0], data: "" };
    },
  ],
  // A part that is not the last: the client makes one only for a paste over
  // 32 KiB, so the fixture's text is followed by enough to need a second
  // part, and the first part is compared from its start.
  [
    "paste.json",
    () => {
      const data = "line one\nline two\r\n";
      const [first] = input.pasteMessages(data + "x".repeat(40 * 1024));
      assert.equal(first.data.startsWith(data), true, "the text is sent as written (line endings are the supervisor's)");
      return { ...first, data };
    },
  ],
  ["text.json", () => {
    const messages = input.textMessages('héllo 漢字 "quoted"\t');
    assert.equal(messages.length, 1, "short text is one message");
    return messages[0];
  }],
]);

/**
 * For each tolerated input fixture, a check that the client sends the
 * canonical form of its event instead: the supervisor accepts these, but the
 * client has no reason to write them.
 */
const INPUT_TOLERATED = new Map([
  // A `mac` written out as false: the client leaves it out off a Mac, and
  // writes it as true on one.
  [
    "key-mac-false.json",
    (f) => {
      assert.equal(f.mac, false, "premise: the tolerated form writes mac out as false");
      const ev = keyEvent({ key: f.key, code: f.code, keyCode: f.kc });
      assert.deepEqual(input.keyMessage(ev, LINUX), without(f, "mac"));
      assert.deepEqual(input.keyMessage(ev, MAC), { ...without(f, "mac"), mac: true });
    },
  ],
  // A field the format does not define: the client's key message for the
  // same event has exactly the format's fields.
  [
    "key-unknown-field.json",
    (f) => assert.deepEqual(input.keyMessage(keyEvent({ key: f.key, code: f.code, keyCode: f.kc }), LINUX), without(f, "repeat")),
  ],
]);

/**
 * The fixture's text split by `split` (textMessages or pasteMessages): the
 * fixture is exactly one byte past `bound`, the client's own copy of the
 * limit (the Rust tests pin the same fixture to the format's constant), so
 * the client must send it as several messages, each within the bound,
 * losing nothing.
 */
function assertSplitWithinBound(split, bound, f) {
  assert.equal(Buffer.byteLength(f.data, "utf8"), bound + 1, "premise: the fixture is one byte past the client's limit");
  const messages = split(f.data);
  assert.ok(messages.length > 1, "text one byte past the bound is split");
  assert.equal(messages.map((m) => m.data).join(""), f.data, "nothing is dropped");
  for (const m of messages) assert.ok(Buffer.byteLength(m.data, "utf8") <= bound, `a part of ${Buffer.byteLength(m.data)} bytes`);
}

/**
 * The fixture's key, which has a field exactly one byte past the client's
 * own copy of the 64-byte limit, makes no message.
 */
function assertNoKeyMessage(f) {
  const longest = Math.max(Buffer.byteLength(f.key), Buffer.byteLength(f.code));
  assert.equal(longest, input.MAX_KEY_FIELD_BYTES + 1, "premise: a field one byte past the client's limit");
  assert.equal(input.keyMessage(keyEvent({ key: f.key, code: f.code, keyCode: f.kc }), LINUX), null);
}

/**
 * For each invalid input fixture, why the client never sends it: `reason`
 * says so, and `check`, where one is cheap, shows it. The size bounds the
 * client copies from the format by hand are checked against the fixtures
 * one past them, so a bound changed on one side only fails here.
 */
const INPUT_INVALID = new Map([
  ["code-too-long.json", { reason: "keyMessage makes no message for a field past 64 bytes", check: assertNoKeyMessage }],
  ["focus-missing-in.json", { reason: "focusReport always writes `in`", check: () => assert.deepEqual(input.focusReport(undefined), { t: "focus", in: false }) }],
  ["key-mac-not-bool.json", { reason: "withMacFlag only ever adds `mac: true` (INPUT_TOLERATED shows it)" }],
  ["key-too-long.json", { reason: "keyMessage makes no message for a field past 64 bytes", check: assertNoKeyMessage }],
  [
    "key-undefined-modifier.json",
    {
      reason: "modifiers() sets only Shift, Alt, Ctrl and Meta (1 to 8)",
      check: () => assert.equal(input.modifiers({ shiftKey: true, altKey: true, ctrlKey: true, metaKey: true }), 15),
    },
  ],
  ["mouse-down-no-button.json", { reason: "a press goes out only with pressButton's 0, 1 or 2; -1 is moveButton's, for moves", check: () => assert.equal(input.pressButton(-1), null) }],
  ["mouse-unknown-kind.json", { reason: "the component reports only down, up, move and wheel" }],
  [
    "mouse-wheel-button-2.json",
    { reason: "wheelButton gives 0 or 1", check: () => assert.deepEqual([input.wheelButton({ deltaY: -3 }), input.wheelButton({ deltaY: 3 })], [0, 1]) },
  ],
  ["mouse-x-past-largest.json", { reason: "mouseCell clamps to the grid, at most 1000 columns", check: () => assert.equal(cellAt(1e9, 0).x, 999) }],
  ["paste-missing-last.json", { reason: "pasteMessages writes `last` on every part", check: () => assert.equal(typeof input.pasteMessages("a")[0].last, "boolean") }],
  ["paste-too-long.json", { reason: "pasteMessages splits at 32 KiB", check: (f) => assertSplitWithinBound(input.pasteMessages, input.MAX_PASTE_BYTES, f) }],
  ["text-too-long.json", { reason: "textMessages splits at 16 KiB", check: (f) => assertSplitWithinBound(input.textMessages, input.MAX_TEXT_BYTES, f) }],
  ["unknown-type.json", { reason: "a resize goes out as the attachment's control message, never as grid input" }],
]);

// ## The tests

test("the client knows every category of golden fixture", () => {
  // Why: the per-category rules below are the whole contract; a new kind
  // of fixture (another directory) would otherwise be read by nobody here.
  // Spec: grid-fixtures/ holds exactly `input` and `output`, each with
  // exactly `invalid`, `tolerated` and `valid`.
  const dirs = (where) => fs.readdirSync(where, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name).sort();
  assert.deepEqual(dirs(FIXTURES), ["input", "output"]);
  for (const side of ["input", "output"]) {
    assert.deepEqual(dirs(path.join(FIXTURES, side)), ["invalid", "tolerated", "valid"], side);
  }
});

test("every valid output fixture decodes to exactly what it says", () => {
  // Why: these are the messages the supervisor sends, written as the
  // format describes them; the Rust side proves its sender produces them,
  // and this proves the browser accepts them and keeps their content (a
  // cursor kept where it was sent, a shift moving the screen, a link
  // resolved, a style's colors, a history line's width, a reset clearing
  // history, a clipboard write's text and target). Spec: each file under
  // output/valid/ is accepted after the observing prelude, and the model
  // then holds the fixture's values (see `assertModelHolds`).
  const names = namesIn("output/valid");
  assert.ok(names.length > 0);
  for (const name of names) assertModelHolds(acceptFixture("output/valid", name), jsonOf("output/valid", name), name);
});

test("every tolerated output fixture is accepted and means its canonical form", () => {
  // Why: unknown fields and explicitly empty optional fields are how the
  // format grows within one protocol version; refusing one would turn an
  // additive change into a disconnect and a reconnect loop, and ignoring
  // the whole message instead of the extension would leave the screen
  // stale. Spec: each file under output/tolerated/ is accepted after the
  // observing prelude, and the model then holds what its canonical form in
  // `TOLERATED_CANONICAL` says; the table names exactly the files there.
  assertNamesExactly(TOLERATED_CANONICAL, "output/tolerated", "TOLERATED_CANONICAL");
  for (const name of namesIn("output/tolerated")) {
    const model = acceptFixture("output/tolerated", name);
    assertModelHolds(model, TOLERATED_CANONICAL.get(name)(jsonOf("output/tolerated", name)), name);
  }
});

test("every invalid output fixture is refused for the rule it is named after", () => {
  // Why: the supervisor may be another machine (`--ssh`) and a program in
  // the terminal controls much of what it sends, so the client must refuse
  // what the format forbids, and refuse the same things the Rust decoder
  // does. Checking the refusal's wording keeps a fixture that broke by
  // accident, or one refused only for lack of context, from passing. Spec:
  // each file under output/invalid/ throws `GridProtocolError` from
  // decoding or applying it after the plain prelude, with the message
  // `INVALID_REASONS` gives, and the table names exactly the files there.
  assertNamesExactly(INVALID_REASONS, "output/invalid", "INVALID_REASONS");
  for (const name of namesIn("output/invalid")) {
    assert.throws(
      () => applyPlain(bytesOf("output/invalid", name)),
      { name: "GridProtocolError", message: INVALID_REASONS.get(name) },
      name,
    );
  }
});

test("the client's input builders produce every valid input fixture", () => {
  // Why: the supervisor encodes keys and clicks from these messages alone,
  // so a field the client leaves out or adds changes what a program reads
  // (a Mac viewer's key without `mac: true` is encoded as a Linux one). The
  // fixtures are what `farhelm-proto` encodes for each event, so they are
  // what the client must send. Spec: for each file under input/valid/, the
  // builder for the event it describes returns a message deep-equal to the
  // file, and `INPUT_PRODUCERS` names exactly the files there.
  assertNamesExactly(INPUT_PRODUCERS, "input/valid", "INPUT_PRODUCERS");
  for (const name of namesIn("input/valid")) {
    assert.deepEqual(INPUT_PRODUCERS.get(name)(), jsonOf("input/valid", name), name);
  }
});

test("the client sends the canonical form of every tolerated input fixture", () => {
  // Why: a tolerated form is accepted, not intended; a client that drifted
  // into writing one (a `mac: false`, an extra field) would still work but
  // would no longer send what `farhelm-proto` encodes. Spec: for each file
  // under input/tolerated/, the check in `INPUT_TOLERATED` holds, and the
  // table names exactly the files there.
  assertNamesExactly(INPUT_TOLERATED, "input/tolerated", "INPUT_TOLERATED");
  for (const name of namesIn("input/tolerated")) INPUT_TOLERATED.get(name)(jsonOf("input/tolerated", name));
});

test("the client never builds an invalid input fixture", () => {
  // Why: the supervisor ends the attachment on input it refuses, and the
  // client copies the format's input bounds by hand; if one side's bound
  // moves, an ordinary long paste or typed text ends the session. Spec:
  // `INPUT_INVALID` names exactly the files under input/invalid/, gives each
  // a reason the client cannot build it, and every check there holds: a key
  // field past 64 bytes makes no message, and text and pastes one byte past
  // their bounds are split into parts within them.
  assertNamesExactly(INPUT_INVALID, "input/invalid", "INPUT_INVALID");
  for (const name of namesIn("input/invalid")) {
    const entry = INPUT_INVALID.get(name);
    assert.ok(entry.reason, `${name} has a reason`);
    if (entry.check) entry.check(jsonOf("input/invalid", name));
  }
});
