/**
 * A scrollport with no usable child must remain a keyboard destination in
 * WebKit. HTTP fixtures select the real renderer's empty and pending states;
 * Tab and PageDown exercise native focus and scrolling rather than DOM writes.
 */
import { type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { hostRowByName, openHostMenu, resetPreferences, SESSION_LISTING } from "./helpers/fleet";
import { expectKeyboardScroll } from "./helpers/keyboard-scroll";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

const HOST = "keyboard-scroll.example.invalid";
const HOST_ID = 9019;

test.beforeEach(async ({ request }) => {
  await resetPreferences(request);
});
test.afterEach(async ({ request }) => {
  await resetPreferences(request);
});

/** The help menu is the ordinary mounting path, including modal focus handoff. */
async function openHelpDialog(page: Page, item: "ssh needs 2FA?" | "send feedback") {
  await page.getByRole("button", { name: "help", exact: true }).click();
  const menu = page.getByRole("menu", { name: "help", exact: true });
  await expect(menu).toBeVisible();
  await menu.locator(`[data-bar-menu-item="${item}"]`).click();
  const dialog = page.getByRole("dialog", { name: item, exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

/**
 * Preserve the real local host while adding one connected display fixture.
 * No SSH connection or registry mutation is needed to render its dialogs.
 */
async function stubRemote(page: Page) {
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const listing = await response.json();
    listing.hosts = [...listing.hosts.filter((host: any) => host.kind === "local"), {
      id: HOST_ID, kind: "ssh", destination: HOST, name: HOST,
      identity: "keyboard-scroll-identity", incarnation: 1,
      remote_farhelm: null, remote_state_dir: null,
      state: { phase: "connected", identity: "keyboard-scroll-identity", build_version: "0.1.0",
        refresh: { status: "ok", sessions: 0 } },
    }];
    await route.fulfill({ response, json: listing });
  });
}

for (const state of ["loading", "error", "no hosts", "no commands"] as const) {
  /**
   * Copy buttons cannot provide access in these states. The introductory text
   * overflows a short viewport even before a command response arrives.
   */
  test(`SSH help remains keyboard-scrollable with ${state}`, async ({ page }) => {
    let release!: () => void;
    const held = new Promise<void>((resolve) => { release = resolve; });
    let requested = false;
    await page.route("**/api/ssh-connections", async (route) => {
      requested = true;
      if (state === "loading") await held;
      if (state === "error") {
        await fulfillAsHelm(route, { status: 500, contentType: "text/plain", body: "connection help unavailable" });
      } else {
        await fulfillAsHelm(route, { json: { unavailable: [], extra: null, hosts: state === "no commands" ? [{
          name: HOST, destination: HOST, sessions: null, setup: null,
        }] : [] } });
      }
    });
    try {
      await page.setViewportSize({ width: 360, height: 320 });
      await page.goto("/");
      const dialog = await openHelpDialog(page, "ssh needs 2FA?");
      const body = dialog.locator(".ssh-help-body");
      await expect.poll(() => requested, { message: "help requested its controlled response" }).toBe(true);
      if (state === "loading") await expect(body.getByRole("status")).toHaveText("loading connection commands…");
      if (state === "error") await expect(body.locator(".create-session-error")).toBeVisible();
      if (state === "no hosts") await expect(body).toContainText("No ssh hosts are registered");
      if (state === "no commands") await expect(body.locator(".ssh-help-host")).toContainText(HOST);
      await expect(body.getByRole("button")).toHaveCount(0);
      await expectKeyboardScroll(page, dialog.locator(".host-settings-dismiss"), body);
      if (state === "loading") await expect(body.getByRole("status"), "keyboard input did not release the held read")
        .toHaveText("loading connection commands…");
    } finally {
      release();
      await page.unrouteAll({ behavior: "wait" });
    }
  });
}

/**
 * The narrow command wraps on screen but retains every byte for Copy. A native
 * clipboard seam records the actual copied value without touching the host clipboard.
 */
test("long SSH commands wrap while Copy preserves their exact bytes", async ({ page }) => {
  const sessions = `ssh -o ControlPath=/tmp/${"control-path-".repeat(35)} -N 'user@host.example.invalid'`;
  const setup = `ssh -o ControlMaster=yes -o ControlPath=/tmp/${"setup-path-".repeat(35)} -N 'user@host.example.invalid'`;
  await page.route("**/api/ssh-connections", (route) => fulfillAsHelm(route, {
    json: { unavailable: [], extra: null, hosts: [{ name: HOST, destination: HOST, sessions, setup }] },
  }));
  await page.setViewportSize({ width: 360, height: 640 });
  await page.goto("/");
  await page.evaluate(() => {
    (window as any).__keyboardScrollCopies = [];
    (window as any).__farhelmNativeClipboardWrite = (value: string) => {
      (window as any).__keyboardScrollCopies.push(value);
      return Promise.resolve();
    };
  });
  const dialog = await openHelpDialog(page, "ssh needs 2FA?");
  const commands = dialog.locator(".ssh-help-command pre");
  await expect(commands).toHaveText([sessions, setup]);
  await page.evaluate(() => document.fonts.ready);
  for (const command of await commands.all()) {
    expect(await command.evaluate((node) => node.scrollWidth <= node.clientWidth + 1),
      "the full command fits the scrollport horizontally").toBe(true);
    expect(await command.evaluate((node) => {
      const range = document.createRange();
      range.selectNodeContents(node);
      return new Set(Array.from(range.getClientRects(), (rect) => Math.round(rect.top))).size;
    }), "a command longer than the card uses multiple text lines").toBeGreaterThan(1);
  }
  await dialog.getByRole("button", { name: "copy sessions connection", exact: true }).click();
  await dialog.getByRole("button", { name: "copy setup and updates connection", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__keyboardScrollCopies)).toEqual([sessions, setup]);
});

/**
 * Removal's explanation contains no controls. Its text is deliberately short,
 * so a height cap supplies overflow without replacing the shipped text or its
 * overflow and focus styles; keyboard reach must not depend on text length.
 */
test("the remove-host explanation is keyboard-scrollable", async ({ page }) => {
  await stubRemote(page);
  await page.setViewportSize({ width: 360, height: 420 });
  await page.goto("/");
  await page.addStyleTag({ content: ".host-remove-dialog > .host-settings-body { max-height: 48px; }" });
  const row = hostRowByName(page, HOST);
  await openHostMenu(row);
  await row.locator(".host-remove").click();
  const dialog = page.locator(".host-remove-dialog");
  await expect(dialog.locator(".host-remove-explanation")).toContainText("Its supervisor and sessions keep running");
  const body = dialog.locator(".host-settings-body");
  await expect(body.getByRole("button")).toHaveCount(0);
  await expectKeyboardScroll(page, dialog.locator(".host-settings-dismiss"), body);
});

/**
 * A destination save natively disables every setting, removing the body's
 * usual keyboard entry points. Hold the request and prove that unsettled
 * boundary both before and after native scrolling; do not wait for the save.
 */
test("host settings remain keyboard-scrollable while every setting is saving", async ({ page }) => {
  await stubRemote(page);
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  let submitted: unknown;
  await page.route(`**/api/hosts/${HOST_ID}/destination`, async (route) => {
    submitted = route.request().postDataJSON();
    await held;
    await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "fixture leaves the original destination in place" });
  });
  try {
    await page.setViewportSize({ width: 360, height: 420 });
    await page.goto("/");
    const row = hostRowByName(page, HOST);
    await openHostMenu(row);
    await row.locator(".host-settings").click();
    const dialog = page.locator(".host-settings-refreshed");
    const body = dialog.locator(".host-settings-body");
    await expect(body).not.toHaveAttribute("tabindex", "0");
    await dialog.locator(".host-edit").click();
    const input = dialog.locator(".host-destination-input");
    await input.fill("changed.example.invalid");
    await expect(input).toBeFocused();
    await page.keyboard.press("Enter");
    await expect.poll(() => submitted, { message: "the held destination save was submitted" })
      .toEqual({ ssh: "changed.example.invalid" });
    const controls = body.locator("button, input, select, textarea");
    expect(await controls.count(), "the settings body contains real disabled controls").toBeGreaterThan(0);
    for (const control of await controls.all()) await expect(control).toBeDisabled();
    await expectKeyboardScroll(page, dialog.locator(".host-settings-dismiss"), body);
    for (const control of await controls.all()) await expect(control).toBeDisabled();
  } finally {
    release();
    await page.unrouteAll({ behavior: "wait" });
  }
});

