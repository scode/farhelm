import { expect, test } from "./helpers/evidence";
import type { Locator, Page } from "@playwright/test";
import { cleanupSession, createSession, openRowMenu } from "./helpers/fleet";

/** Pin the agreed conservative fallback at overlapping-key boundaries.
 * A held earlier WebKit typing key and Chromium's already handled ending key
 * both leave the next key ordinary. That deliberately misses WebKit rollover
 * so Chromium's separate real key survives before the ending key is released.
 * Synthetic replay documents this limitation, not native Mac timing.
 */
for (const order of ["webkit", "chromium"]) {
  for (const keyCode of [27, 229]) {
    test(`overlap boundary leaves the next key ordinary (${order}, ${keyCode})`, async ({ page }) => {
      const { panel, field } = await openTemplateSave(page);
      const observed = await field.evaluate((node, { order, keyCode }) => {
        const clock = Object.getOwnPropertyDescriptor(performance, "now");
        const realNow = performance.now.bind(performance);
        // Equal elapsed input makes the threshold conflict decisive. Restore
        // the browser clock before yielding; no native timing claim follows.
        Object.defineProperty(performance, "now", { value: () => 100, configurable: true });
        try {
          const key = keyCode === 229 ? "Enter" : "Escape";
          const prior = key === "Enter" ? "Escape" : "Enter";
          node.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
          node.dispatchEvent(new KeyboardEvent("keydown", {
            key: order === "chromium" ? prior : "Process", code: order === "chromium" ? prior : "KeyA",
            keyCode: 229, isComposing: true, bubbles: true, cancelable: true,
          }));
          node.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true }));
          const endedAt = realNow();
          const event = new KeyboardEvent("keydown", {
            key, code: key, keyCode, isComposing: false, bubbles: true, cancelable: true,
          });
          node.dispatchEvent(event);
          return { composing: event.isComposing, gap: realNow() - endedAt };
        } finally {
          if (clock) Object.defineProperty(performance, "now", clock);
          else delete (performance as unknown as Record<string, unknown>).now;
        }
      }, { order, keyCode });
      test.info().annotations.push({ type: "overlap-gap-ms", description: String(observed.gap) });
      expect(observed.composing).toBe(false);
      if (keyCode === 27) await expect(panel).toHaveCount(0);
    });
  }
}

/** Open an inline editor without creating or launching any session. */
async function openTemplateSave(page: Page) {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await form.locator(".launch-composer-save-template").click();
  const panel = form.locator(".save-template-panel");
  await expect(panel.locator(".save-template-name")).toBeFocused();
  return { form, panel, field: panel.locator(".save-template-name") };
}

/** Replay an engine's ordering, not a real input method or the Mac app.
 * A released earlier composing key establishes the WebKit boundary; Chromium's
 * ending key is still held when compositionend arrives. Return the event's
 * observed flag and timing so a product assertion cannot hide missing marking.
 */
async function endComposition(field: Locator, key: string, keyCode: number, order = "webkit", chord = "none") {
  await expect(field).toBeFocused();
  const observed = await field.evaluate((node, { key, keyCode, order, chord }) => {
    const down = (key: string, isComposing: boolean, keyCode: number) => new KeyboardEvent("keydown", {
      key, code: key.length === 1 ? `Key${key.toUpperCase()}` : key, keyCode, isComposing,
      metaKey: chord === "mac", ctrlKey: chord === "other", shiftKey: chord === "other",
      bubbles: true, cancelable: true,
    });
    node.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
    node.dispatchEvent(down("Process", true, 229));
    node.dispatchEvent(new KeyboardEvent("keyup", { key: "Process", code: "Process", bubbles: true }));
    node.dispatchEvent(new InputEvent("input", { inputType: "insertCompositionText", isComposing: true, bubbles: true }));
    const event = down(key, order === "chromium", keyCode);
    const before = event.isComposing;
    if (order === "chromium") node.dispatchEvent(event);
    node.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true }));
    const endedAt = performance.now();
    if (order === "webkit") node.dispatchEvent(event);
    const gap = performance.now() - endedAt;
    node.dispatchEvent(new KeyboardEvent("keyup", {
      key, code: key.length === 1 ? `Key${key.toUpperCase()}` : key, bubbles: true,
    }));
    return { before, after: event.isComposing, gap };
  }, { key, keyCode, order, chord });
  expect(observed.before).toBe(order === "chromium");
  expect(observed.after, "the real event must reach existing guards as composing").toBe(true);
  test.info().annotations.push({ type: "replayed-composition-key-gap-ms", description: String(observed.gap) });
}

