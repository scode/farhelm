// Static enforcement of SPEC_impl.md's looping-animation rule ("GUI:
// Dioxus", looping animations) over app.css: every animation that repeats
// indefinitely uses a stepped timing function, and changes what is drawn
// at most 10 times per second.
//
// Why this exists: the running-status pulse used to be a
// `2s ease-in-out infinite` opacity animation. An interpolating timing
// function produces a new frame on every display refresh for as long as it
// runs, and on a 120 Hz MacBook that one dot was observed to hold the
// window server at roughly 40-50% CPU, even with the Farhelm window
// hidden, because the compositor kept redrawing the window at the display's
// full rate. Nothing on screen looks wrong when that happens, so neither
// review of a screenshot nor any other suite would notice the next
// `linear infinite` someone adds. This check reads the stylesheet text and
// fails on it.
//
// What it asserts, for every `animation` shorthand outside `@keyframes`
// whose iteration count is `infinite`:
//
// - its timing function is `steps(n[, position])`, `step-start` or
//   `step-end` (an omitted timing function is CSS's default `ease`, and
//   fails like any other interpolating one);
// - its change rate is at most MAX_CHANGES_PER_SECOND. CSS applies the
//   timing function to EACH interval between adjacent keyframes, not to
//   the cycle as a whole, so the changes per cycle are the step count times
//   the number of keyframe intervals of the `@keyframes` it names (the
//   implicit 0% and 100% count). `steps(4)` over a 0%/50%/100% pulse is
//   eight changes per cycle, not four.
//
// It also refuses what it cannot read, rather than passing it: animation
// longhands outside `@keyframes`, a per-keyframe `animation-timing-function`,
// a `@keyframes` whose frames do not all set the same properties (CSS builds
// the intervals per property, so the shared count above would undercount),
// and a shorthand naming a `@keyframes` this file does not define. The
// `-webkit-` prefixed spellings are read like the unprefixed ones.
//
// What it deliberately does not do: check the window-inactive pause (one
// universal `animation-play-state` rule in app.css covers every animation,
// present and future, so there is nothing per-animation to check), detect
// long non-infinite animations (none exist), or read the vendored
// xterm.css, which is not ours to lint.
//
// Like app-css-tokens.test.js, the checker is a pure function over a CSS
// string so the negative cases can feed it small synthetic snippets. It is
// a small brace-matching scanner sized to app.css's actual shape, not a
// general CSS parser.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const APP_CSS_PATH = path.join(__dirname, "../assets/app.css");

// U4 of the plan that introduced this check: 10 visual changes per second
// for anything that repeats on its own.
const MAX_CHANGES_PER_SECOND = 10;

const INTERPOLATING_RE = /^(?:linear|ease|ease-in|ease-out|ease-in-out|cubic-bezier\(.*\)|linear\(.*\))$/;
const STEPS_RE = /^steps\(\s*(\d+)\s*(?:,\s*([a-z-]+)\s*)?\)$/;
const TIME_RE = /^(\d*\.?\d+)(s|ms)$/;
// Shorthand keywords that are not an animation name. Anything else that is
// not a time, a timing function or a number is taken as the name.
const NON_NAME_KEYWORDS = new Set([
  "infinite",
  "normal",
  "reverse",
  "alternate",
  "alternate-reverse",
  "none",
  "forwards",
  "backwards",
  "both",
  "running",
  "paused",
  "step-start",
  "step-end",
]);

/**
 * Parse CSS text into nested items: a declaration is a trimmed string, a
 * block is `{ prelude, items }`. Comments are dropped, and quoted strings
 * (backslash escapes included) are copied through whole, so neither a `;`
 * or brace inside `content: "..."` nor a `/*` inside a string can split or
 * hide a declaration.
 */
