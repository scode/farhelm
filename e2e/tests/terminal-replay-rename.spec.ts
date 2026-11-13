// ---------------------------------------------------------------------
// M5: replay presentation and rename (PLAN_M5.md items 5 and 6)
//
// This file has its own stack reset, so the replay tests can fill their
// sessions' scrollback without polluting another section.
//
// Several of these tests HOLD a catch-up phase open (see
// `holdCatchUpFromNextLoad`) rather than racing it. That is what makes the
// acceptance assertable at all: the contract is about what is on screen
// DURING the catch-up, and a real supervisor's marker arrives too fast to
// observe from Node — the window is not merely short, it has no lower
// bound this suite controls.
// ---------------------------------------------------------------------


import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { type Page } from "@playwright/test";
import { createSession, openRowMenu, SESSION_LISTING, stubFeed } from "./helpers/fleet";
import { attachSession, cleanupSession, termText, waitForTermText, terminalInput, programInput, type InputFrame, recordInput } from "./helpers/term";
import {
  addTab,
  armTerminalFocusSpy,
  closeTabAndAwaitAgentFallback,
  createTabSession,
  disableReconnectFromNextLoad,
  FAKE_AGENT_INVOCATION,
  fulfillAsHelm,
  installTerminalFocusSpy,
  replayRecord,
  runInShell,
  selectTerminal,
  selectTabOverRevealedAgent,
  sharedSessionRow,
  shellMarker,
  terminalFocusCount,
  waitForIslandText,
  waitForReplayReveal,
  installTerminalSuiteHooks,
} from "./helpers/terminal-suite";
import { waitForSessionMounted, waitForSessionRevealed } from "./helpers/terminal-readiness";

installTerminalSuiteHooks({ tabSweep: true });

/** Rename shows the exact title's hidden characters, in the current title and
 * in the field it seeds, without changing the stored title: the field starts
 * escaped so an agent-set title cannot hide characters from the user who edits
 * it. A peer-authored refusal uses the same isolated display rule. A short
 * phone frame must keep Close and both answers reachable beside that draft. */
