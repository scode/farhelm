/**
 * The window-drag spacer's web-side contract: present but hidden, with no
 * bridge script and no reachable handler — plus the drag-region boundary
 * geometry the macOS shell class switches on.
 *
 * Why this file exists: the desktop double-click work (window_chrome.rs)
 * adds a page script and a Rust decision to the spacer's mousedown. The
 * web build must take NEITHER — the script is desktop-only rendering, and
 * the web handler is an inert `let _ = event`. A wiring slip (script in
 * the shared shell, handler enabled by OS rather than by shell class)
 * would be invisible to Rust tests and to every existing browser spec:
 * nothing else asserts the script's absence, the spacer's hidden default,
 * or that the forced-class spacer sits strictly BETWEEN the header
 * controls rather than overlapping one. The sibling macOS-geometry test
 * in sidebar.spec.ts pins native-button clearance while scrolling; this
 * file pins the spacer's own box and the web's inertness.
 *
 * What this file deliberately does NOT prove: native dragging, zooming,
 * or the click-count bridge itself. Those need AppKit and live in
 * docs/manual-mac-checklist.md's Integrated window header section.
 */
import { expect, test } from "./helpers/evidence";

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
  await expect(bar.locator(".profiles-toggle")).toBeVisible();
  await expect(bar.locator(".app-version")).not.toHaveText("");

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

test("the forced-class spacer fills only the gap between Profiles and the version", async ({
  page,
}) => {
  const pageErrors: Error[] = [];
  page.on("pageerror", (err) => pageErrors.push(err));
  await page.setViewportSize({ width: 1100, height: 700 });
  await page.goto("/");
  const shell = page.locator(".app-shell");
  const bar = page.locator(".app-bar");
  const profiles = bar.locator(".profiles-toggle");
  const version = bar.locator(".app-version");
  const spacer = bar.locator(".window-drag-region");
  await expect(profiles).toBeVisible({ timeout: 20_000 });
  await expect(version).not.toHaveText("");

  // Opt production markup into its macOS CSS, as sidebar.spec.ts's
  // native-button test does. Only the class is synthetic; every node
  // measured below is production.
  await shell.evaluate((element) => element.classList.add("macos-window"));
  await expect(spacer).toBeVisible();

  const profilesBox = (await profiles.boundingBox())!;
  const versionBox = (await version.boundingBox())!;
  const spacerBox = (await spacer.boundingBox())!;
  expect(profilesBox, "Profiles must have a box").not.toBeNull();
  expect(versionBox, "the version must have a box").not.toBeNull();
  expect(spacerBox, "the spacer must have a box").not.toBeNull();
  // The drag region is the gap and only the gap: strictly between the
  // two controls horizontally, sharing their row vertically.
  expect(spacerBox.x).toBeGreaterThanOrEqual(profilesBox.x + profilesBox.width - 1);
  expect(spacerBox.x + spacerBox.width).toBeLessThanOrEqual(versionBox.x + 1);
  expect(spacerBox.y).toBeLessThanOrEqual(profilesBox.y + profilesBox.height);
  expect(spacerBox.y + spacerBox.height).toBeGreaterThanOrEqual(profilesBox.y);
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
  const profiles = bar.locator(".profiles-toggle");
  const version = bar.locator(".app-version");
  const spacer = bar.locator(".window-drag-region");
  await expect(profiles).toBeVisible({ timeout: 20_000 });
  await expect(version).not.toHaveText("");
  await shell.evaluate((element) => element.classList.add("macos-window"));
  await expect(spacer).toBeVisible();

  // Control activation beside the spacer: Profiles still opens its
  // popup, proving no spacer handler intercepts control presses.
  await profiles.click();
  await expect(page.locator(".profiles-popover")).toBeVisible({ timeout: 10_000 });
  await page.keyboard.press("Escape");
  await expect(page.locator(".profiles-popover")).toBeHidden({ timeout: 10_000 });

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
