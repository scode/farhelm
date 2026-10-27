/**
 * Modal geometry uses overflowing real dialog content, not an empty card.
 *
 * Routes control only the host/list/archive facts needed to make each style
 * family tall. The renderer, modal isolation and CSS remain the shipped app;
 * screenshots precede the geometry assertion so a failed margin has evidence.
 * The keyboard guard inspects actual overflow, including horizontal overflow;
 * a declared scroll style alone does not establish a region that needs access.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator, type Page } from "@playwright/test";
import { agentLaunchRow, hostRowByName, openHostMenu, openRowMenu, SESSION_LISTING } from "./helpers/fleet";
import { selectSessionActionWith } from "./helpers/session-actions";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

const PEER = "layout-host.example.invalid";
const SESSION = "77777777-2222-3333-4444-555555555555";
const LONG_REFUSAL = "The requested value cannot be saved; keep this draft and correct the destination. ".repeat(30);

/** A connected remote supplies Appearance as well as Connection and Permissions. */
async function stubHost(page: Page) {
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.hosts = [...body.hosts.filter((host: any) => host.kind === "local"), {
      id: 9011, kind: "ssh", destination: PEER, name: PEER,
      identity: "layout-identity", incarnation: 1,
      remote_farhelm: null, remote_state_dir: null,
      state: { phase: "connected", identity: "layout-identity", build_version: "0.1.0",
        refresh: { status: "ok", sessions: 0 } },
    }];
    await route.fulfill({ response, json: body });
  });
}

/** Archives are display fixtures; no disk tree or destructive operation is needed. */
async function stubTrash(page: Page) {
  await page.route("**/api/checkout-trash/list", async (route) => {
    const { host } = route.request().postDataJSON();
    await fulfillAsHelm(route, { json: { issues: [], checkouts: Array.from({ length: 12 }, (_, i) => ({
      id: `archive-${host}-${i}`, name: `archived-checkout-${i}`, repository: "example/repository",
      path: `/work/checkouts/.farhelm-trash/archive-${i}`,
      archived_at: 1_700_000_000 + i, bytes: 1024,
    })) } });
  });
}

/** Preserve real host metadata while offering a resumable row for modal editing. */
async function stubSession(page: Page) {
  await page.route(SESSION_LISTING, async (route) => {
    const response = await route.fetch();
    const listing = await response.json();
    expect(listing.sessions.length, "the stack supplies the injected row's host metadata").toBeGreaterThan(0);
    listing.sessions.push({ ...listing.sessions[0], id: SESSION, title: "layout-session", cwd: "/work",
      invocation: "codex", launch: agentLaunchRow({ harness: "codex", model: null, effort: null,
        permissions: null, workspace_trust: null }), status: { state: "interrupted" }, restart_offer: "resume" });
    listing.total += 1;
    await route.fulfill({ response, json: listing });
  });
}

