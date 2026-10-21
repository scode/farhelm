/** Helm-wide confirmation choices must change the next action in this client.
 * These tests keep the real preference route and host registry, so a checkbox
 * that changes only its own appearance cannot satisfy the round trip.
 */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { clickDialogGap } from "./helpers/dialog-gap";
import { openSettings, selectSettingsSection, type SettingsSection } from "./helpers/settings";
import type { APIRequestContext, Request, Route } from "@playwright/test";
import {
  forceBuildSkew,
  hostRowByName,
  openHostMenu,
  patchPreferences,
  readPreferences,
} from "./helpers/fleet";

/** The web fixture has no updater, so its navigation omits the app pane. */
const webSettingsSections: SettingsSection[] = ["sessions", "hosts", "terminal", "checkouts", "agent trust", "notifications"];

/**
 * The shared installer serves Settings even though Settings has no single
 * confirming action. Empty space preserves the current control, container
 * Enter stays inert, and a real pointer drag can still select help text.
 */
test("settings gap keeps focus while text and container Enter keep their own behavior", async ({ page, request }) => {
  // Loading the page saves the automatic session selection (the last selection and the
  // visit history); only the Settings interactions below must leave preferences untouched.
  const withoutSelection = (preferences: Awaited<ReturnType<typeof readPreferences>>) => {
    const { last_selected: _selected, visit_history: _visits, ...rest } = preferences;
    return rest;
  };
  const before = withoutSelection(await readPreferences(request));
  await page.goto("/");
  await page.getByRole("button", { name: "settings", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  await expect(dialog).toBeVisible();
  const choice = dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true });
  const wasChecked = await choice.isChecked();
  await choice.focus();
  await expect(choice).toBeFocused();
  await clickDialogGap(page, dialog);
  await expect(choice).toBeFocused();
  await dialog.focus();
  await expect(dialog).toBeFocused();
  await dialog.press("Enter");
  await expect(dialog).toBeVisible();
  expect(await choice.isChecked(), "container Enter must not toggle a Settings choice").toBe(wasChecked);
  expect(withoutSelection(await readPreferences(request))).toEqual(before);

  const help = dialog.locator("#session-confirmations-help");
  const box = await help.boundingBox();
  expect(box, "the visible help text must have a painted selection target").not.toBeNull();
  await page.mouse.move(box!.x + 12, box!.y + 10);
  await page.mouse.down();
  await page.mouse.move(box!.x + 180, box!.y + 10, { steps: 8 });
  await page.mouse.up();
  expect(await page.evaluate(() => document.getSelection()?.toString().trim()), "dragging over help text must still select it").not.toBe("");
});

/**
 * Container Enter in a destructive dialog must use the visible confirmation.
 * Host removal closes the prompt as soon as it accepts the answer, so its
 * disabled-button contract is checked by setting the button's native disabled
 * state before the real confirmation. That must prevent a deletion.
 */
test("host removal container Enter confirms once and respects the disabled button", async ({ page, request }, testInfo) => {
  const previous = await readPreferences(request);
  const name = `focus-remove-${testInfo.project.name}-${Date.now()}.invalid`;
  const created = await request.post("/api/hosts", { data: { ssh: name } });
  expect(created.ok(), await created.text()).toBe(true);
  const host = await created.json();
  let deletes = 0;
  const intercept = async (route: Route) => {
    if (route.request().method() !== "DELETE") return route.continue();
    deletes += 1;
    await route.continue();
  };
  try {
    await patchPreferences(request, { skip_host_remove_confirmation: false });
    await page.route(`**/api/hosts/${host.id}`, intercept);
    await page.goto("/");
    const row = hostRowByName(page, name);
    await expect(row).toBeVisible();
    await openHostMenu(row);
    await row.locator(".host-remove").click();
    const dialog = page.locator(".host-remove-dialog");
    const confirm = dialog.getByRole("button", { name: "remove", exact: true });
    await expect(confirm).toBeFocused();
    await clickDialogGap(page, dialog);
    await expect(confirm).toBeFocused();
    await confirm.evaluate((button: HTMLButtonElement) => { button.disabled = true; });
    await expect(confirm).toBeDisabled();
    await dialog.focus();
    await dialog.press("Enter");
    expect(deletes, "disabled confirmation must not send a deletion").toBe(0);
    await expect(row).toBeVisible();
    await confirm.evaluate((button: HTMLButtonElement) => { button.disabled = false; });
    await dialog.focus();
    await dialog.press("Enter");
    await expect.poll(() => deletes).toBe(1);
    await expect(row).toHaveCount(0);
  } finally {
    await page.unroute(`**/api/hosts/${host.id}`, intercept);
    await cleanupOwnedSteps(false, [
      [`host ${host.id}`, () => request.delete(`/api/hosts/${host.id}`)],
      ["host removal consent", () => patchPreferences(request, { skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null })],
    ]);
  }
});

/** Closing must remain possible without scrolling a long settings body.
 * Real overflow and a changed scroll offset establish the short-window premise;
 * unchanged frame geometry and hit testing distinguish pinned controls from
 * controls merely reported visible outside the viewport. Both exits restore
 * focus to the gear without changing the initial focus on the first switch.
 */
