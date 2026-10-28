/**
 * The New dialog's launch-kind tabs (SPEC.md, Launch composer and Command
 * launch): one tab per launch kind, each showing only its own fields and
 * keeping its own draft, and search that moves to the agent tab only for an
 * agent-launch choice.
 *
 * Nothing is launched: each test drives the dialog against the real stack's
 * catalog and history and inspects what it holds.
 */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { answerYolo } from "./helpers/term";

/** Open New and return its form, waiting until it is on screen. */
async function openNew(page: Page) {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  return form;
}

/**
 * Spec: New opens on the agent tab with no agent type chosen, and each tab
 * shows only its own fields; a command typed and answered on the command
 * tab, and an agent type picked on the agent tab, both survive switching
 * back and forth.
 *
 * Why: the two kinds launch different things, so a field from one tab
 * showing on the other would let a value the user cannot see decide what
 * runs; and SPEC.md requires each tab to keep its draft, so a look at the
 * other tab never costs what was typed.
 */
test("New opens on the agent tab, and each tab keeps its own fields and draft", async ({ page }) => {
  const form = await openNew(page);
  const agentTab = form.getByRole("tab", { name: "agent", exact: true });
  const commandTab = form.getByRole("tab", { name: "command", exact: true });
  const harness = form.locator(".launch-composer-harness-choice");
  await expect(agentTab).toHaveAttribute("aria-selected", "true");
  await expect(commandTab).toHaveAttribute("aria-selected", "false");
  await expect(harness.locator('[aria-pressed="true"]')).toHaveCount(0);
  await expect(form.getByLabel("agent command")).toHaveCount(0);

  await commandTab.click();
  await expect(commandTab).toHaveAttribute("aria-selected", "true");
  // None of the agent tab's fields, not only its agent picker.
  await expect(harness).toHaveCount(0);
  await expect(form.locator(".launch-composer-model-choice")).toHaveCount(0);
  await expect(form.locator(".launch-composer-summary")).toHaveCount(0);
  await form.getByLabel("agent command").fill("sleep 300");
  await answerYolo(form, true);

  await agentTab.click();
  await expect(form.getByLabel("agent command")).toHaveCount(0);
  await harness.getByRole("button", { name: "Claude", exact: true }).click();
  await expect(harness.getByRole("button", { name: "Claude", exact: true })).toHaveAttribute("aria-pressed", "true");

  await commandTab.click();
  await expect(form.getByLabel("agent command")).toHaveValue("sleep 300");
  await expect(form.locator(".launch-composer-launch-context")).toContainText("Command · ");
  await expect(form.locator(".launch-composer-launch-context")).not.toContainText("Claude");
  await expect(form.locator(".create-session-submit")).toHaveAccessibleName(/^launch Command · /);
  await expect(
    form.getByRole("group", { name: "runs in YOLO mode" }).getByLabel("yes (YOLO)", { exact: true }),
  ).toBeChecked();
  await agentTab.click();
  await expect(harness.getByRole("button", { name: "Claude", exact: true })).toHaveAttribute("aria-pressed", "true");
});

/**
 * Spec: a prefix is parsed into literal words in the submitted agent selection;
 * an unfinished quote remains visible and prevents a create request.
 *
 * Why: dropping invalid text would silently run the agent without the wrapper
 * the user thought they had chosen.
 */
test("agent wrapper input refuses unfinished text and submits parsed words", async ({ page, request }) => {
  const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"];
  expect(build, "the controlled refusal must retain the live helm's build identity").toBeTruthy();
  const bodies: Record<string, any>[] = [];
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    bodies.push(route.request().postDataJSON());
    await route.fulfill({
      status: 400,
      headers: { "x-farhelm-build": build, "content-type": "text/plain" },
      body: "wrapper fixture refused create",
    });
  });

  const form = await openNew(page);
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  const wrapper = form.locator(".launch-composer-wrapper");
  await expect(wrapper).toBeVisible();
  await wrapper.fill("with-endpoint 'unfinished");
  await expect(wrapper).toHaveAttribute("aria-invalid", "true");
  await expect(form.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "true");
  await form.locator(".create-session-submit").click({ force: true });
  await expect(form.locator(".launch-composer-refusal")).toContainText("invalid wrapper prefix: missing closing quote");
  expect(bodies, "invalid wrapper text must not become a create").toHaveLength(0);

  await wrapper.fill("with-endpoint --profile 'work profile' --");
  await expect(wrapper).toHaveAttribute("aria-invalid", "false");
  await expect(form.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "false");
  await form.locator(".create-session-submit").click();
  await expect(form.locator(".create-session-error")).toContainText("wrapper fixture refused create");
  expect(bodies).toHaveLength(1);
  expect(bodies[0]).toMatchObject({
    launch: { harness: "codex", wrapper_prefix: ["with-endpoint", "--profile", "work profile", "--"] },
  });
});

/**
 * Spec: saving the command tab projects only command choices. A malformed
 * wrapper draft waits on the agent tab and cannot veto an unrelated save.
 *
 * Why: the two tabs retain independent drafts, so a hidden validation error
 * must not make the visible command setup impossible to save.
 */
test("a dormant invalid wrapper does not block saving a command template", async ({ page }) => {
  const form = await openNew(page);
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  await form.locator(".launch-composer-wrapper").fill("with-endpoint 'unfinished");
  await form.getByRole("tab", { name: "command", exact: true }).click();
  await form.getByLabel("agent command").fill("printf hello");
  await form.locator(".launch-composer-save-template").click();
  const panel = form.locator(".save-template-panel");
  await expect(panel).toBeVisible();
  await expect(panel.locator('[data-field="command"]')).toBeChecked();
  await expect(panel.locator('[data-field="wrapper"]')).toHaveCount(0);
});

