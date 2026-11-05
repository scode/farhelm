import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { chooseSort, chooseStatus } from "./helpers/list-header";
import { waitForSessionReady, waitForSessionRevealed } from "./helpers/terminal-readiness";
import { type InputFrame, programInput, recordInput } from "./helpers/program-input";
import { requireHelmBuild } from "./helpers/helm-build";
import { routeGate } from "./helpers/route-gate";
import { terminalInput, waitForTermText } from "./helpers/term";
import {
  cleanupSession,
  createSession,
  listHosts,
  listSessions,
  observeFeedReaders,
  openRowMenu,
  patchPreferences,
  pinAutoSelect,
  resetPreferences,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";

/** Routing identity stays separate from the title whose selectable text this spec exercises. */
function row(page: Page, id: string): Locator {
  return page.locator(`.session-row[data-session-id="${id}"]`);
}

/** The native Cmd+N listener clicks the current New control. Exercise that exact
 * dispatch target in both engines; this does not claim native Mac key delivery. */
test("marking retains New dispatch and launcher cancellation focus", async ({ page, request }) => {
  let id: string | undefined;
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    id = (await createSession(request, { title: `bulk-new-${Date.now()}` })).id;
    await page.goto("/");
    await row(page, id).locator(".session-row-open").click({ modifiers: ["Control"] });
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "1");
    const newButton = page.locator(".new-session-button");
    await expect(newButton).toHaveCount(1);
    await expect(newButton).toBeHidden();
    await expect(newButton).toBeEnabled();
    await newButton.evaluate((button: HTMLButtonElement) => button.click());
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    await form.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(form).toHaveCount(0);
    await expect(page.locator(".marked-menu-toggle")).toBeFocused();
    await expect(row(page, id)).toHaveAttribute("data-session-marked", "true");
    // Reset preferences left this number unbound. Its ordinary shortcut opens
    // Templates while the marked bar still conceals the usual return button.
    await page.keyboard.press("Control+Shift+Digit9");
    const templates = page.locator('.templates-dialog[role="dialog"]');
    await expect(templates).toBeVisible();
    await templates.locator(".templates-close").click();
    await expect(templates).toHaveCount(0);
    await expect(page.locator(".marked-menu-toggle")).toBeFocused();
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ...(id ? [["owned session", () => cleanupSession(request, id!)]] as [string, () => Promise<void>][] : []),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/** A host warning moves the toolbar without changing its targets or menu offers.
 * Controlled read replies retain a real owned session and exercise the existing
 * regional movement report, with no pointer press that could dismiss the menu. */
test("host attention movement closes the marked menu and reopening remeasures", async ({ page, request }) => {
  let id: string | undefined;
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    id = (await createSession(request, { title: `bulk-movement-${Date.now()}` })).id;
    await pinAutoSelect(page, id);
    const hostsReply = await request.get("/api/hosts");
    expect(hostsReply.ok()).toBe(true);
    const build = requireHelmBuild(hostsReply, "bulk menu movement");
    const registry = await hostsReply.json();
    const local = registry.hosts.find((host: { kind: string }) => host.kind === "local");
    expect(local?.state.phase, "the real selected session's host must be connected").toBe("connected");
    const fakeId = 9307;
    expect(local.id).not.toBe(fakeId);
    let problem = false;
    await page.route((url) => url.pathname === "/api/hosts", async (route) => {
      expect(route.request().method()).toBe("GET");
      await route.fulfill({
        headers: { "x-farhelm-build": build, "content-type": "application/json" },
        json: {
          hosts: [local, {
            ...local,
            id: fakeId,
            kind: "ssh",
            name: "bulk-layout.invalid",
            destination: "bulk-layout.invalid",
            alias: null,
            identity: null,
            remote_farhelm: null,
            remote_state_dir: null,
            state: problem ? { phase: "unreachable-reprobing", cause: "connection", last_error: "layout fixture" } : {
              phase: "connected",
              identity: null,
              build_version: build,
              old_version: false,
              newer_version: false,
              refresh: { status: "ok", sessions: 0 },
            },
          }],
        },
      });
    });
    await page.route((url) => url.pathname === `/api/hosts/${fakeId}/provisioning`, (route) =>
      route.fulfill({
        headers: { "x-farhelm-build": build, "content-type": "application/json" },
        json: { host_id: fakeId, run_id: null, operation: null, status: "completed", steps: [], message: null },
      }));
    const listingReply = await request.get("/api/sessions");
    expect(listingReply.ok()).toBe(true);
    const listing = await listingReply.json();
    const source = listing.sessions.find((session: { id: string }) => session.id === id);
    expect(source, "the marking must name an actual owned session").toBeTruthy();
    await page.route((url) => url.pathname === "/api/sessions", async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      await route.fulfill({
        headers: { "x-farhelm-build": build, "content-type": "application/json" },
        json: {
          ...listing,
          sessions: [{
            ...source,
            status: { state: "idle" },
            stale: false,
            pinned: false,
            last_activity_at: 1,
            seen_activity_at: 1,
          }],
          total: 1,
          matching: 1,
          truncated: false,
        },
      });
    });
    await observeFeedReaders(page);
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    await page.goto("/");
    await feed.waitForConnection(1);
    await waitForFeedReadersSettled(page);
    await expect(page.locator(`[data-attention-host-id="${fakeId}"]`)).toHaveCount(0);
    await row(page, id).locator(".session-row-open").click({ modifiers: ["Control"] });
    const toggle = page.locator(".marked-menu-toggle");
    await toggle.focus();
    await expect(toggle).toBeFocused();
    await toggle.press("ArrowDown");
    const menu = page.getByRole("menu", { name: "Marked session actions", exact: true });
    await expect(menu).toBeVisible();
    const before = (await toggle.boundingBox())!;
    expect(before.height).toBeGreaterThan(0);
    const offers = await menu.getByRole("menuitem").allTextContents();
    problem = true;
    feed.notify(2);
    await expect(page.locator(`[data-attention-host-id="${fakeId}"]`)).toBeVisible();
    await waitForFeedReadersSettled(page);
    expect((await toggle.boundingBox())!.y).toBeGreaterThan(before.y);
    await expect(menu).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(toggle).toBeFocused();
    await toggle.press("ArrowDown");
    await expect(menu).toBeVisible();
    expect(await menu.getByRole("menuitem").allTextContents()).toEqual(offers);
    const flyout = page.locator(".marked-menu-flyout");
    await expect(flyout).toHaveCSS("opacity", "1");
    const after = (await toggle.boundingBox())!;
    const popup = (await flyout.boundingBox())!;
    expect(popup.y + popup.height).toBeGreaterThan(after.y);
    expect(popup.y).toBeLessThan(after.y + after.height);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "ignoreErrors" })],
      ...(id ? [["owned session", () => cleanupSession(request, id!)]] as [string, () => Promise<void>][] : []),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/** A retained hidden mark can become effective from a feed reply while Trash
 * owns focus. Hiding its trigger must preserve the exact active dialog owner. */