test("settings keeps both close controls pinned around its grouped scrolling body", async ({ page }, testInfo) => {
  await page.goto("/");
  const gear = page.getByRole("button", { name: "settings", exact: true });
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  const body = dialog.locator(".host-settings-body");
  const dismiss = dialog.getByRole("button", { name: "close settings", exact: true });
  const close = dialog.getByRole("button", { name: "close", exact: true });
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 560 });
    await gear.click();
    await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
    await expect(dialog.getByRole("checkbox", { includeHidden: true })).toHaveCount(17);
    await page.evaluate(() => document.fonts.ready);
    await selectSettingsSection(page, "agent trust");
    await expect(dismiss).toHaveAttribute("data-tooltip", "close settings; unsaved fields are discarded");
    await expect(dismiss).not.toHaveClass(/app-settings-close/);
    const overflow = await body.evaluate((el) => ({
      content: el.scrollHeight, height: el.clientHeight, width: el.clientWidth, contentWidth: el.scrollWidth,
    }));
    expect(overflow.height).toBeGreaterThan(0);
    expect(overflow.content, "premise: settings really exceed the short body").toBeGreaterThan(overflow.height);
    expect(overflow.contentWidth).toBeLessThanOrEqual(overflow.width);
    const header = (await dialog.locator(".host-settings-header").boundingBox())!;
    const footer = (await dialog.locator(".host-settings-footer").boundingBox())!;
    await dialog.screenshot({ path: testInfo.outputPath(`settings-pinned-top-${width}.png`) });
    await body.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await expect.poll(() => body.evaluate((el) => el.scrollTop), {
      message: "body must actually scroll before frame stability is meaningful",
    }).toBeGreaterThan(0);
    expect(await dialog.locator(".host-settings-header").boundingBox()).toEqual(header);
    expect(await dialog.locator(".host-settings-footer").boundingBox()).toEqual(footer);
    const bodyBox = (await body.boundingBox())!;
    expect(header.y).toBeGreaterThanOrEqual(0);
    expect(header.y + header.height).toBeLessThanOrEqual(bodyBox.y + 1);
    expect(bodyBox.y + bodyBox.height).toBeLessThanOrEqual(footer.y + 1);
    expect(footer.y + footer.height).toBeLessThanOrEqual(560);
    expect(await dialog.evaluate((el) => el.scrollTop)).toBe(0);
    for (const control of [dismiss, close]) {
      expect(await control.evaluate((el) => {
        const box = el.getBoundingClientRect();
        const hit = document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2);
        return Boolean(hit && el.contains(hit));
      }), "pinned close control must take a real click in the viewport").toBe(true);
    }
    await dialog.screenshot({ path: testInfo.outputPath(`settings-pinned-bottom-${width}.png`) });
    await dismiss.click();
    await expect(dialog).toHaveCount(0);
    await expect(gear).toBeFocused();
    await gear.click();
    await expect(dialog).toBeVisible();
    await close.click();
    await expect(dialog).toHaveCount(0);
    await expect(gear).toBeFocused();
  }
  // Measure the same long pane in a tall window, so the no-overflow proof
  // cannot pass merely because opening returns to the shorter sessions pane.
  await page.setViewportSize({ width: 1280, height: 5000 });
  await gear.click();
  await selectSettingsSection(page, "agent trust");
  await expect(dialog.getByRole("checkbox", { includeHidden: true })).toHaveCount(17);
  const tall = await body.evaluate((el) => ({ content: el.scrollHeight, height: el.clientHeight }));
  expect(tall.height).toBeGreaterThan(0);
  expect(tall.content, "a tall window must fit the settings without scrolling").toBeLessThanOrEqual(tall.height);
  await dialog.screenshot({ path: testInfo.outputPath("settings-pinned-tall.png") });
  await close.click();
  await expect(gear).toBeFocused();
});

/** Section changes reveal one mounted pane and move focus to its heading.
 * Phone navigation must use the real list/back path, reset body scrolling,
 * and forget its selection on close. Mounted hidden controls are checked
 * separately from visible panes so a missing section cannot pass the sweep.
 */
test("settings navigation keeps mounted panes and follows the narrow list path", async ({ page, browserName }) => {
  await page.setViewportSize({ width: 1280, height: 560 });
  await page.goto("/");
  const dialog = await openSettings(page);
  const navigation = dialog.getByRole("navigation", { name: "settings sections", exact: true });
  const body = dialog.locator(".host-settings-body");
  const panels = dialog.locator("section.app-settings-panel");
  await expect(navigation.getByRole("button")).toHaveText(webSettingsSections);
  await expect(navigation.getByRole("button", { name: "app", exact: true })).toHaveCount(0);
  await expect(panels).toHaveCount(webSettingsSections.length);
  await expect(dialog.getByRole("checkbox", { includeHidden: true })).toHaveCount(17);
  await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
  for (const section of webSettingsSections) {
    const panel = await selectSettingsSection(page, section);
    await expect(dialog.locator("section.app-settings-panel:visible")).toHaveCount(1);
    await expect(panel.locator(":scope > h3")).toHaveText(section);
    await expect(panel.locator(":scope > h3")).toHaveAttribute("tabindex", "-1");
    const button = navigation.getByRole("button", { name: section, exact: true });
    await expect(button).toHaveAttribute("aria-pressed", "true");
    await expect(button).toHaveAttribute("aria-controls", (await panel.getAttribute("id"))!);
    // The longest section name must fit its own column rather than forcing
    // its selected fill across the divider into the settings themselves.
    expect(await button.evaluate(el => {
      const button = el.getBoundingClientRect();
      const list = el.closest("nav")!.getBoundingClientRect();
      return button.left >= list.left && button.right <= list.right && el.scrollWidth <= el.clientWidth;
    })).toBe(true);
    await expect(navigation.locator('[aria-pressed="true"]')).toHaveCount(1);
    await expect(dialog.locator("section.app-settings-panel[hidden]")).toHaveCount(webSettingsSections.length - 1);
    if (section !== "sessions") await expect(panel.locator(":scope > h3")).toBeFocused();
  }
  const notifications = await selectSettingsSection(page, "notifications");
  await expect(notifications.getByRole("region", { name: "sounds", exact: true })).toBeVisible();
  await expect(notifications.getByRole("region", { name: "ntfy alerts", exact: true })).toBeVisible();

  // Plain buttons use the browser's normal Tab order. WebKit deliberately
  // skips buttons under its default Tab policy; no extra tab stops compensate.
  if (browserName !== "webkit") {
    const sessions = navigation.getByRole("button", { name: "sessions", exact: true });
    const hosts = navigation.getByRole("button", { name: "hosts", exact: true });
    await sessions.focus();
    await expect(sessions).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(hosts).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(dialog.locator('section.app-settings-panel[aria-label="hosts"] > h3')).toBeFocused();
  }

  await selectSettingsSection(page, "agent trust");
  await page.setViewportSize({ width: 390, height: 560 });
  await expect(navigation).toBeHidden();
  const back = dialog.getByRole("button", { name: "‹ settings", exact: true });
  await expect(back).toBeVisible();
  expect(await body.evaluate(el => el.scrollHeight > el.clientHeight), "agent trust must overflow before testing scroll reset").toBe(true);
  await body.evaluate(el => { el.scrollTop = el.scrollHeight; });
  await expect.poll(() => body.evaluate(el => el.scrollTop)).toBeGreaterThan(0);
  await back.click();
  await expect(navigation).toBeVisible();
  await expect(dialog.locator("section.app-settings-panel:visible")).toHaveCount(0);
  await expect(back).toBeHidden();
  const trustButton = navigation.getByRole("button", { name: "agent trust", exact: true });
  await expect(trustButton).toBeFocused();
  // Back supplies a native focus origin in both engines, including WebKit's
  // button-skipping Tab policy. Enter must activate this ordinary button.
  await page.keyboard.press("Enter");
  const trust = dialog.locator('section.app-settings-panel[aria-label="agent trust"]');
  await expect(trust).toBeVisible();
  await expect(trust.locator(":scope > h3")).toBeFocused();
  await expect.poll(() => body.evaluate(el => el.scrollTop)).toBe(0);
  await back.click();
  await navigation.getByRole("button", { name: "checkouts", exact: true }).click();
  await expect(dialog.locator('section.app-settings-panel[aria-label="checkouts"] > h3')).toBeFocused();
  await expect(navigation).toBeHidden();
  await page.setViewportSize({ width: 1280, height: 560 });
  await expect(navigation).toBeVisible();
  await expect(dialog.locator('section.app-settings-panel[aria-label="checkouts"]')).toBeVisible();
  await dialog.getByRole("button", { name: "close", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 560 });
  await openSettings(page);
  await expect(dialog.locator('section.app-settings-panel[aria-label="sessions"]')).toBeVisible();
  await expect(navigation).toBeHidden();
  await expect(back).toBeVisible();
  await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
});

