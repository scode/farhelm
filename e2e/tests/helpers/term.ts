// Shared terminal-island test surface: only stable cross-spec helpers live
// here. Divergent one-off helpers remain local to the specs that need them.

import { expect, type APIRequestContext, type Page } from "@playwright/test";

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
 */
export async function waitForTermText(page: Page, needle: string, timeout = 15_000) {
  await expect
    .poll(() => termText(page), { timeout, message: `waiting for ${needle}` })
    .toContain(needle);
}

/**
 * Open a newly created session and wait until its terminal can receive input.
 *
 * Mounting alone is insufficient: terminal.js exposes its readiness global
 * before the WebSocket necessarily reaches OPEN, and input sent in that gap
 * is dropped. This helper intentionally relies on the same terminal-island
 * test surface as `termText`.
 */
export async function attachSession(page: Page, id: string): Promise<void> {
  const target = page.locator(`[data-session-id="${id}"]`);
  await expect(target).toBeVisible({ timeout: 20_000 });
  await target.locator(".session-row-open").click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await page.waitForFunction(() => (window as any).__farhelmWs?.readyState === WebSocket.OPEN);
}

/**
 * Open the inline create form and fill its required fields without submitting.
 *
 * This is the list view's plain toggled `<div>`, not a modal. `title` is
 * required because every caller needs a known title, including the explicit
 * empty-title case. Filling and submitting stay separate because callers need
 * to inspect the form while a request is pending or after it fails. Selecting
 * the custom-command option makes the helper independent of a deleted profile
 * that a shared stack might otherwise retain as its last-used choice.
 */
export async function fillCreateForm(
  page: Page,
  { cwd, invocation, title }: { cwd: string; invocation: string; title: string },
) {
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  // The agent picker is told, explicitly, that this create means the command
  // below. It is not a formality: the dialog defaults to the target host's
  // last-used profile, and when that profile has since been DELETED — which
  // is the state any run that exercised profiles leaves the shared stack in —
  // it selects nothing at all and blocks the create until someone answers
  // (SPEC.md's ask-don't-guess). Saying "custom command" here is what a user
  // in that state would do, and it makes this helper independent of whatever
  // the last profile-backed create left behind.
  await form.locator(".create-session-profile").selectOption("");
  await form.locator('input[type="text"]').nth(0).fill(cwd);
  await form.locator('input[type="text"]').nth(1).fill(invocation);
  await form.locator('input[type="text"]').nth(2).fill(title);
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
