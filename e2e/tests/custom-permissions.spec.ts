/** Custom flags remain an editable, explicit YOLO choice across launcher surfaces.
 * Real helm refusals and template storage exercise the browser/API boundary;
 * these tests never confirm a launch into an installed vendor agent.
 */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { type Page, type Route } from "@playwright/test";
import { agentLaunchRow, listHosts, localHostId, openRowMenu, patchPreferences, readPreferences, SESSION_LISTING, setLocalYoloWithoutAsking } from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

/** Wait for the real launcher before acting on its agent and permission controls. */
async function openNew(page: Page) {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Claude", exact: true }).click();
  return form;
}

/** Search must focus the custom editor; invalid quotes remain repairable, and
 * words retain literal expansion syntax. Changing agents cannot reuse flags
 * written for a different command-line interface.
 */
test("custom permission search focuses literal words and switching agents clears them", async ({ page }) => {
  const form = await openNew(page);
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("perms:custom");
  await form.getByRole("option", { name: "Permissions: custom", exact: true }).click();
  const flags = form.locator(".custom-permission-flags");
  await expect(flags).toBeFocused();
  await flags.fill("  ");
  await expect(flags).toHaveValue("  ");
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  await flags.fill("--mode '");
  await expect(flags).toHaveValue("--mode '");
  await expect(form.locator(".custom-permission-choice [role=alert]")).toContainText("invalid permission flags");
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  await flags.fill('--mode "ask me" "$HOME"');
  await expect(form.locator(".custom-permission-words code")).toHaveText(["--mode", "ask me", "$HOME"]);
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await expect(flags).toHaveCount(0);
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
  await expect(flags).toHaveValue("");
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Goose", exact: true }).click();
  await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true })).toHaveCount(0);
});

/** An input queued behind Launch cannot replace its permission words or retire
 * its retry key. Dispatch past the cosmetic disabled state to exercise the
 * actual callback's live operation check, then prove ordinary edits still work.
 */
test("queued custom permission edits cannot change an in-flight launch", async ({ page }) => {
  const held = routeGate();
  const bodies: any[] = [];
  const completions: Promise<void>[] = [];
  const endpoint = "**/api/sessions";
  const refuseCreate = async (route: Route) => {
    if (route.request().method() !== "POST") return route.continue();
    const attempt = bodies.push(route.request().postDataJSON());
    const completion = (async () => {
      if (attempt === 1) await held.wait();
      // No definitely-unaccepted marker: an ambiguous refusal must keep the
      // original request's key available for an unchanged explicit retry.
      await fulfillAsHelm(route, {
        status: 503,
        contentType: "text/plain",
        body: `held custom refusal ${attempt}`,
      });
    })();
    completions.push(completion);
    await completion;
  };
  await page.route(endpoint, refuseCreate);
  try {
    const form = await openNew(page);
    await form.getByLabel("folder", { exact: true }).fill("/tmp");
    const custom = form.locator(".launch-composer-permissions-choice")
      .getByRole("button", { name: "custom", exact: true });
    await custom.click();
    const flags = form.locator(".custom-permission-flags");
    const words = form.locator(".custom-permission-words code");
    const launch = form.locator(".create-session-submit");
    const originalWords = ["--mode", "ask me", "$HOME"];
    await flags.fill("--mode 'ask me' '$HOME'");
    await expect(words).toHaveText(originalWords);
    await expect(custom).toHaveAttribute("aria-pressed", "true");
    await expect(launch).toBeEnabled();
    await launch.click();
    await expect.poll(() => bodies.length, { message: "the first create reached its held route" }).toBe(1);
    expect(bodies[0].intent_key).toEqual(expect.any(String));
    expect(bodies[0].intent_key).not.toBe("");
    expect(bodies[0].launch).toMatchObject({ permissions: "custom", permission_flags: originalWords });
    await expect(flags).toBeDisabled();
    await expect(launch).toBeDisabled();

    // A normal fill would stop at disabled rather than challenge the Rust
    // callback. Parsed words and the next request observe the draft; the DOM
    // value set here is deliberately not an oracle for reactive state.
    await flags.evaluate((element: HTMLInputElement) => {
      element.value = "--mode changed";
      element.dispatchEvent(new Event("input", { bubbles: true }));
    });
    held.release();
    await expect(form.locator(".create-session-error")).toContainText("held custom refusal 1");
    await expect(launch).toBeEnabled();
    await expect(words).toHaveText(originalWords);
    await expect(custom).toHaveAttribute("aria-pressed", "true");

    await launch.click();
    await expect(form.locator(".create-session-error")).toContainText("held custom refusal 2");
    await expect(launch).toBeEnabled();
    expect(bodies).toHaveLength(2);
    expect(bodies[1].intent_key).toBe(bodies[0].intent_key);
    expect(bodies[1]).toEqual(bodies[0]);

    // An enabled edit must still reach the same callback and create a new
    // intent; an inert editor would otherwise satisfy the refusal assertions.
    await flags.fill("--mode edited");
    await expect(words).toHaveText(["--mode", "edited"]);
    await launch.click();
    await expect(form.locator(".create-session-error")).toContainText("held custom refusal 3");
    await expect(launch).toBeEnabled();
    expect(bodies).toHaveLength(3);
    expect(bodies[2].launch).toMatchObject({ permissions: "custom", permission_flags: ["--mode", "edited"] });
    expect(bodies[2].intent_key).toEqual(expect.any(String));
    expect(bodies[2].intent_key).not.toBe("");
    expect(bodies[2].intent_key).not.toBe(bodies[0].intent_key);
  } finally {
    held.release();
    await page.unroute(endpoint, refuseCreate);
    // Drain only routes that actually entered; setup failures before Launch
    // have no pending route to wait for and create no session to remove.
    await Promise.all(completions);
  }
});

