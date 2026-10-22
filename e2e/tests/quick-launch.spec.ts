/** Quick launch uses New's authority without painting New on success, then
 * offers naming while the agent starts. Real input and replies distinguish
 * modal ownership from input leaked to an agent or a quietly retried refusal. */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import type { APIRequestContext, Locator, Page, Route } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  FAKE_AGENT,
  listHosts,
  listSessions,
  observeFeedReaders,
  openHostsPanel,
  openRowMenu,
  patchPreferences,
  pinAutoSelect,
  readPreferences,
  resetPreferences,
  setLocalYoloWithoutAsking,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";
import { waitForSessionReady, waitForSessionRevealed } from "./helpers/terminal-readiness";
import { routeGate } from "./helpers/route-gate";
import { requireHelmBuild } from "./helpers/helm-build";

/** Fabricated replies retain the owned helm's real build identity. The
 * terminal-family helper depends on that family's reset hooks, which these
 * independent specs do not install; an empty stamp would instead test skew. */
async function fulfillQuickFixture(
  route: Route,
  request: APIRequestContext,
  status: number,
  body: string,
  contentType = "text/plain",
) {
  const probe = await request.get("/api/preferences");
  expect(probe.ok(), await probe.text()).toBe(true);
  const stamp = requireHelmBuild(probe, "quick-launch reply fixture");
  await route.fulfill({ status, contentType, body, headers: { "x-farhelm-build": stamp } });
}

/** The shortcut and terminal must both classify the platform before mounting. */
async function platform(page: Page, mac: boolean) {
  await page.addInitScript((value) => {
    Object.defineProperty(Navigator.prototype, "platform", { get: () => value });
  }, mac ? "MacIntel" : "Linux x86_64");
}

/** Seed an actual command template; the fixture agent has no outbound tools. */
async function commandTemplate(request: APIRequestContext, name: string, yolo = false) {
  const response = await request.put(`/api/templates/${encodeURIComponent(name)}`, {
    data: { kind: "command", command: FAKE_AGENT, yolo, name: `${name}-session` },
  });
  expect(response.ok(), await response.text()).toBe(true);
}

/** A direct quick launch must apply a template's prefix before building the
 * agent request. A stamped refusal reveals the submitted draft without
 * starting an agent, which distinguishes an applied prefix from list text. */
test("quick launch submits the wrapper prefix saved in its agent template", async ({ page, request }) => {
  const name = `quick-wrapper-${Date.now()}`;
  const prefix = ["env", "WRAPPER_CASE=quick", "--"];
  const bodies: Record<string, any>[] = [];
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    bodies.push(route.request().postDataJSON());
    await fulfillQuickFixture(route, request, 409, "wrapper quick-launch fixture refusal");
  });
  try {
    const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, {
      data: { kind: "agent", agent: "codex", destination: { folder: "/tmp" }, name: `${name}-session`, wrapper_prefix: prefix },
    });
    expect(saved.ok(), await saved.text()).toBe(true);
    const listing = await request.get("/api/templates");
    expect(listing.ok(), await listing.text()).toBe(true);
    const template = (await listing.json()).templates.find((entry: { name: string }) => entry.name === name);
    expect(template?.fields, "the quick-launch fixture must exist before the chord").toMatchObject({ wrapper_prefix: prefix });
    await patchPreferences(request, { quick_launch_template: name });
    expect((await readPreferences(request)).quick_launch_template).toBe(name);

    await page.goto("/");
    await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-error")).toContainText("wrapper quick-launch fixture refusal");
    await expect(form.locator(".launch-composer-wrapper")).toHaveValue("env 'WRAPPER_CASE=quick' --");
    expect(bodies, "quick launch must submit once before revealing its refused draft").toHaveLength(1);
    expect(bodies[0]).toMatchObject({ launch: { harness: "codex", wrapper_prefix: prefix } });
  } finally {
    await page.unroute("**/api/sessions");
    await patchPreferences(request, { quick_launch_template: null });
    const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
    expect(removed.ok() || removed.status() === 404, "cleaning up the wrapper template").toBe(true);
  }
});

/** A real key chord, including repeat, must pass through the capture listener. */
async function chord(page: Page, mac = false, repeat = false) {
  await expect(page.locator(".quick-launch-trigger")).toHaveCount(1);
  await page.keyboard.down(mac ? "Meta" : "Control");
  if (!mac) await page.keyboard.down("Shift");
  try {
    await page.keyboard.down("p");
    if (repeat) await page.keyboard.down("p");
  } finally {
    await page.keyboard.up("p");
    if (!mac) await page.keyboard.up("Shift");
    await page.keyboard.up(mac ? "Meta" : "Control");
  }
}

/** Number shortcuts always use the physical digit's web chord, even on a Mac.
 * Waiting for all triggers establishes that the Rust list has admitted input.
 */
async function numberChord(page: Page, key = "1", repeat = false) {
  await expect(page.locator(".template-number-trigger")).toHaveCount(10);
  await page.keyboard.down("Control");
  await page.keyboard.down("Shift");
  try {
    await page.keyboard.down(`Digit${key}`);
    if (repeat) await page.keyboard.down(`Digit${key}`);
  } finally {
    await page.keyboard.up(`Digit${key}`);
    await page.keyboard.up("Shift");
    await page.keyboard.up("Control");
  }
}

/** A naming opening must own focus and select the whole actual session title.
 * Returning the field without dismissing it keeps each test's close action
 * explicit, including tests that intentionally close before replay finishes. */
async function namingField(page: Page, title: string): Promise<Locator> {
  const dialog = page.locator(".rename-dialog");
  await expect(dialog).toHaveCount(1);
  await expect(dialog).toBeVisible();
  const field = dialog.locator(".rename-input");
  await expect(field).toHaveValue(title);
  await expect(field).toBeFocused();
  await expect.poll(() => field.evaluate((node: HTMLTextAreaElement) => ({
    start: node.selectionStart, end: node.selectionEnd, length: node.value.length,
  })), { message: "naming selects the entire session title" }).toEqual({ start: 0, end: title.length, length: title.length });
  return field;
}

/** Creation must start the requested agent without waiting for a name.
 * This focus-neutral attachment boundary rejects bytes from the old source
 * terminal while allowing the naming dialog to keep keyboard ownership. */
async function agentStartedWhileNaming(page: Page, id: string, field: Locator) {
  await waitForSessionRevealed(page, id);
  await waitForTermText(page, "FAKE-AGENT READY");
  await waitForTermText(page, "> ");
  await expect(page.locator(".rename-dialog")).toBeVisible();
  await expect(field).toBeFocused();
}

/** Observe the product's focus return, then prove this attachment accepts input.
 * A real response distinguishes restored focus from an idle or dead terminal. */
async function echoAfterNaming(page: Page, id: string, probe: string) {
  await expect(page.locator(".rename-dialog")).toHaveCount(0);
  await waitForSessionReady(page, id);
  await waitForTermText(page, "> ");
  await page.keyboard.type(probe);
  await page.keyboard.press("Enter");
  await waitForTermText(page, `echo:${probe}`);
}

test.beforeEach(async ({ request }) => {
  await resetPreferences(request);
});
test.afterEach(async ({ request }) => {
  await resetPreferences(request);
});

/** A partial first template and runnable final template require a render after
 * the last replay. The held create and paint observer distinguish a direct
 * composed launch from a draft exposed with an obsolete incomplete reason. */
test("launch sequence stacks a number template and bolt without showing New", async ({ page, request }, testInfo) => {
  const prefix = `sequence-${Date.now()}`;
  const partial = `${prefix}-partial`;
  const runnable = `${prefix}-run`;
  const source = await createSession(request, { title: `${prefix}-source` });
  const gate = routeGate();
  const created: string[] = [];
  const bodies: any[] = [];
  const binary: Buffer[] = [];
  let bodyFailed = false;
  try {
    await commandTemplate(request, runnable);
    const saved = await request.put(`/api/templates/${partial}`, {
      data: { name: "earlier-name", kind: "agent" },
    });
    expect(saved.ok(), await saved.text()).toBe(true);
    page.on("websocket", (socket) => {
      if (new URL(socket.url()).pathname.endsWith("/term")) {
        socket.on("framesent", ({ payload }) => {
          if (Buffer.isBuffer(payload)) binary.push(payload);
        });
      }
    });
    await patchPreferences(request, { template_shortcuts: { "1": partial }, quick_launch_template: runnable });
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator("#terminal .xterm-helper-textarea")).toBeFocused();
    binary.length = 0;
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      bodies.push(route.request().postDataJSON());
      await gate.wait();
      const response = await route.fetch();
      if (response.ok()) created.push((await response.json()).id);
      await route.fulfill({ response });
    });
    await page.evaluate(() => {
      const root = window as unknown as { sequencePainted: boolean; sequenceObserver: MutationObserver };
      root.sequencePainted = false;
      root.sequenceObserver = new MutationObserver(() => {
        if (document.querySelector(".create-session-form")?.getClientRects().length) root.sequencePainted = true;
      });
      root.sequenceObserver.observe(document.body, { childList: true, subtree: true, attributes: true });
    });
    await expect(page.locator('.launch-sequence-trigger[data-sequence-action="begin"]')).toBeEnabled();
    await page.keyboard.press("Control+Shift+KeyX");
    const indicator = page.locator(".launch-sequence-indicator");
    await expect(indicator).toHaveAttribute("role", "status");
    await expect(indicator).toContainText("press number keys");
    await page.keyboard.press("Digit1");
    await chord(page);
    await expect(indicator).toContainText(partial);
    await expect(indicator).toContainText(runnable);
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    await page.screenshot({ path: testInfo.outputPath("launch-sequence.png") });
    await page.keyboard.press("Enter");
    await expect.poll(() => bodies.length).toBe(1);
    const form = page.locator(".create-session-form");
    await expect(form).toHaveCount(1);
    await expect(form).toBeHidden();
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    await expect(indicator).toHaveCount(0);
    expect(bodies[0]).toMatchObject({ title: `${runnable}-session`, command: { command: FAKE_AGENT, yolo: false } });
    expect(Buffer.concat(binary).toString(), "the sequence owns its complete input").toBe("");
    gate.release();
    await expect.poll(() => created.length).toBe(1);
    await expect(page.locator(`[data-session-id="${created[0]}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(form).toHaveCount(0);
    const field = await namingField(page, `${runnable}-session`);
    await agentStartedWhileNaming(page, created[0], field);
    expect(Buffer.concat(binary).toString(), "composing and naming send no terminal input").toBe("");
    await page.keyboard.press("Enter");
    const probe = `sequence-live-${Date.now()}`;
    await echoAfterNaming(page, created[0], probe);
    expect(Buffer.concat(binary).toString()).toBe(`${probe}\r`);
    expect(await page.evaluate(() => (window as unknown as { sequencePainted: boolean }).sequencePainted)).toBe(false);
    expect(bodies).toHaveLength(1);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["launch gate", () => gate.release()],
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ["paint observer", () => page.evaluate(() =>
        (window as unknown as { sequenceObserver?: MutationObserver }).sequenceObserver?.disconnect()
      )],
      ["created sessions", () => cleanupOwnedSteps(false, created.map((id) => [id, () => cleanupSession(request, id)]))],
      ["source", () => cleanupSession(request, source.id)],
      ["templates", async () => {
        for (const name of [partial, runnable]) {
          const removed = await request.delete(`/api/templates/${name}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }
      }],
    ]);
  }
});