/** Naming defaults on without writing a choice during Settings open. The real
 * route and a reload distinguish durable opt-out from an optimistic checkbox;
 * session confirmations stay independent of this adjacent switch.
 */
test("quick-launch naming defaults on and its settings switch persists independently", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const gear = page.getByRole("button", { name: "settings", exact: true });
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  const naming = dialog.getByRole("checkbox", { name: "name sessions on quick launch", exact: true });
  try {
    for (const value of [null, true, false]) {
      await patchPreferences(request, { name_session_on_quick_launch: value });
      expect((await readPreferences(request)).name_session_on_quick_launch ?? null).toBe(value);
      await page.goto("/");
      await gear.click();
      await expect(naming).toBeChecked({ checked: value !== false });
      expect((await readPreferences(request)).name_session_on_quick_launch ?? null).toBe(value);
      await dialog.getByRole("button", { name: "close", exact: true }).click();
    }
    await gear.click();
    await expect(naming).not.toBeChecked();
    for (const value of [true, false]) {
      await naming.setChecked(value);
      await expect.poll(async () => (await readPreferences(request)).name_session_on_quick_launch).toBe(value);
      expect((await readPreferences(request)).skip_session_confirmations).toBe(previous.skip_session_confirmations);
      await dialog.getByRole("button", { name: "close", exact: true }).click();
      await page.reload();
      await gear.click();
      await expect(naming).toBeChecked({ checked: value });
    }
  } finally {
    await patchPreferences(request, { name_session_on_quick_launch: previous.name_session_on_quick_launch ?? null });
  }
});

/** Read the protected public shape to verify persistence independently of the
 * mounted field's local draft or success line. */
async function readCheckoutRoot(request: APIRequestContext): Promise<{ root: string | null }> {
  const reply = await request.get("/api/checkout-root");
  expect(reply.ok(), await reply.text()).toBe(true);
  return await reply.json() as { root: string | null };
}

/** Inspect the stored all-hosts command independently of the field's draft.
 * Null and an explicit disable must stay distinct unless the user saves. */
async function readPostClone(request: APIRequestContext): Promise<{ command: string | null }> {
  const reply = await request.get("/api/checkout-post-clone");
  expect(reply.ok(), await reply.text()).toBe(true);
  return await reply.json() as { command: string | null };
}

/** Each link switch persists its own grant without changing command or YOLO authority.
 * An owned disconnected host keeps this proof independent of the suite's live session host. */
test("host browser permissions persist independently through the settings switches", async ({ page, request }, testInfo) => {
  const name = `browser-settings-${testInfo.project.name}.invalid`;
  const created = await request.post("/api/hosts", { data: { ssh: name } });
  expect(created.ok(), await created.text()).toBe(true);
  const host = await created.json();
  /** Read the stored view, so an optimistic checkbox cannot stand in for persistence. */
  const read = async () => {
    const response = await request.get("/api/hosts");
    expect(response.ok(), await response.text()).toBe(true);
    const body = await response.json();
    return body.hosts.find((row: { id: number }) => row.id === host.id);
  };
  try {
    expect(await read()).toMatchObject({ links_without_asking: false, local_links_without_asking: false, commands_without_asking: false, yolo_without_asking: false });
    await page.goto("/");
    const row = hostRowByName(page, name);
    await expect(row).toBeVisible();
    await openHostMenu(row);
    await row.locator(".host-settings").click();
    const dialog = page.locator(".host-settings-dialog");
    await expect(dialog).toBeVisible();
    const publicLinks = dialog.getByRole("checkbox", { name: "Open links without asking", exact: true });
    const localLinks = dialog.getByRole("checkbox", { name: "Open local-network links without asking", exact: true });
    await expect(publicLinks).not.toBeChecked();
    await expect(localLinks).not.toBeChecked();
    await publicLinks.check();
    await expect.poll(read).toMatchObject({ links_without_asking: true, local_links_without_asking: false, commands_without_asking: false, yolo_without_asking: false });
    await expect(localLinks).not.toBeChecked();
    await localLinks.check();
    await expect.poll(read).toMatchObject({ links_without_asking: true, local_links_without_asking: true, commands_without_asking: false });
    await publicLinks.uncheck();
    await expect.poll(read).toMatchObject({ links_without_asking: false, local_links_without_asking: true, commands_without_asking: false });
    await dialog.locator(".host-settings-close").click();
    await openHostMenu(row);
    await row.locator(".host-settings").click();
    await expect(publicLinks).not.toBeChecked();
    await expect(localLinks).toBeChecked();
    await localLinks.uncheck();
    await expect.poll(read).toMatchObject({ links_without_asking: false, local_links_without_asking: false, commands_without_asking: false });
  } finally {
    const removed = await request.delete(`/api/hosts/${host.id}`);
    expect(removed.ok(), await removed.text()).toBe(true);
  }
});

/** Opening is read-only for all three stored states. Explicit Save preserves
 * executable spacing and newlines, refusal keeps the draft and outcome across
 * navigation, and an all-whitespace save clears rather than storing a disable. */
