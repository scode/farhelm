// Clone's browser contract: the "clone" menu item opens the create form
// pre-filled from the clicked row (crates/farhelm-ui/src/list/row.rs's
// `.session-row-clone`, `create_form::CreatePrefill`), the prefill reflects
// the row's own agent — a profile id when the row was created from one
// still `Present` in the catalog, the raw invocation otherwise. A selected
// profile displays its own invocation while the raw value remains the seed
// for custom mode. Submitting the edited copy creates a SEPARATE session
// while leaving the cloned row exactly as it was.

import { expect, test } from "./helpers/evidence";
import { Locator, Page } from "@playwright/test";
import {
  cleanupProfile,
  cleanupSession,
  createProfile,
  createSession,
  FAKE_AGENT,
  hideSeenState,
  localHostId,
  openRowMenu,
  type SessionRow,
} from "./helpers/fleet";
import {
  armFocusGate,
  attachFocusTrace,
  focusGateHeldCount,
  freezeDialogGate,
  installFocusTrace,
  releaseFocusGate,
} from "./helpers/focus-trace";
import { routeGate } from "./helpers/route-gate";
import { stackScratchDir } from "./helpers/scratch";
import { attachSession, termText, waitForTermText } from "./helpers/term";

/** Find one session by its opaque server id, independent of title changes. */
function row(page: Page, id: string) {
  return page.locator(`.session-row[data-session-id="${id}"]`);
}

/**
 * Fill the create form's changed working directory and submit it, waiting
 * on the real `POST /api/sessions` response for the new session's id.
 *
 * Kept separate from the create-form fill sequence in `real-agent.spec.ts`'s
 * own helper: that one fills every field from scratch for an ordinary
 * create, while a clone's whole point is that only ONE field — the
 * directory — needs touching, with the rest already carrying the row's own
 * values into the request.
 */
async function submitClonedCwd(page: Page, form: Locator, cwd: string) {
  await form.getByLabel("folder", { exact: true }).fill(cwd);
  const [response] = await Promise.all([
    page.waitForResponse(
      (r) => r.request().method() === "POST" && r.url().endsWith("/api/sessions"),
    ),
    form.locator(".create-session-submit").click(),
  ]);
  const body = await response.json();
  return body.id as string;
}

/**
 * Edit a clone's name through its label.
 *
 * The name field sits on the composer's top action row now, so there is no
 * disclosure left to open — this helper survives only because callers share
 * one name for "however the clone's name field is reached."
 */
async function fillCloneTitle(form: Locator, title: string) {
  await form.getByLabel("name (optional)").fill(title);
}

/** Switch the shared harness picker to the legacy profile/command controls. */
async function chooseCommandMode(form: Locator) {
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "other / command", exact: true }).click();
  await expect(form).toHaveAttribute("data-composer-mode", "command");
  await expect(form.locator('.launch-composer-search input[role="combobox"]')).toBeFocused();
}

/**
 * A GUI clone carries a structured parent all the way to a ready successor.
 *
 * The parent is created through the real API so the browser cannot seed its
 * own source state. Clone must copy every explicit launch choice into the
 * mounted dialog without posting; only the visible Launch click may create
 * the child. The stack-owned `codex` wrapper records its generation and argv
 * immediately before `exec`, which distinguishes the child's process from
 * the parent's terminal history even if a later replay retains both.
 */
