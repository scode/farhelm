// Number shortcuts use the build's client kind, not navigator.platform:
// browsers on a Mac must keep Cmd+digits for their own tab and zoom commands.
(function (root) {
  /** Return a physical digit only for this client's exact shortcut chord.
   * The printed character varies with keyboard layout; keypad and IME input
   * remain the terminal's, while repeat is consumed by the listener below.
   */
  function templateNumberKey(event, macDesktop) {
    if (event.isComposing || !/^Digit[0-9]$/.test(event.code)) return null;
    const chord = macDesktop
      ? event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey
      : event.ctrlKey && event.shiftKey && !event.metaKey && !event.altKey;
    return chord ? event.code.slice(-1) : null;
  }

  /** Capture the chord before xterm, leaving visible modals in control.
   * Rust's list owns launch admission through one hidden trigger per digit.
   * A disabled trigger still consumes the chord so pending launch input never
   * reaches the old agent; repeating a held key never admits another launch.
   */
  function installTemplateNumberShortcuts(macDesktop) {
    if (root.__farhelmTemplateNumberShortcuts) return;
    root.__farhelmTemplateNumberShortcuts = true;
    root.addEventListener("keydown", (event) => {
      const key = templateNumberKey(event, macDesktop);
      if (key === null) return;
      if ([...document.querySelectorAll('[role="dialog"][aria-modal="true"]')]
        .some(node => node.getClientRects().length)) return;
      const trigger = document.querySelector(`.template-number-trigger[data-number-key="${key}"]`);
      if (!trigger) return;
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat && !trigger.disabled) trigger.click();
    }, true);
  }

  root.installTemplateNumberShortcuts = installTemplateNumberShortcuts;
  if (typeof module === "object") module.exports = { templateNumberKey };
})(typeof window === "object" ? window : globalThis);
