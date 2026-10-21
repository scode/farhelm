/**
 * A scrollport with no usable child must remain a keyboard destination in
 * WebKit. HTTP fixtures select the real renderer's empty and pending states;
 * Tab and PageDown exercise native focus and scrolling rather than DOM writes.
 */
import { type ElementHandle, type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { hostRowByName, openHostMenu, pinAutoSelect, resetPreferences, SESSION_LISTING } from "./helpers/fleet";
import { expectKeyboardScroll } from "./helpers/keyboard-scroll";
import { replayRecord, waitForReplayReveal, waitForSessionMounted } from "./helpers/terminal-readiness";
import { findSessionIdByTitle, fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";
import { routeGate } from "./helpers/route-gate";
import { cleanupOwnedSteps } from "./helpers/cleanup";

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

/** Cross the documented maximum two-frame recovery after its DOM premise exists. */
async function afterFocusRecovery(page: Page): Promise<void> {
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
}

/**
 * A menu that already owns keyboard focus must keep it when the selected
 * terminal's initial replay ends. Otherwise the next menu key can reach the
 * agent, and WebKit can scroll the narrow shell and close the menu outright.
 */
test("help menu keeps keyboard focus through a late terminal reveal", async ({ page, request }) => {
  const id = await findSessionIdByTitle(request, "e2e-session");
  expect(id, "the stack supplies the selected terminal").toBeTruthy();
  await pinAutoSelect(page, id!);
  await page.addInitScript(() => {
    (window as any).__farhelmTestReplay = { holdMarker: true, idleMs: 60_000 };
  });
  await page.setViewportSize({ width: 360, height: 320 });
  await page.goto("/");
  await waitForSessionMounted(page, id!);
  await expect.poll(() => replayRecord(page, "terminal"), {
    message: "the selected terminal must hold replay while the menu opens",
  }).toMatchObject({ heldReason: "marker", revealed: false });

  await page.getByRole("button", { name: "help", exact: true }).click();
  const menu = page.getByRole("menu", { name: "help", exact: true });
  const first = menu.getByRole("menuitem").first();
  await expect(first).toBeFocused();
  await page.evaluate(() => (window as any).__farhelmIslands.terminal.test.releaseCatchUp());
  await waitForReplayReveal(page, "terminal");
  await expect(first, "terminal reveal must leave the open menu's focus intact").toBeFocused();
  await expect(menu).toBeVisible();
  await page.keyboard.press("ArrowDown");
  await expect(menu.getByRole("menuitem").nth(1)).toBeFocused();
});

/**
 * Preserve the real local host while adding one connected display fixture.
 * No SSH connection or registry mutation is needed to render its dialogs.
 * Return the owned row so a controlled successful write can update later reads.
 */
async function stubRemote(page: Page) {
  const remote = {
    id: HOST_ID, kind: "ssh", destination: HOST, name: HOST,
    identity: "keyboard-scroll-identity", incarnation: 1, yolo_without_asking: false, commands_without_asking: false,
    remote_farhelm: null, remote_state_dir: null,
    state: { phase: "connected", identity: "keyboard-scroll-identity", build_version: "0.1.0",
      refresh: { status: "ok", sessions: 0 } },
  };
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const listing = await response.json();
    listing.hosts = [...listing.hosts.filter((host: any) => host.kind === "local"), remote];
    await route.fulfill({ response, json: listing });
  });
  return remote;
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

for (const outcome of ["refused destination", "successful checkbox"] as const) {
  /**
   * Saving temporarily makes the body a tab stop. Once it loses that role,
   * both a refused field write and a successful switch write must recover a
   * real control before the next key, rather than letting isolation consume it.
   */
  test(`host settings keep usable focus after a ${outcome} save`, async ({ page }) => {
    const remote = await stubRemote(page);
    const destination = outcome === "refused destination";
    let release!: () => void;
    const held = new Promise<void>((resolve) => { release = resolve; });
    let submitted: unknown;
    await page.route(`**/api/hosts/${HOST_ID}/${destination ? "destination" : "yolo-without-asking"}`, async (route) => {
      submitted = route.request().postDataJSON();
      await held;
      if (destination) {
        await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "fixture refuses the destination" });
      } else {
        remote.yolo_without_asking = true;
        await fulfillAsHelm(route, { json: remote });
      }
    });
    try {
      await page.goto("/");
      const row = hostRowByName(page, HOST);
      await openHostMenu(row);
      await row.locator(".host-settings").click();
      const dialog = page.locator(".host-settings-refreshed");
      const body = dialog.locator(".host-settings-body");
      const toggle = dialog.locator(".host-yolo-without-asking-toggle");
      const input = dialog.locator(".host-destination-input");
      if (destination) {
        await dialog.locator(".host-edit").click();
        await input.fill("changed.example.invalid");
        await expect(input).toBeFocused();
        await page.keyboard.press("Enter");
      } else {
        await expect(toggle).not.toBeChecked();
        await toggle.focus();
        await expect(toggle).toBeFocused();
        await page.keyboard.press("Space");
      }
      await expect.poll(() => submitted, { message: "the selected save is held before completing" })
        .toEqual(destination ? { ssh: "changed.example.invalid" } : { yolo_without_asking: true });
      await expect(toggle).toBeDisabled();
      await expect(body).toHaveAttribute("tabindex", "0");
      await dialog.locator(".host-settings-dismiss").focus();
      await expect(dialog.locator(".host-settings-dismiss")).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(body).toBeFocused();
      release();
      await expect(body).not.toHaveAttribute("tabindex", "0");
      if (destination) {
        await expect(input).toBeEnabled();
        await expect(input).toBeFocused();
        await page.keyboard.type("x");
        await expect(input).toHaveValue("changed.example.invalidx");
      } else {
        await expect(toggle).toBeEnabled();
        await expect(toggle).toBeChecked();
        await expect(toggle).toBeFocused();
        await page.keyboard.press("Tab");
        await expect(dialog.locator(".host-commands-without-asking-toggle")).toBeFocused();
      }
    } finally {
      release();
      await page.unrouteAll({ behavior: "wait" });
    }
  });
}

