/**
 * Modal boundaries keep keyboard navigation inside their actual controls.
 * Dialog-specific suites cover request ownership and close destinations;
 * these cases exercise the shared trap and page isolation on rendered roots.
 */
import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import {
  cleanupSession, createSession, FAKE_AGENT, openRowMenu, pinAutoSelect,
  resetPreferences, stubFeed,
} from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { attachSession, waitForTermText } from "./helpers/term";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

// Controlled approval replies carry the real helm's build stamp; the shared
// hook also drains any remaining route handlers before the page is disposed.
installTerminalSuiteHooks();

/**
 * Specify the endpoints rather than deriving them from the trap's predicate.
 * Checking both directions from a focused root catches a boundary handler
 * that handles descendants correctly but leaves its own fallback stranded.
 */
async function expectBoundaries(page: Page, root: Locator, first: Locator, last: Locator) {
  await expect(first).toBeVisible();
  await expect(first).toBeEnabled();
  await expect(last).toBeVisible();
  await expect(last).toBeEnabled();
  await first.focus();
  await expect(first).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(last).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(first).toBeFocused();
  await root.focus();
  await expect(root).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(first).toBeFocused();
  await root.focus();
  await expect(root).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(last).toBeFocused();
}

test.afterEach(async ({ request }) => { await resetPreferences(request); });

for (const mode of ["sessions", "templates"] as const) {
  /**
   * Arrow-owned results stay outside sequential navigation in both switcher
   * modes. The two actual Tab stops must wrap even with visible result buttons;
   * a test with an empty list would miss a broad button selector admitting -1.
   */
  test(`the ${mode} switcher wraps search and close, excluding result options`, async ({ page, request }) => {
    const name = `modal-trap-${mode}-${Date.now()}`;
    const session = await createSession(request, { title: `${name}-source` });
    let bodyFailed = true;
    try {
      if (mode === "templates") {
        const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, {
          data: { kind: "command", command: FAKE_AGENT, yolo: false },
        });
        expect(saved.ok(), await saved.text()).toBe(true);
      }
      await pinAutoSelect(page, session.id);
      await page.goto("/");
      await attachSession(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      const terminal = page.locator("#terminal .xterm-helper-textarea");
      await terminal.focus();
      await expect(terminal).toBeFocused();
      await expect(page.locator(mode === "sessions" ? ".quick-switcher-trigger" : ".quick-switcher-template-trigger"))
        .toHaveCount(1);
      const mac = await page.evaluate(() => navigator.platform.includes("Mac"));
      await page.keyboard.press(mode === "sessions"
        ? (mac ? "Meta+k" : "Control+Shift+k")
        : (mac ? "Meta+j" : "Control+Shift+j"));
      const dialog = page.getByRole("dialog", { name: mode === "sessions" ? "quick switcher" : "template launcher" });
      const search = dialog.getByRole("combobox");
      const close = dialog.getByRole("button", { name: "close", exact: true });
      await expect(dialog).toBeVisible();
      await expect(search).toBeFocused();
      await search.fill(mode === "sessions" ? session.title : name);
      const result = mode === "sessions"
        ? dialog.locator(".quick-switcher-results .quick-switcher-row").filter({ hasText: session.title })
        : dialog.locator(".quick-switcher-template").filter({ hasText: name });
      await expect(result).toHaveCount(1);
      await expect(result).toBeVisible();
      await expect(result).toBeEnabled();
      await expect(result).toHaveAttribute("tabindex", "-1");
      const options = dialog.getByRole("option");
      expect(await options.count(), "the exclusion requires rendered result options").toBeGreaterThan(0);
      for (const option of await options.all()) await expect(option).toHaveAttribute("tabindex", "-1");

      await search.focus();
      await expect(search).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(close, "native forward navigation skips arrow-owned options").toBeFocused();
      await page.keyboard.press("Shift+Tab");
      await expect(search).toBeFocused();
      await expectBoundaries(page, dialog, search, close);
      await page.keyboard.press("Escape");
      await expect(dialog).toHaveCount(0);
      await expect(terminal).toBeFocused();
      bodyFailed = false;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["source session", () => cleanupSession(request, session.id)],
        ["owned template", async () => {
          if (mode !== "templates") return;
          const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }],
      ]);
    }
  });
}

/**
 * Ordinary New gains the same isolation as hidden preparation. A preexisting
 * inert main pane must survive close; a new sibling must become inert while
 * open and become usable again afterwards. The sibling is a controlled DOM
 * mutation of the kind the isolation observer promises to handle, not an
 * exemption or a replacement modal. The form and all boundary targets are real.
 */
