/**
 * The shortcut overview must describe this client's actual listeners, take its
 * opening chord before xterm, and return keyboard ownership when it closes.
 * Real owned template bindings distinguish a fresh overview from startup hints.
 */
import type { APIRequestContext, Locator, Page } from "@playwright/test";
import { expect, newObservedContext, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import {
  cleanupSession,
  createSession,
  FAKE_AGENT,
  observeFeedReaders,
  patchPreferences,
  pinAutoSelect,
  readPreferences,
  resetPreferences,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { routeGate } from "./helpers/route-gate";
import { attachSession, waitForTermText } from "./helpers/term";

/** Classify both the listener and the overview before either mounts. */
async function platform(page: Page, mac: boolean): Promise<void> {
  await page.addInitScript((value) => {
    Object.defineProperty(Navigator.prototype, "platform", { get: () => value });
  }, mac ? "MacIntel" : "Linux x86_64");
}

/** The authenticated relay is the admission boundary, not merely a painted sidebar. */
async function openByChord(page: Page, mac = false): Promise<Locator> {
  await expect(page.locator(".keyboard-shortcuts-trigger")).toHaveCount(1);
  await page.keyboard.press(mac ? "Meta+Slash" : "Control+Shift+Slash");
  const dialog = page.getByRole("dialog", { name: "Keyboard shortcuts", exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** Exercise the real Help item so its focus return target is captured normally. */
async function openByMenu(page: Page): Promise<Locator> {
  const help = page.getByRole("button", { name: "Help", exact: true });
  await help.click();
  const menu = page.getByRole("menu", { name: "Help", exact: true });
  await expect(menu).toBeVisible();
  await menu.locator('[data-bar-menu-item="Keyboard shortcuts"]').click();
  await expect(menu).toHaveCount(0);
  const dialog = page.getByRole("dialog", { name: "Keyboard shortcuts", exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** Keep label assertions tied to the action instead of relying on row order. */
function row(dialog: Locator, action: string): Locator {
  return dialog.locator(".keyboard-shortcuts-row").filter({ hasText: action });
}

/** Register genuine templates: preference names should refer to saved catalog entries. */
async function saveTemplate(request: APIRequestContext, name: string): Promise<void> {
  const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, {
    data: { kind: "command", command: FAKE_AGENT, yolo: false },
  });
  expect(saved.ok(), await saved.text()).toBe(true);
  const listing = await request.get("/api/templates");
  expect(listing.ok(), await listing.text()).toBe(true);
  expect((await listing.json()).templates.map((entry: { name: string }) => entry.name)).toContain(name);
}

test.beforeEach(async ({ request }) => { await resetPreferences(request); });
test.afterEach(async ({ request }) => { await resetPreferences(request); });

for (const mac of [false, true]) {
  /**
   * Binary frames expose a chord leaked to the program even if the dialog also
   * opens. The subsequent owned echo proves restored focus and a live peer.
   * Mac browsers deliberately combine OS-following and web-build labels.
   */
  test(`the ${mac ? "Mac" : "non-Mac"} overview chord owns terminal input and labels`, async ({ page, request }, testInfo) => {
    await platform(page, mac);
    const session = await createSession(request, { title: `shortcuts-input-${Date.now()}` });
    const binary: Buffer[] = [];
    page.on("websocket", (socket) => {
      if (!new URL(socket.url()).pathname.endsWith("/term")) return;
      socket.on("framesent", ({ payload }) => { if (Buffer.isBuffer(payload)) binary.push(payload); });
    });
    try {
      await pinAutoSelect(page, session.id);
      await page.goto("/");
      await attachSession(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      const terminal = page.locator("#terminal .xterm-helper-textarea");
      await expect(terminal).toBeFocused();
      const dialog = await openByChord(page, mac);
      await expect(dialog).toHaveAttribute("aria-modal", "true");
      for (const group of ["Sessions", "Launching", "Terminal"]) await expect(dialog.getByText(group, { exact: true })).toBeVisible();
      await expect(row(dialog, "Quick switcher").locator("kbd")).toHaveText(mac ? "⌘K" : "Ctrl+Shift+K");
      await expect(row(dialog, "Templates").locator("kbd")).toHaveText(mac ? "⌘J" : "Ctrl+Shift+J");
      await expect(row(dialog, "Quick launch:").locator("kbd")).toHaveText(mac ? "⌘P" : "Ctrl+Shift+P");
      // Empty bindings must still teach the direct shortcut that opens Templates.
      await expect(row(dialog, "Launch a numbered template").locator("kbd")).toHaveText("Ctrl+Shift+0–9");
      await expect(row(dialog, "Delete").locator("kbd")).toHaveText("Ctrl+Shift+Backspace");
      await expect(row(dialog, "Rename").locator("kbd")).toHaveText("Ctrl+Shift+L");
      await expect(row(dialog, "Begin launch sequence").locator("kbd")).toHaveText("Ctrl+Shift+X");
      await expect(row(dialog, "Smaller terminal text").locator("kbd")).toHaveText(mac ? "⌘+Shift+−" : "Ctrl+Shift+−");
      await expect(row(dialog, "Larger terminal text").locator("kbd")).toHaveText(mac ? "⌘+Shift++" : "Ctrl+Shift++");
      await expect(row(dialog, "In a sequence: add a numbered template").locator("kbd")).toHaveText("1–9, 0");
      await expect(row(dialog, "In a sequence: add a numbered template")).toContainText("no word in progress");
      await expect(row(dialog, "In a sequence: type a template name").locator("kbd")).toHaveText("Characters (digits continue a word)");
      await expect(row(dialog, "In a sequence: queue the word").locator("kbd")).toHaveText("Space");
      await expect(row(dialog, "In a sequence: complete a unique name").locator("kbd")).toHaveText("Tab");
      await expect(row(dialog, "In a sequence: edit the word").locator("kbd")).toHaveText("Backspace");
      await expect(row(dialog, "In a sequence: add the ⚡ template").locator("kbd")).toHaveText(mac ? "⌘P" : "Ctrl+Shift+P");
      await expect(row(dialog, "In a sequence: launch").locator("kbd")).toHaveText("Enter");
      await expect(row(dialog, "In a sequence: cancel").locator("kbd")).toHaveText("Escape");
      await expect(row(dialog, "New session")).toHaveCount(0);
      await expect(row(dialog, "Keyboard shortcuts").locator("kbd")).toHaveText(mac ? "⌘/" : "Ctrl+Shift+/");
      await expect(dialog.getByRole("button", { name: "Close keyboard shortcuts", exact: true })).toBeFocused();
      await dialog.screenshot({ path: testInfo.outputPath("keyboard-shortcuts.png") });
      await page.keyboard.press("Escape");
      await expect(dialog).toHaveCount(0);
      await expect(terminal).toBeFocused();
      const probe = `overview-restored-${Date.now()}`;
      await page.keyboard.type(probe);
      await page.keyboard.press("Enter");
      await waitForTermText(page, `echo:${probe}`);
      expect(Buffer.concat(binary).toString(), "only the deliberate probe reaches the program").toBe(`${probe}\r`);

      await page.locator(".new-session-button").click();
      const other = page.locator('.create-session-form[aria-modal="true"]');
      await expect(other).toBeVisible();
      await other.getByLabel("Name (optional)").focus();
      await expect(other.getByLabel("Name (optional)")).toBeFocused();
      await page.keyboard.press(mac ? "Meta+Slash" : "Control+Shift+Slash");
      await expect(dialog).toHaveCount(0);
      await expect(other).toBeVisible();
      await expect(other.getByLabel("Name (optional)")).toBeFocused();
    } finally {
      await cleanupSession(request, session.id);
    }
  });
}

/**
 * Every ordinary dismissal must release the same modal boundary. Opening from
 * Help makes its toggle, rather than a terminal, the required return target.
 */
test("the Help item closes with Close, Escape or outside and restores its toggle", async ({ page, request }) => {
  const session = await createSession(request, { title: `shortcuts-help-${Date.now()}` });
  try {
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    // Establish this attachment's focus before a menu can become its opener.
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    for (const close of ["button", "escape", "outside"]) {
      const dialog = await openByMenu(page);
      const button = dialog.getByRole("button", { name: "Close keyboard shortcuts", exact: true });
      await expect(button).toBeFocused();
      if (close === "button") {
        // A text-only body needs keyboard entry for scrolling, inside the
        // same Tab boundary as Close rather than escaping to the sidebar.
        const body = dialog.locator(".keyboard-shortcuts-body");
        await expect(body).toHaveAttribute("tabindex", "0");
        await page.keyboard.press("Tab");
        await expect(body).toBeFocused();
        await page.keyboard.press("Tab");
        await expect(button).toBeFocused();
      }
      if (close === "button") await button.click();
      else if (close === "escape") await page.keyboard.press("Escape");
      else await page.mouse.click(4, 4);
      await expect(dialog).toHaveCount(0);
      await expect(page.getByRole("button", { name: "Help", exact: true })).toBeFocused();
    }
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * Synthetic events let the test name repeat and composition precisely. The
 * admitted control event proves the capture listener was actually installed.
 * Near chords stay unconsumed; an opening chord's repeat is swallowed without
 * opening. Neither repeat nor composition may mount a dialog.
 */
for (const mac of [false, true]) {
  test(`the ${mac ? "Mac" : "non-Mac"} overview rejects near chords, repeat and composition`, async ({ page }) => {
    await platform(page, mac);
    await page.goto("/");
    await expect(page.locator(".keyboard-shortcuts-trigger")).toHaveCount(1);
    const accepted = mac ? { metaKey: true } : { ctrlKey: true, shiftKey: true };
    const rejected = [
      {},
      { ctrlKey: true },
      mac ? { ctrlKey: true, shiftKey: true } : { metaKey: true },
      { ...accepted, altKey: true },
      { ...accepted, [mac ? "ctrlKey" : "metaKey"]: true },
      ...(mac ? [{ ...accepted, shiftKey: true }] : []),
      { ...accepted, repeat: true },
      { ...accepted, isComposing: true },
      { ...accepted, code: "Backslash" },
    ];
    for (const modifiers of rejected) {
      const consumed = await page.evaluate((options) => {
        const event = new KeyboardEvent("keydown", { key: "/", code: "Slash", bubbles: true, cancelable: true, ...options });
        document.body.dispatchEvent(event);
        return event.defaultPrevented;
      }, modifiers);
      expect(consumed, JSON.stringify(modifiers)).toBe("repeat" in modifiers);
      await expect(page.locator(".keyboard-shortcuts-dialog")).toHaveCount(0);
    }
    const consumed = await page.evaluate((modifiers) => {
      // A non-US layout can report another key value for the physical Slash.
      const event = new KeyboardEvent("keydown", { key: "?", code: "Slash", bubbles: true, cancelable: true, ...modifiers });
      document.body.dispatchEvent(event);
      return event.defaultPrevented;
    }, accepted);
    expect(consumed).toBe(true);
    await expect(page.getByRole("dialog", { name: "Keyboard shortcuts", exact: true })).toBeVisible();
  });
}

/**
 * Authentication and the initial preferences read are separate admission
 * boundaries. Hold the latter explicitly so a fast local helm cannot hide it.
 */
test("the chord is unavailable before sign-in or initial preferences", async ({ page, browser, timeline, baseURL }) => {
  const context = await newObservedContext(browser, timeline, { storageState: { cookies: [], origins: [] }, extraHTTPHeaders: {} });
  const gate = routeGate();
  let held = 0;
  try {
    const signedOut = await context.newPage();
    await platform(signedOut, false);
    await signedOut.goto(baseURL!);
    await expect(signedOut.locator(".auth-page")).toBeVisible();
    await signedOut.keyboard.press("Control+Shift+Slash");
    await expect(signedOut.locator(".keyboard-shortcuts-trigger")).toHaveCount(0);
    await expect(signedOut.locator(".keyboard-shortcuts-dialog")).toHaveCount(0);
    await platform(page, false);
    await page.route("**/api/preferences", async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      held++;
      await gate.wait();
      await route.continue();
    });
    await page.goto("/");
    await expect.poll(() => held, { message: "initial preferences are held before shortcut admission" }).toBeGreaterThan(0);
    await expect(page.locator(".auth-page")).toHaveCount(0);
    await page.keyboard.press("Control+Shift+Slash");
    await expect(page.locator(".keyboard-shortcuts-trigger")).toHaveCount(0);
    await expect(page.locator(".keyboard-shortcuts-dialog")).toHaveCount(0);
    gate.release();
    const dialog = await openByChord(page);
    await expect(dialog).toBeVisible();
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "wait" });
    await context.close();
  }
});

/**
 * Bindings changed in another client must replace startup names once per open.
 * A failed fresh read must retain the last successful names, rather than claim
 * nothing is assigned. Clearing bindings must remove individual number rows.
 */
test("each opening refreshes template bindings and a failed read keeps cached names", async ({ page, request }) => {
  const names = ["old", "fresh"].map((suffix) => `shortcut-overview-${Date.now()}-${suffix}`);
  let bodyFailed = true;
  let reads = 0;
  let fail = false;
  try {
    for (const name of names) await saveTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: names[0], template_shortcuts: { "1": names[0] } });
    expect(await readPreferences(request)).toMatchObject({ quick_launch_template: names[0], template_shortcuts: { "1": names[0] } });
    await observeFeedReaders(page);
    await platform(page, true);
    await page.goto("/");
    await expect(page.locator(".templates-button")).toHaveAttribute("data-tooltip", new RegExp(names[0]));
    await waitForFeedReadersSettled(page);
    await patchPreferences(request, { quick_launch_template: names[1], template_shortcuts: { "1": names[1] } });
    expect(await readPreferences(request)).toMatchObject({ quick_launch_template: names[1], template_shortcuts: { "1": names[1] } });
    const live = await request.get("/api/preferences");
    expect(live.ok(), await live.text()).toBe(true);
    const stamp = requireHelmBuild(live, "shortcut overview failed read");
    await page.route("**/api/preferences", async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      reads++;
      if (fail) await route.fulfill({ status: 503, headers: { "x-farhelm-build": stamp }, body: "overview preference read unavailable" });
      else await route.continue();
    });
    await expect(page.locator(".templates-button")).toHaveAttribute("data-tooltip", new RegExp(names[0]));
    let dialog = await openByChord(page, true);
    await expect(row(dialog, `Quick launch: ⚡ ${names[1]}`)).toBeVisible();
    await expect(row(dialog, `1 · ${names[1]}`).locator("kbd")).toHaveText("Ctrl+Shift+1");
    await expect(dialog).not.toContainText(names[0]);
    expect(reads, "one overview refresh, after startup readers settled").toBe(1);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    fail = true;
    const failedRead = page.waitForResponse((response) => new URL(response.url()).pathname === "/api/preferences" && response.request().method() === "GET");
    dialog = await openByChord(page, true);
    expect((await failedRead).status()).toBe(503);
    await expect.poll(() => reads).toBe(2);
    await expect(row(dialog, `Quick launch: ⚡ ${names[1]}`)).toBeVisible();
    await expect(row(dialog, `1 · ${names[1]}`)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    fail = false;
    await patchPreferences(request, { quick_launch_template: null, template_shortcuts: {} });
    expect((await readPreferences(request)).quick_launch_template).toBeUndefined();
    expect((await readPreferences(request)).template_shortcuts ?? {}).toEqual({});
    dialog = await openByChord(page, true);
    await expect(row(dialog, "Quick launch:")).toContainText(/none.*marked/i);
    await expect(dialog).not.toContainText(names[1]);
    await expect(dialog.locator(".keyboard-shortcuts-row").filter({ hasText: /^1 ·/ })).toHaveCount(0);
    expect(reads).toBe(3);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["page writers", () => page.goto("about:blank")],
      ["preference route", () => page.unrouteAll({ behavior: "wait" })],
      ["template bindings", () => patchPreferences(request, { quick_launch_template: null, template_shortcuts: {} })],
      ...names.map((name): [string, () => Promise<void>] => [name, async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }]),
    ]);
  }
});
