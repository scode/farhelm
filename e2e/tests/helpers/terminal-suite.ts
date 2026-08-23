// Shared fixture contract for the terminal.spec family: every file resets the
// stack, stamps fabricated helm replies, and shares the tab-session factory.
// Every file in this family MUST call `installTerminalSuiteHooks`; otherwise
// its `fulfillAsHelm` fixtures play an unstamped helm and trigger skew UI.

import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { stackScratchDir } from "./scratch";
import { waitForTermText } from "./term";

// Any status a session's agent can carry while it is UP (PLAN_M6_75.md
// item 3's live split: running / waiting / idle).
//
// Almost every terminal-family assertion that mentions a live badge is a
// LIFECYCLE assertion — "the session survived that", "the restart brought
// it back" — and does not care which of the three the classifier chose.
// Naming one exact word there would tie the whole suite to today's
// classification and turn the status sampler's arrival into a hundred
// unrelated failures; worse, it would fail for a session that is perfectly
// healthy and merely quiet.
//
// Exact words are still asserted, deliberately, where a test already KNOWS
// the word: the badge-render test in terminal.spec.ts (which is ABOUT the
// vocabulary, and waits for the live classification to settle before
// asserting the word the listing itself carries), and those route fixtures
// that compare the status they authored verbatim. Neither PREDICTS one — a
// test that predicts which of the three a real session will be in is the
// flake this constant exists to prevent, and that is exactly how the
// badge-render test used to fail (see `settledSharedSessionRow` in
// terminal.spec.ts).
export const LIVE_BADGE = /^(running|waiting|idle)$/;

// The exit code is optional in the pattern because a stopped session may or
// may not have one — a signal death tmux cannot reduce to a code leaves the
// badge at plain `exited`, and a shell that ran its EXIT trap first reports
// a real code — and no test can predict which it gets. It is not optional in
// ORDER: the code leads the annotation ("exited (code 7) — stopped by
// user"), because the badge is capped at 32ch and the older code-last
// wording let a long annotation ellipsize away the one datum the badge
// exists to report (`status.rs`'s `Exited` arm). Anchored at both ends so a
// regression back to code-last fails here rather than passing on a prefix.
export const STOPPED_BADGE = /^exited( \(code \d+\))? — stopped by user$/;

// The same set as `LIVE_BADGE`, for the API-level assertions that read a
// status out of `/api/sessions` rather than off the DOM.
export const LIVE_STATES = ["running", "waiting", "idle"];

/**
 * The build stamp the live helm is sending, captured once so that every
 * FABRICATED reply in the consuming specs can carry it too.
 *
 * Route-intercepted replies stand in for the helm, and since PLAN_M6.md
 * item 6 the helm stamps every reply it gives — successes, refusals, even
 * the origin guard's 403. A fixture that omitted the stamp would be
 * playing a helm that predates this milestone, which the UI correctly
 * reads as a version mismatch: the skew banner would appear, and on the
 * upload path a second error line would too. Captured rather than
 * hardcoded so a version bump does not silently turn every one of those
 * fixtures into a mismatch.
 */
let HELM_BUILD = "";

/** The build captured by the spec family's reset, for fixtures that need it as data. */
export function helmBuild(): string {
  return HELM_BUILD;
}

/**
 * Fulfil one intercepted request the way the HELM would: with the build
 * stamp on it.
 *
 * The one door every fabricated reply in the terminal-family specs goes
 * through, and that matters beyond tidiness. Since PLAN_M6.md item 6 the UI
 * reads a build stamp off every reply and treats its ABSENCE as a version
 * mismatch — a helm that sends none predates the stamp — so a fixture that
 * forgot it was not standing in for the helm at all: it was quietly playing
 * an incompatible one, which surfaces as a skew banner and, on the upload
 * path, a second error line that breaks single-element assertions about
 * something else entirely. Four fixtures had drifted that way already.
 *
 * `contentType` is spelled as a header because Playwright takes one or the
 * other, not both, and headers are what carry the stamp.
 */
