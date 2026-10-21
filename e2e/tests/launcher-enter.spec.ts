/**
 * Launch refusals must be visible at the action that failed. Native disabled
 * used to swallow an incomplete draft's implicit Enter, while request errors
 * appeared below the scrolling controls. These cases distinguish an explained
 * refusal from silence without creating a session merely to inspect the UI.
 */
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { clickDialogGap } from "./helpers/dialog-gap";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { cleanupSession, createSession, listHosts, openRowMenu, stubFeed } from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";

/** Clone must offer the typed name visibly before a separate Enter launches.
 * A real fake-agent source and successor distinguish setting a draft from an
 * accidental immediate create, and the source must remain unchanged.
 */
test("Clone search Enter picks an unmatched name before a second Enter launches", async ({ page, request }) => {
  let sourceId: string | undefined;
  let childId: string | undefined;
  let bodyFailed = false;
  try {
    const source = await createSession(request, { title: `enter-source-${Date.now()}` });
    sourceId = source.id;
    await page.goto("/");
    const row = page.locator(`.session-row[data-session-id="${source.id}"]`);
    await expect(row).toBeVisible();
    await openRowMenu(row);
    await row.locator(".session-row-clone").click();
    const form = page.locator(".create-session-form");
    const search = form.locator(".launch-composer-search input");
    await expect(search).toBeFocused();
    const creates: Record<string, unknown>[] = [];
    page.on("request", (sent) => {
      if (sent.method() === "POST" && sent.url().endsWith("/api/sessions")) creates.push(sent.postDataJSON());
    });
    await search.fill("asdf");
    await expect(form.getByRole("option", { name: "Set session name: asdf", exact: true, selected: true })).toBeVisible();
    await search.press("Enter");
    await expect(form.getByLabel("name (optional)", { exact: true })).toHaveValue("asdf");
    await expect(search).toHaveValue("");
    await expect(search).toBeFocused();
    expect(creates, "accepting a name must not create the clone").toEqual([]);
    const [response] = await Promise.all([
      page.waitForResponse((reply) => reply.request().method() === "POST" && reply.url().endsWith("/api/sessions")),
      search.press("Enter"),
    ]);
    expect(response.ok(), await response.text()).toBe(true);
    childId = (await response.json()).id as string;
    expect(childId).not.toBe(source.id);
    expect(creates).toHaveLength(1);
    expect(creates[0].title).toBe("asdf");
    await expect(form).toHaveCount(0);
    await expect(row).toContainText(source.title);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["cloned session", async () => { if (childId) await cleanupSession(request, childId); }],
      ["source session", async () => { if (sourceId) await cleanupSession(request, sourceId); }],
    ]);
  }
});

/** Live messages and result rows share one dismissible area. Reopening with
 * Enter must never accept a hidden row or silently launch the unfinished form.
 */
test("launcher search explains empty results and Enter reopens before picking", async ({ page }) => {
  let creates = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") creates += 1;
    await route.continue();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  const search = form.locator(".launch-composer-search input");
  const area = form.locator("#launch-composer-search-results");
  await search.fill("host:no-such-host-for-enter");
  await expect(area).toHaveText("no host matches");
  await search.press("Escape");
  await expect(area).toHaveCount(0);
  await expect(search).toBeFocused();
  await search.press("Enter");
  await expect(area).toHaveText("no host matches");
  await search.press("Enter");
  await expect(area).toHaveText("no host matches");
  await search.fill("asdf");
  await expect(form.getByRole("option", { name: "Set session name: asdf", exact: true })).toBeVisible();
  await search.press("Escape");
  await expect(area).toHaveCount(0);
  await search.press("Enter");
  await expect(form.getByRole("option", { name: "Set session name: asdf", exact: true })).toBeVisible();
  await expect(search).toHaveValue("asdf");
  await expect(form.getByLabel("name (optional)", { exact: true })).toHaveValue("");
  await search.press("Enter");
  await expect(search).toHaveValue("");
  await expect(form.getByLabel("name (optional)", { exact: true })).toHaveValue("asdf");
  expect(creates).toBe(0);
});

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

/**
 * An empty-space click must not discard the user's search or its focus. If
 * focus reaches the launcher container by another route, Enter still follows
 * Launch's ordinary refusal instead of silently doing nothing.
 */
