// Browser-level coverage for terminal.js's pre-mount font gate ("## Font
// settling before mount" in its own header): JetBrains Mono Nerd Font is
// a bundled asset, so failing to load it is a broken deployment — the
// terminal must not mount until BOTH weights have loaded, and a failed
// download leaves it unmounted with exactly one console error. This file
// proves that contract holds, through the real Font Loading API neither a
// `node --test` unit nor a read of the source can exercise:
//
// - F7a (fail-closed on rejection): a rejected font download leaves the
//   terminal unmounted and logs exactly one Farhelm font error. This is
//   the only coverage of the failed-download decision, and it also proves
//   each engine rejects rather than answering with an empty list (which
//   would loop silently and never log).
// - F7b (held request): bold held at the network layer keeps the terminal
//   unmounted past the OLD 3 s settle deadline even after regular loads;
//   once bold is released the terminal mounts and renders real output in
//   the JetBrains Mono family. The family check reads the constructor's
//   option back after release — since the family is unconditional, that
//   read is a literal check, folded in here rather than kept as its own
//   test.
//
// Its own file, per this suite's standing convention for new coverage
// (mouse-modes.spec.ts's header) — one subject, findable and runnable
// together. Unlike terminal-clipboard.spec.ts, this file needs no special
// browser permission and runs on BOTH engines. The two source text-size
// cases (four executions across two engines) at the top are the original
// limit-button focus coverage and are unaffected by the font gate.
//
// ## Failure is simulated at the NETWORK layer, not by monkey-patching
//    `document.fonts.load`
//
// The first version of this file replaced `document.fonts.load` directly
// (via `page.addInitScript`, with `Object.defineProperty` for good
// measure). It worked on Chromium and silently did NOT on WebKit: probed
// directly, the override was gone again within the first few hundred
// milliseconds of navigation — WebKit reconstructs (or otherwise discards
// expando state on) `document.fonts` early in its own load sequence in a
// way `addInitScript`'s "runs before any page script" guarantee does not
// cover, so terminal.js ended up calling the REAL implementation regardless
// of what this file had just installed. Rather than chase that engine
// quirk, this file blocks the actual font FILE REQUESTS instead
// (`page.route` against the two `.woff2` URLs `app.css`'s `@font-face` rules
// name) — `route.abort()` for the reject leg, a held (never resolved)
// route for the hang leg. That is both more portable (pure HTTP-layer
// interception, nothing engine-specific to trust) and more HONEST: what
// actually fails in production is the network fetch a real `@font-face`
// triggers, not the JS call that asks about it — this is the same class
// of failure a slow CDN or a lost connection produces.
//
// ## Deliberately NOT covered here
//
// The empty-result retry (`ensureFontSettling` re-issues `load()` when the
// stylesheet has not registered the face yet) has no deterministic
// registration-order test here: exercising it needs control over the order
// Dioxus injects `app.css` and `terminal.js`, which this harness does not
// have. Ordinary mounts exercise readiness through the same gate, but do
// not prove the empty-result branch was taken.
import { expect, test } from "./helpers/evidence";
import { type Page, type Route } from "@playwright/test";
import { attachSession, waitForTermText } from "./helpers/term";
import { cleanupSession, createSession, pinAutoSelect } from "./helpers/fleet";
import { waitForSessionReady } from "./helpers/terminal-readiness";

/** The limit button still owes its focus handoff. Reading unchanged size
 * alone would miss the bug: real typed input must reach the owned session. */