export async function fulfillAsHelm(
  route: { fulfill: (options: Record<string, unknown>) => Promise<void> },
  options: { status?: number; contentType?: string; body?: string; json?: unknown },
) {
  const headers: Record<string, string> = { "x-farhelm-build": HELM_BUILD };
  if (options.contentType) headers["content-type"] = options.contentType;
  const { contentType, ...rest } = options;
  await route.fulfill({ ...rest, headers });
}

/**
 * Restore the stack to one freshly launched shared `e2e-session`.
 *
 * The suite runs against one long-lived stack (see playwright.config.ts's
 * webServer), so each spec file runs this reset in its own `beforeAll`; one
 * file's deliberate scrollback pollution cannot reach another file. A
 * `beforeAll` rather than a setup project or a name-ordered spec file is
 * deliberate: see playwright.config.ts's projects comment for why those
 * alternatives are unsound. Failure stops the calling spec's tests instead
 * of letting them run against half-reset state.
 *
 * The canonical session's cwd and invocation come from the live listing
 * rather than a duplicate of start-stack.sh. Its absence is an invariant
 * violation — either a test deleted it (nothing may) or an earlier reset
 * failed mid-way — and is worth failing loudly on rather than hiding behind
 * a hardcoded recreation.
 */
export async function resetStack(request: APIRequestContext) {
  const probe = await request.get("/api/sessions");
  HELM_BUILD = probe.headers()["x-farhelm-build"] ?? "";
  expect(
    HELM_BUILD,
    "the helm must stamp its replies: every fabricated terminal fixture borrows this value",
  ).toBeTruthy();
  const listing = await probe.json();
  const shared = listing.sessions.find(
    (s: { title: string }) => s.title === "e2e-session",
  );
  expect(
    shared,
    "the shared e2e-session must exist: a test deleted it, or an earlier project's reset died between delete and recreate",
  ).toBeTruthy();

  // Delete everything (leaked per-test sessions included) and relaunch
  // the shared session with its own recorded parameters. Deleting the
  // polluted original rather than restarting its agent is what actually
  // resets the SCROLLBACK: stop/relaunch would keep the tmux history.
  for (const s of listing.sessions) {
    const deleted = await request.delete(`/api/sessions/${s.id}`);
    expect(deleted.ok(), `deleting leftover session ${s.title}`).toBe(true);
  }
  const created = await request.post("/api/sessions", {
    data: {
      cwd: shared.cwd,
      invocation: shared.invocation,
      title: shared.title,
    },
  });
  expect(created.ok(), "recreating the shared e2e-session").toBe(true);
}

/**
 * Register the reset and teardown guards every terminal-family spec needs.
 *
 * A spec that calls `createTabSession` must enable `tabSweep`; that factory
 * records scratch directories whose cleanup belongs to the spec's final
 * hook. Specs that never create tab sessions leave it disabled.
 */
export function installTerminalSuiteHooks(options: { tabSweep?: boolean } = {}) {
  test.beforeAll(async ({ request }) => resetStack(request));
  test.afterEach(async ({ page }) => {
    // A final assertion can coincide with an invalidation-driven refresh. If
    // Playwright owns teardown immediately afterward, it disposes the
    // `route.fetch()` response while the async handler is still reading it and
    // turns a passing test into an unhandled teardown failure. Unregistering
    // first closes the door to new handlers; `wait` lets every handler already
    // inside finish while its page and request context are still alive.
    await page.unrouteAll({ behavior: "wait" });
  });
  if (options.tabSweep) {
    test.afterAll(() => {
      for (const dir of tabSessionDirs) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
      tabSessionDirs.length = 0;
    });
  }
}

/**
 * A real, distinguishable agent invocation for the create-dialog tests
 * (PLAN_M2.md step 8): the same fake-agent binary and `basic` script
 * start-stack.sh uses for the shared "e2e-session", built from an
 * absolute path so it works regardless of the supervisor's own cwd. Every
 * dialog-driven session in the terminal-family specs uses this invocation
 * rather than a bare `sleep` — the multi-session flow types into one of
 * them, which `sleep` cannot answer.
 */
