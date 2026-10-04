/**
 * Farhelm's own hover tooltip (assets/tooltip.js): the wiring that only a
 * browser can show.
 *
 * The tooltip replaced the browser's `title` tooltip because the native
 * delay (a second or more, owned by the engine) made hover help effectively
 * invisible; the maintainer had not noticed the hover texts that existed.
 * The placement geometry is pinned without a browser in
 * crates/farhelm-ui/js-tests/tooltip.test.js. What is left for this file is
 * whether real pointer, keyboard and touch input reach the script, whether
 * the element it shows actually lands where the geometry says on a real
 * page, and whether the page still carries native tooltips that would show
 * on top of it.
 *
 * Timing is observed in the page rather than with test-side clocks. The
 * delay test records the moment of the `pointerover` and the moment the
 * tooltip unhides with the page's own `performance.now()`, and only asserts
 * a lower bound, so a slow machine cannot fail it. The follow-on test checks
 * the tooltip synchronously inside the same `pointerover` dispatch, which is
 * what "at once" means and needs no clock at all.
 *
 * The page runs against a stubbed listing (one live session with a known
 * activity stamp), so the row's controls are present and identical on every
 * run without starting a real agent.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator, type Page } from "@playwright/test";
import { SESSION_LISTING } from "./helpers/fleet";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

const SESSION_ID = "synthetic-tooltip-row";

/** The tooltip element tooltip.js appends to `body`; one per page. */
function tooltip(page: Page): Locator {
  return page.locator("body > .farhelm-tooltip");
}

/**
 * Load the app with one stubbed live session and wait for its row.
 *
 * The listing is the only stubbed endpoint; everything else (hosts,
 * preferences, the feed) is the suite's real stack, which is why the reply
 * goes through `fulfillAsHelm` and carries the helm's build stamp.
 */
async function loadRow(page: Page): Promise<Locator> {
  // Fixed once per test: the page re-reads the listing on its own, and a
  // stamp recomputed per reply would change the age's hover text by a
  // second between two reads of it.
  const lastActivity = Math.floor(Date.now() / 1000) - 125;
  await page.route(SESSION_LISTING, (route) =>
    fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sessions: [{
          id: SESSION_ID,
          title: SESSION_ID,
          cwd: "/tmp",
          invocation: "sleep 300",
          status: { state: "running" },
          annotation: null,
          last_activity_at: lastActivity,
        }],
        total: 1,
        truncated: false,
      }),
    }));
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  await expect(row.locator(".status-time"), "the stubbed row must render its activity age").toHaveText("2m", {
    timeout: 20_000,
  });
  return row;
}

/**
 * Park the pointer over a corner of the page that carries no tooltip, so a
 * test starts from "no tooltip, nothing pending" rather than from wherever
 * an earlier action left the mouse.
 */
async function parkPointer(page: Page) {
  const spot = { x: 1, y: page.viewportSize()!.height - 1 };
  // Premise: nothing under the parking spot has hover text, or parking
  // itself would start a tooltip and turn the test's next hover into an
  // instant follow-on.
  const underSpot = await page.evaluate(({ x, y }) =>
    document.elementFromPoint(x, y)?.closest("[data-tooltip]")?.outerHTML.slice(0, 120) ?? null, spot);
  expect(underSpot, "the parking spot must carry no hover text").toBeNull();
  await page.mouse.move(spot.x, spot.y);
  await expect(tooltip(page)).toBeHidden();
}

/** The tooltip's and a target's rectangles, read in one round trip. */
async function boxes(page: Page, target: Locator) {
  const tip = (await tooltip(page).boundingBox())!;
  const anchor = (await target.boundingBox())!;
  return { tip, anchor, viewport: page.viewportSize()! };
}

/** The tooltip lies wholly inside the window. */
function expectInsideViewport(tip: { x: number; y: number; width: number; height: number }, viewport: {
  width: number;
  height: number;
}) {
  expect(tip.x, "the tooltip must not leave the window on the left").toBeGreaterThanOrEqual(0);
  expect(tip.y, "the tooltip must not leave the window at the top").toBeGreaterThanOrEqual(0);
  expect(tip.x + tip.width, "the tooltip must not leave the window on the right").toBeLessThanOrEqual(viewport.width);
  expect(tip.y + tip.height, "the tooltip must not leave the window at the bottom").toBeLessThanOrEqual(
    viewport.height,
  );
}