/**
 * Automatic busy-region focus must retain the control that started the save.
 * Commands is deliberately different from the YOLO fallback: recovering that
 * fallback would conceal lost focus memory. No input moves focus while held.
 */
test("host settings automatic busy recovery remembers the commands checkbox", async ({ page }) => {
  const remote = await stubRemote(page);
  const gate = routeGate();
  let submitted: unknown;
  await page.route(`**/api/hosts/${HOST_ID}/commands-without-asking`, async (route) => {
    submitted = route.request().postDataJSON();
    await gate.wait();
    remote.commands_without_asking = true;
    await fulfillAsHelm(route, { json: remote });
  });
  try {
    await page.goto("/");
    const row = hostRowByName(page, HOST);
    await openHostMenu(row);
    await row.locator(".host-settings").click();
    const dialog = page.locator(".host-settings-refreshed");
    const body = dialog.locator(".host-settings-body");
    const commands = dialog.locator(".host-commands-without-asking-toggle");
    await expect(commands).not.toBeChecked();
    await commands.focus();
    await expect(commands).toBeFocused();
    await page.keyboard.press("Space");
    await expect.poll(() => submitted, { message: "the commands save remains held" })
      .toEqual({ commands_without_asking: true });
    await expect(commands).toBeDisabled();
    await expect(body).toHaveAttribute("tabindex", "0");
    // This focus must arrive from automatic recovery. Navigating to the
    // region here would replace the very memory this regression protects.
    await expect(body).toBeFocused();
    gate.release();
    await expect(body).not.toHaveAttribute("tabindex");
    await expect(commands).toBeEnabled();
    await expect(commands).toBeChecked();
    await expect(commands).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.locator(".host-links-without-asking-toggle")).toBeFocused();
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "wait" });
  }
});

