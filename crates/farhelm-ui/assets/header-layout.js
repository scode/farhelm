// A tunnel pill changes how much room the header needs, not whether its
// actions should always be small. Measure the normal presentation before
// choosing the compact one, so compacting cannot make its own premise false.
(() => {
  window.__farhelmHeaderLayout?.();
  const header = document.querySelector(".session-topbar");
  const actions = header?.querySelector(".titlebar-actions");
  if (!header || !actions) return;
  let frame = 0;
  let stopped = false;
  let observedPill;

  /** Compare real content with the available row, without adding containment.
   * Measurements are synchronous and restored before paint: the pill may
   * abbreviate its host, but must not conceal the width pressure that caused it.
   * Absolute menus and questions never participate in these flex-item widths.
   */
  const update = () => {
    frame = 0;
    header.classList.remove("actions-compact");
    const pill = header.querySelector(":scope > .session-tunnel-pill");
    if (pill !== observedPill) {
      if (observedPill) resize.unobserve(observedPill);
      if (pill) resize.observe(pill);
      observedPill = pill;
    }
    if (!pill || !pill.getClientRects().length) return;
    const shrink = pill.style.flexShrink;
    pill.style.flexShrink = "0";
    const pillWidth = pill.getBoundingClientRect().width;
    pill.style.flexShrink = shrink;
    if (!pillWidth) return;
    const style = getComputedStyle(header);
    const available = header.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
    // Stale-opened and interrupted sessions can have no terminal strip. Its
    // return creates a new node under the same header, so never retain it.
    const tabs = header.querySelector(":scope > .tab-strip");
    const hasTabs = tabs && tabs.getClientRects().length;
    const tabFloor = hasTabs ? parseFloat(getComputedStyle(tabs).minWidth) : 0;
    const required = actions.getBoundingClientRect().width + pillWidth
      + tabFloor + (hasTabs ? 2 : 1) * parseFloat(style.columnGap);
    header.classList.toggle("actions-compact", required > available);
  };

  /** Batch resize and content changes into one measurement before the next paint. */
  const schedule = () => {
    if (!stopped && !frame) frame = requestAnimationFrame(update);
  };
  const resize = new ResizeObserver(schedule);
  resize.observe(header);
  resize.observe(actions);
  const content = new MutationObserver(schedule);
  content.observe(header, { childList: true, subtree: true, characterData: true });
  document.fonts.ready.then(schedule);
  document.fonts.addEventListener("loadingdone", schedule);
  // SessionView owns this observer, including callbacks already scheduled when
  // switching sessions. A new header also retires the previous owner on mount.
  window.__farhelmHeaderLayout = () => {
    stopped = true;
    cancelAnimationFrame(frame);
    resize.disconnect();
    content.disconnect();
    document.fonts.removeEventListener("loadingdone", schedule);
  };
  update();
})();