test("a structured GUI clone pre-fills without launching, then starts its ready successor", async ({
  page,
  request,
}) => {
  const local = await localHostId(request);
  const cwd = stackScratchDir("structured-gui-clone-");
  const title = `structured-gui-clone-${Date.now()}`;
  const selection = {
    harness: "codex",
    model: "gpt-6-astra",
    effort: "high",
    permissions: "yolo",
  };
  let parentId: string | undefined;
  let childId: string | undefined;
  try {
    const created = await request.post("/api/sessions", {
      data: { cwd, title, host: local, launch: selection },
    });
    expect(created.ok(), `creating structured parent: ${await created.text()}`).toBe(true);
    const parent = await created.json();
    parentId = parent.id;
    expect(parent.launch).toEqual(selection);

    await page.goto("/");
    const source = row(page, parentId);
    await expect(source).toBeVisible({ timeout: 20_000 });
    let createPosts = 0;
    const countCreates = (issued: import("@playwright/test").Request) => {
      if (issued.method() === "POST" && new URL(issued.url()).pathname === "/api/sessions") {
        createPosts += 1;
      }
    };
    page.on("request", countCreates);
    try {
      await openRowMenu(source);
      await source.locator(".session-row-clone").click();
      const form = page.locator(".create-session-form");
      await expect(form).toBeVisible();
      await expect(form.locator(".create-session-host")).toHaveValue(String(local));
      // The search field and recent-folder group also mention “folder”; the
      // composer input is the one control whose literal value becomes the
      // successor destination.
      await expect(form.locator('input[aria-label="folder"]')).toHaveValue(cwd);
      // The chip strip that used to echo every choice is gone; the harness
      // shows on its pressed chip and the rest on the summary line.
      await expect(form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true })).toHaveAttribute("aria-pressed", "true");
      await expect(form.locator(".launch-composer-summary")).toHaveText("model: gpt-6-astra · effort: high · permissions: yolo");
      expect(createPosts, "opening and inspecting a clone must not launch it").toBe(0);

      const [response] = await Promise.all([
        page.waitForResponse(
          (r) => r.request().method() === "POST" && new URL(r.url()).pathname === "/api/sessions",
        ),
        form.locator(".create-session-submit").click(),
      ]);
      expect(response.request().postDataJSON().launch).toEqual(selection);
      const child = await response.json();
      childId = child.id;
      expect(childId).not.toBe(parentId);
      expect(child.launch).toEqual(selection);
    } finally {
      page.off("request", countCreates);
    }

    // The create reply is admission evidence. Re-reading both the live detail
    // route and the fleet list makes the persisted projection a separate
    // assertion rather than trusting that reply to have been painted back.
    const detail = await request.get(`/api/sessions/${childId}`);
    expect(detail.ok(), `reading structured successor: ${await detail.text()}`).toBe(true);
    expect((await detail.json()).launch).toEqual(selection);
    const listed = await request.get("/api/sessions");
    expect(listed.ok(), `listing structured successor: ${await listed.text()}`).toBe(true);
    const live = (await listed.json()).sessions.find((session: any) => session.id === childId);
    expect(live, "the admitted child must be present in the live listing").toBeTruthy();
    expect(live.launch).toEqual(selection);

    await attachSession(page, childId);
    await waitForTermText(page, `STRUCTURED-LAUNCH-GENERATION:${childId}:1`, 20_000);
    const output = await termText(page);
    expect(output).toContain("STRUCTURED-LAUNCH-ARGV: -m gpt-6-astra");
    expect(output).toContain("model_reasoning_effort=high");
    expect(output).toContain("--yolo");
    await waitForTermText(page, "FAKE-AGENT READY", 20_000);
  } finally {
    if (childId) await cleanupSession(request, childId);
    if (parentId) await cleanupSession(request, parentId);
  }
});

/**
 * A clone opening must land focus in search by itself, before anything types.
 *
 * "New" focuses the composer search on open so typing starts at once; the
 * report behind this test is that clone (and "replace with") leave focus
 * elsewhere, so the first keystrokes go nowhere — or worse, somewhere
 * unintended. The assertions run BEFORE any fill, focus, mode-button click,
 * or result selection: an eventual-focus check after those would pass by
 * refocusing rather than by proving the opening's own handoff. Typing a
 * unique query through `page.keyboard` (not `search.fill`, which focuses
 * its target first and would mask the bug) proves the handoff reached the
 * page's real keystroke routing, and the POST counter proves the opening
 * itself launched nothing. The attached focus trace names the losing
 * operation when this fails: which claimant's `focus()` call ran, against
 * which node, and whether search could even receive focus at that point.
 */
test("opening a clone focuses search before any typing or mode click", async ({
  page,
  request,
}, testInfo) => {
  const title = `clone-opening-focus-${Date.now()}`;
  const cwd = stackScratchDir("clone-opening-focus-");
  const source = await createSession(request, { title, cwd });
  try {
    await page.goto("/");
    const sourceRow = row(page, source.id);
    await expect(sourceRow).toBeVisible({ timeout: 20_000 });
    // A stable selected-terminal fixture: attaching and seeing the ready
    // banner proves this session's terminal revealed, so a later focus
    // movement cannot be the initial reveal still settling.
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    // The host list's own mount-time read changes `hosts_list_shape`,
    // which the row menu's close-on-layout-shift effect watches — wait
    // for it to settle (sidebar.spec.ts's `waitForHostsListSettled`)
    // before opening the menu whose teardown is under test.
    await expect(
      page.locator(".hosts-status", { hasText: "loading hosts" }),
    ).toHaveCount(0, { timeout: 20_000 });
    await installFocusTrace(page);
    let createPosts = 0;
    const countCreates = (issued: import("@playwright/test").Request) => {
      if (issued.method() === "POST" && new URL(issued.url()).pathname === "/api/sessions") {
        createPosts += 1;
      }
    };
    page.on("request", countCreates);
    try {
      await openRowMenu(sourceRow);
      await sourceRow.locator(".session-row-clone").click();
      const form = page.locator('.create-session-form[role="dialog"]');
      await expect(form).toBeVisible();
      const search = form.locator('.launch-composer-search input[role="combobox"]');
      await expect(search).toBeEnabled();
      await expect(page.locator(".session-row-menu-panel")).toHaveCount(0);
      await expect(search).toBeFocused();
      const query = `clone-focus-probe-${Date.now()}`;
      await page.keyboard.type(query);
      await expect(search).toHaveValue(query);
      expect(createPosts, "opening and typing into a clone must not launch it").toBe(0);
    } finally {
      page.off("request", countCreates);
      await attachFocusTrace(page, testInfo, "clone-opening-focus-trace.json");
    }
  } finally {
    await cleanupSession(request, source.id);
  }
});

