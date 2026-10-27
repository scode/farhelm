/** Template chords edit a visible launcher draft without taking its launch action. */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import type { APIRequestContext, Locator, Page } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  listHosts,
  listSessions,
  openRowMenu,
  patchPreferences,
  readPreferences,
  resetPreferences,
} from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { requireHelmBuild } from "./helpers/helm-build";
import { stackScratchDir } from "./helpers/scratch";

/** Keep the real template store as the fixture so shortcut dispatch must resolve fresh references. */
async function template(request: APIRequestContext, name: string, fields: Record<string, unknown>) {
  const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: fields });
  expect(saved.ok(), await saved.text()).toBe(true);
  const listing = await request.get("/api/templates");
  expect(listing.ok(), await listing.text()).toBe(true);
  expect((await listing.json()).templates).toEqual(expect.arrayContaining([expect.objectContaining({ name, fields })]));
}

/** Each test owns only its named rows; a failed setup may leave a name absent. */
async function removeTemplate(request: APIRequestContext, name: string) {
  const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
  expect(removed.ok() || removed.status() === 404, `removing ${name}`).toBe(true);
}

/** Number keys use the browser build's physical Control+Shift chord. */
async function number(page: Page, digit: string) {
  await expect(page.locator(".template-number-trigger")).toHaveCount(10);
  await page.keyboard.press(`Control+Shift+Digit${digit}`);
}

/** New must already be visible before a shortcut can target its current draft. */
async function openNew(page: Page): Promise<Locator> {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator('.create-session-form[role="dialog"]');
  await expect(form).toBeVisible();
  return form;
}

test.beforeEach(async ({ request }) => resetPreferences(request));
test.afterEach(async ({ request }) => resetPreferences(request));

/** A held first read makes the second press race it. Both must edit New in order,
 * and the shortcut must leave the user's search and focus alone without creating. */
test("back-to-back bolt and number chords stack on New without submitting", async ({ page, request }) => {
  const stem = `launcher-keys-${Date.now()}`;
  const first = `${stem}-bolt`;
  const second = `${stem}-number`;
  const seed = `${stem}-fallback`;
  const gate = routeGate();
  let heldRead = false;
  let creates = 0;
  let bodyFailed = false;
  try {
    await template(request, first, { kind: "agent", agent: "codex", effort: "high", name: "from bolt" });
    await template(request, second, { kind: "agent", effort: "low", name: "from number" });
    await template(request, seed, { kind: "agent", agent: "codex", name: "fallback seed" });
    await patchPreferences(request, { quick_launch_template: first, template_shortcuts: { "1": second, "2": seed } });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      creates++;
      const probe = await request.get("/api/preferences");
      expect(probe.ok(), await probe.text()).toBe(true);
      await route.fulfill({ status: 409, contentType: "text/plain", body: "launcher shortcut fixture refusal",
        headers: { "x-farhelm-build": requireHelmBuild(probe, "launcher shortcut fixture") } });
    });
    await page.goto("/");
    await number(page, "2");
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-error")).toContainText("launcher shortcut fixture refusal");
    expect(creates, "the hidden quick launch submitted once before revealing its draft").toBe(1);
    // A quick-launch fallback reveals the once-hidden form. Its modal
    // isolation makes the background relay inert while the capture listener
    // still routes later keyboard shortcuts into this draft.
    await expect.poll(() => page.locator(".quick-launch-trigger").evaluate(node => !!node.closest("[inert]")))
      .toBe(true);
    expect((await readPreferences(request)).quick_launch_template).toBe(first);
    expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": second, "2": seed });
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill("keep this query");
    await expect(search).toBeFocused();
    await page.route("**/api/preferences", async (route) => {
      if (route.request().method() === "GET" && !heldRead) {
        heldRead = true;
        await gate.wait();
      }
      await route.continue();
    });
    // A cancelable event reports whether the launcher's chord is consumed.
    // It uses the same capture listener as the physical number press below.
    const prevented = await search.evaluate((node) => {
      const event = new KeyboardEvent("keydown", {
        bubbles: true, cancelable: true, code: "KeyP", key: "p", ctrlKey: true, shiftKey: true,
      });
      node.dispatchEvent(event);
      return event.defaultPrevented;
    });
    expect(prevented, "the launcher owns the bolt chord").toBe(true);
    await expect.poll(() => heldRead, { message: "the first shortcut's fresh read is held" }).toBe(true);
    await number(page, "1");
    gate.release();
    await expect(form.getByLabel("name (optional)")).toHaveValue("from number");
    await expect(form.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true }))
      .toHaveAttribute("aria-pressed", "true");
    await expect(form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }))
      .toHaveAttribute("aria-pressed", "true");
    await expect(search).toHaveValue("keep this query");
    await expect(search).toBeFocused();
    await expect(form).toBeVisible();
    expect(creates, "neither shortcut submits the revealed draft").toBe(1);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["fresh-read gate", () => gate.release()],
      ["preference route", () => page.unroute("**/api/preferences")],
      ["session route", () => page.unroute("**/api/sessions")],
      ["templates", () => cleanupOwnedSteps(false, [
        [first, () => removeTemplate(request, first)],
        [second, () => removeTemplate(request, second)],
        [seed, () => removeTemplate(request, seed)],
      ])],
    ]);
  }
});