test("a newly displayed retained mark preserves the open Trash dialog", async ({ page, request }) => {
  let id: string | undefined;
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    id = (await createSession(request, { title: `bulk-trash-owner-${Date.now()}` })).id;
    await pinAutoSelect(page, id);
    const response = await request.get("/api/sessions");
    expect(response.ok()).toBe(true);
    const build = requireHelmBuild(response, "marked Trash owner");
    const listing = await response.json();
    const source = listing.sessions.find((session: { id: string }) => session.id === id);
    expect(source, "the controlled row must exist on the real host").toBeTruthy();
    let status = "idle";
    await page.route((url) => url.pathname === "/api/sessions", async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      const filter = new URL(route.request().url()).searchParams.get("status");
      const sessions = !filter || filter === status
        ? [{ ...source, status: { state: status }, stale: false, last_activity_at: 1, seen_activity_at: 1 }]
        : [];
      await route.fulfill({
        headers: { "x-farhelm-build": build, "content-type": "application/json" },
        json: { ...listing, sessions, total: 1, matching: sessions.length, truncated: false },
      });
    });
    await observeFeedReaders(page);
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    await page.goto("/");
    await feed.waitForConnection(1);
    await waitForFeedReadersSettled(page);
    await row(page, id).locator(".session-row-open").click({ modifiers: ["Control"] });
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "1");
    await chooseStatus(page, "Running");
    await expect(row(page, id)).toHaveCount(0);
    await expect(page.locator(".marked-bar")).toHaveCount(0);
    const trigger = page.locator(".checkout-trash-button");
    await expect(trigger).toBeVisible();
    await trigger.click();
    const dialog = page.locator('.checkout-trash-dialog[role="dialog"]');
    const close = dialog.locator(".trash-close");
    await expect(dialog).toBeVisible();
    await close.focus();
    await expect(close).toBeFocused();
    const original = await dialog.elementHandle();
    expect(original).toBeTruthy();
    status = "running";
    feed.notify(2);
    await expect(row(page, id)).toHaveAttribute("data-session-marked", "true");
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "1");
    await expect(trigger).toBeHidden();
    await expect(dialog).toBeVisible();
    expect(
      await page.evaluate(
        (node) => node!.isConnected && node === document.querySelector(".checkout-trash-dialog"),
        original,
      ),
    ).toBe(true);
    await expect(close).toBeFocused();
    await close.click();
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".marked-menu-toggle")).toBeFocused();
    await original?.dispose();
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "ignoreErrors" })],
      ...(id ? [["owned session", () => cleanupSession(request, id!)]] as [string, () => Promise<void>][] : []),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/** Wire an affirmative archive outcome only after each real owned Delete
 * succeeds. This tests cue presentation, not filesystem archival. A positive
 * visible-target flight proves the observer can detect the hidden-target bug. */
test("archive cues require a visible Trash destination while marks remain", async ({ page, request }) => {
  const created: string[] = [];
  let bodyFailed = false;
  await resetPreferences(request);
  await patchPreferences(request, { skip_session_confirmations: true });
  try {
    for (const name of ["marked", "hidden-cue", "visible-cue"]) {
      created.push((await createSession(request, { title: `bulk-${name}-${Date.now()}` })).id);
    }
    const [marked, hidden, visible] = created;
    const completed = new Set<string>();
    for (const id of [hidden, visible]) {
      await page.route((url) => url.pathname === `/api/sessions/${encodeURIComponent(id)}`, async (route) => {
        if (route.request().method() !== "DELETE") return route.continue();
        const response = await route.fetch();
        expect(response.ok(), "the real Delete must complete before controlling its archive outcome").toBe(true);
        const outcome = await response.json();
        await route.fulfill({ response, json: { ...outcome, archived: true } });
        completed.add(id);
      });
    }
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.goto("/");
    await row(page, marked).locator(".session-row-open").click({ modifiers: ["Control"] });
    await expect(page.locator(".checkout-trash-button")).toBeHidden();
    await page.evaluate(() => {
      const probe = { flights: 0 };
      Object.assign(window, { __bulkTrashCueProbe: probe });
      new MutationObserver((records) => {
        for (const record of records) {
          for (const node of record.addedNodes) {
            if (node instanceof Element && node.matches(".checkout-trash-flight")) probe.flights++;
          }
        }
      }).observe(document.body, { childList: true });
    });
    const flights = () =>
      page.evaluate(() =>
        (window as unknown as { __bulkTrashCueProbe: { flights: number } }).__bulkTrashCueProbe.flights
      );
    expect(await flights()).toBe(0);
    await openRowMenu(row(page, hidden));
    await row(page, hidden).locator(".session-row-delete").click();
    await expect(row(page, hidden)).toHaveCount(0);
    await expect.poll(() => completed.has(hidden)).toBe(true);
    await expect(row(page, marked)).toHaveAttribute("data-session-marked", "true");
    expect(await flights(), "an affirmative archive must not fly toward a hidden zero-sized control").toBe(0);
    await page.locator(".marked-clear").click();
    const destination = page.locator(".checkout-trash-button");
    await expect(destination).toBeVisible();
    const rect = (await destination.boundingBox())!;
    expect(rect.width).toBeGreaterThan(0);
    expect(rect.height).toBeGreaterThan(0);
    expect(await page.evaluate(() => matchMedia("(prefers-reduced-motion: reduce)").matches)).toBe(false);
    await openRowMenu(row(page, visible));
    await row(page, visible).locator(".session-row-delete").click();
    await expect(row(page, visible)).toHaveCount(0);
    await expect.poll(() => completed.has(visible)).toBe(true);
    await expect.poll(flights, { message: "the same observer must see the ordinary visible-target cue" }).toBe(1);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "ignoreErrors" })],
      ...created.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/** Count actual displayed slots, including any unrelated fixture between the two endpoints. */
