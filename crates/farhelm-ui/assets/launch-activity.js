// A completed launch may navigate only while the user's last answer still owns
// their attention. Observe physical presses independently of modal openings:
// waking a host and hiding the launcher are not new user choices.
(function (root) {
  const SUBMIT_REPEAT_MS = 500;
  const SUBMIT_POINTER_SLOP = 4;
  const MODIFIERS = new Set([
    "Alt", "AltGraph", "CapsLock", "Control", "Fn", "FnLock", "Hyper", "Meta",
    "NumLock", "ScrollLock", "Shift", "Super", "Symbol", "SymbolLock",
  ]);

  /** One page counter, published before the original event reaches Rust.
   * Reinstalling after an authenticated-tree remount retains the same count.
   * The counter observes events only: it never consumes a key or changes focus.
   */
  function installLaunchActivity(doc, win) {
    if (win.__farhelmLaunchActivity) return win.__farhelmLaunchActivity;
    win.__farhelmLaunchPressCount = 0;
    let previousSubmit = null;

    /** Remember the opening and control whose native submit gesture may repeat.
     * Other launcher buttons and textareas keep their ordinary key meanings;
     * a quick-launch relay click has no physical press and is never counted.
     */
    function submitIdentity(event) {
      const form = event.target?.closest?.('.create-session-form[role="dialog"]');
      if (!form) return null;
      const button = event.target.closest(".create-session-submit");
      if (event.type === "pointerdown" && !button) return null;
      if (event.type === "keydown" &&
        (event.key !== "Enter" || !button &&
          !event.target.matches?.('input:not([type="button"]):not([type="checkbox"]):not([type="radio"])'))) return null;
      return { form, control: button || event.target, opening: form.getAttribute("data-opening-generation") };
    }

    /** Publish a scalar, not one queued record per answer. Desktop's event
     * transport sends this bubbling input before the original captured event;
     * the web renderer handles it synchronously for the same ordering.
     */
    win.__farhelmLaunchActivitySync = () => {
      const relay = doc.querySelector("input[data-launch-activity]");
      if (!relay) return;
      relay.value = JSON.stringify(win.__farhelmLaunchPressCount);
      relay.dispatchEvent(new win.Event("input", { bubbles: true }));
    };
    const read = () => win.__farhelmLaunchPressCount;
    win.__farhelmLaunchActivity = read;

    /** A submitted form hides before an accidental second press can arrive.
     * Match the original gesture, not just its target: the next pointerdown
     * hits the page underneath, and Enter falls to body. Coordinate slop is
     * deliberately small so clicking somewhere else remains a new choice.
     * Keep the first timestamp: ignored repeats must not extend this window.
     */
    function repeatsSubmit(event, own, now) {
      const previous = previousSubmit;
      if (!previous || event.type !== previous.type || now < previous.at ||
        now - previous.at > SUBMIT_REPEAT_MS ||
        previous.form.getAttribute("data-opening-generation") !== previous.opening) return false;
      const sameControl = own && own.form === previous.form &&
        own.control === previous.control && own.opening === previous.opening;
      const hidden = !previous.form.isConnected || previous.form.hidden;
      if (event.type === "pointerdown") {
        if (event.button !== previous.button || !(sameControl || !own && hidden)) return false;
        const pointsKnown = [event.clientX, event.clientY, previous.clientX, previous.clientY]
          .every(Number.isFinite);
        // Native PointerEvent.detail can be zero. Prefer actual coordinates;
        // use the browser's repeat detail only when no point was available.
        return pointsKnown
          ? Math.abs(event.clientX - previous.clientX) <= SUBMIT_POINTER_SLOP &&
            Math.abs(event.clientY - previous.clientY) <= SUBMIT_POINTER_SLOP
          : event.detail > 1;
      }
      return event.key === "Enter" && (sameControl || hidden &&
        (event.target === doc.body || event.target === doc.documentElement ||
          event.target?.nodeType > 0 && previous.form.contains(event.target)));
    }

    /** Observe presses without altering their native effect on the released page.
     * Any unrelated press retires the submit gesture, even if the user returns
     * to its old coordinates or control inside the repeat interval.
     */
    function observe(event) {
      if (event.type === "keydown" && (event.repeat || MODIFIERS.has(event.key))) return;
      const own = submitIdentity(event);
      const now = win.performance.now();
      if (repeatsSubmit(event, own, now)) return;
      previousSubmit = own ? {
        ...own, type: event.type, button: event.button, at: now, clientX: event.clientX, clientY: event.clientY,
      } : null;
      // A wrapped count could make old work current again. Exhaustion is
      // unknown forever instead; it cannot grant navigation authority.
      const count = win.__farhelmLaunchPressCount;
      win.__farhelmLaunchPressCount = Number.isSafeInteger(count) && count >= 0 &&
          count < Number.MAX_SAFE_INTEGER ? count + 1 : null;
      win.__farhelmLaunchActivitySync();
    }
    win.addEventListener("keydown", observe, true);
    win.addEventListener("pointerdown", observe, true);
    return read;
  }

  if (typeof module === "object" && module.exports) module.exports = { installLaunchActivity, SUBMIT_REPEAT_MS };
  else installLaunchActivity(root.document, root);
})(globalThis);