test("launcher gap keeps focus and container Enter explains an incomplete draft", async ({ page }) => {
  let posts = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") posts += 1;
    await route.continue();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  const search = form.getByRole("combobox", { name: "search folders, harnesses, models, and efforts", exact: true });
  await search.focus();
  await expect(search).toBeFocused();
  const gap = await clickDialogGap(page, form);
  await expect(search).toBeFocused();
  await page.mouse.dblclick(gap.x, gap.y);
  expect(await page.evaluate(() => document.getSelection()?.toString().trim()), "an empty gap cannot select the dialog").toBe("");
  await expect(search).toBeFocused();
  await page.keyboard.type("codex");
  await expect(search).toHaveValue("codex");

  await form.focus();
  await expect(form).toBeFocused();
  expect(await form.evaluate((node) => getComputedStyle(node).outlineStyle)).toBe("none");
  await form.press("Enter");
  await expectReasonByLaunch(form, "choose a structured harness before launching");
  expect(posts, "a container refusal must not create a session").toBe(0);
});

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
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
  const launch = form.locator(".create-session-submit");
  await expect(launch).toHaveAttribute("aria-disabled", "false");
  await launch.click();
  await expectReasonByLaunch(form, "launch fixture refused this request");
  expect(posts, "the refusal must come from exactly one ordinary launch attempt").toBe(1);
});

/** Invalid custom flags are a repairable editor problem, not a catalog
 * incompatibility. Both whitespace and an unfinished quote must explain Enter
 * locally without invoking the installed agent.
 */
test("launcher Enter names invalid custom permission flags", async ({ page }) => {
  let posts = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") posts++;
    await route.continue();
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
  const flags = form.locator(".custom-permission-flags");
  for (const draft of ["", "  ", "--mode '"]) {
    await flags.fill(draft);
    await expect(flags).toHaveValue(draft);
    await expect(form.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "true");
    await expect(form.locator(".create-session-submit")).toHaveAttribute("data-tooltip", /custom permission flags/);
    await flags.focus();
    await expect(flags).toBeFocused();
    await flags.press("Enter");
    await expectReasonByLaunch(form, "enter valid custom permission flags before launching");
    await expect(form).not.toContainText("choose a compatible model or effort");
    expect(posts).toBe(0);
  }
});

/** Pending template replay has no Launch action yet. Count submit events,
 * because an accidental requestSubmit can still stop at missing prerequisites
 * and produce zero POSTs. Releasing the catalog proves replay can finish.
 */
test("launcher container Enter waits for visible session setup loading", async ({ page, request }) => {
  const name = `enter-loading-${Date.now()}`;
  const sessionName = `${name}-applied`;
  const gate = routeGate();
  let reads = 0;
  let posts = 0;
  let bodyFailed = false;
  try {
    // A name-only partial template can apply without an agent. Effort-only
    // templates instead refuse after loading when no harness is selected.
    const saved = await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: { name: sessionName } });
    expect(saved.ok(), await saved.text()).toBe(true);
    await page.route("**/api/launch-catalog", async (route) => {
      reads++;
      await gate.wait();
      await route.fulfill({ response: await route.fetch() });
    });
    const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"];
    expect(build).toBeTruthy();
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() === "POST") posts++;
      if (route.request().method() !== "POST") return route.continue();
      await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "loading fixture refused an unexpected create" });
    });
    await page.goto("/");
    await expect(page.locator(".quick-switcher-template-trigger")).toHaveCount(1);
    await page.keyboard.press("Control+Shift+j");
    const list = page.getByRole("dialog", { name: "template launcher" });
    await expect(list.getByRole("combobox")).toBeFocused();
    await list.getByRole("combobox").fill(name);
    await expect(list.getByRole("option")).toHaveCount(1);
    await list.getByRole("combobox").press("Enter");
    const form = page.locator(".create-session-form");
    const loading = form.getByRole("status").filter({ hasText: "loading session setup…" });
    await expect(form).toBeVisible();
    await expect(loading).toBeVisible();
    await expect.poll(() => reads, { message: "the catalog reply is held during setup replay" }).toBeGreaterThan(0);
    await expect(form.locator(".create-session-submit")).toHaveCount(0);
    await expect(form.locator(".launch-composer-loading-cancel")).toBeFocused();
    await form.evaluate((node) => {
      node.dataset.submitWitness = "0";
      node.addEventListener("submit", () => {
        node.dataset.submitWitness = String(Number(node.dataset.submitWitness) + 1);
      });
    });
    await form.focus();
    await expect(form).toBeFocused();
    await form.press("Enter");
    await expect(form).toHaveAttribute("data-submit-witness", "0");
    await expect(loading).toBeVisible();
    expect(posts).toBe(0);
    gate.release();
    await expect(loading).toHaveCount(0);
    await expect(form.getByLabel("name (optional)", { exact: true })).toHaveValue(sessionName);
    expect(posts).toBe(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    gate.release();
    await cleanupOwnedSteps(bodyFailed, [
      ["held browser routes", async () => { await page.unrouteAll({ behavior: "wait" }); }],
      ["loading template", async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});

/** An old target's refusal still ends visibly after the selected installation
 * changes, without attributing its private prose to the new destination. Only
 * the browser's host read changes; the owned supervisor remains untouched.
 */
test("launcher shows a generic refusal after its target changes", async ({ page, request }) => {
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local?.identity).toBeTruthy();
  const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const feed = await stubFeed(page);
  feed.notifyOnConnect(0);
  const gate = routeGate();
  let moved = false;
  let posts = 0;
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    if (moved) {
      const host = body.hosts.find((row: { id: number }) => row.id === local!.id);
      host.destination = "fixture-retarget";
      host.identity = "fixture-replaced-install";
      host.state = { ...host.state, identity: host.identity };
    }
    await route.fulfill({ response, json: body });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts++;
    await gate.wait();
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "old-target private refusal" });
  });
  try {
    await page.goto("/");
    await feed.waitForConnection(1);
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await form.getByLabel("folder", { exact: true }).fill("/tmp");
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
    const launch = form.locator(".create-session-submit");
    await expect(launch).toHaveAttribute("aria-disabled", "false");
    const original = await form.getAttribute("data-create-target-fingerprint");
    expect(original).toBeTruthy();
    await launch.click();
    await expect.poll(() => posts, { message: "the old target's create reply is held" }).toBe(1);
    moved = true;
    expect(feed.openSockets()).toBe(1);
    feed.notify(2);
    await expect(form).not.toHaveAttribute("data-create-target-fingerprint", original!);
    await expect(form).not.toHaveAttribute("data-create-target-fingerprint", "");
    gate.release();
    await expectReasonByLaunch(form, "a launch for an earlier setup was refused");
    await expect(form).not.toContainText("old-target private refusal");
    expect(posts).toBe(1);
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "wait" });
  }
});

