/**
 * The sidebar/session chrome/menu-panel layering contract — the
 * three-level surface ladder and keyboard-focus roles that
 * app.css's `:root` block declares as design constraints, not accidents
 * (see that file's header comment and the surface-levels paragraph inside
 * `:root`). Computed styles catch a missing selector or specificity
 * regression even when the classes and accessible state remain correct.
 *
 * The three declared levels must produce distinct computed backgrounds with
 * the right elements grouped onto each one. Keyboard focus uses accent on
 * neutral surfaces and inverse on selection, staying away from mouse clicks. A selected
 * identity must remain readable when a confirmation disables navigation. Kept
 * small and deterministic on purpose — this file is not trying to be a
 * general CSS regression harness, just a witness for the two claims nothing
 * else would catch.
 */
import { expect, test } from "./helpers/evidence";
import { resolveToken } from "./helpers/color";
import { type Locator, type Page } from "@playwright/test";
import { cleanupSession, createSession, openRowMenu, patchPreferences, pinAutoSelect, readPreferences } from "./helpers/fleet";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";

function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/** The computed `background-color` a browser would actually paint for
 * `locator`, as a string comparable across elements (`"rgb(26, 29, 35)"`
 * and the like) — comparing THIS rather than the source hex is what
 * makes the surface-ladder test below immune to a token being read
 * through an inherited/cascaded value instead of a direct declaration. */
function backgroundOf(locator: Locator): Promise<string> {
  return locator.evaluate((el) => getComputedStyle(el).backgroundColor);
}

test("the three-level surface ladder renders distinct, correctly grouped levels", async ({
  page,
  request,
}) => {
  const session = await createSession(request, {
    title: `chrome-ladder-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    // GROUND: `body` rather than `.app-main`, deliberately. `.app-main`
    // never declares its own `background` (app.css relies on it showing
    // the page's `body` background through), so its OWN computed
    // background-color is transparent, not `--bg-0` — asserting on it
    // would pass by accident on a transparent element, not on the ground
    // token this test means to pin.
    const ground = await backgroundOf(page.locator("body"));

    // RAISED CHROME: the sidebar and both session rows are separate
    // elements the palette comments claim sit at the SAME
    // level. If they only look alike by coincidence rather than by
    // both citing `--bg-1`, a future edit to either could drift them
    // apart with nothing here to notice.
    const sidebarBg = await backgroundOf(page.locator(".app-sidebar"));
    const topbarBg = await backgroundOf(page.locator(".session-topbar"));
    const detailsBg = await backgroundOf(page.locator(".session-details"));
    expect(sidebarBg, "sidebar and top row are both declared RAISED CHROME").toBe(topbarBg);
    expect(sidebarBg, "identity stays on the same raised surface").toBe(detailsBg);
    expect(sidebarBg, "raised chrome must be visibly above the ground").not.toBe(ground);

    // FLOATING: the row actions menu panel, opened for real through the
    // same helper every other menu-panel test in the suite uses (see its
    // own doc in helpers/fleet.ts for why a bare `.click()` is not enough
    // to trust the panel has finished measuring itself).
    await openRowMenu(target);
    const floatingBg = await backgroundOf(page.locator(".session-row-menu-panel"));
    expect(floatingBg, "the floating level must differ from the ground").not.toBe(ground);
    expect(floatingBg, "the floating level must differ from raised chrome").not.toBe(sidebarBg);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** A confirmation must disable activation without dimming selected identity below AA.
 * The native button contains the title and status; opacity on that ancestor
 * blends every inverse value into the periwinkle, even when each computed
 * foreground is correct. Exercise the real confirmation rather than assigning
 * a disabled property or testing only the title's color.
 */
test("selected session identity stays legible while confirmation disables navigation", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const session = await createSession(request, {
    title: `chrome-disabled-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await patchPreferences(request, { skip_session_confirmations: false });
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    const target = row(page, session.id);
    await waitForSessionRevealed(page, session.id);
    await expect(target).toHaveAttribute("data-session-selected", "true");
    await expect(target).toHaveAttribute("data-session-stale", "false");
    await openRowMenu(target);
    await target.locator(".session-row-delete").click();
    await expect(target.locator(".confirm-consequence")).toBeVisible();
    await expect(target.locator(".session-row-open")).toBeDisabled();
    await expect(target.locator(".session-row-open")).toHaveCSS("opacity", "1");
    await expect(target.locator(".session-title")).toHaveCSS("color", await resolveToken(page, "--on-accent-dim"));
    await expect(target).toHaveCSS("background-color", await resolveToken(page, "--accent-fill-hover"));
    await target.locator(".confirm-cancel").click();
    await expect(target.locator(".session-row-open")).toBeEnabled();
    await expect(target.locator(".session-title")).toHaveCSS("color", await resolveToken(page, "--on-fill"));
  } finally {
    await cleanupSession(request, session.id);
    await patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null });
  }
});

