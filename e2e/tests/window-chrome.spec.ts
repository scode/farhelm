/**
 * The macOS window's title band: which presses along the top of the window
 * drag and zoom it, plus the web build's inertness and the band's geometry.
 *
 * Why this file exists: the desktop window draws its own title bar. Its whole
 * top band (the sidebar's app bar, and the main pane's top row of tabs and
 * session actions) is meant to behave like a classic title bar, with every
 * press on empty background there dragging the window and a double-click
 * zooming it, while controls and text keep their own interactions. Twice
 * that silently fell short: for a long time only one narrow spacer dragged
 * at all, and nobody noticed because the maintainer keeps windows
 * fullscreen. The band test below runs the page's real press classifier
 * (`assets/click-detail.js`) over the whole band, so a dead strip or a
 * control that would drag fails here instead of on someone's desk.
 *
 * The web build must take none of it: the bridge script is desktop-only
 * rendering, and the surfaces' handlers are inert outside the macOS shell.
 * The sibling macOS-geometry test in sidebar.spec.ts pins native-button
 * clearance while scrolling.
 *
 * What this file deliberately does NOT prove: native dragging, zooming, or
 * the click-count bridge's round trip to Rust. Those need AppKit; the Rust
 * press decision is unit-tested in window_chrome.rs, the bridge's reports in
 * crates/farhelm-ui/js-tests/click-detail.test.js, and the native behavior in
 * the release brick test.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession, SESSION_LISTING } from "./helpers/fleet";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";

/**
 * macOS aligns the merged top row with the native app bar, while the
 * identity bar keeps its compact height below the terminal. A copied old
 * titlebar rule would leave the new row short or inset the wrong surface.
 */
