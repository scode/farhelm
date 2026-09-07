// The drive-a-real-agent Playwright helper's own spec (PLAN_M6_5.md item
// 3). A dedicated file, not an addition to the terminal spec family, per this
// milestone's testing decision (see mouse-modes.spec.ts's identical
// header note).
//
// Every test but the last is CI-runnable and needs no vendor credentials
// — they drive `helpers/real-agent.ts` against
// `farhelm internal fake-agent --script basic`, using its ECHO behavior
// as a synthetic-marker generator: typing arbitrary text and waiting for
// `echo:<that text>` lets a test plant or release ANY marker it wants
// with no fixture change, which is how the regression tests below pin
// specific bugs two review passes found in the helper's earlier versions
// (each test names the finding it exists for). They prove the helper's
// PLUMBING, not that it defeats a real vendor's onboarding UI or paste
// heuristic — the fake agent has neither.
//
// The LAST test drives a real `claude` invocation end to end (trust
// dialog through detected reply) and is gated on `FARHELM_REAL_AGENT=1`.
// Real agents need vendor auth and network CI does not have and must
// never depend on, so it SKIPS LOUDLY (a visible reason, both on the
// skip itself and as a `console.log` CI's dot reporter will not eat)
// rather than silently passing when the flag is unset — the same
// discipline the cgroup tests use when no systemd user manager is
// available (see e.g. `crates/farhelm/tests/e2e/harness.rs`). Real-agent
// smoke stays manual per SPEC_impl.md's testing section; this test is
// what makes the next manual round a
// `FARHELM_REAL_AGENT=1 npx playwright test real-agent.spec.ts` away
// instead of a re-discovery.
import { test, expect, Page, APIRequestContext } from "@playwright/test";
import path from "node:path";
import fs from "node:fs";
import {
  CLAUDE_CODE_MARKERS,
  submitPrompt,
  waitForReplyMarker,
  waitUntilAgentReady,
  type SubmitPromptEvent,
} from "./helpers/real-agent";
import { stackScratchDir } from "./helpers/scratch";
import { waitForSessionReady } from "./helpers/terminal-readiness";

/**
 * The fake-agent invocation for every CI-runnable leg below, built from
 * an absolute path exactly like the terminal family's shared fixture and
 * mouse-modes.spec.ts's own constant.
 */
const FAKE_AGENT_INVOCATION = `"${
  path.resolve(__dirname, "../../target/debug/farhelm")
}" internal fake-agent --script basic`;

/**
 * The word the real-agent leg asks Claude to reverse, and the reversal it
 * then waits for as the reply marker.
 *
 * The marker must never appear in the PROMPT text itself, or
 * `waitForReplyMarker` would resolve the instant the composer echoes
 * what was just typed — long before Claude has answered anything. A
 * word's reversal is not GUARANTEED to differ from the word (a
 * palindrome's does not), so "positive" being visibly not one is not
 * what this relies on — the real-agent test below checks the built
 * prompt programmatically before ever submitting it.
 */
const PROBE_WORD = "positive";
const REPLY_MARKER = [...PROBE_WORD].reverse().join("");

/**
 * Open the list view's inline create form, fill in its three fields, and
 * submit — capturing the resulting session's id straight off the create
 * RESPONSE, not a follow-up `GET /api/sessions` lookup by title.
 *
 * The submit is folded in here rather than left to the caller: an
 * earlier version returned the form locator after filling it, but both
 * call sites immediately clicked submit with it and nothing here ever
 * needed the form kept around afterward. The id capture via
 * `page.waitForResponse` (registered before the click, so it cannot miss
 * a response that lands fast) is what makes a caller's later cleanup
 * TRUSTWORTHY: a title-based search run sometime after creation can
 * simply miss (a race, a rename, a transient failure), silently turning
 * a leaked session into a swallowed no-op — a finding against an earlier
 * version of this file. The create response's own `id` field cannot miss
 * in that way; it is the one fact about a just-created session nothing
 * has had a chance to invalidate yet.
 */
