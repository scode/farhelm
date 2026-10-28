/**
 * The real bundle must deliver Inter for interface text and JetBrains Mono
 * for technical values. Computed CSS alone cannot prove a bundled face
 * loaded: require a matching loaded FontFace before accepting each role.
 */
import { type Locator } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { cleanupSession, createSession, localHostId, openRowMenu, patchPreferences, pinAutoSelect, resetPreferences } from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { waitForSessionReady } from "./helpers/terminal-readiness";

/** Check the element's chosen family and its actual loaded face, including weight matching. */
async function expectLoadedFont(target: Locator, family: string) {
  await expect(target).toBeVisible();
  await expect.poll(() => target.evaluate(async (element, family) => {
    const style = getComputedStyle(element);
    if (style.fontFamily.split(",")[0].replace(/["']/g, "").trim() !== family) return false;
    const faces = await document.fonts.load(`${style.fontWeight} ${style.fontSize} "${family}"`, element.textContent || "M");
    return faces.length > 0 && faces.every((face) => face.status === "loaded");
  }, family), { message: `${family} must load for this rendered value` }).toBe(true);
}

/** Names remain proportional while paths, ages, commands, versions and the Nerd Font pin stay mono. */
test("interface and technical values load their own bundled font roles", async ({ page, request }) => {
  const session = await createSession(request, {
    title: `font-roles-${Date.now()}`, invocation: "sleep 300", cwd: "/tmp",
  });
  let bodyFailed = false;
  try {
    await patchPreferences(request, { compact: false, welcome_seen: true });
    const pin = await request.put(`/api/sessions/${encodeURIComponent(session.id)}/pin`, { data: { pinned: true } });
    expect(pin.ok()).toBe(true);
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    const row = page.locator(`.session-row[data-session-id="${session.id}"]`);
    await expect(row).toHaveAttribute("data-session-pinned", "true", { timeout: 20_000 });
    await waitForSessionReady(page, session.id);
    await expectLoadedFont(row.locator(".session-title"), "Inter");
    await expect(row.locator(".session-row-meta")).toBeVisible();
    await expectLoadedFont(row.locator(".session-row-meta .session-host"), "Inter");
    await expectLoadedFont(page.locator(".new-session-button"), "Inter");
    await expectLoadedFont(row.locator(".session-cwd"), "JetBrains Mono");
    await expectLoadedFont(row.locator(".status-time"), "JetBrains Mono");
    await expectLoadedFont(row.locator(".session-pin"), "JetBrains Mono");
    await expectLoadedFont(page.locator(".app-version"), "JetBrains Mono");
    const command = page.locator(".session-details .header-copy > .peer-value").filter({ hasText: "sleep 300" });
    await expect(command).toHaveText("sleep 300");
    await expectLoadedFont(command, "JetBrains Mono");

    // A command and its status share the menu summary, but only command bytes
    // are technical text. Small rendered labels can otherwise look alike in
    // pictures even when their computed faces are different.
    await openRowMenu(row);
    const menu = row.locator(".session-row-menu-panel");
    const summary = menu.locator(".session-row-menu-summary-runs");
    await expectLoadedFont(summary.locator(".font-mono"), "JetBrains Mono");
    const state = summary.locator("span:not(.peer-value)").last();
    await expect(state).toHaveText(/\S/);
    await expectLoadedFont(state, "Inter");
    await page.keyboard.press("Escape");
    await expect(menu).toBeHidden();

    await page.locator(".new-session-button").click();
    const launcher = page.locator("form.create-session-form");
    await expect(launcher).toBeVisible();
    await launcher.getByRole("group", { name: "Destination type", exact: true }).getByRole("button", { name: "Folder", exact: true }).click();
    await expectLoadedFont(launcher.getByRole("combobox", { name: "Folder", exact: true }), "JetBrains Mono");
    const search = launcher.locator(".launch-composer-search input");
    await expectLoadedFont(search, "Inter");
    await search.fill("/tmp");
    for (const label of ["Use this path: /tmp", "Browse this path: /tmp"]) {
      const option = launcher.getByRole("option", { name: label, exact: true }).first();
      await expect(option).toBeVisible();
      await expectLoadedFont(option, "Inter");
      const path = option.locator(".peer-value");
      await expect(path).toHaveText("/tmp");
      await expectLoadedFont(path, "JetBrains Mono");
    }
    await search.fill("");
    await launcher.getByRole("tab", { name: "Command", exact: true }).click();
    const input = launcher.getByLabel("Agent command", { exact: true });
    await input.fill("printf hello");
    await expect(input).toHaveValue("printf hello");
    await expectLoadedFont(input, "JetBrains Mono");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["font-role session", () => cleanupSession(request, session.id)],
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/**
 * Archive dates are technical evidence beside app prose. Unknown historical
 * dates must stay an honest prose message, with no synthetic timestamp or mono
 * inherited merely because the known case contains one.
 */
test("archive metadata keeps labels in Inter and only known timestamps in mono", async ({ page, request }) => {
  const host = await localHostId(request);
  const identity = await request.get("/api/hosts");
  expect(identity.ok()).toBe(true);
  const build = requireHelmBuild(identity, "archive font fixture");
  const checkouts = [1_700_000_000, null].map((archived_at, index) => ({
    id: `font-archive-${index}`, name: `font archive ${index}`, repository: "example/repository",
    path: `/work/archived/font-archive-${index}`, archived_at, bytes: 1024,
  }));
  await page.route("**/api/checkout-trash/list", async (route) => {
    const selected = route.request().postDataJSON().host;
    await route.fulfill({
      headers: { "x-farhelm-build": build },
      json: { checkouts: selected === host ? checkouts : [], issues: [] },
    });
  });
  try {
    await page.goto("/");
    const trigger = page.locator(".checkout-trash-button");
    await expect(trigger).toHaveAttribute("data-count", "2");
    await trigger.click();
    const dialog = page.locator(".checkout-trash-dialog");
    await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(2);
    const metadata = (name: string) => dialog.locator(".checkout-trash-entry")
      .filter({ has: page.getByText(name, { exact: true }) }).locator(".checkout-trash-entry-meta > span:last-child");
    const known = metadata("font archive 0");
    await expect(known).toHaveText("Archived 2023-11-14 22:13 UTC");
    await expectLoadedFont(known, "Inter");
    await expectLoadedFont(known.locator(".font-mono"), "JetBrains Mono");
    const unknown = metadata("font archive 1");
    await expect(unknown).toHaveText("Archive time unknown");
    await expectLoadedFont(unknown, "Inter");
    await expect(unknown.locator(".font-mono")).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
  } finally {
    await page.unrouteAll({ behavior: "wait" });
  }
});

/**
 * Saved wrapper/argument bytes retain mono in both selection surfaces, while
 * shortcut chords keep the technical mono role and tabular digits. Measuring
 * the configured chords catches a digit-metric regression hidden by normalization.
 */
test("template summaries split command fonts and shortcut digits keep equal widths", async ({ page, request }) => {
  const prefix = `font-summary-${Date.now()}`;
  const names = [`${prefix}-one`, `${prefix}-eight`];
  const fields = {
    kind: "agent", agent: "claude", permissions: "custom",
    permission_flags: ["--mode", "ask"], wrapper_prefix: ["wrapper", "--"],
  };
  let bodyFailed = false;
  try {
    for (const name of names) {
      const response = await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: fields });
      expect(response.ok(), `storing font fixture ${name}: ${response.status()}`).toBe(true);
    }
    const listing = await request.get("/api/templates");
    expect(listing.ok()).toBe(true);
    const stored = (await listing.json()).templates;
    for (const name of names) {
      expect(stored.find((template: { name: string }) => template.name === name)?.fields).toMatchObject(fields);
    }
    await patchPreferences(request, { welcome_seen: true, template_shortcuts: { "1": names[0], "8": names[1] } });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "Templates", exact: true });
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    const row = dialog.locator(".templates-row").filter({ has: page.getByText(names[0], { exact: true }) });
    const summary = row.locator(".templates-row-summary");
    await expectLoadedFont(summary.getByText("Wrapper prefix", { exact: true }), "Inter");
    await expectLoadedFont(summary.locator(".font-mono").filter({ hasText: /^wrapper --$/ }), "JetBrains Mono");
    await expectLoadedFont(summary.locator(".font-mono").filter({ hasText: /^--mode ask$/ }), "JetBrains Mono");

    const widths = [];
    for (const [index, name] of names.entries()) {
      const toggle = dialog.getByRole("button", { name: `Number shortcut ${name}`, exact: true });
      await expect(toggle).toHaveText(`Ctrl+Shift+${index === 0 ? "1" : "8"}`);
      await expectLoadedFont(toggle, "JetBrains Mono");
      await expect(toggle).toHaveCSS("font-variant-numeric", "tabular-nums");
      widths.push(await toggle.evaluate((element) => {
        const range = document.createRange();
        range.selectNodeContents(element);
        const text = range.getBoundingClientRect().width;
        range.detach();
        return { text, control: element.getBoundingClientRect().width };
      }));
    }
    expect(widths[0].text).toBeGreaterThan(0);
    expect(widths[0].text).toBeCloseTo(widths[1].text, 2);
    expect(widths[0].control).toBeCloseTo(widths[1].control, 2);

    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(page.locator(".quick-switcher-trigger")).toHaveCount(1);
    const mac = await page.evaluate(() => /Mac/.test(navigator.platform));
    await page.keyboard.press(mac ? "Meta+k" : "Control+Shift+k");
    const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
    const search = switcher.getByRole("combobox");
    await expect(search).toBeFocused();
    await search.fill(`tl:${names[0]}`);
    const result = switcher.locator(".quick-switcher-template");
    await expect(result).toHaveCount(1);
    await expect(result).toContainText(names[0]);
    const switchSummary = result.locator(".quick-switcher-template-summary");
    await expectLoadedFont(switchSummary.getByText("Wrapper prefix", { exact: true }), "Inter");
    await expectLoadedFont(switchSummary.locator(".font-mono").filter({ hasText: /^wrapper --$/ }), "JetBrains Mono");
    await expectLoadedFont(switchSummary.locator(".font-mono").filter({ hasText: /^--mode ask$/ }), "JetBrains Mono");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ...names.map((name): [string, () => Promise<void>] => [`font template ${name}`, async () => {
        const response = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(response.ok() || response.status() === 404).toBe(true);
      }]),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});