/**
 * A keyboard-opened clone must land focus in search by itself, too.
 *
 * Same handoff as the pointer test above, reached through trusted keyboard
 * activation instead: focus the row toggle, open the menu with ArrowDown,
 * step to the clone item through the real menu order, and activate with
 * Enter — then again with Space, which native buttons activate on keyup
 * rather than keydown. Either key leaves DOM focus on the clone item at
 * the moment it unmounts, which is exactly the shape that must not hand
 * focus back to the row toggle once the composer has claimed it. The
 * source here is structured (the pointer test's is legacy) so the
 * opening-focus contract covers both prefill shapes.
 */
test("opening a clone from the keyboard focuses search before any typing", async ({
  page,
  request,
}, testInfo) => {
  const local = await localHostId(request);
  const cwd = stackScratchDir("clone-keyboard-focus-");
  const title = `clone-keyboard-focus-${Date.now()}`;
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
    // Fixed menu order (rename, clone, …), not the seen-state feature:
    // this test steps the real item order by position, and a live
    // `seen_activity_at` classification would insert mark-seen between
    // rename and clone on the real classifier's own clock.
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
    let createPosts = 0;
    const countCreates = (issued: import("@playwright/test").Request) => {
      if (issued.method() === "POST" && new URL(issued.url()).pathname === "/api/sessions") {
        createPosts += 1;
      }
    };
    page.on("request", countCreates);
    try {
      for (const key of ["Enter", " "]) {
        const toggle = sourceRow.locator(".session-row-menu");
        await toggle.focus();
        await expect(toggle).toBeFocused();
        await page.keyboard.press("ArrowDown");
        await expect(sourceRow.locator(".session-row-menu-panel")).toBeVisible();
        // Opening from the closed toggle lands on the first item; one
        // step down the real menu order (rename, clone, …) reaches clone.
        await expect(sourceRow.locator(".session-row-rename")).toBeFocused();
        await page.keyboard.press("ArrowDown");
        await expect(sourceRow.locator(".session-row-clone")).toBeFocused();
        await page.keyboard.press(key);
        const form = page.locator('.create-session-form[role="dialog"]');
        await expect(form).toBeVisible();
        const search = form.locator('.launch-composer-search input[role="combobox"]');
        await expect(search).toBeEnabled();
        await expect(page.locator(".session-row-menu-panel")).toHaveCount(0);
        await expect(search).toBeFocused();
        const query = `clone-kb-${key === " " ? "space" : "enter"}-${Date.now()}`;
        await page.keyboard.type(query);
        await expect(search).toHaveValue(query);
        // A no-result query still owns the first Escape; only the second
        // dismisses the dialog (the sidebar composer's own contract).
        await page.keyboard.press("Escape");
        await page.keyboard.press("Escape");
        await expect(form).toHaveCount(0);
      }
      expect(createPosts, "opening and typing into keyboard clones must not launch them").toBe(0);
    } finally {
      page.off("request", countCreates);
      await attachFocusTrace(page, testInfo, "clone-keyboard-focus-trace.json");
    }
  } finally {
    await cleanupSession(request, sourceId);
  }
});

/**
 * A deliberately entered field keeps focus when late data renders.
 *
 * The opening handoff owns INITIAL focus; once the user deliberately moves
 * into another field, later catalog, host, or history renders must not
 * steal it back — not to search, not anywhere. This holds the page's
 * launch-catalog read from before navigation, opens a structured clone,
 * deliberately clicks into the name field and types, and only then
 * releases the catalog (fulfilled with fixture-only models, so the
 * reconciled render is provably the held reply's doing). Focus and text
 * must survive without any refocus after release.
 */
