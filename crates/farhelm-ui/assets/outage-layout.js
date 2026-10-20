// Session notices can exhaust the terminal at any window height.
// 80px leaves a line plus padding; the session disconnects this observer on exit.
(() => {
  window.__farhelmOutageLayout?.disconnect();
  const pane = document.querySelector(".terminal-panes");
  if (!pane) return;
  const update = () => pane.classList.toggle("outage-viewport", pane.getBoundingClientRect().height < 80);
  window.__farhelmOutageLayout = new ResizeObserver(update);
  window.__farhelmOutageLayout.observe(pane);
  update();
})();