/** Reset ends the source agent's ownership of custom words. Same-agent choices
 * must still preserve literal editor text, so clearing on every agent choice
 * cannot satisfy this regression by discarding useful input.
 */
test("reset then agent search cannot restore another agent's custom flags", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const bodies: Record<string, unknown>[] = [];
  const completions: Promise<void>[] = [];
  const endpoint = "**/api/sessions";
  const refuseCreate = async (route: Route) => {
    if (route.request().method() !== "POST") return route.continue();
    const attempt = bodies.push(route.request().postDataJSON());
    const completion = fulfillAsHelm(route, {
      status: 503,
      contentType: "text/plain",
      body: `reset flags fixture refused create ${attempt}`,
    });
    completions.push(completion);
    await completion;
  };
  let bodyFailed = false;
  try {
    // Seed the real preference before loading the page. The bug needs a
    // remembered mode supported by both agents, not a fabricated UI label.
    await patchPreferences(request, { remembered_permissions: "yolo" });
    expect((await readPreferences(request)).remembered_permissions).toBe("yolo");
    await page.route(endpoint, refuseCreate);
    const form = await openNew(page);
    const permissions = form.locator(".launch-composer-permissions-choice");
    const custom = permissions.getByRole("button", { name: "custom", exact: true });
    const yolo = permissions.getByRole("button", { name: "yolo", exact: true });
    const flags = form.locator(".custom-permission-flags");
    const words = form.locator(".custom-permission-words code");
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    const claude = form.locator(".launch-composer-harness-choice")
      .getByRole("button", { name: "Claude", exact: true });
    await expect(claude).toHaveAttribute("aria-pressed", "true");
    await expect(yolo).toHaveAttribute("aria-pressed", "true");
    await custom.click();
    await expect(flags).toBeFocused();
    const original = '  --mode "claude only"   ';
    await flags.fill(original);
    await expect(words).toHaveText(["--mode", "claude only"]);

    // These controls prove that normal re-selection retains exact spacing.
    // Reset below must end ownership even though its remembered mode survives.
    await claude.click();
    await expect(flags).toHaveValue(original);
    await search.fill("harness:claude");
    await form.getByRole("option", { name: "Harness: Claude", exact: true }).click();
    await expect(search).toHaveValue("");
    await expect(flags).toHaveValue(original);
    await expect(custom).toHaveAttribute("aria-pressed", "true");

    await form.getByRole("button", { name: "reset choices", exact: true }).click();
    await expect(claude).toHaveAttribute("aria-pressed", "false");
    await expect(form.locator('.launch-composer-harness-choice button[aria-pressed="true"]')).toHaveCount(0);
    await expect(yolo).toHaveAttribute("aria-pressed", "true");
    await expect(flags).toHaveCount(0);
    await search.fill("harness:codex");
    await form.getByRole("option", { name: "Harness: Codex", exact: true }).click();
    await expect(search).toHaveValue("");
    await expect(form.locator(".launch-composer-harness-choice")
      .getByRole("button", { name: "Codex", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(yolo).toHaveAttribute("aria-pressed", "true");
    await custom.click();
    await expect(flags).toHaveValue("");
    await expect(words).toHaveCount(0);
    // Custom schedules its own focus. Observe that handoff before fill can
    // supply focus and leave a pending callback able to interrupt folder input.
    await expect(flags).toBeFocused();
    expect(bodies, "picker and Reset actions do not create sessions").toHaveLength(0);

    // The completed stamped refusal observes actual submission without
    // launching a vendor process; the DOM buffer alone is not the oracle.
    await flags.fill('--mode "codex only"');
    await expect(words).toHaveText(["--mode", "codex only"]);
    const folder = form.getByLabel("folder", { exact: true });
    await folder.fill("/tmp");
    await expect(folder).toHaveValue("/tmp");
    await expect(flags).toHaveValue('--mode "codex only"');
    const launch = form.locator(".create-session-submit");
    await expect(launch).toHaveAttribute("aria-disabled", "false");
    await launch.click();
    await expect(form.locator(".create-session-error")).toContainText("reset flags fixture refused create 1");
    await expect(launch).toHaveAttribute("aria-disabled", "false");
    expect(bodies).toHaveLength(1);
    expect(bodies[0].intent_key).toEqual(expect.any(String));
    expect(bodies[0].intent_key).not.toBe("");
    expect(bodies[0].launch).toMatchObject({ harness: "codex", permissions: "custom", permission_flags: ["--mode", "codex only"] });
    expect(bodies[0]).not.toHaveProperty("confirm_yolo");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["create route", async () => {
        await page.unroute(endpoint, refuseCreate);
        await Promise.all(completions);
      }],
      ["remembered permissions", () => patchPreferences(request, { remembered_permissions: previous.remembered_permissions ?? null })],
    ]);
  }
});

