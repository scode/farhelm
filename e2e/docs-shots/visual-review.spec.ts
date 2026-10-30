/**
 * Review the real app across palettes and useful window widths on the docs fleet.
 * These unannotated pictures belong to development review, never to a docs page
 * or publication. Later Steel changes extend this set when they add a surface.
 * Setup uses only the approved invented host and its simulated backend;
 * Remove stops at Cancel, and uninstall at a live-session refusal before probes.
 * Native updater/App controls, loaded SSH-help commands, successful uninstall
 * plans remain uncaptured. Setup progress uses the existing owned backend's
 * held action, with the real helm publishing progress and controlled failures.
 * The focused host/settings selection also denies one authentic Spawn request
 * after photographing it; the requested child is never launched. Ordinary
 * session review reuses the same staged rows and real resumable header source.
 * Trash review reuses the approved archive listing. Footer review also holds
 * delete requests, then releases them into the existing staged refusal.
 * Browser sign-in review uses fresh credential-free contexts and submits only
 * an empty token, photographing local validation without an exchange request.
 * Terminal review hovers real replay bytes and temporarily attaches a second
 * owned view, restoring the first through its ordinary reclaim control.
 */
import { expect, test, type APIRequestContext, type Browser, type BrowserContext, type Locator, type Page, type Request as NetworkRequest, type Route } from "@playwright/test";
import { cleanupSession, hostRowByName, listHosts, listSessions, openHostMenu, openRowMenu, patchPreferences, readPreferences, setLocalYoloWithoutAsking, showHostsPanel, showSessionsPanel } from "../tests/helpers/fleet";
import { setLocalCommandsWithoutAsking } from "../tests/helpers/approvals";
import { cleanupOwnedSteps } from "../tests/helpers/cleanup";
import { submitPrompt } from "../tests/helpers/real-agent";
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
const sentenceLauncherOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sentence-launcher";
const sentenceHostsSettingsOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sentence-hosts-settings";
const sentenceApprovalOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sentence-approval";
const sentenceSessionsOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sentence-sessions";
const sentenceSessionHintsOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sentence-session-hints";
const sentenceHelpOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sentence-help";
const sentenceTrashOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sentence-trash";
const trashFooterOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "trash-footer";
const sentenceAuthOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sentence-auth";
const sentenceTerminalOnly = process.env.FARHELM_VISUAL_REVIEW_SCOPE === "sentence-terminal";

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

/** Photograph the real browser prompt and its local empty-token refusal.
 * The owned context explicitly starts without cookies, stored origins or
 * authorization headers; it never reads the capture's authenticated state.
 * Ordinary unauthorized app reads produce the prompt without routing replies.
 * No token is acquired or typed. The observer stays through cleanup so a late
 * exchange request cannot escape the proof that blank submission stayed local.
 * Auth has no sidebar: both widths measure the whole window, not a main pane.
 * Native bootstrap and successful sign-in are outside this capture.
 */
