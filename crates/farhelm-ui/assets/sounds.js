// Sounds belong to this client: neither its switches nor its audio context
// cross the helm boundary. Rust supplies accepted sidebar snapshots; these pure
// rules choose an event before Web Audio attempts best-effort playback.
(() => {
  const DEFAULTS = { waiting: true, approval: true, finished: false };
  const PRIORITY = ["approval", "waiting", "finished"];

  /** Decode each device switch independently; damaged storage is not an opt-out. */
  function readSettings(storage) {
    const settings = { ...DEFAULTS };
    for (const kind of PRIORITY) {
      try {
        const value = storage.getItem(`farhelm.sound.${kind}`);
        if (value === "true" || value === "false") settings[kind] = value === "true";
      } catch (_) { /* Storage refusal leaves this event's documented default. */ }
    }
    return settings;
  }

  /** Classify changes, never the initial snapshot or a disappeared session.
   *
   * Approval ids are the client's entire seen history, not just the requests
   * still waiting: a repeated id cannot ring again after leaving the list.
   * New sessions only ring when already Waiting; Idle alone is no evidence
   * that this client witnessed a completed turn.
   */
  function detectEvents(previous, current) {
    if (!previous) return [];
    const events = [];
    const seen = new Set(previous.approvalIds);
    for (const request of current.approvals) {
      if (!seen.has(request.id)) events.push({ kind: "approval", session: request.session });
    }
    for (const [session, status] of Object.entries(current.statuses)) {
      const before = previous.statuses[session];
      if (status === "waiting" && before !== "waiting") events.push({ kind: "waiting", session });
      else if (status === "idle" && before === "running") events.push({ kind: "finished", session });
    }
    return events;
  }

  /** Quiet and disabled events cannot hide an audible, less urgent event.
   * One snapshot chooses at most one sound, regardless of fleet size.
   */
  function chooseSound(events, settings, active, selected) {
    return PRIORITY.find((kind) => settings[kind] && events.some((event) =>
      event.kind === kind && !(active && event.session === selected))) || null;
  }

  // The approved audition's notes and fixed master level. Overtones use the
  // same envelope as their fundamental, so a bell's timbre stays consistent.
  const NOTES = {
    waiting: [{ f: 784, t: 0, d: 0.7, a: 0.4, bell: true }, { f: 1047, t: 0.18, d: 0.9, a: 0.35, bell: true }],
    approval: [{ f: 880, t: 0, d: 0.6, a: 0.4, bell: true }, { f: 698, t: 0.25, d: 0.9, a: 0.4, bell: true }],
    finished: [{ f: 523, t: 0, d: 0.35, a: 0.45, wave: "triangle" }],
  };

  /** One page context, with no queue: suspended or refused playback is lost.
   * The constructor is injected so the shipped envelopes and refusal boundary
   * can be exercised without an audio device or a browser permission prompt.
   */
  function createPlayer(AudioContext) {
    let context;
    let master;
    return {
      /** Try desktop autoplay, and retry/resume on trusted page gestures. */
      unlock() {
        try {
          if (!context) {
            context = new AudioContext();
            master = context.createGain();
            master.gain.value = 0.3;
            master.connect(context.destination);
          }
          if (context.state !== "running") context.resume().catch(() => {});
        } catch (_) { /* No audio support is a silent best-effort refusal. */ }
      },
      /** Schedule only this event; later gestures never replay an earlier one. */
      play(kind) {
        if (!context || context.state !== "running") return;
        try {
          const start = context.currentTime + 0.02;
          for (const note of NOTES[kind]) {
            const tones = [[1, 1], ...(note.bell ? [[2, 0.25], [3, 0.08]] : [])];
            for (const [ratio, relative] of tones) {
              const oscillator = context.createOscillator();
              const gain = context.createGain();
              oscillator.type = note.wave || "sine";
              oscillator.frequency.value = note.f * ratio;
              gain.gain.setValueAtTime(0.0001, start + note.t);
              gain.gain.exponentialRampToValueAtTime(note.a * relative, start + note.t + 0.005);
              gain.gain.exponentialRampToValueAtTime(0.0001, start + note.t + note.d);
              oscillator.connect(gain);
              gain.connect(master);
              oscillator.start(start + note.t);
              oscillator.stop(start + note.t + note.d + 0.05);
            }
          }
        } catch (_) { /* Device or context failure never becomes a GUI error. */ }
      },
    };
  }

  const api = { readSettings, detectEvents, chooseSound, createPlayer };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  if (typeof window === "undefined" || window.farhelmSounds) return;
  let storage;
  try { storage = window.localStorage; } catch (_) { /* Defaults still work. */ }
  const settings = readSettings(storage);
  const player = createPlayer(window.AudioContext || window.webkitAudioContext);
  player.unlock();
  const unlock = (event) => { if (event.isTrusted) player.unlock(); };
  // Touch grants transient activation when the contact ends, not on press.
  // Click also covers browsers whose touch input does not emit pointer events.
  window.addEventListener("pointerup", unlock, true);
  window.addEventListener("click", unlock, true);
  window.addEventListener("keydown", unlock, true);

  /** JS owns checked properties, like terminal text size's stateless controls.
   * Rust can rerender other Settings choices without writing these unchanged
   * properties back. A failed store keeps the choice for this page's lifetime.
   */
  function mountSettings() {
    for (const input of document.querySelectorAll("input[data-sound-event]")) {
      input.checked = settings[input.dataset.soundEvent];
    }
  }
  window.addEventListener("change", (event) => {
    const input = event.target;
    const kind = input.dataset?.soundEvent;
    if (!PRIORITY.includes(kind)) return;
    settings[kind] = input.checked;
    try { storage.setItem(`farhelm.sound.${kind}`, String(input.checked)); } catch (_) { /* Keep the local choice. */ }
  });
  window.farhelmSounds = {
    mountSettings,
    /** Selection and focus are read when a reply is applied, not when sent. */
    observe(previous, current, selected) {
      const kind = chooseSound(detectEvents(previous, current), settings,
        document.documentElement.dataset.windowActive === "true", selected);
      if (kind) player.play(kind);
    },
  };
})();
