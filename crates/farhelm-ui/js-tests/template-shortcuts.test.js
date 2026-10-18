/** Pin the one keyboard boundary separately from Rust's dispatch and renders.
 * Pure rows cover Mac clients unavailable to Linux browser tests. VM tests run
 * the shipped installer, including same-window capture propagation and live
 * relay clicks; stopPropagation alone cannot make those assertions pass.
 */
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const { classifyTemplateShortcutKey, templateShortcutContext } = require("../assets/template-shortcuts.js");
const source = fs.readFileSync(require.resolve("../assets/template-shortcuts.js"), "utf8");
/** Mirror the serialized definition's exact modifier masks. A changed mask
 * must affect matching without editing the JS classifier itself.
 */
function modifiers(source) {
  return { source,
    mac: { meta: true, ctrl: false, shift: false, alt: false },
    other: { meta: false, ctrl: true, shift: true, alt: false } };
}
const defaultDefinition = {
  bolt: { code: "KeyP", modifiers: modifiers("platform") },
  number: { keys: Array.from("1234567890", value => ({ code: `Digit${value}`, value })),
    modifiers: modifiers("mac_desktop_build") },
  prefix: { code: "KeyX", modifiers: modifiers("mac_desktop_build") },
  template_list: { code: "KeyJ", modifiers: modifiers("platform") },
};
const clients = [
  { name: "Mac desktop", mac_desktop: true, mac_platform: true,
    number: { metaKey: true }, bolt: { metaKey: true } },
  { name: "Mac browser", mac_desktop: false, mac_platform: true,
    number: { ctrlKey: true, shiftKey: true }, bolt: { metaKey: true } },
  { name: "other browser", mac_desktop: false, mac_platform: false,
    number: { ctrlKey: true, shiftKey: true }, bolt: { ctrlKey: true, shiftKey: true } },
];

/** Complete defaults make each refusal row vary only its named boundary. */
function key(overrides = {}) {
  return { code: "KeyX", key: "x", ctrlKey: false, shiftKey: false, altKey: false,
    metaKey: false, isComposing: false, repeat: false, ...overrides };
}