test("settings saves and clears the all-hosts post-clone command", async ({ page, request }, testInfo) => {
  const previous = await readPostClone(request);
  const read = () => readPostClone(request);
  await page.goto("/");
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  const field = dialog.locator(".checkout-post-clone-field");
  const command = field.getByRole("textbox", { name: "post-clone command", exact: true });
  const save = field.getByRole("button", { name: "save", exact: true });
  const chosen = "  printf 'first'\nprintf 'second'  ";
  try {
    for (const value of [null, "", "  existing command\nnext line  "]) {
      const seeded = await request.put("/api/checkout-post-clone", { data: { command: value } });
      expect(seeded.ok(), await seeded.text()).toBe(true);
      expect(await read()).toEqual({ command: value });
      await openSettings(page, "checkouts");
      await expect(command).toBeEditable();
      await expect(command).toHaveValue(value ?? "");
      expect(await read()).toEqual({ command: value });
      if (value === null) {
        await field.scrollIntoViewIfNeeded();
        await dialog.screenshot({ path: testInfo.outputPath("post-clone-empty.png") });
      }
      await dialog.getByRole("button", { name: "close", exact: true }).click();
      expect(await read()).toEqual({ command: value });
    }
    await openSettings(page, "checkouts");
    await expect(command).toBeEditable();
    const before = await read();
    const refused = "x".repeat(16385);
    await command.fill(refused);
    await save.click();
    await expect(field.getByRole("alert")).toHaveText("post-clone command is 16385 bytes; the limit is 16384 bytes");
    await expect(command).toHaveValue(refused);
    expect(await read()).toEqual(before);
    await selectSettingsSection(page, "sessions");
    await selectSettingsSection(page, "checkouts");
    await expect(command).toHaveValue(refused);
    await expect(field.getByRole("alert")).toHaveText("post-clone command is 16385 bytes; the limit is 16384 bytes");
    expect(await read()).toEqual(before);
    await dialog.screenshot({ path: testInfo.outputPath("post-clone-refused.png") });

    await command.fill("  printf 'first'");
    await command.focus();
    await expect(command).toBeFocused();
    await command.press("End");
    await command.press("Enter");
    await command.pressSequentially("printf 'second'  ");
    await expect(command).toHaveValue(chosen);
    expect(await read()).toEqual(before);
    const refetched = page.waitForResponse((response) =>
      response.url().endsWith("/api/checkout-post-clone") && response.request().method() === "GET"
    );
    await save.click();
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    const refreshed = await refetched;
    expect(refreshed.ok(), await refreshed.text()).toBe(true);
    expect(await refreshed.json()).toEqual({ command: chosen });
    expect(await read()).toEqual({ command: chosen });
    await expect(field.getByRole("alert")).toHaveCount(0);
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 900 });
      await field.scrollIntoViewIfNeeded();
      await dialog.screenshot({ path: testInfo.outputPath(`post-clone-command-${width}.png`) });
      const inputBounds = await command.boundingBox();
      const fieldBounds = await field.boundingBox();
      expect(inputBounds).not.toBeNull();
      expect(fieldBounds).not.toBeNull();
      expect(inputBounds!.x + inputBounds!.width).toBeLessThanOrEqual(fieldBounds!.x + fieldBounds!.width + 1);
    }
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(chosen);
    await command.fill(" \n\t ");
    await save.click();
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    expect(await read()).toEqual({ command: null });
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(command).toBeEditable();
    await expect(command).toHaveValue("");
  } finally {
    const restored = await request.put("/api/checkout-post-clone", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await read()).toEqual(previous);
  }
});

/** Textarea normalization must not silently rewrite an executable command.
 * Seed real CR and CRLF values through the API, prove editing cannot persist
 * their normalized form, then clear and save normally in the same mounted field.
 */
test("post-clone commands with carriage returns refuse edits but can be cleared", async ({ page, request }) => {
  const previous = await readPostClone(request);
  let writes = 0;
  const submitted: Request[] = [];
  const intercept = async (route: Route) => {
    if (route.request().method() === "PUT") {
      writes++;
      submitted.push(route.request());
    }
    await route.continue();
  };
  await page.route("**/api/checkout-post-clone", intercept);
  try {
    await page.goto("/");
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    const field = dialog.locator(".checkout-post-clone-field");
    const command = field.getByRole("textbox", { name: "post-clone command", exact: true });
    const save = field.getByRole("button", { name: "save", exact: true });
    for (const stored of ["printf 'one\rtwo'", "printf 'one\r\ntwo'"]) {
      const seeded = await request.put("/api/checkout-post-clone", { data: { command: stored } });
      expect(seeded.ok(), await seeded.text()).toBe(true);
      expect(await readPostClone(request)).toEqual({ command: stored });
      await openSettings(page, "checkouts");
      await expect(command).toBeEditable();
      await expect(command).toHaveValue(stored.replace(/\r\n?/g, "\n"));
      await command.focus();
      await expect(command).toBeFocused();
      await command.press("End");
      await command.pressSequentially(" # edited");
      const edited = await command.inputValue();
      expect(edited).not.toContain("\r");
      expect(edited).toContain("# edited");
      const before = writes;
      await save.click();
      await expect(field.getByRole("alert")).toContainText("carriage return this field would turn into a newline");
      await expect(command).toHaveValue(edited);
      expect(writes).toBe(before);
      expect(await readPostClone(request)).toEqual({ command: stored });

      await command.fill(" \n\t ");
      const refetched = page.waitForResponse(response =>
        response.url().endsWith("/api/checkout-post-clone") && response.request().method() === "GET"
      );
      await save.click();
      await expect(field.getByRole("status")).toHaveText("post-clone command saved");
      expect(await (await refetched).json()).toEqual({ command: null });
      expect(await readPostClone(request)).toEqual({ command: null });
      await expect(field.getByRole("alert")).toHaveCount(0);
      await command.fill("printf 'ordinary'");
      await save.click();
      await expect(field.getByRole("status")).toHaveText("post-clone command saved");
      expect(await readPostClone(request)).toEqual({ command: "printf 'ordinary'" });
      await dialog.getByRole("button", { name: "close", exact: true }).click();
    }
  } finally {
    // Even an assertion during a save cannot let its late PUT undo restoration.
    for (const put of submitted) {
      const reply = await put.response();
      if (reply) await reply.finished();
    }
    await page.unroute("**/api/checkout-post-clone", intercept);
    const restored = await request.put("/api/checkout-post-clone", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await readPostClone(request)).toEqual(previous);
  }
});

/** A delayed initial read cannot turn the blank placeholder into a clear.
 * Hold the GET at the network boundary and try Save while locked, then release
 * it and require the stored command to seed the field without any PUT. */
