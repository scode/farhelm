// Replace's browser contract: the "replace" menu item (row.rs's
// `.session-row-replace`) opens an inline confirmation in the same panel
// clone/delete already use, and confirming creates a fresh session
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
//
// This file also covers "replace with" (row.rs's `.session-row-replace-with`,
// SPEC.md's bullet of that name): clone's exact editable create form, seeded
// from the clicked row, whose launch button sends the same edited fields to
// `POST /api/sessions/{id}/replace`'s "with" override instead of to
// `POST /api/sessions` — replace's create-then-delete contract reached
// through clone's form rather than through an inline confirmation. The
// helm-level "with" override (its own field-by-field composition and its
// same-host refusal) is `sessions_tests.rs`'s job; this file's replace-with
// tests are about the one thing only a real browser proves: that editing the
// pre-filled form through the launch composer's own search box, then
// launching, reaches the override endpoint with exactly what was typed.

import { expect, test } from "./helpers/evidence";
import { Page } from "@playwright/test";
import { cleanupSession, createSession, FAKE_AGENT, hideSeenState, localHostId, openRowMenu, type SessionRow } from "./helpers/fleet";
import { attachFocusTrace, installFocusTrace } from "./helpers/focus-trace";
import { stackScratchDir } from "./helpers/scratch";
import { attachSession, waitForTermText } from "./helpers/term";

/** Find one session by its opaque server id, independent of title changes —
 * the same helper `clone.spec.ts` defines locally,
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
    // The inline prompt: consequence and title, the same shape delete's
    // prompt uses (row.rs's `.confirm-consequence`/
    // `.confirm-title`), and the one sentence that distinguishes this
    // prompt from deletion — every `replace_consequence` arm ends by naming
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

/** The narrowed shape these replace-with tests read back off `GET
 * /api/sessions` — mirrors `composer-word-search.spec.ts`'s own local
 * `ListedSession`, kept local rather than added to `fleet.ts`'s shared
 * `SessionRow` for the same reason that file's own doc gives: most specs
 * never look at `launch`, and folding it into the shared narrowing would
 * make every other spec's use of that type claim to cover a field it does
 * not check. */
type ListedSessionWithLaunch = {
  id: string;
  title: string;
  cwd: string;
  launch?: { harness: string; model: string | null; effort: string | null } | null;
};

async function listedSessionsWithLaunch(
  request: import("@playwright/test").APIRequestContext,
): Promise<ListedSessionWithLaunch[]> {
  const response = await request.get("/api/sessions");
  expect(response.ok(), "the session listing must be readable").toBe(true);
  return (await response.json() as { sessions: ListedSessionWithLaunch[] }).sessions;
}

/**
 * A "replace with" opening must land focus in search by itself — the same
 * handoff clone's own opening-focus test pins, through the sibling menu
 * item that shares its teardown. The focus assertions run BEFORE any fill,
 * mode-button click, or result selection: the full replace-with test below
 * checks focus only after its mode buttons have explicitly refocused
 * search, which would pass even with the opening handoff broken. Both
 * mutation endpoints are counted, since a replace-with composer can reach
 * either `POST /api/sessions` or the replace endpoint on submit.
 */
test("opening replace-with focuses search before any typing or mode click", async ({
  page,
  request,
}, testInfo) => {
  const title = `replace-with-opening-focus-${Date.now()}`;
  const cwd = stackScratchDir("replace-with-opening-focus-");
  const source = await createSession(request, { title, cwd });
  try {
    await page.goto("/");
    const sourceRow = row(page, source.id);
    await expect(sourceRow).toBeVisible({ timeout: 20_000 });
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(
      page.locator(".hosts-status", { hasText: "loading hosts" }),
    ).toHaveCount(0, { timeout: 20_000 });
    await installFocusTrace(page);
    const posts: string[] = [];
    const countPosts = (issued: import("@playwright/test").Request) => {
      if (issued.method() !== "POST") return;
      const pathname = new URL(issued.url()).pathname;
      if (pathname === "/api/sessions" || pathname === `/api/sessions/${source.id}/replace`) {
        posts.push(pathname);
      }
    };
    page.on("request", countPosts);
    try {
      await openRowMenu(sourceRow);
      await sourceRow.locator(".session-row-replace-with").click();
      const form = page.locator('.create-session-form[role="dialog"]');
      await expect(form).toBeVisible();
      const search = form.locator('.launch-composer-search input[role="combobox"]');
      await expect(search).toBeEnabled();
      await expect(page.locator(".session-row-menu-panel")).toHaveCount(0);
      await expect(search).toBeFocused();
      const query = `replace-with-focus-probe-${Date.now()}`;
      await page.keyboard.type(query);
      await expect(search).toHaveValue(query);
      expect(posts, "opening and typing into a replace-with must not submit it").toEqual([]);
    } finally {
      page.off("request", countPosts);
      await attachFocusTrace(page, testInfo, "replace-with-opening-focus-trace.json");
    }
  } finally {
    await cleanupSession(request, source.id);
  }
});

