// Ghostty's default terminal theme, read verbatim from upstream's
// `src/config/Config.zig` and `src/terminal/color.zig` at commit
// 44f2a44df7e8c4a0c6df3f7d872ef3d7ead88e51 on 2026-09-11. Keeping this as a
// plain asset object makes the browser palette and its node regression test
// consume the same values without introducing a module loader to the page.
(function () {
  const theme = {
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
  };

  if (typeof window !== "undefined") window.farhelmTerminalTheme = theme;
  if (typeof module !== "undefined" && module.exports) module.exports = theme;
})();