async function sentenceAuthShots(browser: Browser, baseURL: string, colorScheme: "dark" | "light", constrained: boolean) {
  const viewport = { width: constrained ? 650 : 1600, height: 1000 };
  const context = await browser.newContext({
    baseURL,
    storageState: { cookies: [], origins: [] },
    extraHTTPHeaders: {},
    colorScheme,
    reducedMotion: "reduce",
    viewport,
  });
  let exchanges = 0;
  const observe = (request: NetworkRequest) => {
    if (request.method() === "POST" && new URL(request.url()).pathname === "/api/auth/token") exchanges++;
  };
  context.on("request", observe);
  let bodyFailed = true;
  try {
    const page = await context.newPage();
    await page.goto("/");
    const auth = page.locator(".auth-page");
    const form = auth.locator("form.auth-card");
    const heading = form.getByRole("heading", { name: "Authenticate this device", exact: true });
    const input = form.getByLabel("Web token", { exact: true });
    const submit = form.getByRole("button", { name: "Continue", exact: true });
    const alert = form.getByRole("alert");
    await expect(auth).toBeVisible();
    await expect(auth.locator("form")).toHaveCount(1);
    await expect(form).toBeVisible();
    await expect(heading).toBeVisible();
    await expect(page.locator(".app-main, .host-rail")).toHaveCount(0);
    await expect(alert).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-appearance", colorScheme);
    await page.evaluate(() => document.fonts.ready.then(() => undefined));
    expect(page.viewportSize(), "auth widths describe the complete window").toEqual(viewport);
    await expect(input).toHaveAttribute("type", "password");
    await expect(input).toBeEnabled();
    await expect(input).toHaveValue("");
    // Observe the product's autofocus before sending any input. Focusing the
    // field ourselves would conceal a failed handoff from the replaced app.
    await expect(input).toBeFocused();
    await expect(submit).toBeEnabled();
    await expect(submit).toHaveAttribute("data-tooltip", "Continue: sign in with this token");
    for (const text of [heading, form.locator('label[for="farhelm-token"]'), input, submit]) {
      await expect(text).toHaveCSS("font-family", /^Inter/);
    }
    await expect(form.locator("code")).toHaveCSS("font-family", /JetBrains Mono/);
    /** Check the painted form and real hit targets before either picture.
     * Visibility alone would accept a control covered by another surface.
     */
    const reachable = async () => {
      await insideViewport(page, form);
      for (const control of [input, submit]) {
        await insideViewport(page, control);
        expect(await control.evaluate(element => {
          const box = element.getBoundingClientRect();
          return element.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
        }), "auth controls retain their actual center hit targets").toBe(true);
      }
    };
    await reachable();
    const name = `visual-review/${colorScheme}-${constrained ? "window-650" : "wide"}`;
    await shot(page, `${name}-auth-prompt`, [page.locator("body")], { pad: 0 });

    // Native Tab establishes the action's keyboard modality and actual help.
    // Enter submits the still-empty draft through the product's local guard.
    await input.press("Tab");
    await expect(submit).toBeFocused();
    expect(await submit.evaluate(element => element.matches(":focus-visible"))).toBe(true);
    await expect(input).toHaveValue("");
    await submit.press("Enter");
    await expect(alert).toHaveText("Enter the token printed by `farhelm helm token show`");
    await expect(input).toHaveValue("");
    await expect(input).toBeEnabled();
    await expect(submit).toBeEnabled();
    await expect(submit).toBeFocused();
    await reachable();
    await insideViewport(page, alert);
    expect(exchanges, "blank submission never reaches token exchange").toBe(0);
    await expect(page.locator(".farhelm-tooltip")).toHaveText("Continue: sign in with this token");
    await insideViewport(page, page.locator(".farhelm-tooltip"));
    await focusedShot(page, `${name}-auth-blank-token`, submit);
    bodyFailed = false;
  } finally {
    // Each page and the context get independent close attempts. Keep passive
    // observation until both finish, and detach even if either close fails.
    await cleanupOwnedSteps(bodyFailed, [
      ...context.pages().map((page): [string, () => Promise<void>] => ["close an owned auth page", () => page.close()]),
      ["close the owned auth context", () => context.close()],
      ["remove auth observer and verify no token exchange", () => {
        context.off("request", observe);
        expect(exchanges, "auth review sends no token exchange, including during cleanup").toBe(0);
      }],
    ]);
  }
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

/** Hover actual visible terminal cells after this session's replay is revealed.
 * Canvas text has no DOM target. The bounded observation keeps attachment and
 * visibility facts in a timeout instead of treating an absent path as readiness.
 * Nothing is printed, clicked or opened to manufacture the target.
 */
async function hoverTerminalReviewText(page: Page, id: string, needle: string) {
  await waitForSessionRevealed(page, id);
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  expect(await page.evaluate(() => document.fonts.check('13px "Inter"') && document.fonts.check('13px "JetBrains Mono"')),
    "both vendored faces must be ready before measuring terminal cells").toBe(true);
  await expect(page.locator("#terminal")).toBeVisible();
  await expect(page.locator("#term-banner")).toBeHidden();
  await page.mouse.move(0, 0);
  await expect(page.locator(".farhelm-tooltip")).toBeHidden();
  await expect(page.locator("#terminal .terminal-link-target:visible")).toHaveCount(0);
  let cell: { x: number; y: number } | null = null;
  await expect.poll(async () => {
    const observed = await page.evaluate(({ id, needle }) => {
      const island = (window as any).__farhelmIslands?.terminal;
      const term = island?.term;
      const screen = term?.element.querySelector(".xterm-screen");
      const box = screen?.getBoundingClientRect();
      let cell = null;
      if (box && box.width > 0 && box.height > 0) {
        for (let row = 0; row < term.rows; row++) {
          const line = term.buffer.active.getLine(term.buffer.active.viewportY + row)?.translateToString(true) || "";
          const column = line.indexOf(needle);
          if (column < 0) continue;
          const x = box.x + (column + needle.length / 2) * box.width / term.cols;
          const y = box.y + (row + 0.5) * box.height / term.rows;
          if (x >= 0 && x < innerWidth && y >= 0 && y < innerHeight) cell = { x, y };
          break;
        }
      }
      return {
        selected: document.querySelector('.session-row[data-session-selected="true"]')?.getAttribute("data-session-id") === id,
        socketOpen: island?.ws?.readyState === WebSocket.OPEN,
        replayRevealed: !!island?.test?.replay?.revealed,
        cell,
      };
    }, { id, needle });
    cell = observed.cell;
    return { ...observed, found: cell !== null };
  }, { timeout: 10_000, message: `the revealed live session must show a visible cell for ${needle}` })
    .toMatchObject({ selected: true, socketOpen: true, replayRevealed: true, found: true });
  expect(await page.evaluate(({ x, y }) => !!document.elementFromPoint(x, y)?.closest("#terminal"), cell!),
    "the actual terminal owns the hover hit target").toBe(true);
  await page.mouse.move(cell!.x, cell!.y);
}

/** Photograph a genuine second attachment displacing the first view.
 * Only this capture's in-memory browser state is copied. Ordinary open installs
 * the same approved privacy rewrites before attaching the default staged session.
 * Cleanup closes every owned page/context independently, then uses the real
 * reclaim control; no synthetic socket or product state is introduced.
 */
async function terminalDisplacementShot(page: Page, request: APIRequestContext, session: ScenarioSession, name: string, colorScheme: "dark" | "light") {
  const id = readFleet().ids.get(session.title);
  if (!id) throw new Error("terminal displacement needs its owned staged session");
  await waitForSessionRevealed(page, id);
  const browser = page.context().browser();
  const viewport = page.viewportSize();
  if (!browser || !viewport) throw new Error("terminal displacement needs the existing browser and explicit viewport");
  const socket = await page.evaluateHandle(() => (window as any).__farhelmIslands.terminal.ws);
  let second: BrowserContext | undefined;
  let bodyFailed = true;
  try {
    second = await browser.newContext({
      baseURL: new URL(page.url()).origin,
      viewport,
      colorScheme,
      reducedMotion: "reduce",
      deviceScaleFactor: await page.evaluate(() => devicePixelRatio),
      storageState: await page.context().storageState(),
    });
    const other = await second.newPage();
    // Browser device storage and the fixture API bearer are separate auth
    // channels. Reuse the already authenticated API fixture for ordinary open.
    await open(other, request, { terminalFiles: true });
    await waitForSessionRevealed(other, id);
    await expect(other.locator("html")).toHaveAttribute("data-appearance", colorScheme);
    expect(other.viewportSize()).toEqual(viewport);
    const banner = page.locator("#term-banner");
    const reclaim = banner.locator(".banner-reclaim");
    await expect(banner).toContainText(/^Detached:/);
    await expect(reclaim).toHaveText("Take control");
    await expect(reclaim).toBeEnabled();
    await expect.poll(() => socket.evaluate((socket) => socket.readyState), {
      timeout: 10_000, message: "the original attachment's socket closes after the second view takes control",
    }).toBe(3);
    await expect(page.locator("#term-connecting")).toBeHidden();
    await expect(page.locator(".terminal-reconnect-now")).toHaveCount(0);
    await expect(page.locator(`.session-row[data-session-id="${id}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator("#terminal")).toBeVisible();
    expect(await page.evaluate(() => !!(window as any).__farhelmIslands?.terminal?.test?.replay?.revealed),
      "the displaced view retains its genuinely revealed terminal").toBe(true);
    await page.evaluate(() => document.fonts.ready.then(() => undefined));
    await expect(reclaim).toHaveCSS("font-family", /^Inter/);
    await insideViewport(page, banner);
    await insideViewport(page, reclaim);
    // Bounds and enabled state do not rule out another surface covering the
    // action. The real button must also own its center before this picture.
    expect(await reclaim.evaluate(button => {
      const box = button.getBoundingClientRect();
      return document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2)?.closest("button") === button;
    }), "the real Take control button owns its center hit target").toBe(true);
    await waitForSessionRevealed(other, id);
    await shot(page, `${name}-terminal-take-control`, [page.locator("body")], { pad: 0 });
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ...(second?.pages() ?? []).map((other): [string, () => Promise<void>] => ["close an owned takeover page", () => other.close()]),
      ["close the owned takeover context", async () => { await second?.close(); }],
      ["restore the original terminal attachment", async () => {
        const reclaim = page.locator("#term-banner .banner-reclaim");
        if (await reclaim.isVisible()) {
          await expect(reclaim).toHaveText("Take control");
          await expect(reclaim).toBeEnabled();
          await insideViewport(page, reclaim);
          await reclaim.click();
        }
        await showSession(page, session);
        await expect(page.locator("#term-banner")).toBeHidden();
      }],
      ["release the original socket observation", () => socket.dispose()],
    ]);
  }
}

/** Keep terminal review to three genuine hovers and one ordinary takeover.
 * The added OSC transcript is docs-owned replay data; file checks use only the
 * existing adapter. Exact seen stamps and ordinary-open preferences restore
 * after selection, including failures, without taking ownership from the outer
 * capture. No download, external link, upload or session action is performed.
 */
async function sentenceTerminalShots(page: Page, request: APIRequestContext, scenario: Scenario, name: string, colorScheme: "dark" | "light") {
  const initial = scenario.sessions.find(session => session.open);
  const review = scenario.sessions.find(session => session.transcript === "terminal-review.txt");
  if (!initial || !review) throw new Error("terminal review needs the original open session and declared OSC replay");
  const initialId = readFleet().ids.get(initial.title);
  const reviewId = readFleet().ids.get(review.title);
  if (!initialId || !reviewId) throw new Error("both terminal review sessions must belong to this staged fleet");
  const ready = scenario.terminal_files?.find(file => file.path === "e2e/attach.spec.ts");
  expect(ready?.host, "the declared ready path belongs to the original transcript's host").toBe(initial.host);
  expect(ready?.size).toBe(6412);
  expect(scenario.terminal_files?.some(file => file.path === "e2e/helpers/stack.ts"),
    "the existing file adapter must supply its genuine missing-path case").toBe(false);
  const savedPreferences = await readPreferences(request);
  const restorePreferences = {
    compact: savedPreferences.compact ?? null,
    list_sort: savedPreferences.list_sort ?? null,
    last_selected: savedPreferences.last_selected ?? null,
  };
  const listing = await request.get("/api/sessions");
  expect(listing.ok(), "read actual seen stamps before selecting the review row").toBe(true);
  const rows = (await listing.json()).sessions as { id: string; seen_activity_at: number | null }[];
  const seen = [initialId, reviewId].map(id => {
    const row = rows.find(row => row.id === id);
    expect(row, "both review sources remain in the real listing").toBeDefined();
    expect(Object.hasOwn(row!, "seen_activity_at")).toBe(true);
    return { id, stamp: row!.seen_activity_at };
  });
  let bodyFailed = true;
  try {
    await chooseHost(page);
    await chooseStatus(page, "All");
    await expect(hostChoice(page)).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("radio", { name: "All", exact: true })).toBeChecked();
    await showSession(page, initial);
    // Both file states already occur in the original transcript. Only the
    // approved check adapter answers them; the download action is never used.
    const fileTarget = page.locator("#terminal .terminal-file-target");
    await hoverTerminalReviewText(page, initialId, "e2e/attach.spec.ts");
    await expect(fileTarget.locator(".terminal-file-name > span:not(.terminal-file-icon)")).toHaveText("attach.spec.ts");
    await expect(fileTarget.locator(".terminal-file-directory")).toHaveText(`this machine:${ready!.shown_path.slice(0, -"attach.spec.ts".length)}`);
    await expect(fileTarget.locator(".terminal-file-size")).toHaveText("6.4 KB · click to download");
    await expect(fileTarget.locator(".terminal-file-refusal")).toHaveCount(0);
    await expect(fileTarget.locator(".terminal-file-name")).toHaveCSS("font-family", /JetBrains Mono/);
    await insideViewport(page, fileTarget);
    await waitForSessionRevealed(page, initialId);
    await captureShot(page, `${name}-terminal-file-ready`, [page.locator("body")], { pad: 0, keepPointer: true });

    await hoverTerminalReviewText(page, initialId, "e2e/helpers/stack.ts");
    await expect(fileTarget.locator(".terminal-file-name > span:not(.terminal-file-icon)")).toHaveText("stack.ts");
    await expect(fileTarget.locator(".terminal-file-directory")).toHaveText(":e2e/helpers/");
    await expect(fileTarget.locator(".terminal-file-refusal")).toHaveText("Not found");
    await expect(fileTarget.locator(".terminal-file-size")).toHaveCount(0);
    await insideViewport(page, fileTarget);
    await waitForSessionRevealed(page, initialId);
    await captureShot(page, `${name}-terminal-file-not-found`, [page.locator("body")], { pad: 0, keepPointer: true });
    await page.mouse.move(0, 0);
    await expect(fileTarget).toBeHidden();

    // This is a real OSC span from declared replay bytes, rather than a bare
    // URL whose label and destination could not disagree.
    await showSession(page, review);
    await hoverTerminalReviewText(page, reviewId, "https://display.example/review");
    const linkTarget = page.locator("#terminal .terminal-link-target:not(.terminal-file-target)");
    await expect(linkTarget).toHaveClass(/\bterminal-link-target-mismatch\b/);
    const warning = linkTarget.locator(".terminal-link-target-warning");
    await expect(warning).toHaveText("Link text does not match where it goes");
    await expect(warning).toHaveCSS("text-transform", "none");
    await expect(linkTarget.locator(".terminal-link-target-text")).toHaveText("Text says: https://display.example/review");
    await expect(linkTarget).toContainText("Actually goes to:");
    await expect(linkTarget.locator(".terminal-link-target-url")).toHaveText("https://target.example/review");
    await expect(linkTarget.locator("strong")).toHaveText("target.example");
    await insideViewport(page, linkTarget);
    await waitForSessionRevealed(page, reviewId);
    await captureShot(page, `${name}-terminal-link-mismatch`, [page.locator("body")], { pad: 0, keepPointer: true });
    await page.mouse.move(0, 0);
    await expect(linkTarget).toBeHidden();

    await showSession(page, initial);
    await terminalDisplacementShot(page, request, initial, name, colorScheme);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["park the terminal review pointer", () => page.mouse.move(0, 0)],
      ["restore all hosts", () => chooseHost(page)],
      ["restore All status", () => chooseStatus(page, "All")],
      ["restore the original selected terminal", () => showSession(page, initial)],
      // Selection may mark output seen. Restore the saved stamps only after
      // our final attachment, including an originally null unread value.
      ...seen.map(({ id, stamp }): [string, () => Promise<void>] => ["restore an exact terminal review seen stamp", async () => {
        const response = await request.put(`/api/sessions/${encodeURIComponent(id)}/seen`, { data: { seen_activity_at: stamp } });
        expect(response.ok()).toBe(true);
        await expect.poll(async () => {
          const response = await request.get("/api/sessions");
          expect(response.ok()).toBe(true);
          return (await response.json()).sessions.find((row: { id: string }) => row.id === id)?.seen_activity_at;
        }, { message: "restore the exact pre-review seen stamp, including null" }).toBe(stamp);
      }]),
      ["restore the exact pre-helper ordinary-open preferences", async () => {
        await patchPreferences(request, restorePreferences);
        await expect.poll(async () => {
          const current = await readPreferences(request);
          return Object.fromEntries(Object.keys(restorePreferences).map(key => [key, current[key as keyof typeof current] ?? null]));
        }, { message: "restore only the preferences ordinary terminal review writes" }).toEqual(restorePreferences);
      }],
    ]);
  }
}

/** Show missing shortcut names by queuing references, never launching them.
 * Real preferences make both references unbound. The physical prefix owns
 * subsequent keys, and Escape retires each queue without submitting Enter.
 * Exact bindings restore independently of keyboard cleanup; no template or
 * session is created, and the request observer remains through restoration.
 */
async function unboundShortcutShots(page: Page, request: APIRequestContext, name: string) {
  const saved = await readPreferences(request);
  const bindings = {
    quick_launch_template: saved.quick_launch_template ?? null,
    template_shortcuts: saved.template_shortcuts ?? null,
  };
  let creates = 0;
  const observe = (request: NetworkRequest) => {
    if (request.method() === "POST" && new URL(request.url()).pathname === "/api/sessions") creates++;
  };
  page.on("request", observe);
  let bodyFailed = true;
  try {
    await patchPreferences(request, { quick_launch_template: null, template_shortcuts: {} });
    const cleared = await readPreferences(request);
    expect(cleared.quick_launch_template ?? null).toBeNull();
    expect(cleared.template_shortcuts ?? {}).toEqual({});
    await expect.poll(async () => JSON.parse(await page.locator(".launch-sequence-bindings").getAttribute("data-bindings") ?? "null"), {
      message: "the actual shortcut indicator observes both unbound preferences",
    }).toMatchObject({ quick_launch_template: null, template_shortcuts: {} });
    const begin = page.locator('.launch-sequence-trigger[data-sequence-action="begin"]');
    await expect(begin).toBeEnabled();
    const terminalInput = page.locator("#terminal .xterm-helper-textarea");
    await terminalInput.focus();
    await expect(terminalInput).toBeFocused();
    const indicator = page.locator(".launch-sequence-indicator");
    const mac = await page.evaluate(() => /Mac/.test(navigator.platform));
    for (const [key, label, suffix] of [
      ["Digit0", "Unbound key 0", "unbound-number"],
      [mac ? "Meta+KeyP" : "Control+Shift+KeyP", "Unbound ⚡", "unbound-quick-launch"],
    ]) {
      await page.keyboard.press("Control+Shift+KeyX");
      await expect(indicator).toBeVisible();
      await expect(indicator).toHaveAttribute("data-in-launcher", "false");
      await page.keyboard.press(key);
      await expect(indicator.locator(".launch-sequence-names")).toHaveText(label);
      await expect(indicator.locator(".launch-sequence-hint")).toHaveText("Type names or number keys · Tab complete · Enter launch · Esc cancel");
      await insideViewport(page, indicator);
      expect(await indicator.evaluate(element => element.scrollWidth <= element.clientWidth + 1),
        "the actual unbound sequence fits the measured main pane").toBe(true);
      await shot(page, `${name}-${suffix}`, [page.locator("body")], { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(indicator).toHaveCount(0);
    }
    expect(creates, "queuing and cancelling unbound references creates no session").toBe(0);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["cancel any remaining owned shortcut sequence", async () => {
        await page.keyboard.press("Escape");
        await expect(page.locator(".launch-sequence-indicator")).toHaveCount(0);
      }],
      ["restore exact shortcut bindings", async () => {
        await patchPreferences(request, bindings);
        const restored = await readPreferences(request);
        expect(restored.quick_launch_template ?? null).toEqual(bindings.quick_launch_template);
        expect(restored.template_shortcuts ?? null).toEqual(bindings.template_shortcuts);
      }],
      ["detach shortcut observer and verify no session creation", () => {
        page.off("request", observe);
        expect(creates, "no session create occurred, including during cleanup").toBe(0);
      }],
    ]);
  }
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
  await templates.getByRole("button", { name: "Number shortcut codex-deep", exact: true }).click();
  const panel = templates.locator(".templates-number-flyout .host-row-menu-panel");
  const menu = panel.getByRole("menu", { name: "Number key for codex-deep", exact: true });
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
 * Session-only review omits the independent template-number menu.
 */
async function menuShots(page: Page, request: APIRequestContext, scenario: Scenario, name: string, includeTemplateNumbers = true) {
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
  /** Settle opening focus before preparing an ordinary pointer-menu picture.
   * Rename can retain keyboard-visible focus after the preceding focus shot.
   * Hovering the read-state command moves focus through the real menu policy
   * and clears Rename's help without activating a command or hiding the DOM.
   */
  const openPointerMenu = async () => {
    await openRowMenu(row);
    await expect(panel.getByRole("menuitem").first()).toBeFocused();
    const readState = panel.getByRole("menuitem", { name: /^Mark (read|unread)$/ });
    await readState.hover();
    await expect(readState).toBeFocused();
  };
  // Cleanup can run after pointer activation. Re-establish menu-owned focus
  // before Escape so dismissal follows the same supported handoff as a user.
  const closeMenu = async () => {
    if (await panel.isVisible()) {
      await panel.getByRole("menuitem").first().focus();
      await page.keyboard.press("Escape");
      await expect(panel).toBeHidden();
    }
  };
  let bodyFailed = true;
  try {
    expect((await request.put(pinUrl, { data: { pinned: false } })).ok()).toBe(true);
    await expect(row).toHaveAttribute("data-session-pinned", "false");
    // Selecting establishes read state through the ordinary reveal path.
    await showSession(page, session);
    await openPointerMenu();
    await expect(panel.getByRole("menuitem")).toHaveCount(8);
    await expect(panel.locator(".menu-description")).toHaveCount(8);
    await expect(panel.getByRole("separator")).toHaveCount(4);
    await expect(panel.getByRole("menuitem", { name: "Mark unread", exact: true }))
      .toHaveAccessibleDescription("Show new output again");
    await expect(panel.getByRole("menuitem", { name: "Pin to top", exact: true }))
      .toHaveAccessibleDescription("Keep this session above unpinned ones");
    await inspectMenu(page, panel);
    await shot(page, `${name}-session-menu`, window, { pad: 0 });
    // The repair review needs this actual caller, not another full menu tour.
    // Returning inside finally still restores selection, pin and seen state.
    if (sentenceSessionHintsOnly) {
      await expect(panel.locator(".session-row-menu-summary")).toHaveText("claude · Idle");
      bodyFailed = false;
      return;
    }
    const items = panel.getByRole("menuitem");
    await items.first().press("ArrowDown");
    await expect(items.nth(1)).toBeFocused();
    await items.nth(1).press("Home");
    await focusedShot(page, `${name}-session-menu-focused`, items.first());
    await panel.getByRole("menuitem", { name: "Mark unread", exact: true }).click();
    await expect(panel).toBeHidden();
    await expect(row.locator(".status-badge.idle.unseen")).toHaveCount(1);
    await openPointerMenu();
    await expect(panel.getByRole("menuitem", { name: "Mark read", exact: true }))
      .toHaveAccessibleDescription("Treat the latest output as seen");
    await shot(page, `${name}-session-menu-mark-read`, window, { pad: 0 });
    await closeMenu();
    expect((await request.put(pinUrl, { data: { pinned: true } })).ok()).toBe(true);
    await expect(row).toHaveAttribute("data-session-pinned", "true");
    await openPointerMenu();
    await expect(panel.getByRole("menuitem", { name: "Unpin", exact: true }))
      .toHaveAccessibleDescription("Sort this session with the others again");
    await inspectMenu(page, panel);
    await shot(page, `${name}-session-menu-unpin`, window, { pad: 0 });
    await closeMenu();

    await page.setViewportSize({ ...viewport, height: 400 });
    await row.scrollIntoViewIfNeeded();
    await openPointerMenu();
    await expect(panel.getByRole("menuitem")).toHaveCount(8);
    expect(await panel.evaluate((element) => element.scrollHeight > element.clientHeight + 1),
      "the short-menu picture must exercise actual panel overflow").toBe(true);
    await inspectMenu(page, panel);
    await shot(page, `${name}-session-menu-short`, window, { pad: 0 });
    await items.first().press("End");
    const last = items.last();
    await expect(last).toHaveAccessibleName("Delete");
    await expect(last).toBeFocused();
    await expect(last).toBeInViewport();
    await focusedShot(page, `${name}-session-menu-short-last-focused`, last);
    await closeMenu();
    await page.setViewportSize(viewport);

    // Ordinary session review owns no template editor. Return inside the
    // existing restoration boundary so pin and seen stamps still restore.
    if (!includeTemplateNumbers) {
      bodyFailed = false;
      return;
    }
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
    bodyFailed = false;
  } finally {
    // Browser cleanup can fail while the request client still works. Attempt
    // both backend restorations independently, after selecting the original
    // terminal so its ordinary mark-on-open cannot overwrite the saved stamp.
    await cleanupOwnedSteps(bodyFailed, [
      ["close the owned session menu", closeMenu],
      ["restore exact menu viewport", () => page.setViewportSize(viewport)],
      ["restore original selected terminal", () => showSession(page, initial)],
      ["restore exact menu pin state", async () => {
        expect((await request.put(pinUrl, { data: { pinned: saved.pinned } })).ok()).toBe(true);
        await expect(row).toHaveAttribute("data-session-pinned", String(saved.pinned));
      }],
      ["restore exact menu seen stamp", async () => {
        expect((await request.put(`/api/sessions/${encodeURIComponent(id)}/seen`, {
          data: { seen_activity_at: saved.seen_activity_at },
        })).ok()).toBe(true);
        await expect.poll(async () => {
          const listing = await request.get("/api/sessions");
          expect(listing.ok()).toBe(true);
          return (await listing.json()).sessions.find((candidate: { id: string }) => candidate.id === id)?.seen_activity_at;
        }, { message: "restore the menu fixture's exact original seen stamp" }).toBe(saved.seen_activity_at);
      }],
    ]);
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
    await expect(remove.locator(".dialog-actions button")).toHaveText(["Cancel", "Remove, don't ask again", "Remove"]);
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
    await expect(add.locator(".dialog-actions button")).toHaveText(["Cancel", "Set up, don't ask again", "Set up"]);
    await expect(add.locator("#host-setup-permanent-hint")).toHaveText(hint);
    await insideViewport(page, add.locator("#host-setup-permanent-hint"));
    if (sentenceHostsSettingsOnly) await expect(add.locator(".provisioning-copy-plan")).toBeInViewport();
    await shot(page, `${name}-setup-plan`, window, { pad: 0 });
    if (sentenceHostsSettingsOnly) {
      const body = add.locator(".host-settings-body");
      if (await body.evaluate(element => element.scrollHeight > element.clientHeight + 1)) {
        await scrollToEnd(body);
        await expect(add.locator(".provisioning-plan-step").last()).toBeInViewport();
        await shot(page, `${name}-setup-plan-end`, window, { pad: 0 });
      }
    }
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
    await launcher.getByLabel("Folder", { exact: true }).fill("~");
    await launcher.getByLabel("Folder", { exact: true }).press("Escape");
    await expect(launcher.getByRole("listbox", { name: "Folders" })).toHaveCount(0);
    await launcher.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await launcher.locator(".named-conversation").fill("review-conversation");
    await launcher.locator(".initial-prompt").fill("Review the session launcher.\nKeep the action footer visible.\nExplain the result.");
    await expect(launcher.locator(".launch-composer-summary")).toContainText("Prompt: 3 lines");
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
    const folder = launcher.getByRole("combobox", { name: "Folder", exact: true });
    const folderOptions = launcher.getByRole("listbox", { name: "Folders", exact: true });
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
    const destination = launcher.getByRole("group", { name: "Destination type", exact: true });
    await destination.getByRole("button", { name: "Managed checkout", exact: true }).click();
    await expect(launcher.getByRole("combobox", { name: "Repository", exact: true })).toBeVisible();
    await capture("launcher-managed-checkout");
    await destination.getByRole("button", { name: "Folder", exact: true }).click();

    await showOmpEffort(page, request, launcher);
    await capture("launcher-omp-effort");
    await launcher.locator(".launch-composer-permissions-choice").getByRole("button", { name: "Guarded", exact: true }).click();
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
    await banner.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(banner).toBeHidden();

    await launcher.getByRole("tab", { name: "Command", exact: true }).click();
    await launcher.getByLabel("Agent command", { exact: true }).fill("claude {farhelm_args}");
    await launcher.getByRole("radio", { name: "No", exact: true }).check();
    await launcher.getByRole("combobox", { name: "Agent type", exact: true }).selectOption("claude");
    await launcher.locator(".launch-command-resume-toggle").check();
    await launcher.getByLabel("Resume command", { exact: true }).fill("claude --resume {conversation} {farhelm_args}");
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
      const folder = launcher.getByLabel("Folder", { exact: true });
      await folder.fill(longPath);
      await folder.press("Escape");
      await expect(launcher.getByRole("listbox", { name: "Folders" })).toHaveCount(0);
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

/** Show the editor's independent field labels and local refusals without saving.
 * The stored scenario template supplies the initial agent fields. Added fields
 * remain an ordinary unsaved draft, and every departure takes Discard. Command
 * fields use a separate new draft so incompatible agent fields do not obscure
 * their normal layout. The invalid wrapper's Save attempt must refuse locally,
 * with no template mutation request. No reply or catalog entry is rewritten.
 */
async function sentenceTemplateShots(page: Page, name: string) {
  const templates = page.locator(".templates-dialog");
  const window = [page.locator("body")];
  const mutations: string[] = [];
  const observe = (request: NetworkRequest) => {
    if (new URL(request.url()).pathname.startsWith("/api/templates") && request.method() !== "GET") {
      mutations.push(request.method());
    }
  };
  page.on("request", observe);
  const add = async (label: string) => {
    await templates.locator(".templates-add").click();
    const menu = templates.locator(".templates-add-menu");
    await expect(menu).toBeVisible();
    await menu.getByRole("button", { name: label, exact: true }).click();
    await expect(menu).toBeHidden();
  };
  const discard = async () => {
    await templates.locator(".templates-close").click();
    const answer = templates.locator(".templates-discard");
    await expect(answer).toBeVisible();
    await answer.click();
    await expect(templates).toBeHidden();
  };
  const fieldShot = async (key: string, suffix = key) => {
    const field = templates.locator(`.templates-set-field[data-field="${key}"]`);
    await field.scrollIntoViewIfNeeded();
    await templates.focus();
    await expect(templates).toBeFocused();
    await shot(page, `${name}-template-${suffix}`, window, { pad: 0 });
  };
  try {
    await page.locator(".templates-button").click();
    await expect(templates.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await templates.locator(".templates-row").filter({ has: page.getByText("codex-deep", { exact: true }) }).click();
    for (const label of ["Approvals", "Wrapper prefix", "Initial prompt", "Host", "Destination", "Session name"]) {
      await add(label);
    }
    await templates.locator('[data-field="permissions"]').getByRole("group", { name: "Approvals", exact: true })
      .getByRole("button", { name: "Custom", exact: true }).click();
    await templates.locator(".custom-permission-flags").fill("--permission-mode ask");
    await templates.locator(".templates-wrapper").fill("review-wrapper --");
    await templates.locator(".templates-prompt").fill("Review the labels.\nKeep user text unchanged.");
    const host = templates.locator('[data-field="host"]').getByRole("group", { name: "Host", exact: true });
    await expect(host.getByRole("button", { name: "build-box", exact: true })).toBeVisible();
    await host.getByRole("button", { name: "build-box", exact: true }).click();
    await templates.locator(".templates-destination").fill("~/work/review");
    await templates.locator(".templates-session-name").fill("review-labels");
    for (const key of ["agent", "model", "effort", "permissions", "wrapper", "prompt", "host", "destination", "name"]) {
      await fieldShot(key);
    }
    await templates.locator(".templates-wrapper").fill("review-wrapper 'unfinished");
    await expect(templates.locator(".templates-save")).toBeEnabled();
    await templates.locator(".templates-save").click();
    const refusal = templates.locator(".templates-error").filter({ hasText: "missing closing quote" });
    await expect(refusal).toBeVisible();
    expect(mutations, "an unfinished wrapper must refuse before any template write").toEqual([]);
    await refusal.scrollIntoViewIfNeeded();
    await templates.focus();
    await shot(page, `${name}-template-wrapper-refusal`, window, { pad: 0 });
    await discard();

    await page.locator(".templates-button").click();
    await expect(templates.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    await templates.locator(".templates-new").click();
    await templates.getByRole("group", { name: "Switches launcher to", exact: true })
      .getByRole("button", { name: "Command", exact: true }).click();
    for (const label of ["Command", "Approvals", "Resume command"]) await add(label);
    await templates.locator(".templates-command").fill("review-tool {farhelm_args}");
    await templates.locator(".templates-resume").fill("review-tool --resume {conversation} {farhelm_args}");
    await templates.locator('[data-field="yolo"]').getByRole("group", { name: "Approvals", exact: true })
      .getByRole("button", { name: "Keeps safety checks", exact: true }).click();
    for (const key of ["command", "yolo", "resume"]) await fieldShot(key, `command-${key}`);
    await discard();
  } finally {
    try {
      if (await templates.isVisible()) {
        await templates.locator(".templates-close").click();
        const answer = templates.locator(".templates-discard");
        if (await answer.isVisible()) await answer.click();
        await expect(templates).toBeHidden();
      }
    } finally {
      page.off("request", observe);
      expect(mutations, "review only edits drafts and photographs a local refusal").toEqual([]);
    }
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
  await launcher.getByRole("tab", { name: "Agent", exact: true }).click();
  await chooseLauncherHost(launcher.getByRole("combobox", { name: "Host", exact: true }), String(local!.id));
  await launcher.getByLabel("Folder", { exact: true }).fill("~");
  await launcher.getByLabel("Folder", { exact: true }).press("Escape");
  await expect(launcher.getByRole("listbox", { name: "Folders" })).toHaveCount(0);
  await launcher.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await launcher.locator(".launch-composer-permissions-choice").getByRole("button", { name: "YOLO", exact: true }).click();
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

/** Show the ordinary title editor and optionally a real refusal without saving.
 * Footer comparison needs only the untouched editor. Other review selections
 * share the supervisor validation boundary; cleanup cancels the owned dialog
 * and restores its viewport even when capture fails or comparison returns early.
 */
async function renameShots(page: Page, scenario: Scenario, name: string, includePhone = false, comparisonOnly = false) {
  const viewport = page.viewportSize()!;
  const window = [page.locator("body")];
  try {
    const row = rowOf(page, scenario, (session) => session.status === "idle" && session.seen === false);
    const id = await row.getAttribute("data-session-id");
    if (!id) throw new Error("Rename review needs its staged session identity");
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    const rename = page.locator(".rename-dialog");
    await expect(rename).toHaveAccessibleName("Rename session");
    await expect(rename.locator(".rename-input")).toHaveAccessibleName("New session title");
    await expect(rename.locator(".rename-input")).toBeFocused();
    await insideViewport(page, rename.locator(".dialog-header"));
    await insideViewport(page, rename.locator(".dialog-footer"));
    await shot(page, `${name}-rename`, window, { pad: 0 });
    if (comparisonOnly) return;
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
  } finally {
    try {
      const rename = page.locator(".rename-dialog");
      if (await rename.isVisible()) await rename.locator(".rename-cancel").click();
      await expect(rename).toBeHidden();
      const menu = rowOf(page, scenario, session => session.status === "idle" && session.seen === false)
        .locator(".session-row-menu-panel");
      if (await menu.isVisible()) {
        await menu.getByRole("menuitem").first().focus();
        await page.keyboard.press("Escape");
      }
      await expect(menu).toBeHidden();
    } finally {
      await page.setViewportSize(viewport);
    }
  }
}

/** Declare the target frame independently of the state-driving instrumentation.
 * Before captures replace only this helper with the old header expectations;
 * the submitted version must require the footer rather than accept either UI.
 */
async function expectTrashFrame(page: Page, trash: Locator, confirming = false) {
  const footer = trash.locator(".dialog-footer");
  await expect(footer).toHaveCount(1);
  await expect(footer.locator(".trash-close")).toHaveCount(1);
  await expect(footer.locator(".trash-delete-all")).toHaveCount(1);
  await expect(footer.locator(".checkout-trash-progress[role='status']")).toHaveCount(1);
  await expect(trash.locator(".dialog-header .dialog-close")).toHaveCount(1);
  await insideViewport(page, trash.locator(".dialog-header"));
  await insideViewport(page, footer);
  if (confirming) {
    const question = trash.locator(".checkout-trash-confirm");
    const region = trash.locator(":scope > .checkout-trash-global-confirmation");
    await expect(trash.locator(".checkout-trash-body .checkout-trash-confirm")).toHaveCount(0);
    await expect(region.locator(":scope > .checkout-trash-confirm")).toHaveCount(1);
    // The warning may exceed its scrollport; its unclipped content bounds
    // need not fit above the footer. The region must, and the caller probes
    // both answers' actual painted hit targets after the focus handoff.
    await insideViewport(page, region);
    const box = (await region.boundingBox())!;
    if (trashFooterOnly) {
      // These ordinary fixtures have three archives on two short-named hosts.
      // The safety text must be visible with the answers, rather than losing
      // all of its space to the archive list. Extreme long names are exercised
      // separately by the 360×300 keyboard-scroll tests.
      const warning = (await question.locator("p").boundingBox())!;
      expect(warning.y, "the ordinary question starts inside its scrollport").toBeGreaterThanOrEqual(box.y - 1);
      expect(warning.y + warning.height).toBeLessThanOrEqual(box.y + box.height + 1);
    }
    for (const answer of await question.locator("button").all()) {
      await insideViewport(page, answer);
      const button = (await answer.boundingBox())!;
      // A visible center alone can conceal a clipped edge. The complete
      // answer must fit its local scrollport, allowing only pixel rounding.
      expect(button.y).toBeGreaterThanOrEqual(box.y - 1);
      expect(button.y + button.height).toBeLessThanOrEqual(box.y + box.height + 1);
    }
    const actions = (await footer.boundingBox())!;
    expect(box.y + box.height, "the global confirmation scrollport meets its pinned footer").toBeCloseTo(actions.y, 0);
  }
}

/** Show the approved archives and their global question, without deleting.
 * The existing checkoutTrash listing rewrite supplies all three entries and
 * refuses deletion. Wait for the product's row-movement guard before the
 * listing; the question then owns main-answer focus and disables entry actions.
 * Sentence review takes only these two pictures. Dialog review also keeps its
 * phone listing. Failure cleanup independently attempts cancellation, closing
 * and viewport restoration, even if an earlier release fails.
 */
async function trashShots(page: Page, name: string, includePhone = false) {
  const viewport = page.viewportSize()!;
  const window = [page.locator("body")];
  const trash = page.locator(".checkout-trash-dialog");
  const question = trash.locator(".checkout-trash-confirm");
  const deletes = trash.locator(".trash-entry-delete");
  const deleteAll = trash.locator(".trash-delete-all");
  const close = trash.locator(".trash-close");
  // Painted controls must remain reachable without scrolling the pinned
  // frame or activating an answer merely to prove its hit target.
  const reachable = async (controls: Locator[]) => {
    for (const control of controls) {
      await insideViewport(page, control);
      expect(await control.evaluate(element => {
        const box = element.getBoundingClientRect();
        return element.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
      }), "the trash control's center remains its actual hit target").toBe(true);
    }
  };
  let bodyFailed = true;
  try {
    await page.locator(".checkout-trash-button").click();
    await expect(trash).toHaveAccessibleName("Trash");
    await expect(trash.locator(".checkout-trash-entry")).toHaveCount(3);
    await expect(trash.locator(".checkout-trash-host:has(.checkout-trash-entry)")).toHaveCount(2);
    await expect(deletes).toHaveCount(3);
    for (const button of await deletes.all()) {
      await expect(button).toBeEnabled();
      await expect(button).toHaveCSS("opacity", "1");
    }
    for (const basename of await trash.locator(".checkout-trash-entry-heading strong").all()) {
      await expect(basename).toHaveCSS("font-family", /JetBrains Mono/);
    }
    await expect(trash.locator(".dialog-title")).toHaveCSS("font-family", /^Inter/);
    await expect(deleteAll).toHaveText("Delete all");
    await expect(close).toHaveText("Close");
    for (const button of [deleteAll, close]) {
      await expect(button).toBeEnabled();
      await expect(button).toHaveCSS("font-family", /^Inter/);
    }
    await expect(close).toBeFocused();
    await expectTrashFrame(page, trash);
    await reachable([deleteAll, close]);
    await shot(page, `${name}-trash`, window, { pad: 0 });
    await deleteAll.click();
    await expect(question).toHaveAccessibleName("Confirm permanent checkout deletion");
    const submit = question.locator(".trash-confirm-submit");
    const cancel = question.locator(".trash-confirm-cancel");
    // Observe the product's initial handoff. Never activate this main answer,
    // even though the approved listing rewrite would refuse its request.
    await expect(submit).toBeFocused();
    await expect(submit).toHaveText("Delete permanently");
    await expect(cancel).toHaveText("Cancel");
    for (const button of [submit, cancel]) {
      await expect(button).toBeEnabled();
      await expect(button).toHaveCSS("font-family", /^Inter/);
    }
    for (const button of await deletes.all()) await expect(button).toBeDisabled();
    await expectTrashFrame(page, trash, true);
    await reachable([deleteAll, close, submit, cancel]);
    await shot(page, `${name}-trash-confirmation`, window, { pad: 0 });
    await cancel.click();
    await expect(question).toBeHidden();
    await expect(deleteAll).toBeFocused();
    if (includePhone) {
      await page.setViewportSize({ width: 390, height: 520 });
      // Resizing re-arms the same row-movement guard. The phone picture keeps
      // its original resting-control premise instead of bypassing that pause.
      for (const button of await deletes.all()) {
        await expect(button).toBeEnabled();
        await expect(button).toHaveCSS("opacity", "1");
      }
      await expectTrashFrame(page, trash);
      await reachable([deleteAll, close]);
      await shot(page, `${name}-trash-phone`, window, { pad: 0 });
    }
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["cancel the owned trash question", async () => {
        const cancel = question.locator(".trash-confirm-cancel");
        if (await cancel.isVisible()) await cancel.click();
        await expect(question).toBeHidden();
      }],
      ["close the owned trash dialog", async () => {
        // Close cancels an open band before it closes the dialog. Retain that
        // supported fallback if the separate Cancel step could not finish.
        if (await question.isVisible()) {
          await close.click();
          await expect(question).toBeHidden();
        }
        if (await trash.isVisible()) await close.click();
        await expect(trash).toBeHidden();
      }],
      ["restore the exact trash viewport", () => page.setViewportSize(viewport)],
    ]);
  }
}

/** Hold genuine renderer work, then refuse every staged ID without forwarding.
 * The scenario's existing two hosts supply one active request apiece and one
 * queued checkout. The release only falls through to installCheckoutTrash's
 * permanent refusal; a failure must never expose invented IDs to real deletion.
 */
async function trashPendingShot(page: Page, request: APIRequestContext, scenario: Scenario, name: string) {
  const trash = page.locator(".checkout-trash-dialog");
  const close = trash.locator(".trash-close");
  const status = trash.locator(".checkout-trash-progress");
  const hosts = await realHosts(request);
  const expected = new Map<number, string[]>();
  for (const archive of scenario.checkout_trash ?? []) {
    const declared = scenario.hosts.find(host => host.key === archive.host);
    expect(declared, "each staged archive declares its real fleet host").toBeDefined();
    const host = hosts.find(host => declared!.kind === "local" ? host.kind === "local" : host.destination === declared!.ssh);
    expect(host, "the scenario host exists in the owned staged fleet").toBeDefined();
    expected.set(host!.id, [...(expected.get(host!.id) ?? []), `docs-${archive.host}-${archive.checkout}`]);
  }
  expect(expected.size, "pending review needs archives on two genuine hosts").toBe(2);
  expect([...expected.values()].map(ids => ids.length).sort()).toEqual([1, 2]);

  let release!: () => void;
  const ready = new Promise<void>(resolve => { release = resolve; });
  const calls: { host: number; ids: string[]; expected_incarnation: number }[] = [];
  let outstanding = 0;
  const deletePath = (url: URL) => url.pathname === "/api/checkout-trash/delete";
  const holdDelete = async (route: Route) => {
    outstanding++;
    try {
      calls.push(route.request().postDataJSON());
      await ready;
      await route.fallback();
    } finally {
      outstanding--;
    }
  };
  let bodyFailed = true;
  await page.route(deletePath, holdDelete);
  try {
    await page.locator(".checkout-trash-button").click();
    await expect(trash).toHaveAccessibleName("Trash");
    await expect(trash.locator(".checkout-trash-entry")).toHaveCount(3);
    await expect(close).toBeFocused();
    for (const [host, ids] of expected) {
      await expect(trash.locator(`.checkout-trash-host[data-host-id="${host}"] .checkout-trash-entry`)).toHaveCount(ids.length);
    }
    for (const action of await trash.locator(".trash-entry-delete").all()) {
      await expect(action).toBeEnabled();
      await expect(action).toHaveCSS("opacity", "1");
    }
    await expect(status).toHaveAttribute("role", "status");
    await expect(status).toHaveText("");
    await expectTrashFrame(page, trash);
    await trash.locator(".trash-delete-all").click();
    const question = trash.locator(".checkout-trash-confirm");
    await expect(question).toHaveAccessibleName("Confirm permanent checkout deletion");
    await expect(question.locator(".trash-confirm-submit")).toBeFocused();
    await page.keyboard.press("Enter");
    await expect.poll(() => calls.map(call => call.host).sort((a, b) => a - b), {
      message: "both real staged hosts reach their first held delete response",
    }).toEqual([...expected.keys()].sort((a, b) => a - b));
    for (const call of calls) {
      expect(call.ids, "each host dispatches only one frozen archive at a time").toHaveLength(1);
      expect(expected.get(call.host), "only that host's scenario archives may be selected").toContain(call.ids[0]);
      expect(call.expected_incarnation, "pending work retains its connected incarnation").toBeGreaterThan(0);
    }
    await expect(question).toBeHidden();
    await expect(status).toHaveText("Deleting… 3 checkouts left");
    for (const [host, ids] of expected) {
      await expect(trash.locator(`.checkout-trash-host[data-host-id="${host}"] .trash-host-progress`)).toHaveText(`0 of ${ids.length} deleted`);
    }
    await expect(trash.locator(".trash-entry-progress .delete-spinner")).toHaveCount(2);
    await expect(trash.locator(".trash-entry-waiting")).toHaveText("Waiting");
    const closeControls = trash.locator(".trash-close, .dialog-header .dialog-close");
    for (const control of await closeControls.all()) {
      await expect(control).toBeDisabled();
      const explanation = control.locator("..");
      await expect(explanation).toHaveAttribute("data-tooltip", "Available when deletion finishes.");
      await insideViewport(page, explanation);
    }
    await expect(trash.locator(".trash-delete-all")).toBeDisabled();
    await insideViewport(page, status);
    await expectTrashFrame(page, trash);
    await trash.focus();
    await expect(trash).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(trash).toBeVisible();
    await expect(status).toHaveText("Deleting… 3 checkouts left");
    expect(calls, "neither held host starts its next checkout").toHaveLength(2);
    await shot(page, `${name}-trash-deleting`, [page.locator("body")], { pad: 0 });
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["release and drain only the owned delete hold", async () => {
        release();
        await expect.poll(() => outstanding, { message: "held callbacks finish through the staged refusal" }).toBe(0);
        await page.unroute(deletePath, holdDelete);
      }],
      ["observe refused cleanup while its fleet is alive", async () => {
        if (!await trash.isVisible()) return;
        await expect(status).toHaveText("");
        await expect(close).toBeEnabled();
        await expect(trash.locator(".checkout-trash-entry")).toHaveCount(3);
        await expect(page.locator(".build-skew")).toHaveCount(0);
      }],
      ["close the owned pending-review dialog", async () => {
        const cancel = trash.locator(".trash-confirm-cancel");
        if (await cancel.isVisible()) await cancel.click();
        if (await trash.isVisible()) await close.click();
        await expect(trash).toBeHidden();
      }],
    ]);
  }
}

/** Keep footer review to three Trash states and one unchanged dialog baseline.
 * These are whole-window dimensions, not the legacy main-pane measurement.
 * Each pending run ends in a refusal before the next size opens the same fleet.
 */
async function trashFooterShots(page: Page, request: APIRequestContext, scenario: Scenario, colorScheme: "dark" | "light") {
  const viewport = page.viewportSize()!;
  try {
    for (const [width, size] of [["wide", { width: 1600, height: 1000 }], ["phone", { width: 390, height: 520 }]] as const) {
      await page.setViewportSize(size);
      const name = `visual-review/${colorScheme}-${width}`;
      await renameShots(page, scenario, name, false, true);
      await trashShots(page, name);
      await trashPendingShot(page, request, scenario, name);
    }
  } finally {
    await page.setViewportSize(viewport);
  }
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
  await renameShots(page, scenario, name, includePhone);

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

  await trashShots(page, name, includePhone);
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
  await expect(settings.getByRole("checkbox", { name: "Skip session confirmations", exact: true })).not.toBeChecked();
  await shot(page, `${name}-settings`, window, { pad: 0 });
  const namingSwitch = settings.getByRole("checkbox", { name: "Name sessions on quick launch", exact: true });
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
  // Photograph both confirmation opt-outs and the background-update opt-in
  // without enabling them; API readback alone does not prove the mounted state.
  const hostChoices = await selectSettingsSection(page, "hosts");
  await expect(hostChoices.getByRole("checkbox")).toHaveCount(3);
  for (const name of ["Set up new hosts without asking", "Remove hosts without asking", "Update remote hosts automatically"]) {
    const choice = hostChoices.getByRole("checkbox", { name, exact: true });
    await expect(choice).toBeEnabled();
    await expect(choice).not.toBeChecked();
  }
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
  const sounds = notifications.getByRole("region", { name: "Sounds", exact: true });
  await expect.poll(() => page.evaluate(() => typeof (globalThis as typeof globalThis & {
    farhelmSounds?: { mountSettings?: unknown };
  }).farhelmSounds?.mountSettings), { message: "the real sound asset is available for its mounted controls" }).toBe("function");
  await expect(sounds.locator('[data-sound-event="waiting"]')).toBeChecked();
  await expect(sounds.locator('[data-sound-event="approval"]')).toBeChecked();
  await expect(sounds.locator('[data-sound-event="finished"]')).not.toBeChecked();
  const ntfy = notifications.locator(".ntfy-settings");
  await expect(ntfy.locator('input[type="url"]')).toHaveCSS("font-family", /JetBrains Mono/);
  await expect(ntfy.getByRole("checkbox", { name: "Send ntfy alerts", exact: true })).toBeEnabled();
  await expect(ntfy.getByRole("checkbox", { name: "Send ntfy alerts", exact: true })).not.toBeChecked();
  await sounds.scrollIntoViewIfNeeded();
  await shot(page, `${name}-settings-notifications`, window, { pad: 0 });
  await scrollToEnd(settings.locator(".host-settings-body"));
  await expect(ntfy.getByRole("button", { name: "Save", exact: true })).toBeInViewport();
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

/** Keep host settings evidence shared with the ordinary capture sequence.
 * Optional editor pictures use public unsaved drafts, then Cancel; the real
 * destination's private seed must never enter a screenshot. Icon/color focus
 * is observed without activating a choice or writing host preferences.
 */
async function hostSettingsShots(page: Page, scenario: Scenario, name: string, includeEditors = false) {
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote) throw new Error("the visual review needs a staged remote host");
  const host = hostRowByName(page, remote.alias);
  await openHostMenu(host);
  await inspectMenu(page, host.locator(".host-row-menu-panel"));
  await shot(page, `${name}-host-menu`, [page.locator("body")], { pad: 0 });
  await host.locator(".host-settings").click();
  const settings = page.locator(".host-settings-dialog");
  try {
    await expect(settings).toBeVisible();
    await expect(settings.locator(".host-links-without-asking-toggle")).toBeEnabled();
    await shot(page, `${name}-host-settings`, [page.locator("body")], { pad: 0 });
    for (const kind of ["icon", "color"]) {
      const selected = settings.locator(`.host-${kind}-choice[aria-pressed="true"]`);
      await expect(selected).toBeEnabled();
      await keyboardFocusControl(page, settings, selected);
      await focusedShot(page, `${name}-host-${kind}-focused`, selected, settings);
      await expect(selected).toHaveAttribute("aria-pressed", "true");
    }
    // The ordinary height hides lower permissions; the second image proves
    // those labels independently of the header and appearance controls.
    await scrollToEnd(settings.locator(".host-settings-body"));
    await expect(settings.locator(".host-local-links-without-asking-toggle")).toBeInViewport();
    await shot(page, `${name}-host-settings-end`, [page.locator("body")], { pad: 0 });
    if (includeEditors) {
      for (const [label, draft, slug] of [
        ["Edit alias", "review-box", "alias"],
        ["Edit destination", "dev@review-box", "destination"],
      ]) {
        await settings.getByRole("button", { name: label, exact: true }).click();
        const editor = settings.locator(".host-destination-form");
        await expect(editor).toBeVisible();
        // Editing seeds the actual registry value, bypassing the display
        // rewrite. Replace it before photographing, without ever saving it.
        await editor.locator(".host-destination-input").fill(draft);
        await expect(editor.locator(".host-save-destination")).toHaveText("Save");
        await expect(editor.locator(".host-cancel-edit")).toHaveText("Cancel");
        await shot(page, `${name}-host-${slug}-draft`, [page.locator("body")], { pad: 0 });
        await keyboardFocusControl(page, settings, editor.locator(".host-cancel-edit"));
        // Unlike the ordinary dialog focus pictures, retain this control's
        // real help so tooltip spelling also receives visual evidence.
        await focusedShot(page, `${name}-host-${slug}-cancel-focused`, editor.locator(".host-cancel-edit"));
        await editor.locator(".host-cancel-edit").click();
        await expect(editor).toBeHidden();
        await expect(settings.locator(`[data-setting="${slug}"] .host-settings-value`))
          .toHaveText(slug === "alias" ? remote.alias : remote.shown_ssh!);
      }
    }
  } finally {
    if (await settings.isVisible()) await page.keyboard.press("Escape");
    await expect(settings).toBeHidden();
  }
}

/** Photograph the ordinary host list and its real details without mutation.
 * Details is client disclosure state; restore its exact value so later setup
 * pictures do not inherit a conveniently expanded premise from this helper.
 */
async function hostPanelShots(page: Page, name: string) {
  await showHostsPanel(page);
  const details = page.locator(".host-details-toggle");
  const saved = await details.isChecked();
  try {
    await details.uncheck();
    await shot(page, `${name}-hosts-panel`, [page.locator("body")], { pad: 0 });
    await details.check();
    await expect(page.locator(".host-row[data-host-kind='ssh'] .host-detail").first()).toBeVisible();
    await shot(page, `${name}-hosts-panel-details`, [page.locator("body")], { pad: 0 });
  } finally {
    await details.setChecked(saved);
    await showSessionsPanel(page);
  }
}

/** Exercise Settings' actual phone breakpoint, which main-650 does not reach.
 * Long panes must scroll inside the pinned frame; Back returns native focus
 * to the selected list button, and Enter reopens its pane at the top. Both
 * palettes use real controls and the same owned fleet, without saving drafts.
 * The focused sentence-case selection visits every web section; only panes
 * whose measured body overflows get an end picture and last-control check.
 */
async function phoneSettingsShots(page: Page, settings: Locator, name: string) {
  const ordinaryViewport = page.viewportSize()!;
  try {
    await page.setViewportSize({ width: 390, height: 560 });
    const navigation = settings.getByRole("navigation", { name: "Settings sections", exact: true });
    const back = settings.getByRole("button", { name: "‹ Settings", exact: true });
    const body = settings.locator(".host-settings-body");
    const close = settings.getByRole("button", { name: "Close", exact: true });
    const sections = sentenceHostsSettingsOnly
      ? ["sessions", "Appearance", "terminal", "hosts", "checkouts", "agent trust", "notifications"] as const
      : ["agent trust", "notifications"] as const;
    for (const section of sections) {
      const panel = await selectSettingsSection(page, section);
      const heading = panel.locator(":scope > h3");
      await expect(heading).toBeFocused();
      await expect(navigation).toBeHidden();
      await expect(back).toBeVisible();
      await expect.poll(() => body.evaluate(element => element.scrollTop)).toBe(0);
      const overflows = await body.evaluate(element => element.scrollHeight > element.clientHeight + 1);
      if (section === "agent trust" || section === "notifications") {
        expect(overflows, "the long phone pane must overflow before its scroll and frame checks").toBe(true);
      }
      const header = settings.locator(".host-settings-header");
      const footer = settings.locator(".host-settings-footer");
      for (const control of [header, footer, close, back]) await insideViewport(page, control);
      const frame = { header: await header.boundingBox(), footer: await footer.boundingBox() };
      const slug = section.toLowerCase().replace(" ", "-");
      await shot(page, `${name}-settings-${slug}`, [page.locator("body")], { pad: 0 });
      if (overflows) {
        await scrollToEnd(body);
        await expect.poll(() => body.evaluate(element => element.scrollTop)).toBeGreaterThan(0);
        const lastControl = section === "agent trust"
          ? panel.locator("#checkout-pi-trust input").first()
          : section === "notifications"
          ? panel.locator(".ntfy-settings").getByRole("button", { name: "Save", exact: true })
          // Segmented controls clip their native input deliberately; the
          // enclosing label is the painted control whose reachability matters.
          : panel.locator("button, select, textarea, label:has(input)").last();
        await expect(lastControl).toBeInViewport();
        expect(await header.boundingBox(), "scrolling preserves the pinned Settings header").toEqual(frame.header);
        expect(await footer.boundingBox(), "scrolling preserves the pinned Settings footer").toEqual(frame.footer);
        await insideViewport(page, close);
        await shot(page, `${name}-settings-${slug}-end`, [page.locator("body")], { pad: 0 });
      }
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

/** Open only the actual web Help menu, whose six items need no updater seam.
 * The count is the real web-build premise: update checks are native-only and
 * must not appear here through a capture provider or fabricated update state.
 */
async function openHelp(page: Page) {
  await page.locator(".app-help-toggle").click();
  const panel = page.locator('.bar-menu-flyout[data-bar-menu="help"] .bar-menu-panel');
  await expect(panel.getByRole("menu", { name: "Help", exact: true }).getByRole("menuitem")).toHaveCount(6);
  await inspectMenu(page, panel);
  return panel;
}

/** Open the real empty form without sending an external feedback message.
 * Empty contact memory is a fresh-fleet premise, not a field rewrite. Send
 * remains disabled; the caller photographs and uses the ordinary Cancel path.
 */
async function openEmptyFeedback(page: Page) {
  const help = await openHelp(page);
  await help.locator('[data-bar-menu-item="Send feedback"]').click();
  const dialog = page.locator(".feedback-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".feedback-version")).not.toBeEmpty();
  await expect(dialog.locator(".feedback-message")).toHaveValue("");
  await expect(dialog.locator(".feedback-contact")).toHaveValue("");
  await expect(dialog.locator(".feedback-send")).toBeDisabled();
  return dialog;
}

/** Photograph Help's real local forms without sending or opening external pages.
 * Drafts belong only to their dialog instance. The short form establishes real
 * overflow before testing its pinned chrome; cleanup closes every owned surface,
 * restores the caller's viewport and observes requests through those close paths.
 * Shared preferences remain the enclosing capture case's responsibility.
 */
async function sentenceHelpShots(page: Page, name: string) {
  const viewport = page.viewportSize()!;
  const window = [page.locator("body")];
  const feedback = page.locator(".feedback-dialog");
  const welcome = page.locator(".welcome-dialog");
  const help = page.locator('.bar-menu-flyout[data-bar-menu="help"] .bar-menu-panel');
  let feedbackRequests = 0;
  const observe = (request: NetworkRequest) => {
    if (new URL(request.url()).pathname === "/api/feedback") feedbackRequests++;
  };
  page.on("request", observe);
  let bodyFailed = true;
  try {
    await openHelp(page);
    await shot(page, `${name}-help-menu`, window, { pad: 0 });
    await page.keyboard.press("Escape");
    await expect(help).toBeHidden();

    await openEmptyFeedback(page);
    await shot(page, `${name}-feedback`, window, { pad: 0 });
    await feedback.locator(".feedback-cancel").click();
    await expect(feedback).toBeHidden();

    await openEmptyFeedback(page);
    const message = feedback.locator(".feedback-message");
    const overlong = "x".repeat(4001);
    await message.fill(overlong);
    // Establish the input premise before relying on its local validation. No
    // submission or intercepted response is needed to mount this actual alert.
    await expect(message).toHaveValue(overlong);
    const alert = feedback.getByRole("alert");
    await expect(alert).toHaveText("The message is longer than 4000 characters");
    await expect(feedback.locator(".feedback-send")).toBeDisabled();
    await alert.scrollIntoViewIfNeeded();
    await expect(alert).toBeInViewport();
    expect(feedbackRequests, "local validation has sent no feedback request").toBe(0);
    await shot(page, `${name}-feedback-invalid`, window, { pad: 0 });
    await feedback.locator(".feedback-cancel").click();
    await expect(feedback).toBeHidden();

    await openHelp(page);
    await help.locator('[data-bar-menu-item="Welcome"]').click();
    await expect(welcome).toBeVisible();
    await shot(page, `${name}-welcome`, window, { pad: 0 });
    await welcome.locator(".welcome-dismiss").click();
    await expect(welcome).toBeHidden();

    // Keep the case's width, including its measured 650px main pane. Reopen at
    // the short height so the actual modal owner lays out its initial frame.
    await page.setViewportSize({ ...viewport, height: 320 });
    await openEmptyFeedback(page);
    await feedback.locator(".feedback-message").fill("Feedback in a short window.");
    await feedback.locator(".feedback-contact").fill("contact@example.test");
    const reuse = feedback.getByRole("checkbox", { name: "Re-use for future feedback", exact: true });
    await expect(reuse).toBeChecked();
    await expect(feedback.locator(".feedback-send")).toBeEnabled();
    const body = feedback.locator(".dialog-body");
    await expect.poll(() => body.evaluate(element => element.scrollHeight > element.clientHeight), {
      message: "the short form must actually overflow before testing its scroll contract",
    }).toBe(true);
    await expect(body).toHaveCSS("overflow-y", /^(auto|scroll)$/);
    // The end view can leave this conditional label above the scrollport.
    // Photograph it explicitly rather than treating a mounted node as pixels.
    await reuse.scrollIntoViewIfNeeded();
    await expect(reuse).toBeInViewport();
    await shot(page, `${name}-feedback-short-fields`, window, { pad: 0 });
    await body.evaluate(element => { element.scrollTop = 0; });
    await expect.poll(() => body.evaluate(element => element.scrollTop)).toBe(0);
    const header = feedback.locator(".dialog-header");
    const footer = feedback.locator(".dialog-footer");
    const frame = { header: await header.boundingBox(), footer: await footer.boundingBox() };
    await body.evaluate(element => { element.scrollTop = element.scrollHeight; });
    await expect.poll(() => body.evaluate(element => element.scrollTop), {
      message: "the owned feedback body scrolls while its header and footer remain pinned",
    }).toBeGreaterThan(0);
    expect(await header.boundingBox()).toEqual(frame.header);
    expect(await footer.boundingBox()).toEqual(frame.footer);
    for (const part of [header, footer, feedback.locator(".feedback-dismiss"), feedback.locator(".feedback-cancel"), feedback.locator(".feedback-send")]) {
      await insideViewport(page, part);
    }
    for (const selector of [".feedback-dismiss", ".feedback-cancel", ".feedback-send"]) {
      expect(await feedback.locator(selector).evaluate(element => {
        const box = element.getBoundingClientRect();
        return element.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
      }), "the pinned feedback controls remain reachable after body scrolling").toBe(true);
    }
    await shot(page, `${name}-feedback-short`, window, { pad: 0 });
    await feedback.locator(".feedback-cancel").click();
    await expect(feedback).toBeHidden();
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["close owned feedback", async () => {
        if (await feedback.isVisible()) await feedback.locator(".feedback-cancel").click();
        await expect(feedback).toBeHidden();
      }],
      ["close owned Welcome", async () => {
        if (await welcome.isVisible()) await welcome.locator(".welcome-dismiss").click();
        await expect(welcome).toBeHidden();
      }],
      ["close owned Help menu", async () => {
        if (await help.isVisible()) await page.keyboard.press("Escape");
        await expect(help).toBeHidden();
      }],
      ["restore exact viewport", () => page.setViewportSize(viewport)],
      ["remove observer and verify no feedback request", () => {
        page.off("request", observe);
        expect(feedbackRequests, "none of these local forms sends feedback").toBe(0);
      }],
    ]);
  }
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
  const picker = launcher.getByRole("combobox", { name: "Model", exact: true });
  await picker.fill(model!.id);
  await launcher.getByRole("option", { name: model!.id, exact: true }).click();
  const effort = launcher.getByRole("combobox", { name: "Effort", exact: true });
  await expect(effort).toHaveClass(/launch-composer-effort-select/);
  expect(await effort.locator("option").evaluateAll((options) => options.map((option) => (option as HTMLOptionElement).value)))
    .toEqual(["default", ...model!.efforts]);
  await expect(launcher.getByRole("group", { name: "Effort", exact: true })).toHaveCount(0);
  await effort.selectOption("high");
  await expect(effort).toHaveValue("high");
  const permissions = launcher.getByRole("group", { name: "Permission mode", exact: true });
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
    await expect(row.locator(".stale-badge")).toHaveText("Waiting for supervisor");
    await expect(row.locator(".status-badge.waiting")).toHaveCount(1);
    await expect(row.locator(".status-dot")).toHaveCSS("animation-name", "none");
    await expect(row.locator(".status-dot")).toHaveCSS("color", "rgb(255, 255, 255)");
    await expect(card).toBeVisible();
    await expect(card.locator(".session-unavailable-heading")).toHaveText("Session unavailable — waiting for supervisor");
    await expect(card.locator(".session-unavailable-detail")).toContainText(remote.alias);
    await expect(page.locator("#term-connecting")).toBeHidden();
    await shot(page, `${name}-selected-stale`, [page.locator("body")], { pad: 0 });
    if (sentenceHostsSettingsOnly) {
      await showHostsPanel(page);
      const details = page.locator(".host-details-toggle");
      const saved = await details.isChecked();
      const unavailableRow = page.locator(`.host-row[data-host-id="${host.id}"]`);
      try {
        await shot(page, `${name}-unavailable-host`, [page.locator("body")], { pad: 0 });
        await details.check();
        await expect(unavailableRow.locator(".host-detail")).toBeVisible();
        // Raw transport diagnostics keep the same privacy gate as every
        // other shot; there is no new error-text substitution for this state.
        await shot(page, `${name}-unavailable-host-details`, [page.locator("body")], { pad: 0 });
        // The ordinary menu already pictures these authored controls. Its
        // disconnected header substitutes the peer's executable path for its
        // version in the same slot. This real remote's private path is omitted.
      } finally {
        await details.setChecked(saved);
        await showSessionsPanel(page);
      }
    }
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

/** Show retained setup steps while their owned run still exists.
 * Details expands the production presentation. A failed run's menu is only
 * observed, never activated; opening Hosts also genuinely acknowledges its
 * rail warning. Restore disclosure before returning to the original lifecycle.
 */
async function setupPanelShots(page: Page, id: number, name: string, failed = false) {
  await showHostsPanel(page);
  const details = page.locator(".host-details-toggle");
  const saved = await details.isChecked();
  const host = page.locator(`.host-row[data-host-id="${id}"]`);
  const run = host.locator(".provisioning-run");
  try {
    await details.check();
    await expect(run).toHaveAttribute("data-provisioning-status", failed ? "failed" : "running");
    await expect(run.locator(".provisioning-step").first()).toBeVisible();
    await run.locator(".provisioning-run-header").scrollIntoViewIfNeeded();
    await shot(page, `${name}-setup-${failed ? "failed" : "running"}-steps`, [page.locator("body")], { pad: 0 });
    const last = run.locator(".provisioning-step").last();
    await last.scrollIntoViewIfNeeded();
    await expect(last).toBeInViewport();
    await shot(page, `${name}-setup-${failed ? "failed" : "running"}-steps-end`, [page.locator("body")], { pad: 0 });
    if (failed) {
      await expect(run.locator(".provisioning-run-message")).toContainText("Setup could not finish");
      await openHostMenu(host);
      const menu = host.locator(".host-row-menu-panel");
      await expect(menu.locator(".provisioning-rerun .session-row-menu-label")).toHaveText("Re-run");
      await expect(menu.locator(".provisioning-rerun .session-row-menu-description")).toHaveText("Try the failed setup again");
      await inspectMenu(page, menu);
      await shot(page, `${name}-setup-failed-menu`, [page.locator("body")], { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(menu).toBeHidden();
    }
  } finally {
    await details.setChecked(saved);
    await showSessionsPanel(page);
  }
}

/** Photograph actual setup progress using the docs stack's existing stand-in.
 * Only its approved invented destination is registered. Restoring the control
 * file releases held work even when a screenshot fails; only the new host is
 * removed, after its ordinary busy guard lets go. Warning review releases the
 * same simulated action to failure, then holds a retry to show that starting
 * work does not acknowledge an unseen failure. The sentence-case selection
 * opens the real failure in Hosts before retry, so that failure is acknowledged.
 * No installation runs. */
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

      if (sentenceHostsSettingsOnly) {
        await setupPanelShots(page, id, name, true);
        await expect(avatar).toHaveAttribute("data-host-attention", "false");
      }

      // A later running action shares the real helm's retained progress path.
      // Ordinary warning review keeps Hosts closed. The focused sentence-case
      // path has already photographed and acknowledged this failure in Hosts.
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
      await expect(avatar).toHaveAttribute("data-host-attention", String(!sentenceHostsSettingsOnly));
      if (!sentenceHostsSettingsOnly) await expect(avatar.locator(".host-rail-attention")).toBeVisible();
      await shot(page, `${name}-rail-warning-retry`, [page.locator("body")], { pad: 0 });
      await chooseHost(page);
      await expect(avatar).toHaveAttribute("aria-pressed", "false");
      await expect(avatar).toHaveAttribute("data-host-attention", String(!sentenceHostsSettingsOnly));
      await shot(page, `${name}-rail-warning-retry-unselected`, [page.locator("body")], { pad: 0 });
      return;
    }
    await shot(page, `${name}-rail-setup`, [page.locator("body")], { pad: 0 });
    if (sentenceHostsSettingsOnly) await setupPanelShots(page, id, name);
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

/** Show real session and template results without picking or launching one.
 * Search input and mounted rows establish each asynchronous read's premise.
 * The template-only shortcut is the existing public web shortcut; its Enter
 * action launches, so only Close is used. Cleanup closes only this dialog.
 */
async function sentenceSwitcherShots(page: Page, scenario: Scenario, name: string) {
  const dialog = page.locator(".quick-switcher-dialog");
  const window = [page.locator("body")];
  const close = async () => {
    if (await dialog.isVisible()) await dialog.locator(".quick-switcher-close").click();
    await expect(dialog).toBeHidden();
  };
  const capture = async (suffix: string) => {
    // Search owns delayed focus help. Move focus to the actual dialog before
    // a neutral picture, rather than racing the tooltip's scheduled show.
    await dialog.focus();
    await expect(dialog).toBeFocused();
    await shot(page, `${name}-${suffix}`, window, { pad: 0 });
  };
  try {
    await expect(page.locator(".quick-switcher-trigger")).toBeEnabled();
    await page.keyboard.press("Control+Shift+k");
    await expect(dialog).toHaveAccessibleName("Quick switcher");
    const search = dialog.getByRole("combobox", { name: "Search sessions or templates", exact: true });
    await expect(search).toBeFocused();
    await expect(search).toHaveValue("");
    await expect(dialog.locator(".quick-switcher-heading")).toHaveText("Recent sessions");
    const opened = scenario.sessions.find(session => session.open);
    if (!opened) throw new Error("switcher review needs the scenario's initially open session");
    await expect(dialog.locator(".quick-switcher-title").filter({ hasText: opened.title })).toBeVisible();
    await capture("switcher-recent");

    await search.fill(opened.title);
    await expect(search).toHaveValue(opened.title);
    await expect(dialog.locator(".quick-switcher-heading")).toHaveText("Sessions");
    await expect(dialog.locator(".quick-switcher-title").filter({ hasText: opened.title })).toBeVisible();
    await capture("switcher-sessions");

    const unmatched = "review-unmatched-session-name";
    await search.fill(unmatched);
    await expect(search).toHaveValue(unmatched);
    await expect(dialog.locator(".quick-switcher-results").getByRole("option")).toHaveCount(0);
    await expect(dialog.getByRole("status")).toHaveText("No sessions match");
    await expect(dialog.locator(".quick-switcher-new")).toHaveText(`New session named "${unmatched}"`);
    await capture("switcher-no-match");

    const templates = scenario.templates;
    if (!templates?.length) throw new Error("switcher review needs the real staged templates");
    await search.fill("tl:");
    await expect(search).toHaveValue("tl:");
    await expect(dialog.locator(".quick-switcher-heading")).toHaveText("Templates");
    await expect(dialog.locator(".quick-switcher-template")).toHaveCount(templates.length);
    for (const template of templates) {
      await expect(dialog.locator(".quick-switcher-template-name").getByText(template.name, { exact: true })).toBeVisible();
    }
    await capture("switcher-templates");
    await close();

    await expect(page.locator(".quick-switcher-template-trigger")).toBeEnabled();
    await page.keyboard.press("Control+Shift+j");
    await expect(dialog).toHaveAccessibleName("Template launcher");
    const templateSearch = dialog.getByRole("combobox", { name: "Search templates", exact: true });
    await expect(templateSearch).toBeFocused();
    await expect(templateSearch).toHaveValue("");
    await expect(dialog.locator(".quick-switcher-template")).toHaveCount(templates.length);
    await expect(dialog.locator(".quick-switcher-heading")).toHaveText("Templates");
    await capture("template-launcher");
    await close();
  } finally {
    await close();
  }
}

/** Review ordinary session wording on the same real fleet and attachments.
 * All/all-hosts is established before dependent helpers and restored before
 * returning. The outer case owns exact helm preferences; nested menu/outage
 * helpers retain their narrower pin, seen-stamp and remote ownership.
 * No session action, template pick, successful rename or shell-tab creation
 * belongs here. Live/ended observations come from existing staged sessions.
 */
async function sentenceSessionShots(page: Page, request: APIRequestContext, scenario: Scenario, name: string) {
  const initial = scenario.sessions.find(session => session.open);
  const waiting = scenario.sessions.find(session => session.status === "waiting");
  const idle = scenario.sessions.find(session => session.status === "idle" && session.seen === false);
  const ended = scenario.sessions.find(session => session.status === "exited");
  if (!initial || !waiting || !idle || !ended) throw new Error("ordinary session review needs real open, waiting, idle and ended sessions");
  const window = [page.locator("body")];
  let bodyFailed = true;
  try {
    await chooseHost(page);
    await chooseStatus(page, "All");
    await expect(hostChoice(page)).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("radio", { name: "All", exact: true })).toBeChecked();
    await showSession(page, initial);
    if (!sentenceSessionHintsOnly) await shot(page, `${name}-main`, window, { pad: 0 });
    await menuShots(page, request, scenario, name, false);
    if (sentenceSessionHintsOnly) {
      await toolbarShots(page, name);
      bodyFailed = false;
      return;
    }
    await renameShots(page, scenario, name);

    const idleRow = rowOf(page, scenario, session => session === idle);
    const title = idleRow.locator(".session-title");
    await expect(title).toHaveText(idle.title);
    await expect(idleRow.locator(".status-badge.idle.unseen")).toHaveCount(1);
    await title.hover();
    const tooltip = page.locator(".farhelm-tooltip");
    const summary = tooltip.locator(".session-summary");
    await expect(summary).toBeVisible();
    await expect(summary.locator(".session-summary-title")).toHaveText(idle.title);
    await expect(summary.locator(".session-summary-state-word")).toHaveText("Idle — new output");
    await expect(summary.locator(".session-summary-label")).toContainText(["Host", "Directory", "Started", "Last active"]);
    await expect(summary.locator(".session-summary-footer")).toHaveText("Click to open⋯ For actions");
    await insideViewport(page, tooltip);
    await captureShot(page, `${name}-row-summary`, window, { pad: 0, keepPointer: true });
    await page.mouse.move(0, 0);
    await expect(tooltip).toBeHidden();

    const age = idleRow.locator(".status-time");
    await expect(age).toBeVisible();
    const ageHelp = await age.getAttribute("data-tooltip");
    expect(ageHelp, "the real activity stamp owns its absolute-time help").toMatch(/^Last activity /);
    await age.hover();
    await expect(tooltip).toBeVisible();
    await expect(tooltip).toHaveText(ageHelp!);
    await insideViewport(page, tooltip);
    await captureShot(page, `${name}-last-activity`, window, { pad: 0, keepPointer: true });
    await page.mouse.move(0, 0);
    await expect(tooltip).toBeHidden();

    await toolbarShots(page, name);
    // This is the reporting source selected by toolbarShots. It has a real
    // resume offer; only Replace/Delete questions are opened and cancelled.
    for (const action of ["replace", "delete"] as const) {
      const trigger = page.locator(`.header-${action}`);
      const question = page.locator(`.header-${action}-confirm`);
      try {
        await expect(trigger).toBeEnabled();
        await expect(trigger).not.toHaveAttribute("aria-disabled", "true");
        await trigger.click();
        await expect(question).toBeVisible();
        await expect(question.locator(".btn-danger")).toBeFocused();
        await expect(question.locator(".btn-neutral")).toHaveText("Cancel");
        await insideViewport(page, question);
        await clickDialogGap(page, question);
        await shot(page, `${name}-header-${action}-question`, window, { pad: 0 });
        await question.locator(".btn-neutral").click();
        await expect(question).toBeHidden();
      } finally {
        if (await question.isVisible()) await question.locator(".btn-neutral").click();
        await expect(question).toBeHidden();
      }
    }

    const waitingRow = await showSession(page, waiting);
    await expect(waitingRow.locator(".status-badge.waiting")).toHaveCount(1);
    await shot(page, `${name}-selected-waiting`, window, { pad: 0 });
    await rowQuestionShots(page, waitingRow, name);
    const endedRow = await showSession(page, ended);
    await expect(endedRow.locator(".session-row-detail .status-badge")).toContainText("Exited");
    await expect(page.locator(".session-details .status-badge")).toContainText("Exited");
    await shot(page, `${name}-selected-ended`, window, { pad: 0 });
    await showUnavailable(page, request, scenario, waiting, name);

    // The public controls dispatch to the browser's clipboard. Observe the
    // product's caption only; that caption does not prove OS clipboard delivery.
    expect(await page.evaluate(() => typeof navigator.clipboard?.writeText === "function"),
      "copy evidence needs the browser's actual clipboard-write facility").toBe(true);
    const copies = page.locator(".session-details .header-copy");
    await expect(copies).toHaveCount(2);
    for (const [index, suffix] of [[0, "directory"], [1, "command"]] as const) {
      const copy = copies.nth(index);
      await expect(copy).toBeVisible();
      await copy.click();
      await expect(copy.locator(".copy-feedback")).toHaveText("✓ Copied");
      await page.mouse.click(0, 0);
      await shot(page, `${name}-copied-${suffix}`, window, { pad: 0 });
    }

    try {
      await chooseStatus(page, "Waiting");
      await expect(waitingRow).toBeVisible();
      await expect(page.locator(".session-count")).toHaveText(/^\d+ of \d+$/);
      await shot(page, `${name}-status-waiting`, window, { pad: 0 });
      const local = (await realHosts(request)).find(host => host.kind === "local");
      if (!local) throw new Error("ordinary filter review needs the owned local host");
      await chooseHost(page, local.id);
      await expect(page.locator(".filter-empty")).toHaveText("No sessions match this filter");
      await expect(page.locator(".session-count")).toHaveText(/^0 of \d+$/);
      await shot(page, `${name}-status-empty`, window, { pad: 0 });
    } finally {
      await chooseHost(page);
      await chooseStatus(page, "All");
      await expect(waitingRow).toBeVisible();
    }
    await sentenceSwitcherShots(page, scenario, name);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["restore all hosts", () => chooseHost(page)],
      ["restore All status", () => chooseStatus(page, "All")],
      ["restore original selected staged terminal", () => showSession(page, initial)],
    ]);
  }
}

/** Keep row questions on the selected real waiting session, with Cancel only.
 * Menu-owned focus is restored before Escape because a cancelled answer is
 * removed from the DOM. No lifecycle action is ever confirmed.
 */
async function rowQuestionShots(page: Page, waitingRow: Locator, name: string) {
  const window = [page.locator("body")];
  try {
    await expect(waitingRow).toHaveAttribute("data-session-selected", "true");
    await expect(waitingRow.locator(".status-badge.waiting")).toHaveCount(1);
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
  } finally {
    const panel = waitingRow.locator(".session-row-menu-panel");
    if (await panel.isVisible()) {
      for (const selector of [".confirm-cancel", ".replace-cancel"]) {
        const cancel = panel.locator(selector);
        if (await cancel.isVisible()) await cancel.click();
      }
      await panel.getByRole("menuitem").first().focus();
      await page.keyboard.press("Escape");
    }
    await expect(panel).toBeHidden();
  }
}

/** Show real split menus and genuine unavailable/available Restart controls.
 * The reporting source is staged once by the existing setup condition. Never
 * activate its idle Restart: that would act immediately rather than ask.
 */
async function toolbarShots(page: Page, name: string) {
  const main = page.locator(".app-main");
  await expect(page.locator(".header-action-menu:visible, .session-row-menu-panel:visible")).toHaveCount(0);
  try {
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
    for (const action of (sentenceSessionHintsOnly ? [] : ["Restart", "Replace"] as const)) {
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
    await expect(restart).toHaveAttribute("data-tooltip", /^Restart unavailable — this command launch/);
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
    await expect(restart).toHaveAttribute("data-tooltip", "Resume conversation — restarting resumes this session's own conversation.");
    await keyboardFocusControl(page, page.locator("body"), restart);
    await focusedShot(page, `${name}-toolbar-available-restart`, restart);
    if (!sentenceSessionHintsOnly) {
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
    }
    const reviewRow = page.locator(`[data-session-id="${reviewId}"]`);
    await openRowMenu(reviewRow);
    const rowPanel = page.locator(".session-row-menu-flyout");
    await expect(rowPanel).toBeVisible();
    await shot(page, `${name}-row-replace-glyph`, [page.locator("body")], { pad: 0 });
    await page.keyboard.press("Escape");
    await expect(rowPanel).toBeHidden();
  } finally {
    // A failed menu assertion still leaves a mounted input owner. Dismiss
    // only the menus this helper opens before caller restoration touches rail
    // controls or attempts another attachment.
    const headerMenu = page.locator(".header-action-menu");
    if (await headerMenu.isVisible()) {
      await headerMenu.getByRole("menuitem").first().focus();
      await page.keyboard.press("Escape");
    }
    await expect(headerMenu).toBeHidden();
    const rowMenu = page.locator(".session-row-menu-panel:visible");
    if (await rowMenu.count()) {
      await rowMenu.getByRole("menuitem").first().focus();
      await page.keyboard.press("Escape");
    }
    await expect(rowMenu).toHaveCount(0);
  }
}

for (const colorScheme of ["dark", "light"] as const) {
  for (const width of ["wide", "main-650"] as const) {
    // Footer review supplies its own true phone case once per palette.
    if (trashFooterOnly && width === "main-650") continue;
    if (sentenceAuthOnly) {
      /** The prompt replaces the app, so the legacy main-650 loop key means
       * a 650px whole window here. This callback deliberately requests no
       * page/request fixture: those would load authenticated capture state
       * before an early return could isolate the browser prompt.
       */
      test(`${colorScheme} ${width === "main-650" ? "window-650" : width}`, async ({ browser, baseURL }) => {
        if (!baseURL) throw new Error("auth review needs the docs fixture's local base URL");
        await sentenceAuthShots(browser, baseURL, colorScheme, width === "main-650");
      });
      continue;
    }
    /**
     * The main-pane measurement keeps the narrow case meaningful when a later
     * plan widens the sidebar for the host rail. Native controls and real menus
     * remain visible in a full-window capture; no review overlay hides defects.
     * Settings adds its phone breakpoint once per palette after main-650.
     */
    test(`${colorScheme} ${width}`, async ({ page, request }) => {
      test.skip(sentenceApprovalOnly, "only the pending approval pictures need a repeat");
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
        const { scenario } = await open(page, request, { checkoutTrash: true, terminalFiles: sentenceTerminalOnly });
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
        if (trashFooterOnly) {
          await trashFooterShots(page, request, scenario, colorScheme);
          return;
        }
        if (sentenceTerminalOnly) {
          await sentenceTerminalShots(page, request, scenario, name, colorScheme);
          await unboundShortcutShots(page, request, name);
          return;
        }
        if (sentenceTrashOnly) {
          await trashShots(page, name);
          return;
        }
        if (sentenceSessionsOnly || sentenceSessionHintsOnly) {
          await sentenceSessionShots(page, request, scenario, name);
          return;
        }
        if (sentenceHelpOnly) {
          await sentenceHelpShots(page, name);
          return;
        }
        if (sentenceHostsSettingsOnly) {
          await settingsShots(page, name, width === "main-650");
          await hostPanelShots(page, name);
          await hostSettingsShots(page, scenario, name, true);
          const original = await hostHintShots(page, request, scenario, name, true);
          const refusal = await openLiveUninstallRefusal(page, request, scenario, original);
          try {
            await shot(page, `${name}-uninstall-refusal`, window, { pad: 0 });
          } finally {
            if (await refusal.isVisible()) await refusal.locator(".host-refusal-close").click();
            await expect(refusal).toBeHidden();
            await sameConnectedHost(request, original);
          }
          await showSessionsPanel(page);
          await showRailSetup(page, request, scenario, name);
          await showRailSetup(page, request, scenario, name, true);
          const waiting = scenario.sessions.find(session => session.status === "waiting");
          if (!waiting) throw new Error("the unavailable pictures need the owned waiting session");
          await showUnavailable(page, request, scenario, waiting, name);
          return;
        }
        if (sentenceLauncherOnly) {
          await launcherShots(page, request, name, width === "main-650");
          await sentenceTemplateShots(page, name);
          // These two checkout fields also appear in Settings. Its surrounding
          // controls retain the head's exact current spelling and real state.
          await settingsShots(page, name, width === "main-650");
          return;
        }
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
          await toolbarShots(page, name);
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
          await expect(page.getByRole("menu", { name: "Help", exact: true })).toBeVisible();
          await shot(page, `${name}-rail-help`, window, { pad: 0 });
          await page.keyboard.press("Escape");
          await expect(page.getByRole("menu", { name: "Help", exact: true })).toBeHidden();
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

        await hostSettingsShots(page, scenario, name);

        const originalHost = await hostHintShots(page, request, scenario, name, true);
        const refusal = await openLiveUninstallRefusal(page, request, scenario, originalHost);
        await shot(page, `${name}-uninstall-refusal`, window, { pad: 0 });
        await refusal.locator(".host-refusal-close").click();
        await expect(refusal).toBeHidden();
        await sameConnectedHost(request, originalHost);

        // The refusal opens Hosts through its menu helper. Return through the
        // real panel control before using Templates in the concealed session UI.
        await showSessionsPanel(page);
        await page.locator(".templates-button").click();
        const templates = page.locator(".templates-dialog");
        await expect(templates.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
        await templates.locator(".templates-row").filter({ has: page.getByText("codex-deep", { exact: true }) }).click();
        await expect(templates.locator(".templates-editor-heading h3")).toHaveText("Edit template");
        await expect(templates.getByRole("button", { name: "Number shortcut codex-deep", exact: true }))
          .toHaveCSS("font-family", /JetBrains Mono/);
        await expect(templates.locator(".templates-name")).toHaveValue("codex-deep");
        await expect(templates.locator(".templates-name")).toBeFocused();
        // Selection hands focus to Name on the next frame. Dismiss its help
        // through a real dialog gap before taking this neutral editor picture.
        await clickDialogGap(page, templates);
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
        await page.locator('[data-bar-menu="help"] [data-bar-menu-item="Welcome"]').click();
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
        await expect(launcher.getByRole("combobox", { name: "Model", exact: true })).toHaveCSS("font-family", /JetBrains Mono/);
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
        await rowQuestionShots(page, waitingRow, name);
        await showUnavailable(page, request, scenario, waiting, name);
        const idleRow = await showSession(page, idle);
        await expect(idleRow.locator(".status-badge.idle:not(.unseen)")).toHaveCount(1);
        await expect(idleRow.locator(".status-dot")).toHaveCSS("color", "rgb(255, 255, 255)");
        await shot(page, `${name}-selected-seen`, window, { pad: 0 });
        await openRowMenu(idleRow);
        const unread = idleRow.getByRole("menuitem", { name: "Mark unread", exact: true });
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

/** Photograph one real request without consuming it between palette/width cases.
 * A single pending Spawn supplies shared ordinary approval anatomy, not the
 * distinct queued, late, HostAdd or preapproved-link presentations. The real
 * GUI/feed must be connected before the authenticated source asks. Deny is
 * the only answer, and every cleanup step runs after an earlier failure too.
 */
// Skip at registration for auth review, before authenticated fixtures load.
const ordinaryApprovalTest = sentenceAuthOnly ? test.skip : test;
ordinaryApprovalTest("sentence-case ordinary approval in both themes and widths", async ({ page, request }) => {
  test.skip(!sentenceHostsSettingsOnly && !sentenceApprovalOnly, "the owned Spawn source is staged only for the focused selection");
  const fleet = readFleet();
  const source = fleet.spawnReview;
  const id = source && fleet.ids.get(source.title);
  if (!source || !id) throw new Error("the focused capture must stage its owned Spawn source first");
  let local: Awaited<ReturnType<typeof listHosts>>[number] | undefined;
  let restore: Parameters<typeof patchPreferences>[1] | undefined;
  let before: string[] | undefined;
  let triggered = false;
  let denied = false;
  let bodyFailed = true;
  let feed: import("@playwright/test").WebSocket | undefined;
  let greeted = false;
  page.on("websocket", socket => {
    if (new URL(socket.url()).pathname !== "/api/events") return;
    feed = socket;
    greeted = false;
    socket.on("framereceived", frame => {
      if (feed === socket) {
        const revision = JSON.parse(String(frame.payload)).revision;
        greeted = Number.isSafeInteger(revision) && revision >= 0;
      }
    });
  });
  /** Read only the real card fields this capture must establish.
   * Keeping the wire premise beside its use avoids importing a fabricated
   * approval fixture as a convenient source of presentation state.
   */
  type Pending = {
    id: string; host_id: number; host_name: string; expires_at_ms: number;
    session: { id: string; title: string; host_name: string };
    action: { kind: string; verb: string; cwd: string; launch: { kind: string; command: string; yolo: boolean } };
  };
  /** Read the helm's real pending table without routing or constructing it. */
  const pending = async (): Promise<Pending[]> => {
    const response = await request.get("/api/approvals");
    expect(response.ok(), "read the real pending requests").toBe(true);
    return (await response.json()).approvals;
  };
  /** Deny only this source's request and observe its real refusal/no-child result.
   * Keep the check inside the source's lifetime. Its terminal proves Deny
   * reached the CLI; a vanished GUI card alone could also mean expiry.
   */
  const denyAndVerify = async () => {
    const current = (await pending()).filter(card => card.session.id === id);
    expect(current.length).toBeLessThanOrEqual(1);
    if (current.length) {
      const card = page.locator(`.approval-card[data-approval-id="${current[0].id}"]`);
      await expect(card).toBeVisible();
      await card.locator(".approval-deny").click();
      await expect(card).toHaveCount(0);
    }
    if (triggered && !denied) {
      await waitForTermText(page, "SPAWN-ERROR:");
      await waitForTermText(page, "declined");
      expect((await pending()).filter(card => card.session.id === id)).toEqual([]);
      expect((await listSessions(request)).sessions.map(session => session.id).sort(), "Deny creates no child").toEqual(before);
      denied = true;
    }
  };
  try {
    local = (await listHosts(request)).find(host => host.kind === "local");
    if (!local) throw new Error("the approval requester needs the owned local host");
    const saved = await readPreferences(request);
    restore = { list_sort: saved.list_sort ?? null, last_selected: saved.last_selected ?? null, compact: saved.compact ?? null };
    await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1600, height: 1000 });
    await open(page, request);
    await expect.poll(() => greeted && Boolean(feed && !feed.isClosed()), {
      message: "the genuine app receives its live feed's revision handshake before Spawn",
    }).toBe(true);
    await attachSession(page, id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await setLocalCommandsWithoutAsking(request, false);
    await setLocalYoloWithoutAsking(request, true);
    expect(await pending(), "the dedicated request starts with no competing cards").toEqual([]);
    before = (await listSessions(request)).sessions.map(session => session.id).sort();
    triggered = true;
    await submitPrompt(page, "spawn /tmp", 100);
    let requestId: string | undefined;
    await expect.poll(async () => {
      const cards = await pending();
      const own = cards.find(card => card.session.id === id);
      requestId = own?.id;
      return own?.action.kind;
    }, { timeout: 20_000, message: "the real Spawn request becomes pending on this source" }).toBe("launch");
    for (const colorScheme of ["dark", "light"] as const) {
      for (const width of ["wide", "main-650"] as const) {
        await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
        await page.setViewportSize({ width: 1600, height: 1000 });
        const main = page.locator(".app-main");
        if (width === "main-650") {
          const box = await main.boundingBox();
          expect(box).not.toBeNull();
          await page.setViewportSize({ width: Math.round(box!.x + 650), height: 1000 });
          await expect.poll(async () => (await main.boundingBox())?.width).toBeCloseTo(650, 0);
        }
        await page.evaluate(() => document.fonts.ready.then(() => undefined));
        await expect(page.locator("html")).toHaveAttribute("data-appearance", colorScheme);
        await expect(page.locator(".build-skew")).toHaveCount(0);
        const cards = await pending();
        expect(cards).toHaveLength(1);
        expect(cards[0]).toMatchObject({
          id: requestId, host_id: local.id,
          session: { id, title: source.title },
          action: { kind: "launch", verb: "spawn_inherited", cwd: "/tmp", launch: { kind: "command", command: "claude {farhelm_args}", yolo: false } },
        });
        expect(cards[0].expires_at_ms, "all pictures precede the unchanged production expiry").toBeGreaterThan(Date.now());
        expect(greeted && Boolean(feed && !feed.isClosed()), "the real GUI/feed remains connected").toBe(true);
        const card = page.locator(`.approval-card[data-approval-id="${requestId}"]`);
        await expect(card).toBeVisible();
        await expect(card.locator(".approval-card-badge")).toHaveText("Agent request");
        await expect(card.locator(".approval-card-heading")).toHaveText("Start a new session running the asking session's own launch");
        await expect(card.locator(".approval-card-requester")).toContainText(source.title);
        await expect(card.locator(".approval-card-requester")).toContainText(id);
        await expect(card.locator(".approval-card-requester")).toContainText(cards[0].host_name);
        await expect(card.locator(".approval-card-command")).toContainText("claude {farhelm_args}");
        await expect(card.locator(".approval-card-rows")).toContainText("/tmp");
        await expect(card.locator(".approval-allow")).toHaveText("Allow");
        await expect(card.locator(".approval-always-allow")).toContainText("Always allow from");
        await expect(card.locator(".approval-deny")).toHaveText("Deny");
        // The real card briefly disarms its answers after selection. Its
        // captions alone are not readiness evidence for the pictured action row.
        for (const selector of [".approval-allow", ".approval-always-allow", ".approval-deny"]) {
          await expect(card.locator(selector)).toBeEnabled();
        }
        expect(cards[0].expires_at_ms, "the armed picture still precedes production expiry").toBeGreaterThan(Date.now());
        await insideViewport(page, card);
        await shot(page, `visual-review/${colorScheme}-${width}-ordinary-approval`, [page.locator("body")], { pad: 0 });
      }
    }
    await denyAndVerify();
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["deny the owned pending request and observe refusal/no child", denyAndVerify],
      // Drain the independent fetches inside admitted route callbacks while
      // their source still exists; closing alone does not join those callbacks.
      ["drain the requester view routes before removal", () => page.unrouteAll({ behavior: "wait" })],
      ["close the requester view before removal", () => page.close()],
      ["remove only the owned requester", () => cleanupSession(request, id)],
      ["restore the exact command permission", () => local && setLocalCommandsWithoutAsking(request, local.commands_without_asking)],
      ["restore the exact YOLO permission", () => local && setLocalYoloWithoutAsking(request, local.yolo_without_asking)],
      ["restore the exact fleet-opening preferences", () => restore && patchPreferences(request, restore)],
    ]);
  }
});
