/** One-time conversation selectors belong to the launcher request, not saved
 * setups. Requests are intercepted before the real helm can launch a vendor
 * agent; the shared listing still supplies the actual host/form prerequisites.
 */
import { expect, test } from "./helpers/evidence";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

/** A first message is transient request data. Multiline Enter must edit it,
 * each supported agent must offer it, and an unchanged retry must keep its
 * key while an edited message gets a new one. POST interception proves the
 * bytes without spending a real vendor turn.
 */
test("initial prompt launcher follows capabilities and binds literal request retries", async ({ page }, info) => {
  const bodies: any[] = [];
  await page.route("**/api/sessions", async route => {
    if (route.request().method() !== "POST") { await route.continue(); return; }
    bodies.push(route.request().postDataJSON());
    await fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: "initial prompt fixture refusal" });
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  const prompt = form.getByLabel("initial prompt", { exact: true });
  await expect(prompt).toHaveCount(0);
  for (const [agent, supported] of [["Claude", true], ["Codex", true], ["Cursor", true], ["Grok", true], ["Muse", true], ["OMP", true], ["OpenCode", true], ["Goose", false], ["Pi", false]] as const) {
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: agent, exact: true }).click();
    await expect(prompt).toHaveCount(supported ? 1 : 0);
  }
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  await prompt.fill("--literal {farhelm_args}");
  await prompt.focus();
  await expect(prompt).toBeFocused();
  await prompt.press("End");
  await prompt.press("Enter");
  await expect(prompt).toHaveValue("--literal {farhelm_args}\n");
  await prompt.press("Tab");
  // Tab moves focus normally; the input value still contains the new line.
  const message = "--literal {farhelm_args}\n\tsecond";
  await prompt.fill(message);
  await expect(form.locator(".launch-composer-summary")).toContainText("prompt: 2 lines");
  expect(bodies).toHaveLength(0);
  for (const [name, width, height] of [["desktop", 1280, 900], ["phone", 390, 844]] as const) {
    await page.setViewportSize({ width, height });
    await prompt.scrollIntoViewIfNeeded();
    const formBounds = await form.boundingBox();
    const bounds = await prompt.boundingBox();
    expect(formBounds).not.toBeNull(); expect(bounds).not.toBeNull();
    expect(bounds!.x).toBeGreaterThanOrEqual(formBounds!.x);
    expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(formBounds!.x + formBounds!.width);
    await expect(prompt).toHaveCSS("resize", "vertical");
    const path = info.outputPath(`initial-prompt-${name}.png`);
    await form.screenshot({ path, animations: "disabled" });
    await info.attach(`initial prompt ${name}`, { path, contentType: "image/png" });
  }
  await form.locator(".create-session-submit").click();
  await expect(form).toContainText("initial prompt fixture refusal");
  expect(bodies).toHaveLength(1);
  expect(bodies[0].prompt).toBe(message);
  expect(bodies[0].launch).not.toHaveProperty("prompt");
  await form.locator(".create-session-submit").click();
  await expect.poll(() => bodies.length).toBe(2);
  expect(bodies[1].intent_key).toBe(bodies[0].intent_key);
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await prompt.fill("changed message");
  await form.locator(".named-conversation").fill("known-target");
  await form.locator(".create-session-submit").click();
  await expect.poll(() => bodies.length).toBe(3);
  expect(bodies[2]).toMatchObject({ prompt: "changed message", conversation: "known-target" });
  expect(bodies[2].intent_key).not.toBe(bodies[1].intent_key);
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await form.getByRole("tab", { name: "command", exact: true }).click();
  await expect(prompt).toHaveCount(0);
  await form.getByRole("tab", { name: "agent", exact: true }).click();
  await expect(prompt).toHaveValue("changed message");
});

/** Every offering agent must explain which kind of value it takes and whether
 * Farhelm can learn that conversation for later Restart. Command launches use
 * their command text instead and must not expose a second selector.
 */
