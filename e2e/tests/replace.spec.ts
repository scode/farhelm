// Replace's browser contract: the "replace" menu item (row.rs's
// `.session-row-replace`) opens an inline confirmation in the same panel
// clone/archive/delete already use, and confirming creates a fresh session
// with the source's cwd, title, and invocation, selects it, and removes the
// source — all in one round trip through `POST /api/sessions/{id}/replace`
// (SPEC.md's "replace"). The helm-level composition (create-then-delete, the
// two failure modes, the profile-fallback divergence from clone) is covered
// by `sessions_tests.rs`'s own `POST /api/sessions/{id}/replace` suite; this
// file's job is only what a real browser click chain actually does: the
// prompt, the selection swap, and that the replacement's terminal starts
// genuinely fresh rather than carrying the source's scrollback forward
// (SPEC.md's "a fresh session with the same settings takes its place" is
// this file's one property confirm_consequence-shaped unit tests cannot
// see).

import { expect, test } from "./helpers/evidence";
import { Page } from "@playwright/test";
import { cleanupSession, createSession, FAKE_AGENT, openRowMenu, type SessionRow } from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";

/** Find one session by its opaque server id, independent of title changes —
 * the same helper `clone.spec.ts` and `archive.spec.ts` each define locally,
 * repeated here rather than shared because it is three lines and the
 * sharing would cost an import cycle nobody else needs. */
function row(page: Page, id: string) {
  return page.locator(`.session-row[data-session-id="${id}"]`);
}