/** The prefix is available on launcher controls and in an unselected launcher
 * editor, but selected text keeps the editor's ordinary key behavior. */
test("launcher sequences apply from buttons and unselected search text", async ({ page, request }) => {
  const stem = `launcher-sequence-${Date.now()}`;
  const first = `${stem}-one`;
  const second = `${stem}-two`;
  // An incomplete body keeps its original error if an owned release also fails.
  let bodyFailed = true;
  try {
    await template(request, first, { kind: "agent", agent: "codex", effort: "high", name: "first sequence" });
    await template(request, second, { kind: "agent", effort: "low", name: "second sequence" });
    await patchPreferences(request, { template_shortcuts: { "1": first, "2": second } });
    expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": first, "2": second });
    const form = await openNew(page);
    const button = form.getByRole("tab", { name: "agent", exact: true });
    await button.focus();
    await expect(button).toBeFocused();
    await page.keyboard.press("Control+Shift+KeyX");
    const indicator = page.locator(".launch-sequence-indicator");
    await expect(indicator).toContainText("apply");
    await page.keyboard.press("Digit1");
    await page.keyboard.press("Digit2");
    await page.keyboard.press("Enter");
    await expect(indicator).toHaveCount(0);
    await expect(form.getByLabel("name (optional)")).toHaveValue("second sequence");
    await expect(form.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true }))
      .toHaveAttribute("aria-pressed", "true");
    await expect(button).toBeFocused();

    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill("keep selected text");
    await search.evaluate((node: HTMLInputElement) => node.setSelectionRange(0, 4));
    await expect(search).toBeFocused();
    const selectedPrefixPrevented = await search.evaluate((node) => {
      const event = new KeyboardEvent("keydown", {
        bubbles: true, cancelable: true, code: "KeyX", key: "x", ctrlKey: true, shiftKey: true,
      });
      node.dispatchEvent(event);
      return event.defaultPrevented;
    });
    expect(selectedPrefixPrevented, "selected launcher text keeps its editing chord").toBe(false);
    await expect(indicator).toHaveCount(0);
    await expect(search).toHaveValue("keep selected text");
    await search.evaluate((node: HTMLInputElement) => node.setSelectionRange(node.value.length, node.value.length));
    const unselectedPrefixPrevented = await search.evaluate((node) => {
      const event = new KeyboardEvent("keydown", {
        bubbles: true, cancelable: true, code: "KeyX", key: "x", ctrlKey: true, shiftKey: true,
      });
      node.dispatchEvent(event);
      return event.defaultPrevented;
    });
    expect(unselectedPrefixPrevented, "unselected launcher text starts a sequence").toBe(true);
    await expect(indicator).toBeVisible();
    await page.keyboard.press("Digit1");
    await page.keyboard.press("Enter");
    await expect(form.getByLabel("name (optional)")).toHaveValue("first sequence");
    await expect(search).toHaveValue("keep selected text");
    await expect(search).toBeFocused();
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      [first, () => removeTemplate(request, first)],
      [second, () => removeTemplate(request, second)],
    ]);
  }
});

