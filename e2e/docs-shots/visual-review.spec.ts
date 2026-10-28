/**
 * Review the real app across palettes and useful window widths on the docs fleet.
 * These unannotated pictures belong to development review, never to a docs page
 * or publication. Later Steel changes extend this set when they add a surface.
 * Setup uses only the approved invented host and its simulated backend;
 * Remove stops at Cancel, and uninstall at a live-session refusal before probes.
 * Native updater/App controls, loaded SSH-help commands, successful uninstall
 * plans remain uncaptured. Setup progress uses the existing owned backend's
 * held action, with the real helm publishing progress and controlled failures.
 */
import { expect, test, type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { hostRowByName, listHosts, openHostMenu, openRowMenu, patchPreferences, readPreferences, showHostsPanel, showSessionsPanel } from "../tests/helpers/fleet";
import { chooseLauncherHost } from "../tests/helpers/host-picker";
import { openSettings, selectSettingsSection } from "../tests/helpers/settings";
import { attachSession, waitForTermText } from "../tests/helpers/term";
import { waitForSessionRevealed } from "../tests/helpers/terminal-readiness";
import { clickDialogGap } from "../tests/helpers/dialog-gap";
import { lastTranscriptLine, readStackInfo } from "../readme-hero/stage";
import { readFile, writeFile, rename } from "node:fs/promises";
import path from "node:path";
import { randomUUID } from "node:crypto";
import type { Scenario, ScenarioSession } from "../readme-hero/scenario";
import { open, openLauncher, rowOf } from "./open";
import { DOCS_SHOTS_DIR, DOCS_STACK_INFO_PATH } from "./paths";
import { shot as captureShot, type CropOptions } from "./shot";
import { readFleet } from "./stage-docs";
import { chooseStatus } from "../tests/helpers/list-header";
import { chooseHost, hostChoice } from "../tests/helpers/host-rail";
import { mountNativeTunnelConsentPreview, mountNativeTunnelPreview } from "../tests/helpers/native-tunnel-preview";

// A focused PR needs its changed states, rather than every unchanged surface.
// Each selection retains the same script, fleet, shot helper and cleanup.
const sidebarHeaderOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sidebar-header";
const sidebarStatusOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sidebar-status";
const sidebarRailOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sidebar-rail";
const sidebarWarningOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sidebar-warning";
const toolbarOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "session-toolbar";
const tunnelOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "session-tunnels";
const settingsOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "settings";
const menusOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "menus";
const dialogsOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "dialogs";
const launcherOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "launcher";

/** Photograph native-only chrome with the goal's explicit CSS-only fallback.
 * The ordinary staged web app supplies the surrounding frame and terminals.
 * Only invented listener markup is inserted, matching tunnels.rs; these presentation
 * fixtures prove geometry/contrast, never native callbacks, requests or consent.
 * Native Rust tests and review own that evidence. No API reply is rewritten.
 */
async function tunnelShots(page: Page, name: string) {
  const topbar = page.locator(".session-topbar");
  const original = (await topbar.boundingBox())!;
  const terminal = (await page.locator(".terminal-panes").boundingBox())!;
  for (const state of ["one", "one-list", "many", "many-list", "closing", "error", "error-list"] as const) {
    await mountNativeTunnelPreview(page, state);
    expect((await topbar.boundingBox())!.height, "the preview preserves the desktop top-row height").toBe(original.height);
    expect(await page.locator(".terminal-panes").boundingBox(), "the native preview adds no terminal row").toEqual(terminal);
    const pane = (await page.locator(".app-main").boundingBox())!;
    const actions = page.locator(".titlebar-actions");
    for (const control of await actions.locator("button:visible").all()) {
      const box = (await control.boundingBox())!;
      expect(box.x).toBeGreaterThanOrEqual(pane.x);
      expect(box.x + box.width).toBeLessThanOrEqual(pane.x + pane.width);
    }
    if (state !== "error") {
      const toggle = page.locator(".session-tunnel-toggle");
      const pill = (await toggle.boundingBox())!;
      const tabs = (await page.locator(".tab-strip").boundingBox())!;
      expect(pill.x).toBeGreaterThanOrEqual(tabs.x + tabs.width);
      expect(pill.x + pill.width).toBeLessThanOrEqual((await actions.boundingBox())!.x);
      expect(pill.width).toBeLessThanOrEqual(280);
      const selected = (await page.locator(".tab.selected").boundingBox())!;
      expect(selected.x, "the selected tab starts inside its scrollport").toBeGreaterThanOrEqual(tabs.x - 1);
      expect(selected.x + selected.width, "the selected tab remains readable beside the pill").toBeLessThanOrEqual(tabs.x + tabs.width + 1);
      if (state === "one" || state === "one-list") {
        const fixed = toggle.locator(".session-tunnel-mapping > span:not(.session-tunnel-host)");
        await expect(fixed).toHaveText([":57779 → ", ":44227"]);
        for (const port of await fixed.all()) {
          expect(await port.evaluate((element) => element.scrollWidth <= element.clientWidth), "both ports remain readable").toBe(true);
        }
      }
    }
    const panel = page.locator(".session-tunnel-panel");
    if (await panel.count()) {
      await insideViewport(page, panel);
      // Native keyboard focus is real; no fixture click pretends to close a
      // listener. The callback and offline contracts are native-only evidence.
      const close = panel.locator(".session-tunnel-close-all");
      await keyboardFocusControl(page, page.locator("body"), close);
      await focusedShot(page, `${name}-tunnel-${state}`, close);
    } else if (state === "error") {
      await insideViewport(page, page.locator(".session-tunnel-error"));
      await shot(page, `${name}-tunnel-${state}`, [page.locator("body")], { pad: 0 });
    } else {
      await shot(page, `${name}-tunnel-${state}`, [page.locator("body")], { pad: 0 });
      if (state === "one") {
        const toggle = page.locator(".session-tunnel-toggle");
        // Approach from the next toolbar stop. Visiting Add first would
        // deliberately scroll the tab strip to that other control.
        await toggle.focus();
        await page.keyboard.press("Tab");
        await page.keyboard.press("Shift+Tab");
        await expect(toggle).toBeFocused();
        expect(await toggle.evaluate((element) => element.matches(":focus-visible"))).toBe(true);
        await focusedShot(page, `${name}-tunnel-one-focused`, toggle);
        await page.keyboard.press("Escape");
        await expect(page.locator(".farhelm-tooltip")).toBeHidden();
      }
    }
  }
  await page.evaluate(() => document.querySelectorAll("[data-tunnel-preview]").forEach((element) => element.remove()));
}

/** Park before observing tooltip state, not only at the screenshot's last step.
 * Pointer departure cancels pending hover help. Focus captures establish their
 * own dismissal or tooltip premise without changing the photographed surface.
 */
async function shot(page: Page, name: string, regions: Locator[], options: CropOptions = {}, preserveFocus = false) {
  await page.mouse.move(0, 0);
  expect(await page.evaluate(() => document.elementFromPoint(0, 0)?.closest("[data-tooltip], [data-tooltip-card]") == null),
    "the parked pointer must not own another tooltip").toBe(true);
  if (!preserveFocus) {
    await expect(page.locator(".farhelm-tooltip")).toBeHidden();
  }
  return captureShot(page, name, regions, options);
}

/** Observe real keyboard focus without activating the chosen control.
 * Modal focus loops provide a known close-button origin. Other surfaces use
 * the target's preceding native Tab stop, retaining the existing DOM order.
 */
async function keyboardFocusControl(page: Page, root: Locator, control: Locator) {
  await expect(control).toBeVisible();
  const close = root.locator(".dialog-close");
  if (await close.count()) {
    await close.first().focus();
    const stops = await root.locator("button:visible, input:visible, select:visible, textarea:visible, a[href]:visible").count();
    for (let step = 0; step <= stops; step++) {
      await page.keyboard.press("Tab");
      if (await control.evaluate((element) => document.activeElement === element)) break;
    }
  } else {
    await control.focus();
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Tab");
  }
  await expect(control).toBeFocused();
  expect(await control.evaluate((element) => element.matches(":focus-visible")), "focus must come from native keyboard input").toBe(true);
}

/** Keep focus evidence through supported tooltip dismissal, or show its help.
 * Gap clicks are supported only where the product preserves focus on gaps.
 * Main chrome and menus instead show the actual keyboard tooltip deliberately.
 */
async function focusedShot(page: Page, name: string, control: Locator, gapPanel?: Locator) {
  await page.mouse.move(0, 0);
  let showTooltip = false;
  if (gapPanel) await clickDialogGap(page, gapPanel);
  else {
    const words = await control.evaluate((element) => element.closest("[data-tooltip]")?.getAttribute("data-tooltip"));
    if (words) {
      await expect(page.locator(".farhelm-tooltip")).toBeVisible();
      await expect(page.locator(".farhelm-tooltip")).toHaveText(words);
      showTooltip = true;
    }
  }
  await expect(control).toBeFocused();
  expect(await control.evaluate((element) => element.matches(":focus-visible")), "tooltip handling must preserve visible keyboard focus").toBe(true);
  if (!showTooltip) await expect(page.locator(".farhelm-tooltip")).toBeHidden();
  await shot(page, name, [page.locator("body")], { pad: 0 }, true);
}

/** Select a real staged terminal only after its replay and transcript are visible. */
async function showSession(page: Page, session: ScenarioSession) {
  const id = readFleet().ids.get(session.title);
  if (!id) throw new Error(`the staged fleet has no session ${session.title}`);
  const row = page.locator(`.session-row[data-session-id="${id}"]`);
  // Closing a menu returns focus to its toggle. A picture needs revealed
  // replay, not keyboard-input readiness, when the session is already open.
  if (await row.getAttribute("data-session-selected") === "true") await waitForSessionRevealed(page, id);
  else await attachSession(page, id);
  if (session.transcript) await waitForTermText(page, lastTranscriptLine(DOCS_SHOTS_DIR, session.transcript), 30_000);
  await expect(row).toHaveAttribute("data-session-selected", "true");
  return row;
}

/** The real host fields needed to preserve this capture's remote identity. */
interface CaptureHost {
  id: number;
  kind: string;
  destination: string | null;
  identity: string | null;
  state: { phase: string; last_error?: string | null };
}

/** Read through the API context so destination rewrites cannot enter cleanup. */
async function realHosts(request: APIRequestContext): Promise<CaptureHost[]> {
  const response = await request.get("/api/hosts");
  expect(response.ok(), "read the owned fleet's actual host destinations").toBe(true);
  return (await response.json()).hosts;
}

/** Require the painted surface to fit, rather than trusting screenshot clipping.
 * One CSS pixel allows fractional layout rounding at the viewport edge. This
 * does not establish bottom-clamp placement; that needs its own row premise.
 */
async function insideViewport(page: Page, surface: Locator) {
  await expect(surface).toBeVisible();
  const box = await surface.boundingBox();
  const viewport = page.viewportSize();
  expect(box).not.toBeNull();
  expect(viewport).not.toBeNull();
  expect(box!.x).toBeGreaterThanOrEqual(-1);
  expect(box!.y).toBeGreaterThanOrEqual(-1);
  expect(box!.x + box!.width).toBeLessThanOrEqual(viewport!.width + 1);
  expect(box!.y + box!.height).toBeLessThanOrEqual(viewport!.height + 1);
}

/** Inspect the real menu frame without adding actions or a placement oracle.
 * Headers stay outside native menu navigation. Ordinary descriptions wrap;
 * peer headers may retain their bounded line caps and full hover text. A short
 * window may scroll its panel, but must still make the final action reachable.
 */
async function inspectMenu(page: Page, panel: Locator) {
  await insideViewport(page, panel);
  await expect(panel.locator(".menu-header")).toHaveCount(1);
  const menu = panel.getByRole("menu");
  await expect(menu.locator(".menu-header")).toHaveCount(0);
  for (const header of await panel.locator(".menu-title, .menu-summary").all()) {
    expect(await header.evaluate((element) => element.scrollWidth <= element.clientWidth + 1),
      "the ordinary menu header fits its allocated width").toBe(true);
  }
  for (const description of await panel.locator(".menu-description").all()) {
    await expect(description).not.toHaveCSS("white-space", "nowrap");
    await expect(description).not.toHaveCSS("text-overflow", "ellipsis");
  }
  for (const item of await menu.getByRole("menuitem").all()) {
    for (const id of (await item.getAttribute("aria-describedby") ?? "").split(/\s+/).filter(Boolean)) {
      await expect(panel.locator(`[id=${JSON.stringify(id)}]`)).not.toBeEmpty();
    }
  }
  const last = menu.getByRole("menuitem").last();
  const scrollTop = await panel.evaluate((element) => element.scrollTop);
  // Scroll the panel itself: scrolling its sidebar ancestor invalidates the
  // measured anchor and correctly closes the menu instead of revealing it.
  await panel.evaluate((element) => { element.scrollTop = element.scrollHeight; });
  await expect(last).toBeInViewport();
  const lastBox = (await last.boundingBox())!;
  const panelBox = (await panel.boundingBox())!;
  expect(lastBox.y + lastBox.height).toBeLessThanOrEqual(panelBox.y + panelBox.height + 1);
  await panel.evaluate((element, top) => { element.scrollTop = top; }, scrollTop);
  await expect(menu).toBeVisible();
}

/** Show shortcut help as separate copy, including both real seeded owners.
 * No choice is activated: these pictures cannot change a binding or template.
 * The full review and focused menu review share the same popup lifecycle.
 */
async function numberMenuShots(page: Page, name: string, templates: Locator) {
  await templates.getByRole("button", { name: "number shortcut codex-deep", exact: true }).click();
  const panel = templates.locator(".templates-number-flyout .host-row-menu-panel");
  const menu = panel.getByRole("menu", { name: "number key for codex-deep", exact: true });
  try {
    await expect(menu).toBeVisible();
    await expect(menu.getByRole("menuitem")).toHaveCount(11);
    await expect(panel.locator(".templates-number-owner")).toHaveCount(2);
    await inspectMenu(page, panel);
    const choices = [
      ["No shortcut", "Remove this template's number shortcut", "none"],
      ["Ctrl+Shift+1", "This template", "own"],
      ["Ctrl+Shift+2", "No template uses this key", "unbound"],
      ["Ctrl+Shift+8", "quick-claude (moves here)", "moving"],
    ];
    for (const [label, description] of choices) {
      await expect(menu.getByRole("menuitem", { name: label, exact: true })).toHaveAccessibleDescription(description);
    }
    await shot(page, `${name}-templates-number-menu`, [page.locator("body")], { pad: 0 });
    for (const [label, , suffix] of choices) {
      // Home gives every traversal a known menu-owned origin. Arrow input
      // supplies keyboard modality without activating any option.
      await menu.getByRole("menuitem").first().press("ArrowDown");
      await expect(menu.getByRole("menuitem").nth(1)).toBeFocused();
      await page.keyboard.press("Home");
      await expect(menu.getByRole("menuitem").first()).toBeFocused();
      const option = menu.getByRole("menuitem", { name: label, exact: true });
      for (let step = 0; step < 11 && !await option.evaluate((element) => document.activeElement === element); step++) {
        await page.keyboard.press("ArrowDown");
      }
      await expect(option).toBeFocused();
      await focusedShot(page, `${name}-templates-number-${suffix}-focused`, option);
    }
    // The final key is 0. Native End must reveal it through the panel's own
    // scrolling; explicitly scrolling first would conceal a keyboard failure.
    await page.keyboard.press("End");
    const last = menu.getByRole("menuitem").last();
    await expect(last).toHaveAccessibleName("Ctrl+Shift+0");
    await expect(last).toBeFocused();
    await insideViewport(page, last);
    const lastBox = (await last.boundingBox())!;
    const panelBox = (await panel.boundingBox())!;
    expect(lastBox.y).toBeGreaterThanOrEqual(panelBox.y);
    expect(lastBox.y + lastBox.height).toBeLessThanOrEqual(panelBox.y + panelBox.height);
    await focusedShot(page, `${name}-templates-number-last-focused`, last);
  } finally {
    if (await menu.isVisible()) {
      await menu.getByRole("menuitem").first().focus();
      await page.keyboard.press("Escape");
      await expect(menu).toBeHidden();
    }
  }
}

/** Capture every changed session-menu state on one owned quiet session.
 * Seen and pin state come from real writes, not reply rewrites. Restore the
 * exact seen stamp after returning to the original terminal, so automatic
 * mark-on-open cannot undo cleanup. The short window must actually overflow.
 */
async function menuShots(page: Page, request: APIRequestContext, scenario: Scenario, name: string) {
  const session = scenario.sessions.find((candidate) => candidate.status === "idle" && candidate.seen === false);
  const initialId = await page.locator('.session-row[data-session-selected="true"]').getAttribute("data-session-id");
  const initial = scenario.sessions.find((candidate) => readFleet().ids.get(candidate.title) === initialId);
  if (!session || !initial) throw new Error("menu review needs its quiet staged session and original selected terminal");
  const row = rowOf(page, scenario, (candidate) => candidate === session);
  const id = await row.getAttribute("data-session-id");
  if (!id) throw new Error("the menu row must have a staged session identity");
  const response = await request.get("/api/sessions");
  expect(response.ok()).toBe(true);
  const saved = (await response.json()).sessions.find((candidate: { id: string }) => candidate.id === id);
  expect(saved?.status.state, "read/unread pictures need the quiet live fixture").toBe("idle");
  expect(Object.hasOwn(saved, "seen_activity_at")).toBe(true);
  expect(typeof saved.pinned).toBe("boolean");
  const viewport = page.viewportSize()!;
  const pinUrl = `/api/sessions/${encodeURIComponent(id)}/pin`;
  const panel = row.locator(".session-row-menu-panel");
  const window = [page.locator("body")];
  // Cleanup can run after pointer activation. Re-establish menu-owned focus
  // before Escape so dismissal follows the same supported handoff as a user.
  const closeMenu = async () => {
    if (await panel.isVisible()) {
      await panel.getByRole("menuitem").first().focus();
      await page.keyboard.press("Escape");
      await expect(panel).toBeHidden();
    }
  };
  try {
    expect((await request.put(pinUrl, { data: { pinned: false } })).ok()).toBe(true);
    await expect(row).toHaveAttribute("data-session-pinned", "false");
    // Selecting establishes read state through the ordinary reveal path.
    await showSession(page, session);
    await openRowMenu(row);
    await expect(panel.getByRole("menuitem")).toHaveCount(8);
    await expect(panel.locator(".menu-description")).toHaveCount(8);
    await expect(panel.getByRole("separator")).toHaveCount(4);
    await expect(panel.getByRole("menuitem", { name: "mark unread", exact: true }))
      .toHaveAccessibleDescription("Show new output again");
    await expect(panel.getByRole("menuitem", { name: "Pin to top", exact: true }))
      .toHaveAccessibleDescription("Keep this session above unpinned ones");
    await inspectMenu(page, panel);
    await shot(page, `${name}-session-menu`, window, { pad: 0 });
    const items = panel.getByRole("menuitem");
    await items.first().press("ArrowDown");
    await expect(items.nth(1)).toBeFocused();
    await items.nth(1).press("Home");
    await focusedShot(page, `${name}-session-menu-focused`, items.first());
    await panel.getByRole("menuitem", { name: "mark unread", exact: true }).click();
    await expect(panel).toBeHidden();
    await expect(row.locator(".status-badge.idle.unseen")).toHaveCount(1);
    await openRowMenu(row);
    await expect(panel.getByRole("menuitem", { name: "mark read", exact: true }))
      .toHaveAccessibleDescription("Treat the latest output as seen");
    await shot(page, `${name}-session-menu-mark-read`, window, { pad: 0 });
    await closeMenu();
    expect((await request.put(pinUrl, { data: { pinned: true } })).ok()).toBe(true);
    await expect(row).toHaveAttribute("data-session-pinned", "true");
    await openRowMenu(row);
    await expect(panel.getByRole("menuitem", { name: "Unpin", exact: true }))
      .toHaveAccessibleDescription("Sort this session with the others again");
    await inspectMenu(page, panel);
    await shot(page, `${name}-session-menu-unpin`, window, { pad: 0 });
    await closeMenu();

    await page.setViewportSize({ ...viewport, height: 400 });
    await row.scrollIntoViewIfNeeded();
    await openRowMenu(row);
    await expect(panel.getByRole("menuitem")).toHaveCount(8);
    expect(await panel.evaluate((element) => element.scrollHeight > element.clientHeight + 1),
      "the short-menu picture must exercise actual panel overflow").toBe(true);
    await inspectMenu(page, panel);
    await shot(page, `${name}-session-menu-short`, window, { pad: 0 });
    await items.first().press("End");
    const last = items.last();
    await expect(last).toHaveAccessibleName("delete");
    await expect(last).toBeFocused();
    await expect(last).toBeInViewport();
    await focusedShot(page, `${name}-session-menu-short-last-focused`, last);
    await closeMenu();
    await page.setViewportSize(viewport);

    await page.locator(".templates-button").click();
    const templates = page.locator(".templates-dialog");
    try {
      await expect(templates.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
      expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": "codex-deep", "8": "quick-claude" });
      await numberMenuShots(page, name, templates);
    } finally {
      if (await templates.isVisible()) await templates.locator(".templates-close").click();
      await expect(templates).toBeHidden();
    }
  } finally {
    await closeMenu();
    await page.setViewportSize(viewport);
    await showSession(page, initial);
    expect((await request.put(pinUrl, { data: { pinned: saved.pinned } })).ok()).toBe(true);
    expect((await request.put(`/api/sessions/${encodeURIComponent(id)}/seen`, {
      data: { seen_activity_at: saved.seen_activity_at },
    })).ok()).toBe(true);
    await expect(row).toHaveAttribute("data-session-pinned", String(saved.pinned));
    await expect.poll(async () => {
      const listing = await request.get("/api/sessions");
      expect(listing.ok()).toBe(true);
      return (await listing.json()).sessions.find((candidate: { id: string }) => candidate.id === id)?.seen_activity_at;
    }, { message: "restore the menu fixture's exact original seen stamp" }).toBe(saved.seen_activity_at);
  }
}

/** Photograph the two changed permanent-answer hints, cancelling both plans.
 * The connected host keeps its exact identity. Setup uses only the approved
 * invented destination and never submits either affirmative answer.
 */
async function hostHintShots(page: Page, request: APIRequestContext, scenario: Scenario, name: string, includeAdd = false) {
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote || !scenario.add_host) throw new Error("host hint review needs the staged remote and invented setup destination");
  const originalHost = (await realHosts(request)).find((candidate) => candidate.destination === remote.ssh);
  if (!originalHost?.identity) throw new Error("the host question needs the connected staged install");
  expect(originalHost.state.phase).toBe("connected");
  expect((await readPreferences(request)).skip_host_remove_confirmation).toBe(false);
  const host = hostRowByName(page, remote.alias);
  const remove = page.locator(".host-remove-dialog");
  const window = [page.locator("body")];
  const hint = 'Undo "don\'t ask again" with the gear at the foot of the host rail';
  try {
    await openHostMenu(host);
    await host.locator(".host-remove").click();
    await expect(remove).toBeVisible();
    await expect(remove.locator(".dialog-actions button")).toHaveText(["cancel", "remove, don't ask again", "remove"]);
    await expect(remove.locator(".btn-outline")).toHaveClass(/btn-danger/);
    await expect(remove.locator("#host-remove-permanent-hint")).toHaveText(hint);
    await insideViewport(page, remove.locator("#host-remove-permanent-hint"));
    await shot(page, `${name}-remove-host`, window, { pad: 0 });
  } finally {
    if (await remove.isVisible()) await remove.locator(".host-settings-close").click();
    await expect(remove).toBeHidden();
    await sameConnectedHost(request, originalHost);
  }

  expect((await readPreferences(request)).skip_host_setup_confirmation).toBe(false);
  const beforeAdd = (await realHosts(request)).map((candidate) => candidate.id).sort((a, b) => a - b);
  const add = page.locator(".host-add-dialog");
  try {
    await showHostsPanel(page);
    await page.locator(".add-host-button").click();
    await expect(add).toBeVisible();
    await add.locator(".add-host-ssh").fill(scenario.add_host.ssh);
    await expect(add.locator(".add-host-ssh")).toHaveCSS("font-family", /JetBrains Mono/);
    await expect(add.locator(".add-host-submit")).toBeEnabled();
    if (includeAdd) await shot(page, `${name}-add-host`, window, { pad: 0 });
    await add.locator(".add-host-submit").click();
    await expect(add.locator(".provisioning-plan")).toContainText(scenario.add_host.ssh);
    await expect(add.locator(".provisioning-confirm")).toBeVisible();
    await expect(add.locator(".host-setup-permanent")).toHaveClass(/btn-outline/);
    await expect(add.locator(".dialog-actions button")).toHaveText(["cancel", "set up, don't ask again", "set up"]);
    await expect(add.locator("#host-setup-permanent-hint")).toHaveText(hint);
    await insideViewport(page, add.locator("#host-setup-permanent-hint"));
    await shot(page, `${name}-setup-plan`, window, { pad: 0 });
  } finally {
    if (await add.isVisible()) await add.locator(".add-host-cancel").click();
    await expect(add).toBeHidden();
    expect((await realHosts(request)).map((candidate) => candidate.id).sort((a, b) => a - b)).toEqual(beforeAdd);
    await showSessionsPanel(page);
  }
  return originalHost;
}

/** Scroll only the product's existing scrollports, preserving frame ownership.
 * Settings has nested content while the launcher has its own scrolling body;
 * neither needs a replacement wrapper or a synthetic layout for the capture.
 */
async function scrollToEnd(root: Locator) {
  await root.evaluate((element) => {
    for (const region of [element, ...element.querySelectorAll<HTMLElement>("*")]) {
      if (region.scrollHeight > region.clientHeight && /auto|scroll/.test(getComputedStyle(region).overflowY)) {
        region.scrollTop = region.scrollHeight;
      }
    }
  });
}

/** Review real launcher drafts without admitting a launch or template save.
 * Populated agent and command forms exercise the pinned footer. The reporting
 * fixture supplies a real restartable conversation; permission escalation uses
 * actual agent choices, and every operation ends at Cancel or Undo.
 */
async function launcherShots(page: Page, request: APIRequestContext, name: string, includePhone: boolean) {
  const viewport = page.viewportSize()!;
  const window = [page.locator("body")];
  const launcher = await openLauncher(page);
  // Typed fields can retain keyboard help after the pointer leaves. The
  // existing gap gesture dismisses it without changing the launcher draft.
  const capture = async (suffix: string) => {
    await clickDialogGap(page, launcher);
    await shot(page, `${name}-${suffix}`, window, { pad: 0 });
  };
  // The compact footer keeps the complete target in shared help. Photograph
  // its real text anchor, including when the phone layout ellipsizes it.
  const captureTargetHelp = async (suffix: string) => {
    const target = launcher.locator(".launch-composer-launch-context");
    await expect(target).not.toHaveAttribute("title", /./);
    await target.hover();
    const help = page.locator(".farhelm-tooltip");
    await expect(help).toBeVisible();
    await expect(help).toHaveText((await target.getAttribute("data-tooltip"))!);
    await captureShot(page, `${name}-${suffix}`, window, { pad: 0, keepPointer: true });
    await page.mouse.move(0, 0);
    await expect(help).toBeHidden();
  };
  const close = async () => {
    await launcher.locator(".launch-composer-cancel").click();
    await expect(launcher).toBeHidden();
  };
  const populateAgent = async () => {
    // Scenario history has invented paths; use a real portable destination
    // so ordinary draft pictures do not start with an unrelated folder error.
    await launcher.getByLabel("folder", { exact: true }).fill("~");
    await launcher.getByLabel("folder", { exact: true }).press("Escape");
    await expect(launcher.getByRole("listbox", { name: "folders" })).toHaveCount(0);
    await launcher.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await launcher.locator(".named-conversation").fill("review-conversation");
    await launcher.locator(".initial-prompt").fill("Review the session launcher.\nKeep the action footer visible.\nExplain the result.");
    await expect(launcher.locator(".launch-composer-summary")).toContainText("prompt: 3 lines");
  };
  try {
    await populateAgent();
    await launcher.locator(".launch-composer-body").evaluate((element) => { element.scrollTop = 0; });
    await capture("launcher");
    const footer = launcher.locator(".launch-composer-footer");
    await insideViewport(page, footer);
    await captureTargetHelp("launcher-target-tooltip");
    const pinned = await footer.boundingBox();
    await scrollToEnd(launcher);
    expect(await footer.boundingBox(), "scrolling choices must not move the action footer").toEqual(pinned);
    await capture("launcher-end");

    const wrapper = launcher.locator(".launch-composer-wrapper");
    await wrapper.fill("with-endpoint 'unfinished");
    await expect(wrapper).toHaveAttribute("aria-invalid", "true");
    // A refused folder Enter must clear the floating suggestions above the
    // next controls. Use the real client refusal without launching a session.
    await expect(launcher.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "true");
    const folder = launcher.getByRole("combobox", { name: "folder", exact: true });
    const folderOptions = launcher.getByRole("listbox", { name: "folders", exact: true });
    await folder.focus();
    await expect(folderOptions).toBeVisible();
    await expect(folderOptions.locator('[aria-selected="true"]')).toHaveCount(0);
    await shot(page, `${name}-launcher-folder-suggestions`, window, { pad: 0 });
    await folder.press("Enter");
    await expect(launcher.locator(".launch-composer-refusal")).toContainText("missing closing quote");
    await expect(folderOptions).toHaveCount(0);
    await expect(folder).toHaveAttribute("aria-expanded", "false");
    await expect(folder).toHaveValue("~");
    await insideViewport(page, launcher.locator(".launch-composer-refusal"));
    await capture("launcher-refusal");
    // The refusal belongs to that attempt until the launcher is dismissed.
    // Start a fresh valid draft for the independent views that follow it.
    await close();
    await openLauncher(page);
    await populateAgent();
    await expect(launcher.locator(".launch-composer-refusal")).toHaveCount(0);

    // The staged fleet has no cloned repository. Show the real destination
    // editor, while ordinary managed-checkout tests cover its preview states.
    const destination = launcher.getByRole("group", { name: "destination type", exact: true });
    await destination.getByRole("button", { name: "managed checkout", exact: true }).click();
    await expect(launcher.getByRole("combobox", { name: "repository", exact: true })).toBeVisible();
    await capture("launcher-managed-checkout");
    await destination.getByRole("button", { name: "folder", exact: true }).click();

    await showOmpEffort(page, request, launcher);
    await capture("launcher-omp-effort");
    await launcher.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
    await launcher.locator(".launch-composer-harness-choice").getByRole("button", { name: "Pi", exact: true }).click();
    const banner = launcher.locator(".permission-escalation-banner");
    await expect(banner).toContainText("Permission changed: Guarded → YOLO");
    await launcher.locator(".launch-composer-body").evaluate((element) => { element.scrollTop = 0; });
    // Its animation is also its lifetime. Use the existing screenshot option
    // and real hover pause, or fast-forwarding would dismiss it before Undo.
    await banner.locator(":scope > span").hover();
    await expect(banner).toHaveCSS("animation-play-state", "paused");
    await expect(page.locator(".farhelm-tooltip")).toBeHidden();
    await captureShot(page, `${name}-launcher-permission-banner`, window,
      { pad: 0, keepPointer: true, keepAnimations: true });
    await expect(banner).toBeVisible();
    await banner.getByRole("button", { name: "undo", exact: true }).click();
    await expect(banner).toBeHidden();

    await launcher.getByRole("tab", { name: "command", exact: true }).click();
    await launcher.getByLabel("agent command", { exact: true }).fill("claude {farhelm_args}");
    await launcher.getByRole("radio", { name: "no", exact: true }).check();
    await launcher.getByRole("combobox", { name: "agent type", exact: true }).selectOption("claude");
    await launcher.locator(".launch-command-resume-toggle").check();
    await launcher.getByLabel("resume command", { exact: true }).fill("claude --resume {conversation} {farhelm_args}");
    await capture("launcher-command");
    await launcher.locator(".launch-composer-save-template").click();
    const panel = launcher.locator(".save-template-panel");
    await expect(panel.locator(".save-template-name")).toBeFocused();
    await capture("launcher-save-template");
    await panel.locator(".save-template-cancel").click();
    await expect(panel).toBeHidden();
    await close();

    const fleet = readFleet();
    expect(fleet.toolbarReview, "launcher mode review needs the real reporting fixture").toBeDefined();
    const id = fleet.ids.get(fleet.toolbarReview!.title)!;
    await attachSession(page, id);
    await waitForSessionRevealed(page, id);
    for (const action of ["Replace", "Restart"] as const) {
      await page.getByRole("button", { name: `${action} options`, exact: true }).click();
      const item = page.getByRole("menu", { name: action, exact: true }).getByRole("menuitem", { name: `${action} with…`, exact: true });
      await expect(item).toHaveAttribute("aria-disabled", "false");
      await item.click();
      await expect(launcher).toHaveAttribute("data-launcher-mode", action === "Restart" ? "restart" : "replace");
      await insideViewport(page, launcher.locator(".dialog-footer"));
      await capture(`launcher-${action.toLowerCase()}`);
      await close();
    }
    if (includePhone) {
      await page.setViewportSize({ width: 390, height: 844 });
      await openLauncher(page);
      await populateAgent();
      await launcher.locator(".launch-composer-body").evaluate((element) => { element.scrollTop = 0; });
      await insideViewport(page, launcher.locator(".dialog-header"));
      await insideViewport(page, launcher.locator(".dialog-footer"));
      await capture("launcher-phone");
      await scrollToEnd(launcher);
      await capture("launcher-phone-end");
      // A typed draft needs no filesystem or API staging: never Launch it.
      // Require actual truncation so this view proves why full help matters.
      const longPath = "~/work/projects/a-long-directory-name-for-launch-summary";
      const folder = launcher.getByLabel("folder", { exact: true });
      await folder.fill(longPath);
      await folder.press("Escape");
      await expect(launcher.getByRole("listbox", { name: "folders" })).toHaveCount(0);
      await clickDialogGap(page, launcher);
      const target = launcher.locator(".launch-composer-launch-context");
      await expect(target).toContainText(longPath);
      expect(await target.evaluate((node) => node.scrollWidth > node.clientWidth),
        "the phone draft must actually ellipsize its long target").toBe(true);
      await captureTargetHelp("launcher-phone-long-target-tooltip");
      await close();
    }
    await page.setViewportSize(viewport);
    await launcherYoloShots(page, request, name, includePhone);
  } finally {
    if (await launcher.isVisible()) {
      const panelCancel = launcher.locator(".save-template-cancel");
      if (await panelCancel.isVisible()) await panelCancel.click();
      await close();
    }
    await page.setViewportSize(viewport);
  }
}

/** Photograph the real refusal and both ends of its bounded answer region.
 * The host must ask first; no affirmative answer is sent. On phones, the
 * question can scroll while the ordinary launcher actions stay visible.
 */
async function launcherYoloShots(page: Page, request: APIRequestContext, name: string, includePhone: boolean) {
  const viewport = page.viewportSize()!;
  const window = [page.locator("body")];
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local?.yolo_without_asking, "the staged host must ask before a YOLO launch").toBe(false);
  const launcher = await openLauncher(page);
  await launcher.getByRole("tab", { name: "agent", exact: true }).click();
  await chooseLauncherHost(launcher.getByRole("combobox", { name: "host", exact: true }), String(local!.id));
  await launcher.getByLabel("folder", { exact: true }).fill("~");
  await launcher.getByLabel("folder", { exact: true }).press("Escape");
  await expect(launcher.getByRole("listbox", { name: "folders" })).toHaveCount(0);
  await launcher.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await launcher.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  const [refusal] = await Promise.all([
    page.waitForResponse((response) => response.request().method() === "POST" && response.url().endsWith("/api/sessions")),
    // Native keyboard submission gives the initial safe answer visible
    // keyboard focus, rather than the pointer modality of a Launch click.
    launcher.locator(".create-session-submit").press("Enter"),
  ]);
  expect(refusal.status(), "the helm refuses the unconfirmed launch").toBe(409);
  const question = launcher.locator(".yolo-confirmation");
  const notices = launcher.locator(".launch-composer-notices");
  await expect(question.locator(".yolo-cancel")).toBeFocused();
  expect(await question.locator(".yolo-cancel").evaluate((element) => element.matches(":focus-visible"))).toBe(true);
  await expect(question.locator(".yolo-cancel")).toBeInViewport({ ratio: 0.99 });
  await insideViewport(page, notices);
  await focusedShot(page, `${name}-yolo-question-focused`, question.locator(".yolo-cancel"), launcher);
  await notices.evaluate((element) => { element.scrollTop = 0; });
  await shot(page, `${name}-yolo-question`, window, { pad: 0 });
  await scrollToEnd(notices);
  await shot(page, `${name}-yolo-question-end`, window, { pad: 0 });
  if (includePhone) {
    for (const [height, suffix] of [[844, "phone"], [500, "phone-short"]] as const) {
      await page.setViewportSize({ width: 390, height });
      if (height === 500) {
        // The short-phone footer used to leave only a sliver for answers.
        // Native Tab must reveal the safe answer before its focus photo.
        await keyboardFocusControl(page, launcher, question.locator(".yolo-cancel"));
        await expect(question.locator(".yolo-cancel")).toBeInViewport({ ratio: 0.99 });
        await focusedShot(page, `${name}-yolo-question-${suffix}-focused`, question.locator(".yolo-cancel"), launcher);
      }
      await notices.evaluate((element) => { element.scrollTop = 0; });
      await insideViewport(page, launcher.locator(".launch-composer-footer"));
      await shot(page, `${name}-yolo-question-${suffix}`, window, { pad: 0 });
      await scrollToEnd(notices);
      await expect(question.locator(".yolo-confirm")).toBeInViewport();
      await shot(page, `${name}-yolo-question-${suffix}-end`, window, { pad: 0 });
    }
  }
  await question.locator(".yolo-cancel").click();
  await expect(question).toBeHidden();
  await launcher.locator(".launch-composer-cancel").click();
  await expect(launcher).toBeHidden();
  await page.setViewportSize(viewport);
}

