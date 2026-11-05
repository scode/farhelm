const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const source = fs.readFileSync(require.resolve("../assets/pin-order.js"), "utf8");

/** Model capture and listeners explicitly, so cancellation proves cleanup
 * rather than succeeding because the mock silently discarded its handlers.
 * Geometry is mutable: each sample must consult today's displayed rows.
 */
function fixture(t, ids = ["A", "B", "C"]) {
  /** Listener sets expose duplicate subscription and actual removal while
   * preserving synchronous lost-capture delivery during cleanup.
   */
  function events(parent) {
    const listeners = new Map();
    return {
      listeners,
      addEventListener(type, listener) {
        if (!listeners.has(type)) listeners.set(type, new Set());
        listeners.get(type).add(listener);
      },
      removeEventListener(type, listener) { listeners.get(type)?.delete(listener); },
      dispatch(type, event) {
        for (const listener of [...listeners.get(type) ?? []]) listener(event);
      },
      emit(type, fields = {}) {
        const event = { button: 0, isPrimary: true, pointerId: 1, clientX: 6, clientY: 20,
          preventDefault() { this.prevented = true; },
          stopPropagation() { this.stopped = true; }, ...fields };
        parent?.dispatch(type, event);
        this.dispatch(type, event);
        return event;
      },
    };
  }
  const doc = { ...events(), documentElement: { clientWidth: 500, clientHeight: 300 } };
  const sidebar = { rect: { left: 0, right: 300, top: 0, bottom: 300 },
    getBoundingClientRect() { return this.rect; } };
  // The scroller inside the sidebar; surfaces such as the host attention list
  // sit above it, inside the sidebar, when a test lowers its top edge.
  const content = { rect: { left: 0, right: 300, top: 0, bottom: 300 },
    getBoundingClientRect() { return this.rect; } };
  /** Each mount needs fresh nodes, grip ownership and listeners. Shallow
   * cloning a list would hide the very replacement-ownership bug under test.
   */
  function makeRoot(token) {
    const root = { ...events(doc), ownerDocument: doc, isConnected: true, hidden: false,
      dataset: { pinDragMount: token },
      rect: { left: 0, right: 300, top: 0, bottom: 500 },
      closest: selector => selector === ".app-sidebar" ? sidebar
        : selector === ".sidebar-content" ? content : null,
      getBoundingClientRect() { return this.rect; },
      getClientRects() { return this.hidden ? [] : [this.rect]; } };
    root.children = ids.map((id, index) => {
      const row = { dataset: { sessionId: id, sessionPinned: "true", pinDragEligible: "true" },
        classList: { contains: name => name === "session-row" }, parentElement: root,
        rect: { left: 0, right: 300, top: index * 40, bottom: (index + 1) * 40 }, visible: true,
        getBoundingClientRect() { return this.rect; },
        getClientRects() { return this.visible ? [this.rect] : []; } };
      const captured = new Set();
      row.grip = { disabled: false, focused: false,
        closest: selector => selector === ".session-pin-grip" ? row.grip
          : selector === ".session-row" ? row : null,
        focus() { this.focused = true; },
        setPointerCapture(id) { captured.add(id); },
        hasPointerCapture: id => captured.has(id),
        releasePointerCapture(id) {
          captured.delete(id);
          root.emit("lostpointercapture", { pointerId: id });
        } };
      return row;
    });
    return root;
  }
  const root = makeRoot("first");
  const window = {};
  vm.runInNewContext(source, { window });
  const api = window.farhelmPinDrag;
  const sent = [];
  const send = intent => sent.push(JSON.parse(JSON.stringify(intent)));
  const promise = api.subscribe(root, send, "first");
  t.after(() => api.release("first"));
  return { root, doc, sidebar, content, api, sent, send, promise, makeRoot,
    press(index = 0, fields = {}) {
      const row = root.children[index];
      return root.emit("pointerdown", { target: row.grip, clientY: row.rect.top + 20, ...fields });
    },
    move(y, fields = {}) { return root.emit("pointermove", { clientY: y, ...fields }); },
    up(y, fields = {}) { return root.emit("pointerup", { clientY: y, ...fields }); } };
}

/** A finished/cancelled gesture must leave no cursor, line, capture or key
 * listener, including when a release itself emits lostpointercapture.
 */