test("post-clone command stays locked until its initial read completes", async ({ page, request }) => {
  const previous = await readPostClone(request);
  let release!: () => void;
  let signalReading!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const reading = new Promise<void>((resolve) => { signalReading = resolve; });
  let writes = 0;
  const intercept = async (route: Route) => {
    if (route.request().method() === "GET") {
      signalReading();
      await held;
    } else if (route.request().method() === "PUT") {
      writes++;
    }
    await route.continue();
  };
  await page.route("**/api/checkout-post-clone", intercept);
  try {
    await page.goto("/");
    await openSettings(page, "checkouts");
    await reading;
    const field = page.locator(".checkout-post-clone-field");
    const command = field.getByRole("textbox", { name: "post-clone command", exact: true });
    const save = field.getByRole("button", { name: "save", exact: true });
    await expect(command).not.toBeEditable();
    await expect(command).toHaveValue("");
    await expect(save).toHaveAttribute("aria-disabled", "true");
    // aria-disabled retains focusability for tooltips and keyboard discovery;
    // dispatch a real click to prove the handler refuses a locked Save.
    await save.dispatchEvent("click");
    release();
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(previous.command ?? "");
    expect(writes).toBe(0);
    expect(await readPostClone(request)).toEqual(previous);
  } finally {
    release();
    await page.unroute("**/api/checkout-post-clone", intercept);
  }
});

/** A save keeps its lock across section changes and outlives its dismissed field.
 * Reopening must wait before seeding an editable draft, or an older held save
 * could overwrite a newer one while reopened Settings claims it was saved. */
test("a pending post-clone save orders reopened settings before the next save", async ({ page, request }) => {
  const previous = await readPostClone(request);
  let release!: () => void;
  let signalSubmitted!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const submitted = new Promise<void>((resolve) => { signalSubmitted = resolve; });
  let submittedRequest: Request | undefined;
  const intercept = async (route: Route) => {
    if (route.request().method() !== "PUT") return route.continue();
    submittedRequest = route.request();
    signalSubmitted();
    await held;
    await route.continue();
  };
  const chosen = "printf 'save survives close'";
  await page.route("**/api/checkout-post-clone", intercept);
  try {
    await page.goto("/");
    await openSettings(page, "checkouts");
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    const field = dialog.locator(".checkout-post-clone-field");
    const command = field.getByRole("textbox", { name: "post-clone command", exact: true });
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(previous.command ?? "");
    await command.fill(chosen);
    await field.getByRole("button", { name: "save", exact: true }).click();
    await submitted;
    const pendingSave = field.getByRole("button", { name: "saving…", exact: true });
    await expect(pendingSave).toBeFocused();
    await expect(pendingSave).toHaveAttribute("aria-disabled", "true");
    await expect(command).not.toBeEditable();
    expect(submittedRequest!.postDataJSON()).toEqual({ command: chosen });
    expect(await readPostClone(request)).toEqual(previous);
    await selectSettingsSection(page, "sessions");
    await selectSettingsSection(page, "checkouts");
    await expect(command).toHaveValue(chosen);
    await expect(command).not.toBeEditable();
    await expect(pendingSave).toHaveAttribute("aria-disabled", "true");
    expect(await readPostClone(request)).toEqual(previous);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    // The old field is gone: this lock must belong to the reopened read,
    // not the previous component's local saving flag.
    await openSettings(page, "checkouts");
    await expect(dialog).toBeVisible();
    await expect(command).not.toBeEditable();
    expect(await readPostClone(request)).toEqual(previous);
    // sleep-ok: observe the reopened field while the earlier save is held; initial loading alone is not an ordering proof.
    await page.waitForTimeout(150);
    await expect(command).not.toBeEditable();
    release();
    await expect.poll(() => readPostClone(request)).toEqual({ command: chosen });
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(chosen);
    const newer = "printf 'newer save wins'";
    await command.fill(newer);
    await field.getByRole("button", { name: "save", exact: true }).click();
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    expect(await readPostClone(request)).toEqual({ command: newer });
    await selectSettingsSection(page, "hosts");
    await selectSettingsSection(page, "checkouts");
    await expect(command).toHaveValue(newer);
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    expect(await readPostClone(request)).toEqual({ command: newer });
  } finally {
    release();
    // Settle the owned PUT before restoration even when an assertion fails,
    // or a late forwarded request could overwrite the previous command.
    const reply = await submittedRequest?.response();
    if (reply) await reply.finished();
    await page.unroute("**/api/checkout-post-clone", intercept);
    const restored = await request.put("/api/checkout-post-clone", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await readPostClone(request)).toEqual(previous);
  }
});

/** Editing a folder is an explicit configuration write, not a switch or a
 * local-only draft. Real route readback and reopening distinguish persisted
 * save/clear from appearance; navigation must preserve refused drafts and
 * both refusal and success outcomes without changing the stored root.
 */