async function createSession(
  page: Page,
  { cwd, invocation, title }: { cwd: string; invocation: string; title: string },
): Promise<{ id: string }> {
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
  const [response] = await Promise.all([
    page.waitForResponse(
      (r) => r.request().method() === "POST" && r.url().endsWith("/api/sessions"),
    ),
    form.locator('button[type="submit"]').click(),
  ]);
  const body = await response.json();
  return { id: body.id as string };
}

/**
 * Look up a session's id from the real API by its title — used ONLY as
 * `cleanupOrThrow`'s last resort, when the caller's own happy path never
 * got far enough to capture an id from the create response itself (e.g.
 * a failure between `page.goto` and `createSession` returning).
 */
async function findSessionIdByTitle(
  request: APIRequestContext,
  title: string,
): Promise<string | undefined> {
  const listing = await (await request.get("/api/sessions")).json();
  return listing.sessions.find((s: any) => s.title === title)?.id;
}

/**
 * Delete a session, tolerating it already being gone. No separate stop
 * call first, unlike an earlier version of this helper: SPEC.md's delete
 * "removes the session and its stored state, in any state, terminating
 * the agent and tabs if running" already covers a still-live session, so
 * chaining stop-then-delete only added a step whose OWN failure could
 * block the delete that would have worked anyway.
 */
async function cleanupSession(request: APIRequestContext, id: string) {
  const deleted = await request.delete(`/api/sessions/${id}`);
  if (!deleted.ok() && deleted.status() !== 404) {
    throw new Error(
      `cleanup: deleting session ${id} failed (${deleted.status()}): ${await deleted.text()}`,
    );
  }
}

/**
 * Resolve a session to clean up — `id` if `createSession` captured one,
 * else a best-effort title lookup — and delete it, THROWING if neither
 * source produces an id.
 *
 * That throw is deliberate, not a bug to guard against: an unidentifiable
 * session is not "nothing to clean up", it is a LEAK — and on the
 * real-agent leg specifically, a leaked session means an authenticated
 * `claude` PROCESS left running with no way for this run to ever find and
 * stop it again, right before the scratch directory that was its cwd
 * gets deleted out from under it. Swallowing that into a silent no-op
 * (an earlier version of this file did, via `id ?? lookup().catch(() =>
 * undefined)`) hid exactly the failure this function exists to surface.
 */
async function cleanupOrThrow(
  request: APIRequestContext,
  title: string,
  id: string | undefined,
): Promise<void> {
  const resolved = id ?? (await findSessionIdByTitle(request, title));
  if (!resolved) {
    throw new Error(
      `cleanup: no session id was captured at creation, and none could be found by title ` +
        `${JSON.stringify(title)} — this session has leaked`,
    );
  }
  await cleanupSession(request, resolved);
}

/**
 * Run `body`, then always run `cleanup` — aggregating failures rather
 * than letting either mask the other.
 *
 * Plain `try { body() } finally { cleanup() }` has a sharp edge:
 * `cleanupOrThrow` above is designed to fail LOUDLY (a leaked session is
 * a real bug), but a `finally` block that throws REPLACES whatever error
 * the body was already failing with — plain JS semantics, and exactly
 * backwards here, since a cleanup failure is secondary information next
 * to whatever the test was actually asserting. This appends the cleanup
 * failure to the body's own error when both fail, and lets either one
 * surface untouched when only one does — which is what makes `cleanup`
 * safe to let throw at all instead of needing its own swallow-and-log.
 */
async function runWithCleanup(body: () => Promise<void>, cleanup: () => Promise<void>): Promise<void> {
  try {
    await body();
  } catch (bodyError) {
    try {
      await cleanup();
    } catch (cleanupError) {
      const describe = (e: unknown) => (e instanceof Error ? e.stack ?? e.message : String(e));
      throw new Error(
        `${describe(bodyError)}\n\nadditionally, cleanup failed:\n${describe(cleanupError)}`,
      );
    }
    throw bodyError;
  }
  await cleanup();
}

