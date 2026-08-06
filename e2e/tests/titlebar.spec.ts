// The titlebar-overflow bug's own coverage (PLAN_M6_5.md item 4). A
// dedicated file, not an addition to terminal.spec.ts: that file exports
// nothing to reach back into and is already enormous, and every
// remaining milestone adds named tests, so per this milestone's own
// testing decision new specs start their own per-area file instead of
// growing it further (mouse-modes.spec.ts made the same call, and its
// helpers below are duplicated from terminal.spec.ts's identical ones
// for the same reason).
//
// The M6.5 ledger's bug, as recorded: the session view's header
// (`.titlebar`) carries the title, the rename affordance, and a `.meta`
// span (cwd em-dash invocation) in one non-wrapping flex row, and a long
// invocation — 120-odd characters was enough — overflowed the row and
// pushed the rename control out of reach. Two states, two different
// outcomes once this test actually strengthened its own assertions:
//
// - CLOSED (the plain `rename` button): does NOT reproduce against the
//   current header, and needs no CSS fix. `.meta`'s `overflow: hidden`
//   already zeroes its own automatic flex-shrink minimum per the
//   flexbox spec, and `.meta` sits AFTER the button in flex order, so it
//   could never reposition it regardless of how wide `.meta` grows (see
//   app.css's comment on `.titlebar .meta`). The M5-era rename-
//   affordance rebuild (`session_view.rs`'s "Rename (PLAN_M5.md item 6)"
//   doc) appears to have fixed this half incidentally.
// - OPEN (the rename FORM the button opens): reproduces, and needed a
//   real fix. Widening this test's own assertions to check the open
//   form's controls, not just the closed button, caught a live bug —
//   the shared `.rename-form` rule's flex-basis of a literal `0`
//   (app.css) meant it got none of the row's negative-free-space
//   distribution once a long invocation forced `.title`/`.meta` to
//   compete for it, squeezing the input to an unusable 18px in both
//   engines. `.titlebar .rename-form` (app.css) is the fix; see its own
//   comment for the mechanism.
//
// This test therefore pins a REACHABILITY CONTRACT for BOTH states
// against future header rework: the rename control, and the form it
// opens, must both stay fully reachable and usable while `.meta` is
// under maximum overflow pressure — the closed half because the M6.5
// ledger already found it once, the open half because this milestone
// just did.
import { test, expect, Page, APIRequestContext, Locator } from "@playwright/test";
import path from "node:path";

/**
 * The `basic` fake-agent invocation, built from an absolute path exactly
 * like terminal.spec.ts's and mouse-modes.spec.ts's own do, with a long
 * `--record-home` value appended to push the submitted command STRING
 * well past the 120-odd characters that reproduced the overflow. This is
 * what the create form actually sends over the wire — the supervisor
 * shell-splits it into argv only once the session launches — and it
 * shares `CreateSession`'s 64 KiB cap with `cwd` and `title` (combined,
 * not per-field: `farhelm-proto`'s `CREATE_FIELD_CAP`), not an unbounded
 * field.
 *
 * `--record-home` (not a made-up flag) is the genuinely inert choice:
 * the `FakeAgent` subcommand's clap surface (`crates/farhelm/src/main.rs`)
 * accepts no free-standing trailing arguments at all — an unrecognized
 * flag would fail argv parsing and the session would never reach
 * `FAKE-AGENT READY` — but `--record-home` is a real, always-accepted
 * flag whose own doc comment (same file, `InternalCmd::FakeAgent`)
 * states it is "ignored by every other script", `basic` included. The
 * value itself needs no filesystem meaning (record-writing scripts are
 * the only readers, and this session never runs one): a long run of `x`
 * characters is enough to push the invocation's total length well past
 * the reproduction threshold regardless of how long this checkout's own
 * absolute path happens to be.
 */
const LONG_INVOCATION_MARKER = "x".repeat(200);
const LONG_INVOCATION = `"${
  path.resolve(__dirname, "../../target/debug/farhelm")
}" internal fake-agent --script basic --record-home ${LONG_INVOCATION_MARKER}`;

/**
 * Poll the xterm.js buffer (scrollback + viewport, not just the DOM's
 * rendered rows — see terminal.spec.ts's own file header for why) until
 * `needle` shows up. Polling, not a one-shot read: terminal output
 * arrives asynchronously over the WebSocket with no DOM event to await.
 */
async function waitForTermText(page: Page, needle: string, timeout = 15_000) {
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const term = (window as any).__farhelmTerm;
          if (!term) return "";
          const buf = term.buffer.active;
          const lines: string[] = [];
          for (let i = 0; i < buf.length; i++) {
            lines.push(buf.getLine(i)?.translateToString(true) ?? "");
          }
          return lines.join("\n");
        }),
      { timeout, message: `waiting for ${needle}` },
    )
    .toContain(needle);
}

/**
 * Look up a session's id from the real API by its title, for best-effort
 * cleanup when the happy path did not get far enough to hand back an id
 * itself.
 */
async function findSessionIdByTitle(
  request: APIRequestContext,
  title: string,
): Promise<string | undefined> {
  const listing = await (await request.get("/api/sessions")).json();
  return listing.sessions.find((s: any) => s.title === title)?.id;
}