/** Model search and same-agent choices must not reconstruct the editor from
 * parsed words: unfinished quotes and intentional spacing are still drafts.
 * A real agent change retires them because flags belong to that agent's CLI.
 */
test("same-agent search and chips preserve custom permission drafts byte for byte", async ({ page, request }) => {
  const response = await request.get("/api/launch-catalog");
  expect(response.ok()).toBe(true);
  const catalog = await response.json() as { harness: string; id: string }[];
  const model = catalog.find((entry) => entry.harness === "claude");
  expect(model, "the owned stack must offer a Claude model for same-agent selection").toBeTruthy();
  const form = await openNew(page);
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  const custom = form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true });
  await custom.click();
  const flags = form.locator(".custom-permission-flags");
  for (const draft of ["--permission-mode 'ask me", '  --mode "ask me"   \'$HOME\'  ']) {
    await flags.fill(draft);
    await expect(flags).toHaveValue(draft);
    if (draft.startsWith("--permission-mode")) {
      await expect(form.locator(".custom-permission-choice [role=alert]")).toContainText("invalid permission flags");
    } else {
      await expect(form.locator(".custom-permission-words code")).toHaveText(["--mode", "ask me", "$HOME"]);
    }
    for (const choice of [
      { query: `model:${model!.id}`, label: `Model: ${model!.id} (Claude)` },
      { query: "harness:claude", label: "Harness: Claude" },
    ]) {
      await search.fill(choice.query);
      const option = form.getByRole("option", { name: choice.label, exact: true });
      await expect(option).toBeVisible();
      await option.click();
      await expect(search).toHaveValue("");
      await expect(custom).toHaveAttribute("aria-pressed", "true");
      await expect(flags).toHaveValue(draft);
    }
    const currentAgent = form.locator(".launch-composer-harness-choice")
      .getByRole("button", { name: "Claude", exact: true });
    await currentAgent.click();
    await expect(flags).toHaveValue(draft);
    if (draft.startsWith("--permission-mode")) {
      // An unfinished quote forbids submission, so Enter exercises chip
      // selection without starting an installed vendor agent.
      await expect(form.locator(".create-session-submit")).toBeDisabled();
      await currentAgent.focus();
      await expect(currentAgent).toBeFocused();
      await currentAgent.press("Enter");
      await expect(flags).toHaveValue(draft);
    }
  }
  await search.fill("harness:codex");
  await form.getByRole("option", { name: "Harness: Codex", exact: true }).click();
  await expect(flags).toHaveCount(0);
  await custom.click();
  await expect(flags).toHaveValue("");
});