export const FAKE_AGENT_INVOCATION = `"${path.resolve(__dirname, "../../../target/debug/farhelm")}" internal fake-agent --script basic`;

/**
 * The fake agent's flood producer as a COMMAND to type into a shell,
 * ungated, for the per-terminal isolation tests.
 *
 * A tab is a plain shell, so the suite's own flood fixture runs inside one
 * as an ordinary command — no fixture support on either side, and a
 * genuinely per-terminal producer rather than a second session. Ungated
 * because the gate exists to let a test control when a producer starts
 * relative to an ATTACH, and here the attach is long since done: the
 * command starts the flood at the moment it is typed.
 */
export const FLOOD_AGENT_COMMAND = `"${path.resolve(__dirname, "../../../target/debug/farhelm")}" internal fake-agent --script flood`;

/**
 * Locator for a session row by its exact title, matched against the
 * `.session-title` element specifically rather than `hasText` on the
 * whole row: `hasText` matches a row's full text content (title, cwd, AND
 * invocation concatenated), so it would happily also match a row whose
 * cwd or invocation merely CONTAINS the wanted title as a substring.
 * Anchoring the regex against just the title element is what actually
 * pins "the row with THIS title", not "some row that mentions it
 * somewhere".
 */
export function rowByTitle(page: Page, title: string) {
  return page.locator(".session-row").filter({
    has: page.locator(".session-title", { hasText: new RegExp(`^${title}$`) }),
  });
}

/**
 * Look up a session's id from the real API by its title, for
 * best-effort cleanup in a `finally` block when the UI flow under test
 * did not get far enough to hand back an id itself (e.g. a create that
 * is expected to fail, or a row that may already be gone by the time
 * cleanup runs). Swallows a missing session rather than throwing, since
 * "already cleaned up by the test's own happy path" is the common case,
 * not an error.
 */
export async function findSessionIdByTitle(
  request: APIRequestContext,
  title: string,
): Promise<string | undefined> {
  const listing = await (await request.get("/api/sessions")).json();
  return listing.sessions.find((s: any) => s.title === title)?.id;
}

/**
 * Locator for the shared "e2e-session" row start-stack.sh creates at boot
 * — just `rowByTitle` fixed to that one name, since the terminal-family
 * tests need exactly this row and nothing else.
 */
export function sharedSessionRow(page: Page) {
  return rowByTitle(page, "e2e-session");
}

/**
 * Load the app and wait until the terminal is genuinely usable — mounted,
 * socket attached, agent listening. Every wait keys on a marker rather
 * than a sleep, which is why these tests are not flaky on a loaded CI box.
 *
 * PLAN_M2.md step 7 replaces M1's single hardwired session view with a
 * list-then-terminal navigation, so getting to a live terminal now goes
 * through the list UI itself (goto, wait for the row, click it) rather
 * than landing straight on the terminal — every terminal-family test that
 * used to assume M1's one-view app keeps passing through this same helper.
 */
export async function openTerminal(page: Page) {
  await page.goto("/");
  const row = sharedSessionRow(page);
  await expect(row).toBeVisible();
  await row.click();
  // The island sets this once xterm is mounted and the WS is opening.
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  // The fake agent prints this banner once its modes are set.
  await waitForTermText(page, "FAKE-AGENT READY");
}

/**
 * Delete every session with this title, however many exist.
 *
 * Plural on purpose: these tests exist because a duplicate is possible, so
 * their cleanup must not assume the thing they are testing for. A
 * single-session cleanup would leave a stray agent running for the rest of
 * the suite exactly when the test failed.
 */
export async function cleanUpSessionsTitled(request: APIRequestContext, title: string) {
  const listing = await (await request.get("/api/sessions")).json().catch(() => null);
  for (const session of listing?.sessions ?? []) {
    if (session.title !== title) continue;
    await request.post(`/api/sessions/${session.id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${session.id}`).catch(() => {});
  }
}