/**
 * Hover shows the target's text above it, centred, inside the window, and
 * only after the delay. Why: this is the whole feature (fast, themed, out
 * from under the cursor); a regression to the native tooltip, or to placing
 * the box below a small mark where the cursor covers it, would leave every
 * other test in the suite passing.
 */
test("hovering a row mark shows its text above it after the delay", async ({ page }) => {
  const row = await loadRow(page);
  await parkPointer(page);
  const age = row.locator(".status-time");
  const expected = await age.getAttribute("data-tooltip");
  expect(expected, "premise: the age carries hover text").toMatch(/^last activity /);

  // Record, in the page's own clock, when the pointer arrived and when the
  // tooltip first became visible.
  await page.evaluate(() => {
    const record = { over: -1, shown: -1 };
    (window as unknown as { __tooltipTiming: typeof record }).__tooltipTiming = record;
    document.addEventListener("pointerover", (event) => {
      if (record.over < 0 && (event.target as Element).closest(".status-time")) record.over = performance.now();
    }, true);
    const observer = new MutationObserver(() => {
      const tip = document.querySelector("body > .farhelm-tooltip");
      if (record.shown < 0 && tip && !(tip as HTMLElement).hidden) record.shown = performance.now();
    });
    observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["hidden"] });
  });

  await age.hover();
  await expect(tooltip(page)).toBeVisible();
  await expect(tooltip(page)).toHaveText(expected!);
  await expect(tooltip(page)).toHaveAttribute("role", "tooltip");

  const timing = await page.evaluate(() =>
    (window as unknown as { __tooltipTiming: { over: number; shown: number } }).__tooltipTiming
  );
  expect(timing.over, "premise: the pointer's arrival was observed").toBeGreaterThan(0);
  // A lower bound only: the 300 ms delay, less timer slack. Waiting longer
  // on a loaded machine is not a failure.
  expect(timing.shown - timing.over, "the tooltip must wait for the pointer to rest").toBeGreaterThanOrEqual(250);

  const { tip, anchor, viewport } = await boxes(page, age);
  expect(tip.y + tip.height, "the tooltip sits above the mark").toBeLessThanOrEqual(anchor.y);
  expect(anchor.y - (tip.y + tip.height), "about 6 px above").toBeLessThanOrEqual(8);
  expectInsideViewport(tip, viewport);

  // Leaving takes it away.
  await parkPointer(page);
});

/**
 * With no room above (the app bar's gear sits at the top of the window),
 * the tooltip goes below with the larger gap that clears the cursor's body.
 * Why: the fallback is the one placement where the cursor can cover the
 * text, and the larger gap is the only thing preventing that.
 */
test("a control at the top edge gets its tooltip below, clear of the cursor", async ({ page }) => {
  await loadRow(page);
  await parkPointer(page);
  const gear = page.locator(".app-settings-toggle");
  await gear.hover();
  await expect(tooltip(page)).toBeVisible();
  await expect(tooltip(page)).toHaveText(/^settings/);
  const { tip, anchor, viewport } = await boxes(page, gear);
  expect(anchor.y - 6 - tip.height, "premise: there is no room above the gear").toBeLessThan(4);
  expect(tip.y, "the tooltip sits below the gear").toBeGreaterThanOrEqual(anchor.y + anchor.height);
  expect(tip.y - (anchor.y + anchor.height), "with the 28 px gap").toBeGreaterThanOrEqual(26);
  expectInsideViewport(tip, viewport);
});

/**
 * Moving from one target to the next while a tooltip shows replaces it
 * within the same `pointerover` dispatch, with no new delay. Why: scanning a
 * row of marks one by one is the common case, and a fresh delay at each
 * would make that slower than it was with no tooltip at all.
 */
