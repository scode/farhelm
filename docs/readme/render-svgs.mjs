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
    accentFill: "#15263d",
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
    accentFill: "#ddf4ff",
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

// "How it works" at a glance: the helm on a laptop, remote boxes over ssh,
// and one session shown on both ends. The laptop's screen is a miniature of
// the real UI (host list, session list, one terminal pane) drawn on the
// app's own dark ground in both themes, because the app has no light mode
// and a dark screen on a light page reads as a screenshot, which is the
// point. Each remote box lists its sessions as chips; the sidebar lists the
// same titles; and the selected session carries the accent in all three
// places (chip, row, pane) so the reader can see that the pane IS that
// chip. Terminal contents are drawn as bars rather than text: at this size
// real text would be unreadable and invite squinting at invented output.
//
// Everything here is invented: host names, session titles, statuses. None
// of it is a capture.
const screen = {
  bg: "#05070a",
  raised: "#141920",
  edge: "#242c37",
  fg: "#e6ebf1",
  dim: "#8b95a3",
  bar: "#3a4450",
  accentFill: "#15263d",
  accent: "#5896e8",
  ok: "#7fd88f",
  warn: "#d8b45f",
  danger: "#e47a72",
  idle: "#5a6472",
  cursor: "#5CF08A",
};

// Sessions per host. `mark` is the status glyph's colour; the selected one
// is the session the pane shows. Order in the sidebar is by recency, which
// is why it is not grouped by host, exactly as in the real list.
const fleet = [
  { host: "build-box", kind: "remote", sessions: [
    { title: "fix flaky attach test", mark: screen.ok, selected: true },
    { title: "port ci to nextest", mark: screen.warn },
    { title: "release 0.3.1", mark: screen.idle },
  ] },
  { host: "gpu-1", kind: "remote", sessions: [
    { title: "readme rewrite", mark: screen.warn },
    { title: "bench tokenizer", mark: screen.ok },
  ] },
];
const localSessions = [{ title: "migrate auth tokens", mark: screen.danger }];
const sidebar = [
  fleet[0].sessions[0], localSessions[0], fleet[1].sessions[0],
  fleet[1].sessions[1], fleet[0].sessions[1], fleet[0].sessions[2],
];

function statusDot(x, y, colour) {
  return `<circle cx="${x}" cy="${y}" r="3" fill="${colour}"/>`;
}

// The miniature UI inside the laptop's screen. (x, y) is the screen's top
// left; the screen is 376 by 226.
function screenContent(x, y) {
  const sbw = 138;
  let out = `<rect x="${x}" y="${y}" width="376" height="226" fill="${screen.bg}"/>
    <rect x="${x}" y="${y}" width="${sbw}" height="226" fill="${screen.raised}"/>
    <text x="${x + 10}" y="${y + 18}" font-family="${mono}" font-size="8" fill="${screen.dim}" letter-spacing="1">HOSTS</text>`;
  const hosts = ["this mac", ...fleet.map((h) => h.host)];
  hosts.forEach((h, i) => {
    const yy = y + 34 + i * 15;
    out += `${statusDot(x + 14, yy - 3, screen.ok)}<text x="${x + 24}" y="${yy}" font-family="${mono}" font-size="9" fill="${screen.fg}">${h}</text>`;
  });
  out += `<text x="${x + 10}" y="${y + 92}" font-family="${mono}" font-size="8" fill="${screen.dim}" letter-spacing="1">SESSIONS</text>`;
  sidebar.forEach((s, i) => {
    const yy = y + 100 + i * 19;
    if (s.selected) {
      out += `<rect x="${x}" y="${yy}" width="${sbw}" height="19" fill="${screen.accentFill}"/>
      <rect x="${x}" y="${yy}" width="2" height="19" fill="${screen.accent}"/>`;
    }
    out += `${statusDot(x + 14, yy + 10, s.mark)}<text x="${x + 24}" y="${yy + 13}" font-family="${mono}" font-size="9" fill="${screen.fg}">${s.title}</text>`;
  });
  // The pane: a title line naming the session and its host, then terminal
  // output as bars, ending in the block cursor.
  const px = x + sbw + 12;
  const sel = fleet[0].sessions[0];
  out += `<text x="${px}" y="${y + 20}" font-family="${mono}" font-size="9" font-weight="700" fill="${screen.fg}">${sel.title}</text>
    <text x="${px}" y="${y + 33}" font-family="${mono}" font-size="8" fill="${screen.accent}">${fleet[0].host}</text>
    <line x1="${px}" y1="${y + 42}" x2="${x + 366}" y2="${y + 42}" stroke="${screen.edge}"/>`;
  const widths = [150, 90, 200, 120, 60, 180, 100, 140, 30, 170];
  widths.forEach((w, i) => {
    const yy = y + 54 + i * 15;
    const indent = i % 3 === 0 ? 0 : 12;
    out += `<rect x="${px + indent}" y="${yy}" width="${w}" height="6" rx="3" fill="${screen.bar}"/>`;
  });
  out += `<rect x="${px}" y="${y + 204}" width="7" height="12" fill="${screen.cursor}"/>`;
  return out;
}

// A laptop: screen with bezel, then a base drawn as a flat trapezoid. The
// screen interior comes from screenContent.
function laptop(p, x, y) {
  return `<rect x="${x}" y="${y}" width="400" height="250" rx="12" fill="${p.raised}" stroke="${p.edge}" stroke-width="1.5"/>
    ${screenContent(x + 12, y + 12)}
    <path d="M${x - 20} ${y + 250} H${x + 420} L${x + 440} ${y + 268} H${x - 40} Z" fill="${p.raised}" stroke="${p.edge}" stroke-width="1.5"/>
    <rect x="${x + 160}" y="${y + 250}" width="80" height="4" fill="${p.edge}"/>`;
}

