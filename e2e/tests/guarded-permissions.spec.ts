/**
 * The permission controls must explain the mode that Farhelm actually requests,
 * and a pre-upgrade no-flag row must stay visibly unclassified. These browser
 * assertions cover the hover target and SVG wiring that pure choice tests cannot
 * see; the real-agent probes and compiler tests establish what the flags do.
 */
import { expect, test } from "./helpers/evidence";
import { SESSION_LISTING } from "./helpers/fleet";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

/** Every offered guarded chip names its vendor mode and the exact forcing flags. */
test("guarded choices explain each agent and Cursor keeps its own setting", async ({ page }, testInfo) => {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  for (const [agent, mode, flags] of [
    ["Claude", "Claude auto mode", "--permission-mode auto"],
    ["Codex", "Codex sandboxed auto", "-a on-request -s workspace-write"],
    ["Grok", "Grok auto-review", "--permission-mode auto"],
    ["Muse", "Muse auto-review", "--permission-profile :auto-review"],
    ["OMP", "OMP write mode", "--approval-mode write"],
  ]) {
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: agent, exact: true }).click();
    const guarded = form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true });
    await expect(guarded, `${agent} must actually offer the mode before its help is inspected`).toBeVisible();
    await expect(guarded).toHaveAccessibleDescription(new RegExp(mode));
    await expect(guarded).toHaveAccessibleDescription(/Flags:[\s\S]*can't guarantee how it behaves/);
    await guarded.click();
    await expect(guarded).toHaveAttribute("aria-pressed", "true");
    // A press intentionally dismisses help until the pointer leaves the chip;
    // test the next ordinary hover rather than bypassing that dismissal rule.
    await form.locator(".launch-composer-section-label").first().hover();
    await guarded.hover();
    const tooltip = page.locator("body > .farhelm-tooltip");
    await expect(tooltip).toBeVisible();
    await expect(tooltip).toContainText(mode);
    await expect(tooltip).toContainText(flags);
    await expect(tooltip).toContainText("can't guarantee how it behaves");
    // DOM text alone passed when the old eight-line clamp hid the caveat.
    // The full explanation must fit the painted box, including at phone width.
    expect(await tooltip.evaluate(node => node.scrollHeight <= node.clientHeight)).toBe(true);
    const help = form.getByRole("button", { name: "About permission modes", exact: true });
    await expect(help).toBeVisible();
    await expect(help).toHaveAccessibleDescription(new RegExp(mode));
    const chipBox = await guarded.boundingBox();
    const helpBox = await help.boundingBox();
    expect(chipBox && helpBox, "both permission and documentation controls have a painted box").toBeTruthy();
    expect(Math.abs(chipBox!.y - helpBox!.y), "help stays beside the permission chips").toBeLessThanOrEqual(2);
    if (agent === "Claude") {
      const search = form.locator('.launch-composer-search input[role="combobox"]');
      await search.fill("perms:guarded");
      const result = form.getByRole("option", { name: "Permissions: guarded", exact: true });
      await expect(result).toBeVisible();
      await expect(result).toHaveAccessibleDescription(/Flags:[\s\S]*can't guarantee how it behaves/);
      await result.hover();
      await expect(tooltip).toBeVisible();
      await expect(tooltip).toContainText(flags);
      expect(await tooltip.evaluate(node => node.scrollHeight <= node.clientHeight)).toBe(true);
      await search.fill("");
      await guarded.hover();
      await expect(tooltip).toBeVisible();
      await page.screenshot({ path: testInfo.outputPath("guarded-launcher-help.png") });
      const viewport = page.viewportSize()!;
      await page.setViewportSize({ width: 390, height: 800 });
      await expect(help).toBeVisible();
      // Ancestor scrolling dismisses hover help. Reveal the phone's control
      // first, then use a real keyboard handoff to inspect its focused help.
      await help.scrollIntoViewIfNeeded();
      await expect(help).toBeInViewport({ ratio: 1 });
      await help.focus();
      await page.keyboard.press("Shift+Tab");
      await page.keyboard.press("Tab");
      await expect(help).toBeFocused();
      await expect(tooltip).toBeVisible();
      expect(await tooltip.evaluate(node => node.scrollHeight <= node.clientHeight)).toBe(true);
      await page.screenshot({ path: testInfo.outputPath("guarded-launcher-phone-help.png") });
      await page.setViewportSize(viewport);
    }
  }
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Cursor", exact: true }).click();
  const permissions = form.locator(".launch-composer-permissions-choice");
  await expect(permissions.getByRole("button", { name: "guarded", exact: true })).toHaveCount(0);
  await permissions.getByRole("button", { name: "agent's own setting", exact: true }).click();
  await expect(permissions.getByRole("button", { name: "agent's own setting", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(permissions.getByRole("button", { name: "agent's own setting", exact: true })).toHaveAccessibleDescription(/Not classified.*agent's own setting/);
  const help = form.getByRole("button", { name: "About permission modes", exact: true });
  await help.hover();
  await expect(page.locator("body > .farhelm-tooltip")).toContainText("Not classified");
  await expect(page.locator("body > .farhelm-tooltip")).toContainText("hosts that ask before YOLO launches treat it as YOLO");
  // Observe the existing browser-open seam without navigating away from the
  // fixture or making an outbound request to the live documentation site.
  await page.evaluate(() => {
    const root = window as unknown as { farhelmTerminalLinks: { openTerminalUrl(url: string): void }; guardedHelpUrl?: string };
    root.farhelmTerminalLinks.openTerminalUrl = url => { root.guardedHelpUrl = url; };
  });
  await help.click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { guardedHelpUrl?: string }).guardedHelpUrl)).toBe("https://farhelm.io/docs/agents/#permission-modes");
});

