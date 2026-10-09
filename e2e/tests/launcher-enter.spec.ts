/**
 * Launch refusals must be visible at the action that failed. Native disabled
 * used to swallow an incomplete draft's implicit Enter, while request errors
 * appeared below the scrolling controls. These cases distinguish an explained
 * refusal from silence without creating a session merely to inspect the UI.
 */
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";

/** Pin the reason inside the initial dialog viewport, directly after its actions. */
async function expectReasonByLaunch(form: Locator, text: string) {
  const reason = form.locator(".launch-composer-refusal");
  await expect(reason).toBeVisible();
  await expect(reason).toContainText(text);
  const actions = await form.locator(".launch-composer-actions").boundingBox();
  const message = await reason.boundingBox();
  const dialog = await form.boundingBox();
  expect(actions, "the action row must be painted before measuring the reason").not.toBeNull();
  expect(message, "the refusal must have its own visible box").not.toBeNull();
  expect(dialog, "the dialog's initial viewport must be measurable").not.toBeNull();
  expect(message!.y).toBeGreaterThanOrEqual(actions!.y + actions!.height);
  expect(message!.y + message!.height).toBeLessThanOrEqual(dialog!.y + dialog!.height);
}

for (const attempt of ["Enter", "Launch"] as const) {
  /** An incomplete draft explains itself without sending a create; correcting it clears that reason. */
  test(`launcher ${attempt} explains an incomplete setup beside Launch`, async ({ page }) => {
    let posts = 0;
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() === "POST") posts += 1;
      await route.continue();
    });
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    const launch = form.locator(".create-session-submit");
    await expect(launch).toHaveAttribute("aria-disabled", "true");
    await expect(launch).toHaveAttribute("data-tooltip", "choose a structured harness before launching");
    await expect(form.locator(".launch-composer-refusal"), "opening an unfinished draft is not an error").toHaveCount(0);
    if (attempt === "Enter") {
      const search = form.getByRole("combobox", { name: "search folders, harnesses, models, and efforts", exact: true });
      await expect(search).toHaveValue("");
      await search.focus();
      await expect(search).toBeFocused();
      await search.press("Enter");
    } else {
      // aria-disabled describes the refusal but intentionally keeps native
      // activation alive. Playwright otherwise refuses this real pointer input.
      await launch.click({ force: true });
    }
    await expectReasonByLaunch(form, "choose a structured harness before launching");
    expect(posts, "a visible prerequisite refusal must post no create").toBe(0);
    await form.getByRole("button", { name: "Codex", exact: true }).click();
    await expect(launch).toHaveAttribute("aria-disabled", "false");
    await expect(form.locator(".launch-composer-refusal"), "correcting the prerequisite clears its reason").toHaveCount(0);
  });
}

/** Empty-command validation belongs to the visible refusal path, including implicit text-field Enter. */
test("launcher text-field Enter explains an empty command beside Launch", async ({ page }) => {
  let posts = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") posts += 1;
    await route.continue();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByRole("tab", { name: "command", exact: true }).click();
  await form.getByRole("radio", { name: "no", exact: true }).check();
  const command = form.getByLabel("agent command", { exact: true });
  await expect(command).toHaveValue("");
  await expect(form.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "false");
  await command.focus();
  await expect(command).toBeFocused();
  await command.press("Enter");
  await expectReasonByLaunch(form, "enter an agent command before launching");
  expect(posts, "the local empty-command refusal must post no create").toBe(0);
});

/** The ordinary submit path keeps the helm's refusal near Launch, within the initial viewport. */
test("launcher shows the helm's refusal beside Launch", async ({ page, request }) => {
  const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"];
  expect(build, "the controlled response must preserve the live helm's build stamp").toBeTruthy();
  let posts = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts += 1;
    await route.fulfill({ status: 400, headers: { "x-farhelm-build": build, "content-type": "text/plain" }, body: "launch fixture refused this request" });
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "default", exact: true }).click();
  const launch = form.locator(".create-session-submit");
  await expect(launch).toHaveAttribute("aria-disabled", "false");
  await launch.click();
  await expectReasonByLaunch(form, "launch fixture refused this request");
  expect(posts, "the refusal must come from exactly one ordinary launch attempt").toBe(1);
});

