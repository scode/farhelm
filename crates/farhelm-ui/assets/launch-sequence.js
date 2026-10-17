// Only the synchronous keyboard boundary lives here. Rust owns the ordered
// queue and resolves its template references when the user presses Enter.
(function (root) {
  /** Classify one key without consulting the DOM or changing sequence state.
   * macDesktop governs the prefix and number keys; macPlatform independently
   * governs the existing bolt chord. A Mac browser therefore uses Ctrl+Shift+X
   * and Ctrl+Shift+digits, but Cmd+P. Only an idle prefix excludes text editors.
   * `none` consumes a held recognized key without admitting another action.
   */
  function classifyLaunchSequenceKey(event, armed, macDesktop, macPlatform, ordinaryTextEditor) {
    const pass = { action: "pass", consume: false };
    const cancelPass = { action: "cancel", consume: false };
    if (event.isComposing) return armed ? cancelPass : pass;
    const bare = !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
    const numberChord = macDesktop
      ? event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey
      : event.ctrlKey && event.shiftKey && !event.metaKey && !event.altKey;
    const prefix = event.code === "KeyX" && numberChord;
    if (!armed) {
      if (!prefix || ordinaryTextEditor) return pass;
      return { action: event.repeat ? "none" : "begin", consume: true };
    }

    let action;
    let digit;
    if (/^Digit[0-9]$/.test(event.code) && (bare || numberChord)) {
      action = "digit";
      digit = event.code.slice(-1);
    } else {
      const boltChord = macPlatform
        ? event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey
        : event.ctrlKey && event.shiftKey && !event.metaKey && !event.altKey;
      if (event.code === "KeyP" && boltChord) action = "bolt";
      else if (event.key === "Enter" && bare) action = "launch";
      else if (event.key === "Escape") action = "cancel";
      // A held prefix must not clear the queue it just started. Pressing the
      // prefix anew is otherwise an unrelated key, with normal cancellation.
      else if (prefix && event.repeat) action = "none";
      else if (["Meta", "Control", "Shift"].includes(event.key) && !event.altKey) return pass;
      else return cancelPass;
    }
    if (event.repeat) return { action: "none", consume: true };
    return digit === undefined ? { action, consume: true } : { action, consume: true, digit };
  }

  /** Distinguish real editors from xterm's implementation-detail textarea.
   * isContentEditable includes editable ancestors and respects a nested
   * contenteditable=false boundary. Input and textarea ancestors cover events
   * dispatched from their descendants without remembering a stale focus owner.
   */
  function ordinaryTextEditorTarget(target) {
    if (!(target instanceof Element)) return false;
    if (target.matches(".xterm-helper-textarea")) return false;
    return !!target.closest("input, textarea") || target.isContentEditable;
  }

  /** Install before command listeners so a queued chord cannot launch directly.
   * Relays must remain mounted, and digit/bolt/launch/cancel must stay enabled:
   * prefix, digit and Enter may arrive before Rust's next render. Rust callbacks
   * own live admission checks; rendered disabled state is only an early guard.
   */
  function installLaunchSequence(macDesktop) {
    if (root.__farhelmLaunchSequence) return;
    root.__farhelmLaunchSequence = true;
    const macPlatform = /Mac/.test(navigator.platform || "");
    let armed = false;
    const relaySelector = ".launch-sequence-trigger";
    const modalSelector = '[role="dialog"][aria-modal="true"]';
    const relay = (action, digit) => document.querySelector(
      `${relaySelector}[data-sequence-action="${action}"]` +
      (digit === undefined ? "" : `[data-number-key="${digit}"]`),
    );
    const visibleModal = () => [...document.querySelectorAll(modalSelector)]
      .some(node => node.getClientRects().length);

    /** Retire both state owners before forwarding input or opening a dialog.
     * Idempotence lets focus, pointer and modal-opening hooks share this path.
     * The cancel relay is usable even when new launch admission is disabled.
     */
    function cancel() {
      if (!armed) return;
      armed = false;
      relay("cancel")?.click();
    }
    root.cancelFarhelmLaunchSequence = cancel;

    root.addEventListener("keydown", event => {
      if (visibleModal()) {
        cancel();
        return;
      }
      const result = classifyLaunchSequenceKey(
        event, armed, macDesktop, macPlatform, ordinaryTextEditorTarget(event.target),
      );
      if (result.action === "pass") return;
      // An unmounted application has no sequence keyboard ownership. Disabled
      // Begin still reserves its prefix while a launch is pending.
      const begin = !armed ? relay("begin") : null;
      if (!armed && !begin) return;
      if (result.consume) {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
      if (result.action === "none") return;
      if (result.action === "cancel") {
        cancel();
      } else if (result.action === "begin") {
        if (begin.disabled) return;
        armed = true;
        begin.click();
      } else {
        // Spend keyboard ownership before Launch's callback can open New.
        if (result.action === "launch") armed = false;
        relay(result.action, result.digit)?.click();
      }
    }, true);

    const cancelOnPointer = event => {
      // Relay clicks are synchronous app dispatch, not a user leaving the
      // sequence. Letting Begin's own click cancel would orphan every queue.
      if (event.target instanceof Element && event.target.closest(relaySelector)) return;
      cancel();
    };
    root.addEventListener("pointerdown", cancelOnPointer, true);
    root.addEventListener("click", cancelOnPointer, true);
    root.addEventListener("blur", cancel);
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) cancel();
    });
    root.addEventListener("focusin", event => {
      if (!(event.target instanceof Element)) return;
      const modal = event.target.closest(modalSelector);
      if (modal?.getClientRects().length) cancel();
    }, true);
  }

  root.installLaunchSequence = installLaunchSequence;
  if (typeof module === "object") module.exports = { classifyLaunchSequenceKey, ordinaryTextEditorTarget };
})(typeof window === "object" ? window : globalThis);
