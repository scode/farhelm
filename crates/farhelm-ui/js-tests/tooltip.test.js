// Unit coverage for tooltip.js's pure decisions, run with node's built-in
// test runner (the same shape as the other files in this directory: `node
// --test` loads the exact file the page ships).
//
// Placement is the rule worth pinning here rather than in a browser: the
// maintainer chose ABOVE-first with a small gap and a much larger gap BELOW,
// because the cursor covers the space under the hot spot and never the space
// above it (see tooltip.js's module docs). A regression that flipped the
// preference, shrank the below gap, or let a tooltip run off a window edge
// would still "show a tooltip" in a browser test, so the browser suite only
// proves the wiring and these cases pin the geometry. The follow-on and
// scroll-relevance helpers are pinned for the same reason: their failure
// modes (a delay at every icon in a row; a tooltip that vanishes whenever
// the terminal scrolls under live output) are behaviors, not crashes.
const test = require("node:test");
const assert = require("node:assert/strict");
const { placeTooltip, instantFollowOn, scrollMovesTarget } = require("../assets/tooltip.js");

const VIEWPORT = { width: 1000, height: 800 };

/** A target rectangle from its left/top corner and size, as getBoundingClientRect reports one. */
function rect(left, top, width, height) {
  return { left, top, right: left + width, bottom: top + height, width, height };
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

test("a target with room above gets the tooltip centred 6 px above it", () => {
  const place = placeTooltip(rect(500, 400, 20, 20), { width: 100, height: 30 }, VIEWPORT);
  assert.deepEqual(place, { left: 460, top: 364, side: "above" });
});

test("a target near the top edge falls back below with the larger 28 px gap", () => {
  const place = placeTooltip(rect(500, 10, 20, 20), { width: 100, height: 30 }, VIEWPORT);
  assert.deepEqual(place, { left: 460, top: 58, side: "below" });
});

test("above is kept while it fits exactly at the 4 px edge margin", () => {
  // top - 6 - 30 = 4: the tooltip just fits, so the cursor-safe side wins.
  const place = placeTooltip(rect(500, 40, 20, 20), { width: 100, height: 30 }, VIEWPORT);
  assert.equal(place.side, "above");
  assert.equal(place.top, 4);
});

test("a target near the left edge shifts the tooltip right to stay inside", () => {
  const place = placeTooltip(rect(0, 400, 16, 16), { width: 200, height: 30 }, VIEWPORT);
  assert.equal(place.left, 4);
  assert.equal(place.side, "above");
});

test("a target near the right edge shifts the tooltip left to stay inside", () => {
  const place = placeTooltip(rect(990, 400, 10, 16), { width: 200, height: 30 }, VIEWPORT);
  assert.equal(place.left, 1000 - 4 - 200);
});

test("a tooltip wider than the window is pinned to the left edge, where its text starts", () => {
  const place = placeTooltip(rect(400, 400, 20, 20), { width: 1200, height: 30 }, VIEWPORT);
  assert.equal(place.left, 4);
});

test("a tooltip that fits on neither side takes the roomier side, clamped inside the window", () => {
  // 300 px tall in a 400 px window around a target in the lower half: there
  // is more room above, so it goes above and is clamped to the top margin,
  // overlapping the target rather than leaving the screen.
  const small = { width: 1000, height: 400 };
  const lower = placeTooltip(rect(500, 250, 20, 20), { width: 100, height: 300 }, small);
  assert.deepEqual(lower, { left: 460, top: 4, side: "above" });
  // The same tooltip around a target in the upper half goes below, clamped
  // so its bottom stays 4 px above the window's bottom edge.
  const upper = placeTooltip(rect(500, 100, 20, 20), { width: 100, height: 300 }, small);
  assert.deepEqual(upper, { left: 460, top: 96, side: "below" });
});

// ---------------------------------------------------------------------------
// Follow-on and scroll relevance
// ---------------------------------------------------------------------------

test("a new target shows at once while a tooltip is showing", () => {
  assert.equal(instantFollowOn(10_000, -Infinity, true), true);
});

test("a new target shows at once within the 300 ms grace after a hide, and not after it", () => {
  assert.equal(instantFollowOn(1_299, 1_000, false), true);
  assert.equal(instantFollowOn(1_300, 1_000, false), false);
  assert.equal(instantFollowOn(5_000, -Infinity, false), false);
});

test("a scroll of the document or of a container holding the target moves it", () => {
  const target = {};
  const container = { contains: (node) => node === target };
  const doc = { documentElement: {}, body: {} };
  assert.equal(scrollMovesTarget(doc, target, doc), true);
  assert.equal(scrollMovesTarget(doc.documentElement, target, doc), true);
  assert.equal(scrollMovesTarget(doc.body, target, doc), true);
  assert.equal(scrollMovesTarget(container, target, doc), true);
});

test("a scroll of an unrelated container (the terminal under live output) does not", () => {
  const target = {};
  const terminalViewport = { contains: () => false };
  const doc = { documentElement: {}, body: {} };
  assert.equal(scrollMovesTarget(terminalViewport, target, doc), false);
});