/** Exercise each distinct height cap with content that reaches its scrolling ceiling. */
async function openTallDialog(page: Page, family: string): Promise<Locator> {
  await stubHost(page);
  if (family === "trash") await stubTrash(page);
  if (family === "rename" || family === "restart-with") await stubSession(page);
  await page.goto("/");
  if (family === "host-settings") {
    const row = hostRowByName(page, PEER);
    await openHostMenu(row);
    await row.locator(".host-settings").click();
    return page.locator(".host-settings-refreshed");
  }
  if (family === "trash") {
    // The count proves both host fixtures have reached the view before this
    // layout test opens a dialog that must contain all their rows.
    const button = page.locator(".checkout-trash-button");
    await expect(button).toHaveAttribute("data-count", "24");
    await button.click();
    const dialog = page.locator(".checkout-trash-dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(24);
    return dialog;
  }
  if (family === "templates") {
    await page.getByRole("button", { name: "Templates", exact: true }).click();
    const dialog = page.locator(".templates-dialog");
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await dialog.locator(".templates-new").click();
    // Command fields are only offered for addition in command mode; establish it before
    // using their height to exercise the card's scrolling ceiling.
    await dialog.getByRole("group", { name: "switches launcher to", exact: true })
      .getByRole("button", { name: "command", exact: true }).click();
    await expect(dialog.getByRole("group", { name: "switches launcher to", exact: true })
      .getByRole("button", { name: "command", exact: true })).toHaveAttribute("aria-pressed", "true");
    for (const name of ["command", "resume command", "session name", "destination"]) {
      await dialog.locator(".templates-add").click();
      await dialog.locator(".templates-add-menu").getByRole("button", { name, exact: true }).click();
    }
    return dialog;
  }
  if (family === "launcher") {
    await page.locator(".new-session-button").click();
    const dialog = page.locator(".create-session-form");
    await dialog.getByRole("tab", { name: "command", exact: true }).click();
    await dialog.locator(".launch-composer-save-template").click();
    await expect(dialog.locator(".save-template-panel")).toBeVisible();
    return dialog;
  }
  const row = page.locator(`[data-session-id="${SESSION}"]`);
  await expect(row).toBeVisible({ timeout: 20_000 });
  if (family === "rename") {
    await page.route(`**/api/sessions/${encodeURIComponent(SESSION)}/rename`, (route) =>
      fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: LONG_REFUSAL }));
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    const dialog = page.locator(".rename-dialog");
    await dialog.locator(".rename-input").fill("edited layout title");
    await dialog.locator(".rename-submit").click();
    await expect(dialog.locator(".rename-error")).toContainText("cannot be saved");
    return dialog;
  }
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION)}/restart`, (route) =>
    fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: LONG_REFUSAL }));
  await row.locator(".session-row-open").click();
  await selectSessionActionWith(page, "Restart");
  const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
  // Cancel is the restart opening boundary; wait for it before replacing
  // focus with the permission edit that makes this long refusal possible.
  await expect(dialog.locator(".launch-composer-cancel")).toBeFocused();
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  await dialog.locator(".create-session-submit").click();
  await expect(dialog.locator(".launch-composer-refusal-alert")).toContainText("cannot be saved");
  return dialog;
}

/**
 * Every overflowing scrollport needs a keyboard entry point, including WebKit
 * where a plain overflow container does not become a Tab stop automatically.
 * A named region supplies its own stop when its content has no usable control.
 * Native editors already provide both focus and scrolling on the element itself.
 * Menus and suggestion lists own arrow-key navigation, so their roles exempt
 * their scrollports without adding a product attribute for the test to recognize.
 * This is a structural guard; the per-region tests exercise Tab and PageDown.
 */
async function expectKeyboardReachableScrollers(dialog: Locator) {
  const scan = await dialog.evaluate((node) => {
    const tabbable = (element: HTMLElement) => {
      const style = getComputedStyle(element);
      return element.tabIndex >= 0 && !element.matches(":disabled") &&
        !element.closest('[inert], [aria-disabled="true"]') &&
        style.visibility === "visible" && element.getClientRects().length > 0;
    };
    const namedRegion = (element: HTMLElement) => {
      const labelledBy = (element.getAttribute("aria-labelledby") ?? "").split(/\s+/)
        .map((id) => document.getElementById(id)?.textContent?.trim() ?? "").join(" ").trim();
      return element.getAttribute("role") === "region" &&
        Boolean(element.getAttribute("aria-label")?.trim() || labelledBy) && tabbable(element);
    };
    const scrollers = [];
    const unreachable = [];
    for (const element of [node, ...node.querySelectorAll("*")]) {
      if (!(element instanceof HTMLElement) || element.getClientRects().length === 0) continue;
      const style = getComputedStyle(element);
      if (style.visibility !== "visible") continue;
      const axes = [];
      if (["auto", "scroll"].includes(style.overflowX) && element.scrollWidth > element.clientWidth + 1) axes.push("x");
      if (["auto", "scroll"].includes(style.overflowY) && element.scrollHeight > element.clientHeight + 1) axes.push("y");
      if (axes.length === 0) continue;
      const description = `${element.tagName.toLowerCase()}.${[...element.classList].join(".")} (${axes.join(",")})`;
      scrollers.push(description);
      if (element.closest('[role="menu"], [role="listbox"]')) continue;
      if ((element.matches("input, textarea") || element.isContentEditable) && tabbable(element)) continue;
      if (namedRegion(element)) continue;
      if ([...element.querySelectorAll<HTMLElement>("*")].some(tabbable)) continue;
      unreachable.push(description);
    }
    return { scrollers, unreachable };
  });
  expect(scan.scrollers, "the dialog fixture has actual overflowing auto/scroll content").not.toEqual([]);
  expect(scan.unreachable, "each scrollport has a named Tab stop or an enabled tabbable control").toEqual([]);
}

/**
 * Keep confirmation and feedback fixtures independent of shared preferences.
 * A sent message stores contact even when it is empty; intercept that write as
 * well as the send so a layout assertion cannot change the next spec's seed.
 */
async function stubDialogPreferences(page: Page) {
  await page.route("**/api/preferences", async (route) => {
    if (route.request().method() === "PUT") {
      await fulfillAsHelm(route, { status: 204 });
      return;
    }
    const response = await route.fetch();
    const preferences = await response.json();
    await route.fulfill({ response, json: {
      ...preferences, skip_host_remove_confirmation: false, feedback_contact: null,
    } });
  });
}

/** Open the real help-menu entry after its menu is mounted. */
async function openHelpDialog(page: Page, name: string) {
  await page.getByRole("button", { name: "help", exact: true }).click();
  const menu = page.getByRole("menu", { name: "help", exact: true });
  await expect(menu).toBeVisible();
  await menu.locator(`[data-bar-menu-item="${name}"]`).click();
  const dialog = page.getByRole("dialog", { name, exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

for (const width of [1000, 360]) {
  for (const family of ["host-settings", "templates", "trash", "rename", "restart-with", "launcher"]) {
    /** Every separate modal style must preserve breathing room when content overflows. */
    test(`overflowing ${family} keeps vertical margins at width ${width}`, async ({ page }, info) => {
      await page.setViewportSize({ width, height: 720 });
      const dialog = await openTallDialog(page, family);
      await expect(dialog).toBeVisible();
      await page.screenshot({ path: info.outputPath(`${family}-${width}-tall.png`) });
      await page.setViewportSize({ width, height: 300 });
      // Host settings and templates can scroll inside their card; inspect all
      // descendants so pinning controls later cannot make this premise vacuous.
      expect(await dialog.evaluate((node) => [node, ...node.querySelectorAll("*")]
        .some((element) => element.scrollHeight > element.clientHeight + 1 &&
          ["auto", "scroll"].includes(getComputedStyle(element).overflowY))),
        `${family} has overflowing scroll content`).toBe(true);
      await page.screenshot({ path: info.outputPath(`${family}-${width}.png`) });
      const box = (await dialog.boundingBox())!;
      const margin = width <= 620 ? 12 : 24;
      expect(box.y, "space above the card").toBeGreaterThanOrEqual(margin - 1);
      expect(box.y + box.height, "space below the card").toBeLessThanOrEqual(300 - margin + 1);
    });
  }
}

for (const family of ["host-settings", "templates", "trash", "rename", "restart-with", "launcher"]) {
  /** A geometry-safe card still needs keyboard access to every nested scrollport. */
  test(`overflowing ${family} scrollports have keyboard entry points`, async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 720 });
    const dialog = await openTallDialog(page, family);
    await expect(dialog).toBeVisible();
    await page.setViewportSize({ width: 360, height: 300 });
    await expectKeyboardReachableScrollers(dialog);
  });
}

/** Empty help has no copy buttons to make its overflowing explanation reachable. */
test("text-only SSH help scrollports have keyboard entry points", async ({ page }) => {
  await page.route("**/api/ssh-connections", (route) => fulfillAsHelm(route, {
    json: { unavailable: [], hosts: [], extra: null },
  }));
  await page.setViewportSize({ width: 360, height: 720 });
  await page.goto("/");
  const dialog = await openHelpDialog(page, "ssh needs 2FA?");
  await expect(dialog).toContainText("No ssh hosts are registered.");
  await expect(dialog.locator(".ssh-help-body button")).toHaveCount(0);
  await page.setViewportSize({ width: 360, height: 220 });
  await expectKeyboardReachableScrollers(dialog);
});

/** Footer buttons cannot stand in for keyboard access to the text-only body. */
test("remove-host scrollports have keyboard entry points", async ({ page }) => {
  await stubDialogPreferences(page);
  await stubHost(page);
  await page.setViewportSize({ width: 360, height: 720 });
  await page.goto("/");
  const row = hostRowByName(page, PEER);
  await openHostMenu(row);
  await row.locator(".host-remove").click();
  const dialog = page.locator(".host-remove-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("Its supervisor and sessions keep running");
  await page.setViewportSize({ width: 360, height: 220 });
  await expectKeyboardReachableScrollers(dialog);
});

/**
 * Sent replaces the editable body with text. Freeze its 1.5-second close timer
 * before sending so this assertion cannot race dismissal on a busy browser.
 */
test("sent-feedback scrollports have keyboard entry points", async ({ page }) => {
  await stubDialogPreferences(page);
  let sends = 0;
  await page.route("**/api/feedback", async (route) => {
    sends += 1;
    await fulfillAsHelm(route, { status: 204 });
  });
  await page.setViewportSize({ width: 360, height: 720 });
  await page.goto("/");
  const dialog = await openHelpDialog(page, "send feedback");
  await dialog.locator(".feedback-message").fill("Keep the sent message reachable.");
  await page.clock.install();
  await page.clock.pauseAt(new Date(Date.now() + 5_000));
  await dialog.locator(".feedback-send").click();
  await expect(dialog.locator(".feedback-thanks")).toContainText("Thanks.");
  expect(sends, "the controlled send completed before inspecting its text-only state").toBe(1);
  await expect(dialog.locator(".feedback-message")).toHaveCount(0);
  await page.setViewportSize({ width: 360, height: 220 });
  // The paused clock also holds requestAnimationFrame, so the body has not yet
  // run the reconciliation frame, queued when Send replaced the editors with
  // text, that gives it a named Tab stop. Fifty milliseconds runs that frame,
  // well before Feedback's 1500 ms close timer, which stays held.
  await page.clock.runFor(50);
  await expect(dialog.locator(".feedback-thanks"), "premise: the sent state is still open").toBeVisible();
  await expectKeyboardReachableScrollers(dialog);
});

/**
 * A held save disables every body control. Inspect that unsettled boundary,
 * before the refusal restores the controls that normally provide Tab entry.
 */
test("host-settings scrollports stay reachable while saving", async ({ page }) => {
  let releaseSave!: () => void;
  const saveReleased = new Promise<void>((resolve) => (releaseSave = resolve));
  let submitted: unknown;
  await page.route("**/api/hosts/9011/destination", async (route) => {
    submitted = route.request().postDataJSON();
    await saveReleased;
    await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "held layout refusal" });
  });
  try {
    await page.setViewportSize({ width: 360, height: 720 });
    const dialog = await openTallDialog(page, "host-settings");
    await dialog.locator(".host-edit").click();
    const input = dialog.locator(".host-destination-input");
    await input.fill("edited-layout-host.example.invalid");
    await expect(input).toBeFocused();
    await page.keyboard.press("Enter");
    await expect.poll(() => submitted, { message: "the destination save is held" })
      .toEqual({ ssh: "edited-layout-host.example.invalid" });
    const body = dialog.locator(".host-settings-body");
    await expect(input).toBeDisabled();
    await expect(body.locator("button:enabled, input:enabled, select:enabled, textarea:enabled")).toHaveCount(0);
    await page.setViewportSize({ width: 360, height: 300 });
    await expectKeyboardReachableScrollers(dialog);
  } finally {
    releaseSave();
    await page.unrouteAll({ behavior: "wait" });
  }
});
