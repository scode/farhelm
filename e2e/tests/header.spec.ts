/**
 * Session chrome has two rows around the terminal: tabs and lifecycle
 * actions above it, identity and copy fields below it. Geometry matters:
 * text size and both split actions must survive a 650px pane, identity must
 * not consume extra rows, and copy feedback must remain readable above the bottom edge.
 * The tests also preserve full values behind truncated peer fields.
 */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { clickDialogGap } from "./helpers/dialog-gap";
import {
  cleanupSession,
  createResumableSession,
  createSession,
  listSessions,
  openRowMenu,
  stopSession,
  stubFeed,
} from "./helpers/fleet";
import { openSessionActionMenu, selectSessionActionWith } from "./helpers/session-actions";
import { mountNativeTunnelPreview } from "./helpers/native-tunnel-preview";
import { waitForTermText } from "./helpers/term";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";
import { addTab, FAKE_AGENT_INVOCATION, selectTerminal } from "./helpers/terminal-suite";

/** Find only this test's owned session; other specs can share the listing. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

// The supported floor belongs to the main pane, not the whole window. Text
// size, both split pairs, Clone and Delete keep their width; only tabs scroll.
// Smaller panes may clip, so measuring one would test a different contract.
const SUPPORTED_MAIN_PANE_WIDTH = 650;
const VIEWPORT_HEIGHT = 600;
const PERSISTENT_ACTION_SELECTORS = [
  '[data-text-size="smaller"]',
  '[data-text-size="larger"]',
  ".restart-primary",
  ".restart-menu-toggle",
  ".header-replace",
  ".replace-menu-toggle",
  ".header-clone",
  ".header-delete",
] as const;

/** Both browser engines measure the native-only pill's CSS at its width floor.
 * Mirrored presentation markup is an explicit visual fixture, not native operation
 * evidence. It catches WebKit font/flex differences that Chromium appshots
 * cannot, including long hosts hiding a port or displacing a lifecycle action.
 */