/** Cancelling by ordinary typing must deliver that same key to the old agent.
 * Escape and programmatic modal focus instead cancel without creating anything.
 * The attention pill is centered on the terminal, including the Mac chrome
 * offset, but moves to the bottom while applying to an editable launcher. */
test("launch sequence cancellation leaves ordinary terminal input and modals in control", async ({ page, request }, testInfo) => {
  const source = await createSession(request, { title: `sequence-cancel-${Date.now()}` });
  try {
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator("#terminal .xterm-helper-textarea")).toBeFocused();
    const indicator = page.locator(".launch-sequence-indicator");
    const details = page.locator(".session-details");
    await expect(details, "the open session shows its details bar below the terminal").toBeVisible();
    await page.keyboard.press("Control+Shift+KeyX");
    await expect(indicator).toBeVisible();
    /** Compare the actual terminal bounds so the Mac's taller title band
     * cannot silently pull the pill above the terminal's center. */
    const expectCentered = async () => expect.poll(async () => {
      const pill = await indicator.boundingBox();
      const bar = await details.boundingBox();
      const terminal = await page.locator(".terminal-panes").boundingBox();
      if (!pill || !bar || !terminal) return `missing: pill ${!!pill}, bar ${!!bar}, terminal ${!!terminal}`;
      if (pill.y + pill.height > bar.y) return `pill bottom ${pill.y + pill.height} overlaps bar top ${bar.y}`;
      if (pill.y < terminal.y || pill.y + pill.height > terminal.y + terminal.height) return "pill outside the terminal area";
      if (Math.abs(pill.x + pill.width / 2 - terminal.x - terminal.width / 2) > 1) return "pill not horizontally centered";
      if (Math.abs(pill.y + pill.height / 2 - terminal.y - terminal.height / 2) > 1) return "pill not vertically centered";
      return "ok";
    }, { message: "the pill is centered inside the terminal and clear of the details bar" }).toBe("ok");
    await expectCentered();
    await expect(indicator).toHaveCSS("pointer-events", "none");
    await expect(indicator).toHaveCSS("opacity", "1");
    await testInfo.attach("launch-sequence-centered", { body: await page.screenshot(), contentType: "image/png" });
    // Emulate only the existing CSS class, not a native AppKit window.
    await page.locator(".app-shell").evaluate(node => node.classList.add("macos-window"));
    await expect(page.locator(".session-topbar")).toHaveCSS("min-height", "48px");
    await expectCentered();
    await page.locator(".app-shell").evaluate(node => node.classList.remove("macos-window"));
    await page.emulateMedia({ reducedMotion: "reduce" });
    await expect(indicator).toHaveCSS("animation-name", "none");
    const probe = `sequence-pass-${Date.now()}`;
    await page.keyboard.type(probe);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${probe}`);
    await expect(indicator).toHaveCount(0);
    await page.keyboard.press("Control+Shift+KeyX");
    await expect(indicator).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(indicator).toHaveCount(0);
    await page.keyboard.press("Control+Shift+KeyX");
    await expect(indicator).toBeVisible();
    // Mount and focus a modal fixture without dispatching any click or key.
    // A synthetic button.click() would cancel through the click listener and
    // could pass even if the modal-focus cancellation were missing.
    await page.evaluate(() => {
      const dialog = document.createElement("div");
      dialog.id = "sequence-modal-fixture";
      dialog.setAttribute("role", "dialog");
      dialog.setAttribute("aria-modal", "true");
      dialog.tabIndex = -1;
      dialog.textContent = "programmatic modal";
      document.body.append(dialog);
      dialog.focus();
    });
    await expect(page.locator("#sequence-modal-fixture")).toBeFocused();
    await expect(indicator).toHaveCount(0);
    await page.locator("#sequence-modal-fixture").evaluate((dialog) => dialog.remove());
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".launch-composer-search input")).toBeFocused();
    await page.keyboard.press("Control+Shift+KeyX");
    await expect(indicator).toContainText("Apply sequence");
    // Preserve the old fixed bottom reservation. WebKit can give the details
    // bar 31px rather than its 30px minimum, so an exact 14px gap is not its contract.
    await expect.poll(async () => {
      const pill = await indicator.boundingBox();
      const bar = await details.boundingBox();
      const main = await page.locator(".app-main").boundingBox();
      if (!pill || !bar || !main) return `missing: pill ${!!pill}, bar ${!!bar}, main ${!!main}`;
      const gap = bar.y - pill.y - pill.height;
      const reservation = main.y + main.height - pill.y - pill.height;
      if (gap > 0 && Math.abs(reservation - 44) < 1) return "ok";
      return `gap ${gap}, bar height ${bar.height}, bottom reservation ${reservation}`;
    }, { message: "the apply pill keeps its bottom position above the details bar" }).toBe("ok");
    await page.keyboard.press("Escape");
    await expect(indicator).toHaveCount(0);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
  } finally {
    await cleanupSession(request, source.id);
  }
});

/** The prefix's editing exception applies before a sequence starts, while an
 * unbound digit remains queued until Enter opens the existing binding hint. */
test("launch prefix keeps text fields and resolves an unbound digit only at Enter", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator('.launch-sequence-trigger[data-sequence-action="begin"]')).toBeEnabled();
  // All ordinary app editors currently sit in dialogs. A non-modal editor
  // fixture distinguishes the text-field exception from the modal guard.
  await page.evaluate(() => {
    const input = document.createElement("input");
    input.id = "sequence-editor-fixture";
    input.addEventListener("keydown", (event) => {
      if (event.code === "KeyX") input.dataset.prefixPassed = String(!event.defaultPrevented);
    });
    document.querySelector(".app-main")!.append(input);
    input.focus();
  });
  const editor = page.locator("#sequence-editor-fixture");
  await expect(editor).toBeFocused();
  await expect(page.locator('[role="dialog"][aria-modal="true"]:visible')).toHaveCount(0);
  await page.keyboard.press("Control+Shift+KeyX");
  await expect(editor).toHaveAttribute("data-prefix-passed", "true");
  await expect(page.locator(".launch-sequence-indicator")).toHaveCount(0);
  await editor.evaluate((input) => input.remove());
  await page.keyboard.press("Control+Shift+KeyX");
  await page.keyboard.press("Digit9");
  await expect(page.locator(".launch-sequence-indicator")).toBeVisible();
  await expect(page.locator(".templates-dialog")).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(page.locator(".templates-dialog")).toBeVisible();
  await expect(page.locator(".templates-dialog")).toContainText("number");
  await expect(page.locator(".launch-sequence-indicator")).toHaveCount(0);
});

/** A sequence's incomplete result needs one explanation regardless of how
 * many keys contributed. A disabled Launch and no create request distinguish
 * the intended editable draft from a refused automatic submission. */
for (const multiple of [false, true]) {
  test(`an incomplete ${multiple ? "multi-template" : "one-template"} sequence explains the combined setup`, async ({ page, request }) => {
    const names = [1, 2].map((key) => `sequence-partial-${Date.now()}-${key}`);
    let creates = 0;
    let bodyFailed = false;
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      creates++;
      await fulfillQuickFixture(route, request, 409, "incomplete sequence must not create");
    });
    try {
      for (const name of names) {
        const saved = await request.put(`/api/templates/${name}`, { data: { effort: "high" } });
        expect(saved.ok(), await saved.text()).toBe(true);
      }
      const listing = await request.get("/api/templates");
      expect(listing.ok(), await listing.text()).toBe(true);
      const templates = (await listing.json()).templates;
      for (const name of names) {
        expect(templates.find((entry: { name: string }) => entry.name === name)?.fields,
          "the partial fixture supplies neither an agent nor a command").toEqual({ effort: "high" });
      }
      await patchPreferences(request, { template_shortcuts: { "1": names[0], "2": names[1] } });
      expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": names[0], "2": names[1] });
      await page.goto("/");
      await expect(page.locator('.launch-sequence-trigger[data-sequence-action="begin"]')).toBeEnabled();
      await page.keyboard.press("Control+Shift+KeyX");
      await page.keyboard.press("Digit1");
      const indicator = page.locator(".launch-sequence-indicator");
      await expect(indicator).toContainText(names[0]);
      if (multiple) {
        await page.keyboard.press("Digit2");
        await expect(indicator).toContainText(names[1]);
      }
      await page.keyboard.press("Enter");
      const form = page.locator(".create-session-form");
      await expect(form).toBeVisible();
      await expect(form.locator(".partial-template-banner")).toHaveText("This sequence doesn't make a complete setup; complete it below.");
      await expect(form.locator(".create-session-submit")).toBeDisabled();
      expect(creates).toBe(0);
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["create route", () => page.unroute("**/api/sessions")],
        ...names.map((name): [string, () => Promise<void>] => [name, async () => {
          const removed = await request.delete(`/api/templates/${name}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }]),
      ]);
    }
  });
}

