// Browser-level coverage for terminal.js's pre-mount font settling ("## Font
// settling before mount" in its own header): the merge-gate failure class
// this design exists to close was a POST-mount font swap that resized the
// terminal and made tmux reflow the pane — pre-attach output landing in
// scrollback (spawn ready-marker timeouts), and a reproducible transient
// blank frame during the redraw (terminal.spec.ts's "renders the session"
// flake). This file proves the FIX's own contract holds, through the real
// Font Loading API neither a `node --test` unit nor a read of the source
// can exercise:
//
// - F1/F2 (fail-open, not fail-closed): a font whose fetch fails outright,
//   or one whose fetch never completes at all, must never prevent a
//   terminal from mounting — it must fall back to the default font instead,
//   and stay USABLE (real output still renders), never merely "did not
//   crash".
// - F8 (constructor-time selection): in the common, unstubbed case, the
//   font must already be correct at `new Terminal(...)` time — proven by
//   reading `options.fontFamily` the moment the terminal is ready, with no
//   settle-flavored wait of this test's own that could blur "the
//   constructor already had it" against "a fast post-mount swap corrected
//   it before this test happened to look".
//
// Its own file, per this suite's standing convention for new coverage
// (mouse-modes.spec.ts's header) — one subject, findable and runnable
// together. Unlike terminal-clipboard.spec.ts, this file needs no special
// browser permission and runs on BOTH engines.
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
// (`page.route` against the two `.ttf` URLs `app.css`'s `@font-face` rules
// name) — `route.abort()` for the reject leg, an intercepted route that is
// simply never resolved for the hang leg. That is both more portable (pure
// HTTP-layer interception, nothing engine-specific to trust) and more
// HONEST: what actually fails in production is the network fetch a real
// `@font-face` triggers, not the JS call that asks about it — this is the
// same class of failure a slow CDN or a lost connection produces, which is
// exactly the case `FONT_SETTLE_DEADLINE_MS` exists for.
//
// ## F9 — deliberately NOT covered here, and why
//
// The one path this file leaves untested is the "timeout-then-late-load"
// backstop: `fontSettled` flips true on the ~3s deadline (not on a real
// load), a terminal mounts in the fallback font, and SOME time after that
// the real load finally lands and `mount()`'s backstop swaps the family in
// and re-fits. Exercising that deterministically needs TWO kinds of control
// at once — a font-load promise this test can resolve on demand (to prove
// the swap happens, and happens correctly) AND control over
// `FONT_SETTLE_DEADLINE_MS`'s own clock (so the test is not stuck actually
// waiting out a real ~3 second deadline before it can even START on the
// "late load" half) — and this harness has neither piece today. A held
// `page.route` (the SAME mechanism `interceptFontRequests` below uses for
// the "hang" leg) can be fulfilled on demand whenever this test decides the
// "late load" should land, but terminal.js's deadline is a bare
// `setTimeout` against the REAL clock with no injected clock or fake-timer
// seam, and Playwright's
// own `page.clock` virtual-time control (were it wired in here) would have
// to fight the same real WebSocket/tmux round trips every OTHER test in
// this suite depends on staying real. Adding either piece is a harness
// change, not a test-file one, which is why this gap is recorded here
// rather than silently left for a later reader to rediscover missing
// coverage for.
import { expect, test, type Page } from "@playwright/test";
import { cleanupSession, createSession } from "./helpers/fleet";
import { attachSession, termText, waitForTermText } from "./helpers/term";

/**
 * Intercept both JetBrains Mono weight requests — `app.css`'s `@font-face`
 * `src: url(...)` targets, unhashed flat paths per lib.rs's own asset
 * registration — before navigation, so the very FIRST attempt to fetch
 * either face is already caught.
 *
 * `"reject"` aborts the request outright, which fails the underlying
 * `@font-face` load and rejects any `document.fonts.load()` call waiting
 * on it — a real, fast, definitive failure. `"hang"` installs the
 * interception and never resolves it (no `fulfill`/`continue`/`abort`),
 * leaving the request permanently in flight — a real fetch that never
 * completes, exactly the shape `FONT_SETTLE_DEADLINE_MS` exists to bound.
 * See this file's header for why this replaces an earlier, more direct
 * approach that monkey-patched `document.fonts.load` instead.
 */