/** A row's shield hover uses stored argv, and an older omission never borrows the new default. */
test("guarded and older own-setting shields show their actual launch provenance", async ({ page }) => {
  const now = Math.floor(Date.now() / 1000);
  const sessions = [
    { id: "guarded-row", permission: "guarded", start: ["codex", "-a", "on-request", "-s", "workspace-write", "{farhelm_args}"] },
    { id: "older-row", permission: null, start: ["codex", "--older-flag", "{farhelm_args}"] },
  ].map(({ id, permission, start }) => ({
    id, title: id, cwd: "/workspace", invocation: "contradictory-display --yolo",
    launch: { kind: "agent", selection: { harness: "codex", model: null, effort: null, permissions: permission }, start, resume: null },
    status: { state: "running" }, last_activity_at: now,
  }));
  await page.route(SESSION_LISTING, route => fulfillAsHelm(route, {
    status: 200, contentType: "application/json",
    body: JSON.stringify({ sessions, total: 2, matching: 2, truncated: false }),
  }));
  await page.goto("/");
  const guarded = page.locator('[data-session-id="guarded-row"] .permission-glyph');
  const older = page.locator('[data-session-id="older-row"] .permission-glyph');
  await expect(guarded).toHaveAttribute("data-glyph", "shielded");
  await expect(older).toHaveAttribute("data-glyph", "own-setting");
  await expect(older.locator("path").first()).toHaveAttribute("stroke-dasharray", "1.6 1.4");
  await guarded.hover();
  const tooltip = page.locator("body > .farhelm-tooltip");
  await expect(tooltip).toBeVisible();
  await expect(tooltip).toContainText("Guarded · Codex sandboxed auto");
  await expect(tooltip).toContainText("Launched with: codex -a on-request -s workspace-write");
  await expect(tooltip).not.toContainText("contradictory-display");
  await older.hover();
  await expect(tooltip).toContainText("Not classified · agent's own setting");
  await expect(tooltip).toContainText("Launched with: codex --older-flag");
  await expect(tooltip).not.toContainText("Guarded · Codex");
});
