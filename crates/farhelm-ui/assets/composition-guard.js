// Normalize composition-ending keys before Dioxus or page shortcuts read them.
(function (root) {
  // A recent end is only a compatibility hint. Bound it to 16ms rather
  // than treating the next IME-stamped Enter, however much later, as composing.
  const ENDING_KEY_WINDOW_MS = 16;

  /** Decide whether an otherwise ordinary key belongs to the just-ended edit.
   * Matching the target prevents a composition in one editor from consuming a
   * key in another. keyCode 229 alone is deliberately insufficient: Pinyin can
   * put it on Enter even when no composition has occurred.
   */
  function isEndingKey(ended, event, now) {
    const age = ended ? now - ended.at : -1;
    return !event.isComposing && ended !== null && ended.target === event.target &&
      age >= 0 && age <= ENDING_KEY_WINDOW_MS;
  }

  /** Prefer keeping a real key usable when held composing input is ambiguous.
   * Chromium's ending key and earlier WebKit typing can both remain held when
   * compositionend arrives. Neither target nor elapsed time proves which key
   * caused the end. Declining a hint in both cases spares Chromium rollover,
   * but deliberately leaves WebKit's earlier-held-key case unprotected. Only
   * released earlier typing allows the common WebKit end-before-key trace.
   * Keep one key's release identity, not per-component composition state.
   */
  function createCompositionTracker() {
    let ended = null;
    let composingKey = null;
    return {
      end(target, now) {
        ended = composingKey?.target === target ? null : { target, at: now };
      },
      keydown(event, now) {
        const mark = isEndingKey(ended, event, now);
        ended = null;
        composingKey = event.isComposing ? { target: event.target, code: event.code } : null;
        return mark;
      },
      keyup(event) {
        if (composingKey?.code === event.code) composingKey = null;
        ended = null;
      },
    };
  }

  /** Install once per page, before other capture-phase shortcut listeners.
   * Only the event's presentation to existing guards changes: propagation and
   * default behavior remain theirs. xterm owns its textarea's input-method
   * handling and must never receive this normalization.
   */
  function installCompositionGuard() {
    if (root.__farhelmCompositionGuard) return;
    root.__farhelmCompositionGuard = true;
    const tracker = createCompositionTracker();
    const terminalInput = target => target instanceof Element && target.matches(".xterm-helper-textarea");
    root.addEventListener("compositionend", event => {
      if (!terminalInput(event.target)) tracker.end(event.target, performance.now());
    }, true);
    root.addEventListener("keydown", event => {
      const mark = tracker.keydown(event, performance.now());
      if (mark && !terminalInput(event.target)) Object.defineProperty(event, "isComposing", { value: true });
    }, true);
    root.addEventListener("keyup", event => tracker.keyup(event), true);
  }

  root.installCompositionGuard = installCompositionGuard;
  if (typeof module === "object") module.exports = { createCompositionTracker, isEndingKey, ENDING_KEY_WINDOW_MS };
})(typeof window === "object" ? window : globalThis);
