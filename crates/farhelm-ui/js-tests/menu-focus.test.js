/** Exercise the shipped page boundary with synchronous native focus events.
 * Each call retains the epoch that Rust copied at its physical key press;
 * delaying that call models an already-sent desktop focus request.
 */
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const { installMenuFocus } = require("../assets/menu-focus.js");

/** Model the synchronous relay before delegated focus handling. The selector,
 * capture phase and bubbling event are strict to catch bridge contract drift.
 */
function page({ relayMounted = true } = {}) {
  const listeners = new Map();
  const nodes = [];
  const publications = [];
  const win = { Event: class {
    constructor(type, options) { Object.assign(this, { type }, options); }
  } };
  const relay = {
    mounted: relayMounted,
    value: "",
    dispatchEvent(event) {
      assert.ok(event instanceof win.Event);
      assert.equal(event.type, "input");
      assert.equal(event.bubbles, true);
      publications.push(this.value);
      this.onInput?.(event);
    },
  };
  const doc = {
    activeElement: null,
    addEventListener(type, listener, capture) {
      assert.equal(capture, true);
      if (!listeners.has(type)) listeners.set(type, []);
      listeners.get(type).push(listener);
    },
    querySelector(selector) {
      assert.equal(selector, "input[data-menu-focus-epoch]");
      return relay.mounted ? relay : null;
    },
    querySelectorAll(selector) {
      assert.equal(selector, "[data-menu-focus]");
      return nodes.filter(node => node.key !== undefined);
    },
  };
  /** Native event dispatch completes before focus() returns. */
  function dispatch(type, target, key, fields = {}) {
    for (const listener of listeners.get(type) || []) listener({ target, key, ...fields });
  }
  /** Make menu membership and native focus refusal explicit fixture inputs.
   * An unmarked child delegates identity lookup to its parent, as an item's
   * icon or description does. Menus exist only while their panel is mounted.
   */
  function element(key, menu = true, parentElement = null) {
    const node = {
      key,
      parentElement,
      isConnected: true,
      focusable: true,
      focusCalls: 0,
      focusOptions: [],
      getAttribute(name) {
        assert.equal(name, "data-menu-focus");
        return this.key;
      },
      closest(selector) {
        if (selector === "[data-menu-focus]") {
          return this.key !== undefined ? this : this.parentElement?.closest(selector) || null;
        }
        if (selector === '[role="menu"]') return menu ? this : null;
        assert.equal(selector, '[role="menu"], [role="menuitem"], [aria-haspopup="menu"]');
        return menu ? this : null;
      },
      focus(options) {
        this.focusCalls++;
        this.focusOptions.push(options);
        if (!this.focusable || doc.activeElement === this) return;
        doc.activeElement = this;
        dispatch("focusin", this);
        this.afterFocus?.();
      },
    };
    nodes.push(node);
    return node;
  }
  return { doc, win, relay, publications, listeners, element, dispatch,
    epoch() { return JSON.parse(relay.value)[0]; },
    key(target, key = "ArrowDown") { dispatch("keydown", target, key); },
    move(target, pointerType = "mouse") { dispatch("pointermove", target, undefined, { pointerType }); },
    install() { return installMenuFocus(doc, win); } };
}

// Hover must enter the same independent-focus path as a click or a screen
// reader, rather than granting an old desktop navigation call new authority.
test("mouse and pen hover focus the nearest menu marker without scrolling and retire old arrows", () => {
  for (const pointerType of ["mouse", "pen"]) {
    const p = page();
    const focus = p.install();
    const first = p.element("first");
    const hovered = p.element("hovered");
    const child = p.element(undefined, true, hovered);
    const pending = p.element("pending");
    first.focus();
    p.key(first);
    const epoch = p.epoch();
    assert.equal(p.doc.activeElement, first);
    p.move(child, pointerType);
    assert.equal(p.doc.activeElement, hovered);
    assert.deepEqual(hovered.focusOptions, [{ preventScroll: true }]);
    assert.equal(child.focusCalls, 0);
    assert.deepEqual(JSON.parse(p.relay.value), [epoch + 1, false]);
    assert.equal(focus("pending", false, epoch), false);
    assert.equal(pending.focusCalls, 0);
    assert.equal(p.win.__farhelmMenuPointerFocus, undefined);
    p.move(child, pointerType);
    assert.equal(hovered.focusCalls, 1, "movement within the focused item must not refocus it");
    assert.equal(p.epoch(), epoch + 1);
  }
});