/**
 * Observe ordinary creates without starting an agent. Optional catalog rows
 * make compatibility changes deterministic; numbered, stamped refusals prove
 * each request completed while retaining its unresolved retry identity.
 * Permission-default templates can omit the usual explicit Guarded seed so
 * that setup does not overwrite the raw default they are meant to establish.
 */
async function observeChoiceCreates(
  page: Page,
  request: APIRequestContext,
  models?: readonly { id: string; harness: string; efforts: string[] }[],
  seedGuarded = true,
) {
  const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"];
  expect(build, "controlled replies must retain the live helm identity").toBeTruthy();
  if (models) {
    await page.route("**/api/launch-catalog", async (route) => {
      await route.fulfill({
        status: 200,
        headers: { "x-farhelm-build": build, "content-type": "application/json" },
        body: JSON.stringify(models),
      });
    });
  }
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
  if (seedGuarded) {
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
  }
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

/**
 * Retrying unchanged choices retains one intent, but each real change
 * retires it. Stamped refusals keep the form open without spawning agents,
 * and identify each completed attempt before its key is compared.
 */
test("launcher choice retries preserve keys only for unchanged values", async ({ page, request }) => {
  const { form, bodies } = await observeChoiceCreates(page, request);
  const codex = form.getByRole("button", { name: "Codex", exact: true });
  await codex.click();
  await expect(form.locator(".launch-composer-trust-choice")).toHaveCount(0);
  let previous = await enterCreateChoice(form, codex, bodies);
  expect(previous.intent_key).toBeTruthy();
  const unchanged = [
    codex,
    form.getByRole("tab", { name: "agent", exact: true }),
    form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }),
  ];
  for (const control of unchanged) {
    const retry = await enterCreateChoice(form, control, bodies);
    expect(retry.intent_key).toBe(previous.intent_key);
    previous = retry;
  }
  for (const control of [
    form.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true }),
    form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }),
  ]) {
    const changed = await enterCreateChoice(form, control, bodies);
    expect(changed.intent_key).toBeTruthy();
    expect(changed.intent_key).not.toBe(previous.intent_key);
    const retry = await enterCreateChoice(form, control, bodies);
    expect(retry.intent_key).toBe(changed.intent_key);
    previous = retry;
  }
});

/** Choice buttons must apply their value before submission, including a tab's dormant draft. */
test("launcher choice-button Enter applies then launches", async ({ page, request }) => {
  const { form, bodies } = await observeChoiceCreates(page, request);
  expect(await enterCreateChoice(form, form.getByRole("button", { name: "Codex", exact: true }), bodies))
    .toMatchObject({ launch: { harness: "codex" } });
  expect(await enterCreateChoice(form, form.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true }), bodies))
    .toMatchObject({ launch: { harness: "codex", effort: "low" } });
  expect(await enterCreateChoice(form, form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }), bodies))
    .toMatchObject({ launch: { permissions: "yolo" } });
  await expect(form.locator(".launch-composer-trust-choice")).toHaveCount(0);
  const commandTab = form.getByRole("tab", { name: "command", exact: true });
  const agentTab = form.getByRole("tab", { name: "agent", exact: true });
  await commandTab.click();
  await form.getByLabel("agent command", { exact: true }).fill("printf hello");
  await form.getByRole("radio", { name: "no", exact: true }).check();
  await agentTab.click();
  expect(await enterCreateChoice(form, commandTab, bodies)).toMatchObject({ command: { command: "printf hello", yolo: false } });
  expect(bodies.at(-1)).not.toHaveProperty("launch");
  expect(await enterCreateChoice(form, agentTab, bodies)).toMatchObject({ launch: { harness: "codex", effort: "low", permissions: "yolo" } });
  expect(bodies.at(-1)?.launch).not.toHaveProperty("workspace_trust");
  expect(bodies.at(-1)).not.toHaveProperty("command");
});

