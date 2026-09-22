#!/usr/bin/env node
// Renders the README's drawn blocks (the header mark and the pillars grid) as
// SVG files, one light and one dark variant each, into this directory, plus
// the standalone wordmark files beside the app icon in packaging/.
//
// Why a generator instead of four hand-edited files: the light and dark
// variants share every coordinate and differ only in colors, and the pillars
// grid is six copies of one cell. Editing that by hand in four places is how
// the variants drift apart. Run `node docs/readme/render-svgs.mjs` after
// changing anything here and commit the SVGs it writes; nothing in CI runs
// this, so the checked-in files are the artifact and this script is how they
// are reproduced.
//
// What GitHub allows in an SVG it shows through <img>, which is the only way a
// README can show one: no scripts, no external stylesheets or fonts, no
// references to other files, and CSS only inline. Everything below stays inside
// that. Text uses system font stacks, so the exact rendering differs per
// reader's platform; the line breaks are hand-placed with slack for that.
// Light and dark are separate files because an SVG shown through <img> cannot
// see the page's color scheme; the README picks the file with a <picture>
// element and a prefers-color-scheme media query, which GitHub honors.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

// The dark palette is the web UI's own (crates/farhelm-ui/assets/app.css:
// --bg-0/--bg-1, --accent, --ok, --warn, --danger-strong) plus the icon's
// cursor green, so the README reads as the same product as the screenshot
// under it. The light palette maps each role onto GitHub's light theme colors,
// since that is the ground the light file is shown on.
const palettes = {
  dark: {
    raised: "#141920",
    edge: "#2a3340",
    tileEdge: "#2a3340",
    fg: "#e6ebf1",
    dim: "#b8c0cc",
    faint: "#7c8694",
    accent: "#5896e8",
    ok: "#7fd88f",
    cursor: "#5CF08A",
    warn: "#d8b45f",
    danger: "#e47a72",
  },
  light: {
    raised: "#f6f8fa",
    edge: "#d1d9e0",
    fg: "#1f2328",
    dim: "#59636e",
    faint: "#818b98",
    accent: "#0969da",
    ok: "#1a7f37",
    cursor: "#1f883d",
    warn: "#9a6700",
    danger: "#cf222e",
  },
};

// The icon tile is the same in both themes: it is the app icon, and the app
// icon does not have a light mode.
const iconTile = "#0B0D10";
const iconGlyph = "#F4F7F5";

const mono = "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace";
const sans = "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans', Helvetica, Arial, sans-serif";

// The app icon's geometry, copied from packaging/farhelm-desktop/icon.svg so
// the README mark and the Dock tile are the same drawing. `size` is the
// rendered edge length; the source is drawn in a 256-unit space.
//
// On a dark ground the tile gets a one-unit edge in the palette's edge color:
// the tile is #0B0D10 and GitHub's dark page is #0d1117, close enough that
// without the edge the square vanishes and the F floats next to the wordmark
// as if the name were written twice. The light file has no edge because the
// tile already contrasts with a light page, and the icon itself is unchanged.
function iconMark(p, x, y, size) {
  const s = size / 256;
  const edge = p.tileEdge ? ` stroke="${p.tileEdge}" stroke-width="${(1 / s).toFixed(2)}"` : "";
  return `<g transform="translate(${x} ${y}) scale(${s})">
    <rect width="256" height="256" rx="58" fill="${iconTile}"${edge}/>
    <g fill="${iconGlyph}">
      <rect x="60" y="56" width="32" height="144"/>
      <rect x="60" y="56" width="100" height="30"/>
      <rect x="60" y="116" width="76" height="28"/>
    </g>
    <rect x="172" y="156" width="28" height="44" fill="${p.cursor}"/>
  </g>`;
}

// The wordmark: "farhelm" in JetBrains Mono Nerd Font Bold, the app's own
// face, with the icon's block cursor after it so the name and the tile read
// as one mark. The letters are outlines, not text: outline-wordmark.py next
// to this file pulls the glyph paths out of the vendored woff2 into
// wordmark-outline.json, and this inlines them, so the mark renders the same
// everywhere GitHub or anything else shows it. `size` is the font size the
// outlines are scaled to, and (x, y) is the text baseline, the same contract
// a <text> element would have had. Both the README header and the standalone
// wordmark files come from this one function.
const outline = JSON.parse(readFileSync(join(here, "wordmark-outline.json"), "utf8"));

