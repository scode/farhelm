/** Pin the keyboard boundary separately from Rust's queue and render timing.
 * Pure rows cover Mac clients unavailable to Linux browser tests. VM tests run
 * the shipped installer and synchronous relay clicks, including same-window
 * propagation: stopPropagation alone must not make these assertions pass.
 */
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const { classifyLaunchSequenceKey } = require("../assets/launch-sequence.js");
const source = fs.readFileSync(require.resolve("../assets/launch-sequence.js"), "utf8");
const clients = [
  { name: "Mac desktop", macDesktop: true, macPlatform: true, number: { metaKey: true }, bolt: { metaKey: true } },
  { name: "Mac browser", macDesktop: false, macPlatform: true,
    number: { ctrlKey: true, shiftKey: true }, bolt: { metaKey: true } },
  { name: "other browser", macDesktop: false, macPlatform: false,
    number: { ctrlKey: true, shiftKey: true }, bolt: { ctrlKey: true, shiftKey: true } },
];

/** Complete defaults make each refusal row vary only its named boundary. */
function key(overrides = {}) {
  return { code: "KeyX", key: "x", ctrlKey: false, shiftKey: false, altKey: false,
    metaKey: false, isComposing: false, repeat: false, ...overrides };
}

/** Keep client identity explicit; no tests infer it from process environment. */
function classify(client, event, armed = true, editor = false) {
  return classifyLaunchSequenceKey(key(event), armed, client.macDesktop, client.macPlatform, editor);
}

for (const client of clients) {
  test(`${client.name}: prefix and numbers use build modifiers, bolt uses the platform`, () => {
    assert.deepEqual(classify(client, { ...client.number }, false), { action: "begin", consume: true });
    assert.deepEqual(classify(client, { ...client.number }, false, true), { action: "pass", consume: false });
    for (const digit of "1234567890") {
      for (const modifiers of [{}, client.number]) {
        assert.deepEqual(classify(client, { code: `Digit${digit}`, key: "layout-specific", ...modifiers }),
          { action: "digit", consume: true, digit });
      }
    }
    assert.deepEqual(classify(client, { code: "KeyP", key: "p", ...client.bolt }),
      { action: "bolt", consume: true });
    assert.deepEqual(classify(client, { code: "KeyP", key: "p", ...client.bolt }, false),
      { action: "pass", consume: false }, "idle bolt must retain its existing direct handler");
    assert.deepEqual(classify(client, { code: "Digit3", ...client.number }, false),
      { action: "pass", consume: false }, "idle numbers must retain their existing direct handler");
    assert.deepEqual(classify(client, { code: "KeyY", key: "x", ...client.number }, false),
      { action: "pass", consume: false }, "prefix identity is physical, not the printed character");
    assert.deepEqual(classify(client, { code: "KeyP", key: "p" }), { action: "cancel", consume: false });
    // Holding the prefix cannot erase the queue; a fresh prefix is unrelated.
    assert.deepEqual(classify(client, { ...client.number, repeat: true }), { action: "none", consume: true });
    assert.deepEqual(classify(client, { ...client.number }), { action: "cancel", consume: false });
    assert.deepEqual(classify(client, { ...client.number, repeat: true }, false), { action: "none", consume: true });
  });

  test(`${client.name}: extra modifiers, keypad digits and IME never queue`, () => {
    const extraModifiers = client.macDesktop
      ? [{ shiftKey: true }, { ctrlKey: true }, { altKey: true }]
      : [{ metaKey: true }, { altKey: true }, { shiftKey: false }, { ctrlKey: false }];
    for (const extra of extraModifiers) {
      assert.deepEqual(classify(client, { ...client.number, ...extra }, false), { action: "pass", consume: false });
      assert.deepEqual(classify(client, { code: "Digit3", ...client.number, ...extra }),
        { action: "cancel", consume: false });
    }
    for (const code of ["Numpad3", "Digit", "Digit10", "digit3", ""]) {
      assert.deepEqual(classify(client, { code, ...client.number }), { action: "cancel", consume: false });
    }
    for (const modifiers of [{ altKey: true }, client.macPlatform ? { ctrlKey: true } : { metaKey: true },
      client.macPlatform ? { shiftKey: true } : { shiftKey: false }]) {
      assert.deepEqual(classify(client, { code: "KeyP", ...client.bolt, ...modifiers }),
        { action: "cancel", consume: false });
    }
    for (const event of [{ ...client.number }, { code: "Digit3" }, { code: "KeyP", ...client.bolt },
      { code: "Enter", key: "Enter" }]) {
      assert.deepEqual(classify(client, { ...event, isComposing: true }), { action: "cancel", consume: false });
    }
    assert.deepEqual(classify(client, { ...client.number, isComposing: true }, false),
      { action: "pass", consume: false });
    for (const event of [{ code: "Digit3" }, { code: "Digit3", ...client.number },
      { code: "KeyP", ...client.bolt }, { code: "Enter", key: "Enter" }, { code: "Escape", key: "Escape" }]) {
      assert.deepEqual(classify(client, { ...event, repeat: true }), { action: "none", consume: true });
    }
  });
}