test("settings saves and clears the all-hosts checkout folder", async ({ page, request }, testInfo) => {
  await page.goto("/");
  await openSettings(page, "checkouts");
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  await expect(dialog).toBeVisible();
  const read = () => readCheckoutRoot(request);
  const previous = await read();
  const field = dialog.locator(".checkout-root-field");
  const folder = field.getByRole("textbox", { name: "checkout folder", exact: true });
  const save = field.getByRole("button", { name: "save", exact: true });
  const chosen = "~/managed-checkouts";
  const submitted: Request[] = [];
  let bodyFailed = false;
  page.on("request", outgoing => {
    if (outgoing.url().endsWith("/api/checkout-root") && outgoing.method() === "PUT") submitted.push(outgoing);
  });
  try {
    await expect(folder).toHaveValue(previous.root ?? "");
    await expect(folder).toBeEditable();
    await dialog.screenshot({ path: testInfo.outputPath("checkout-settings-loaded.png") });
    await folder.fill("relative-folder");
    await save.click();
    await expect(field.getByRole("alert")).toContainText("not the relative path 'relative-folder'");
    expect(await read()).toEqual(previous);
    await expect(folder).toHaveValue("relative-folder");
    await selectSettingsSection(page, "sessions");
    await selectSettingsSection(page, "checkouts");
    await expect(folder).toHaveValue("relative-folder");
    await expect(field.getByRole("alert")).toContainText("not the relative path 'relative-folder'");
    expect(await read()).toEqual(previous);

    await folder.fill(chosen);
    // Editing alone must not write; a separate click commits configuration.
    expect(await read()).toEqual(previous);
    // The mounted field must refetch without waiting for the revision watcher.
    // Independent route readback and reopening cannot prove this client action.
    const refetched = page.waitForResponse((response) =>
      response.url().endsWith("/api/checkout-root") && response.request().method() === "GET"
    );
    await save.click();
    await expect(field.getByRole("status")).toHaveText("checkout folder saved");
    const refreshed = await refetched;
    expect(refreshed.ok(), await refreshed.text()).toBe(true);
    expect(await refreshed.json()).toEqual({ root: chosen });
    expect(await read()).toEqual({ root: chosen });
    await expect(field.getByRole("alert")).toHaveCount(0);
    await selectSettingsSection(page, "hosts");
    await selectSettingsSection(page, "checkouts");
    await expect(folder).toHaveValue(chosen);
    await expect(field.getByRole("status")).toHaveText("checkout folder saved");
    expect(await read()).toEqual({ root: chosen });
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 900 });
      await field.screenshot({ path: testInfo.outputPath(`checkout-settings-saved-${width}.png`) });
      const inputBounds = await folder.boundingBox();
      const fieldBounds = await field.boundingBox();
      expect(inputBounds).not.toBeNull();
      expect(fieldBounds).not.toBeNull();
      expect(inputBounds!.x + inputBounds!.width).toBeLessThanOrEqual(fieldBounds!.x + fieldBounds!.width + 1);
    }
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(folder).toHaveValue(chosen);
    await expect(folder).toBeEditable();
    await folder.fill("");
    await save.click();
    await expect(field.getByRole("status")).toHaveText("checkout folder saved");
    expect(await read()).toEqual({ root: null });
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue("");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["submitted checkout saves", async () => {
        // A page-owned save can outlive the failed assertion. Its terminal
        // response (including a transport failure) must precede restoration.
        for (const outgoing of submitted) {
          const reply = await outgoing.response().catch(() => null);
          if (reply) await reply.finished().catch(() => null);
        }
      }],
      ["checkout folder", async () => {
        const restored = await request.put("/api/checkout-root", { data: previous });
        expect(restored.ok(), await restored.text()).toBe(true);
        expect(await read()).toEqual(previous);
      }],
    ]);
  }
});

/** Navigation keeps both unsaved checkout drafts, while closing discards them.
 * Real API readback and a zero-PUT assertion distinguish a mounted draft from
 * accidental autosave. No configuration write is needed for this boundary.
 */
test("checkout drafts survive switching sections and are discarded on close", async ({ page, request }) => {
  const previousRoot = await readCheckoutRoot(request);
  const previousCommand = await readPostClone(request);
  const writes: Request[] = [];
  page.on("request", outgoing => {
    if (outgoing.method() === "PUT" && /\/api\/checkout-(root|post-clone)$/.test(outgoing.url())) writes.push(outgoing);
  });
  await page.goto("/");
  const dialog = await openSettings(page, "checkouts");
  const folder = dialog.getByRole("textbox", { name: "checkout folder", exact: true });
  const command = dialog.getByRole("textbox", { name: "post-clone command", exact: true });
  await expect(folder).toBeEditable();
  await expect(command).toBeEditable();
  await expect(folder).toHaveValue(previousRoot.root ?? "");
  await expect(command).toHaveValue((previousCommand.command ?? "").replace(/\r\n?/g, "\n"));
  const folderDraft = `${previousRoot.root ?? "~/checkouts"}/unsaved-section-draft`;
  const commandDraft = `${(previousCommand.command ?? "").replace(/\r\n?/g, "\n")}\nprintf 'unsaved section draft'`;
  await folder.fill(folderDraft);
  await command.fill(commandDraft);
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 800 });
    await selectSettingsSection(page, "notifications");
    await selectSettingsSection(page, "checkouts");
    await expect(folder).toHaveValue(folderDraft);
    await expect(command).toHaveValue(commandDraft);
    expect(await readCheckoutRoot(request)).toEqual(previousRoot);
    expect(await readPostClone(request)).toEqual(previousCommand);
    expect(writes).toHaveLength(0);
  }
  await dialog.getByRole("button", { name: "close", exact: true }).click();
  await openSettings(page);
  await expect(dialog.locator('section.app-settings-panel[aria-label="sessions"]')).toBeVisible();
  await selectSettingsSection(page, "checkouts");
  await expect(folder).toBeEditable();
  await expect(command).toBeEditable();
  await expect(folder).toHaveValue(previousRoot.root ?? "");
  await expect(command).toHaveValue((previousCommand.command ?? "").replace(/\r\n?/g, "\n"));
  expect(await readCheckoutRoot(request)).toEqual(previousRoot);
  expect(await readPostClone(request)).toEqual(previousCommand);
  expect(writes).toHaveLength(0);
});

/** Saving without editing must not rename a literal CLI-configured folder.
 * Only edited text loses surrounding whitespace; independent API readback
 * distinguishes stored spelling from the text left visible in the editor.
 */
test("settings preserves untouched literal checkout folders and trims edited paths", async ({ page, request }) => {
  const previous = await readCheckoutRoot(request);
  const submitted: Request[] = [];
  let bodyFailed = false;
  page.on("request", outgoing => {
    if (outgoing.url().endsWith("/api/checkout-root") && outgoing.method() === "PUT") submitted.push(outgoing);
  });
  try {
    const literal = "~/literal-checkouts ";
    const seeded = await request.put("/api/checkout-root", { data: { root: literal } });
    expect(seeded.ok()).toBe(true);
    expect(await readCheckoutRoot(request)).toEqual({ root: literal });
    await page.goto("/");
    await openSettings(page, "checkouts");
    const field = page.getByRole("dialog", { name: "settings", exact: true }).locator(".checkout-root-field");
    const folder = field.getByRole("textbox", { name: "checkout folder", exact: true });
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue(literal);
    /** Each Save needs its own terminal reply; an old success notice cannot
     * establish that the next configuration write has completed.
     */
    const saveAndWait = async () => {
      const finished = page.waitForResponse(response =>
        response.url().endsWith("/api/checkout-root") && response.request().method() === "PUT"
      );
      await field.getByRole("button", { name: "save", exact: true }).click();
      expect((await finished).ok()).toBe(true);
      await expect(field.getByRole("status")).toHaveText("checkout folder saved");
    };
    await saveAndWait();
    expect(await readCheckoutRoot(request)).toEqual({ root: literal });
    await expect(folder).toHaveValue(literal);
    await folder.fill("  ~/edited-checkouts  ");
    await saveAndWait();
    expect(await readCheckoutRoot(request)).toEqual({ root: "~/edited-checkouts" });
    await expect(folder).toHaveValue("~/edited-checkouts");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["submitted checkout saves", async () => {
        for (const outgoing of submitted) {
          const reply = await outgoing.response();
          if (reply) await reply.finished();
        }
      }],
      ["checkout folder", async () => {
        const restored = await request.put("/api/checkout-root", { data: previous });
        expect(restored.ok()).toBe(true);
        expect(await readCheckoutRoot(request)).toEqual(previous);
      }],
    ]);
  }
});