async function rangeBetween(page: Page, anchor: string, target: string): Promise<string[]> {
  const ids = await page.locator(".session-row").evaluateAll((rows) =>
    rows.map((one) => one.getAttribute("data-session-id")!)
  );
  const a = ids.indexOf(anchor);
  const b = ids.indexOf(target);
  expect(a, "the open range anchor must be displayed").toBeGreaterThanOrEqual(0);
  expect(b, "the clicked range endpoint must be displayed").toBeGreaterThanOrEqual(0);
  return ids.slice(Math.min(a, b), Math.max(a, b) + 1);
}

/** Compare the whole marking so an accidental mark on another row cannot hide behind a count. */
async function markedIds(page: Page): Promise<string[]> {
  return page.locator('.session-row[data-session-marked="true"]').evaluateAll(
    (rows) => rows.map((one) => one.getAttribute("data-session-id")!),
  );
}

/** Keep text and endpoint ownership: unchanged text alone can miss an extended same-word selection. */
async function selectedText(page: Page) {
  return page.evaluate(() => {
    const selection = window.getSelection();
    const owner = (node: Node | null) =>
      (node instanceof Element ? node : node?.parentElement)
        ?.closest(".session-row")?.getAttribute("data-session-id");
    return {
      text: selection?.toString(),
      anchor: owner(selection?.anchorNode ?? null),
      focus: owner(selection?.focusNode ?? null),
      anchorOffset: selection?.anchorOffset,
      focusOffset: selection?.focusOffset,
    };
  });
}

/** Marking must not navigate or extend text selection. Real dragging is tested independently of the seeded range,
 * because a programmatic Range would still work if ordinary mouse text selection had been disabled. */
test(
  "range and toggle marking preserve the open session and ordinary text selection",
  async ({ page, request }, testInfo) => {
    const created: string[] = [];
    let bodyFailed = false;
    await resetPreferences(request);
    try {
      const stamp = Date.now();
      for (const name of ["a", "b", "c"]) {
        created.push((await createSession(request, { title: `bulk-${stamp}-${name} selectable title text` })).id);
      }
      const [a, b, c] = created;
      await page.goto("/");
      await chooseSort(page, "title");
      for (const id of created) await expect(row(page, id)).toBeVisible({ timeout: 20_000 });
      await row(page, a).locator(".session-row-open").click();
      await waitForSessionRevealed(page, a);
      await expect(row(page, a)).toHaveAttribute("data-session-selected", "true");
      await expect(row(page, b)).toHaveAttribute("data-session-selected", "false");
      await expect(row(page, c)).toHaveAttribute("data-session-selected", "false");

      const title = row(page, a).locator(".session-title").first();
      await title.evaluate((element) => {
        const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
        const text = walker.nextNode();
        if (!text || (text.textContent?.length ?? 0) < 6) throw new Error("fixture title has no selectable text");
        const selection = window.getSelection();
        const range = document.createRange();
        range.setStart(text, 1);
        range.setEnd(text, 5);
        selection?.removeAllRanges();
        selection?.addRange(range);
      });
      const before = await selectedText(page);
      expect(before.text?.length, "premise: a real range exists before Shift mouse-down").toBe(4);
      expect(before.anchor).toBe(a);
      const expectedRange = await rangeBetween(page, a, c);
      await row(page, c).locator(".session-row-open").click({ modifiers: ["Shift"] });
      await expect.poll(() => markedIds(page)).toEqual(expectedRange);
      expect(await selectedText(page), "Shift marking must leave the page's text selection unchanged").toEqual(before);
      await expect(row(page, a)).toHaveAttribute("data-session-selected", "true");
      await expect(row(page, c)).toHaveAttribute("data-session-selected", "false");
      await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", String(expectedRange.length));
      await page.evaluate(() => window.getSelection()?.removeAllRanges());
      for (const palette of ["dark", "light"] as const) {
        await page.emulateMedia({ colorScheme: palette });
        await expect(page.locator("html")).toHaveAttribute("data-appearance", palette);
        await testInfo.attach(`marked-sidebar-${palette}`, {
          body: await page.locator(".app-sidebar").screenshot(),
          contentType: "image/png",
        });
      }

      await row(page, a).locator(".session-row-open").click({ modifiers: ["Control"] });
      await expect(row(page, a)).toHaveAttribute("data-session-marked", "false");
      await row(page, a).locator(".session-row-open").click({ modifiers: ["Meta"] });
      await expect(row(page, a)).toHaveAttribute("data-session-marked", "true");
      await row(page, b).locator(".session-row-open").click({ modifiers: ["Meta"] });
      await expect(row(page, b)).toHaveAttribute("data-session-marked", "false");
      await expect(row(page, a)).toHaveAttribute("data-session-selected", "true");

      await row(page, b).locator(".session-row-open").click();
      await waitForSessionRevealed(page, b);
      await expect(row(page, b)).toHaveAttribute("data-session-selected", "true");
      await expect.poll(() => markedIds(page)).toEqual([]);
      await expect(page.locator(".marked-bar")).toHaveCount(0);

      // Native buttons do not promise draggable text. Ordinary page text must remain selectable; the web version
      // readout is a real span, so this catches a blanket selection ban without inventing button behavior.
      await page.evaluate(() => window.getSelection()?.removeAllRanges());
      const dragText = page.locator("span.app-version");
      await expect(dragText).toHaveText("0.0.0-unreleased");
      expect(await dragText.evaluate((element) => getComputedStyle(element).userSelect)).not.toBe("none");
      const box = await dragText.boundingBox();
      expect(box, "the version readout must have a pointer target").not.toBeNull();
      expect(box!.width).toBeGreaterThan(60);
      await page.mouse.move(box!.x + 5, box!.y + box!.height / 2);
      await page.mouse.down();
      await page.mouse.move(box!.x + Math.min(95, box!.width - 5), box!.y + box!.height / 2, { steps: 12 });
      await page.mouse.up();
      await expect.poll(async () => (await selectedText(page)).text?.length ?? 0).toBeGreaterThan(0);
      expect(
        await dragText.evaluate((element) => {
          const selection = window.getSelection();
          return !!selection?.anchorNode && !!selection.focusNode && element.contains(selection.anchorNode) &&
            element.contains(selection.focusNode);
        }),
        "the native drag selected the actual version text",
      ).toBe(true);
      await testInfo.attach("marking-sidebar", {
        body: await page.locator(".app-sidebar").screenshot(),
        contentType: "image/png",
      });
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["preferences", () => resetPreferences(request)],
        ...created.reverse().map((
          id,
        ): [string, () => Promise<void>] => [`session ${id}`, () => cleanupSession(request, id)]),
      ]);
    }
  },
);

/** Menus and the live terminal retain Escape while marks exist. A list-owned Escape clears marks without
 * preventing the browser default, even if a child menu dismisses synchronously during bubbling. */