for (const focus of ["body", "dismiss", "backdrop"] as const) {
  /**
   * Command arrival removes the loading body's tab stop. Recover Close when
   * that body held focus or a real scrim click left no control in charge, but
   * preserve a usable control the user already chose. Fresh Tab or Enter must
   * perform its normal action, rather than being consumed to repair focus.
   */
  test(`SSH command arrival keeps usable focus from ${focus}`, async ({ page }) => {
    let release!: () => void;
    const held = new Promise<void>((resolve) => { release = resolve; });
    let requested = false;
    await page.route("**/api/ssh-connections", async (route) => {
      requested = true;
      await held;
      await fulfillAsHelm(route, { json: { unavailable: [], extra: null, hosts: [{
        name: HOST, destination: HOST, sessions: "ssh sessions.example.invalid", setup: "ssh setup.example.invalid",
      }] } });
    });
    try {
      await page.goto("/");
      const dialog = await openHelpDialog(page, "ssh needs 2FA?");
      const body = dialog.locator(".ssh-help-body");
      const dismiss = dialog.locator(".host-settings-dismiss");
      await expect.poll(() => requested).toBe(true);
      await expect(body.getByRole("status")).toHaveText("loading connection commands…");
      await expect(body).toHaveAttribute("tabindex", "0");
      await dismiss.focus();
      await expect(dismiss).toBeFocused();
      if (focus === "body") {
        await page.keyboard.press("Tab");
        await expect(body).toBeFocused();
      } else if (focus === "backdrop") {
        const point = { x: 5, y: 5 };
        expect(await page.evaluate(({ x, y }) =>
          document.elementFromPoint(x, y)?.matches(".host-settings-backdrop") ?? false, point),
          "the actual click target is the dialog's backdrop").toBe(true);
        await page.mouse.click(point.x, point.y);
        await expect(dialog).toBeVisible();
        expect(await page.evaluate(() => document.activeElement === document.body),
          "the native backdrop click left no chosen control").toBe(true);
      }
      release();
      await expect(dialog.getByRole("button", { name: "copy setup and updates connection", exact: true })).toBeVisible();
      await expect(body).not.toHaveAttribute("tabindex", "0");
      if (focus === "backdrop") {
        await expect(dialog.locator(".ssh-help-close")).toBeFocused();
        await page.keyboard.press("Enter");
        await expect(dialog).toHaveCount(0);
        await expect(page.getByRole("button", { name: "help", exact: true })).toBeFocused();
      } else {
        await expect(focus === "body" ? dialog.locator(".ssh-help-close") : dismiss).toBeFocused();
        await page.keyboard.press(focus === "body" ? "Shift+Tab" : "Tab");
        await expect(dialog.getByRole("button", {
          name: focus === "body" ? "copy setup and updates connection" : "copy sessions connection", exact: true,
        })).toBeFocused();
      }
    } finally {
      release();
      await page.unrouteAll({ behavior: "wait" });
    }
  });
}

/**
 * Sending retains disabled editors but no usable body control. The temporary
 * stop must admit native scrolling, then disappear when a refusal restores
 * the unchanged editable message and its ordinary field-to-field Tab order.
 */
test("sending feedback gains a body stop and refusal restores editor entry", async ({ page }) => {
  const gate = routeGate();
  const message = "Keep feedback reachable while the send remains pending.";
  const submissions: unknown[] = [];
  await page.route("**/api/feedback", async (route) => {
    submissions.push(route.request().postDataJSON());
    await gate.wait();
    await fulfillAsHelm(route, { status: 503, contentType: "text/plain", body: "Couldn't send feedback: held feedback refusal" });
  });
  try {
    await page.setViewportSize({ width: 360, height: 320 });
    await page.goto("/");
    const dialog = await openHelpDialog(page, "send feedback");
    const body = dialog.locator(".host-settings-body");
    const field = dialog.locator(".feedback-message");
    await expect(body).not.toHaveAttribute("tabindex");
    await field.fill(message);
    await dialog.locator(".feedback-send").click();
    await expect.poll(() => submissions.length).toBe(1);
    expect(submissions[0]).toMatchObject({ message });
    await expect(dialog.locator(".feedback-send")).toHaveText("sending…");
    const controls = dialog.locator("button, input, textarea");
    expect(await controls.count()).toBeGreaterThan(0);
    for (const control of await controls.all()) await expect(control).toBeDisabled();
    await expect(body).toHaveAccessibleName("feedback");
    // Header and footer answers are disabled too. The real focusable dialog
    // supplies the origin; Tab must enter the body rather than repair body focus.
    await expectKeyboardScroll(page, dialog, body);
    for (const control of await controls.all()) await expect(control).toBeDisabled();
    expect(submissions).toHaveLength(1);
    gate.release();
    await expect(dialog.locator(".feedback-error")).toContainText("held feedback refusal");
    await expect(field).toBeEnabled();
    await expect(field).toHaveValue(message);
    await expect(body).not.toHaveAttribute("tabindex");
    await expect(body).not.toHaveAttribute("role");
    await expect(body).not.toHaveAttribute("aria-label");
    await expect(field).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.locator(".feedback-contact")).toBeFocused();
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "wait" });
  }
});