function wordmark(p, x, y, size = 58) {
  const s = size / outline.unitsPerEm;
  const paths = outline.letters.map((l) => `<path d="${l.d}"/>`).join("");
  // The cursor is one glyph cell wide at a third of the advance, as tall as
  // the capitals, and sits where the next letter would be typed.
  const cursorX = outline.advance + 60;
  const cursorW = 300;
  return `<g transform="translate(${x} ${y}) scale(${s.toFixed(5)})" fill="${p.fg}">${paths}</g>
  <rect x="${(x + cursorX * s).toFixed(1)}" y="${(y - outline.capHeight * s).toFixed(1)}" width="${(cursorW * s).toFixed(1)}" height="${(outline.capHeight * s).toFixed(1)}" fill="${p.cursor}"/>`;
}

// The wordmark on its own, on a transparent ground, for anything that wants
// the name without the tile: the light file is for light grounds and the dark
// file for dark ones, the same way the README's <picture> picks them.
function wordmarkSvg(p) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 284 72" width="284" role="img" aria-label="farhelm">
  ${wordmark(p, 6, 56)}
</svg>
`;
}

// Icon, wordmark, tagline. The row of host dots under the tagline is a hint
// at the fleet, nothing more; the names are invented.
function headerSvg(p) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 760 190" width="760" role="img"
  aria-label="farhelm: take the helm of your agents, far away and local">
  ${iconMark(p, 200, 20, 96)}
  ${wordmark(p, 320, 88)}
  <text x="320" y="130" font-family="${sans}" font-size="18" fill="${p.dim}">Take the helm. Your agents, far away and local.</text>
  <g font-family="${mono}" font-size="12" fill="${p.faint}">
    <circle cx="326" cy="162" r="4" fill="${p.ok}"/><text x="338" y="166">mac</text>
    <circle cx="392" cy="162" r="4" fill="${p.ok}"/><text x="404" y="166">build-box</text>
    <circle cx="498" cy="162" r="4" fill="${p.ok}"/><text x="510" y="166">gpu-1</text>
    <circle cx="568" cy="162" r="4" fill="${p.warn}"/><text x="580" y="166">lab</text>
  </g>
</svg>
`;
}

