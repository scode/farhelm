// Pure decisions for the Shift+Enter "insert newline" chord `terminal.js`
// wires into xterm.js. Split out on its own so `node --test` runs the EXACT
// functions the page loads (see term-bytes.js's header for why that matters
// more than it might seem: a hand-copied test double could silently drift
// from what ships).
//
// ## Why ESC CR, and why it must leave as ONE write
//
// xterm.js encodes a plain Enter as `\r` regardless of the Shift key — it
// has no built-in idea of "Shift+Enter means something else". The chord is
// therefore synthesized here as ESC CR (`\x1b\r`), the sequence Ghostty
// emits for Shift+Enter and the newline binding line-editing TUIs
// understand in place of submit.
//
// The delivery constraint is the part that was learned the hard way. A
// SMALL outbound websocket message — one that fits a single protocol frame
// and a single `send-keys -H` command; the supervisor chunks larger input
// at 256 bytes per command, and the transport frames at 32KiB — becomes
// one command, and tmux flushes each command to the pane's pty as a
// separate write (verified empirically on the pinned tmux 3.7b,
// 2026-08-19: two commands arrived as two reads ~7ms apart, never
// coalesced; one command with both bytes arrived as one read). The
// two-byte chord always fits, which is what this file's guarantee rides
// on; nothing here claims atomic delivery for larger messages. An earlier
// version of this chord sent the ESC from the keydown handler and let
// xterm's own Enter path send the CR — two producers, two messages, two
// pty writes. Readers that disambiguate a lone ESC by READ BOUNDARY
// rather than by timeout then see Escape followed by plain Enter: the
// crossterm family (Codex — upstream openai/codex#21699 shows the same
// symptom in tmux, though its diagnosis is extended-key negotiation, not
// write splitting), and any zsh with a small KEYTIMEOUT (the vi-mode
// convention `KEYTIMEOUT=1` means 10ms). bash's readline, with its 500ms
// keyseq-timeout, papered over the split — which is why the bug shipped.
// Sending both bytes in one message closes the split at the source: one
// message, one `send-keys`, one pty write. Verified against the real TUIs
// (2026-08-19, scratch tmux 3.7b): single-write ESC CR inserts a genuine
// line break in both codex-cli 0.147.0 and Claude Code 2.1.235, with
// nothing submitted — so one sequence serves every pane and no per-agent
// branching exists anywhere.
//
// ## Why ARM-and-MERGE rather than sending the pair directly
//
// A previous revision sent the full two-byte sequence from the key handler
// and returned `false`, taking xterm out of the loop for this key.
// Chromium/WebKit probing during review caught what that shortcut breaks:
// `false` tells xterm the key was handled elsewhere, so xterm skips its own
// Enter-keydown path — `scrollOnUserInput`'s jump-to-bottom, selection
// dismissal, cleanup of the hidden input textarea it uses for IME and
// screen-reader input, and its accessibility announcements. Observably,
// both engines left a stray line break sitting in that hidden textarea.
// None of that is Shift+Enter-specific plumbing this file should
// reimplement.
//
// So the chord keeps xterm's Enter path intact and merges at the output
// funnel instead: the keydown handler ARMS the merge (returning `true` so
// xterm processes the Enter normally), and the `term.onData` wrapper joins
// the pending ESC onto the `\r` xterm emits for that very keystroke —
// synchronously, in the same event dispatch. The arm's LIFETIME is that
// dispatch and nothing more: the wiring in terminal.js queues a microtask
// at arm time that expires it, so a chord whose `\r` never materializes
// (xterm swallowed the Enter for a reason of its own) leaves no stale arm
// behind to ESC-prefix some unrelated later Enter. Within the dispatch, a
// non-CR chunk arriving first — xterm can flush a pending IME composition
// commit ahead of the Enter's own `\r` — passes through untouched WITHOUT
// consuming the arm, so the `\r` behind it still gets the prefix. On no
// path is the ESC ever flushed alone: a stray lone ESC is precisely the
// byte shape this design exists to never emit, and a chord that did
// nothing is the safer failure.
//
// A plain shell has no ESC-CR binding of its own; what happens next is
// whatever that shell's line editor does with the sequence (stock
// emacs-mode zsh inserts a newline via self-insert-unmeta; bash's default
// is a no-op) — the same outcomes those shells give the identical bytes
// under Ghostty, which is the parity this feature promises. There is no
// reliable way for the browser side to know what program the pty is
// currently running, so nothing here special-cases shells.
(function () {
  /**
   * Decide what one xterm.js `attachCustomKeyEventHandler` call should do
   * for a Shift+Enter chord, given the KeyboardEvent (or a plain object
   * with the same shape — the node tests never construct a real DOM event).
   *
   * Returns one of:
   *  - `"arm"`: this is the `keydown` of a genuine Shift+Enter. The caller
   *    should arm the one-shot ESC merge (see [`mergeArmedPrefix`]) and
   *    return `true`, letting xterm process the Enter itself; the `\r` it
   *    emits synchronously is what the armed merge attaches the ESC to.
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
   * @returns {"arm"|"pass"}
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
    return isChord ? "arm" : "pass";
  }

  /**
   * Fold an armed Shift+Enter's ESC prefix into the outbound chunk it was
   * armed for, deciding what one `term.onData` chunk should actually send.
   *
   * The contract, stated as the three cases the caller can hit:
   *  - not armed: the chunk passes through untouched — this wrapper must be
   *    invisible to every keystroke and paste that is not the chord;
   *  - armed and the chunk begins with `\r`: this IS the Enter the chord
   *    armed for (xterm emits it synchronously in the same dispatch), so
   *    the ESC is prepended, the whole thing leaves as ONE message — the
   *    single-write guarantee the module header explains — and the arm is
   *    consumed;
   *  - armed and the chunk is anything else: the chunk passes through
   *    untouched and the arm SURVIVES — xterm can flush a pending IME
   *    composition commit ahead of the chord's own `\r`, and consuming the
   *    arm on it would strip the prefix off the `\r` right behind it.
   *
   * Nothing here un-arms on its own in the fizzle case; bounding the arm's
   * lifetime is the CALLER's job (terminal.js expires it in a microtask
   * queued at arm time, i.e. at the end of the same synchronous keydown
   * dispatch — see the module header). Pure on purpose so the node tests
   * exercise the exact merge the page ships.
   *
   * @param {boolean} armed arm state set by an `"arm"` keydown
   * @param {string} data the chunk xterm handed to `term.onData`
   * @returns {{send: string, armed: boolean}}
   */
  function mergeArmedPrefix(armed, data) {
    if (armed && data.startsWith("\r")) {
      return { send: "\x1b" + data, armed: false };
    }
    return { send: data, armed };
  }

  const api = { shiftEnterKeyAction, mergeArmedPrefix };
  if (typeof window !== "undefined") window.farhelmShiftEnterKey = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})();
