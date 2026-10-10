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

  const update = () => {
    if (!region.isConnected) {
      release();
      return;
    }
    if (!pane?.isConnected) return;
    const bounds = pane.getBoundingClientRect();
    const chrome = pane.querySelector(".tab-strip") || pane.querySelector(".titlebar");
    const top = (chrome ? chrome.getBoundingClientRect().bottom : bounds.top) + 12;
    const width = Math.min(660, Math.max(0, bounds.width - 24));
    // The cap preserves the prompt below the overlay; a tall header must not
    // let a nominal fraction of the pane extend past its remaining space.
    const height = Math.max(0, Math.min(bounds.height * 0.55, bounds.bottom - top - 12));
    Object.assign(region.style, {
      left: `${bounds.left + (bounds.width - width) / 2}px`,
      top: `${top}px`,
      width: `${width}px`,
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
      // Direct chrome bands can change the tab strip's position without
      // changing its own size. Terminal descendants cannot move that anchor,
      // so never observe their output mutations.
      structure.observe(layout, { childList: true });
      for (const child of layout.children) sizes.observe(child);
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