function parseCss(css) {
  const src = css;
  let i = 0;
  function block() {
    const items = [];
    let buf = "";
    while (i < src.length) {
      const c = src[i++];
      if (c === "/" && src[i] === "*") {
        const end = src.indexOf("*/", i + 1);
        i = end === -1 ? src.length : end + 2;
      } else if (c === '"' || c === "'") {
        buf += c;
        while (i < src.length) {
          const d = src[i++];
          buf += d;
          if (d === "\\" && i < src.length) buf += src[i++];
          else if (d === c) break;
        }
      } else if (c === "{") {
        const prelude = buf.trim();
        buf = "";
        items.push({ prelude, items: block() });
      } else if (c === "}") {
        break;
      } else if (c === ";") {
        if (buf.trim()) items.push(buf.trim());
        buf = "";
      } else {
        buf += c;
      }
    }
    if (buf.trim()) items.push(buf.trim());
    return items;
  }
  return block();
}

/** Split on `sep` at parenthesis depth zero: `steps(4, end) 2s` stays whole per token. */
function splitTopLevel(value, sep) {
  const parts = [];
  let depth = 0;
  let buf = "";
  for (const c of value) {
    if (c === "(") depth++;
    if (c === ")") depth--;
    if (depth === 0 && (sep === " " ? /\s/.test(c) : c === sep)) {
      if (buf.trim()) parts.push(buf.trim());
      buf = "";
    } else {
      buf += c;
    }
  }
  if (buf.trim()) parts.push(buf.trim());
  return parts;
}

/** Keyframe selector list (`0%, 100%`, `from`, `to`) to offsets in percent. */
function keyframeOffsets(prelude) {
  return prelude.split(",").map((s) => {
    const t = s.trim().toLowerCase();
    if (t === "from") return 0;
    if (t === "to") return 100;
    return parseFloat(t);
  });
}

/**
 * Changes per cycle for one step function across a keyframe set: the step
 * count per interval times the number of intervals. `jump-both` adds a
 * jump at each end of every interval. `jump-none` is counted like the
 * default rather than one lower: it drops a jump inside each interval, but
 * a cycle whose first and last values differ still jumps at the wrap, and
 * counting high by at most one per interval is the safe side of a cap.
 */
function changesPerCycle(steps, position, intervals) {
  const perInterval = position === "jump-both" ? steps + 1 : steps;
  return perInterval * intervals;
}

/**
 * Check every looping animation in `css` against the stepped-timing rule.
 *
 * Returns `{ violations, looping }`: `violations` is a list of
 * human-readable strings (empty means the stylesheet complies), and
 * `looping` lists each infinite animation that was checked as
 * `{ selector, name, changesPerSecond }`, so a caller can confirm the
 * scanner actually saw the animations it expects rather than passing
 * because it matched nothing.
 */