// The six claims, ranked: reading order is left to right, then down, and the
// first cell is the one a reader should take away if they take away one.
// Each cell is an icon, a claim, and up to three lines of proof. SVG has no
// text wrapping and the font is whatever the reader's platform has, so every
// line is hand broken well short of the cell (about 44 characters at 14px)
// to survive a wide fallback face.
const pillars = (p) => [
  {
    title: "Sessions outlive everything",
    lines: [
      "Close the tab. Restart the control plane.",
      "Reboot the box the agent runs on.",
      "It comes back where it left off.",
    ],
    icon: `<path d="M6 20a14 14 0 1 1 4 9.9" fill="none" stroke="${p.accent}" stroke-width="2.5" stroke-linecap="round"/>
      <path d="M4 32v-8h8" fill="none" stroke="${p.accent}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
      <circle cx="20" cy="20" r="4" fill="${p.ok}"/>`,
  },
  {
    title: "Every host, one screen",
    lines: ["Your Mac plus any number of Linux boxes,", "all in one list, one helm."],
    icon: `<rect x="3" y="6" width="14" height="10" rx="2" fill="none" stroke="${p.accent}" stroke-width="2.5"/>
      <rect x="23" y="6" width="14" height="10" rx="2" fill="none" stroke="${p.accent}" stroke-width="2.5"/>
      <rect x="13" y="26" width="14" height="10" rx="2" fill="none" stroke="${p.accent}" stroke-width="2.5"/>
      <path d="M10 16v5h20v-5M20 21v5" fill="none" stroke="${p.faint}" stroke-width="2"/>`,
  },
  {
    title: "The real terminal",
    lines: ["Not a chat wrapper. You drive the agent’s", "own TUI, keystroke for keystroke."],
    icon: `<rect x="3" y="6" width="34" height="28" rx="3" fill="none" stroke="${p.accent}" stroke-width="2.5"/>
      <path d="M10 15l6 5-6 5" fill="none" stroke="${p.fg}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
      <rect x="19" y="23" width="9" height="3" fill="${p.cursor}"/>`,
  },
  {
    title: "SSH is the whole network",
    lines: ["No open ports, no relay, no account.", "If you can ssh to it, you can add it."],
    icon: `<rect x="8" y="18" width="24" height="18" rx="3" fill="none" stroke="${p.accent}" stroke-width="2.5"/>
      <path d="M13 18v-5a7 7 0 0 1 14 0v5" fill="none" stroke="${p.accent}" stroke-width="2.5"/>
      <circle cx="20" cy="27" r="2.5" fill="${p.fg}"/>`,
  },
  {
    title: "Status at a glance",
    lines: ["Working, waiting on you, done, crashed.", "Every session, without opening any of them."],
    icon: `<circle cx="9" cy="10" r="4" fill="${p.ok}"/><rect x="17" y="8" width="20" height="4" rx="2" fill="${p.faint}"/>
      <circle cx="9" cy="20" r="4" fill="${p.warn}"/><rect x="17" y="18" width="14" height="4" rx="2" fill="${p.faint}"/>
      <circle cx="9" cy="30" r="4" fill="${p.danger}"/><rect x="17" y="28" width="17" height="4" rx="2" fill="${p.faint}"/>`,
  },
  {
    title: "Any agent, any VCS",
    lines: [
      "Codex, Claude Code, Muse, Cursor, OMP, Pi",
      "and more. Anything else still runs, with",
      "fewer of the smarts.",
    ],
    icon: `<rect x="4" y="4" width="14" height="14" rx="3" fill="none" stroke="${p.accent}" stroke-width="2.5"/>
      <rect x="22" y="4" width="14" height="14" rx="3" fill="none" stroke="${p.accent}" stroke-width="2.5"/>
      <rect x="4" y="22" width="14" height="14" rx="3" fill="none" stroke="${p.accent}" stroke-width="2.5"/>
      <path d="M29 23v12M23 29h12" fill="none" stroke="${p.faint}" stroke-width="2.5" stroke-linecap="round"/>`,
  },
];

// Two columns of three. Three columns was tried first and lost: at that width
// a 13px line had to stay under about 36 characters, which no honest proof
// line managed.
function pillarsSvg(p) {
  const cols = 2;
  const cw = 480;
  const rh = 128;
  const cells = pillars(p)
    .map((c, i) => {
      const x = (i % cols) * cw;
      const y = Math.floor(i / cols) * rh;
      const tspans = c.lines.map((l, j) => `<tspan x="88" y="${68 + j * 19}">${l}</tspan>`).join("");
      return `<g transform="translate(${x} ${y})">
    <rect x="8" y="8" width="${cw - 16}" height="${rh - 16}" rx="8" fill="${p.raised}" stroke="${p.edge}"/>
    <g transform="translate(30 26)">${c.icon}</g>
    <text x="88" y="44" font-family="${sans}" font-size="17" font-weight="600" fill="${p.fg}">${c.title}</text>
    <text font-family="${sans}" font-size="14" fill="${p.dim}">${tspans}</text>
  </g>`;
    })
    .join("\n  ");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${cols * cw} ${rh * 3}" width="${cols * cw}" role="img"
  aria-label="What Farhelm gives you: durable sessions, every host on one screen, the real terminal, SSH as the only network, status at a glance, any agent">
  ${cells}
</svg>
`;
}

// The standalone wordmark lives beside the app icon it is drawn to match, so
// anyone looking for the brand marks finds both in one place.
const brandDir = join(here, "..", "..", "packaging", "farhelm-desktop");

for (const [theme, p] of Object.entries(palettes)) {
  writeFileSync(join(here, `header-${theme}.svg`), headerSvg(p));
  writeFileSync(join(here, `pillars-${theme}.svg`), pillarsSvg(p));
  writeFileSync(join(brandDir, `wordmark-${theme}.svg`), wordmarkSvg(p));
}
