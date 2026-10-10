// Sidebar width belongs to the screen in front of the user, like terminal
// text size. Keep its storage and DOM handling on the page so web and native
// desktop builds use the same localStorage contract. The document-root value
// survives authenticated-tree remounts; nothing is sent to the helm.
(function () {
  const MIN = 240;
  const MAX = 600;
  const DEFAULT = 340;
  const KEY = "farhelm.sidebar-width";

  /** Accept only the stored decimal form, and bound even oversized values. */
  function storedWidth(raw) {
    return typeof raw === "string" && /^\d+$/.test(raw)
      ? clampWidth(Number(raw))
      : DEFAULT;
  }

  /** All gestures and stored values share the same supported interval. */
  function clampWidth(width) {
    return Math.min(MAX, Math.max(MIN, width));
  }

  if (typeof module !== "undefined") {
    module.exports = { storedWidth, clampWidth };
  }
  if (typeof document === "undefined") return;
  if (window.farhelmSidebarWidth) return;

  let width = DEFAULT;
  try {
    width = storedWidth(window.localStorage.getItem(KEY));
  } catch (_) {
    // Storage may be unavailable; resizing still works for this page.
  }
  const root = document.documentElement;

  /** Media queries cannot use custom properties; preserve their old cutoffs. */
  function updateLayout() {
    root.style.setProperty("--sidebar-width", width + "px");
    root.classList.toggle("sidebar-narrow-window", window.innerWidth <= width + 321);
    root.classList.toggle("sidebar-menu-overlaps", window.innerWidth <= width + 261);
    const handle = document.querySelector(".sidebar-resize-handle");
    if (handle) handle.setAttribute("aria-valuenow", String(width));
  }

  /** Persist completed gestures, never every pointer sample in a drag. */
  function remember() {
    try {
      window.localStorage.setItem(KEY, String(width));
    } catch (_) {
      // A storage refusal must not turn a layout preference into an error.
    }
  }

  /** Bind each mounted handle once, after Rust has rendered it.
   * Pointer capture keeps a drag working outside the handle, without adding
   * document-wide input listeners or changing terminal resize scheduling.
   */
  function mount() {
    const handle = document.querySelector(".sidebar-resize-handle");
    if (!handle || handle.dataset.sidebarBound) return;
    handle.dataset.sidebarBound = "true";
    updateLayout();
    let drag = null;
    handle.addEventListener("pointerdown", (event) => {
      if (event.button !== 0 || drag) return;
      event.preventDefault();
      handle.focus({ preventScroll: true });
      handle.setPointerCapture(event.pointerId);
      drag = { id: event.pointerId, x: event.clientX, width };
      // Tie transient styling to this mounted handle. Removing the tree
      // during a drag cannot leave a resize cursor on the login screen.
      handle.dataset.resizing = "true";
    });
    handle.addEventListener("pointermove", (event) => {
      if (!drag || event.pointerId !== drag.id) return;
      width = clampWidth(Math.round(drag.width + event.clientX - drag.x));
      updateLayout();
    });
    /** Cancellation keeps the last visible width, just like pointer release.
     * Capture loss also ends the gesture, so its styling and storage agree
     * when the browser interrupts input rather than delivering pointerup.
     */
    function finish(event) {
      if (!drag || event.pointerId !== drag.id) return;
      drag = null;
      delete handle.dataset.resizing;
      remember();
    }
    handle.addEventListener("pointerup", finish);
    handle.addEventListener("pointercancel", finish);
    handle.addEventListener("lostpointercapture", finish);
    handle.addEventListener("dblclick", (event) => {
      event.preventDefault();
      width = DEFAULT;
      updateLayout();
      remember();
    });
    handle.addEventListener("keydown", (event) => {
      if (event.isComposing || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
      event.preventDefault();
      event.stopPropagation();
      width = clampWidth(width + (event.key === "ArrowRight" ? 10 : -10));
      updateLayout();
      remember();
    });
  }

  window.addEventListener("resize", updateLayout);
  window.farhelmSidebarWidth = { mount };
  updateLayout();
})();