function checkLoopingAnimations(css) {
  const tree = parseCss(css);
  const keyframes = new Map();
  const uses = [];
  const violations = [];

  function walk(items) {
    for (const item of items) {
      if (typeof item === "string") continue;
      const prelude = item.prelude;
      const kf = prelude.match(/^@(?:-webkit-)?keyframes\s+([\w-]+)$/);
      if (kf) {
        const offsets = new Set([0, 100]);
        const propertySets = new Set();
        for (const frame of item.items) {
          if (typeof frame === "string") continue;
          for (const o of keyframeOffsets(frame.prelude)) offsets.add(o);
          const props = frame.items
            .filter((d) => typeof d === "string")
            .map((d) => d.split(":")[0].trim())
            .sort();
          propertySets.add(props.join(","));
          for (const decl of frame.items) {
            if (typeof decl === "string" && /^animation-timing-function\s*:/.test(decl)) {
              violations.push(
                `@keyframes ${kf[1]}: per-keyframe animation-timing-function is not checked; ` +
                  `put the timing function on the animation shorthand`,
              );
            }
          }
        }
        if (propertySets.size > 1) {
          violations.push(
            `@keyframes ${kf[1]}: frames set different properties, so the change rate cannot be counted; ` +
              `give every frame the same properties`,
          );
        }
        keyframes.set(kf[1], offsets.size - 1);
      } else if (prelude.startsWith("@")) {
        walk(item.items);
      } else {
        for (const decl of item.items) {
          if (typeof decl !== "string") continue;
          const m = decl.match(/^(?:-webkit-)?(animation(?:-[a-z-]+)?)\s*:\s*([\s\S]*)$/);
          if (!m) continue;
          if (m[1] === "animation") {
            uses.push({ selector: prelude, value: m[2].replace(/\s*!important$/, "") });
          } else if (m[1] !== "animation-play-state") {
            // Pausing is the one longhand that cannot make anything redraw
            // faster; the window-inactive rule relies on it.
            violations.push(
              `${prelude}: ${m[1]} longhand is not checked; use the animation shorthand`,
            );
          }
        }
        walk(item.items.filter((x) => typeof x !== "string"));
      }
    }
  }
  walk(tree);

  const looping = [];
  for (const { selector, value } of uses) {
    for (const single of splitTopLevel(value, ",")) {
      const tokens = splitTopLevel(single, " ");
      if (tokens.length === 1 && tokens[0] === "none") continue;
      if (!tokens.includes("infinite")) continue;

      let timing = null;
      let duration = null;
      let name = null;
      for (const t of tokens) {
        const time = t.match(TIME_RE);
        if (time) {
          // The first time is the duration; a second one is the delay.
          if (duration === null) duration = parseFloat(time[1]) * (time[2] === "ms" ? 0.001 : 1);
        } else if (STEPS_RE.test(t) || t === "step-start" || t === "step-end" || INTERPOLATING_RE.test(t)) {
          timing = t;
        } else if (!NON_NAME_KEYWORDS.has(t) && !/^\d*\.?\d+$/.test(t)) {
          name = t;
        }
      }

      const where = `${selector}: animation: ${single}`;
      if (timing === null || INTERPOLATING_RE.test(timing)) {
        violations.push(`${where}: looping animation must use steps(), step-start or step-end`);
        continue;
      }
      if (!duration) {
        violations.push(`${where}: looping animation needs a non-zero duration`);
        continue;
      }
      if (!keyframes.has(name)) {
        violations.push(`${where}: names no @keyframes defined in this stylesheet`);
        continue;
      }
      let steps = 1;
      let position = "end";
      const sm = timing.match(STEPS_RE);
      if (sm) {
        steps = parseInt(sm[1], 10);
        position = sm[2] || "end";
      }
      const changesPerSecond = changesPerCycle(steps, position, keyframes.get(name)) / duration;
      looping.push({ selector, name, changesPerSecond });
      if (changesPerSecond > MAX_CHANGES_PER_SECOND) {
        violations.push(
          `${where}: ${changesPerSecond} changes per second exceeds ${MAX_CHANGES_PER_SECOND}`,
        );
      }
    }
  }
  return { violations, looping };
}

// ---------------------------------------------------------------------------
// The real stylesheet
// ---------------------------------------------------------------------------

/**
 * The shipped app.css complies, and the scanner saw each of the four
 * looping indicators the rule was written for. Without the second half a
 * scanner that silently matched nothing would also report zero violations.
 */
test("app.css: every looping animation is stepped and within the rate cap", () => {
  const { violations, looping } = checkLoopingAnimations(fs.readFileSync(APP_CSS_PATH, "utf8"));
  const selectors = looping.map((l) => l.selector);
  for (const expected of [
    ".status-badge.running .status-dot",
    ".host-update-dot",
    ".delete-spinner",
    ".delete-progress-dot",
  ]) {
    assert.ok(selectors.includes(expected), `expected a looping animation on ${expected}; saw ${selectors}`);
  }
  assert.deepEqual(violations, []);
});

/**
 * The agreed rates, pinned: the pulse at four changes per second and the
 * spinner at ten. These are the values the maintainer chose (8 steps per
 * 2 s, 8 steps per 0.8 s); a change to either is a design change, not a
 * refactor, and should be made on purpose.
 */