test("named conversation labels and reporting help follow the selected agent", async ({ page }, testInfo) => {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  for (const [agent, label, captures] of [
    ["Claude", "conversation id", true], ["Codex", "conversation id", true],
    ["Grok", "conversation id", true], ["Muse", "conversation id", false],
    ["Cursor", "chat id", false], ["Goose", "session id", false],
    ["OpenCode", "session id", false], ["Pi", "session file", true], ["OMP", "session file", true],
  ] as const) {
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: agent, exact: true }).click();
    const choice = form.locator(".named-conversation-choice");
    await expect(choice.locator(".launch-composer-section-label")).toHaveText(label);
    await expect(choice.locator(".named-conversation")).toHaveValue("");
    await expect(choice.getByText("Farhelm will not capture", { exact: false })).toHaveCount(captures ? 0 : 1);
  }
  // Include the reporting warning in the width proof, so its wrapping is
  // checked beside the same labeled field rather than only through copy.
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Muse", exact: true }).click();
  // Retain both layouts for visual inspection; the narrow input must stay
  // inside the launcher even when its label and reporting help wrap.
  for (const [name, width, height] of [["desktop", 1280, 900], ["phone", 390, 844]] as const) {
    await page.setViewportSize({ width, height });
    const choice = form.locator(".named-conversation-choice");
    await choice.scrollIntoViewIfNeeded();
    const bounds = await form.boundingBox();
    const choiceBounds = await choice.boundingBox();
    const input = await form.locator(".named-conversation").boundingBox();
    expect(bounds).not.toBeNull();
    expect(choiceBounds).not.toBeNull();
    expect(input).not.toBeNull();
    expect(input!.x).toBeGreaterThanOrEqual(bounds!.x);
    expect(input!.x + input!.width).toBeLessThanOrEqual(bounds!.x + bounds!.width);
    // Desktop reserves a label column; phone stacks that label above the
    // field. Compare with the actual field cell, not the whole labeled row.
    const fieldWidth = await choice.evaluate((node) => {
      const columns = getComputedStyle(node).gridTemplateColumns.split(/\s+/).map(Number.parseFloat);
      return columns[columns.length - 1];
    });
    expect(Number.isFinite(fieldWidth)).toBe(true);
    expect(Math.abs(input!.width - fieldWidth)).toBeLessThanOrEqual(1);
    expect(Math.abs(input!.x + input!.width - choiceBounds!.x - choiceBounds!.width)).toBeLessThanOrEqual(1);
    const help = await choice.locator("p").boundingBox();
    expect(help).not.toBeNull();
    expect(help!.y).toBeGreaterThanOrEqual(input!.y + input!.height);
    expect(help!.x).toBeGreaterThanOrEqual(bounds!.x);
    expect(help!.x + help!.width).toBeLessThanOrEqual(bounds!.x + bounds!.width);
    const path = testInfo.outputPath(`named-launcher-${name}.png`);
    await form.screenshot({ path, animations: "disabled" });
    await testInfo.attach(`named launcher ${name}`, { path, contentType: "image/png" });
  }
  await form.getByRole("tab", { name: "command", exact: true }).click();
  await expect(form.locator(".named-conversation-choice")).toHaveCount(0);
});

/** A named New request keeps the selection ordinary and a refusal keeps its
 * target editable. No vendor process starts: the observable is the intercepted
 * body, followed by the still-open field rather than absence of agent output.
 */
test("New sends a named conversation outside the reusable launch and preserves refusal edits", async ({ page }) => {
  const bodies: Record<string, unknown>[] = [];
  await page.route("**/api/sessions", async route => {
    if (route.request().method() !== "POST") { await route.continue(); return; }
    bodies.push(route.request().postDataJSON());
    await fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: "named start fixture refusal" });
  });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Claude", exact: true }).click();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
  await form.locator(".named-conversation").fill("known-target");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await form.locator(".create-session-submit").click();
  await expect(form).toContainText("named start fixture refusal");
  expect(bodies).toHaveLength(1);
  expect(bodies[0]).toMatchObject({ conversation: "known-target", launch: { harness: "claude" } });
  expect(bodies[0].launch).not.toHaveProperty("conversation");
  await expect(form.locator(".named-conversation")).toHaveValue("known-target");
});