test("only plain Enter launches; Escape consumes and three modifier keys preserve the sequence", () => {
  const client = clients[0];
  assert.deepEqual(classify(client, { code: "Enter", key: "Enter" }), { action: "launch", consume: true });
  for (const modifier of ["ctrlKey", "shiftKey", "metaKey", "altKey"]) {
    assert.deepEqual(classify(client, { code: "Enter", key: "Enter", [modifier]: true }),
      { action: "cancel", consume: false });
  }
  assert.deepEqual(classify(client, { code: "Escape", key: "Escape", shiftKey: true }),
    { action: "cancel", consume: true });
  for (const [name, modifier] of [["Meta", "metaKey"], ["Control", "ctrlKey"], ["Shift", "shiftKey"]]) {
    assert.deepEqual(classify(client, { code: `${name}Left`, key: name, [modifier]: true }),
      { action: "pass", consume: false });
  }
  assert.deepEqual(classify(client, { code: "AltLeft", key: "Alt", altKey: true }),
    { action: "cancel", consume: false });
});

/** Model only DOM contracts the installer uses, retaining ancestor/editability
 * behavior so input descendants and contenteditable=false are distinct cases.
 */
class Element {
  constructor({ tag = "div", parent = null, terminal = false, editable, modal = false,
    visible = true, action, digit, disabled = false } = {}) {
    Object.assign(this, { tag, parent, terminal, editable, modal, visible, action, digit, disabled });
  }
  get isContentEditable() {
    return this.editable === undefined ? !!this.parent?.isContentEditable : this.editable;
  }
  matches(selector) {
    if (selector === ".xterm-helper-textarea") return this.terminal;
    if (selector === "input, textarea") return ["input", "textarea"].includes(this.tag);
    if (selector === ".launch-sequence-trigger") return this.action !== undefined;
    if (selector === '[role="dialog"][aria-modal="true"]') return this.modal;
    throw new Error(`unexpected selector ${selector}`);
  }
  closest(selector) {
    return this.matches(selector) ? this : this.parent?.closest(selector) || null;
  }
  getClientRects() { return this.visible ? [{}] : []; }
}

/** Dispatch all listeners on one target unless stopImmediatePropagation fires.
 * This deliberately lets a later direct-launch listener run after ordinary
 * stopPropagation, reproducing the browser boundary the sequence must protect.
 */
function eventTarget() {
  const listeners = new Map();
  return {
    listeners,
    addEventListener(type, listener) {
      if (!listeners.has(type)) listeners.set(type, []);
      listeners.get(type).push(listener);
    },
    dispatch(type, overrides = {}) {
      const event = { ...key(), target: new Element(), defaultPrevented: false, propagationStopped: false,
        immediateStopped: false, preventDefault() { this.defaultPrevented = true; },
        stopPropagation() { this.propagationStopped = true; },
        stopImmediatePropagation() { this.immediateStopped = true; this.propagationStopped = true; }, ...overrides };
      for (const listener of listeners.get(type) || []) {
        listener(event);
        if (event.immediateStopped) break;
      }
      return event;
    },
  };
}

