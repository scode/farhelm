import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession, localHostId, openHostsPanel, patchPreferences, pinAutoSelect, readPreferences, showHostsPanel } from "./helpers/fleet";
import { chooseHost, hostChoice } from "./helpers/host-rail";
import { chooseSort, chooseStatus } from "./helpers/list-header";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";

/** Presentation is independent of query, disclosure and session ownership.
 * A real selected terminal stays mounted even when the applied status hides
 * its row; reopening management must preserve the same native details choice.
 */
test("Hosts swaps ordinary content and preserves filters, sort, selection and details", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const host = await localHostId(request);
  const session = await createSession(request, { title: `panel-preservation-${Date.now()}`, invocation: "sleep 300", host });
  try {
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    await waitForSessionRevealed(page, session.id);
    // The actual island identity survives the swap, rather than only a
    // matching session title painting again after a remount.
    const island = page.locator("#terminal");
    await expect(island).toHaveCount(1);
    await island.evaluate((node) => node.setAttribute("data-panel-lifetime", "original"));
    await chooseHost(page, host);
    await chooseSort(page, "title");
    await chooseStatus(page, "Waiting");
    const toggle = page.locator(".hosts-panel-toggle");
    await expect(toggle).toHaveAttribute("aria-pressed", "false");
    await expect(page.locator(".hosts-heading")).not.toBeVisible();
    await showHostsPanel(page);
    await expect(toggle).toBeFocused();
    // The remembered filter uses a quiet background in management. Its
    // keyboard outline must keep the ordinary accent in the light palette.
    await page.emulateMedia({ colorScheme: "light" });
    await expect(page.locator("html")).toHaveAttribute("data-appearance", "light");
    const remembered = hostChoice(page, host);
    await remembered.focus();
    await remembered.press("ArrowRight");
    await expect(remembered).toBeFocused();
    await expect.poll(() => remembered.evaluate((node) => node.matches(":focus-visible"))).toBe(true);
    const focusStyle = await remembered.evaluate((node) => {
      const style = getComputedStyle(node);
      return { outline: style.outlineColor, border: style.borderTopColor };
    });
    expect(focusStyle.outline).toBe(focusStyle.border);
    expect(focusStyle.outline).not.toBe("rgb(255, 255, 255)");
    await toggle.focus();
    await openHostsPanel(page);
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("region", { name: "Host management", exact: true })).toBeVisible();
    await expect(page.locator(".session-heading")).not.toBeVisible();
    await expect(page.locator(".session-status-filter")).not.toBeVisible();
    await expect(page.locator(".host-details-toggle")).toBeChecked();
    await toggle.focus();
    await expect(toggle).toBeFocused();
    await page.keyboard.press("Space");
    await expect(toggle).toBeFocused();
    await expect(toggle).toHaveAttribute("aria-pressed", "false");
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("radio", { name: "Waiting", exact: true })).toBeChecked();
    await expect(hostChoice(page, host)).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator(".sort-title")).toHaveAttribute("data-sort", "title");
    await expect(island).toHaveAttribute("data-panel-lifetime", "original");
    await showHostsPanel(page);
    await expect(page.locator(".host-details-toggle")).toBeChecked();
    // Primary activation is navigation back to Sessions even for an already
    // selected avatar. It must preserve the other query dimension.
    await hostChoice(page, host).click();
    await expect(toggle).toHaveAttribute("aria-pressed", "false");
    await expect(page.getByRole("radio", { name: "Waiting", exact: true })).toBeChecked();
    await expect(island).toHaveAttribute("data-panel-lifetime", "original");
    await waitForSessionRevealed(page, session.id);
  } finally {
    await cleanupSession(request, session.id);
    await patchPreferences(request, { list_sort: previous.list_sort ?? null });
  }
});

/** Main-pane launcher actions stay available over Hosts. Their accepted
 * opening must reveal the New control that owns the existing Cancel return.
 */
test("header Clone from Hosts reveals Sessions and restores launcher focus", async ({ page, request }) => {
  const session = await createSession(request, { title: `panel-clone-${Date.now()}`, invocation: "sleep 300" });
  try {
    await pinAutoSelect(page, session.id);
    await page.goto("/");
    await waitForSessionRevealed(page, session.id);
    await showHostsPanel(page);
    const clone = page.locator(".header-clone");
    await expect(clone).toBeVisible();
    await expect(clone).not.toHaveAttribute("aria-disabled", "true");
    await clone.click();
    const launcher = page.locator(".create-session-form");
    await expect(launcher).toBeVisible();
    await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
    await launcher.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(launcher).toHaveCount(0);
    await expect(page.locator(".new-session-button")).toBeVisible();
    await expect(page.locator(".new-session-button")).toBeFocused();
  } finally {
    await cleanupSession(request, session.id);
  }
});
