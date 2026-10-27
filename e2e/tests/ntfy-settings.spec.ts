/** Explicit-save ntfy settings stay separate from replayed preferences. The real
 * settings API proves persistence; only test sends are intercepted so browser
 * checks never contact a notification service or expose a private subscription. */
import { expect, test } from "./helpers/evidence";
import { createServer } from "node:http";
import { openSettings, selectSettingsSection } from "./helpers/settings";
import { resolveToken } from "./helpers/color";

/** Reopening must fetch non-secret settings, preserve a saved token while editing
 * other fields, and keep rejected drafts editable. Phone geometry matters because
 * this section makes the settings dialog taller than its viewport. Native
 * priority-button keys edit only the draft; refusal must retain that edit
 * while the real saved configuration keeps its preceding priority. */
test("ntfy settings explicitly save, refuse, preserve and clear a token", async ({ page, request }, testInfo) => {
  await page.goto("/");
  const original = await (await request.get("/api/ntfy-alerts")).json();
  expect(original.token_set, "fixture must not replace a pre-existing secret it cannot restore").toBe(false);
  // This is the real helm's outbound endpoint, not a page.route interception:
  // enabling the setting must never publish fleet titles to a public topic.
  const endpoint = createServer((incoming, response) => { incoming.resume(); response.end(); });
  await new Promise<void>((resolve) => endpoint.listen(0, "127.0.0.1", resolve));
  const address = endpoint.address();
  if (!address || typeof address === "string") throw new Error("loopback server did not bind a TCP port");
  const serverUrl = `http://127.0.0.1:${address.port}`;
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  const section = dialog.getByRole("region", { name: "ntfy alerts" });
  const priority = section.getByRole("group", { name: "priority", exact: true });
  const topic = section.getByRole("textbox", { name: "topic", exact: true });
  const password = section.getByLabel("access token (optional)", { exact: true });
  try {
    await openSettings(page, "notifications");
    await expect(topic).toBeEditable();
    await expect(section.getByLabel("send ntfy alerts", { exact: true })).not.toBeChecked();
    const server = section.getByRole("textbox", { name: "server URL", exact: true });
    await expect(server).toHaveValue("https://ntfy.sh");
    await expect(server).toHaveCSS("font-family", /JetBrains Mono/);
    const actions = section.locator(".dialog-actions");
    await expect(actions.getByRole("button")).toHaveText(["send test", "save"]);
    const save = actions.getByRole("button", { name: "save", exact: true });
    const sendTest = actions.getByRole("button", { name: "send test", exact: true });
    await expect(save).toHaveCSS("background-color", await resolveToken(page, "--accent-fill"));
    await expect(sendTest).toHaveCSS("background-color", await resolveToken(page, "--bg-2"));
    await server.fill(serverUrl);
    await topic.fill("browser-alerts");
    await password.fill("browser-token");
    await expect(priority.getByRole("button")).toHaveCount(5);
    await expect(priority.getByRole("button", { name: original.priority, exact: true })).toHaveAttribute("aria-pressed", "true");
    const high = priority.getByRole("button", { name: "high", exact: true });
    await high.focus();
    await expect(high).toBeFocused();
    await high.press("Space");
    await expect(high).toHaveAttribute("aria-pressed", "true");
    await section.getByLabel("send ntfy alerts", { exact: true }).check();
    expect((await (await request.get("/api/ntfy-alerts")).json()).topic).toBe(original.topic);
    expect((await (await request.get("/api/ntfy-alerts")).json()).priority).toBe(original.priority);
    await section.getByRole("button", { name: "save", exact: true }).click();
    await expect(section.getByRole("status")).toHaveText("ntfy alert settings saved");
    const saved = await (await request.get("/api/ntfy-alerts")).json();
    expect(saved).toEqual({ enabled: true, server_url: serverUrl, topic: "browser-alerts", priority: "high", token_set: true });
    expect(JSON.stringify(saved)).not.toContain("browser-token");
    await expect(password).toHaveValue("");
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 800 });
      await topic.scrollIntoViewIfNeeded();
      const geometry = await section.evaluate((el) => ({ width: el.clientWidth, content: el.scrollWidth }));
      expect(geometry.content).toBeLessThanOrEqual(geometry.width);
      // The explicit actions stay together and in reading order when the
      // shared action row stacks; neither button becomes another form owner.
      const testBox = (await sendTest.boundingBox())!;
      const saveBox = (await save.boundingBox())!;
      if (width <= 560) expect(testBox.y + testBox.height).toBeLessThanOrEqual(saveBox.y);
      else expect(testBox.x + testBox.width).toBeLessThanOrEqual(saveBox.x);
      await expect(save).toHaveCSS("font-size", await dialog.getByRole("button", { name: "close", exact: true })
        .evaluate((element) => getComputedStyle(element).fontSize));
      await section.screenshot({ path: testInfo.outputPath(`ntfy-settings-${width}.png`) });
    }
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await openSettings(page, "notifications");
    await expect(topic).toHaveValue("browser-alerts");
    await expect(password).toHaveValue("");
    await expect(password).toHaveAttribute("placeholder", "token saved; leave blank to keep it");
    await expect(high).toHaveAttribute("aria-pressed", "true");
    // Native button Enter edits the draft, including a draft whose next Save
    // will be refused. It must neither save early nor erase the refused edit.
    const urgent = priority.getByRole("button", { name: "urgent", exact: true });
    await urgent.focus();
    await expect(urgent).toBeFocused();
    await urgent.press("Enter");
    await expect(urgent).toHaveAttribute("aria-pressed", "true");
    expect((await (await request.get("/api/ntfy-alerts")).json()).priority).toBe("high");
    await topic.fill("bad/topic");
    await section.getByRole("button", { name: "save", exact: true }).click();
    await expect(section.getByRole("alert")).toContainText("topic must contain");
    await expect(topic).toHaveValue("bad/topic");
    await expect(urgent).toHaveAttribute("aria-pressed", "true");
    expect((await (await request.get("/api/ntfy-alerts")).json()).topic).toBe("browser-alerts");
    expect((await (await request.get("/api/ntfy-alerts")).json()).priority).toBe("high");
    await topic.fill("browser-alerts-edited");
    await section.getByRole("button", { name: "save", exact: true }).click();
    await expect(section.getByRole("status")).toHaveText("ntfy alert settings saved");
    expect((await (await request.get("/api/ntfy-alerts")).json()).token_set).toBe(true);
    expect((await (await request.get("/api/ntfy-alerts")).json()).priority).toBe("urgent");
    // A path edit stays on the same server; another origin must explain the
    // secret's removal before Save, without teaching the form the token.
    const originHint = section.locator(".ntfy-token-origin-hint");
    await server.fill("http://localhost:1");
    await expect(originHint).toHaveText("changing the server clears the saved token when you save");
    // Back on the saved server with only a path added: the hint must go away
    // again, which only a same-server comparison can do.
    await server.fill(`${serverUrl}/prefix`);
    await expect(originHint).toHaveCount(0);
    await expect(password).toHaveAttribute("placeholder", "token saved; leave blank to keep it");
    await server.fill("http://localhost:1");
    await expect(originHint).toHaveText("changing the server clears the saved token when you save");
    await expect(password).toHaveAttribute("placeholder", "saved token will be cleared");
    await expect(password).toHaveAttribute("data-tooltip", /leaving it blank clears/);
    await password.fill("replacement-token");
    await expect(originHint).toHaveCount(0);
    await password.fill("");
    await expect(originHint).toBeVisible();
    // Disable background sends before saving this intentionally non-listening
    // loopback origin; the fixture must never contact a public ntfy server.
    await section.getByLabel("send ntfy alerts", { exact: true }).uncheck();
    await section.getByRole("button", { name: "save", exact: true }).click();
    await expect(section.getByRole("status")).toHaveText("ntfy alert settings saved");
    expect((await (await request.get("/api/ntfy-alerts")).json()).token_set).toBe(false);
    await expect(originHint).toHaveCount(0);
    await expect(password).toHaveAttribute("placeholder", "no token saved");
    await server.fill(serverUrl);
    await password.fill("replacement-token");
    await section.getByRole("button", { name: "save", exact: true }).click();
    await expect(section.getByRole("status")).toHaveText("ntfy alert settings saved");
    expect((await (await request.get("/api/ntfy-alerts")).json()).token_set).toBe(true);
    await section.getByLabel("clear saved token", { exact: true }).check();
    await section.getByRole("button", { name: "save", exact: true }).click();
    await expect(section.getByRole("status")).toHaveText("ntfy alert settings saved");
    expect((await (await request.get("/api/ntfy-alerts")).json()).token_set).toBe(false);
  } finally {
    try {
      const restored = await request.put("/api/ntfy-alerts", { data: { ...original, token: "" } });
      expect(restored.ok(), await restored.text()).toBe(true);
    } finally {
      endpoint.closeAllConnections();
      await new Promise<void>((resolve, reject) => endpoint.close((error) => error ? reject(error) : resolve()));
    }
  }
});

