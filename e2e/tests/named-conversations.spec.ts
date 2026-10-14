/** One-time conversation selectors belong to the launcher request, not saved
 * setups. Requests are intercepted before the real helm can launch a vendor
 * agent; the shared listing still supplies the actual host/form prerequisites.
 */
import { expect, test } from "./helpers/evidence";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

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
    expect(input!.width).toBeGreaterThanOrEqual(choiceBounds!.width * 0.9);
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