/**
 * Observe the ordinary create body without starting an agent. A numbered,
 * stamped refusal is the completion oracle for each attempt, so consecutive
 * choices cannot be asserted against an earlier request or a still-busy form.
 */
async function observeChoiceCreates(page: Page, request: APIRequestContext) {
  const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"];
  expect(build, "controlled replies must retain the live helm identity").toBeTruthy();
  const bodies: Record<string, unknown>[] = [];
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    bodies.push(route.request().postDataJSON());
    await route.fulfill({ status: 400, headers: { "x-farhelm-build": build, "content-type": "text/plain" }, body: `choice fixture refused create ${bodies.length}` });
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "default", exact: true }).click();
  return { form, bodies, build };
}

/** Establish keyboard focus and await this attempt's ordinary refusal before inspecting its body. */
async function enterCreateChoice(form: Locator, control: Locator, bodies: Record<string, unknown>[]) {
  const before = bodies.length;
  await control.focus();
  await expect(control).toBeFocused();
  await control.press("Enter");
  await expectReasonByLaunch(form, `choice fixture refused create ${before + 1}`);
  expect(bodies, "one Enter must send exactly one ordinary create").toHaveLength(before + 1);
  await expect(form.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "false");
  return bodies[before];
}

/** Choice buttons must apply their value before submission, including a tab's dormant draft. */
test("launcher choice-button Enter applies then launches", async ({ page, request }) => {
  const { form, bodies } = await observeChoiceCreates(page, request);
  expect(await enterCreateChoice(form, form.getByRole("button", { name: "Codex", exact: true }), bodies))
    .toMatchObject({ launch: { harness: "codex" } });
  expect(await enterCreateChoice(form, form.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true }), bodies))
    .toMatchObject({ launch: { harness: "codex", effort: "low" } });
  expect(await enterCreateChoice(form, form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }), bodies))
    .toMatchObject({ launch: { permissions: "yolo" } });
  expect(await enterCreateChoice(form, form.locator(".launch-composer-trust-choice").getByRole("button", { name: "true", exact: true }), bodies))
    .toMatchObject({ launch: { workspace_trust: true } });
  const commandTab = form.getByRole("tab", { name: "command", exact: true });
  const agentTab = form.getByRole("tab", { name: "agent", exact: true });
  await commandTab.click();
  await form.getByLabel("agent command", { exact: true }).fill("printf hello");
  await form.getByRole("radio", { name: "no", exact: true }).check();
  await agentTab.click();
  expect(await enterCreateChoice(form, commandTab, bodies)).toMatchObject({ command: { command: "printf hello", yolo: false } });
  expect(bodies.at(-1)).not.toHaveProperty("launch");
  expect(await enterCreateChoice(form, agentTab, bodies)).toMatchObject({ launch: { harness: "codex", effort: "low", permissions: "yolo", workspace_trust: true } });
  expect(bodies.at(-1)).not.toHaveProperty("command");
});