// Merely revealing or moving content under a resting pointer must not pull
// focus away from keyboard navigation; touch has no hover contract at all.
test("pointerover, touch and targets outside mounted menus leave focus alone", () => {
  const p = page();
  p.install();
  const first = p.element("first");
  const second = p.element("second");
  const outside = p.element("outside", false);
  const unmarked = p.element(undefined);
  first.focus();
  const publications = p.publications.length;
  p.dispatch("pointerover", second, undefined, { pointerType: "mouse" });
  p.move(second, "touch");
  p.move(second, "");
  p.move(outside);
  p.move(unmarked);
  p.move({});
  assert.equal(p.doc.activeElement, first);
  assert.equal(second.focusCalls, 0);
  assert.equal(outside.focusCalls, 0);
  assert.equal(p.publications.length, publications);
});

// Hover can beat the deliberately ungated opening request across the bridge.
// That accepted focus race may change the current item, but never activates it.
test("hover does not gate an old opening request or claim its focus as pointer-owned", () => {
  const p = page();
  const focus = p.install();
  const first = p.element("first");
  const hovered = p.element("hovered");
  p.win.__farhelmMenuFocusSync();
  const openingEpoch = p.epoch();
  p.move(hovered);
  assert.equal(p.doc.activeElement, hovered);
  assert.equal(p.epoch(), openingEpoch + 1);
  assert.equal(focus("first", true, openingEpoch), true);
  assert.equal(p.doc.activeElement, first);
  assert.deepEqual(JSON.parse(p.relay.value), [openingEpoch + 1, true]);
  assert.equal(p.win.__farhelmMenuPointerFocus, undefined);
});

// Focus handlers can synchronously cause another pointer move or fail. The
// tooltip marker belongs to the current native call, never to later focus.
test("pointer focus restores provenance across reentrant moves and exceptions", () => {
  const p = page();
  p.install();
  const outer = p.element("outer");
  const inner = p.element("inner");
  const failing = p.element("failing");
  const markers = [];
  p.doc.addEventListener("focusin", event => markers.push([event.target, p.win.__farhelmMenuPointerFocus]), true);
  outer.afterFocus = () => {
    p.move(inner);
    assert.equal(p.win.__farhelmMenuPointerFocus, outer);
  };
  p.move(outer);
  assert.deepEqual(markers, [[outer, outer], [inner, inner]]);
  assert.equal(p.doc.activeElement, inner);
  assert.equal(p.win.__farhelmMenuPointerFocus, undefined);
  failing.afterFocus = () => { throw new Error("focus handler failed"); };
  assert.throws(() => p.move(failing), /focus handler failed/);
  assert.equal(p.win.__farhelmMenuPointerFocus, undefined);
});

// A new navigation key can authorize its own move, never revive an older call.
test("an immutable request epoch stays stale after independent focus and a fresh arrow", () => {
  for (const insideMenu of [true, false]) {
    const p = page();
    const focus = p.install();
    const first = p.element("first");
    const pending = p.element("pending");
    const independent = p.element("independent", insideMenu);
    first.focus();
    p.key(first);
    const oldEpoch = p.epoch();
    independent.focus();
    assert.equal(p.doc.activeElement, independent, "establish independent focus before releasing the call");
    assert.equal(focus("pending", false, oldEpoch), false);
    assert.equal(pending.focusCalls, 0);
    assert.equal(p.doc.activeElement, independent);
    if (!insideMenu) first.focus();
    p.key(insideMenu ? independent : first);
    const freshEpoch = p.epoch();
    assert.notEqual(freshEpoch, oldEpoch);
    assert.equal(focus("pending", false, oldEpoch), false, "a fresh arrow cannot revive the old request");
    assert.equal(focus("pending", false, freshEpoch), true);
    assert.equal(p.doc.activeElement, pending);
  }
});

