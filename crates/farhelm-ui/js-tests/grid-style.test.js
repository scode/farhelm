// Unit coverage for grid-style.js, the mapping from a grid run's style to
// inline CSS.
//
// A new terminal must look like an old one, so these cases pin the grid's
// color rules, which are the old terminal's as Farhelm configures it: the
// theme's palette and the standard xterm 256-color table, bold drawing
// palette colors 0-7 in their bright twins, inverse swapping the defaults,
// dim at half opacity, and the underline variants. They also pin that the CSS
// is built only from fixed tables and validated integers, which is what keeps
// a program's output from reaching style text.
const test = require("node:test");
const assert = require("node:assert/strict");
const style = require("../assets/grid-style.js");
const model = require("../assets/grid-model.js");
const theme = require("../assets/terminal-theme.js");

const { FLAGS } = style;
const palette = style.buildPalette(theme);
const defaults = style.defaultColors(theme, null, null);

function resolve(raw, colors = defaults) {
  return style.resolveStyle(model.decodeStyle(raw), palette, colors);
}

test("the palette is the theme's 16 colors, the standard 256-color cube and its gray ramp", () => {
  assert.equal(palette.length, 256);
  assert.equal(palette[1], "#cc6666");
  assert.equal(palette[15], "#eaeaea");
  assert.equal(palette[16], "#000000");
  assert.equal(palette[21], "#0000ff");
  assert.equal(palette[196], "#ff0000");
  assert.equal(palette[231], "#ffffff");
  assert.equal(palette[232], "#080808");
  assert.equal(palette[255], "#eeeeee");
});

test("default colors come from the theme unless a program redefined them", () => {
  assert.deepEqual(defaults, { fg: "#ffffff", bg: "#282c34" });
  assert.deepEqual(style.defaultColors(theme, 0x1000000 + 0x102030, 0x1000000), { fg: "#102030", bg: "#000000" });
});

test("palette, direct and default colors resolve to concrete values", () => {
  assert.deepEqual(resolve([0, 2, 4]).fg, "#b5bd68");
  assert.deepEqual(resolve([0, 2, 4]).bg, "#81a2be");
  assert.equal(resolve([0, 0x1abcdef, 0x1000001]).fg, "#abcdef");
  assert.equal(resolve([0, 0x1abcdef, 0x1000001]).bg, "#000001");
  // The default foreground and background need no declaration at all: the
  // terminal's own colors show through.
  assert.equal(resolve([0, -1, -1]).fg, null);
  assert.equal(resolve([0, -1, -1]).bg, null);
});

test("bold draws palette colors 0-7 in their bright twins, as old terminals do", () => {
  assert.equal(resolve([FLAGS.BOLD, 1, -1]).fg, palette[9]);
  assert.equal(resolve([FLAGS.BOLD, 9, -1]).fg, palette[9]);
  assert.equal(resolve([FLAGS.BOLD, 100, -1]).fg, palette[100]);
  assert.equal(resolve([FLAGS.BOLD, 0x1000001, -1]).fg, "#000001");
  assert.equal(resolve([FLAGS.BOLD, 1, 1]).bg, palette[1], "backgrounds are not brightened");
  assert.equal(resolve([FLAGS.BOLD | FLAGS.UNDERLINE, -1, -1, 3]).ul, palette[11]);
});

test("inverse swaps the colors, and a swapped default becomes the other default", () => {
  assert.deepEqual(
    [resolve([FLAGS.INVERSE, -1, -1]).fg, resolve([FLAGS.INVERSE, -1, -1]).bg],
    ["#282c34", "#ffffff"],
  );
  assert.deepEqual([resolve([FLAGS.INVERSE, 2, 4]).fg, resolve([FLAGS.INVERSE, 2, 4]).bg], [palette[4], palette[2]]);
  // Bold brightening applies to whatever ends up as the foreground.
  assert.equal(resolve([FLAGS.INVERSE | FLAGS.BOLD, -1, 1]).fg, palette[9]);
});

test("dim is the foreground at half opacity, including the default and direct colors", () => {
  assert.equal(resolve([FLAGS.DIM, -1, -1]).fg, "rgba(255,255,255,0.5)");
  assert.equal(resolve([FLAGS.DIM, 1, -1]).fg, "rgba(204,102,102,0.5)");
  assert.equal(resolve([FLAGS.DIM, 0x1102030, -1]).fg, "rgba(16,32,48,0.5)");
});

test("underline variants, underline color and strikethrough become text decorations", () => {
  const css = (raw) => style.styleCss(resolve(raw));
  assert.equal(css([FLAGS.UNDERLINE, -1, -1]), "text-decoration-line:underline;");
  assert.equal(css([FLAGS.DOUBLE_UNDERLINE, -1, -1]), "text-decoration-line:underline;text-decoration-style:double;");
  assert.equal(css([FLAGS.UNDERCURL, -1, -1, 1]), `text-decoration-line:underline;text-decoration-style:wavy;text-decoration-color:${palette[1]};`);
  assert.equal(css([FLAGS.DOTTED_UNDERLINE, -1, -1]), "text-decoration-line:underline;text-decoration-style:dotted;");
  assert.equal(css([FLAGS.DASHED_UNDERLINE, -1, -1]), "text-decoration-line:underline;text-decoration-style:dashed;");
  assert.equal(css([FLAGS.STRIKETHROUGH, -1, -1, 1]), "text-decoration-line:line-through;", "an underline color alone draws no line");
  assert.equal(css([FLAGS.UNDERLINE | FLAGS.STRIKETHROUGH, -1, -1]), "text-decoration-line:underline line-through;");
});

test("bold, italic and hidden are attributes; blink draws nothing", () => {
  assert.equal(style.styleCss(resolve([FLAGS.BOLD | FLAGS.ITALIC, -1, -1])), "font-weight:bold;font-style:italic;");
  assert.equal(resolve([FLAGS.HIDDEN, -1, -1]).hidden, true);
  assert.equal(style.styleCss(resolve([FLAGS.BLINK, -1, -1])), "", "text blink is not drawn");
});

test("every style the format allows maps to CSS built only from safe characters", () => {
  // Sampled over the flag space and every color encoding: no output can
  // contain anything but declarations of fixed properties with hex or
  // rgba values.
  const colors = [-1, 0, 7, 8, 15, 16, 231, 255, 0x1000000, 0x1ffffff, 0x11a1b2c];
  for (let flags = 0; flags < 4096; flags += 7) {
    for (const fg of colors) {
      for (const bg of colors) {
        const css = style.styleCss(resolve([flags, fg, bg, fg]));
        assert.match(css, /^([a-z-]+:[a-z0-9#(),. -]+;)*$/, css);
      }
    }
  }
});

test("the mapper caches CSS per style and drops the cache when the defaults change", () => {
  const mapper = style.createStyleMapper(theme);
  const dim = model.decodeStyle([FLAGS.DIM, -1, -1]);
  assert.equal(mapper.css(dim), "color:rgba(255,255,255,0.5);");
  assert.equal(mapper.setDefaults(null, null), false, "unchanged defaults keep the cache");
  assert.equal(mapper.setDefaults(0x1000000 + 0x102030, null), true);
  assert.equal(mapper.css(dim), "color:rgba(16,32,48,0.5);");
  assert.deepEqual(mapper.defaults(), { fg: "#102030", bg: "#282c34" });
  assert.deepEqual(mapper.cursor(), { color: "#ffffff", accent: "#282c34" });
});