/** Prove event marking reaches a real Rust guard before relying on it globally.
 * The unmarked control must close the same inline panel. This tests event
 * transport in two browser engines, not a real input method or the Mac app.
 */
test("capture-phase composition marking reaches a Dioxus guard", async ({ page }) => {
  await page.addInitScript(() => {
    window.addEventListener("keydown", event => {
      if (event.code === "ImeMarkingProof") Object.defineProperty(event, "isComposing", { value: true });
    }, true);
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  const toggle = form.locator(".launch-composer-save-template");
  const panel = form.locator(".save-template-panel");
  const field = panel.locator(".save-template-name");

  await toggle.click();
  await expect(field).toBeFocused();
  await field.dispatchEvent("keydown", { key: "Escape", code: "Escape", isComposing: false });
  await expect(panel).toHaveCount(0);

  await toggle.click();
  await expect(field).toBeFocused();
  const observed = await field.evaluate(node => {
    const event = new KeyboardEvent("keydown", {
      key: "Escape", code: "ImeMarkingProof", isComposing: false, bubbles: true, cancelable: true,
    });
    const before = event.isComposing;
    node.dispatchEvent(event);
    return { before, after: event.isComposing };
  });
  expect(observed).toEqual({ before: false, after: true });
  await expect(panel).toBeVisible();
  await expect(field).toBeFocused();
  await field.press("Escape");
  await expect(panel).toHaveCount(0);
  await expect(form).toBeVisible();
});

/** Replay both orders through the real template writer. A distinct final name
 * prevents an early save from masquerading as consent from the later Enter.
 * These synthetic events do not prove a real Mac input method.
 */
for (const [order, keyCode] of [["webkit", 13], ["webkit", 229], ["chromium", 13]] as const) {
  test(`composition-ending Enter keeps template saving idle (${order}, ${keyCode})`, async ({ page, request }) => {
    const name = `composition-${order}-${keyCode}-${Date.now()}`;
    const unfinished = `${name}-unfinished`;
    let writes = 0;
    page.on("request", request => {
      if (request.method() === "PUT" && new URL(request.url()).pathname.startsWith("/api/templates/")) writes++;
    });
    try {
      const { panel, field } = await openTemplateSave(page);
      await field.fill(unfinished);
      await endComposition(field, "Enter", keyCode, order);
      await expect(panel).toBeVisible();
      await expect(field).toHaveValue(unfinished);
      expect(writes).toBe(0);
      await field.fill(name);
      await field.press("Enter");
      await expect(page.locator(".templates-dialog")).toBeVisible();
      expect(writes).toBe(1);
      const stored = await request.get("/api/templates");
      expect(stored.ok()).toBe(true);
      const names = (await stored.json()).templates.map((template: { name: string }) => template.name);
      expect(names).toContain(name);
      expect(names).not.toContain(unfinished);
    } finally {
      for (const candidate of [name, unfinished]) {
        const response = await request.delete(`/api/templates/${encodeURIComponent(candidate)}`);
        expect(response.ok() || response.status() === 404).toBe(true);
      }
    }
  });
}

/** Replay the WebKit ending key with both reported key codes, and Chromium's
 * opposite order. Each must spare the next real Escape without a timer wait.
 */
for (const [order, keyCode] of [["webkit", 27], ["webkit", 229], ["chromium", 27]] as const) {
  test(`composition-ending Escape keeps the template draft (${order}, ${keyCode})`, async ({ page }) => {
    const { form, panel, field } = await openTemplateSave(page);
    await field.fill("unfinished composition");
    await endComposition(field, "Escape", keyCode, order);
    await expect(panel).toBeVisible();
    await expect(field).toHaveValue("unfinished composition");
    await expect(field).toBeFocused();
    await field.press("Escape");
    await expect(panel).toHaveCount(0);
    await expect(form).toBeVisible();
  });
}

/** Pinyin's ordinary 229 Enter must retain its action without a preceding end.
 * This replays the event shape; it does not drive a live Pinyin input method.
 */
test("Enter 229 without composition still saves a template", async ({ page, request }) => {
  const name = `plain-ime-enter-${Date.now()}`;
  try {
    const { field } = await openTemplateSave(page);
    await field.fill(name);
    await field.dispatchEvent("keydown", { key: "Enter", code: "Enter", keyCode: 229, isComposing: false });
    await expect(page.locator(".templates-dialog")).toBeVisible();
    const response = await request.get("/api/templates");
    expect(response.ok()).toBe(true);
    expect((await response.json()).templates.map((template: { name: string }) => template.name)).toContain(name);
  } finally {
    const response = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
    expect(response.ok() || response.status() === 404).toBe(true);
  }
});

/** A replayed WebKit ending key must neither submit nor cancel rename. The
 * deliberate next key keeps ordinary rename semantics, using a real owned row.
 * No live input method or desktop-shell behavior is exercised here.
 */
for (const key of ["Enter", "Escape"]) {
  test(`composition-ending ${key} stays in rename`, async ({ page, request }) => {
    const session = await createSession(request, { title: `ime-rename-${Date.now()}` });
    try {
      await page.goto("/");
      const row = page.locator(`[data-session-id="${session.id}"]`);
      // The web bundle can still be initializing after navigation completes;
      // use the same initial-list readiness budget as the existing rename spec.
      await expect(row).toBeVisible({ timeout: 15_000 });
      await openRowMenu(row);
      await row.locator(".session-row-rename").click();
      const dialog = page.locator(".rename-dialog");
      const field = dialog.locator(".rename-input");
      await field.fill("unfinished rename composition");
      await endComposition(field, key, key === "Enter" ? 13 : 27);
      await expect(dialog).toBeVisible();
      await expect(field).toHaveValue("unfinished rename composition");
      const response = await request.get(`/api/sessions/${encodeURIComponent(session.id)}`);
      expect(response.ok()).toBe(true);
      expect((await response.json()).title).toBe(session.title);
      const finalName = `${session.title}-final`;
      if (key === "Enter") await field.fill(finalName);
      await field.press(key);
      await expect(dialog).toHaveCount(0);
      await expect(row.locator(".session-title")).toHaveText(key === "Enter" ? finalName : session.title);
    } finally {
      await cleanupSession(request, session.id);
    }
  });
}

/** The model text field owns its Enter choice directly (enter_choice is for
 * buttons). Replay WebKit's order here without submitting the launcher; the
 * separate real Enter must choose and close the list. This is not a Mac IME test.
 */
test("composition-ending Enter leaves a model choice uncommitted", async ({ page }) => {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  const field = form.getByRole("combobox", { name: "Model", exact: true });
  await field.fill("ime-custom-model");
  await expect(field).toHaveAttribute("aria-expanded", "true");
  await endComposition(field, "Enter", 229);
  await expect(field).toHaveAttribute("aria-expanded", "true");
  await field.press("Enter");
  await expect(field).toHaveAttribute("aria-expanded", "false");
  await expect(field).toHaveValue("ime-custom-model");
  await expect(form).toBeVisible();
});

/** A native fixture input outside every modal makes the window chord's own
 * guard decisive. A modal input would pass even without composition marking.
 * Replay WebKit's order; this does not drive a real Mac input method.
 */
test("composition marking precedes window shortcut capture", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".quick-switcher-trigger")).toBeAttached();
  await page.evaluate(() => {
    const input = document.createElement("input");
    input.id = "ime-window-fixture";
    document.body.append(input);
  });
  const field = page.locator("#ime-window-fixture");
  await field.fill("composition draft");
  const mac = await page.evaluate(() => /Mac/.test(navigator.platform || ""));
  const chord = mac ? "Meta+k" : "Control+Shift+k";
  const dialog = page.locator(".quick-switcher-dialog");
  // Establish that this input really can invoke the window handler first.
  await field.press(chord);
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await field.focus();
  await endComposition(field, "k", 75, "webkit", mac ? "mac" : "other");
  await expect(dialog).toHaveCount(0);
  await field.press(chord);
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});
