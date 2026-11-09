// Approval cards remain siblings of the shell so modal isolation leaves them
// answerable. Follow the pane's real rectangle instead of relocating them into
// an inert subtree or baking the sidebar/header dimensions into CSS.
(() => {
  const region = document.querySelector(".approval-cards");
  if (!region || region.__farhelmApprovalLayout) return;
  region.__farhelmApprovalLayout = true;
  const owner = region.parentElement;
  let pane = null;
  let layout = null;
  let released = false;

  // Measure only space that cannot surrender height to the card body. The
  // body's current flex allocation is an output, so including it here would
  // make the fallback depend on the very cap it is choosing.
  const pinnedMinimum = () => {
    const px = (value) => parseFloat(value) || 0;
    const frame = (style) =>
      px(style.paddingTop) + px(style.paddingBottom) + px(style.borderTopWidth) + px(style.borderBottomWidth);
    const outerHeight = (element) => {
      const style = getComputedStyle(element);
      return element.getBoundingClientRect().height + px(style.marginTop) + px(style.marginBottom);
    };
    const occupants = Array.from(region.children).filter((child) => getComputedStyle(child).display !== "none");
    const regionStyle = getComputedStyle(region);
    let minimum = frame(regionStyle) + Math.max(0, occupants.length - 1) * px(regionStyle.rowGap);
    for (const occupant of occupants) {
      if (!occupant.matches(".approval-card")) {
        // This includes the capped waiting list and a late notice even when
        // no request remains selected. Neither can give its height to a card.
        minimum += outerHeight(occupant);
        continue;
      }
      const style = getComputedStyle(occupant);
      const rows = Array.from(occupant.children).filter((child) => getComputedStyle(child).display !== "none");
      minimum += frame(style) + px(style.marginTop) + px(style.marginBottom)
        + Math.max(0, rows.length - 1) * px(style.rowGap);
      for (const row of rows) {
        if (!row.matches(".approval-card-body")) minimum += outerHeight(row);
      }
    }
    // Fractional font metrics must not leave the last answer fractionally
    // clipped. All card gaps remain necessary even when the body reaches zero.
    return Math.ceil(minimum);
  };

  const update = () => {
    if (!region.isConnected) {
      release();
      return;
    }
    if (!pane?.isConnected) return;
    const bounds = pane.getBoundingClientRect();
    // Tabs now share the top row with actions. Notices and tunnel chips can
    // follow that row, so only the terminal area's top clears all of them.
    const terminal = pane.querySelector(".terminal-panes");
    const chrome = pane.querySelector(".session-topbar");
    let top = (terminal
      ? terminal.getBoundingClientRect().top
      : chrome ? chrome.getBoundingClientRect().bottom : bounds.top) + 12;
    const width = Math.min(660, Math.max(0, bounds.width - 24));
    // Fixed rows must wrap at the intended pane width before measuring them,
    // including the first hidden mount and a sidebar/window resize.
    Object.assign(region.style, {
      left: `${bounds.left + (bounds.width - width) / 2}px`,
      width: `${width}px`,
    });
    // The cap preserves the prompt below the overlay; a tall header must not
    // let a nominal fraction of the pane extend past its remaining space.
    let height = Math.max(0, Math.min(bounds.height * 0.75, bounds.bottom - top - 12));
    // Include every fixed region occupant, not just the card: a notice and
    // waiting headers can exhaust the space left for its pinned answer bar.
    const minimum = pinnedMinimum();
    if (height < minimum) {
      top = 0;
      height = window.innerHeight;
    }
    // Even pinned chrome cannot fit an extremely short viewport. That one
    // fallback keeps the old whole-region scroll so every answer is reachable.
    region.dataset.tinyPane = String(height < minimum);
    Object.assign(region.style, {
      top: `${top}px`,
      maxHeight: `${height}px`,
      visibility: "visible",
    });
  };
  const sizes = new ResizeObserver(update);
  const structure = new MutationObserver(() => bind());
  const bind = () => {
    if (!region.isConnected) {
      release();
      return;
    }
    pane = document.querySelector(".app-main");
    layout = pane?.querySelector(":scope > .layout");
    sizes.disconnect();
    structure.disconnect();
    if (pane) {
      sizes.observe(pane);
      // At the pane's minimum width, resizing the sidebar changes the pane's
      // position without changing its size. That sibling is therefore a
      // geometry input even though terminal descendants are not.
      const sidebar = pane.parentElement?.querySelector(":scope > .app-sidebar");
      if (sidebar) sizes.observe(sidebar);
      structure.observe(pane, { childList: true });
    }
    if (layout) {
      // Direct chrome bands can move the terminal area's top without
      // changing the pane's own size. Terminal descendants cannot move that anchor,
      // so never observe their output mutations.
      structure.observe(layout, { childList: true });
      for (const child of layout.children) sizes.observe(child);
    }
    // Rebind when a request, notice or waiting list appears or is replaced.
    // Their fixed rows can resize with peer text or loaded fonts. Observing
    // only those rows avoids feeding body/region height changes back into the
    // cap; direct-child mutations likewise exclude terminal and text output.
    structure.observe(region, { childList: true });
    for (const occupant of region.children) {
      if (occupant.matches(".approval-card")) {
        structure.observe(occupant, { childList: true });
        for (const row of occupant.children) {
          if (!row.matches(".approval-card-body")) sizes.observe(row);
        }
      } else {
        sizes.observe(occupant);
      }
    }
    update();
  };
  const onScroll = (event) => {
    if (event.target instanceof Element && event.target.matches(".app-shell")) update();
  };
  // The owning sibling list covers unmount and the initial shell mount. Its
  // subtree is intentionally excluded: card answers and terminal output do
  // not create a new positioning lifecycle.
  const lifetime = new MutationObserver(() => bind());
  const release = () => {
    if (released) return;
    released = true;
    sizes.disconnect();
    structure.disconnect();
    lifetime.disconnect();
    document.removeEventListener("scroll", onScroll, true);
    window.removeEventListener("resize", update);
  };
  lifetime.observe(owner, { childList: true });
  document.addEventListener("scroll", onScroll, true);
  window.addEventListener("resize", update);
  bind();
})();