test("the macOS inset belongs to the merged top row and leaves details compact", async ({ page, request }) => {
  const session = await createSession(request, { title: `window-chrome-${Date.now()}` });
  try {
    await page.setViewportSize({ width: 1100, height: 700 });
    await page.goto("/");
    const target = page.locator(`[data-session-id="${session.id}"] .session-row-open`);
    await expect(target).toBeVisible();
    await target.click();
    await waitForSessionRevealed(page, session.id);
    const top = page.locator(".session-topbar");
    const details = page.locator(".session-details");
    await expect(details).toBeVisible();
    await page.evaluate(() => document.fonts.ready);
    const browserTop = (await top.boundingBox())!;
    const browserDetails = (await details.boundingBox())!;
    expect(browserTop.height).toBeLessThanOrEqual(32);
    await page.locator(".app-shell").evaluate((node) => node.classList.add("macos-window"));
    const macTop = (await top.boundingBox())!;
    const appBar = (await page.locator(".app-bar").boundingBox())!;
    expect(macTop.height).toBe(48);
    expect(macTop.y).toBeCloseTo(appBar.y, 0);
    expect(macTop.height).toBeCloseTo(appBar.height, 0);
    expect(await details.boundingBox()).toEqual(browserDetails);
    await expect(top.locator(".tab-strip")).toBeVisible();
    await expect(top.locator(".titlebar-actions")).toBeVisible();
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("the web shell hides the spacer and loads no bridge script", async ({ page }) => {
  const pageErrors: Error[] = [];
  page.on("pageerror", (err) => pageErrors.push(err));
  await page.setViewportSize({ width: 1100, height: 700 });
  await page.goto("/");
  const shell = page.locator(".app-shell");
  const bar = page.locator(".app-bar");
  const spacer = bar.locator(".window-drag-region");
  // Fixture premise: the production header rendered with its controls.
  await expect(shell).toBeVisible({ timeout: 20_000 });
  await expect(bar.locator(".app-version")).not.toHaveText("");
  // The wordmark leads the bar, named for assistive technology by the brand
  // file's own label, and sits left of the version.
  const wordmark = bar.getByRole("img", { name: "farhelm" });
  await expect(wordmark).toBeVisible();
  const wordmarkBox = (await wordmark.boundingBox())!;
  const versionBox = (await bar.locator(".app-version").boundingBox())!;
  expect(wordmarkBox.x + wordmarkBox.width).toBeLessThanOrEqual(versionBox.x);
  expect(wordmarkBox.height).toBeGreaterThan(10);

  // The spacer exists in production markup (so geometry tests can force
  // the shell class onto it) but is hidden without that class.
  await expect(spacer).toHaveCount(1);
  await expect(spacer).toBeHidden();
  await expect(spacer).toHaveAttribute("aria-hidden", "true");

  // The bridge script is desktop-only rendering; the web page must not
  // even fetch it, let alone run a capture listener from it.
  const bridgeScripts = await page.locator('script[src*="click-detail"]').count();
  expect(bridgeScripts, "the web build must not load the desktop bridge script").toBe(0);
  const interpreter = await page.evaluate(() => (window as never as { interpreter?: unknown }).interpreter);
  expect(interpreter, "no desktop interpreter global exists on the web build").toBeUndefined();

  expect(pageErrors, "no page error while asserting the hidden spacer").toEqual([]);
});

test("the forced-class spacer fills only the gap before the version", async ({
  page,
}) => {
  const pageErrors: Error[] = [];
  page.on("pageerror", (err) => pageErrors.push(err));
  await page.setViewportSize({ width: 1100, height: 700 });
  await page.goto("/");
  const shell = page.locator(".app-shell");
  const bar = page.locator(".app-bar");
  const version = bar.locator(".app-version");
  const spacer = bar.locator(".window-drag-region");
  await expect(version).not.toHaveText("");

  // Opt production markup into its macOS CSS, as sidebar.spec.ts's
  // native-button test does. Only the class is synthetic; every node
  // measured below is production.
  await shell.evaluate((element) => element.classList.add("macos-window"));
  await expect(spacer).toBeVisible();

  const versionBox = (await version.boundingBox())!;
  const spacerBox = (await spacer.boundingBox())!;
  expect(versionBox, "the version must have a box").not.toBeNull();
  expect(spacerBox, "the spacer must have a box").not.toBeNull();
  // The drag region is the gap and only the gap: strictly between the
  // two controls horizontally, sharing their row vertically.
  expect(spacerBox.x + spacerBox.width).toBeLessThanOrEqual(versionBox.x + 1);
  expect(spacerBox.y).toBeLessThanOrEqual(versionBox.y + versionBox.height);
  expect(spacerBox.y + spacerBox.height).toBeGreaterThanOrEqual(versionBox.y);
  // A zero-area spacer would satisfy the ordering above while owning
  // no pressable surface; the flex minimum is 12px (app.css).
  expect(spacerBox.width).toBeGreaterThan(0);
  expect(spacerBox.height).toBeGreaterThan(0);
  // The spacer owns no interactive descendants: a press on it can never
  // be a control activation, and no control press can borrow the spacer.
  await expect(spacer.locator("button, input, a, select, textarea, [tabindex]")).toHaveCount(0);

  expect(pageErrors, "no page error while measuring the spacer").toEqual([]);
});

test("header controls stay interactive and text stays selectable beside the forced-class spacer", async ({
  page,
}) => {
  const pageErrors: Error[] = [];
  page.on("pageerror", (err) => pageErrors.push(err));
  await page.setViewportSize({ width: 1100, height: 700 });
  await page.goto("/");
  const shell = page.locator(".app-shell");
  const bar = page.locator(".app-bar");
  const version = bar.locator(".app-version");
  const spacer = bar.locator(".window-drag-region");
  await expect(version).not.toHaveText("");
  await shell.evaluate((element) => element.classList.add("macos-window"));
  await expect(spacer).toBeVisible();

  // Real text selection in the header: double-clicking the version
  // selects a word of its own text, proving header text stays
  // selectable with the spacer present.
  const versionText = (await version.textContent()) ?? "";
  expect(versionText.trim(), "the version carries selectable text").not.toBe("");
  await version.dblclick();
  const selection = await page.evaluate(() => window.getSelection()?.toString() ?? "");
  expect(selection.length, "double-clicking the version selects text").toBeGreaterThan(0);
  expect(
    versionText,
    "the selection comes from the version's own text"
  ).toContain(selection.trim());

  // A press on the spacer itself dispatches an ordinary, unprevented
  // mousedown: the web handler is inert, and nothing else in the page
  // claims the press either. The target listener retains the ACTUAL
  // event object rather than copying its flags: Dioxus handles the
  // press later, in its delegated root-bubble listener, and only the
  // retained reference can show whether that later handler prevented
  // it. The real input pipeline supplies the press, so the observed
  // detail is the engine's, not a synthetic value.
  await page.evaluate(() => {
    const target = document.querySelector(".window-drag-region")!;
    (window as never as { __spacerEvent?: unknown }).__spacerEvent = null;
    target.addEventListener(
      "mousedown",
      (event) => ((window as never as { __spacerEvent?: unknown }).__spacerEvent = event),
      { once: true }
    );
  });
  await spacer.click();
  // The click resolved, so the full dispatch — including the delegated
  // root listener where the production handler runs — has completed and
  // had its opportunity to prevent the press. Read the FINAL state.
  const observed = await page.evaluate(() => {
    const event = (window as never as { __spacerEvent?: MouseEvent | null }).__spacerEvent;
    return event ? { detail: event.detail, defaultPrevented: event.defaultPrevented } : null;
  });
  expect(observed, "a real spacer press dispatches an ordinary mousedown").toEqual({
    detail: 1,
    defaultPrevented: false,
  });

  expect(pageErrors, "no page error from header interaction").toEqual([]);
});

/**
 * Load the desktop bridge script's real classifier into a web page.
 *
 * The web build never loads the script, so the test injects the repository's
 * own source, wrapped so its CommonJS export lands on `window.__fhWindowDrag`.
 * Without `window.interpreter` the script's press listener never POSTs, so
 * injecting it changes nothing else about the page.
 */
async function loadPressClassifier(page: Page) {
  const source = readFileSync(join(__dirname, "../../crates/farhelm-ui/assets/click-detail.js"), "utf8");
  await page.addScriptTag({
    content: `(function () { var module = { exports: {} };\n${source}\nwindow.__fhWindowDrag = module.exports; })();`,
  });
  const loaded = await page.evaluate(
    () => typeof (window as never as { __fhWindowDrag?: { classifyPress?: unknown } }).__fhWindowDrag?.classifyPress
  );
  expect(loaded, "the bridge script's classifier loaded").toBe("function");
}

/** One sampled point of the band and what the page decided about it. */
type BandPoint = { x: number; y: number; element: string; surface: boolean; drag: boolean; control: boolean };

/**
 * Classify a grid of points inside a viewport rectangle with the page's real
 * classifier, passing each point's coordinates as the press listener does.
 *
 * `control` is the test's own oracle for "this press belongs to a control":
 * a native control, link, tab, focusable element, or the sidebar resize
 * separator. It is deliberately narrower than the classifier's own list
 * (which also spares text), so a safety failure cannot be hidden by the two
 * sharing a typo.
 */
async function classifyArea(
  page: Page,
  area: { x: number; y: number; width: number; height: number },
  step = 4
): Promise<BandPoint[]> {
  return page.evaluate(
    ({ area, step }) => {
      const classify = (window as never as {
        __fhWindowDrag: {
          classifyPress(target: Element | null, x: number, y: number): { surface: Element | null; drag: boolean };
        };
      }).__fhWindowDrag.classifyPress;
      const describe = (element: Element) =>
        element.tagName.toLowerCase() +
        (typeof element.className === "string" && element.className
          ? "." + element.className.trim().replace(/\s+/g, ".")
          : "");
      const points = [];
      for (let y = Math.ceil(area.y) + 1; y < area.y + area.height - 0.5; y += step) {
        for (let x = Math.ceil(area.x) + 1; x < area.x + area.width - 0.5; x += step) {
          const element = document.elementFromPoint(x, y);
          if (!element) continue;
          const result = classify(element, x, y);
          const control =
            element.closest(
              "button, a[href], input, select, textarea, [role='tab'], [role='separator'], [tabindex]:not([tabindex='-1'])"
            ) !== null;
          points.push({ x, y, element: describe(element), surface: result.surface !== null, drag: result.drag, control });
        }
      }
      return points;
    },
    { area, step }
  );
}

/** Whether a press at one viewport point would drag the window. */
async function dragsAt(page: Page, point: { x: number; y: number }): Promise<boolean> {
  return page.evaluate(({ x, y }) => {
    const classify = (window as never as {
      __fhWindowDrag: { classifyPress(target: Element | null, x: number, y: number): { drag: boolean } };
    }).__fhWindowDrag.classifyPress;
    return classify(document.elementFromPoint(x, y), x, y).drag;
  }, point);
}

/**
 * Assert the band invariants over the top 48px of the viewport: every point
 * lies in a drag surface or in a control, no control point drags, and most
 * of the band drags, so an over-broad exclusion cannot leave the band
 * technically covered but practically dead.
 */
async function expectBandDrags(page: Page, label: string) {
  const width = page.viewportSize()!.width;
  const points = await classifyArea(page, { x: 0, y: 0, width, height: 48 });
  expect(points.length, `the band was sampled (${label})`).toBeGreaterThan(500);
  const uncovered = points.filter((point) => !point.surface && !point.control);
  expect(uncovered, `band points that neither drag surface nor control owns (${label})`).toEqual([]);
  const draggingControls = points.filter((point) => point.control && point.drag);
  expect(draggingControls, `controls that would drag the window (${label})`).toEqual([]);
  const dragging = points.filter((point) => point.drag).length;
  expect(dragging / points.length, `share of the band that drags (${label})`).toBeGreaterThan(0.5);
}

/** Open `sessionId` in a fresh page load at `width`, in the forced macOS shell. */
async function openInMacShell(page: Page, sessionId: string, width: number) {
  await page.setViewportSize({ width, height: 700 });
  await page.goto("/");
  const target = page.locator(`[data-session-id="${sessionId}"] .session-row-open`);
  await expect(target).toBeVisible();
  await target.click();
  await waitForSessionRevealed(page, sessionId);
  await page.evaluate(() => document.fonts.ready);
  await page.locator(".app-shell").evaluate((node) => node.classList.add("macos-window"));
  await loadPressClassifier(page);
}

/**
 * The whole macOS title band drags except where a press belongs to a
 * control or text, in the ordinary layout and in the narrow one, and nothing
 * outside the band does.
 *
 * Specification: SPEC.md "Desktop window chrome". In the ordinary layout the
 * band is the sidebar's app bar beside the session's top row; in a narrow
 * window (the sidebar plus the main pane's minimum no longer fit) the app
 * bar alone spans the window's top, fixed above both scrolling panes. In
 * both, named empty spots (the traffic-light padding, the wordmark, the gap
 * before the version, the band's top edge, and in the ordinary layout the
 * gaps of the session's top row) drag. A session's confirmation panel hangs
 * below its top row but is part of the same surface in the DOM, and a stale
 * session's tab strip is inert, which makes hit testing hand its presses to
 * the row behind it: neither may drag the window.
 */
test("the whole macOS title band drags except on controls and text", async ({ page, request }) => {
  const session = await createSession(request, { title: `window-band-${Date.now()}` });
  try {
    // --- The ordinary layout: app bar and session top row side by side.
    await openInMacShell(page, session.id, 1100);
    const appBar = page.locator(".app-bar");
    const topRow = page.locator(".session-topbar");
    // Fixture premise: not the narrow layout, and both rows render at the
    // macOS height, flush with the top of the viewport, controls present.
    await expect(page.locator("html")).not.toHaveClass(/sidebar-narrow-window/);
    const barBox = (await appBar.boundingBox())!;
    const rowBox = (await topRow.boundingBox())!;
    expect(barBox.y).toBeCloseTo(0, 0);
    expect(rowBox.y).toBeCloseTo(0, 0);
    expect(barBox.height).toBe(48);
    expect(rowBox.height).toBe(48);
    await expect(appBar.locator(".app-settings-toggle")).toBeVisible();
    await expect(topRow.locator(".tab-agent")).toBeVisible();
    await expectBandDrags(page, "ordinary layout");

    // Both palette variants stay mounted. Hit testing must use the painted
    // mark, and exactly one variant must own that place in the title band.
    const visibleWordmark = appBar.locator(".app-wordmark:visible");
    await expect(visibleWordmark).toHaveCount(1);
    await expect(visibleWordmark).toBeVisible();
    const wordmark = (await visibleWordmark.boundingBox())!;
    const version = (await appBar.locator(".app-version").boundingBox())!;
    const agentTab = (await topRow.locator(".tab-agent").boundingBox())!;
    const actions = (await topRow.locator(".titlebar-actions").boundingBox())!;
    const spots: Record<string, { x: number; y: number }> = {
      "traffic-light padding": { x: barBox.x + 30, y: barBox.y + 24 },
      "the wordmark": { x: wordmark.x + wordmark.width / 2, y: wordmark.y + wordmark.height / 2 },
      "the gap before the version": { x: version.x - 6, y: version.y + version.height / 2 },
      "the app bar's top edge": { x: version.x + version.width / 2, y: 2 },
      "the top row's top edge": { x: agentTab.x + agentTab.width / 2, y: 2 },
      "beside the agent tab": { x: agentTab.x - 4, y: agentTab.y + agentTab.height / 2 },
      "before the session actions": { x: actions.x - 6, y: actions.y + actions.height / 2 },
    };
    for (const [name, spot] of Object.entries(spots)) {
      expect(await dragsAt(page, spot), `${name} drags the window`).toBe(true);
    }

    // A confirmation panel opened from the top row hangs below it: nothing
    // in it may drag, padding and gaps included.
    await topRow.locator(".header-delete").click();
    const confirm = topRow.locator(".header-delete-confirm");
    await expect(confirm).toBeVisible();
    const confirmBox = (await confirm.boundingBox())!;
    const inConfirm = await classifyArea(page, confirmBox, 3);
    expect(inConfirm.length, "the confirmation panel was sampled").toBeGreaterThan(50);
    expect(
      inConfirm.filter((point) => point.drag),
      `points of the confirmation panel that would drag the window (top row ${JSON.stringify(
        await topRow.boundingBox()
      )}, panel ${JSON.stringify(confirmBox)})`
    ).toEqual([]);
    await page.keyboard.press("Escape");
    await expect(confirm).toBeHidden();

    // A stale session's tab strip is inert. The premise is that hit testing
    // really does hand a press over the agent tab to the row behind it;
    // the assertion is that such a press still does not drag.
    const strip = topRow.locator(".tab-strip");
    await strip.evaluate((node) => node.setAttribute("inert", ""));
    const overTab = { x: agentTab.x + agentTab.width / 2, y: agentTab.y + agentTab.height / 2 };
    const hit = await page.evaluate(({ x, y }) => document.elementFromPoint(x, y)?.closest(".tab-agent") ?? null, overTab);
    expect(hit, "hit testing skips the inert tab").toBeNull();
    expect(await dragsAt(page, overTab), "a press over an inert tab drags the window").toBe(false);
    await strip.evaluate((node) => node.removeAttribute("inert"));

    // --- The narrow layout: the app bar alone spans the window's top.
    await openInMacShell(page, session.id, 640);
    await expect(page.locator("html")).toHaveClass(/sidebar-narrow-window/);
    const narrowBar = (await appBar.boundingBox())!;
    expect(narrowBar.x).toBeCloseTo(0, 0);
    expect(narrowBar.y).toBeCloseTo(0, 0);
    expect(narrowBar.width).toBeCloseTo(640, 0);
    expect(narrowBar.height).toBe(48);
    await expectBandDrags(page, "narrow layout");
    const narrowVersion = (await appBar.locator(".app-version").boundingBox())!;
    expect(await dragsAt(page, { x: narrowBar.x + 30, y: 24 }), "narrow traffic-light padding drags").toBe(true);
    expect(await dragsAt(page, { x: narrowVersion.x - 6, y: 24 }), "the narrow gap before the version drags").toBe(
      true
    );
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * With no session open, the main pane's half of the band still drags.
 *
 * Specification: SPEC.md "Desktop window chrome" ("With no session open, the
 * main pane's half of the band is empty background too"). The main pane then
 * has no top row of its own; a stand-in strip covers its top 48px. If that
 * strip lost its surface class, stopped rendering, or sat behind the empty
 * pane's text, half the band would be dead in exactly the state a fresh
 * install starts in, and no other test would notice. The fleet listing is
 * served empty so the pane shows its "no sessions" state, and the build
 * mismatch banner (whose band would sit above the shell) must be absent.
 */
test("with no session open the main pane's half of the band drags", async ({ page }) => {
  // The real reply's headers carry the helm's build stamp; only the body is
  // replaced, so the page sees its own helm reporting an empty fleet.
  await page.route(SESSION_LISTING, async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, json: { sessions: [], total: 0, matching: 0, truncated: false } });
  });
  await page.setViewportSize({ width: 1100, height: 700 });
  await page.goto("/");
  // Fixture premise: the empty-fleet state, in the ordinary layout.
  await expect(page.locator(".main-empty")).toHaveText("no sessions — create one");
  await expect(page.locator(".session-topbar")).toHaveCount(0);
  await expect(page.locator(".build-skew")).toHaveCount(0);
  await page.locator(".app-shell").evaluate((node) => node.classList.add("macos-window"));
  await expect(page.locator("html")).not.toHaveClass(/sidebar-narrow-window/);
  await loadPressClassifier(page);

  const strip = page.locator(".main-empty-titlebar");
  const main = (await page.locator(".app-main").boundingBox())!;
  const stripBox = (await strip.boundingBox())!;
  expect(stripBox.x).toBeCloseTo(main.x, 0);
  expect(stripBox.y).toBeCloseTo(0, 0);
  expect(stripBox.width).toBeCloseTo(main.width, 0);
  expect(stripBox.height).toBe(48);
  // The strip is what a press there actually hits, not the text behind it.
  const center = { x: stripBox.x + stripBox.width / 2, y: 24 };
  const hit = await page.evaluate(
    ({ x, y }) => document.elementFromPoint(x, y)?.classList.contains("main-empty-titlebar") ?? false,
    center
  );
  expect(hit, "a press on the main pane's top hits the strip").toBe(true);

  await expectBandDrags(page, "no session open");
  expect(await dragsAt(page, center), "the main pane's top drags the window").toBe(true);
});