/** Clone owns a draft too; the number key must edit it without starting a
 * second session or reopening New. */
test("a number shortcut applies to Clone's draft", async ({ page, request }) => {
  const name = `launcher-clone-${Date.now()}`;
  const source = await createSession(request, { title: `${name}-source` });
  let creates = 0;
  let bodyFailed = true;
  try {
    await template(request, name, { kind: "command", name: "cloned by shortcut" });
    await patchPreferences(request, { template_shortcuts: { "1": name } });
    expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": name });
    await page.goto("/");
    const row = page.locator(`.session-row[data-session-id="${source.id}"]`);
    await expect(row).toBeVisible();
    await openRowMenu(row);
    await row.locator(".session-row-clone").click();
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await expect(search).toBeFocused();
    page.on("request", (sent) => {
      if (sent.method() === "POST" && new URL(sent.url()).pathname === "/api/sessions") creates++;
    });
    await number(page, "1");
    await expect(form.getByLabel("name (optional)")).toHaveValue("cloned by shortcut");
    await expect(search).toBeFocused();
    expect(creates, "applying a template never submits Clone").toBe(0);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      [name, () => removeTemplate(request, name)],
      ["source session", () => cleanupSession(request, source.id)],
    ]);
  }
});

/** Replace with fixes the source host. A different host must refuse the whole
 * sequence, leave a later press usable, and yield to a modal above the form. */
test("Replace with refuses a whole sequence and yields to another modal", async ({ page, request }) => {
  const name = `launcher-replace-${Date.now()}`;
  const first = `${name}-first`;
  const local = (await listHosts(request)).find((host) => host.kind === "local")?.identity;
  expect(local, "premise: the local host has an install identity").toBeTruthy();
  const otherIdentity = "11111111-2222-3333-4444-555555555555";
  expect(local, "premise: the refused shortcut must change the installation identity").not.toBe(otherIdentity);
  const source = await createSession(request, { title: `${name}-source` });
  let bodyFailed = true;
  try {
    await template(request, name, { host: otherIdentity, name: "must not apply" });
    await template(request, first, { name: "applied after refusal" });
    await patchPreferences(request, { template_shortcuts: { "1": name, "2": first } });
    expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": name, "2": first });
    await page.goto("/");
    const row = page.locator(`.session-row[data-session-id="${source.id}"]`);
    await expect(row).toBeVisible();
    await openRowMenu(row);
    await row.locator(".session-row-replace-with").click();
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    const nameField = form.getByLabel("name (optional)");
    const before = await nameField.inputValue();
    await number(page, "1");
    await expect(form.locator(".launch-composer-template-error"))
      .toContainText("this dialog keeps the session's host");
    await expect(nameField, "the refused template changes nothing").toHaveValue(before);
    await number(page, "9");
    await expect(form.locator(".launch-composer-template-error")).toContainText("9");
    await expect(page.locator(".templates-dialog")).toHaveCount(0);
    await expect(nameField).toHaveValue(before);

    await nameField.focus();
    await nameField.evaluate((node: HTMLInputElement) => node.setSelectionRange(0, 0));
    await expect(nameField).toBeFocused();
    await page.keyboard.press("Control+Shift+KeyX");
    await expect(page.locator(".launch-sequence-indicator")).toBeVisible();
    await page.keyboard.press("Digit2");
    await page.keyboard.press("Digit1");
    await page.keyboard.press("Enter");
    await expect(form.locator(".launch-composer-template-error"))
      .toContainText("this dialog keeps the session's host");
    await expect(nameField, "a failed later template leaves the entire press unapplied").toHaveValue(before);
    await number(page, "2");
    await expect(nameField, "a later press still applies after the failed sequence")
      .toHaveValue("applied after refusal");

    // The open launcher makes anything later added beside it inert and
    // swallows keys whose focus is outside it. The one way an element stays
    // live is `data-modal-exempt`, the marker agent approval cards carry, so
    // the fixture uses it to model a second visible modal; the premise below
    // checks it stayed live. The shortcut yields because of the fixture's
    // `role="dialog"` and `aria-modal`, not because of the exemption.
    await page.evaluate(() => {
      const dialog = document.createElement("div");
      dialog.id = "launcher-shortcut-modal-fixture";
      dialog.setAttribute("role", "dialog");
      dialog.setAttribute("aria-modal", "true");
      dialog.setAttribute("data-modal-exempt", "true");
      dialog.tabIndex = -1;
      dialog.textContent = "modal fixture";
      document.body.append(dialog);
      dialog.focus();
    });
    const overlay = page.locator("#launcher-shortcut-modal-fixture");
    await expect(overlay, "premise: the exempt fixture is not made inert").not.toHaveAttribute("inert");
    await expect(overlay).toBeFocused();
    const yielded = await overlay.evaluate((node) => {
      const event = new KeyboardEvent("keydown", {
        bubbles: true, cancelable: true, code: "Digit1", key: "1", ctrlKey: true, shiftKey: true,
      });
      node.dispatchEvent(event);
      return event.defaultPrevented;
    });
    expect(yielded, "the upper modal keeps its chord").toBe(false);
    await expect(nameField).toHaveValue("applied after refusal");
    await expect(form).toBeVisible();
    await overlay.evaluate((node) => node.remove());
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      [name, () => removeTemplate(request, name)],
      [first, () => removeTemplate(request, first)],
      ["source session", () => cleanupSession(request, source.id)],
    ]);
  }
});