/** Enter on value controls submits their shown state; it cannot toggle a checkbox or radio. */
test("launcher value-control Enter keeps the shown choices", async ({ page, request }) => {
  const { form, bodies } = await observeChoiceCreates(page, request);
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  const host = form.getByLabel("host", { exact: true });
  const hostId = await host.inputValue();
  expect(hostId, "the connected host selection must have settled").not.toBe("");
  expect(await enterCreateChoice(form, host, bodies)).toMatchObject({ host: Number(hostId) });
  await expect(host).toHaveValue(hostId);
  await form.getByRole("tab", { name: "command", exact: true }).click();
  await form.getByLabel("agent command", { exact: true }).fill("claude {farhelm_args}");
  await form.getByRole("radio", { name: "no", exact: true }).check();
  const agent = form.locator(".launch-command-agent");
  await agent.selectOption("claude");
  expect(await enterCreateChoice(form, agent, bodies)).toMatchObject({ command: { agent: "claude", yolo: false } });
  await expect(agent).toHaveValue("claude");
  const resume = form.locator(".launch-command-resume-toggle");
  await expect(resume).not.toBeChecked();
  const unchecked = await enterCreateChoice(form, resume, bodies);
  expect(unchecked.command).toMatchObject({ resume: null });
  await expect(resume).not.toBeChecked();
  // Space remains the value-setting key; Enter acts on the value it set.
  await resume.press("Space");
  await expect(resume).toBeChecked();
  await form.getByLabel("resume command", { exact: true }).fill("claude --resume {conversation} {farhelm_args}");
  expect(await enterCreateChoice(form, resume, bodies)).toMatchObject({ command: { resume: "claude --resume {conversation} {farhelm_args}" } });
  await expect(resume).toBeChecked();
  const yes = form.getByRole("radio", { name: "yes (YOLO)", exact: true });
  await expect(yes).not.toBeChecked();
  expect(await enterCreateChoice(form, yes, bodies)).toMatchObject({ command: { yolo: false } });
  await expect(yes).not.toBeChecked();
});

/** A choice with no launchable harness gives the same refusal, rather than just selecting itself. */
test("launcher choice Enter explains an incomplete draft", async ({ page }) => {
  let posts = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") posts += 1;
    await route.continue();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  const effort = form.locator(".launch-composer-effort-choice").getByRole("button", { name: "default", exact: true });
  await effort.focus();
  await expect(effort).toBeFocused();
  await effort.press("Enter");
  await expectReasonByLaunch(form, "choose a structured harness before launching");
  expect(posts).toBe(0);
});

/** Held Enter must never apply-and-launch or fall through to native form submission. */
test("launcher repeated Enter never launches", async ({ page, request }) => {
  const source = await request.get("/api/sessions");
  const build = source.headers()["x-farhelm-build"];
  const host = (await source.json()).sessions[0]?.host;
  expect(build, "recent history must use the live helm identity").toBeTruthy();
  expect(typeof host, "the startup row must identify its actual host").toBe("number");
  await page.route("**/api/launch-history**", async (route) => {
    await route.fulfill({ status: 200, headers: { "x-farhelm-build": build, "content-type": "application/json" }, body: JSON.stringify({
      launches: [{ host, canonical_cwd: "/tmp", cwd: "/tmp", selection: { harness: "codex", model: null, effort: "high", permissions: null, workspace_trust: null }, created_at: 1, creation_seq: 1 }],
      folders: [],
    }) });
  });
  const { form, bodies } = await observeChoiceCreates(page, request);
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  await expect(form.getByLabel("host", { exact: true }), "the recent must belong to the selected connected destination").toHaveValue(String(host));
  const recent = form.locator(".launch-composer-recents button");
  await expect(recent).toHaveCount(1);
  await expect(recent).toContainText("High");
  await expect(form.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "false");
  for (const control of [
    form.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true }),
    form.getByLabel("host", { exact: true }),
    form.getByLabel("folder", { exact: true }),
    form.locator(".create-session-submit"),
    recent,
  ]) {
    await control.focus();
    await expect(control).toBeFocused();
    // Inject only the repeat edge. A prior admitted key would make the busy
    // guard hide a broken repeat guard, so there is deliberately no first key.
    const uncanceled = await control.evaluate((node) => node.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", repeat: true, bubbles: true, cancelable: true })));
    expect(uncanceled, "a repeated key must cancel native activation as well as explicit submission").toBe(false);
  }
  await expect(form.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true })).toHaveAttribute("aria-pressed", "false");
  await expect(form.locator(".launch-composer-effort-choice").getByRole("button", { name: "default", exact: true }), "repeated recent Enter must not apply its high-effort choice").toHaveAttribute("aria-pressed", "true");
  const cancel = form.getByRole("button", { name: "cancel", exact: true });
  await cancel.focus();
  await expect(cancel).toBeFocused();
  await cancel.press("Enter");
  await expect(form).toHaveCount(0);
  expect(bodies, "repeats and the normal Cancel press must post nothing").toEqual([]);
});

