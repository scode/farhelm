// Rust owns menu navigation. This synchronous page boundary refuses a move
// already sent across the desktop bridge after focus has independently moved.
(function (root) {
  /** Install once per page; repeated Rust evaluations must retain the epochs.
   * The exported factory lets unit tests use separate pages without globals.
   */
  function installMenuFocus(doc, win) {
    if (win.__farhelmMenuFocus) return win.__farhelmMenuFocus;
    let independent = 0;
    let focusing = null;
    // The ordinary input bridge publishes the current event's source before
    // Rust handles focus-in. Epoch equality alone cannot identify an old but
    // legitimate ungated opening; mounting and key events report no own move.
    win.__farhelmMenuFocusSync = (appOwned = false) => {
      const relay = doc.querySelector("input[data-menu-focus-epoch]");
      if (!relay) return;
      relay.value = JSON.stringify([independent, appOwned]);
      relay.dispatchEvent(new win.Event("input", { bubbles: true }));
    };
    doc.addEventListener("focusin", event => {
      const appOwned = event.target === focusing;
      if (!appOwned) independent++;
      win.__farhelmMenuFocusSync(appOwned);
    }, true);
    doc.addEventListener("keydown", event => {
      if (["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key) &&
        event.target?.closest?.('[role="menu"], [role="menuitem"], [aria-haspopup="menu"]')) {
        win.__farhelmMenuFocusSync();
      }
    }, true);
    /** An opening focus is ungated, but remains app-owned like an arrow move.
     * Gated calls carry the immutable epoch copied by Rust at their key press;
     * a newer key must never make an older call current again.
     * Compare marker values directly: keys are item identities, never selectors
     * or positions. Missing, removed and natively unfocusable items fail closed.
     */
    win.__farhelmMenuFocus = (key, ungated = false, epoch) => {
      if (!ungated && epoch !== independent) return false;
      const target = [...doc.querySelectorAll("[data-menu-focus]")]
        .find(node => node.isConnected && node.getAttribute("data-menu-focus") === key);
      if (!target) return false;
      focusing = target;
      try {
        target.focus();
      } finally {
        focusing = null;
      }
      return doc.activeElement === target;
    };
    return win.__farhelmMenuFocus;
  }
  if (typeof module !== "undefined" && module.exports) module.exports = { installMenuFocus };
  else installMenuFocus(root.document, root);
})(globalThis);