/** Photograph changed dialog frames and answers, cancelling every operation.
 * Rename's refusal is real: a newline is an invalid title, whereas an empty
 * title is legal. Templates only edits an unsaved draft. YOLO stops at the
 * helm's real consent refusal, with the owned host's asking policy verified
 * first; no affirmative answer or successful launch belongs in this capture.
 */
async function dialogShots(page: Page, request: APIRequestContext, scenario: Scenario, name: string, includePhone: boolean) {
  const viewport = page.viewportSize()!;
  const window = [page.locator("body")];
  const row = rowOf(page, scenario, (session) => session.status === "idle" && session.seen === false);
  const id = await row.getAttribute("data-session-id");
  if (!id) throw new Error("Rename review needs its staged session identity");
  await openRowMenu(row);
  await row.locator(".session-row-rename").click();
  const rename = page.locator(".rename-dialog");
  await expect(rename.locator(".rename-input")).toBeFocused();
  await shot(page, `${name}-rename`, window, { pad: 0 });
  await rename.locator(".rename-input").fill("review\ninvalid");
  const [refusal] = await Promise.all([
    page.waitForResponse((response) => response.request().method() === "POST" && response.url().endsWith(`/api/sessions/${encodeURIComponent(id)}/rename`)),
    rename.locator(".rename-submit").click(),
  ]);
  expect(refusal.status(), "the supervisor rejects the control character without renaming").toBe(400);
  await expect(rename.locator(".rename-error")).toBeVisible();
  await shot(page, `${name}-rename-refusal`, window, { pad: 0 });
  if (includePhone) {
    await page.setViewportSize({ width: 390, height: 320 });
    await insideViewport(page, rename.locator(".dialog-header"));
    await insideViewport(page, rename.locator(".dialog-footer"));
    await shot(page, `${name}-rename-phone-refusal`, window, { pad: 0 });
    await scrollToEnd(rename);
    await insideViewport(page, rename.locator(".rename-input"));
    await shot(page, `${name}-rename-phone-refusal-end`, window, { pad: 0 });
    await page.setViewportSize(viewport);
  }
  await rename.locator(".rename-cancel").click();
  await expect(rename).toBeHidden();

  await page.locator(".templates-button").click();
  const templates = page.locator(".templates-dialog");
  await expect(templates.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
  await templates.locator(".templates-row").filter({ has: page.getByText("codex-deep", { exact: true }) }).click();
  await expect(templates.locator(".templates-name")).toHaveValue("codex-deep");
  await expect(templates.locator(".templates-name")).toBeFocused();
  // A neutral editor shot must retire the name field's delayed focus tooltip,
  // not merely park the pointer while that field still owns focus.
  await templates.focus();
  await expect(templates).toBeFocused();
  await shot(page, `${name}-template-editor`, window, { pad: 0 });
  await templates.locator(".templates-name").fill("codex-deep-unsaved");
  await templates.locator(".templates-close").click();
  await expect(templates.locator(".templates-save-leave")).toBeFocused();
  await shot(page, `${name}-template-leave-question`, window, { pad: 0 });
  await templates.locator(".templates-keep-editing").click();
  await expect(templates.locator(".templates-name")).toHaveValue("codex-deep-unsaved");
  await expect(templates.locator(".templates-name")).toBeFocused();
  if (includePhone) {
    await page.setViewportSize({ width: 390, height: 844 });
    await templates.focus();
    await expect(templates).toBeFocused();
    await shot(page, `${name}-template-editor-phone`, window, { pad: 0 });
    await templates.locator(".templates-close").click();
    await expect(templates.locator(".templates-save-leave")).toBeFocused();
    await expect(templates.locator(".templates-editor-footer")).toBeInViewport({ ratio: 0.99 });
    await shot(page, `${name}-template-leave-question-phone`, window, { pad: 0 });
  } else await templates.locator(".templates-close").click();
  await templates.locator(".templates-discard").click();
  await expect(templates).toBeHidden();
  await page.setViewportSize(viewport);

  await page.locator(".checkout-trash-button").click();
  const trash = page.locator(".checkout-trash-dialog");
  await expect(trash.locator(".checkout-trash-entry")).toHaveCount(3);
  for (const button of await trash.locator(".trash-entry-delete").all()) await expect(button).toBeEnabled();
  await expect(trash.locator(".dialog-footer")).toHaveCount(0);
  await shot(page, `${name}-trash`, window, { pad: 0 });
  await trash.locator(".trash-delete-all").click();
  // The existing inline question initially focuses its enabled main answer.
  // Photograph that handoff, then cancel without sending a delete request.
  await expect(trash.locator(".trash-confirm-submit")).toBeFocused();
  await shot(page, `${name}-trash-confirmation`, window, { pad: 0 });
  await trash.locator(".trash-confirm-cancel").click();
  await expect(trash.locator(".trash-confirm-cancel")).toBeHidden();
  if (includePhone) {
    await page.setViewportSize({ width: 390, height: 520 });
    // Resizing arms Trash's existing row-movement guard. Photograph its
    // resting controls only after that guard releases them again.
    for (const button of await trash.locator(".trash-entry-delete").all()) {
      await expect(button).toBeEnabled();
      await expect(button).toHaveCSS("opacity", "1");
    }
    await shot(page, `${name}-trash-phone`, window, { pad: 0 });
    await page.setViewportSize(viewport);
  }
  await page.keyboard.press("Escape");
  await expect(trash).toBeHidden();
  await launcherYoloShots(page, request, name, includePhone);

  // The consent controller is desktop-only. This established preview route
  // shows its exact frame/copy, with native callbacks explicitly untested here.
  try {
    for (const saving of [false, true]) {
      await mountNativeTunnelConsentPreview(page, saving);
      const consent = page.locator(".tunnel-consent-dialog");
      await insideViewport(page, consent.locator(".dialog-header"));
      await insideViewport(page, consent.locator(".dialog-footer"));
      await shot(page, `${name}-tunnel-consent${saving ? "-saving" : ""}`, window, { pad: 0 });
    }
    if (includePhone) {
      await page.setViewportSize({ width: 390, height: 520 });
      await mountNativeTunnelConsentPreview(page);
      const consent = page.locator(".tunnel-consent-dialog");
      const body = consent.locator(".dialog-body");
      await insideViewport(page, consent.locator(".dialog-header"));
      await insideViewport(page, consent.locator(".dialog-footer"));
      expect(await body.evaluate((element) => element.scrollHeight > element.clientHeight), "the warning must genuinely scroll in a short phone frame").toBe(true);
      await shot(page, `${name}-tunnel-consent-phone`, window, { pad: 0 });
      await scrollToEnd(consent);
      await shot(page, `${name}-tunnel-consent-phone-end`, window, { pad: 0 });
    }
  } finally {
    await page.evaluate(() => document.querySelectorAll("[data-tunnel-consent-preview]").forEach((element) => element.remove()));
    await page.setViewportSize(viewport);
  }
}

/** Reach a checked switch with real Tab input so the visible focus ring is real.
 * A programmatic focus following a mouse click can keep pointer modality. The
 * known close control starts a bounded walk through this modal's native loop;
 * no Space/Enter is sent, because focus must not change settings or launch.
 */
async function keyboardFocusSwitch(page: Page, dialog: Locator, input: Locator) {
  await expect(input).toHaveAttribute("type", "checkbox");
  await expect(input).toBeChecked();
  await expect(input).toBeEnabled();
  await keyboardFocusControl(page, dialog, input);
}

/** Show every web Settings pane without saving the illustrative checkout drafts.
 * Both the full review and the Settings-only selection use this sequence, so
 * palette, focus and long-content evidence cannot drift between capture modes.
 * App remains native-only; no updater seam is introduced to photograph it.
 */
async function settingsShots(page: Page, name: string, includePhone: boolean) {
  const window = [page.locator("body")];
  const settings = await openSettings(page);
  await expect(settings.locator(".app-settings-navigation button")).toHaveCount(7);
  for (const part of [".host-settings-header", ".host-settings-footer", ".app-settings-close"]) {
    await insideViewport(page, settings.locator(part));
  }
  await expect(settings.getByRole("checkbox", { name: "skip session confirmations", exact: true })).not.toBeChecked();
  await shot(page, `${name}-settings`, window, { pad: 0 });
  const namingSwitch = settings.getByRole("checkbox", { name: "name sessions on quick launch", exact: true });
  await keyboardFocusSwitch(page, settings, namingSwitch);
  await focusedShot(page, `${name}-settings-focused`, namingSwitch, settings);
  await expect(namingSwitch).toBeChecked();
  await expect(namingSwitch).toBeEnabled();
  const selectedSection = settings.locator('.app-settings-navigation button[aria-pressed="true"]');
  await keyboardFocusControl(page, settings, selectedSection);
  await focusedShot(page, `${name}-settings-navigation-focused`, selectedSection, settings);
  await expect(selectedSection).toHaveAttribute("aria-pressed", "true");
  // Show the device choice's real default, without changing the stored palette.
  await selectSettingsSection(page, "Appearance");
  await expect(settings.getByRole("radio", { name: "System", exact: true })).toBeChecked();
  await shot(page, `${name}-settings-appearance`, window, { pad: 0 });
  const terminalSettings = await selectSettingsSection(page, "terminal");
  const terminalChoices = terminalSettings.getByRole("radiogroup", { name: "Terminal web links", exact: true });
  await expect(terminalChoices.getByRole("radio")).toHaveCount(3);
  await expect(terminalChoices.locator('input[type="radio"]:checked')).toHaveCount(1);
  for (const choice of await terminalChoices.getByRole("radio").all()) await expect(choice).toBeEnabled();
  await expect(terminalChoices.locator(".terminal-link-description")).toHaveCount(3);
  await terminalChoices.scrollIntoViewIfNeeded();
  await shot(page, `${name}-settings-terminal`, window, { pad: 0 });
  // API readback alone does not establish this mounted client's off state.
  const hostChoices = await selectSettingsSection(page, "hosts");
  for (const choice of await hostChoices.getByRole("checkbox").all()) {
    await expect(choice).toBeEnabled();
    await expect(choice).not.toBeChecked();
  }
  await expect(hostChoices.getByRole("checkbox")).toHaveCount(2);
  await shot(page, `${name}-settings-hosts`, window, { pad: 0 });
  const trust = await selectSettingsSection(page, "agent trust");
  // Enabling these real resources would affect future launches, not just pixels.
  for (const key of ["claude", "codex", "muse", "pi"]) {
    await expect(trust.locator(`#checkout-${key}-trust input`).first()).toBeEnabled();
  }
  await expect(trust.locator(".app-settings-error")).toHaveCount(0);
  await trust.getByRole("heading", { name: "Agent trust", exact: true }).scrollIntoViewIfNeeded();
  await shot(page, `${name}-settings-agent-trust`, window, { pad: 0 });
  await scrollToEnd(settings.locator(".host-settings-body"));
  await expect(trust.locator("#checkout-pi-trust input").first()).toBeInViewport();
  await shot(page, `${name}-settings-agent-trust-end`, window, { pad: 0 });
  const notifications = await selectSettingsSection(page, "notifications");
  const sounds = notifications.getByRole("region", { name: "sounds", exact: true });
  await expect.poll(() => page.evaluate(() => typeof (globalThis as typeof globalThis & {
    farhelmSounds?: { mountSettings?: unknown };
  }).farhelmSounds?.mountSettings), { message: "the real sound asset is available for its mounted controls" }).toBe("function");
  await expect(sounds.locator('[data-sound-event="waiting"]')).toBeChecked();
  await expect(sounds.locator('[data-sound-event="approval"]')).toBeChecked();
  await expect(sounds.locator('[data-sound-event="finished"]')).not.toBeChecked();
  const ntfy = notifications.locator(".ntfy-settings");
  await expect(ntfy.locator('input[type="url"]')).toHaveCSS("font-family", /JetBrains Mono/);
  await expect(ntfy.getByRole("checkbox", { name: "send ntfy alerts", exact: true })).toBeEnabled();
  await expect(ntfy.getByRole("checkbox", { name: "send ntfy alerts", exact: true })).not.toBeChecked();
  await sounds.scrollIntoViewIfNeeded();
  await shot(page, `${name}-settings-notifications`, window, { pad: 0 });
  await scrollToEnd(settings.locator(".host-settings-body"));
  await expect(ntfy.getByRole("button", { name: "save", exact: true })).toBeInViewport();
  await shot(page, `${name}-settings-notifications-end`, window, { pad: 0 });
  // Unsaved directory/command examples expose the mono fields beside Inter
  // labels without changing checkout configuration or running a command.
  await selectSettingsSection(page, "checkouts");
  await settings.locator(".checkout-root-field input").fill("~/work/checkouts");
  await settings.locator(".checkout-post-clone-field textarea").fill("cargo fetch --locked");
  await expect(settings.locator("#checkout-spare input")).toBeEnabled();
  await shot(page, `${name}-settings-checkouts`, window, { pad: 0 });
  const checkoutSave = settings.locator(".checkout-root-field .btn-primary");
  await expect(checkoutSave).not.toHaveAttribute("aria-disabled", "true");
  await keyboardFocusControl(page, settings, checkoutSave);
  await focusedShot(page, `${name}-primary-focused`, checkoutSave, settings);
  if (includePhone) await phoneSettingsShots(page, settings, name.replace(/-main-650$/, "-phone"));
  await page.keyboard.press("Escape");
  await expect(settings).toBeHidden();
}

/** Exercise Settings' actual phone breakpoint, which main-650 does not reach.
 * Long panes must scroll inside the pinned frame; Back returns native focus
 * to the selected list button, and Enter reopens its pane at the top. Both
 * palettes use real controls and the same owned fleet, without saving drafts.
 */
async function phoneSettingsShots(page: Page, settings: Locator, name: string) {
  const ordinaryViewport = page.viewportSize()!;
  try {
    await page.setViewportSize({ width: 390, height: 560 });
    const navigation = settings.getByRole("navigation", { name: "settings sections", exact: true });
    const back = settings.getByRole("button", { name: "‹ Settings", exact: true });
    const body = settings.locator(".host-settings-body");
    const close = settings.getByRole("button", { name: "Close", exact: true });
    for (const section of ["agent trust", "notifications"] as const) {
      const panel = await selectSettingsSection(page, section);
      const heading = panel.locator(":scope > h3");
      await expect(heading).toBeFocused();
      await expect(navigation).toBeHidden();
      await expect(back).toBeVisible();
      await expect.poll(() => body.evaluate(element => element.scrollTop)).toBe(0);
      expect(await body.evaluate(element => element.scrollHeight > element.clientHeight),
        "the real phone pane must overflow before its scroll and frame checks").toBe(true);
      const header = settings.locator(".host-settings-header");
      const footer = settings.locator(".host-settings-footer");
      for (const control of [header, footer, close, back]) await insideViewport(page, control);
      const frame = { header: await header.boundingBox(), footer: await footer.boundingBox() };
      const slug = section.replace(" ", "-");
      await shot(page, `${name}-settings-${slug}`, [page.locator("body")], { pad: 0 });
      await scrollToEnd(body);
      await expect.poll(() => body.evaluate(element => element.scrollTop)).toBeGreaterThan(0);
      const lastControl = section === "agent trust"
        ? panel.locator("#checkout-pi-trust input").first()
        : panel.locator(".ntfy-settings").getByRole("button", { name: "save", exact: true });
      await expect(lastControl).toBeInViewport();
      expect(await header.boundingBox(), "scrolling preserves the pinned Settings header").toEqual(frame.header);
      expect(await footer.boundingBox(), "scrolling preserves the pinned Settings footer").toEqual(frame.footer);
      await insideViewport(page, close);
      await shot(page, `${name}-settings-${slug}-end`, [page.locator("body")], { pad: 0 });
      await keyboardFocusControl(page, settings, back);
      await back.press("Enter");
      await expect(navigation).toBeVisible();
      await expect(settings.locator("section.app-settings-panel:visible")).toHaveCount(0);
      await expect(back).toBeHidden();
      const selected = navigation.locator('button[aria-pressed="true"]');
      await expect(selected).toBeFocused();
      expect(await selected.evaluate(element => element.matches(":focus-visible")), "Back preserves keyboard modality at its list target").toBe(true);
      await insideViewport(page, close);
      await focusedShot(page, `${name}-settings-${slug}-list-focused`, selected, settings);
      await selected.press("Enter");
      await expect(heading).toBeFocused();
      await expect(navigation).toBeHidden();
      await expect.poll(() => body.evaluate(element => element.scrollTop)).toBe(0);
    }
  } finally {
    await page.setViewportSize(ordinaryViewport);
  }
}

/** Open only the actual web Help menu, whose five items need no updater seam.
 * The count is the real web-build premise: update checks are native-only and
 * must not appear here through a capture provider or fabricated update state.
 */
async function openHelp(page: Page) {
  await page.locator(".app-help-toggle").click();
  const panel = page.locator('.bar-menu-flyout[data-bar-menu="help"] .bar-menu-panel');
  await expect(panel.getByRole("menu", { name: "help", exact: true }).getByRole("menuitem")).toHaveCount(5);
  await inspectMenu(page, panel);
  return panel;
}

/** Open the real empty form without sending an external feedback message.
 * Empty contact memory is a fresh-fleet premise, not a field rewrite. Send
 * remains disabled; the caller photographs and uses the ordinary Cancel path.
 */
async function openEmptyFeedback(page: Page) {
  const help = await openHelp(page);
  await help.locator('[data-bar-menu-item="send feedback"]').click();
  const dialog = page.locator(".feedback-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".feedback-version")).not.toBeEmpty();
  await expect(dialog.locator(".feedback-message")).toHaveValue("");
  await expect(dialog.locator(".feedback-contact")).toHaveValue("");
  await expect(dialog.locator(".feedback-send")).toBeDisabled();
  return dialog;
}

/** Require Cancel to preserve the exact remote this capture opened a question for.
 * API-context reads bypass the page's destination sanitization. Comparing the
 * identity and destination avoids accepting a replacement row with the same alias.
 */
async function sameConnectedHost(request: APIRequestContext, original: CaptureHost) {
  await expect.poll(async () => {
    const current = (await realHosts(request)).find((host) => host.id === original.id);
    return current?.identity === original.identity && current.destination === original.destination && current.state.phase === "connected";
  }, { timeout: 30_000, message: "the same owned remote and install remain connected after Cancel" }).toBe(true);
}

/** Use only the real catalog's longer effort list, without launching its harness.
 * The current release owns this OMP model and seven explicit efforts. Runtime
 * assertions must establish that the built helm agrees before driving the UI;
 * there is no fake-catalog fallback if that prerequisite changes.
 */
async function showOmpEffort(page: Page, request: APIRequestContext, launcher: Locator) {
  const response = await request.get("/api/launch-catalog");
  expect(response.ok(), "read the real release-owned launch catalog").toBe(true);
  const catalog = await response.json() as Array<{ id: string; harness: string; efforts: string[] }>;
  const model = catalog.find((entry) => entry.harness === "omp" && entry.id === "openai/gpt-6.1-sol");
  expect(model, "the real OMP model must exist before its picker is used").toBeDefined();
  expect(model!.efforts).toEqual(["off", "minimal", "low", "medium", "high", "xhigh", "max"]);
  await launcher.locator(".launch-composer-harness-choice button", { hasText: /^OMP$/ }).click();
  const picker = launcher.getByRole("combobox", { name: "model", exact: true });
  await picker.fill(model!.id);
  await launcher.getByRole("option", { name: model!.id, exact: true }).click();
  const effort = launcher.getByRole("combobox", { name: "effort", exact: true });
  await expect(effort).toHaveClass(/launch-composer-effort-select/);
  expect(await effort.locator("option").evaluateAll((options) => options.map((option) => (option as HTMLOptionElement).value)))
    .toEqual(["default", ...model!.efforts]);
  await expect(launcher.getByRole("group", { name: "effort", exact: true })).toHaveCount(0);
  await effort.selectOption("high");
  await expect(effort).toHaveValue("high");
  const permissions = launcher.getByRole("group", { name: "permission mode", exact: true });
  await expect(permissions.getByRole("button")).toHaveCount(4);
  await permissions.scrollIntoViewIfNeeded();
  await expect(effort).toBeInViewport();
}

/** Reach only uninstall's real live-session refusal, before installation probes.
 * The helm's uninstall_plan calls require_no_running_sessions before backend
 * probe/inspect. This owned waiting replay never ends by itself. Its exact
 * host/identity/live-state premise plus the precise 409 reply distinguish that
 * path from a successful plan, transport failure or installation inspection.
 * No confirmation body, Remove remedy or follow-up operation is ever sent.
 */
async function openLiveUninstallRefusal(page: Page, request: APIRequestContext, scenario: Scenario, original: CaptureHost) {
  const remote = scenario.hosts.find((candidate) => candidate.key === "build-box");
  const session = scenario.sessions.find((candidate) => candidate.host === remote?.key && candidate.status === "waiting");
  if (!remote?.ssh || !session) throw new Error("the refusal capture needs build-box's owned live waiting session");
  expect(original.destination).toBe(remote.ssh);
  await sameConnectedHost(request, original);
  const id = readFleet().ids.get(session.title);
  expect(id).toBeTruthy();
  const response = await request.get("/api/sessions");
  expect(response.ok()).toBe(true);
  const rows = (await response.json()).sessions as Array<{
    id: string; title: string; host: number; host_identity: string | null; stale: boolean; status: { state: string };
  }>;
  expect(rows.find((candidate) => candidate.id === id)).toMatchObject({
    title: session.title, host: original.id, host_identity: original.identity, stale: false, status: { state: "waiting" },
  });
  const host = hostRowByName(page, remote.alias);
  await expect(host).toHaveAttribute("data-host-phase", "connected");
  await openHostMenu(host);
  const uninstall = host.locator(".provisioning-uninstall");
  await expect(uninstall).not.toHaveAttribute("aria-disabled", "true");
  const [refused] = await Promise.all([
    page.waitForResponse((reply) => new URL(reply.url()).pathname === `/api/hosts/${original.id}/uninstall` &&
      reply.request().method() === "POST"),
    uninstall.click(),
  ]);
  expect(refused.request().postData(), "planning has no confirmation body").toBeNull();
  expect(refused.status(), "the live session refuses planning before probe/inspect").toBe(409);
  // The helm quotes its already Debug-quoted peer title. This fixture uses a
  // plain ASCII title, so keep that exact refusal rather than weakening the
  // premise to an arbitrary 409 response that could come from another guard.
  expect(session.title).toMatch(/^[a-z ]+$/);
  const reason = `uninstall is refused while sessions on this host have not ended or still have terminal tabs open: ""${session.title}"". Stop those sessions and close their tabs, then choose uninstall again`;
  expect(await refused.text()).toBe(reason);
  const dialog = page.locator(".host-refusal-dialog");
  await expect(dialog.locator(".host-refusal-reason")).toHaveText(reason);
  await expect(dialog.locator(".host-refusal-remove")).toHaveCount(0);
  await expect(page.locator(".host-uninstall-dialog")).toHaveCount(0);
  return dialog;
}

/** Capture a real failed connection without changing the staged session's status.
 * Retargeting keeps a learned install's cached sessions. Cleanup starts before
 * the first write because the route may commit a destination before reporting
 * a reconciliation failure; even that response must restore the original host.
 */
async function showUnavailable(page: Page, request: APIRequestContext, scenario: Scenario, session: ScenarioSession, name: string) {
  const remote = scenario.hosts.find((host) => host.key === session.host);
  if (remote?.kind !== "remote" || !remote.ssh) throw new Error("the stale review needs a staged remote session");
  const row = await showSession(page, session);
  await expect(row).toHaveAttribute("data-session-stale", "false");
  const host = (await realHosts(request)).find((candidate) => candidate.kind === "ssh" && candidate.destination === remote.ssh);
  if (!host?.identity || !host.destination) throw new Error("the staged remote must have a learned identity and destination");
  expect(host.state.phase).toBe("connected");
  const destinationUrl = `/api/hosts/${host.id}/destination`;
  const unavailable = "visual-review.invalid.";
  const card = page.locator(".host-stale-notice");
  try {
    const retarget = await request.post(destinationUrl, { data: { ssh: unavailable } });
    expect(retarget.ok(), "temporarily retarget the owned staged remote").toBe(true);
    // Connecting alone is already stale. Require this destination's actual
    // failed transport before photographing last-known status and its card.
    await expect.poll(async () => {
      const current = (await realHosts(request)).find((candidate) => candidate.id === host.id);
      return current?.destination === unavailable && current.identity === host.identity &&
        ["connecting", "unreachable-reprobing"].includes(current.state.phase) &&
        Boolean(current.state.last_error?.includes("visual-review.invalid"));
    }, { timeout: 30_000, message: "the owned remote records a failed connection to the invalid destination" }).toBe(true);
    await expect(row).toHaveAttribute("data-session-selected", "true");
    await expect(row).toHaveAttribute("data-session-stale", "true");
    await expect(row.locator(".stale-badge")).toHaveText("waiting for supervisor");
    await expect(row.locator(".status-badge.waiting")).toHaveCount(1);
    await expect(row.locator(".status-dot")).toHaveCSS("animation-name", "none");
    await expect(row.locator(".status-dot")).toHaveCSS("color", "rgb(255, 255, 255)");
    await expect(card).toBeVisible();
    await expect(card.locator(".session-unavailable-heading")).toHaveText("Session unavailable — waiting for supervisor");
    await expect(card.locator(".session-unavailable-detail")).toContainText(remote.alias);
    await expect(page.locator("#term-connecting")).toBeHidden();
    await shot(page, `${name}-selected-stale`, [page.locator("body")], { pad: 0 });
  } finally {
    const restore = await request.post(destinationUrl, { data: { ssh: host.destination } });
    expect(restore.ok(), "restore the owned remote's exact original destination").toBe(true);
    await expect.poll(async () => {
      const current = (await realHosts(request)).find((candidate) => candidate.id === host.id);
      return current?.destination === host.destination && current.identity === host.identity && current.state.phase === "connected";
    }, { timeout: 30_000, message: "the same staged host and install reconnect after restoration" }).toBe(true);
    await expect(row).toHaveAttribute("data-session-stale", "false");
    await expect(card).toBeHidden();
    await showSession(page, session);
  }
}

/** Photograph actual setup progress using the docs stack's existing stand-in.
 * Only its approved invented destination is registered. Restoring the control
 * file releases held work even when a screenshot fails; only the new host is
 * removed, after its ordinary busy guard lets go. Warning review releases the
 * same simulated action to failure, then holds a retry to show that starting
 * work does not acknowledge an unseen failure. No installation runs. */
async function showRailSetup(page: Page, request: APIRequestContext, scenario: Scenario, name: string, warningOnly = false) {
  const destination = scenario.add_host?.ssh;
  if (!destination) throw new Error("setup review needs the approved invented destination");
  const configPath = path.join(readStackInfo(DOCS_STACK_INFO_PATH).state, "provisioning-backend", "config.json");
  const original = await readFile(configPath, "utf8");
  const config = JSON.parse(original);
  const behavior = config.targets?.[`ssh:${destination}`];
  expect(behavior, "the docs backend must explicitly own this invented target").toBeDefined();
  expect((await realHosts(request)).some((host) => host.destination === destination)).toBe(false);
  const publish = async (contents: string) => {
    const temporary = `${configPath}.${randomUUID()}`;
    await writeFile(temporary, contents);
    await rename(temporary, configPath);
  };
  try {
    await publish(JSON.stringify({ ...config, targets: { ...config.targets, [`ssh:${destination}`]: { ...behavior, hold_actions: true } } }));
    const probe = await request.post("/api/hosts/probe", { data: { target: { kind: "ssh", destination } } });
    expect(probe.ok()).toBe(true);
    const plan = await probe.json();
    expect(plan.result).toBe("provisionable");
    const accepted = await request.post("/api/hosts/provision", { data: { probe_id: plan.probe_id } });
    expect(accepted.ok()).toBe(true);
    const { host_id: id } = await accepted.json();
    await expect.poll(async () => {
      const progress = await request.get(`/api/hosts/${id}/provisioning`);
      expect(progress.ok()).toBe(true);
      return (await progress.json()).status;
    }).toBe("running");
    const avatar = hostChoice(page, id);
    await expect(avatar).toHaveAttribute("data-host-progress", "true");
    await expect(avatar).toHaveAttribute("aria-label", /setting up/);
    if (warningOnly) {
      await publish(JSON.stringify({ ...config, targets: { ...config.targets, [`ssh:${destination}`]: {
        ...behavior, hold_actions: false, fail_action: "attach-supervisor", message: "Setup could not finish",
      } } }));
      await expect.poll(async () => {
        const progress = await request.get(`/api/hosts/${id}/provisioning`);
        expect(progress.ok()).toBe(true);
        return (await progress.json()).status;
      }).toBe("failed");
      await expect(avatar).toHaveAttribute("data-host-attention", "true");
      await expect(avatar).toHaveAttribute("data-host-progress", "false");
      await expect(avatar.locator(".host-rail-attention")).toBeVisible();
      await expect(page.locator(".hosts-panel")).toBeHidden();
      await expect(page.locator(".session-list")).toBeVisible();
      await shot(page, `${name}-rail-warning`, [page.locator("body")], { pad: 0 });
      await chooseHost(page, id);
      await expect(avatar).toHaveAttribute("aria-pressed", "true");
      await shot(page, `${name}-rail-warning-selected`, [page.locator("body")], { pad: 0 });

      // A later running action shares the real helm's retained progress path.
      // Keep Hosts closed so the warning's coexistence with its ring is real.
      await publish(JSON.stringify({ ...config, targets: { ...config.targets, [`ssh:${destination}`]: {
        ...behavior, hold_actions: true,
      } } }));
      const retryProbe = await request.post("/api/hosts/probe", { data: { target: { kind: "ssh", destination } } });
      expect(retryProbe.ok()).toBe(true);
      const retryPlan = await retryProbe.json();
      expect(retryPlan.result).toBe("provisionable");
      const retry = await request.post("/api/hosts/provision", { data: { probe_id: retryPlan.probe_id } });
      expect(retry.status()).toBe(202);
      expect((await retry.json()).host_id).toBe(id);
      await expect(avatar).toHaveAttribute("data-host-progress", "true");
      await expect(avatar).toHaveAttribute("data-host-attention", "true");
      await expect(avatar.locator(".host-rail-attention")).toBeVisible();
      await shot(page, `${name}-rail-warning-retry`, [page.locator("body")], { pad: 0 });
      await chooseHost(page);
      await expect(avatar).toHaveAttribute("aria-pressed", "false");
      await expect(avatar).toHaveAttribute("data-host-attention", "true");
      await shot(page, `${name}-rail-warning-retry-unselected`, [page.locator("body")], { pad: 0 });
      return;
    }
    await shot(page, `${name}-rail-setup`, [page.locator("body")], { pad: 0 });
    await showHostsPanel(page);
    await expect(page.locator(`[data-host-id="${id}"] .provisioning-run`)).toBeVisible();
    await page.mouse.click(0, 0);
    await shot(page, `${name}-hosts-panel-setup`, [page.locator("body")], { pad: 0 });
    await showSessionsPanel(page);
  } finally {
    await publish(original);
    const added = (await realHosts(request)).find((host) => host.destination === destination);
    if (added) {
      await expect.poll(async () => {
        const removed = await request.delete(`/api/hosts/${added.id}`);
        if (removed.ok()) return true;
        expect(removed.status()).toBe(409);
        expect(await removed.text()).toContain("busy with a setup, update or uninstall");
        return false;
      }, { timeout: 30_000, message: "the released setup lets go of its owned host" }).toBe(true);
      await expect(hostChoice(page, added.id)).toHaveCount(0);
    }
  }
}

for (const colorScheme of ["dark", "light"] as const) {
  for (const width of ["wide", "main-650"] as const) {
    /**
     * The main-pane measurement keeps the narrow case meaningful when a later
     * plan widens the sidebar for the host rail. Native controls and real menus
     * remain visible in a full-window capture; no review overlay hides defects.
     * Settings adds its phone breakpoint once per palette after main-650.
     */
    test(`${colorScheme} ${width}`, async ({ page, request }) => {
      await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
      await page.setViewportSize({ width: 1600, height: 1000 });
      const saved = await readPreferences(request);
      // These are precisely the fields seeded here or by openStagedFleet.
      // Null clears an originally absent value; {} would not restore absence.
      const restore = {
        template_shortcuts: saved.template_shortcuts ?? null,
        skip_host_setup_confirmation: saved.skip_host_setup_confirmation ?? null,
        skip_host_remove_confirmation: saved.skip_host_remove_confirmation ?? null,
        skip_session_confirmations: saved.skip_session_confirmations ?? null,
        name_session_on_quick_launch: saved.name_session_on_quick_launch ?? null,
        compact: saved.compact ?? null,
        list_sort: saved.list_sort ?? null,
        last_selected: saved.last_selected ?? null,
      };
      // Cleanup owns even a write whose response fails after committing. It
      // also covers failures in any early picture, before the pin leg exists.
      try {
        await patchPreferences(request, {
          template_shortcuts: { "1": "codex-deep", "8": "quick-claude" },
          skip_host_setup_confirmation: false,
          skip_host_remove_confirmation: false,
          skip_session_confirmations: false,
          name_session_on_quick_launch: true,
        });
        const seeded = await readPreferences(request);
        expect(seeded.skip_host_setup_confirmation).toBe(false);
        expect(seeded.skip_host_remove_confirmation).toBe(false);
        expect(seeded.skip_session_confirmations).toBe(false);
        expect(seeded.name_session_on_quick_launch).toBe(true);
        const { scenario } = await open(page, request, { checkoutTrash: true });
        const main = page.locator(".app-main");
        await expect(main).toBeVisible();
        await expect(page.locator("html")).toHaveAttribute("data-appearance", colorScheme);
        if (width === "main-650") {
          const box = await main.boundingBox();
          expect(box, "the open main pane establishes the sidebar's occupied width").not.toBeNull();
          await page.setViewportSize({ width: Math.round(box!.x + 650), height: 1000 });
          await expect.poll(async () => (await main.boundingBox())?.width, {
            message: "the constrained capture has a 650px main pane",
          }).toBeCloseTo(650, 0);
        }
        await page.evaluate(() => document.fonts.ready.then(() => undefined));
        await expect(page.locator(".session-details")).toBeVisible();
        await expect(page.locator(".build-skew")).toHaveCount(0);
        const name = `visual-review/${colorScheme}-${width}`;
        const window = [page.locator("body")];
        if (settingsOnly) {
          await settingsShots(page, name, width === "main-650");
          return;
        }
        if (menusOnly) {
          await menuShots(page, request, scenario, name);
          await hostHintShots(page, request, scenario, name);
          return;
        }
        if (dialogsOnly) {
          await dialogShots(page, request, scenario, name, width === "main-650");
          return;
        }
        if (launcherOnly) {
          await launcherShots(page, request, name, width === "main-650");
          return;
        }
        const row = rowOf(page, scenario, (session) => session.status === "idle" && session.seen === false);
        const hostName = row.locator(".session-row-meta .session-host");
        await expect(hostName).toHaveText("gpu-1");
        await expect(hostName).toHaveCSS("font-family", /^Inter/);
        await expect(row.locator(".session-host-separator")).toHaveText(":");
        await expect(row.locator(".session-cwd")).toHaveCSS("font-family", /JetBrains Mono/);
        // Short proportional names must join their colon as text, rather than
        // leaving the old mono layout's minimum-width cell between host and path.
        const hostGap = await hostName.evaluate((host) => {
          const colon = host.parentElement?.querySelector(".session-host-separator");
          if (!colon) throw new Error("the staged short-host row must have its path separator");
          const text = document.createRange();
          text.selectNodeContents(host);
          const gap = colon.getBoundingClientRect().left - text.getBoundingClientRect().right;
          text.detach();
          return gap;
        });
        expect(Math.abs(hostGap), "the short host and colon have no empty width floor").toBeLessThan(1);
        if (tunnelOnly) {
          // Matches the established window-chrome fixture. The native title
          // band itself remains a source-only review on this Linux web build.
          await page.locator(".app-shell").evaluate((element) => element.classList.add("macos-window"));
          await shot(page, `${name}-tunnel-zero`, window, { pad: 0 });
          await tunnelShots(page, name);
          return;
        }
        await shot(page, `${name}-main`, window, { pad: 0 });
        if (toolbarOnly) {
          // The fixed actions must fit the measured main pane. A nominal
          // viewport width alone would miss clipping beside a wider sidebar.
          const actions = page.locator(".titlebar-actions");
          await expect(actions.locator("button:visible")).toHaveCount(8);
          const pane = (await main.boundingBox())!;
          for (const button of await actions.locator("button:visible").all()) {
            const box = (await button.boundingBox())!;
            expect(box.x).toBeGreaterThanOrEqual(pane.x);
            expect(box.x + box.width).toBeLessThanOrEqual(pane.x + pane.width);
          }
          for (const action of ["Restart", "Replace"] as const) {
            const caret = page.getByRole("button", { name: `${action} options`, exact: true });
            await keyboardFocusControl(page, page.locator("body"), caret);
            await focusedShot(page, `${name}-${action.toLowerCase()}-caret-focused`, caret);
            await caret.press("ArrowDown");
            const menu = page.getByRole("menu", { name: action, exact: true });
            const panel = page.locator(".header-action-menu");
            await expect(menu).toBeVisible();
            await inspectMenu(page, panel);
            await menu.getByRole("menuitem").first().press("End");
            const withAction = menu.getByRole("menuitem", { name: `${action} with…`, exact: true });
            await expect(withAction).toBeFocused();
            await focusedShot(page, `${name}-${action.toLowerCase()}-menu-focused`, withAction);
            await withAction.press("Escape");
            await expect(menu).toBeHidden();
            await expect(caret).toBeFocused();
          }
          const restart = page.locator(".restart-primary");
          await expect(restart).toHaveAttribute("aria-disabled", "true");
          await keyboardFocusControl(page, page.locator("body"), restart);
          await focusedShot(page, `${name}-toolbar-unavailable-restart`, restart);
          const fleet = readFleet();
          expect(fleet.toolbarReview, "the ordinary stage must provide the genuine reporting fixture").toBeDefined();
          const reviewId = fleet.ids.get(fleet.toolbarReview!.title)!;
          await attachSession(page, reviewId);
          await waitForSessionRevealed(page, reviewId);
          if (fleet.toolbarReview!.transcript) {
            await waitForTermText(page, lastTranscriptLine(DOCS_SHOTS_DIR, fleet.toolbarReview!.transcript));
          }
          await expect(restart).toHaveAttribute("aria-disabled", "false");
          await keyboardFocusControl(page, page.locator("body"), restart);
          await focusedShot(page, `${name}-toolbar-available-restart`, restart);
          const caret = page.locator(".restart-menu-toggle");
          await keyboardFocusControl(page, page.locator("body"), caret);
          await caret.press("ArrowDown");
          const menu = page.getByRole("menu", { name: "Restart", exact: true });
          const plain = menu.getByRole("menuitem", { name: "Restart", exact: true });
          await expect(plain).toHaveAttribute("aria-disabled", "false");
          await expect(menu.getByRole("menuitem", { name: "Restart with…", exact: true })).toHaveAttribute("aria-disabled", "false");
          await expect(plain).toBeFocused();
          await focusedShot(page, `${name}-restart-menu-available-focused`, plain);
          await plain.press("Escape");
          const reviewRow = page.locator(`[data-session-id="${reviewId}"]`);
          await openRowMenu(reviewRow);
          const rowPanel = page.locator(".session-row-menu-flyout");
          await expect(rowPanel).toBeVisible();
          await shot(page, `${name}-row-replace-glyph`, [page.locator("body")], { pad: 0 });
          await page.keyboard.press("Escape");
          return;
        }
        if (sidebarWarningOnly) {
          await showRailSetup(page, request, scenario, name, true);
          return;
        }
        // The title menu and the supported width bounds are the changed
        // sidebar surface. Use its real resize control, not a CSS override.
        const sortTitle = page.locator(".sort-title");
        await sortTitle.click();
        const sortMenu = page.getByRole("menu", { name: "Sort", exact: true });
        await expect(sortMenu).toBeVisible();
        await expect(sortMenu.getByRole("menuitemradio")).toHaveCount(3);
        await expect(sortMenu.getByRole("menuitemcheckbox", { name: /^Compact/ })).toHaveAttribute("aria-checked", "false");
        await insideViewport(page, page.locator(".sort-menu-panel"));
        await shot(page, `${name}-sort-menu`, window, { pad: 0 });
        await sortMenu.getByRole("menuitemradio").first().press("Escape");
        await expect(sortMenu).toBeHidden();
        const resize = page.locator(".sidebar-resize-handle");
        await expect(resize).toHaveAttribute("aria-valuenow", "396");
        const widthShots = async (density = "") => {
          for (const [size, key, steps] of [["minimum", "ArrowLeft", 10], ["maximum", "ArrowRight", 36]] as const) {
            await resize.focus();
            for (let step = 0; step < steps; step++) await resize.press(key);
            await expect(resize).toHaveAttribute("aria-valuenow", size === "minimum" ? "296" : "656");
            if (width === "main-650") {
              const mainBox = (await main.boundingBox())!;
              await page.setViewportSize({ width: Math.round(mainBox.x + 650), height: 1000 });
              await expect.poll(async () => (await main.boundingBox())?.width).toBeCloseTo(650, 0);
            }
            await page.mouse.click(0, 0);
            await shot(page, `${name}${density}-sidebar-${size}`, window, { pad: 0 });
          }
          await resize.dblclick();
          await expect(resize).toHaveAttribute("aria-valuenow", "396");
          if (width === "main-650") {
            const mainBox = (await main.boundingBox())!;
            await page.setViewportSize({ width: Math.round(mainBox.x + 650), height: 1000 });
            await expect.poll(async () => (await main.boundingBox())?.width).toBeCloseTo(650, 0);
          }
        };
        await widthShots();
        if (sidebarStatusOnly) {
          // These are the staged supervisors' real statuses. Combining a
          // waiting query with the local host proves an honestly empty view.
          await chooseStatus(page, "Waiting");
          await expect(rowOf(page, scenario, (session) => session.status === "waiting")).toBeVisible();
          await expect(page.locator(".session-count")).toHaveText(/^\d+ of \d+$/);
          await page.mouse.click(0, 0);
          await shot(page, `${name}-status-waiting`, window, { pad: 0 });
          await widthShots("-status-waiting");
          const local = (await realHosts(request)).find((host) => host.kind === "local");
          expect(local, "the staged registry identifies its local host").toBeDefined();
          await chooseHost(page, local!.id);
          await expect(page.locator(".filter-empty")).toBeVisible();
          await expect(page.locator(".session-count")).toHaveText(/^0 of \d+$/);
          await page.mouse.click(0, 0);
          await shot(page, `${name}-status-empty`, window, { pad: 0 });
          await chooseHost(page);
          await chooseStatus(page, "All");
          await expect(row).toBeVisible();
        }
        if (sidebarRailOnly) {
          const remote = (await realHosts(request)).find((host) => host.kind === "ssh");
          expect(remote, "the real staged fleet has a remote avatar").toBeDefined();
          const avatar = hostChoice(page, remote!.id);
          await chooseHost(page, remote!.id);
          await expect(page.locator(".session-count")).toHaveText(/^\d+ of \d+$/);
          await shot(page, `${name}-rail-selected`, window, { pad: 0 });
          await avatar.focus();
          await expect(avatar).toBeFocused();
          await avatar.press("Shift+F10");
          const hostMenu = page.locator(`.host-row[data-host-id="${remote!.id}"] .host-row-menu-panel`);
          await inspectMenu(page, hostMenu);
          await shot(page, `${name}-rail-menu`, window, { pad: 0 });
          await page.keyboard.press("Escape");
          await expect(hostMenu).toBeHidden();
          await expect(avatar).toBeFocused();
          await chooseHost(page);
          await showHostsPanel(page);
          await page.mouse.click(0, 0);
          await shot(page, `${name}-hosts-panel`, window, { pad: 0 });
          await widthShots("-hosts-panel");
          const managementRow = page.locator(`.host-row[data-host-id="${remote!.id}"]`);
          await openHostMenu(managementRow);
          await inspectMenu(page, hostMenu);
          await shot(page, `${name}-hosts-panel-menu`, window, { pad: 0 });
          await page.keyboard.press("Escape");
          await expect(hostMenu).toBeHidden();
          await page.locator(".host-details-toggle").check();
          await expect(managementRow.locator(".host-detail")).toBeVisible();
          await page.mouse.click(0, 0);
          await shot(page, `${name}-hosts-panel-details`, window, { pad: 0 });
          await page.locator(".host-details-toggle").uncheck();
          await showSessionsPanel(page);
          const settings = await openSettings(page);
          await shot(page, `${name}-rail-settings`, window, { pad: 0 });
          await page.keyboard.press("Escape");
          await expect(settings).toBeHidden();
          await page.locator(".app-help-toggle").click();
          await expect(page.getByRole("menu", { name: "help", exact: true })).toBeVisible();
          await shot(page, `${name}-rail-help`, window, { pad: 0 });
          await page.keyboard.press("Escape");
          await expect(page.getByRole("menu", { name: "help", exact: true })).toBeHidden();
          // Escape returns keyboard focus to Help. Use the ordinary chrome
          // gap to dismiss that control's help before the unrelated setup shot.
          await page.mouse.click(0, 0);
          await showRailSetup(page, request, scenario, name);
          const waiting = scenario.sessions.find((session) => session.status === "waiting");
          expect(waiting, "the stale capture needs a real waiting session").toBeDefined();
          await showUnavailable(page, request, scenario, waiting!, name);
          // Force only the existing desktop layout class. This checks the
          // closest web geometry, not native traffic lights or AppKit input.
          await page.locator(".app-shell").evaluate((element) => element.classList.add("macos-window"));
          await expect(page.locator(".app-bar")).toHaveCSS("height", "48px");
          expect((await page.locator(".host-rail").boundingBox())!.y).toBeGreaterThanOrEqual(48);
          await shot(page, `${name}-rail-macos`, window, { pad: 0 });
          await page.locator(".app-shell").evaluate((element) => element.classList.remove("macos-window"));
        }
        if (sidebarHeaderOnly || sidebarStatusOnly || sidebarRailOnly) {
          // Seed through the same awaited API path as the full capture.
          // An optimistic menu write could otherwise race the final restore.
          await patchPreferences(request, { compact: true });
          await page.reload();
          const opened = scenario.sessions.find((session) => session.open);
          expect(opened, "the staged fleet names its open terminal").toBeDefined();
          await showSession(page, opened!);
          await expect(row.locator(".session-identity-copy")).toHaveClass(/\bcompact\b/);
          await expect(row.locator(".session-row-meta")).toHaveCount(0);
          await page.mouse.click(0, 0);
          await shot(page, `${name}-compact`, window, { pad: 0 });
          await sortTitle.click();
          const density = sortMenu.getByRole("menuitemcheckbox", { name: /^Compact/ });
          await expect(density).toHaveAttribute("aria-checked", "true");
          await shot(page, `${name}-compact-sort-menu`, window, { pad: 0 });
          await density.press("Escape");
          await expect(sortMenu).toBeHidden();
          await widthShots("-compact");
          return;
        }
        const unavailableRestart = page.locator(".restart-primary");
        await expect(unavailableRestart).toHaveAttribute("aria-disabled", "true");
        await keyboardFocusControl(page, page.locator("body"), unavailableRestart);
        await focusedShot(page, `${name}-unavailable-primary-focused`, unavailableRestart);
        await expect(unavailableRestart).toHaveAttribute("aria-disabled", "true");
        const selectedTab = page.locator(".tab.selected").first();
        await expect(selectedTab).toBeEnabled();
        await keyboardFocusControl(page, page.locator(".tab-strip"), selectedTab);
        await focusedShot(page, `${name}-selected-tab-focused`, selectedTab);

        await menuShots(page, request, scenario, name);

        await settingsShots(page, name, width === "main-650");

        const remote = scenario.hosts.find((host) => host.kind === "remote");
        if (!remote) throw new Error("the visual review needs a staged remote host");
        const host = hostRowByName(page, remote.alias);
        await openHostMenu(host);
        await inspectMenu(page, host.locator(".host-row-menu-panel"));
        await shot(page, `${name}-host-menu`, window, { pad: 0 });
        await host.locator(".host-settings").click();
        const hostSettings = page.locator(".host-settings-dialog");
        await expect(hostSettings).toBeVisible();
        await expect(hostSettings.locator(".host-links-without-asking-toggle")).toBeEnabled();
        await shot(page, `${name}-host-settings`, window, { pad: 0 });
        for (const kind of ["icon", "color"]) {
          const selected = hostSettings.locator(`.host-${kind}-choice[aria-pressed="true"]`);
          await expect(selected).toBeEnabled();
          await keyboardFocusControl(page, hostSettings, selected);
          await focusedShot(page, `${name}-host-${kind}-focused`, selected, hostSettings);
          await expect(selected).toHaveAttribute("aria-pressed", "true");
        }
        // The ordinary height historically hides the lower permissions. Keep
        // an explicit second image instead of assuming the new frame fits them.
        await scrollToEnd(hostSettings.locator(".host-settings-body"));
        await expect(hostSettings.locator(".host-local-links-without-asking-toggle")).toBeInViewport();
        await shot(page, `${name}-host-settings-end`, window, { pad: 0 });
        await page.keyboard.press("Escape");
        await expect(hostSettings).toBeHidden();

        const originalHost = await hostHintShots(page, request, scenario, name, true);
        const refusal = await openLiveUninstallRefusal(page, request, scenario, originalHost);
        await shot(page, `${name}-uninstall-refusal`, window, { pad: 0 });
        await refusal.locator(".host-refusal-close").click();
        await expect(refusal).toBeHidden();
        await sameConnectedHost(request, originalHost);

        await page.locator(".templates-button").click();
        const templates = page.locator(".templates-dialog");
        await expect(templates.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
        await templates.locator(".templates-row").filter({ has: page.getByText("codex-deep", { exact: true }) }).click();
        await expect(templates.locator(".templates-editor-heading h3")).toHaveText("edit template");
        await expect(templates.getByRole("button", { name: "number shortcut codex-deep", exact: true }))
          .toHaveCSS("font-family", /JetBrains Mono/);
        await shot(page, `${name}-template-editor`, window, { pad: 0 });
        const selectedTemplate = templates.locator('.templates-row[aria-pressed="true"]');
        await keyboardFocusControl(page, templates, selectedTemplate);
        await focusedShot(page, `${name}-selected-template-focused`, selectedTemplate);
        await page.keyboard.press("Escape");
        await expect(templates).toBeHidden();

        const help = await openHelp(page);
        await shot(page, `${name}-help-menu`, window, { pad: 0 });
        await page.keyboard.press("Escape");
        await expect(help).toBeHidden();
        const feedback = await openEmptyFeedback(page);
        await shot(page, `${name}-feedback`, window, { pad: 0 });
        await feedback.locator(".feedback-cancel").click();
        await expect(feedback).toBeHidden();
        await openHelp(page);
        await page.locator('[data-bar-menu="help"] [data-bar-menu-item="welcome"]').click();
        const welcome = page.locator(".welcome-dialog");
        await expect(welcome).toBeVisible();
        await shot(page, `${name}-welcome`, window, { pad: 0 });
        await welcome.locator(".welcome-dismiss").click();
        await expect(welcome).toBeHidden();

        // Reopen after resizing so placement and modal layout are measured from
        // their actual short window, not a stale anchor from the full-height UI.
        const ordinaryViewport = page.viewportSize()!;
        try {
          await page.setViewportSize({ ...ordinaryViewport, height: 520 });
          const shortFeedback = await openEmptyFeedback(page);
          for (const part of [".dialog-header", ".dialog-close", ".dialog-footer"]) {
            await insideViewport(page, shortFeedback.locator(part));
          }
          const body = shortFeedback.locator(".dialog-body");
          expect(await body.evaluate((element) => element.scrollHeight <= element.clientHeight + 1 ||
            /auto|scroll/.test(getComputedStyle(element).overflowY)), "overflow belongs to the existing feedback body").toBe(true);
          await shot(page, `${name}-feedback-short`, window, { pad: 0 });
          await shortFeedback.locator(".feedback-cancel").click();
          await expect(shortFeedback).toBeHidden();
        } finally {
          await page.setViewportSize(ordinaryViewport);
        }

        // These are the approved staged archives. Photograph the listing only;
        // deletion remains outside the review capture. The stale-state leg below
        // changes only the owned remote's destination and restores it afterward.
        await page.locator(".checkout-trash-button").click();
        const trash = page.locator(".checkout-trash-dialog");
        await expect(trash.locator(".checkout-trash-entry")).toHaveCount(3);
        for (const basename of await trash.locator(".checkout-trash-entry-heading strong").all()) {
          await expect(basename).toHaveCSS("font-family", /JetBrains Mono/);
        }
        // Entry deletion deliberately pauses after rows move. Photograph the
        // resting tier after that safety pause, without activating any delete.
        const deletes = trash.locator(".trash-entry-delete");
        await expect(deletes).toHaveCount(3);
        for (const button of await deletes.all()) {
          await expect(button).toBeEnabled();
          await expect(button).toHaveCSS("opacity", "1");
        }
        await shot(page, `${name}-trash`, window, { pad: 0 });
        await page.keyboard.press("Escape");
        await expect(trash).toBeHidden();

        const launcher = await openLauncher(page);
        await launcher.locator(".launch-composer-harness-choice button", { hasText: /^Codex$/ }).click();
        await expect(launcher.locator(".launch-composer-model-choice")).toBeVisible();
        await shot(page, `${name}-launcher`, window, { pad: 0 });
        // The footer is fixed while launch choices scroll. A top-only picture
        // misses lower controls and cannot establish that they remain readable.
        const launch = launcher.locator(".create-session-submit");
        await launch.scrollIntoViewIfNeeded();
        await launcher.evaluate((form) => {
          for (const region of [form, ...form.querySelectorAll<HTMLElement>("*")]) {
            if (region.scrollHeight > region.clientHeight && /auto|scroll/.test(getComputedStyle(region).overflowY)) {
              region.scrollTop = region.scrollHeight;
            }
          }
        });
        await shot(page, `${name}-launcher-end`, window, { pad: 0 });
        await showOmpEffort(page, request, launcher);
        await expect(launcher.getByRole("combobox", { name: "model", exact: true })).toHaveCSS("font-family", /JetBrains Mono/);
        await shot(page, `${name}-launcher-omp-effort`, window, { pad: 0 });
        await page.keyboard.press("Escape");
        await expect(launcher).toBeHidden();
        // New regains keyboard focus when the launcher closes. Press the
        // neutral corner only after dismissal, so no open panel is disturbed.
        await page.mouse.click(0, 0);

        // Every selected status needs its real shape against the solid fill.
        // Opening the idle session marks it read; the menu's manual unread
        // action then exercises the distinct diamond without rewriting status.
        const waiting = scenario.sessions.find((session) => session.status === "waiting");
        const idle = scenario.sessions.find((session) => session.status === "idle");
        const initial = scenario.sessions.find((session) => session.open);
        if (!waiting || !idle || !initial) throw new Error("the palette review needs waiting, idle and initially open sessions");
        const waitingRow = await showSession(page, waiting);
        await expect(waitingRow.locator(".status-badge.waiting")).toHaveCount(1);
        await expect(waitingRow.locator(".status-dot")).toHaveCSS("color", "rgb(255, 255, 255)");
        await shot(page, `${name}-selected-waiting`, window, { pad: 0 });
        // A real unanswered confirmation disables the selected open button.
        // Its identity must remain readable through that ancestor, too.
        await openRowMenu(waitingRow);
        await waitingRow.locator(".session-row-delete").click();
        await expect(waitingRow.locator(".confirm-consequence")).toBeVisible();
        await expect(waitingRow.locator(".session-row-open")).toBeDisabled();
        await expect(waitingRow.locator(".session-row-open")).toHaveCSS("opacity", "1");
        await clickDialogGap(page, waitingRow.locator(".session-row-menu-panel"));
        await shot(page, `${name}-selected-confirmation`, window, { pad: 0 });
        const deleteAnswer = waitingRow.locator(".confirm-delete");
        await expect(deleteAnswer).toBeEnabled();
        await keyboardFocusControl(page, waitingRow.locator(".session-row-menu-panel"), deleteAnswer);
        await focusedShot(page, `${name}-danger-focused`, deleteAnswer, waitingRow.locator(".session-row-menu-panel"));
        await waitingRow.locator(".confirm-cancel").click();
        await expect(waitingRow.locator(".confirm-consequence")).toBeHidden();
        await expect(waitingRow.locator(".session-row-open")).toBeEnabled();
        await waitingRow.locator(".session-row-replace").click();
        const replaceAnswer = waitingRow.locator(".confirm-replace");
        await expect(replaceAnswer).toBeEnabled();
        await keyboardFocusControl(page, waitingRow.locator(".session-row-menu-panel"), replaceAnswer);
        await focusedShot(page, `${name}-replace-confirmation-focused`, replaceAnswer, waitingRow.locator(".session-row-menu-panel"));
        await waitingRow.locator(".replace-cancel").click();
        await expect(replaceAnswer).toBeHidden();
        // Cancel removes its focused answer. Restore an ordinary menu owner
        // before Escape, matching the menu's supported keyboard handoff.
        const firstMenuItem = waitingRow.locator(".session-row-menu-panel [role='menuitem']").first();
        await expect(firstMenuItem).toBeVisible();
        await firstMenuItem.focus();
        await expect(firstMenuItem).toBeFocused();
        await page.keyboard.press("Escape");
        await expect(waitingRow.locator(".session-row-menu-panel")).toBeHidden();
        // Escape returns focus to the row-menu toggle and its help. The menu
        // is closed now, so a neutral press dismisses help before the main shot.
        await page.mouse.click(0, 0);
        await showUnavailable(page, request, scenario, waiting, name);
        const idleRow = await showSession(page, idle);
        await expect(idleRow.locator(".status-badge.idle:not(.unseen)")).toHaveCount(1);
        await expect(idleRow.locator(".status-dot")).toHaveCSS("color", "rgb(255, 255, 255)");
        await shot(page, `${name}-selected-seen`, window, { pad: 0 });
        await openRowMenu(idleRow);
        const unread = idleRow.getByRole("menuitem", { name: "mark unread", exact: true });
        await expect(unread).toBeVisible();
        await unread.click();
        await expect(idleRow.locator(".session-row-menu-panel")).toBeHidden();
        await expect(idleRow.locator(".status-badge.idle.unseen")).toHaveCount(1);
        await expect(idleRow.locator(".status-dot")).toHaveCSS("color", "rgb(255, 255, 255)");
        await shot(page, `${name}-selected-unseen`, window, { pad: 0 });
        await showSession(page, initial);

        // A real shared pin and compact preference exercise the mixed-font
        // baseline the ordinary rows cannot show. Restore both for other shots.
        const id = await row.getAttribute("data-session-id");
        expect(id, "the staged compact row has a real session identity").toBeTruthy();
        const pinUrl = `/api/sessions/${encodeURIComponent(id!)}/pin`;
        const wasPinned = await row.getAttribute("data-session-pinned");
        expect(wasPinned).toMatch(/^(true|false)$/);
        try {
          const pin = await request.put(pinUrl, { data: { pinned: true } });
          expect(pin.ok()).toBe(true);
          await patchPreferences(request, { compact: true });
          await page.reload();
          // Reload starts a new terminal attachment. A visible details bar is
          // not replay/reveal readiness; retain the ordinary fleet's live oracle.
          await showSession(page, initial);
          await expect(row.locator(".session-identity-copy")).toHaveClass(/\bcompact\b/);
          await expect(row).toHaveAttribute("data-session-pinned", "true");
          await expect(row.locator(".session-pin")).toBeVisible();
          await page.evaluate(() => document.fonts.ready.then(() => undefined));
          await expect(page.locator(".session-details")).toBeVisible();
          await expect(page.locator(".build-skew")).toHaveCount(0);
          await shot(page, `${name}-compact-pinned`, window, { pad: 0 });
          await page.locator(".sort-title").click();
          const density = page.getByRole("menuitemcheckbox", { name: /^Compact/ });
          await expect(density).toHaveAttribute("aria-checked", "true");
          await shot(page, `${name}-compact-sort-menu`, window, { pad: 0 });
          await density.press("Escape");
          await expect(page.locator(".sort-menu-panel")).toBeHidden();
        } finally {
          const unpin = await request.put(pinUrl, { data: { pinned: wasPinned === "true" } });
          expect(unpin.ok()).toBe(true);
          await expect(row).toHaveAttribute("data-session-pinned", wasPinned!);
        }
      } finally {
        await patchPreferences(request, restore);
        await expect.poll(async () => {
          const current = await readPreferences(request);
          return Object.fromEntries(Object.keys(restore).map((key) => [
            key, current[key as keyof typeof current] ?? null,
          ]));
        }, { message: "restore precisely the owned capture's preference fields, including absence" }).toEqual(restore);
      }
    });
  }
}
