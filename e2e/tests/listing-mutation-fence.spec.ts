/** A listing captured before a successful mutation cannot prove its new
 * session absent. Hold both that old reply and its successor so selection
 * cannot be silently repaired before the test observes the stale boundary. */
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession, observeFeedReaders, openRowMenu, pinAutoSelect, stubFeed, waitForFeedReadersSettled } from "./helpers/fleet";
import { fillCreateForm } from "./helpers/term";
import { routeGate } from "./helpers/route-gate";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";

for (const operation of ["create", "replace"] as const) {
  test(`a pre-${operation} listing cannot close the newly selected session`, async ({ page, request }) => {
    const title = `listing-fence-${operation}-${Date.now()}`;
    const source = await createSession(request, { title: `${title}-source` });
    let newId: string | undefined;
    const old = routeGate();
    const fresh = routeGate();
    await observeFeedReaders(page);
    const feed = await stubFeed(page);
    // A connected socket is not a healthy feed until greeted. Retire that
    // handshake's reads before arming the deliberately old mutation snapshot.
    feed.notifyOnConnect(1);
    let armed = false;
    let oldCaptured = false;
    let freshCaptured = false;
    await page.route((url) => url.pathname === "/api/sessions", async (route) => {
      if (route.request().method() !== "GET" || !armed) return route.continue();
      const response = await route.fetch();
      const body = await response.json();
      if (!oldCaptured) {
        expect(body.sessions.some((session: { id: string }) => session.id === source.id)).toBe(true);
        oldCaptured = true;
        await old.wait();
      } else {
        expect(body.sessions.some((session: { id: string }) => session.id === newId)).toBe(true);
        freshCaptured = true;
        await fresh.wait();
      }
      await route.fulfill({ response, json: body });
    });
    try {
      await pinAutoSelect(page, source.id);
      await page.goto("/");
      await feed.waitForConnection(1);
      await waitForSessionRevealed(page, source.id);
      const row = page.locator(`[data-session-id="${source.id}"]`);
      let submit;
      if (operation === "create") {
        const form = await fillCreateForm(page, { cwd: "/tmp", invocation: "sleep 300", title });
        submit = form.locator(".create-session-submit");
      } else {
        await openRowMenu(row);
        await row.locator(".session-row-replace").click();
        submit = row.locator(".confirm-replace");
      }
      await expect(submit).toBeEnabled();
      await waitForFeedReadersSettled(page);
      armed = true;
      feed.notify(2);
      await expect.poll(() => oldCaptured, { message: "the pre-mutation snapshot must be held" }).toBe(true);
      const mutation = page.waitForResponse((response) => response.request().method() === "POST"
        && new URL(response.url()).pathname === (operation === "create" ? "/api/sessions" : `/api/sessions/${source.id}/replace`));
      await submit.click();
      const response = await mutation;
      expect(response.ok()).toBe(true);
      newId = (await response.json()).id;
      expect(newId).toBeTruthy();
      expect(newId).not.toBe(source.id);
      await waitForSessionRevealed(page, newId!);
      // Reader serialization holds the new explicit read behind the old one.
      // Release only after the mutation selected its reply, then observe that
      // selection while the authoritative successor is still withheld.
      old.release();
      await expect.poll(() => freshCaptured, { message: "the fenced read must have a fresh successor" }).toBe(true);
      await waitForSessionRevealed(page, newId!);
      await expect(page.locator(".titlebar .title")).toHaveText(operation === "create" ? title : source.title);
      fresh.release();
      const created = page.locator(`[data-session-id="${newId}"]`);
      await expect(created).toHaveAttribute("data-session-selected", "true");
    } finally {
      old.release();
      fresh.release();
      if (newId) await cleanupSession(request, newId);
      await cleanupSession(request, source.id);
    }
  });
}