test("a deliberately entered name field keeps focus through the late catalog render", async ({
  page,
  request,
}, testInfo) => {
  const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(build, "fabricated catalog replies must retain the helm build stamp").toBeTruthy();
  const local = await localHostId(request);
  const cwd = stackScratchDir("clone-deliberate-focus-");
  const title = `clone-deliberate-focus-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: {
      cwd,
      title,
      host: local,
      launch: { harness: "codex", model: "gpt-6-astra", effort: "high" },
    },
  });
  expect(created.ok(), `creating structured source: ${await created.text()}`).toBe(true);
  const sourceId = (await created.json()).id as string;
  const catalogGate = routeGate();
  let catalogGets = 0;
  // Active invocations of the owned handler below. `page.unroute` stops new
  // dispatches but does not wait for calls already running, so cleanup
  // drains this set explicitly — otherwise a held `route.fulfill` can race
  // page teardown and bury the real failure under a target-closed error.
  const catalogInFlight = new Set<Promise<void>>();
  // Named matcher and handler so cleanup removes exactly this route.
  const catalogMatcher = (url: URL) => url.pathname === "/api/launch-catalog";
  const catalogHandler = async (route: import("@playwright/test").Route) => {
    const run = (async () => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      catalogGets += 1;
      await catalogGate.wait();
      await route.fulfill({
        status: 200,
        headers: { "content-type": "application/json", "x-farhelm-build": build },
        body: JSON.stringify([
          { id: "deliberate-focus-codex", harness: "codex", efforts: ["high"] },
          { id: "deliberate-focus-claude", harness: "claude", efforts: ["low"] },
        ]),
      });
    })();
    const tracked = run.catch(() => {});
    catalogInFlight.add(tracked);
    try {
      await run;
    } finally {
      catalogInFlight.delete(tracked);
    }
  };
  try {
    await page.route(catalogMatcher, catalogHandler);
    try {
      await page.goto("/");
      const sourceRow = row(page, sourceId);
      await expect(sourceRow).toBeVisible({ timeout: 20_000 });
      await attachSession(page, sourceId);
      await waitForTermText(page, "FAKE-AGENT READY");
      await expect(
        page.locator(".hosts-status", { hasText: "loading hosts" }),
      ).toHaveCount(0, { timeout: 20_000 });
      await installFocusTrace(page);
      await openRowMenu(sourceRow);
      await sourceRow.locator(".session-row-clone").click();
      const form = page.locator('.create-session-form[role="dialog"]');
      await expect(form).toBeVisible();
      await expect(form.locator('.launch-composer-search input[role="combobox"]')).toBeFocused();
      // The deliberate move, before any catalog bytes exist: click into
      // the name field (a trusted pointer move, not a programmatic
      // focus) and type through page-level keystrokes.
      const name = form.getByLabel("name (optional)");
      await name.click();
      await expect(name).toBeFocused();
      // The click caret lands wherever it lands; End puts it after the
      // prefilled title so the typed suffix appends deterministically.
      await page.keyboard.press("End");
      const suffix = `-deliberate-${Date.now()}`;
      await page.keyboard.type(suffix);
      await expect(name).toHaveValue(title + suffix);
      // Entered receipt: the page's catalog read is still held — no
      // catalog content could have rendered yet.
      await expect.poll(() => catalogGets).toBeGreaterThan(0);
      const effortButtons = form.locator(".launch-composer-effort-choice").getByRole("button");
      const effortsBefore = await effortButtons.count();
      catalogGate.release();
      // Consumption receipt: the held reply's vocabulary reached the
      // rendered effort choice — its button set changes once the
      // fixture-only catalog lands. (The prefilled model and summary
      // do NOT change here: an unknown model is preserved as an
      // arbitrary choice, so neither can witness the render.)
      await expect.poll(() => effortButtons.count()).not.toBe(effortsBefore);
      // The deliberate field kept focus and text throughout, with no
      // refocus after release.
      await expect(name).toBeFocused();
      await expect(name).toHaveValue(title + suffix);
    } finally {
      // Unconditional drain: release the gate, remove exactly the owned
      // route, and await its already-running invocations no matter where
      // the test failed — navigation, visibility, terminal readiness, host
      // settlement, trace installation, or the assertions above.
      // `allSettled` never throws, so a draining failure cannot skip
      // diagnostics; session cleanup in the outer finally still runs even
      // if diagnostics fail below.
      catalogGate.release();
      await page.unroute(catalogMatcher, catalogHandler);
      await Promise.allSettled([...catalogInFlight]);
      await attachFocusTrace(page, testInfo, "clone-deliberate-focus-trace.json");
    }
  } finally {
    await cleanupSession(request, sourceId);
  }
});

/**
 * A clone transfer retires the originating dismissal's toggle return.
 *
 * The delayed-handoff twin of the opening-focus tests: instead of only
 * asserting search won, arm a gate holding any row-toggle focus delivery,
 * open the clone, and prove NOTHING was held — the retirement receipt for
 * a branch that must emit no toggle eval at all. The same gate then proves
 * it is live (and the ordinary dismissal preserved) by catching the toggle
 * return an Escape from a focused menu item must still issue.
 */
test("a clone transfer retires the toggle return while ordinary Escape keeps it", async ({
  page,
  request,
}, testInfo) => {
  const title = `clone-transfer-retirement-${Date.now()}`;
  const cwd = stackScratchDir("clone-transfer-retirement-");
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
    await armFocusGate(page, "toggle");
    try {
      await openRowMenu(sourceRow);
      await sourceRow.locator(".session-row-clone").click();
      const form = page.locator('.create-session-form[role="dialog"]');
      await expect(form).toBeVisible();
      const search = form.locator('.launch-composer-search input[role="combobox"]');
      await expect(search).toBeFocused();
      const query = `clone-gate-probe-${Date.now()}`;
      await page.keyboard.type(query);
      await expect(search).toHaveValue(query);
      // Retirement receipt: the transfer's dismissal attempted no toggle
      // return while search took and kept focus through real typing.
      expect(await focusGateHeldCount(page), "the transfer must retire its toggle return").toBe(0);
      await page.keyboard.press("Escape");
      await page.keyboard.press("Escape");
      await expect(form).toHaveCount(0);
      // Liveness: the same gate catches the ordinary return an Escape
      // from a focused item still owes the toggle — proving the trap
      // works and the dismissal survives outside transfers.
      await openRowMenu(sourceRow);
      await expect(sourceRow.locator(".session-row-rename")).toBeFocused();
      await page.keyboard.press("Escape");
      await expect(sourceRow.locator(".session-row-menu-panel")).toHaveCount(0);
      await expect.poll(() => focusGateHeldCount(page)).toBe(1);
      const receipts = await releaseFocusGate(page);
      expect(receipts).toHaveLength(1);
      expect(receipts[0].target).toContain("session-row-menu");
      expect(receipts[0].connectedBefore).toBe(true);
      await expect(sourceRow.locator(".session-row-menu")).toBeFocused();
    } finally {
      await releaseFocusGate(page);
      await attachFocusTrace(page, testInfo, "clone-transfer-retirement-trace.json");
    }
  } finally {
    await cleanupSession(request, source.id);
  }
});

/**
 * A closed opening's held focus work stays stale across a reopen.
 *
 * Holds opening A's actual mount focus deliveries at the DOM boundary,
 * closes A, reopens the SAME source as B, deliberately enters B's name
 * field, and only then releases A: the released calls must name detached
 * nodes (stale no-ops) while B keeps its own handoff, focus, and text.
 * Same-source reopen also repeats prefill generation zero, the shape a
 * generation-keyed identity would confuse with the older opening.
 */
test("a closed opening's held focus work stays stale when released after reopen", async ({
  page,
  request,
}, testInfo) => {
  const title = `clone-stale-opening-${Date.now()}`;
  const cwd = stackScratchDir("clone-stale-opening-");
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
    // Armed before opening A so both mount deliveries are captured no
    // matter how fast the bridge runs; frozen to A's dialog once they
    // arrive so B's own mount work passes through.
    await armFocusGate(page, "dialog");
    try {
      await openRowMenu(sourceRow);
      await sourceRow.locator(".session-row-clone").click();
      const form = page.locator('.create-session-form[role="dialog"]');
      await expect(form).toBeVisible();
      // Entered receipt: A's complete opening work arrived at the gate —
      // the parent selector eval and the child mounted focus.
      await expect.poll(() => focusGateHeldCount(page)).toBe(2);
      await freezeDialogGate(page);
      // A's search never focuses (its deliveries are held), so close
      // through the visible Cancel: Escape cannot reach a dialog no
      // focused control sits inside.
      await form.getByRole("button", { name: "cancel", exact: true }).click();
      await expect(form).toHaveCount(0);
      // Opening B, same source: its own mount work passes the frozen
      // gate (a different dialog node) and must complete its handoff.
      await openRowMenu(sourceRow);
      await sourceRow.locator(".session-row-clone").click();
      await expect(form).toBeVisible();
      const search = form.locator('.launch-composer-search input[role="combobox"]');
      await expect(search).toBeFocused();
      // Deliberate newer intent before A's release.
      const name = form.getByLabel("name (optional)");
      await name.click();
      await expect(name).toBeFocused();
      await page.keyboard.press("End");
      const suffix = `-reopen-${Date.now()}`;
      await page.keyboard.type(suffix);
      await expect(name).toHaveValue(title + suffix);
      // A's full accounting is still exactly its two mount deliveries —
      // no straggler arrived across the close/reopen boundary.
      expect(await focusGateHeldCount(page)).toBe(2);
      const receipts = await releaseFocusGate(page);
      expect(receipts).toHaveLength(2);
      for (const receipt of receipts) {
        expect(
          receipt.connectedBefore,
          `released call ${receipt.target} must name A's detached search`,
        ).toBe(false);
      }
      // B retains everything: focus, text, and its own handoff — A's
      // release delivered two detached no-ops and consumed nothing.
      await expect(name).toBeFocused();
      await expect(name).toHaveValue(title + suffix);
    } finally {
      await releaseFocusGate(page);
      await attachFocusTrace(page, testInfo, "clone-stale-opening-trace.json");
    }
  } finally {
    await cleanupSession(request, source.id);
  }
});

