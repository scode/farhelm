import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { chooseSort, chooseStatus } from "./helpers/list-header";
import { waitForSessionReady, waitForSessionRevealed } from "./helpers/terminal-readiness";
import { type InputFrame, programInput, recordInput } from "./helpers/program-input";
import { requireHelmBuild } from "./helpers/helm-build";
import {
  cleanupSession,
  createSession,
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
