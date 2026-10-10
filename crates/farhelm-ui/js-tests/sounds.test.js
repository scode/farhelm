// Exercise the shipped attention rules without audio hardware. Browser cases
// separately prove that actual feed reads and Settings reach these rules.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const { detectEvents, chooseSound, readSettings, createPlayer } = require("../assets/sounds.js");
const defaults = { waiting: true, approval: true, finished: false };
const current = (statuses = {}, approvals = []) => ({ statuses, approvals });
const previous = (statuses = {}, approvalIds = []) => ({ statuses, approvalIds });

/** Existing attention at sign-in is a baseline, never a chorus of old events. */
test("first read is quiet even with waiting sessions and approvals", () => {
  assert.deepEqual(detectEvents(null, current({ a: "waiting" }, [{ id: "ask", session: "a" }])), []);
});

/** Waiting is an attention edge from every other status, including a new row. */
test("waiting rings on entry but not on a repeated waiting listing", () => {
  for (const before of [undefined, "running", "idle", "other", "waiting"]) {
    assert.deepEqual(detectEvents(previous(before ? { a: before } : {}), current({ a: "waiting" })),
      before === "waiting" ? [] : [{ kind: "waiting", session: "a" }]);
  }
});

/** Completion requires an observed Running-to-Idle edge, not absence or an exit. */
test("turn finished excludes exits errors interruptions and new idle sessions", () => {
  assert.deepEqual(detectEvents(previous({ a: "running" }), current({ a: "idle" })), [{ kind: "finished", session: "a" }]);
  for (const status of ["running", "other", "idle"]) {
    assert.deepEqual(detectEvents(previous(), current({ a: status })), []);
  }
  assert.deepEqual(detectEvents(previous({ a: "running" }), current({ a: "other" })), []);
  assert.deepEqual(detectEvents(previous({ a: "running" }), current()), []);
  assert.deepEqual(detectEvents(previous({ a: "running" }), current({ a: "waiting" })), [{ kind: "waiting", session: "a" }]);
});

/** Request identity survives removal; newly seen approvals keep their asking session. */
test("only unseen approval ids ring", () => {
  assert.deepEqual(detectEvents(previous({}, ["old"]), current({}, [
    { id: "old", session: "a" }, { id: "new", session: "b" },
  ])), [{ kind: "approval", session: "b" }]);
});

/** Quieting is conjunctive: the open session in an inactive window still needs sound. */
test("only the selected session in an active window is quiet", () => {
  for (const kind of ["waiting", "approval", "finished"]) {
    const events = [{ kind, session: "a" }];
    const enabled = { ...defaults, finished: true };
    assert.equal(chooseSound(events, enabled, true, "a"), null);
    assert.equal(chooseSound(events, enabled, false, "a"), kind);
    assert.equal(chooseSound(events, enabled, true, "b"), kind);
    assert.equal(chooseSound(events, enabled, true, null), kind);
  }
});

/** A quiet or disabled high-priority event cannot suppress a useful lower one. */
test("burst priority follows per-event switches and per-session quiet rules", () => {
  const events = [
    { kind: "finished", session: "a" }, { kind: "waiting", session: "b" },
    { kind: "waiting", session: "c" }, { kind: "approval", session: "d" },
  ];
  assert.equal(chooseSound(events, defaults, false, null), "approval");
  assert.equal(chooseSound(events, defaults, true, "d"), "waiting");
  assert.equal(chooseSound(events, { ...defaults, approval: false }, false, null), "waiting");
  assert.equal(chooseSound(events, { waiting: false, approval: false, finished: true }, false, null), "finished");
  assert.equal(chooseSound(events, { waiting: false, approval: false, finished: false }, false, null), null);
});

/** Each preference fails independently; garbage must not become a silent opt-out. */
test("device preferences accept only boolean strings with independent defaults", () => {
  assert.deepEqual(readSettings(undefined), defaults);
  for (const value of [null, "", "1", "TRUE", "null", "{}", " false "]) {
    assert.deepEqual(readSettings({ getItem: () => value }), defaults);
  }
  assert.deepEqual(readSettings({ getItem: (key) => {
    if (key.endsWith("waiting")) return "false";
    if (key.endsWith("finished")) return "true";
    throw new Error("storage inaccessible");
  } }), { waiting: false, approval: true, finished: true });
});

/** Record real oscillator starts rather than replacing the sound scheduling function. */
function audioFixture(state = "running") {
  const oscillators = [];
  const gains = [];
  let instance;
  let contexts = 0;
  class AudioContext {
    constructor() { contexts++; instance = this; this.state = state; this.currentTime = 10; this.destination = {}; }
    resume() { return Promise.reject(new Error("gesture required")); }
    createGain() {
      const node = { gain: { value: 0, calls: [],
        setValueAtTime(...args) { this.calls.push(args); },
        exponentialRampToValueAtTime(...args) { this.calls.push(args); } }, connect() {} };
      gains.push(node); return node;
    }
    createOscillator() {
      const node = { frequency: { value: 0 }, type: "", connect() {}, start(at) { this.startAt = at; }, stop(at) { this.stopAt = at; } };
      oscillators.push(node); return node;
    }
  }
  return { AudioContext, player: createPlayer(AudioContext), oscillators, gains, context: () => instance, contexts: () => contexts };
}

