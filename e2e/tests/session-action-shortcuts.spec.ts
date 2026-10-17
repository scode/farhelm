/** Delete shortcuts must preserve header consent and terminal input ownership. */
import { expect, test } from "./helpers/evidence";
import type { Page } from "@playwright/test";
import { cleanupSession, createSession, listSessions, patchPreferences, readPreferences } from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { installTerminalSuiteHooks, LIVE_BADGE } from "./helpers/terminal-suite";
import { attachSession, waitForTermText } from "./helpers/term";

installTerminalSuiteHooks();

/** Establish live bytes through this page's own revealed, focused attachment.
 * The fixture's processed echo distinguishes readiness from replayed output or
 * the PTY's echo of input that the agent has not handled yet.
 */
async function openLiveSession(page: Page, id: string) {
  await page.goto("/");
  await attachSession(page, id);
  await waitForTermText(page, "FAKE-AGENT READY");
  await waitForTermText(page, "> ");
  const terminal = page.locator("#terminal .xterm-helper-textarea");
  await expect(terminal).toBeFocused();
  const marker = `shortcut-live-${Date.now()}`;
  await page.keyboard.type(marker);
  await page.keyboard.press("Enter");
  await waitForTermText(page, `echo:${marker}`);
  await expect(page.locator(".titlebar .status-badge")).toHaveText(LIVE_BADGE);
  return terminal;
}

/** The chord must open the same question as the header without sending bytes.
 * A live echo after Escape proves the handoff returns to the existing agent;
 * binary frames catch a shortcut that opens a question after also reaching xterm.
 */
