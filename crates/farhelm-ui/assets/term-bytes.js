// The byte-domain conversion `terminal.js` needs for `term.onBinary`,
// pulled out on its own so `node --test` can load the EXACT file the page
// loads (PLAN_M6_5.md item 1) — no bundler, no copy that could drift from
// what ships.
//
// Loaded as a plain script alongside xterm.js and addon-fit.js (see
// terminal.js's header). Registration order puts this file ahead of
// terminal.js, but Dioxus injects scripts asynchronously, so order is NOT
// an execution guarantee — terminal.js's mount readiness gate waits for
// the global this file assigns, and that gate, not load order, is the
// contract. No module system in the browser, so this file exposes itself
// as a global. It also assigns
// `module.exports` when that exists, which is true under `node --test` and
// false in the browser — the same file, unmodified, serves both callers.
//
// The whole body is an IIFE so `binaryStringToBytes` itself never becomes a
// bare global: without it, the top-level `function` declaration below would
// leak `window.binaryStringToBytes` in addition to the intentionally
// namespaced `window.farhelmTermBytes.binaryStringToBytes`, growing the
// global surface by one unremovable name for no reason anyone would choose
// on purpose.
(function () {
  /**
   * xterm.js's `onBinary` callback hands us a JS string in which every
   * UTF-16 code unit already IS a byte value (0-255) — the "binary
   * string" convention xterm uses for non-UTF8 input such as mouse reports,
   * which routinely carry bytes 0x80-0xff that would be lossy round-tripped
   * through UTF-8. (Code UNIT, not code point: `charCodeAt` and `length`
   * operate on code units, so an astral code point would yield two output
   * bytes — which is correct for this contract, and why the docs must not
   * say "code point".) The contract this function pins is byte-for-byte:
   * every code unit becomes its low byte, full stop — the `Uint8Array`
   * element assignment below performs that truncation on its own, so there
   * is nothing else here to detect a caller passing a code unit over 0xff
   * (that would be a caller bug, not this function's to guard against).
   *
   * @param {string} binaryString - one UTF-16 code unit per output byte.
   * @returns {Uint8Array} the same length as `binaryString`.
   */
  function binaryStringToBytes(binaryString) {
    const bytes = new Uint8Array(binaryString.length);
    for (let i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i);
    }
    return bytes;
  }

  // Browser global (matches terminal.js's `window.farhelmTerm`-style
  // naming for page-visible helpers) when a `window` exists, and a
  // CommonJS export when `module` exists instead — `node --test` has the
  // latter but not the former, so both assignments are individually
  // guarded rather than assumed. That is what lets the page and the unit
  // tests load this exact file rather than a hand-maintained copy of it.
  if (typeof window !== "undefined") {
    window.farhelmTermBytes = { binaryStringToBytes };
  }
  if (typeof module !== "undefined" && module.exports) {
    module.exports = { binaryStringToBytes };
  }
})();
