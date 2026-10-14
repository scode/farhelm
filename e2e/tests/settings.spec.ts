/** Helm-wide confirmation choices must change the next action in this client.
 * These tests keep the real preference route and host registry, so a checkbox
 * that changes only its own appearance cannot satisfy the round trip.
 */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Request, Route } from "@playwright/test";
import {
  forceBuildSkew,
  hostRowByName,
  openHostMenu,
  patchPreferences,
  readPreferences,
} from "./helpers/fleet";

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
    await expect(dialog.getByRole("checkbox")).toHaveCount(8);
    await page.evaluate(() => document.fonts.ready);
    expect(await body.locator(":scope > section > h3").allTextContents()).toEqual([
      "sessions", "hosts", "terminal links", "managed checkouts", "sounds", "ntfy alerts",
    ]);
    await expect(body.locator('section[aria-label="updates"]')).toHaveCount(0);
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
  // The explicit ntfy fields add a full section; this leg deliberately gives
  // every section enough height, independently of the phone overflow proof.
  await page.setViewportSize({ width: 1280, height: 2800 });
  await gear.click();
  await expect(dialog.getByRole("checkbox")).toHaveCount(8);
  const tall = await body.evaluate((el) => ({ content: el.scrollHeight, height: el.clientHeight }));
  expect(tall.height).toBeGreaterThan(0);
  expect(tall.content, "a tall window must fit the settings without scrolling").toBeLessThanOrEqual(tall.height);
  await dialog.screenshot({ path: testInfo.outputPath("settings-pinned-tall.png") });
  await close.click();
  await expect(gear).toBeFocused();
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
 * executable spacing and newlines, refusal keeps the draft and stored value,
 * and an all-whitespace save clears rather than storing an explicit disable. */