/** Clearing a quick-launch question must preserve the editor that owns a
 * template chord. Explicit Cancel still hands focus back to the composer;
 * that handoff must not also run over an unrelated live name selection.
 */
test("a template shortcut preserves editor focus when clearing a quick-launch YOLO question", async ({ page, request }) => {
  const stem = `quick-question-focus-${Date.now()}`;
  const yolo = `${stem}-yolo`;
  const guarded = `${stem}-guarded`;
  const initialName = "keep this draft name";
  const query = "keep this search query";
  const cwd = stackScratchDir("quick-question-focus-");
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local?.identity, "the quick template needs the owned local installation").toBeTruthy();
  expect(local?.yolo_without_asking, "fixture premise: the real local guard asks before YOLO launches").toBe(false);
  if (!local?.identity) throw new Error("the local host has no installation identity");
  const build = requireHelmBuild(await request.get("/api/sessions"), "quick question focus fixture");
  const bodies: any[] = [];
  let bodyFailed = false;
  try {
    await template(request, yolo, {
      kind: "agent", agent: "codex", permissions: "yolo", host: local.identity,
      destination: { folder: cwd }, name: initialName, name_kind: "fixed",
    });
    await template(request, guarded, { kind: "agent", permissions: "guarded" });
    await patchPreferences(request, { quick_launch_template: yolo, template_shortcuts: { "1": guarded } });
    const preferences = await readPreferences(request);
    expect(preferences.quick_launch_template).toBe(yolo);
    expect(preferences.template_shortcuts).toEqual({ "1": guarded });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const body = route.request().postDataJSON();
      bodies.push(body);
      if (body.host === local.id && body.launch?.harness === "codex" && body.launch.permissions === "yolo" && body.confirm_yolo !== true) {
        // Only the real guard creates the question. An unexpected submit or
        // retained override must fail the test without executing an agent.
        await route.continue();
      } else {
        await route.fulfill({
          status: 503,
          headers: { "x-farhelm-build": build, "content-type": "text/plain" },
          body: "unexpected create in quick question focus fixture",
        });
      }
    });
    await page.goto("/");
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toHaveCount(0);
    await expect(page.locator(".quick-launch-trigger")).toHaveCount(1);
    const isCreate = (candidate: { request(): { method(): string }; url(): string }) =>
      candidate.request().method() === "POST" && new URL(candidate.url()).pathname === "/api/sessions";

    // Do not use openNew: the hidden quick preparation and its real refusal
    // establish the quick-origin reveal effect that a manual New cannot reach.
    const [first] = await Promise.all([
      page.waitForResponse(isCreate),
      page.keyboard.press("Control+Shift+p"),
    ]);
    expect(first.status()).toBe(409);
    expect(first.headers()["x-farhelm-yolo-confirmation"]).toBe("confirmation-required");
    await expect(form).toBeVisible();
    const question = form.locator(".yolo-confirmation");
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    const name = form.getByLabel("name (optional)", { exact: true });
    await expect(question).toBeVisible();
    await expect(question.locator(".yolo-cancel")).toBeFocused();
    await expect(name).toHaveValue(initialName);
    expect(bodies).toHaveLength(1);
    expect(bodies[0]).toMatchObject({ host: local.id, cwd, title: initialName, launch: { harness: "codex", permissions: "yolo" } });

    /** Observe after the accepted DOM state and its deferred focus work.
     * The second frame measures the outcome of a focus callback queued for
     * the first frame; an immediate assertion could miss that later theft.
     */
    const afterFocusFrames = () => page.evaluate(() => new Promise<void>((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
    }));

    // A blanket removal of reveal focus would hide the regression while
    // breaking the real Cancel handoff. Keep that control in this same case.
    await question.locator(".yolo-cancel").click();
    await expect(question).toHaveCount(0);
    await afterFocusFrames();
    await expect(search).toBeFocused();
    const [again] = await Promise.all([page.waitForResponse(isCreate), form.locator(".create-session-submit").click()]);
    expect(again.status()).toBe(409);
    expect(again.headers()["x-farhelm-yolo-confirmation"]).toBe("confirmation-required");
    await expect(question).toBeVisible();
    // Visibility precedes the question's initial safe-answer focus handoff.
    // Settle that opening before editing; the later shortcut still has to
    // retire the question without disturbing the name field's selection.
    await expect(question.locator(".yolo-cancel")).toBeFocused();
    expect(bodies).toHaveLength(2);

    await search.focus();
    await expect(search).toBeFocused();
    await search.fill(query);
    await expect(search).toHaveValue(query);
    await expect(question).toBeVisible();
    // The name is already template-owned. Focus and native select-all must
    // not edit it or retire the question before the numbered shortcut does.
    await name.focus();
    await expect(name).toBeFocused();
    await page.keyboard.press("ControlOrMeta+A");
    const before = await name.evaluate((element) => {
      const input = element as HTMLInputElement;
      return { focused: document.activeElement === input, value: input.value, start: input.selectionStart, end: input.selectionEnd };
    });
    expect(before).toEqual({ focused: true, value: initialName, start: 0, end: initialName.length });
    await expect(question).toBeVisible();
    expect(bodies).toHaveLength(2);

    await number(page, "1");
    await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }))
      .toHaveAttribute("aria-pressed", "true");
    await expect(question).toHaveCount(0);
    await afterFocusFrames();
    const after = await name.evaluate((element) => {
      const input = element as HTMLInputElement;
      return { focused: document.activeElement === input, value: input.value, start: input.selectionStart, end: input.selectionEnd };
    });
    expect(after, "question retirement must preserve the shortcut owner's focus and selection after deferred focus work").toEqual(before);
    await expect(search).toHaveValue(query);
    expect(bodies, "the bound template only edits the revealed draft").toHaveLength(2);
    for (const body of bodies) expect(body).not.toHaveProperty("confirm_yolo");
    expect((await listHosts(request)).find((host) => host.id === local.id)?.yolo_without_asking).toBe(false);
    expect((await listSessions(request)).sessions.filter((session) => session.cwd === cwd)).toHaveLength(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["entered routes", () => page.unrouteAll({ behavior: "wait" })],
      [yolo, () => removeTemplate(request, yolo)],
      [guarded, () => removeTemplate(request, guarded)],
    ]);
  }
});