/** Enter captures the names the pill displayed, even if one definition was
 * deleted after it was queued. A named refusal in New distinguishes this from
 * an unbound key, which instead opens Templates with its binding hint. */
test("a sequence naming a deleted template reveals New with the missing name", async ({ page, request }) => {
  const names = ["missing", "remaining"].map((suffix) => `sequence-deleted-${Date.now()}-${suffix}`);
  let creates = 0;
  let bodyFailed = false;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates++;
    await fulfillQuickFixture(route, request, 409, "missing sequence template must not create");
  });
  try {
    for (const name of names) {
      const saved = await request.put(`/api/templates/${name}`, { data: { effort: "high" } });
      expect(saved.ok(), await saved.text()).toBe(true);
    }
    await patchPreferences(request, { template_shortcuts: { "1": names[0], "2": names[1] } });
    expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": names[0], "2": names[1] });
    await page.goto("/");
    await expect(page.locator('.launch-sequence-trigger[data-sequence-action="begin"]')).toBeEnabled();
    await page.keyboard.press("Control+Shift+KeyX");
    await page.keyboard.press("Digit1");
    await page.keyboard.press("Digit2");
    const indicator = page.locator(".launch-sequence-indicator");
    for (const name of names) await expect(indicator).toContainText(name);
    const removed = await request.delete(`/api/templates/${names[0]}`);
    expect(removed.ok(), await removed.text()).toBe(true);
    const listing = await request.get("/api/templates");
    expect(listing.ok(), await listing.text()).toBe(true);
    expect((await listing.json()).templates.map((entry: { name: string }) => entry.name)).not.toContain(names[0]);
    await page.keyboard.press("Enter");
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-error")).toContainText(`template ${names[0]} is no longer available`);
    await expect(page.locator(".templates-dialog")).toHaveCount(0);
    expect(creates).toBe(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["create route", () => page.unroute("**/api/sessions")],
      ...names.map((name): [string, () => Promise<void>] => [name, async () => {
        const removed = await request.delete(`/api/templates/${name}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }]),
    ]);
  }
});

/** A prefix refresh must replace this window's loaded bindings before Enter
 * captures the displayed names. The refused create proves which template was
 * applied without starting an agent or relying only on the pill's label. */
for (const bolt of [false, true]) {
  test(`a sequence honors a ${bolt ? "Bolt mark" : "number binding"} changed after this window loaded`, async ({ page, request }) => {
    const names = ["old", "new"].map((suffix) => `sequence-refresh-${Date.now()}-${suffix}`);
    const bodies: Record<string, any>[] = [];
    let bodyFailed = false;
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      bodies.push(route.request().postDataJSON());
      await fulfillQuickFixture(route, request, 409, "sequence refresh fixture refusal");
    });
    try {
      for (const name of names) await commandTemplate(request, name);
      await patchPreferences(request, { quick_launch_template: names[0], template_shortcuts: { "1": names[0] } });
      await page.goto("/");
      await expect(page.locator(".templates-button")).toHaveAttribute("data-tooltip", new RegExp(names[0]));
      await patchPreferences(request, bolt ? { quick_launch_template: names[1] } : { template_shortcuts: { "1": names[1] } });
      const fresh = await readPreferences(request);
      expect(bolt ? fresh.quick_launch_template : fresh.template_shortcuts?.["1"]).toBe(names[1]);
      await expect(page.locator('.launch-sequence-trigger[data-sequence-action="begin"]')).toBeEnabled();
      await page.keyboard.press("Control+Shift+KeyX");
      if (bolt) await chord(page);
      else await page.keyboard.press("Digit1");
      const indicator = page.locator(".launch-sequence-indicator");
      await expect(indicator).toContainText(names[1]);
      await expect(indicator).not.toContainText(names[0]);
      await page.keyboard.press("Enter");
      const form = page.locator(".create-session-form");
      await expect(form).toBeVisible();
      await expect(form.locator(".create-session-error")).toContainText("sequence refresh fixture refusal");
      expect(bodies).toHaveLength(1);
      expect(bodies[0]).toMatchObject({ title: `${names[1]}-session`, command: { command: FAKE_AGENT, yolo: false } });
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["create route", () => page.unroute("**/api/sessions")],
        ...names.map((name): [string, () => Promise<void>] => [name, async () => {
          const removed = await request.delete(`/api/templates/${name}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }]),
      ]);
    }
  });
}

/** The page copies the shown bindings at each sequence's Enter, and the app
 * pairs those copies with its keyboard launches purely by order. When the app
 * refuses a prefix that the page, rendered earlier, still showed as enabled,
 * that sequence's Enter must still retire its own copy. Before it did, the
 * copy stayed queued and the next sequence launched with it: here, the key-1
 * binding from before the change.
 *
 * The refusal is real: a quick launch whose preference read is held keeps the
 * app refusing new launches, and Begin's stale `disabled` is removed from the
 * DOM to stand in for the render that has not caught up yet. The app does
 * not patch the attribute back while its own value stays unchanged. Refused
 * creates name the launched template without starting an agent.
 *
 * The second launch's title alone would also pass if the first sequence
 * never left a copy behind (the page did not arm, or the app accepted Begin
 * after all). So the test asserts both premises: the page starts each
 * sequence with focus on the body, no sequence pill ever appears for the
 * refused one, and a wrapper around the page's snapshot hand-off records
 * every copy the app takes, in order: the bolt's (no bindings), the refused
 * Enter's old binding, then the second Enter's fresh one.
 */