/**
 * A keyboard-opened "replace with" must land focus in search by itself.
 *
 * The trusted-keyboard twin of the pointer test above: open the row menu
 * from its focused toggle with ArrowDown, step down the real menu order
 * (rename, clone, replace with) to the item, and activate with Enter —
 * then again with Space. The source here is structured (the pointer
 * test's is legacy) so the opening-focus contract covers both shapes.
 */
test("opening replace-with from the keyboard focuses search before any typing", async ({
  page,
  request,
}, testInfo) => {
  const local = await localHostId(request);
  const cwd = stackScratchDir("replace-with-keyboard-focus-");
  const title = `replace-with-keyboard-focus-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: {
      cwd,
      title,
      host: local,
      launch: { harness: "codex", model: "gpt-6-astra", effort: "high", permissions: "yolo" },
    },
  });
  expect(created.ok(), `creating structured source: ${await created.text()}`).toBe(true);
  const sourceId = (await created.json()).id as string;
  try {
    // Fixed menu order (rename, clone, replace with, …), not the
    // seen-state feature — see the keyboard clone test's own comment.
    await hideSeenState(page);
    await page.goto("/");
    const sourceRow = row(page, sourceId);
    await expect(sourceRow).toBeVisible({ timeout: 20_000 });
    await attachSession(page, sourceId);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(
      page.locator(".hosts-status", { hasText: "loading hosts" }),
    ).toHaveCount(0, { timeout: 20_000 });
    await installFocusTrace(page);
    const posts: string[] = [];
    const countPosts = (issued: import("@playwright/test").Request) => {
      if (issued.method() !== "POST") return;
      const pathname = new URL(issued.url()).pathname;
      if (pathname === "/api/sessions" || pathname === `/api/sessions/${sourceId}/replace`) {
        posts.push(pathname);
      }
    };
    page.on("request", countPosts);
    try {
      for (const key of ["Enter", " "]) {
        const toggle = sourceRow.locator(".session-row-menu");
        await toggle.focus();
        await expect(toggle).toBeFocused();
        await page.keyboard.press("ArrowDown");
        await expect(sourceRow.locator(".session-row-menu-panel")).toBeVisible();
        await expect(sourceRow.locator(".session-row-rename")).toBeFocused();
        await page.keyboard.press("ArrowDown");
        await expect(sourceRow.locator(".session-row-clone")).toBeFocused();
        await page.keyboard.press("ArrowDown");
        await expect(sourceRow.locator(".session-row-replace-with")).toBeFocused();
        await page.keyboard.press(key);
        const form = page.locator('.create-session-form[role="dialog"]');
        await expect(form).toBeVisible();
        const search = form.locator('.launch-composer-search input[role="combobox"]');
        await expect(search).toBeEnabled();
        await expect(page.locator(".session-row-menu-panel")).toHaveCount(0);
        await expect(search).toBeFocused();
        const query = `replace-with-kb-${key === " " ? "space" : "enter"}-${Date.now()}`;
        await page.keyboard.type(query);
        await expect(search).toHaveValue(query);
        await page.keyboard.press("Escape");
        await page.keyboard.press("Escape");
        await expect(form).toHaveCount(0);
      }
      expect(posts, "opening and typing into keyboard replace-withs must not submit them").toEqual([]);
    } finally {
      page.off("request", countPosts);
      await attachFocusTrace(page, testInfo, "replace-with-keyboard-focus-trace.json");
    }
  } finally {
    await cleanupSession(request, sourceId);
  }
});

test("replace with pre-fills clone's form from the source, and editing the harness and effort through search then launching creates the edited session in the source's place", async ({
  page,
  request,
}) => {
  const local = await localHostId(request);
  const cwd = stackScratchDir("replace-with-");
  const title = `replace-with-live-${Date.now()}`;
  // A structured source, exactly like `clone.spec.ts`'s own structured GUI
  // clone test — the launch composer's search box only renders on the
  // Structured surface, and this is the direct route there: the row menu's
  // "replace with" click must seed the composer on whatever surface the
  // source itself used, and a structured source is what puts the search box
  // on screen without an extra "back to harnesses" click this test has no
  // reason to make.
  const initial = { harness: "codex", model: "gpt-6-astra", effort: "high" };
  let sourceId: string | undefined;
  let replacedId: string | undefined;
  try {
    const created = await request.post("/api/sessions", {
      data: { cwd, title, host: local, launch: initial },
    });
    expect(created.ok(), `creating structured source: ${await created.text()}`).toBe(true);
    const source: SessionRow = await created.json();
    sourceId = source.id;

    await page.goto("/");
    const sourceRow = row(page, sourceId);
    await expect(sourceRow).toBeVisible({ timeout: 20_000 });

    await openRowMenu(sourceRow);
    await sourceRow.locator(".session-row-replace-with").click();

    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    // Clone's exact prefill: this row's own title and directory, filled
    // without any edit yet, and the launch button already naming the
    // destructive half of what pressing it will do.
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue(cwd);
    await expect(form.getByLabel("name (optional)")).toHaveValue(title);
    await expect(form.locator(".create-session-submit")).toContainText("replace");

    // Replace with has the same shell as New and Clone: switching launch
    // modes cannot change the fixed source destination or its editable name.
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "other / command", exact: true }).click();
    await expect(form).toHaveAttribute("data-composer-mode", "command");
    await expect(form.locator('.launch-composer-search input[role="combobox"]')).toBeFocused();
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue(cwd);
    await expect(form.getByLabel("name (optional)")).toHaveValue(title);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await expect(form).toHaveAttribute("data-composer-mode", "structured");

    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await expect(search).toBeFocused();

    // Change the harness through the search box: accepting a result clears
    // the box and keeps focus there (the word-search feature this test
    // exercises alongside replace-with).
    await search.fill("claude");
    await search.press("Enter");
    await expect(
      form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Claude", exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(search).toHaveValue("");
    await expect(search).toBeFocused();

    // Change the effort the same way — overriding the source's own "high"
    // (still a valid Claude effort, so this is a deliberate edit, not a
    // value the harness switch cleared on its own).
    await search.fill("medium");
    await search.press("Enter");
    await expect(
      form.locator(".launch-composer-effort-choice").getByRole("button", { name: "medium", exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(search).toHaveValue("");
    await expect(search).toBeFocused();

    const launchButton = form.locator(".create-session-submit");
    await expect(launchButton).toBeEnabled();
    // Enter on the now-empty search box launches a complete, valid
    // selection through the ordinary Launch path (SPEC.md's launch-composer
    // bullet) — which for a replace-with prefill is `POST
    // /api/sessions/{id}/replace` rather than `POST /api/sessions`.
    const [response] = await Promise.all([
      page.waitForResponse(
        (r) => r.request().method() === "POST" && r.url().endsWith(`/api/sessions/${sourceId}/replace`),
      ),
      search.press("Enter"),
    ]);
    const replaced = await response.json();
    replacedId = replaced.id;
    expect(replacedId).not.toBe(sourceId);
    expect(replaced.title).toBe(title);
    expect(replaced.cwd).toBe(cwd);
    expect(replaced.launch).toMatchObject({ harness: "claude", effort: "medium" });

    // The source id is gone; the edited replacement exists — both read back
    // through the API listing, independent of the create reply and of
    // whatever the row-level feed has painted by the time this runs.
    const listing = await listedSessionsWithLaunch(request);
    const replacement = listing.find((session) => session.id === replacedId);
    expect(replacement, "the edited replacement must be listed").toBeTruthy();
    expect(replacement?.title).toBe(title);
    expect(replacement?.cwd).toBe(cwd);
    expect(replacement?.launch).toMatchObject({ harness: "claude", effort: "medium" });
    expect(
      listing.some((session) => session.id === sourceId),
      "the source id must no longer be listed",
    ).toBe(false);
  } finally {
    if (replacedId) await cleanupSession(request, replacedId);
    if (sourceId) await cleanupSession(request, sourceId);
  }
});

test("cancelling a replace-with composer leaves the source untouched and creates nothing", async ({
  page,
  request,
}) => {
  const title = `replace-with-cancel-${Date.now()}`;
  const cwd = "/tmp";
  const session = await createSession(request, { title, cwd });
  // Recorded from before the click, exactly like the plain-replace
  // cancellation test above: proving NOTHING reached either endpoint is the
  // whole point, and a `page.on` listener lets every other request the page
  // makes keep flowing normally.
  const posts: string[] = [];
  page.on("request", (req) => {
    if (req.method() !== "POST") return;
    const url = new URL(req.url());
    if (url.pathname === "/api/sessions" || url.pathname === `/api/sessions/${session.id}/replace`) {
      posts.push(url.pathname);
    }
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    await openRowMenu(target);
    await target.locator(".session-row-replace-with").click();

    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    await expect(form.getByLabel("name (optional)")).toHaveValue(title);

    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);

    expect(posts).toEqual([]);

    // The same row, same id, still just as it was — no create, no delete,
    // nothing to clean up beyond the fixture itself.
    await expect(target).toBeVisible();
    await expect(target.locator(".session-title")).toHaveText(title);
  } finally {
    await cleanupSession(request, session.id);
  }
});