/**
 * Hold exactly one reveal for the primary terminal at its real replay
 * marker, release it through the production path, and observe the
 * application's own reveal receipt. Mirrors `profiles.spec.ts`, the
 * origin of this pattern; the trio is repeated here rather than shared
 * for the same reason `row()` is local to each spec that needs it.
 */
async function holdPrimaryTerminalReveal(page: Page) {
  await page.addInitScript(() => {
    (window as any).__farhelmTestReplay = {
      holdMarker: true,
      targetEl: "terminal",
      idleMs: 60_000,
    };
  });
}

/** Wait until the selected primary terminal has reached the held marker. */
async function waitForHeldPrimaryReveal(page: Page) {
  await expect
    .poll(
      () =>
        page.evaluate(
          () => (window as any).__farhelmIslands?.terminal?.test?.replay?.heldReason ?? null,
        ),
      { timeout: 60_000 },
    )
    .toBe("marker");
}

/** Release the primary terminal through terminal.js's production reveal path. */
async function releasePrimaryReveal(page: Page) {
  await page.evaluate(() => (window as any).__farhelmIslands.terminal.test.releaseCatchUp());
}

/** Release the held reveal if one is armed; never throws, for `finally` use. */
async function releasePrimaryRevealQuietly(page: Page) {
  await page.evaluate(() => (window as any).__farhelmIslands?.terminal?.test?.releaseCatchUp?.());
}

/**
 * Composer search survives a held terminal reveal completing late.
 *
 * The opening-focus fixtures settle the terminal before activating; this
 * is the complementary order — hold the primary reveal at its real replay
 * marker, open the clone and establish search focus with typed text, then
 * release through the production path and prove search kept focus and
 * text with no refocus after the application's own reveal receipt.
 */