/**
 * Command arrival may remove a focused body's stop, but it must preserve an
 * independently usable approval answer chosen while the HTTP read is held.
 * A post-recovery assertion prevents an early transient focus pass.
 */
test("SSH command arrival preserves an independent approval owner", async ({ page }) => {
  const gate = routeGate();
  const approvalId = "ssh-help-scroll-owner";
  let requested = false;
  const answers: string[] = [];
  await page.route(SESSION_LISTING, (route) => fulfillAsHelm(route, {
    json: { sessions: [], total: 0, matching: 0, truncated: false },
  }));
  await page.route((url) => url.pathname === "/api/approvals", (route) => fulfillAsHelm(route, {
    json: { approvals: [{
      id: approvalId, host_id: 1, host_name: "fixture-host",
      session: { id: "7c1e9a52-4b0d-4f7e-9a3c-2d81f6b0e413", title: "requester", host_name: "fixture-host" },
      expires_at_ms: Date.now() + 540_000,
      action: {
        kind: "launch", verb: "create", host_name: "fixture-host", cwd: "/work/project", title: null, source: null,
        launch: { kind: "agent", selection: { harness: "claude", model: "opus", effort: "high", permissions: "guarded" },
          start: ["claude", "--model", "opus", "--effort", "high", "--permission-mode", "auto", "{farhelm_args}"], resume: null },
      },
    }] },
  }));
  await page.route((url) => url.pathname === `/api/approvals/${approvalId}`, async (route) => {
    answers.push(route.request().postDataJSON().answer);
    await fulfillAsHelm(route, { status: 204 });
  });
  await page.route("**/api/ssh-connections", async (route) => {
    requested = true;
    await gate.wait();
    await fulfillAsHelm(route, { json: { unavailable: [], extra: null, hosts: [{
      name: HOST, destination: HOST, sessions: "ssh sessions.example.invalid", setup: "ssh setup.example.invalid",
    }] } });
  });
  try {
    await page.goto("/");
    const dialog = await openHelpDialog(page, "ssh needs 2FA?");
    const body = dialog.locator(".ssh-help-body");
    await expect.poll(() => requested).toBe(true);
    await expect(body.getByRole("status")).toHaveText("loading connection commands…");
    await expect(body).toHaveAttribute("tabindex", "0");
    const answer = page.locator(`.approval-card[data-approval-id="${approvalId}"]`).getByRole("button", { name: "allow", exact: true });
    await expect(answer).toBeVisible();
    await expect(answer).toBeEnabled();
    expect(await answer.evaluate((node) => node.closest("[data-modal-exempt]") !== null)).toBe(true);
    expect(await answer.evaluate((node) => node.closest("[inert]") !== null)).toBe(false);
    await answer.focus();
    await expect(answer).toBeFocused();
    gate.release();
    await expect(dialog.getByRole("button", { name: "copy setup and updates connection", exact: true })).toBeVisible();
    await expect(body).not.toHaveAttribute("tabindex");
    await expect(body).not.toHaveAttribute("role");
    await expect(body).not.toHaveAttribute("aria-label");
    await afterFocusRecovery(page);
    await expect(answer, "late help recovery leaves the chosen approval answer alone").toBeFocused();
    expect(answers, "preserving an approval owner does not answer its request").toEqual([]);
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "wait" });
  }
});

/**
 * A closed region cannot react to its old reply or disturb a fresh opening.
 * Two independently held reads establish the stale/current boundary, while
 * native Tab after reopen proves the new stop and recovery remain usable.
 */