/**
 * Create a `basic` fake-agent session and wait for it to be ready,
 * through the exact create-dialog flow every other spec in this suite
 * uses. Shared by every CI-runnable test below rather than inlined per
 * test — each test gets its OWN fresh session (never a shared one),
 * because several of them deliberately leave odd content in the pane (a
 * synthetic dialog marker that never clears, a bare Enter's empty echo)
 * that must not leak into another test's own marker matching.
 */
async function createReadyFakeAgentSession(page: Page, title: string): Promise<string> {
  await page.goto("/");
  const { id } = await createSession(page, { cwd: "/tmp", invocation: FAKE_AGENT_INVOCATION, title });
  // Success navigates straight into the new session's terminal, same as
  // every create-dialog flow in terminal.spec.ts.
  await waitForSessionReady(page, id);
  // The fake agent never shows an onboarding dialog, so this exercises
  // `waitUntilAgentReady`'s "nothing to dismiss" leg specifically — the
  // same code path a real agent's non-empty markers drive.
  await waitUntilAgentReady(page, { trustDialogMarkers: [], readyMarker: "FAKE-AGENT READY" });
  return id;
}

test("the real-agent helper's pacing and marker-waiting work against the fake agent", async ({ page, request }) => {
  const title = `real-agent-helper-${Date.now()}`;
  let id: string | undefined;
  await runWithCleanup(
    async () => {
      id = await createReadyFakeAgentSession(page, title);

      // Round-tripped twice with distinct payloads: proof that typing,
      // settling, and sending Enter separately delivers the WHOLE line
      // intact (nothing dropped, doubled, or reordered by the settle
      // gap), and that marker-based detection finds each reply
      // specifically rather than merely noticing "something changed".
      await submitPrompt(page, "helper-probe-one");
      await waitForReplyMarker(page, "echo:helper-probe-one");
      await submitPrompt(page, "helper-probe-two");
      await waitForReplyMarker(page, "echo:helper-probe-two");
    },
    () => cleanupOrThrow(request, title, id),
  );
});

// Regression for a bypass an earlier review pass found: `waitUntilAgentReady`'s
// dialog-dismiss branch used to `continue` straight back to the top of its
// loop WITHOUT ever checking the deadline, so a dialog marker that never
// clears (a real stuck dialog, or simply the wrong marker) pressed Enter
// forever and the function's own promised timeout — and its pane
// diagnostic — never fired. The fix moved the deadline check to the very
// top of every iteration, unconditionally; this test pins that by
// manufacturing a marker that persists in the pane by construction (the
// fake agent's echo never goes away) and asserting the wait still
// rejects, on schedule, with the diagnostic intact.
test("waitUntilAgentReady still honors its deadline when a dialog marker never clears", async ({ page, request }) => {
  const title = `real-agent-deadline-${Date.now()}`;
  let id: string | undefined;
  await runWithCleanup(
    async () => {
      id = await createReadyFakeAgentSession(page, title);

      // Conjure a synthetic "dialog" with no fixture change at all: the
      // basic fake agent echoes back whatever it is sent, so typing this
      // line and waiting for its echo plants a string that then sits in
      // the pane exactly like an undismissable dialog would — nothing
      // about it ever clears on its own.
      const dialogMarker = `synthetic-dialog-${Date.now()}`;
      await submitPrompt(page, dialogMarker, 50);
      await waitForReplyMarker(page, `echo:${dialogMarker}`);

      const start = Date.now();
      await expect(
        waitUntilAgentReady(
          page,
          { trustDialogMarkers: [dialogMarker], readyMarker: "a-ready-marker-that-never-appears" },
          { timeoutMs: 1_000, pollMs: 100, dialogRetryMs: 100 },
        ),
      ).rejects.toThrow(/timed out waiting for ready marker/);
      // Bounded comfortably past the 1s `timeoutMs` above: proof this is
      // a genuine, on-schedule rejection from the function itself, not
      // this ASSERTION's own timeout papering over a wait that never
      // actually settles.
      expect(Date.now() - start).toBeLessThan(5_000);
    },
    () => cleanupOrThrow(request, title, id),
  );
});

