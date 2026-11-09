/** Host settings must change the configuration that a real launch preview
 * resolves, rather than a draft or a host-list decoration. No session is
 * launched and no Git operation or remote host is needed for these checks. */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { drainedRoute } from "./helpers/drained-route";
import { openHostMenu, openHostsPanel, showSessionsPanel } from "./helpers/fleet";
import { currentStackSlot } from "../stack-slots";
import type { APIRequestContext, Page } from "@playwright/test";
import { mkdtemp, readFile, realpath, rm } from "node:fs/promises";
import path from "node:path";

type FolderSettings = { root: string | null; default: string | null };

/** Use the actual protected route for independent save/clear evidence. */
async function readFolder(request: APIRequestContext, host: number): Promise<FolderSettings> {
  const reply = await request.get(`/api/hosts/${host}/checkout-root`);
  expect(reply.ok(), await reply.text()).toBe(true);
  return reply.json();
}

/** Open this host's real dialog without relying on its mutable display name. */
async function openFolder(page: Page, host: number) {
  await openHostsPanel(page);
  const row = page.locator(`.host-row[data-host-id="${host}"]`);
  await expect(row).toBeVisible();
  await openHostMenu(row);
  await row.locator(".host-settings").click();
  const dialog = page.locator(".host-settings-dialog");
  await expect(dialog).toBeVisible();
  const field = dialog.locator(".checkout-root-field");
  const folder = field.getByRole("textbox", { name: "Checkout folder", exact: true });
  await expect(folder).toBeEditable();
  return { dialog, field, folder, save: field.getByRole("button", { name: "Save", exact: true }) };
}

/** Inheritance is resolved for each preview, so a default change must reach
 * hosts already registered while a saved override must keep taking priority.
 * Real route readbacks and reopening distinguish saved state from local text;
 * the invalid-path case proves refusals leave that saved state intact. */