test("a sequence refused at its prefix leaves later sequences their own bindings", async ({ page, request }) => {
  const prefix = `sequence-refused-${Date.now()}`;
  const [bolt, old, fresh] = ["bolt", "old", "new"].map((suffix) => `${prefix}-${suffix}`);
  const gate = routeGate();
  let holdRead = false;
  let held = 0;
  const bodies: Record<string, any>[] = [];
  let bodyFailed = false;
  // Hold exactly one preference read: the first one after the bolt chord arms
  // the hold, which is that press's own fresh read.
  await page.route("**/api/preferences", async (route) => {
    if (route.request().method() !== "GET" || !holdRead) return route.continue();
    holdRead = false;
    held++;
    await gate.wait();
    await route.fulfill({ response: await route.fetch() });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    bodies.push(route.request().postDataJSON());
    await fulfillQuickFixture(route, request, 409, "refused sequence fixture refusal");
  });
  await observeFeedReaders(page);
  try {
    for (const name of [bolt, old, fresh]) await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: bolt, template_shortcuts: { "1": old } });
    await page.goto("/");
    const begin = page.locator('.launch-sequence-trigger[data-sequence-action="begin"]');
    const trigger = page.locator(".quick-launch-trigger");
    const indicator = page.locator(".launch-sequence-indicator");
    const form = page.locator(".create-session-form");
    await expect(begin).toBeEnabled();
    // The first Enter copies these bindings; the old pairing replayed them.
    await expect(page.locator(".launch-sequence-bindings")).toHaveAttribute("data-bindings", new RegExp(old));
    // Record each snapshot the app takes from the page, in order, as the key-1
    // binding it copied (null for a direct press, which copies none). The
    // installer assigns the hand-off once and the app calls it by name each
    // time, so wrapping it here sees every take.
    await expect.poll(() =>
      page.evaluate(() =>
        typeof (window as unknown as { farhelmTakeTemplateShortcutPress?: unknown }).farhelmTakeTemplateShortcutPress
      )
    ).toBe("function");
    await page.evaluate(() => {
      const root = window as unknown as {
        farhelmTakeTemplateShortcutPress: () => { sequence_bindings: string | null } | undefined;
        takenBindings: (string | null)[];
      };
      const take = root.farhelmTakeTemplateShortcutPress;
      root.takenBindings = [];
      root.farhelmTakeTemplateShortcutPress = () => {
        const snapshot = take();
        root.takenBindings.push(
          snapshot === undefined
            ? "no snapshot"
            : snapshot.sequence_bindings === null
            ? null
            : JSON.parse(snapshot.sequence_bindings).template_shortcuts?.["1"] ?? "unbound",
        );
        return snapshot;
      };
    });
    const taken = () => page.evaluate(() => (window as unknown as { takenBindings: (string | null)[] }).takenBindings);
    // Startup readers, automatic selection included, read preferences too.
    // Only the bolt press's read may meet the hold.
    await waitForFeedReadersSettled(page);
    holdRead = true;
    await chord(page);
    await expect.poll(() => held, { message: "the bolt press's preference read is held" }).toBe(1);
    const opening = await trigger.getAttribute("data-opening-generation");
    expect(opening).toBeTruthy();
    // Premises: the held read is the bolt's (its continuation has not run),
    // and the app's render already refuses Begin.
    expect(await trigger.getAttribute("data-read-completed-generation")).not.toBe(opening);
    await expect(begin).toBeDisabled();
    await begin.evaluate((node) => node.removeAttribute("disabled"));
    await expect(begin).toBeEnabled();
    // The prefix yields to a focused text field. If the page did not arm, it
    // would copy nothing at Enter and leave nothing behind, and the test
    // would pass without the refusal it exists for.
    await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
    await expect.poll(() => page.evaluate(() => document.activeElement === document.body)).toBe(true);
    // The app's refusal of Begin is otherwise only inferred from the held
    // read. A sequence pill is its observable opposite: the app accepted the
    // prefix. Watch for one until the refused Enter's copy is drained.
    await expect(indicator).toHaveCount(0);
    await page.evaluate(() => {
      const root = window as unknown as { refusedPillShown: boolean; refusedPillObserver: MutationObserver };
      root.refusedPillShown = false;
      root.refusedPillObserver = new MutationObserver(() => {
        if (document.querySelector(".launch-sequence-indicator")) root.refusedPillShown = true;
      });
      root.refusedPillObserver.observe(document.body, { childList: true, subtree: true });
    });

    await page.keyboard.press("Control+Shift+KeyX");
    await page.keyboard.press("Digit1");
    await page.keyboard.press("Enter");
    // The bolt press cannot finish before the release below, so these keys'
    // relay clicks, already dispatched, meet the app's refusal. A Begin the
    // app accepted instead would launch the old binding and show up in
    // `bodies` below.
    expect(await trigger.getAttribute("data-read-completed-generation")).not.toBe(opening);
    expect(
      bodies,
      "the bolt press's own read was held (a bolt POST here means another reader met the hold)",
    ).toHaveLength(0);

    gate.release();
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-error")).toContainText("refused sequence fixture refusal");
    expect(bodies.map((body) => body.title), "only the bolt press launched").toEqual([`${bolt}-session`]);
    // The worker reaches the refused Enter's entry only after the bolt's, so
    // poll. Before the fix that entry never existed and the old copy stayed
    // on the page, so this times out at [null].
    await expect.poll(taken, { message: "the refused Enter's copy is taken and dropped" }).toEqual([null, old]);
    expect(
      await page.evaluate(() => {
        const root = window as unknown as { refusedPillShown: boolean; refusedPillObserver: MutationObserver };
        root.refusedPillObserver.disconnect();
        return root.refusedPillShown;
      }),
      "the app refused Begin: no sequence pill appeared",
    ).toBe(false);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);

    await patchPreferences(request, { template_shortcuts: { "1": fresh } });
    expect((await readPreferences(request)).template_shortcuts?.["1"]).toBe(fresh);
    // Begin's attribute was edited by hand; the untouched bolt relay shares
    // its admission rule and says when the app accepts a prefix again.
    await expect(trigger).toBeEnabled();
    // The prefix yields to a focused text field, and cancelling New may hand
    // focus anywhere. Start this sequence from the page, as the first one did.
    await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
    await expect.poll(() => page.evaluate(() => document.activeElement === document.body)).toBe(true);
    await page.keyboard.press("Control+Shift+KeyX");
    await page.keyboard.press("Digit1");
    await expect(indicator).toContainText(fresh);
    await expect(indicator).not.toContainText(old);
    await page.keyboard.press("Enter");
    await expect.poll(() => bodies.length, { message: "the second sequence launches once" }).toBe(2);
    expect(bodies[1].title, "the second sequence launches its own Enter's binding").toBe(`${fresh}-session`);
    await expect(form.locator(".create-session-error")).toContainText("refused sequence fixture refusal");
    expect(bodies).toHaveLength(2);
    expect(await taken(), "every copy was taken once, in press order").toEqual([null, old, fresh]);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["preference gate", () => gate.release()],
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ["pill observer", () => page.evaluate(() =>
        (window as unknown as { refusedPillObserver?: MutationObserver }).refusedPillObserver?.disconnect()
      )],
      ...[bolt, old, fresh].map((name): [string, () => Promise<void>] => [name, async () => {
        const removed = await request.delete(`/api/templates/${name}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }]),
    ]);
  }
});

/** One unbound number refuses the whole sequence, but its bound Bolt must
 * not be mistaken for an absent mark. The specific number hint establishes
 * settlement before asserting the misleading mark hint is absent. */
test("a bound Bolt and unbound number show only the number binding hint", async ({ page, request }) => {
  const name = `sequence-bound-bolt-${Date.now()}`;
  let creates = 0;
  let bodyFailed = false;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates++;
    await fulfillQuickFixture(route, request, 409, "unbound sequence number must not create");
  });
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name, template_shortcuts: {} });
    const preferences = await readPreferences(request);
    expect(preferences.quick_launch_template).toBe(name);
    expect(preferences.template_shortcuts ?? {}).toEqual({});
    await page.goto("/");
    await expect(page.locator('.launch-sequence-trigger[data-sequence-action="begin"]')).toBeEnabled();
    await page.keyboard.press("Control+Shift+KeyX");
    await chord(page);
    await page.keyboard.press("Digit9");
    await expect(page.locator(".launch-sequence-indicator")).toContainText(name);
    await page.keyboard.press("Enter");
    const dialog = page.locator(".templates-dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".templates-number-hint")).toContainText("Give a template a number key");
    await expect(dialog.locator(".templates-quick-hint")).toHaveCount(0);
    await expect(page.locator(".create-session-form")).toHaveCount(0);
    expect(creates).toBe(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["create route", () => page.unroute("**/api/sessions")],
      [name, async () => {
        const removed = await request.delete(`/api/templates/${name}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});

/** A completed preparation read belongs to its opening, even if the user
 * opens and cancels New while the read is held. The diagnostic generation
 * proves that the old continuation finished; absence alone is not readiness. */
for (const takeover of ["New", "settings", "Rename"]) {
  test(
    takeover !== "New"
      ? `closing ${takeover} retires an older quick-launch read`
      : "cancelling a newer New draft retires an older quick-launch read",
    async ({ page, request }) => {
      const name = `quick-read-cancel-${Date.now()}`;
      const source = takeover === "Rename" ? await createSession(request, { title: `${name}-source` }) : null;
      const gate = routeGate();
      let reads = 0;
      let held = 0;
      let creates = 0;
      await page.route("**/api/templates", async (route) => {
        if (route.request().method() !== "GET" || ++reads !== 1) return route.continue();
        held++;
        await gate.wait();
        await route.fulfill({ response: await route.fetch() });
      });
      await page.route("**/api/sessions", async (route) => {
        if (route.request().method() === "POST") creates++;
        await route.continue();
      });
      try {
        await commandTemplate(request, name);
        await patchPreferences(request, { quick_launch_template: name });
        await page.goto("/");
        await chord(page);
        await expect.poll(() => held).toBe(1);
        const trigger = page.locator(".quick-launch-trigger");
        const opening = await trigger.getAttribute("data-opening-generation");
        expect(opening).toBeTruthy();
        const form = page.locator(".create-session-form");
        if (takeover === "settings") {
          await page.getByRole("button", { name: "settings", exact: true }).click();
          const dialog = page.getByRole("dialog", { name: "settings", exact: true });
          await expect(dialog).toBeVisible();
          await dialog.getByRole("button", { name: "close settings", exact: true }).click();
          await expect(dialog).toHaveCount(0);
        } else if (source) {
          const row = page.locator(`[data-session-id="${source.id}"]`);
          await expect(row).toBeVisible();
          await openRowMenu(row);
          await row.locator(".session-row-rename").click();
          const dialog = page.locator(".rename-dialog");
          await expect(dialog.locator(".rename-input")).toBeFocused();
          await dialog.locator(".rename-cancel").click();
          await expect(dialog).toHaveCount(0);
        } else {
          await page.locator(".new-session-button").click();
          await expect(form).toBeVisible();
          await form.getByRole("button", { name: "cancel", exact: true }).click();
        }
        await expect(form).toHaveCount(0);
        gate.release();
        await expect(trigger).toHaveAttribute("data-read-completed-generation", opening!);
        await expect(form).toHaveCount(0);
        expect(creates).toBe(0);
      } finally {
        gate.release();
        await page.unroute("**/api/templates");
        await page.unroute("**/api/sessions");
        if (source) await cleanupSession(request, source.id);
        await request.delete(`/api/templates/${name}`);
      }
    },
  );
}

/** Background keyboard input cannot activate Replace with while New is
 * hidden. A callback already admitted before isolation may still arrive, so
 * explicitly deliver that callback and require ordinary replacement consent. */
test("hidden preparation isolates keys and a queued Replace with requires Launch", async ({ page, request }) => {
  const name = `quick-replace-fence-${Date.now()}`;
  const source = await createSession(request, { title: `${name}-source` });
  const gate = routeGate();
  let catalogReads = 0;
  const bodies: any[] = [];
  await page.route("**/api/launch-catalog", async (route) => {
    catalogReads++;
    await gate.wait();
    await route.fulfill({ response: await route.fetch() });
  });
  await page.route("**/api/sessions**", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    bodies.push(route.request().postDataJSON());
    expect(new URL(route.request().url()).pathname).toBe(`/api/sessions/${encodeURIComponent(source.id)}/replace`);
    await fulfillQuickFixture(route, request, 409, "replacement fixture refusal");
  });
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const replace = page.locator(".header-replace-with");
    await expect(replace).toBeVisible();
    await expect(page.locator("#terminal .xterm-helper-textarea")).toBeFocused();
    await chord(page);
    await expect.poll(() => catalogReads).toBeGreaterThan(0);
    const form = page.locator(".create-session-form");
    await expect(form).toBeHidden();
    await expect.poll(() => replace.evaluate((node) => !!node.closest("[inert]")), {
      message: "the hidden form owns background keyboard isolation",
    }).toBe(true);
    await page.keyboard.press("Tab");
    await page.keyboard.press("Enter");
    expect(bodies).toHaveLength(0);
    await expect(form).toBeHidden();
    // Programmatic delivery models a callback queued before the modal became
    // inert; it bypasses pointer hit-testing without bypassing the real handler.
    await replace.evaluate((node: HTMLButtonElement) => node.click());
    await expect(form).toBeVisible();
    gate.release();
    await expect(form.getByLabel("name (optional)")).toHaveValue(`${name}-source`);
    await expect(form.locator(".create-session-submit")).toContainText("replace");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    expect(bodies).toHaveLength(0);
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".create-session-error")).toContainText("replacement fixture refusal");
    expect(bodies).toHaveLength(1);
  } finally {
    gate.release();
    await page.unroute("**/api/launch-catalog");
    await page.unroute("**/api/sessions**");
    await cleanupSession(request, source.id);
    await request.delete(`/api/templates/${name}`);
  }
});

