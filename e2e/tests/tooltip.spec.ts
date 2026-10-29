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
import { expectCompactRows } from "./helpers/list-header";
import { expect, test } from "./helpers/evidence";
import { resolveToken } from "./helpers/color";
import { type Locator, type Page } from "@playwright/test";
import { agentLaunchRow, localHostId, patchPreferences, resetPreferences, SESSION_LISTING } from "./helpers/fleet";
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
 * Legacy rows may have no activity timestamp, so callers can wait for the
 * row itself when an age would be an invented fixture premise.
 */
async function loadRow(page: Page, overrides: Record<string, unknown> = {}, expectAge = true): Promise<Locator> {
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
          ...overrides,
        }],
        total: 1,
        truncated: false,
      }),
    }));
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  if (expectAge) {
    await expect(row.locator(".status-time"), "the stubbed row must render its activity age").toHaveText("2m", {
      timeout: 20_000,
    });
  } else {
    await expect(row, "the legacy listing must render before checking omitted facts").toBeVisible({ timeout: 20_000 });
    await expect(row.locator(".status-time")).toHaveCount(0);
  }
  return row;
}

/**
 * Title, open space and keyboard focus must resolve to one card in either
 * density. The fixture uses a managed checkout so a missing body row or a
 * truncated directory cannot be hidden by an ordinary short path.
 */
test("a session's title and open area share its summary card", async ({ page, request }) => {
  const local = await localHostId(request);
  const started = 1_700_000_000;
  try {
    await patchPreferences(request, { compact: false });
    const row = await loadRow(page, {
      title: "A <peer> title",
      cwd: "/srv/checkouts/project-2/src",
      host: local,
      host_name: "this machine",
      created_at: started,
      launch: agentLaunchRow({ harness: "codex", permissions: "guarded", model: "gpt-6", effort: "high" }),
      working_copy: {
        id: "checkout", repo: { owner: "owner", name: "project" },
        canonical_path: "/srv/checkouts/project-2", origin_session_id: SESSION_ID,
      },
    });
    const source = page.locator(`#session-summary-${SESSION_ID}`);
    await expect(source).toBeHidden();
    await expect(row.locator(".session-row-open")).toHaveAttribute("data-tooltip-card", `session-summary-${SESSION_ID}`);
    await expect(row.locator(".session-title")).not.toHaveAttribute("data-tooltip", /./);
    await expect(source.locator(".session-summary-title")).toHaveText("A <peer> title");
    await expect(source.locator(".session-summary-row")).toHaveCount(5);

    let normalMarkup: string | undefined;
    for (const compact of [false, true]) {
      await patchPreferences(request, { compact });
      await page.reload();
      await expectCompactRows(page, compact);
      await parkPointer(page);
      await row.locator(".session-title").hover();
      const card = tooltip(page).locator(".session-summary");
      await expect(card).toBeVisible();
      await expect(card.locator(".session-summary-title")).toHaveText("A <peer> title");
      await expect(card.locator(".session-summary-state-word")).toHaveText("running");
      await expect(card.locator(".session-summary-agent")).toContainText("codex");
      await expect(card.locator(".session-summary-host")).toContainText("this machine");
      await expect(card.locator(".session-summary-value")).toContainText([
        /this machine/, /owner\/project · project-2/, "/srv/checkouts/project-2/src",
        "2023-11-14 22:13:20 UTC", /\d+m · .* UTC/,
      ]);
      await expect(card.locator(".session-summary-footer")).toHaveText("click to open⋯ for actions");
      const rowIcon = row.locator(".host-kind-icon");
      const cardIcon = card.locator(".host-kind-icon");
      await expect(cardIcon).toHaveAttribute("data-glyph", await rowIcon.getAttribute("data-glyph") ?? "");
      // The neutral card keeps the local host's identity color even when
      // selection inverts the sidebar's copy of the same glyph.
      await expect(rowIcon).toHaveAttribute("data-host-color", "red");
      await expect(cardIcon).toHaveAttribute("data-host-color", "red");
      await expect(cardIcon).toHaveCSS("color", await resolveToken(page, "--host-icon-red"));
      // The relative age can tick between density changes without changing
      // what the card offers. Compare the rest of the rendered card exactly.
      const stableMarkup = async () => card.evaluate((element) => {
        const copy = element.cloneNode(true) as HTMLElement;
        copy.querySelector(".session-summary-row:last-child .session-summary-value")?.replaceChildren("<age>");
        return copy.innerHTML;
      });
      const titleMarkup = await stableMarkup();
      if (normalMarkup) expect(titleMarkup, "compact and normal rows show the same card").toBe(normalMarkup);
      else normalMarkup = titleMarkup;

      const agentMark = row.locator(".session-agent");
      await agentMark.hover();
      await expect(tooltip(page)).not.toHaveAttribute("data-kind", "card");
      await expect(tooltip(page)).toHaveText(await agentMark.getAttribute("data-tooltip") ?? "");

      await parkPointer(page);
      const open = row.locator(".session-row-open");
      const box = (await open.boundingBox())!;
      await open.hover({ position: { x: box.width / 2, y: box.height - 2 } });
      await expect(card).toBeVisible();
      expect(await stableMarkup(), "the open area shows the title's card").toBe(titleMarkup);

      await parkPointer(page);
      await open.focus();
      await expect(open).toBeFocused();
      await expect(card).toBeVisible();
      expect(await stableMarkup(), "keyboard focus shows the same card").toBe(titleMarkup);
      await open.blur();
      await expect(tooltip(page)).toBeHidden();
    }
  } finally {
    await resetPreferences(request);
  }
});