/** Stop and delete a session, tolerating either already being done. */
async function cleanupSession(request: APIRequestContext, id: string) {
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

/**
 * A locator's `getBoundingClientRect()`, read via `evaluate` rather than
 * Playwright's own `boundingBox()`: `boundingBox()` returns `null` for a
 * detached or non-visible element, which would force every call site
 * below to null-check and non-null-assert its way past a case that
 * cannot actually happen here — every rect this test reads is taken
 * after a `toBeVisible`/`toBeInViewport` assertion on the same element
 * already passed. `evaluate` simply throws if the element is gone, which
 * is the right failure mode for a genuinely-should-never-happen case.
 */
async function rectOf(locator: Locator): Promise<DOMRect> {
  return locator.evaluate((el) => el.getBoundingClientRect());
}

test("long-invocation-titlebar-rename", async ({ page, request }) => {
  test.setTimeout(60_000);
  expect(
    LONG_INVOCATION.length,
    "the fixture invocation must actually reproduce the bug's length trigger",
  ).toBeGreaterThan(120);

  const title = `titlebar-overflow-${Date.now()}`;
  const renamed = `${title}-renamed`;
  let id: string | undefined;
  try {
    // Inline create-dialog fill, matching terminal.spec.ts's own
    // `fillCreateForm` step for step — not factored into a helper here
    // since this file has exactly the one call site.
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await form.locator('input[type="text"]').nth(0).fill("/tmp");
    await form.locator('input[type="text"]').nth(1).fill(LONG_INVOCATION);
    await form.locator('input[type="text"]').nth(2).fill(title);
    await form.locator('button[type="submit"]').click();
    // Success navigates straight into the new session's terminal, same
    // as every create-dialog flow in terminal.spec.ts and
    // mouse-modes.spec.ts.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");
    id = await findSessionIdByTitle(request, title);

    const meta = page.locator(".titlebar .meta");

    // The overflow CSS is genuinely doing work here, not merely absent
    // because the metadata line happened to fit anyway — same oracle
    // terminal.spec.ts's huge-title confirm-row test uses: a truncated
    // element's scrollWidth exceeds its clientWidth.
    const metaOverflowing = await meta.evaluate((el) => el.scrollWidth > el.clientWidth);
    expect(
      metaOverflowing,
      "the long invocation must actually be clipped for this test to mean anything",
    ).toBe(true);

    // The CLOSED rename button survives the overflow: FULLY inside the
    // viewport (`ratio: 1` — a sliver poking in at the edge is not
    // "reachable"), and not overlapped by the metadata line that follows
    // it in the row.
    const renameButton = page.locator(".titlebar .session-rename");
    await expect(renameButton).toBeVisible();
    await expect(renameButton).toBeInViewport({ ratio: 1 });
    const [renameRect, metaRectClosed] = await Promise.all([
      rectOf(renameButton),
      rectOf(meta),
    ]);
    expect(renameRect.right).toBeLessThanOrEqual(metaRectClosed.left + 1);

    // The OPEN rename form has its own reachability contract, separate
    // from the closed button's: `.rename-form` (app.css) is itself
    // allowed to shrink, unlike `.session-rename`'s fixed
    // `flex-shrink: 0`, so a future header change could narrow the form
    // down without ever touching the button this test already checked.
    // Every one of its controls must therefore be checked again once the
    // form is actually open: fully in viewport, wide enough to use (not
    // squeezed to a sliver), and clear of the metadata line.
    await renameButton.click();
    const field = page.locator(".titlebar .rename-input");
    const saveButton = page.locator(".titlebar .rename-submit");
    const cancelButton = page.locator(".titlebar .rename-cancel");
    await expect(field).toBeVisible();
    await expect(field).toHaveValue(title);

    for (const control of [field, saveButton, cancelButton]) {
      await expect(control).toBeInViewport({ ratio: 1 });
    }
    const [fieldRect, saveRect, cancelRect, metaRectOpen] = await Promise.all([
      rectOf(field),
      rectOf(saveButton),
      rectOf(cancelButton),
      rectOf(meta),
    ]);
    for (const [name, rect] of [
      ["input", fieldRect],
      ["save", saveRect],
      ["cancel", cancelRect],
    ] as const) {
      expect(
        rect.width,
        `the ${name} control must keep a usable width, not get squeezed toward zero`,
      ).toBeGreaterThan(20);
    }
    expect(fieldRect.right).toBeLessThanOrEqual(metaRectOpen.left + 1);
    expect(saveRect.right).toBeLessThanOrEqual(metaRectOpen.left + 1);
    expect(cancelRect.right).toBeLessThanOrEqual(metaRectOpen.left + 1);

    // And it is not just visually present but actually usable: complete
    // a real rename through the visible save CONTROL (not a keyboard
    // shortcut) — the assertion this whole test exists for.
    await field.fill(renamed);
    await saveButton.click();

    await expect(page.locator(".titlebar .title")).toHaveText(renamed);
    await expect(page.locator(".rename-error")).toHaveCount(0);
    await expect(page.locator(".rename-form")).toHaveCount(0);

    const fetched = await (await request.get(`/api/sessions/${id}`)).json();
    expect(fetched.title).toBe(renamed);
  } finally {
    // Best-effort: the happy path above should already have created
    // exactly one session, but a failed assertion partway through must
    // not leak a long-running fake-agent process into a later run.
    if (!id) id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (!id) id = await findSessionIdByTitle(request, renamed).catch(() => undefined);
    if (id) await cleanupSession(request, id);
  }
});