test("browser Delete chord shares header confirmation and restores terminal focus without forwarding input", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const session = await createSession(request, { title: `delete-shortcut-confirm-${Date.now()}` });
  const binary: Buffer[] = [];
  const deletes: string[] = [];
  page.on("websocket", (socket) => {
    if (new URL(socket.url()).pathname !== `/api/sessions/${session.id}/term`) return;
    socket.on("framesent", ({ payload }) => { if (Buffer.isBuffer(payload)) binary.push(payload); });
  });
  page.on("request", (candidate) => {
    if (candidate.method() === "DELETE" && new URL(candidate.url()).pathname === `/api/sessions/${session.id}`) {
      deletes.push(candidate.url());
    }
  });
  try {
    await patchPreferences(request, { skip_session_confirmations: null });
    const terminal = await openLiveSession(page, session.id);
    expect(binary.length, "the input observer must see this attachment's live probe").toBeGreaterThan(0);
    binary.length = 0;
    const prompt = page.locator(".header-delete-confirm");
    await page.locator(".header-delete").click();
    await expect(prompt).toBeVisible();
    const consequence = await prompt.locator(".confirm-consequence").allTextContents();
    expect(consequence.length, "the header question must state the deletion consequence").toBeGreaterThan(0);
    await expect(prompt.locator(".btn-danger")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(prompt).toHaveCount(0);
    await expect(terminal).toBeFocused();

    await page.keyboard.press("Control+Shift+Backspace");
    await expect(prompt).toBeVisible();
    await expect(prompt.locator(".confirm-consequence")).toHaveText(consequence);
    await expect(prompt.locator(".btn-danger")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(prompt).toHaveCount(0);
    await expect(terminal).toBeFocused();
    const marker = `delete-escape-restored-${Date.now()}`;
    await page.keyboard.type(marker);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${marker}`);
    expect(Buffer.concat(binary).toString()).toBe(`${marker}\r`);
    expect(deletes, "both unanswered questions must leave the session intact").toEqual([]);
    expect((await listSessions(request)).sessions.some((entry) => entry.id === session.id)).toBe(true);
  } finally {
    try {
      await cleanupSession(request, session.id);
    } finally {
      await patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null });
    }
  }
});

/** Skipping questions must take the real header deletion path immediately.
 * Hold the DELETE before its reply so a transient confirmation cannot hide
 * behind the selected session disappearing after a successful deletion.
 */
test("browser Delete chord honors skipped confirmations and deletes the open session", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const session = await createSession(request, { title: `delete-shortcut-direct-${Date.now()}` });
  const releaseDelete = routeGate();
  const deleteReached = routeGate();
  const matches = (url: URL) => url.pathname === `/api/sessions/${session.id}`;
  await page.route(matches, async (route) => {
    if (route.request().method() !== "DELETE") return route.fallback();
    deleteReached.release();
    await releaseDelete.wait();
    await route.continue();
  });
  try {
    await patchPreferences(request, { skip_session_confirmations: true });
    const terminal = await openLiveSession(page, session.id);
    await expect(page.locator(".header-delete")).toBeEnabled();
    await expect(terminal).toBeFocused();
    const deleted = page.waitForResponse((reply) => reply.request().method() === "DELETE" && matches(new URL(reply.url())));
    await page.keyboard.press("Control+Shift+Backspace");
    await deleteReached.wait();
    await expect(page.locator(`.session-row[data-session-id="${session.id}"]`)).toBeVisible();
    await expect(page.locator(".header-delete-confirm")).toHaveCount(0);
    releaseDelete.release();
    const reply = await deleted;
    expect(reply.ok(), await reply.text()).toBe(true);
    await expect(page.locator(`.session-row[data-session-id="${session.id}"]`)).toHaveCount(0);
    expect((await listSessions(request)).sessions.some((entry) => entry.id === session.id)).toBe(false);
  } finally {
    releaseDelete.release();
    try {
      await page.unrouteAll({ behavior: "wait" });
    } finally {
      try {
        await cleanupSession(request, session.id);
      } finally {
        await patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null });
      }
    }
  }
});

/** Modal controls and ordinary text fields keep their editing chord.
 * The standalone input isolates the text-field rule from the modal rule:
 * the session's rename and create editors are modal, so either could pass
 * even if the shortcut stopped protecting ordinary editable fields.
 */
test("browser Delete chord yields to a visible modal and an ordinary text field", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const session = await createSession(request, { title: `delete-shortcut-yields-${Date.now()}` });
  const deletes: string[] = [];
  page.on("request", (candidate) => {
    if (candidate.method() === "DELETE" && new URL(candidate.url()).pathname === `/api/sessions/${session.id}`) {
      deletes.push(candidate.url());
    }
  });
  try {
    await patchPreferences(request, { skip_session_confirmations: null });
    const terminal = await openLiveSession(page, session.id);
    await page.getByRole("button", { name: "settings", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    await expect(dialog).toBeVisible();
    const close = dialog.getByRole("button", { name: "close", exact: true });
    await close.focus();
    await expect(close).toBeFocused();
    await page.keyboard.press("Control+Shift+Backspace");
    await expect(dialog).toBeVisible();
    await expect(close).toBeFocused();
    await expect(page.locator(".header-delete-confirm")).toHaveCount(0);
    await close.click();
    await expect(dialog).toHaveCount(0);

    await page.evaluate(() => {
      const input = document.createElement("input");
      input.setAttribute("aria-label", "shortcut editing fixture");
      input.addEventListener("keydown", (event) => {
        if (event.code === "Backspace" && event.ctrlKey && event.shiftKey) {
          input.dataset.chordDefaultPrevented = String(event.defaultPrevented);
        }
      });
      document.body.append(input);
    });
    const input = page.getByRole("textbox", { name: "shortcut editing fixture", exact: true });
    await input.fill("keep editing");
    await expect(input).toBeFocused();
    await page.keyboard.press("Control+Shift+Backspace");
    await expect(input).toHaveAttribute("data-chord-default-prevented", "false");
    await expect(input).toBeFocused();
    await expect(page.locator(".header-delete-confirm")).toHaveCount(0);
    await input.evaluate((node) => node.remove());

    await terminal.focus();
    await expect(terminal).toBeFocused();
    const marker = `delete-yield-survived-${Date.now()}`;
    await page.keyboard.type(marker);
    await page.keyboard.press("Enter");
    await waitForTermText(page, `echo:${marker}`);
    expect(deletes, "yielding must send no session deletion").toEqual([]);
  } finally {
    try {
      await cleanupSession(request, session.id);
    } finally {
      await patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null });
    }
  }
});