// A remote box: a chassis with a server glyph and its host name, then one
// chip per session. The selected session's chip carries the accent.
function remoteBox(p, x, y, host) {
  const w = 250;
  const h = 44 + host.sessions.length * 32;
  let out = `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="8" fill="${p.raised}" stroke="${p.edge}" stroke-width="1.5"/>
    <g stroke="${p.faint}" stroke-width="2" stroke-linecap="round">
      <line x1="${x + 16}" y1="${y + 14}" x2="${x + 34}" y2="${y + 14}"/>
      <line x1="${x + 16}" y1="${y + 20}" x2="${x + 34}" y2="${y + 20}"/>
      <line x1="${x + 16}" y1="${y + 26}" x2="${x + 34}" y2="${y + 26}"/>
    </g>
    <text x="${x + 44}" y="${y + 24}" font-family="${mono}" font-size="13" font-weight="700" fill="${p.fg}">${host.host}</text>
    <text x="${x + w - 12}" y="${y + 24}" text-anchor="end" font-family="${sans}" font-size="11" fill="${p.faint}">supervisor</text>`;
  host.sessions.forEach((s, i) => {
    const yy = y + 38 + i * 32;
    const stroke = s.selected ? p.accent : p.edge;
    const fill = s.selected ? p.accentFill : "none";
    out += `<rect x="${x + 12}" y="${yy}" width="${w - 24}" height="26" rx="5" fill="${fill}" stroke="${stroke}" stroke-width="${s.selected ? 2 : 1}"/>
      <rect x="${x + 22}" y="${yy + 8}" width="12" height="10" rx="2" fill="none" stroke="${p.faint}" stroke-width="1.5"/>
      <path d="M${x + 25} ${yy + 11}l2 2-2 2" fill="none" stroke="${p.faint}" stroke-width="1.2"/>
      ${statusDot(x + 46, yy + 13, s.mark)}
      <text x="${x + 56}" y="${yy + 17}" font-family="${sans}" font-size="12" fill="${p.fg}">${s.title}</text>`;
  });
  return out;
}

// An ssh link between the laptop and a box: a line with a padlock on it and
// the word ssh, no arrowheads, because the connection is the helm's to make
// but the traffic goes both ways.
function sshLink(p, x1, y1, x2, y2) {
  const mx = (x1 + x2) / 2;
  const my = (y1 + y2) / 2;
  return `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${p.faint}" stroke-width="2"/>
    <rect x="${mx - 22}" y="${my - 11}" width="44" height="22" rx="11" fill="${p.raised}" stroke="${p.edge}"/>
    <rect x="${mx - 14}" y="${my - 2}" width="9" height="7" rx="1.5" fill="${p.faint}"/>
    <path d="M${mx - 12} ${my - 2}v-2.5a2.5 2.5 0 0 1 5 0v2.5" fill="none" stroke="${p.faint}" stroke-width="1.5"/>
    <text x="${mx - 1}" y="${my + 4}" font-family="${mono}" font-size="11" fill="${p.dim}">ssh</text>`;
}

// Captions are centred under their subject; the laptop's is 50px from the
// drawing's left edge, so a caption line has to stay under about 400px at
// 13px sans (roughly 55 characters) or its start gets clipped, which the
// first draft's second line did.
function howItWorksSvg(p) {
  const W = 960;
  const H = 400;
  const lapX = 50;
  const lapY = 30;
  const boxX = 660;
  const box1Y = 30;
  const box2Y = 30 + 44 + 3 * 32 + 30;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" role="img"
  aria-label="How Farhelm works: the helm runs on your laptop and shows every session from every host in one list; it reaches a supervisor on each remote box over ssh; the selected session's real terminal fills the pane.">
  ${sshLink(p, lapX + 412, lapY + 110, boxX, box1Y + 60)}
  ${sshLink(p, lapX + 412, lapY + 170, boxX, box2Y + 50)}
  ${laptop(p, lapX, lapY)}
  ${remoteBox(p, boxX, box1Y, fleet[0])}
  ${remoteBox(p, boxX, box2Y, fleet[1])}
  <text x="${lapX + 200}" y="${lapY + 300}" text-anchor="middle" font-family="${sans}" font-size="13" fill="${p.dim}">your laptop: the helm, its UI, and a local supervisor</text>
  <text x="${lapX + 200}" y="${lapY + 320}" text-anchor="middle" font-family="${sans}" font-size="13" fill="${p.dim}">every host's sessions in one list, one real terminal</text>
  <text x="${boxX + 125}" y="${H - 28}" text-anchor="middle" font-family="${sans}" font-size="13" fill="${p.dim}">any Linux box you can ssh to:</text>
  <text x="${boxX + 125}" y="${H - 10}" text-anchor="middle" font-family="${sans}" font-size="13" fill="${p.dim}">a supervisor that owns its sessions' terminals</text>
</svg>
`;
}

// The standalone wordmark lives beside the app icon it is drawn to match, so
// anyone looking for the brand marks finds both in one place.
const brandDir = join(here, "..", "..", "packaging", "farhelm-desktop");

for (const [theme, p] of Object.entries(palettes)) {
  writeFileSync(join(here, `header-${theme}.svg`), headerSvg(p));
  writeFileSync(join(here, `pillars-${theme}.svg`), pillarsSvg(p));
  writeFileSync(join(here, `how-it-works-${theme}.svg`), howItWorksSvg(p));
  writeFileSync(join(brandDir, `wordmark-${theme}.svg`), wordmarkSvg(p));
}