/** A test send is a separate request with no form payload. Drafts and both
 * outcomes survive navigation; closing discards drafts, including the token.
 * Success still needs an actual endpoint reply rather than a local notice. */
test("ntfy send test reports outcome while leaving unsaved fields alone", async ({ page }) => {
  await page.goto("/");
  await openSettings(page, "notifications");
  const section = page.getByRole("region", { name: "ntfy alerts" });
  const topic = section.getByRole("textbox", { name: "topic", exact: true });
  await expect(topic).toBeEditable();
  const savedTopic = await topic.inputValue();
  await topic.fill("unsaved-topic");
  const server = section.getByRole("textbox", { name: "server URL", exact: true });
  const token = section.getByLabel("access token (optional)", { exact: true });
  const savedServer = await server.inputValue();
  await server.fill("http://127.0.0.1:1");
  await token.fill("unsaved-browser-token");
  let sends = 0;
  await page.route("**/api/ntfy-alerts/test", async (route) => {
    expect(route.request().method()).toBe("POST");
    expect(route.request().postData()).toBeNull();
    sends++;
    await route.fulfill(sends === 1
      ? { status: 502, body: "the ntfy server could not be reached" }
      : { status: 204 });
  });
  await section.getByRole("button", { name: "send test", exact: true }).click();
  await expect(section.getByRole("alert")).toHaveText("the ntfy server could not be reached");
  // Switching panes must not discard either rejected outcomes or unsaved
  // secrets. The send-test request above intentionally has no form payload.
  await selectSettingsSection(page, "sessions");
  await selectSettingsSection(page, "notifications");
  await expect(section.getByRole("alert")).toHaveText("the ntfy server could not be reached");
  await expect(topic).toHaveValue("unsaved-topic");
  await expect(server).toHaveValue("http://127.0.0.1:1");
  await expect(token).toHaveValue("unsaved-browser-token");
  await section.getByRole("button", { name: "send test", exact: true }).click();
  await expect(section.getByRole("status")).toHaveText("test alert sent");
  await selectSettingsSection(page, "sessions");
  await selectSettingsSection(page, "notifications");
  await expect(section.getByRole("status")).toHaveText("test alert sent");
  await expect(topic).toHaveValue("unsaved-topic");
  await expect(server).toHaveValue("http://127.0.0.1:1");
  await expect(token).toHaveValue("unsaved-browser-token");
  expect(sends).toBe(2);
  await page.getByRole("dialog", { name: "settings", exact: true }).getByRole("button", { name: "close", exact: true }).click();
  await openSettings(page, "notifications");
  await expect(topic).toBeEditable();
  await expect(topic).toHaveValue(savedTopic);
  await expect(server).toHaveValue(savedServer);
  await expect(token).toHaveValue("");
  await expect(section.getByRole("status")).toHaveCount(0);
});