/** Failed readiness is a settled result, not a pending launch. Recovery
 * applies the saved template to the revealed draft but cannot revive automatic
 * submission; a controlled feed names the host-read recovery boundary. */
for (const recover of [false, true]) {
  test(
    recover
      ? "an initial host-read failure reveals New and recovery stays manual"
      : "a host-read fallback permits keyboard cancellation before recovery",
    async ({ page, request }) => {
      const name = `quick-host-read-${Date.now()}`;
      const feed = await stubFeed(page);
      let failing = true;
      let creates = 0;
      await page.route("**/api/hosts", async (route) => {
        // Fail this reader's decoding without taking down the helm transport or
        // its feed, which independently supplies the recovery notification.
        if (failing) {
          await fulfillQuickFixture(
            route,
            request,
            200,
            JSON.stringify({ hosts: "quick host-list fixture failure" }),
            "application/json",
          );
        } else await route.continue();
      });
      await page.route("**/api/sessions", async (route) => {
        if (route.request().method() === "POST") creates++;
        await route.continue();
      });
      try {
        await commandTemplate(request, name);
        await patchPreferences(request, { quick_launch_template: name });
        await page.goto("/");
        await feed.waitForConnection(1);
        await openHostsPanel(page);
        await expect(page.locator(".hosts-load-error")).toContainText("error decoding response body");
        await chord(page);
        const form = page.locator(".create-session-form");
        await expect(form).toBeVisible();
        await expect(form.locator(".create-session-error")).toContainText("the host list failed");
        await expect(form.getByRole("button", { name: "cancel", exact: true })).toBeEnabled();
        await expect(form.getByRole("button", { name: "cancel", exact: true })).toBeFocused();
        expect(creates).toBe(0);
        if (!recover) {
          await page.keyboard.press("Tab");
          await expect.poll(() => form.evaluate((node) => node.contains(document.activeElement))).toBe(true);
          await page.keyboard.press("Enter");
          await expect(form).toHaveCount(0);
          expect(creates).toBe(0);
          return;
        }
        failing = false;
        await expect.poll(() => feed.openSockets(), { message: "the owned feed is live before recovery notification" })
          .toBeGreaterThan(0);
        feed.notify(2);
        await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
        await expect(form.locator(".create-session-submit")).toBeEnabled();
        await expect(form).toBeVisible();
        expect(creates).toBe(0);
        await form.getByRole("button", { name: "cancel", exact: true }).click();
      } finally {
        await page.unroute("**/api/hosts");
        await page.unroute("**/api/sessions");
        await request.delete(`/api/templates/${name}`);
      }
    },
  );
}

/** Fresh shared reads may replace acknowledged state, but they cannot undo
 * this window's explicit dirty choice. Hold the PUT before it reaches the helm
 * (or refuse it), then close Templates and observe the actual create payload.
 * A held save also fences the shortcut's read until released; once acknowledged,
 * only the newly persisted mark can decide which template launches. */
for (const failed of [false, true]) {
  test(`quick launch honors a ${failed ? "failed" : "held"} local mark over the shared read`, async ({ page, request }) => {
    const prefix = `quick-local-mark-${Date.now()}`;
    const [old, chosen] = ["old", "chosen"].map((suffix) => `${prefix}-${suffix}`);
    const gate = routeGate();
    let held = 0;
    let settled = 0;
    let preferenceReads = 0;
    // Arrival order at the route: "read" for a preference read, "save-reply"
    // when the held mark save starts answering the page.
    const events: string[] = [];
    const bodies: any[] = [];
    await page.route("**/api/preferences", async (route) => {
      if (route.request().method() === "GET") {
        preferenceReads++;
        events.push("read");
      }
      if (route.request().method() !== "PUT" || route.request().postDataJSON().quick_launch_template !== chosen) {
        return route.continue();
      }
      held++;
      if (failed) {
        await fulfillQuickFixture(route, request, 503, "local mark fixture refusal");
        settled++;
      } else {
        await gate.wait();
        const response = await route.fetch();
        events.push("save-reply");
        await route.fulfill({ response });
      }
    });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      bodies.push(route.request().postDataJSON());
      await fulfillQuickFixture(route, request, 409, "local mark launch fixture refusal");
    });
    try {
      await commandTemplate(request, old);
      await commandTemplate(request, chosen);
      await patchPreferences(request, { quick_launch_template: old });
      await page.goto("/");
      await page.locator(".templates-button").click();
      const dialog = page.getByRole("dialog", { name: "templates", exact: true });
      await expect(dialog.getByRole("button", { name: `quick launch ${old}`, exact: true })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
      const mark = dialog.getByRole("button", { name: `quick launch ${chosen}`, exact: true });
      await mark.click();
      await expect(mark).toHaveAttribute("aria-pressed", "true");
      await expect.poll(() => held).toBe(1);
      if (failed) await expect.poll(() => settled).toBe(1);
      expect((await readPreferences(request)).quick_launch_template).toBe(old);
      await dialog.locator(".templates-close").click();
      await expect(dialog).toHaveCount(0);
      const readsBeforeShortcut = preferenceReads;
      const eventsBeforeShortcut = events.length;
      const trigger = page.locator(".quick-launch-trigger");
      const previousOpening = Number(await trigger.getAttribute("data-opening-generation"));
      await chord(page);
      if (!failed) {
        await expect.poll(async () => Number(await trigger.getAttribute("data-opening-generation"))).toBeGreaterThan(previousOpening);
        // sleep-ok: observe whether a shortcut admitted during the held save dispatches its dependent read.
        await page.waitForTimeout(150);
        expect(preferenceReads, "the shortcut read cannot overtake the held mark save").toBe(readsBeforeShortcut);
        expect(bodies).toHaveLength(0);
        gate.release();
        // The shortcut's read does happen, and only once the save is
        // answering: without the ordering it would have been sent while the
        // save was still held, ahead of "save-reply".
        await expect.poll(() => events.slice(eventsBeforeShortcut)).toContain("read");
        const after = events.slice(eventsBeforeShortcut);
        expect(after[0], `the shortcut's read waited for the save: ${after}`).toBe("save-reply");
      }
      const form = page.locator(".create-session-form");
      await expect(form).toBeVisible();
      await expect(form.locator(".create-session-error")).toContainText("local mark launch fixture refusal");
      expect(bodies).toHaveLength(1);
      expect(bodies[0].title).toBe(`${chosen}-session`);
      await expect(page.locator(".templates-button")).toHaveAttribute("data-tooltip", new RegExp(chosen));
    } finally {
      gate.release();
      await page.unrouteAll({ behavior: "wait" });
      for (const name of [old, chosen]) await request.delete(`/api/templates/${name}`);
    }
  });
}

/** A preparation refusal has no host ownership. Binding the ordinary New
 * defaults must keep its explanation, rather than clearing it as an error
 * from a previous target. Only the shortcut's preference read fails: reads stay
 * live until the page's startup readers, automatic selection included, have
 * settled, since that selection reads preferences too while the page loads. */
test("a quick preparation error survives New's first host binding", async ({ page, request }) => {
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local?.identity).toBeTruthy();
  let failReads = false;
  let failed = 0;
  await page.route("**/api/preferences", async (route) => {
    if (route.request().method() !== "GET" || !failReads) return route.continue();
    failed++;
    await fulfillQuickFixture(route, request, 503, "quick preference-read fixture failure");
  });
  await observeFeedReaders(page);
  try {
    await page.goto("/");
    await openHostsPanel(page);
    await expect(page.locator(`[data-host-id="${local!.id}"] .host-name`)).toBeVisible();
    await waitForFeedReadersSettled(page);
    failReads = true;
    await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.getByRole("combobox", { name: "host", exact: true })).toHaveAttribute(
      "data-host-id",
      String(local!.id),
    );
    await expect(form).not.toHaveAttribute("data-create-target-fingerprint", "");
    await expect(form.locator(".create-session-error")).toContainText("couldn't prepare quick launch");
    await expect(form.locator(".create-session-error")).toContainText("quick preference-read fixture failure");
    expect(failed).toBe(1);
  } finally {
    await page.unroute("**/api/preferences");
  }
});

/** The old target's refusal must not describe a newly retargeted host, but
 * the admitted attempt still ends visibly. The form's bound fingerprint is
 * the readiness oracle; only the client's host read is retargeted, preserving
 * the real owned supervisor and its sessions. */
test("a refusal after target change reveals the spent quick-launch draft", async ({ page, request }) => {
  const name = `quick-target-refusal-${Date.now()}`;
  const feed = await stubFeed(page);
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local?.identity).toBeTruthy();
  const gate = routeGate();
  let moved = false;
  let creates = 0;
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    if (moved) {
      const host = body.hosts.find((row: any) => row.id === local!.id);
      host.destination = "fixture-retarget";
      host.identity = "fixture-replaced-install";
      host.state = { ...host.state, identity: host.identity };
    }
    await route.fulfill({ response, json: body });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates++;
    await gate.wait();
    await fulfillQuickFixture(route, request, 409, "old-target fixture refusal");
  });
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    await page.goto("/");
    await feed.waitForConnection(1);
    await chord(page);
    await expect.poll(() => creates, { message: "the owned create refusal is held" }).toBe(1);
    const form = page.locator(".create-session-form");
    await expect(form).toBeHidden();
    const original = await form.getAttribute("data-create-target-fingerprint");
    expect(original).toBeTruthy();
    moved = true;
    feed.notify(2);
    await expect(form).not.toHaveAttribute("data-create-target-fingerprint", original!);
    await expect(form).not.toHaveAttribute("data-create-target-fingerprint", "");
    gate.release();
    await expect(form).toBeVisible();
    await expect(form.locator(".launch-composer-refusal")).toHaveText("a launch for an earlier setup was refused");
    await expect(form).not.toContainText("old-target fixture refusal");
    await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
    expect(creates).toBe(1);
  } finally {
    gate.release();
    await page.unroute("**/api/hosts");
    await page.unroute("**/api/sessions");
    await request.delete(`/api/templates/${name}`);
  }
});