/** An unfinished permission editor cannot make a supported effort incompatible
 * with a custom model. Both empty text and malformed quotes remain repairable
 * without silently changing the independent reasoning choice.
 */
test("custom model acceptance keeps compatible effort with incomplete permission flags", async ({ page, request }) => {
  const response = await request.get("/api/launch-catalog");
  expect(response.ok()).toBe(true);
  const catalog = await response.json() as { harness: string; efforts: string[] }[];
  expect(catalog.some(entry => entry.harness === "claude" && entry.efforts.includes("high"))).toBe(true);
  const form = await openNew(page);
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
  const flags = form.locator(".custom-permission-flags");
  const high = form.locator(".launch-composer-effort-choice").getByRole("button", { name: "high", exact: true });
  const model = form.getByRole("combobox", { name: "model", exact: true });
  for (const [round, draft] of ["", "--mode '"].entries()) {
    await flags.fill(draft);
    await high.click();
    await expect(high).toHaveAttribute("aria-pressed", "true");
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    // A distinct id per round, checked once the field closes: an open field
    // shows the typed draft, so only the closed one proves Enter accepted it,
    // and a reused id would let a later round pass on the earlier commit.
    const id = `private-permission-test-model-${round}`;
    await model.fill(id);
    await expect(model).toHaveAttribute("aria-expanded", "true");
    await model.press("Enter");
    await expect(model).toHaveAttribute("aria-expanded", "false");
    await expect(model).toHaveValue(id);
    await expect(high).toHaveAttribute("aria-pressed", "true");
    await expect(flags).toHaveValue(draft);
    await expect(form.locator(".create-session-submit")).toBeDisabled();
  }
});

/** Enter on Custom opens its required editor instead of attempting a launch.
 * The focus assertion distinguishes the handoff from a disabled-primary refusal.
 */
test("custom choice Enter focuses its empty editor without submitting", async ({ page }) => {
  const form = await openNew(page);
  const posts: string[] = [];
  page.on("request", request => { if (request.method() === "POST") posts.push(request.url()); });
  const custom = form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true });
  await custom.focus();
  await expect(custom).toBeFocused();
  await custom.press("Enter");
  await expect(form.locator(".custom-permission-flags")).toBeFocused();
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  expect(posts).toEqual([]);
});