test("Escape belongs to menus and the terminal before list clearing", async ({ page, request }) => {
  const created: string[] = [];
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    for (const name of ["open", "target"]) {
      created.push((await createSession(request, { title: `bulk-escape-${name}-${Date.now()}` })).id);
    }
    const [open, target] = created;
    const frames: InputFrame[] = [];
    page.on("websocket", (socket) => {
      if (new URL(socket.url()).pathname === `/api/sessions/${encodeURIComponent(open)}/term`) {
        recordInput(socket, frames);
      }
    });
    await page.goto("/");
    await expect(row(page, open)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, target)).toBeVisible();
    await row(page, open).locator(".session-row-open").click();
    await waitForSessionRevealed(page, open);
    await expect(row(page, open)).toHaveAttribute("data-session-selected", "true");
    await row(page, target).locator(".session-row-open").click({ modifiers: ["Control"] });
    await expect(row(page, target)).toHaveAttribute("data-session-marked", "true");

    await openRowMenu(row(page, target));
    const pin = row(page, target).getByRole("menuitem", { name: "Pin to top", exact: true });
    await pin.focus();
    await expect(pin).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(row(page, target).locator(".session-row-menu-flyout")).toHaveCount(0);
    await expect(row(page, target)).toHaveAttribute("data-session-marked", "true");

    await page.getByRole("button", { name: "Marked session actions", exact: true }).click();
    const menu = page.getByRole("menu", { name: "Marked session actions", exact: true });
    await expect(menu).toBeVisible();
    const bulkPin = menu.getByRole("menuitem", { name: /^Pin(?:$|\s)/ });
    await bulkPin.focus();
    await expect(bulkPin).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    await expect(row(page, target)).toHaveAttribute("data-session-marked", "true");

    const terminal = page.getByRole("textbox", { name: "Terminal input", exact: true });
    await terminal.focus();
    await waitForSessionReady(page, open);
    frames.length = 0;
    await page.keyboard.press("Escape");
    await expect.poll(() => programInput(frames), {
      message: "the focused terminal must send Escape to its own socket",
    }).toBe("\x1b");
    await expect(row(page, target)).toHaveAttribute("data-session-marked", "true");

    // A window bubble listener observes the default state after the delegated application handler ran.
    await page.evaluate(() => {
      window.addEventListener("keydown", (event) => {
        if (event.key === "Escape") document.documentElement.dataset.bulkEscapeDefault = String(event.defaultPrevented);
      });
    });
    const clear = page.getByRole("button", { name: "Clear marked sessions", exact: true });
    await clear.focus();
    await expect(clear).toBeFocused();
    await page.keyboard.press("Escape");
    await expect.poll(() => markedIds(page)).toEqual([]);
    await expect(page.locator("html")).toHaveAttribute("data-bulk-escape-default", "false");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["preferences", () => resetPreferences(request)],
      ...created.reverse().map((
        id,
      ): [string, () => Promise<void>] => [`session ${id}`, () => cleanupSession(request, id)]),
    ]);
  }
});

/** Filtering must remove a hidden mark from real metadata writes, while mixed eligibility stays per row.
 * An ended marked row cannot acquire Mark read merely because another marked session has unseen output. */
test("bulk metadata acts only on displayed eligible rows", async ({ page, request }) => {
  test.setTimeout(150_000);
  const created: string[] = [];
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    for (const name of ["open", "unread", "ended"]) {
      // Eligibility needs an ended row, not a Stop/startup race. A successful
      // short command establishes that premise through its own actual exit.
      created.push(
        (await createSession(request, {
          title: `bulk-metadata-${name}-${Date.now()}`,
          ...(name === "ended" ? { invocation: "true" } : {}),
        })).id,
      );
    }
    const [open, unread, ended] = created;
    await expect.poll(
      async () => (await listSessions(request)).sessions.find((one) => one.id === ended)?.status?.state,
      {
        timeout: 20_000,
        message: "the short command must really exit successfully before metadata eligibility is tested",
      },
    ).toBe("exited");
    // Newest-created timestamps can tie; pin the opener so the unread fixture cannot be auto-opened first.
    await pinAutoSelect(page, open);
    await page.goto("/");
    await expect(row(page, open)).toBeVisible({ timeout: 20_000 });
    await row(page, open).locator(".session-row-open").click();
    await expect(row(page, open)).toHaveAttribute("data-session-selected", "true");
    await expect(row(page, unread).locator(".status-badge.idle.unseen")).toHaveText("Idle — new output", {
      timeout: 45_000,
    });
    await expect(row(page, ended).locator(".status-badge")).toContainText("Exited");
    for (const id of [unread, ended]) {
      await row(page, id).locator(".session-row-open").click({ modifiers: ["Control"] });
    }
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "2");

    await chooseStatus(page, "Idle");
    await expect(row(page, ended)).toHaveCount(0);
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "1");
    await chooseStatus(page, "All");
    await expect(row(page, ended)).toHaveAttribute("data-session-marked", "true");
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "2");
    await chooseStatus(page, "Idle");
    await expect(row(page, ended)).toHaveCount(0);
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "1");
    const pins: string[] = [];
    page.on("request", (request) => {
      if (request.method() === "PUT" && new URL(request.url()).pathname.endsWith("/pin")) pins.push(request.url());
    });
    const pinned = page.waitForResponse((response) =>
      response.request().method() === "PUT" &&
      new URL(response.url()).pathname === `/api/sessions/${encodeURIComponent(unread)}/pin`
    );
    await page.getByRole("button", { name: "Marked session actions", exact: true }).click();
    await page.getByRole("menu", { name: "Marked session actions", exact: true }).getByRole("menuitem", {
      name: /^Pin(?:$|\s)/,
    }).click();
    expect((await pinned).ok()).toBe(true);
    await expect(page.locator(".marked-bar")).toHaveCount(0);
    await expect(row(page, unread)).toHaveAttribute("data-session-pinned", "true");
    expect(pins).toHaveLength(1);
    const afterPin = await listSessions(request);
    expect(afterPin.sessions.find((one) => one.id === unread)?.pinned).toBe(true);
    expect(afterPin.sessions.find((one) => one.id === ended)?.pinned).toBe(false);

    await chooseStatus(page, "All");
    await expect(row(page, ended)).toBeVisible();
    for (const id of [unread, ended]) {
      await row(page, id).locator(".session-row-open").click({ modifiers: ["Control"] });
    }
    await page.getByRole("button", { name: "Marked session actions", exact: true }).click();
    const menu = page.getByRole("menu", { name: "Marked session actions", exact: true });
    await expect(menu.getByRole("menuitem", { name: /^Pin(?:$|\s)/ })).toBeVisible();
    await expect(menu.getByRole("menuitem", { name: /^Unpin(?:$|\s)/ })).toBeVisible();
    await expect(menu.getByRole("menuitem", { name: /^Mark unread(?:$|\s)/ })).toHaveCount(0);
    const seenWrites: { id: string; body: unknown }[] = [];
    const markedSeenPaths = new Set([unread, ended].map((id) => `/api/sessions/${encodeURIComponent(id)}/seen`));
    page.on("request", (request) => {
      const path = new URL(request.url()).pathname;
      if (request.method() === "PUT" && markedSeenPaths.has(path)) {
        seenWrites.push({ id: path, body: request.postDataJSON() });
      }
    });
    const seen = page.waitForResponse((response) =>
      response.request().method() === "PUT" &&
      new URL(response.url()).pathname === `/api/sessions/${encodeURIComponent(unread)}/seen`
    );
    await menu.getByRole("menuitem", { name: /^Mark read(?:$|\s)/ }).click();
    expect((await seen).ok()).toBe(true);
    await expect(page.locator(".marked-bar")).toHaveCount(0);
    await expect(row(page, unread).locator(".status-badge.idle.unseen")).toHaveCount(0);
    expect(seenWrites).toHaveLength(1);
    expect(seenWrites[0].id).toBe(`/api/sessions/${encodeURIComponent(unread)}/seen`);
    expect(seenWrites[0].body).toMatchObject({ seen_activity_at: expect.any(Number) });
    const afterSeen = await listSessions(request);
    expect(afterSeen.sessions.find((one) => one.id === unread)?.seen_activity_at).toEqual(expect.any(Number));
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["preferences", () => resetPreferences(request)],
      ...created.reverse().map((
        id,
      ): [string, () => Promise<void>] => [`session ${id}`, () => cleanupSession(request, id)]),
    ]);
  }
});