for (const movedAway of [false, true]) {
  /** A completed other-window mark change must outrank a sign-in seed during
   * rename. These are sequential operations; concurrent changes during the
   * rename remain the ordinary shared-field last-writer-wins contract. */
  test(`rename reads a mark ${movedAway ? "moved away" : "added"} in another window`, async ({ page, request }) => {
    const prefix = `quick-rename-fresh-${Date.now()}`;
    const [old, renamed, other] = ["old", "renamed", "other"].map((s) => `${prefix}-${s}`);
    try {
      await commandTemplate(request, old);
      await commandTemplate(request, other);
      if (movedAway) await patchPreferences(request, { quick_launch_template: old });
      await page.goto("/");
      await page.locator(".templates-button").click();
      const dialog = page.getByRole("dialog", { name: "templates", exact: true });
      await expect(dialog.getByRole("button", { name: `quick launch ${old}`, exact: true })).toHaveAttribute(
        "aria-pressed",
        String(movedAway),
      );
      await dialog.getByText(old, { exact: true }).click();
      await dialog.locator(".templates-name").fill(renamed);
      await patchPreferences(request, { quick_launch_template: movedAway ? other : old });
      expect((await readPreferences(request)).quick_launch_template).toBe(movedAway ? other : old);
      await dialog.getByRole("button", { name: "save", exact: true }).click();
      await expect(dialog.getByText(renamed, { exact: true })).toBeVisible();
      await expect(dialog.getByText(old, { exact: true })).toHaveCount(0);
      expect((await readPreferences(request)).quick_launch_template).toBe(movedAway ? other : renamed);
      await expect(dialog.locator('.templates-quick-mark[aria-pressed="true"]')).toHaveCount(1);
    } finally {
      for (const name of [old, renamed, other]) await request.delete(`/api/templates/${name}`);
    }
  });
}

/** The switcher preserves its first recent selection and releases the modal
 * before direct launch. Its cached label is only advertising: a mark added
 * from another client after mount still controls the action's fresh read. */