/** Enter on value controls submits their shown state; it cannot toggle a checkbox or radio. */
test("launcher value-control Enter keeps the shown choices", async ({ page, request }) => {
  const { form, bodies } = await observeChoiceCreates(page, request);
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  const host = form.getByLabel("host", { exact: true });
  const hostId = await host.getAttribute("data-host-id");
  expect(hostId, "the connected host selection must have settled").not.toBe("");
  expect(await enterCreateChoice(form, host, bodies)).toMatchObject({ host: Number(hostId) });
  await expect(host).toHaveAttribute("data-host-id", hostId);
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
  await expect(form.getByLabel("host", { exact: true }), "the recent must belong to the selected connected destination").toHaveAttribute("data-host-id", String(host));
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

const scalarRetryCatalog = [
  { id: "scalar-alpha", harness: "codex", efforts: ["low", "high"] },
  { id: "scalar-beta", harness: "codex", efforts: ["low"] },
];

/** Search accepts a choice without launching. Empty text and returned focus
 * distinguish completed acceptance from an uncommitted search draft.
 */
async function acceptScalarSearch(form: Locator, query: string, label: string) {
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.focus();
  await expect(search).toBeFocused();
  await search.fill(query);
  const option = form.getByRole("option", { name: label, exact: true });
  await expect(option).toBeVisible();
  await option.click();
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();
}

/** Reopen the picker through a real focus change, then click its exact row.
 * The accepted value and closed list prove the callback ran, even for a row
 * that leaves the submitted model unchanged.
 */
async function acceptScalarModelRow(form: Locator, id: string) {
  const folder = form.getByLabel("folder", { exact: true });
  const model = form.getByRole("combobox", { name: "model", exact: true });
  await folder.focus();
  await expect(folder).toBeFocused();
  await model.focus();
  await expect(model).toBeFocused();
  await model.fill(id);
  const results = form.locator("#launch-composer-model-results");
  await expect(results).toBeVisible();
  await results.getByRole("option", { name: id, exact: true }).click();
  await expect(results).toHaveCount(0);
  await expect(model).toHaveValue(id);
  await expect(model).toHaveAttribute("aria-expanded", "false");
  await expect(model).toBeFocused();
}

/** An unknown exact id reaches custom-model Enter rather than a catalog row.
 * Keep the picker un-navigated so no active option overrides the typed id.
 */
async function acceptScalarCustomModel(form: Locator, id: string) {
  const folder = form.getByLabel("folder", { exact: true });
  const model = form.getByRole("combobox", { name: "model", exact: true });
  await folder.focus();
  await expect(folder).toBeFocused();
  await model.focus();
  await expect(model).toBeFocused();
  await model.fill(id);
  await expect(form.locator("#launch-composer-model-results")).toBeVisible();
  await expect(model).not.toHaveAttribute("aria-activedescendant", /.+/);
  await model.press("Enter");
  await expect(form.locator("#launch-composer-model-results")).toHaveCount(0);
  await expect(model).toHaveValue(id);
  await expect(model).toHaveAttribute("aria-expanded", "false");
  await expect(model).not.toHaveAttribute("aria-invalid", "true");
  await expect(model).toBeFocused();
}

/** Equal requests retain their complete unresolved identity, including a
 * nonempty key. Comparing only keys could hide a changed request under one key.
 */
function expectScalarRetry(previous: Record<string, unknown>, next: Record<string, unknown>) {
  expect(next.intent_key).toEqual(expect.any(String));
  expect(next.intent_key).not.toBe("");
  expect(next.intent_key).toBe(previous.intent_key);
  expect(next).toEqual(previous);
}

/** A real choice change must change both identity and substantive body.
 * Excluding only the key prevents a needless remint from passing this check.
 */
function expectScalarChange(previous: Record<string, unknown>, next: Record<string, unknown>) {
  const { intent_key: previousKey, ...previousRequest } = previous;
  const { intent_key: nextKey, ...nextRequest } = next;
  expect(nextKey).toEqual(expect.any(String));
  expect(nextKey).not.toBe("");
  expect(nextKey).not.toBe(previousKey);
  expect(nextRequest).not.toEqual(previousRequest);
}

/** Selecting an already selected scalar choice is an edit gesture, not a
 * new create intent. Each acceptance stays separate from explicit Launch.
 */
test("unchanged launcher tab, harness, permissions and default model retain the retry", async ({ page, request }) => {
  const { form, bodies } = await observeChoiceCreates(page, request, scalarRetryCatalog);
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  const model = form.getByRole("combobox", { name: "model", exact: true });
  await expect(model).toHaveValue("harness default");
  const launch = form.locator(".create-session-submit");
  let previous = await enterCreateChoice(form, launch, bodies);
  expect(previous.intent_key).toEqual(expect.any(String));
  expect(previous.intent_key).not.toBe("");
  expect(previous).toMatchObject({ launch: { harness: "codex", model: null, permissions: "guarded" } });

  for (const choice of ["selected Agent Home", "same harness search", "same permissions search", "already default model"]) {
    await test.step(choice, async () => {
      const before = bodies.length;
      if (choice === "selected Agent Home") {
        const agent = form.getByRole("tab", { name: "agent", exact: true });
        await expect(agent).toHaveAttribute("aria-selected", "true");
        await agent.focus();
        await expect(agent).toBeFocused();
        await agent.press("Home");
        await expect(agent).toHaveAttribute("aria-selected", "true");
        await expect(agent).toBeFocused();
      } else if (choice === "same harness search") {
        await acceptScalarSearch(form, "harness:codex", "Harness: Codex");
        await expect(form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }))
          .toHaveAttribute("aria-pressed", "true");
      } else if (choice === "same permissions search") {
        await acceptScalarSearch(form, "perms:guarded", "Permissions: guarded");
        await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }))
          .toHaveAttribute("aria-pressed", "true");
      } else {
        await acceptScalarModelRow(form, "harness default");
      }
      await expect(launch).toHaveAttribute("aria-disabled", "false");
      expect(bodies, "acceptance itself must not submit").toHaveLength(before);
      const next = await enterCreateChoice(form, launch, bodies);
      expectScalarRetry(previous, next);
      previous = next;
    });
  }
});