/** Save is a submitted operation even if Settings closes while it is pending.
 * Hold the real PUT before forwarding it, dismiss the mounted field, then
 * release it and inspect persistent state. Component-owned cancellation would
 * leave the old folder, so the success line alone cannot satisfy this test. */
test("a pending checkout folder save survives closing settings", async ({ page, request }) => {
  const previous = await readCheckoutRoot(request);
  let release!: () => void;
  let signalSubmitted!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const submitted = new Promise<void>((resolve) => { signalSubmitted = resolve; });
  let submittedRequest: Request | undefined;
  const intercept = async (route: Route) => {
    if (route.request().method() !== "PUT") return route.continue();
    submittedRequest = route.request();
    signalSubmitted();
    await held;
    await route.continue();
  };
  await page.route("**/api/checkout-root", intercept);
  try {
    await page.goto("/");
    await openSettings(page, "checkouts");
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    const folder = dialog.getByRole("textbox", { name: "checkout folder", exact: true });
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue(previous.root ?? "");
    await folder.fill("~/save-survives-close");
    await dialog.locator(".checkout-root-field").getByRole("button", { name: "save", exact: true }).click();
    await submitted;
    const pendingSave = dialog.getByRole("button", { name: "saving…", exact: true });
    await expect(pendingSave).toBeFocused();
    await expect(pendingSave).toHaveAttribute("aria-disabled", "true");
    expect(await readCheckoutRoot(request)).toEqual(previous);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    release();
    await expect.poll(() => readCheckoutRoot(request)).toEqual({ root: "~/save-survives-close" });
    await openSettings(page, "checkouts");
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue("~/save-survives-close");
  } finally {
    release();
    // A failed assertion can leave the released PUT in flight. Wait for its
    // response or cancellation before restoring, or it can overwrite cleanup.
    const reply = await submittedRequest?.response();
    if (reply) await reply.finished();
    await page.unroute("**/api/checkout-root", intercept);
    const restored = await request.put("/api/checkout-root", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await readCheckoutRoot(request)).toEqual(previous);
  }
});

/** Host-removal consent can be changed back without reloading the client.
 * Independent cleanup must restore both host and session consent even when
 * removing one of the owned hosts fails, since later specs share the helm.
 */
test("settings reverses a permanent removal answer without reloading", async ({ page, request }, testInfo) => {
  const previous = await readPreferences(request);
  const owned: number[] = [];
  const names: string[] = [];
  let bodyFailed = false;
  try {
    // The session opt-out does not answer a host's independent remove question.
    await patchPreferences(request, { skip_host_remove_confirmation: null, skip_session_confirmations: true });
    for (let n = 0; n < 3; n++) {
      const name = `settings-${testInfo.project.name}-${n}.invalid`;
      const reply = await request.post("/api/hosts", { data: { ssh: name } });
      expect(reply.ok(), await reply.text()).toBe(true);
      const host = await reply.json();
      owned.push(host.id);
      names.push(name);
    }
    await page.goto("/");
    const first = hostRowByName(page, names[0]);
    await expect(first).toBeVisible();
    await openHostMenu(first);
    await first.locator(".host-remove").click();
    const removal = page.locator(".host-remove-dialog");
    await expect(removal).toBeVisible();
    await expect(removal.getByText(`undo "don't ask again" with the gear at the top of the sidebar`)).toBeVisible();
    await removal.getByRole("button", { name: "remove, don't ask again", exact: true }).click();
    await expect(first).toHaveCount(0);
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(true);

    // The permanent answer itself takes effect before settings changes it.
    const second = hostRowByName(page, names[1]);
    await openHostMenu(second);
    await second.locator(".host-remove").click();
    await expect(second).toHaveCount(0);
    await expect(removal).toHaveCount(0);

    const gear = page.getByRole("button", { name: "settings", exact: true });
    await openSettings(page, "hosts");
    const settings = page.getByRole("dialog", { name: "settings", exact: true });
    const toggle = settings.getByRole("checkbox", { name: "remove hosts without asking", exact: true });
    await expect(toggle).toBeChecked();
    await toggle.uncheck();
    await expect(toggle).not.toBeChecked();
    await expect(settings.locator("#host-remove-preference-help")).toContainText("asks before forgetting");
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(false);
    await settings.getByRole("button", { name: "close", exact: true }).click();
    await expect(gear).toBeFocused();

    const third = hostRowByName(page, names[2]);
    await openHostMenu(third);
    await third.locator(".host-remove").click();
    await expect(removal).toBeVisible();
    await expect(third).toBeVisible();
    await removal.getByRole("button", { name: "cancel", exact: true }).click();
    // Ticking the settings checkbox is also a permanent answer, not merely
    // a way to clear one. The same still-registered host now removes at once.
    await openSettings(page, "hosts");
    await toggle.check();
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(true);
    await settings.getByRole("button", { name: "close", exact: true }).click();
    await openHostMenu(third);
    await third.locator(".host-remove").click();
    await expect(third).toHaveCount(0);
    await expect(removal).toHaveCount(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ...owned.map((id): [string, () => Promise<unknown>] => [`host ${id}`, () => request.delete(`/api/hosts/${id}`)]),
      ["host removal consent", () => patchPreferences(request, { skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null })],
      ["session consent", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
    ]);
  }
});

/** Every pane's switches must fit beside their labels and keep native keyboard
 * behavior. The modal also isolates the page and returns focus to its opener;
 * styling a replacement control must not bypass either lifecycle.
 */
