// Shared terminal-island test surface: only stable cross-spec helpers live
// here. Divergent one-off helpers remain local to the specs that need them.

import { expect, type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { waitForSessionReady } from "./terminal-readiness";
import { recordPage } from "./timeline";

/**
 * Restart the open session from its header while its agent reads idle, so
 * the click restarts directly instead of opening the confirmation.
 *
 * Restart asks for confirmation only while the agent is working (SPEC.md,
 * Lifecycle operations). The fixture agents these specs launch go quiet
 * after printing, and the activity sampler reads a quiet screen as idle only
 * after several samples, so a spec that restarts as a step toward something
 * else waits for that reading first: clicking earlier can still find the
 * agent reading working and open the prompt instead. The badge is the
 * oracle rather than `data-confirms` alone, because the create reply's
 * `Unknown` placeholder also leaves `data-confirms` false. Pass
 * `afterInput` when the spec typed into the agent just before; see the
 * option's comment below.
 */
export async function restartIdleAgent(page: Page, options: { afterInput?: boolean } = {}): Promise<void> {
  const badge = page.locator(".titlebar .status-badge");
  if (options.afterInput) {
    // A spec that has just typed into the agent may still see the idle
    // reading from BEFORE that input, while the supervisor has already
    // sampled the echo and reads working; restarting then is refused as
    // unconfirmed. The changed screen reads working for several samples,
    // so wait for that reading first, then for the idle one after it.
    await expect(badge).toHaveClass(/\brunning\b/, { timeout: 30_000 });
  }
  await expect(badge).toHaveClass(/\bidle\b/, { timeout: 30_000 });
  const restartButton = page.locator(".restart-primary");
  // Premise: Restart is available. It only ever resumes, and a page whose
  // copy says the session cannot resume turns the click into a silent no-op
  // that would only surface later as an unexplained timeout.
  await expect(restartButton).not.toHaveAttribute("aria-disabled", "true");
  await expect(restartButton).toHaveAttribute("data-confirms", "false");
  await restartButton.click();
}

/**
 * Read the complete terminal buffer, including scrollback rather than only
 * the rows xterm currently renders in the DOM.
 *
 * This is the terminal island's test-surface contract: callers may depend on
 * `window.__farhelmTerm` and `term.buffer.active` while the terminal tests
 * keep asserting replayed or off-screen output. It returns an empty string
 * before mounting, so polling callers can start before the island is ready.
 */
export async function termText(page: Page): Promise<string> {
  return page.evaluate(() => {
    const term = (window as any).__farhelmTerm;
    if (!term) return "";
    const buf = term.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < buf.length; i++) {
      lines.push(buf.getLine(i)?.translateToString(true) ?? "");
    }
    return lines.join("\n");
  });
}

/**
 * Poll the terminal buffer until `needle` arrives.
 *
 * Terminal output arrives asynchronously over a WebSocket with no DOM event
 * to await, so a one-shot buffer read cannot establish that output landed.
 * Search inside the page: transferring megabytes of scrollback on every
 * mismatch makes the observation itself expensive and floods failure reports.
 * The bounded tail and attachment state remain available when the wait fails;
 * containment still covers the complete buffer, including line separators.
 */
export async function waitForTermText(page: Page, needle: string, timeout = 15_000) {
  await expect
    .poll(() => page.evaluate((expected) => {
      const term = (window as any).__farhelmTerm;
      const lines: string[] = [];
      if (term) {
        const buffer = term.buffer.active;
        for (let i = 0; i < buffer.length; i++) {
          lines.push(buffer.getLine(i)?.translateToString(true) ?? "");
        }
      }
      const text = lines.join("\n");
      return {
        found: text.includes(expected),
        mounted: !!term,
        socketState: (window as any).__farhelmWs?.readyState ?? null,
        characters: text.length,
        tail: text.slice(-1024),
      };
    }, needle), { timeout, message: `waiting for ${needle}` })
    .toMatchObject({ found: true });
}