test("rename exposes peer title and refusal characters and seeds an escaped draft", async ({ page, request }) => {
  const title = `rename-peer-${Date.now()}\u202Etitle\u200B`;
  const shown = title.replace("\u202E", "<U+202E>").replace("\u200B", "<U+200B>");
  const session = await createTabSession(request, title);
  await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/rename`, async (route) => {
    await fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: "refused \u202Ehost\u200B" });
  });
  try {
    await page.setViewportSize({ width: 390, height: 320 });
    await page.goto("/");
    const row = page.locator(`[data-session-id="${session.id}"]`);
    await expect(row.locator(".session-title")).toHaveText(shown);
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    const dialog = page.locator(".rename-dialog");
    const current = dialog.locator(".rename-current-title");
    const draft = dialog.locator(".rename-input");
    await expect(current).toHaveText(shown);
    await expect(current).toHaveAttribute("dir", "ltr");
    expect(await current.evaluate((node) => getComputedStyle(node).unicodeBidi)).toBe("isolate");
    await expect(draft).toHaveValue(shown);
    await expect(draft).toBeFocused();
    await expect(dialog.locator(".dialog-header .rename-close")).toBeInViewport({ ratio: 1 });
    await expect(dialog.locator(".dialog-footer .rename-cancel")).toBeInViewport({ ratio: 1 });
    await expect(dialog.locator(".dialog-footer .rename-submit")).toBeInViewport({ ratio: 1 });
    await draft.fill(`${title}-edited`);
    await draft.press("Enter");
    const error = dialog.locator(".rename-error .peer-value");
    await expect(error).toHaveText("refused <U+202E>host<U+200B>");
    await expect(error).toHaveAttribute("dir", "ltr");
    expect(await error.evaluate((node) => getComputedStyle(node).unicodeBidi)).toBe("isolate");
    await expect(draft).toHaveValue(`${title}-edited`);
    await expect(dialog.locator(".dialog-footer .rename-cancel")).toBeInViewport({ ratio: 1 });
    await expect(dialog.locator(".dialog-footer .rename-submit")).toBeInViewport({ ratio: 1 });
    const response = await request.get(`/api/sessions/${encodeURIComponent(session.id)}`);
    expect(response.ok()).toBe(true);
    expect((await response.json()).title).toBe(title);
  } finally {
    await cleanupSession(request, session.id);
  }
});
/**
 * Arm terminal.js's test-only replay controls for every attach the page
 * makes from its NEXT navigation onward (`window.__farhelmTestReplay`,
 * read once per mount — see `replayControls` in terminal.js).
 *
 * An init script rather than a plain `evaluate`, because the attaches
 * these tests care about happen after a reload, which would wipe a global
 * set on the current document.
 *
 * `holdMarker` keeps a catch-up phase open past the marker that would
 * normally end it — the seam every hidden-state assertion below depends
 * on. The idle limit lets the watchdog bound be crossed in milliseconds
 * instead of seconds: no fixture in this suite can make a real supervisor
 * go silent mid-catch-up, so without it the bound would be untested rather
 * than tested loosely.
 *
 * The idle window is stretched to a minute by default, and that is not
 * belt-and-braces: holding a phase open means NOTHING else may end it out
 * from under the assertions, and once the real replay is buffered the
 * stream is by definition silent — so the production five-second watchdog
 * would flush the buffer partway through a test's own round trips, turning
 * a held phase into an `idle` one at random. A test that wants the idle
 * bound passes a shorter `idleMs` of its own.
 */
async function holdCatchUpFromNextLoad(
  page: Page,
  overrides: { idleMs?: number } = {},
) {
  await page.addInitScript((overrides) => {
    (window as any).__farhelmTestReplay = {
      holdMarker: true,
      idleMs: 60_000,
      ...overrides,
    };
  }, overrides);
}

/**
 * Wait until this island's catch-up phase is being HELD open — the replay
 * has arrived and been applied to the still-hidden terminal, and the marker
 * that would have ended the phase has been recorded instead of acted on.
 *
 * This is the synchronization point the hidden-state assertions need:
 * "mid-catch-up" is otherwise a moment no test can reliably be inside.
 */
async function waitForHeldCatchUp(page: Page, elementId: string) {
  await expect
    .poll(
      () =>
        page.evaluate(
          (el) =>
            (window as any).__farhelmIslands?.[el]?.test?.replay?.heldReason ?? null,
          elementId,
        ),
      {
        timeout: 60_000,
        message: `waiting for ${elementId} to be holding its catch-up open`,
      },
    )
    .toBe("marker");
}

/** Let a held catch-up phase finish, applying the ending it deferred. */
async function releaseCatchUp(page: Page, elementId: string) {
  await page.evaluate(
    (el) => (window as any).__farhelmIslands[el].test.releaseCatchUp(),
    elementId,
  );
}

/**
 * Replace `window.WebSocket`, from the page's next navigation onward, with
 * one that never finishes connecting: it stays in CONNECTING forever, and
 * only `close()` ever moves it.
 *
 * The one failure this suite cannot stage any other way. A real helm
 * either completes the handshake or refuses it — "the socket is still
 * negotiating and always will be" is a network state, not a server
 * behavior, and it is precisely the state in which a terminal has no path
 * for input at all.
 *
 * The static readyState constants are part of the shim because
 * terminal.js reads them off the GLOBAL `WebSocket` (`ws.readyState ===
 * WebSocket.OPEN`), which is this class once it is installed — omitting
 * them would make every liveness check compare against `undefined` and
 * quietly answer "not open" for the wrong reason.
 */
async function stuckWebSocketFromNextLoad(page: Page) {
  await page.addInitScript(() => {
    class StuckWebSocket {
      static CONNECTING = 0;
      static OPEN = 1;
      static CLOSING = 2;
      static CLOSED = 3;
      url: string;
      readyState = 0;
      binaryType = "blob";
      onopen: ((ev: unknown) => void) | null = null;
      onmessage: ((ev: unknown) => void) | null = null;
      onclose: ((ev: unknown) => void) | null = null;
      onerror: ((ev: unknown) => void) | null = null;
      constructor(url: string) {
        this.url = url;
      }
      send() {}
      close() {
        this.readyState = 3;
        const onclose = this.onclose;
        // Asynchronously, like the real close handshake — a synchronous
        // callback would run inside the caller that just closed it.
        // sleep-ok: schedule the fake close event after close returns, preserving the real socket's non-reentrant callback boundary.
        if (onclose) setTimeout(() => onclose({}), 0);
      }
      addEventListener() {}
      removeEventListener() {}
    }
    (window as any).WebSocket = StuckWebSocket;
  });
}

/**
 * From the page's next navigation onward, hold the main thread busy for
 * `blockMs` straight after the FIRST terminal socket for `sessionId` is
 * constructed, and never again.
 *
 * This stages the island-cap flake's first-mount shape without its 33
 * terminals or a loaded machine. The block runs as a microtask, so it
 * starts only after the mount that constructed the socket has returned.
 * By then the mount has armed the catch-up watchdog. The helm finishes
 * the handshake while the page is blocked, and when the block ends the
 * page holds an overdue watchdog expiry alongside the socket's queued
 * `open` and first frames. That is the moment the watchdog must not take
 * for silence.
 *
 * Keyed to one session because the page may attach another session
 * first. On load it opens whichever session it last had selected, which
 * in a suite that shares a stack can be another test's, and blocking or
 * counting that socket would test nothing.
 *
 * Every terminal socket the page constructs for the session is counted in
 * `window.__farhelmTermSocketsOpened`, so a test can tell the original
 * attachment surviving from a reconnect that replaced it: both end in a
 * revealed terminal.
 *
 * A subclass rather than a stand-in (compare `stuckWebSocketFromNextLoad`):
 * everything after the block has to be a real attachment for the test to
 * mean anything, and subclassing keeps the static readyState constants
 * terminal.js reads off the global.
 */
async function starveAfterFirstTerminalSocketFromNextLoad(
  page: Page,
  sessionId: string,
  blockMs: number,
) {
  await page.addInitScript(
    ({ sessionId, blockMs }) => {
      const Real = window.WebSocket;
      const prefix = `/api/sessions/${encodeURIComponent(sessionId)}/term`;
      let spent = false;
      class Starving extends Real {
        constructor(url: string | URL, protocols?: string | string[]) {
          super(url, protocols);
          if (!new URL(String(url)).pathname.startsWith(prefix)) return;
          (window as any).__farhelmTermSocketsOpened =
            ((window as any).__farhelmTermSocketsOpened ?? 0) + 1;
          if (spent) return;
          spent = true;
          queueMicrotask(() => {
            const until = performance.now() + blockMs;
            while (performance.now() < until) {
              // A busy wait, deliberately: the point is a main thread that
              // cannot run tasks, which no timer-based pause produces.
            }
          });
        }
      }
      (window as any).WebSocket = Starving;
    },
    { sessionId, blockMs },
  );
}

/**
 * Trigger two reads of the URLs `matches` accepts and resolve once both have
 * landed in the page.
 *
 * Two, always, and it is not padding: the first read is the one that RETIRES
 * an optimistic correction (`ListView`'s and `SessionView`'s `renamed`
 * signals hold the typed title until a reply settles it), so a title still
 * standing after the SECOND is a title the view is rendering from the
 * server's own answer with no correction left underneath it — which is the
 * whole difference between a rename that landed and one that only painted.
 *
 * `trigger` is what asks for each read, and it is a parameter rather than a
 * wait because nothing asks on its own any more: this used to count polls,
 * and a healthy page performs none (PLAN_M6_75.md item 6). Callers pass the
 * feed stub's notification, which is what the helm would have sent.
 *
 * `matches` takes a parsed `URL` rather than a regex over the raw string
 * deliberately. It was a regex anchored at the end (`/\/api\/sessions$/`),
 * which stopped matching the moment the listing started carrying a query —
 * and a predicate that never matches here does not fail loudly, it hangs for
 * thirty seconds and then blames the read. Matching on `pathname` is immune
 * to whatever parameters the request grows next.
 */
async function afterTwoReads(page: Page, matches: (url: URL) => boolean, trigger: () => void) {
  for (let i = 0; i < 2; i++) {
    const landed = page.waitForResponse(
      (response) => response.request().method() === "GET" && matches(new URL(response.url())),
      { timeout: 30_000 },
    );
    trigger();
    await landed;
  }
}

// PLAN_M5.md acceptance 1 for the agent terminal, in the deterministic
// form that plan settles on: reattaching to a session whose scrollback
// runs deeper than one screen lands at the tail with NO intermediate state
// shown.
//
// The assertions come in two halves, and both are needed. DURING the
// catch-up — held open, which is the only way to be inside that window —
// the terminal element is genuinely hidden and the placeholder is genuinely
// on screen: those are computed-style facts, not the island's opinion of
// itself, so deleting the line that hides the terminal fails this test.
// AFTER the release, the marker is what ended the catch-up, the whole
// replay is there, and the view follows the tail.
//
// This test fails against M4's behavior — where every replay chunk was
// written into a visible terminal — which is what makes it the milestone's
// executable acceptance rather than a description of the code.
test("reattach-lands-at-tail (agent terminal): hidden through catch-up, viewport at the tail", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-agent-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);
    await waitForTermText(page, "FAKE-AGENT READY");

    // Scrollback deeper than one screen: the fake agent's `spam` exists
    // for exactly this (see its own docs), and 200 lines is far past any
    // viewport this suite runs at.
    await page.locator("#terminal").click();
    await page.keyboard.type("spam 200");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-200");

    // A reload is the harshest detach/reattach there is: a brand-new
    // grid with an empty model, so everything below came from this
    // attach's own replay. The hold is armed first, so the reattach's
    // catch-up stays open instead of completing before Node can look.
    await holdCatchUpFromNextLoad(page);
    await page.reload();
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");

    // Mid-catch-up, asserted against the DOM rather than against the
    // island's own bookkeeping.
    await expect(page.locator("#terminal")).toBeHidden();
    await expect(page.locator("#term-connecting")).toBeVisible();
    // A grid applies the replay's frames as they arrive, to a terminal that
    // stays hidden until the marker, so the hiding is what holds the replay
    // back from view.

    await releaseCatchUp(page, "terminal");
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason, "the marker is what ended the catch-up").toBe("marker");
    expect(replay.viewportAtTailOnReveal, "the first visible frame is at the tail").toBe(true);
    expect(replay.buffering).toBe(false);

    // The replay is not merely batched but complete, and the terminal is
    // genuinely on screen afterwards.
    expect(await termText(page)).toContain("spam-line-200");
    await expect(page.locator("#terminal")).toBeVisible();
    await expect(page.locator("#term-connecting")).toBeHidden();
    // Live, and typeable without a click: focus is placed at the reveal,
    // since an element that is `visibility: hidden` cannot take it (the
    // regression this covers made every freshly opened session need a
    // click before it would accept a keystroke).
    await page.keyboard.type("after-replay");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:after-replay");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

/**
 * A reveal lands when the replay finishes, a moment the user did not choose.
 * Focus they moved after selecting the session (a header button tabbed to
 * while the terminal caught up) is their choice and must survive the reveal,
 * or an Enter meant for that button would reach the agent. Select-then-type,
 * with focus still where the selection left it, is the test above.
 */
test("a reveal leaves focus on a header button moved to during catch-up", async ({ page, request }) => {
  test.setTimeout(120_000);
  let id: string | undefined;
  try {
    id = (await createTabSession(request, `replay-focus-${Date.now()}`)).id;
    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");
    const button = page.locator(".header-replace");
    await button.focus();
    await expect(button, "premise: focus moved to the header during catch-up").toBeFocused();
    await releaseCatchUp(page, "terminal");
    await waitForReplayReveal(page, "terminal");
    await expect(page.locator("#terminal")).toBeVisible();
    await expect(button, "the reveal must not take focus the user moved").toBeFocused();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The same acceptance for a TAB, which is a separate claim rather than a
// duplicate: every terminal has its own socket and its own marker
// (PLAN_M5.md item 5), so the buffering is per island — a version that
// only ever hid the agent terminal would pass the test above and fail this
// one.
//
// The tab is SELECTED before the hidden-state assertions, deliberately: an
// unselected pane is hidden as a whole (app.css's `.terminal-pane`), so a
// hidden terminal inside one proves nothing at all. Selected, the pane is
// on screen and the only thing hiding the terminal is its own catch-up.
test("reattach-lands-at-tail (tab): hidden through catch-up, viewport at the tail", async ({
  page,
  request,
}) => {
  test.setTimeout(150_000);
  const title = `replay-tab-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);

    const tabId = await addTab(page, 0);
    const element = `terminal-${tabId}`;
    await waitForSessionMounted(page, id, { tabId });
    // Deeper than one screen, printed by the tab's own shell. The marker
    // only ever appears in the OUTPUT, never in the typed line, for the
    // reason `runInShell` documents.
    await runInShell(
      page,
      element,
      "sh -c 'for i in $(seq 1 200); do echo TABSPAM-$i; done'",
      "TABSPAM-200",
    );

    await holdCatchUpFromNextLoad(page);
    await page.reload();
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForSessionMounted(page, id, { tabId });
    await waitForHeldCatchUp(page, element);

    // Selection is not persisted, so the reattached view lands on the
    // agent terminal; this brings the tab's pane on screen so that what is
    // hidden below is the TERMINAL, not the pane around it.
    await selectTerminal(page, tabId);
    await expect(page.locator(`[id="${element}"]`)).toBeHidden();
    await expect(page.locator(`[id="term-connecting-${tabId}"]`)).toBeVisible();
    // A grid applies the replay to the hidden terminal as it arrives (see
    // the agent terminal's test above).

    await releaseCatchUp(page, element);
    const replay = await waitForReplayReveal(page, element);
    expect(replay.revealReason).toBe("marker");
    expect(replay.viewportAtTailOnReveal).toBe(true);

    await waitForIslandText(page, element, "TABSPAM-200", 30_000);
    await expect(page.locator(`[id="${element}"]`)).toBeVisible();
    await expect(page.locator(`[id="term-connecting-${tabId}"]`)).toBeHidden();

    // Typed with NO click first, which is the tab's half of the focus
    // contract: focus is placed when the terminal is revealed, reading the
    // selection AS IT IS THEN. An implementation that placed focus at
    // mount time would have given it to the agent terminal — the
    // reattached view's own initial selection — and these keystrokes would
    // have gone there instead, leaving this wait to time out.
    const after = shellMarker("AFTER-REPLAY");
    await page.keyboard.type(after.command);
    await page.keyboard.press("Enter");
    await waitForIslandText(page, element, after.expected, 30_000);
    expect(
      await termText(page),
      "the keystrokes went to the selected tab, not to the agent terminal",
    ).not.toContain(after.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md has listed rename in the v1 client surface since the beginning
// (PLAN_M5.md item 6). From the list, the row's own field renames the
// session and the row takes the new title AT ONCE — and, two re-reads
// later, still has it, which is what separates a rename that reached the
// supervisor from one this view merely painted over its own listing.
//
// The re-reads are driven from here through a stubbed feed rather than
// waited for: they used to be the listing poll, which M6.75 removed, and
// making them this test's to trigger also removes the shared stack's
// revision churn from the picture.
test("rename-from-list: the row takes the new title and keeps it across re-reads", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `rename-list-${Date.now()}`;
  const renamed = `${title}-renamed`;
  let id: string | undefined;
  const feed = await stubFeed(page);
  let revision = 1;
  try {
    const created = await request.post("/api/sessions", {
      data: { cwd: "/tmp", command: { command: FAKE_AGENT_INVOCATION, yolo: false }, title },
    });
    expect(created.status(), await created.text()).toBe(200);
    id = (await created.json()).id as string;

    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(revision);
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible({ timeout: 15_000 });
    await openRowMenu(row);
    const toggle = row.locator(".session-row-menu");
    await row.locator(".session-row-rename").click();
    // The field opens seeded with the current title, which is what makes
    // renaming an edit rather than a retype.
    const dialog = page.locator(".rename-dialog");
    await expect(dialog.locator(".rename-input")).toHaveValue(title);
    await dialog.locator(".rename-input").fill(renamed);
    await dialog.locator(".rename-submit").click();

    // The field closes and the row reads the new name without waiting for
    // any re-read — the optimistic half of the contract.
    await expect(dialog).toHaveCount(0);
    await expect(toggle).toBeFocused();
    await expect(row.locator(".session-title")).toHaveText(renamed);

    // The server's own answer, not this view's: the supervisor serves its
    // listing from the in-memory session map, so a store-only rename would
    // fail exactly here.
    const listed = await (await request.get(`/api/sessions/${encodeURIComponent(id)}`)).json();
    expect(listed.title).toBe(renamed);

    // And the re-read listing agrees — see `afterTwoReads` for why two.
    await afterTwoReads(page, SESSION_LISTING, () => feed.notify(++revision));
    await expect(row.locator(".session-title")).toHaveText(renamed);
    await expect(row.locator(".action-error")).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

/**
 * Click submission keeps its action focused while the response is pending.
 *
 * Native disabling used to blur Save onto the document body, erase the
 * dialog's ownership record, and make a successful completion skip its
 * return to the opener. Holding the route makes that browser event sequence
 * observable; moving focus to the still-focusable read-only field also proves
 * the modal remains keyboard-reachable for copying during the request.
 */
test("held rename success keeps dialog focus and returns it after unmount", async ({ page, request }) => {
  const title = `rename-held-success-${Date.now()}`;
  const renamed = `${title}-renamed`;
  const session = await createTabSession(request, title);
  let releaseRename!: () => void;
  const release = new Promise<void>((resolve) => { releaseRename = resolve; });
  let markEntered!: () => void;
  const entered = new Promise<void>((resolve) => { markEntered = resolve; });
  await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/rename`, async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    markEntered();
    await release;
    await route.continue();
  });
  try {
    await page.goto("/");
    const source = page.locator(`[data-session-id="${session.id}"]`);
    await expect(source).toBeVisible({ timeout: 20_000 });
    await openRowMenu(source);
    const toggle = source.locator(".session-row-menu");
    await source.locator(".session-row-rename").click();
    const dialog = page.locator(".rename-dialog");
    const field = dialog.locator(".rename-input");
    const save = dialog.locator(".rename-submit");
    await field.fill(renamed);
    await save.click();
    await entered;

    await expect(save, "click submission must keep its focus owner").toBeFocused();
    // ARIA disables the action semantically; native focusability must remain.
    await expect(save).toHaveJSProperty("disabled", false);
    await expect(save).toHaveAttribute("aria-disabled", "true");
    await expect(field).toBeEnabled();
    await expect(field).not.toBeEditable();
    await expect(dialog.locator(".rename-cancel")).toBeDisabled();
    await field.focus();
    await expect(field, "the submitted title remains focusable and copyable").toBeFocused();

    releaseRename();
    await expect(dialog).toHaveCount(0);
    await expect(toggle).toBeFocused();
    await expect(source.locator(".session-title")).toHaveText(renamed);
    await expect(source.locator(".session-row-menu-panel")).toHaveCount(0);
  } finally {
    releaseRename();
    await page.unroute(`**/api/sessions/${encodeURIComponent(session.id)}/rename`);
    await cleanupSession(request, session.id);
  }
});

/**
 * A focus destination chosen after submission is newer than focus return.
 *
 * The held success still closes its own dialog, but it may not reclaim focus
 * from an independently usable approval answer reached while the request was
 * outstanding. Approval cards remain live over modal isolation, so this is a
 * newer focus owner the dialog must preserve through successful completion.
 */
test("held rename success preserves an unrelated focus destination", async ({ page, request }) => {
  const title = `rename-held-external-focus-${Date.now()}`;
  const renamed = `${title}-renamed`;
  const session = await createTabSession(request, title);
  let releaseRename!: () => void;
  const release = new Promise<void>((resolve) => { releaseRename = resolve; });
  let markEntered!: () => void;
  const entered = new Promise<void>((resolve) => { markEntered = resolve; });
  let waiting = false;
  const approvalId = `rename-newer-focus-${Date.now()}`;
  const answers: string[] = [];
  let bodyFailed = true;
  try {
    await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/rename`, async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      markEntered();
      await release;
      await route.continue();
    });
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    // This normal approval reader stays empty until the submitted Rename is
    // held. The card then arrives as an independent owner, rather than being
    // present before Rename's initial focus admission.
    await page.route((url) => url.pathname === "/api/approvals", (route) => fulfillAsHelm(route, {
      json: { approvals: waiting ? [{
        id: approvalId, host_id: 1, host_name: "fixture-host",
        session: { id: session.id, title, host_name: "fixture-host" },
        expires_at_ms: Date.now() + 540_000,
        action: {
          kind: "launch", verb: "create", host_name: "fixture-host", cwd: "/work/project",
          title: null, source: null,
          launch: { kind: "agent", selection: { harness: "claude", model: "opus", effort: "high", permissions: "guarded" },
            start: ["claude", "--model", "opus", "--effort", "high", "--permission-mode", "auto", "{farhelm_args}"], resume: null },
        },
      }] : [] },
    }));
    await page.route((url) => url.pathname === `/api/approvals/${approvalId}`, async (route) => {
      expect(route.request().method()).toBe("POST");
      answers.push(route.request().postDataJSON().answer);
      waiting = false;
      await fulfillAsHelm(route, { status: 204 });
    });
    await page.goto("/");
    const source = page.locator(`[data-session-id="${session.id}"]`);
    await expect(source).toBeVisible({ timeout: 20_000 });
    await openRowMenu(source);
    await source.locator(".session-row-rename").click();
    const dialog = page.locator(".rename-dialog");
    const field = dialog.locator(".rename-input");
    await field.fill(renamed);
    await expect(field).toBeFocused();
    await field.press("Enter");
    await entered;
    await expect(field).toBeEnabled();
    await expect(field).not.toBeEditable();
    await expect(field).toBeFocused();

    waiting = true;
    await expect.poll(() => feed.openSockets()).toBe(1);
    feed.notify(2);
    const card = page.locator(`.approval-card[data-approval-id="${approvalId}"]`);
    const destination = card.getByRole("button", { name: "Allow", exact: true });
    await expect(card).toBeVisible();
    await expect(destination).toBeVisible();
    await expect(destination).toBeEnabled();
    expect(await destination.evaluate((node) => node.closest("[data-modal-exempt]") !== null)).toBe(true);
    expect(await destination.evaluate((node) => node.closest("[inert]") !== null)).toBe(false);
    await expect(field).not.toBeEditable();
    await expect(dialog.locator(".rename-cancel")).toBeDisabled();
    await destination.focus();
    await expect(destination).toBeFocused();
    releaseRename();

    await expect(dialog).toHaveCount(0);
    await expect(destination, "completion must preserve the newer focus destination").toBeFocused();
    await expect(source.locator(".session-title")).toHaveText(renamed);
    await expect(source.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(destination).toBeFocused();
    await page.keyboard.press("Enter");
    await expect.poll(() => answers).toEqual(["allow"]);
    await expect(card).toHaveCount(0);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["held rename", () => releaseRename()],
      ["rename and approval routes", () => page.unrouteAll({ behavior: "wait" })],
      ["source session", () => cleanupSession(request, session.id)],
    ]);
  }
});