test("app.css: the pulse runs at 4 changes per second and the spinner at 10", () => {
  const { looping } = checkLoopingAnimations(fs.readFileSync(APP_CSS_PATH, "utf8"));
  const rate = (selector) => looping.find((l) => l.selector === selector).changesPerSecond;
  assert.equal(rate(".status-badge.running .status-dot"), 4);
  assert.equal(rate(".delete-spinner"), 10);
});

// ---------------------------------------------------------------------------
// Synthetic cases: each one proves the checker rejects (or accepts) a shape
// on its own, so a regression in the scanner cannot hide behind app.css
// happening to comply.
// ---------------------------------------------------------------------------

const PULSE = "@keyframes p { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }";
const SPIN = "@keyframes s { to { transform: rotate(360deg); } }";

/** The original bug's exact shape, plus the other interpolating spellings. */
test("synthetic: interpolating timing on an infinite animation is rejected", () => {
  for (const timing of ["ease-in-out", "linear", "ease", "cubic-bezier(0.4, 0, 0.2, 1)"]) {
    const { violations } = checkLoopingAnimations(`${PULSE} .a { animation: p 2s ${timing} infinite; }`);
    assert.equal(violations.length, 1, timing);
    assert.match(violations[0], /must use steps/);
  }
});

/** An omitted timing function is CSS's default `ease`, so it interpolates too. */
test("synthetic: a looping animation with no timing function is rejected", () => {
  const { violations } = checkLoopingAnimations(`${PULSE} .a { animation: p 2s infinite; }`);
  assert.equal(violations.length, 1);
  assert.match(violations[0], /must use steps/);
});

/**
 * The timing function applies per keyframe interval. A 0%/50%/100% pulse
 * with `steps(8)` over 1 s is 16 changes per second, over the cap, even
 * though 8 / 1 s alone would look fine; with `steps(4)` over 2 s it is 4.
 */
test("synthetic: the rate multiplies steps by keyframe intervals", () => {
  const fast = checkLoopingAnimations(`${PULSE} .a { animation: p 1s steps(8) infinite; }`);
  assert.equal(fast.violations.length, 1);
  assert.match(fast.violations[0], /16 changes per second/);

  const ok = checkLoopingAnimations(`${PULSE} .a { animation: p 2s steps(4) infinite; }`);
  assert.deepEqual(ok.violations, []);
  assert.equal(ok.looping[0].changesPerSecond, 4);
});

/** A single-interval animation over the cap, with the duration given in ms. */
test("synthetic: a stepped animation faster than 10 per second is rejected", () => {
  const { violations } = checkLoopingAnimations(`${SPIN} .a { animation: s 500ms steps(12) infinite; }`);
  assert.equal(violations.length, 1);
  assert.match(violations[0], /24 changes per second/);
});

/** `step-end` over explicit keyframes counts one change per interval. */
test("synthetic: step-end counts one change per keyframe interval", () => {
  const css = "@keyframes e { 0% {opacity:1} 25% {opacity:.8} 50% {opacity:.6} 75% {opacity:.8} }"
    + " .a { animation: e 1s step-end infinite; }";
  const { violations, looping } = checkLoopingAnimations(css);
  assert.deepEqual(violations, []);
  assert.equal(looping[0].changesPerSecond, 4);
});

/**
 * One-shot animations are allowed to interpolate (SPEC_impl.md permits
 * short animations triggered by a user action), and `none` is not an
 * animation at all.
 */
test("synthetic: non-looping and none animations are not checked", () => {
  const css = `${PULSE} .a { animation: p 300ms ease-out; } .b { animation: p 1s linear 2; } .c { animation: none; }`;
  const { violations, looping } = checkLoopingAnimations(css);
  assert.deepEqual(violations, []);
  assert.deepEqual(looping, []);
});