test("ordinary New isolates the page and wraps its concrete command controls", async ({ page }) => {
  let bodyFailed = true;
  try {
    await page.goto("/");
    const opener = page.locator(".new-session-button");
    await expect(opener).toBeEnabled();
    expect(await opener.evaluate((node) => document.querySelector(".app-main")!.contains(node)),
      "the preexisting inert fixture must not disable the New opener").toBe(false);
    await expect(page.locator("[inert]")).toHaveCount(0);
    await page.locator(".app-main").evaluate((node) => node.setAttribute("inert", ""));
    await expect(page.locator("[inert]")).toHaveCount(1);
    await opener.click();
    const form = page.getByRole("dialog", { name: "launch a session" });
    await expect(form).toBeVisible();
    await expect(form.locator('.launch-composer-search input[role="combobox"]')).toBeFocused();
    expect(await form.evaluate((node) => document.querySelector(".app-main")!.contains(node)),
      "the preexisting inert fixture must be outside the modal's ancestor path").toBe(false);
    await expect.poll(() => opener.evaluate((node) => node.closest("[inert]") !== null)).toBe(true);
    expect(await form.evaluate((node) => node.closest("[inert]") !== null)).toBe(false);

    await form.getByRole("tab", { name: "command", exact: true }).click();
    await expect(form).toHaveAttribute("data-composer-mode", "command");
    // The shared frame follows reading order: Close, fields, then the footer
    // with its primary last. Keep native wrapping rather than old top actions.
    const first = form.locator(".launch-composer-close");
    const last = form.locator(".create-session-submit");
    const agentType = form.getByRole("combobox", { name: "agent type", exact: true });
    await expect(agentType).toBeVisible();
    await expect(agentType).toBeEnabled();
    await agentType.selectOption("");
    await form.getByLabel("agent command", { exact: true }).fill(FAKE_AGENT);
    await form.getByRole("radio", { name: "no", exact: true }).check();
    await expect(agentType).toHaveValue("");
    await expect(form.locator(".launch-command-resume-toggle, .launch-composer-browser")).toHaveCount(0);
    await expectBoundaries(page, form, first, last);

    const late = page.locator('[data-modal-late-sibling="composer"]');
    await page.evaluate(() => {
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.modalLateSibling = "composer";
      button.textContent = "late isolation fixture";
      document.body.append(button);
    });
    await expect(late).toBeVisible();
    await expect(late).toHaveAttribute("inert", "");
    await expect(last).toBeFocused();
    await late.evaluate((node) => (node as HTMLElement).focus());
    await expect(last, "new outside siblings cannot take focus").toBeFocused();
    await form.locator(".launch-composer-cancel").click();
    await expect(form).toHaveCount(0);
    await expect(opener).toBeFocused();
    await expect(page.locator("[inert]")).toHaveCount(1);
    await expect(page.locator(".app-main")).toHaveAttribute("inert", "");
    await expect(late).not.toHaveAttribute("inert", "");
    await late.focus();
    await expect(late, "the observer's own inert claim is released").toBeFocused();
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["late sibling", () => page.locator('[data-modal-late-sibling="composer"]').evaluateAll((nodes) => nodes.forEach((node) => node.remove()))],
      ["preexisting inert fixture", () => page.locator(".app-main").evaluate((node) => node.removeAttribute("inert"))],
    ]);
  }
});

/**
 * Rename gets page isolation without losing its independent approval owner.
 * The real card's enabled answer must receive native Enter while Rename is
 * open; merely placing a fabricated exempt element beside it proves less.
 * The header-to-footer Tab order must coexist with the textarea's initial
 * focus; adding Close must not hand the opening keystroke to a new target.
 */
