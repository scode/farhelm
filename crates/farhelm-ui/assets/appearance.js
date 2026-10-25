// Appearance belongs to this browser or desktop webview, never the helm.
// This classic script runs in the head before application content can paint;
// the root records the resolved palette, while this owner retains the choice.
(() => {
  const KEY = "farhelm.appearance";
  const CHOICES = ["system", "dark", "light"];
  const CONTROLS = 'input[type="radio"][data-appearance-choice]';

  /** Missing or damaged device storage restores the default System choice. */
  function decodeChoice(raw) {
    return CHOICES.includes(raw) ? raw : "system";
  }

  /** Explicit choices ignore the OS; an unavailable system query means Dark. */
  function resolveAppearance(choice, systemLight) {
    return choice === "system" ? (systemLight ? "light" : "dark") : choice;
  }

  /** Install one page owner, even when the same asset is evaluated again.
   *
   * Window and document are supplied so tests can exercise storage refusal,
   * media changes and control remounts without changing process globals.
   * The only public operation synchronizes newly mounted native radios.
   */
  function installAppearance(window, document) {
    if (window.farhelmAppearance) return window.farhelmAppearance;
    let storage;
    let media;
    let choice = "system";
    try {
      storage = window.localStorage;
      choice = decodeChoice(storage.getItem(KEY));
    } catch (_) { /* Refused storage keeps the default for this page. */ }
    try {
      media = window.matchMedia?.("(prefers-color-scheme: light)");
    } catch (_) { /* An unavailable system query resolves System to Dark. */ }

    /** Rust renders labels and inputs, but never a second copy of checked state.
     * Hidden Settings panels and newly mounted dialogs receive the same choice.
     */
    function mountSettings() {
      for (const input of document.querySelectorAll(CONTROLS)) {
        input.checked = input.dataset.appearanceChoice === choice;
      }
    }

    /** Apply synchronously before persistence or any renderer update can lag it. */
    function apply() {
      document.documentElement.dataset.appearance = resolveAppearance(choice, media?.matches === true);
      mountSettings();
    }

    // This first application is independent of Settings and authentication.
    // It must happen during head parsing, before the renderer creates content.
    apply();
    // Keep one subscription through overrides, so returning to System uses the
    // current query without replacing listeners or remembering a stale result.
    const systemChanged = () => {
      if (choice === "system") apply();
    };
    if (media?.addEventListener) media.addEventListener("change", systemChanged);
    else if (media?.addListener) media.addListener(systemChanged);

    window.addEventListener("change", (event) => {
      const input = event.target;
      const next = input?.dataset?.appearanceChoice;
      if (input?.type !== "radio" || !input.checked || !CHOICES.includes(next)) return;
      choice = next;
      apply();
      try { storage.setItem(KEY, choice); } catch (_) { /* Keep the current page's choice. */ }
    });
    window.addEventListener("storage", (event) => {
      // sessionStorage can emit the same key; only localStorage owns appearance.
      if (event.storageArea && event.storageArea !== storage) return;
      if (event.key !== KEY && event.key !== null) return;
      // The event carries the other page's accepted value, including removal.
      // A clear event has no key, and restores System without another read.
      choice = decodeChoice(event.key === null ? null : event.newValue);
      apply();
    });
    window.farhelmAppearance = { mountSettings };
    return window.farhelmAppearance;
  }

  if (typeof module !== "undefined" && module.exports) {
    module.exports = { decodeChoice, resolveAppearance, installAppearance };
  }
  if (typeof window !== "undefined" && typeof document !== "undefined") installAppearance(window, document);
})();
