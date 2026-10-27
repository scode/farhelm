// Exercise the shipped page owner with injected browser objects. Browser tests
// separately cover native radio keyboard behavior and renderer mount timing.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const { decodeChoice, resolveAppearance, installAppearance } = require("../assets/appearance.js");
const source = fs.readFileSync(require.resolve("../assets/appearance.js"), "utf8");
const KEY = "farhelm.appearance";
const CHOICES = ["system", "dark", "light"];

/** Model only browser boundaries; the actual asset owns every state transition.
 * Controls start absent, as they are when the head script runs. Tests explicitly
 * mount or remove them to distinguish page lifetime from Settings lifetime.
 */
function browserFixture(options = {}) {
  const listeners = new Map();
  const mediaListeners = [];
  const queries = [];
  const reads = [];
  const writes = [];
  const values = new Map([[KEY, options.stored ?? null]]);
  const document = {
    documentElement: { dataset: {} },
    controls: [],
    querySelectorAll(selector) {
      assert.equal(selector, 'input[type="radio"][data-appearance-choice]');
      return this.controls;
    },
  };
  const ownStorage = {
    getItem(key) {
      reads.push(key);
      if (options.refuseRead) throw new Error("storage read refused");
      return values.get(key) ?? null;
    },
    setItem(key, value) {
      writes.push({ key, value, palette: document.documentElement.dataset.appearance,
        checked: document.controls.filter((input) => input.checked).map((input) => input.dataset.appearanceChoice) });
      if (options.refuseWrite) throw new Error("storage write refused");
      values.set(key, value);
    },
  };
  const storage = options.storage ?? ownStorage;
  const media = {
    matches: options.systemLight === true,
    addEventListener(event, listener) {
      assert.equal(event, "change");
      mediaListeners.push(listener);
    },
  };
  const window = {
    get localStorage() {
      if (options.refuseStorage) throw new Error("storage access refused");
      return storage;
    },
    addEventListener(event, listener) {
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push(listener);
    },
  };
  if (!options.noMedia) {
    window.matchMedia = (query) => {
      queries.push(query);
      if (options.refuseMedia) throw new Error("media unavailable");
      return media;
    };
  }
  const fixture = {
    window, document, media, storage, listeners, mediaListeners, queries, reads, writes,
    install: () => installAppearance(window, document),
    emit(event, value) {
      for (const listener of listeners.get(event) ?? []) listener(value);
    },
    system(light) {
      media.matches = light;
      for (const listener of mediaListeners) listener({ matches: light });
    },
    mount() {
      document.controls = CHOICES.map((choice) => ({ type: "radio", checked: false,
        dataset: { appearanceChoice: choice } }));
      window.farhelmAppearance.mountSettings();
    },
    choose(choice) {
      const input = document.controls.find((input) => input.dataset.appearanceChoice === choice);
      assert.ok(input, "the requested radio is mounted");
      input.checked = true;
      fixture.emit("change", { target: input });
    },
    external(value, key = KEY, storageArea = storage) {
      fixture.emit("storage", { key, newValue: value, storageArea });
    },
  };
  return fixture;
}

/** Check both the visible palette and exclusive native checked properties. */
function assertAppearance(fixture, palette, choice) {
  assert.equal(fixture.document.documentElement.dataset.appearance, palette);
  assert.deepEqual(fixture.document.controls.filter((input) => input.checked)
    .map((input) => input.dataset.appearanceChoice), [choice]);
}

/** Corrupt values cannot create an unsupported palette or disable System. */
test("only exact stored choices are accepted", () => {
  for (const value of [null, undefined, "", "Dark", " light ", "auto", "{}", true, 1]) {
    assert.equal(decodeChoice(value), "system");
  }
  for (const value of CHOICES) assert.equal(decodeChoice(value), value);
  assert.equal(resolveAppearance("system", true), "light");
  assert.equal(resolveAppearance("system", false), "dark");
  assert.equal(resolveAppearance("dark", true), "dark");
  assert.equal(resolveAppearance("light", false), "light");
});

/** First application happens during script parsing, before Settings exists. */
test("classic script resolves initial appearance before controls mount", () => {
  for (const [options, palette] of [
    [{ systemLight: true }, "light"],
    [{ systemLight: false }, "dark"],
    [{ systemLight: true, stored: "dark" }, "dark"],
    [{ systemLight: false, stored: "light" }, "light"],
    [{ systemLight: true, stored: "invalid" }, "light"],
  ]) {
    const fixture = browserFixture(options);
    assert.equal(fixture.document.controls.length, 0);
    vm.runInNewContext(source, { window: fixture.window, document: fixture.document });
    assert.equal(fixture.document.documentElement.dataset.appearance, palette);
    assert.deepEqual(Object.keys(fixture.window.farhelmAppearance), ["mountSettings"]);
    assert.deepEqual(fixture.queries, ["(prefers-color-scheme: light)"]);
  }
});