for (const [size, direction] of [[9, "smaller"], [28, "larger"]] as const) {
  test(`terminal text size ${size} keeps input focus at its limit`, async ({ page, request }) => {
    const marker = `font-limit-${size}-${Date.now()}`;
    await page.addInitScript((size) => localStorage.setItem("farhelm.terminal-font-size", String(size)), size);
    const session = await createSession(request, { title: marker, invocation: "sh -c 'printf \"FONTREADY\\n\"; read line; printf \"received:%s\\n\" \"$line\"; sleep 300'", cwd: "/tmp" });
    try {
      await page.goto("/");
      await attachSession(page, session.id);
      await waitForTermText(page, "FONTREADY");
      const terminal = page.locator("#terminal .xterm-helper-textarea");
      await expect(terminal).toBeFocused();
      expect(await page.evaluate(() => (window as any).__farhelmTerm.options.fontSize)).toBe(size);
      const button = page.locator(`[data-text-size="${direction}"]`);
      await expect(button).toBeEnabled();
      await button.click();
      await expect(terminal).toBeFocused();
      expect(await page.evaluate(() => (window as any).__farhelmTerm.options.fontSize)).toBe(size);
      await page.keyboard.type(marker);
      await page.keyboard.press("Enter");
      await waitForTermText(page, `received:${marker}`);
    } finally {
      await cleanupSession(request, session.id);
    }
  });
}

/**
 * Intercept both JetBrains Mono weight requests — `app.css`'s `@font-face`
 * `src: url(...)` targets, unhashed flat paths per lib.rs's own asset
 * registration — before navigation, so the very FIRST attempt to fetch
 * either face is already caught.
 *
 * `"reject"` aborts the request outright, which fails the underlying
 * `@font-face` load and rejects any `document.fonts.load()` call waiting
 * on it — a real, fast, definitive failure. `"hold"` installs the
 * interception and never resolves it (no `fulfill`/`continue`/`abort`),
 * leaving the request permanently in flight until the test releases it
 * via `route.continue()` on the captured routes — a real fetch that never
 * completes on its own. See this file's header for why this replaces an
 * earlier, more direct approach that monkey-patched `document.fonts.load`
 * instead.
 *
 * The returned observation records which faces were actually requested and
 * holds the pending routes (hold mode only), so the test can prove the
 * mount was asked for and is being held at the network layer rather than
 * never attempted.
 */
async function interceptFontRequests(page: Page, mode: "reject" | "hold"): Promise<{
  regularRequested: boolean;
  boldRequested: boolean;
  heldRoutes: Route[];
}> {
  const observation = { regularRequested: false, boldRequested: false, heldRoutes: [] as Route[] };
  await page.route("**/assets/JetBrainsMonoNerdFont-*.woff2", (route) => {
    const pathname = new URL(route.request().url()).pathname;
    if (pathname === "/assets/JetBrainsMonoNerdFont-Regular.woff2") {
      observation.regularRequested = true;
    } else if (pathname === "/assets/JetBrainsMonoNerdFont-Bold.woff2") {
      observation.boldRequested = true;
    }
    if (mode === "reject") {
      void route.abort("failed");
      return;
    }
    observation.heldRoutes.push(route);
    // "hold": deliberately no fulfill/continue/abort call — see this
    // function's own docs.
  });
  return observation;
}

/** Collect the single error terminal.js logs when the bundled font fails. */
function collectFarhelmFontErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error" && msg.text().includes("farhelm: terminal font")) {
      errors.push(msg.text());
    }
  });
  return errors;
}