/**
 * Full text of ONE terminal's buffer (scrollback + viewport), addressed by
 * the DOM element id its island was mounted into — `terminal` for the
 * agent, `terminal-<tabId>` for a tab (`tab_terminal_element_id` in
 * farhelm-ui/src/lib.rs).
 *
 * Reads terminal.js's per-island registry rather than the legacy
 * `__farhelmTerm` singleton, which by design only ever points at the agent
 * terminal. Returns "" for an island that is not mounted, so callers can
 * poll this while a mount is still in flight.
 */
export async function islandText(page: Page, elementId: string): Promise<string> {
  return page.evaluate((el) => {
    const island = (window as any).__farhelmIslands?.[el];
    if (!island) return "";
    const buf = island.term.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < buf.length; i++) {
      lines.push(buf.getLine(i)?.translateToString(true) ?? "");
    }
    return lines.join("\n");
  }, elementId);
}

/**
 * Wait until the island at `elementId` is MOUNTED — an xterm instance and
 * a socket exist for it.
 *
 * Presence in terminal.js's registry is exactly that signal and no more:
 * that file publishes an island on the last line of a successful
 * `mount()`, which is after the `WebSocket` is constructed but BEFORE its
 * `onopen` fires and well before the supervisor-side attach that follows.
 * So this says "the client got as far as opening a socket", never "the
 * terminal is attached". A test that needs the latter has to wait for
 * something only a live attachment can produce — output from the pane, or
 * a command the shell answered.
 *
 * Deliberately not a wait on CONTENT: the agent terminal has the fake
 * agent's banner, but a tab holds a login shell whose prompt is whatever
 * the host's `$SHELL` and rc files produce, which is not something a test
 * may assume the shape of.
 */
export async function waitForIslandMounted(page: Page, elementId: string) {
  await expect
    .poll(
      () => page.evaluate((el) => !!(window as any).__farhelmIslands?.[el], elementId),
      { timeout: 20_000, message: `waiting for the island at ${elementId} to mount` },
    )
    .toBe(true);
}

/** `waitForTermText`, addressed at one island rather than the agent's. */
export async function waitForIslandText(
  page: Page,
  elementId: string,
  needle: string,
  timeout = 15_000,
) {
  await expect
    .poll(() => islandText(page, elementId), {
      timeout,
      message: `waiting for ${needle} in ${elementId}`,
    })
    .toContain(needle);
}

/**
 * The working directories this suite helper's `createTabSession` minted,
 * removed once the spec file is done with them.
 *
 * Swept at the END rather than in each test's own `finally` for a reason
 * that outlives tidiness: a directory removed while its session still
 * exists is a VANISHED working directory, which is a first-class failure
 * mode in this system (SPEC.md fails a tab open and a restart by name when
 * it happens). Deleting these mid-file would inject that condition into
 * whatever ran next, so the removals wait until every session that could
 * be pointing at them is gone. Failure to remove one is not worth failing
 * the run over — an empty leftover directory in the OS temp dir is
 * inert — so this is `force`.
 */
const tabSessionDirs: string[] = [];

/**
 * Create a session whose working directory exists only for this test, and
 * hand back both. The unique cwd is what makes "the tab's shell started in
 * the SESSION's working directory" checkable at all: a shared `/tmp` would
 * match whatever the shell happened to inherit.
 *
 * The agent is the ordinary fake agent — a tab needs the session's tmux
 * session to exist (PLAN_M4.md item 2 refuses a tab on a session with no
 * terminal substrate), so the session has to be genuinely alive.
 */
export async function createTabSession(
  request: APIRequestContext,
  title: string,
): Promise<{ id: string; cwd: string }> {
  const cwd = stackScratchDir("fh-tabs-");
  tabSessionDirs.push(cwd);
  const created = await request.post("/api/sessions", {
    data: { cwd, invocation: FAKE_AGENT_INVOCATION, title },
  });
  expect(created.status(), await created.text()).toBe(200);
  return { id: (await created.json()).id, cwd };
}

/**
 * Click the strip's add control and return the new tab's id, read back
 * from the DOM (`data-tab-id` on the tab's own slot) rather than from the
 * API — which is deliberate: it pins that the UI itself learned the id the
 * open reply carried, since everything it does next (attaching, closing)
 * depends on having it right.
 *
 * `previous` is how many tabs the strip already had, so this waits for the
 * strip to actually grow instead of racing its own click.
 */
