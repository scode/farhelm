// How a grid run's style becomes inline CSS: pure, no DOM.
//
// The grid stream carries a style as integers (a flag set and three
// colors, see grid-model.js). This file maps them to the declarations the
// renderer puts in a span's `style` attribute, through fixed tables: the
// theme's palette (Ghostty's defaults, terminal-theme.js) for the 16 named
// colors, xterm's standard cube and gray ramp for 16-255, and hex for a
// direct color. Because every value comes from those tables or from a
// validated integer, no wire string can reach CSS.
//
// ## Matching old terminals
//
// A new terminal draws styles by the same rules as an old one, so a
// session looks the same whichever kind of terminal shows it. The rules
// were read from the vendored xterm.js bundle's `DomRendererRowFactory`,
// as Farhelm configures it:
//
// - Inverse swaps foreground and background first; a default color that
//   gets swapped becomes the other default.
// - Bold text in a palette color 0-7 draws in its bright twin 8-15
//   (`drawBoldTextInBrightColors`, on by default), as does a palette
//   underline color under bold.
// - Dim is the foreground at half opacity.
// - Hidden text keeps its background and decorations; the renderer swaps
//   its characters for spaces, so the text is not in the page at all.
// - Blink (SGR 5) is not drawn, as old terminals draw no text blink.
//
// Two deliberate departures, both where the old terminal's CSS loses a
// declaration to the cascade rather than choosing a look: a dim direct
// color is dimmed (there, the inline color overrides the dim rule), and an
// underlined, struck-through cell shows both lines (there, two classes
// both set `text-decoration` and the later one wins).
//
// ## Why inline styles, cached per style
//
// The renderer benchmark behind this design found that adding a
// stylesheet rule per style invalidates style for the whole page on every
// new style, which collapsed WebKit under true-color output. Inline
// declarations do not, and caching the string per style key keeps the
// per-run cost to a map lookup and a string comparison.
(function (root) {
  "use strict";

  const FLAGS = Object.freeze({
    BOLD: 1,
    DIM: 2,
    ITALIC: 4,
    UNDERLINE: 8,
    DOUBLE_UNDERLINE: 16,
    UNDERCURL: 32,
    DOTTED_UNDERLINE: 64,
    DASHED_UNDERLINE: 128,
    INVERSE: 256,
    HIDDEN: 512,
    STRIKETHROUGH: 1024,
    BLINK: 2048,
  });
  const ANY_UNDERLINE = FLAGS.UNDERLINE | FLAGS.DOUBLE_UNDERLINE | FLAGS.UNDERCURL | FLAGS.DOTTED_UNDERLINE |
    FLAGS.DASHED_UNDERLINE;
  const DIRECT_COLOR_BASE = 0x1000000;
  const ANSI_NAMES = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "brightBlack",
    "brightRed",
    "brightGreen",
    "brightYellow",
    "brightBlue",
    "brightMagenta",
    "brightCyan",
    "brightWhite",
  ];
  // The cache is a convenience, not a correctness boundary; a program
  // cycling through true colors would otherwise grow it without limit.
  const CSS_CACHE_LIMIT = 4096;

  function hex2(value) {
    return value.toString(16).padStart(2, "0");
  }

  /**
   * A theme color as lowercase `#rrggbb`. The theme is this bundle's own
   * asset, but it still passes through here so nothing but a hex color can
   * reach a style string.
   */
  function themeColor(value, fallback) {
    return typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value) ? value.toLowerCase() : fallback;
  }

  /**
   * The 256-color palette: the theme's 16 named colors, then xterm's
   * standard 6x6x6 cube and 24-step gray ramp (the same table old terminals
   * compute for indices the theme does not name).
   */
  function buildPalette(theme) {
    const palette = ANSI_NAMES.map((name) => themeColor(theme && theme[name], "#000000"));
    const levels = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
    for (let r = 0; r < 6; r++) {
      for (let g = 0; g < 6; g++) {
        for (let b = 0; b < 6; b++) palette.push(`#${hex2(levels[r])}${hex2(levels[g])}${hex2(levels[b])}`);
      }
    }
    for (let i = 0; i < 24; i++) {
      const v = hex2(8 + i * 10);
      palette.push(`#${v}${v}${v}`);
    }
    return palette;
  }

  /** A validated direct color (`0x1000000 + 0xRRGGBB`) as `#rrggbb`. */
  function directColor(value) {
    return `#${(value - DIRECT_COLOR_BASE).toString(16).padStart(6, "0")}`;
  }

  /** `#rrggbb` at half opacity, the dim text color (as in old terminals). */
  function halfOpacity(color) {
    const n = parseInt(color.slice(1), 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},0.5)`;
  }

  /**
   * The colors the terminal treats as "default": the theme's, or the ones a
   * program redefined with OSC 10/11 (the `state` message's `default_fg`
   * and `default_bg`, direct colors or null).
   */
  function defaultColors(theme, defaultFg, defaultBg) {
    return {
      fg: defaultFg === null || defaultFg === undefined ? themeColor(theme && theme.foreground, "#ffffff") : directColor(defaultFg),
      bg: defaultBg === null || defaultBg === undefined ? themeColor(theme && theme.background, "#000000") : directColor(defaultBg),
    };
  }

  /**
   * Resolve a validated style to concrete colors and text attributes.
   *
   * `fg` and `bg` are `#rrggbb`, an `rgba(...)` for dim text, or null where
   * the run inherits the terminal's default (no declaration needed); `ul`
   * is null for "same as the text".
   */
  function resolveStyle(style, palette, defaults) {
    const flags = style.flags;
    const bold = !!(flags & FLAGS.BOLD);
    let fg = style.fg;
    let bg = style.bg;
    const inverse = !!(flags & FLAGS.INVERSE);
    if (inverse) [fg, bg] = [bg, fg];

    let bgColor = null;
    if (bg === -1) bgColor = inverse ? defaults.fg : null;
    else if (bg < 256) bgColor = palette[bg];
    else bgColor = directColor(bg);

    let fgColor;
    if (fg === -1) fgColor = inverse ? defaults.bg : null;
    else if (fg < 256) fgColor = palette[bold && fg < 8 ? fg + 8 : fg];
    else fgColor = directColor(fg);
    if (flags & FLAGS.DIM) fgColor = halfOpacity(fgColor || defaults.fg);

    let ulColor = null;
    if (style.ul !== -1) {
      ulColor = style.ul < 256 ? palette[bold && style.ul < 8 ? style.ul + 8 : style.ul] : directColor(style.ul);
    }

    let underline = null;
    if (flags & ANY_UNDERLINE) {
      underline = flags & FLAGS.DOUBLE_UNDERLINE
        ? "double"
        : flags & FLAGS.UNDERCURL
        ? "wavy"
        : flags & FLAGS.DOTTED_UNDERLINE
        ? "dotted"
        : flags & FLAGS.DASHED_UNDERLINE
        ? "dashed"
        : "solid";
    }

    return {
      fg: fgColor,
      bg: bgColor,
      ul: ulColor,
      bold,
      italic: !!(flags & FLAGS.ITALIC),
      underline,
      strikethrough: !!(flags & FLAGS.STRIKETHROUGH),
      hidden: !!(flags & FLAGS.HIDDEN),
    };
  }

  /** The inline declarations for a resolved style ("" for the default). */
  function styleCss(resolved) {
    let css = "";
    if (resolved.fg) css += `color:${resolved.fg};`;
    if (resolved.bg) css += `background-color:${resolved.bg};`;
    if (resolved.bold) css += "font-weight:bold;";
    if (resolved.italic) css += "font-style:italic;";
    const lines = [];
    if (resolved.underline) lines.push("underline");
    if (resolved.strikethrough) lines.push("line-through");
    if (lines.length) {
      css += `text-decoration-line:${lines.join(" ")};`;
      if (resolved.underline && resolved.underline !== "solid") css += `text-decoration-style:${resolved.underline};`;
      if (resolved.ul && resolved.underline) css += `text-decoration-color:${resolved.ul};`;
    }
    return css;
  }

  /**
   * A style-to-CSS mapper for one terminal: the palette, the current
   * default colors, and a bounded cache of CSS strings by style key.
   *
   * `setDefaults` returns whether the defaults changed; when they did, the
   * cache is dropped (a dim or inverse default depends on them) and the
   * caller must redraw every row.
   */
  function createStyleMapper(theme) {
    const palette = buildPalette(theme);
    let defaultFg = null;
    let defaultBg = null;
    let defaults = defaultColors(theme, null, null);
    let cache = new Map();
    return {
      palette,
      defaults: () => defaults,
      cursor: () => ({
        color: themeColor(theme && theme.cursor, defaults.fg),
        accent: themeColor(theme && theme.cursorAccent, defaults.bg),
      }),
      setDefaults(fg, bg) {
        if (fg === defaultFg && bg === defaultBg) return false;
        defaultFg = fg;
        defaultBg = bg;
        defaults = defaultColors(theme, fg, bg);
        cache = new Map();
        return true;
      },
      css(style) {
        let css = cache.get(style.key);
        if (css === undefined) {
          if (cache.size >= CSS_CACHE_LIMIT) cache = new Map();
          css = styleCss(resolveStyle(style, palette, defaults));
          cache.set(style.key, css);
        }
        return css;
      },
    };
  }

  const api = {
    FLAGS,
    buildPalette,
    createStyleMapper,
    defaultColors,
    directColor,
    resolveStyle,
    styleCss,
  };
  root.farhelmGridStyle = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(typeof window === "object" ? window : globalThis);
