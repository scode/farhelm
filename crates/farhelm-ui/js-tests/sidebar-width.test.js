const assert = require("node:assert/strict");
const test = require("node:test");
const fs = require("node:fs");
const vm = require("node:vm");
const { storedWidth, clampWidth, storedRailOpen, shownWidth } = require("../assets/sidebar-width.js");
const source = fs.readFileSync(require.resolve("../assets/sidebar-width.js"), "utf8");
const WIDTH_KEY = "farhelm.sidebar-width";
const RAIL_KEY = "farhelm.host-rail-open";

// Bad browser storage must not change a fresh profile's layout or accept
// the permissive number formats that Number() alone would silently parse.
test("stored sidebar width requires plain decimal digits", () => {
  for (const value of [null, "", " 400", "400 ", "-240", "+400", "3e2", "396.5", "NaN", "x"]) {
    assert.equal(storedWidth(value), 396);
  }
  assert.equal(storedWidth("420"), 476);
  assert.equal(storedWidth("00420"), 476);
});

// A remembered width from a larger display remains usable on this device;
// huge decimal strings also clamp instead of leaking Infinity into CSS.
test("stored widths clamp at both bounds", () => {
  assert.equal(storedWidth("0"), 296);
  assert.equal(storedWidth("240"), 296);
  assert.equal(storedWidth("600"), 656);
  assert.equal(storedWidth("999999"), 656);
  assert.equal(storedWidth("9".repeat(400)), 656);
  assert.equal(clampWidth(200), 296);
  assert.equal(clampWidth(700), 656);
});

/** A fresh device and corrupt storage must not opt into an expanded rail. */
test("only the exact persisted true flag expands the rail", () => {
  for (const value of [null, undefined, "", "false", "TRUE", " true ", "1", true, 1, {}]) {
    assert.equal(storedRailOpen(value), false);
  }
  assert.equal(storedRailOpen("true"), true);
});

/** The rail borrows only available pane space; the saved width is its floor. */
test("shown width preserves the list when room allows and caps expansion otherwise", () => {
  assert.equal(shownWidth(396, false, 1200), 396);
  assert.equal(shownWidth(396, true, 1200), 560);
  assert.equal(shownWidth(656, true, 1400), 820);
  assert.equal(shownWidth(396, true, 800), 478);
  assert.equal(shownWidth(396, true, 718), 396);
  assert.equal(shownWidth(396, true, 500), 396);
});

/** Expansion must not activate narrow-window behavior on either cutoff edge.
 * Include both neighbors of each cutoff so an off-by-one cap cannot pass.
 */
test("expanding alone preserves both width-derived root class decisions", () => {
  for (const width of [296, 396, 656]) {
    for (const viewport of [1, width + 260, width + 261, width + 262,
      width + 320, width + 321, width + 322, width + 323, width + 485, 1600]) {
      const shown = shownWidth(width, true, viewport);
      assert.equal(viewport <= shown + 321, viewport <= width + 321);
      assert.equal(viewport <= shown + 261, viewport <= width + 261);
    }
  }
});

/** Model element lifetime and browser boundaries, leaving every state change
 * to the shipped script. Controls are absent at script load, as before Rust
 * mounts; a second fixture can reuse storage without sharing a page or DOM.
 */