test("native tunnel preview keeps ports and actions inside a 650px pane", async ({ page, request }) => {
  const session = await createSession(request, { title: `tunnel-geometry-${Date.now()}`, invocation: FAKE_AGENT_INVOCATION });
  try {
    await page.goto("/");
    await row(page, session.id).locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await page.locator(".app-shell").evaluate((element) => element.classList.add("macos-window"));
    const main = page.locator(".app-main");
    const occupied = (await main.boundingBox())!.x;
    await page.setViewportSize({ width: Math.round(occupied + 650), height: 700 });
    await page.evaluate(() => document.fonts.ready);
    const topbar = page.locator(".session-topbar");
    const frame = (await topbar.boundingBox())!;
    expect(frame.width, "the fixture measures the actual main pane").toBeCloseTo(650, 0);
    expect(frame.height).toBe(48);
    const terminal = (await page.locator(".terminal-panes").boundingBox())!;
    for (const colorScheme of ["dark", "light"] as const) {
      await page.emulateMedia({ colorScheme });
      await expect(page.locator("html")).toHaveAttribute("data-appearance", colorScheme);
      for (const state of ["one", "one-list", "many", "many-list", "closing", "error", "error-list"] as const) {
        await mountNativeTunnelPreview(page, state);
        expect((await topbar.boundingBox())!.height).toBe(frame.height);
        expect(await page.locator(".terminal-panes").boundingBox(), "conditional tunnel UI adds no row").toEqual(terminal);
        for (const selector of PERSISTENT_ACTION_SELECTORS) {
          const control = page.locator(selector);
          await expect(control).toBeVisible();
          const box = (await control.boundingBox())!;
          expect(box.x).toBeGreaterThanOrEqual(frame.x);
          expect(box.x + box.width).toBeLessThanOrEqual(frame.x + frame.width);
        }
        if (state !== "error") {
          const toggle = page.locator(".session-tunnel-toggle");
          const pill = (await toggle.boundingBox())!;
          const tabs = (await page.locator(".tab-strip").boundingBox())!;
          const selected = (await page.locator(".tab.selected").boundingBox())!;
          expect(tabs.width, "an ordinary shell tab needs a useful scrollport").toBeGreaterThanOrEqual(112);
          expect(selected.x).toBeGreaterThanOrEqual(tabs.x - 1);
          expect(selected.x + selected.width, "the selected tab is readable beside the pill").toBeLessThanOrEqual(tabs.x + tabs.width + 1);
          expect(pill.x).toBeGreaterThanOrEqual(tabs.x + tabs.width);
          expect(pill.x + pill.width).toBeLessThanOrEqual((await page.locator(".titlebar-actions").boundingBox())!.x);
          const ports = toggle.locator(".session-tunnel-mapping > span:not(.session-tunnel-host)");
          if (state === "one" || state === "one-list") {
            await expect(ports).toHaveText([":57779 → ", ":44227"]);
            for (const port of await ports.all()) {
              const box = (await port.boundingBox())!;
              expect(box.x).toBeGreaterThanOrEqual(pill.x);
              expect(box.x + box.width, "a long host must not hide either port").toBeLessThanOrEqual(pill.x + pill.width);
            }
            // An overfull host must still have room to paint an honest
            // abbreviation; a bare clipped letter can look like its full name.
            const abbreviation = await toggle.locator(".session-tunnel-host").evaluate((element) => {
              const style = getComputedStyle(element);
              const context = document.createElement("canvas").getContext("2d")!;
              context.font = style.font;
              return {
                width: element.getBoundingClientRect().width,
                minimum: context.measureText(`${element.textContent!.slice(0, 1)}…`).width,
                overflows: element.scrollWidth > element.clientWidth,
                textOverflow: style.textOverflow,
              };
            });
            expect(abbreviation.overflows, "the long-host fixture actually abbreviates").toBe(true);
            expect(abbreviation.textOverflow).toBe("ellipsis");
            expect(abbreviation.width, "the host can paint its first letter and ellipsis").toBeGreaterThanOrEqual(abbreviation.minimum);
          }
          if (state === "one-list") {
            // Token contrast alone cannot catch a shared hover/focus rule
            // replacing the open pill's fill while leaving inverse text.
            // Measure the settled CSS state: the normal 100ms transition
            // could otherwise pass on its readable starting color.
            await toggle.evaluate((element) => { (element as HTMLElement).style.transition = "none"; });
            for (const input of ["pointer", "keyboard"] as const) {
              if (input === "pointer") {
                await toggle.hover();
                expect(await toggle.evaluate((element) => element.matches(":hover"))).toBe(true);
              } else {
                await page.mouse.move(0, 0);
                await toggle.focus();
                await page.keyboard.press("Tab");
                await page.keyboard.press("Shift+Tab");
                await expect(toggle).toBeFocused();
                expect(await toggle.evaluate((element) => element.matches(":focus-visible"))).toBe(true);
              }
              const contrast = await toggle.evaluate((element) => {
                const style = getComputedStyle(element);
                const luminance = (color: string) => {
                  const channels = color.match(/[\d.]+/g)!.map(Number);
                  if (channels.length === 4 && channels[3] !== 1) throw new Error("pill contrast requires opaque colors");
                  const linear = channels.slice(0, 3).map((channel) => {
                    const value = channel / 255;
                    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
                  });
                  return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
                };
                const values = [luminance(style.color), luminance(style.backgroundColor)].sort((a, b) => b - a);
                return (values[0] + 0.05) / (values[1] + 0.05);
              });
              expect(contrast, `the expanded pill stays readable under ${input} input`).toBeGreaterThanOrEqual(4.5);
            }
            await page.keyboard.press("Escape");
          }
        }
        for (const overlay of await page.locator(".session-tunnel-panel, .session-tunnel-error").all()) {
          const box = (await overlay.boundingBox())!;
          expect(box.x).toBeGreaterThanOrEqual(0);
          expect(box.x + box.width).toBeLessThanOrEqual(Math.round(occupied + 650));
          expect(box.y + box.height).toBeLessThanOrEqual(700);
        }
        if (state === "error-list") {
          const error = page.locator(".session-tunnel-error");
          await expect(error).toHaveCount(1);
          await expect(error).toContainText("The tunnel could not be closed");
          const failure = (await error.boundingBox())!;
          const firstRow = (await page.locator(".session-tunnel-row").first().boundingBox())!;
          expect(firstRow.y, "the open list must not cover its own failure notice").toBeGreaterThanOrEqual(failure.y + failure.height);
        }
      }
    }
    // Real shell selection exercises the wider tab label/close slot. Keep
    // ordinary user scrolling: selection reveals its own tab, without a
    // fixture scrollLeft repair or a continuously pinned selected tab.
    const shell = await addTab(page, 0);
    for (const colorScheme of ["dark", "light"] as const) {
      await page.emulateMedia({ colorScheme });
      await page.setViewportSize({ width: Math.round(occupied + 1200), height: 700 });
      await mountNativeTunnelPreview(page, "one", "short");
      const strip = page.locator(".tab-strip");
      expect(await strip.evaluate((element) => element.scrollWidth <= element.clientWidth), "wide adjacency needs nonoverflowing tabs").toBe(true);
      const lastTab = (await page.locator(".tab-add").boundingBox())!;
      const pill = (await page.locator(".session-tunnel-toggle").boundingBox())!;
      expect(pill.x - lastTab.x - lastTab.width, "the pill follows actual tabs, not empty flex space").toBeCloseTo(6, 0);
      expect(await page.locator(".session-tunnel-toggle .session-tunnel-host").evaluate((element) => element.scrollWidth <= element.clientWidth),
        "a normal hostname fits the wider pill").toBe(true);
      for (const width of [650, 800, 820, 900]) {
        await page.setViewportSize({ width: Math.round(occupied + width), height: 700 });
        await selectTerminal(page, shell);
        await mountNativeTunnelPreview(page, "one");
        const tabs = (await strip.boundingBox())!;
        const slot = (await page.locator(`.tab-slot[data-tab-id="${shell}"]`).boundingBox())!;
        const label = page.locator(`.tab-slot[data-tab-id="${shell}"] .tab`);
        await expect(label).toHaveClass(/selected/);
        // Readability concerns the text, not a few pixels of button padding
        // that native nearest-edge focus scrolling may leave outside.
        const selected = await label.evaluate((element) => {
          const range = document.createRange();
          range.selectNodeContents(element);
          const box = range.getBoundingClientRect();
          return { x: box.x, width: box.width };
        });
        expect(selected.x).toBeGreaterThanOrEqual(tabs.x - 1);
        expect(selected.x + selected.width, "selection reveals the readable shell label").toBeLessThanOrEqual(tabs.x + tabs.width + 1);
        // Native focus reveals the selected button, not its sibling close
        // button. Both fit this track, but ordinary scrolling/focus may
        // reveal either; this change does not pin the whole tab slot.
        expect(slot.width, "the track can accommodate a shell label and its close control").toBeLessThanOrEqual(tabs.width);
        for (const control of await page.locator('.titlebar-actions button:visible, .session-tunnel-toggle .session-tunnel-mapping > span:not(.session-tunnel-host)').all()) {
          const box = (await control.boundingBox())!;
          expect(box.x).toBeGreaterThanOrEqual(occupied);
          expect(box.x + box.width, `all persistent controls and both ports fit a ${width}px pane`).toBeLessThanOrEqual(occupied + width);
        }
      }
    }
  } catch (error) {
    // Framework screenshots happen after finally. Retain the failed geometry
    // while this owned session still exists, before cleanup changes selection.
    await test.info().attach("tunnel-geometry-at-failure", { body: await page.screenshot(), contentType: "image/png" });
    throw error;
  } finally {
    await cleanupSession(request, session.id);
  }
});

