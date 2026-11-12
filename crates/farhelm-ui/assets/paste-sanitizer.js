// The paste sanitizing `terminal.js` and the grid component apply to text
// they paste, pulled out on its own so `node --test` can load the EXACT file
// the page loads (PLAN_M6_5.md item 1) — no bundler, no copy that could
// drift from what ships.
//
// Registration order puts this file ahead of terminal.js, but Dioxus
// injects scripts asynchronously, so order is NOT an execution guarantee —
// terminal.js's mount readiness gate waits for the global this file
// assigns, and that gate, not load order, is the contract. No module system
// in the browser, so this file exposes itself as a global. It also assigns
// `module.exports` when that exists, which is true under `node --test` and
// false in the browser — the same file, unmodified, serves both callers.
//
// The whole body is an IIFE so the helper itself never becomes a bare
// global: without it, a top-level `function` declaration would leak
// `window.sanitizePastedText` in addition to the intentionally namespaced
// `window.farhelmPasteSanitizer.sanitizePastedText`.
(function () {
  /**
   * Keep pasted text inside the terminal's own bracketed-paste frame.
   * An embedded end marker would turn the remaining text into typed input.
   * Remove that marker and all remaining ESC bytes: preserving ESC fragments
   * could assemble another end marker when the embedded one is removed.
   * Ordinary text, Unicode and line breaks still reach the paste intact.
   */
  function sanitizePastedText(text) {
    return text.replace(/\x1b\[201~|\x1b/g, "");
  }

  // Browser global (matches terminal.js's `window.farhelmTerm`-style
  // naming for page-visible helpers) when a `window` exists, and a
  // CommonJS export when `module` exists instead — `node --test` has the
  // latter but not the former, so both assignments are individually
  // guarded rather than assumed. That is what lets the page and the unit
  // tests load this exact file rather than a hand-maintained copy of it.
  if (typeof window !== "undefined") {
    window.farhelmPasteSanitizer = { sanitizePastedText };
  }
  if (typeof module !== "undefined" && module.exports) {
    module.exports = { sanitizePastedText };
  }
})();