test("settings saves and clears the all-hosts post-clone command", async ({ page, request }, testInfo) => {
  const previous = await readPostClone(request);
  const read = () => readPostClone(request);
  await page.goto("/");
  const gear = page.getByRole("button", { name: "settings", exact: true });
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
      await gear.click();
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
    await gear.click();
    await expect(command).toBeEditable();
    const before = await read();
    const refused = "x".repeat(16385);
    await command.fill(refused);
    await save.click();
    await expect(field.getByRole("alert")).toHaveText("post-clone command is 16385 bytes; the limit is 16384 bytes");
    await expect(command).toHaveValue(refused);
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
    await gear.click();
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(chosen);
    await command.fill(" \n\t ");
    await save.click();
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    expect(await read()).toEqual({ command: null });
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await gear.click();
    await expect(command).toBeEditable();
    await expect(command).toHaveValue("");
  } finally {
    const restored = await request.put("/api/checkout-post-clone", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await read()).toEqual(previous);
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
    await page.getByRole("button", { name: "settings", exact: true }).click();
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

/** The command field owns a separate save path from the folder field. Hold
 * its real PUT before forwarding, prove it is pending and locked, then close
 * Settings; persistent readback must still observe the submitted command. */
test("a pending post-clone command save survives closing settings", async ({ page, request }) => {
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
    await page.getByRole("button", { name: "settings", exact: true }).click();
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
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    release();
    await expect.poll(() => readPostClone(request)).toEqual({ command: chosen });
    await page.getByRole("button", { name: "settings", exact: true }).click();
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(chosen);
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
 * save/clear from appearance; refusal must preserve both draft and saved root.
 */
test("settings saves and clears the all-hosts checkout folder", async ({ page, request }, testInfo) => {
  await page.goto("/");
  await page.getByRole("button", { name: "settings", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  await expect(dialog).toBeVisible();
  const read = () => readCheckoutRoot(request);
  const previous = await read();
  const field = dialog.locator(".checkout-root-field");
  const folder = field.getByRole("textbox", { name: "checkout folder", exact: true });
  const save = field.getByRole("button", { name: "save", exact: true });
  const chosen = "~/managed-checkouts";
  try {
    await expect(folder).toHaveValue(previous.root ?? "");
    await expect(folder).toBeEditable();
    await dialog.screenshot({ path: testInfo.outputPath("checkout-settings-loaded.png") });
    await folder.fill("relative-folder");
    await save.click();
    await expect(field.getByRole("alert")).toContainText("not the relative path 'relative-folder'");
    expect(await read()).toEqual(previous);
    await expect(folder).toHaveValue("relative-folder");

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
    await page.getByRole("button", { name: "settings", exact: true }).click();
    await expect(folder).toHaveValue(chosen);
    await expect(folder).toBeEditable();
    await folder.fill("");
    await save.click();
    await expect(field.getByRole("status")).toHaveText("checkout folder saved");
    expect(await read()).toEqual({ root: null });
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await page.getByRole("button", { name: "settings", exact: true }).click();
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue("");
  } finally {
    const restored = await request.put("/api/checkout-root", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await read()).toEqual(previous);
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
    await page.getByRole("button", { name: "settings", exact: true }).click();
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
    await page.getByRole("button", { name: "settings", exact: true }).click();
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

test("settings reverses a permanent removal answer without reloading", async ({ page, request }, testInfo) => {
  const previous = await readPreferences(request);
  const owned: number[] = [];
  const names: string[] = [];
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
    await gear.click();
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
    await gear.click();
    await toggle.check();
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(true);
    await settings.getByRole("button", { name: "close", exact: true }).click();
    await openHostMenu(third);
    await third.locator(".host-remove").click();
    await expect(third).toHaveCount(0);
    await expect(removal).toHaveCount(0);
  } finally {
    for (const id of owned) await request.delete(`/api/hosts/${id}`);
    await patchPreferences(request, { skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null });
    await patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null });
  }
});

/** Settings switches must fit beside their labels and keep native keyboard
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
    await expect(dialog.getByRole("checkbox")).toHaveCount(8);
    const setup = dialog.getByRole("checkbox", { name: "set up new hosts without asking", exact: true });
    await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
    await expect(setup).not.toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "remove hosts without asking", exact: true })).not.toBeChecked();
    expect(await dialog.evaluate((el) => el.closest(".app-bar"))).toBeNull();
    expect(await gear.evaluate((el) => Boolean(el.closest("[inert]")))).toBe(true);
    await page.evaluate(() => document.fonts.ready);
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 800 });
      await dialog.screenshot({ path: testInfo.outputPath(`settings-switches-${width}.png`) });
      // Measure rendered text independently of the label's full-width box:
      // a checkbox above or before its caption must fail this contract. Radios
      // share the label layout but retain native geometry, unlike these switches.
      const rows = await dialog.locator('.app-settings-choice:has(input[type="checkbox"])').evaluateAll((labels) => labels.map((label) => {
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
      expect(rows).toHaveLength(8);
      for (const row of rows) {
        expect(row.textRight).toBeLessThan(row.controlLeft);
        expect(row.controlTop).toBeLessThan(row.textBottom);
        expect(row.controlBottom).toBeGreaterThan(row.textTop);
        expect(row.controlRight).toBeCloseTo(row.rowRight, 0);
        expect(row.width).toBe(34);
        expect(row.height).toBe(20);
        expect(row.appearance).toBe("none");
      }
      const alignment = await dialog.evaluate((el) => ({
        choices: Array.from(el.querySelectorAll(".app-settings-choice"), (row) => row.getBoundingClientRect().left),
        help: Array.from(el.querySelectorAll(".host-settings-help"), (help) => help.getBoundingClientRect().left),
        heading: el.querySelector("section[aria-label='sounds'] h3")!.getBoundingClientRect().left,
        overflowing: el.scrollWidth > el.clientWidth,
      }));
      expect(alignment.overflowing).toBe(false);
      for (const left of [...alignment.choices, ...alignment.help]) expect(left).toBeCloseTo(alignment.heading, 0);
    }
    // Space exercises the native input rather than Playwright's checked-state
    // setter, and the real helm reply proves the preference writer still runs.
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