test("SSH help closed before its reply leaves a reopened region independent", async ({ page }) => {
  const gates = [routeGate(), routeGate()];
  const drained: Array<"delivered" | "cancelled" | undefined> = [];
  let requested = 0;
  let oldRoot: ElementHandle<HTMLElement | SVGElement> | null = null;
  let bodyFailed = true;
  await page.route("**/api/ssh-connections", async (route) => {
    const opening = requested++;
    expect(opening, "only the old and reopened help reads are expected").toBeLessThan(2);
    await gates[opening].wait();
    const failure = route.request().failure();
    if (failure) {
      expect(failure.errorText).toMatch(/abort|cancel/i);
      // A cancelled browser fetch still owns a Playwright route invocation.
      // Returning without an action leaves its handled promise pending, so
      // unrouteAll(wait) cannot finish after the test's body has passed.
      await route.abort("aborted");
      drained[opening] = "cancelled";
      return;
    }
    try {
      await fulfillAsHelm(route, { json: { unavailable: [], extra: null, hosts: [{
        name: HOST, destination: HOST, sessions: `ssh opening-${opening}.example.invalid`, setup: "ssh setup.example.invalid",
      }] } });
      drained[opening] = "delivered";
    } catch (error) {
      // Closing a scope can abort its fetch while the route is being drained.
      // Only that observed cancellation is an expected alternative to delivery.
      if (!/abort|cancel/i.test(route.request().failure()?.errorText ?? "")) throw error;
      drained[opening] = "cancelled";
    }
  });
  try {
    await page.goto("/");
    let dialog = await openHelpDialog(page, "ssh needs 2FA?");
    await expect.poll(() => requested).toBe(1);
    let body = dialog.locator(".ssh-help-body");
    await expect(body).toHaveAttribute("tabindex", "0");
    await dialog.locator(".host-settings-dismiss").focus();
    await expect(dialog.locator(".host-settings-dismiss")).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(body).toBeFocused();
    oldRoot = await dialog.elementHandle();
    expect(oldRoot).not.toBeNull();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    expect(await oldRoot!.evaluate((node) => node.isConnected)).toBe(false);
    await expect(page.getByRole("button", { name: "help", exact: true })).toBeFocused();
    dialog = await openHelpDialog(page, "ssh needs 2FA?");
    await expect.poll(() => requested).toBe(2);
    body = dialog.locator(".ssh-help-body");
    const dismiss = dialog.locator(".host-settings-dismiss");
    await expect(body.getByRole("status")).toHaveText("loading connection commands…");
    await expect(body).toHaveAttribute("tabindex", "0");
    await dismiss.focus();
    await expect(dismiss).toBeFocused();
    gates[0].release();
    await expect.poll(() => drained[0], { message: "the old request is delivered or observably cancelled" }).toBeDefined();
    await afterFocusRecovery(page);
    expect(await oldRoot!.evaluate((node) => node.isConnected)).toBe(false);
    await expect(body.getByRole("status")).toHaveText("loading connection commands…");
    await expect(body).toHaveAttribute("tabindex", "0");
    await expect(body.getByRole("button")).toHaveCount(0);
    await expect(dismiss).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(body).toBeFocused();
    gates[1].release();
    await expect(dialog.locator(".ssh-help-command pre").first()).toHaveText("ssh opening-1.example.invalid");
    await expect(body).not.toHaveAttribute("tabindex");
    await expect(dialog.locator(".ssh-help-close")).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog.getByRole("button", { name: "copy setup and updates connection", exact: true })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(page.getByRole("button", { name: "help", exact: true })).toBeFocused();
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["held help reads", () => gates.forEach((gate) => gate.release())],
      ["help routes", () => page.unrouteAll({ behavior: "wait" })],
      ["old root handle", () => oldRoot?.dispose()],
    ]);
  }
});

/**
 * Sent replaces the editors with prose for only 1.5 seconds. Hold that timer
 * before sending, but advance the observer's reconciliation frames explicitly
 * so the fixture does not freeze the keyboard entry it intends to measure.
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
  // pauseAt also holds requestAnimationFrame. Fifty milliseconds lets the
  // queued content/resize reconciliation run, well before the 1500 ms close.
  await page.clock.runFor(50);
  await expect(body).toHaveAccessibleName("feedback sent");
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