/**
 * The last known status of an unreachable host may still say running, but
 * its card must not pulse or claim the live status color. Meaningful row
 * marks remain independent tooltip targets alongside that summary.
 */
test("a stale card is dim while row marks keep their own hover", async ({ page }) => {
  const row = await loadRow(page, { stale: true, status: { state: "running" } });
  await parkPointer(page);
  await row.locator(".session-title").hover();
  const card = tooltip(page).locator(".session-summary");
  await expect(card).toHaveClass(/stale/);
  await expect(card.locator(".session-summary-label")).toHaveText(["directory", "last active"]);
  await expect(card.locator(".status-dot")).toHaveCSS("animation-name", "none");
  const dimColor = await resolveToken(page, "--fg-2");
  await expect(card.locator(".session-summary-status")).toHaveCSS("color", dimColor);
  for (const mark of [row.locator(".status-dot"), row.locator(".status-time")]) {
    await mark.hover();
    await expect(tooltip(page)).not.toHaveAttribute("data-kind", "card");
    await expect(tooltip(page)).toHaveText(await mark.getAttribute("data-tooltip") ?? "");
  }
});

/**
 * Old listings can lack every optional fact. The card should expose the
 * directory it has without inventing a host identity or epoch timestamps.
 */
test("a legacy session card omits unavailable facts", async ({ page }) => {
  const row = await loadRow(page, { created_at: 0, last_activity_at: 0 }, false);
  const source = page.locator(`#session-summary-${SESSION_ID}`);
  await expect(row.locator(".host-kind-icon")).toHaveCount(0);
  await expect(source.locator(".session-summary-label")).toHaveText(["directory"]);
  await expect(source.locator(".session-summary-path")).toHaveText("/tmp");
  await expect(source.locator(".host-kind-icon")).toHaveCount(0);
});

/**
 * A host name from the session remains useful when locality is unknown, but
 * drawing a local or remote mark would claim a placement Farhelm has not made.
 */
test("an unplaced host keeps its name without a locality mark", async ({ page }) => {
  await loadRow(page, { host_name: "unplaced-host" });
  const source = page.locator(`#session-summary-${SESSION_ID}`);
  await expect(source.locator(".session-summary-label")).toHaveText(["host", "directory", "last active"]);
  await expect(source.locator(".session-summary-host")).toHaveText("unplaced-host");
  await expect(source.locator(".session-summary-host .host-kind-icon")).toHaveCount(0);
});

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
 * With no room above (the version readout sits at the top of the window),
 * the tooltip goes below with the larger gap that clears the cursor's body.
 * Why: the fallback is the one placement where the cursor can cover the
 * text, and the larger gap is the only thing preventing that.
 */