/**
 * A held refusal restores the same correction surface and focus owner.
 *
 * This covers the failure half of busy-state focus: the submitted field stays
 * mounted and read-only while pending, then becomes editable beside the local
 * error. Correcting and retrying uses that same dialog and closes it without
 * resurrecting the row menu.
 */
test("held rename refusal keeps local error, draft, focus, and retry", async ({ page, request }) => {
  const title = `rename-held-refusal-${Date.now()}`;
  const refused = `${title}-refused`;
  const corrected = `${title}-corrected`;
  const sentinel = "held rename refusal sentinel";
  const session = await createTabSession(request, title);
  let releaseRename!: () => void;
  const release = new Promise<void>((resolve) => { releaseRename = resolve; });
  let markEntered!: () => void;
  const entered = new Promise<void>((resolve) => { markEntered = resolve; });
  const routePattern = `**/api/sessions/${encodeURIComponent(session.id)}/rename`;
  await page.route(routePattern, async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    markEntered();
    await release;
    await fulfillAsHelm(route, {
      status: 400,
      contentType: "text/plain",
      body: sentinel,
    });
  });
  try {
    await page.goto("/");
    const source = page.locator(`[data-session-id="${session.id}"]`);
    await expect(source).toBeVisible({ timeout: 20_000 });
    await openRowMenu(source);
    const toggle = source.locator(".session-row-menu");
    await source.locator(".session-row-rename").click();
    const dialog = page.locator(".rename-dialog");
    const field = dialog.locator(".rename-input");
    await field.fill(refused);
    await expect(field).toBeFocused();
    await field.press("Enter");
    await entered;
    await expect(field).toBeEnabled();
    await expect(field).not.toBeEditable();
    await expect(field).toBeFocused();

    releaseRename();
    await expect(dialog.locator(".rename-error")).toContainText(sentinel);
    await expect(field).toHaveValue(refused);
    await expect(field).toBeEditable();
    await expect(field).toBeFocused();

    await field.fill(corrected);
    await page.unroute(routePattern);
    await dialog.locator(".rename-submit").click();
    await expect(dialog).toHaveCount(0);
    await expect(toggle).toBeFocused();
    await expect(source.locator(".session-title")).toHaveText(corrected);
    await expect(source.locator(".session-row-menu-panel")).toHaveCount(0);
  } finally {
    releaseRename();
    await page.unroute(routePattern);
    await cleanupSession(request, session.id);
  }
});


