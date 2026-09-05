/**
 * The sidebar/titlebar/menu-panel chrome's own layering contract — the
 * three-level surface ladder and the single keyboard-focus accent that
 * app.css's `:root` block declares as design constraints, not accidents
 * (see that file's header comment and the surface-levels paragraph inside
 * `:root`). Nothing else in the suite reads a computed style to check
 * either claim: the rest of the browser suite treats CSS as an
 * implementation detail behind whatever attribute or text it asserts on,
 * which is exactly why a specificity bug (a selected tab's hover fill
 * losing to `.btn:hover`) or an accidental token swap can ship unnoticed
 * behind a screenshot nobody diffed pixel-for-pixel.
 *
 * Two properties, two tests: that the three declared levels are actually
 * three distinct computed backgrounds with the right elements grouped
 * onto each one, and that the sheet's single `:focus-visible` accent
 * shows up for a keyboard user and stays away from a mouse one. Kept
 * small and deterministic on purpose — this file is not trying to be a
 * general CSS regression harness, just a witness for the two claims nothing
 * else would catch.
 */
import { expect, test, type Locator, type Page } from "@playwright/test";
import { cleanupSession, createSession, openRowMenu } from "./helpers/fleet";

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

    // RAISED CHROME: the sidebar and the main pane's titlebar are two
    // unrelated elements the palette comments claim sit at the SAME
    // level. If they only look alike by coincidence rather than by
    // both citing `--bg-1`, a future edit to either could drift them
    // apart with nothing here to notice.
    const sidebarBg = await backgroundOf(page.locator(".app-sidebar"));
    const titlebarBg = await backgroundOf(page.locator(".titlebar"));
    expect(sidebarBg, "sidebar and titlebar are both declared RAISED CHROME").toBe(titlebarBg);
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

/**
 * Resolves a `:root` custom property to the RGB string a browser would
 * compute for any real element that used it — via a live probe element's
 * `getComputedStyle`, the same normalization `outline-color` goes
 * through — rather than hardcoding the token's hex value in this file.
 * `--danger-strong` was already retuned once by this very fixup; a test
 * that baked in a hex would go stale on the next retune instead of
 * tracking it.
 */
async function resolveToken(page: Page, token: string): Promise<string> {
  return page.evaluate((name) => {
    const probe = document.createElement("div");
    probe.style.color = `var(${name})`;
    document.body.appendChild(probe);
    const value = getComputedStyle(probe).color;
    probe.remove();
    return value;
  }, token);
}

test("keyboard focus paints the accent outline; a mouse click does not", async ({
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
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    // The sole session auto-selects and auto-attaches on load, and the
    // terminal's mount takes focus for itself when it lands. Probing the
    // ring before that mount has happened is a race: `.focus()` below
    // lands on the button, the terminal then steals focus, and the style
    // read sees an unfocused button (`outline-style: none`) — exactly the
    // failure one loaded CI run produced. Waiting for the mount first
    // makes the focus moves below the LAST focus changes on the page.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, undefined, {
      timeout: 20_000,
    });

    const accentRgb = await resolveToken(page, "--accent");
    // A native checkbox with no contextual focus override of its own —
    // unlike `.session-row-open` below, which restates its own ring for
    // a documented reason (see that class's `:focus-visible` comment in
    // app.css) — so this half of the test exercises the sheet-wide rule
    // declared once near the top of the file.
    const btn = page.locator(".host-details-toggle");
    const openButton = target.locator(".session-row-open");

    // `.focus()` rather than a real `page.keyboard.press("Tab")` crawl:
    // the sole session on this page auto-selects and auto-attaches on
    // load (the same "recently active, falls back to newest" behavior
    // sidebar.spec.ts's own auto-select test pins), and a `sleep 300`
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
      async function expectFocusRing(el: Locator) {
        await expect(async () => {
          await el.focus();
          const ring = await el.evaluate((node) => {
            const style = getComputedStyle(node);
            return { outlineStyle: style.outlineStyle, outlineColor: style.outlineColor };
          });
          expect(ring).toEqual({ outlineStyle: "solid", outlineColor: accentRgb });
        }).toPass({ timeout: 10_000 });
      }
      await expectFocusRing(btn);
      await expectFocusRing(openButton);
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
