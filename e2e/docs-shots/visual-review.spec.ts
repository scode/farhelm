/**
 * Review the real app across palettes and useful window widths on the docs fleet.
 * These unannotated pictures belong to development review, never to a docs page
 * or publication. Later Steel changes extend this set when they add a surface.
 */
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { hostRowByName, openHostMenu, openRowMenu, patchPreferences, readPreferences } from "../tests/helpers/fleet";
import { openSettings, selectSettingsSection } from "../tests/helpers/settings";
import { attachSession, waitForTermText } from "../tests/helpers/term";
import { waitForSessionRevealed } from "../tests/helpers/terminal-readiness";
import { lastTranscriptLine } from "../readme-hero/stage";
import type { Scenario, ScenarioSession } from "../readme-hero/scenario";
import { open, openLauncher, rowOf } from "./open";
import { DOCS_SHOTS_DIR } from "./paths";
import { shot } from "./shot";
import { readFleet } from "./stage-docs";

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

for (const colorScheme of ["dark", "light"] as const) {
  for (const width of ["wide", "main-650"] as const) {
    /**
     * The main-pane measurement keeps the narrow case meaningful when a later
     * plan widens the sidebar for the host rail. Native controls and real menus
     * remain visible in a full-window capture; no review overlay hides defects.
     */
    test(`${colorScheme} ${width}`, async ({ page, request }) => {
      await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
      await page.setViewportSize({ width: 1600, height: 1000 });
      const shortcuts = (await readPreferences(request)).template_shortcuts ?? {};
      await patchPreferences(request, { template_shortcuts: { "1": "codex-deep", "8": "quick-claude" } });
      const { scenario } = await open(page, request, { checkoutTrash: true });
      const main = page.locator(".app-main");
      await expect(main).toBeVisible();
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
      const name = `visual-review/${colorScheme}-${width}`;
      const window = [page.locator("body")];
      await shot(page, `${name}-main`, window, { pad: 0 });

      await openRowMenu(row);
      const menu = page.locator(".session-row-menu-panel");
      await expect(menu).toBeVisible();
      await shot(page, `${name}-session-menu`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(menu).toBeHidden();

      const settings = await openSettings(page);
      await shot(page, `${name}-settings`, window, { pad: 0 });
      // Directory and command fields must keep their mono role beside Inter
      // labels. The ordinary sessions panel alone does not exercise that split.
      await selectSettingsSection(page, "checkouts");
      // Unsaved examples reveal the fields' actual glyphs without changing
      // checkout configuration or running a command on the staged hosts.
      await settings.locator(".checkout-root-field input").fill("~/work/checkouts");
      await settings.locator(".checkout-post-clone-field textarea").fill("cargo fetch --locked");
      await shot(page, `${name}-settings-checkouts`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(settings).toBeHidden();

      const remote = scenario.hosts.find((host) => host.kind === "remote");
      if (!remote) throw new Error("the visual review needs a staged remote host");
      const host = hostRowByName(page, remote.alias);
      await openHostMenu(host);
      await expect(page.locator(".host-row-menu-panel")).toBeVisible();
      await shot(page, `${name}-host-menu`, window, { pad: 0 });
      await host.locator(".host-settings").click();
      const hostSettings = page.locator(".host-settings-dialog");
      await expect(hostSettings).toBeVisible();
      await shot(page, `${name}-host-settings`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(hostSettings).toBeHidden();

      await page.locator(".templates-button").click();
      const templates = page.locator(".templates-dialog");
      await expect(templates.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
      await templates.locator(".templates-row").filter({ has: page.getByText("codex-deep", { exact: true }) }).click();
      await expect(templates.locator(".templates-editor-heading h3")).toHaveText("edit template");
      await shot(page, `${name}-template-editor`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(templates).toBeHidden();

      // These are the approved staged archives. Photograph the listing only;
      // deletion remains outside the review capture. The stale-state leg below
      // changes only the owned remote's destination and restores it afterward.
      await page.locator(".checkout-trash-button").click();
      const trash = page.locator(".checkout-trash-dialog");
      await expect(trash.locator(".checkout-trash-entry")).toHaveCount(3);
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
      await page.keyboard.press("Escape");
      await expect(launcher).toBeHidden();

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
      await shot(page, `${name}-selected-confirmation`, window, { pad: 0 });
      await waitingRow.locator(".confirm-cancel").click();
      await expect(waitingRow.locator(".confirm-consequence")).toBeHidden();
      await expect(waitingRow.locator(".session-row-open")).toBeEnabled();
      // Cancel removes its focused control. Use the real toggle rather than
      // sending Escape without a menu focus owner after that handoff.
      await waitingRow.locator(".session-row-menu").click();
      await expect(waitingRow.locator(".session-row-menu-panel")).toBeHidden();
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
      const pin = await request.put(pinUrl, { data: { pinned: true } });
      expect(pin.ok()).toBe(true);
      try {
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
      } finally {
        const unpin = await request.put(pinUrl, { data: { pinned: false } });
        expect(unpin.ok()).toBe(true);
        await patchPreferences(request, { compact: false, template_shortcuts: shortcuts });
      }
    });
  }
}
