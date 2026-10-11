/**
 * Modal geometry uses overflowing real dialog content, not an empty card.
 *
 * Routes control only the host/list/archive facts needed to make each style
 * family tall. The renderer, modal isolation and CSS remain the shipped app;
 * screenshots precede the geometry assertion so a failed margin has evidence.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator, type Page } from "@playwright/test";
import { agentLaunchRow, hostRowByName, openHostMenu, openRowMenu, SESSION_LISTING } from "./helpers/fleet";
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
      path: `/work/checkouts/farhelm-archived-working-copies/archive-${i}`,
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
    await page.locator(".checkout-trash-button").click();
    const dialog = page.locator(".checkout-trash-dialog");
    await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(24);
    return dialog;
  }
  if (family === "templates") {
    await page.getByRole("button", { name: "templates", exact: true }).click();
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
    await page.route(`**/api/sessions/${SESSION}/rename`, (route) =>
      fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: LONG_REFUSAL }));
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    const dialog = page.locator(".rename-dialog");
    await dialog.locator(".rename-input").fill("edited layout title");
    await dialog.locator(".rename-submit").click();
    await expect(dialog.locator(".rename-error")).toContainText("cannot be saved");
    return dialog;
  }
  await page.route(`**/api/sessions/${SESSION}/restart`, (route) =>
    fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: LONG_REFUSAL }));
  await row.locator(".session-row-open").click();
  await page.locator(".restart-with-trigger").click();
  const dialog = page.locator(".restart-with-dialog");
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  await dialog.locator(".restart-with-submit").click();
  await expect(dialog.locator(".restart-with-error")).toContainText("cannot be saved");
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