/**
 * Spec: each agent type has its own remembered prefix, restored on selection;
 * unsafe stored bytes are visible but cannot be edited as escaped text.
 *
 * Why: switching agents must not run another agent's wrapper, and a prefix
 * stored by a host or template must not disguise direction-changing bytes.
 * The preference reply is the fixture premise; the helm's persistence path is
 * covered separately from this browser projection.
 */
test("agent switch loads that agent's remembered wrapper prefix", async ({ page }) => {
  let seededReads = 0;
  await page.route("**/api/preferences", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const response = await route.fetch();
    expect(response.ok(), "the fixture needs a real preference row").toBe(true);
    const preferences = await response.json();
    const remembered_wrapper_prefixes = {
      codex: ["with-endpoint", "--profile", "codex", "--"],
      claude: ["env", "CLAUDE_PROFILE=work\u202e"],
    };
    expect(remembered_wrapper_prefixes.codex).not.toEqual(remembered_wrapper_prefixes.claude);
    seededReads++;
    await route.fulfill({ response, json: { ...preferences, remembered_wrapper_prefixes } });
  });

  const form = await openNew(page);
  await expect.poll(() => seededReads, { message: "the remembered-prefix fixture was read" }).toBeGreaterThan(0);
  const wrapper = form.locator(".launch-composer-wrapper");
  const harness = form.locator(".launch-composer-harness-choice");
  await harness.getByRole("button", { name: "Codex", exact: true }).click();
  await expect(wrapper).toHaveValue("with-endpoint --profile codex --");
  await wrapper.fill("temporary --");
  await harness.getByRole("button", { name: "Claude", exact: true }).click();
  await expect(wrapper).toHaveValue("env 'CLAUDE_PROFILE=work<U+202E>'");
  await expect(wrapper).toHaveJSProperty("readOnly", true);
  await form.getByRole("button", { name: "clear wrapper prefix" }).click();
  await expect(wrapper).toHaveValue("");
  await expect(wrapper).toHaveJSProperty("readOnly", false);
  await harness.getByRole("button", { name: "Codex", exact: true }).click();
  await expect(wrapper).toHaveValue("with-endpoint --profile codex --");
});

/**
 * Spec: on the command tab, accepting an agent type from search switches to
 * the agent tab with that type chosen, while accepting a folder or a name
 * keeps the command tab and the command being written.
 *
 * Why: SPEC.md has search keep offering agent-launch choices on the command
 * tab, and an agent type only means something on the agent tab; a folder is
 * shared by both kinds, so choosing one must not throw away the command the
 * user is writing.
 */
test("search on the command tab switches to the agent tab only for an agent choice", async ({ page }) => {
  const form = await openNew(page);
  const agentTab = form.getByRole("tab", { name: "agent", exact: true });
  const commandTab = form.getByRole("tab", { name: "command", exact: true });
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await commandTab.click();
  await form.getByLabel("agent command").fill("sleep 300");

  // The folder history can offer /tmp a second time under the same words.
  await search.fill("/tmp");
  await form.getByRole("option", { name: "Use this path: /tmp", exact: true }).first().click();
  await expect(commandTab).toHaveAttribute("aria-selected", "true");
  await expect(form.getByLabel("folder", { exact: true })).toHaveValue("/tmp");
  await expect(form.getByLabel("agent command")).toHaveValue("sleep 300");

  await search.fill("name:tabs-named");
  await form.getByRole("option").first().click();
  await expect(commandTab, "a name keeps the command tab").toHaveAttribute("aria-selected", "true");
  await expect(form.getByLabel("name (optional)")).toHaveValue("tabs-named");
  await expect(form.getByLabel("agent command")).toHaveValue("sleep 300");

  await search.fill("claude");
  await form.getByRole("option", { name: "Harness: Claude", exact: true }).click();
  await expect(agentTab).toHaveAttribute("aria-selected", "true");
  await expect(
    form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Claude", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await commandTab.click();
  await expect(form.getByLabel("agent command"), "the command draft survives the switch").toHaveValue("sleep 300");
});

/**
 * Spec: the tab strip is one tab stop, and the arrow keys move between the
 * two tabs, selecting the one they land on.
 *
 * Why: the strip announces itself as tabs, and people using a keyboard or a
 * screen reader expect the arrow keys to move between them; a second tab
 * stop would also put an extra keystroke between search and the fields.
 */
test("the arrow keys move between the launch-kind tabs", async ({ page }) => {
  const form = await openNew(page);
  const agentTab = form.getByRole("tab", { name: "agent", exact: true });
  const commandTab = form.getByRole("tab", { name: "command", exact: true });
  await expect(agentTab).toHaveAttribute("tabindex", "0");
  await expect(commandTab).toHaveAttribute("tabindex", "-1");
  await agentTab.focus();
  await page.keyboard.press("ArrowRight");
  await expect(commandTab).toHaveAttribute("aria-selected", "true");
  await expect(commandTab).toBeFocused();
  await expect(form.getByLabel("agent command")).toBeVisible();
  await page.keyboard.press("ArrowLeft");
  await expect(agentTab).toHaveAttribute("aria-selected", "true");
  await expect(agentTab).toBeFocused();
});