test("the version at the top edge gets its tooltip below, clear of the cursor", async ({ page }) => {
  await loadRow(page);
  await parkPointer(page);
  // Settings moved to the rail's foot. The real version readout still
  // exercises the top-edge fallback without manufacturing element geometry.
  const version = page.locator(".app-version");
  const words = await version.getAttribute("data-tooltip");
  expect(words, "the readout carries its actual version explanation").toBeTruthy();
  await version.hover();
  await expect(tooltip(page)).toBeVisible();
  await expect(tooltip(page)).toHaveText(words!);
  const { tip, anchor, viewport } = await boxes(page, version);
  expect(anchor.y - 6 - tip.height, "premise: there is no room above the version").toBeLessThan(4);
  expect(tip.y, "the tooltip sits below the version").toBeGreaterThanOrEqual(anchor.y + anchor.height);
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

/**
 * A card source stays outside the target, as a row's hidden summary will.
 * This proves the shared tooltip can follow Rust-rendered content without
 * interpreting peer text as markup or copying the source's identity.
 */
test("a card target clones its hidden source into the shared tooltip", async ({ page }) => {
  await loadRow(page);
  await parkPointer(page);
  await page.evaluate(() => {
    const source = document.createElement("div");
    source.id = "tooltip-card-source";
    source.hidden = true;
    const line = document.createElement("p");
    line.textContent = "A <peer> title";
    source.appendChild(line);
    document.body.appendChild(source);

    const target = document.createElement("button");
    target.id = "tooltip-card-target";
    target.dataset.tooltipCard = source.id;
    target.textContent = "Card target";
    target.setAttribute("style", "position:fixed;z-index:45;left:200px;top:300px;width:120px;height:40px;");
    document.body.appendChild(target);
  });

  await page.locator("#tooltip-card-target").hover();
  await expect(tooltip(page)).toBeVisible();
  await expect(tooltip(page)).toHaveAttribute("data-kind", "card");
  await expect(tooltip(page)).toHaveAttribute("aria-hidden", "true");
  await expect(tooltip(page).locator("p")).toHaveText("A <peer> title");
  expect(await page.locator("#tooltip-card-source").count(), "the source id is never cloned").toBe(1);
  expect(await tooltip(page).locator("peer").count(), "peer text must stay text").toBe(0);

  await page.locator("#tooltip-card-source p").evaluate((line) => line.textContent = "Updated card");
  await expect(tooltip(page).locator("p")).toHaveText("Updated card");
  await page.locator("#tooltip-card-source").evaluate((source) => source.remove());
  await expect(tooltip(page), "a removed source leaves no stale card").toBeHidden();
});

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
  await expect(tooltip(page)).toHaveText(/^Settings/);
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
  await expect(tooltip(page)).toHaveText(/^Settings/);
  await page.keyboard.press("Escape");
  await expect(tooltip(page)).toBeHidden();
  await help.focus();
  await expect(help).toBeFocused();
  await expect(tooltip(page)).toHaveText(/^help/);
  await gear.focus();
  await expect(gear).toBeFocused();
  await expect(tooltip(page), "the dismissal must not outlive the focus").toHaveText(/^Settings/);
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

/**
 * Re-executing the shipped asset must retain quiet focus and its dismissal
 * state without installing another listener set. AppBody can remount; a hook
 * lost during that lifecycle would bring back the tooltip on later dialogs.
 * The first hover arrives with no intervening pointer departure, distinguishing
 * quiet automatic focus from press/Escape's deliberately stronger suppression.
 */
test("quiet focus survives asset reinstall and allows the first later hover", async ({ page }) => {
  await loadRow(page);
  await parkPointer(page);
  const gear = page.locator(".app-settings-toggle");
  await page.keyboard.press("Tab");
  await gear.focus();
  await expect(tooltip(page)).toBeVisible();
  await expect(tooltip(page)).toHaveText(/^Settings/);
  const source = await page.locator('script[src*="tooltip"]').first().getAttribute("src");
  expect(source, "premise: the installed shipped asset is present").toBeTruthy();
  await page.addScriptTag({ url: source! });
  // A new focus event is essential: refocusing the gear while it already
  // owns focus would pass even if the hook suppressed help after focus().
  const help = page.locator(".app-help-toggle");
  await help.focus();
  await expect(help).toBeFocused();
  await expect(tooltip(page)).toHaveText(/^help/);
  await gear.evaluate((element) => {
    (window as unknown as { farhelmTooltip: { focusWithoutTooltip(element: Element): void } }).farhelmTooltip.focusWithoutTooltip(element);
  });
  await expect(gear).toBeFocused();
  expect(await gear.evaluate((element) => element.matches(":focus-visible"))).toBe(true);
  await page.waitForTimeout(600); // sleep-ok: quiet-focus absence window exceeds the tooltip's 300 ms show delay
  await expect(tooltip(page)).toBeHidden();
  await gear.hover();
  await expect(tooltip(page)).toBeVisible();
  await expect(tooltip(page)).toHaveText(/^Settings/);
});