/** Same model selections retain identity through search, catalog and custom
 * Enter. Distinct catalog efforts prove that reconciliation still retires
 * identity when a selected model really removes an incompatible effort.
 */
test("launcher model reselection retains retries while real scalar changes replace them", async ({ page, request }) => {
  const { form, bodies } = await observeChoiceCreates(page, request, scalarRetryCatalog);
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  const model = form.getByRole("combobox", { name: "model", exact: true });
  const effort = form.locator(".launch-composer-effort-choice");
  const permissions = form.locator(".launch-composer-permissions-choice");
  const launch = form.locator(".create-session-submit");
  await acceptScalarModelRow(form, "scalar-alpha");
  await effort.getByRole("button", { name: "high", exact: true }).click();
  await expect(effort.getByRole("button", { name: "high", exact: true })).toHaveAttribute("aria-pressed", "true");
  expect(bodies, "setting the initial choices does not launch").toEqual([]);
  let previous = await enterCreateChoice(form, launch, bodies);
  expect(previous.intent_key).toEqual(expect.any(String));
  expect(previous.intent_key).not.toBe("");
  expect(previous).toMatchObject({ launch: { model: "scalar-alpha", effort: "high", permissions: "guarded" } });

  for (const surface of ["search", "catalog row"] as const) {
    await test.step(`same model through ${surface}`, async () => {
      const before = bodies.length;
      if (surface === "search") {
        await acceptScalarSearch(form, "model:scalar-alpha", "Model: scalar-alpha (Codex)");
        await expect(model).toHaveValue("scalar-alpha");
        await expect(form.locator("#launch-composer-model-results")).toHaveCount(0);
      } else {
        await acceptScalarModelRow(form, "scalar-alpha");
      }
      expect(bodies, "model acceptance itself must not submit").toHaveLength(before);
      const next = await enterCreateChoice(form, launch, bodies);
      expectScalarRetry(previous, next);
      previous = next;
    });
  }

  let before = bodies.length;
  await acceptScalarModelRow(form, "scalar-beta");
  await expect(effort.getByRole("button", { name: "default", exact: true })).toHaveAttribute("aria-pressed", "true");
  expect(bodies, "a changed model still only edits").toHaveLength(before);
  let next = await enterCreateChoice(form, launch, bodies);
  expect(next).toMatchObject({ launch: { model: "scalar-beta", effort: null } });
  expectScalarChange(previous, next);
  previous = next;

  before = bodies.length;
  await acceptScalarModelRow(form, "scalar-beta");
  expect(bodies, "same changed-model row does not auto-submit").toHaveLength(before);
  next = await enterCreateChoice(form, launch, bodies);
  expectScalarRetry(previous, next);
  previous = next;

  before = bodies.length;
  await effort.getByRole("button", { name: "low", exact: true }).click();
  await expect(effort.getByRole("button", { name: "low", exact: true })).toHaveAttribute("aria-pressed", "true");
  expect(bodies, "effort click does not auto-submit").toHaveLength(before);
  next = await enterCreateChoice(form, launch, bodies);
  expect(next).toMatchObject({ launch: { model: "scalar-beta", effort: "low" } });
  expectScalarChange(previous, next);
  previous = next;

  for (const changed of [true, false]) {
    before = bodies.length;
    await acceptScalarSearch(form, "perms:yolo", "Permissions: yolo");
    await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute("aria-pressed", "true");
    expect(bodies, "permission search does not auto-submit").toHaveLength(before);
    next = await enterCreateChoice(form, launch, bodies);
    expect(next).toMatchObject({ launch: { permissions: "yolo" } });
    if (changed) expectScalarChange(previous, next);
    else expectScalarRetry(previous, next);
    previous = next;
  }

  for (const changed of [true, false]) {
    before = bodies.length;
    await acceptScalarCustomModel(form, "private-scalar-model");
    expect(bodies, "custom-model Enter only accepts, never launches").toHaveLength(before);
    next = await enterCreateChoice(form, launch, bodies);
    expect(next).toMatchObject({ launch: { model: "private-scalar-model", effort: "low" } });
    if (changed) expectScalarChange(previous, next);
    else expectScalarRetry(previous, next);
    previous = next;
  }
});