test("replacing a live session creates a fresh row in its place, selected, with no prior scrollback", async ({
  page,
  request,
}) => {
  const title = `replace-live-${Date.now()}`;
  const cwd = "/tmp";
  const source: SessionRow = await createSession(request, { title, cwd });
  let replacementId: string | undefined;
  try {
    await page.goto("/");
    const sourceRow = row(page, source.id);
    await expect(sourceRow).toBeVisible({ timeout: 20_000 });

    // Attach and leave a MARKER in the source's scrollback — the fake
    // agent's `basic` script echoes typed input (see `FAKE_AGENT`'s own
    // doc), so this is real terminal history a fresh session must not
    // inherit, not a fixture standing in for one.
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await page.locator("#terminal").click();
    await page.keyboard.type("PRE-REPLACE-MARKER");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:PRE-REPLACE-MARKER");

    // A test-only SENTINEL on the source's own xterm instance, the same
    // stale-singleton guard `terminal.spec.ts`'s own session-switch test
    // uses: `__farhelmTermReady` and the READY banner are already true for
    // THIS instance, so neither can tell the replacement's terminal apart
    // from the source's — only a marker that a fresh mount could not
    // possibly carry can.
    await page.evaluate(() => {
      (window as any).__farhelmTerm.__testMarker = "pre-replace";
    });

    await openRowMenu(sourceRow);
    await sourceRow.locator(".session-row-replace").click();
    // The inline prompt: consequence and title, same shape delete's and
    // archive's own prompts use (row.rs's `.confirm-consequence`/
    // `.confirm-title`), and the one sentence that distinguishes this
    // prompt from either — every `replace_consequence` arm ends by naming
    // the replacement.
    await expect(sourceRow.locator(".confirm-consequence")).toContainText(
      "a fresh session with the same settings takes its place",
    );
    await expect(sourceRow.locator(".confirm-title")).toContainText(title);

    const [response] = await Promise.all([
      page.waitForResponse(
        (r) =>
          r.request().method() === "POST"
          && r.url().endsWith(`/api/sessions/${source.id}/replace`),
      ),
      sourceRow.locator(".confirm-replace").click(),
    ]);
    const replaced: SessionRow = await response.json();
    replacementId = replaced.id;
    expect(replacementId).not.toBe(source.id);
    expect(replaced.cwd).toBe(cwd);
    expect(replaced.invocation).toBe(FAKE_AGENT);

    // The old id is gone; the new one is up, carries the source's title,
    // and is SELECTED without an extra click — `list::view::ListView`'s
    // `do_replace` writes the selection itself before requesting the list
    // re-read that would otherwise let auto-select pick something else.
    await expect(sourceRow).toHaveCount(0, { timeout: 20_000 });
    const replacementRow = row(page, replacementId);
    await expect(replacementRow).toBeVisible({ timeout: 20_000 });
    await expect(replacementRow.locator(".session-title")).toHaveText(title);
    await expect(replacementRow).toHaveAttribute("data-session-selected", "true");

    // The terminal now showing is the replacement's own: a fresh READY
    // banner, and — the property this whole test exists to pin — no trace
    // of the marker the SOURCE's agent echoed. A broken replace that
    // reused the old terminal (or the old session's id under a new label)
    // would leave the marker right there.
    //
    // `__farhelmTermReady` and the READY text are already TRUE for the
    // source's own instance before any of this runs, so neither one can
    // prove the replacement's terminal has actually been published by the
    // time this test inspects it — a correct implementation and one that
    // left the stale source instance mounted a moment too long would both
    // satisfy them. The sentinel closes that gap instead (the same
    // stale-singleton fix `terminal.spec.ts`'s session-switch test uses):
    // poll until `__farhelmTerm` is DEFINED (not merely absent during an
    // unmount), no longer carries the marker stamped on the source's
    // instance above, and its socket has actually reached OPEN — only THEN
    // is it safe to wait for READY and read the buffer.
    await expect
      .poll(() =>
        page.evaluate(() => {
          const term = (window as any).__farhelmTerm;
          const ws = (window as any).__farhelmWs;
          return (
            Boolean(term)
            && term.__testMarker !== "pre-replace"
            && ws?.readyState === WebSocket.OPEN
          );
        })
      )
      .toBe(true);
    await waitForTermText(page, "FAKE-AGENT READY");
    const buffer = await page.evaluate(() => {
      const term = (window as any).__farhelmTerm;
      if (!term) return "";
      const buf = term.buffer.active;
      const lines: string[] = [];
      for (let i = 0; i < buf.length; i++) {
        lines.push(buf.getLine(i)?.translateToString(true) ?? "");
      }
      return lines.join("\n");
    });
    expect(buffer).not.toContain("PRE-REPLACE-MARKER");
  } finally {
    if (replacementId) await cleanupSession(request, replacementId);
    await cleanupSession(request, source.id);
  }
});

test("cancelling a replace confirmation leaves the session untouched", async ({ page, request }) => {
  const title = `replace-cancel-${Date.now()}`;
  const session = await createSession(request, { title, cwd: "/tmp" });
  // Recorded from before the click, not asserted on until after cancel:
  // proving NOTHING reached the endpoint is the whole point, and a `page.on`
  // listener (rather than a route intercept, which would have to decide how
  // to answer a request this test does not expect to see at all) is what
  // lets every OTHER request the page makes keep flowing normally.
  const replaceRequests: string[] = [];
  page.on("request", (req) => {
    if (
      req.method() === "POST"
      && req.url().endsWith(`/api/sessions/${session.id}/replace`)
    ) {
      replaceRequests.push(req.url());
    }
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    await openRowMenu(target);
    await target.locator(".session-row-replace").click();
    await expect(target.locator(".confirm-consequence")).toBeVisible();

    await target.locator(".replace-cancel").click();

    // Nothing was ever sent: a cancellation path that accidentally created
    // a replacement while leaving this row alone would still pass the
    // row-survives assertions below, and would silently leak the orphaned
    // session past the fixture's own cleanup.
    expect(replaceRequests).toEqual([]);

    // The SAME row, same id, still just as it was — no create, no
    // delete, nothing to clean up beyond the fixture itself.
    await expect(target).toBeVisible();
    await expect(target.locator(".session-title")).toHaveText(title);
    await expect(target.locator(".confirm-consequence")).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});
