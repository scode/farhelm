import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { drainedRoute } from "./helpers/drained-route";
import {
  cleanupSession,
  createSession,
  observeFeedReaders,
  openRowMenu,
  pinAutoSelect,
  resetPreferences,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { type InputFrame, programInput, recordInput } from "./helpers/program-input";
import { routeGate } from "./helpers/route-gate";
import { attachSession, terminalInput, waitForTermText } from "./helpers/term";
import { waitForSessionReady } from "./helpers/terminal-readiness";

/** Keep routing identity independent of the user-facing title and status. */
function row(page: Page, id: string): Locator {
  return page.locator(`.session-row[data-session-id="${id}"]`);
}

/** Observe input only on the bystander's actual terminal attachment.
 * A focused textarea alone cannot prove that the next Enter escaped the
 * newly arrived confirmation and reached the intended session's socket. */
function recordSessionInput(page: Page, id: string): InputFrame[] {
  const frames: InputFrame[] = [];
  page.on("websocket", (socket) => {
    if (new URL(socket.url()).pathname === `/api/sessions/${encodeURIComponent(id)}/term`) {
      recordInput(socket, frames);
    }
  });
  return frames;
}

/** Cross the mounted reply's deferred focus boundary before testing the next key.
 * Two frames leave room for the mounted effect and its own scheduled handoff.
 * This observes scheduling; the caller establishes attachment readiness. */
async function settleMountedFocus(page: Page): Promise<void> {
  await page.evaluate(() => new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  }));
}

/** An inline Replace question is an asynchronous reply, not a new modal.
 * Its safe default may recover document focus, but it must not take Enter
 * from another session's live terminal merely because the question arrived. */