/** Ordinary optional names normalize only the absent title, retaining visible
 * text and every byte of a nonblank title. Reaccepting its current search Name
 * must likewise leave the complete unresolved request intact.
 */
test("ordinary title no-ops preserve retries without trimming nonblank names", async ({ page, request }) => {
  const { form, bodies } = await observeChoiceCreates(page, request, []);
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  const name = form.getByLabel("name (optional)", { exact: true });
  const launch = form.locator(".create-session-submit");
  await expect(name).toHaveValue("");
  let previous = await enterCreateChoice(form, launch, bodies);
  expect(previous.intent_key).toEqual(expect.any(String));
  expect(previous.intent_key).not.toBe("");
  expect(previous.title).toBeNull();

  let count = bodies.length;
  await name.focus();
  await expect(name).toBeFocused();
  await name.press("Space");
  await expect(name).toHaveValue(" ");
  expect(bodies, "a name keystroke does not launch").toHaveLength(count);
  let next = await enterCreateChoice(form, launch, bodies);
  expect(next.title).toBeNull();
  expectScalarRetry(previous, next);
  await expect(name, "absence normalization must not rewrite the visible draft").toHaveValue(" ");
  previous = next;

  count = bodies.length;
  await name.fill(" work ");
  await expect(name).toHaveValue(" work ");
  expect(bodies).toHaveLength(count);
  next = await enterCreateChoice(form, launch, bodies);
  expect(next.title, "nonblank surrounding bytes remain request data").toBe(" work ");
  expectScalarChange(previous, next);
  previous = next;

  count = bodies.length;
  await name.fill("work");
  expect(bodies).toHaveLength(count);
  next = await enterCreateChoice(form, launch, bodies);
  expect(next.title).toBe("work");
  expectScalarChange(previous, next);
  previous = next;
  for (const [value, changed] of [["work", false], ["other", true]] as const) {
    count = bodies.length;
    await acceptScalarSearch(form, `name:${value}`, `Set session name: ${value}`);
    await expect(name).toHaveValue(value);
    expect(bodies, "Name search acceptance only edits").toHaveLength(count);
    next = await enterCreateChoice(form, launch, bodies);
    expect(next.title).toBe(value);
    if (changed) expectScalarChange(previous, next);
    else expectScalarRetry(previous, next);
    previous = next;
  }
});

for (const editor of ["wrapper", "custom flags"] as const) {
  /** One trailing-space keystroke changes editable text without changing its
   * literal argv. Adding a new word must still replace the request identity.
   */
  test(`valid ${editor} spacing preserves raw text and retry words`, async ({ page, request }) => {
    const { form, bodies } = await observeChoiceCreates(page, request, []);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    if (editor === "custom flags") {
      await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
    }
    const field = form.locator(editor === "wrapper" ? ".launch-composer-wrapper" : ".custom-permission-flags");
    const original = editor === "wrapper" ? "wrapper --" : "--mode ask";
    const words = editor === "wrapper" ? ["wrapper", "--"] : ["--mode", "ask"];
    const property = editor === "wrapper" ? "wrapper_prefix" : "permission_flags";
    if (editor === "custom flags") {
      // Observe Custom's own deferred handoff before fill can supply focus
      // itself and leave that handoff pending when the test moves to Launch.
      await expect(field).toBeFocused();
    }
    await field.fill(original);
    await expect(field).toHaveValue(original);
    const launch = form.locator(".create-session-submit");
    const previous = await enterCreateChoice(form, launch, bodies);
    expect(previous.intent_key).toEqual(expect.any(String));
    expect(previous.intent_key).not.toBe("");
    expect(previous).toMatchObject({ launch: { [property]: words } });

    let count = bodies.length;
    await field.focus();
    await expect(field).toBeFocused();
    await field.press("End");
    await field.press("Space");
    await expect(field).toHaveValue(`${original} `);
    if (editor === "custom flags") await expect(form.locator(".custom-permission-words code")).toHaveText(words);
    expect(bodies, "literal-editor spacing only edits").toHaveLength(count);
    const same = await enterCreateChoice(form, launch, bodies);
    expectScalarRetry(previous, same);
    await expect(field).toHaveValue(`${original} `);

    count = bodies.length;
    await field.focus();
    await expect(field).toBeFocused();
    await field.press("End");
    await field.press("x");
    await expect(field).toHaveValue(`${original} x`);
    expect(bodies).toHaveLength(count);
    const changed = await enterCreateChoice(form, launch, bodies);
    expect(changed).toMatchObject({ launch: { [property]: [...words, "x"] } });
    expectScalarChange(same, changed);
  });
}