test("Rename wraps Close through Save while a real approval remains usable", async ({ page, request }) => {
  const session = await createSession(request, { title: `modal-trap-rename-${Date.now()}` });
  let bodyFailed = true;
  let waiting = false;
  const answers: string[] = [];
  const approvalId = `modal-trap-approval-${Date.now()}`;
  try {
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    await page.route((url) => url.pathname === "/api/approvals", (route) => fulfillAsHelm(route, {
      json: { approvals: waiting ? [{
        id: approvalId, host_id: 1, host_name: "fixture-host",
        session: { id: session.id, title: session.title, host_name: "fixture-host" },
        expires_at_ms: Date.now() + 540_000,
        action: {
          kind: "launch", verb: "create", host_name: "fixture-host", cwd: "/work/project",
          title: null, source: null,
          launch: { kind: "agent", selection: { harness: "claude", model: "opus", effort: "high", permissions: "guarded" },
            start: ["claude", "--model", "opus", "--effort", "high", "--permission-mode", "auto", "{farhelm_args}"], resume: null },
        },
      }] : [] },
    }));
    await page.route((url) => url.pathname === `/api/approvals/${approvalId}`, async (route) => {
      expect(route.request().method()).toBe("POST");
      answers.push(route.request().postDataJSON().answer);
      waiting = false;
      await fulfillAsHelm(route, { status: 204 });
    });
    await page.goto("/");
    await expect.poll(() => feed.openSockets()).toBe(1);
    const row = page.locator(`[data-session-id="${session.id}"]`);
    await expect(row).toBeVisible();
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    const dialog = page.getByRole("dialog", { name: "rename session" });
    const field = dialog.getByRole("textbox", { name: "new session title" });
    const close = dialog.getByRole("button", { name: "Close rename", exact: true });
    const cancel = dialog.getByRole("button", { name: "Cancel", exact: true });
    const save = dialog.getByRole("button", { name: "Save", exact: true });
    await expect(dialog).toBeVisible();
    await expect(field).toBeFocused();
    await expect(field).toHaveValue(session.title);
    await expect.poll(() => row.evaluate((node) => node.closest("[inert]") !== null)).toBe(true);
    expect(await dialog.evaluate((node) => node.closest("[inert]") !== null)).toBe(false);
    await expect(dialog.locator(".dialog-header .rename-close")).toHaveCount(1);
    await expect(dialog.locator(".dialog-footer .dialog-actions button")).toHaveText(["Cancel", "Save"]);
    await expectBoundaries(page, dialog, close, save);
    await close.focus();
    await expect(close).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(field).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(cancel).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(save).toBeFocused();

    // Deliver the card after isolation is installed, through the real reader.
    waiting = true;
    expect(feed.openSockets(), "the owned feed must still be connected").toBe(1);
    feed.notify(2);
    const card = page.locator(`.approval-card[data-approval-id="${approvalId}"]`);
    const allow = card.getByRole("button", { name: "allow", exact: true });
    await expect(card).toBeVisible();
    await expect(allow).toBeVisible();
    await expect(allow).toBeEnabled();
    expect(await allow.evaluate((node) => node.closest("[data-modal-exempt]") !== null)).toBe(true);
    expect(await allow.evaluate((node) => node.closest("[inert]") !== null)).toBe(false);
    await allow.focus();
    await expect(allow).toBeFocused();
    await page.keyboard.press("Enter");
    await expect.poll(() => answers).toEqual(["allow"]);
    await expect(card).toHaveCount(0);
    await expect(dialog).toBeVisible();
    await expect(field).toHaveValue(session.title);
    await cancel.click();
    await expect(dialog).toHaveCount(0);
    await expect(row.locator(".session-row-menu")).toBeFocused();
    expect(await row.evaluate((node) => node.closest("[inert]") !== null)).toBe(false);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["approval routes", () => page.unrouteAll({ behavior: "wait" })],
      ["source session", () => cleanupSession(request, session.id)],
    ]);
  }
});

/**
 * A held save leaves Templates visible with every descendant control disabled.
 * Tab must remain on its root in both directions. The root itself is not a
 * descendant candidate, even when it is also a keyboard-reachable scroll
 * region with tabindex=0; it cannot make the empty-set branch disappear.
 */
test("busy Templates contains Tab on its root with no enabled descendant", async ({ page, request }) => {
  const name = `modal-trap-busy-${Date.now()}`;
  const gate = routeGate();
  let held = false;
  let bodyFailed = true;
  try {
    const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: { name: "before" } });
    expect(saved.ok(), await saved.text()).toBe(true);
    await page.route(`**/api/templates/${encodeURIComponent(name)}`, async (route) => {
      if (route.request().method() !== "PUT") return route.continue();
      held = true;
      await gate.wait();
      await route.fulfill({ response: await route.fetch() });
    });
    await page.goto("/");
    // The sidebar icon and the dialog keep distinct accessible names.
    await page.getByRole("button", { name: "Templates", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await expect(dialog.locator(".templates-retry")).toHaveCount(0);
    await dialog.locator(".templates-row").filter({ has: page.getByText(name, { exact: true }) }).click();
    await dialog.getByLabel("session name", { exact: true }).fill("after");
    const save = dialog.getByRole("button", { name: "Save", exact: true });
    await expect(save).toBeEnabled();
    await save.click();
    await expect.poll(() => held, { message: "the owned PUT must remain held during keyboard assertions" }).toBe(true);
    await expect(dialog.locator(".templates-close")).toBeDisabled();
    const visibleControls = dialog.locator("button:visible, input:visible, textarea:visible, select:visible");
    expect(await visibleControls.count()).toBeGreaterThan(0);
    for (const control of await visibleControls.all()) await expect(control).toBeDisabled();
    await expect(dialog.locator('[tabindex="0"], a[href], summary, [contenteditable="true"]')).toHaveCount(0);
    await dialog.focus();
    await expect(dialog).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog).toBeFocused();
    await expect(save).toBeDisabled();
    expect(held, "the request was not released to make the empty state disappear").toBe(true);
    gate.release();
    await expect(dialog.locator(".templates-close")).toBeEnabled();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    await dialog.locator(".templates-close").click();
    await expect(dialog).toHaveCount(0);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["held save", () => gate.release()],
      ["save route", () => page.unrouteAll({ behavior: "wait" })],
      ["owned template", async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});