/** Mark explicit owned references, preserving the open session as the range anchor. */
async function markRows(page: Page, ids: string[]) {
  for (const id of ids) await row(page, id).locator(".session-row-open").click({ modifiers: ["Control"] });
  await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", String(ids.length));
}

/** An already open row question must refuse just that target. Activating the
 * toolbar programmatically intentionally avoids a pointer's outside-dismissal;
 * the question is asserted first, so a missing engagement cannot pass this test. */
test("bulk Stop reports an engaged row without skipping its eligible sibling", async ({ page, request }) => {
  const ids: string[] = [];
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    for (let n = 0; n < 2; n++) {
      ids.push((await createSession(request, { title: `bulk-engaged-${n}-${Date.now()}` })).id);
    }
    await pinAutoSelect(page, ids[0]);
    await page.goto("/");
    for (const id of ids) await establishStarted(page, id);
    await markRows(page, ids);
    await openRowMenu(row(page, ids[0]));
    await row(page, ids[0]).locator(".session-row-delete").click();
    await expect(row(page, ids[0]).locator(".confirm-delete")).toBeVisible();
    const sent: string[] = [];
    page.on("request", (candidate) => {
      const path = new URL(candidate.url()).pathname;
      if (candidate.method() === "POST" && path.endsWith("/stop")) sent.push(decodeURIComponent(path.split("/")[3]));
    });
    await page.locator(".marked-stop").evaluate((button: HTMLButtonElement) => button.click());
    await expect(row(page, ids[0]).locator(".action-error")).toContainText("Stop: not started");
    await expect(page.locator(".marked-bar")).toHaveCount(0, { timeout: 30_000 });
    expect(sent).toEqual([ids[1]]);
    await expect(row(page, ids[1]).locator(".status-badge")).toContainText("Exited");
    expect((await listSessions(request)).sessions.find((entry) => entry.id === ids[0])?.status?.state).toMatch(
      /^(running|waiting|idle)$/,
    );
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/** The ordinary Delete shortcut must keep its open-session target while the
 * marked set names only another session. Rename has the same target distinction. */
test("session shortcuts keep acting on the open session while another is marked", async ({ page, request }) => {
  const ids: string[] = [];
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    for (let n = 0; n < 2; n++) {
      ids.push((await createSession(request, { title: `bulk-shortcut-${n}-${Date.now()}` })).id);
    }
    await pinAutoSelect(page, ids[0]);
    await page.goto("/");
    await establishStarted(page, ids[0]);
    await markRows(page, [ids[1]]);
    const input = terminalInput(page);
    await input.focus();
    await expect(input).toBeFocused();
    await page.keyboard.press("Control+Shift+Backspace");
    await expect(page.locator(".header-delete-confirm")).toBeVisible();
    await expect(page.locator(".bulk-delete-dialog")).toHaveCount(0);
    await expect(row(page, ids[0])).toHaveAttribute("data-session-selected", "true");
    await page.keyboard.press("Escape");
    await expect(page.locator(".header-delete-confirm")).toHaveCount(0);
    await expect(input).toBeFocused();
    await page.keyboard.press("Control+Shift+l");
    const rename = page.getByRole("dialog", { name: "Rename session", exact: true });
    await expect(rename).toBeVisible();
    await expect(rename.getByRole("textbox", { name: "New session title", exact: true })).toHaveValue(
      (await listSessions(request)).sessions.find((entry) => entry.id === ids[0])!.title,
    );
    await page.keyboard.press("Escape");
    await expect(rename).toHaveCount(0);
    await expect(input).toBeFocused();
    const marker = `bulk-shortcut-alive-${Date.now()}`;
    await page.keyboard.type(marker);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${marker}`);
    await expect(row(page, ids[1])).toHaveAttribute("data-session-marked", "true");
    expect((await listSessions(request)).sessions.filter((entry) => ids.includes(entry.id))).toHaveLength(2);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/** Controlled listings make disappearance and changed wording deterministic.
 * They retain real owned references, while only destructive replies are refused.
 * Native token tests separately prove that the replaced opening rejects an old
 * queued answer; this browser case proves the actual modal redraw and wire guard. */
test(
  "bulk Delete rewords and shrinks its structured question without gaining targets",
  async ({ page, request }, info) => {
    const ids: string[] = [];
    let bodyFailed = false;
    await resetPreferences(request);
    try {
      for (let n = 0; n < 3; n++) {
        ids.push(
          (await createSession(request, { title: `bulk-drawn-${n}-${Date.now()}` })).id,
        );
      }
      await pinAutoSelect(page, ids[2]);
      const response = await request.get("/api/sessions");
      expect(response.ok()).toBe(true);
      const build = requireHelmBuild(response, "bulk drawn consent");
      const listing = await response.json();
      const sources = ids.map((id) => listing.sessions.find((entry: { id: string }) => entry.id === id));
      expect(sources.every(Boolean)).toBe(true);
      const fullHost = `a-very-long-host-alias (destination-with-a-long-name.example.test)`;
      const hostileTitle = "long title <b>literal</b> \u202e" + "readable-name-".repeat(8);
      let stage = 0;
      await page.route((url) => url.pathname === "/api/sessions", async (route) => {
        if (route.request().method() !== "GET") return route.fallback();
        const first = {
          ...sources[0],
          title: hostileTitle,
          host_name: fullHost,
          status: { state: "unknown" },
          stale: false,
          working_copy: {
            id: "drawing-only",
            repo: { owner: "example", name: "repo" },
            canonical_path: "/work/example",
            origin_session_id: "drawing-only",
          },
        };
        const second = {
          ...sources[1],
          host_name: fullHost,
          status: stage === 0 ? { state: "running" } : { state: "exited", exit_code: 0 },
          annotation: null,
          tabs: [],
        };
        const sessions = stage === 0 ? [first, second, sources[2]] : [second, sources[2]];
        await route.fulfill({
          headers: { "x-farhelm-build": build },
          json: { ...listing, sessions, total: sessions.length, matching: sessions.length, truncated: false },
        });
      });
      await observeFeedReaders(page);
      const feed = await stubFeed(page);
      feed.notifyOnConnect(1);
      await page.addInitScript(() => localStorage.setItem("farhelm.sidebar-width", "240"));
      await page.goto("/");
      await feed.waitForConnection(1);
      await waitForFeedReadersSettled(page);
      await markRows(page, ids.slice(0, 2));
      for (const selector of [".marked-stop", ".marked-delete"]) {
        await expect(page.locator(selector)).toHaveAttribute("data-tooltip", /marked sessions currently shown/);
      }
      await expect.poll(() =>
        page.locator(".app-sidebar").evaluate((element) => parseFloat(getComputedStyle(element).width))
      ).toBe(296);
      for (const palette of ["dark", "light"] as const) {
        await page.emulateMedia({ colorScheme: palette });
        await expect(page.locator("html")).toHaveAttribute("data-appearance", palette);
        const bar = (await page.locator(".marked-bar").boundingBox())!;
        for (
          const selector of [".marked-count", ".marked-stop", ".marked-delete", ".marked-menu-toggle", ".marked-clear"]
        ) {
          const box = (await page.locator(selector).boundingBox())!;
          expect(box.x).toBeGreaterThanOrEqual(bar.x);
          expect(box.x + box.width).toBeLessThanOrEqual(bar.x + bar.width + 1);
        }
        await info.attach(`bulk-narrow-sidebar-${palette}`, {
          body: await page.locator(".app-sidebar").screenshot(),
          contentType: "image/png",
        });
      }
      await page.locator(".marked-delete").click();
      const dialog = page.locator(".bulk-delete-dialog");
      await expect(dialog.locator(".bulk-delete-row")).toHaveCount(2);
      for (const selector of [".dialog-close", ".bulk-delete-cancel", ".bulk-delete-confirm"]) {
        await expect(dialog.locator(selector)).toHaveAttribute("data-tooltip", /\S/);
      }
      await expect(dialog.locator(".status-badge.unknown")).toHaveText("Unknown");
      // Text content alone passed while flex shrinking painted only "U…".
      // The destructive question must keep its uncertainty label readable.
      await expect.poll(() =>
        dialog.locator(".status-badge.unknown").evaluate((element) => element.scrollWidth <= element.clientWidth)
      ).toBe(true);
      await expect(dialog.locator(".bulk-delete-title").first()).toContainText("<b>literal</b> <U+202E>");
      await expect(dialog.locator(".bulk-delete-title b")).toHaveCount(0);
      await expect(dialog.locator(".bulk-delete-host").first()).toHaveText(fullHost);
      await expect(dialog.locator(".bulk-delete-consequence").filter({ hasText: "The managed checkout stays" }))
        .toHaveCount(1);
      for (const palette of ["dark", "light"] as const) {
        await page.emulateMedia({ colorScheme: palette });
        await expect(page.locator("html")).toHaveAttribute("data-appearance", palette);
        await info.attach(`bulk-delete-dialog-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });
      }
      await page.setViewportSize({ width: 380, height: 640 });
      await expect(dialog).toBeVisible();
      await expect.poll(() =>
        dialog.locator(".status-badge.unknown").evaluate((element) => element.scrollWidth <= element.clientWidth)
      ).toBe(true);
      expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
      await expect(dialog.locator(".bulk-delete-confirm")).toBeInViewport();
      await info.attach("bulk-delete-dialog-narrow", { body: await dialog.screenshot(), contentType: "image/png" });
      await dialog.locator(".bulk-delete-cancel").click();
      await expect(dialog).toHaveCount(0);
      await expect(page.locator(".marked-menu-toggle")).toBeFocused();
      await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "2");
      await page.locator(".marked-delete").click();
      await expect(dialog.locator(".bulk-delete-row")).toHaveCount(2);
      stage = 1;
      feed.notify(2);
      await expect(dialog.locator(".bulk-delete-row")).toHaveCount(1);
      await expect(dialog.locator(`[data-delete-target="${ids[1]}"] .bulk-delete-consequence`)).toHaveText(
        "Delete anyway:",
      );
      await expect(dialog.locator(`[data-delete-target="${ids[2]}"]`)).toHaveCount(0);
      const sent: URL[] = [];
      await page.route(
        (url) => ids.some((id) => url.pathname === `/api/sessions/${encodeURIComponent(id)}`),
        async (route) => {
          if (route.request().method() !== "DELETE") return route.fallback();
          sent.push(new URL(route.request().url()));
          await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "drawn guard fixture" });
        },
      );
      await dialog.locator(".bulk-delete-confirm").click();
      await expect(dialog).toHaveCount(0);
      await expect(row(page, ids[1]).locator(".action-error")).toContainText("drawn guard fixture");
      expect(sent).toHaveLength(1);
      expect(decodeURIComponent(sent[0].pathname.split("/")[3])).toBe(ids[1]);
      expect(sent[0].searchParams.get("only_if_nothing_alive")).toBe("true");
      await expect(page.locator(".marked-bar")).toHaveCount(0);
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["routes", () => page.unrouteAll({ behavior: "wait" })],
        ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
        ["preferences", () => resetPreferences(request)],
      ]);
    }
  },
);