test("a rejected font download leaves the terminal unmounted and logs exactly one Farhelm font error (F7a)", async ({
  page,
  request,
}) => {
  // Installed before navigation, so the font's very first fetch attempt is
  // already intercepted.
  const observed = await interceptFontRequests(page, "reject");
  const errors = collectFarhelmFontErrors(page);
  const marker = `font-reject-e2e-${Date.now()}`;
  const session = await createSession(request, {
    title: `font-reject-${Date.now()}`,
    cwd: "/tmp",
    invocation: `sh -c 'printf "${marker}\\n"; sleep 300'`,
  });
  try {
    await page.goto("/");
    const target = page.locator(`[data-session-id="${session.id}"]`);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await target.locator(".session-row-open").click();

    // The single console error is the failure oracle. An engine that
    // answers an empty face list instead of rejecting never logs it, so
    // this wait fails that engine rather than silently passing.
    await expect
      .poll(() => errors.length, {
        timeout: 10_000,
        message: "expected the single Farhelm font error to be logged",
      })
      .toBe(1);

    // sleep-ok: observe no mount after a definitive failure, past the old 3 s settle deadline; finite window.
    await page.waitForTimeout(4_000);
    expect(errors.length, "exactly one Farhelm font error, however many weights failed").toBe(1);

    expect(observed.regularRequested, "the regular face request must have been attempted").toBe(true);
    expect(observed.boldRequested, "the bold face request must have been attempted").toBe(true);

    expect(
      await page.evaluate(() => (window as any).__farhelmTermReady),
      "the terminal must not be ready when the font failed",
    ).toBeUndefined();
    expect(
      await page.evaluate(() => (window as any).__farhelmTerm),
      "no terminal instance may exist",
    ).toBeUndefined();
    await expect(page.locator("#terminal .xterm"), "no xterm element may exist").toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a held font request keeps the terminal unmounted past the old 3 s deadline, then mounts once released (F7b)", async ({
  page,
  request,
}) => {
  const observed = await interceptFontRequests(page, "hold");
  const marker = `font-hang-e2e-${Date.now()}`;
  const session = await createSession(request, {
    title: `font-hang-${Date.now()}`,
    cwd: "/tmp",
    invocation: `sh -c 'printf "${marker}\\n"; sleep 300'`,
  });
  try {
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    const target = page.locator(`[data-session-id="${session.id}"]`);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await target.locator(".session-row-open").click();

    // Both weights' fetches must reach the held routes before the
    // observation window starts, so the window measures a held download
    // rather than one never made. Chrome's own use of the face (app.css's
    // `--font-ui`) may have issued them before any mount; the terminal
    // appearing once bold is released below is what proves a mount was
    // waiting on them.
    await expect
      .poll(() => observed.heldRoutes.length, {
        timeout: 10_000,
        message: "both font weight requests must reach the held routes",
      })
      .toBeGreaterThanOrEqual(2);
    expect(observed.regularRequested).toBe(true);
    expect(observed.boldRequested).toBe(true);

    // Let regular finish while bold remains held. Holding both until the
    // final release would also pass a gate that required regular alone,
    // leaving the removed late-bold repaint without regression coverage.
    const regularRoute = observed.heldRoutes.find((route) =>
      new URL(route.request().url()).pathname.endsWith("-Regular.woff2")
    );
    const boldRoute = observed.heldRoutes.find((route) =>
      new URL(route.request().url()).pathname.endsWith("-Bold.woff2")
    );
    expect(regularRoute).toBeDefined();
    expect(boldRoute).toBeDefined();
    await regularRoute!.continue();
    await expect
      .poll(() => page.evaluate(() => document.fonts.check('14px "JetBrains Mono"')), {
        timeout: 10_000,
        message: "regular must be loaded while the bold request remains held",
      })
      .toBe(true);

    // sleep-ok: observe bold alone prevents mounting past the old 3 s settle deadline; finite window.
    await page.waitForTimeout(4_000);

    expect(
      await page.evaluate(() => (window as any).__farhelmTermReady),
      "the terminal must not mount while the font request is held",
    ).toBeUndefined();
    expect(
      await page.evaluate(() => (window as any).__farhelmTerm),
      "no terminal instance may exist while the font request is held",
    ).toBeUndefined();
    await expect(page.locator("#terminal .xterm"), "no xterm element may exist while the font request is held").toHaveCount(0);

    // Only bold is pending now: its completion must open the same gate.
    // The family assertion belongs here rather than in a literal-only test.
    await boldRoute!.continue();

    await waitForSessionReady(page, session.id);
    await waitForTermText(page, marker);
    expect(
      await page.evaluate(() => (window as any).__farhelmTerm.options.fontFamily),
      "the terminal must be constructed with JetBrains Mono",
    ).toMatch(/JetBrains Mono/);
  } finally {
    await cleanupSession(request, session.id);
  }
});