for (const action of ["clone", "replace-with"] as const) {
  /** An injected listing retains real host metadata while exposing a custom
   * launch the fake agent does not generate. Refused mutation routes keep the
   * proof on the editor/request boundary, without launching a vendor agent.
   */
  test(`${action} preserves custom words and shows the stored YOLO provenance`, async ({ page }) => {
    const id = "77777777-3333-4444-5555-666666666666";
    const words = ["--mode", "ask me", "$HOME", "hidden\u200Bword"];
    const selection = { harness: "claude", model: null, effort: null, permissions: "custom", permission_flags: words };
    await page.route(SESSION_LISTING, async route => {
      const response = await route.fetch();
      const listing = await response.json();
      expect(listing.sessions.length, "the stack must supply real host metadata").toBeGreaterThan(0);
      listing.sessions.push({ ...listing.sessions[0], id, title: "custom-source", cwd: "/tmp",
        invocation: "misleading-display", launch: { ...agentLaunchRow(selection),
          start: ["claude", ...words, "{farhelm_args}"],
          resume: ["claude", ...words, "--resume", "{conversation}", "{farhelm_args}"] },
        // This resumable command has no captured conversation yet. The offer
        // must use the actual wire vocabulary so the listing still decodes.
        status: { state: "interrupted" }, restart_offer: "not_captured" });
      listing.total += 1;
      await route.fulfill({ response, json: listing });
    });
    const bodies: any[] = [];
    await page.route(action === "clone" ? "**/api/sessions" : `**/api/sessions/${encodeURIComponent(id)}/replace`, async route => {
      if (route.request().method() !== "POST") return route.continue();
      bodies.push(route.request().postDataJSON());
      await fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: "controlled custom refusal" });
    });
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible();
    const shield = row.locator(".permission-glyph");
    await expect(shield).toHaveAttribute("data-glyph", "yolo");
    await shield.hover();
    const help = page.locator("body > .farhelm-tooltip");
    await expect(help).toContainText("custom permission flags");
    await expect(help).toContainText("treats this as YOLO");
    await expect(help).toContainText("hidden<U+200B>word");
    await expect(help).toContainText("Launched with: claude");
    await expect(help).not.toContainText("misleading-display");
    await openRowMenu(row);
    await row.locator(`.session-row-${action}`).click();
    const form = page.locator(".create-session-form");
    const flags = form.locator(".custom-permission-flags");
    await expect(flags).toHaveValue("--mode 'ask me' '$HOME' hidden<U+200B>word");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".create-session-error")).toContainText("controlled custom refusal");
    expect(bodies).toHaveLength(1);
    expect(action === "clone" ? bodies[0].launch : bodies[0].with.launch).toMatchObject(selection);
    await flags.fill('--mode "edited words"');
    await form.locator(".create-session-submit").click();
    await expect.poll(() => bodies.length).toBe(2);
    expect((action === "clone" ? bodies[1].launch : bodies[1].with.launch).permission_flags).toEqual(["--mode", "edited words"]);
  });
}

/** The real host guard must ask even when custom words name a guarded vendor
 * mode. Inspect the actual refused request, then cancel: no vendor is started.
 */
test("custom permission words reach the real YOLO refusal with their own explanation", async ({ page, request }) => {
  const host = await localHostId(request);
  const before = (await listHosts(request)).find((row) => row.id === host);
  expect(before, "local host fixture must exist").toBeTruthy();
  try {
    await setLocalYoloWithoutAsking(request, false);
    const form = await openNew(page);
    await form.getByLabel("folder", { exact: true }).fill("/tmp");
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
    await form.locator(".custom-permission-flags").fill("--permission-mode auto");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    const [response] = await Promise.all([
      page.waitForResponse((response) => response.request().method() === "POST" && new URL(response.url()).pathname === "/api/sessions"),
      form.locator(".create-session-submit").click(),
    ]);
    expect(response.status()).toBe(409);
    const body = response.request().postDataJSON();
    expect(body.launch.permissions).toBe("custom");
    expect(body.launch.permission_flags).toEqual(["--permission-mode", "auto"]);
    const confirmation = form.locator(".yolo-confirmation");
    await expect(confirmation).toContainText("uses custom permission flags");
    await expect(confirmation).toContainText("treats the session as YOLO");
    await confirmation.locator(".yolo-cancel").click();
    await expect(confirmation).toHaveCount(0);
  } finally {
    await setLocalYoloWithoutAsking(request, before!.yolo_without_asking);
  }
});

/** Stored template words must reappear as editable quoted text, then clear
 * when a later permission template overrides them. A template with words but
 * no Custom declaration is refused at the actual save endpoint. Each template
 * deletion is independent so a refused cleanup cannot pollute later catalog tests.
 */