/** Privacy refusals use System; missing or refused media queries use Dark. */
test("storage and media refusals preserve the documented defaults", () => {
  for (const options of [{ refuseStorage: true }, { refuseRead: true }, { stored: "invalid" }]) {
    const fixture = browserFixture({ systemLight: true, stored: "dark", ...options });
    fixture.install();
    fixture.mount();
    assertAppearance(fixture, "light", "system");
  }
  for (const options of [{ noMedia: true }, { refuseMedia: true }]) {
    const fixture = browserFixture({ systemLight: true, ...options });
    fixture.install();
    fixture.mount();
    assertAppearance(fixture, "dark", "system");
    fixture.choose("light");
    assertAppearance(fixture, "light", "light");
  }
});

/** System remains live even when Settings is absent; explicit overrides are stable. */
test("system changes affect only the System choice", () => {
  const fixture = browserFixture();
  fixture.install();
  fixture.system(true);
  assert.equal(fixture.document.documentElement.dataset.appearance, "light");
  fixture.mount();
  assertAppearance(fixture, "light", "system");
  fixture.choose("dark");
  fixture.system(false);
  fixture.system(true);
  assertAppearance(fixture, "dark", "dark");
  fixture.choose("light");
  fixture.system(false);
  assertAppearance(fixture, "light", "light");
  fixture.choose("system");
  assertAppearance(fixture, "dark", "system");
  fixture.system(true);
  assertAppearance(fixture, "light", "system");
});

/** All three choices update root and radios before the best-effort write. */
test("native changes synchronously select and persist every choice", () => {
  const fixture = browserFixture({ systemLight: true });
  fixture.install();
  fixture.mount();
  for (const choice of ["dark", "light", "system"]) {
    fixture.choose(choice);
    const palette = choice === "system" ? "light" : choice;
    assertAppearance(fixture, palette, choice);
    assert.deepEqual(fixture.writes.at(-1), { key: KEY, value: choice, palette, checked: [choice] });
    const reload = browserFixture({ storage: fixture.storage });
    reload.install();
    reload.mount();
    assertAppearance(reload, choice === "system" ? "dark" : choice, choice);
  }
  assert.deepEqual(fixture.reads, [KEY, KEY, KEY, KEY]);
});

/** A storage denial must not undo a working page's immediately applied choice. */
test("write refusal retains page choice through system changes and remount", () => {
  for (const options of [{ refuseWrite: true }, { refuseStorage: true }]) {
    const fixture = browserFixture(options);
    fixture.install();
    fixture.mount();
    fixture.choose("light");
    fixture.system(false);
    assertAppearance(fixture, "light", "light");
    fixture.document.controls = [];
    fixture.mount();
    assertAppearance(fixture, "light", "light");
    fixture.choose("system");
    fixture.system(true);
    assertAppearance(fixture, "light", "system");
  }
});

/** Other same-origin pages can override a mounted or closed Settings dialog. */
test("storage updates removals invalid values and clear synchronize page and controls", () => {
  const fixture = browserFixture({ systemLight: true });
  fixture.install();
  fixture.mount();
  for (const choice of CHOICES) {
    fixture.external(choice);
    assertAppearance(fixture, choice === "system" ? "light" : choice, choice);
  }
  fixture.external("dark");
  fixture.external("light", "unrelated-setting");
  fixture.external("light", KEY, {});
  assertAppearance(fixture, "dark", "dark");
  for (const value of ["invalid", null]) {
    fixture.external("dark");
    fixture.external(value);
    assertAppearance(fixture, "light", "system");
  }
  fixture.external("dark");
  fixture.external(null, null);
  assertAppearance(fixture, "light", "system");
  fixture.document.controls = [];
  fixture.external("dark");
  assert.equal(fixture.document.documentElement.dataset.appearance, "dark");
  fixture.mount();
  assertAppearance(fixture, "dark", "dark");
  assert.equal(fixture.writes.length, 0, "external changes must not write back to storage");
});

/** Mounting and repeated script evaluation cannot multiply page listeners. */
test("one owner survives remounts and duplicate classic script evaluation", () => {
  const fixture = browserFixture();
  const owner = fixture.install();
  fixture.mount();
  fixture.choose("light");
  fixture.document.controls = [];
  fixture.mount();
  assert.equal(fixture.install(), owner);
  vm.runInNewContext(source, { window: fixture.window, document: fixture.document });
  assert.equal(fixture.window.farhelmAppearance, owner);
  assertAppearance(fixture, "light", "light");
  assert.equal(fixture.mediaListeners.length, 1);
  assert.equal(fixture.listeners.get("change").length, 1);
  assert.equal(fixture.listeners.get("storage").length, 1);
  assert.equal(fixture.queries.length, 1);
  assert.equal(fixture.reads.length, 1);
});

/** Delegation must not turn unrelated controls or an unchecked radio into a choice. */
test("only checked appearance radios can change the page choice", () => {
  const fixture = browserFixture();
  fixture.install();
  fixture.mount();
  for (const target of [null, {}, { type: "checkbox", checked: true, dataset: { appearanceChoice: "light" } },
    { type: "radio", checked: false, dataset: { appearanceChoice: "light" } },
    { type: "radio", checked: true, dataset: { appearanceChoice: "invalid" } }]) {
    fixture.emit("change", { target });
  }
  assertAppearance(fixture, "dark", "system");
  assert.equal(fixture.writes.length, 0);
});