test("host checkout folder saves, inherits, clears and reaches launch preview", async ({ page, request }, testInfo) => {
  const listing = await request.get("/api/hosts");
  expect(listing.ok()).toBe(true);
  const hosts = (await listing.json()).hosts;
  const host = hosts.find((candidate: any) => candidate.kind === "local");
  expect(host, "the owned stack must have a connected local host").toBeTruthy();
  expect(host.state.phase).toBe("connected");
  expect(host.incarnation).toBeGreaterThan(0);
  const previous = await readFolder(request, host.id);
  const stack = JSON.parse(await readFile(currentStackSlot().stackInfoPath, "utf8")) as { state: string };
  const root = await realpath(await mkdtemp(path.join(stack.state, "host-checkout-settings-")));
  const defaultRoot = path.join(root, "default");
  const hostRoot = path.join(root, "host");
  const changedDefault = path.join(root, "changed-default");
  let bodyFailed = false;
  const stopSaves = await drainedRoute(page, `**/api/hosts/${host.id}/checkout-root`, async route => {
    if (route.request().method() !== "PUT") return route.continue();
    await route.fulfill({ response: await route.fetch() });
  });
  try {
    expect((await request.put("/api/checkout-root", { data: { root: defaultRoot } })).ok()).toBe(true);
    expect((await request.put(`/api/hosts/${host.id}/checkout-root`, { data: { root: null } })).ok()).toBe(true);
    expect(await readFolder(request, host.id)).toEqual({ root: null, default: defaultRoot });
    await page.goto("/");
    const { dialog, field, folder, save } = await openFolder(page, host.id);
    await expect(folder).toHaveValue("");
    await expect(folder).toHaveAttribute("placeholder", `Default: ${defaultRoot}`);
    await expect(field.getByRole("button", { name: "Use default", exact: true })).toHaveCount(0);
    await expect(save).toHaveAttribute("aria-disabled", "true");
    await expect(field).toContainText("new checkouts");

    await folder.fill("~another-user/work");
    await save.click();
    await expect(field.getByRole("alert")).toBeVisible();
    await expect(folder).toHaveValue("~another-user/work");
    expect(await readFolder(request, host.id)).toEqual({ root: null, default: defaultRoot });
    await folder.fill(hostRoot);
    expect(await readFolder(request, host.id)).toEqual({ root: null, default: defaultRoot });
    await save.click();
    await expect(field.getByRole("status")).toHaveText("Checkout folder saved");
    expect(await readFolder(request, host.id)).toEqual({ root: hostRoot, default: defaultRoot });
    await expect(field.getByRole("button", { name: "Use default", exact: true })).toBeVisible();

    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 800 });
      await folder.scrollIntoViewIfNeeded();
      await expect(dialog.locator(".host-settings-close")).toBeVisible();
      expect(await field.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
      await dialog.screenshot({ path: testInfo.outputPath(`host-checkout-folder-${width}.png`) });
    }
    await page.setViewportSize({ width: 1280, height: 800 });
    await dialog.locator(".host-settings-close").click();
    await showSessionsPanel(page);
    await page.locator(".new-session-button").click();
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toHaveAttribute("data-host-id", String(host.id));
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill("gh:acme/bar");
    const choice = form.getByRole("option", { name: "Managed checkout: acme/bar", exact: true });
    await expect(choice).toHaveAttribute("aria-selected", "true");
    await search.press("Enter");
    const preview = form.locator(".launch-composer-checkout-preview");
    await expect(preview).toHaveAttribute("data-preview-state", "ready");
    await expect(preview).toContainText(hostRoot);
    await form.getByRole("button", { name: "Cancel", exact: true }).click();

    const reopened = await openFolder(page, host.id);
    await expect(reopened.folder).toHaveValue(hostRoot);
    await reopened.field.getByRole("button", { name: "Use default", exact: true }).click();
    await expect(reopened.folder).toHaveValue("");
    expect(await readFolder(request, host.id)).toEqual({ root: null, default: defaultRoot });
    await expect(reopened.field.getByRole("button", { name: "Use default", exact: true })).toHaveCount(0);
    await reopened.folder.fill(hostRoot);
    await reopened.save.click();
    await expect(reopened.field.getByRole("status")).toHaveText("Checkout folder saved");
    await reopened.folder.fill("");
    await reopened.save.click();
    await expect(reopened.field.getByRole("status")).toHaveText("Checkout folder saved");
    expect(await readFolder(request, host.id)).toEqual({ root: null, default: defaultRoot });
    await reopened.dialog.locator(".host-settings-close").click();

    expect((await request.put("/api/checkout-root", { data: { root: changedDefault } })).ok()).toBe(true);
    const inherited = await request.post("/api/github-checkout-preview", {
      data: { host: host.id, expected_incarnation: host.incarnation, repo: "acme/bar", title: null },
    });
    expect(inherited.ok(), await inherited.text()).toBe(true);
    expect((await inherited.json()).canonical_root).toBe(changedDefault);
    expect((await request.put("/api/checkout-root", { data: { root: null } })).ok()).toBe(true);
    const unset = await openFolder(page, host.id);
    await expect(unset.folder).toHaveValue("");
    await expect(unset.folder).toHaveAttribute("placeholder", "No default set");
    await unset.dialog.locator(".host-settings-close").click();
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    // Drain the backend forwarding work, including a response lost by the
    // browser, before restoring configuration or deleting its directory.
    await cleanupOwnedSteps(bodyFailed, [
      ["submitted host-folder saves", stopSaves],
      ["host checkout override", async () => {
        const reply = await request.put(`/api/hosts/${host.id}/checkout-root`, { data: { root: previous.root } });
        expect(reply.ok(), await reply.text()).toBe(true);
      }],
      ["default checkout folder", async () => {
        const reply = await request.put("/api/checkout-root", { data: { root: previous.default } });
        expect(reply.ok(), await reply.text()).toBe(true);
      }],
      ["owned preview directory", () => rm(root, { recursive: true, force: true })],
    ]);
  }
});