/**
 * Sent replaces the editors with prose for only 1.5 seconds. Freeze that timer
 * before sending so keyboard assertions observe the sent state throughout.
 * The height cap supplies overflow for the short real thank-you sentence.
 */
test("sent feedback is keyboard-scrollable without changing the editing Tab order", async ({ page }) => {
  await page.route("**/api/feedback", (route) => fulfillAsHelm(route, { status: 204 }));
  await page.clock.install();
  await page.setViewportSize({ width: 360, height: 500 });
  await page.goto("/");
  const dialog = await openHelpDialog(page, "send feedback");
  const body = dialog.locator(".host-settings-body");
  await expect(body).not.toHaveAttribute("tabindex", "0");
  await dialog.locator(".feedback-dismiss").focus();
  await page.keyboard.press("Tab");
  await expect(dialog.locator(".feedback-message")).toBeFocused();
  await dialog.locator(".feedback-message").fill("Keyboard scrolling fixture.");
  await page.clock.pauseAt(new Date(Date.now() + 5_000));
  await dialog.getByRole("button", { name: "send", exact: true }).click();
  await expect(dialog.locator(".feedback-thanks")).toContainText("Thanks. Your feedback is on its way");
  await expect(body.locator("input, textarea, button")).toHaveCount(0);
  // Apply the cap after the form has gone, so an editor's old scroll offset
  // cannot leave this new text already at its bottom before PageDown. This
  // content-box cap is smaller than one line; the body's padding stays intact.
  await body.evaluate((node) => { (node as HTMLElement).style.maxHeight = "8px"; });
  await expect(body).toHaveCSS("max-height", "8px");
  await expectKeyboardScroll(page, dialog.locator(".feedback-dismiss"), body);
  await expect(dialog.locator(".feedback-thanks"), "the close timer remains held during keyboard input").toBeVisible();
});

