// Pure decision for the Shift+Enter "insert newline" chord `terminal.js`
// wires into xterm.js's `attachCustomKeyEventHandler`. Split out on its own
// so `node --test` runs the EXACT function the page loads (see
// term-bytes.js's header for why that matters more than it might seem: a
// hand-copied test double could silently drift from what ships).
//
// ## Why ESC CR, and why PREFIX it rather than send it whole
//
// xterm.js encodes a plain Enter as `\r` regardless of the Shift key — it
// has no built-in idea of "Shift+Enter means something else". Claude Code
// and Codex, like most line-editing TUIs, bind `\x1b\r` (ESC, then CR) to
// "insert a literal newline without submitting" — it is the sequence their
// own terminal-setup guides tell users to map Shift+Enter to.
//
// An earlier version of this handler sent the FULL two-byte sequence itself
// and returned `false`, taking xterm out of the loop entirely for this key.
// Chromium/WebKit probing during review caught what that shortcut breaks:
// `false` tells xterm the key was handled elsewhere, so xterm skips its own
// Enter-keydown path — `scrollOnUserInput`'s jump-to-bottom, the selection
// dismissal `onKey` normally triggers, cleanup of the hidden input textarea
// xterm uses to catch IME and screen-reader input, and its accessibility
// announcements. Observably, both engines left a stray line break sitting
// in that hidden textarea afterward. None of that is Shift+Enter-specific
// plumbing this file should reimplement; it is xterm's ordinary Enter
// handling, which already does the right thing for a plain `\r`.
//
// The fix is to PREFIX rather than replace: on the recognized keydown, send
// only `\x1b` and return `true`. xterm then processes the Enter exactly as
// it would without this handler at all — full input path, `\r` emitted
// through the normal route, `_keyDownHandled` set so the browser's
// following `keypress` event is a no-op inside xterm (see `_keyPress`'s own
// `if (this._keyDownHandled) return` in assets/vendor/xterm.js — that guard
// is xterm's, not this file's, and it is what makes a SEPARATE "keypress"
// outcome unnecessary here). The two bytes land on the wire back-to-back —
// this file's `\x1b` immediately followed by xterm's own `\r` — which is
// byte-for-byte the same ESC CR sequence the old version assembled by hand,
// just produced by letting xterm finish the job instead of racing it.
//
// A plain shell has no ESC-CR binding of its own; what happens next is
// whatever that shell's line editor does with a leading ESC (commonly a
// sequence-prefix wait that times out, occasionally a bell) followed by an
// ordinary Enter. That is accepted as this feature's shape — there is no
// reliable way for the browser side to know what program the pty is
// currently running, so this file does not attempt to special-case shells.
(function () {
  /**
   * Decide what one xterm.js `attachCustomKeyEventHandler` call should do
   * for a Shift+Enter chord, given the KeyboardEvent (or a plain object
   * with the same shape — the node tests never construct a real DOM event).
   *
   * Returns one of:
   *  - `"prefix"`: this is the `keydown` of a genuine Shift+Enter. The
   *    caller should write a bare `\x1b` down the terminal's normal
   *    outbound data path and then return `true`, letting xterm process
   *    the Enter itself (see this file's header for why deferring to
   *    xterm, instead of sending `\r` here too, matters).
   *  - `"pass"`: not this chord — wrong key, a modifier combination this
   *    binding does not claim, an AltGraph-shifted key on an international
   *    layout, or mid-IME composition — or a non-keydown event type (with
   *    this design there is nothing left for keypress/keyup to do; xterm's
   *    own `_keyDownHandled` bookkeeping already dedupes keypress, and
   *    keyup never sends data in the first place). The caller returns
   *    `true` unconditionally and lets xterm handle the event exactly as
   *    it always has.
   *
   * The modifier check is deliberately an exact match, not "shiftKey is
   * set": Ctrl/Alt/Meta+Shift+Enter are xterm/application-defined chords in
   * their own right (e.g. some terminals' own newline or scroll bindings),
   * and this function must not steal them. AltGraph is excluded the same
   * way via `getModifierState` — `altKey`/`ctrlKey` do not reliably reflect
   * it across engines, and an international keyboard layout that uses
   * AltGraph+Shift+Enter for its own composed character must not have that
   * chord hijacked. `isComposing` is excluded for the same reason IME
   * composition is excluded everywhere else input is intercepted in this
   * codebase — an in-progress composition's Enter commits the composed
   * text, not a newline request. `keyCode === 229` is the DOM's older
   * in-composition marker: some WebKit/IME paths do not reliably flip
   * `isComposing` but still stamp this sentinel on the synthesized
   * keydown, so it is checked as a second, independent signal rather than
   * assumed redundant with `isComposing`.
   *
   * @param {{type: string, key: string, keyCode: number, shiftKey: boolean,
   *   ctrlKey: boolean, altKey: boolean, metaKey: boolean,
   *   isComposing: boolean, getModifierState?: (mod: string) => boolean}} ev
   * @returns {"prefix"|"pass"}
   */
  function shiftEnterKeyAction(ev) {
    if (!ev || ev.type !== "keydown") return "pass";
    const composing = !!ev.isComposing || ev.keyCode === 229;
    const altGraph = !!(ev.getModifierState && ev.getModifierState("AltGraph"));
    const isChord = ev.key === "Enter"
      && ev.shiftKey === true
      && !ev.ctrlKey
      && !ev.altKey
      && !ev.metaKey
      && !composing
      && !altGraph;
    return isChord ? "prefix" : "pass";
  }

  const api = { shiftEnterKeyAction };
  if (typeof window !== "undefined") window.farhelmShiftEnterKey = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})();