test("settings dismisses a row menu, isolates the page and returns focus on Escape", async ({ page, request }, testInfo) => {
  const previous = await readPreferences(request);
  try {
    await patchPreferences(request, { skip_host_setup_confirmation: null, skip_host_remove_confirmation: false });
    await page.goto("/");
    const row = page.locator(".host-row").first();
    await expect(row).toBeVisible();
    await openHostMenu(row);
    await expect(row.locator('[role="menu"]')).toBeVisible();
    const gear = page.getByRole("button", { name: "settings", exact: true });
    await gear.click();
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    await expect(dialog).toBeVisible();
    await expect(page.locator('[role="menu"]')).toHaveCount(0);
    await expect(dialog.getByRole("checkbox", { includeHidden: true })).toHaveCount(17);
    const setup = dialog.getByRole("checkbox", { name: "set up new hosts without asking", exact: true });
    await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
    await selectSettingsSection(page, "hosts");
    await expect(setup).not.toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "remove hosts without asking", exact: true })).not.toBeChecked();
    expect(await dialog.evaluate((el) => el.closest(".app-bar"))).toBeNull();
    expect(await gear.evaluate((el) => Boolean(el.closest("[inert]")))).toBe(true);
    await page.evaluate(() => document.fonts.ready);
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 800 });
      let measuredSwitches = 0;
      for (const section of webSettingsSections) {
        const panel = await selectSettingsSection(page, section);
        await dialog.screenshot({ path: testInfo.outputPath(`settings-switches-${width}-${section}.png`) });
        // Measure rendered text independently of the label's full-width box:
        // a checkbox above or before its caption must fail this contract. Radios
        // share the label layout but retain native geometry, unlike these switches.
        const rows = await panel.locator('.app-settings-choice:has(input[type="checkbox"])').evaluateAll((labels) => labels.map((label) => {
          const input = label.querySelector("input")!;
          const range = document.createRange();
          const text = Array.from(label.childNodes).find((node) => node !== input && node.textContent?.trim())!;
          range.selectNode(text);
          const caption = range.getBoundingClientRect();
          const control = input.getBoundingClientRect();
          const row = label.getBoundingClientRect();
          return {
            textRight: caption.right, textTop: caption.top, textBottom: caption.bottom,
            controlLeft: control.left, controlRight: control.right,
            controlTop: control.top, controlBottom: control.bottom,
            width: control.width, height: control.height, rowRight: row.right,
            appearance: getComputedStyle(input).appearance,
          };
        }));
        measuredSwitches += rows.length;
        for (const row of rows) {
          expect(row.textRight).toBeLessThan(row.controlLeft);
          expect(row.controlTop).toBeLessThan(row.textBottom);
          expect(row.controlBottom).toBeGreaterThan(row.textTop);
          expect(row.controlRight).toBeCloseTo(row.rowRight, 0);
          expect(row.width).toBe(34);
          expect(row.height).toBe(20);
          expect(row.appearance).toBe("none");
        }
        const alignment = await panel.evaluate((el) => ({
          choices: Array.from(el.querySelectorAll(".app-settings-choice"), (row) => row.getBoundingClientRect().left),
          help: Array.from(el.querySelectorAll(".host-settings-help"), (help) => help.getBoundingClientRect().left),
          heading: el.querySelector(":scope > h3")!.getBoundingClientRect().left,
          overflowing: el.scrollWidth > el.clientWidth,
        }));
        expect(alignment.overflowing).toBe(false);
        for (const left of [...alignment.choices, ...alignment.help]) expect(left).toBeCloseTo(alignment.heading, 0);
      }
      expect(measuredSwitches, "every mounted switch must be measured in its visible pane").toBe(17);
    }
    // Space exercises the native input rather than Playwright's checked-state
    // setter, and the real helm reply proves the preference writer still runs.
    await selectSettingsSection(page, "hosts");
    await setup.focus();
    await expect(setup).toBeFocused();
    await setup.press("Space");
    await expect(setup).toBeChecked();
    await expect.poll(async () => (await readPreferences(request)).skip_host_setup_confirmation).toBe(true);
    await setup.press("Space");
    await expect(setup).not.toBeChecked();
    await expect.poll(async () => (await readPreferences(request)).skip_host_setup_confirmation).toBe(false);
    await setup.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(gear).toBeFocused();
    expect(await gear.evaluate((el) => Boolean(el.closest("[inert]")))).toBe(false);
  } finally {
    await patchPreferences(request, {
      skip_host_setup_confirmation: previous.skip_host_setup_confirmation ?? null,
      skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null,
    });
  }
});

/** A long reported version yields to the gear in both normal and macOS
 * header layouts. This exercises real hit testing, not AppKit dragging.
 */
test("settings gear stays clickable beside a long version in narrow headers", async ({ page }, testInfo) => {
  await forceBuildSkew(page, "9.9.9-" + "long-version-".repeat(4));
  await page.goto("/");
  const version = page.locator(".app-version");
  await expect(version).toContainText("long-version");
  const gear = page.getByRole("button", { name: "settings", exact: true });
  for (const macos of [false, true]) {
    await page.locator(".app-shell").evaluate((el, active) => el.classList.toggle("macos-window", active), macos);
    await page.locator(".window-root").evaluate((el, active) => el.classList.toggle("macos-root", active), macos);
    for (const width of [900, 400]) {
      await page.setViewportSize({ width, height: 600 });
      const bar = (await page.locator(".app-bar").boundingBox())!;
      const stamp = (await version.boundingBox())!;
      const button = (await gear.boundingBox())!;
      expect(button.width).toBeGreaterThan(0);
      expect(button.x).toBeGreaterThanOrEqual(stamp.x + stamp.width);
      expect(button.x + button.width).toBeLessThanOrEqual(bar.x + bar.width);
      // One 22px button row plus padding/border fits below 40px; macOS
      // reserves its existing 48px band for the native titlebar controls.
      expect(bar.height).toBeLessThanOrEqual(macos ? 48 : 40);
      expect(button.y).toBeGreaterThanOrEqual(bar.y);
      expect(button.y + button.height).toBeLessThanOrEqual(bar.y + bar.height);
      expect(await gear.evaluate((el) => Boolean(el.closest(".window-drag-region")))).toBe(false);
      await gear.click();
      const dialog = page.getByRole("dialog", { name: "settings", exact: true });
      await expect(dialog).toBeVisible();
      if (macos && width === 400) {
        // Retain the narrow modal for visual inspection alongside geometry evidence.
        await page.screenshot({ path: testInfo.outputPath("settings-narrow.png") });
      }
      await dialog.getByRole("button", { name: "close", exact: true }).click();
      await expect(gear).toBeFocused();
    }
  }
});