test("moving to the next mark while a tooltip shows replaces it at once", async ({ page }) => {
  const row = await loadRow(page);
  await parkPointer(page);
  const dot = row.locator(".status-dot");
  const age = row.locator(".status-time");

  await dot.hover();
  await expect(tooltip(page)).toHaveText(/^running/);

  // A capture listener added after tooltip.js's own runs after it for the
  // same event, so it sees the tooltip exactly as the dispatch left it.
  await page.evaluate(() => {
    const seen: { text: string | null; expected: string | null; hidden: boolean | null } = {
      text: null,
      expected: null,
      hidden: null,
    };
    (window as unknown as { __followOn: typeof seen }).__followOn = seen;
    document.addEventListener("pointerover", (event) => {
      const mark = (event.target as Element).closest(".status-time");
      if (seen.text === null && mark) {
        const tip = document.querySelector("body > .farhelm-tooltip") as HTMLElement;
        seen.text = tip.textContent;
        seen.expected = mark.getAttribute("data-tooltip");
        seen.hidden = tip.hidden;
      }
    }, true);
  });
  await age.hover();
  const seen = await page.evaluate(() =>
    (window as unknown as { __followOn: { text: string | null; expected: string | null; hidden: boolean | null } })
      .__followOn
  );
  expect(seen.expected, "premise: the age carries hover text").toMatch(/^last activity /);
  expect(seen.hidden, "the tooltip must stay up while moving between marks").toBe(false);
  expect(seen.text, "the next mark's text must show in the same dispatch").toBe(seen.expected);
});

/**
 * Put three tooltip targets of the test's own into the page: `outer`, with
 * `inner` nested inside it, and a separate `sibling` below. The script
 * serves any element with `data-tooltip`, and fixed geometry of the test's
 * choosing is what lets it point at "inside outer but not inside inner"
 * deterministically, which the app's own nested marks are too small to
 * guarantee. Appended to `body`, outside the tree Dioxus renders into, at
 * z-index 45: above the app's own panels and backdrops, so the pointer
 * reaches them, and below the tooltip's 50.
 */
async function injectTargets(page: Page) {
  await page.evaluate(() => {
    const make = (id: string, text: string, style: string) => {
      const element = document.createElement("div");
      element.id = id;
      element.dataset.tooltip = text;
      element.setAttribute("style", style);
      return element;
    };
    const outer = make("tt-outer", "outer", "position:fixed;z-index:45;left:200px;top:300px;width:320px;height:80px;");
    outer.appendChild(make("tt-inner", "inner", "position:absolute;left:200px;top:20px;width:60px;height:30px;"));
    document.body.appendChild(outer);
    document.body.appendChild(make("tt-sibling", "sibling", "position:fixed;z-index:45;left:200px;top:420px;width:120px;height:40px;"));
  });
}

/** Dispatch a mouse `pointerover` on an element, the event a dismissal exists to ignore. */
async function pointerOver(page: Page, selector: string) {
  await page.locator(selector).evaluate((element) => {
    element.dispatchEvent(new PointerEvent("pointerover", { bubbles: true, pointerType: "mouse" }));
  });
}

/**
 * Escape and a press each dismiss the element's tooltip, and keep it down
 * against a fresh `pointerover` on that element, while a target nested
 * inside it still gets its own tooltip, waiting the full delay, and loses it
 * again when the pointer returns to the dismissed element. Why: a tooltip
 * that reappeared over what a click just opened, or that Escape could not
 * put away, would cover the very thing the user acted on; and a nested
 * tooltip left on screen after the pointer moved off it is the stale popup
 * the dismissal logic once produced.
 */