async function interceptFontRequests(page: Page, mode: "reject" | "hang"): Promise<void> {
  await page.route("**/assets/JetBrainsMonoNerdFont-*.ttf", async (route) => {
    if (mode === "reject") {
      await route.abort("failed");
    }
    // "hang": deliberately no fulfill/continue/abort call — see this
    // function's own docs.
  });
}

test("a failed font fetch still mounts a usable terminal in the fallback font (F7a)", async ({
  page,
  request,
}) => {
  // Installed before navigation, so the font's very first fetch attempt is
  // already intercepted.
  await interceptFontRequests(page, "reject");
  const marker = `font-reject-e2e-${Date.now()}`;
  const session = await createSession(request, {
    title: `font-reject-${Date.now()}`,
    cwd: "/tmp",
    invocation: `sh -c 'printf "${marker}\\n"; sleep 300'`,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);

    // A rejection is a DEFINITIVE answer (terminal.js's `pollFontWeight`
    // settles on it immediately rather than waiting out the deadline — see
    // its own header), so this test should resolve quickly rather than
    // paying the ~3s deadline the never-resolves leg below deliberately
    // does pay.
    expect(
      await page.evaluate(() => (window as any).__farhelmTerm.options.fontFamily),
      "a rejected load must never apply the real font family",
    ).not.toMatch(/JetBrains/i);

    // USABLE, not merely "did not crash": real pty output must actually
    // render in the fallback font.
    await waitForTermText(page, marker);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a font fetch that never completes delays mount until the settle deadline, then mounts once (F7b)", async ({
  page,
  request,
}) => {
  await interceptFontRequests(page, "hang");
  const marker = `font-hang-e2e-${Date.now()}`;
  const session = await createSession(request, {
    title: `font-hang-${Date.now()}`,
    cwd: "/tmp",
    invocation: `sh -c 'printf "${marker}\\n"; sleep 300'`,
  });
  try {
    await page.goto("/");
    const target = page.locator(`[data-session-id="${session.id}"]`);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await target.locator(".session-row-open").click();

    // Comfortably under terminal.js's ~3s `FONT_SETTLE_DEADLINE_MS`: a
    // font load that never settles at all must not let the gate open
    // early — there is no signal here for it to open on besides the
    // deadline itself.
    await page.waitForTimeout(1_000);
    expect(
      await page.evaluate(() => (window as any).__farhelmTermReady === true),
      "a hung font load must not let the terminal mount before its settle deadline",
    ).toBe(false);

    // The real wait past the deadline — acceptable here, deliberately:
    // this test's whole subject IS that deadline. Once it fires,
    // `fontSettled` goes true on the TIMEOUT path (never a loaded font)
    // and the terminal mounts in the fallback.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, undefined, {
      timeout: 10_000,
    });
    await waitForTermText(page, marker);

    // "Mounts exactly once": the ready flag does not flap back to
    // something falsy on a later poll, which is the shape a spurious
    // extra mount/unmount cycle would leave behind.
    await page.waitForTimeout(500);
    expect(await page.evaluate(() => (window as any).__farhelmTermReady)).toBe(true);

    expect(
      await page.evaluate(() => (window as any).__farhelmTerm.options.fontFamily),
      "the fallback font, not JetBrains Mono — the load never actually resolved",
    ).not.toMatch(/JetBrains/i);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("the font settles before mount in the common case, selected directly by the constructor (F8)", async ({
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `font-happy-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);

    // Read the moment the terminal is ready — no `fonts.ready` wait of
    // this test's OWN, and no delay beyond what `attachSession` already
    // waits for (mount plus socket-open). That absence is the point: if
    // the family only became correct via the post-mount backstop rather
    // than the constructor, a read this immediate would frequently still
    // catch the fallback family, racing the backstop's own promise
    // resolution exactly the way the merge-gate flake did before this fix.
    expect(
      await page.evaluate(() => (window as any).__farhelmTerm.options.fontFamily),
    ).toMatch(/JetBrains Mono/);
  } finally {
    await cleanupSession(request, session.id);
  }
});
