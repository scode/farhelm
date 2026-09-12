// The shipped xterm.js theme is a cross-surface contract: the browser's
// palette, the CSS terminal ground, and tmux's OSC answers must describe the
// same readable default. This test pins the asset directly so a visual
// change cannot quietly drift from the source table or the CSS surface.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const theme = require("../assets/terminal-theme.js");

const APP_CSS_PATH = path.join(__dirname, "../assets/app.css");

test("terminal theme matches Ghostty's default palette", () => {
  assert.deepEqual(theme, {
    background: "#282C34",
    foreground: "#FFFFFF",
    cursor: "#FFFFFF",
    cursorAccent: "#282C34",
    black: "#1D1F21",
    red: "#CC6666",
    green: "#B5BD68",
    yellow: "#F0C674",
    blue: "#81A2BE",
    magenta: "#B294BB",
    cyan: "#8ABEB7",
    white: "#C5C8C6",
    brightBlack: "#666666",
    brightRed: "#D54E53",
    brightGreen: "#B9CA4A",
    brightYellow: "#E7C547",
    brightBlue: "#7AA6DA",
    brightMagenta: "#C397D8",
    brightCyan: "#70C0B1",
    brightWhite: "#EAEAEA",
  });
});

test("terminal CSS surface matches the theme background", () => {
  const css = fs.readFileSync(APP_CSS_PATH, "utf8");
  const match = css.match(/--terminal-bg\s*:\s*(#[0-9a-fA-F]{3,6})\s*;/);
  assert.ok(match, "app.css must declare --terminal-bg");
  assert.equal(match[1].toLowerCase(), theme.background.toLowerCase());
});