test("a press or Escape dismisses the tooltip until the pointer leaves", async ({ page }) => {
  await loadRow(page);
  await parkPointer(page);
  await injectTargets(page);
  const outerOnly = { x: 210, y: 310 };

  await page.mouse.move(outerOnly.x, outerOnly.y);
  await expect(tooltip(page)).toHaveText("outer");
  await page.keyboard.press("Escape");
  await expect(tooltip(page)).toBeHidden();
  await pointerOver(page, "#tt-outer");
  await page.waitForTimeout(600); // sleep-ok: observation window longer than the 300 ms show delay
  await expect(tooltip(page), "a dismissed element stays dismissed under a fresh pointerover").toBeHidden();

  // The nested target shows its own tooltip, and not at once: a dismissal
  // does not open the instant follow-on window.
  await page.evaluate(() => {
    const seen = { hiddenAtArrival: null as boolean | null };
    (window as unknown as { __nested: typeof seen }).__nested = seen;
    document.addEventListener("pointerover", (event) => {
      if (seen.hiddenAtArrival === null && (event.target as Element).id === "tt-inner") {
        const tip = document.querySelector("body > .farhelm-tooltip") as HTMLElement | null;
        seen.hiddenAtArrival = tip === null || tip.hidden;
      }
    }, true);
  });
  const inner = (await page.locator("#tt-inner").boundingBox())!;
  await page.mouse.move(inner.x + inner.width / 2, inner.y + inner.height / 2);
  await expect(tooltip(page)).toHaveText("inner");
  const nested = await page.evaluate(() =>
    (window as unknown as { __nested: { hiddenAtArrival: boolean | null } }).__nested
  );
  expect(nested.hiddenAtArrival, "after a dismissal the next tooltip waits the full delay").toBe(true);

  // Back onto the dismissed element: the nested tooltip goes and nothing
  // replaces it.
  await page.mouse.move(outerOnly.x, outerOnly.y);
  await expect(tooltip(page), "the nested tooltip must not outlive the pointer leaving it").toBeHidden();
  await page.waitForTimeout(600); // sleep-ok: observation window longer than the 300 ms show delay
  await expect(tooltip(page)).toBeHidden();

  // Leaving clears the dismissal; a press dismisses the same way.
  const sibling = (await page.locator("#tt-sibling").boundingBox())!;
  await page.mouse.move(sibling.x + 10, sibling.y + 10);
  await expect(tooltip(page)).toHaveText("sibling");
  await page.mouse.move(outerOnly.x, outerOnly.y);
  await expect(tooltip(page)).toHaveText("outer");
  await page.mouse.down();
  await expect(tooltip(page)).toBeHidden();
  await page.mouse.up();
  await pointerOver(page, "#tt-outer");
  await page.waitForTimeout(600); // sleep-ok: observation window longer than the 300 ms show delay
  await expect(tooltip(page), "a pressed element stays dismissed under a fresh pointerover").toBeHidden();
});

/**
 * Keyboard focus shows the tooltip of the focused control, and moving focus
 * away hides it. Why: keyboard users get the same hover help (the
 * maintainer asked for it), and a tooltip left behind by a control that no
 * longer has focus would describe the wrong thing.
 */
test("keyboard focus shows the focused control's tooltip", async ({ page }) => {
  await loadRow(page);
  await parkPointer(page);
  const gear = page.locator(".app-settings-toggle");
  // Programmatic focus with no click before it matches `:focus-visible` in
  // both engines, as Tab focus does. This proves the script's focus wiring;
  // it does not drive the Tab key itself, whose focus order is the
  // browser's.
  await gear.focus();
  await expect(gear).toBeFocused();
  await expect(tooltip(page)).toBeVisible();
  await expect(tooltip(page)).toHaveText(/^settings/);
  await gear.blur();
  await expect(tooltip(page)).toBeHidden();
});

/**
 * Escape dismisses a focus-shown tooltip only while that control keeps
 * focus: tabbing away and back shows it again. Why: a dismissal that
 * outlived the focus would leave a keyboard user with a control whose hover
 * text never comes back until they happen to move the mouse over it.
 */
test("an Escape dismissal ends when focus leaves the control", async ({ page }) => {
  await loadRow(page);
  await parkPointer(page);
  const gear = page.locator(".app-settings-toggle");
  const help = page.locator(".app-help-toggle");
  await gear.focus();
  await expect(tooltip(page)).toHaveText(/^settings/);
  await page.keyboard.press("Escape");
  await expect(tooltip(page)).toBeHidden();
  await help.focus();
  await expect(help).toBeFocused();
  await expect(tooltip(page)).toHaveText(/^help/);
  await gear.focus();
  await expect(gear).toBeFocused();
  await expect(tooltip(page), "the dismissal must not outlive the focus").toHaveText(/^settings/);
});