/** Exercise the real asset without waiting for a render between relay clicks.
 * All relays exist from the outset, mirroring the Rust mounting contract; the
 * missing/disabled cases below vary that premise explicitly before the input.
 */
function page(client = clients[2]) {
  const window = eventTarget();
  const document = eventTarget();
  const clicks = [];
  const relays = new Map();
  const modals = [];
  document.hidden = false;
  document.querySelectorAll = selector => {
    assert.equal(selector, '[role="dialog"][aria-modal="true"]');
    return modals;
  };
  document.querySelector = selector => {
    const action = selector.match(/data-sequence-action="(\w+)"/)?.[1];
    const digit = selector.match(/data-number-key="(\d)"/)?.[1];
    assert.ok(action, `relay selector expected: ${selector}`);
    return relays.get(action === "digit" ? `digit${digit}` : action) || null;
  };
  for (const action of ["begin", "bolt", "launch", "cancel", ...Array.from("1234567890", d => `digit${d}`)]) {
    const relay = new Element({ action: action.startsWith("digit") ? "digit" : action,
      digit: action.startsWith("digit") ? action.slice(-1) : undefined });
    relay.click = () => {
      // The real .click() enters window's click capture before Rust's handler.
      window.dispatch("click", { target: relay });
      clicks.push(action);
    };
    relays.set(action, relay);
  }
  vm.runInNewContext(source, { window, document, Element,
    navigator: { platform: client.macPlatform ? "MacIntel" : "Linux" } });
  window.installLaunchSequence(client.macDesktop);
  const press = event => window.dispatch("keydown", key(event));
  const prefix = overrides => press({ ...client.number, ...overrides });
  return { window, document, clicks, relays, modals, press, prefix };
}

for (const client of clients) {
  test(`${client.name}: fast relay dispatch is exclusive and does not cancel itself`, () => {
    const p = page(client);
    let direct = 0;
    p.window.addEventListener("keydown", () => direct++);
    assert.equal(p.relays.get("begin").disabled, false);
    assert.equal(p.prefix().immediateStopped, true);
    p.press({ code: "Digit1", ...client.number });
    p.press({ code: "Digit1" });
    p.press({ code: "KeyP", ...client.bolt });
    p.press({ code: "Enter", key: "Enter" });
    assert.deepEqual(p.clicks, ["begin", "digit1", "digit1", "bolt", "launch"]);
    assert.equal(direct, 0, "queued modified keys must not reach existing direct launch handlers");
    assert.equal(p.press({ code: "Digit2" }).defaultPrevented, false);
    assert.equal(direct, 1, "Launch must synchronously retire keyboard ownership");
  });
}

test("missing Begin leaves every key alone; disabled Begin consumes without arming", () => {
  for (const missing of [true, false]) {
    const p = page();
    if (missing) p.relays.delete("begin");
    else p.relays.get("begin").disabled = true;
    assert.equal(p.prefix().defaultPrevented, !missing);
    assert.equal(p.press({ code: "Digit1" }).defaultPrevented, false);
    assert.equal(p.press({ code: "Enter", key: "Enter" }).defaultPrevented, false);
    assert.deepEqual(p.clicks, []);
  }
});

test("held recognized keys are consumed without a second action or premature launch", () => {
  const p = page();
  assert.equal(p.prefix({ repeat: true }).defaultPrevented, true);
  assert.deepEqual(p.clicks, [], "a repeat without an accepted initial prefix cannot arm");
  p.prefix();
  for (const event of [{ code: "Digit1" }, { code: "KeyP", ctrlKey: true, shiftKey: true },
    { code: "Enter", key: "Enter" }, { code: "Escape", key: "Escape" },
    { code: "KeyX", ctrlKey: true, shiftKey: true }]) {
    assert.equal(p.press({ ...event, repeat: true }).immediateStopped, true);
  }
  p.press({ code: "Digit2" });
  p.press({ code: "Enter", key: "Enter" });
  assert.deepEqual(p.clicks, ["begin", "digit2", "launch"]);
});