/** Action buttons retain their press instead of being mistaken for launch choices. */
test("launcher action-button Enter keeps reset browse and cancel", async ({ page, request }) => {
  const { form, bodies } = await observeChoiceCreates(page, request);
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  const reset = form.getByRole("button", { name: "reset choices", exact: true });
  await reset.focus();
  await expect(reset).toBeFocused();
  await reset.press("Enter");
  await expect(form.getByRole("button", { name: "Codex", exact: true })).toHaveAttribute("aria-pressed", "false");
  const browse = form.getByRole("button", { name: "browse this path", exact: true });
  await browse.focus();
  await expect(browse).toBeFocused();
  await browse.press("Enter");
  const use = form.locator(".launch-composer-browser").getByRole("button", { name: "use /tmp", exact: true });
  await expect(use).toBeVisible();
  await use.focus();
  await expect(use).toBeFocused();
  await use.press("Enter");
  await expect(form.locator(".launch-composer-browser")).toHaveCount(0);
  await expect(form.getByLabel("folder", { exact: true })).toHaveValue("/tmp");
  const cancel = form.getByRole("button", { name: "cancel", exact: true });
  await cancel.focus();
  await expect(cancel).toBeFocused();
  await cancel.press("Enter");
  await expect(form).toHaveCount(0);
  expect(bodies, "these actions must never create a session").toEqual([]);
});

/** Retry re-reads the catalog, while Enter on Launch keeps the ordinary request path. */
test("launcher action-button Enter keeps retry and Launch", async ({ page, request }) => {
  let catalogReads = 0;
  await page.route("**/api/launch-catalog", async (route) => {
    catalogReads += 1;
    if (catalogReads > 1) return route.continue();
    const response = await route.fetch();
    await route.fulfill({ response, status: 500, body: "choice fixture catalog unavailable" });
  });
  const { form, bodies } = await observeChoiceCreates(page, request);
  const retry = form.locator(".create-catalog-error").getByRole("button", { name: "retry", exact: true });
  await expect(retry).toBeVisible();
  expect(catalogReads).toBe(1);
  await retry.focus();
  await expect(retry).toBeFocused();
  await retry.press("Enter");
  await expect(form.locator(".create-catalog-error")).toHaveCount(0);
  await expect.poll(() => catalogReads).toBe(2);
  expect(bodies, "Retry must not launch").toEqual([]);
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  expect(await enterCreateChoice(form, form.locator(".create-session-submit"), bodies)).toMatchObject({ launch: { harness: "codex" } });
});

/** A launch shortcut opens the YOLO question, whose focused Cancel still owns Enter. */
test("launcher choice Enter leaves the YOLO question's Enter rules intact", async ({ page, request }) => {
  const { form, bodies, build } = await observeChoiceCreates(page, request);
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    bodies.push(route.request().postDataJSON());
    await route.fulfill({ status: 409, headers: {
      "x-farhelm-build": build, "content-type": "text/plain",
      "x-farhelm-yolo-confirmation": "confirmation-required",
    }, body: "choice fixture requires YOLO confirmation" });
  });
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  const yolo = form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true });
  await yolo.focus();
  await expect(yolo).toBeFocused();
  await yolo.press("Enter");
  const question = form.locator(".yolo-confirmation");
  await expect(question).toBeVisible();
  const cancel = question.locator(".yolo-cancel");
  await expect(cancel, "the question must hand focus to its default Cancel before Enter").toBeFocused();
  await cancel.press("Enter");
  await expect(question).toHaveCount(0);
  await expect(form).toBeVisible();
  expect(bodies, "Cancel must neither confirm nor send another create").toHaveLength(1);
  expect(bodies[0]).not.toHaveProperty("confirm_yolo");
});
