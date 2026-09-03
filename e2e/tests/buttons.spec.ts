/**
 * The two-tier button system's own contract (app.css's "Shared button
 * look" section and SPEC_impl.md's GUI paragraph on it): `.btn` is ghost by
 * default across the whole sheet, and `.btn-primary` is the one filled
 * exception a given SURFACE gets — never more than one live at a time on
 * the sidebar's resting chrome, and never more than one live at a time
 * inside any single open dialog. Nothing else in the suite reads computed
 * color to check that promise (the rest of the browser suite treats CSS as
 * an implementation detail behind whatever text or attribute it asserts
 * on), which is exactly how a specificity bug or a stray literal color
 * could ship unnoticed behind a screenshot nobody diffed pixel-for-pixel.
 *
 * Three tests: the permitted-primaries/ghost-everything-else color sweep,
 * the row kebab's opacity reveal states, and the kebab's coarse-pointer
 * (touch) fallback. Each is kept to a fixed, deliberately small sample of
 * controls per class of claim rather than trying to enumerate every `.btn`
 * call site in the sheet — the point is to witness the SYSTEM working, not
 * to duplicate app.css itself here.
 */
import { expect, test, type Page } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  openRowMenu,
  pinAutoSelect,
} from "./helpers/fleet";
import { attachSession } from "./helpers/term";
import { addTab } from "./helpers/terminal-suite";

function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/**
 * Resolves a `:root` custom property to the RGB string a browser would
 * compute for any real element that used it, via a live probe element's
 * `getComputedStyle` — the same technique chrome.spec.ts's own
 * `resolveToken` uses, duplicated here rather than shared because the two
 * files otherwise have no coupling and a shared helper module would be the
 * only reason to import one from the other.
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

// What a browser reports for `background: none` and for `border: 1px solid
// transparent` alike — both normalize to the same fully-transparent black,
// which is what makes a single constant enough to assert "no visible fill"
// and "no visible border" with the same value.
const GHOST = "rgba(0, 0, 0, 0)";

test("the four permitted primaries carry the accent fill; every other sampled button stays ghost, and a destructive item is red text only", async ({
  page,
  request,
}) => {
  test.setTimeout(60_000);
  const session = await createSession(request, {
    title: `buttons-primary-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    const accentFill = await resolveToken(page, "--accent-fill");
    const accentEdge = await resolveToken(page, "--accent-edge");
    const accent = await resolveToken(page, "--accent");
    const pressedFill = await resolveToken(page, "--control-hover-bg");
    const danger = await resolveToken(page, "--danger");

    // `toHaveCSS` (polling) rather than a one-shot `getComputedStyle`
    // read: `.btn` and `.tab` transition `background-color` over 100ms,
    // so a sample taken right after the action that changed a button's
    // state lands mid-fade — a deselected agent tab read as
    // `rgba(22, 41, 74, 0.016)` on one loaded CI run, the tail of its
    // selected fill draining away. The web-first assertion waits for the
    // value to settle instead of racing the transition.
    async function expectPrimary(selector: string) {
      const el = page.locator(selector);
      await expect(el, `${selector} must be visible to check its resting style`).toBeVisible();
      await expect(el, `${selector} is a permitted primary and must carry the accent fill`)
        .toHaveCSS("background-color", accentFill);
      await expect(el, `${selector} is a permitted primary and must carry the accent-edge border`)
        .toHaveCSS("border-top-color", accentEdge);
    }

    async function expectGhost(selector: string) {
      const el = page.locator(selector);
      await expect(el, `${selector} must be visible to check its resting style`).toBeVisible();
      await expect(
        el,
        `${selector} is not one of the four permitted primaries and must have no fill at rest`,
      ).toHaveCSS("background-color", GHOST);
      await expect(
        el,
        `${selector} is not one of the four permitted primaries and must have no visible border at rest`,
      ).toHaveCSS("border-top-color", GHOST);
    }

    // --- Primary #1: the sidebar's standing "new session" control. Checked
    // before anything else touches the page, so this is genuinely resting
    // state rather than a post-interaction snapshot.
    await expectPrimary(".new-session-button");

    // --- Primary #2: the create-session dialog's own submit.
    await page.locator(".new-session-button").click();
    await expect(page.locator(".create-session-form")).toBeVisible();
    await expectPrimary(".create-session-submit");
    // Close it again rather than leaving it open into the next section —
    // an open create form is its own dialog surface and this test has no
    // further business with it.
    await page.locator(".new-session-button").click();
    await expect(page.locator(".create-session-form")).toHaveCount(0);

    // --- Primary #3: the "add a host" dialog's own submit.
    await page.getByRole("button", { name: "add host" }).click();
    await expect(page.locator(".add-host-form")).toBeVisible();
    await expectPrimary(".add-host-submit");
    // The list never unmounts. Close the add form through its own toggle so
    // the following row-menu samples start from the resting layout.
    await page.getByRole("button", { name: "add host" }).click();
    await expect(page.locator(".add-host-form")).toHaveCount(0);

    // A disclosure is ghost at rest and takes the shared pressed accent only
    // while the content named by `aria-expanded` is visible.
    const details = page.locator(".host-details-toggle");
    await expect(details).toHaveCSS("background-color", GHOST);
    await details.click();
    await expect(details).toHaveAttribute("aria-expanded", "true");
    await expect(details).toHaveCSS("background-color", pressedFill);
    await expect(details).toHaveCSS("color", accent);
    await details.click();
    await expect(details).toHaveAttribute("aria-expanded", "false");

    // --- Ghost sample #1 (row-menu item) and the destructive item, both
    // read straight out of the row's open actions menu without clicking
    // either — clicking "delete" would open ITS OWN confirmation, which is
    // a different control (`.confirm-delete`) than the one under test here.
    await openRowMenu(target);
    await expectGhost(".session-row-stop");
    const deleteBtn = target.locator(".session-row-delete");
    await expect(deleteBtn).toBeVisible();
    expect(
      await deleteBtn.evaluate((node) => getComputedStyle(node).color),
      "a destructive row action reads in red TEXT",
    ).toBe(danger);
    expect(
      await deleteBtn.evaluate((node) => getComputedStyle(node).backgroundColor),
      "a destructive row action must not also carry a red FILL — that would be a third tier",
    ).toBe(GHOST);

    // --- Primary #4: the rename dialog's own submit, plus the "cancel
    // buttons" ghost sample living right beside it in the same form.
    await target.locator(".session-row-rename").click();
    await expect(target.locator(".rename-form")).toBeVisible();
    await expectPrimary(".rename-submit");
    await expectGhost(".rename-cancel");
    await target.locator(".rename-cancel").click();
    await expect(target.locator(".rename-form")).toHaveCount(0);

    // --- Ghost sample #2 (a tab). The agent tab starts selected (and
    // `.tab.selected` is its own deliberate exception to the ghost
    // default — see app.css and terminal-tabs.spec.ts's own coverage of
    // it), so this opens a second tab first: adding one selects IT
    // instead (session_view.rs's `on_add_tab`), which leaves the agent
    // tab genuinely unselected and ghost for this check.
    await attachSession(page, session.id);
    const agentTab = page.locator('.tab-strip [data-terminal="agent"]');
    await addTab(page, 0);
    await expect(agentTab).not.toHaveClass(/selected/);
    await expectGhost('.tab-strip [data-terminal="agent"]');
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The row kebab's four independent reveal conditions (app.css's
 * `.session-row-menu` opacity rules): hover, keyboard focus anywhere in
 * the row, the row being the current selection, and the kebab's own menu
 * being open. Each is exercised, and dropped again where dropping it is
 * possible, so a rule that revealed the toggle PERMANENTLY (a `display`
 * flip instead of the intended `opacity` one, say) fails here too, not
 * only a rule that never revealed it at all.
 */