function assertClean(f) {
  assert.equal(f.root.dataset.pinDragging, undefined);
  for (const type of ["keydown", "pointermove", "pointerup", "pointercancel", "lostpointercapture"]) {
    assert.equal(f.doc.listeners.get(type)?.size ?? 0, 0, `${type} must be removed after the gesture`);
  }
  for (const row of f.root.children) {
    assert.equal(row.dataset.pinDragging, undefined);
    assert.equal(row.dataset.pinDrop, undefined);
    assert.equal(row.grip.hasPointerCapture(1), false);
  }
}

/** Distinct target and direction assertions detect accidental index-based
 * moves or a drop implementation that always uses the same side.
 */
test("drop preserves full raw neighbor identity and before/after direction", t => {
  const ids = ['A"\\[]', "host::B", "C\n"];
  const f = fixture(t, ids);
  const press = f.press(0);
  assert.equal(press.prevented, true);
  assert.equal(press.stopped, undefined, "list activity must still receive the pointer");
  assert.equal(f.root.children[0].grip.focused, true);
  assert.equal(f.move(110).stopped, undefined);
  assert.equal(f.root.children[0].dataset.pinDragging, "true");
  assert.equal(f.root.children[2].dataset.pinDrop, "after");
  assert.deepEqual(f.root.children.map(row => row.dataset.sessionId), ids, "drag never reorders DOM");
  f.up(110);
  assert.deepEqual(f.sent, [{ id: ids[0], neighbor: ids[2], before: false }]);
  assertClean(f);

  f.press(2);
  f.move(5);
  assert.equal(f.root.children[0].dataset.pinDrop, "before");
  f.up(5);
  assert.deepEqual(f.sent[1], { id: ids[2], neighbor: ids[0], before: true });
  assertClean(f);
});

/** A grip click, sub-threshold motion, the original slot and a lone pin
 * cannot turn an unchanged display into an unnecessary helm mutation.
 */
test("click, original-slot drop and a lone pin are no-ops", t => {
  const f = fixture(t);
  for (const y of [20, 22, 24, 35, 55]) {
    f.press(0);
    f.move(y);
    assert.equal(f.root.children[0].dataset.pinDragging, Math.abs(y - 20) >= 4 ? "true" : undefined);
    assert.equal(f.root.children.some(row => row.dataset.pinDrop), false);
    f.up(y);
    assertClean(f);
  }
  assert.deepEqual(f.sent, []);
  const lone = fixture(t, ["only"]);
  lone.press();
  lone.move(35);
  assert.equal(lone.root.children[0].dataset.pinDragging, "true");
  lone.up(35);
  assert.deepEqual(lone.sent, []);
  assertClean(lone);
});

/** Pointer capture can report an inside event target for an outside release;
 * bounds must include the pin run, horizontal edges and the visible viewport.
 */
test("outside-group and clipped releases cancel even with captured input", t => {
  const f = fixture(t);
  for (const point of [{ clientY: -1 }, { clientY: 120 }, { clientX: -1, clientY: 110 },
    { clientX: 300, clientY: 110 }]) {
    f.press();
    f.move(110);
    f.up(110, point);
    assertClean(f);
  }
  f.sidebar.rect.bottom = 80;
  f.press();
  f.move(70);
  f.up(100);
  assertClean(f);
  f.sidebar.rect.bottom = 300;
  f.doc.documentElement.clientHeight = 80;
  f.press();
  f.up(100);
  assertClean(f);
  f.doc.documentElement.clientHeight = 300;
  // A surface above the scroller (the host attention list) hides the first
  // pin; a release over it is outside the group, not a drop beside that pin.
  f.content.rect.top = 50;
  f.press(2);
  f.move(20);
  f.up(20);
  assertClean(f);
  assert.deepEqual(f.sent, []);
});

/** New held-tail pins remain ineligible as both sources and targets, and
 * changing membership or source locks during capture cancels the drop.
 */
test("each sample uses current eligibility and source locks", t => {
  const f = fixture(t);
  f.root.children[2].dataset.pinDragEligible = "false";
  f.press(2);
  assert.equal(f.root.children[2].grip.hasPointerCapture(1), false);
  f.press(0);
  f.move(110);
  f.up(110);
  assert.deepEqual(f.sent, []);
  assertClean(f);
  f.root.children[2].dataset.pinDragEligible = "true";
  for (const change of [
    () => { f.root.children[0].dataset.pinDragEligible = "false"; },
    () => { f.root.children[0].dataset.sessionPinned = "false"; },
    () => { f.root.children[0].grip.disabled = true; },
    () => { f.root.children[0].visible = false; },
  ]) {
    const row = f.root.children[0];
    row.dataset.pinDragEligible = "true";
    row.dataset.sessionPinned = "true";
    row.grip.disabled = false;
    row.visible = true;
    f.press(0);
    f.move(110);
    change();
    f.up(110);
    assertClean(f);
  }
  assert.deepEqual(f.sent, []);
});