export async function addTab(page: Page, previous: number): Promise<string> {
  await page.locator(".tab-add").click();
  const slots = page.locator(".tab-slot");
  await expect(slots).toHaveCount(previous + 1, { timeout: 20_000 });
  const id = await slots.nth(previous).getAttribute("data-tab-id");
  expect(id, "a rendered tab must carry the id the open reply minted").toBeTruthy();
  return id!;
}

/**
 * Select a terminal in the strip and wait for its pane to actually be on
 * screen. `terminal` here is the strip's `data-terminal` value: "agent",
 * or a tab id.
 *
 * The visibility wait is the point: unselected panes are hidden with
 * `visibility` (app.css's `.terminal-pane`, which explains why it cannot
 * be `display: none`), and Playwright treats a `visibility: hidden`
 * element as not visible — so this both drives and verifies the switch.
 */
export async function selectTerminal(page: Page, terminal: string) {
  await page.locator(`.tab-strip [data-terminal="${terminal}"]`).click();
  await expect(
    page.locator(`.terminal-pane[data-terminal="${terminal}"]`),
  ).toBeVisible();
}

/**
 * Type `command` into the terminal mounted at `elementId` and wait for
 * `expected` to appear in its buffer, RETRYING the whole thing until it
 * does.
 *
 * The retry is not flake-tolerance, it is the honest way to drive an
 * interactive login shell: unlike the fake agent (whose `FAKE-AGENT READY`
 * banner is a real readiness signal), `$SHELL -l -i` publishes nothing to
 * wait on, and keystrokes delivered before readline is set up are simply
 * lost. There is no marker to poll for and no sleep that is correct on
 * every host, so the loop keeps offering the command until the shell is
 * far enough along to answer it. The commands these tests send are all
 * idempotent echoes, so a duplicate delivery costs an extra output line
 * and nothing else.
 *
 * `expected` MUST be something only the command's OUTPUT can satisfy,
 * never something its own echoed input line already contains — an
 * interactive shell echoes what you type, so a caller waiting on a literal
 * that appears in the command text is satisfied by the echo alone and
 * returns before the command has run. That is not hypothetical: it made
 * the close test read a pid out of a buffer that only held
 * `echo "TAB-PID:$$"`, and it only failed under enough load to separate
 * the echo from the answer. Hence the `RegExp` option — matching the
 * SHAPE of an expanded result (`/TAB-PID:\d+/`) is how a caller waits for
 * a value it cannot predict.
 */
export async function runInShell(
  page: Page,
  elementId: string,
  command: string,
  expected: string | RegExp,
  timeout = 45_000,
) {
  const deadline = Date.now() + timeout;
  for (;;) {
    await page.locator(`[id="${elementId}"]`).click();
    await page.keyboard.type(command);
    await page.keyboard.press("Enter");
    try {
      await expect
        .poll(() => islandText(page, elementId), { timeout: 5_000 })
        .toMatch(expected);
      return;
    } catch {
      if (Date.now() >= deadline) {
        throw new Error(
          `${expected} never appeared in ${elementId}; buffer was:\n${await islandText(page, elementId)}`,
        );
      }
    }
  }
}

/**
 * A command whose OUTPUT contains `<marker>-42` while its typed form does
 * not, plus that expected text.
 *
 * Two things are deliberate here.
 *
 * The EXPANSION is what makes the assertion about a working shell: an
 * interactive shell echoes the line you type, so a test waiting for a
 * literal already present in the command would be satisfied by the echo
 * alone and would never learn whether anything ran (see `runInShell`'s
 * docs for the failure that taught this). Only a shell that executed the
 * line can produce `42`.
 *
 * The `sh -c` WRAPPER is what makes it portable. A tab runs the user's
 * real login shell — `$SHELL -l -i`, per SPEC.md's SSH-and-type contract —
 * which on a developer's machine is as likely to be fish as bash, and
 * fish parses neither `$((...))` nor `$$`. Typing an explicit `sh -c` is
 * something every one of those shells does understand, so these tests
 * assert on Farhelm's terminal path instead of on whoever's login shell
 * the host happens to have. Single quotes keep the inner text from being
 * expanded by the OUTER shell, which is what leaves the expansion visible
 * in the echo and absent from the output until `sh` runs it.
 */