for (const focus of ["terminal", "document"] as const) {
  test(`row Replace YOLO arrival respects ${focus} focus`, async ({ page, request }) => {
    const ids: string[] = [];
    const held = routeGate();
    let disposeRoute: (() => Promise<void>) | undefined;
    let bodyFailed = false;
    await resetPreferences(request);
    try {
      await page.setViewportSize({ width: 1280, height: 600 });
      const source = await createSession(request, { title: `completion-yolo-source-${focus}-${Date.now()}` });
      ids.push(source.id);
      const other = await createSession(request, { title: `completion-yolo-other-${focus}-${Date.now()}` });
      ids.push(other.id);
      const frames = recordSessionInput(page, other.id);
      const build = requireHelmBuild(await request.get("/api/hosts"), "inline Replace completion");
      let requestHeld = false;
      disposeRoute = await drainedRoute(page, (url) => url.pathname === `/api/sessions/${encodeURIComponent(source.id)}/replace`, async (route) => {
        expect(route.request().method()).toBe("POST");
        requestHeld = true;
        await held.wait();
        await route.fulfill({
          status: 409,
          headers: { "x-farhelm-build": build, "x-farhelm-yolo-confirmation": "confirmation-required" },
          body: "fixture asks for YOLO confirmation",
        });
      });
      await pinAutoSelect(page, other.id);
      await page.goto("/");
      await attachSession(page, other.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await waitForTermText(page, "> ");
      await openRowMenu(row(page, source.id));
      await row(page, source.id).locator(".session-row-replace").click();
      await row(page, source.id).locator(".confirm-replace").click();
      await expect.poll(() => requestHeld, { message: "Replace remains unanswered while the user moves focus" }).toBe(true);
      await expect(page.locator(".yolo-confirmation")).toHaveCount(0);
      await terminalInput(page).focus();
      await waitForSessionReady(page, other.id);
      if (focus === "terminal") {
        frames.length = 0;
        await page.keyboard.type("completion-yolo-input");
        await expect.poll(() => programInput(frames)).toBe("completion-yolo-input");
      }
      const sidebarScroll = await page.locator(".sidebar-content").evaluate((sidebar) => sidebar.scrollTop);
      if (focus === "document") {
        await terminalInput(page).evaluate((input: HTMLElement) => input.blur());
        await expect.poll(() => page.evaluate(() => document.activeElement === document.body)).toBe(true);
      }
      held.release();
      const question = page.locator(".yolo-confirmation");
      await expect(question).toBeVisible();
      if (focus === "document") {
        await expect(question.locator(".yolo-cancel")).toBeFocused();
      } else {
        await settleMountedFocus(page);
        await expect(terminalInput(page)).toBeFocused();
        await expect(question.locator(".yolo-cancel")).not.toBeFocused();
        expect(await page.locator(".sidebar-content").evaluate((sidebar) => sidebar.scrollTop),
          "the asynchronous inline question must not scroll the user's sidebar").toBe(sidebarScroll);
        frames.length = 0;
        await page.keyboard.press("Enter");
        await expect.poll(() => programInput(frames), { message: "the next Enter reaches the bystander's terminal" }).toBe("\r");
        await waitForTermText(page, "echo:completion-yolo-input");
        await expect(question).toBeVisible();
      }
      await expect(row(page, other.id)).toHaveAttribute("data-session-selected", "true");
      await question.locator(".yolo-cancel").click();
      await expect(question).toHaveCount(0);
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      held.release();
      await cleanupOwnedSteps(bodyFailed, [
        ["Replace route", async () => { await disposeRoute?.(); }],
        ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
        ["preferences", () => resetPreferences(request)],
      ]);
    }
  });
}

/** Stop's own reply settles its claim and partial notice immediately.
 * A separately held status refresh must not keep Clone disabled, even when
 * the old listing still describes the agent as alive. The Clone click proves
 * handler admission as well as the visible enabled state. */
test("Stop settles before its held status refresh", async ({ page, request }) => {
  const ids: string[] = [];
  const stopReply = routeGate();
  const statusReply = routeGate();
  const disposers: (() => Promise<void>)[] = [];
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    const source = await createSession(request, { title: `completion-stop-${Date.now()}` });
    ids.push(source.id);
    const build = requireHelmBuild(await request.get("/api/hosts"), "Stop settlement");
    await observeFeedReaders(page);
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await waitForFeedReadersSettled(page);
    let stopHeld = false;
    let holdStatus = false;
    let statusHeld = false;
    let statusReturned = false;
    disposers.push(await drainedRoute(page, (url) => url.pathname === `/api/sessions/${encodeURIComponent(source.id)}/stop`, async (route) => {
      const response = await route.fetch({ maxRetries: 0 });
      expect(response.ok(), await response.text()).toBe(true);
      stopHeld = true;
      await stopReply.wait();
      await route.fulfill({
        status: 200,
        headers: { "x-farhelm-build": build, "content-type": "application/json" },
        json: { notice: "Terminal 2 could not be closed: fixture partial Stop" },
      });
    }));
    disposers.push(await drainedRoute(page, (url) => url.pathname === "/api/sessions", async (route) => {
      if (!holdStatus) return route.fallback();
      statusHeld = true;
      await statusReply.wait();
      const response = await route.fetch({ maxRetries: 0 });
      await route.fulfill({ response });
      statusReturned = true;
    }));
    await openRowMenu(row(page, source.id));
    await row(page, source.id).locator(".session-row-stop").click();
    await expect.poll(() => stopHeld, { message: "the real Stop has completed but its reply is still held" }).toBe(true);
    await expect(row(page, source.id).locator(".stop-progress")).toHaveText("Stopping…");
    holdStatus = true;
    stopReply.release();
    await expect.poll(() => statusHeld, { message: "Stop's post-reply status read is held independently" }).toBe(true);
    await expect(row(page, source.id).locator(".stop-progress")).toHaveCount(0);
    await expect(row(page, source.id).locator(".action-error")).toContainText("fixture partial Stop");
    await expect(page.getByRole("button", { name: "New terminal", exact: true })).toBeEnabled();
    await openRowMenu(row(page, source.id));
    const clone = row(page, source.id).locator(".session-row-clone");
    await expect(clone).not.toHaveAttribute("aria-disabled", "true");
    await clone.click();
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    expect(statusReturned, "the session claim is released before the status read returns").toBe(false);
    await form.locator(".launch-composer-cancel").click();
    statusReply.release();
    await expect.poll(() => statusReturned).toBe(true);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    stopReply.release();
    statusReply.release();
    await cleanupOwnedSteps(bodyFailed, [
      ...disposers.map((dispose): [string, () => Promise<void>] => ["held route", dispose]),
      ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});

/** Clearing the batch's marks is still required when all its work settles.
 * That asynchronous toolbar removal must not reclaim focus from the session
 * the user is typing in, including the next key after the batch finishes. */
test("bulk Stop completion leaves the bystander terminal focused", async ({ page, request }) => {
  const ids: string[] = [];
  const held = routeGate();
  let disposeRoute: (() => Promise<void>) | undefined;
  let bodyFailed = false;
  await resetPreferences(request);
  try {
    const source = await createSession(request, { title: `completion-bulk-source-${Date.now()}` });
    ids.push(source.id);
    const other = await createSession(request, { title: `completion-bulk-other-${Date.now()}` });
    ids.push(other.id);
    const frames = recordSessionInput(page, other.id);
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await attachSession(page, other.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await waitForTermText(page, "> ");
    let replyHeld = false;
    disposeRoute = await drainedRoute(page, (url) => url.pathname === `/api/sessions/${encodeURIComponent(source.id)}/stop`, async (route) => {
      const response = await route.fetch({ maxRetries: 0 });
      expect(response.ok(), await response.text()).toBe(true);
      replyHeld = true;
      await held.wait();
      await route.fulfill({ response });
    });
    await row(page, source.id).locator(".session-row-open").click({ modifiers: ["Control"] });
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "1");
    await page.locator(".marked-stop").click();
    await expect.poll(() => replyHeld, { message: "the accepted bulk Stop is still waiting on its reply" }).toBe(true);
    await terminalInput(page).focus();
    await waitForSessionReady(page, other.id);
    frames.length = 0;
    await page.keyboard.type("completion-bulk-input");
    await expect.poll(() => programInput(frames)).toBe("completion-bulk-input");
    await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "1");
    held.release();
    await expect(page.locator(".marked-bar")).toHaveCount(0);
    await expect(row(page, source.id).locator(".stop-progress")).toHaveCount(0);
    await settleMountedFocus(page);
    await expect(terminalInput(page)).toBeFocused();
    frames.length = 0;
    await page.keyboard.press("Enter");
    await expect.poll(() => programInput(frames), { message: "the next key after bulk completion reaches the bystander" }).toBe("\r");
    await waitForTermText(page, "echo:completion-bulk-input");
    await expect(row(page, other.id)).toHaveAttribute("data-session-selected", "true");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    held.release();
    await cleanupOwnedSteps(bodyFailed, [
      ["Stop route", async () => { await disposeRoute?.(); }],
      ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
      ["preferences", () => resetPreferences(request)],
    ]);
  }
});