/** Targets remain valid through unrelated operation locks. Only source
 * admission is restricted, matching the existing relative menu moves.
 */
test("a locked visible target stays a neighbor and current geometry is read", t => {
  const f = fixture(t);
  f.root.children[2].grip.disabled = true;
  f.press(0);
  f.move(110);
  f.root.children[2].rect = { left: 0, right: 300, top: 70, bottom: 90 };
  f.up(85);
  assert.deepEqual(f.sent, [{ id: "A", neighbor: "C", before: false }]);
  assertClean(f);
});

/** Disconnecting a captured row removes its former ancestors from event
 * delivery. Document-targeted capture loss must still clear that row's paint,
 * the list cursor and every gesture listener before a later outside release.
 */
test("capture loss delivered at document cancels a removed source row", t => {
  const f = fixture(t);
  f.press();
  f.move(110);
  const removed = f.root.children.shift();
  removed.parentElement = null;
  f.doc.emit("lostpointercapture");
  assertClean(f);
  assert.equal(removed.dataset.pinDragging, undefined);
  assert.equal(removed.grip.hasPointerCapture(1), false);
  f.doc.emit("pointerup", { clientX: 350, clientY: 110 });
  assert.deepEqual(f.sent, []);
});

/** Every cancellation path clears the active gesture and sends nothing;
 * foreign input cannot steal or complete the primary pointer's capture.
 */
test("Escape, interruption, hiding and teardown cancel without a move", async t => {
  const f = fixture(t);
  for (const cancel of [
    () => f.doc.emit("keydown", { key: "Escape" }),
    () => f.root.emit("pointercancel"),
    () => f.root.emit("lostpointercapture"),
    () => { f.root.hidden = true; f.up(110); f.root.hidden = false; },
    () => { f.root.isConnected = false; f.up(110); f.root.isConnected = true; },
    () => f.api.cancel("first"),
  ]) {
    f.press();
    f.move(110);
    cancel();
    assertClean(f);
    f.up(110);
  }
  for (const fields of [{ button: 2 }, { isPrimary: false }]) {
    f.press(0, fields);
    assertClean(f);
  }
  f.press();
  f.move(110, { pointerId: 2 });
  f.up(110, { pointerId: 2 });
  assert.equal(f.root.children[0].grip.hasPointerCapture(1), true);
  assert.equal(f.root.dataset.pinDragging, undefined);
  f.move(110);
  f.api.release("first");
  await f.promise;
  assertClean(f);
  for (const listeners of f.root.listeners.values()) assert.equal(listeners.size, 0);
  f.up(110);
  assert.deepEqual(f.sent, []);
});

/** Mount identity protects a successor from late release, duplicate mount
 * hooks, and a detached predecessor whose setup finishes after replacement.
 * The native pin_drag_list_recovery regression proves the production list
 * creates these distinct root tokens while its surrounding parent survives.
 */
test("duplicate mount and delayed old cleanup cannot disable a replacement", async t => {
  const f = fixture(t);
  assert.equal(f.api.subscribe(f.root, f.send, "first"), f.promise);
  assert.equal(f.root.listeners.get("pointerdown").size, 1);
  f.press();
  f.move(110);
  const next = f.makeRoot("second");
  const replacement = f.api.subscribe(next, f.send, "second");
  await f.promise;
  assertClean(f);
  f.api.release("first");
  assert.equal(next.listeners.get("pointerdown").size, 1);
  delete f.root.dataset.pinDragMount;
  await f.api.subscribe(f.root, f.send, "first");
  assert.equal(next.listeners.get("pointerdown").size, 1,
    "retirement before DOM removal also rejects delayed setup");
  f.root.isConnected = false;
  await f.api.subscribe(f.root, f.send, "first");
  assert.equal(next.listeners.get("pointerdown").size, 1);
  next.emit("pointerdown", { target: next.children[2].grip, clientY: 100 });
  next.emit("pointermove", { clientY: 5 });
  next.emit("pointerup", { clientY: 5 });
  assert.deepEqual(f.sent, [{ id: "C", neighbor: "A", before: true }],
    "the successor must still deliver exactly one real move after old cleanup");
  f.api.release("second");
  await replacement;
  for (const listeners of next.listeners.values()) assert.equal(listeners.size, 0);
});