test("unrelated input cancels before passing through to later listeners", () => {
  for (const event of [{ code: "KeyA", key: "a" }, { code: "KeyP", key: "p" },
    { code: "Enter", key: "Enter", shiftKey: true }, { code: "Digit1", isComposing: true }]) {
    const p = page();
    p.prefix();
    let seen = 0;
    p.window.addEventListener("keydown", () => {
      seen++;
      assert.deepEqual(p.clicks, ["begin", "cancel"]);
    });
    assert.equal(p.press(event).defaultPrevented, false);
    assert.equal(seen, 1);
    assert.equal(p.press({ code: "Digit2" }).defaultPrevented, false);
  }
});

test("text fields keep their prefix while xterm's textarea can begin", () => {
  const editable = new Element({ editable: true });
  const targets = [new Element({ tag: "input" }), new Element({ tag: "textarea" }),
    editable, new Element({ parent: editable }), new Element({ parent: new Element({ tag: "input" }) })];
  for (const target of targets) {
    const p = page();
    assert.equal(p.prefix({ target }).defaultPrevented, false);
    assert.deepEqual(p.clicks, []);
  }
  for (const target of [new Element({ tag: "textarea", terminal: true }),
    new Element({ parent: editable, editable: false }), new Element()]) {
    const p = page();
    assert.equal(p.prefix({ target }).defaultPrevented, true);
    assert.deepEqual(p.clicks, ["begin"]);
  }
});

test("cancellation paths retire both owners once and allow the next terminal key", () => {
  const cancellation = [
    p => p.press({ code: "Escape", key: "Escape" }),
    p => p.window.dispatch("pointerdown"),
    p => p.window.dispatch("click"),
    p => p.window.dispatch("blur"),
    p => { p.document.hidden = true; p.document.dispatch("visibilitychange"); },
    p => p.window.cancelFarhelmLaunchSequence(),
    p => p.window.dispatch("focusin", { target: new Element({ parent: new Element({ modal: true }) }) }),
  ];
  for (const cancel of cancellation) {
    const p = page();
    p.prefix();
    cancel(p);
    p.window.cancelFarhelmLaunchSequence();
    assert.deepEqual(p.clicks, ["begin", "cancel"]);
    assert.equal(p.press({ code: "Digit1" }).defaultPrevented, false);
  }
});

test("visible dialogs own keys immediately; hidden dialogs and ordinary focus do not cancel", () => {
  const p = page();
  const hidden = new Element({ modal: true, visible: false });
  p.modals.push(hidden);
  p.prefix();
  p.window.dispatch("focusin", { target: hidden });
  p.window.dispatch("focusin", { target: new Element() });
  p.document.dispatch("visibilitychange");
  p.press({ code: "Digit1" });
  assert.deepEqual(p.clicks, ["begin", "digit1"]);
  hidden.visible = true;
  assert.equal(p.press({ code: "Enter", key: "Enter" }).defaultPrevented, false);
  assert.deepEqual(p.clicks, ["begin", "digit1", "cancel"]);
  assert.equal(p.prefix().defaultPrevented, false);
});

test("installer is idempotent and empty Enter spends ownership synchronously", () => {
  const p = page();
  p.window.installLaunchSequence(false);
  assert.equal(p.window.listeners.get("keydown").length, 1);
  assert.equal(p.document.listeners.get("visibilitychange").length, 1);
  p.prefix();
  p.press({ code: "Enter", key: "Enter" });
  assert.deepEqual(p.clicks, ["begin", "launch"]);
  assert.equal(p.press({ code: "Enter", key: "Enter" }).defaultPrevented, false);
});
