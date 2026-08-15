// Pure decision behind terminal.js's herdr-style copy-on-select: when a
// completed mouse gesture on the terminal should push the current LOCAL
// xterm selection to the system clipboard. Split out on its own for the
// same reason shift-enter-key.js is (see that file's header) — `node
// --test` runs the EXACT function terminal.js calls, not a hand-copied
// double that could silently drift from what ships.
//
// ## Which selection this covers
//
// A session terminal has TWO independent ideas of "the user selected text",
// and this module is deliberately about only one of them.
//
// - An agent TUI (Claude Code, Codex) that has turned mouse reporting on
//   handles a plain drag ITSELF: xterm.js's SelectionService is disabled
//   for as long as mouse tracking is active (confirmed against the
//   vendored xterm.js — `CoreBrowserTerminal.bindMouse`'s
//   `onProtocolChange` handler calls `this._selectionService.disable()`
//   the moment an app requests mouse events), so a plain drag never becomes
//   a LOCAL selection at all — this page's own selection handlers never
//   fire, and OSC 52 (the vendored `@xterm/addon-clipboard`, wired in
//   terminal.js's `mount()`) is the only path that can reach the system
//   clipboard for it.
// - Holding Shift while dragging FORCES a local xterm selection even with
//   mouse reporting on (`SelectionService.shouldForceSelection` returns
//   `event.shiftKey` on every platform but macOS, checked before the
//   disabled-selection early return; on macOS the same function reads
//   Option instead — see terminal.js's `macOptionClickForcesSelection`
//   comment), exactly like double-click word-select and triple-click
//   line-select, and exactly like ordinary dragging once nothing has mouse
//   reporting on at all. THIS is the selection this module's decision is
//   about.
//
// Both paths are real and neither subsumes the other, which is why this
// fix has two halves living side by side rather than one.
//
// ## Every completed non-empty selection copies — no "did this change" cache
//
// An earlier version of this module compared the gesture's selection
// against the text it last copied, skipping a mouseup that reproduced it —
// intended as a guard against redundant copies, but wrong: it silently
// suppressed a REAL re-copy whenever the SAME text was selected twice in a
// row, which is exactly what happens when something else (another app, a
// different terminal) has overwritten the system clipboard in between and
// the user reselects the text they want back. A copy mechanism that
// sometimes declines to copy what is plainly selected is a worse bug than
// the redundant write it was trying to prevent — a write of identical bytes
// is not observable as a problem, but a copy that silently did not happen
// is.
//
// The property that guard was actually protecting — "a plain click without
// a drag must not clobber the clipboard" — never depended on the cache in
// the first place. It falls out of xterm's OWN selection model: a click
// with no movement never gets a selection END (`SelectionService`'s
// `_handleSingleClick` sets `selectionStart` but leaves `selectionEnd`
// undefined), so `hasSelection()` is false and this function already
// declines on that alone. The decision is therefore just: did this gesture
// end with a non-empty local selection.
(function () {
  /**
   * Whether a completed mouse gesture on the terminal should push the
   * current LOCAL xterm selection to the system clipboard.
   *
   * `hasSelection` is `term.hasSelection()` and `selectionText` is
   * `term.getSelection()` — passed separately, rather than inferring
   * "selected" from a non-empty string, because they are xterm's own two
   * independent signals (see its `hasSelection` getter) and this stays a
   * thin decision over both rather than a second guess about their
   * relationship.
   *
   * @param {{hasSelection: boolean, selectionText: string}} state
   * @returns {boolean} true to write `selectionText` to the clipboard now
   */
  function copySelectionOnMouseUp(state) {
    return !!(state && state.hasSelection && state.selectionText);
  }

  const api = { copySelectionOnMouseUp };
  if (typeof window !== "undefined") window.farhelmCopyOnSelect = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})();