/**
 * A real prompt-bearing approval keeps its own bounded scrollport, separate
 * from the card's scroll area. Reverse Tab from Allow enters that prompt using
 * ordinary document order, without fabricating a focus origin or a live agent.
 */
test("an approval's complete initial prompt is keyboard-scrollable", async ({ page }) => {
  const prompt = Array.from({ length: 100 }, (_, index) => `Prompt line ${index + 1}: inspect the full request before allowing it.`).join("\n");
  await page.route(SESSION_LISTING, (route) => fulfillAsHelm(route, {
    json: { sessions: [], total: 0, matching: 0, truncated: false },
  }));
  await page.route((url) => url.pathname === "/api/approvals", (route) => fulfillAsHelm(route, {
    json: { approvals: [{
      id: "keyboard-scroll-prompt", host_id: 1, host_name: "fixture-host",
      session: { id: "7c1e9a52-4b0d-4f7e-9a3c-2d81f6b0e413", title: "requester", host_name: "fixture-host" },
      expires_at_ms: Date.now() + 540_000,
      action: {
        kind: "launch_with_prompt", verb: "create", host_name: "fixture-host", cwd: "/work/project",
        repo: null, title: null, source: null, prompt,
        launch: { kind: "agent", selection: { harness: "claude", model: "opus", effort: "high", permissions: "guarded" },
          start: ["claude", "--model", "opus", "--effort", "high", "--permission-mode", "auto", "{farhelm_args}"], resume: null },
      },
    }] },
  }));
  await page.setViewportSize({ width: 1100, height: 850 });
  await page.goto("/");
  await expect(page.locator(".main-empty")).toHaveText("no sessions — create one");
  const card = page.locator('.approval-card[data-approval-id="keyboard-scroll-prompt"]');
  const allow = card.getByRole("button", { name: "allow", exact: true });
  await expect(allow).toBeEnabled();
  const region = card.locator(".approval-card-prompt");
  await expect(region).toHaveText(prompt);
  await expectKeyboardScroll(page, allow, region, "Shift+Tab");
});
