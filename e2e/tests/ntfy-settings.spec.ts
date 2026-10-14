/** Explicit-save ntfy settings stay separate from replayed preferences. The real
 * settings API proves persistence; only test sends are intercepted so browser
 * checks never contact a notification service or expose a private subscription. */
import { expect, test } from "./helpers/evidence";
import { createServer } from "node:http";

/** Reopening must fetch non-secret settings, preserve a saved token while editing
 * other fields, and keep rejected drafts editable. Phone geometry matters because
 * this section makes the settings dialog taller than its viewport. */
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
  const gear = page.getByRole("button", { name: "settings", exact: true });
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  const section = dialog.getByRole("region", { name: "ntfy alerts" });
  const topic = section.getByRole("textbox", { name: "topic", exact: true });
  const password = section.getByLabel("access token (optional)", { exact: true });
  try {
    await gear.click();
    await expect(topic).toBeEditable();
    await expect(section.getByLabel("send ntfy alerts", { exact: true })).not.toBeChecked();
    const server = section.getByRole("textbox", { name: "server URL", exact: true });
    await expect(server).toHaveValue("https://ntfy.sh");
    await server.fill(serverUrl);
    await topic.fill("browser-alerts");
    await password.fill("browser-token");
    await section.getByRole("combobox", { name: "priority", exact: true }).selectOption("high", { timeout: 5000 });
    await expect(section.getByRole("combobox", { name: "priority", exact: true })).toHaveCSS("color-scheme", "dark");
    await section.getByLabel("send ntfy alerts", { exact: true }).check();
    expect((await (await request.get("/api/ntfy-alerts")).json()).topic).toBe(original.topic);
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
      await section.screenshot({ path: testInfo.outputPath(`ntfy-settings-${width}.png`) });
    }
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await gear.click();
    await expect(topic).toHaveValue("browser-alerts");
    await expect(password).toHaveValue("");
    await expect(password).toHaveAttribute("placeholder", "token saved; leave blank to keep it");
    await topic.fill("bad/topic");
    await section.getByRole("button", { name: "save", exact: true }).click();
    await expect(section.getByRole("alert")).toContainText("topic must contain");
    await expect(topic).toHaveValue("bad/topic");
    expect((await (await request.get("/api/ntfy-alerts")).json()).topic).toBe("browser-alerts");
    await topic.fill("browser-alerts-edited");
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

/** A test send is a separate request with no form payload. Failure stays inline
 * and editable drafts remain unsaved; success needs an actual endpoint reply. */
test("ntfy send test reports outcome while leaving unsaved fields alone", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "settings", exact: true }).click();
  const section = page.getByRole("region", { name: "ntfy alerts" });
  const topic = section.getByRole("textbox", { name: "topic", exact: true });
  await expect(topic).toBeEditable();
  await topic.fill("unsaved-topic");
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
  await expect(topic).toHaveValue("unsaved-topic");
  await section.getByRole("button", { name: "send test", exact: true }).click();
  await expect(section.getByRole("status")).toHaveText("test alert sent");
  await expect(topic).toHaveValue("unsaved-topic");
  expect(sends).toBe(2);
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
  await page.getByRole("button", { name: "settings", exact: true }).click();
  const section = page.getByRole("region", { name: "ntfy alerts" });
  await expect(section.getByRole("alert")).toHaveText("could not read ntfy alert settings");
  await expect(section.getByRole("button", { name: "save", exact: true })).toBeDisabled();
  await section.getByRole("button", { name: "retry loading", exact: true }).click();
  await expect(section.getByRole("textbox", { name: "topic", exact: true })).toBeEditable();
  await expect(section.getByRole("button", { name: "save", exact: true })).toBeEnabled();
});