// Regression for the other half of the same finding: nothing proved
// `waitForReplyMarker` actually WAITS, as opposed to trivially resolving
// on whatever happens to be on screen already. Two properties, pinned
// separately: it must reject (never hang, never resolve) when its marker
// genuinely never shows up — silence is not success — and it must stay
// pending across unrelated output and a quiet gap, resolving only once
// its own marker is actually produced.
test("waitForReplyMarker rejects when its marker never appears, rather than resolving on silence", async ({ page, request }) => {
  const title = `real-agent-never-marker-${Date.now()}`;
  let id: string | undefined;
  await runWithCleanup(
    async () => {
      id = await createReadyFakeAgentSession(page, title);
      await expect(
        waitForReplyMarker(page, `marker-that-will-never-print-${Date.now()}`, 1_000),
      ).rejects.toThrow();
    },
    () => cleanupOrThrow(request, title, id),
  );
});

test("waitForReplyMarker stays pending through unrelated output and a quiet gap, resolving only once its marker is echoed", async ({ page, request }) => {
  const title = `real-agent-pending-${Date.now()}`;
  let id: string | undefined;
  await runWithCleanup(
    async () => {
      id = await createReadyFakeAgentSession(page, title);

      const releaseMarker = `release-marker-${Date.now()}`;
      let settled = false;
      const waiter = waitForReplyMarker(page, `echo:${releaseMarker}`, 10_000).then(() => {
        settled = true;
      });

      // Unrelated output arrives, and then the terminal goes quiet — the
      // exact shape a SILENCE-based "done replying" heuristic would
      // misread as completion (lesson (c)). The waiter above must not
      // care about either.
      await submitPrompt(page, "unrelated-output-while-waiting");
      await waitForReplyMarker(page, "echo:unrelated-output-while-waiting");
      await page.waitForTimeout(1_000);
      expect(settled, "the release marker has not been echoed yet; the waiter must still be pending")
        .toBe(false);

      // Release it: only now does the awaited marker actually appear,
      // and only now may the waiter resolve.
      await submitPrompt(page, releaseMarker);
      await waiter;
      expect(settled).toBe(true);
    },
    () => cleanupOrThrow(request, title, id),
  );
});

// Regression for a stale-marker footgun a later review pass found: nothing
// stopped `waitForReplyMarker` from being handed a marker that was
// already sitting in the buffer for an unrelated reason (or reused from
// an earlier wait), which would resolve the wait INSTANTLY on history it
// did not itself explain — silently wrong, never loudly wrong. The fix is
// a precondition at the top of the function (see its docs); this test
// establishes a marker is genuinely on screen, then asserts a second wait
// for that SAME marker rejects with the precondition's own message.
test("waitForReplyMarker rejects a marker that is already present in the buffer", async ({ page, request }) => {
  const title = `real-agent-stale-marker-${Date.now()}`;
  let id: string | undefined;
  await runWithCleanup(
    async () => {
      id = await createReadyFakeAgentSession(page, title);
      const marker = `stale-marker-${Date.now()}`;
      await submitPrompt(page, marker);
      await waitForReplyMarker(page, `echo:${marker}`);

      await expect(waitForReplyMarker(page, `echo:${marker}`, 1_000)).rejects.toThrow(
        /precondition violated/,
      );
    },
    () => cleanupOrThrow(request, title, id),
  );
});