test("composer search survives a held terminal reveal", async ({ page, request }, testInfo) => {
  const title = `clone-held-reveal-${Date.now()}`;
  const cwd = stackScratchDir("clone-held-reveal-");
  const source = await createSession(request, { title, cwd });
  // The init script must precede navigation to catch the primary island.
  await holdPrimaryTerminalReveal(page);
  try {
    await page.goto("/");
    const sourceRow = row(page, source.id);
    await expect(sourceRow).toBeVisible({ timeout: 20_000 });
    // Select to start the primary reveal; it holds at the replay marker
    // instead of completing. No READY wait: the held phase is the
    // premise, and readiness may legitimately wait for the release.
    await sourceRow.locator(".session-row-open").click();
    await expect(sourceRow).toHaveAttribute("data-session-selected", "true", { timeout: 20_000 });
    await waitForHeldPrimaryReveal(page);
    await expect(
      page.locator(".hosts-status", { hasText: "loading hosts" }),
    ).toHaveCount(0, { timeout: 20_000 });
    await installFocusTrace(page);
    try {
      await openRowMenu(sourceRow);
      await sourceRow.locator(".session-row-clone").click();
      const form = page.locator('.create-session-form[role="dialog"]');
      await expect(form).toBeVisible();
      const search = form.locator('.launch-composer-search input[role="combobox"]');
      await expect(search).toBeFocused();
      const prefix = `reveal-prefix-${Date.now()}`;
      await page.keyboard.type(prefix);
      await expect(search).toHaveValue(prefix);
      await releasePrimaryReveal(page);
      await expect
        .poll(() =>
          page.evaluate(() => (window as any).__farhelmIslands.terminal.test.replay.revealed)
        )
        .toBe(true);
      // The delayed reveal completed through the production path; search
      // kept focus and text throughout, with no refocus after release.
      await expect(search).toBeFocused();
      await expect(search).toHaveValue(prefix);
      const suffix = "-after-reveal";
      await page.keyboard.type(suffix);
      await expect(search).toHaveValue(prefix + suffix);
    } finally {
      await releasePrimaryRevealQuietly(page);
      await attachFocusTrace(page, testInfo, "clone-held-reveal-trace.json");
    }
  } finally {
    await cleanupSession(request, source.id);
  }
});

test("clone pre-fills the create form from a profile-backed row, and the edited copy leaves the original untouched", async ({
  page,
  request,
}) => {
  const invocationA = FAKE_AGENT;
  const invocationB = FAKE_AGENT.replace("--script basic", "--script altscreen");
  const profileA = await createProfile(request, {
    name: `clone-profile-a-${Date.now()}`,
    invocation: invocationA,
  });
  const profileB = await createProfile(request, {
    name: `clone-profile-b-${Date.now()}`,
    invocation: invocationB,
  });
  const title = `clone-source-${Date.now()}`;
  const originalCwd = "/tmp";
  const original = await createSession(request, {
    title,
    cwd: originalCwd,
    profile_id: profileA.id,
  });
  // A THROWAWAY session created from a DIFFERENT profile afterwards, purely
  // to move the helm's remembered default onto B while the row this test
  // clones still names A. Without this, "the row's own profile" and "the
  // helm's remembered default" would be the same id, and a broken
  // implementation that discards the clone's own choice and seeds only
  // from the remembered default would select the identical profile — the
  // exact regression this fixture exists to distinguish from the correct
  // behavior.
  const rememberedDefaultShift = await createSession(request, {
    title: `clone-remembered-default-${Date.now()}`,
    cwd: "/tmp",
    profile_id: profileB.id,
  });
  let cloneId: string | undefined;
  try {
    await page.goto("/");
    const source = row(page, original.id);
    await expect(source).toBeVisible({ timeout: 20_000 });

    await openRowMenu(source);
    await source.locator(".session-row-clone").click();

    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue(originalCwd);
    await expect(form.getByLabel("name (optional)")).toHaveValue(title);
    // The row's OWN profile (A) wins the picker over the helm's remembered
    // default (B, moved there by the throwaway session above) — see that
    // fixture's own comment for why the two must differ for this
    // assertion to mean anything.
    await expect(form.locator(".create-session-profile")).toHaveValue(profileA.id, {
      timeout: 20_000,
    });
    // Substring match on purpose: this field's accessible name grows a
    // parenthetical while a profile is selected (asserted via `commandLabel`
    // below), and both spellings start with "agent command".
    const command = form.getByLabel("agent command");
    // The label is located from the input upward: a `has` filter rooted at the
    // form can never match, because the inner locator would be re-rooted at
    // each candidate label and the form is not inside its own label.
    const commandLabel = command.locator("xpath=..");
    await expect(command).toBeDisabled();
    await expect(command).toHaveValue(invocationA);
    await expect(commandLabel).toContainText(
      'agent command (the selected profile\'s own; choose "custom command" above to edit)',
    );

    // The destination belongs to the shared shell, while the two launch
    // drafts retain their own values across a round trip through a harness.
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await expect(form).toHaveAttribute("data-composer-mode", "structured");
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue(originalCwd);
    await expect(form.getByLabel("name (optional)")).toHaveValue(title);
    await chooseCommandMode(form);
    await expect(form.locator(".create-session-profile")).toHaveValue(profileA.id);
    await expect(command).toHaveValue(invocationA);

    await form.locator(".create-session-profile").selectOption(profileB.id);
    await expect(command).toHaveValue(invocationB);
    await expect(command).toBeDisabled();

    await form.locator(".create-session-profile").selectOption("");
    await expect(command).toBeEnabled();
    await expect(command).toHaveValue(invocationA);
    await expect(commandLabel).toHaveText("agent command");

    // Keep the original profile-backed submit assertion below meaningful after
    // the custom-mode display assertions above.
    await form.locator(".create-session-profile").selectOption(profileA.id);

    const newCwd = stackScratchDir("clone-e2e-");
    await form.getByLabel("folder", { exact: true }).fill(newCwd);
    const [response] = await Promise.all([
      page.waitForResponse(
        (r) => r.request().method() === "POST" && r.url().endsWith("/api/sessions"),
      ),
      form.locator(".create-session-submit").click(),
    ]);
    // The wire body itself, not just what the picker showed — the two
    // could disagree if the reseed effect wrote the picker's display
    // without also writing what submit actually reads.
    expect(response.request().postDataJSON().profile_id).toBe(profileA.id);
    const body = await response.json();
    cloneId = body.id as string;

    const cloned = row(page, cloneId);
    await expect(cloned).toBeVisible({ timeout: 20_000 });
    await expect(cloned.locator(".session-title")).toHaveText(title);
    await expect(cloned.locator(".session-cwd")).toHaveAttribute("title", newCwd);

    // The original row: same directory, same title, still there — cloning
    // must not have touched it.
    await expect(source).toBeVisible();
    await expect(source.locator(".session-cwd")).toHaveAttribute("title", originalCwd);
    await expect(source.locator(".session-title")).toHaveText(title);
  } finally {
    if (cloneId) await cleanupSession(request, cloneId);
    await cleanupSession(request, rememberedDefaultShift.id);
    await cleanupSession(request, original.id);
    await cleanupProfile(request, profileA.id);
    await cleanupProfile(request, profileB.id);
  }
});