/** A real template resets raw permissions to None while Codex submits its
 * effective Guarded mode. Each surface gets a fresh template application so
 * an earlier explicit Guarded action cannot erase the raw-default premise.
 */
test("effective Guarded reacceptance preserves a template-default request", async ({ page, request }) => {
  const template = `retry-permission-default-${randomUUID()}`;
  let bodyFailed = false;
  try {
    const saved = await request.put(`/api/templates/${encodeURIComponent(template)}`, { data: { permissions: null } });
    expect(saved.ok()).toBe(true);
    const stored = await request.get("/api/templates");
    expect(stored.ok()).toBe(true);
    expect((await stored.json()).templates.find((row: { name: string }) => row.name === template)?.fields)
      .toEqual({ permissions: null });
    // Skip the observer's usual Guarded click. The actual template below
    // must be the last permission write before every baseline request.
    const { form, bodies } = await observeChoiceCreates(page, request, [], false);
    const codex = form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true });
    await codex.click();
    await expect(form).toHaveAttribute("data-initial-templates-ready", "true");
    const permissions = form.locator(".launch-composer-permissions-choice");
    const guarded = permissions.getByRole("button", { name: "guarded", exact: true });
    const launch = form.locator(".create-session-submit");
    let previous: Record<string, unknown> | undefined;
    for (const surface of ["pointer", "choice Enter", "search"] as const) {
      await test.step(`raw template default to Guarded via ${surface}`, async () => {
        let count = bodies.length;
        await acceptScalarSearch(form, `tl:${template}`, `Template: ${template}`);
        await expect(codex).toHaveAttribute("aria-pressed", "true");
        await expect(guarded).toHaveAttribute("aria-pressed", "true");
        expect(bodies, "permission-default template acceptance does not launch").toHaveLength(count);
        const baseline = await enterCreateChoice(form, launch, bodies);
        expect(baseline.intent_key).toEqual(expect.any(String));
        expect(baseline.intent_key).not.toBe("");
        expect(baseline).toMatchObject({ launch: { harness: "codex", permissions: "guarded" } });
        count = bodies.length;
        let next: Record<string, unknown>;
        if (surface === "choice Enter") {
          // Choice Enter intentionally accepts and submits through the
          // ordinary Launch path; the existing helper checks exactly one POST.
          next = await enterCreateChoice(form, guarded, bodies);
        } else {
          if (surface === "pointer") await guarded.click();
          else await acceptScalarSearch(form, "perms:guarded", "Permissions: guarded");
          await expect(guarded).toHaveAttribute("aria-pressed", "true");
          expect(bodies, "pointer/search only accepts the effective permission").toHaveLength(count);
          next = await enterCreateChoice(form, launch, bodies);
        }
        expectScalarRetry(baseline, next);
        previous = next;
      });
    }
    const count = bodies.length;
    const yolo = permissions.getByRole("button", { name: "yolo", exact: true });
    await yolo.click();
    await expect(yolo).toHaveAttribute("aria-pressed", "true");
    expect(bodies).toHaveLength(count);
    const changed = await enterCreateChoice(form, launch, bodies);
    expect(changed).toMatchObject({ launch: { permissions: "yolo" } });
    expectScalarChange(previous!, changed);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["pending fixture routes", () => page.unrouteAll({ behavior: "wait" })],
      ["permission-default template", async () => {
        const removed = await request.delete(`/api/templates/${encodeURIComponent(template)}`);
        expect(removed.ok()).toBe(true);
      }],
    ]);
  }
});

/** Observe a single native replacement without assigning or dispatching input.
 * An engine that suppresses identical insertion fails the delivery assertion;
 * it cannot silently count as callback coverage.
 */