/**
 * The delay counts from the pointer coming to rest: while it keeps moving
 * inside a target, nothing shows, and the tooltip appears about 300 ms after
 * the last movement. Why: the maintainer asked for the tooltip to appear when
 * the pointer rests on a control; one that popped up 300 ms into a sweep
 * across a wide row would flash over whatever the user is heading for.
 */
test("the delay restarts while the pointer keeps moving inside a target", async ({ page }) => {
  const row = await loadRow(page);
  await parkPointer(page);
  const title = row.locator(".session-title");
  const box = (await title.boundingBox())!;
  expect(box.width, "premise: the title is wide enough to move inside").toBeGreaterThan(40);

  // Record, in the page's clock, how long before the tooltip first showed
  // the pointer last moved over the title. Read at the moment of showing, so
  // a loaded machine that spaces the steps out further than planned can only
  // make the gap longer, never fail the assertion falsely.
  await page.evaluate(() => {
    const record = { lastMove: -1, gapAtShow: -1 };
    (window as unknown as { __restTiming: typeof record }).__restTiming = record;
    document.addEventListener("pointermove", (event) => {
      if ((event.target as Element).closest(".session-title")) record.lastMove = performance.now();
    }, true);
    const observer = new MutationObserver(() => {
      const tip = document.querySelector("body > .farhelm-tooltip");
      if (record.gapAtShow < 0 && tip && !(tip as HTMLElement).hidden) {
        record.gapAtShow = performance.now() - record.lastMove;
      }
    });
    observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["hidden"] });
  });

  // Six small steps 100 ms apart: 500 ms inside the title in all, longer
  // than the delay measured from entry.
  const y = box.y + box.height / 2;
  for (let step = 0; step < 6; step += 1) {
    await page.mouse.move(box.x + 4 + step * 6, y);
    await page.waitForTimeout(100); // sleep-ok: scheduling stimulus, pointer movement spread over time
  }
  await expect(tooltip(page)).toBeVisible();
  const timing = await page.evaluate(() =>
    (window as unknown as { __restTiming: { lastMove: number; gapAtShow: number } }).__restTiming
  );
  expect(timing.lastMove, "premise: movement over the title was observed").toBeGreaterThan(0);
  expect(timing.gapAtShow, "the tooltip must wait for the pointer to rest").toBeGreaterThanOrEqual(250);
});

/**
 * Touch input never shows a tooltip. Why: a tap has no hover, and a tooltip
 * left behind by one would sit over what the user is touching. Driven with
 * a synthetic touch `pointerover`, the event the script decides on; a real
 * tap also fires compatibility mouse events the script ignores.
 */
test("touch input never shows a tooltip", async ({ page }) => {
  const row = await loadRow(page);
  await parkPointer(page);
  await row.locator(".status-time").evaluate((element) => {
    element.dispatchEvent(new PointerEvent("pointerover", { bubbles: true, pointerType: "touch" }));
  });
  await page.waitForTimeout(600); // sleep-ok: observation window longer than the 300 ms show delay
  await expect(tooltip(page)).toBeHidden();
});

/**
 * No element keeps a native `title`. Why: a `title` brings the browser's
 * own slow tooltip back, on top of Farhelm's, which is the double popup this
 * whole mechanism exists to replace. Checked on the sidebar with a row and
 * its open actions menu, the states this page reaches without a real agent.
 */
test("the page carries no native title tooltips", async ({ page }) => {
  const row = await loadRow(page);
  const titled = () => page.evaluate(() => [...document.querySelectorAll("[title]")].map((node) => node.outerHTML.slice(0, 120)));
  expect(await titled()).toEqual([]);
  await row.hover();
  await row.locator(".session-row-menu").click();
  await expect(page.locator(".session-row-menu-panel")).toBeVisible();
  expect(await titled()).toEqual([]);
});