/**
 * A deleted profile is not evidence for substituting another profile. Clone
 * must keep the source invocation exact and surface command mode so a person
 * can review or edit the fallback before a request is sent.
 */
test("a clone of a missing-profile session opens other command with its exact invocation", async ({ page, request }) => {
  const invocation = FAKE_AGENT.replace("--script basic", "--script altscreen");
  const profile = await createProfile(request, {
    name: `clone-missing-profile-${Date.now()}`,
    invocation,
  });
  const source = await createSession(request, {
    title: `clone-missing-profile-source-${Date.now()}`,
    cwd: "/tmp",
    profile_id: profile.id,
  });
  try {
    await cleanupProfile(request, profile.id);
    await page.goto("/");
    const sourceRow = row(page, source.id);
    await expect(sourceRow).toBeVisible({ timeout: 20_000 });
    await openRowMenu(sourceRow);
    await sourceRow.locator(".session-row-clone").click();

    const form = page.locator(".create-session-form");
    await expect(form).toHaveAttribute("data-composer-mode", "command");
    await expect(form.locator(".create-session-profile")).toHaveValue("");
    await expect(form.getByLabel("agent command")).toHaveValue(invocation);
  } finally {
    await cleanupSession(request, source.id);
    await cleanupProfile(request, profile.id);
  }
});

/**
 * Each clone must restore every prefilled field, including when the same row
 * is cloned again after an intervening edit. The composer is a real modal,
 * so a row menu behind it is deliberately unreachable; this test closes and
 * remounts the draft between clone actions through the only user-reachable
 * route. It covers remount seeding, while renderer-free generation tests own
 * reseeding an already-mounted component.
 *
 * Pure generation checks cannot show whether Dioxus re-seeds the visible
 * signals on each dialog mount. The assertions below keep that renderer
 * contract covered without making the modal leak pointer access to its
 * obscured sidebar.
 */