/**
 * Select a session and wait until its terminal can receive keyboard input.
 *
 * Opening a row is not enough: an island can exist while its socket is still
 * connecting or its replay remains hidden, and the row click must leave the
 * terminal's own focus intact for keyboard-driven callers. This does not
 * prove that a fixture agent reached a prompt; callers that need one keep
 * their own agent-specific witness. This ordinary fixture expects an
 * unfiltered session list and the initial focus handoff. Tests that already
 * moved focus into another control must choose a focus-neutral boundary.
 */
export async function attachSession(page: Page, id: string): Promise<void> {
  recordPage(page, "attach-session-intent", [["session", id]]);
  const target = page.locator(`[data-session-id="${id}"]`);
  await expect(target).toBeVisible({ timeout: 20_000 });
  // A fresh page auto-selects before any test action. Wait for that choice
  // to commit before deciding whether a click is needed: clicking the same
  // row after its terminal revealed would move focus back to the row, with
  // no new reveal to hand it to xterm again.
  await expect(page.locator('.session-row[data-session-selected="true"]')).toHaveCount(1, {
    timeout: 20_000,
  });
  if ((await target.getAttribute("data-session-selected")) !== "true") {
    recordPage(page, "attach-session-selection", [["session", id], ["branch", "open"]]);
    await target.locator(".session-row-open").click();
  } else {
    recordPage(page, "attach-session-selection", [["session", id], ["branch", "already-selected"]]);
  }
  await waitForSessionReady(page, id);
}

/**
 * Answer a command launch's required YOLO question in the launcher (`yolo`,
 * default no). The question has no default, so a command-mode create cannot
 * be submitted without it.
 */
export async function answerYolo(form: Locator, yolo = false) {
  await form
    .getByRole("group", { name: "runs without approval prompts" })
    .getByLabel(yolo ? "yes (YOLO)" : "no", { exact: true })
    .check();
}

/**
 * Open the inline create form and fill its required fields without submitting.
 *
 * This is the list view's plain toggled `<div>`, not a modal. `title` is
 * required because every caller needs a known title, including the explicit
 * empty-title case. Filling and submitting stay separate because callers need
 * to inspect the form while a request is pending or after it fails. Only the
 * command tab accepts an arbitrary command, so the helper selects it before
 * the command field becomes the request's source of intent, and
 * answers the command's required YOLO question (`yolo`, default no).
 */
export async function fillCreateForm(
  page: Page,
  { cwd, invocation, title, yolo = false }: { cwd: string; invocation: string; title: string; yolo?: boolean },
) {
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByRole("tab", { name: "command", exact: true }).click();
  await form.getByLabel("folder", { exact: true }).fill(cwd);
  await form.getByLabel("agent command").fill(invocation);
  // A command launch has no default YOLO answer: the form asks every time.
  await answerYolo(form, yolo);
  // The name field sits on the top action row now, visible without opening
  // anything — fill it by its label directly rather than by DOM position.
  await form.getByLabel("name (optional)").fill(title);
  return form;
}

/**
 * Stop then delete a session, tolerating only a session that is already gone.
 *
 * This deliberately differs from `helpers/fleet.ts`'s `cleanupSession`:
 * fleet issues stop and delete eagerly, so a failed stop does not block the
 * delete; this terminal helper stops at the first non-404 failure. That
 * fail-fast policy is inherited from the terminal specs. Changing it would be
 * observable fixture behavior, not part of moving the duplicate helpers.
 */
export async function cleanupSession(request: APIRequestContext, id: string): Promise<void> {
  const stopped = await request.post(`/api/sessions/${id}/stop`);
  if (!stopped.ok() && stopped.status() !== 404) {
    throw new Error(
      `cleanup: stopping session ${id} failed (${stopped.status()}): ${await stopped.text()}`,
    );
  }
  const deleted = await request.delete(`/api/sessions/${id}`);
  if (!deleted.ok() && deleted.status() !== 404) {
    throw new Error(
      `cleanup: deleting session ${id} failed (${deleted.status()}): ${await deleted.text()}`,
    );
  }
}