function browserFixture(options = {}) {
  const values = options.values ?? new Map();
  const reads = [];
  const writes = [];
  const classes = new Set();
  const properties = new Map();
  const windowListeners = new Map();
  const document = {
    handle: null,
    toggle: null,
    documentElement: {
      style: { setProperty: (key, value) => properties.set(key, value) },
      classList: {
        toggle(name, enabled) {
          if (enabled) classes.add(name);
          else classes.delete(name);
        },
      },
    },
    querySelector(selector) {
      if (selector === ".sidebar-resize-handle") return this.handle;
      if (selector === "#host-rail-toggle") return this.toggle;
      assert.fail("unexpected selector: " + selector);
    },
    addEventListener() {
      assert.fail("sidebar controls must not install document input listeners");
    },
  };
  const storage = {
    getItem(key) {
      reads.push(key);
      if (options.refuseRead) throw new Error("storage read refused");
      return values.get(key) ?? null;
    },
    setItem(key, value) {
      writes.push([key, value]);
      if (options.refuseWrite) throw new Error("storage write refused");
      values.set(key, value);
    },
  };
  const window = {
    innerWidth: options.viewport ?? 1200,
    get localStorage() {
      if (options.refuseStorage) throw new Error("storage access refused");
      return storage;
    },
    addEventListener(type, listener) {
      if (!windowListeners.has(type)) windowListeners.set(type, []);
      windowListeners.get(type).push(listener);
    },
  };
  /** Retain registered listeners so duplicate mounts are observable, rather
   * than overwritten by a mock that would conceal double toggling.
   */
  function element() {
    const attributes = new Map();
    const listeners = new Map();
    return {
      attributes, listeners, dataset: {},
      setAttribute: (name, value) => attributes.set(name, value),
      addEventListener(type, listener) {
        if (!listeners.has(type)) listeners.set(type, []);
        listeners.get(type).push(listener);
      },
      focus() {},
      setPointerCapture() {},
      emit(type, fields = {}) {
        const event = { preventDefault() {}, stopPropagation() {}, ...fields };
        for (const listener of listeners.get(type) ?? []) listener(event);
      },
    };
  }
  return {
    window, document, values, reads, writes, classes, properties, windowListeners,
    install: () => vm.runInNewContext(source, { window, document }),
    mountToggle() {
      document.toggle = element();
      window.farhelmSidebarWidth.mountRailToggle();
      return document.toggle;
    },
    mountHandle() {
      document.handle = element();
      window.farhelmSidebarWidth.mount();
      return document.handle;
    },
    resize(viewport) {
      window.innerWidth = viewport;
      for (const listener of windowListeners.get("resize") ?? []) listener();
    },
  };
}

/** The visible state, accessible name and tooltip describe the same action. */
function assertRail(fixture, expanded, width) {
  assert.equal(fixture.classes.has("host-rail-open"), expanded);
  assert.equal(fixture.properties.get("--sidebar-width"), width + "px");
  if (fixture.document.toggle) {
    const attributes = fixture.document.toggle.attributes;
    assert.equal(attributes.get("aria-expanded"), String(expanded));
    const label = expanded ? "Collapse navigation rail" : "Expand navigation rail";
    assert.equal(attributes.get("aria-label"), label);
    assert.equal(attributes.get("data-tooltip"), label);
  }
}

/** Restore before renderer content exists, without persisting default values. */
test("page load restores root layout before the footer or separator mounts", () => {
  const fresh = browserFixture();
  fresh.install();
  assert.equal(fresh.document.toggle, null);
  assert.equal(fresh.document.handle, null);
  assertRail(fresh, false, 396);
  assert.deepEqual(fresh.reads, [WIDTH_KEY, RAIL_KEY]);
  assert.deepEqual(fresh.writes, []);

  const restored = browserFixture({ values: new Map([[WIDTH_KEY, "420"], [RAIL_KEY, "true"]]) });
  restored.install();
  assert.equal(restored.document.toggle, null);
  assertRail(restored, true, 640);
  restored.mountToggle();
  const handle = restored.mountHandle();
  assertRail(restored, true, 640);
  assert.equal(handle.attributes.get("aria-valuenow"), "476");
});

/** A duplicate asset or repeated mounted hook must never make one click flip
 * twice. Replacement controls inherit the page's state without rereading it.
 */
test("toggle binding is element-local and survives renderer remounts", () => {
  const fixture = browserFixture();
  fixture.install();
  fixture.window.farhelmSidebarWidth.mountRailToggle();
  const first = fixture.mountToggle();
  fixture.window.farhelmSidebarWidth.mountRailToggle();
  fixture.install();
  assert.equal(first.listeners.get("click").length, 1);
  assert.equal(fixture.windowListeners.get("resize").length, 1);
  assert.deepEqual(fixture.reads, [WIDTH_KEY, RAIL_KEY]);
  assertRail(fixture, false, 396);
  first.emit("click");
  assertRail(fixture, true, 560);
  assert.deepEqual(fixture.writes, [[RAIL_KEY, "true"]]);

  fixture.document.toggle = null;
  const replacement = fixture.mountToggle();
  assertRail(fixture, true, 560);
  assert.equal(replacement.listeners.get("click").length, 1);
  replacement.emit("click");
  assertRail(fixture, false, 396);
  assert.deepEqual(fixture.writes, [[RAIL_KEY, "true"], [RAIL_KEY, "false"]]);
});