test("the row kebab is hidden at rest and revealed by hover, focus, selection, or its own open menu", async ({
  page,
  request,
}) => {
  test.setTimeout(60_000);
  // Two sessions, not one: a lone session on this page would auto-attach
  // (sidebar.spec.ts's own auto-select policy), which would make the row
  // under test SELECTED before this function ever touches it, and
  // "opacity: 0 at rest" needs a row that is genuinely not selected to
  // mean anything. `newer` exists only to be the one the page opens
  // instead — and it is PINNED as the remembered choice below rather
  // than left to the newest-session fallback: the suite's shared session
  // is "recently active" whenever another spec project is driving it at
  // the same time, and the fallback then opens that one (observed on a
  // loaded CI run), which is fine for the assertion here only if the
  // page's choice is made deterministic.
  const older = await createSession(request, {
    title: `kebab-older-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let newer: { id: string } | undefined;
  try {
    // created_at has one-second granularity (sidebar.spec.ts's own
    // auto-select test notes the same gap for the same reason); a real
    // gap is what makes "newest" deterministic rather than a coin flip on
    // which of two same-second rows the fallback prefers.
    await new Promise((resolve) => setTimeout(resolve, 1_100));
    newer = await createSession(request, {
      title: `kebab-newer-${Date.now()}`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });

    await pinAutoSelect(page, newer.id);
    await page.goto("/");
    const target = row(page, older.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".titlebar .title")).toContainText("kebab-newer-", {
      timeout: 20_000,
    });

    const kebab = target.locator(".session-row-menu");
    // `toHaveCSS`, not a one-shot `getComputedStyle` read, throughout this
    // test: app.css transitions this opacity over 100ms, so a value read
    // immediately after the action that changes it can land mid-fade
    // (`"0.5766…"`, observed) rather than at its settled `"0"`/`"1"`. The
    // web-first assertion polls until the value settles instead of racing
    // the transition.

    // AT REST: unselected (see above), unhovered, unfocused, menu closed.
    await expect(kebab, "hidden until there is a reason to show it").toHaveCSS("opacity", "0");

    // HOVER, then moved off again — the toggle must not get STUCK visible
    // once the reason for showing it goes away.
    await target.hover();
    await expect(kebab).toHaveCSS("opacity", "1");
    await page.mouse.move(0, 0);
    await expect(kebab, "hover alone must not latch the reveal").toHaveCSS("opacity", "0");

    // KEYBOARD FOCUS anywhere in the row — the toggle itself here, via
    // `:focus-within` on the row ancestor (app.css's own comment on this
    // selector explains why no separate `:focus` rule is needed).
    await kebab.focus();
    await expect(kebab).toHaveCSS("opacity", "1");
    await kebab.blur();
    await expect(kebab, "focus alone must not latch the reveal either").toHaveCSS("opacity", "0");

    // OPEN MENU, with the pointer since moved away from the row entirely —
    // checked BEFORE selecting the row below, deliberately: the row is
    // still unselected here, so this isolates the `[aria-expanded="true"]`
    // fallback (app.css's own comment on that selector explains why it
    // exists) from the SELECTED condition tested next, which would also
    // keep the toggle at opacity 1 and could mask this one going missing.
    await kebab.click();
    await expect(target.locator(".session-row-menu-panel")).toBeVisible();
    await page.mouse.move(0, 0);
    await expect(
      kebab,
      "the toggle must stay visible for as long as its own menu is open",
    ).toHaveCSS("opacity", "1");
    // Close it again and confirm the reveal was tied to the OPEN menu, not
    // some other latched state left over from the earlier steps. A click
    // leaves BOTH the pointer AND keyboard focus on the kebab — the mouse
    // move undoes the first, `.blur()` the second, since a still-focused
    // toggle would keep `:focus-within` satisfied and mask exactly the
    // regression this isolation is meant to catch.
    await kebab.click();
    await expect(target.locator(".session-row-menu-panel")).toHaveCount(0);
    await page.mouse.move(0, 0);
    await kebab.blur();
    await expect(kebab, "closing the menu drops the reveal again").toHaveCSS("opacity", "0");

    // SELECTED: the row becomes the open session, and stays revealed for
    // as long as it holds that selection.
    await target.locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText("kebab-older-", {
      timeout: 20_000,
    });
    await expect(kebab, "the current selection keeps its kebab reachable").toHaveCSS(
      "opacity",
      "1",
    );
  } finally {
    await cleanupSession(request, older.id);
    if (newer) await cleanupSession(request, newer.id);
  }
});

/**
 * The kebab's coarse-pointer fallback (`@media (hover: none), (any-pointer:
 * coarse)`), on a real touch-emulated context rather than
 * `page.emulateMedia()` — that API only overrides `prefers-color-scheme`
 * and a handful of other features, `any-pointer` not among them (verified
 * against the installed Playwright directly before writing this test).
 * `browser.newContext({ hasTouch: true })` does reach it: Chromium's touch
 * emulation flips `(hover: hover)` to false and BOTH `(any-pointer: coarse)`
 * and `(pointer: coarse)` to true together.
 *
 * That "together" is this test's real limitation, stated plainly rather
 * than left implicit: `hasTouch` reproduces a pure touch device, which the
 * OLD single-condition `(hover: none)` rule already covered on its own —
 * it cannot reproduce the HYBRID device the two-condition fix actually
 * targets (a precise pointer reporting `hover: hover` while a coarse one
 * is ALSO present), because Playwright exposes no way to set `hover` and
 * `any-pointer` independently. What this test still proves is that adding
 * the second condition did not regress the original touch case the first
 * condition already handled — real coverage, just not of the specific gap
 * the fix closed.
 */
test("the row kebab stays revealed at rest under Playwright's touch emulation", async ({
  browser,
  request,
}) => {
  test.setTimeout(60_000);
  const session = await createSession(request, {
    title: `kebab-touch-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  let context: Awaited<ReturnType<typeof browser.newContext>> | undefined;
  try {
    context = await browser.newContext({ hasTouch: true });
    const page = await context.newPage();
    await page.goto("/");
    const kebab = row(page, session.id).locator(".session-row-menu");
    await expect(kebab).toBeVisible({ timeout: 20_000 });
    await expect(
      kebab,
      "a coarse pointer has no :hover state to reveal the toggle by, so the fallback must",
    ).toHaveCSS("opacity", "1");
  } finally {
    if (context) await context.close();
    await cleanupSession(request, session.id);
  }
});