// App-owned moves share one epoch, preserving every rapid arrow in a burst.
test("a burst retains its request epoch through the helper's own focus events", () => {
  const p = page();
  const focus = p.install();
  const first = p.element("first");
  const second = p.element("second");
  const third = p.element("third");
  first.focus();
  p.key(first);
  const epoch = p.epoch();
  p.key(first);
  assert.equal(focus("second", false, epoch), true);
  assert.equal(focus("third", false, epoch), true);
  p.key(third, "ArrowUp");
  assert.equal(p.epoch(), epoch);
  assert.equal(focus("second", false, epoch), true);
  assert.equal(p.doc.activeElement, second);
});

// Initial opening has no navigation token; its own focus-in must still leave
// tokens from arrows already pressed against the opening toggle valid.
test("ungated opening ignores the token and preserves a fast following arrow", () => {
  const p = page();
  const focus = p.install();
  const toggle = p.element(undefined);
  const first = p.element("first");
  const second = p.element("second");
  assert.equal(focus("first"), false, "a missing epoch fails closed");
  assert.equal(focus("first", true), true);
  toggle.focus();
  p.key(toggle);
  const epoch = p.epoch();
  p.key(toggle);
  assert.equal(focus("first", true, -1), true);
  assert.equal(focus("second", false, epoch), true);
  assert.equal(p.epoch(), epoch);
});

// Only navigation against a menu needs the defensive startup publication.
// No key creates a new epoch: independent focus is the sole source of changes.
test("menu navigation republishes the epoch; unrelated keys and outside arrows do not", () => {
  const p = page();
  p.install();
  const item = p.element("item");
  const outside = p.element(undefined, false);
  item.focus();
  const epoch = p.epoch();
  const before = p.publications.length;
  for (const key of ["Enter", "Escape", "Tab", "a", "ArrowLeft", "ArrowRight"]) p.key(item, key);
  for (const key of ["ArrowDown", "ArrowUp", "Home", "End"]) p.key(outside, key);
  assert.equal(p.publications.length, before);
  for (const key of ["ArrowDown", "ArrowUp", "Home", "End"]) {
    p.relay.value = "stale";
    p.key(item, key);
    assert.equal(p.epoch(), epoch, key);
    assert.deepEqual(JSON.parse(p.relay.value), [epoch, false]);
  }
  assert.equal(p.publications.length, before + 4);
});

// Rust must learn both epoch and source before delegated focus classification.
// An app-owned landing has its own publication without advancing the epoch.
test("every focus publishes its source synchronously before downstream focus listeners", () => {
  const p = page();
  const focus = p.install();
  const first = p.element("first");
  const second = p.element("second");
  const order = [];
  p.relay.onInput = () => order.push(`input:${p.relay.value}`);
  p.doc.addEventListener("focusin", () => order.push(`focus:${p.relay.value}`), true);
  first.focus();
  assert.deepEqual(order, ["input:[1,false]", "focus:[1,false]"]);
  order.length = 0;
  assert.equal(focus("second", false, p.epoch()), true);
  assert.deepEqual(order, ["input:[1,true]", "focus:[1,true]"]);
});

// An opening can legitimately land after its captured epoch changed. Its
// ownership comes from the synchronous call, never its age or target identity.
test("old ungated opening publishes current ownership; independent same-target focus publishes false", () => {
  const p = page();
  const focus = p.install();
  const first = p.element("first");
  const outside = p.element(undefined, false);
  first.focus();
  const openingEpoch = p.epoch();
  outside.focus();
  const currentEpoch = p.epoch();
  assert.notEqual(currentEpoch, openingEpoch);
  assert.equal(focus("first", true, openingEpoch), true);
  assert.deepEqual(JSON.parse(p.relay.value), [currentEpoch, true]);
  outside.focus();
  first.focus();
  assert.deepEqual(JSON.parse(p.relay.value), [currentEpoch + 2, false]);
});