/** Only a new page reads storage; a separate device still starts collapsed. */
test("expanded preference survives reload and stays local to its device storage", () => {
  const fixture = browserFixture();
  fixture.install();
  fixture.mountToggle().emit("click");
  assertRail(fixture, true, 560);
  const reload = browserFixture({ values: fixture.values });
  reload.install();
  reload.mountToggle();
  assertRail(reload, true, 560);
  const otherDevice = browserFixture();
  otherDevice.install();
  otherDevice.mountToggle();
  assertRail(otherDevice, false, 396);
});

/** Resizing the window reapplies the cap without changing the user's saved
 * choice. Expansion alone leaves the narrow-window modes where they were.
 */
test("window resize adjusts shown width without storing or collapsing the rail", () => {
  const fixture = browserFixture({ values: new Map([[RAIL_KEY, "true"]]) });
  fixture.install();
  fixture.mountToggle();
  for (const [viewport, shown, narrow, overlaps] of [
    [800, 478, false, false], [718, 396, false, false],
    [717, 396, true, false], [657, 396, true, true], [1200, 560, false, false],
  ]) {
    fixture.resize(viewport);
    assertRail(fixture, true, shown);
    assert.equal(fixture.classes.has("sidebar-narrow-window"), narrow);
    assert.equal(fixture.classes.has("sidebar-menu-overlaps"), overlaps);
  }
  assert.deepEqual(fixture.writes, []);
});

/** Separator input continues to edit and announce the stored total, even when
 * expansion makes the visible sidebar wider. Rail clicks write only its flag.
 */
test("keyboard reset and drag retain the pre-expansion storage and aria contract", () => {
  const fixture = browserFixture();
  fixture.install();
  const handle = fixture.mountHandle();
  fixture.window.farhelmSidebarWidth.mount();
  assert.equal(handle.listeners.get("keydown").length, 1);
  fixture.mountToggle().emit("click");
  assert.equal(handle.attributes.get("aria-valuenow"), "396");

  handle.emit("keydown", { key: "ArrowRight" });
  assertRail(fixture, true, 570);
  assert.equal(handle.attributes.get("aria-valuenow"), "406");
  assert.equal(fixture.values.get(WIDTH_KEY), "350");
  handle.emit("keydown", { key: "ArrowLeft", isComposing: true });
  assert.equal(handle.attributes.get("aria-valuenow"), "406");
  handle.emit("dblclick");
  assertRail(fixture, true, 560);
  assert.equal(handle.attributes.get("aria-valuenow"), "396");
  assert.equal(fixture.values.get(WIDTH_KEY), "340");

  fixture.resize(800);
  assertRail(fixture, true, 478);
  const writesBeforeDrag = fixture.writes.length;
  handle.emit("pointerdown", { button: 0, pointerId: 1, clientX: 478 });
  handle.emit("pointermove", { pointerId: 1, clientX: 498 });
  assertRail(fixture, true, 478);
  assert.equal(handle.attributes.get("aria-valuenow"), "416");
  assert.equal(fixture.writes.length, writesBeforeDrag);
  handle.emit("pointerup", { pointerId: 1 });
  assert.equal(fixture.values.get(WIDTH_KEY), "360");
  assert.equal(fixture.values.get(RAIL_KEY), "true");
  fixture.document.toggle.emit("click");
  assertRail(fixture, false, 416);
  assert.equal(handle.attributes.get("aria-valuenow"), "416");
});

/** Privacy modes may refuse either storage access boundary. The current page
 * must still let the user expand and collapse without throwing or stale aria.
 */
test("storage refusals preserve a working toggle and fresh collapsed default", () => {
  for (const options of [{ refuseStorage: true }, { refuseRead: true }, { refuseWrite: true }]) {
    const fixture = browserFixture(options);
    fixture.install();
    const toggle = fixture.mountToggle();
    assertRail(fixture, false, 396);
    toggle.emit("click");
    assertRail(fixture, true, 560);
    toggle.emit("click");
    assertRail(fixture, false, 396);
  }
});