/** Focus must remain visible on ordinary chrome and on solid selection.
 * The selected row needs an inverse ring; its accent-colored counterpart
 * remains the contract for controls on neutral surfaces.
 */
test("keyboard focus uses accent on ordinary controls and inverse on selection; a mouse click does not", async ({
  page,
  request,
  browserName,
}) => {
  const session = await createSession(request, {
    title: `chrome-focus-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    // The pinned session auto-selects and auto-attaches on load, and the
    // terminal's replay reveal takes focus for itself when it lands. Probing the
    // ring before that reveal has happened is a race: `.focus()` below
    // lands on the button, the terminal then steals focus, and the style
    // read sees an unfocused button (`outline-style: none`) — exactly the
    // failure one loaded CI run produced. Waiting for replay reveal first
    // places the focus moves after the initial focus handoff.
    await waitForSessionRevealed(page, session.id);

    const accentRgb = await resolveToken(page, "--accent");
    const selectedRing = await resolveToken(page, "--on-fill");
    await expect(target).toHaveAttribute("data-session-selected", "true");
    // A native checkbox with no contextual focus override of its own —
    // unlike `.session-row-open` below, which restates its own ring for
    // a documented reason (see that class's `:focus-visible` comment in
    // app.css) — so this half of the test exercises the sheet-wide rule
    // declared once near the top of the file.
    const btn = page.locator(".host-details-toggle");
    const openButton = target.locator(".session-row-open");

    // `.focus()` rather than a real `page.keyboard.press("Tab")` crawl:
    // the pinned session on this page auto-selects and auto-attaches on
    // load, and a `sleep 300`
    // invocation is not a real agent, so its terminal cycles through
    // reconnect attempts every second — real DOM churn that raced a
    // blind multi-press Tab crawl and made it land on the wrong element.
    // `:focus-visible`'s own browser heuristic is what actually decides
    // whether the ring paints, and it keys off HOW focus arrived, not
    // literally which key was pressed: a script-driven `.focus()` call
    // not immediately preceded by a pointer event on the same element is
    // treated the same as keyboard arrival (this is the standard trick
    // accessibility test suites use to probe `:focus-visible`
    // deterministically), while `.click()` below is a REAL pointer event
    // and exercises the other side of that same heuristic for real.
    //
    // WebKit is left out of this keyboard half, deliberately and narrowly.
    // Its `:focus-visible` heuristic paints the ring only for focus that
    // ARRIVES by keystroke, so the script-driven `.focus()` that Chromium
    // (and the accessibility-testing convention) accepts paints nothing
    // there; and its plain Tab skips buttons (Safari's Option+Tab
    // convention), so a keystroke-driven arrival cannot be made
    // deterministic against this page's churn either — the
    // step-off/step-on pair was tried and does not round-trip. The CSS
    // under test is one sheet-wide rule, so Chromium's pass covers its
    // correctness; the click half below still runs on both engines.
    if (browserName !== "webkit") {
      // Each ring check re-focuses, then reads style and color in ONE
      // `getComputedStyle` snapshot, and the whole block retries via
      // `toPass`. Two separate reads are not one observation: the
      // `sleep 300` terminal's once-a-second reconnect churn (the same
      // churn the `.focus()` comment above describes) can steal focus at
      // ANY instant, including between a passing style read and the color
      // read after it — at which point the element no longer matches
      // `:focus-visible` and `outline-color` computes to its
      // `currentcolor` fallback (`--fg-1`, observed on a loaded CI run).
      // A steal inside one attempt fails that attempt atomically, and the
      // retry's re-focus makes the next attempt independent of it.
      /** Observe focus and its paint together despite terminal reconnect churn. */
      async function expectFocusRing(el: Locator, color: string) {
        await expect(async () => {
          await el.focus();
          const ring = await el.evaluate((node) => {
            const style = getComputedStyle(node);
            return { outlineStyle: style.outlineStyle, outlineColor: style.outlineColor };
          });
          expect(ring).toEqual({ outlineStyle: "solid", outlineColor: color });
        }).toPass({ timeout: 10_000 });
      }
      await expectFocusRing(btn, accentRgb);
      await expectFocusRing(openButton, selectedRing);
    }

    // The mouse-click half, with a real click this time.
    await btn.click();
    expect(await btn.evaluate((el) => getComputedStyle(el).outlineStyle)).toBe("none");

    await openButton.click();
    expect(await openButton.evaluate((el) => getComputedStyle(el).outlineStyle)).toBe("none");
  } finally {
    await cleanupSession(request, session.id);
  }
});
