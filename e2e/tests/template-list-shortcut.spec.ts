/** Cmd-J uses the quick switcher's modal handoff and the direct template path.
 * The tests distinguish an exact name from a preceding substring row and a
 * fresh catalog read from the switcher's earlier snapshot.
 */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import type { APIRequestContext, Page } from "@playwright/test";
import {
  cleanupSession, createSession, FAKE_AGENT, patchPreferences,
  pinAutoSelect, resetPreferences,
} from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";
import { routeGate } from "./helpers/route-gate";

/** Fix both the capture listener and terminal's idea of this client. */
async function platform(page: Page, mac: boolean) {
  await page.addInitScript((value) => {
    Object.defineProperty(Navigator.prototype, "platform", { get: () => value });
  }, mac ? "MacIntel" : "Linux x86_64");
}

/** Persist a runnable command template before the browser takes a snapshot. */
async function saveCommand(request: APIRequestContext, name: string) {
  const response = await request.put(`/api/templates/${encodeURIComponent(name)}`, {
    data: { kind: "command", command: FAKE_AGENT, yolo: false, name: `${name}-session` },
  });
  expect(response.ok(), await response.text()).toBe(true);
}

/** The relay appears only after authentication and preference loading. */
async function openTemplateList(page: Page, mac = false) {
  await expect(page.locator(".quick-switcher-template-trigger")).toHaveCount(1);
  await page.keyboard.press(mac ? "Meta+j" : "Control+Shift+j");
  const dialog = page.getByRole("dialog", { name: "template launcher" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("combobox")).toBeFocused();
  return dialog;
}

test.afterEach(async ({ request }) => { await resetPreferences(request); });

/** The row shown as selected must be the one Enter starts, even when another
 * saved name is listed first and both contain the typed query. */
test("Cmd-J exact name launches through the fresh quick-launch path", async ({ page, request }) => {
  await platform(page, false);
  const name = `cmdj-exact-${Date.now()}`;
  const earlier = `a${name}`;
  const source = await createSession(request, { title: `${name}-source` });
  const created: string[] = [];
  let posts = 0;
  let bodyFailed = false;
  try {
    await saveCommand(request, earlier);
    await saveCommand(request, name);
    await patchPreferences(request, { quick_launch_template: name, template_shortcuts: { "1": name } });
    await pinAutoSelect(page, source.id);
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      posts++;
      const response = await route.fetch();
      if (response.ok()) created.push((await response.json()).id);
      await route.fulfill({ response });
    });
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const terminal = page.locator("#terminal .xterm-helper-textarea");
    await expect(terminal).toBeFocused();
    // The sequence pill is not a modal. Cmd-J must retire its queued
    // template and take the same list path as an idle page press.
    await page.keyboard.press("Control+Shift+x");
    await page.keyboard.press("2");
    const dialog = await openTemplateList(page);
    await dialog.getByRole("combobox").fill(name);
    const rows = dialog.locator(".quick-switcher-template");
    await expect(rows).toHaveCount(2);
    await expect(rows.first().locator(".quick-switcher-template-name")).toHaveText(earlier);
    await expect(rows.nth(1).locator(".quick-switcher-template-name")).toHaveText(name);
    await expect(rows.nth(1).locator(".quick-switcher-template-summary")).not.toBeEmpty();
    await expect(rows.nth(1)).toHaveAttribute("aria-selected", "true");
    await expect(rows.nth(1).locator(".quick-switcher-template-keys")).toContainText("Ctrl+Shift+P");
    await expect(rows.nth(1).locator(".quick-switcher-template-keys")).toContainText("Ctrl+Shift+1");
    await page.keyboard.press("ArrowUp");
    await expect(rows.first()).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("ArrowDown");
    await expect(rows.nth(1)).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    await expect.poll(() => created.length).toBe(1);
    await expect.poll(() => posts).toBe(1);
    await expect(page.locator(`[data-session-id="${created[0]}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(".rename-dialog .rename-input")).toHaveValue(`${name}-session`);
    await page.locator(".rename-dialog .rename-cancel").click();
    await expect(terminal).toBeFocused();
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ["created sessions", () => cleanupOwnedSteps(false, created.map((id) => [id, () => cleanupSession(request, id)]))],
      ["source session", () => cleanupSession(request, source.id)],
      ...[name, earlier].map((template) => [template, async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(template)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }] as [string, () => Promise<void>]),
    ]);
  }
});

/** Other dialogs own Cmd-J, and a hidden quick-launch form reserves it while
 * the launch catalog is still pending. Neither context may open a second list.
 */
test("Cmd-J yields to dialogs and hidden quick-launch preparation", async ({ page, request }) => {
  const name = `cmdj-preparing-${Date.now()}`;
  const gate = routeGate();
  let catalogReads = 0;
  const created: string[] = [];
  // An incomplete body keeps its original error if an owned release also fails.
  let bodyFailed = true;
  try {
    await saveCommand(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    await page.goto("/");
    await expect(page.locator(".quick-switcher-template-trigger")).toHaveCount(1);
    await page.getByRole("button", { name: "templates", exact: true }).click();
    const templates = page.locator('.templates-dialog[role="dialog"]');
    await expect(templates).toBeVisible();
    await page.keyboard.press("Control+Shift+j");
    await expect(page.locator(".quick-switcher-dialog")).toHaveCount(0);
    await templates.locator(".templates-close").click();
    await expect(templates).toHaveCount(0);

    await page.route("**/api/launch-catalog", async (route) => {
      catalogReads++;
      await gate.wait();
      await route.fulfill({ response: await route.fetch() });
    });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const response = await route.fetch();
      if (response.ok()) created.push((await response.json()).id);
      await route.fulfill({ response });
    });
    await page.keyboard.press("Control+Shift+p");
    await expect.poll(() => catalogReads).toBeGreaterThan(0);
    await expect(page.locator(".create-session-form")).toHaveCount(1);
    await expect(page.locator(".create-session-form")).toBeHidden();
    await page.keyboard.press("Control+Shift+j");
    await expect(page.locator(".quick-switcher-dialog")).toHaveCount(0);
    await expect(page.locator(".create-session-form")).toBeHidden();
    gate.release();
    await expect.poll(() => created.length).toBe(1);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["catalog gate", () => gate.release()],
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ["created sessions", async () => {
        // Draining routes can reveal a create that was still pending at failure.
        await cleanupOwnedSteps(false, created.map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]));
      }],
      [name, async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});

/** An empty result must leave Enter inert; Escape still returns to the
 * terminal. A partial row uses New without sending a create request. */
test("Cmd-J has no-match and partial-template fallbacks", async ({ page, request }) => {
  const name = `cmdj-partial-${Date.now()}`;
  const source = await createSession(request, { title: `${name}-source` });
  let posts = 0;
  let bodyFailed = true;
  try {
    const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: { effort: "high" } });
    expect(saved.ok(), await saved.text()).toBe(true);
    await pinAutoSelect(page, source.id);
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() === "POST") posts++;
      await route.continue();
    });
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const terminal = page.locator("#terminal .xterm-helper-textarea");
    let dialog = await openTemplateList(page);
    await dialog.getByRole("combobox").fill(`${name}-absent`);
    await expect(dialog.getByRole("option")).toHaveCount(0);
    await page.keyboard.press("Enter");
    await expect(dialog).toBeVisible();
    expect(posts).toBe(0);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(terminal).toBeFocused();

    dialog = await openTemplateList(page);
    await dialog.getByRole("combobox").fill(name);
    await expect(dialog.getByRole("option")).toHaveCount(1);
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".create-session-form")).toBeVisible();
    expect(posts).toBe(0);
    await page.keyboard.press("Control+Shift+j");
    await expect(page.locator(".quick-switcher-dialog")).toHaveCount(0);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ["source session", () => cleanupSession(request, source.id)],
      [name, async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});

/** Enter must not give authority to a template captured before another
 * window removed it. The existing New error slot explains that refusal. */
test("Cmd-J rechecks a selected name after the list closes", async ({ page, request }) => {
  const name = `cmdj-deleted-${Date.now()}`;
  let posts = 0;
  try {
    await saveCommand(request, name);
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() === "POST") posts++;
      await route.continue();
    });
    await page.goto("/");
    const dialog = await openTemplateList(page);
    await dialog.getByRole("combobox").fill(name);
    await expect(dialog.getByRole("option")).toHaveCount(1);
    const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
    expect(removed.ok()).toBe(true);
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-error")).toContainText("no longer available");
    expect(posts).toBe(0);
  } finally {
    await page.unrouteAll({ behavior: "wait" });
    const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
    expect(removed.ok() || removed.status() === 404).toBe(true);
  }
});