// Coverage for a wrapped-line finding: xterm splits a paragraph into
// several buffer ROWS purely because of the terminal's column width, and
// an earlier version of `termText` joined every row with a bare `\n`,
// turning each wrap point into an invisible line break that could slice a
// marker in half. The fix reconstructs logical lines using each buffer
// row's `isWrapped` flag before joining (see `helpers/real-agent.ts`'s
// `bufferText`). This drives that fix for real: `term.cols` (read live,
// not assumed) sizes a padded line so the marker straddles the screen's
// right edge, meaning roughly half its characters land on one wrapped row
// and half on the next.
test("termText reconstructs a reply marker that xterm wrapped across the screen edge", async ({ page, request }) => {
  const title = `real-agent-wrap-${Date.now()}`;
  let id: string | undefined;
  await runWithCleanup(
    async () => {
      id = await createReadyFakeAgentSession(page, title);

      const cols = await page.evaluate(() => (window as any).__farhelmTerm.cols as number);
      const marker = `wrap-marker-${Date.now()}`;
      const echoPrefixLen = "echo:".length;
      // Padding fills up to just short of the right edge, so the marker
      // itself begins before column `cols` and finishes after it — half
      // on the row that wraps, half on the row it wraps onto.
      const padLen = Math.max(0, cols - echoPrefixLen - Math.floor(marker.length / 2));
      const padded = "x".repeat(padLen) + marker;

      await submitPrompt(page, padded);
      await waitForReplyMarker(page, `echo:${padded}`);
    },
    () => cleanupOrThrow(request, title, id),
  );
});

// Regression for a third finding: nothing proved `submitPrompt`'s settle
// gap (lesson (b)) was a real elapsed wait rather than a no-op that
// happened to still work against the fake agent's forgiving line reader.
// `SubmitPromptEvent` is a pure observation seam — it cannot alter what
// gets typed, waited, or sent — so asserting on its timestamps is
// asserting on `submitPrompt`'s ACTUAL behavior, not a parallel
// implementation of it.
test("submitPrompt's settle gap is a real elapsed wait between typing and Enter", async ({ page, request }) => {
  const title = `real-agent-pacing-${Date.now()}`;
  let id: string | undefined;
  await runWithCleanup(
    async () => {
      id = await createReadyFakeAgentSession(page, title);

      const settleMs = 500;
      const events: SubmitPromptEvent[] = [];
      await submitPrompt(page, "pacing-probe", settleMs, (event) => events.push(event));

      expect(events.map((e) => e.event)).toEqual(["type-complete", "settle-elapsed", "enter-sent"]);
      expect(events[1].atMs - events[0].atMs).toBeGreaterThanOrEqual(settleMs);
      // Enter itself should follow the settle almost immediately — this
      // pins the settle gap specifically, not general test slowness.
      expect(events[2].atMs - events[1].atMs).toBeLessThan(1_000);

      await waitForReplyMarker(page, "echo:pacing-probe");
    },
    () => cleanupOrThrow(request, title, id),
  );
});