/** A blocked event is discarded: unlocking later plays only a subsequent event. */
test("suspended playback is silent and never queued", async () => {
  const fixture = audioFixture("suspended");
  fixture.player.unlock();
  fixture.player.play("waiting");
  assert.equal(fixture.oscillators.length, 0);
  fixture.context().state = "running";
  fixture.player.unlock();
  assert.equal(fixture.oscillators.length, 0);
  fixture.player.play("finished");
  assert.equal(fixture.oscillators.length, 1);
  assert.equal(fixture.contexts(), 1);
  await Promise.resolve();
});

/** Distinct approved sounds keep their audition pitches, overtones and fixed level. */
test("bell and pluck scheduling preserves the approved sound signatures", () => {
  for (const [kind, pitches] of [
    ["waiting", [784, 1568, 2352, 1047, 2094, 3141]],
    ["approval", [880, 1760, 2640, 698, 1396, 2094]],
    ["finished", [523]],
  ]) {
    const fixture = audioFixture();
    fixture.player.unlock(); fixture.player.play(kind);
    assert.deepEqual(fixture.oscillators.map((o) => o.frequency.value), pitches);
    assert.equal(fixture.gains[0].gain.value, 0.3);
    assert.equal(fixture.oscillators[0].startAt, 10.02);
    assert.equal(fixture.oscillators[0].type, kind === "finished" ? "triangle" : "sine");
    assert.ok(fixture.oscillators.every((o) => o.stopAt > o.startAt));
  }
});

/** Missing audio support never becomes a client error, even on repeated gestures. */
test("unavailable audio context is a silent refusal", () => {
  const player = createPlayer(undefined);
  assert.doesNotThrow(() => { player.unlock(); player.play("approval"); player.unlock(); });
});

/** Touch permission starts on release or click. A press-only registration
 * leaves mobile browsers suspended forever, while synthetic input must not
 * serve as the gesture that unlocks audio.
 */
test("trusted release and click unlock audio that initial autoplay refused", async () => {
  for (const event of ["pointerup", "click"]) {
    const audio = audioFixture("suspended");
    let permission = false;
    audio.AudioContext.prototype.resume = function () {
      if (!permission) return Promise.reject(new Error("gesture required"));
      this.state = "running";
      return Promise.resolve();
    };
    const listeners = {};
    const window = {
      AudioContext: audio.AudioContext,
      addEventListener(kind, callback) { listeners[kind] = callback; },
    };
    const document = { documentElement: { dataset: { windowActive: "false" } } };
    vm.runInNewContext(fs.readFileSync(require.resolve("../assets/sounds.js"), "utf8"), { window, document });
    window.farhelmSounds.observe(previous({ a: "running" }), current({ a: "waiting" }), null);
    assert.equal(audio.oscillators.length, 0, "autoplay must first be refused");
    assert.equal(typeof listeners[event], "function", `${event} must be registered`);
    permission = true;
    listeners[event]({ isTrusted: false });
    window.farhelmSounds.observe(previous({ a: "running" }), current({ a: "waiting" }), null);
    assert.equal(audio.oscillators.length, 0, "synthetic input cannot unlock the context");
    listeners[event]({ isTrusted: true });
    window.farhelmSounds.observe(previous({ a: "running" }), current({ a: "waiting" }), null);
    assert.equal(audio.oscillators.length, 6, `${event} allows a later waiting sound`);
    await Promise.resolve();
  }
});

/** Refused device writes keep the visible switch and its effect for this page. */
test("storage refusal does not undo live device switches", () => {
  const audio = audioFixture();
  const listeners = {};
  const input = { dataset: { soundEvent: "waiting" }, checked: false };
  const window = {
    AudioContext: audio.AudioContext,
    localStorage: { getItem() { throw new Error("read refused"); }, setItem() { throw new Error("write refused"); } },
    addEventListener(kind, callback) { listeners[kind] = callback; },
  };
  const document = { documentElement: { dataset: { windowActive: "false" } }, querySelectorAll: () => [input] };
  vm.runInNewContext(fs.readFileSync(require.resolve("../assets/sounds.js"), "utf8"), { window, document });
  window.farhelmSounds.mountSettings();
  assert.equal(input.checked, true, "unreadable storage starts with the waiting default");
  window.farhelmSounds.observe(previous({ a: "running" }), current({ a: "waiting" }), null);
  assert.equal(audio.oscillators.length, 6, "the enabled waiting default really reaches audio");
  input.checked = false;
  assert.doesNotThrow(() => listeners.change({ target: input }));
  window.farhelmSounds.mountSettings();
  assert.equal(input.checked, false, "opening Settings again preserves the unsaved page choice");
  window.farhelmSounds.observe(previous({ a: "running" }), current({ a: "waiting" }), null);
  assert.equal(audio.oscillators.length, 6, "disabled waiting adds no sound despite write refusal");
});