/** Creation's provisional live status is insufficient for a Stop fixture.
 * Observe the actual fake agent through this page's own revealed attachment. */
async function establishStarted(page: Page, id: string) {
  const target = row(page, id);
  await expect(target).toBeVisible({ timeout: 20_000 });
  if (await target.getAttribute("data-session-selected") !== "true") await target.locator(".session-row-open").click();
  await waitForSessionRevealed(page, id);
  await waitForTermText(page, "FAKE-AGENT READY");
  await expect(target.locator(".status-badge")).toHaveText(/^(Running|Waiting|Idle(?: — new output)?)$/);
}

/** More than the supervisor's admission limit must succeed without a burst.
 * Holding the first request on each real host proves simultaneous host lanes;
 * counting each request through its server reply distinguishes dispatch pacing
 * from sequential result collection. Queued rows must already lock navigation. */
test("bulk Stop paces nine rows on one host while another host progresses", async ({ page, request }) => {
  test.setTimeout(180_000);
  const ids: string[] = [];
  const release = routeGate();
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    const hosts = await listHosts(request);
    const local = hosts.find((host) => host.kind === "local")!;
    const remote = hosts.find((host) => host.kind !== "local" && host.state.phase === "connected")!;
    expect(local, "local host is registered").toBeTruthy();
    expect(remote, "a second real connected host is required").toBeTruthy();
    const hostById = new Map<string, number>();
    for (let n = 0; n < 10; n++) {
      const host = n < 9 ? local.id : remote.id;
      const created = await createSession(request, { title: `bulk-stop-${n}-${Date.now()}`, host });
      ids.push(created.id);
      hostById.set(created.id, host);
    }
    await pinAutoSelect(page, ids[0]);
    await page.goto("/");
    for (const id of ids) await establishStarted(page, id);
    await markRows(page, ids);
    const drawn = await markedIds(page);
    const started: string[] = [];
    const active = new Map<number, number>();
    const maximum = new Map<number, number>();
    let globalMaximum = 0;
    const replies: number[] = [];
    await page.route((url) => /^\/api\/sessions\/[^/]+\/stop$/.test(url.pathname), async (route) => {
      const id = decodeURIComponent(new URL(route.request().url()).pathname.split("/")[3]);
      const host = hostById.get(id);
      if (host === undefined) return route.fallback();
      expect(route.request().method()).toBe("POST");
      started.push(id);
      active.set(host, (active.get(host) ?? 0) + 1);
      maximum.set(host, Math.max(maximum.get(host) ?? 0, active.get(host)!));
      globalMaximum = Math.max(globalMaximum, [...active.values()].reduce((a, b) => a + b, 0));
      await release.wait();
      const reply = await route.fetch({ maxRetries: 0 });
      replies.push(reply.status());
      // The browser can send its next request before fulfill's promise returns.
      // The server reply is the end of this request's observable lifetime.
      active.set(host, active.get(host)! - 1);
      await route.fulfill({ response: reply });
    });
    await page.locator(".marked-stop").click();
    await expect.poll(() => started.length).toBe(2);
    expect(new Set(started.map((id) => hostById.get(id)))).toEqual(new Set([local.id, remote.id]));
    for (const id of ids) await expect(row(page, id).locator(".session-row-open")).toBeDisabled();
    for (const action of [".marked-stop", ".marked-delete", ".marked-menu-toggle", ".marked-clear"]) {
      await expect(page.locator(action)).toBeDisabled();
    }
    release.release();
    await expect(page.locator(".marked-bar")).toHaveCount(0, { timeout: 60_000 });
    expect(started).toHaveLength(10);
    expect(replies).toHaveLength(10);
    expect(replies.every((status) => status >= 200 && status < 300)).toBe(true);
    expect(maximum).toEqual(new Map([[local.id, 1], [remote.id, 1]]));
    expect(globalMaximum).toBe(2);
    for (const host of [local.id, remote.id]) {
      expect(started.filter((id) => hostById.get(id) === host)).toEqual(
        drawn.filter((id) => hostById.get(id) === host),
      );
    }
    await expect.poll(async () => {
      const listing = await listSessions(request);
      return ids.map((id) => listing.sessions.find((entry) => entry.id === id)?.status?.state);
    }, { timeout: 30_000 }).toEqual(ids.map(() => "exited"));
    for (const id of ids) await expect(row(page, id).locator(".action-error")).toHaveCount(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    release.release();
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/** A mixed question carries three different guards, and a fast host deleting
 * the open session must not select a queued target on a slower host. Keep the
 * second host held after the local success and local refusal have settled. */
test("bulk Delete keeps drawn guards, row errors and cross-host selection exclusion", async ({ page, request }) => {
  const ids: string[] = [];
  const localRelease = routeGate();
  const remoteRelease = routeGate();
  const selectionRelease = routeGate();
  const selectionReached = routeGate();
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    const hosts = await listHosts(request);
    const local = hosts.find((host) => host.kind === "local")!;
    const remote = hosts.find((host) => host.kind !== "local" && host.state.phase === "connected")!;
    expect(local).toBeTruthy();
    expect(remote).toBeTruthy();
    const live = (await createSession(request, { title: `a-bulk-delete-${Date.now()}`, host: local.id })).id;
    ids.push(live);
    const ended =
      (await createSession(request, { title: `b-bulk-delete-${Date.now()}`, host: remote.id, invocation: "true" })).id;
    ids.push(ended);
    const tabbed =
      (await createSession(request, { title: `c-bulk-delete-${Date.now()}`, host: local.id, invocation: "true" })).id;
    ids.push(tabbed);
    await expect.poll(async () => {
      const listing = await listSessions(request);
      return [ended, tabbed].map((id) => listing.sessions.find((entry) => entry.id === id)?.status?.state);
    }, { timeout: 20_000 }).toEqual(["exited", "exited"]);
    const openedTab = await request.post(`/api/sessions/${encodeURIComponent(tabbed)}/tabs`);
    expect(openedTab.ok(), await openedTab.text()).toBe(true);
    await pinAutoSelect(page, live);
    await page.goto("/");
    await establishStarted(page, live);
    await chooseSort(page, "title");
    await markRows(page, ids);
    const build = requireHelmBuild(await request.get("/api/hosts"), "bulk Delete refusal");
    const sent: URL[] = [];
    const newAttachments: string[] = [];
    page.on("websocket", (socket) => {
      const path = new URL(socket.url()).pathname;
      if (/\/term(?:\/unowned)?$/.test(path)) newAttachments.push(decodeURIComponent(path.split("/")[3]));
    });
    await page.route(
      (url) => ids.some((id) => url.pathname === `/api/sessions/${encodeURIComponent(id)}`),
      async (route) => {
        if (route.request().method() !== "DELETE") return route.fallback();
        const url = new URL(route.request().url());
        const id = decodeURIComponent(url.pathname.split("/")[3]);
        sent.push(url);
        await (id === ended ? remoteRelease : localRelease).wait();
        if (id === tabbed) {
          await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "bulk fixture refusal" });
        } else {
          const reply = await route.fetch({ maxRetries: 0 });
          expect(reply.ok(), await reply.text()).toBe(true);
          await route.fulfill({ response: reply });
        }
      },
    );
    await page.locator(".marked-delete").click();
    const dialog = page.locator(".bulk-delete-dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".bulk-delete-row")).toHaveCount(3);
    await expect(dialog.locator(`[data-delete-target="${live}"] .bulk-delete-consequence`)).toContainText(
      "Still running",
    );
    await expect(dialog.locator(`[data-delete-target="${ended}"] .bulk-delete-consequence`)).toHaveText(
      "Delete anyway:",
    );
    await expect(dialog.locator(`[data-delete-target="${tabbed}"] .bulk-delete-consequence`)).toContainText("1");
    await expect(dialog.locator(".bulk-delete-confirm")).toBeFocused();
    await dialog.locator(".bulk-delete-confirm").click();
    await expect(dialog).toHaveCount(0);
    await expect.poll(() => sent.length).toBe(2);
    await expect(page.locator(".marked-count")).toBeFocused();
    for (const action of [".marked-stop", ".marked-delete", ".marked-menu-toggle", ".marked-clear"]) {
      await expect(page.locator(action)).toBeDisabled();
    }
    localRelease.release();
    await expect(row(page, live)).toHaveCount(0, { timeout: 20_000 });
    await expect(row(page, tabbed).locator(".action-error")).toContainText("Delete: bulk fixture refusal");
    await expect(page.locator('.session-row[data-session-selected="true"]')).toHaveCount(0);
    await expect(row(page, ended)).toBeVisible();
    await expect(row(page, ended).locator(".session-row-open")).toBeDisabled();
    expect(newAttachments.filter((id) => ids.includes(id)), "no still-deleting target may be opened").toEqual([]);
    await expect(page.locator(".marked-bar")).toBeVisible();
    // Isolate the toolbar's completion handback from the later legitimate
    // focus handoff when automatic selection reveals a surviving terminal.
    await page.route((url) => url.pathname === "/api/preferences", async (route) => {
      if (route.request().method() !== "GET") return route.fallback();
      selectionReached.release();
      await selectionRelease.wait();
      await route.continue();
    });
    remoteRelease.release();
    await expect(row(page, ended)).toHaveCount(0, { timeout: 20_000 });
    await expect(page.locator(".marked-bar")).toHaveCount(0);
    await selectionReached.wait();
    await expect.poll(() => page.evaluate(() => document.activeElement?.outerHTML.slice(0, 500)), {
      message: "completion returns toolbar focus before the held automatic-selection read finishes",
    }).toContain("sort-title");
    selectionRelease.release();
    expect(sent).toHaveLength(3);
    for (
      const [id, guard] of [[live, null], [ended, "only_if_nothing_alive"], [tabbed, "only_if_agent_ended"]] as const
    ) {
      const url = sent.find((url) => decodeURIComponent(url.pathname.split("/")[3]) === id)!;
      expect(url, "each drawn target got a request").toBeTruthy();
      expect(url.searchParams.get("only_if_nothing_alive")).toBe(guard === "only_if_nothing_alive" ? "true" : null);
      expect(url.searchParams.get("only_if_agent_ended")).toBe(guard === "only_if_agent_ended" ? "true" : null);
    }
    await expect(row(page, tabbed)).toBeVisible();
    await expect(row(page, tabbed).locator(".action-error")).toContainText("bulk fixture refusal");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    localRelease.release();
    remoteRelease.release();
    selectionRelease.release();
    await cleanupOwnedSteps(bodyFailed, [
      ["routes", () => page.unrouteAll({ behavior: "wait" })],
      ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/** Direct batches must preserve narrower consent even when the setting skips
 * the question. A held request makes the absence of a transient dialog visible. */
for (const skip of [false, true]) {
  test(`bulk Delete ${skip ? "skips the live question" : "directly removes ended rows"}`, async ({ page, request }) => {
    const ids: string[] = [];
    const release = routeGate();
    let bodyFailed = false;
    await resetPreferences(request);
    try {
      await patchPreferences(request, { skip_session_confirmations: skip });
      for (let n = 0; n < 2; n++) {
        ids.push(
          (await createSession(request, {
            title: `bulk-direct-${n}-${Date.now()}`,
            ...(skip ? {} : { invocation: "true" }),
          })).id,
        );
      }
      if (!skip) {
        await expect.poll(async () => {
          const listing = await listSessions(request);
          return ids.map((id) => listing.sessions.find((entry) => entry.id === id)?.status?.state);
        }, { timeout: 20_000 }).toEqual(["exited", "exited"]);
      }
      await pinAutoSelect(page, ids[0]);
      await page.goto("/");
      if (skip) { for (const id of ids) await establishStarted(page, id); }
      await markRows(page, ids);
      const sent: URL[] = [];
      await page.route(
        (url) => ids.some((id) => url.pathname === `/api/sessions/${encodeURIComponent(id)}`),
        async (route) => {
          if (route.request().method() !== "DELETE") return route.fallback();
          sent.push(new URL(route.request().url()));
          await release.wait();
          await route.continue();
        },
      );
      await page.locator(".marked-delete").click();
      await expect.poll(() => sent.length).toBe(1);
      await expect(page.locator(".bulk-delete-dialog")).toHaveCount(0);
      await expect(page.locator(".marked-delete")).toBeDisabled();
      release.release();
      await expect(page.locator(".marked-bar")).toHaveCount(0, { timeout: 30_000 });
      expect(sent).toHaveLength(2);
      for (const url of sent) expect(url.searchParams.get("only_if_nothing_alive")).toBe(skip ? null : "true");
      for (const id of ids) await expect(row(page, id)).toHaveCount(0);
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      release.release();
      await cleanupOwnedSteps(bodyFailed, [
        ["routes", () => page.unrouteAll({ behavior: "wait" })],
        ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
        ["preferences", () => resetPreferences(request)],
      ]);
    }
  });
}