async function replaceOwnedTextOnce(page: Page, field: Locator, value: string) {
  await field.focus();
  await expect(field).toBeFocused();
  await expect(field).toBeEditable();
  await field.press("ControlOrMeta+A");
  const selection = await field.evaluate((element) => {
    const node = element as HTMLInputElement | HTMLTextAreaElement;
    return { value: node.value, start: node.selectionStart, end: node.selectionEnd };
  });
  expect(selection.start).toBe(0);
  expect(selection.end).toBe(selection.value.length);
  const capture = await field.evaluateHandle((element) => {
    const node = element as HTMLInputElement | HTMLTextAreaElement;
    const events: { value: string; inputType: string | null; isTrusted: boolean }[] = [];
    const listener = (event: Event) => {
      events.push({ value: node.value, inputType: event instanceof InputEvent ? event.inputType : null, isTrusted: event.isTrusted });
    };
    node.addEventListener("input", listener);
    return { events, remove: () => node.removeEventListener("input", listener) };
  });
  try {
    await page.keyboard.insertText(value);
    await expect(field).toHaveValue(value);
    const events = await capture.evaluate((record) => record.events);
    expect(events, "one native input must reach the active editor").toHaveLength(1);
    expect(events[0]).toMatchObject({ value, isTrusted: true });
    return events;
  } finally {
    await capture.evaluate((record) => record.remove());
    await capture.dispose();
  }
}

for (const kind of ["title", "folder", "command", "resume", "prompt", "conversation"] as const) {
  /** Already-owned same-byte replacement is one input transaction, not a
   * series of genuinely different prefixes. A different-byte replacement
   * proves the editor still reaches the active submitted request.
   */
  test(`one native same-owned ${kind} replacement retains its retry`, async ({ page, request }, info) => {
    const { form, bodies } = await observeChoiceCreates(page, request, []);
    let field: Locator;
    let initial: string;
    let edited: string;
    if (kind === "command" || kind === "resume") {
      await form.getByRole("tab", { name: "command", exact: true }).click();
      await form.getByRole("radio", { name: "no", exact: true }).check();
      const invocation = form.getByLabel("agent command", { exact: true });
      if (kind === "resume") {
        await form.locator(".launch-command-agent").selectOption("claude");
        await invocation.fill("claude {farhelm_args}");
        await form.locator(".launch-command-resume-toggle").check();
        await expect(form.locator(".launch-command-resume-toggle")).toBeChecked();
        field = form.getByLabel("resume command", { exact: true });
        initial = "claude --resume {conversation} {farhelm_args}";
      } else {
        field = invocation;
        initial = "printf hello";
      }
      // Command strings are literal request bytes. A trailing space is a
      // real change even when a shell would happen to parse the same argv.
      edited = `${initial} `;
    } else {
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
      if (kind === "title") {
        field = form.getByLabel("name (optional)", { exact: true });
        initial = "work";
        edited = " work ";
      } else if (kind === "folder") {
        field = form.getByLabel("folder", { exact: true });
        initial = "/tmp";
        edited = "/var/tmp";
      } else if (kind === "prompt") {
        field = form.getByLabel("initial prompt", { exact: true });
        initial = "hello";
        edited = "hello ";
        await expect(form.locator(".named-conversation")).toHaveValue("");
      } else {
        field = form.locator(".named-conversation");
        initial = "conversation-1";
        edited = "conversation-2";
        await expect(form.getByLabel("initial prompt", { exact: true })).toHaveValue("");
      }
    }
    await expect(field).toBeVisible();
    await field.fill(initial);
    await expect(field).toHaveValue(initial);
    const launch = form.locator(".create-session-submit");
    const previous = await enterCreateChoice(form, launch, bodies);
    expect(previous.intent_key).toEqual(expect.any(String));
    expect(previous.intent_key).not.toBe("");
    const expectedBody = (text: string) => kind === "command" ? { command: { command: text } }
      : kind === "resume" ? { command: { resume: text } }
      : kind === "title" ? { title: text }
      : kind === "folder" ? { cwd: text }
      : kind === "prompt" ? { prompt: text }
      : { conversation: text };
    expect(previous).toMatchObject(expectedBody(initial));
    if (kind === "prompt") expect(previous).not.toHaveProperty("conversation");
    if (kind === "conversation") expect(previous).not.toHaveProperty("prompt");

    let count = bodies.length;
    const sameDelivery = await replaceOwnedTextOnce(page, field, initial);
    await info.attach("native-same-text-input-delivery", {
      body: JSON.stringify({ kind, events: sameDelivery }), contentType: "application/json",
    });
    expect(bodies, "a native text replacement alone does not launch").toHaveLength(count);
    const same = await enterCreateChoice(form, launch, bodies);
    expectScalarRetry(previous, same);

    count = bodies.length;
    const changedDelivery = await replaceOwnedTextOnce(page, field, edited);
    await info.attach("native-changed-text-input-delivery", {
      body: JSON.stringify({ kind, events: changedDelivery }), contentType: "application/json",
    });
    expect(bodies).toHaveLength(count);
    const changed = await enterCreateChoice(form, launch, bodies);
    expect(changed).toMatchObject(expectedBody(edited));
    expectScalarChange(same, changed);
  });
}