test("each clone restores every field after an intervening draft edit", async ({
  page,
  request,
}) => {
  const local = await localHostId(request);
  const titleA = `clone-reseed-a-${Date.now()}`;
  const titleB = `clone-reseed-b-${Date.now()}`;
  // Allocated inside the stack's own state dir rather than a literal
  // `/tmp/...` path: a clean runner never creates that directory, and
  // farhelm's create precondition would refuse the fixture session before
  // this test ever reached the browser.
  let cwdA: string | undefined;
  let cwdB: string | undefined;
  let sessionA: SessionRow | undefined;
  let sessionB: SessionRow | undefined;
  let cloneId: string | undefined;
  try {
    cwdA = stackScratchDir("clone-reseed-a-");
    sessionA = await createSession(request, { title: titleA, cwd: cwdA });
    cwdB = stackScratchDir("clone-reseed-b-");
    sessionB = await createSession(request, { title: titleB, cwd: cwdB });

    await page.goto("/");
    const rowA = row(page, sessionA.id);
    const rowB = row(page, sessionB.id);
    await expect(rowA).toBeVisible({ timeout: 20_000 });
    await expect(rowB).toBeVisible({ timeout: 20_000 });

    await openRowMenu(rowA);
    await rowA.locator(".session-row-clone").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-host")).toHaveValue(String(local));
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue(cwdA);
    await expect(form.getByLabel("name (optional)")).toHaveValue(titleA);
    await expect(form.getByLabel("agent command")).toHaveValue(FAKE_AGENT);

    // Edit every field before cloning again — a partial reseed (one field
    // replaced, another left at this edit) would otherwise be invisible
    // to an assertion that only checked the fields B's clone changes.
    await form.getByLabel("folder", { exact: true }).fill("/tmp/edited-in-between");
    await form.getByLabel("agent command").fill("sleep 999");
    await fillCloneTitle(form, "edited in between");

    // A modal keeps its obscured sidebar inert. Cancel this edited draft
    // before selecting row B; reopening must still replace every field with
    // B's prefill rather than retaining A's edits in component state.
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);
    await openRowMenu(rowB);
    await rowB.locator(".session-row-clone").click();
    await expect(form.locator(".create-session-host")).toHaveValue(String(local));
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue(cwdB);
    await expect(form.getByLabel("name (optional)")).toHaveValue(titleB);
    await expect(form.getByLabel("agent command")).toHaveValue(FAKE_AGENT);

    // Edit again, close, then clone B a second time. A later mount of the
    // same source must not restore the abandoned draft merely because its
    // prefill resembles the previous clone.
    await form.getByLabel("folder", { exact: true }).fill("/tmp/edited-again");
    await fillCloneTitle(form, "edited again");
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);
    await openRowMenu(rowB);
    await rowB.locator(".session-row-clone").click();
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue(cwdB);
    await expect(form.getByLabel("name (optional)")).toHaveValue(titleB);

    const newCwd = stackScratchDir("clone-reseed-e2e-");
    cloneId = await submitClonedCwd(page, form, newCwd);
    await expect(row(page, cloneId)).toBeVisible({ timeout: 20_000 });
  } finally {
    if (cloneId) await cleanupSession(request, cloneId);
    if (sessionA) await cleanupSession(request, sessionA.id);
    if (sessionB) await cleanupSession(request, sessionB.id);
  }
});

/**
 * `clone_prefill` is stored above the create form so it survives while the
 * form stays open (`list::view::ListView`), which leaves it exactly two
 * cleanup paths: cancelling through the dialog action, and a successful
 * create. Either one skipped, or ordered wrong, would let the NEXT ordinary
 * New inherit clone-only title and agent state. Its directory remains the
 * deliberate ordinary-New context: the currently selected session's folder.
 * "Fresh defaults" means the harness unpreselected; the permissions segment
 * carries the helm-wide remembered mode on every fresh open, which is not
 * clone state and is not what this test checks.
 */
test("closing a clone without submitting, or submitting it, both leave the next New Session with fresh defaults", async ({
  page,
  request,
}) => {
  const local = await localHostId(request);
  const title = `clone-prefill-cleanup-${Date.now()}`;
  const newSessionButton = page.locator(".new-session-button");
  let sourceCwd: string | undefined;
  let session: SessionRow | undefined;
  let cloneId: string | undefined;
  try {
    // Allocated inside the stack's own state dir, like every other fixture
    // directory in this file — a literal `/tmp/...` path is never created
    // for the test, so the source session's create would be refused before
    // the browser had anything to clone.
    sourceCwd = stackScratchDir("clone-prefill-cleanup-");
    session = await createSession(request, { title, cwd: sourceCwd });

    await page.goto("/");
    const source = row(page, session.id);
    await expect(source).toBeVisible({ timeout: 20_000 });

    const form = page.locator(".create-session-form");
    const assertFreshDefaults = async (expectedCwd: string) => {
      await expect(form).toBeVisible();
      await expect(form.locator(".create-session-host")).toHaveValue(String(local));
      await expect(form.getByLabel("folder", { exact: true })).toHaveValue(expectedCwd);
      await expect(form.getByRole("button", { name: "Codex", exact: true })).toHaveAttribute(
        "aria-pressed",
        "false",
      );
      await expect(form.locator(".create-session-submit")).toBeDisabled();
    };

    // (a) Clone, then cancel through the dialog action. The modal keeps its
    // opener behind an inert backdrop, so that visible Cancel control is the
    // user-reachable route that clears `clone_prefill`. Reopening must show
    // ordinary structured choices, not the cancelled clone's title or
    // agent. The selected source row still deliberately supplies its folder.
    await openRowMenu(source);
    await source.locator(".session-row-clone").click();
    await expect(form).toBeVisible();
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue(sourceCwd);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);
    await newSessionButton.click();
    await assertFreshDefaults(sourceCwd);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);

    // (b) Clone, submit it successfully, then reopen: the same fresh
    // defaults, not the just-submitted clone's fields either — the other
    // path that clears `clone_prefill` (`on_created`).
    await openRowMenu(source);
    await source.locator(".session-row-clone").click();
    await expect(form).toBeVisible();
    const newCwd = stackScratchDir("clone-prefill-cleanup-e2e-");
    cloneId = await submitClonedCwd(page, form, newCwd);
    await expect(row(page, cloneId)).toBeVisible({ timeout: 20_000 });

    await newSessionButton.click();
    await assertFreshDefaults(newCwd);
  } finally {
    if (cloneId) await cleanupSession(request, cloneId);
    if (session) await cleanupSession(request, session.id);
  }
});