test("custom template words round trip into the launcher and clear on a stacked permission", async ({ page, request }) => {
  const name = `e2e-custom-${Date.now()}`;
  const later = `${name}-guarded`;
  const other = `${name}-other`;
  const otherWords = ["--mode", "second template words"];
  const fields = { kind: "agent", agent: "claude", permissions: "custom", permission_flags: ["--mode", "ask me", "$HOME"] };
  let bodyFailed = false;
  try {
    expect((await request.put(`/api/templates/${name}`, { data: fields })).ok()).toBe(true);
    expect((await request.put(`/api/templates/${other}`, { data: { ...fields, permission_flags: otherWords } })).ok()).toBe(true);
    expect((await request.put(`/api/templates/${later}`, { data: { kind: "agent", permissions: "guarded" } })).ok()).toBe(true);
    const refused = await request.put(`/api/templates/${name}-invalid`, { data: { ...fields, permissions: "guarded" } });
    expect(refused.status()).toBe(400);
    const form = await openNew(page);
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill(`tl:${name}`);
    await form.getByRole("option", { name: `Template: ${name}`, exact: true }).click();
    await expect(form.locator(".custom-permission-words code")).toHaveText(fields.permission_flags);
    await expect(form.locator(".custom-permission-flags")).toHaveValue("--mode 'ask me' '$HOME'");
    await search.fill(`tl:${later}`);
    await form.getByRole("option", { name: `Template: ${later}`, exact: true }).click();
    await expect(form.locator(".custom-permission-flags")).toHaveCount(0);
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
    await expect(form.locator(".custom-permission-flags")).toHaveValue("");
    await form.locator(".launch-composer-cancel").click();
    await page.getByRole("button", { name: "templates", exact: true }).click();
    const dialog = page.locator(".templates-dialog");
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await dialog.locator(".templates-row-name").filter({ hasText: new RegExp(`^${name}$`) }).click();
    const flags = dialog.locator(".custom-permission-flags");
    await expect(flags).toHaveValue("--mode 'ask me' '$HOME'");
    let writes = 0;
    page.on("request", request => {
      if (request.method() === "PUT" && new URL(request.url()).pathname.startsWith("/api/templates/")) writes++;
    });
    await flags.fill("--mode '");
    await expect(dialog.locator(".custom-permission-choice [role=alert]")).toContainText("invalid permission flags");
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator('.templates-error[role="status"]')).toContainText("custom permissions need permission flags");
    expect(writes, "an invalid draft must be refused before its API write").toBe(0);
    await flags.fill('--mode "edited template words"');
    await expect(dialog.locator(".templates-save")).toBeEnabled();
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    const stored = await request.get("/api/templates");
    expect(stored.ok()).toBe(true);
    expect((await stored.json()).templates.find((template: { name: string }) => template.name === name).fields.permission_flags)
      .toEqual(["--mode", "edited template words"]);
    // Switching saved sources must replace field-local raw text, without a
    // blanket parsed-word sync that would erase a quote being repaired.
    await dialog.locator(".templates-row-name").filter({ hasText: new RegExp(`^${other}$`) }).click();
    await expect(flags).toHaveValue("--mode 'second template words'");
    await expect(dialog.locator(".custom-permission-words code")).toHaveText(otherWords);
    await dialog.locator(".templates-add").click();
    await dialog.locator(".templates-add-menu").getByRole("button", { name: "model", exact: true }).click();
    await expect(dialog.locator(".templates-save")).toBeEnabled();
    await dialog.locator(".templates-save").click();
    await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
    const after = await request.get("/api/templates");
    expect(after.ok()).toBe(true);
    expect((await after.json()).templates.find((template: { name: string }) => template.name === other).fields.permission_flags)
      .toEqual(otherWords);
    await flags.fill("--broken '");
    await expect(flags).toHaveValue("--broken '");
    await expect(dialog.locator(".custom-permission-choice [role=alert]")).toContainText("invalid permission flags");
    await dialog.locator(".templates-row-name").filter({ hasText: new RegExp(`^${name}$`) }).click();
    await dialog.locator(".templates-discard").click();
    await expect(flags).toHaveValue("--mode 'edited template words'");
    await expect(dialog.locator(".custom-permission-choice [role=alert]")).toHaveCount(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [name, later, other, `${name}-invalid`].map(
      (template): [string, () => Promise<void>] => [template, async () => {
        const response = await request.delete(`/api/templates/${template}`);
        expect(response.ok() || response.status() === 404).toBe(true);
      }],
    ));
  }
});