test("drives a real `claude` session through the trust dialog to a detected reply", async ({ page, request }) => {
  if (process.env.FARHELM_REAL_AGENT !== "1") {
    // `test.skip`'s own reason string is NOT printed by CI's default
    // "dot" reporter (Playwright surfaces it only in the HTML/list
    // reporters), so the loud-skip discipline this leg owes needs its
    // OWN visible line — otherwise "skips loudly" is true on paper and
    // silent in the logs a human actually reads in CI.
    console.log(
      "SKIPPED: real-agent leg — FARHELM_REAL_AGENT!=1 (needs vendor credentials and network " +
        "CI does not have; set FARHELM_REAL_AGENT=1 to run it deliberately — PLAN_M6_5.md item 3)",
    );
  }
  test.skip(
    process.env.FARHELM_REAL_AGENT !== "1",
    "set FARHELM_REAL_AGENT=1 to run this against a real `claude` binary on PATH, already " +
      "authenticated (needs vendor credentials and network CI does not have — PLAN_M6_5.md " +
      "item 3 keeps this manual-only; SPEC_impl.md's testing section names real-agent smoke " +
      "as a standing manual gap)",
  );
  // The default project timeout (60s, playwright.config.ts) is sized for
  // the fake agent's instant echo; a real completion over the network,
  // plus the trust-dialog dismiss loop's own retry waits, needs more
  // slack than that.
  test.setTimeout(180_000);

  // A FRESH scratch directory, not `/tmp` (unlike every fake-agent leg
  // above): Claude Code's folder-trust dialog only appears for a
  // genuinely untrusted workspace, and this test exists specifically to
  // prove that dialog gets detected and dismissed. Removed afterwards
  // regardless of outcome (see the nested `finally` below); best-effort,
  // since accepting trust is the vendor's OWN write into the user's real
  // `~/.claude.json`, never into this directory, and is not this test's
  // concern to undo.
  const scratch = stackScratchDir("farhelm-real-agent-");
  const title = `real-agent-${Date.now()}`;
  let id: string | undefined;
  try {
    await runWithCleanup(
      async () => {
        await page.goto("/");
        const created = await createSession(page, {
          cwd: scratch,
          // No flags, and Claude Code found on PATH: the plain invocation
          // is the one a real user types (SPEC.md), and the one basename
          // derivation must recognize.
          invocation: "claude",
          title,
        });
        id = created.id;
        await waitForSessionReady(page, id);

        // Lesson (a): the scratch cwd above is untrusted, so Claude
        // Code's folder-trust dialog is EXPECTED here.
        // `minDialogDismissals: 1` makes that expectation part of the
        // wait itself, not merely an assertion bolted on afterward —
        // readiness is not accepted unless a dialog was actually seen
        // and dismissed, which is what stops this leg from silently
        // passing on a run where dialog detection missed it, or the
        // directory turned out to already be trusted somehow. Timeout
        // raised past the helper's own 30s default: real network latency
        // and the dialog's own retry waits both need more slack than the
        // generic (dialog-free, fake-agent) case does.
        const { dialogDismissals } = await waitUntilAgentReady(page, CLAUDE_CODE_MARKERS, {
          timeoutMs: 60_000,
          minDialogDismissals: 1,
        });
        expect(dialogDismissals, "the trust dialog must actually have been dismissed")
          .toBeGreaterThanOrEqual(1);

        // Lessons (b) and (c) together: submit a prompt whose answer this
        // build cannot predict (a real LLM), split into typed-text-then-
        // settled-Enter so the paste heuristic cannot eat the
        // submission, and wait for the COMPUTED reversal — never for
        // silence — as proof a reply actually arrived. No digits
        // anywhere in the prompt: a numbered dialog racing this send
        // could otherwise read a stray digit as a menu selection (the
        // same caution `real_agent_capture.rs` documents for its own
        // prompt).
        const prompt = `Reverse the letters of the word "${PROBE_WORD}" and reply with only the result: ` +
          `no explanation, no punctuation, no other words.`;
        // Checked here rather than merely asserted in a comment: a
        // word's reversal is not GUARANTEED to differ from the word
        // itself (a palindrome's does not), and if `PROBE_WORD` ever
        // became one, the marker would already sit in the composer the
        // instant the prompt is typed, resolving `waitForReplyMarker`
        // before Claude ever answers.
        expect(prompt, "the probe word must not be a palindrome").not.toContain(REPLY_MARKER);
        await submitPrompt(page, prompt);
        await waitForReplyMarker(page, REPLY_MARKER, 90_000);
      },
      () => cleanupOrThrow(request, title, id),
    );
  } finally {
    // Scratch removal in its own finally, outside `runWithCleanup`: it
    // must run even if session cleanup itself throws (which
    // `runWithCleanup` now lets happen, deliberately, when a session
    // cannot be identified — see `cleanupOrThrow`'s docs).
    fs.rmSync(scratch, { recursive: true, force: true });
  }
});