/** Each comma-separated animation in one shorthand is checked on its own. */
test("synthetic: every animation in a comma-separated list is checked", () => {
  const css = `${PULSE} ${SPIN} .a { animation: p 2s steps(4) infinite, s 1s linear infinite; }`;
  const { violations, looping } = checkLoopingAnimations(css);
  assert.equal(looping.length, 1);
  assert.equal(violations.length, 1);
  assert.match(violations[0], /s 1s linear infinite: looping animation must use steps/);
});

/** Rules inside `@media` are checked like top-level ones. */
test("synthetic: an animation inside @media is checked", () => {
  const css = `${PULSE} @media (min-width: 1px) { .a { animation: p 2s linear infinite; } }`;
  assert.equal(checkLoopingAnimations(css).violations.length, 1);
});

/**
 * Shapes the scanner cannot evaluate fail instead of passing: longhands,
 * a per-keyframe timing function, and an unknown keyframes name. A
 * `animation-play-state` longhand is the exception, since the
 * window-inactive pause depends on it and it cannot speed anything up.
 */
test("synthetic: unreadable shapes are rejected, play-state is allowed", () => {
  const longhand = checkLoopingAnimations(`${PULSE} .a { animation-iteration-count: infinite; }`);
  assert.match(longhand.violations[0], /longhand is not checked/);

  const perFrame = checkLoopingAnimations(
    "@keyframes k { 0% { opacity: 1; animation-timing-function: linear; } }"
      + " .a { animation: k 2s steps(2) infinite; }",
  );
  assert.match(perFrame.violations[0], /per-keyframe animation-timing-function/);

  const unknown = checkLoopingAnimations(".a { animation: missing 2s steps(2) infinite; }");
  assert.match(unknown.violations[0], /names no @keyframes/);

  const paused = checkLoopingAnimations(`:root[x] * { animation-play-state: paused !important; }`);
  assert.deepEqual(paused.violations, []);
});

/**
 * Quoted strings and comments cannot split a declaration or hide one: a
 * `;` or brace in a string, an escaped quote, and a `/*` inside a string
 * all leave the next rule visible, and a commented-out rule is ignored.
 */
test("synthetic: quotes and comments do not confuse the scanner", () => {
  const css = `${PULSE} .q::before { content: "a;b{c}"; } /* .x { animation: p 1s linear infinite; } */`
    + ` .e::before { content: "a\\"b;{"; } .s::before { content: "/*"; }`
    + " .a { animation: p 2s linear infinite; }";
  const { violations } = checkLoopingAnimations(css);
  assert.equal(violations.length, 1);
  assert.match(violations[0], /^\.a: /);
});

/** The `-webkit-` spellings are checked, not silently skipped. */
test("synthetic: a -webkit-animation shorthand is checked", () => {
  const css = `@-webkit-keyframes w { to { opacity: 0; } } .a { -webkit-animation: w 1s linear infinite; }`;
  const { violations } = checkLoopingAnimations(css);
  assert.equal(violations.length, 1);
  assert.match(violations[0], /must use steps/);
});

/**
 * Keyframes whose frames set different properties get their intervals
 * per property, which the shared interval count cannot see (two
 * properties with interleaved keyframes change more often than either
 * alone), so the checker refuses them.
 */
test("synthetic: keyframes with mixed property sets are rejected", () => {
  const css = "@keyframes m { 0% { opacity: 0; transform: none; } 30% { transform: rotate(10deg); }"
    + " 100% { opacity: 1; transform: none; } } .a { animation: m 1s steps(5) infinite; }";
  const { violations } = checkLoopingAnimations(css);
  assert.equal(violations.length, 1);
  assert.match(violations[0], /frames set different properties/);
});

/** `jump-none` is counted like the default, so the wrap jump is not missed. */
test("synthetic: jump-none is not counted below the default", () => {
  const css = `@keyframes j { from { opacity: 0; } to { opacity: 1; } } .a { animation: j 0.2s steps(2, jump-none) infinite; }`;
  const { looping } = checkLoopingAnimations(css);
  assert.equal(looping[0].changesPerSecond, 10);
});