export function shellMarker(marker: string): { command: string; expected: string } {
  return { command: `sh -c 'echo "${marker}-$((6*7))"'`, expected: `${marker}-42` };
}

/** One island's catch-up record, as terminal.js publishes it. */
interface ReplayRecord {
  buffering: boolean;
  bufferedBytes: number;
  bufferedChunks: number;
  writesWhileHidden: number;
  revealReason: string | null;
  revealed: boolean;
  revealedInWriteCallback: boolean;
  viewportAtTailOnReveal: boolean | null;
  holdMarker: boolean;
  heldReason: string | null;
  limits: { bufferBytes: number; bufferChunks: number; idleMs: number };
}

/** One island's catch-up record right now, without waiting for anything. */
export async function replayRecord(page: Page, elementId: string): Promise<ReplayRecord> {
  return page.evaluate(
    (el) => (window as any).__farhelmIslands[el].test.replay,
    elementId,
  );
}

/**
 * Wait until this island has REVEALED, then return its catch-up record.
 *
 * Polled on `revealed` rather than on `revealReason`, and the difference
 * is not pedantry: the reason is recorded when the phase ENDS, which is
 * one asynchronous write-completion ahead of the reveal itself — so a read
 * keyed on the reason can catch `revealedInWriteCallback` and
 * `viewportAtTailOnReveal` before either has been written, and assert
 * against their initial values instead of against what happened.
 */
export async function waitForReplayReveal(
  page: Page,
  elementId: string,
  timeout = 60_000,
): Promise<ReplayRecord> {
  await expect
    .poll(
      () =>
        page.evaluate(
          (el) => !!(window as any).__farhelmIslands?.[el]?.test?.replay?.revealed,
          elementId,
        ),
      { timeout, message: `waiting for ${elementId} to reveal` },
    )
    .toBe(true);
  return replayRecord(page, elementId);
}

/**
 * Retune the reconnect ladder, the probe interval, and the heartbeat for
 * the page loaded next.
 *
 * An init script rather than an `evaluate`: terminal.js resolves these
 * values once per mount, and the mount happens during the navigation these
 * tests are about to make. Installing them before that navigation is the
 * only way to affect the new attachment.
 *
 * The values are deliberately tiny. What is under test is the SHAPE — the
 * climb, the boundary into probing, the heartbeat noticing silence — and
 * the shape is what the Rust unit tests cannot reach; the numbers
 * themselves are pinned there instead (`reconnect::tests`), where they cost
 * no wall clock at all.
 */
export async function reconnectTimingsFromNextLoad(
  page: Page,
  overrides: {
    delaysMs?: number[];
    probeIntervalMs?: number;
    heartbeatIdleMs?: number;
    heartbeatTimeoutMs?: number;
  },
) {
  await page.addInitScript((overrides) => {
    (window as any).__farhelmTestReconnect = overrides;
  }, overrides);
}

/**
 * Turn auto-reconnect and the heartbeat OFF for the page loaded next,
 * leaving a terminal that loses its socket exactly as it behaved before
 * PLAN_M6.md item 7.
 *
 * Used by the tests that predate this feature and whose subject is a
 * terminal in its DETACHED state — M5's degrade-on-close presentation, and
 * the two attachment contracts about a socket that is gone. Auto-reconnect
 * repairs precisely that state, within a few hundred milliseconds, so
 * without this those tests would quietly stop testing what they were
 * written for and start racing this feature instead. The contracts they
 * pin are unchanged and still hold; what changed is only that the state
 * they need no longer persists on its own.
 */
export async function disableReconnectFromNextLoad(page: Page) {
  await page.addInitScript(() => {
    (window as any).__farhelmTestReconnect = { disabled: true };
  });
}