// SPEC.md names control characters as THE refusal for a supplied title,
// and PLAN_M5.md item 6 makes the supervisor's own message the contract:
// the client sends what was typed verbatim and shows what comes back,
// rather than duplicating the rule client-side and inventing its own
// wording for it.
//
// What must survive the refusal is everything else, and "everywhere" is
// meant literally: the row still SHOWS the old title while the rejected
// draft sits in the still-open field, the server still holds it, and the
// list is unchanged once the field is closed. A version that replaced the
// title with the field would leave the only visible name being the one the
// supervisor just refused.
test("rename-refused: a control-character title shows the supervisor's words and changes nothing", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `rename-refused-${Date.now()}`;
  // A C0 control character embedded in an otherwise ordinary title: this
  // is the terminal-injection case the supervisor refuses, since a title
  // is echoed into terminals by `tracing` consumers.
  const refused = "bad\u0001title";
  let id: string | undefined;
  try {
    const created = await request.post("/api/sessions", {
      data: { cwd: "/tmp", command: { command: FAKE_AGENT_INVOCATION, yolo: false }, title },
    });
    expect(created.status(), await created.text()).toBe(200);
    id = (await created.json()).id as string;

    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible({ timeout: 15_000 });
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    const dialog = page.locator(".rename-dialog");
    await dialog.locator(".rename-input").fill(refused);
    await dialog.locator(".rename-submit").click();

    // The supervisor's own sentence, relayed verbatim through the helm.
    await expect(row.locator(".action-error")).toContainText("control characters", {
      timeout: 15_000,
    });
    // The old title is still on screen next to the field that was
    // refused, not replaced by the draft.
    await expect(dialog.locator(".rename-current-title")).toHaveText(title);
    // The field stays open with the rejected text still in it, so fixing
    // the title is one keystroke rather than a retype.
    await expect(dialog.locator(".rename-input")).toHaveValue(refused);
    // Nothing about the session moved: not on the server...
    const fetched = await (await request.get(`/api/sessions/${encodeURIComponent(id)}`)).json();
    expect(fetched.title).toBe(title);
    // ...and not in the list, which still shows the old name once the
    // field is closed.
    await dialog.locator(".rename-cancel").click();
    await expect(row.locator(".session-title")).toHaveText(title);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The verbatim contract's sharpest case, and the reason the field is a
// `<textarea>`: `<input type=text>` STRIPS line breaks out of a pasted
// value, so a multi-line title would have been silently repaired into
// something acceptable before it was ever sent — the client quietly
// altering the user's data AND stepping in front of the supervisor's own
// refusal. Both halves are pinned here: the field really holds the
// newline, and the supervisor really refuses it.
//
// Inserted with `insertText` rather than typed: it is the paste path (an
// input insertion no keydown handler sees), which matters because Enter in
// this field submits instead of inserting a line break — typing cannot
// produce this title at all.
//
// The session view is the surface under test so that its own half of "the
// old title stays" is covered too: the header keeps showing the real name
// while the refusal is on screen.
test("rename-refused-pasted-newline: a multi-line title reaches the supervisor intact and is refused", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `rename-newline-${Date.now()}`;
  const pasted = `${title}\nsecond-line`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionRevealed(page, id);
    await waitForTermText(page, "FAKE-AGENT READY");

    await openRowMenu(page.locator(`[data-session-id="${id}"]`));
    await page.locator(".session-row-rename").click();
    const field = page.locator(".rename-dialog .rename-input");
    await field.click();
    await page.keyboard.press("Control+A");
    await page.keyboard.insertText(pasted);
    // The field kept the line break — the property an `<input>` would have
    // destroyed before anything could be sent.
    await expect(field).toHaveValue(pasted);

    await page.locator(".rename-submit").click();

    // The supervisor's own refusal, for the newline it actually received —
    // rendered in the row's action-error line, which the open panel hosts.
    await expect(
      page.locator(`[data-session-id="${id}"] .action-error`),
    ).toContainText("control characters", { timeout: 15_000 });
    // The header still shows the real title while the refusal stands.
    await expect(page.locator(".session-details .title")).toHaveText(title);
    await expect(field).toHaveValue(pasted);
    const fetched = await (await request.get(`/api/sessions/${encodeURIComponent(id)}`)).json();
    expect(fetched.title).toBe(title);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A socket that never finishes connecting is the one state where revealing
// the terminal would be a lie: terminal.js drops keystrokes on a socket
// that is not OPEN, so a normal-looking pane would swallow everything
// typed into it with nothing on screen to say why — the silent failure
// SPEC.md requires be reported instead of left to be inferred.
//
// Since the 2026-09-17 decision, the watchdog's never-connected expiry on a
// FIRST mount is a failed attempt like any other: where auto-reconnect can
// run, the ladder surface takes over and a later attempt reattaches —
// banner-and-stop had no answer for a helm that is up but stalls one
// handshake (the island-cap burst). Two contracts, two tests:
//
// - With recovery available: the reconnect surface appears, and the
//   attempt counter ADVANCES — the retry itself is the behavior being
//   pinned. The caret is deliberately left where the user put it; nothing
//   of a terminal that cannot carry input is ever focused.
// - With recovery switched off: the M5 banner remains the remedy of last
//   resort, and the ending is still its own outcome ("unconnected"), not
//   the idle degradation.

test("replay-unconnected: a stalled first mount retries on the reconnect ladder", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-unconnected-retry-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await stuckWebSocketFromNextLoad(page);
    await holdCatchUpFromNextLoad(page, { idleMs: 1_500 });
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);

    // The watchdog expires 1.5s in, the first rung waits 500ms, and each
    // retry remounts onto the same stuck socket — so the second expiry,
    // and with it attempt 2's surface, lands a few seconds later.
    await expect(page.locator("#term-connecting .terminal-reconnect-status")).toBeVisible({
      timeout: 20_000,
    });
    // The never-connected banner must NOT paint when the ladder took over —
    // the two surfaces contradict each other.
    await expect(page.locator("#term-banner")).toBeHidden();
    await expect
      .poll(
        () =>
          page
            .locator("#term-connecting")
            .getAttribute("data-reconnect-attempt")
            .then((v) => Number(v)),
        { timeout: 15_000 },
      )
      .toBeGreaterThanOrEqual(2);
    expect(
      await page.evaluate(() => {
        const active = document.activeElement;
        const terminal = document.getElementById("terminal");
        return !!active && !!terminal && terminal.contains(active);
      }),
      "focus must not be placed into a terminal that cannot carry input",
    ).toBe(false);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

test("replay-unconnected: without recovery the banner reports it instead of revealing a dead terminal", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-unconnected-banner-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await disableReconnectFromNextLoad(page);
    await stuckWebSocketFromNextLoad(page);
    await holdCatchUpFromNextLoad(page, { idleMs: 1_500 });
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);

    await expect(page.locator("#term-banner")).toContainText("Not connected", {
      timeout: 20_000,
    });
    const replay = await replayRecord(page, "terminal");
    expect(
      replay.revealReason,
      "the never-connected ending is its own outcome, not the idle degradation",
    ).toBe("unconnected");
    // The placeholder is gone — the catch-up really did end — but nothing
    // pretends the terminal is usable: the banner is the message, and the
    // caret was not moved into a pane that cannot send.
    await expect(page.locator("#term-connecting")).toBeHidden();
    expect(
      await page.evaluate(() => {
        const active = document.activeElement;
        const terminal = document.getElementById("terminal");
        return !!active && !!terminal && terminal.contains(active);
      }),
      "focus must not be placed into a terminal that cannot carry input",
    ).toBe(false);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The watchdog measures silence, and a page whose main thread is held
// busy cannot hear any. When the hold outlasts the window, the expiry is
// overdue by the time the page runs again, and it can run before the
// socket's queued `open` and first frames. A watchdog that believed that
// expiry would throw away a socket the helm had already accepted, calling
// it never-connected, or would reveal an empty catch-up as idle. The
// island-cap test hit this in WebKit when its first mount built 33
// terminals in one task (FLAKES.md, 2026-09-29).
//
// The contract pinned here: a window the main thread stalled through
// re-arms instead of ending the phase, and the attachment it would have
// discarded goes on to reveal normally on its marker, on the one socket it
// started with. Without the re-arm, both engines failed this test in the
// run that introduced it: WebKit replaced the socket through the reconnect
// ladder, and Chromium revealed on the expiry (`idle`) ahead of the marker.
//
// Which task an engine runs first once the block ends is its own choice,
// and in about one WebKit repetition in five the socket's events won, so
// the re-arm never ran. The outcome is correct either way, so the test
// asserts the outcome and records whether the re-arm ran as an annotation
// rather than failing when it did not.
test("replay-starved: a watchdog expiry that fires late does not end a live catch-up", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-starved-${Date.now()}`;
  // A one-second window starved for four, well past the one-second stall
  // bound in terminal.js.
  const idleMs = 1_000;
  const blockMs = 4_000;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await starveAfterFirstTerminalSocketFromNextLoad(page, id, blockMs);
    await page.addInitScript((idleMs) => {
      (window as any).__farhelmTestReplay = { idleMs };
    }, idleMs);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);

    const replay = await waitForReplayReveal(page, "terminal", 30_000);
    expect(
      await page.evaluate(() => (window as any).__farhelmTermSocketsOpened),
      "the first socket carried the attachment; no reconnect replaced it",
    ).toBe(1);
    expect(
      replay.revealReason,
      "the socket the helm accepted during the stall reveals on its marker",
    ).toBe("marker");
    test.info().annotations.push({
      type: "stalled-idle-rearms",
      description: String(replay.stalledIdleRearms),
    });
    await expect(page.locator("#term-banner")).toBeHidden();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The lifecycle guard, from the outside: a catch-up whose mount is gone
// must not be able to act when its deferred ending finally runs. A queued
// reveal can still run after its mount is disposed, and the replacement
// mount is by then using the SAME DOM nodes — so a stale reveal would
// un-hide a terminal that is mid-catch-up on someone else's replay.
//
// Staged with the hold seam because the real race is a millisecond wide:
// the old mount's ending is held, the view is navigated away and back
// (which unmounts and remounts), and only then is the OLD mount's ending
// released.
test("replay-stale-mount: a torn-down island's deferred ending cannot touch its replacement", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `replay-stale-${Date.now()}`;
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await row.click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");
    // Captured before the teardown: after it, the registry points at the
    // replacement, and this reference is the only way back to the mount
    // under test.
    await page.evaluate(() => {
      (window as any).__staleIsland = (window as any).__farhelmIslands["terminal"].test;
    });

    // Bounce to the shared session and back: a full unmount, then a fresh mount
    // into the same element, whose own catch-up is held too.
    const sharedRow = sharedSessionRow(page);
    await expect(sharedRow).toHaveAttribute("data-session-id", /.+/, { timeout: 20_000 });
    const sharedId = (await sharedRow.getAttribute("data-session-id"))!;
    await sharedRow.click();
    await expect(page.locator(".session-details .title")).toHaveText("e2e-session");
    await waitForSessionMounted(page, sharedId);
    await row.click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");

    // A list selection can commit before terminal reconciliation. Prove this
    // is a replacement before releasing a callback saved from the old mount.
    await expect.poll(() => page.evaluate(() =>
      (window as any).__farhelmIslands?.["terminal"]?.test !== (window as any).__staleIsland
    ), { timeout: 20_000 }).toBe(true);

    await page.evaluate(() => (window as any).__staleIsland.releaseCatchUp());

    const stale = await page.evaluate(() => (window as any).__staleIsland.replay);
    expect(
      stale.revealReason,
      "the torn-down mount's ending is inert, not merely harmless-looking",
    ).toBeNull();
    const replacement = await replayRecord(page, "terminal");
    expect(replacement.revealed, "the replacement is still catching up").toBe(false);
    await expect(page.locator("#terminal")).toBeHidden();
    await expect(page.locator("#term-connecting")).toBeVisible();
    expect(pageErrors, "a stale callback must not throw into the page").toEqual([]);

    // And the replacement still completes normally on its own release —
    // the guard stops the stale mount, not this one.
    await releaseCatchUp(page, "terminal");
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason).toBe("marker");
    await expect(page.locator("#terminal")).toBeVisible();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A rename field is an edit in progress, and the list re-renders for
// reasons the user did not cause: one failed listing read swaps the rows
// for an error line. The ListView-owned dialog must retain the same textarea
// DOM node through that replacement, or the browser loses the caret and IME
// composition even if a copied draft happens to look right afterwards.
//
// The failure is injected the same way the confirming-state tests inject
// theirs — a 500 latched on while the field is open, and a feed
// notification to ask for the read that hits it — so what is exercised is
// the real reader's real failure path.
test("rename-draft-survives-a-failed-read: the stable field keeps focus and its DOM identity", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `rename-read-fail-${Date.now()}`;
  const draft = `half-typed-${Date.now()}`;
  let id: string | undefined;

  let failing = false;
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    if (failing) {
      await fulfillAsHelm(route, {
        status: 500,
        contentType: "text/plain",
        body: "injected read failure",
      });
      return;
    }
    await route.continue();
  });

  const feed = await stubFeed(page);
  try {
    const created = await request.post("/api/sessions", {
      data: { cwd: "/tmp", command: { command: FAKE_AGENT_INVOCATION, yolo: false }, title },
    });
    expect(created.status(), await created.text()).toBe(200);
    id = (await created.json()).id as string;

    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible({ timeout: 15_000 });
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    const dialog = page.locator(".rename-dialog");
    await dialog.locator(".rename-input").fill(draft);
    await expect(dialog.locator(".rename-input")).toBeFocused();
    await dialog.locator(".rename-input").evaluate((field) => {
      (window as any).__renameTextarea = field;
    });

    // Only now does a listing read fail, strictly after the field holds
    // the draft.
    failing = true;
    feed.notify(2);
    // The rows really do go away — this is the transient state the draft
    // has to survive, not a bug the test is stepping around.
    await expect(page.locator(".status.error")).toBeVisible({ timeout: 10_000 });
    await expect(row).toHaveCount(0);

    // The dialog remains mounted while the rows are absent. A copied string
    // in a remounted field is insufficient: identity and focus prove the
    // selection/composition-bearing native control did not move.
    await expect(dialog.locator(".rename-input")).toBeFocused();
    expect(await dialog.locator(".rename-input").evaluate((field) =>
      field === (window as any).__renameTextarea,
    )).toBe(true);

    // The next read succeeds and the row comes back with the same editor
    // still open. The reader retries a failed read on its own; the
    // notification is belt and braces.
    failing = false;
    feed.notify(3);
    await expect(row).toHaveCount(1, { timeout: 10_000 });
    await expect(dialog.locator(".rename-input")).toHaveValue(draft);

    // Continue typing after the list changed, then prove the stable editor
    // still submits to the original id rather than the row now at its old
    // position.
    await dialog.locator(".rename-input").press("End");
    await page.keyboard.type("-tail");
    await dialog.locator(".rename-submit").click();
    await expect(row.locator(".session-title")).toHaveText(`${draft}-tail`);
  } finally {
    await page.unroute(SESSION_LISTING);
    if (id) await cleanupSession(request, id);
  }
});

/**
 * A reordered listing used to close the anchored menu that contained the
 * editor. This test changes the response order while the native textarea is
 * focused with an interior selection, then proves that the same node and
 * selection receive more typing and save the source id rather than whichever
 * row now occupies its former position. It also pins the IME Escape boundary:
 * composing Escape belongs to the candidate UI, while ordinary Escape still
 * cancels the dialog.
 */
test("rename survives a real committed row reorder without moving its textarea", async ({
  page,
  request,
}) => {
  const first = await createSession(request, {
    cwd: "/tmp",
    invocation: FAKE_AGENT_INVOCATION,
    title: `rename-reorder-first-${Date.now()}`,
  });
  const second = await createSession(request, {
    cwd: "/tmp",
    invocation: FAKE_AGENT_INVOCATION,
    title: `rename-reorder-second-${Date.now()}`,
  });
  const draft = `${first.title}-draft`;
  const selectionStart = Math.floor(draft.length / 2);
  const selectionEnd = selectionStart + 3;
  const inserted = "-tail-";
  const expected = `${draft.slice(0, selectionStart)}${inserted}${draft.slice(selectionEnd)}`;
  let reverse = false;
  const feed = await stubFeed(page);
  try {
    await page.route(SESSION_LISTING, async (route) => {
      if (route.request().method() !== "GET" || !reverse) return route.continue();
      const response = await route.fetch();
      const body = await response.json();
      const sourceIndex = body.sessions.findIndex((session: { id: string }) => session.id === first.id);
      const [source] = body.sessions.splice(sourceIndex, 1);
      // Move the source to the opposite edge, which makes the test's order
      // premise true even when unrelated shared-stack rows are present.
      if (sourceIndex === 0) body.sessions.push(source);
      else body.sessions.unshift(source);
      await route.fulfill({ response, json: body });
    });
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const source = page.locator(`[data-session-id="${first.id}"]`);
    await expect(source).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(`[data-session-id="${second.id}"]`)).toBeVisible();
    const sourceIndex = await page.locator(".session-row").evaluateAll((rows, id) =>
      rows.findIndex((row) => row.getAttribute("data-session-id") === id), first.id);
    await openRowMenu(source);
    await source.locator(".session-row-rename").click();
    const field = page.locator(".rename-dialog .rename-input");
    await field.fill(draft);
    await expect(field).toBeFocused();
    await field.evaluate((node, [start, end]: [number, number]) => {
      (window as any).__renameReorderField = node;
      (node as HTMLTextAreaElement).setSelectionRange(start, end);
      node.dispatchEvent(new KeyboardEvent("keydown", {
        key: "Escape",
        bubbles: true,
        isComposing: true,
      }));
    }, [selectionStart, selectionEnd]);
    await expect(page.locator(".rename-dialog")).toBeVisible();
    await expect(field).toHaveValue(draft);
    expect(await field.evaluate((node) => ({
      start: (node as HTMLTextAreaElement).selectionStart,
      end: (node as HTMLTextAreaElement).selectionEnd,
    }))).toEqual({ start: selectionStart, end: selectionEnd });

    reverse = true;
    feed.notify(2);
    // The pointer entered the list to open the menu, so the list's order is
    // held (SPEC.md, Session list) until the pointer leaves or rests for five
    // seconds. The dialog replaced the element under the pointer, which not
    // every engine reports as leaving the list, so the reorder may only land
    // with the stillness release; the timeout allows for it. Where the pointer
    // rests is not what this test is about; the dialog's stability is.
    await expect.poll(() => page.locator(".session-row").evaluateAll((rows, id) =>
      rows.findIndex((row) => row.getAttribute("data-session-id") === id), first.id),
    { timeout: 15_000 }).not.toBe(sourceIndex);
    expect(await field.evaluate((node) => node === (window as any).__renameReorderField)).toBe(true);
    await expect(field).toBeFocused();
    expect(await field.evaluate((node) => ({
      start: (node as HTMLTextAreaElement).selectionStart,
      end: (node as HTMLTextAreaElement).selectionEnd,
    }))).toEqual({ start: selectionStart, end: selectionEnd });
    await page.keyboard.insertText(inserted);
    await expect(field).toHaveValue(expected);
    await field.press("Enter");
    await expect(source.locator(".session-title")).toHaveText(expected);
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    await expect(source.locator(".session-row-menu-panel")).toHaveCount(0);
    expect((await (await request.get(`/api/sessions/${encodeURIComponent(first.id)}`)).json()).title).toBe(expected);

    await openRowMenu(source);
    await source.locator(".session-row-rename").click();
    const reopened = page.locator(".rename-dialog");
    await expect(reopened.locator(".rename-input")).toBeFocused();
    await reopened.locator(".rename-input").press("Escape");
    await expect(reopened).toHaveCount(0);
    await expect(source.locator(".session-row-menu-panel")).toHaveCount(0);
  } finally {
    await page.unroute(SESSION_LISTING);
    await cleanupSession(request, first.id);
    await cleanupSession(request, second.id);
  }
});


// The reveal happens when the replay lands, which is a moment the user did
// not choose — and by then they may be typing somewhere else. The rename
// field is the obvious victim: open it while a terminal is still catching
// up, start typing a title, and a reveal that took focus unconditionally
// would pull the caret into the pty mid-word and send the rest of the name
// to the agent.
//
// Held on purpose, because that is what makes the window wide enough to
// stage. Everything asserted here is about where focus and keystrokes
// ended up, not about the island's opinion of itself.
test("reveal-does-not-steal-focus: a rename field being typed into keeps focus and its text", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `focus-steal-${Date.now()}`;
  const typed = `renaming-while-connecting-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await holdCatchUpFromNextLoad(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await waitForSessionMounted(page, id);
    await waitForHeldCatchUp(page, "terminal");

    // The user turns to the row menu's rename field (the one rename
    // surface) while the terminal is still catching up, and types a new
    // title — the reveal must not steal the caret out from under them.
    await openRowMenu(page.locator(`[data-session-id="${id}"]`));
    await page.locator(".session-row-rename").click();
    const field = page.locator(".rename-dialog .rename-input");
    await expect(field).toBeFocused();
    await field.fill("");
    await page.keyboard.type(typed);

    await releaseCatchUp(page, "terminal");
    const replay = await waitForReplayReveal(page, "terminal");
    expect(replay.revealReason).toBe("marker");
    await expect(page.locator("#terminal")).toBeVisible();

    // Focus stayed where the user put it, with what they typed intact.
    await expect(field).toBeFocused();
    await expect(field).toHaveValue(typed);
    expect(
      await termText(page),
      "not one keystroke of the title may have reached the agent",
    ).not.toContain(typed);

    // And the keyboard still belongs to the field: more typing lands
    // there, not in the terminal that just appeared.
    await page.keyboard.type("-tail");
    await expect(field).toHaveValue(`${typed}-tail`);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

/** Observe real agent input independently of the terminal's focus bookkeeping.
 * Binary frames carry terminal input; text frames carry protocol controls.
 * The final live echo in each caller also proves this observer saw the
 * attachment that would have received a stray Enter.
 */
function observeQuestionInput(page: Page, id: string): InputFrame[] {
  const binary: InputFrame[] = [];
  page.on("websocket", (socket) => {
    if (new URL(socket.url()).pathname !== `/api/sessions/${encodeURIComponent(id)}/term`) return;
    recordInput(socket, binary);
  });
  return binary;
}

/** Both ways of declining a question must leave terminal input intentional.
 * Enter on Cancel must cancel without sending a carriage return. Reopening
 * and using Escape then restores the selected terminal, whose processed echo
 * settles the outgoing-frame observation without a time-only quiet window.
 */
async function declineQuestionAndCheckInput(page: Page, action: "delete" | "replace", binary: InputFrame[]) {
  const trigger = page.locator(`.header-${action}`);
  const question = page.locator(`.header-${action}-confirm`);
  await expect(question.locator(".btn-danger")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(question.locator(".btn-neutral")).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(question).toHaveCount(0);

  await trigger.click();
  await expect(question.locator(".btn-danger")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(question).toHaveCount(0);
  await expect(terminalInput(page)).toBeFocused();
  const marker = `question-escape-live-${Date.now()}`;
  await page.keyboard.type(marker);
  await page.keyboard.press("Enter");
  await waitForTermText(page, `echo:${marker}`);
  expect(programInput(binary), "only input after Escape may reach the terminal").toBe(`${marker}\r`);
}

for (const action of ["delete", "replace"] as const) {
  /** A replay completion is automatic, even when a question opened after attach.
   * The held marker makes that ordering deterministic. The answer must retain
   * focus throughout reveal so Enter cannot become input to the live agent.
   */
  test(`replay reveal preserves a header ${action} question's keyboard ownership`, async ({ page, request }) => {
    test.setTimeout(120_000);
    const session = await createTabSession(request, `replay-question-${action}-${Date.now()}`);
    // The session is already owned, even if observer setup rejects.
    try {
      const binary = observeQuestionInput(page, session.id);
      await installTerminalFocusSpy(page);
      await holdCatchUpFromNextLoad(page);
      await page.goto("/");
      await page.locator(`[data-session-id="${session.id}"]`).click();
      await waitForSessionMounted(page, session.id);
      await waitForHeldCatchUp(page, "terminal");
      await expect(page.locator("#terminal")).toBeHidden();
      await page.locator(`.header-${action}`).click();
      const answer = page.locator(`.header-${action}-confirm .btn-danger`);
      await expect(answer).toBeFocused();
      binary.length = 0;
      await armTerminalFocusSpy(page);

      await releaseCatchUp(page, "terminal");
      const replay = await waitForReplayReveal(page, "terminal");
      expect(replay.revealReason).toBe("marker");
      await expect(page.locator("#terminal")).toBeVisible();
      await expect(answer).toBeFocused();
      expect(await terminalFocusCount(page), "reveal must never focus the terminal behind the question").toBe(0);
      await declineQuestionAndCheckInput(page, action, binary);
    } finally {
      await cleanupSession(request, session.id);
    }
  });
}

/** Losing a selected tab changes the selected pane without an explicit click.
 * An already-revealed agent exercises the direct fallback focus path rather
 * than another replay. The open header question must still own the keyboard.
 */
test("selected tab removal preserves a header delete question's keyboard ownership", async ({ page, request }) => {
  test.setTimeout(120_000);
  const session = await createTabSession(request, `fallback-question-${Date.now()}`);
  // The session is already owned, even if observer setup rejects.
  try {
    const binary = observeQuestionInput(page, session.id);
    await installTerminalFocusSpy(page);
    await page.goto("/");
    await attachSession(page, session.id);
    const tabId = await addTab(page, 0);
    await selectTabOverRevealedAgent(page, session.id, tabId);
    await page.locator(".header-delete").click();
    const answer = page.locator(".header-delete-confirm .btn-danger");
    await expect(answer).toBeFocused();
    binary.length = 0;
    await armTerminalFocusSpy(page);

    await closeTabAndAwaitAgentFallback(page, request, session.id, tabId);
    await expect(answer).toBeFocused();
    expect(await terminalFocusCount(page), "tab fallback must never focus the terminal behind the question").toBe(0);
    await declineQuestionAndCheckInput(page, "delete", binary);
  } finally {
    await cleanupSession(request, session.id);
  }
});
