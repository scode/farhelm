// Sidebar width and the expanded host rail belong to the screen in front of
// the user, like terminal text size. Keep storage and DOM handling on the page
// so web and native desktop builds use the same localStorage contract. The
// document-root layout survives authenticated-tree remounts; nothing is sent
// to the helm.
(function () {
  const RAIL = 56;
  const MIN = 296;
  const MAX = 656;
  const DEFAULT = 396;
  const KEY = "farhelm.sidebar-width";
  const RAIL_KEY = "farhelm.host-rail-open";
  // Match app.css's --host-rail-width: 220px expanded minus 56px collapsed.
  const RAIL_EXTRA = 164;

  /** Storage keeps the old content width; adding the rail preserves every
   * existing device choice without a new key or migration marker. */
  function storedWidth(raw) {
    return typeof raw === "string" && /^\d+$/.test(raw)
      ? clampWidth(Number(raw) + RAIL)
      : DEFAULT;
  }

  /** Gestures use total width; storage retains the pre-rail content width. */
  function clampWidth(width) {
    return Math.min(MAX, Math.max(MIN, width));
  }

  /** Only the flag this script writes opts a device into the wider rail.
   * Absent or malformed storage keeps a fresh device collapsed.
   */
  function storedRailOpen(raw) {
    return raw === "true";
  }

  /** Expansion borrows main-pane space without changing the stored width.
   * Keep the shown edge below the width + 321 narrow-window cutoff whenever
   * the stored width already fits. Expansion alone must not turn on either
   * narrow-window class; an already narrow window keeps its stored minimum.
   */
  function shownWidth(width, railOpen, windowWidth) {
    return railOpen ? Math.max(width, Math.min(width + RAIL_EXTRA, windowWidth - 322)) : width;
  }

  if (typeof module !== "undefined") {
    module.exports = { storedWidth, clampWidth, storedRailOpen, shownWidth };
  }
  if (typeof document === "undefined") return;
  if (window.farhelmSidebarWidth) return;

  let width = DEFAULT;
  try {
    width = storedWidth(window.localStorage.getItem(KEY));
  } catch (_) {
    // Storage may be unavailable; resizing still works for this page.
  }
  let railOpen = false;
  try {
    railOpen = storedRailOpen(window.localStorage.getItem(RAIL_KEY));
  } catch (_) {
    // A refused rail preference must not prevent restoring the sidebar width.
  }
  const root = document.documentElement;

  /** Keep the root layout and mounted controls in sync with page-owned state.
   * Media queries cannot use custom properties, so retain their old cutoffs
   * against the shown width. The separator still reports the stored total.
   */
  function updateLayout() {
    const shown = shownWidth(width, railOpen, window.innerWidth);
    root.style.setProperty("--sidebar-width", shown + "px");
    root.classList.toggle("host-rail-open", railOpen);
    root.classList.toggle("sidebar-narrow-window", window.innerWidth <= shown + 321);
    root.classList.toggle("sidebar-menu-overlaps", window.innerWidth <= shown + 261);
    const handle = document.querySelector(".sidebar-resize-handle");
    if (handle) handle.setAttribute("aria-valuenow", String(width));
    const toggle = document.querySelector("#host-rail-toggle");
    if (toggle) {
      const label = railOpen ? "Collapse navigation rail" : "Expand navigation rail";
      toggle.setAttribute("aria-expanded", String(railOpen));
      toggle.setAttribute("aria-label", label);
      toggle.setAttribute("data-tooltip", label);
    }
  }

  /** Persist completed gestures, never every pointer sample in a drag. */
  function remember() {
    try {
      window.localStorage.setItem(KEY, String(width - RAIL));
    } catch (_) {
      // A storage refusal must not turn a layout preference into an error.
    }
  }

  /** Bind the footer independently of the separator's component lifetime.
   * Rust supplies collapsed markup; mounting replaces that default from the
   * page-owned state. Remounting cannot reset the preference or double-bind
   * a click, and no document-wide input listener is needed.
   */
  function mountRailToggle() {
    const toggle = document.querySelector("#host-rail-toggle");
    if (!toggle || toggle.dataset.hostRailBound) return;
    toggle.dataset.hostRailBound = "true";
    updateLayout();
    toggle.addEventListener("click", () => {
      railOpen = !railOpen;
      updateLayout();
      try {
        window.localStorage.setItem(RAIL_KEY, String(railOpen));
      } catch (_) {
        // Storage failure leaves this page's working toggle usable.
      }
    });
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
    /** Cancellation keeps the last stored-width adjustment, like pointer release.
     * The shown edge can stay capped while an expanded rail has too little room.
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
  window.farhelmSidebarWidth = { mount, mountRailToggle };
  updateLayout();
})();