test("the quick switcher row launches through the fresh direct path", async ({ page, request }) => {
  const name = `quick-switch-row-${Date.now()}`;
  const source = await createSession(request, { title: `${name}-source` });
  const created: string[] = [];
  const gate = routeGate();
  let posts = 0;
  let bodyFailed = false;
  try {
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      posts++;
      await gate.wait();
      const response = await route.fetch();
      if (response.ok()) created.push((await response.json()).id);
      await route.fulfill({ response });
    });
    await commandTemplate(request, name);
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await expect(page.locator(".quick-switcher-trigger")).toHaveCount(1);
    await page.keyboard.press("Control+Shift+k");
    const dialog = page.getByRole("dialog", { name: "quick switcher" });
    await expect(dialog.getByRole("combobox")).toBeFocused();
    const quick = dialog.locator(".quick-switcher-quick-launch");
    await expect(quick).toContainText("mark a template");
    await expect(dialog.getByRole("option").first()).toHaveAttribute("aria-selected", "true");
    await expect(quick).toHaveAttribute("aria-selected", "false");
    await patchPreferences(request, { quick_launch_template: name });
    await quick.click();
    await expect.poll(() => posts, { message: "the quick-switcher create request is held" }).toBe(1);
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".create-session-form")).toBeHidden();
    gate.release();
    await expect.poll(() => created.length).toBe(1);
    await expect(page.locator(`[data-session-id="${created[0]}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(".create-session-form")).toHaveCount(0);
    await namingField(page, `${name}-session`);
    // A switcher opening and the following naming opening own different
    // modals. Cancel must return directly to the newly selected terminal.
    await page.locator(".rename-dialog .rename-cancel").click();
    await echoAfterNaming(page, created[0], `switcher-named-${Date.now()}`);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["launch gate", () => gate.release()],
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      // Draining the released route first lets an in-flight create publish
      // its ID before this callback enumerates the sessions it must release.
      ["created sessions", () => cleanupOwnedSteps(false, created.map((id) => [id, () => cleanupSession(request, id)]))],
      ["source session", () => cleanupSession(request, source.id)],
      ["template", async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});

for (const [mac, numbered] of [[false, false], [true, false], [false, true], [true, true]]) {
  /** The held create reply establishes the hidden in-flight boundary. Actual
   * binary input frames prove that both the chord and naming stay away from
   * the agents; output while naming and a later echo prove startup and handoff. */
  test(`${mac ? "Mac" : "non-Mac"} ${numbered ? "number shortcut" : "quick launch"} owns input and submits once without showing New`, async ({ page, request }) => {
    await platform(page, mac);
    const source = await createSession(request, { title: `quick-source-${Date.now()}` });
    const name = `quick-direct-${Date.now()}`;
    const created: string[] = [];
    const binary: Buffer[] = [];
    const gate = routeGate();
    const bodies: any[] = [];
    const renames: { path: string; body: unknown }[] = [];
    let bodyFailed = false;
    try {
      page.on("request", (request) => {
        const path = new URL(request.url()).pathname;
        if (request.method() === "POST" && path.endsWith("/rename")) {
          renames.push({ path, body: request.postDataJSON() });
        }
      });
      page.on("websocket", (socket) => {
        if (new URL(socket.url()).pathname.endsWith("/term")) {
          socket.on("framesent", ({ payload }) => {
            if (Buffer.isBuffer(payload)) binary.push(payload);
          });
        }
      });
      await page.route("**/api/sessions", async (route) => {
        if (route.request().method() !== "POST") return route.continue();
        bodies.push(route.request().postDataJSON());
        await gate.wait();
        const response = await route.fetch();
        if (response.ok()) created.push((await response.json()).id);
        await route.fulfill({ response });
      });
      await commandTemplate(request, name);
      await pinAutoSelect(page, source.id);
      await page.goto("/");
      await attachSession(page, source.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await expect(page.locator("#terminal .xterm-helper-textarea")).toBeFocused();
      // A later hidden snapshot would miss a launcher flashed before submit.
      // Retain one bounded observation bit across every actual DOM transition.
      await page.evaluate(() => {
        const root = window as unknown as { quickLauncherPainted: boolean; quickPaintObserver: MutationObserver };
        root.quickLauncherPainted = false;
        root.quickPaintObserver = new MutationObserver(() => {
          if (document.querySelector(".create-session-form")?.getClientRects().length) root.quickLauncherPainted = true;
        });
        root.quickPaintObserver.observe(document.body, { childList: true, subtree: true, attributes: true });
      });
      // The page was seeded without a mark: only a fresh shortcut read can
      // discover this other client's change without a reload.
      await patchPreferences(request, numbered ? { template_shortcuts: { "1": name } } : { quick_launch_template: name });
      if (numbered) await numberChord(page, "1", true);
      else await chord(page, mac, true);
      await expect.poll(() => bodies.length, { message: "the owned create request is captured and held" }).toBe(1);
      const form = page.locator(".create-session-form");
      await expect(form).toHaveCount(1);
      await expect(form).toBeHidden();
      await expect(page.locator(".rename-dialog")).toHaveCount(0);
      expect(bodies).toHaveLength(1);
      expect(bodies[0]).toMatchObject({
        cwd: source.cwd,
        title: `${name}-session`,
        command: { command: FAKE_AGENT, yolo: false },
      });
      expect(Buffer.concat(binary).toString(), "the captured chord and repeat reach no agent input frame").toBe("");
      const response = page.waitForResponse((r) =>
        r.request().method() === "POST" && new URL(r.url()).pathname === "/api/sessions"
      );
      gate.release();
      const accepted = await response;
      expect(accepted.ok(), await accepted.text()).toBe(true);
      const session = await accepted.json();
      await expect(form).toHaveCount(0);
      await expect(page.locator(`[data-session-id="${session.id}"]`)).toHaveAttribute("data-session-selected", "true");
      const field = await namingField(page, `${name}-session`);
      await agentStartedWhileNaming(page, session.id, field);
      const changed = `${name}-renamed`;
      // One changed Enter, one unchanged Enter, Escape and Cancel exercise
      // the distinct exits without multiplying the platform/source matrix.
      if (numbered || mac) {
        await page.keyboard.type(changed);
        await expect(field).toHaveValue(changed);
      }
      expect(Buffer.concat(binary).toString(), "typing a name reaches no terminal input frame").toBe("");
      if (mac && numbered) await page.locator(".rename-dialog .rename-cancel").click();
      else await page.keyboard.press(mac ? "Escape" : "Enter");
      const probe = `quick-live-${Date.now()}`;
      await echoAfterNaming(page, session.id, probe);
      const expectedTitle = numbered && !mac ? changed : `${name}-session`;
      const stored = await request.get(`/api/sessions/${encodeURIComponent(session.id)}`);
      expect(stored.ok(), await stored.text()).toBe(true);
      expect((await stored.json()).title).toBe(expectedTitle);
      expect(renames).toEqual(numbered && !mac
        ? [{ path: `/api/sessions/${encodeURIComponent(session.id)}/rename`, body: { title: changed } }]
        : []);
      expect(bodies).toHaveLength(1);
      expect(Buffer.concat(binary).toString()).toBe(`${probe}\r`);
      expect(await page.evaluate(() => (window as unknown as { quickLauncherPainted: boolean }).quickLauncherPainted))
        .toBe(false);
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["launch gate", () => gate.release()],
        ["routes", () => page.unrouteAll({ behavior: "wait" })],
        ["paint observer", () => page.evaluate(() =>
          (window as unknown as { quickPaintObserver?: MutationObserver }).quickPaintObserver?.disconnect()
        )],
        // Enumerate after route draining; a released create can still add
        // its session ID while teardown is waiting on that handler.
        ["created sessions", () => cleanupOwnedSteps(false, created.map((id) => [id, () => cleanupSession(request, id)]))],
        ["source session", () => cleanupSession(request, source.id)],
        ["template", async () => {
          const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }],
      ]);
    }
  });
}

/** Later naming openings must focus and select again in the same document.
 * The first close uses no replay wait; this does not prove how far replay has
 * progressed. The second Save covers submission after observed startup. */
test("repeated naming selects the title and returns focus after immediate Enter and Save", async ({ page, request }) => {
  const name = `quick-repeat-name-${Date.now()}`;
  const created: string[] = [];
  const renames: { path: string; body: unknown }[] = [];
  let bodyFailed = false;
  try {
    page.on("request", (request) => {
      const path = new URL(request.url()).pathname;
      if (request.method() === "POST" && path.endsWith("/rename")) renames.push({ path, body: request.postDataJSON() });
    });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const response = await route.fetch();
      if (response.ok()) created.push((await response.json()).id);
      await route.fulfill({ response });
    });
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    await page.goto("/");
    await chord(page);
    await expect.poll(() => created.length).toBe(1);
    await namingField(page, `${name}-session`);
    // Do not require terminal reveal before accepting the default. The
    // following identity-aware wait still requires the actual focus handoff.
    await page.keyboard.press("Enter");
    await echoAfterNaming(page, created[0], `immediate-name-${Date.now()}`);
    expect(renames).toEqual([]);
    await chord(page);
    await expect.poll(() => created.length).toBe(2);
    const field = await namingField(page, `${name}-session`);
    await agentStartedWhileNaming(page, created[1], field);
    const title = `${name}-saved`;
    await page.keyboard.type(title);
    await expect(field).toHaveValue(title);
    await page.locator(".rename-dialog .rename-submit").click();
    await echoAfterNaming(page, created[1], `saved-name-${Date.now()}`);
    expect(renames).toEqual([{ path: `/api/sessions/${encodeURIComponent(created[1])}/rename`, body: { title } }]);
    const stored = await request.get(`/api/sessions/${encodeURIComponent(created[1])}`);
    expect(stored.ok(), await stored.text()).toBe(true);
    expect((await stored.json()).title).toBe(title);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ["created sessions", () => cleanupOwnedSteps(false, created.map((id) => [id, () => cleanupSession(request, id)]))],
      ["template", async () => {
        expect((await request.delete(`/api/templates/${encodeURIComponent(name)}`)).ok()).toBe(true);
      }],
    ]);
  }
});

/** Turning naming off preserves direct terminal ownership after a quick launch.
 * A live echo pins the successful handoff, so absence of a dialog cannot pass
 * merely because launch or selection failed. Other tests keep the default on. */
test("the naming preference off launches directly into the terminal", async ({ page, request }) => {
  const name = `quick-name-off-${Date.now()}`;
  const created: string[] = [];
  let bodyFailed = false;
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name, name_session_on_quick_launch: false });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const response = await route.fetch();
      if (response.ok()) created.push((await response.json()).id);
      await route.fulfill({ response });
    });
    await page.goto("/");
    await chord(page);
    await expect.poll(() => created.length).toBe(1);
    await echoAfterNaming(page, created[0], `naming-off-${Date.now()}`);
    const stored = await request.get(`/api/sessions/${encodeURIComponent(created[0])}`);
    expect(stored.ok(), await stored.text()).toBe(true);
    expect((await stored.json()).title).toBe(`${name}-session`);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ["created sessions", () => cleanupOwnedSteps(false, created.map((id) => [id, () => cleanupSession(request, id)]))],
      ["template", async () => {
        expect((await request.delete(`/api/templates/${encodeURIComponent(name)}`)).ok()).toBe(true);
      }],
    ]);
  }
});

/** The host owns YOLO admission. Cancel retains the draft without retrying;
 * a later explicit answer may launch, but New has already supplied its name. */
for (const numbered of [false, true]) {
test(`${numbered ? "number shortcut" : "quick launch"} reveals the ordinary YOLO question and retains its draft`, async ({ page, request }) => {
  const name = `quick-yolo-${Date.now()}`;
  const posts: any[] = [];
  const created: string[] = [];
  let bodyFailed = false;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts.push(route.request().postDataJSON());
    const response = await route.fetch();
    if (response.ok()) created.push((await response.json()).id);
    await route.fulfill({ response });
  });
  try {
    await setLocalYoloWithoutAsking(request, false);
    await commandTemplate(request, name, true);
    await patchPreferences(request, numbered ? { template_shortcuts: { "1": name } } : { quick_launch_template: name });
    await page.goto("/");
    if (numbered) await numberChord(page); else await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".yolo-confirmation")).toBeVisible();
    await expect(form.locator(".yolo-cancel")).toBeFocused();
    expect(posts).toHaveLength(1);
    await form.locator(".yolo-cancel").click();
    await expect(form.locator(".yolo-confirmation")).toHaveCount(0);
    await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
    await expect(form.getByLabel("name (optional)")).toHaveValue(`${name}-session`);
    expect(posts).toHaveLength(1);
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".yolo-confirmation")).toBeVisible();
    expect(posts).toHaveLength(2);
    await form.locator(".yolo-confirm").click();
    await expect.poll(() => created.length).toBe(1);
    await echoAfterNaming(page, created[0], `yolo-revealed-${Date.now()}`);
    expect(posts).toHaveLength(3);
    expect(posts[2].confirm_yolo).toBe(true);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ["created sessions", () => cleanupOwnedSteps(false, created.map((id) => [id, () => cleanupSession(request, id)]))],
      ["template", async () => {
        expect((await request.delete(`/api/templates/${encodeURIComponent(name)}`)).ok()).toBe(true);
      }],
    ]);
  }
});
}

/** A refusal spends the shortcut attempt, while a later deliberate Launch
 * reuses the mounted draft. Its eventual success must retain the ordinary
 * New flow. After a committed create loses its reply, reapplying a template
 * must recover that create unless the effective request changes. */
for (const numbered of [false, true]) {
for (const retry of ["refusal", "unchanged-template", "changed-template"] as const) {
test(`${numbered ? "number shortcut" : "quick launch"} ${retry === "refusal" ? "reveals a refusal without automatically retrying" : `reapplies ${retry} after a lost reply`}`, async ({ page, request }) => {
  const name = `quick-refused-${Date.now()}`;
  const bodies: any[] = [];
  const created: string[] = [];
  let bodyFailed = false;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    bodies.push(route.request().postDataJSON());
    if (bodies.length === 1 && retry === "refusal") await fulfillQuickFixture(route, request, 409, "quick fixture refusal");
    else {
      const response = await route.fetch();
      if (response.ok()) created.push((await response.json()).id);
      if (bodies.length === 1) await route.abort(); // The helm committed; only the browser loses the reply.
      else await route.fulfill({ response });
    }
  });
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, numbered ? { template_shortcuts: { "1": name } } : { quick_launch_template: name });
    await page.goto("/");
    if (numbered) await numberChord(page); else await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-error")).toBeVisible();
    if (retry === "refusal") await expect(form.locator(".create-session-error")).toContainText("quick fixture refusal");
    await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
    expect(bodies).toHaveLength(1);
    if (retry !== "refusal") {
      expect(created).toHaveLength(1);
      expect((await listSessions(request)).sessions.filter((one) => one.title.startsWith(name))).toHaveLength(1);
      const title = retry === "changed-template" ? `${name}-changed` : bodies[0].title;
      // Include the same folder explicitly, exercising search edits too.
      const saved = await request.put(`/api/templates/${name}`, {
        data: { kind: "command", command: FAKE_AGENT, yolo: false, name: title, destination: { folder: bodies[0].cwd } },
      });
      expect(saved.ok(), await saved.text()).toBe(true);
      const generation = await form.getAttribute("data-browse-generation");
      expect(generation).toMatch(/^\d+$/);
      if (numbered) await numberChord(page); else await chord(page);
      // Template acceptance invalidates browsing synchronously with its edits;
      // unchanged field text alone would not prove the replay finished.
      await expect(form).not.toHaveAttribute("data-browse-generation", generation!);
      await expect(form.getByLabel("name (optional)")).toHaveValue(title);
      expect(bodies).toHaveLength(1);
    }
    await form.locator(".create-session-submit").click();
    await expect.poll(() => bodies.length).toBe(2);
    if (retry === "changed-template") expect(bodies[1].intent_key).not.toBe(bodies[0].intent_key);
    else expect(bodies[1]).toEqual(bodies[0]);
    await expect.poll(() => created.length).toBe(retry === "refusal" ? 1 : 2);
    expect((await listSessions(request)).sessions.filter((one) => one.title.startsWith(name)))
      .toHaveLength(retry === "changed-template" ? 2 : 1);
    await echoAfterNaming(page, created.at(-1)!, `refused-revealed-${Date.now()}`);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ["created sessions", () => cleanupOwnedSteps(false, [...new Set(created)].map((id) => [id, () => cleanupSession(request, id)]))],
      ["template", async () => {
        expect((await request.delete(`/api/templates/${encodeURIComponent(name)}`)).ok()).toBe(true);
      }],
    ]);
  }
});
}
}

/** Unbound and incomplete number bindings use the existing Templates/New
 * fallbacks, while a visible modal must keep input and launch authority.
 * A number alone recommends the prefix; the disabled primary action pins
 * incompleteness rather than a timed absence of a create request.
 */
test("number shortcuts respect modals and explain unbound or incomplete templates", async ({ page, request }) => {
  const name = `number-partial-${Date.now()}`;
  const runnable = `${name}-ready`;
  let creates = 0;
  page.on("request", request => {
    if (request.method() === "POST" && new URL(request.url()).pathname === "/api/sessions") creates++;
  });
  try {
    expect((await request.put(`/api/templates/${name}`, { data: { effort: "high" } })).ok()).toBe(true);
    await commandTemplate(request, runnable);
    await patchPreferences(request, { template_shortcuts: { "1": runnable } });
    expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": runnable });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".templates-new")).toBeFocused();
    await dialog.evaluate(node => {
      const received: { code: string; prevented: boolean }[] = [];
      (window as unknown as { numberModalInput: typeof received }).numberModalInput = received;
      node.addEventListener("keydown", event => {
        const key = event as KeyboardEvent;
        if (key.code === "Digit1" && key.ctrlKey && key.shiftKey) received.push({ code: key.code, prevented: key.defaultPrevented });
      }, true);
    });
    await numberChord(page);
    expect(await page.evaluate(() => (window as unknown as {
      numberModalInput: { code: string; prevented: boolean }[];
    }).numberModalInput)).toEqual([{ code: "Digit1", prevented: false }]);
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".templates-number-hint")).toHaveCount(0);
    await dialog.locator(".templates-close").click();
    await expect(dialog).toHaveCount(0);
    await numberChord(page, "0");
    await expect(dialog.locator(".templates-number-hint")).toContainText("Give a template a number key");
    await dialog.locator(".templates-close").click();
    await expect(dialog).toHaveCount(0);
    await patchPreferences(request, { template_shortcuts: { "1": name } });
    await numberChord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    await expect(form.locator(".partial-template-banner")).toHaveText(`${name} cannot launch on its own. Use Ctrl+Shift+X to combine template keys, then Enter to launch, or complete this setup below.`);
    expect(creates).toBe(0);
  } finally {
    for (const template of [name, runnable]) await request.delete(`/api/templates/${template}`);
  }
});

/** An unavailable destination never supplies a ready create target. A hidden
 * form must reveal that precondition rather than waiting forever or sending a
 * request. The real local identity pins template selection; only its displayed
 * connection state is replaced, leaving the owned supervisor alive. */
test("quick launch reveals an offline host without dispatching a create", async ({ page, request }) => {
  const name = `quick-offline-${Date.now()}`;
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local?.identity, "the connected local fixture has an install identity").toBeTruthy();
  const stored = await request.put(`/api/templates/${name}`, {
    data: { kind: "command", command: FAKE_AGENT, yolo: false, host: local!.identity, name: `${name}-session` },
  });
  expect(stored.ok(), await stored.text()).toBe(true);
  let unavailableReads = 0;
  let creates = 0;
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    const host = body.hosts.find((row: any) => row.id === local!.id);
    expect(host.identity).toBe(local!.identity);
    host.state = { phase: "unreachable-reprobing", cause: "fixture-offline", last_error: "fixture offline" };
    unavailableReads++;
    await route.fulfill({ response, json: body });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") creates++;
    await route.continue();
  });
  try {
    await patchPreferences(request, { quick_launch_template: name });
    await page.goto("/");
    await expect.poll(() => unavailableReads, { message: "the page consumed the unavailable-host fixture" })
      .toBeGreaterThan(0);
    await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-host-note")).toContainText("the selected host is unavailable");
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
    expect(creates).toBe(0);
  } finally {
    await page.unroute("**/api/hosts");
    await page.unroute("**/api/sessions");
    expect((await request.delete(`/api/templates/${name}`)).ok()).toBe(true);
  }
});

/** Other modals keep the chord, and fresh reads distinguish a stale cached
 * mark from a deleted template or one edited into an incomplete setup. Bolt's
 * incomplete notice asks for choices without recommending a sequence prefix. */
test("quick launch keeps modal ownership and explains missing or incomplete templates", async ({ page, request }) => {
  const name = `quick-incomplete-${Date.now()}`;
  let creates = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") creates++;
    await route.continue();
  });
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    await page.goto("/");
    await page.locator(".templates-button").click();
    await expect(page.getByRole("dialog", { name: "templates", exact: true })).toBeVisible();
    await chord(page);
    expect(creates).toBe(0);
    await expect(page.locator(".create-session-form")).toHaveCount(0);
    await page.getByRole("dialog", { name: "templates", exact: true }).getByRole("button", {
      name: "close",
      exact: true,
    }).click();
    // Each chord below must meet a page with no dialog left over, or a
    // dialog still rendering out could swallow it and pass for the guard.
    await expect(page.getByRole("dialog", { name: "templates", exact: true })).toHaveCount(0);
    expect((await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: { effort: "high" } })).ok()).toBe(
      true,
    );
    await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    await expect(form.locator(".partial-template-banner")).toHaveText(`${name} needs more choices before it can launch; complete it below.`);
    expect(creates).toBe(0);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);
    expect((await request.delete(`/api/templates/${encodeURIComponent(name)}`)).ok()).toBe(true);
    await chord(page);
    await expect(page.locator(".templates-quick-hint")).toContainText("Mark a template with ⚡");
    expect(creates).toBe(0);
  } finally {
    await page.unroute("**/api/sessions");
    await request.delete(`/api/templates/${encodeURIComponent(name)}`);
  }
});

/** Marking is independent of editor fields, one mark moves between names,
 * rename repoints it, and Delete/Undo cannot resurrect the old mark. */
test("the template mark moves, survives rename, and stays cleared after undo", async ({ page, request }) => {
  const prefix = `quick-marks-${Date.now()}`;
  const [a, b, partial, renamed] = ["a", "b", "partial", "renamed"].map((s) => `${prefix}-${s}`);
  try {
    await commandTemplate(request, a);
    await commandTemplate(request, b);
    expect((await request.put(`/api/templates/${partial}`, { data: { effort: "high" } })).ok()).toBe(true);
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await expect(dialog.getByRole("button", { name: `quick launch ${partial}`, exact: true })).toBeDisabled();
    await dialog.getByRole("button", { name: `quick launch ${a}`, exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBe(a);
    await dialog.getByRole("button", { name: `quick launch ${b}`, exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBe(b);
    await expect(dialog.locator('.templates-quick-mark[aria-pressed="true"]')).toHaveCount(1);
    await dialog.getByText(b, { exact: true }).click();
    await expect(dialog.locator(".templates-quick-state")).toContainText("quick launch");
    await dialog.locator(".templates-name").fill(renamed);
    await dialog.getByRole("button", { name: "save", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBe(renamed);
    await expect(dialog.getByText(renamed, { exact: true })).toBeVisible();
    await expect(dialog.getByText(b, { exact: true })).toHaveCount(0);
    await dialog.getByRole("button", { name: "delete", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBeUndefined();
    await dialog.getByRole("button", { name: "undo", exact: true }).click();
    await expect(dialog.getByText(renamed, { exact: true })).toBeVisible();
    await expect(dialog.locator('.templates-quick-mark[aria-pressed="true"]')).toHaveCount(0);
    expect((await readPreferences(request)).quick_launch_template).toBeUndefined();
  } finally {
    for (const name of [a, b, partial, renamed]) await request.delete(`/api/templates/${encodeURIComponent(name)}`);
  }
});

/** A mark can commit while its response is still in flight. Deletion must
 * wait for that owned writer rather than racing it, then Undo restores only
 * fields. The held response and disabled Delete prove the unsettled boundary;
 * no delay or credential remount is used as a proxy for ordering. */
test("deletion drains an in-flight quick mark before clearing it", async ({ page, request }) => {
  const name = `quick-pending-delete-${Date.now()}`;
  const gate = routeGate();
  let held = 0;
  let deletes = 0;
  await page.route("**/api/preferences", async (route) => {
    if (route.request().method() !== "PUT" || route.request().postDataJSON().quick_launch_template !== name) {
      return route.continue();
    }
    const response = await route.fetch();
    expect(response.ok()).toBe(true);
    held++;
    await gate.wait();
    await route.fulfill({ response });
  });
  await page.route(`**/api/templates/${name}`, async (route) => {
    if (route.request().method() === "DELETE") deletes++;
    await route.continue();
  });
  try {
    await commandTemplate(request, name);
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await dialog.getByText(name, { exact: true }).click();
    await dialog.getByRole("button", { name: `quick launch ${name}`, exact: true }).click();
    await expect.poll(() => held, { message: "the mark committed and its response is held" }).toBe(1);
    expect((await readPreferences(request)).quick_launch_template).toBe(name);
    const remove = dialog.getByRole("button", { name: "delete", exact: true });
    await remove.click();
    await expect(remove).toBeDisabled();
    expect(deletes, "DELETE cannot overtake the held preference response").toBe(0);
    gate.release();
    await expect(dialog.getByText(name, { exact: true })).toHaveCount(0);
    expect(deletes).toBe(1);
    expect((await readPreferences(request)).quick_launch_template).toBeUndefined();
    await dialog.getByRole("button", { name: "undo", exact: true }).click();
    await expect(dialog.getByText(name, { exact: true })).toBeVisible();
    await expect(dialog.locator('.templates-quick-mark[aria-pressed="true"]')).toHaveCount(0);
    expect((await readPreferences(request)).quick_launch_template).toBeUndefined();
  } finally {
    gate.release();
    await page.unroute("**/api/preferences");
    await page.unroute(`**/api/templates/${name}`);
    await request.delete(`/api/templates/${name}`);
  }
});

/** A marked template edited into one that can no longer be the quick launch
 * (here through the API, which the editor's eligibility rule does not see)
 * still shows a lit, clickable bolt, and clicking it clears the mark. Why:
 * clicking the lit bolt is the only way to turn a mark off in the UI, so a
 * disabled lit bolt left a mark nobody could remove short of deleting the
 * template. */
test("a marked template that became ineligible can still be unmarked", async ({ page, request }) => {
  const name = `quick-ineligible-${Date.now()}`;
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    expect((await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: { effort: "high" } })).ok()).toBe(
      true,
    );
    expect((await readPreferences(request)).quick_launch_template, "premise: the mark survives the edit").toBe(name);
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    const bolt = dialog.getByRole("button", { name: `quick launch ${name}`, exact: true });
    await expect(bolt).toHaveAttribute("aria-pressed", "true");
    await expect(bolt).toBeEnabled();
    await bolt.click();
    await expect(bolt).toHaveAttribute("aria-pressed", "false");
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template ?? null).toBeNull();
    await expect(bolt, "an ineligible template cannot be marked again").toBeDisabled();
  } finally {
    await request.delete(`/api/templates/${encodeURIComponent(name)}`);
  }
});