// Observation starts before the root relay mounts; onmounted synchronization
// must publish accumulated focus changes rather than resetting the page state.
test("startup synchronization publishes an epoch retained while the relay was absent", () => {
  const p = page({ relayMounted: false });
  const focus = p.install();
  p.element(undefined, false).focus();
  p.element(undefined, false).focus();
  assert.deepEqual(p.publications, []);
  p.relay.mounted = true;
  p.win.__farhelmMenuFocusSync();
  assert.deepEqual(p.publications, ["[2,false]"]);
  const item = p.element("item");
  assert.equal(focus("item", false, p.epoch()), true);
});

// Durable marker identity admits punctuation literally and cannot fall back
// to a prefix or another item after the requested item disappears.
test("lookup requires an exact connected marker and reports native focus refusal", () => {
  const p = page();
  const focus = p.install();
  const removed = p.element("removed");
  removed.isConnected = false;
  const prefix = p.element("menu:item-extra");
  const inert = p.element("inert");
  inert.focusable = false;
  const exact = p.element('menu:"[item]');
  for (const key of ["missing", "removed", "menu:item", "inert"]) assert.equal(focus(key, true), false);
  assert.equal(removed.focusCalls, 0);
  assert.equal(prefix.focusCalls, 0);
  assert.equal(focus('menu:"[item]', true), true);
  assert.equal(p.doc.activeElement, exact);
});

// Only the requested target's focus-in is app-owned. A focus handler that
// immediately redirects elsewhere must invalidate the original token.
test("synchronous focus redirection is independent and invalidates the request epoch", () => {
  const p = page();
  const focus = p.install();
  const first = p.element("first");
  const redirect = p.element("redirect");
  const outside = p.element(undefined, false);
  first.focus();
  const epoch = p.epoch();
  redirect.afterFocus = () => outside.focus();
  assert.equal(focus("redirect", false, epoch), false);
  assert.equal(p.doc.activeElement, outside);
  assert.equal(p.epoch(), epoch + 1);
  assert.deepEqual(JSON.parse(p.relay.value), [epoch + 1, false]);
  assert.equal(focus("first", false, epoch), false);
});

// Re-evaluation cannot reset provenance or stack publishers/listeners; a second
// page nevertheless needs an independent counter and its own relay.
test("installation is idempotent per page and isolated between pages", () => {
  const p = page();
  const focus = p.install();
  const sync = p.win.__farhelmMenuFocusSync;
  const first = p.element("first");
  const other = p.element("other");
  first.focus();
  const oldEpoch = p.epoch();
  other.focus();
  assert.equal(p.install(), focus);
  assert.equal(p.win.__farhelmMenuFocusSync, sync);
  assert.equal(p.listeners.get("focusin").length, 1);
  assert.equal(p.listeners.get("keydown").length, 1);
  assert.equal(p.listeners.get("pointermove").length, 1);
  assert.equal(focus("first", false, oldEpoch), false);
  const separate = page();
  const separateFocus = separate.install();
  const target = separate.element("first");
  separate.win.__farhelmMenuFocusSync();
  assert.equal(separateFocus("first", false, separate.epoch()), true);
  assert.equal(focus("first", false, oldEpoch), false);
});

// Rust includes this same source without CommonJS in desktop and web pages.
test("browser evaluation exposes both globals and retains them on reinstallation", () => {
  const p = page();
  const source = fs.readFileSync(require.resolve("../assets/menu-focus.js"), "utf8");
  const context = vm.createContext({ document: p.doc, Event: p.win.Event });
  vm.runInContext(source, context);
  const focus = context.__farhelmMenuFocus;
  const sync = context.__farhelmMenuFocusSync;
  assert.equal(typeof focus, "function");
  assert.equal(typeof sync, "function");
  vm.runInContext(source, context);
  assert.equal(context.__farhelmMenuFocus, focus);
  assert.equal(context.__farhelmMenuFocusSync, sync);
  sync();
  assert.equal(p.relay.value, "[0,false]");
  const item = p.element("item");
  assert.equal(focus("item", false, p.epoch()), true);
  assert.equal(p.doc.activeElement, item);
});