/** Load refusal does not seed an editable default that could overwrite saved
 * settings. Retry obtains an authoritative snapshot before enabling writes. */
test("ntfy settings load failure is visible and retry restores editing", async ({ page }) => {
  let reads = 0;
  await page.route("**/api/ntfy-alerts", async (route) => {
    if (route.request().method() === "GET" && ++reads === 1) {
      await route.fulfill({ status: 500, body: "could not read ntfy alert settings" });
    } else await route.continue();
  });
  await page.goto("/");
  await openSettings(page, "notifications");
  const section = page.getByRole("region", { name: "ntfy alerts" });
  await expect(section.getByRole("alert")).toHaveText("could not read ntfy alert settings");
  await expect(section.getByRole("button", { name: "save", exact: true })).toBeDisabled();
  await expect(section.getByRole("button", { name: "save", exact: true })).toHaveCSS("opacity", "1");
  await expect(section.getByRole("button", { name: "save", exact: true })).toHaveCSS("background-color", await resolveToken(page, "--bg-2"));
  const priority = section.getByRole("group", { name: "priority", exact: true });
  await expect(priority.getByRole("button")).toHaveCount(5);
  for (const value of ["min", "low", "default", "high", "urgent"]) {
    await expect(priority.getByRole("button", { name: value, exact: true })).toBeDisabled();
  }
  await section.getByRole("button", { name: "retry loading", exact: true }).click();
  await expect(section.getByRole("textbox", { name: "topic", exact: true })).toBeEditable();
  await expect(section.getByRole("button", { name: "save", exact: true })).toBeEnabled();
  for (const value of ["min", "low", "default", "high", "urgent"]) {
    await expect(priority.getByRole("button", { name: value, exact: true })).toBeEnabled();
  }
});
