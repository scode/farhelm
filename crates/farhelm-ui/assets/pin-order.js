// Pointer geometry and temporary drag paint belong to one mounted list.
// The list supplies eligibility from its held display; this asset never
// changes membership, DOM order, or the helm's authoritative pin positions.
(function () {
  /** A drop names a visible neighbor and direction, with the original slot
   * recognized before anything crosses the bridge. The gesture never changes
   * row geometry; incoming listings remain the only source of changed order.
   */
  function dropTarget(rows, source, bounds, x, y) {
    if (x < bounds.left || x >= bounds.right || y < bounds.top || y >= bounds.bottom) return null;
    const others = rows.filter(row => row !== source);
    if (!others.length) return null;
    let neighbor = others.find(row => y < (row.rect.top + row.rect.bottom) / 2);
    const before = Boolean(neighbor);
    if (!neighbor) neighbor = others[others.length - 1];
    const sourceIndex = rows.indexOf(source);
    const neighborIndex = rows.indexOf(neighbor);
    return {
      neighbor,
      before,
      noop: before ? neighborIndex === sourceIndex + 1 : neighborIndex === sourceIndex - 1,
    };
  }

  /** Read only rows the current held display admits; hidden or newly pinned
   * rows are neither sources nor targets, even beside the original pin run.
   */
  function displayedRows(root) {
    return [...root.children]
      .filter(row => row.classList.contains("session-row") && row.dataset.pinDragEligible === "true"
        && row.dataset.sessionPinned === "true" && row.getClientRects().length)
      .map(node => ({ node, id: node.dataset.sessionId, rect: node.getBoundingClientRect() }));
  }

  /** Clip the eligible run to what the user can actually see and release on.
   * `.sidebar-content` is the element that scrolls; `.app-sidebar` around it
   * clips but also holds the app bar and the host attention list above the
   * scroller. Clipping to the scroller keeps pins scrolled out of sight above
   * it from becoming drop targets for a release over those surfaces; clipping
   * to the sidebar and the list too keeps the same contract if either gains
   * its own height constraint later.
   */
  function groupBounds(root, rows, doc) {
    if (!rows.length || root.hidden || !root.getClientRects().length) return null;
    const list = root.getBoundingClientRect();
    const clips = [list, ...[".sidebar-content", ".app-sidebar"]
      .map(selector => root.closest(selector)?.getBoundingClientRect())
      .filter(Boolean)];
    return {
      left: Math.max(...clips.map(rect => rect.left), 0),
      right: Math.min(...clips.map(rect => rect.right), doc.documentElement.clientWidth),
      top: Math.max(rows[0].rect.top, ...clips.map(rect => rect.top), 0),
      bottom: Math.min(rows[rows.length - 1].rect.bottom, ...clips.map(rect => rect.bottom),
        doc.documentElement.clientHeight),
    };
  }

  /** Subscribe once per mount and resolve only when its listeners retire.
   * Dioxus closes eval channels when snippets return, so the awaited promise
   * is the subscription lifetime rather than a never-owned global callback.
   */
  function subscribe(root, send, token) {
    if (!root.isConnected || root.dataset.pinDragMount !== token) return Promise.resolve();
    if (current?.root === root && current.token === token) return current.promise;
    current?.release();
    const doc = root.ownerDocument;
    let drag = null;
    let indicator = null;
    let resolve;
    const promise = new Promise(done => { resolve = done; });

    /** Remove only paint this gesture installed, including a previous target
     * that disappeared or became ineligible during an incoming listing.
     */
    function clearIndicator() {
      if (indicator) delete indicator.dataset.pinDrop;
      indicator = null;
    }

    /** Cancellation owns capture cleanup as well as paint. Clearing state
     * first makes the lost-capture event from release harmless.
     */
    function cancel() {
      if (!drag) return;
      const ended = drag;
      drag = null;
      clearIndicator();
      delete ended.row.dataset.pinDragging;
      delete root.dataset.pinDragging;
      doc.removeEventListener("keydown", escape, true);
      for (const [type, listener] of Object.entries(activeListeners)) doc.removeEventListener(type, listener, true);
      if (ended.grip.hasPointerCapture(ended.pointerId)) ended.grip.releasePointerCapture(ended.pointerId);
    }

    /** Escape belongs to the armed gesture and never sends a partial move. */
    function escape(event) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      cancel();
    }

    /** Re-read current geometry and eligibility before paint and before drop.
     * Capture retargets events to the grip, so event.target cannot say whether
     * release happened inside the pin group.
     */
    function sample(event) {
      clearIndicator();
      if (!drag || !root.isConnected) {
        cancel();
        return null;
      }
      const rows = displayedRows(root);
      const source = rows.find(row => row.node === drag.row);
      const bounds = groupBounds(root, rows, doc);
      if (!source || drag.grip.disabled || !bounds) {
        cancel();
        return null;
      }
      if (!drag.moved && Math.hypot(event.clientX - drag.x, event.clientY - drag.y) >= 4) {
        drag.moved = true;
        drag.row.dataset.pinDragging = "true";
        root.dataset.pinDragging = "true";
      }
      if (!drag.moved) return null;
      const target = dropTarget(rows, source, bounds, event.clientX, event.clientY);
      if (target && !target.noop) {
        indicator = target.neighbor.node;
        indicator.dataset.pinDrop = target.before ? "before" : "after";
      }
      return target;
    }

    /** The grip is a sibling of the open button. Do not stop pointer events:
     * the list's ordinary pointer handler still owns its five-second hold.
     */
    function down(event) {
      if (event.button !== 0 || event.isPrimary === false || drag) return;
      const grip = event.target.closest?.(".session-pin-grip");
      const row = grip?.closest(".session-row");
      if (!row || row.parentElement !== root || grip.disabled
        || !displayedRows(root).some(shown => shown.node === row)) return;
      event.preventDefault();
      grip.focus({ preventScroll: true });
      grip.setPointerCapture(event.pointerId);
      drag = { row, grip, pointerId: event.pointerId, x: event.clientX, y: event.clientY, moved: false };
      doc.addEventListener("keydown", escape, true);
      for (const [type, listener] of Object.entries(activeListeners)) doc.addEventListener(type, listener, true);
    }

    /** A completed drop captures one relative intent before clearing paint. */
    function up(event) {
      if (!drag || event.pointerId !== drag.pointerId) return;
      const target = sample(event);
      const id = drag?.row.dataset.sessionId;
      cancel();
      if (id !== undefined && target && !target.noop) {
        send({ id, neighbor: target.neighbor.id, before: target.before });
      }
    }

    /** Foreign pointers cannot change or finish the captured gesture. */
    function move(event) {
      if (drag && event.pointerId === drag.pointerId) sample(event);
    }

    /** Browser interruption and capture loss are cancellation, never drops. */
    function interrupted(event) {
      if (drag && event.pointerId === drag.pointerId) cancel();
    }

    // A removed capture node can receive lostpointercapture at its document
    // instead. These listeners exist only during this mount's gesture; idle
    // lists own no document pointer listeners and a late release owns nothing.
    const activeListeners = { pointermove: move, pointerup: up,
      pointercancel: interrupted, lostpointercapture: interrupted };
    root.addEventListener("pointerdown", down);
    const subscription = { root, token, promise, cancel, release() {
      cancel();
      root.removeEventListener("pointerdown", down);
      if (current === subscription) current = null;
      resolve();
    } };
    current = subscription;
    return promise;
  }

  let current = null;
  const api = {
    subscribe,
    cancel(token) { if (current?.token === token) current.cancel(); },
    release(token) { if (current?.token === token) current.release(); },
  };
  if (typeof module !== "undefined") module.exports = { dropTarget, displayedRows, groupBounds, ...api };
  if (typeof window !== "undefined" && !window.farhelmPinDrag) window.farhelmPinDrag = api;
})();