test(
  "tabs and actions share one top row above the terminal and details stay below it",
  async ({ page, request }) => {
    const marker = `header-geometry-${Date.now()}`;
    // The title carries the hostile length; the invocation is the real
    // fake-agent command with a shell comment appended to pad it the same
    // way sidebar.spec.ts's oversized-fields test does — `#` starts a
    // comment, so the agent that actually launches is unaffected. `cwd`
    // stays a real directory: a nonexistent one is a precondition failure
    // SPEC.md has the create route refuse outright.
    // Resumable, because the test opens Restart's confirmation, and an
    // unavailable Restart opens nothing.
    const session = await createResumableSession(request, {
      title: `${marker}-${"t".repeat(200)}`,
      cwd: "/tmp",
      invocationSuffix: ` #${"x".repeat(200)}`,
    });
    try {
      await page.goto("/");
      await expect(page.locator(".app-sidebar")).toBeVisible();
      const VIEWPORT_WIDTH = Math.ceil((await page.locator(".app-sidebar").boundingBox())!.width) +
        SUPPORTED_MAIN_PANE_WIDTH;
      await page.setViewportSize({ width: VIEWPORT_WIDTH, height: VIEWPORT_HEIGHT });
      await row(page, session.id).locator(".session-row-open").click();
      await waitForSessionRevealed(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      // The split controls leave enough room for the agent tab alone. Real
      // shell tabs establish overflow before asserting that only tabs scroll.
      for (let previous = 0; previous < 3; previous += 1) {
        await addTab(page, previous);
      }
      await selectTerminal(page, "agent");
      await waitForSessionRevealed(page, session.id);
      // Restart opens its confirmation (measured below) only while the
      // agent is working, and a quiet fixture agent reads idle after a few
      // samples; `busy` keeps its screen changing so it stays working.
      await page.locator("#terminal").click();
      await page.keyboard.type("busy");
      await page.keyboard.press("Enter");
      await waitForTermText(page, "busy-tick-");

      const restartButton = page.locator(".restart-primary");
      // Restart confirms once the agent is classified working — the same
      // signal the restart-confirmation tests wait on — which is also the
      // point at which a badge is guaranteed to exist (a live status is
      // always classified, so `status_badge` never suppresses it the way
      // `Unknown` does).
      await expect(restartButton).toHaveAttribute("data-confirms", "true", {
        timeout: 15_000,
      });
      await page.evaluate(() => document.fonts.ready);

      // Identity must not grow a second row or take the old separate
      // header's height. No conditional band is active in this fixture.
      const headerBox = (await page.locator(".session-topbar").boundingBox())!;
      expect(headerBox.width, "premise: this is an actual 650px main pane").toBeCloseTo(SUPPORTED_MAIN_PANE_WIDTH, 0);
      const detailsBox = (await page.locator(".session-details").boundingBox())!;
      const terminalBox = (await page.locator(".terminal-panes").boundingBox())!;
      const tabsBox = (await page.locator(".session-topbar .tab-strip").boundingBox())!;
      const actionsBox = (await page.locator(".session-topbar .titlebar-actions").boundingBox())!;
      await expect(page.locator("header.titlebar")).toHaveCount(0);
      expect(headerBox.height, "browser top chrome stays about one tab row tall").toBeLessThanOrEqual(32);
      expect(detailsBox.height, "details stay about one tab row tall").toBeLessThanOrEqual(32);
      expect(tabsBox.x + tabsBox.width, "tabs yield width before the actions").toBeLessThanOrEqual(actionsBox.x + 1);
      expect(Math.abs(tabsBox.y + tabsBox.height / 2 - actionsBox.y - actionsBox.height / 2)).toBeLessThanOrEqual(1);
      expect(terminalBox.y).toBeCloseTo(headerBox.y + headerBox.height, 0);
      expect(detailsBox.y).toBeCloseTo(terminalBox.y + terminalBox.height, 0);
      expect(detailsBox.y + detailsBox.height).toBeCloseTo(VIEWPORT_HEIGHT, 0);
      expect(headerBox.height + detailsBox.height, "browser chrome gains about 10px over the old 40px header plus 30px tabs").toBeLessThanOrEqual(62);
      expect(await page.locator(".tab-strip").evaluate((node) => node.scrollWidth > node.clientWidth),
        "tabs scroll instead of pushing the actions out of a 650px pane").toBe(true);
      await expect(page.locator(".session-details-host .session-host")).toHaveText("local (this machine)");
      await expect(page.locator(".session-details-host .host-kind-icon")).toHaveCount(1);
      const identityOrder = await page.locator(".session-details").evaluate((node) =>
        Array.from(node.querySelectorAll(".status-badge, .title, .session-details-host, .status-time, .header-copy"))
          .map((field) => field.classList.contains("header-copy") ? "copy" :
            field.classList.contains("session-details-host") ? "host" : field.classList[0])
      );
      expect(identityOrder).toEqual(["status-badge", "title", "host", "status-time", "copy", "copy"]);

      // The eight persistent buttons stay intact while the tab strip shrinks.
      // With items are deliberately absent until their caret opens a menu.
      const badgeBox = (await page.locator(".session-details .status-badge").boundingBox())!;
      const restartBox = (await restartButton.boundingBox())!;
      await expect(page.locator(".titlebar-actions button:not([hidden])")).toHaveCount(8);
      await expect(page.locator(".restart-with-trigger, .header-replace-with")).toHaveCount(0);
      const boxes: [string, typeof badgeBox][] = [["badge", badgeBox]];
      for (const selector of PERSISTENT_ACTION_SELECTORS) {
        const control = page.locator(selector);
        await expect(control).toBeVisible();
        expect(await control.evaluate((node) => node.scrollWidth <= node.clientWidth), `${selector} does not clip its label`).toBe(true);
        expect(await control.evaluate((node) => node.closest(".tab-strip") === null), `${selector} must not scroll with tabs`).toBe(true);
        const box = (await control.boundingBox())!;
        expect(box.x, `${selector} remains inside the main pane`).toBeGreaterThanOrEqual(headerBox.x);
        expect(box.height, `${selector} retains today's compact button height`).toBeLessThanOrEqual(32);
        boxes.push([selector, box]);
      }
      for (const [name, box] of boxes) {
        expect(box.x, `the ${name} must not be pushed off the left edge`).toBeGreaterThanOrEqual(0);
        expect(
          box.x + box.width,
          `the ${name} must stay fully inside the ${VIEWPORT_WIDTH}px viewport`,
        ).toBeLessThanOrEqual(VIEWPORT_WIDTH + 1);
      }

      // The one-badge rule's non-stale half, asserted here because this
      // test already has a live, non-stale session on screen — cheaper
      // than a dedicated test, and `status_badge_destination`'s own unit
      // test already covers the logic; this is the render actually
      // obeying it.
      await expect(page.locator(".session-details .status-badge")).toHaveCount(1);

      const tabStripBoxBefore = (await page.locator(".tab-strip").boundingBox())!;

      // Confirmation stays under the top action and leaves the terminal
      // and details in place; moving identity must not move this anchor.
      await restartButton.click();
      const restartPanel = page.locator("#restart-confirm-panel");
      await expect(restartPanel).toBeVisible();
      await expect(restartPanel.locator(".restart-confirm")).toBeFocused();
      const restartConsent = await restartPanel.innerText();
      const restartPanelBox = (await restartPanel.boundingBox())!;
      expect(
        (await page.locator(".tab-strip").boundingBox())!,
        "an open restart confirmation must not move the tab strip",
      ).toEqual(tabStripBoxBefore);
      expect(await page.locator(".terminal-panes").boundingBox()).toEqual(terminalBox);
      expect(await page.locator(".session-details").boundingBox()).toEqual(detailsBox);
      expect(restartPanelBox.x).toBeGreaterThanOrEqual(0);
      expect(restartPanelBox.x + restartPanelBox.width).toBeLessThanOrEqual(VIEWPORT_WIDTH + 1);
      expect(restartPanelBox.y + restartPanelBox.height).toBeLessThanOrEqual(VIEWPORT_HEIGHT + 1);
      expect(
        restartPanelBox.y,
        "the restart confirmation must hang BENEATH the button that opened it",
      ).toBeGreaterThanOrEqual(restartBox.y + restartBox.height - 1);
      const restartCancel = restartPanel.getByRole("button", { name: "cancel", exact: true });
      await restartCancel.focus();
      await expect(restartCancel).toBeFocused();
      await page.keyboard.press("Escape");
      await expect(restartPanel).toHaveCount(0);

      // The repeated menu action must preserve the same working-agent
      // consent, including focus on the confirming answer. Cancelling both
      // paths avoids turning a geometry test into another restart lifecycle.
      await expect(restartButton).toHaveAttribute("data-confirms", "true");
      const restartMenu = await openSessionActionMenu(page, "Restart");
      await restartMenu.getByRole("menuitem", { name: "Restart", exact: true }).click();
      await expect(restartMenu).toHaveCount(0);
      await expect(restartPanel).toHaveText(restartConsent, { useInnerText: true });
      await expect(restartPanel.locator(".restart-confirm")).toBeFocused();
      await restartPanel.getByRole("button", { name: "cancel", exact: true }).click();
      await expect(restartPanel).toHaveCount(0);

      // A stopped session has a much longer badge than the live dot. It
      // must remain in the details bar, with its full status still
      // available in the tooltip; it cannot take room from the actions.
      await stopSession(request, session.id);
      const badge = page.locator(".session-details .status-badge");
      await expect(badge).toHaveClass(/\bexited\b/, { timeout: 20_000 });
      await expect(badge).toContainText("stopped by user");
      await expect(badge).toHaveAttribute("data-tooltip", /exited.*stopped by user/);
      for (const selector of PERSISTENT_ACTION_SELECTORS) {
        const control = page.locator(selector);
        await expect(control).toBeVisible();
        const box = (await control.boundingBox())!;
        expect(box.x + box.width, `${selector} remains reachable with an ended badge`).toBeLessThanOrEqual(VIEWPORT_WIDTH + 1);
      }
    } finally {
      await cleanupSession(request, session.id);
    }
  },
);

/** Split menus must work without a pointer at the supported width.
 * Each key starts from asserted focus so a terminal focus steal cannot pass
 * as menu navigation. Tab follows the shared menu's close-and-return rule;
 * the next Tab resumes the persistent toolbar order. */
test("split menus keep keyboard navigation and focus handback at a 650px pane", async ({ page, request }) => {
  const session = await createResumableSession(request, { title: `header-menu-keys-${Date.now()}` });
  try {
    await page.goto("/");
    await expect(page.locator(".app-sidebar")).toBeVisible();
    const viewportWidth = Math.ceil((await page.locator(".app-sidebar").boundingBox())!.width) + SUPPORTED_MAIN_PANE_WIDTH;
    await page.setViewportSize({ width: viewportWidth, height: VIEWPORT_HEIGHT });
    await row(page, session.id).locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await page.evaluate(() => document.fonts.ready);
    const topbar = page.locator(".session-topbar");
    const topbarBox = (await topbar.boundingBox())!;
    expect(topbarBox.width, "premise: the pane itself is 650px wide").toBeCloseTo(SUPPORTED_MAIN_PANE_WIDTH, 0);

    for (const [action, nextSelector] of [["Restart", ".header-replace"], ["Replace", ".header-clone"]] as const) {
      const toggle = page.getByRole("button", { name: `${action} options`, exact: true });
      const menu = page.getByRole("menu", { name: action, exact: true });
      const plain = menu.getByRole("menuitem", { name: action, exact: true });
      const withItem = menu.getByRole("menuitem", { name: `${action} with…`, exact: true });
      await expect(toggle).toHaveAttribute("aria-haspopup", "menu");
      await expect(toggle).toHaveAttribute("aria-expanded", "false");
      await toggle.focus();
      await expect(toggle).toBeFocused();
      await page.keyboard.press("ArrowDown");
      await expect(menu).toBeVisible();
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
      await expect(menu.getByRole("menuitem")).toHaveCount(2);
      await expect(plain).toBeFocused();
      const popupBox = (await page.locator(".header-action-menu").boundingBox())!;
      const toggleBox = (await toggle.boundingBox())!;
      expect(popupBox.x).toBeGreaterThanOrEqual(0);
      expect(popupBox.x + popupBox.width).toBeLessThanOrEqual(viewportWidth + 1);
      expect(popupBox.y).toBeGreaterThanOrEqual(toggleBox.y + toggleBox.height - 1);
      expect(popupBox.y + popupBox.height).toBeLessThanOrEqual(VIEWPORT_HEIGHT + 1);
      expect(await topbar.boundingBox(), "opening the menu must not resize the toolbar").toEqual(topbarBox);

      for (const [key, target] of [
        ["ArrowDown", withItem],
        ["ArrowDown", plain],
        ["ArrowUp", withItem],
        ["Home", plain],
        ["End", withItem],
      ] as const) {
        await page.keyboard.press(key);
        await expect(target, `${action} menu focus after ${key}`).toBeFocused();
      }
      await page.keyboard.press("Escape");
      await expect(menu).toHaveCount(0);
      await expect(toggle).toHaveAttribute("aria-expanded", "false");
      await expect(toggle).toBeFocused();

      await page.keyboard.press("ArrowUp");
      await expect(withItem).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(menu).toHaveCount(0);
      await expect(toggle).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(page.locator(nextSelector)).toBeFocused();

      // Enter and Space use native button activation. They must not also
      // toggle a second time through the menu's own key handler.
      for (const key of ["Enter", "Space"]) {
        await toggle.focus();
        await expect(toggle).toBeFocused();
        await page.keyboard.press(key);
        await expect(plain).toBeFocused();
        await page.keyboard.press("Escape");
        await expect(menu).toHaveCount(0);
        await expect(toggle).toBeFocused();
      }
    }

    // A With item unmounts when it opens the launcher. Cancel must return
    // to the persistent Restart caret after modal isolation releases.
    const restartToggle = page.locator(".restart-menu-toggle");
    await restartToggle.focus();
    await expect(restartToggle).toBeFocused();
    await page.keyboard.press("ArrowUp");
    const restartWith = page.getByRole("menuitem", { name: "Restart with…", exact: true });
    await expect(restartWith).toBeFocused();
    await expect(restartWith).toHaveAttribute("aria-disabled", "false");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menu", { name: "Restart", exact: true })).toHaveCount(0);
    await expect(page.locator(".restart-with-trigger")).toHaveCount(0);
    const launcher = page.locator(".create-session-form[data-launcher-mode=restart]");
    const cancel = launcher.locator(".launch-composer-cancel");
    await expect(cancel).toBeFocused();
    await cancel.click();
    await expect(launcher).toHaveCount(0);
    await expect(restartToggle).toBeFocused();

    // Switching families must replace the first popup, rather than leave
    // two overlapping menus with competing keyboard focus owners.
    await openSessionActionMenu(page, "Restart");
    const replaceMenu = await openSessionActionMenu(page, "Replace");
    await expect(page.getByRole("menu", { name: "Restart", exact: true })).toHaveCount(0);
    const replaceWith = replaceMenu.getByRole("menuitem", { name: "Replace with…", exact: true });
    await replaceWith.focus();
    await expect(replaceWith).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(replaceMenu).toHaveCount(0);
    await expect(page.locator(".replace-menu-toggle")).toBeFocused();
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("oversized title and copy fields overflow their boxes, with full tooltips", async ({
  page,
  request,
}) => {
  const marker = `header-overflow-${Date.now()}`;
  // Distinct oversized values for the title and the two copy buttons, so a
  // bug that swapped the two tooltips (`data-tooltip` attributes)
  // (or truncated one to the other's length) would show up as a mismatch
  // rather than passing by coincidence.
  const title = `${marker}-title-${"a".repeat(250)}`;
  // The readiness marker must come from the process this fixture actually launches.
  const invocation = `${FAKE_AGENT_INVOCATION} #${"b".repeat(250)}`;
  const session = await createSession(request, {
    title,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    const sessionRow = row(page, session.id);
    await expect(sessionRow, "the created session must be listed before opening it").toBeVisible();
    await sessionRow.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".session-details .title")).toHaveAttribute("data-tooltip", title);
    await expect(page.locator(".session-details .header-copy").nth(0)).toHaveAttribute("data-tooltip", /\/tmp/);
    await expect(page.locator(".session-details .header-copy").nth(1)).toHaveAttribute("data-tooltip", `${invocation} — click to copy`);
    // The tooltip is not in the accessibility tree, so the full value and
    // the copy action reach assistive technology through each button's
    // accessible description, as the `title` it replaced once provided.
    await expect(page.locator(".session-details .header-copy").nth(0)).toHaveAccessibleDescription("/tmp — click to copy");
    await expect(page.locator(".session-details .header-copy").nth(1)).toHaveAccessibleDescription(
      `${invocation} — click to copy`,
    );

    // `scrollWidth > clientWidth` is the DOM's own proof of a truncated
    // single-line box (`white-space: nowrap; overflow: hidden` on both
    // spans) — the only way the FULL string above is unreadable without
    // the tooltip this test also pins.
    const titleOverflows = await page.locator(".session-details .title").evaluate(
      (el) => el.scrollWidth > el.clientWidth,
    );
    const commandOverflows = await page.locator(".session-details .header-copy").nth(1).evaluate(
      (el) => el.scrollWidth > el.clientWidth,
    );
    expect(titleOverflows, "the title must actually be clipped, or the tooltip is untested").toBe(
      true,
    );
    expect(commandOverflows, "the command field must actually be clipped, or the tooltip is untested").toBe(
      true,
    );
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The directory and command fields must use the room the details bar has before
 * truncating. They used to be capped at a fixed 18 characters, so even an
 * ordinary command ellipsized in a wide window while a large empty gap sat
 * in the row. In a wide viewport, a normal-length title, directory,
 * and command must all render in full.
 */
test("copy fields use the details bar's free width before truncating", async ({ page, request }) => {
  await page.setViewportSize({ width: 1920, height: 800 });
  // A command of fixed, known length rather than the fake agent's absolute
  // binary path, whose length depends on where the checkout lives: 60
  // characters is far past the old 18-character cap yet leaves a 1920px
  // window ample room for the title, host, directory, and command. The
  // details render from the session record, so no agent output is needed.
  const invocation = `sleep 300 #${"x".repeat(49)}`;
  const session = await createSession(request, {
    title: `header-width-${Date.now()}`,
    cwd: "/tmp",
    invocation,
  });
  try {
    await page.goto("/");
    const sessionRow = row(page, session.id);
    await expect(sessionRow, "the created session must be listed before opening it").toBeVisible();
    await sessionRow.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await expect(page.locator(".session-details .header-copy").nth(1)).toHaveAttribute(
      "data-tooltip",
      `${invocation} — click to copy`,
    );
    const clipped = (selector: string, index = 0) =>
      page.locator(selector).nth(index).evaluate((el) => el.scrollWidth > el.clientWidth);
    expect(await clipped(".session-details .title"), "a short title must not be clipped").toBe(false);
    expect(await clipped(".session-details .header-copy", 0), "the directory must not be clipped").toBe(false);
    expect(await clipped(".session-details .header-copy", 1), "the command must not be clipped").toBe(false);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * The session header keeps persistent controls in keyboard order and offers
 * the editable variants through menus. This test pins that order, proves that
 * both copy buttons hand their complete values to the native bridge, and
 * verifies that header actions reuse the existing composer prefill paths.
 * The Replace prompt must also keep its focused answer when a pointer lands
 * on unused strip space, so the next key still reaches that answer.
 */
test("header actions stay ordered, copy full values, and open the right flows", async ({
  page,
  request,
}) => {
  const cwd = "/tmp";
  const invocation = `${FAKE_AGENT_INVOCATION} #'header copy'`;
  const session = await createSession(request, {
    title: `header-actions-${Date.now()}`,
    cwd,
    invocation,
  });
  try {
    await page.goto("/");
    const sessionRow = row(page, session.id);
    await expect(sessionRow, "the created session must be listed before opening it").toBeVisible();
    await sessionRow.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");

    const buttons = page.locator(".titlebar-actions button:not([hidden])");
    await expect(buttons).toHaveCount(PERSISTENT_ACTION_SELECTORS.length);
    for (const [index, selector] of PERSISTENT_ACTION_SELECTORS.entries()) {
      expect(await buttons.nth(index).evaluate((button, selector) => button.matches(selector), selector),
        `${selector} has its persistent toolbar position`).toBe(true);
    }
    await expect(page.locator(".restart-primary")).toHaveText("Restart");
    await expect(page.locator(".header-replace")).toHaveAccessibleName("Replace");
    await expect(page.locator(".header-clone")).toHaveAccessibleName("Clone");
    await expect(page.locator(".header-delete")).toHaveAccessibleName("Delete");

    await page.evaluate(() => {
      (window as any).__headerCopies = [];
      (window as any).__farhelmNativeClipboardWrite = (value: string) => {
        (window as any).__headerCopies.push(value);
      };
    });
    const copyButtons = page.locator(".session-details .header-copy");
    await copyButtons.nth(0).click();
    await expect
      .poll(() => page.evaluate(() => (window as any).__headerCopies), {
        message: "the directory copy must reach the native bridge untruncated",
      })
      .toEqual([cwd]);
    await expect(copyButtons.nth(0)).toContainText("copied");

    await copyButtons.nth(1).click();
    await expect
      .poll(() => page.evaluate(() => (window as any).__headerCopies), {
        message: "the command copy must reach the native bridge untruncated",
      })
      .toEqual([cwd, invocation]);
    await expect(copyButtons.nth(1)).toContainText("copied");

    const replace = page.locator(".header-replace");
    await replace.click();
    const confirmation = page.locator(".header-replace-confirm");
    await expect(confirmation).toBeVisible();
    const replaceConsent = await confirmation.innerText();
    await expect(confirmation.getByRole("button", { name: "replace", exact: true })).toHaveClass(
      /btn-danger/,
    );
    const confirmReplace = confirmation.getByRole("button", { name: "replace", exact: true });
    await expect(confirmReplace, "a destructive prompt must own focus before the gap click").toBeFocused();
    await clickDialogGap(page, confirmation);
    await expect(confirmReplace, "empty strip space must leave Replace as the keyboard target").toBeFocused();
    await confirmation.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(confirmation).toHaveCount(0);

    // Repeating Replace inside the split menu must not bypass the existing
    // danger question or change the answer that receives the next Enter.
    const replaceMenu = await openSessionActionMenu(page, "Replace");
    await replaceMenu.getByRole("menuitem", { name: "Replace", exact: true }).click();
    await expect(replaceMenu).toHaveCount(0);
    await expect(confirmation).toHaveText(replaceConsent, { useInnerText: true });
    await expect(confirmReplace).toBeFocused();
    await confirmation.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(confirmation).toHaveCount(0);

    const form = page.locator(".create-session-form");
    await page.locator(".header-clone").click();
    await expect(form).toBeVisible();
    await expect(form.locator('input[aria-label="folder"]')).toHaveValue(cwd);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);

    await selectSessionActionWith(page, "Replace");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-submit")).toHaveText(/^replace\s+local \(this machine\) · \/tmp$/);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * Why this matters: the details bar's command tooltip is the only full view of a
 * long command, and its copy button is how people take it to a shell. A host
 * or agent can put a direction override or a newline in the command, so the
 * shown text could read differently from the bytes copied. Spec: the bar
 * shows the command escaped (`<U+202E>`, `<U+000A>`) in a direction-isolated
 * element and in its tooltip, the clipboard receives the exact raw bytes,
 * and after copying the button shows a readable hidden-characters warning;
 * an ordinary value keeps the ordinary feedback.
 */
test("details copy shows escaped peer text, copies raw bytes, and warns upward", async ({
  page,
  request,
}) => {
  const cwd = "/tmp";
  // Everything after `#` is a shell comment on both lines, so the agent
  // still starts normally.
  const invocation = `${FAKE_AGENT_INVOCATION} # \u202Eabc\n# tail`;
  // A long title and a narrow window crowd the details so the copy buttons
  // shrink, which is the condition that used to clip an in-button warning.
  await page.setViewportSize({ width: 900, height: 700 });
  const session = await createSession(request, {
    title: `header-peer-${Date.now()}-${"x".repeat(120)}`,
    cwd,
    invocation,
  });
  try {
    await page.goto("/");
    const sessionRow = row(page, session.id);
    await expect(sessionRow).toBeVisible();
    await sessionRow.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");

    const command = page.locator(".session-details .header-copy").nth(1);
    const shown = command.locator(".peer-value");
    await expect(shown).toHaveAttribute("dir", "ltr");
    await expect(shown).toContainText("<U+202E>abc<U+000A># tail");
    expect(await shown.textContent(), "no raw override reaches the page").not.toContain("\u202E");
    await expect(command).toHaveAttribute("data-tooltip", /<U\+202E>abc<U\+000A># tail — click to copy$/);
    expect(
      await command.evaluate((el) => el.scrollWidth > el.clientWidth),
      "premise: the crowded details bar has shrunk the command button",
    ).toBe(true);

    await page.evaluate(() => {
      (window as any).__headerCopies = [];
      (window as any).__farhelmNativeClipboardWrite = (value: string) => {
        (window as any).__headerCopies.push(value);
      };
    });
    await command.click();
    await expect
      .poll(() => page.evaluate(() => (window as any).__headerCopies), {
        message: "the clipboard gets the exact raw command",
      })
      .toEqual([invocation]);
    const warning = page.locator(".session-details .copy-warning");
    await expect(warning).toHaveText(
      "⚠ the copied command contains hidden characters, displayed as <U+…>",
    );
    // The warning opens upward from the bottom bar. Checking both vertical
    // edges catches an otherwise visible warning clipped by the window.
    const box = await warning.boundingBox();
    const detailsBox = (await page.locator(".session-details").boundingBox())!;
    const viewport = page.viewportSize();
    expect(box && viewport && box.x >= 0 && box.x + box.width <= viewport.width).toBe(true);
    expect(box!.y).toBeGreaterThanOrEqual(0);
    expect(box!.y + box!.height).toBeLessThanOrEqual(detailsBox.y + 1);
    expect(await warning.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);

    // An ordinary value gets the ordinary feedback and no warning.
    await page.locator(".session-details .header-copy").nth(0).click();
    await expect(page.locator(".session-details .header-copy").nth(0).locator(".copy-feedback")).toHaveText(
      "✓ copied",
    );
    await expect(warning).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});

for (const field of ["folder", "command"] as const) {
  /** Feedback describes the clipboard snapshot even after metadata refreshes.
   * Pause the feedback timer only after the view is ready, then deliver a
   * real feed invalidation. The safe replacement must reach the tooltip while
   * the warning still describes the unsafe bytes copied before that refresh.
   */
  test(`copied ${field} warning survives a safe metadata refresh`, async ({ page, request }) => {
    const session = await createSession(request, { title: `copy-refresh-${field}-${Date.now()}` });
    try {
      const response = await request.get(`/api/sessions/${encodeURIComponent(session.id)}`);
      expect(response.ok(), await response.text()).toBe(true);
      const snapshot = await response.json();
      const build = response.headers()["x-farhelm-build"];
      expect(build, "fabricated metadata must carry the real helm's build stamp").toBeTruthy();
      const unsafeValue = field === "folder" ? "/tmp/\u202Ecopied-folder" : `${FAKE_AGENT_INVOCATION} # \u202Ecopied-command`;
      const safeValue = field === "folder" ? "/tmp/refreshed-folder" : `${FAKE_AGENT_INVOCATION} # refreshed-command`;
      let refreshed = false;
      const metadata = (url: URL) => url.pathname === "/api/sessions" || url.pathname === `/api/sessions/${encodeURIComponent(session.id)}`;
      await page.route(metadata, async (route) => {
        if (route.request().method() !== "GET") return route.fallback();
        // Only the displayed metadata changes. The owned process keeps its real
        // folder and command, so this fixture needs no hostile filesystem path.
        const value = refreshed ? safeValue : unsafeValue;
        const shown = {
          ...snapshot,
          ...(field === "folder" ? { cwd: value } : {
            invocation: value,
            launch: { ...snapshot.launch, command: value },
          }),
        };
        const body = new URL(route.request().url()).pathname === "/api/sessions"
          ? { sessions: [shown], total: 1, truncated: false }
          : shown;
        await route.fulfill({
          status: 200,
          headers: { "content-type": "application/json", "x-farhelm-build": build },
          body: JSON.stringify(body),
        });
      });
      const feed = await stubFeed(page);
      feed.notifyOnConnect(1);
      await page.goto("/");
      await feed.waitForConnection(1);
      await row(page, session.id).locator(".session-row-open").click();
      await waitForSessionRevealed(page, session.id);
      const copy = page.locator(".session-details .header-copy").nth(field === "folder" ? 0 : 1);
      await expect(copy.locator(".peer-value")).toHaveText(unsafeValue.replace("\u202E", "<U+202E>"));
      await page.evaluate(() => {
        (window as any).__headerCopies = [];
        (window as any).__farhelmNativeClipboardWrite = (value: string) => {
          (window as any).__headerCopies.push(value);
        };
      });
      // Installing the clock alone leaves it ticking. Pausing before the copy
      // keeps its 1.5-second feedback alive through arbitrary runner latency.
      await page.clock.install();
      await page.clock.pauseAt(new Date(Date.now() + 5_000));
      await copy.click();
      await expect.poll(() => page.evaluate(() => (window as any).__headerCopies)).toEqual([unsafeValue]);
      const warning = page.locator(".session-details .copy-warning");
      const warningText = `⚠ the copied ${field} contains hidden characters, displayed as <U+…>`;
      await expect(copy.locator(".copy-feedback")).toHaveText("✓ copied");
      await expect(warning).toHaveText(warningText);

      refreshed = true;
      expect(feed.openSockets(), "the invalidation must reach a live feed subscription").toBe(1);
      feed.notify(2);
      // The copied label temporarily replaces the value, but its tooltip and
      // accessible description still expose the refreshed metadata.
      await expect(copy, "the safe metadata must actually reach the rendered field").toHaveAttribute("data-tooltip", `${safeValue} — click to copy`);
      await expect(copy).toHaveAccessibleDescription(`${safeValue} — click to copy`);
      await expect(copy.locator(".copy-feedback")).toHaveText("✓ copied");
      await expect(warning).toHaveText(warningText);
      expect(await page.evaluate(() => (window as any).__headerCopies), "refreshing must not replace the copied bytes").toEqual([unsafeValue]);

      await page.clock.runFor(1_500);
      await expect(copy.locator(".copy-feedback")).toHaveCount(0);
      await expect(warning).toHaveCount(0);
      await expect(copy.locator(".peer-value")).toHaveText(safeValue);
    } finally {
      try {
        await page.goto("about:blank");
        await page.unrouteAll({ behavior: "wait" });
      } finally {
        await cleanupSession(request, session.id);
      }
    }
  });
}

/**
 * Deletes the open session from its header, and only after an answered
 * confirmation.
 *
 * Why this matters: the header's delete is a shortcut for the sidebar row's,
 * so it must be at least as careful and must end in the same place. It
 * confirms inline, anchored under the button, and cancelling sends nothing.
 * Confirming runs the list's own delete: the row leaves the sidebar, the main
 * pane stops showing the session, and the helm no longer lists it. The header
 * delete also confirms for an ended session the row would delete without
 * asking, and because that prompt said nothing was alive, the request carries
 * the supervisor-side precondition, the same guard the row's unconfirmed
 * delete sends.
 */
/** A reopened header question must hand Enter to its confirming answer.
 * Deletion still goes through the list's guarded request path and retires the
 * selected view; changing focus cannot weaken the live-versus-ended consent. */
test("the header delete confirms in place and deletes through the list", async ({ page, request }) => {
  const liveTitle = `header-delete-live-${Date.now()}`;
  const live = await createSession(request, {
    title: liveTitle,
    cwd: "/tmp",
    invocation: FAKE_AGENT_INVOCATION,
  });
  const ended = await createSession(request, {
    title: `header-delete-ended-${Date.now()}`,
    cwd: "/tmp",
    invocation: "sleep 300",
  });
  const deleteUrls: URL[] = [];
  await page.route("**/api/sessions/*", async (route) => {
    if (route.request().method() === "DELETE") {
      deleteUrls.push(new URL(route.request().url()));
    }
    await route.fallback();
  });
  try {
    await page.goto("/");
    await row(page, live.id).locator(".session-row-open").click();
    await waitForSessionRevealed(page, live.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    // Classified live (any live status), the point at which the header's
    // delete knows there is an agent to warn about.
    await expect(page.locator(".session-details .status-badge")).toHaveClass(/\b(running|waiting|idle)\b/, {
      timeout: 15_000,
    });

    const deleteButton = page.locator(".header-delete");
    await expect(deleteButton).toHaveClass(/btn-danger/);
    const confirmation = page.locator(".header-delete-confirm");

    // Cancelling is free: nothing is sent and the session stays.
    await deleteButton.click();
    await expect(confirmation).toBeVisible();
    await expect(confirmation.locator(".confirm-consequence").first()).toContainText("still running");
    const buttonBox = (await deleteButton.boundingBox())!;
    const panelBox = (await confirmation.boundingBox())!;
    expect(panelBox.y, "the confirmation hangs beneath the delete button").toBeGreaterThanOrEqual(
      buttonBox.y + buttonBox.height - 1,
    );
    await confirmation.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(confirmation).toHaveCount(0);
    expect(deleteUrls).toHaveLength(0);

    await deleteButton.click();
    await expect(confirmation.getByRole("button", { name: "delete", exact: true })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(row(page, live.id)).toHaveCount(0, { timeout: 20_000 });
    // The main pane lets go of the deleted session. Auto-select may open
    // another one straight away, so the check is on whose details show.
    await expect(page.locator(".session-details .title")).not.toContainText(liveTitle);
    expect((await listSessions(request)).sessions.some((listed) => listed.id === live.id)).toBe(false);
    expect(deleteUrls).toHaveLength(1);
    expect(deleteUrls[0].searchParams.get("only_if_nothing_alive")).toBeNull();

    // An ended session still confirms here, and its "nothing alive" prompt
    // sends the precondition.
    await stopSession(request, ended.id);
    await expect
      .poll(
        async () => (await listSessions(request)).sessions.find((listed) => listed.id === ended.id)?.status?.state,
        { timeout: 20_000 },
      )
      .toBe("exited");
    await expect(row(page, ended.id).locator(".status-badge")).toHaveText(/exited/, { timeout: 20_000 });
    await row(page, ended.id).locator(".session-row-open").click();
    await expect(page.locator(".session-topbar")).toBeVisible();
    await deleteButton.click();
    await expect(confirmation).toBeVisible();
    // The premise is the prompt the user reads, not the sidebar: the header
    // refreshes its own detail independently, and until it has the ended
    // status its prompt still warns of a live agent (and rightly sends no
    // precondition). "delete anyway:" is the ended, tab-less wording.
    await expect(confirmation.locator(".confirm-consequence").first()).toHaveText("delete anyway:", {
      timeout: 20_000,
    });
    await confirmation.getByRole("button", { name: "delete", exact: true }).click();
    await expect(row(page, ended.id)).toHaveCount(0, { timeout: 20_000 });
    expect(deleteUrls).toHaveLength(2);
    expect(deleteUrls[1].searchParams.get("only_if_nothing_alive")).toBe("true");
  } finally {
    await cleanupSession(request, live.id);
    await cleanupSession(request, ended.id);
  }
});

/**
 * A committed delete shows its progress on the row and in the header until
 * the supervisor's reply lands, then resolves either way.
 *
 * Why this matters: the supervisor answers a delete only once the session's
 * whole process tree is gone, which for a live agent takes a few seconds of
 * its own SIGTERM handling, and the row deliberately stays until then. Without
 * a visible state in between, the click looks ignored. Specifies, with the
 * DELETE reply held open by the test: the row gets its `deleting` state and a
 * "Stopping agent…" label in place of its title, and its open button, though
 * disabled like every row's during the delete, is not dimmed; the open
 * session's header shows the same label in place of its actions, and an
 * overlay over the terminal says it again. A refused delete (409) clears all three and leaves
 * the row with its error and the header with its actions; a delete that goes
 * through removes the row, and its own detach never paints the terminal's
 * "Detached" banner on the way.
 */
test("a committed delete shows its progress until the reply lands", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `delete-progress-${Date.now()}`,
    cwd: "/tmp",
    invocation: FAKE_AGENT_INVOCATION,
  });
  const build = (await request.get("/api/hosts")).headers()["x-farhelm-build"];
  expect(build, "the helm stamps every reply with its build").toBeTruthy();
  // Each DELETE waits for the test to release it, then either passes through
  // or is refused the way the helm relays a supervisor conflict.
  let release: ((outcome: "refuse" | "continue") => void) | undefined;
  const held = page.route(`**/api/sessions/${encodeURIComponent(session.id)}`, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.fallback();
      return;
    }
    const outcome = await new Promise<"refuse" | "continue">((resolve) => {
      release = resolve;
    });
    release = undefined;
    if (outcome === "refuse") {
      await route.fulfill({
        status: 409,
        headers: { "content-type": "text/plain", "x-farhelm-build": build },
        body: "held refusal",
      });
    } else {
      await route.fallback();
    }
  });
  await held;
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await target.locator(".session-row-open").click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    // Classified live (any live status), the point at which the header's
    // delete knows there is an agent to warn about.
    await expect(page.locator(".session-details .status-badge")).toHaveClass(/\b(running|waiting|idle)\b/, {
      timeout: 15_000,
    });
    const rowProgress = target.locator(".delete-progress");
    const headerProgress = page.locator(".header-delete-progress");
    const overlay = page.locator(".terminal-delete-overlay");

    // Refused: progress shows while the reply is held, then gives way to the
    // row's error and the header's actions.
    await openRowMenu(target);
    await target.locator(".session-row-delete").click();
    await target.locator(".confirm-delete").click();
    await expect.poll(() => release !== undefined, { message: "the DELETE reached the route" }).toBe(true);
    await expect(target).toHaveClass(/\bdeleting\b/);
    await expect(rowProgress).toHaveText("Stopping agent…");
    await expect(target.locator(".session-title"), "the progress takes the title line's place").toHaveClass(/\bvisually-hidden\b/);
    // The delete disables every row's open button, and a disabled one is
    // drawn at half opacity. The deleting row's is exempt: its progress
    // words are the row's only marker, and half opacity would leave them
    // too faint to notice (an earlier, fainter indicator went unnoticed).
    await expect(target.locator(".session-row-open"), "the open button is disabled during the delete").toBeDisabled();
    await expect(target.locator(".session-row-open"), "the deleting row is not dimmed").toHaveCSS("opacity", "1");
    await expect(headerProgress).toHaveText("Stopping agent…");
    await expect(overlay).toHaveText("Stopping agent…");
    await expect(page.locator(".restart-primary")).toBeHidden();
    release!("refuse");
    await expect(target.locator(".action-error")).toContainText("delete: held refusal");
    await expect(target).not.toHaveClass(/\bdeleting\b/);
    await expect(rowProgress).toHaveCount(0);
    await expect(headerProgress).toHaveCount(0);
    await expect(overlay).toHaveCount(0);
    await expect(target.locator(".session-title")).not.toHaveClass(/\bvisually-hidden\b/);
    await expect(page.locator(".restart-primary")).toBeVisible();

    // Accepted, from the header this time: the same progress, then the row goes.
    await page.locator(".header-delete").click();
    await page.locator(".header-delete-confirm").getByRole("button", { name: "delete", exact: true }).click();
    await expect.poll(() => release !== undefined, { message: "the DELETE reached the route" }).toBe(true);
    await expect(rowProgress).toHaveText("Stopping agent…");
    await expect(headerProgress).toHaveText("Stopping agent…");
    await expect(overlay).toHaveText("Stopping agent…");
    // From here the real delete tears the terminal down. Whether its detach
    // reaches this view before the view goes away with the session depends
    // on the helm's feed and the socket racing, which the browser cannot
    // order; what holds either way is that nothing paints the banner until
    // the row is gone. That the banner is held, and painted if the delete
    // fails, is `terminal.spec.ts`'s "a detach banner is held while a delete
    // is in flight" test, which drives the detach deterministically.
    const banner = page.locator("#term-banner");
    await expect(banner, "premise: the banner exists and is hidden before the delete").toBeHidden();
    expect(await banner.count(), "premise: the banner element exists to be watched").toBe(1);
    await page.evaluate(() => {
      const w = window as any;
      w.__deleteBannerPainted = false;
      const node = document.getElementById("term-banner")!;
      new MutationObserver(() => {
        if (node.style.display === "block") w.__deleteBannerPainted = true;
      }).observe(node, { attributes: true, attributeFilter: ["style"] });
    });
    release!("continue");
    await expect(target).toHaveCount(0, { timeout: 20_000 });
    expect(
      await page.evaluate(() => (window as any).__deleteBannerPainted),
      "the delete's own detach must never have painted the terminal's banner",
    ).toBe(false);
  } finally {
    release?.("continue");
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await cleanupSession(request, session.id);
  }
});
