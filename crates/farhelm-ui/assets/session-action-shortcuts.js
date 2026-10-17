// The build owns the Mac app's Cmd chords; Mac browsers keep their own keys.
(function (root) {
  /** Match only a deliberate session-action chord, independently of focus.
   * Physical letter codes keep keyboard layouts from changing the binding.
   * Repeat is consumed by the installer, without repeating a destructive click.
   */
  function sessionActionKey(event, macDesktop) {
    if (event.isComposing || event.altKey) return null;
    if (macDesktop) {
      if (!event.metaKey || event.ctrlKey || event.shiftKey) return null;
      if (event.code === "KeyW") return "delete";
      if (event.code === "KeyL") return "rename";
    } else {
      if (!event.ctrlKey || !event.shiftKey || event.metaKey) return null;
      if (event.code === "Backspace") return "delete";
      if (event.code === "KeyL") return "rename";
    }
    return null;
  }

  /** Browser Delete remains an editing chord outside xterm's input proxy.
   * The proxy is the terminal program's keyboard entry point, so consuming the
   * chord there is essential. Other controls and editable descendants keep it.
   */
  function sessionActionTextField(target) {
    if (!target?.closest || target.closest(".xterm-helper-textarea")) return false;
    return !!target.closest('input, textarea, select, [role="textbox"]') || !!target.isContentEditable;
  }

  /** Route shortcuts through current controls rather than copying their state.
   * Visible modals retain the keyboard. A busy action still consumes its chord;
   * Delete also consumes it without a session, so it cannot close the Mac app.
   */
  function installSessionActionShortcuts(macDesktop) {
    if (root.__farhelmSessionActionShortcuts) return;
    root.__farhelmSessionActionShortcuts = true;
    root.addEventListener("keydown", (event) => {
      const action = sessionActionKey(event, macDesktop);
      if (action === null) return;
      if ([...document.querySelectorAll('[role="dialog"][aria-modal="true"]')]
        .some(node => node.getClientRects().length)) return;
      if (!macDesktop && action === "delete" && sessionActionTextField(event.target)) return;
      const trigger = document.querySelector(action === "delete" ? ".header-delete" : ".session-rename-trigger");
      if (action === "rename" && !trigger) return;
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat && trigger && !trigger.disabled) {
        // A hidden trigger does not take focus. Rename's mounting handoff
        // may take this exact opener, but must yield to any later focus move.
        if (action === "rename") root.__farhelmSessionRenameOpener = document.activeElement;
        trigger.click();
      }
    }, true);
  }

  root.installSessionActionShortcuts = installSessionActionShortcuts;
  if (typeof module === "object") module.exports = { sessionActionKey, sessionActionTextField };
})(typeof window === "object" ? window : globalThis);