/** Keep client identity explicit; no tests infer it from process environment. */
function classify(client, event, armed = true, editor = false, definition = defaultDefinition) {
  return classifyTemplateShortcutKey(key(event), armed,
    { ...definition, mac_desktop: client.mac_desktop, mac_platform: client.mac_platform }, editor);
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
      { action: "quick-bolt", consume: true });
    assert.deepEqual(classify(client, { code: "Digit3", ...client.number }, false),
      { action: "quick-digit", consume: true, digit: "3" });
    assert.deepEqual(classify(client, { code: "KeyJ", ...client.bolt }, false),
      { action: "template-list", consume: false }, "Cmd-J is defined but not installed in PR1");
    assert.deepEqual(classify(client, { code: "KeyY", key: "x", ...client.number }, false),
      { action: "pass", consume: false }, "prefix identity is physical, not the printed character");
    assert.deepEqual(classify(client, { code: "KeyP", key: "p" }), { action: "cancel", consume: false });
    // Holding the prefix cannot erase the queue; a fresh prefix is unrelated.
    assert.deepEqual(classify(client, { ...client.number, repeat: true }), { action: "none", consume: true });
    assert.deepEqual(classify(client, { ...client.number }), { action: "cancel", consume: false });
    assert.deepEqual(classify(client, { ...client.number, repeat: true }, false),
      { action: "none", consume: true, source: "begin" });
  });

  test(`${client.name}: extra modifiers, keypad digits and IME never queue`, () => {
    const extraModifiers = client.mac_desktop
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
    for (const modifiers of [{ altKey: true }, client.mac_platform ? { ctrlKey: true } : { metaKey: true },
      client.mac_platform ? { shiftKey: true } : { shiftKey: false }]) {
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
    visible = true, action, digit, disabled = false, selectionStart, selectionEnd, savePanel = false } = {}) {
    Object.assign(this, { tag, parent, terminal, editable, modal, visible, action, digit, disabled,
      selectionStart, selectionEnd, savePanel });
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
  contains(target) {
    for (let node = target; node; node = node.parent) if (node === this) return true;
    return false;
  }
  querySelector(selector) {
    assert.equal(selector, ".save-template-panel");
    return this.savePanel ? new Element() : null;
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
function page(client = clients[2], definition = defaultDefinition) {
  const window = eventTarget();
  const document = eventTarget();
  const clicks = [];
  const relays = new Map();
  const direct = new Map();
  const modals = [];
  document.form = null;
  document.hidden = false;
  document.selection = { isCollapsed: true };
  window.getSelection = () => document.selection;
  document.querySelectorAll = selector => {
    assert.equal(selector, '[role="dialog"][aria-modal="true"]');
    return modals;
  };
  document.querySelector = selector => {
    if (selector === ".create-session-form") return document.form;
    if (selector === ".quick-launch-trigger") return direct.get("bolt") || null;
    if (selector.startsWith(".template-number-trigger")) {
      const digit = selector.match(/data-number-key="(\d)"/)?.[1];
      return direct.get(`digit${digit}`) || null;
    }
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
  for (const action of ["bolt", ...Array.from("1234567890", d => `digit${d}`)]) {
    const relay = new Element();
    relay.click = () => {
      window.dispatch("click", { target: relay });
      clicks.push(`quick-${action}`);
    };
    direct.set(action, relay);
  }
  vm.runInNewContext(source, { window, document, Element,
    navigator: { platform: client.mac_platform ? "MacIntel" : "Linux" } });
  window.installTemplateShortcuts({ ...definition, mac_desktop: client.mac_desktop });
  const press = event => window.dispatch("keydown", key(event));
  const prefix = overrides => press({ ...client.number, ...overrides });
  return { window, document, clicks, relays, direct, modals, press, prefix };
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

/** A selection keeps Cut available in New, Clone and Replace with. Without a
 * selection, the same physical Mac-desktop chord starts a sequence; a field
 * outside the launcher still keeps the key even while New is visible.
 */
test("Mac desktop launcher fields reserve selected text for Cut", () => {
  const p = page(clients[0]);
  const form = new Element({ modal: true });
  p.document.form = form;
  p.modals.push(form);
  const input = new Element({ tag: "input", parent: form, selectionStart: 1, selectionEnd: 4 });
  const prefix = () => p.press({ code: "KeyX", metaKey: true, target: input });
  assert.equal(p.window.farhelmTemplateShortcutContext(), "launcher");
  assert.equal(prefix().defaultPrevented, false);
  assert.deepEqual(p.clicks, [], "a selected range must remain available to Cut");
  input.selectionEnd = 1;
  assert.equal(prefix().defaultPrevented, true);
  assert.deepEqual(p.clicks, ["begin"]);
  assert.equal(p.press({ code: "Digit2", target: input }).defaultPrevented, true);
  assert.equal(p.press({ code: "Enter", key: "Enter", target: input }).defaultPrevented, true);
  assert.deepEqual(p.clicks, ["begin", "digit2", "launch"]);

  const outside = new Element({ tag: "input" });
  assert.equal(p.press({ code: "KeyX", metaKey: true, target: outside }).defaultPrevented, false);
  assert.deepEqual(p.clicks, ["begin", "digit2", "launch"]);
});

/** Input selection offsets and contenteditable selection state come from
 * different DOM APIs. Both must preserve selected text and admit an empty
 * selection inside the open launcher.
 */
test("launcher selection rule covers browser fields and contenteditable descendants", () => {
  const p = page();
  const form = new Element({ modal: true });
  p.document.form = form;
  p.modals.push(form);
  const field = new Element({ tag: "textarea", parent: form, selectionStart: 0, selectionEnd: 3 });
  const chord = { code: "KeyX", ctrlKey: true, shiftKey: true };
  assert.equal(p.press({ ...chord, target: field }).defaultPrevented, false);
  field.selectionEnd = 0;
  assert.equal(p.press({ ...chord, target: field }).defaultPrevented, true);
  p.press({ code: "Escape", key: "Escape" });

  const editable = new Element({ parent: form, editable: true });
  const child = new Element({ parent: editable });
  p.document.selection = { isCollapsed: false };
  assert.equal(p.press({ ...chord, target: child }).defaultPrevented, false);
  p.document.selection.isCollapsed = true;
  assert.equal(p.press({ ...chord, target: child }).defaultPrevented, true);
  assert.deepEqual(p.clicks, ["begin", "cancel", "begin"]);
});

/** The launcher exception changes neither modal isolation nor preparation:
 * a dialog over New yields, and a hidden form still reserves recognized keys.
 */
test("launcher shortcuts consume keys only while it owns the visible modal", () => {
  const p = page();
  const form = new Element({ modal: true, visible: false });
  p.document.form = form;
  p.modals.push(form);
  const bolt = { code: "KeyP", ctrlKey: true, shiftKey: true };
  assert.equal(p.press(bolt).defaultPrevented, true);
  assert.deepEqual(p.clicks, []);
  form.visible = true;
  assert.equal(p.press(bolt).defaultPrevented, true);
  assert.equal(p.prefix().defaultPrevented, true);
  assert.equal(p.press({ code: "Digit1", isComposing: true }).defaultPrevented, false);
  assert.deepEqual(p.clicks, ["quick-bolt", "begin", "cancel"]);
  assert.equal(p.press({ code: "KeyJ", ctrlKey: true, shiftKey: true }).defaultPrevented, false);
  const other = new Element({ modal: true });
  p.modals.push(other);
  assert.equal(p.press(bolt).defaultPrevented, false);
  assert.equal(p.prefix().defaultPrevented, false);
  assert.deepEqual(p.clicks, ["quick-bolt", "begin", "cancel"]);
});

test("cancellation paths retire both owners once and allow the next terminal key", () => {
  const cancellation = [
    p => p.press({ code: "Escape", key: "Escape" }),
    p => p.window.dispatch("pointerdown"),
    p => p.window.dispatch("click"),
    p => p.window.dispatch("blur"),
    p => { p.document.hidden = true; p.document.dispatch("visibilitychange"); },
    p => p.window.cancelFarhelmLaunchSequence(),
    p => {
      const modal = new Element({ modal: true });
      p.modals.push(modal);
      p.window.dispatch("focusin", { target: new Element({ parent: modal }) });
    },
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

/** Focus cancellation follows the shared launcher context rather than the
 * focus target's modal ancestry. Moving among its controls keeps the queue.
 */
test("focus into a dialog admitted by the context rule keeps an armed sequence", () => {
  const p = page();
  p.prefix();
  const modal = new Element({ modal: true });
  p.modals.push(modal);
  assert.equal(templateShortcutContext(p.document), "yield");
  p.window.farhelmTemplateShortcutContext = () => "launcher";
  p.window.dispatch("focusin", { target: new Element({ parent: modal }) });
  assert.deepEqual(p.clicks, ["begin"]);
  assert.equal(p.press({ code: "Digit1" }).defaultPrevented, true);
  assert.deepEqual(p.clicks, ["begin", "digit1"]);
});

test("installer is idempotent and empty Enter spends ownership synchronously", () => {
  const p = page();
  p.window.installTemplateShortcuts({ ...defaultDefinition, mac_desktop: false });
  assert.equal(p.window.listeners.get("keydown").length, 1);
  assert.equal(p.document.listeners.get("visibilitychange").length, 1);
  p.prefix();
  p.press({ code: "Enter", key: "Enter" });
  assert.deepEqual(p.clicks, ["begin", "launch"]);
  assert.equal(p.press({ code: "Enter", key: "Enter" }).defaultPrevented, false);
});

/** Direct and sequence relays share one capture listener but have different
 * Rust callbacks. Check their observed clicks, not only classifier results.
 */
for (const client of clients) {
  test(`${client.name}: idle bolt and all number chords use live direct relays`, () => {
    const p = page(client);
    const bolt = p.press({ code: "KeyP", ...client.bolt });
    assert.equal(bolt.defaultPrevented, true);
    assert.equal(bolt.immediateStopped, true);
    for (const digit of "1234567890") {
      assert.equal(p.press({ code: `Digit${digit}`, key: "layout-specific", ...client.number })
        .defaultPrevented, true);
    }
    assert.deepEqual(p.clicks, ["quick-bolt", ...Array.from("1234567890", d => `quick-digit${d}`)]);
    assert.equal(p.press({ code: "KeyJ", ...client.bolt }).defaultPrevented, false,
      "the template list chord stays inactive until Cmd-J is implemented");
  });
}

/** A pending launch disables its relays without handing the chord to xterm.
 * An unmounted list has no such claim, including on repeat keydown events.
 */
test("direct shortcuts reserve disabled and repeated chords, but not missing relays", () => {
  const p = page();
  const bolt = { code: "KeyP", ctrlKey: true, shiftKey: true };
  const digit = { code: "Digit3", ctrlKey: true, shiftKey: true };
  p.direct.get("bolt").disabled = true;
  p.direct.get("digit3").disabled = true;
  for (const event of [bolt, digit]) {
    assert.equal(p.press(event).defaultPrevented, true);
    assert.equal(p.press({ ...event, repeat: true }).defaultPrevented, true);
  }
  assert.deepEqual(p.clicks, []);
  p.direct.delete("bolt");
  p.direct.delete("digit3");
  for (const event of [bolt, digit]) {
    assert.equal(p.press(event).defaultPrevented, false);
    assert.equal(p.press({ ...event, repeat: true }).defaultPrevented, false);
  }
});

/** The Rust dispatch will ask this same rule again after its fresh read.
 * A dialog over New must win over New's otherwise shortcut-capable context.
 */
test("one context rule separates page, hidden preparation, launcher and other modals", () => {
  const p = page();
  assert.equal(templateShortcutContext(p.document), "page");
  p.document.form = new Element({ modal: true, visible: false });
  assert.equal(templateShortcutContext(p.document), "preparing");
  p.modals.push(p.document.form);
  assert.equal(templateShortcutContext(p.document), "preparing");
  p.document.form.visible = true;
  assert.equal(templateShortcutContext(p.document), "launcher");
  p.document.form.savePanel = true;
  assert.equal(templateShortcutContext(p.document), "preparing");
  p.document.form.savePanel = false;
  const other = new Element({ modal: true });
  p.modals.push(other);
  assert.equal(templateShortcutContext(p.document), "yield");
  p.modals.splice(0, 1);
  assert.equal(templateShortcutContext(p.document), "yield");
  p.document.form = null;
  assert.equal(templateShortcutContext(p.document), "yield");
});

/** A cancelled save panel cannot authorize a chord that was pressed while it
 * owned the draft. The listener consumes it before Rust starts any read.
 */
test("save-as-template panel reserves template chords", () => {
  const p = page();
  const form = new Element({ modal: true, savePanel: true });
  p.document.form = form;
  p.modals.push(form);
  assert.equal(p.press({ code: "KeyP", ctrlKey: true, shiftKey: true }).defaultPrevented, true);
  assert.deepEqual(p.clicks, []);
  assert.equal(p.window.farhelmTakeTemplateShortcutPress(), undefined);
});

/** A later modal must revoke the exact physical press even when the worker
 * does not reach its relay until that modal has closed again.
 */
test("dispatch snapshots retain each press's opening epoch in order", () => {
  const p = page();
  const form = new Element({ modal: true });
  p.document.form = form;
  p.modals.push(form);
  p.window.__farhelmModalOpening = 4;
  p.press({ code: "KeyP", ctrlKey: true, shiftKey: true });
  p.window.__farhelmModalOpening = 5;
  p.press({ code: "Digit1", ctrlKey: true, shiftKey: true });
  const first = p.window.farhelmTakeTemplateShortcutPress();
  const second = p.window.farhelmTakeTemplateShortcutPress();
  assert.equal(first.context, "launcher");
  assert.equal(first.modal_opening, 4);
  assert.equal(second.context, "launcher");
  assert.equal(second.modal_opening, 5);
  assert.equal(p.window.farhelmTakeTemplateShortcutPress(), undefined);
});

/** Move all physical keys and give numbers a Ctrl+Alt chord to test modifier
 * masks independently of the default shortcuts and the build selector.
 */
function changedDefinition() {
  const changedNumberModifiers = modifiers("mac_desktop_build");
  changedNumberModifiers.other = {
    meta: false, ctrl: true, shift: false, alt: true,
  };
  return {
    bolt: { code: "KeyB", modifiers: modifiers("platform") },
    number: { keys: Array.from("CDEFGHIMNO", (letter, index) =>
      ({ code: `Key${letter}`, value: String((index + 1) % 10) })), modifiers: changedNumberModifiers },
    prefix: { code: "KeyQ", modifiers: modifiers("mac_desktop_build") },
    template_list: { code: "KeyL", modifiers: modifiers("platform") },
  };
}

/** Changing key codes in this fixture must move every action without editing
 * the matcher. Both admitted contexts use the same definition and relays.
 */
test("a non-default definition moves all shortcuts in page and launcher contexts", () => {
  const changed = changedDefinition();
  const client = clients[1];
  for (const context of ["page", "launcher"]) {
    const p = page(client, changed);
    if (context === "launcher") {
      p.document.form = new Element({ modal: true });
      p.modals.push(p.document.form);
    }
    assert.equal(p.window.farhelmTemplateShortcutContext(), context);
    const bolt = { code: "KeyB", ...client.bolt };
    const numbers = changed.number.keys.map(({ code, value }) =>
      ({ code, value, ctrlKey: true, altKey: true }));
    const prefix = { code: "KeyQ", ...client.number };
    const list = { code: "KeyL", ...client.bolt };
    assert.deepEqual(classify(client, bolt, false, false, changed),
      { action: "quick-bolt", consume: true });
    for (const { code, value, ...modifiers } of numbers) {
      assert.deepEqual(classify(client, { code, ...modifiers }, false, false, changed),
        { action: "quick-digit", consume: true, digit: value });
      assert.deepEqual(classify(client, { code, ...client.number }, false, false, changed),
        { action: "pass", consume: false });
      assert.deepEqual(classify(client, { code }, true, false, changed),
        { action: "digit", consume: true, digit: value });
    }
    assert.deepEqual(classify(client, prefix, false, false, changed),
      { action: "begin", consume: true });
    assert.deepEqual(classify(client, bolt, true, false, changed),
      { action: "bolt", consume: true });
    assert.deepEqual(classify(client, { code: "Enter", key: "Enter" }, true, false, changed),
      { action: "launch", consume: true });
    assert.deepEqual(classify(client, { code: "Escape", key: "Escape" }, true, false, changed),
      { action: "cancel", consume: true });
    assert.deepEqual(classify(client, list, false, false, changed),
      { action: "template-list", consume: false });
    for (const old of [{ code: "KeyP", ...client.bolt },
      { code: "Digit1", ...client.number }, { code: "KeyX", ...client.number },
      { code: "KeyJ", ...client.bolt }]) {
      assert.deepEqual(classify(client, old, false, false, changed), { action: "pass", consume: false });
      assert.equal(p.press(old).defaultPrevented, false);
    }
    assert.equal(p.press({ code: "KeyC", ...client.number }).defaultPrevented, false,
      "the installer must also reject the old Ctrl+Shift mask on the moved number key");
    for (const event of [bolt, ...numbers, prefix]) {
      assert.equal(p.press(event).defaultPrevented, true);
    }
    assert.equal(p.press(list).defaultPrevented, false);
    p.press({ code: "KeyZ" });
    p.press(prefix);
    assert.deepEqual(classify(client, { code: "KeyC" }, true, false, changed),
      { action: "digit", consume: true, digit: "1" });
    assert.equal(p.press({ code: "KeyC" }).defaultPrevented, true);
    assert.equal(p.press(bolt).defaultPrevented, true);
    assert.equal(p.press({ code: "Enter", key: "Enter" }).defaultPrevented, true);
    assert.deepEqual(p.clicks,
      ["quick-bolt", ...changed.number.keys.map(key => `quick-digit${key.value}`),
        "begin", "cancel", "begin", "digit1", "bolt", "launch"]);
  }
});

/** The modifier keydowns preceding a configured Ctrl+Alt number must not
 * cancel an armed sequence, regardless of which modifier the user holds first.
 * An Alt chord absent from the definition remains unrelated input.
 */
test("armed sequences keep configured Alt chords in either modifier order", () => {
  const client = clients[1];
  const changed = changedDefinition();
  const alt = { code: "AltLeft", key: "Alt", altKey: true };
  const control = { code: "ControlLeft", key: "Control", ctrlKey: true };
  const controlAfterAlt = { ...control, altKey: true };
  const altAfterControl = { ...alt, ctrlKey: true };
  for (const lead of [[alt, controlAfterAlt], [control, altAfterControl]]) {
    const p = page(client, changed);
    assert.equal(p.press({ code: "KeyQ", ...client.number }).defaultPrevented, true);
    for (const event of lead) {
      assert.deepEqual(classify(client, event, true, false, changed),
        { action: "pass", consume: false });
      assert.equal(p.press(event).defaultPrevented, false);
      assert.deepEqual(p.clicks, ["begin"], "partial configured chord keeps the sequence armed");
    }
    assert.equal(p.press({ code: "KeyC", ctrlKey: true, altKey: true }).defaultPrevented, true);
    assert.equal(p.press({ code: "Enter", key: "Enter" }).defaultPrevented, true);
    assert.deepEqual(p.clicks, ["begin", "digit1", "launch"]);
  }

  const unconfigured = page(client);
  unconfigured.prefix();
  assert.deepEqual(classify(client, alt, true), { action: "cancel", consume: false });
  assert.equal(unconfigured.press(alt).defaultPrevented, false);
  assert.deepEqual(unconfigured.clicks, ["begin", "cancel"]);
  const alsoUnconfigured = page(client);
  alsoUnconfigured.prefix();
  assert.deepEqual(classify(client, controlAfterAlt, true), { action: "cancel", consume: false });
  assert.equal(alsoUnconfigured.press(controlAfterAlt).defaultPrevented, false);
  assert.deepEqual(alsoUnconfigured.clicks, ["begin", "cancel"]);

  // A shortcut that cannot queue a template must not admit Alt while armed.
  for (const field of ["prefix", "template_list"]) {
    const alternate = { ...defaultDefinition,
      [field]: { ...defaultDefinition[field], modifiers: {
        ...defaultDefinition[field].modifiers,
        other: { meta: false, ctrl: true, shift: true, alt: true },
      } },
    };
    const browser = clients[2];
    const p = page(browser, alternate);
    const begin = field === "prefix" ? { code: "KeyX", ...browser.number, altKey: true } :
      { code: "KeyX", ...browser.number };
    assert.equal(p.press(begin).defaultPrevented, true, "the changed prefix must arm before Alt is tested");
    assert.deepEqual(p.clicks, ["begin"]);
    assert.deepEqual(classify(browser, alt, true, false, alternate), { action: "cancel", consume: false });
    assert.equal(p.press(alt).defaultPrevented, false);
    assert.deepEqual(p.clicks, ["begin", "cancel"]);
  }
});
