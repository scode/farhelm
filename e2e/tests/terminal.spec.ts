// The M1 acceptance suite at the browser level (PLAN_M1.md criterion 5),
// grown by PLAN_M2.md step 7 to cover the list view and navigation, and by
// step 8 to cover the create dialog and per-row stop/delete actions: these
// tests cover output rendering, input round-trip, reconnect replay,
// resize, last-attach-wins takeover, the session list, the create/stop/
// delete UI, and the list/terminal navigation lifecycle — all against a
// real helm, supervisor, tmux, and fake agent, with a handful of
// deliberate exceptions that intercept `page.route` instead: the
// truncation banner (pinning a ~500-session-cap reply without actually
// creating hundreds of sessions), the Unknown-status confirm wording
// (provoking that status needs an old-shaped peer, not anything this
// build's own supervisor can produce — see that test's own docs), the
// stop/delete failure-surfacing tests (forcing failures a healthy stack
// would never hand back on its own), and two confirming-state poll tests
// (a synthetic marker-carrying listing to prove a refetch's RESULT
// reached the DOM, and a synthetic one-shot 500 to prove a failed refetch
// doesn't clear `confirming` — neither is reachable by driving the real
// stack alone). Every other test drives the real stack end to end.
//
// Assertions read the xterm.js BUFFER, not the DOM: the DOM renderer
// materializes only viewport rows, so scrolled-off content (exactly what
// replay tests care about) never appears in .xterm-rows. The buffer is
// the semantic truth of what the terminal holds.
import { test, expect, Page, APIRequestContext } from "@playwright/test";
import path from "node:path";
// The terminal-tab tests at the end of this file need a working directory
// nothing else can be sitting in, so they mint one per test — the stack
// runs on this same machine (see playwright.config.ts's webServer), so a
// directory this process creates is the same directory the supervisor's
// shell lands in.
import fs from "node:fs";
import os from "node:os";

// Restore the stack to its canonical state — exactly one shared
// "e2e-session", freshly launched — before each PROJECT's pass over this
// file. The suite runs against ONE long-lived stack (see
// playwright.config.ts's webServer), and these tests were written
// assuming one stack lifetime per suite run: the multi-megabyte flood
// test is even placed last in this file specifically so its deliberate
// scrollback pollution poisons nothing after it. A second engine project
// (webkit) broke that assumption — "after it" then included the entire
// second project, whose first attach replayed megabytes of flood output
// on WebKit's slower parser and timed out a dozen content assertions.
//
// A `beforeAll` rather than a setup project or a name-ordered spec file:
// see playwright.config.ts's projects comment for why those alternatives
// are unsound. Its failure fails this file's tests outright instead of
// letting them run against half-reset state.
//
// The canonical session's cwd and invocation are captured from the live
// listing rather than duplicated from start-stack.sh, so the two cannot
// drift. The shared session being ABSENT here is a real invariant
// violation — either a test deleted it (nothing may) or a previous
// project's reset failed mid-way — and is worth failing loudly on
// rather than papering over with a hardcoded recreation.
test.beforeAll(async ({ request }) => {
  const listing = await (await request.get("/api/sessions")).json();
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
});

/**
 * A real, distinguishable agent invocation for the create-dialog tests
 * (PLAN_M2.md step 8): the same fake-agent binary and `basic` script
 * start-stack.sh uses for the shared "e2e-session", built from an
 * absolute path so it works regardless of the supervisor's own cwd. Every
 * dialog-driven session in this file uses this invocation rather than a
 * bare `sleep` — the multi-session flow types into one of them, which
 * `sleep` cannot answer.
 */
const FAKE_AGENT_INVOCATION = `"${path.resolve(__dirname, "../../target/debug/farhelm")}" internal fake-agent --script basic`;

/**
 * A producer fast and heavy enough to actually trip PLAN_M2_5.md's
 * watermark flow control, not merely a session that exists: ~12 MiB of
 * consecutively-numbered `FLOOD-NNNNNNNN` records (`FLOOD_RECORDS` below),
 * unpaced and buffered for throughput (see `fake_agent.rs`'s `flood` for
 * the full sizing argument — it exceeds every Farhelm-side bound on the
 * terminal path combined). Ends with a `FLOOD-DONE` marker and then waits
 * to be killed, so the pane stays alive (and therefore its scrollback
 * intact) for a test to inspect after the burst lands.
 *
 * The GATED variant (`flood_gated`, fake_agent.rs's own docs), not plain
 * `flood`: on a fast host the whole unpaced burst can already be sitting in
 * tmux's pane history before a test even finishes attaching, leaving a
 * sub-watermark replay instead of the LIVE producer these tests need to
 * race a client against — a nondeterministic failure mode found directly
 * while writing these tests. `flood_gated` blocks after printing its ready
 * marker until it has read exactly one input byte, so every test below
 * controls precisely when the producer starts (`sendFloodGateByte`) rather
 * than racing it.
 */
const FLOOD_GATED_AGENT_INVOCATION = `"${path.resolve(__dirname, "../../target/debug/farhelm")}" internal fake-agent --script flood-gated`;

/**
 * The same producer as a COMMAND to type into a shell, ungated, for the
 * per-terminal isolation test at the end of this file.
 *
 * A tab is a plain shell, so the suite's own flood fixture runs inside one
 * as an ordinary command — no fixture support on either side, and a
 * genuinely per-terminal producer rather than a second session. Ungated
 * because the gate exists to let a test control when a producer starts
 * relative to an ATTACH, and here the attach is long since done: the
 * command starts the flood at the moment it is typed.
 */
const FLOOD_AGENT_COMMAND = `"${path.resolve(__dirname, "../../target/debug/farhelm")}" internal fake-agent --script flood`;

/**
 * How many records the `flood`/`flood_gated` fake-agent scripts emit.
 * Duplicated from `fake_agent::FLOOD_RECORDS` (crates/farhelm/src/fake_agent.rs)
 * because that module is private to the bin crate and there is no shared
 * build step between it and this TypeScript suite — the same duplication
 * the Rust e2e suite accepts for the same reason (see its own
 * `FLOOD_RECORDS`). Used in an EQUALITY check against the last record this
 * suite observes, so drift here would cause a false test FAILURE, not a
 * silently weakened assertion — the two constants have no way to notice
 * they disagree short of a test actually running.
 */
const FLOOD_RECORDS = 800_000;

/**
 * Open the list view's inline create form (PLAN_M2.md step 8: "not a
 * modal library" — a plain toggled `<div>`, not a dialog element), fill
 * in the three fields, and optionally submit.
 *
 * `title` is a required argument, not optional: every call site in this
 * file needs a distinct, known title anyway (to look the session up
 * afterward, or to assert on the row it produces), so there is no test
 * here that actually wants the field left blank — "a blank title creates
 * a session titled after the working directory's basename, not an empty
 * string" below fills it with the empty string explicitly rather than
 * omitting it, which reads the same in the test body and drops a branch
 * this helper does not otherwise need.
 *
 * Filling and submitting are split into two steps (rather than one
 * "createSession" helper) because the failure and double-submit tests
 * below need to inspect the form BETWEEN filling it and the response
 * landing — e.g. asserting the submit button is disabled mid-flight, or
 * that a failed submit leaves the fields exactly as filled.
 */
async function fillCreateForm(
  page: Page,
  { cwd, invocation, title }: { cwd: string; invocation: string; title: string },
) {
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.locator('input[type="text"]').nth(0).fill(cwd);
  await form.locator('input[type="text"]').nth(1).fill(invocation);
  await form.locator('input[type="text"]').nth(2).fill(title);
  return form;
}

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
function rowByTitle(page: Page, title: string) {
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
async function findSessionIdByTitle(
  request: APIRequestContext,
  title: string,
): Promise<string | undefined> {
  const listing = await (await request.get("/api/sessions")).json();
  return listing.sessions.find((s: any) => s.title === title)?.id;
}

/** Full text content of the terminal buffer (scrollback + viewport). */
async function termText(page: Page): Promise<string> {
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
 * Poll the buffer until `needle` shows up. Polling, not a one-shot read:
 * terminal output arrives asynchronously over the WebSocket with no DOM
 * event to await, so there is nothing else to hook.
 */
async function waitForTermText(page: Page, needle: string, timeout = 15_000) {
  await expect
    .poll(() => termText(page), { timeout, message: `waiting for ${needle}` })
    .toContain(needle);
}

/**
 * Locator for the shared "e2e-session" row start-stack.sh creates at boot
 * — just `rowByTitle` fixed to that one name, since every terminal test
 * below needs exactly this row and nothing else.
 */
function sharedSessionRow(page: Page) {
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
 * than landing straight on the terminal — every test below that used to
 * assume M1's one-view app keeps passing through this same helper.
 */
async function openTerminal(page: Page) {
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
 * Hold `term.write()` completion callbacks in an array instead of ever
 * invoking them, until `window.__farhelmTest.paused` — terminal.js's OWN
 * record of having crossed HIGH_WATER and sent a real pause — flips true,
 * at which point THIS SAME in-page callback restores real writing and
 * fires every held callback at once, all synchronously in one JS turn.
 * Patches the shared `Terminal.prototype.write`, since the test has no
 * handle to the instance `mount()` is about to construct — so this must
 * run before that call, not after — but this is NOT a patch that stays
 * installed indefinitely: it un-patches itself the first time a crossing
 * is observed, typically within the same attachment's very first burst of
 * writes. A caller that wants a SECOND attachment (a reconnect, say) held
 * the same way must call this again for it.
 *
 * Three things went wrong on the way to this shape, each a real failure
 * mode found while writing this test, not a hypothetical one:
 *
 * - A FIXED DELAY per completion (simulating "a renderer slower than the
 *   producer"), tuned against this suite's observed delivery rate for
 *   `flood` over loopback. That rate is not stable across runs, which
 *   left that version flaky under full-suite CPU contention (sometimes
 *   zero pauses observed in 30s) — a delay is only "slow enough" relative
 *   to some ASSUMED rate.
 * - Holding every callback until a caller-chosen BYTE CAP had
 *   accumulated, comfortably above HIGH_WATER. That reliably crosses
 *   HIGH_WATER, but "comfortably above" is exactly the bug: terminal.js
 *   sends its real pause the instant ITS OWN counter crosses HIGH_WATER,
 *   which stops the server from sending any more bytes at all — so if
 *   this function's cap sits above that point, its own `heldBytes` can
 *   never reach it (nothing arrives to grow it further) and it hangs
 *   forever, having proven the crossing but never releasing anything.
 *   Confirmed directly: `heldBytes` parked below a 5 MiB cap indefinitely
 *   once terminal.js's own 4 MiB HIGH_WATER had already silenced the
 *   stream.
 * - Once fixed to release on `paused` correctly, doing so from a
 *   SEPARATE `page.evaluate` the test called after polling `paused` from
 *   Node still flaked under full-suite load: a Node-side poll-then-
 *   release round-trips through Chromium's remote-debugging protocol, and
 *   under enough CPU contention that round trip can itself take longer
 *   than `TMUX_PAUSE_AFTER_SECS` (tmux.rs) — long enough for tmux's OWN
 *   pause-after to trip on a pane this test only meant to leave paused for
 *   an instant. Once THAT happens, recovering needs the supervisor's
 *   reset-then-replay catch-up (PLAN_M2_5.md) — a full history re-capture,
 *   not a resumed live stream — which is measured in tens of seconds for
 *   this fixture's volume, not the sub-second cost recovering from this
 *   test's OWN brief pause should have.
 *
 * Checking terminal.js's OWN flag from inside the SAME synchronous
 * callback sidesteps all three: it releases at the earliest possible
 * moment consistent with a real crossing (no arbitrary margin to
 * overshoot), and the release itself never leaves the page's own JS
 * turn, so it is as fast as this host's JS engine regardless of how
 * loaded the rest of the suite has left it.
 *
 * One more failure mode this closes, found in review: writes already
 * in-flight through this wrapper when it releases (dispatched before the
 * prototype swap, so still routed through this closure when THEIR
 * completion fires) would, without the `released` flag below, re-check
 * `__farhelmTest.paused` on arrival — which may already be `false` again
 * by then, since the FIRST release already sent a resume. Such a callback
 * would sit in `held` forever, never invoked, quietly inflating
 * terminal.js's own `pendingWrite` by however much it represents (up to
 * LOW_WATER's worth) for the rest of the attachment's life. `released`
 * makes every write dispatched through this wrapper, no matter when its
 * completion actually arrives, pass straight through once the real
 * release has happened.
 */
async function holdTermWrites(page: Page) {
  // Guards a real race: `window.Terminal` is set by a plain `<script>` tag
  // that a fresh navigation's 'load' event does not strictly guarantee has
  // already run by the time the FIRST `page.evaluate` after `goto` reaches
  // the browser — observed directly as an intermittent "Cannot read
  // properties of undefined (reading 'prototype')" without this wait.
  await page.waitForFunction(() => !!(window as any).Terminal);
  await page.evaluate(() => {
    const real = (window as any).Terminal.prototype.write;
    let held: Array<() => void> = [];
    let released = false;
    (window as any).Terminal.prototype.write = function (
      data: unknown,
      cb?: () => void,
    ) {
      return real.call(this, data, () => {
        if (!cb) return;
        if (released) {
          // A write dispatched through this wrapper before the release
          // below, whose completion only arrives afterward — see this
          // function's own docs for why it must pass straight through
          // instead of re-checking `paused`.
          cb();
          return;
        }
        held.push(cb);
        if ((window as any).__farhelmTest?.paused) {
          // Same synchronous turn as observing the crossing — see this
          // function's own docs for why that timing is load-bearing.
          released = true;
          (window as any).Terminal.prototype.write = real;
          const toRelease = held;
          held = [];
          for (const c of toRelease) c();
        }
      });
    };
  });
}

/**
 * Send exactly one byte of terminal input over the raw WebSocket — the
 * gate byte `flood_gated` (fake_agent.rs) blocks on before emitting
 * anything, so that every test using it controls precisely when the
 * producer starts rather than racing its own attach against an unpaced
 * fixture that can otherwise outrun a fast host's attach sequence (see
 * `FLOOD_GATED_AGENT_INVOCATION`'s own docs).
 *
 * Sent directly over `window.__farhelmWs`, not via `page.keyboard`,
 * deliberately: this needs to fire the instant the socket is OPEN and
 * every patch a test installed beforehand is already active, with no DOM
 * click/focus round trip adding timing slack of its own. Polls for
 * `readyState` first because `mount()` publishes `__farhelmWs` (and sets
 * `__farhelmTermReady`) before the socket has necessarily finished its
 * handshake — `WebSocket.send()` throws on a socket that is not yet OPEN.
 */
async function sendFloodGateByte(page: Page) {
  await expect
    .poll(() => page.evaluate(() => (window as any).__farhelmWs?.readyState))
    .toBe(1); // WebSocket.OPEN
  await page.evaluate(() => {
    // The value is arbitrary — `flood_gated` only counts bytes, in raw
    // mode, so nothing downstream interprets or echoes it.
    (window as any).__farhelmWs.send(new Uint8Array([0x67]));
  });
}

/**
 * Create a session running the `flood_gated` fake-agent script (see
 * `FLOOD_GATED_AGENT_INVOCATION`'s docs) via the API, without opening its
 * terminal. Split out from `openFloodSession` because the same-realm
 * reconnect test below needs to `goto("/")` and install a page-wide patch
 * BEFORE any session exists, rather than after — see that test's own docs.
 */
async function createFloodGatedSession(
  request: APIRequestContext,
  title: string,
): Promise<string> {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: FLOOD_GATED_AGENT_INVOCATION, title },
  });
  expect(created.status()).toBe(200);
  return (await created.json()).id;
}

/**
 * Create a session running `flood_gated` and open its terminal in `page`,
 * returning its id for the caller's own cleanup. Sends the gate byte
 * (`sendFloodGateByte`) as the last step, once the terminal is mounted and
 * every patch a caller installed is already active — the producer starts
 * only once this function returns.
 *
 * Every PLAN_M2_5.md watermark test below needs its OWN session running
 * this producer, distinct from the shared "e2e-session" the rest of the
 * file depends on: flooding that shared session would blow past its
 * scrollback (as the multi-megabyte-message test near the end of this
 * file already does deliberately, and only because it is placed last for
 * exactly that reason) and pollute every test that runs after it.
 *
 * `holdWrites` applies `holdTermWrites` before the terminal mounts, for
 * callers that need to OBSERVE a pause/resume cycle rather than just
 * survive one — see that function's docs for why this is necessary at all
 * on typical test hardware. `verifyStream` applies
 * `installFloodStreamVerifier` first (order matters — see that function's
 * docs), for callers that need to verify the ENTIRE stream rather than
 * just the scrollback-capped tail `termText` can still see once it lands.
 *
 * A step after creation failing (the goto, the click, either wait) must
 * not strand the session this function already created: none of this
 * function's callers have an id to clean up with yet if they never got one
 * back, so a leaked `flood_gated` session — a long-running fake-agent
 * process — would sit contaminating every test that runs after it in this
 * serially-run suite. The `catch` below is this function's own cleanup of
 * its own partial work, not a substitute for the caller's `finally`.
 */
async function openFloodSession(
  page: Page,
  request: APIRequestContext,
  title: string,
  { holdWrites = false, verifyStream = false }: { holdWrites?: boolean; verifyStream?: boolean } = {},
): Promise<string> {
  const id = await createFloodGatedSession(request, title);
  try {
    await page.goto("/");
    if (verifyStream) await installFloodStreamVerifier(page);
    if (holdWrites) await holdTermWrites(page);
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible();
    await row.click();
    // NOT `waitForTermText(page, "FAKE-AGENT READY")`: a `holdWrites`
    // caller (or the stall-detach test, which patches `term.write()` to a
    // no-op outright) may never render that banner text at all.
    // `termReady` alone (mount succeeded, socket opening) is the one
    // readiness signal every caller of this helper can rely on.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await sendFloodGateByte(page);
    return id;
  } catch (err) {
    await cleanupSession(request, id);
    throw err;
  }
}

/**
 * Drain a `flood_gated` session to its `FLOOD-DONE` marker over a raw
 * WebSocket that belongs to nobody, so that whatever attaches NEXT sees a
 * finished producer and pure replay instead of a live tail.
 *
 * The reconnect test below needs this: it leaves attachment one paused
 * mid-flood on purpose, and a paused client stops the bytes moving, so the
 * fake agent is still producing when that attachment goes away. The next
 * attachment then gets the session's replay AND however much of the
 * ~12 MiB fixture the producer still had left — measured directly at between
 * 1.7 MiB and 5.4 MiB across consecutive runs on an idle machine, which
 * straddles terminal.js's 4 MiB HIGH_WATER. Above the mark, that
 * attachment pauses for entirely legitimate reasons, and a test asserting
 * "a fresh attachment neither pauses nor resumes" fails on a system that
 * is behaving exactly as designed. Quiescing the producer first removes
 * the variable instead of tolerating it.
 *
 * A RAW socket, not a second UI attachment, and that distinction is the
 * whole point: the reconnect invariant under test is about terminal.js's
 * per-mount closure state surviving (or not) an unmount/mount pair, so the
 * attachment doing the draining must not be one of terminal.js's own
 * mounts. This one never touches `mount()`, so the mount that follows is
 * still the FIRST one after the paused mount.
 *
 * `cols`/`rows` come from the caller (whatever geometry the previous
 * attachment used) rather than the query defaults: every attach resizes
 * the tmux window BEFORE it captures the replay (farhelm-supervisor's
 * `Attach` handler), so draining at a different size would reflow the
 * ~12,000 lines of history that the attachment under test then measures.
 * A drain has no business changing what the next replay looks like.
 *
 * Rejects rather than returning on a socket that closes or errors before
 * the marker arrives — a silent early return would hand the caller the
 * live-tail race back, which is the one thing this exists to remove.
 */
function drainFloodOffScreen(
  page: Page,
  id: string,
  geometry: { cols: number; rows: number },
) {
  return page.evaluate(
    ({ id, cols, rows }) =>
      new Promise<void>((resolve, reject) => {
        const ws = new WebSocket(
          `ws://${location.host}/api/sessions/${id}/term?cols=${cols}&rows=${rows}`,
        );
        ws.binaryType = "arraybuffer";
        const decoder = new TextDecoder();
        // The marker can straddle two frames, so carry its length minus
        // one byte across chunks; nothing else about the stream matters
        // here, which keeps this constant-memory over ~12 MiB.
        const marker = "FLOOD-DONE";
        let carry = "";
        let settled = false;
        const finish = (err?: Error) => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          ws.onmessage = null;
          ws.onclose = null;
          ws.onerror = null;
          ws.close();
          if (err) reject(err);
          else resolve();
        };
        const timer = setTimeout(
          () => finish(new Error("flood_gated did not reach FLOOD-DONE in 60s")),
          60_000,
        );
        ws.onmessage = (ev) => {
          // Text frames are the helm's control JSON (a detach notice),
          // never terminal bytes.
          if (typeof ev.data === "string") return;
          const text = carry + decoder.decode(ev.data);
          if (text.includes(marker)) finish();
          else carry = text.slice(-(marker.length - 1));
        };
        ws.onclose = () =>
          finish(new Error("the drain socket closed before FLOOD-DONE"));
        ws.onerror = () => finish(new Error("the drain socket errored"));
      }),
    { id, ...geometry },
  );
}

/**
 * Stop then delete a session, tolerating ONLY "already gone" (404 — the
 * expected case when a test's own happy path already cleaned up). Every
 * OTHER failure is surfaced rather than swallowed: a silently leaked flood
 * session — a long-running fake-agent process — would otherwise
 * contaminate every test that runs after it in this serially-run suite,
 * and a `.catch(() => {})` here would hide exactly that.
 */
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
 * Extract every `FLOOD-NNNNNNNN` record number from `text`, in the order
 * they appear. The tests below use this to check the visible TAIL is
 * consecutive (no gap = no loss, no repeat/step-back = no duplicated
 * replay) — mirroring the Rust e2e suite's own `flood_records` helper for
 * the identical fixture. This only ever sees whatever xterm.js's scrollback
 * cap still retains, NOT the whole stream — see `installFloodStreamVerifier`
 * for the assertion that covers every record, not just the retained tail.
 */
function parseFloodRecords(text: string): number[] {
  return [...text.matchAll(/FLOOD-(\d{8})/g)].map((m) => Number(m[1]));
}

/**
 * Install a constant-memory, in-page verifier over the ENTIRE flood stream
 * — every record from 0 to `FLOOD_RECORDS - 1`, in order, exactly once,
 * ending with `FLOOD-DONE` — patching `term.write()` the same way
 * `holdTermWrites` does (must run before `mount()` constructs a `Terminal`,
 * for the same reason: the test has no handle to the instance it is about
 * to construct).
 *
 * Why this exists, and why the retained-tail check (`parseFloodRecords`
 * over `termText`) is not enough on its own: xterm.js's scrollback is
 * capped (terminal.js's own `scrollback: 12000`), so by the time this
 * fixture's 800,000 records have all landed, only the last ~12,000 are
 * still readable from the buffer. A test that asserted ONLY on that tail
 * could not tell "every record arrived, in order" from "everything before
 * the last 12,000 silently vanished" — exactly the class of bug
 * PLAN_M2_5.md's "never a silent gap" requirement exists to rule out. This
 * verifier inspects every byte as it arrives, before xterm.js's own
 * scrollback eviction ever gets a chance to make it unobservable, and
 * holds only a small carry-over buffer across chunk boundaries rather than
 * the whole transcript — hence "constant memory".
 *
 * Composes with `holdTermWrites` when both are installed (this function
 * MUST run first): each patches whatever it finds as `Terminal.prototype.write`
 * and calls that as its own "real" implementation, so installing this
 * verifier first and `holdTermWrites` second means the verifier keeps
 * observing every byte even after `holdTermWrites` releases itself and
 * hands writes straight through.
 */
async function installFloodStreamVerifier(page: Page) {
  // Same race `holdTermWrites` guards against — see its own docs.
  await page.waitForFunction(() => !!(window as any).Terminal);
  await page.evaluate(() => {
    const real = (window as any).Terminal.prototype.write;
    const decoder = new TextDecoder();
    const state = {
      // Bytes seen since the last complete record/marker, carried across
      // chunk boundaries — never the whole stream, which is the whole
      // point of verifying as data arrives rather than after the fact.
      leftover: "",
      // `flood_gated` prints "FAKE-AGENT READY" (padded to the pane's row
      // width by tmux) BEFORE ever reading the gate byte, so the very
      // first bytes this verifier sees are never a record — `started`
      // marks having found the first one, after which anything
      // unrecognized is a genuine violation rather than expected preamble.
      started: false,
      nextExpected: 0,
      recordsSeen: 0,
      sawDone: false,
      // The FIRST violation only: once something is wrong, later bytes
      // are not interesting, and holding just one message keeps this
      // genuinely constant-memory even in a pathological failure.
      error: null as string | null,
    };
    (window as any).__farhelmFloodVerify = state;

    // Fixed-width by construction (fake_agent.rs's `flood`): "FLOOD-" (6)
    // + 8 digits + "\r\n" (2) = 16 bytes. "FLOOD-DONE\r\n" is 12 — the
    // SHORTER of the two, so it is NOT the right length to gate "have we
    // ruled out both patterns yet" on: a record chunk can legitimately
    // split anywhere, including mid-CRLF (e.g. "FLOOD-00000311\r" with no
    // "\n" yet, 15 bytes — longer than the DONE marker but still an
    // incomplete record, not corruption). Gating on the LONGER pattern's
    // length is what avoids misjudging that case; see below.
    const RECORD_RE = /^FLOOD-(\d{8})\r\n/;
    const DONE_RE = /^FLOOD-DONE\r\n/;
    const RECORD_LEN = "FLOOD-00000000\r\n".length;

    (window as any).Terminal.prototype.write = function (
      data: unknown,
      cb?: () => void,
    ) {
      if (data instanceof Uint8Array && !state.error && !state.sawDone) {
        let text = state.leftover + decoder.decode(data, { stream: false });
        if (!state.started) {
          // Discard the READY banner (and tmux's own row-padding around
          // it) rather than judging it: it is short and bounded (one
          // line), so accumulating it across a chunk boundary or two
          // cannot grow this verifier's memory in any meaningful way.
          const recordsBegin = text.indexOf("FLOOD-");
          if (recordsBegin === -1) {
            state.leftover = text;
            return real.call(this, data, cb);
          }
          text = text.slice(recordsBegin);
          state.started = true;
        }
        let i = 0;
        while (i < text.length) {
          const rest = text.slice(i);
          const recordMatch = RECORD_RE.exec(rest);
          if (recordMatch) {
            const n = Number(recordMatch[1]);
            if (n !== state.nextExpected) {
              state.error = `record ${n} arrived out of order after ${state.recordsSeen} \
verified records (expected ${state.nextExpected}) — a gap is lost output, a repeat or a step \
back is duplicated replay`;
              break;
            }
            state.nextExpected = n + 1;
            state.recordsSeen++;
            i += recordMatch[0].length;
            continue;
          }
          if (DONE_RE.test(rest)) {
            state.sawDone = true;
            i += "FLOOD-DONE\r\n".length;
            break;
          }
          if (rest.length < RECORD_LEN) {
            // Too short to have ruled out a record straddling a chunk
            // boundary (see `RECORD_LEN`'s own docs for why this is the
            // longer, correct threshold rather than the DONE marker's
            // shorter one) — carry it into the next chunk rather than
            // judging it yet.
            break;
          }
          state.error = `unrecognized bytes in the flood stream after ${state.recordsSeen} \
verified records: ${JSON.stringify(rest.slice(0, 24))}`;
          break;
        }
        state.leftover = state.error ? "" : text.slice(i);
      }
      return real.call(this, data, cb);
    };
  });
}

// First pixels: the whole stack standing up and putting an agent's output
// on screen. Everything below assumes this works, so when the suite goes
// red this is the test that says whether the problem is the stack or the
// behavior under test.
test("renders the session and the agent's TUI output", async ({ page }) => {
  await openTerminal(page);
  await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
  await waitForTermText(page, "fake-agent starting");
  // The visible viewport is real DOM text too — the DOM renderer is what
  // makes these tests semantic rather than pixel-diffing.
  await expect(page.locator(".xterm-rows")).toContainText("FAKE-AGENT READY");
});

// The list view itself (PLAN_M2.md step 7): title, cwd, invocation, and a
// truthful status badge per row, sourced from the same GET /api/sessions
// every other test exercises indirectly through openTerminal. cwd and
// invocation are checked against the API's OWN listing rather than mere
// non-emptiness, so a row silently rendering the wrong session's
// metadata (e.g. a copy-paste bug swapping two fields) would still fail
// this test even though every field it prints is individually non-blank.
// The fake agent process backing "e2e-session" is long-running, so the
// row's status must settle on "alive" rather than the create-time
// "unknown" placeholder — `toHaveText` retries on its own, since the
// list computes status fresh from tmux on every fetch rather than
// caching the placeholder forever.
test("list renders the session row with title, cwd, invocation, and an alive badge", async ({
  page,
  request,
}) => {
  const listing = await (await request.get("/api/sessions")).json();
  const expected = listing.sessions.find((s: any) => s.title === "e2e-session");
  expect(expected).toBeTruthy();

  await page.goto("/");
  const row = sharedSessionRow(page);
  await expect(row).toBeVisible();
  await expect(row.locator(".session-title")).toHaveText("e2e-session");
  await expect(row.locator(".session-cwd")).toHaveText(expected.cwd);
  await expect(row.locator(".session-invocation")).toHaveText(expected.invocation);
  await expect(row.locator(".status-badge")).toHaveText("alive", {
    timeout: 10_000,
  });
});

// Keyboard activation (PLAN_M2.md step 7: rows must be
// keyboard-activatable). The open action (`.session-row-open`, PLAN_M2.md
// step 8) is a native <button> rather than a div with a hand-rolled
// onkeydown, so Enter activation (and Space) come from the browser for
// free — this pins that it is actually reachable and operable via
// keyboard, not just that it happens to look like a button. Focusing
// `.session-row-open` directly, not the outer `.session-row` wrapper: step
// 8 turned the row itself into a plain (non-focusable) `<div>` so it could
// also host the stop/delete buttons as siblings — see the SessionRow doc
// in lib.rs — so the row wrapper no longer accepts focus at all.
test("keyboard activation opens the session, matching a real click", async ({
  page,
}) => {
  await page.goto("/");
  await sharedSessionRow(page).locator(".session-row-open").focus();
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
});

// Navigation lifecycle (PLAN_M2.md step 7): SessionView used to assume it
// never unmounted (M1 had exactly one view), so the JS island only ever
// needed a mount-time double-mount guard. This pins the FULL round trip:
// going back must actually tear down the mounted terminal (not just
// leave it running unobserved), and reopening the SAME session must
// produce a genuinely NEW mount rather than either a no-op or a reused
// instance — replay alone cannot distinguish "correctly reattached" from
// "never actually left", since replaying scrollback from a still-open
// socket would look identical to a correct fresh reattach. Stamping the
// live xterm instance before leaving, and asserting a DIFFERENT instance
// exists after reopening, is what closes that gap.
test("back tears down the mounted terminal; reopening the same session mounts a fresh one", async ({
  page,
}) => {
  await openTerminal(page);
  await expect(page.locator(".back-button")).toBeVisible();

  await page.locator("#terminal").click();
  await page.keyboard.type("marker-before-back");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:marker-before-back");

  await page.evaluate(() => {
    (window as any).__farhelmTerm.__testMarker = "before-back";
    // Stashed under different names so they survive terminal.js's own
    // deletes on unmount — the actual WebSocket object and test hook the
    // mount owned, kept around purely so the assertions below can check
    // unmount() really tore them down (and, for the hook, that reopening
    // installs a genuinely NEW one rather than reusing this one).
    (window as any).__testWsBeforeBack = (window as any).__farhelmWs;
    (window as any).__testHookBeforeBack = (window as any).__farhelmTest;
  });

  await page.locator(".back-button").click();

  // The teardown itself must be observable, not just its later effects:
  // every global terminal.js publishes on mount must be gone, __farhelmTest
  // included (PLAN_M2_5.md step 4's per-mount hook — unmount() only
  // deletes it if it still references THIS mount's own object, terminal.js's
  // own docs, so seeing it gone here is also indirect coverage that guard
  // took the branch it was supposed to)...
  await expect
    .poll(() =>
      page.evaluate(() => ({
        ready: (window as any).__farhelmTermReady,
        term: (window as any).__farhelmTerm,
        ws: (window as any).__farhelmWs,
        test: (window as any).__farhelmTest,
      })),
    )
    .toEqual({ ready: undefined, term: undefined, ws: undefined, test: undefined });
  // ...and the socket it owned must be genuinely closed (readyState 3 —
  // CLOSED; there is no browser `WebSocket` global in this Node-side
  // test context to reference `WebSocket.CLOSED` by name), not merely
  // abandoned with a stale reference that could still fire callbacks
  // into whatever mounts next (the WS-teardown-callbacks review finding
  // this guards against).
  await expect
    .poll(() =>
      page.evaluate(() => (window as any).__testWsBeforeBack.readyState),
    )
    .toBe(3);
  // readyState alone is not enough: a socket can be CLOSED while still
  // holding stale `onmessage`/`onclose`/etc callbacks that reference the
  // torn-down term/view (they simply never fire again once the socket is
  // closed — but a callback left in place is exactly what a regression
  // in unmount()'s "null the handlers before closing" step would look
  // like, and readyState would not catch it since assigning the socket's
  // OWN close doesn't require its handler properties to change).
  expect(
    await page.evaluate(() => {
      const ws = (window as any).__testWsBeforeBack;
      return {
        onopen: ws.onopen,
        onmessage: ws.onmessage,
        onerror: ws.onerror,
        onclose: ws.onclose,
      };
    }),
  ).toEqual({ onopen: null, onmessage: null, onerror: null, onclose: null });

  await expect(page.locator(".session-list")).toBeVisible();
  await expect(page.locator("#terminal")).toHaveCount(0);

  await sharedSessionRow(page).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

  const isFreshInstance = await page.evaluate(
    () => (window as any).__farhelmTerm.__testMarker !== "before-back",
  );
  expect(isFreshInstance).toBe(true);

  // The reopened attachment's test hook must be a genuinely NEW object
  // (not the old one somehow surviving unmount, and not a stale reference
  // reused) with a freshly-zeroed watermark state — the same "fresh, not
  // reused" property `isFreshInstance` above pins for the xterm instance,
  // extended to the hook PLAN_M2_5.md step 4 added.
  const hookState = await page.evaluate(() => {
    const hook = (window as any).__farhelmTest;
    const before = (window as any).__testHookBeforeBack;
    return { isDifferentObject: hook !== before, hook };
  });
  expect(hookState.isDifferentObject).toBe(true);
  expect(hookState.hook).toEqual({ paused: false, pauseCount: 0, resumeCount: 0 });

  // Replay must bring back output produced before THIS attachment
  // existed, exactly like the reload test below — the only difference
  // is that here the round trip goes through the list/back UI instead
  // of a full page reload.
  await waitForTermText(page, "echo:marker-before-back");

  // And the fresh mount must be genuinely live, not just showing stale
  // replayed content: a new marker must round-trip through it.
  await page.locator("#terminal").click();
  await page.keyboard.type("marker-after-reopen");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:marker-after-reopen");
});

// Regression test for the "stale mount retry" bug: terminal.js's wait for
// xterm's globals used to live entirely in a bare `setTimeout` chain
// inside the eval'd JS, with nothing SessionView's teardown could reach
// in and cancel. Backing out of a session before that wait resolved left
// it running; if the user then opened a DIFFERENT session, the stale
// loop could eventually fire and mount the FIRST session's terminal into
// the SECOND session's view (and, since the old mount guard was already
// set by the real mount, silently no-op the real one instead).
//
// An earlier version of this test only clicked through the navigation
// quickly and asserted the Dioxus-rendered `.titlebar .title` afterward
// — which passes even if session A's socket is the one that actually
// mounted, since the titlebar text comes from the `session` PROP
// SessionView was given, entirely independent of what terminal.js did.
// It also never forced a genuinely pending retry in the first place: on
// an unloaded box, `mountWhenReady`'s very first synchronous readiness
// check routinely just succeeds, so there was nothing left running to
// cancel and the test could pass for reasons that had nothing to do with
// the fix.
//
// This version forces the pending state for real (withholding
// `window.Terminal`, which `mountWhenReady` cannot proceed without) and
// makes the resulting race deterministic with Playwright's fake clock
// instead of hoping real wall-clock timing falls out favorably: with the
// clock frozen, session A's retry and session B's retry are scheduled
// for the IDENTICAL virtual instant, and same-deadline timers fire in
// registration order — so if A's retry were never cancelled, it would
// deterministically fire before B's and mount session A's socket into
// the (shared, same-DOM-id) terminal element first, with B's later mount
// then no-opping against the "already mounted" guard. Asserting the
// MOUNTED SOCKET'S URL — not any Dioxus-rendered text — is what actually
// catches that.
//
// terminal.js actually has THREE points that can cancel session A's
// retry (`mountWhenReady`'s own `clearTimeout` on entry, `unmount()`'s
// `clearTimeout`, and `tryMount`'s `pending !== attempt` check), and any
// ONE of them alone is enough to stop the race above — checked directly
// while writing this test by disabling each in isolation and confirming
// it still passed. Only disabling all three at once reproduces the
// original bug (confirmed the same way). That is a real, if incidental,
// defense-in-depth; this test is only equipped to fail if ALL of a
// regression's remaining protections vanish together, not to identify
// which single one a future change removed.
test("backing out before a terminal is ready, then opening a different session, mounts the right one", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: {
      cwd: "/tmp",
      invocation: "sleep 300",
      title: "regression-session-b",
    },
  });
  expect(created.status()).toBe(200);
  const { id: idB } = await created.json();

  try {
    await page.goto("/");
    await expect(sharedSessionRow(page)).toBeVisible();

    // Freeze the page's timers. `install()` alone does NOT pause time —
    // it only swaps in fake implementations, which by themselves keep
    // ticking at native speed — so `pauseAt()` is what actually stops
    // the clock; without it, both retries below would still be driven
    // by real elapsed wall-clock time between actions, defeating the
    // whole point of using the fake clock here. Playwright's own waits
    // (`waitForFunction`) poll from OUTSIDE the page over CDP and are
    // unaffected by any of this; only the page's OWN `setTimeout` calls
    // — exactly what `mountWhenReady`'s retry loop uses — come under our
    // control.
    await page.clock.install();
    await page.clock.pauseAt(new Date());

    // Withhold a global `mountWhenReady` genuinely cannot proceed
    // without, so opening session A puts a REAL pending retry into
    // flight (rather than resolving on its first synchronous check, as
    // it almost always would on an unloaded box).
    await page.evaluate(() => {
      (window as any).__testStashedTerminal = (window as any).Terminal;
      delete (window as any).Terminal;
    });

    await sharedSessionRow(page).click();
    await page.locator(".back-button").click();
    await page.locator(`[data-session-id="${idB}"]`).click();

    // Restore the withheld global, THEN advance the frozen clock: both
    // session A's original retry (if a regression left it running) and
    // session B's fresh one were scheduled for the same virtual instant
    // (nothing advanced the clock between the two clicks), so this is
    // what actually exercises the race described above.
    await page.evaluate(() => {
      (window as any).Terminal = (window as any).__testStashedTerminal;
      delete (window as any).__testStashedTerminal;
    });
    await page.clock.runFor(500);

    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    const wsUrl = await page.evaluate(() => (window as any).__farhelmWs.url);
    expect(wsUrl).toContain(idB);
    await expect(page.locator(".titlebar .title")).toHaveText(
      "regression-session-b",
    );
  } finally {
    await request.post(`/api/sessions/${idB}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${idB}`).catch(() => {});
  }
});

// Playwright-level coverage for the PARTIAL-MOUNT ROLLBACK finding:
// mount() sets its guard (`active`, since terminal.js's simplification —
// see its docs) only at the very end of a successful mount, so an
// exception partway through (a `WebSocket` constructor throwing, here)
// must leave `active` exactly as it was before the attempt — not stuck
// in a state that wedges every later mount shut. Monkeypatching
// `window.WebSocket` to throw is the cleanest deterministic way to break
// mount() partway through: it is the very next thing mount() does after
// constructing the xterm.js `Terminal` (already-real work that must
// itself be rolled back — the terminal.js catch block disposes it),
// requires no changes to production code to trigger, and — restored
// before the second attempt — reproduces the exact "mount, fail, mount
// the SAME session again" sequence a real transient failure would leave
// a user facing.
test("a failed mount rolls back cleanly; the same session can be mounted again", async ({
  page,
}) => {
  await page.goto("/");
  await expect(sharedSessionRow(page)).toBeVisible();

  await page.evaluate(() => {
    (window as any).__testRealWebSocket = window.WebSocket;
    (window as any).WebSocket = class {
      constructor() {
        throw new Error("injected failure for rollback test");
      }
    };
  });

  await sharedSessionRow(page).click();
  // termReady never becomes true on this path — mount() throws before
  // reaching the line that sets it — so the banner text (which the
  // catch block does set) is the only thing to wait on here.
  await expect(page.locator("#term-banner")).toContainText(
    "Failed to start terminal",
  );
  // The failed attempt must not have left anything looking mounted.
  expect(
    await page.evaluate(() => (window as any).__farhelmTerm === undefined),
  ).toBe(true);

  await page.evaluate(() => {
    window.WebSocket = (window as any).__testRealWebSocket;
    delete (window as any).__testRealWebSocket;
  });

  // Reopening the SAME session (back to the list, then the same row)
  // must succeed now that the guard was rolled back — a regression that
  // left `active` (or the old `__farhelmMounted` flag) stuck would make
  // this mount silently no-op instead.
  await page.locator(".back-button").click();
  await sharedSessionRow(page).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitForTermText(page, "FAKE-AGENT READY");
});

// Real keystrokes through the whole chain: xterm's onData, the WebSocket,
// the framing protocol, tmux send-keys on the dedicated input control
// client, and back out as pane output. This is the one test that would
// catch an input path wired up but dead — the failure a user would
// describe as "typing goes nowhere".
test("input round-trips through the real terminal path", async ({ page }) => {
  await openTerminal(page);
  await page.locator("#terminal").click();
  await page.keyboard.type("hello-from-playwright");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:hello-from-playwright", 10_000);
});

// Regression test for a real bug: xterm.js auto-answers a DECRQM mode
// query (e.g. vim's own cursor-blink probe, `ESC[?12$p`, which tmux passes
// through unmodified from the pane) with a DECRPM reply (`ESC[?12;2$y`)
// through its OWN `onData` callback — the identical callback real
// keystrokes flow through. terminal.js used to forward that reply
// straight back as pane input; the render-batch-plus-WebSocket round trip
// means it lands a full turnaround later, long after the querying app
// stopped waiting for an answer. vim then parses it as KEYSTROKES rather
// than a stale reply — '$' is a silent motion and 'y' becomes a pending
// operator, observed as a stray pending 'y' on every vim launch. The fix
// (terminal.js's `swallowDecrqm` parser handlers) intercepts the DECRQM
// QUERY itself on the output side, so xterm never mints a reply at all —
// user input is never inspected, and even a pasted look-alike of a reply
// passes through untouched. Safe because tmux answers DECRQM for its own
// panes itself, instantly (verified directly by probing), so the reply
// xterm no longer sends was a late duplicate — pure harm, never the only
// copy.
//
// This asserts on the WEBSOCKET FRAMES ACTUALLY SENT rather than driving
// vim end to end: vim is not otherwise a CI dependency of this suite, and
// reproducing its stray-'y' symptom would mean racing a real editor
// against the network — brittle and slow next to checking the fix's own
// contract directly ("this exact byte shape never leaves the browser as
// input"). Feeding a real shell's pane output raw DECRQM/OSC-11 query
// bytes via `printf` makes xterm.js generate the very same auto-replies
// vim would trigger, deterministically, with no editor involved.
//
// Runs against a fresh `bash` session, not the shared fake-agent one: the
// fake agent's `basic` script only ever echoes typed lines back as text
// (fake_agent.rs) — it never executes anything, so it could never emit
// the raw escape bytes this test needs on the wire in the first place.
test("DECRPM auto-replies to a mode query are dropped, not forwarded as pane input", async ({
  page,
  request,
}) => {
  // Patch only `send` on the WebSocket PROTOTYPE, before any navigation.
  // Replacing the `WebSocket` constructor outright (as the mount-rollback
  // test above does deliberately, to make it throw) would break RECEIVING
  // frames too — this test still needs that, to see PROBE-DONE arrive.
  // Binary frames are recorded as plain byte arrays rather than left as
  // Uint8Array/ArrayBuffer instances, so they survive the `page.evaluate`
  // round trip back to Node intact.
  await page.addInitScript(() => {
    const realSend = WebSocket.prototype.send;
    (window as any).__sentInput = [];
    WebSocket.prototype.send = function (this: WebSocket, data: any) {
      if (data instanceof Uint8Array) {
        (window as any).__sentInput.push(Array.from(data));
      } else if (data instanceof ArrayBuffer) {
        (window as any).__sentInput.push(Array.from(new Uint8Array(data)));
      } else {
        (window as any).__sentInput.push(data);
      }
      return realSend.call(this, data);
    };
  });

  const title = `decrpm-probe-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "bash", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible();
    await row.click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    // `__farhelmTermReady` means the terminal MOUNTED, not that its
    // socket reached OPEN — and terminal.js drops input sent before OPEN.
    // Under WebKit's slower startup that gap is wide enough to eat the
    // first characters of the probe command, so wait for the socket
    // itself, the way sendFloodGateByte does.
    await page.waitForFunction(
      () => (window as any).__farhelmWs?.readyState === WebSocket.OPEN,
    );

    await page.locator("#terminal").click();
    // Two queries go out together: DSR-6 (`\e[6n`, "where is the
    // cursor?"), which xterm must still answer, and the DECRQM query for
    // mode 12 (`\e[?12$p`, vim's cursor-blink probe), which must now
    // provoke NO reply at all. PROBE-DONE is the synchronization marker
    // proving the whole line — including the 1-second gap — actually ran
    // in this real shell, not merely that it was typed.
    //
    // DSR-6 rather than an OSC-11 color query as the control, and that
    // choice is CI-hardened rather than arbitrary: headless WebKit on
    // the CI runner never answers OSC 11 (it has no theme colors to
    // report), so a color-reply control failed there while the real
    // assertion held. A cursor report is computed from xterm's own
    // buffer and is therefore environment-independent.
    //
    // The probe line is PASTED (xterm's programmatic paste API), not
    // typed. This test's one recurring CI flake was the control
    // assertion finding no cursor report: `page.keyboard.type` delivers
    // the line one keystroke at a time, and a single dropped character
    // inside the escape portion under CI load yields a printf that emits
    // no DSR query — while `echo PROBE-DONE`, a separate command after
    // the `;`, still runs, so the synchronization marker looked healthy.
    // A paste delivers the whole line as one input frame, so it either
    // arrives intact or not at all; PROBE-DONE then proves it ran.
    //
    // The marker is built with printf %s so the string "PROBE-DONE"
    // never appears in the COMMAND itself: the shell echoes the pasted
    // line immediately, and a literal marker in the echo would satisfy
    // the wait below before the command — including the 1-second gap
    // the queries need — had actually executed.
    const probeLine =
      "printf '\\e[6n\\e[?12$p'; sleep 1; printf 'PROBE-%s\\n' DONE";
    await page.evaluate(
      (line) => (window as any).__farhelmTerm.paste(line),
      probeLine,
    );
    await page.keyboard.press("Enter");
    await waitForTermText(page, "PROBE-DONE", 15_000);

    const recordedFrames = () =>
      page.evaluate(() =>
        ((window as any).__sentInput as unknown[]).map((f) =>
          Array.isArray(f)
            ? String.fromCharCode(...(f as number[]))
            : String(f),
        ),
      );

    // The control first, and polled rather than sampled once: a cursor-
    // position report DID reach the WebSocket, proving xterm's
    // auto-replies were genuinely live and flowing through this exact
    // recorded path — without it, the assertion below could pass
    // vacuously (e.g. if nothing were recorded at all). The recording
    // hook is synchronous and xterm parses the DSR long before the
    // marker prints, so the poll is defensive depth, not a required
    // wait — the marker output already sequences everything.
    await expect
      .poll(recordedFrames, { timeout: 5_000 })
      .toContainEqual(expect.stringMatching(/\x1b\[[0-9]+;[0-9]+R/));
    // The fix: no DECRPM reply shape ever reached the WebSocket as input.
    expect(await recordedFrames()).not.toContainEqual(
      expect.stringMatching(/\x1b\[\?[0-9;]*\$y/),
    );
  } finally {
    await cleanupSession(request, id);
  }
});

// MT-6 regression test: a select-and-copy in the terminal leaves TWO
// selections behind, and both stay painted over content the user is no
// longer selecting once input moves the buffer underneath them. xterm's
// own selection is anchored to buffer COORDINATES rather than to the
// text in them; the DOM renderer's real text nodes additionally carry a
// NATIVE document selection, which `Terminal.clearSelection()` does not
// touch. Manual testing on macOS found the second one: the highlight
// survived a paste, then survived typing, then survived a forced
// `refresh()` — because only the native selection was still there.
//
// The fix (terminal.js's `dismissSelection`) drops both on user-origin
// input: keyboard through `onKey`, paste through a capture-phase DOM
// listener (xterm's own paste handler sits on the hidden textarea and
// calls stopPropagation, so a bubble-phase listener never runs).
//
// The selection is made with a REAL MOUSE DRAG, not `selectAll()`: only
// a drag produces the native selection that carried this bug, so a
// programmatic selection would test the half that already worked. And
// the native side is asserted through `window.getSelection()` rather
// than `isCollapsed`, which WebKit reports as `true` for a drag-made
// selection whose ranges are still present and still painted.
//
// WHAT `window.getSelection()` ACTUALLY REPORTS HERE, because it is not
// the same thing on the machine that found the bug and the machine that
// runs this suite. xterm.js supports X11's PRIMARY selection by copying
// every mouse selection into its hidden helper textarea and calling
// `focus()` + `select()` on it (`onLinuxMouseSelection`, gated on
// `navigator.platform` containing "Linux" — true for BOTH Playwright
// engines on a Linux host, including WebKit, whose user agent claims
// macOS while its platform string does not). So under this suite the
// document selection a drag leaves behind is anchored in `.xterm-helpers`,
// not in the rendered rows: probing it directly, `getSelection()` tracks
// `textarea.selectionStart..selectionEnd` character for character, and
// reports zero while that textarea's selection is momentarily collapsed
// even though xterm's own selection is unchanged. On real macOS
// (`isLinux` false, no mirror) it is the row-anchored selection the MT-6
// bug was about. Both are cleared by the same `removeAllRanges()`, so
// this test pins the same contract on both — but only the macOS shape is
// ever painted over content.
//
// THE FLAKE, and why the key leg presses a key without releasing it:
// this test failed intermittently under CI load with `nativeChars` back
// at its pre-input value (150 characters on the CI viewport) after the
// dismissal. Reproduced locally at two to three failures per 50-60
// repetitions with a dozen spinning CPU hogs alongside, and traced by
// patching `Selection.prototype.removeAllRanges` and
// `HTMLTextAreaElement.prototype.focus`/`select` to log stacks. The
// blame lands on the KEY RELEASE, not on the dismissal: xterm's own
// `_keyUp` handler calls `Terminal.focus()`, and refocusing a text
// control makes the engine restore that control's cached selection —
// still the mirrored drag text, because `removeAllRanges()` cleared the
// live selection without invalidating that cache. About 30ms after the
// refocus, the selection reappears. Nothing is painted (the helper
// textarea is `opacity: 0`, parked at `left: -9999em`) and no user could
// see it, and terminal.js cannot prevent it either — the restore comes
// from xterm refocusing its own helper element. Whether this test noticed
// came down to whether its first poll sample landed inside the ~40ms
// window between the dismissal and the restore.
//
// Hence `keyboard.down("x")` with the matching `up` deferred all the way
// to `finally`, instead of `press`. That is not a weaker assertion — same
// real, trusted key event, and the keydown is where the input contract
// lives: xterm sends the character and fires `onKey` (and therefore
// `dismissSelection`) on the way DOWN, while the release carries no input
// at all. Deferring it only stops an unrelated xterm behavior from racing
// the state this test reads. It has to be deferred past the LAST
// assertion, not just its own: a restoration scheduled by releasing "x"
// (or by any intervening keypress, which is why the paste leg no longer
// erases the typed character first) lands tens of milliseconds later, by
// which point the paste leg's poll is the one it would corrupt.
//
// Both legs still poll rather than sample once, because the dismissal
// genuinely is eventually-consistent by design: terminal.js sweeps once
// synchronously and once on a `setTimeout(0)`, and traces show either
// sweep landing the actual `removeAllRanges()` depending on where the
// engine had the document selection at that instant.
test("input dismisses both the xterm and native selections", async ({
  page,
}) => {
  await openTerminal(page);
  await page.locator("#terminal").click();

  const selectionState = () =>
    page.evaluate(() => ({
      xterm: (window as any).__farhelmTerm.hasSelection(),
      nativeChars: String(window.getSelection() || "").length,
    }));

  // Drag across a few rows of the terminal's own output.
  const dragSelect = async () => {
    const box = (await page.locator("#terminal").boundingBox())!;
    await page.mouse.move(box.x + 30, box.y + 40);
    await page.mouse.down();
    await page.mouse.move(box.x + 300, box.y + 90, { steps: 10 });
    await page.mouse.up();
    await expect.poll(selectionState).toMatchObject({ xterm: true });
    expect((await selectionState()).nativeChars).toBeGreaterThan(0);
  };

  try {
    // First leg: a selection SURVIVES terminal-generated traffic. The
    // fake agent echoes the typed line back, and that inbound output —
    // like any auto-reply xterm generates in response to queries — flows
    // through paths that must NOT clear a selection: only user-origin
    // input may. A `clearSelection` that migrated into `onData` (where
    // those replies also flow) would fail here.
    await page.keyboard.type("probe");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:probe", 10_000);
    await dragSelect();
    await page.waitForTimeout(200);
    expect(await selectionState()).toMatchObject({ xterm: true });

    // Second leg: a keystroke dismisses both selections. The key is never
    // released inside the test body — see this test's own docs: a release
    // schedules the mirrored selection's restoration, and that restoration
    // would then be in flight across the paste leg below, racing ITS poll
    // the same way it raced this one. The only release is in `finally`,
    // after every assertion has been made.
    await page.keyboard.down("x");
    await expect.poll(selectionState).toEqual({ xterm: false, nativeChars: 0 });

    // Third leg: so does a paste. Dispatched as a synthetic
    // ClipboardEvent rather than driving the OS clipboard, whose
    // permissions differ per engine; the event is exactly what a real
    // ⌘V/Ctrl-V delivers to this same target. No Backspace first: erasing
    // the typed "x" would mean another press, another release, and another
    // restoration in flight — and the `Control+U` in `finally` clears the
    // whole line anyway, which is all this test owes the shared session.
    await dragSelect();
    await page.evaluate(() => {
      const data = new DataTransfer();
      data.setData("text/plain", "pasted");
      (window as any).__farhelmTerm.textarea.dispatchEvent(
        new ClipboardEvent("paste", {
          clipboardData: data,
          bubbles: true,
          cancelable: true,
        }),
      );
    });
    await expect.poll(selectionState).toEqual({ xterm: false, nativeChars: 0 });
  } finally {
    // Release the held "x" (harmless if an earlier failure meant it was
    // never pressed), and only here, so its restoration can no longer
    // overlap any assertion above.
    await page.keyboard.up("x");
    // The typed "x" and the pasted text both land at the prompt; clear the
    // whole line rather than counting characters, so the shared session's
    // prompt is left as this test found it.
    await page.keyboard.press("Control+U");
  }
});

// SPEC.md's core durability promise seen from the browser: close the tab,
// come back, and the session looks as if you had never left. A reload is
// the harshest form of it — a brand-new xterm.js with an empty buffer, so
// everything on screen afterwards came from replay.
test("reload reattaches with replayed scrollback", async ({ page }) => {
  await openTerminal(page);
  await page.locator("#terminal").click();
  await page.keyboard.type("before-reload");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:before-reload");

  await page.reload();
  // A reload resets the app's navigation state (App's `Signal<Option
  // <Session>>` starts at `None` on every fresh load), so it lands back
  // on the list view, not the terminal directly — the row must be
  // clicked again, same as openTerminal's own first attach.
  const row = sharedSessionRow(page);
  await expect(row).toBeVisible();
  await row.click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  // Replay must bring back output produced before this attachment
  // existed — the reconnect-with-replay acceptance criterion.
  await waitForTermText(page, "echo:before-reload");
  await waitForTermText(page, "FAKE-AGENT READY");
});

// Exercise the complete browser-to-PTY resize chain. Xterm's local
// dimensions are only the requested geometry; the fake agent's `stty`
// result proves the WebSocket message reached tmux before later input.
test("resize reaches the real terminal", async ({ page }) => {
  await openTerminal(page);
  const before = await page.evaluate(() => {
    const t = (window as any).__farhelmTerm;
    return { cols: t.cols, rows: t.rows };
  });
  await page.setViewportSize({ width: 700, height: 500 });
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const t = (window as any).__farhelmTerm;
          return { cols: t.cols, rows: t.rows };
        }),
      { message: "viewport change must reflow the terminal via fit()" },
    )
    .not.toEqual(before);
  const geometry = await page.evaluate(() => {
    const t = (window as any).__farhelmTerm;
    return { cols: t.cols, rows: t.rows };
  });
  await page.locator("#terminal").click();
  await page.keyboard.type("size");
  await page.keyboard.press("Enter");
  await waitForTermText(
    page,
    `size:${geometry.rows} ${geometry.cols}`,
    10_000,
  );
});

test("second client takes over; first shows the detach banner", async ({
  browser,
  page,
}) => {
  await openTerminal(page);

  const second = await browser.newContext();
  const page2 = await second.newPage();
  // A fresh context has its own list view, so it goes through the same
  // list-then-click path as `page` did in openTerminal(page) above —
  // there is no direct terminal URL to land on.
  await openTerminal(page2);

  // SPEC.md: last attach wins, and the loser sees it happened.
  await expect(page.locator("#term-banner")).toBeVisible({ timeout: 10_000 });
  await expect(page.locator("#term-banner")).toContainText("Detached");

  // The winner is live: input still round-trips.
  await page2.locator("#terminal").click();
  await page2.keyboard.type("takeover-works");
  await page2.keyboard.press("Enter");
  await waitForTermText(page2, "echo:takeover-works", 10_000);
  await second.close();
});

// PLAN_M2.md acceptance 4: a restart-gap session (tmux gone, metadata
// intact) must open to "metadata plus why there is no terminal", not a
// silently blank pane. Nothing before this test pinned the UI half of
// that criterion — the Rust suite covers only the supervisor side, in
// `restart_gap_lists_sessions_without_a_terminal_and_attach_fails`
// (crates/farhelm/tests/e2e.rs).
//
// This suite's stack cannot restart its supervisor mid-run
// (start-stack.sh boots one long-lived supervisor for the whole file), so
// a genuine restart gap is out of reach here. The stand-in: a session row
// the real supervisor has never heard of. That is a DIFFERENT failure
// branch on the supervisor side — an id absent from `sup.sessions`
// entirely takes the "no such session: {id}" arm of `ControlMsg::Attach`'s
// handler (service.rs), while a genuine restart-gap row (present in the
// map, `entry.terminal` empty) takes the sibling "session {id} has no
// terminal: the supervisor (or its tmux server) restarted after the agent
// ended" arm right below it — distinct branch, distinct wording, not
// reproduced here. What the two DO share, and what this test actually
// exercises, is everything downstream of that error: `serve_term` in
// farhelm-helm/src/lib.rs attaches over a REAL WebSocket, gets back
// whichever error, and relays it as a genuine `detached` control message
// the same way regardless of which arm produced it; the browser side
// (terminal.js's `showBanner`) and the list UI's metadata rendering have
// no way to tell the two apart either. So this pins the shared
// helm/WebSocket/UI error-display path — the UI CONTRACT of "metadata
// shown, plus a server-provided explanation instead of a silently blank
// terminal" — not the restart-gap-specific message, which belongs to the
// Rust test named above.
//
// Only the row's EXISTENCE is synthetic: route-intercepted GET
// /api/sessions, injecting one extra row alongside the real ones (rather
// than fabricating the whole response) so the shared "e2e-session" row
// every other test in this file depends on still comes from the real
// supervisor on THIS request. That protection is necessarily local to
// this one route handler and this one page, though: Playwright routes are
// page-scoped, so nothing here could leak into another test's page even
// if it wanted to. The banner text is asserted only to be non-empty and
// to name the session, whatever exact prose this particular arm's error
// happens to carry.
test("opening a terminal-less session shows its metadata and the server's own explanation", async ({
  page,
}) => {
  // A well-formed but unknown UUID: recognizable in the banner text
  // without colliding with any id a real session in this run could have.
  const bogusId = "00000000-0000-0000-0000-000000000000";
  const title = `terminal-less-${Date.now()}`;
  const cwd = "/tmp/terminal-less-fixture";
  const invocation = "true";

  // This test only ever issues GETs against this route (no create/stop/
  // delete call in its body), so there is no other method to fall through
  // to `route.continue()` for.
  await page.route("**/api/sessions", async (route) => {
    // Fetch the REAL listing and append one row, rather than fabricating
    // the whole response: every other row (in particular the shared
    // "e2e-session" other tests in this file depend on) must keep coming
    // from the real supervisor, unmodified.
    const response = await route.fetch();
    const listing = await response.json();
    listing.sessions.push({
      id: bogusId,
      title,
      cwd,
      invocation,
      // Exactly the shape a restart-gap row has (PLAN_M2.md, and
      // `SessionStatus::Exited` in farhelm-proto/src/lib.rs): known dead,
      // no code to fabricate.
      status: { state: "exited", exit_code: null },
    });
    listing.total += 1;
    await route.fulfill({ response, json: listing });
  });

  await page.goto("/");
  // `.click()` already waits for the target to be visible and stable, so
  // there is nothing an upfront `toBeVisible` would add here.
  await rowByTitle(page, title).locator(".session-row-open").click();

  // (a) metadata IS shown — title and titlebar `.meta` (cwd — invocation,
  // farhelm-ui/src/lib.rs) render from the row's own fields, independent
  // of whether a terminal ever comes up behind them. `toHaveText` retries
  // on its own, so this needs no separate mount-readiness wait first.
  await expect(page.locator(".titlebar .title")).toHaveText(title);
  await expect(page.locator(".titlebar .meta")).toHaveText(
    `${cwd} — ${invocation}`,
  );

  // (b) the banner becomes visible and carries the server's own reason
  // (farhelm-ui/assets/terminal.js's showBanner, fed by serve_term's
  // `detached` notice — "Detached: <reason>"), not a blank pane or a
  // generic "connection closed". This IS this test's real synchronization
  // point: the banner is asynchronous, arriving only after the WS
  // round-trips through the real attach failure, so waiting on it (rather
  // than on `__farhelmTermReady`, which flips as soon as mount() opens the
  // socket and says nothing about how the attach behind it resolves) is
  // what actually proves the failure was relayed all the way to the DOM.
  const banner = page.locator("#term-banner");
  await expect(banner).toBeVisible({ timeout: 10_000 });
  // The visibility assertion above guarantees the element exists, so
  // `textContent()` cannot be null here.
  const bannerText = await banner.textContent();
  expect(bannerText).toMatch(/^Detached: .+/);
  expect(bannerText).toContain(bogusId);

  // (c) no agent output ever reached the terminal: the attach failed
  // before any `TermEvent::Data` could exist to write into the buffer.
  expect((await termText(page)).trim()).toBe("");
});

// The creation API is the one true path (PLAN_M1.md: CLI flags AND the UI
// dialog (PLAN_M2.md step 8) both feed this same endpoint), so its HTTP
// surface needs its own direct coverage. Only the failure case is
// exercised here: a successful POST would leave an extra, untracked
// session sitting in the list for every test after this one, with nothing
// to clean it up.
// The status code is part of the contract, not an implementation detail:
// a missing cwd is the caller's own precondition failure (4xx), distinct
// from a server-side fault (5xx) the caller could not have avoided by
// sending a different request. The supervisor classifies this as
// InvalidRequest and farhelm-helm's http_error maps that to 400 — see
// ErrorKind in farhelm-proto.
//
// "Contains", not "is": the assertion below is `toContain`, not an exact
// match, because the body carries more than just the one sentence pinned
// here (an anyhow error chain — see farhelm-helm's `http_error` — can
// prefix or wrap it with additional context). The test's job is pinning
// that THIS text is present verbatim somewhere in the body, not pinning
// the whole body's exact shape.
test("create API reports a precondition failure containing the supervisor's own text", async ({
  request,
}) => {
  const resp = await request.post("/api/sessions", {
    data: { cwd: "/nonexistent/definitely/not/here", invocation: "true" },
  });
  expect(resp.status()).toBe(400);
  expect(await resp.text()).toContain("working directory does not exist");
});

// Request-level coverage for the stop/delete HTTP surface (PLAN_M2.md step
// 6): the full UI flows (stop/delete buttons, delete's confirmation
// dialog) are PLAN_M2.md step 8's PR, so this exercises the API
// directly against the real stack, following the request-fixture style of
// the create-API test above rather than driving a page. It creates its
// own session (a long-running `sleep`, distinct from the shared
// "e2e-session" every terminal test above depends on) so it can freely
// stop and delete it without perturbing the rest of the suite.
test("stop and delete a session through the HTTP API", async ({ request }) => {
  const totalBefore = (await (await request.get("/api/sessions")).json())
    .total;

  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  // Everything past creation is wrapped so a failed assertion here still
  // cleans up: this suite is one shared, serially-run stack (see the
  // config's fullyParallel/workers comment), so a leaked long-running
  // `sleep 300` session would keep sitting in the list for every test
  // after this one — and could cascade-fail any of them that assumes
  // something about which sessions exist. The `finally` delete tolerates
  // a 404 (and any other failure) because the happy path below already
  // deletes the session itself; the cleanup call is only load-bearing
  // when an assertion above threw first.
  try {
    const afterCreate = await (await request.get("/api/sessions")).json();
    expect(afterCreate.total).toBe(totalBefore + 1);

    const stopped = await request.post(`/api/sessions/${id}/stop`);
    expect(stopped.status()).toBe(200);
    expect(await stopped.json()).toEqual({});

    // tmux marks the pane dead asynchronously once the killed process is
    // reaped, so the next list poll (not the stop response itself) is what
    // proves the kill actually took effect.
    await expect
      .poll(
        async () => {
          const listing = await (await request.get("/api/sessions")).json();
          const session = listing.sessions.find((s: any) => s.id === id);
          return session?.status?.state;
        },
        { timeout: 10_000, message: "stopped session must show as exited" },
      )
      .toBe("exited");

    const deleted = await request.delete(`/api/sessions/${id}`);
    expect(deleted.status()).toBe(200);
    expect(await deleted.json()).toEqual({});

    await expect
      .poll(
        async () => {
          const listing = await (await request.get("/api/sessions")).json();
          return listing.sessions.some((s: any) => s.id === id);
        },
        { timeout: 10_000, message: "deleted session must disappear from the list" },
      )
      .toBe(false);

    const afterDelete = await (await request.get("/api/sessions")).json();
    expect(afterDelete.total).toBe(totalBefore);
  } finally {
    // Best-effort: swallow everything, including a 404 for the (expected)
    // case where the happy path already deleted the session. This must
    // never throw over the top of a real assertion failure above.
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// PLAN_M2.md step 8's UI acceptance flow, driven end to end through the
// create form and per-row buttons rather than the raw API (the test above
// already covers the API's own contract): two sessions created from the
// form run side by side, one is opened and typed into, the other is
// stopped and its badge flips live, and both are deleted — one WITHOUT a
// confirmation prompt (already exited) and one WITH (still alive),
// pinning the exact confirm/no-confirm split SPEC.md's "Lifecycle
// operations" draws between the two states.
//
// The confirmation itself is the inline per-row prompt (`.confirm-consequence`
// plus `.confirm-title` plus `.confirm-delete`/`.confirm-cancel`), not a
// native `window.confirm()` — see `SessionRow`'s doc in lib.rs for why: wry
// ships no dialogs at all on macOS's WKWebView, which made the old
// eval-based path silently do nothing on that target. `.confirm-consequence`'s
// absence after a delete click is what stands in for "no dialog fired"
// below; its presence (checked for text content) is what stands in for
// "dialog mentions the running agent".
test("multi-session flow: create two, open and type in one, stop and delete the other, then delete the first with confirmation", async ({
  page,
  request,
}) => {
  const titleA = `multi-a-${Date.now()}`;
  const titleB = `multi-b-${Date.now()}`;

  try {
    // Create session A through the dialog; success navigates straight
    // into its terminal (SPEC.md: "creation launches the agent; you type
    // your first prompt into its terminal").
    await page.goto("/");
    const formA = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title: titleA,
    });
    await formA.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".titlebar .title")).toHaveText(titleA);

    // Type into session A while it is open, per the flow's "open one,
    // type" step, then go back to the list to create session B.
    await page.locator("#terminal").click();
    await page.keyboard.type("marker-multi-a");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:marker-multi-a");
    await page.locator(".back-button").click();

    const formB = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title: titleB,
    });
    await formB.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".titlebar .title")).toHaveText(titleB);
    await page.locator(".back-button").click();

    // Both rows are back in the list, alive.
    await expect(rowByTitle(page, titleA).locator(".status-badge")).toHaveText(
      "alive",
      { timeout: 10_000 },
    );
    await expect(rowByTitle(page, titleB).locator(".status-badge")).toHaveText(
      "alive",
      { timeout: 10_000 },
    );

    // Stop session B via its row button. No confirmation for stop
    // (SPEC.md gives confirmation to delete/archive, not stop), and the
    // badge must flip on the next poll WITHOUT a reload.
    //
    // The badge still SAYS exited and adds the stop annotation as a
    // qualifier: SPEC.md is explicit that "stopped" is not a distinct
    // status, so the supervisor's durable annotation (PLAN_M3.md item 4)
    // qualifies the exited badge rather than replacing its text. This is
    // the browser-side proof of that whole path — the annotation is
    // written in the supervisor's store, travels the wire and the helm's
    // JSON, and lands in the DOM. Asserted on `.status-badge.exited` so
    // the CSS class rides with it: a stopped session must still LOOK like
    // an ended one.
    await rowByTitle(page, titleB)
      .locator(".session-row-stop")
      .click();
    await expect(
      rowByTitle(page, titleB).locator(".status-badge.exited"),
    ).toHaveText(/^exited — stopped by user/, { timeout: 10_000 });

    // Delete session B (now exited): no confirmation expected — pin that
    // the inline prompt never appears at all, not merely that it gets
    // auto-handled. Stalled via the same route-hold technique as the
    // dedicated "exited session deletes immediately" test: a bare
    // post-click absence check cannot tell "never appeared" from
    // "appeared and vanished before this check ran", and holding the
    // DELETE open is what closes that gap.
    const idB = await findSessionIdByTitle(request, titleB);
    let releaseDeleteB: () => void = () => {};
    const deleteBHeld = new Promise<void>((resolve) => {
      releaseDeleteB = resolve;
    });
    await page.route(`**/api/sessions/${idB}`, async (route) => {
      if (route.request().method() !== "DELETE") {
        await route.continue();
        return;
      }
      await deleteBHeld;
      await route.continue();
    });
    await rowByTitle(page, titleB)
      .locator(".session-row-delete")
      .click();
    await expect(rowByTitle(page, titleB)).toHaveCount(1);
    await expect(rowByTitle(page, titleB).locator(".confirm-consequence")).toHaveCount(
      0,
    );
    releaseDeleteB();
    await expect(rowByTitle(page, titleB)).toHaveCount(0, { timeout: 10_000 });

    // Delete session A (still alive): confirmation expected, wording
    // must say the agent is still running (SPEC.md: "confirmation that
    // says so when anything is still alive").
    await rowByTitle(page, titleA)
      .locator(".session-row-delete")
      .click();
    await expect(rowByTitle(page, titleA).locator(".confirm-consequence")).toContainText(
      "running",
    );
    await rowByTitle(page, titleA)
      .locator(".confirm-delete")
      .click();
    await expect(rowByTitle(page, titleA)).toHaveCount(0, { timeout: 10_000 });
  } finally {
    // Best-effort: both sessions should already be gone via the happy
    // path above, but a failed assertion partway through must not leak a
    // long-running fake-agent process into every later test.
    for (const title of [titleA, titleB]) {
      const id = await findSessionIdByTitle(request, title).catch(() => undefined);
      if (id) {
        await request.post(`/api/sessions/${id}/stop`).catch(() => {});
        await request.delete(`/api/sessions/${id}`).catch(() => {});
      }
    }
  }
});

// macOS autocorrect mangles create-form input both via suggestion popups
// and silent in-place substitution (observed directly: WKWebView silently
// capitalizing "claude" to "Claude" with no visible popup to catch and
// reject) — a corrupted command or path is not a cosmetic issue, since
// these fields hold literal text that gets executed, not prose. All three
// inputs opt out of every browser-level text-mangling feature; this test
// pins that the opt-out attributes actually made it into the rendered DOM
// (a Dioxus rsx typo or a dropped attribute would otherwise silently leave
// autocorrect back on) rather than exercising an OS-level autocorrect
// engine itself, which is not something Playwright's headless Chromium
// runs at all.
test("create form inputs opt out of autocomplete, autocorrect, autocapitalize, and spellcheck", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();

  const inputs = form.locator('input[type="text"]');
  await expect(inputs).toHaveCount(3);
  for (let i = 0; i < 3; i++) {
    const input = inputs.nth(i);
    await expect(input).toHaveAttribute("autocomplete", "off");
    await expect(input).toHaveAttribute("autocorrect", "off");
    await expect(input).toHaveAttribute("autocapitalize", "none");
    await expect(input).toHaveAttribute("spellcheck", "false");
  }
});

// SPEC.md's precondition-failure split for creation: a bad working
// directory must fail the create with the supervisor's OWN error text,
// leave the form open with its values intact (so the user can fix the one
// wrong field rather than retyping everything), and must not leave a
// session behind. The exact "does not exist" wording is the same text
// pinned at the HTTP level by `create API reports a precondition failure
// containing the supervisor's own text` above; this test is the UI's
// obligation to actually SHOW that text rather than swallowing it. It
// goes one step further than "preserved": it actually fixes the one wrong
// field and resubmits, proving the form is genuinely usable again
// afterward — not merely that its stale values are still visible — which
// is also the only thing in this file that pins `submitting` resetting to
// `false` on the failure path.
test("create dialog surfaces a precondition failure, preserves the form, and creates no session", async ({
  page,
  request,
}) => {
  const title = `create-failure-${Date.now()}`;
  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/nonexistent/definitely/not/here",
      invocation: "true",
      title,
    });
    await form.locator('button[type="submit"]').click();

    await expect(form.locator(".create-session-error")).toContainText(
      "does not exist",
    );
    // Preserved, not cleared or reset: the same values the user typed.
    await expect(form.locator('input[type="text"]').nth(0)).toHaveValue(
      "/nonexistent/definitely/not/here",
    );
    await expect(form.locator('input[type="text"]').nth(1)).toHaveValue("true");
    await expect(form.locator('input[type="text"]').nth(2)).toHaveValue(title);
    // The form itself stayed open (a failed create must not silently
    // close it and strand the user with no visible cause).
    await expect(form).toBeVisible();

    const listing = await (await request.get("/api/sessions")).json();
    expect(listing.sessions.some((s: any) => s.title === title)).toBe(false);

    // The other half of "preserved, not stuck": fixing the one wrong
    // field and resubmitting must actually succeed, which pins that
    // `submitting` was reset to `false` on the failure path (the
    // double-submission guard in `CreateSessionForm`'s `onsubmit` would
    // otherwise leave the control permanently disabled after its first,
    // failed attempt).
    await form.locator('input[type="text"]').nth(0).fill("/tmp");
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await expect(page.locator(".titlebar .title")).toHaveText(title);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Double-submission guard (SPEC.md: "one intended create yields one
// session or a clear error, never two silently"): the submit control must
// be disabled for the WHOLE round trip, not just synchronously after the
// click handler returns. A normal create is too fast to observe that
// window reliably, so this delays the POST response by a fixed, short
// amount via route interception — long enough to deterministically
// observe the disabled state, short enough to keep the test fast. Only
// POST is intercepted (GET keeps flowing straight through) so the list's
// own background polling is unaffected. Also covers the two OTHER controls
// this same in-flight `submitting` flag locks: the "new session" toggle
// (which would otherwise unmount the form mid-POST) and every row's open
// button (which would otherwise unmount `ListView` itself mid-POST) — see
// `nav_locked`'s docs in lib.rs for why opening ANY row is unsafe here,
// not just a hypothetically "related" one.
test("create dialog disables the submit control while a create is in flight", async ({
  page,
  request,
}) => {
  const title = `double-submit-${Date.now()}`;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 800));
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    const submit = form.locator('button[type="submit"]');
    await submit.click();

    // The delayed POST is still in flight here — this is exactly the
    // window a double-click or a timeout-triggered retry would otherwise
    // land a second request into.
    await expect(submit).toBeDisabled();
    // The "new session" toggle is ALSO disabled for the same window: it
    // is this form's only cancel/close affordance, and toggling
    // `show_create` off while the create is in flight would unmount
    // `CreateSessionForm` mid-`spawn`, stranding the POST's eventual
    // response with nothing left to act on it (see the toggle button's
    // own doc in lib.rs).
    await expect(page.locator(".new-session-button")).toBeDisabled();
    // And the row-open guard from the same design: opening the shared
    // session right now would navigate away and unmount `ListView`
    // itself, cancelling this in-flight create exactly the same way —
    // see `nav_locked` in lib.rs.
    await expect(
      sharedSessionRow(page).locator(".session-row-open"),
    ).toBeDisabled();

    // Let the delayed response land: success navigates into the new
    // session's terminal, same as the multi-session flow above.
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
      timeout: 15_000,
    });
    await expect(page.locator(".titlebar .title")).toHaveText(title);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Clicking delete on an Alive session (SPEC.md's "confirmation that says
// so when anything is still alive") must open the inline confirm prompt
// — `.confirm-consequence` plus `.confirm-title` plus
// `.confirm-delete`/`.confirm-cancel` swapped in for the row's normal
// stop/delete buttons (`SessionRow`'s doc in lib.rs) — rather than calling
// any API immediately. `window.confirm()` used to be the mechanism here;
// it is gone because wry ships no native JS dialogs at all on macOS's
// WKWebView, which made that path silently do nothing on a primary
// target.
test("alive delete opens an inline confirming state with the is-still-running wording and the session title", async ({
  page,
  request,
}) => {
  const title = `confirm-open-${Date.now()}`;
  let deleteRequests = 0;
  await page.route("**/api/sessions/*", async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await page.locator(".back-button").click();

    const row = rowByTitle(page, title);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await row.locator(".session-row-delete").click();

    // The prompt carries the exact consequence wording (the untruncatable
    // half, `SessionRow`'s doc in lib.rs) AND, separately, the session's
    // own title (rendered as plain Dioxus text — see that doc for why
    // that alone neutralizes anything the title might contain).
    await expect(row.locator(".confirm-consequence")).toHaveText(
      "still running — deleting kills the agent:",
    );
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);
    // Normal buttons are gone while confirming, not merely hidden behind
    // the prompt — `SessionRow` swaps them out entirely.
    await expect(row.locator(".session-row-stop")).toHaveCount(0);
    await expect(row.locator(".session-row-delete")).toHaveCount(0);
    // The open button stays present (cancel is the only way back to
    // normal, not an implicit click on open — see `SessionRow`'s doc) but
    // both disabled AND hidden. MT-8 regression: it used to stay visible
    // and merely disabled, and its title/cwd/invocation content — each
    // with its own non-shrinking `min-width` floor — overflowed the
    // narrower box the confirm prompt's own elements left it and painted
    // over that prompt instead of being replaced by it. `toBeHidden`
    // pins the fix deterministically without needing pixel inspection:
    // app.css's `.session-row-open.confirming` rule is what makes this
    // true.
    await expect(row.locator(".session-row-open")).toBeDisabled();
    await expect(row.locator(".session-row-open")).toBeHidden();
    expect(deleteRequests).toBe(0);

    // Cancel: the row returns to normal, with no DELETE ever sent and the
    // session still listed and alive — not just "not yet deleted" (which
    // a bug that deleted on a timer, or deleted regardless of the
    // confirmation's answer after some delay, could also satisfy).
    await row.locator(".confirm-cancel").click();
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);
    await expect(row.locator(".session-row-stop")).toBeEnabled();
    await expect(row.locator(".session-row-delete")).toBeEnabled();
    await expect(row.locator(".session-row-open")).toBeEnabled();
    expect(deleteRequests).toBe(0);
    await expect(row.locator(".status-badge")).toHaveText("alive");
    const listing = await (await request.get("/api/sessions")).json();
    const session = listing.sessions.find((s: any) => s.title === title);
    expect(session).toBeTruthy();
    expect(session.status.state).toBe("alive");
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// The other half of the confirm flow: clicking "confirm delete" performs
// exactly the DELETE the old accepted `window.confirm()` used to trigger —
// pinned here as EXACTLY one DELETE request, the same request-counting
// pattern the cancel test above uses to pin exactly zero.
test("confirming an inline delete prompt deletes the session with exactly one DELETE request", async ({
  page,
  request,
}) => {
  const title = `confirm-delete-${Date.now()}`;
  let deleteRequests = 0;
  await page.route("**/api/sessions/*", async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await page.locator(".back-button").click();

    const row = rowByTitle(page, title);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-consequence")).toBeVisible();
    await row.locator(".confirm-delete").click();

    await expect(row).toHaveCount(0, { timeout: 10_000 });
    expect(deleteRequests).toBe(1);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Exited sessions are the one status that never confirms at all (SPEC.md:
// delete confirms only when something might still be alive) — this pins
// that directly, rather than relying on it as a side effect of the
// multi-session flow test above.
test("exited session deletes immediately with no confirming state", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "true" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  // Stalled, not answered instantly: a bare post-click check for the
  // prompt's absence cannot distinguish "never appeared" from "appeared
  // and vanished again before this check ran" — a real gap for a status
  // this fast to slip through undetected. Holding the DELETE response
  // open keeps the row on screen long enough to make the absence
  // assertion actually mean something, then releases it to let the
  // delete complete normally.
  let releaseDelete: () => void = () => {};
  const deleteHeld = new Promise<void>((resolve) => {
    releaseDelete = resolve;
  });
  await page.route(`**/api/sessions/${id}`, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.continue();
      return;
    }
    await deleteHeld;
    await route.continue();
  });

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText(/^exited/, {
      timeout: 10_000,
    });

    await row.locator(".session-row-delete").click();
    // The DELETE is stalled, so the row is still here — and, while it
    // is, the confirm prompt has never appeared at all, a synchronous
    // property of `on_delete`'s Exited arm (see lib.rs), not merely a
    // narrow timing window this stall makes easier to hit by luck.
    await expect(row).toHaveCount(1);
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);

    releaseDelete();
    await expect(row).toHaveCount(0, { timeout: 10_000 });
  } finally {
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// A session's title is untrusted data (a supervisor over `--ssh` is a
// different, possibly compromised host) that can legally contain anything,
// including markup — and the confirm prompt is rendered by ordinary Dioxus
// text interpolation straight to a DOM text node (`SessionRow`'s doc in
// lib.rs), which is what actually neutralizes it. The OLD eval-based path
// needed `serde_json::to_string`-encoding to stop a title from breaking
// out of a JS *string literal*; that whole concern is gone along with the
// eval call. The risk that remains is a DIFFERENT regression class:
// something along this render path someday using
// `innerHTML`/`dangerouslySetInnerHTML`-style markup injection instead of
// a text node, which would parse a title as HTML rather than display it
// as text.
//
// Two INDEPENDENT oracles cover that risk, deliberately, rather than
// relying on either alone: the exact `toHaveText` checks below would
// already catch MOST such a regression — a title parsed as markup would
// render a broken `<img>` icon, not the literal `<img src=x
// onerror="...">` text this asserts verbatim — but `toHaveText` only
// proves the WRONG output didn't happen, not that nothing executed;
// asserting `__pwned` stays unset is a genuinely separate signal (was
// anything ever RUN), immune to a hypothetical bug where broken markup
// happened to still leave matching text behind. Together they cover both
// "did the display come out right" and "did anything execute", neither
// implied by the other.
test("delete confirmation safely displays a title containing executable HTML without ever parsing it as markup", async ({
  page,
  request,
}) => {
  const title = `inject-${Date.now()}-<img src=x onerror="window.__pwned=1">`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await row.locator(".session-row-delete").click();

    await expect(row.locator(".confirm-consequence")).toHaveText(
      "still running — deleting kills the agent:",
    );
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);
    // The title's own row (open button) renders the same untrusted string
    // too — checked here as well, since it is a second, independent
    // render site for the exact same data.
    await expect(row.locator(".session-title")).toHaveText(title);
    expect(await page.evaluate(() => (window as any).__pwned)).toBeUndefined();
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// A session's title has no length limit the UI enforces (a legal title
// can run tens of KB — see the existing overflow comment on
// `.session-title` in app.css). Unlike the SPACE-CONTAINING metadata that
// overflow handling elsewhere in this file exercises, a title with NO
// whitespace at all cannot wrap or break naturally — CSS's default
// min-content floor would let such a title claim its own full rendered
// width and push everything after it off the visible row, which is
// exactly what `.confirm-title`'s `min-width: 0` (app.css) exists to
// prevent. `.confirm-consequence`, in contrast, must NEVER be the one
// that gives: it is the safety-critical "will be killed" half, rendered
// as its own untruncatable element specifically so a long title can never
// clip it (`confirm_consequence`'s doc in lib.rs).
//
// This pins the actual CONTRACT, not just an emergent side effect: the
// consequence text renders in full (exact match, not `toContainText`),
// the title element is genuinely being clipped (not merely short enough
// to fit), both buttons stay on screen and don't overlap the title, and
// both buttons keep their own declared `flex-shrink: 0` — checked via
// computed style directly, since that is the one assertion that fails
// immediately and deterministically if a future edit ever drops that
// declaration, independent of whatever the emergent flex arithmetic at
// this particular viewport width happens to produce.
//
// Created via the raw API (a create-FORM round trip through this much
// text would only slow the test down, not exercise anything the API path
// doesn't already), then asserted in the browser's actual layout engine,
// not just in the CSS source.
test("a legal multi-KB, unbroken title keeps the consequence text intact and clips only the title, without disturbing the confirm/cancel buttons", async ({
  page,
  request,
}) => {
  const hugeTitle = "x".repeat(20_000);
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title: hugeTitle },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await row.locator(".session-row-delete").click();

    // The safety-critical consequence half renders in full, exact text —
    // not merely "contains", so any accidental truncation (an ellipsis
    // rule wrongly applied to THIS element instead of just the title)
    // fails immediately.
    await expect(row.locator(".confirm-consequence")).toHaveText(
      "still running — deleting kills the agent:",
    );

    // The title itself IS being clipped — proving the overflow/min-width
    // CSS is actually doing its job on a title this large, not merely
    // absent because a shorter title happened to fit anyway.
    const titleOverflowing = await row
      .locator(".confirm-title")
      .evaluate((el) => el.scrollWidth > el.clientWidth);
    expect(titleOverflowing).toBe(true);

    // Both buttons stay on screen, reachable...
    await expect(row.locator(".confirm-delete")).toBeInViewport();
    await expect(row.locator(".confirm-cancel")).toBeInViewport();
    // ...and not overlapping the (massively wide, if unclipped) title —
    // a real geometry check, not just individual visibility.
    const [titleBox, confirmBox, cancelBox] = await Promise.all([
      row.locator(".confirm-title").boundingBox(),
      row.locator(".confirm-delete").boundingBox(),
      row.locator(".confirm-cancel").boundingBox(),
    ]);
    expect(titleBox).not.toBeNull();
    expect(confirmBox).not.toBeNull();
    expect(cancelBox).not.toBeNull();
    expect(titleBox!.x + titleBox!.width).toBeLessThanOrEqual(confirmBox!.x + 1);
    expect(confirmBox!.x + confirmBox!.width).toBeLessThanOrEqual(cancelBox!.x + 1);

    // The direct CSS contract: both buttons keep their natural
    // (un-shrunk) box — `flex-shrink: 0` is what a reviewer removing
    // either declaration would see fail here immediately, rather than
    // this test depending on emergent flex-overflow arithmetic to notice.
    const [confirmShrink, cancelShrink] = await Promise.all([
      row
        .locator(".confirm-delete")
        .evaluate((el) => getComputedStyle(el).flexShrink),
      row
        .locator(".confirm-cancel")
        .evaluate((el) => getComputedStyle(el).flexShrink),
    ]);
    expect(confirmShrink).toBe("0");
    expect(cancelShrink).toBe("0");
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// The confirming state lives in `ListView`'s own client-side signal, keyed
// by session id (see `confirming`'s doc in lib.rs) — a poll refresh (the
// list's ONLY live-update mechanism in M2) refetches and re-renders the
// whole listing on its own timer, independent of anything the user is
// doing, and must not silently revert an in-progress confirmation out
// from under them.
//
// A distinguishable field on a LATER poll response — not merely a
// counted request — is what actually proves a real refetch's RESULT
// reached the DOM: counting requests alone cannot rule out a regression
// that fires the request but never applies its response (a dropped
// `listing.set`, a silently-ignored decode failure), which would still
// increment a request counter while never actually re-rendering anything.
// Route-intercepting the GET with a synthetic listing carrying a marker
// invocation is what turns "a poll happened" into "a poll's response was
// applied and rendered" — but the marker is only armed AFTER the confirm
// prompt is already open, not from page load onward: arming it up front
// would let the marker show up as a leftover of the FIRST fetch (the one
// that populates the initial list, before any click), which would pass
// this test even if no poll ever landed again while confirming — exactly
// the false positive this ordering exists to rule out.
test("an inline confirming state survives a poll refresh; cancel still works afterward", async ({
  page,
  request,
}) => {
  const title = `confirm-survives-poll-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();
  const marker = "poll-marker-invocation";

  // Baseline listing (the session's real invocation) until `markerArmed`
  // flips — see the comment above for why arming has to wait.
  let markerArmed = false;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sessions: [
          {
            id,
            title,
            cwd: "/tmp",
            invocation: markerArmed ? marker : "sleep 300",
            status: { state: "alive" },
          },
        ],
        total: 1,
        truncated: false,
      }),
    });
  });

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);

    // Only NOW does the route start serving the marker — strictly after
    // the prompt is already open, so the marker appearing can only be
    // the result of a poll that happened WHILE confirming, not the
    // initial page-load fetch.
    markerArmed = true;

    // The marker invocation only ever appears once THIS route's synthetic
    // response has actually been fetched, decoded, and rendered — proof
    // the refresh's result reached the DOM, not just that a request fired.
    await expect(row.locator(".session-invocation")).toHaveText(marker, {
      timeout: 10_000,
    });

    // Still confirming, still the same wording and title — a refresh must
    // not have cleared it (nor silently deleted anything: no DELETE was
    // ever confirmed).
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);
    await expect(row.locator(".status-badge")).toHaveText("alive");

    await row.locator(".confirm-cancel").click();
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);
    await expect(row.locator(".session-row-delete")).toBeEnabled();
  } finally {
    await page.unroute("**/api/sessions");
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// Confirming is per-row state, not global (see `confirming`'s doc in
// lib.rs): a delete click on one row must never bleed into another row's
// buttons, the same "per-session, not one shared slot" property `errors`
// and `pending` already have their own dedicated tests for above.
test("one row's confirming state does not affect another row's controls", async ({
  page,
  request,
}) => {
  const createdA = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  const createdB = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(createdA.status()).toBe(200);
  expect(createdB.status()).toBe(200);
  const { id: idA } = await createdA.json();
  const { id: idB } = await createdB.json();

  try {
    await page.goto("/");
    const rowA = page.locator(`[data-session-id="${idA}"]`);
    const rowB = page.locator(`[data-session-id="${idB}"]`);
    await expect(rowA.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await expect(rowB.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });

    await rowA.locator(".session-row-delete").click();
    await expect(rowA.locator(".confirm-consequence")).toBeVisible();

    // B is completely untouched: still its normal stop/delete pair, both
    // enabled, and no confirm prompt of its own.
    await expect(rowB.locator(".confirm-consequence")).toHaveCount(0);
    await expect(rowB.locator(".session-row-stop")).toBeEnabled();
    await expect(rowB.locator(".session-row-delete")).toBeEnabled();
    await expect(rowB.locator(".session-row-open")).toBeEnabled();

    await rowA.locator(".confirm-cancel").click();
  } finally {
    for (const id of [idA, idB]) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Confirming is `ListView`'s own state, decoupled from the status that
// triggered it (see `confirming`'s doc in lib.rs): a status change under
// an open confirm prompt — this session getting stopped from another
// client, say — must not silently close the prompt or swap back to the
// normal stop/delete pair. `confirm_consequence`'s wording is,
// deliberately, NOT frozen at the moment the prompt opened: it recomputes
// from whatever status the row's LATEST render carries (see that
// function's own doc), and its `Exited` arm exists specifically for this
// transition — a residual case, not dead code, so this pins its exact
// fallback wording rather than leaving it unexercised by anything in this
// suite. The title element is unaffected by any of this (the status
// change touches only the consequence text), so it is checked once,
// before the transition, rather than redundantly re-checked after.
test("an alive-to-exited status change under an open confirm prompt keeps confirming, with the fallback wording", async ({
  page,
  request,
}) => {
  const title = `alive-to-exited-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-consequence")).toContainText("running");
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);

    // Stopped from "elsewhere" (the raw API, standing in for another
    // client) while this row's prompt sits open.
    await request.post(`/api/sessions/${id}/stop`);

    // The next poll picks the exited status up and re-words the SAME
    // open prompt — it does not close it, and does not swap back to the
    // normal stop/delete pair.
    await expect(row.locator(".confirm-consequence")).toHaveText(
      "delete anyway:",
      { timeout: 10_000 },
    );
    // Cancel's continued presence is the interesting half here — proving
    // the row is still genuinely IN the confirming state, not merely that
    // SOME element with that text exists; confirm-delete is about to be
    // clicked below, so Playwright's own actionability wait already
    // covers its visibility.
    await expect(row.locator(".confirm-cancel")).toBeVisible();
    await expect(row.locator(".session-row-stop")).toHaveCount(0);

    await row.locator(".confirm-delete").click();
    await expect(row).toHaveCount(0, { timeout: 10_000 });
  } finally {
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// The poll loop's own error path (`fetch_sessions` failing) swaps the
// WHOLE list view for an error banner (`ListView`'s `Some(Err(e))` render
// arm) rather than leaving stale rows on screen — which means a row's
// `confirming` entry has nothing left to render into for as long as that
// banner is showing. This pins that the entry itself, held in `ListView`'s
// own state independent of any particular render, survives that gap
// intact and reappears the moment the list recovers — a bare "the request
// count went up" would not prove this, since it says nothing about
// whether the confirm prompt for THIS id came back correctly afterward.
test("a failed poll fetch while confirming does not clear the confirming state", async ({
  page,
  request,
}) => {
  const title = `poll-error-while-confirming-${Date.now()}`;
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300", title },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  // A one-shot failure, armed only once the confirm prompt is genuinely
  // open (below) — not from the start: arming up front races the very
  // FIRST poll after page load (which can fire before the delete click
  // even lands), which would fail a fetch that has nothing to do with
  // confirming at all and could flake this test on nothing but timing.
  let failArmed = false;
  let failed = false;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    if (failArmed && !failed) {
      failed = true;
      await route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: "injected poll failure",
      });
      return;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`);

    // Only NOW does the next poll-driven GET fail — strictly after the
    // prompt is confirmed open.
    failArmed = true;

    // The failed fetch swaps the list view for an error banner — this IS
    // that transient state, not a bug this test is tripping over.
    await expect(page.locator(".status.error")).toBeVisible({
      timeout: 10_000,
    });

    // The next poll succeeds; the list — and the SAME confirming prompt,
    // restored from `ListView`'s own state rather than anything baked
    // into this particular render — comes back.
    await expect(row.locator(".confirm-title")).toHaveText(`"${title}"`, {
      timeout: 10_000,
    });
    await expect(row.locator(".status-badge")).toHaveText("alive");
  } finally {
    await page.unroute("**/api/sessions");
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// The other confirm-wording branch: a session whose status is Unknown
// (rather than a known-alive one) must ALSO confirm before deleting, but
// with wording that admits uncertainty rather than borrowing the Alive
// branch's "is still running" claim — SPEC.md's no-guessing rule applies
// to this confirm text exactly as it does to the status badge itself.
// Driven through a synthetic, route-intercepted listing (like the
// truncation-banner test above) rather than a real session: a supervisor
// restart is NOT how to provoke this — PLAN_M2.md's restart-gap behavior
// yields `Exited { exit_code: None }` when tmux did not survive (an
// explicit "known dead, unknown code", not "unknown whether alive" — see
// `SessionStatus::Exited` in lib.rs), and ordinary `Alive`/`Exited` when
// it did. Genuine `Unknown` only ever comes from `Session::status`'s
// serde default kicking in on an old-shaped reply with no `status` field
// at all (see that derive's own docs) — i.e. an old PEER, not a restart of
// this same build's own supervisor — which is not something this suite's
// single, current-build stack can produce, hence the synthetic listing.
test("deleting a session with unknown status confirms first, with wording that admits uncertainty", async ({
  page,
}) => {
  const sessionId = "unknown-status-session";
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sessions: [
          {
            id: sessionId,
            title: sessionId,
            cwd: "/tmp",
            invocation: "true",
            // No "status" field at all — exactly what decodes as Unknown
            // per `Session::status`'s own serde default in lib.rs.
          },
        ],
        total: 1,
        truncated: false,
      }),
    });
  });
  let deleteRequests = 0;
  await page.route(`**/api/sessions/${sessionId}`, async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
  });

  await page.goto("/");
  const row = page.locator(`[data-session-id="${sessionId}"]`);
  await expect(row.locator(".status-badge")).toHaveText("unknown");
  await row.locator(".session-row-delete").click();

  // Confirms before any DELETE: since there is no async eval in the way
  // anymore, the assertion right after the click already proves ordering
  // (the click handler's whole synchronous body only ever inserts into
  // `confirming` for this status — see `on_delete` in lib.rs — so a
  // DELETE this soon could only come from a regression that skipped
  // confirmation outright).
  await expect(row.locator(".confirm-consequence")).toHaveText(
    "status unknown — the agent may still be running and will be killed:",
  );
  await expect(row.locator(".confirm-title")).toHaveText(`"${sessionId}"`);
  expect(deleteRequests).toBe(0);

  await row.locator(".confirm-delete").click();
  await expect.poll(() => deleteRequests).toBe(1);
});

// Double-submission guard, taken one step further than the disabled-button
// test above: that test only proves the CONTROL looks disabled, which a
// user could still defeat with a second Enter keypress landing on the form
// itself rather than the button, or any other path that dispatches a
// native `submit` event without going through the (disabled) button.
// `HTMLFormElement.requestSubmit()` is exactly such a path — it fires a
// real `submit` event the disabled button cannot intercept — so a second
// call here is what actually pins the RUST-SIDE `submitting` guard, not
// merely the disabled attribute's cosmetic effect.
test("submitting the create form twice while one create is in flight produces exactly one session", async ({
  page,
  request,
}) => {
  const title = `double-submit-guard-${Date.now()}`;
  let postCount = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    postCount++;
    await new Promise((resolve) => setTimeout(resolve, 500));
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    // Bypasses the (already disabling) submit button entirely.
    await page.evaluate(() => {
      document
        .querySelector<HTMLFormElement>(".create-session-form")
        ?.requestSubmit();
    });

    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
      timeout: 15_000,
    });
    expect(postCount).toBe(1);

    const listing = await (await request.get("/api/sessions")).json();
    expect(listing.sessions.filter((s: any) => s.title === title)).toHaveLength(1);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Per-session error surfacing: a stop or delete failure must render in
// THAT row's own error line with the server's actual text, without
// disturbing the row itself (no vanishing, no badge lying) or the rest of
// the list. Both failures here happen on the SAME session, one after the
// other — proving errors are keyed by session at all (a failure on one
// session must not touch another's error line) is a separate concern,
// covered by "a failed action's error is keyed to its own session, not
// shared across rows" below. Route-intercepted with distinct sentinel
// bodies for stop and delete so each assertion can tell exactly which
// call produced which text.
test("stop and delete failures surface in the row's own error line, without disturbing the rest of the list", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });

    await page.route(`**/api/sessions/${id}/stop`, (route) =>
      route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: "stop-failure-sentinel",
      }),
    );
    await row.locator(".session-row-stop").click();
    await expect(row.locator(".action-error")).toContainText(
      "stop-failure-sentinel",
    );
    // Scoped to this row and this action: no optimistic flip either way,
    // and the rest of the list keeps working normally.
    await expect(row.locator(".status-badge")).toHaveText("alive");
    await expect(page.locator(".session-list")).toBeVisible();
    await page.unroute(`**/api/sessions/${id}/stop`);

    await page.route(`**/api/sessions/${id}`, async (route) => {
      if (route.request().method() !== "DELETE") {
        await route.continue();
        return;
      }
      await route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: "delete-failure-sentinel",
      });
    });
    // Still alive, so delete opens the inline confirm prompt first —
    // click through it the same way a real user would.
    await row.locator(".session-row-delete").click();
    await row.locator(".confirm-delete").click();
    await expect(row.locator(".action-error")).toContainText(
      "delete-failure-sentinel",
    );
    // A failed delete must not vanish the row.
    await expect(row).toHaveCount(1);
    await page.unroute(`**/api/sessions/${id}`);
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// The other half of "per-session, not one shared slot" (see `errors`'s
// own docs in lib.rs): a failure on session A must not just render in A's
// own row (already covered above) but must ALSO survive an unrelated
// SUCCESS on session B untouched, and B must pick up no error of its own
// from any of it. A single shared `Option<String>` would have failed this
// in either direction — B's success clearing A's error, or A's failure
// somehow bleeding into B's row. Finishes by retrying A (now
// unintercepted) to confirm a later SUCCESS clears only A's own entry.
test("a failed action's error is keyed to its own session, not shared across rows", async ({
  page,
  request,
}) => {
  const createdA = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  const createdB = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(createdA.status()).toBe(200);
  expect(createdB.status()).toBe(200);
  const { id: idA } = await createdA.json();
  const { id: idB } = await createdB.json();

  try {
    await page.goto("/");
    const rowA = page.locator(`[data-session-id="${idA}"]`);
    const rowB = page.locator(`[data-session-id="${idB}"]`);
    await expect(rowA.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await expect(rowB.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });

    // A's stop fails (route-intercepted); B's stop is real and succeeds.
    await page.route(`**/api/sessions/${idA}/stop`, (route) =>
      route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: "error-a-sentinel",
      }),
    );
    await rowA.locator(".session-row-stop").click();
    await expect(rowA.locator(".action-error")).toContainText("error-a-sentinel");

    await rowB.locator(".session-row-stop").click();
    // "exited — stopped by user": the durable stop annotation qualifies
    // the exited badge (PLAN_M3.md item 4, SPEC.md's "'stopped' is not a
    // distinct status").
    await expect(rowB.locator(".status-badge")).toHaveText(
      /^exited — stopped by user/,
      { timeout: 10_000 },
    );

    // A's error survives B's unrelated success untouched, and B picked up
    // no error of its own from any of this.
    await expect(rowA.locator(".action-error")).toContainText("error-a-sentinel");
    await expect(rowB.locator(".action-error")).toHaveCount(0);

    // Retrying A (now unintercepted) must succeed and clear ONLY A's
    // error — B's (already-empty) state is untouched by this too.
    await page.unroute(`**/api/sessions/${idA}/stop`);
    await rowA.locator(".session-row-stop").click();
    await expect(rowA.locator(".status-badge")).toHaveText(
      /^exited — stopped by user/,
      { timeout: 10_000 },
    );
    await expect(rowA.locator(".action-error")).toHaveCount(0);
    await expect(rowB.locator(".action-error")).toHaveCount(0);
  } finally {
    for (const id of [idA, idB]) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// The per-session in-flight guard (`pending` in lib.rs's `ListView`, and
// the GLOBAL `nav_locked` derived from it — see that flag's own docs):
// while session A has a stop or delete running, A's own stop, delete, AND
// open buttons must all be disabled (open via the global nav lock, since
// opening ANY row would unmount `ListView` and cancel A's in-flight op
// just the same), while an unrelated session B's stop and delete stay
// perfectly usable (that half of the guard IS per-session) — and B's OWN
// open button is disabled too, which is the interesting, easy-to-miss
// half of this: the nav lock does not care WHICH session is busy.
test("stop's in-flight guard disables this row's stop, delete, and open, while another row's stop and delete stay usable", async ({
  page,
  request,
}) => {
  const createdA = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  const createdB = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(createdA.status()).toBe(200);
  expect(createdB.status()).toBe(200);
  const { id: idA } = await createdA.json();
  const { id: idB } = await createdB.json();

  // Delayed, then let through with `route.continue()` — NOT fulfilled
  // here: this route needs the REAL stop to actually reach the
  // supervisor and kill session A's real `sleep 300`, or its badge would
  // never flip to exited below and the test could never distinguish "the
  // guard is working" from "the request never even landed".
  let stopRequests = 0;
  await page.route(`**/api/sessions/${idA}/stop`, async (route) => {
    stopRequests++;
    await new Promise((resolve) => setTimeout(resolve, 800));
    await route.continue();
  });

  try {
    await page.goto("/");
    const rowA = page.locator(`[data-session-id="${idA}"]`);
    const rowB = page.locator(`[data-session-id="${idB}"]`);
    await expect(rowA.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await expect(rowB.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });

    // Two native clicks dispatched synchronously in the same JS tick:
    // Playwright's own `.click()` waits for an element to be enabled
    // before clicking, which would never let a second click land on an
    // already-disabled button, so it could only ever exercise the
    // `disabled` ATTRIBUTE, not the guard behind it. A bare DOM
    // `.click()` bypasses that actionability wait entirely and is what
    // actually exercises the RUST-SIDE `pending` re-entry guard (the
    // `if !pending.write().insert(...)` check in `on_stop`).
    await page.evaluate((id) => {
      const btn = document.querySelector<HTMLButtonElement>(
        `[data-session-id="${id}"] .session-row-stop`,
      );
      btn?.click();
      btn?.click();
    }, idA);

    // While the delayed stop is in flight: A's own controls are locked...
    await expect(rowA.locator(".session-row-stop")).toBeDisabled();
    await expect(rowA.locator(".session-row-delete")).toBeDisabled();
    await expect(rowA.locator(".session-row-open")).toBeDisabled();
    // ...B's stop/delete (per-session) are unaffected...
    await expect(rowB.locator(".session-row-stop")).toBeEnabled();
    await expect(rowB.locator(".session-row-delete")).toBeEnabled();
    // ...but B's open is ALSO disabled — the nav lock is global, not
    // scoped to whichever session happens to be busy.
    await expect(rowB.locator(".session-row-open")).toBeDisabled();

    await expect
      .poll(() => rowA.locator(".status-badge").textContent(), {
        timeout: 10_000,
      })
      .toMatch(/^exited — stopped by user/);

    // Everything is usable again once the operation completes, and only
    // ONE request ever reached the route — the second click was rejected
    // by the guard, not merely delayed behind the first.
    await expect(rowA.locator(".session-row-stop")).toBeEnabled();
    await expect(rowA.locator(".session-row-delete")).toBeEnabled();
    await expect(rowA.locator(".session-row-open")).toBeEnabled();
    await expect(rowB.locator(".session-row-open")).toBeEnabled();
    expect(stopRequests).toBe(1);
  } finally {
    for (const id of [idA, idB]) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// Cross-guard regression test: a rapid stop/delete pair on the SAME row —
// two native DOM clicks dispatched in the same JS tick, the same
// bare-`.click()` technique the guard test above uses to bypass
// Playwright's own actionability wait — must never let `on_stop` and the
// delete-confirm flow interleave badly, in EITHER click order:
//   - delete-then-stop: without `on_stop`'s own `confirming` check (see
//     its doc in lib.rs), a stop queued right behind a delete click could
//     slip this id into `pending` WHILE the confirm prompt is opening, so
//     a later, perfectly genuine "confirm delete" click would find
//     `pending` already occupied and silently no-op via `do_delete`'s
//     re-entry guard instead of deleting — a confirmed delete vanishing
//     with no error at all.
//   - stop-then-delete: without `on_delete`'s own `pending` check, the
//     delete click could open a confirm prompt for a session a stop is
//     already acting on, whose eventual confirm would then race that
//     in-flight stop.
// Both sessions use a real, killable `sleep 300` so `on_stop`'s own API
// call has something to reach — a synthetic stub would leave "the guard
// refused it" indistinguishable from "the request never landed at all".
test("rapid stop/delete clicks on the same row never let a confirmed delete silently vanish", async ({
  page,
  request,
}) => {
  const createdA = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  const createdB = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(createdA.status()).toBe(200);
  expect(createdB.status()).toBe(200);
  const { id: idA } = await createdA.json();
  const { id: idB } = await createdB.json();

  let stopRequestsA = 0;
  await page.route(`**/api/sessions/${idA}/stop`, async (route) => {
    stopRequestsA++;
    await route.continue();
  });
  let deleteRequestsA = 0;
  await page.route(`**/api/sessions/${idA}`, async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequestsA++;
    }
    await route.continue();
  });
  let stopRequestsB = 0;
  await page.route(`**/api/sessions/${idB}/stop`, async (route) => {
    stopRequestsB++;
    await route.continue();
  });

  try {
    await page.goto("/");
    const rowA = page.locator(`[data-session-id="${idA}"]`);
    const rowB = page.locator(`[data-session-id="${idB}"]`);
    await expect(rowA.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await expect(rowB.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });

    // Ordering 1 (session A): delete, then stop, dispatched together.
    await page.evaluate((id) => {
      const row = document.querySelector(`[data-session-id="${id}"]`)!;
      row.querySelector<HTMLButtonElement>(".session-row-delete")?.click();
      row.querySelector<HTMLButtonElement>(".session-row-stop")?.click();
    }, idA);

    // The confirm prompt won; the queued stop click was refused outright
    // — no stop request ever reached the network.
    await expect(rowA.locator(".confirm-consequence")).toBeVisible();
    expect(stopRequestsA).toBe(0);

    // The genuinely user-driven confirm click must still work normally:
    // this is the exact click the old (pre-cross-guard) bug would have
    // silently swallowed had a stop slipped into `pending` first.
    await rowA.locator(".confirm-delete").click();
    await expect(rowA).toHaveCount(0, { timeout: 10_000 });
    expect(deleteRequestsA).toBe(1);
    expect(stopRequestsA).toBe(0);

    // Ordering 2 (session B): stop, then delete, dispatched together.
    await page.evaluate((id) => {
      const row = document.querySelector(`[data-session-id="${id}"]`)!;
      row.querySelector<HTMLButtonElement>(".session-row-stop")?.click();
      row.querySelector<HTMLButtonElement>(".session-row-delete")?.click();
    }, idB);

    // The stop won; the queued delete click was refused, so no confirm
    // prompt ever appeared for B at all.
    await expect(rowB.locator(".status-badge")).toHaveText(
      /^exited — stopped by user/,
      { timeout: 10_000 },
    );
    await expect(rowB.locator(".confirm-consequence")).toHaveCount(0);
    expect(stopRequestsB).toBe(1);
  } finally {
    for (const id of [idA, idB]) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// The other cross-guard regression test: `confirm_delete`'s "proceed ONLY
// when `confirming.remove` reports the id was actually present" check
// (lib.rs) exists specifically for a cancel and a confirm click racing
// each other, not for the stop/delete race the test above covers. Both
// buttons are captured BEFORE either is clicked, then clicked together in
// one synchronous block — cancel first, confirm second — the same
// bare-`.click()` technique used elsewhere in this file to bypass
// Playwright's own actionability wait, which would otherwise never let a
// click reach a button its own prior click had logically superseded.
// Without the guard, the confirm click (processed second, after cancel
// has already cleared `confirming`) would still fall through to
// `do_delete` regardless — deleting a session the user had just told the
// UI, in the very same gesture, to leave alone.
test("dispatching cancel and confirm in the same tick never deletes the session", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  let deleteRequests = 0;
  await page.route(`**/api/sessions/${id}`, async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-consequence")).toBeVisible();

    await page.evaluate((sessionId) => {
      const row = document.querySelector(`[data-session-id="${sessionId}"]`)!;
      const cancel = row.querySelector<HTMLButtonElement>(".confirm-cancel")!;
      const confirm = row.querySelector<HTMLButtonElement>(".confirm-delete")!;
      cancel.click();
      confirm.click();
    }, id);

    // Cancel won: the row is back to normal, and no DELETE was ever sent
    // — not merely "not yet", but never at all.
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);
    await expect(row.locator(".session-row-delete")).toBeEnabled();
    expect(deleteRequests).toBe(0);
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// `autofocus` on the cancel button (`SessionRow`'s "Focus-on-open" doc in
// lib.rs) is the safety default: the instant the confirm prompt mounts,
// keyboard focus must already be ON cancel, not confirm, so a stray
// Enter/Space reaching the page right after the delete click (residual
// focus, a fast typist) backs OUT of the destructive action instead of
// into it. Checked via `document.activeElement` (Playwright's
// `toBeFocused`), then exercised through a genuine Enter keypress via
// Playwright's own keyboard API — the actual mechanism a stray keystroke
// would use, not just a synthetic click on cancel.
test("the confirm prompt focuses cancel on open; Enter closes it without deleting", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  let deleteRequests = 0;
  await page.route(`**/api/sessions/${id}`, async (route) => {
    if (route.request().method() === "DELETE") {
      deleteRequests++;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });
    await row.locator(".session-row-delete").click();
    await expect(row.locator(".confirm-consequence")).toBeVisible();

    await expect(row.locator(".confirm-cancel")).toBeFocused();

    await page.keyboard.press("Enter");
    await expect(row.locator(".confirm-consequence")).toHaveCount(0);
    await expect(row.locator(".session-row-delete")).toBeEnabled();
    expect(deleteRequests).toBe(0);
  } finally {
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// SPEC.md's "Title: optional; auto-generated when omitted" through the
// real create endpoint (farhelm-supervisor's `create_session` derives the
// working directory's basename — see its own doc). A regression that sent
// an empty STRING instead of omitting/nulling the field would ask the
// supervisor to name the session "" verbatim (see `create_session`'s doc
// in lib.rs) rather than triggering the derivation at all, so this checks
// both ends: the wire request itself, and the title the created session
// actually got.
test("a blank title creates a session titled after the working directory's basename, not an empty string", async ({
  page,
  request,
}) => {
  let capturedBody: any = null;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    capturedBody = route.request().postDataJSON();
    await route.continue();
  });

  let id: string | undefined;
  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title: "",
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // `== null` deliberately covers BOTH an omitted key (`undefined`
    // after JSON parsing) and an explicit `null` value — `create_session`
    // in lib.rs sends `Option<&str>` through `serde_json::json!`, which
    // serializes `None` as a JSON `null` rather than dropping the key,
    // and either shape is equally correct here: what matters is that it
    // is NOT the empty string.
    expect(capturedBody.title == null).toBe(true);

    const titleText = await page.locator(".titlebar .title").textContent();
    expect(titleText).toBe("tmp");
    expect(titleText).not.toBe("");

    const listing = await (await request.get("/api/sessions")).json();
    const createdSession = listing.sessions.find(
      (s: any) => s.title === "tmp" && s.cwd === "/tmp",
    );
    expect(createdSession).toBeTruthy();
    id = createdSession.id;
  } finally {
    if (id) {
      await request.post(`/api/sessions/${id}/stop`).catch(() => {});
      await request.delete(`/api/sessions/${id}`).catch(() => {});
    }
  }
});

// A real end-to-end status flip, observed through the LIST UI rather than
// the API this time (the test above already covers the API's own view of
// stop/delete). This deliberately does NOT use a session whose command
// exits near-instantly (`sh -c 'exit 7'`, an earlier design): a command
// that is already dead by the time the FIRST list fetch happens would
// only prove that a freshly-fetched already-exited row renders
// correctly — it would never prove that the list REFRESHES an EXISTING
// row from alive to exited, which is the actual polling behavior this
// test exists to pin. `trap ... TERM` keeps the session observably alive
// first, so stopping it via the API is what exercises a genuine in-place
// refresh of the same `data-session-id` row.
test("list refreshes an existing row from alive to exited, then drops it on delete", async ({
  page,
  request,
}) => {
  const created = await request.post("/api/sessions", {
    data: {
      cwd: "/tmp",
      invocation: `sh -c 'trap "exit 7" TERM; sleep 300'`,
    },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row.locator(".status-badge")).toHaveText("alive", {
      timeout: 10_000,
    });

    await request.post(`/api/sessions/${id}/stop`);
    // The exact exit code is NOT pinned here: `kill_process_tree`
    // (farhelm-supervisor/src/service.rs) sends SIGTERM, waits a grace
    // period, then RE-ENUMERATES the tree and SIGKILLs only whatever it
    // still finds alive at that point — a process that already exited
    // from the trap's `exit 7` before the grace period elapsed simply
    // will not be found, and keeps its trap-driven exit code. So the
    // genuine race is whether the trap's `exit 7` completes before that
    // grace period does: if it does not, the process is still alive at
    // re-enumeration time and gets SIGSTOPped then SIGKILLed instead,
    // turning the eventual death into a signal death tmux cannot reduce
    // to a code. The badge could legitimately read "exited (code 7)" or
    // plain "exited" either way; only the COARSE state transition is
    // asserted here. The exact text each `SessionStatus` renders into is
    // already pinned unconditionally by
    // `status_badge_matches_text_and_class_for_each_status` in lib.rs.
    //
    // "exited — stopped by user": this session ended because the user
    // stopped it, and PLAN_M3.md item 4's durable annotation QUALIFIES
    // the exited badge rather than replacing it (SPEC.md: "'stopped' is
    // not a distinct status").
    await expect(row.locator(".status-badge.exited")).toHaveText(
      /^exited — stopped by user/,
      { timeout: 10_000 },
    );

    await request.delete(`/api/sessions/${id}`);
    await expect(row).toHaveCount(0, { timeout: 10_000 });
  } finally {
    // Best-effort cleanup for the case where an assertion above threw
    // before the happy-path delete ran; see the identical pattern (and
    // its rationale) on the HTTP-level test above.
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// PLAN_M2.md acceptance 5: a capped, truncated list reply must be VISIBLE
// as such, not silently presented as complete. Reaching a real truncation
// (the supervisor's ~500-session cap) would mean creating hundreds of
// sessions just to exercise one banner, so this intercepts the same GET
// /api/sessions the real list polls and fulfills it with a small,
// synthetic truncated listing instead — enough to prove the UI's
// truncation logic without the cost or flakiness of a 500-session stack.
// No method check, no unroute: this page never makes a non-GET request
// to /api/sessions, and Playwright tears the route down with the page
// when the test ends.
// PLAN_M3.md item 2 in the browser: an interrupted session must render
// its own badge AND route delete like an ended session — straight through,
// no confirmation. The confirmation prompt exists to protect an agent that
// might still be running, and a host reboot is what produced this status,
// so there is not even a stray descendant left for a delete to kill.
//
// The listing is synthesized rather than provoked, because provoking it
// for real would mean rebooting the machine running the suite: the status
// comes from a boot-id comparison the Rust suite covers directly (e2e.rs's
// `a_reboot_interrupts_live_sessions_and_preserves_ended_ones`). What is
// under test here is only what the UI does with the status, which is
// exactly the half that needs a browser. The DELETE is counted and stalled
// so "no confirm prompt appeared" cannot be confused with "one appeared
// and vanished before the assertion ran".
test("an interrupted session shows its badge and deletes without confirming", async ({
  page,
}) => {
  const session = {
    id: "synthetic-interrupted",
    title: "synthetic-interrupted",
    cwd: "/tmp",
    invocation: "true",
    status: { state: "interrupted" },
    annotation: null,
  };
  await page.route("**/api/sessions", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ sessions: [session], total: 1, truncated: false }),
    }),
  );
  let deleteRequests = 0;
  let releaseDelete: () => void = () => {};
  const deleteHeld = new Promise<void>((resolve) => {
    releaseDelete = resolve;
  });
  await page.route(`**/api/sessions/${session.id}`, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.continue();
      return;
    }
    deleteRequests += 1;
    await deleteHeld;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: "{}",
    });
  });

  await page.goto("/");
  const row = page.locator(`[data-session-id="${session.id}"]`);
  await expect(row.locator(".status-badge.interrupted")).toHaveText(
    "interrupted",
    { timeout: 10_000 },
  );

  await row.locator(".session-row-delete").click();
  // The DELETE is stalled, so the row is still on screen — and while it
  // is, no confirmation controls exist at all.
  await expect(row).toHaveCount(1);
  await expect(row.locator(".confirm-consequence")).toHaveCount(0);
  await expect(row.locator(".confirm-delete")).toHaveCount(0);
  expect(deleteRequests).toBe(1);
  releaseDelete();
});

// PLAN_M3.md item 3: the launch shim's exec-failure sentinel, surfaced on
// the wire as `SessionStatus::Error { detail }`. Synthetic and route-mocked
// exactly like the `interrupted` test just above — real end-to-end coverage
// of the classification itself (create with a genuinely missing binary,
// wait for the supervisor to read the sentinel and commit `Error`) lives in
// `crates/farhelm/tests/e2e.rs`; this test is scoped to what only the
// BROWSER can prove: the badge's exact text and CSS class, and that
// deleting an error row — like an exited or interrupted one — skips the
// confirmation prompt entirely. The reason is not "nothing ever ran": the
// login shell and the launch shim DID run (the shim is what writes this
// very sentinel, from inside a real process) — it is that the AGENT'S OWN
// exec is what failed, before it or anything it might have spawned ever
// existed, so there is no lingering process tree for a delete to warn
// about.
test("an error session shows its badge with detail and deletes without confirming", async ({
  page,
}) => {
  const detail = "exec_failed argv0=/nope errno=2";
  const session = {
    id: "synthetic-error",
    title: "synthetic-error",
    cwd: "/tmp",
    invocation: "/nope",
    status: { state: "error", detail },
    annotation: null,
  };
  await page.route("**/api/sessions", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ sessions: [session], total: 1, truncated: false }),
    }),
  );
  let deleteRequests = 0;
  let releaseDelete: () => void = () => {};
  const deleteHeld = new Promise<void>((resolve) => {
    releaseDelete = resolve;
  });
  await page.route(`**/api/sessions/${session.id}`, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.continue();
      return;
    }
    deleteRequests += 1;
    await deleteHeld;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: "{}",
    });
  });

  await page.goto("/");
  const row = page.locator(`[data-session-id="${session.id}"]`);
  // The badge must state the shim's own recorded detail, not just the
  // bare word "error" — it is the one piece of information that actually
  // explains why the row needs attention, and the class must be the
  // dedicated `error` modifier (red family), never `exited`'s.
  await expect(row.locator(".status-badge.error")).toHaveText(
    `error — ${detail}`,
    { timeout: 10_000 },
  );

  await row.locator(".session-row-delete").click();
  // The DELETE is stalled, so the row is still on screen — and while it
  // is, no confirmation controls exist at all: the agent's own exec never
  // succeeded, so there is no lingering process tree for a delete to warn
  // about (see this test's own top-of-file comment for why that is not
  // the same claim as "nothing ever ran").
  await expect(row).toHaveCount(1);
  await expect(row.locator(".confirm-consequence")).toHaveCount(0);
  await expect(row.locator(".confirm-delete")).toHaveCount(0);
  expect(deleteRequests).toBe(1);
  releaseDelete();
});

// Review-swarm fix batch item 21: the shim's own detail is argv-derived,
// so — unlike every OTHER badge's fixed, short vocabulary — its length is
// not bounded by anything this UI controls. Without `app.css`'s
// `.status-badge` cap (`max-width`/`min-width: 0`/`overflow: hidden`), a
// long detail can widen the row past its siblings' shrink budget and push
// the stop/delete buttons out of reach. Pinned in the browser's actual
// layout engine, not just against the CSS source: the badge visibly
// clips (its scrollWidth exceeds its clientWidth) and the delete button
// stays on screen and clickable regardless.
test("a long error detail clips the badge without pushing the delete button out of reach", async ({
  page,
}) => {
  const detail = `exec_failed argv0=${"/very/long/path/segment".repeat(40)} errno=2`;
  const session = {
    id: "synthetic-error-long",
    title: "synthetic-error-long",
    cwd: "/tmp",
    invocation: "/nope",
    status: { state: "error", detail },
    annotation: null,
  };
  await page.route("**/api/sessions", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ sessions: [session], total: 1, truncated: false }),
    }),
  );

  await page.goto("/");
  const row = page.locator(`[data-session-id="${session.id}"]`);
  const badge = row.locator(".status-badge.error");
  await expect(badge).toBeVisible();

  const clips = await badge.evaluate((el) => el.scrollWidth > el.clientWidth);
  expect(clips).toBe(true);

  const deleteButton = row.locator(".session-row-delete");
  await expect(deleteButton).toBeVisible();
  const box = await deleteButton.boundingBox();
  expect(box).not.toBeNull();
  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();
  expect(box!.x + box!.width).toBeLessThanOrEqual(viewport!.width);
  await deleteButton.click();
  await expect(row.locator(".confirm-consequence")).toHaveCount(0);
});

// Review-swarm fix batch item 21's other half: the SAME injection idiom
// `delete confirmation safely displays a title containing executable
// HTML...` (above) applied to the error badge's `detail` — it renders
// through Dioxus's normal text interpolation exactly like every other
// server-controlled string here, but the badge is a NEW render site this
// PR adds, so it earns its own direct pin rather than relying on the
// title test's coverage to imply it.
test("an error detail containing executable HTML renders literally in the badge", async ({
  page,
}) => {
  const detail = `exec_failed argv0=<img src=x onerror="window.__pwned=1"> errno=2`;
  const session = {
    id: "synthetic-error-xss",
    title: "synthetic-error-xss",
    cwd: "/tmp",
    invocation: "/nope",
    status: { state: "error", detail },
    annotation: null,
  };
  await page.route("**/api/sessions", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ sessions: [session], total: 1, truncated: false }),
    }),
  );

  await page.goto("/");
  const row = page.locator(`[data-session-id="${session.id}"]`);
  await expect(row.locator(".status-badge.error")).toHaveText(`error — ${detail}`, {
    timeout: 10_000,
  });
  expect(await page.evaluate(() => (window as any).__pwned)).toBeUndefined();
});

test("truncation banner shows when the listing reports truncated", async ({
  page,
}) => {
  await page.route("**/api/sessions", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sessions: [
          { id: "synthetic-1", title: "synthetic-1", cwd: "/tmp", invocation: "true" },
          { id: "synthetic-2", title: "synthetic-2", cwd: "/tmp", invocation: "true" },
        ],
        total: 700,
        truncated: true,
      }),
    }),
  );

  await page.goto("/");
  await expect(page.locator(".truncation-banner")).toBeVisible();
  await expect(page.locator(".truncation-banner")).toContainText(
    "showing 2 of 700 sessions",
  );
});

// Polling is M2's whole live-update mechanism (PLAN_M2.md: "Out" defers
// live push to M5), so it needs its own direct test: with the list
// already open, a session created from elsewhere (the HTTP API, standing
// in for "any other client") must appear without a reload. Bounded at
// ~10s — comfortably above the 3s poll interval — so a regression to
// "never polls" fails the test instead of hanging the suite.
test("list polls and picks up a session created elsewhere", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await expect(page.locator(".session-list")).toBeVisible();

  const created = await request.post("/api/sessions", {
    data: { cwd: "/tmp", invocation: "sleep 300" },
  });
  expect(created.status()).toBe(200);
  const { id } = await created.json();

  try {
    await expect(page.locator(`[data-session-id="${id}"]`)).toBeVisible({
      timeout: 10_000,
    });
  } finally {
    // Best-effort: the session is long-running (`sleep 300`), so it must
    // be stopped and deleted regardless of whether the assertion above
    // passed, or it would sit in the list for every test after this one.
    await request.post(`/api/sessions/${id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${id}`).catch(() => {});
  }
});

// Host/Origin validation is what keeps a hostile page from driving the
// helm through DNS rebinding; loopback binding alone does not.
test("requests from a foreign origin are refused", async ({ request }) => {
  const resp = await request.get("/api/sessions", {
    headers: { Origin: "http://evil.example" },
  });
  expect(resp.status()).toBe(403);
});

// The framing defense: the Origin check cannot stop an <iframe> pointed at
// the helm (a GET navigation sends no Origin), and a framed terminal is a
// clickjacking target where delivered keystrokes are command execution.
// The exact header values ARE the contract, so this is an exact-value
// assertion, not a change detector.
test("responses carry the anti-framing headers", async ({ request }) => {
  const resp = await request.get("/api/sessions");
  expect(resp.status()).toBe(200);
  expect(resp.headers()["x-frame-options"]).toBe("DENY");
  expect(resp.headers()["content-security-policy"]).toBe(
    "frame-ancestors 'none'",
  );
});

// PLAN_M2_5.md step 4: terminal.js's watermark pause/resume is the thing
// that keeps a producer faster than the browser can parse from either
// freezing the tab (xterm.js's own ~50MB write-buffer cliff) or losing
// data. This is the SPEC "never a silent gap" pin for the terminal path,
// so it asserts the strongest thing available at each layer: the WHOLE
// 800,000-record stream, not just the scrollback-capped tail, arrived
// exactly once and in order (`installFloodStreamVerifier` — an
// implementation that silently dropped or duplicated records outside the
// retained tail would still pass a tail-only check, which is exactly the
// gap that finding this milestone closes), the visible tail is ALSO
// consecutive (the cheaper, still-worth-keeping check a real user's
// screen would show), and flow control demonstrably engaged (at least one
// pause, matched by a resume) while all of that happened — an
// implementation that buffered the whole 12 MiB unboundedly (exactly the
// bug this milestone closes) would pass the content checks just as easily
// as a correct one, so the pause/resume assertion is what actually pins
// that flow control did the work.
//
// `holdWrites: true` (see `holdTermWrites`'s docs): a real headless
// Chromium on typical hardware parses this fixture's ~12 MiB fast enough
// over loopback that HIGH_WATER is never actually crossed, so provoking a
// genuine pause deterministically means holding write completions back
// rather than merely delaying them, releasing again the instant
// terminal.js's own `paused` flag confirms the crossing — see that
// function's docs for the failure modes that ruled out a fixed delay, an
// arbitrary byte cap, and a Node-side release in turn. By the time this
// test can observe anything, the hold-then-release has already happened
// entirely inside the page, in one ordinary, brief pause/resume cycle.
// `verifyStream: true` installs the whole-stream verifier FIRST (order is
// load-bearing — see its own docs), so it keeps observing every byte even
// after `holdTermWrites` later hands writes straight through.
test("the whole flood stream arrives exactly once and in order, with at least one pause/resume cycle observed", async ({
  page,
  request,
}) => {
  test.setTimeout(60_000);
  const title = `flood-complete-${Date.now()}`;
  let id: string | undefined;
  try {
    id = await openFloodSession(page, request, title, {
      holdWrites: true,
      verifyStream: true,
    });

    await waitForTermText(page, "FLOOD-DONE", 45_000);

    // The write-completion callbacks that drive resume are asynchronous
    // relative to xterm.js appending to its buffer, so `FLOOD-DONE`
    // showing up in the buffer does not itself guarantee the LAST
    // callback (and therefore a resume, if the tail landed mid-pause) has
    // fired yet. Poll rather than reading the hook once.
    await expect
      .poll(() => page.evaluate(() => (window as any).__farhelmTest.paused), {
        message: "the attachment must end unpaused once output is exhausted",
      })
      .toBe(false);

    const hooks = await page.evaluate(() => (window as any).__farhelmTest);
    expect(hooks.pauseCount).toBeGreaterThanOrEqual(1);
    // Exactly-once semantics (terminal.js's own docs): every pause this
    // attachment sent must eventually have been answered by a resume,
    // since nothing here ever leaves output permanently withheld.
    expect(hooks.resumeCount).toBe(hooks.pauseCount);

    // The whole-stream check: every record observed exactly once, in
    // order, with the terminal marker right behind the last one. This is
    // the assertion the retained-tail check below cannot make on its own
    // — see `installFloodStreamVerifier`'s docs for why.
    const verify = await page.evaluate(() => (window as any).__farhelmFloodVerify);
    expect(verify.error).toBeNull();
    expect(verify.recordsSeen).toBe(FLOOD_RECORDS);
    expect(verify.nextExpected).toBe(FLOOD_RECORDS);
    expect(verify.sawDone).toBe(true);

    // Retained-tail check: the buffer's visible tail (scrollback-capped,
    // so this necessarily starts partway through the 800,000 records)
    // must ALSO be strictly consecutive up to the last record, with
    // `FLOOD-DONE` right behind it — this is what a real user's screen
    // would actually show, kept alongside the whole-stream check above
    // rather than instead of it.
    const records = parseFloodRecords(await termText(page));
    expect(records.length).toBeGreaterThan(0);
    for (let i = 1; i < records.length; i++) {
      expect(records[i]).toBe(records[i - 1] + 1);
    }
    expect(records[records.length - 1]).toBe(FLOOD_RECORDS - 1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// PLAN_M2_5.md's stall-detach contract from the browser's side: a viewer
// that stops draining entirely (a wedged tab, a laptop asleep past its
// WebSocket timeout) must not pin the supervisor's buffers forever — past
// STALL_DETACH_TIMEOUT (farhelm-supervisor/src/service.rs, 60s) the
// attachment is detached with a visible reason, exactly like the existing
// takeover detach, and reattaching afterward must work normally.
//
// This waits out the REAL 60-second timeout rather than a shortened one.
// That is not a shortcut left on the table: the timeout is deliberately
// not environment-configurable (farhelm-supervisor's own docs — "this
// repo's tests never mutate the process environment"), and the one
// injection seam that DOES exist — a short-timeout constructor argument
// the Rust integration suite uses directly (`SupervisorTimeouts`, via
// `harness_with_timeouts`) — is reached only through `farhelm supervisor
// run`'s CLI and `farhelm_supervisor::service::run`, and the production
// binary exposes no flag for it. A browser-level test has no way to dial
// this down without adding that flag to the production binary itself, so
// waiting out the real timeout is the honest option, not a workaround.
//
// The timing assertion below exists because "eventually" is not precise
// enough here: the SAME detach reason string can also come from the
// helm's own per-terminal channel-full backstop (crates/farhelm-helm/src/client.rs)
// — a completely different, byte-volume-driven mechanism that has nothing
// to do with pause DURATION and could plausibly fire much sooner. A test
// that only waited for the banner to appear, with no floor on how soon,
// could pass by catching that mechanism instead of the one this test is
// actually named for.
test("a client that stops draining is detached with the stall reason after the full stall interval; reattaching afterward replays", async ({
  page,
  request,
}) => {
  // The nested waits below sum to a bit over STALL_DETACH_TIMEOUT (60s):
  // confirming the pause, the banner's own margin past 60s, and the
  // reattach/replay at the end. Set comfortably above that sum, plus room
  // for setup and cleanup, so a slow-but-legitimate run does not trip
  // Playwright's OWN timeout ahead of the assertions this test relies on
  // to fail correctly.
  test.setTimeout(150_000);
  const title = `flood-stall-${Date.now()}`;
  let id: string | undefined;
  try {
    id = await createFloodGatedSession(request, title);

    await page.goto("/");
    // Same readiness wait `holdTermWrites` documents and relies on — the
    // patch below needs `window.Terminal` to already exist.
    await page.waitForFunction(() => !!(window as any).Terminal);
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible();

    // Patch EVERY future xterm.js instance's write() to swallow its
    // completion callback, BEFORE mount() constructs one — patching the
    // instance afterward would race the flood's first bytes, which can
    // land (and drain normally) before a post-mount patch runs. With no
    // callback ever firing, terminal.js's `pendingWrite` never drains
    // below LOW_WATER, so the pause this test provokes is never answered
    // by a resume: exactly the "viewer stopped consuming" wedge
    // `STALL_DETACH_TIMEOUT` exists for.
    await page.evaluate(() => {
      (window as any).__testRealWrite = (window as any).Terminal.prototype.write;
      (window as any).Terminal.prototype.write = function () {
        // Deliberately no callback invocation.
      };
    });

    await row.click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await sendFloodGateByte(page);

    // The pause must actually happen, and promptly — this is the moment
    // the 60s stall-detach clock (STALL_DETACH_TIMEOUT) starts counting.
    // Asserting it HERE, not just eventually, is what lets the timing
    // assertion below measure from the right instant rather than from
    // whenever this test happened to start.
    await expect
      .poll(() => page.evaluate(() => (window as any).__farhelmTest.pauseCount), {
        timeout: 30_000,
        message: "the attachment must cross HIGH_WATER and pause before the stall clock can start",
      })
      .toBeGreaterThanOrEqual(1);
    const pausedAt = Date.now();

    // The banner is this test's synchronization point for the DETACH
    // itself — it can only appear once the supervisor's stall timeout
    // actually elapses, so this genuinely waits the real ~60s (see this
    // test's own file-level docs for why no shorter seam exists to inject
    // here).
    await expect(page.locator("#term-banner")).toContainText(
      "Detached: terminal stopped consuming output (stalled)",
      { timeout: 75_000 },
    );
    const detachedAt = Date.now();

    // Attribution, not just occurrence (see this test's own file-level
    // docs): a detach arriving materially before a full stall interval
    // had elapsed SINCE THE PAUSE would mean this test caught the helm's
    // channel-full backstop instead of the supervisor's stall-detach
    // timeout — margin below the nominal 60s, not an exact threshold,
    // because wall-clock measurement across a real network and process
    // boundary is inherently a little loose.
    const pausedForMs = detachedAt - pausedAt;
    expect(pausedForMs).toBeGreaterThan(55_000);

    // Restore real rendering before reattaching, or the replay below
    // would be exactly as invisible as the stall that produced it.
    await page.evaluate(() => {
      (window as any).Terminal.prototype.write = (window as any).__testRealWrite;
      delete (window as any).__testRealWrite;
    });

    // Reattach the same way the navigation-lifecycle test above
    // ("back tears down the mounted terminal; reopening the same session
    // mounts a fresh one") does: back to the list, then the same row. The
    // session survived the detach (SPEC.md: no viewer can affect a
    // session it stalls out of), so replay brings back its own tail —
    // already complete in tmux history well before the stall elapsed, so
    // no second gate byte is needed.
    await page.locator(".back-button").click();
    await row.click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FLOOD-DONE", 15_000);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// PLAN_M2_5.md step 4's reconnect invariant: a fresh WebSocket must start
// from a clean slate regardless of what the OLD one last sent. A
// regression that leaked `paused`/pending-write state across attachments
// (e.g. module-scoped instead of per-mount counters) would show up here as
// a resume sent on the new socket before it ever paused on its own — this
// pins that it does not.
//
// SAME-REALM back-navigation, not `page.reload()`: an earlier version of
// this test reloaded the page between attachments, which tears down the
// WHOLE JS realm — and therefore cannot catch a regression that leaked
// state via a module-scoped variable instead of a per-mount closure,
// since a reload resets those too, for reasons that have nothing to do
// with terminal.js's own lifecycle discipline. Only a same-realm
// back-then-reopen (the app's own navigation, `unmount()` then a fresh
// `mount()`) actually exercises whatever terminal.js does at THAT
// boundary. To make that meaningful, attachment one is kept STILL PAUSED
// at the moment of navigating away — proven via the hook, not merely
// assumed — using a permanent swallow patch (like the stall test's own),
// not `holdTermWrites` (which self-releases the instant it observes a
// pause, which would let attachment one recover before this test ever got
// to navigate away from it).
//
// THE FLAKE that cost several M4 triage rounds, and why the drain below
// exists: keeping attachment one paused is exactly what keeps the PRODUCER
// running. A paused client stops the bytes, not the fake agent, so when
// that attachment goes away the rest of the ~12 MiB fixture is still
// coming — and the next attachment gets replay PLUS that live tail, which
// is a legitimate live stream, not a replay. Instrumented byte
// counts on an idle machine put the tail between 1.7 MiB and 5.4 MiB on
// consecutive runs, i.e. straddling terminal.js's 4 MiB HIGH_WATER, and
// the 5.4 MiB run produced `pauseCount: 1, resumeCount: 1` on attachment
// two. That is flow control working, not failing, so the old "replay
// cannot reach HIGH_WATER" premise was simply false whenever the producer
// had not finished yet.
//
// The premise is now made true rather than assumed: `drainFloodOffScreen`
// takes the whole rest of the flood on a raw socket that is nobody's
// mount, so the attachment under test starts against a finished producer
// and sees only tmux's ~12,000-line history — a few hundred KiB, orders of
// magnitude below the mark. The three assertions at the end are unchanged
// and mean exactly what they always claimed to mean. Note that the
// assertions were NOT relaxed to tolerate a live-tail pause: `pauseCount:
// 0` on a replay-only attachment is a much sharper statement than "either
// zero, or one that we will excuse", and the sharper one is available for
// free once the producer is quiesced.
test("reconnecting within the same page resets flow-control state; the new attachment neither inherits a pause nor sends a bare resume", async ({
  page,
  request,
}) => {
  // The producer's whole ~12 MiB now has to finish before the measurement
  // can even start: attachment one carries it as far as the pause, the
  // drain takes the entire rest, and only then comes the reattach. About
  // 20s end to end on an idle box, but the default 60s leaves little room
  // on a loaded CI runner, and busting it would look like a hang rather
  // than the flake it replaced.
  test.setTimeout(180_000);
  const title = `flood-reconnect-${Date.now()}`;
  let id: string | undefined;
  try {
    await page.goto("/");
    await page.waitForFunction(() => !!(window as any).Terminal);
    await page.evaluate(() => {
      (window as any).__testRealWrite = (window as any).Terminal.prototype.write;
      (window as any).Terminal.prototype.write = function () {
        // Deliberately no callback invocation: keeps attachment one
        // paused indefinitely, so it is still, provably, paused at the
        // moment this test navigates away from it below.
      };
    });

    id = await createFloodGatedSession(request, title);
    const row = page.locator(`[data-session-id="${id}"]`);
    await expect(row).toBeVisible();
    await row.click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await sendFloodGateByte(page);

    await expect
      .poll(() => page.evaluate(() => (window as any).__farhelmTest.pauseCount), {
        timeout: 30_000,
        message: "the first attachment must actually cross HIGH_WATER and pause",
      })
      .toBeGreaterThanOrEqual(1);
    // Not just "paused once" — still paused right now, which is the
    // premise the "starts unpaused" assertion on the reopened attachment
    // needs to actually mean something.
    expect(await page.evaluate(() => (window as any).__farhelmTest.paused)).toBe(true);

    // Restore real writing before navigating to the fresh attachment: the
    // patch above is a shared PROTOTYPE hook, so leaving it swallowing
    // would silently break the reconnected instance's own rendering too.
    // This does NOT "unpause" attachment one — its closure-scoped
    // `paused`/`pendingWrite` have nothing to do with the prototype patch
    // surviving; its own already-dispatched writes' callbacks were baked
    // in at call time and will now simply never fire, which is exactly
    // the point.
    await page.evaluate(() => {
      (window as any).Terminal.prototype.write = (window as any).__testRealWrite;
      delete (window as any).__testRealWrite;
    });

    // Captured while a terminal still exists: the drain below reattaches
    // at this same geometry rather than the query defaults, so it does not
    // reflow the pane out from under the replay this test then measures.
    const geometry = await page.evaluate(() => {
      const term = (window as any).__farhelmTerm;
      return { cols: term.cols as number, rows: term.rows as number };
    });

    await page.locator(".back-button").click();
    // The old attachment's hook must be gone entirely — the same
    // assertion the navigation-lifecycle test above pins for
    // `__farhelmTerm`/`__farhelmWs`, extended here to `__farhelmTest`:
    // `unmount()` only deletes the global if it still references THIS
    // mount's own object (terminal.js's own docs), so seeing it gone is
    // also indirect coverage that the delete actually took that branch.
    await expect
      .poll(() => page.evaluate(() => (window as any).__farhelmTest))
      .toBeUndefined();

    // Nothing is attached at this point, so the flood can finish without
    // any of it landing on the attachment this test is about to measure.
    await drainFloodOffScreen(page, id, geometry);

    await row.click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    // No second gate byte: `flood_gated` reads its gate exactly once, and
    // the drain above already carried that one run to its end. What this
    // attachment waits for is the marker coming back out of tmux's
    // history, which is also the proof that what it received is REPLAY —
    // there is no producer left to send it live.
    await waitForTermText(page, "FLOOD-DONE", 15_000);

    // The reattach's replay is bounded by the tmux history floor (~12,000
    // lines — terminal.js's `scrollback` setting and tmux.rs's
    // `HISTORY_LIMIT`), a few hundred KiB at most, far below HIGH_WATER —
    // so a healthy fresh attachment neither pauses nor resumes delivering
    // it. A resume observed here would mean the new socket inherited the
    // OLD one's paused state instead of starting clean; a pause observed
    // here would mean the replay itself grew past HIGH_WATER, which (now
    // that the producer is provably done — see the drain above) would mean
    // the history floor itself had grown past the mark, invalidating this
    // test's premise rather than exercising the invariant it is for.
    const hooks = await page.evaluate(() => (window as any).__farhelmTest);
    expect(hooks.pauseCount).toBe(0);
    expect(hooks.resumeCount).toBe(0);
    expect(hooks.paused).toBe(false);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// An attach failure must arrive as a detach notice on the socket, not a
// bare close: the helm sends the reason before closing precisely because
// a bare close renders as a generic "connection closed" and tells the
// user nothing. Driven through a raw WebSocket because the UI only ever
// opens sockets for sessions the API listed.
test("a terminal socket for an unknown session reports why", async ({
  page,
}) => {
  await page.goto("/");
  const notice = await page.evaluate(
    () =>
      new Promise<string>((resolve, reject) => {
        const ws = new WebSocket(
          `ws://${location.host}/api/sessions/no-such-session/term`,
        );
        const timer = setTimeout(() => reject(new Error("no message")), 10_000);
        ws.onmessage = (ev) => {
          clearTimeout(timer);
          resolve(String(ev.data));
        };
        ws.onclose = () => {
          clearTimeout(timer);
          reject(new Error("socket closed with no detach notice"));
        };
      }),
  );
  const msg = JSON.parse(notice);
  expect(msg.type).toBe("detached");
  expect(msg.reason).toContain("no such session");
});

// The WebSocket message-size cap is sized for large pastes (xterm.js
// hands a whole clipboard paste over as ONE message), and lore records a
// review fix that nearly shipped a 1 MiB cap — which would have dropped
// the connection on exactly the paste chunking exists to support. Only a
// direct socket send can produce a multi-megabyte message, hence the
// __farhelmWs test hook.
//
// SECOND TO LAST in the file on purpose: the suite shares one session and
// runs in file order, and this payload's PTY echo pollutes the terminal
// state — observed directly: with the takeover test after this one, its
// fresh page's READY wait found only a wall of echoed 'a's. (How much
// echoes is bounded by canonical-mode input handling and was not pinned
// down; the placement rule, not the mechanism, is the contract.) The one
// test allowed after this one is the ctrl-c regression below, which kills
// the shared session's fake agent outright — nothing may come after THAT
// depends on this session at all.
test("a multi-megabyte message does not drop the terminal socket", async ({
  page,
}) => {
  await openTerminal(page);
  await page.evaluate(() => {
    const ws = (window as any).__farhelmWs as WebSocket;
    ws.send(new Uint8Array(2 * 1024 * 1024).fill(0x61));
  });
  // The send is async; poll until the socket has drained it, still open.
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const ws = (window as any).__farhelmWs as WebSocket;
          return { state: ws.readyState, buffered: ws.bufferedAmount };
        }),
      { timeout: 15_000, message: "socket must stay open and drain" },
    )
    .toEqual({ state: WebSocket.OPEN, buffered: 0 });
  await page.locator("#terminal").click();
  await page.keyboard.press("Enter");
  await page.keyboard.type("after-big-message");
  await page.keyboard.press("Enter");
  // A generous timeout, not the suite's usual 10-15s: the supervisor's
  // dedicated input control client (`InputClient::send`, tmux.rs) now
  // waits for tmux's `%end` reply to each 256-byte `send-keys` chunk
  // before sending the next, so tmux must fully process this 2 MiB
  // payload — many thousands of chunk round trips — before it even
  // reaches the "after-big-message" line queued behind it on the same
  // connection. That synchronous-per-chunk design is deliberate (a
  // fire-and-forget write could not distinguish "tmux accepted the bytes"
  // from "tmux executed them"), so this test's budget reflects the real
  // cost of validating a payload this large rather than papering over it.
  await waitForTermText(page, "echo:after-big-message", 60_000);
});

// Regression test for the tmux paste-buffer input-mangling bug at the
// browser level: real Backspace and Ctrl+C keypresses go through xterm.js's
// own key handling (DEL, ETX — no custom key binding intercepts them, see
// terminal.js), the WebSocket, and the framing protocol exactly like any
// other keystroke, landing on `basic`'s pty in its ordinary canonical/cooked
// mode (nothing here puts it in raw mode, unlike the `hexecho` fixture the
// Rust e2e suite uses for byte-exact coverage of every mangled control byte,
// including ESC/arrow-up).
//
// ArrowUp is deliberately NOT exercised here, after checking what a
// canonical-mode pty actually does with it: Linux's ECHOCTL local-echo
// renders ANY control byte with no special canonical role — ESC included —
// as two-character caret notation ("^["), regardless of whether it arrived
// as a genuine 0x1b byte or as the bug's literal caret text. The pane's
// rendered output is identical either way, so `basic`'s canonical pty gives
// no browser-observable signal for ESC at all; that gap is exactly what
// `hexecho`'s raw mode (no ECHOCTL, no canonical processing) exists to
// close, and the Rust e2e suite's `input_bytes_survive_verbatim_through_hexecho`
// covers it directly.
//
// Backspace escapes that trap because DEL, unlike ESC, has a special
// canonical-mode role: a correctly delivered 0x7f is consumed as the ERASE
// character (removing the previous character, never echoed as text at
// all), while the bug's mangled delivery is two ordinary printable
// characters that erase nothing and sit in the buffer as literal `^?`. That
// gives a real positive/negative pair to assert on, checked below.
//
// Ctrl+C escapes it differently: ECHOCTL still renders a correctly
// delivered ETX as "^C" text, so the caret text alone proves nothing. What
// DOES distinguish the two is what happens next — a correct ETX is also
// consumed as INTR, raising SIGINT on the fake agent's foreground process
// group and killing it (default disposition; `basic` installs no handler),
// while the bug's mangled two-character delivery is inert text that leaves
// the process running. So the assertion below is "the pane no longer echoes
// new input", not "no ^C text appeared" — and it must reject the marker
// appearing ANYWHERE on an echoed line, not just as an exact substring: a
// mangled ctrl-c leaves the buffered "x" (see below) sitting in canonical
// input, so a later Enter would flush "x" plus the marker together as one
// line, echoing as `echo:x^Cpost-ctrlc-marker` — which a bare
// `.not.toContain("echo:post-ctrlc-marker")` would miss entirely, since
// that exact substring never occurs even though the marker plainly reached
// the (still-alive, still-buggy) agent.
//
// This does NOT end the tmux session, despite ending the fake-agent
// process: `remain-on-exit on` (SPEC.md) keeps a dead pane's session and
// window around so its terminal stays viewable, it just stops accepting
// input. Killing the agent is still sufficient for the assertion, and is
// LAST in the file because it is destructive to every other test's shared
// fixture: a correct fix permanently kills the fake agent every other test
// in this file was typing into.
test("real backspace erases; real ctrl-c kills the fake agent", async ({
  page,
}) => {
  // NOT `openTerminal()`: that helper waits for the "FAKE-AGENT READY"
  // banner, but the multi-megabyte test just before this one pushed well
  // over tmux's 12,000-line history-limit through this same session —
  // observed directly, the banner is gone from replay by the time this
  // test's fresh page attaches. The session is still alive underneath
  // (only its scrollback was evicted), so liveness is reproven below with
  // a fresh marker instead of the banner. Still has to go through the
  // list view to get there, same as openTerminal does.
  await page.goto("/");
  await sharedSessionRow(page).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await page.locator("#terminal").click();

  // Backspace: type a two-character marker, erase the second character,
  // and require the marker to actually disappear — not just that nothing
  // new appeared. A caret-escaped DEL would leave "xy^?" in the buffer
  // (marker intact, artifact appended); a correct erase leaves neither.
  await page.keyboard.type("xy");
  await waitForTermText(page, "xy");
  await page.keyboard.press("Backspace");
  await expect
    .poll(() => termText(page), {
      message: "backspace must erase the typed marker, not print ^?",
    })
    .not.toContain("xy");

  // Flush the "x" still sitting in canonical input BEFORE ctrl-c, and wait
  // for its echo. Without this, ctrl-c's own canonical-mode fate (consumed
  // as INTR vs. left as inert "^C" text) is entangled with "x": a mangled
  // ctrl-c would leave "x^C" buffered together, and the marker typed below
  // would flush on the SAME line as "x", one line earlier than expected.
  // Waiting for "x" to echo here is what makes the post-ctrlc assertion
  // unambiguous about what ctrl-c itself did.
  await page.keyboard.press("Enter");
  await waitForTermText(page, "echo:x");

  await page.keyboard.press("Control+c");

  // A mangled ctrl-c leaves `basic` alive and still echoing; only a real
  // SIGINT kills it. Typing a fresh marker and requiring it to NEVER echo
  // is the proof — and, being an absence, needs sustained observation
  // rather than one poll: the process take-down is not instantaneous, and
  // a single early check could pass before a still-alive process would
  // have replied. The regex (not a plain substring) is deliberate: a
  // mangled ctrl-c is inert TEXT, not a control action, so it stays in the
  // canonical buffer ahead of the marker and both flush together on the
  // same line — e.g. `echo:^Cpost-ctrlc-marker` — which
  // `.not.toContain("echo:post-ctrlc-marker")` would not catch since that
  // exact substring never appears. Matching the marker anywhere after
  // `echo:` on one line closes that gap.
  await page.keyboard.type("post-ctrlc-marker");
  await page.keyboard.press("Enter");
  const deadline = Date.now() + 3_000;
  while (Date.now() < deadline) {
    expect(await termText(page)).not.toMatch(/echo:.*post-ctrlc-marker/);
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
});


// Server-enforced create idempotency, at the level only the browser can
// exercise: the UI's own key lifecycle (PLAN_M3.md item 6, "the UI
// generates one key per intended create and reuses it across retries of
// that intent"). The supervisor's side of the contract — replay, conflict,
// crash reconciliation, the gone-error — is pinned in the Rust e2e suite,
// which can restart supervisors and inject crashes; what only this suite
// can show is that the retry a USER performs actually carries the same key
// the first attempt did.

/**
 * Delete every session with this title, however many exist.
 *
 * Plural on purpose: these tests exist because a duplicate is possible, so
 * their cleanup must not assume the thing they are testing for. A
 * single-session cleanup would leave a stray agent running for the rest of
 * the suite exactly when the test failed.
 */
async function cleanUpSessionsTitled(request: APIRequestContext, title: string) {
  const listing = await (await request.get("/api/sessions")).json().catch(() => null);
  for (const session of listing?.sessions ?? []) {
    if (session.title !== title) continue;
    await request.post(`/api/sessions/${session.id}/stop`).catch(() => {});
    await request.delete(`/api/sessions/${session.id}`).catch(() => {});
  }
}

test("a create whose reply is lost is retried with the same key and yields one session", async ({
  page,
  request,
}) => {
  const title = `intent-retry-${Date.now()}`;
  const keys: (string | undefined)[] = [];
  let firstStatus = 0;
  // The first POST really reaches the server — `route.fetch()` performs
  // it — and only its RESPONSE is thrown away, which is precisely the
  // ambiguous failure this feature exists for: a session now exists that
  // the browser has no way of knowing about. Aborting instead would test
  // only that the key is reused, not that the server dedups against a
  // session the client never heard of.
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    keys.push(JSON.parse(route.request().postData() ?? "{}").intent_key);
    if (keys.length === 1) {
      const response = await route.fetch();
      firstStatus = response.status();
      await route.abort();
      return;
    }
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    // The dropped response surfaces as an ordinary create error, leaving
    // the form usable — the state a user retries from.
    await expect(form.locator(".create-session-error")).toBeVisible();
    // The first attempt SUCCEEDED on the server; without that, the retry
    // below would merely be creating the session for the first time and
    // would prove nothing about deduplication.
    expect(firstStatus).toBe(200);

    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
      timeout: 15_000,
    });
    await expect(page.locator(".titlebar .title")).toHaveText(title);

    expect(keys).toHaveLength(2);
    expect(keys[0]).toBeTruthy();
    expect(keys[1]).toBe(keys[0]);
    // The point of all of it: one intended create, one session, even
    // though the server genuinely handled two requests.
    const listing = await (await request.get("/api/sessions")).json();
    expect(listing.sessions.filter((s: any) => s.title === title)).toHaveLength(1);
  } finally {
    await cleanUpSessionsTitled(request, title);
  }
});

// The other edge of the same rule, and the reason the key is minted at
// first submit rather than when the form opens: editing a field makes the
// next submit a DIFFERENT intent, so it must carry a different key.
//
// What reusing the old key would cost depends on how far the first attempt
// got. Here it failed on a precondition, which the supervisor records as
// that intent's outcome — so a resubmission under the same key would
// REPLAY "working directory does not exist" no matter what the user fixed,
// leaving the form permanently unable to succeed. Where the first attempt
// got further, the same reuse is refused as a conflict instead. Both are
// dead ends; minting a new key is what makes "fix it and try again" work.
//
// Each field gets its own pass, because the key is cleared by each input's
// own handler and a missed one would only show up in whichever field the
// user happened to edit.
//
// Each edit is to another value that ALSO fails, so the assertion is about
// the key alone rather than about whether the corrected request happens to
// succeed — and so both attempts land in the same observable state.
for (const field of [
  { name: "working directory", index: 0, edit: "/nonexistent/also/not/here" },
  { name: "agent command", index: 1, edit: "also-not-an-agent" },
]) {
  test(`editing the ${field.name} after a failed create mints a new intent key`, async ({
    page,
    request,
  }) => {
    const title = `intent-new-${field.index}-${Date.now()}`;
    const keys: (string | undefined)[] = [];
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      keys.push(JSON.parse(route.request().postData() ?? "{}").intent_key);
      await route.continue();
    });

    try {
      await page.goto("/");
      // Both fields start wrong so that fixing EITHER one leaves a
      // request that still differs from the first attempt.
      const form = await fillCreateForm(page, {
        cwd: "/nonexistent/definitely/not/here",
        invocation: "definitely-not-an-agent",
        title,
      });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      await form.locator('input[type="text"]').nth(field.index).fill(field.edit);
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      expect(keys).toHaveLength(2);
      expect(keys[0]).toBeTruthy();
      expect(keys[1]).toBeTruthy();
      expect(keys[1]).not.toBe(keys[0]);
    } finally {
      await cleanUpSessionsTitled(request, title);
    }
  });
}

// The title is prose rather than something that gets executed, but it is
// still part of what makes a create the create it is (the server
// fingerprints it), so editing it starts a new intent exactly like the
// other two fields. Kept separate from the loop above because a bad title
// cannot fail a create — this one has to succeed to be observed at all.
test("editing the title after a failed create mints a new intent key", async ({
  page,
  request,
}) => {
  const title = `intent-title-${Date.now()}`;
  const keys: (string | undefined)[] = [];
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    keys.push(JSON.parse(route.request().postData() ?? "{}").intent_key);
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/nonexistent/definitely/not/here",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await expect(form.locator(".create-session-error")).toBeVisible();

    await form.locator('input[type="text"]').nth(2).fill(`${title}-renamed`);
    await form.locator('input[type="text"]').nth(0).fill("/tmp");
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
      timeout: 15_000,
    });

    expect(keys).toHaveLength(2);
    expect(keys[1]).not.toBe(keys[0]);
  } finally {
    await cleanUpSessionsTitled(request, title);
    await cleanUpSessionsTitled(request, `${title}-renamed`);
  }
});

// The form is inert for the whole submission — inputs included, not just
// the submit button — which is what makes the key lifecycle a rule rather
// than a race: key generation runs in the renderer and is asynchronous, so
// a keystroke landing between minting a key and sending it would otherwise
// publish a key belonging to values the user has already changed.
test("the create form's inputs are disabled while a create is in flight", async ({
  page,
  request,
}) => {
  const title = `intent-inert-${Date.now()}`;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 800));
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    for (const index of [0, 1, 2]) {
      await expect(form.locator('input[type="text"]').nth(index)).toBeDisabled();
    }
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
      timeout: 15_000,
    });
  } finally {
    await cleanUpSessionsTitled(request, title);
  }
});

// The two id generators diverge exactly here, and this pins both halves of
// that divergence in one run, because they are a single decision: with no
// CSPRNG available, an intent key falls back to a weak generator and a
// session LEASE refuses outright (see `mint_intent_key` and `mint_lease` in
// farhelm-ui).
//
// The asymmetry is not fussiness. An intent key only has to be unique among
// one user's own creates, and refusing every create on such a browser would
// be strictly worse for a value that authorizes nothing. A lease is grouped
// by BARE EQUALITY across clients, so a colliding one silently fuses two
// clients into one attachment and bypasses the visible takeover SPEC.md's
// one-attached-client rule is built on — a wrong answer that looks like a
// working one, which is the case for failing closed.
//
// This test USED to wait for the terminal to come up after the create, and
// that is precisely what no longer happens: the wait was replaced with the
// lease refusal, which is the behavior change being pinned.
test("with no CSPRNG, a create still carries a key while the session view refuses to attach", async ({
  page,
  request,
}) => {
  const title = `intent-fallback-${Date.now()}`;
  const keys: (string | undefined)[] = [];
  // Defined away on the PROTOTYPE, where it actually lives: deleting an
  // own property of `crypto` would silently do nothing and the test would
  // pass while exercising the ordinary path.
  await page.addInitScript(() => {
    Object.defineProperty(Crypto.prototype, "randomUUID", {
      value: undefined,
      configurable: true,
    });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    keys.push(JSON.parse(route.request().postData() ?? "{}").intent_key);
    await route.continue();
  });

  try {
    await page.goto("/");
    expect(
      await page.evaluate(() => typeof (globalThis.crypto as any)?.randomUUID),
    ).not.toBe("function");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();

    // The create went through on the weak key, and landed in the session
    // view — the titlebar is the proof, since it renders from the created
    // session regardless of whether any terminal attaches.
    await expect(page.locator(".titlebar .title")).toHaveText(title, {
      timeout: 15_000,
    });
    expect(keys).toHaveLength(1);
    expect(keys[0]).toBeTruthy();

    // ...and the terminal deliberately did NOT: the refusal is visible,
    // names entropy as the reason, and — the part that actually matters —
    // no socket was opened. A view that degraded to a weak or empty lease
    // would attach here, and its terminals would then take each other
    // over the moment a second one existed.
    await expect(page.locator(".lease-error")).toBeVisible({ timeout: 15_000 });
    await expect(page.locator(".lease-error")).toContainText("high-entropy");
    expect(
      await page.evaluate(() => Object.keys((window as any).__farhelmIslands ?? {}).length),
    ).toBe(0);
    expect(await page.evaluate(() => (window as any).__farhelmTermReady)).toBeUndefined();
  } finally {
    await cleanUpSessionsTitled(request, title);
  }
});

// ---------------------------------------------------------------------
// Restart with resume, at the browser level (PLAN_M3.md item 9).
//
// SPEC.md's restart is a lifecycle operation with a UI contract of its
// own: an interrupted session's view leads with the resume offer,
// declining it changes nothing, a live session confirms first, and a
// reused terminal still shows the previous run above the new one. All
// four are below.
// ---------------------------------------------------------------------

// The interrupted state cannot be produced by driving this stack: it takes
// a host reboot (or the injected boot-id change the Rust suite uses), so
// the listing is intercepted exactly like the Unknown-status confirm test
// above does for the same reason. Everything else here is real — the
// component, its wording, and the fact that no request is sent.
//
// "Declining" has no control of its own by design (SPEC.md: opening an
// interrupted session OFFERS restart-with-resume; declining leaves it
// interrupted): the user simply does not click. So what this pins is that
// navigating away sends nothing and leaves the row exactly as it was —
// a restart affordance that fired on open, or on back, would be the bug.
test("an interrupted session's view leads with the resume offer, and declining changes nothing", async ({
  page,
}) => {
  const sessionId = "11111111-2222-3333-4444-555555555555";
  const title = `interrupted-offer-${Date.now()}`;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    // The real listing plus one interrupted row, so every other test's
    // session (and the shared "e2e-session") keeps coming through
    // untouched.
    const response = await route.fetch();
    const listing = await response.json();
    listing.sessions.push({
      id: sessionId,
      title,
      cwd: "/tmp",
      invocation: "claude",
      status: { state: "interrupted" },
      restart_offer: "resume",
    });
    listing.total += 1;
    await route.fulfill({ response, json: listing });
  });
  let restartRequests = 0;
  await page.route(`**/api/sessions/${sessionId}/restart`, async (route) => {
    restartRequests++;
    await route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
  });

  await page.goto("/");
  await rowByTitle(page, title).locator(".session-row-open").click();

  // The offer leads with WHY the terminal is gone and what restarting
  // would do to the conversation — both, because the user is being asked
  // to act on something they did not do.
  const offer = page.locator(".restart-offer-text");
  await expect(offer).toBeVisible();
  await expect(offer).toContainText("interrupted by a host reboot");
  await expect(offer).toContainText("resumes this session's own conversation");
  // The action names the offer, not the mechanism.
  await expect(page.locator(".restart-primary")).toHaveText("resume conversation");
  // An interrupted session has nothing running, so there is no confirm
  // step in front of it.
  await expect(page.locator(".restart-confirm")).toHaveCount(0);
  expect(restartRequests).toBe(0);

  // Declining: leave. Nothing was sent, and the row is still interrupted.
  await page.locator(".back-button").click();
  const row = rowByTitle(page, title);
  await expect(row.locator(".status-badge")).toHaveText("interrupted");
  expect(restartRequests).toBe(0);
});

// A live agent is the one case SPEC.md requires a confirmation for
// ("Restart on a session whose agent is still running confirms, stops the
// agent, then relaunches"), and the confirmation is in-page for the same
// reason delete's is: wry ships no native JS dialogs on macOS's WKWebView,
// where a `window.confirm()` would silently do nothing at all.
//
// Driven against a REAL session, so the request that finally goes out is
// the real one — including `stop_if_running`, which is the whole point of
// the confirmation and is asserted on the wire rather than assumed.
test("restarting a live session confirms first, and only then sends the request with consent", async ({
  page,
  request,
}) => {
  const title = `restart-confirm-${Date.now()}`;
  const bodies: any[] = [];
  await page.route("**/api/sessions/*/restart", async (route) => {
    bodies.push(route.request().postDataJSON());
    await route.continue();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");

    // Wait until the view's own status-derived decision says this click
    // will confirm rather than restart outright (`data-confirms`, set from
    // the session's status): the view opens on the create reply's
    // deliberate `Unknown` placeholder and refreshes once, so clicking
    // before that lands would exercise the stale-hint path instead of the
    // confirmation this test is about.
    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });

    // The first click only opens the prompt: nothing is sent, and the
    // consequence text says what restarting would do to the running agent.
    await restartButton.click();
    await expect(page.locator(".restart-offer .confirm-consequence")).toContainText(
      "still running",
    );
    expect(bodies).toHaveLength(0);

    // Cancel returns the view to its normal state, still having sent
    // nothing — the same "cancel is the only way back" rule the delete
    // prompt follows.
    await page.locator(".restart-cancel").click();
    await expect(page.locator(".restart-primary")).toBeVisible();
    expect(bodies).toHaveLength(0);

    await restartButton.click();
    await page.locator(".restart-confirm").click();
    await expect.poll(() => bodies.length).toBe(1);
    expect(bodies[0].stop_if_running).toBe(true);
    // The mode is the one the session's own offer authorizes — a
    // fake-agent session captures no conversation, so a fresh launch is
    // the only honest thing restart can offer it.
    expect(bodies[0].mode).toBe("fresh");

    // And the relaunch actually comes up. Counted rather than merely
    // matched: the reused terminal's scrollback still holds the FIRST
    // run's banner, so `toContain` would pass without the new run having
    // printed anything at all.
    await expect
      .poll(
        async () => (await termText(page)).split("FAKE-AGENT READY").length - 1,
        {
          timeout: 30_000,
          message: "the relaunched agent's own ready banner",
        },
      )
      .toBeGreaterThanOrEqual(2);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// SPEC.md: "Restart reuses the session's terminal when it still exists —
// the previous run's output stays in scrollback". Asserted from the
// BROWSER's own buffer after the restart's remount, which is the only view
// a user actually has of that promise: the buffer starts empty on remount,
// so everything in it afterwards came back through replay of the reused
// pane's scrollback.
//
// The marker is typed rather than taken from the startup banner, because
// both runs print the same banner — text only the FIRST run could have
// produced is what makes this about retention rather than about the new
// run having printed something.
test("a restarted session's terminal still shows the previous run above the new one", async ({
  page,
  request,
}) => {
  const title = `restart-scrollback-${Date.now()}`;
  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");

    await page.locator("#terminal").click();
    await page.keyboard.type("PRIOR-RUN-MARKER");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:PRIOR-RUN-MARKER");

    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });
    await restartButton.click();
    await page.locator(".restart-confirm").click();

    // Both facts at once, and in order: the prior run's typed marker is
    // still there, and the new run's banner is BELOW it. A plain
    // "contains both" would also pass if the restart had never happened
    // (the pre-restart buffer contains both too), so the anchor is the
    // marker's position relative to the LAST banner.
    await expect
      .poll(
        async () => {
          const text = await termText(page);
          const marker = text.indexOf("PRIOR-RUN-MARKER");
          const banner = text.lastIndexOf("FAKE-AGENT READY");
          return marker >= 0 && banner > marker;
        },
        {
          timeout: 30_000,
          message: "prior run's output above the relaunched agent's",
        },
      )
      .toBe(true);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// A restart whose RESPONSE is lost is not a restart that did not happen:
// the request reaches the supervisor, the agent is relaunched, and only
// the reply dies on the way back. The view has to recover from that on its
// own, because the server has already torn its attachment down — a client
// that treated the failure as "nothing happened" would leave the user
// staring at a permanently detached terminal for a session that is running
// perfectly well.
//
// `route.fetch()` then `route.abort()` reproduces exactly that: the real
// request is performed, and the page sees a network error instead of its
// answer.
test("a restart whose response is lost still recovers the terminal", async ({
  page,
  request,
}) => {
  const title = `restart-lost-reply-${Date.now()}`;
  await page.route("**/api/sessions/*/restart", async (route) => {
    await route.fetch();
    await route.abort("connectionfailed");
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");

    await page.locator("#terminal").click();
    await page.keyboard.type("BEFORE-LOST-RESTART");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:BEFORE-LOST-RESTART");

    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });
    await restartButton.click();
    await page.locator(".restart-confirm").click();

    // The failure is surfaced rather than swallowed — the user is owed
    // that much when their action's outcome is genuinely unknown to the
    // client.
    await expect(page.locator(".restart-error")).toBeVisible({ timeout: 15_000 });

    // ...and the view recovers anyway: it re-reads the session, remounts,
    // and the relaunched agent's own banner appears BELOW the previous
    // run's output in the reused terminal. A view that had concluded
    // "nothing happened" would sit detached here forever.
    await expect
      .poll(
        async () => {
          const text = await termText(page);
          const marker = text.indexOf("BEFORE-LOST-RESTART");
          const banner = text.lastIndexOf("FAKE-AGENT READY");
          return marker >= 0 && banner > marker;
        },
        {
          timeout: 30_000,
          message: "the relaunched agent's terminal, recovered after a lost reply",
        },
      )
      .toBe(true);

    // The session really was restarted, which is what makes the recovery
    // the correct behavior rather than a lucky one.
    const listing = await (await request.get("/api/sessions")).json();
    const session = listing.sessions.find((s: any) => s.title === title);
    expect(session).toBeTruthy();
    expect(session.status.state).toBe("alive");
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// MT-4 (manual testing): restarting a live session left the red "Detached:
// session restarted" banner painted over a terminal that had, by the time
// a human noticed it, already reattached and was working fine — typing
// round-tripped, output rendered, the banner just never went away. Root
// cause was `#term-banner` (farhelm-ui/src/lib.rs) living OUTSIDE the
// `#terminal` div terminal.js remounts, so nothing about a later mount
// ever told a PRIOR mount's sticky banner to clear (terminal.js's
// `showBanner` is deliberately sticky FOR THE LIFE OF ITS OWN SOCKET, so a
// takeover reason survives the generic close that follows it — see that
// function's docs — but nothing was clearing it for the NEXT socket
// either). The fix hooks the new socket's `onopen` — a transport-level
// signal (the upgrade completed), not proof the supervisor-side attach
// succeeded; clearing there is still honest because a failed attach
// closes that same socket and its own close handler re-banners.
//
// This is exactly the restart sequence that produces the bug: a live
// agent's restart tears the OLD attachment down with reason "session
// restarted" (`detach_for_restart`, farhelm-supervisor/src/service.rs)
// before the new one ever exists. Proving the banner APPEARED cannot be a
// locator poll — the fix clears it as soon as the new socket opens, which
// on loopback routinely beats Playwright's first poll, so the transient
// visible state is unobservable from outside (this test flaked exactly
// that way when it polled). A MutationObserver installed before the page
// loads records every banner transition instead, so the assertion reads
// the recorded history: shown with the exact restart reason, then hidden
// once the relaunch is confirmed live — the real sticky-then-clear
// sequence, with no window for the poll to miss.
test("a restarted session's banner clears once the new attachment is live", async ({
  page,
  request,
}) => {
  const title = `restart-banner-clears-${Date.now()}`;
  try {
    await page.addInitScript(() => {
      (window as any).__bannerLog = [];
      const arm = () => {
        const el = document.getElementById("term-banner");
        if (!el) {
          setTimeout(arm, 50);
          return;
        }
        new MutationObserver(() => {
          (window as any).__bannerLog.push({
            shown: el.style.display === "block",
            text: el.textContent,
          });
        }).observe(el, {
          attributes: true,
          attributeFilter: ["style"],
          childList: true,
          characterData: true,
          subtree: true,
        });
      };
      document.addEventListener("DOMContentLoaded", arm);
    });
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");

    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });
    await restartButton.click();
    await page.locator(".restart-confirm").click();

    // The banner's appearance is read from the observer's recorded
    // history (see the doc comment above for why a locator poll cannot
    // see it): the old attachment's detach must have painted the banner
    // with the restart's exact reason at SOME point, however briefly.
    await expect
      .poll(
        async () =>
          await page.evaluate(() =>
            (window as any).__bannerLog.some(
              (e: { shown: boolean; text: string }) =>
                e.shown && e.text.includes("Detached: session restarted"),
            ),
          ),
        { timeout: 15_000, message: "the restart's detach banner was recorded" },
      )
      .toBe(true);

    // The relaunch comes up in the SAME (reused) terminal, so its ready
    // banner is the SECOND occurrence in the buffer — the same anchor the
    // confirm test above uses to prove the new run actually printed
    // something rather than merely reattaching to stale output.
    await expect
      .poll(
        async () => (await termText(page)).split("FAKE-AGENT READY").length - 1,
        { timeout: 30_000, message: "the relaunched agent's own ready banner" },
      )
      .toBeGreaterThanOrEqual(2);

    // The bug: the OLD attachment's detach banner stayed painted over a
    // terminal that is now genuinely live again.
    await expect(page.locator("#term-banner")).toBeHidden({ timeout: 15_000 });

    // And "live" is proven functionally, not just by the banner's
    // absence: typing still round-trips through the new attachment.
    await page.locator("#terminal").click();
    await page.keyboard.type("post-restart-roundtrip");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:post-restart-roundtrip", 10_000);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// ---------------------------------------------------------------------
// Terminal tabs (PLAN_M4.md item 6).
//
// SPEC.md's session view "supports additional terminal tabs: plain shells
// spawned in the session's working directory". These drive the real strip
// against a real supervisor, a real tmux, and a real login shell — the
// fake agent is only ever the SESSION's agent here; every tab below runs
// the user's actual `$SHELL`, which is the whole point of the feature.
//
// Every test here works on its own session rather than the shared
// "e2e-session": a leftover tab would be visible to every test after it in
// this serially-run suite, and the per-project `beforeAll` reset is too
// coarse to catch that within a project's own pass.
// ---------------------------------------------------------------------

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
async function islandText(page: Page, elementId: string): Promise<string> {
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
async function waitForIslandMounted(page: Page, elementId: string) {
  await expect
    .poll(
      () => page.evaluate((el) => !!(window as any).__farhelmIslands?.[el], elementId),
      { timeout: 20_000, message: `waiting for the island at ${elementId} to mount` },
    )
    .toBe(true);
}

/**
 * Every mounted island's `?lease=` value, keyed by terminal — `agent` for
 * the agent terminal, the tab id for a tab.
 *
 * Read off each island's actual socket URL rather than from any UI state,
 * because the lease is a WIRE fact: what matters is what the supervisor
 * was told, not what the view believes it sent.
 */
async function islandLeases(page: Page): Promise<Record<string, string>> {
  return page.evaluate(() => {
    const out: Record<string, string> = {};
    const islands = (window as any).__farhelmIslands ?? {};
    for (const el of Object.keys(islands)) {
      const url = new URL(islands[el].ws.url);
      const key = el === "terminal" ? "agent" : el.replace(/^terminal-/, "");
      out[key] = url.searchParams.get("lease") ?? "";
    }
    return out;
  });
}

/** `waitForTermText`, addressed at one island rather than the agent's. */
async function waitForIslandText(
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
 * The working directories `createTabSession` minted, removed once the file
 * is done with them.
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
test.afterAll(() => {
  for (const dir of tabSessionDirs) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
  tabSessionDirs.length = 0;
});

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
async function createTabSession(
  request: APIRequestContext,
  title: string,
): Promise<{ id: string; cwd: string }> {
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), "fh-tabs-"));
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
async function addTab(page: Page, previous: number): Promise<string> {
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
async function selectTerminal(page: Page, terminal: string) {
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
async function runInShell(
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
function shellMarker(marker: string): { command: string; expected: string } {
  return { command: `sh -c 'echo "${marker}-$((6*7))"'`, expected: `${marker}-42` };
}

/**
 * A snapshot of a live process's identity: its pid plus the boot-relative
 * start time Linux records for it.
 *
 * The start time is not decoration. A bare `kill(pid, 0)` answers "does
 * SOME process have this pid", which is a different question from "is the
 * process I was watching still running" — pids are recycled, and on a busy
 * host the shell this test killed can be replaced by an unrelated process
 * wearing the same number well inside the poll window. The supervisor
 * itself refuses to signal on a pid whose start time does not match (see
 * `signal_validated` in its own tests); this mirrors that discipline
 * rather than inventing a weaker one for the test suite.
 */
interface ProcessIdentity {
  pid: number;
  startTime: string;
}

/**
 * Read `/proc/<pid>/stat`, returning the process's state character and its
 * start time, or `undefined` if it is gone.
 *
 * The comm field (field 2) is parenthesized and may itself contain spaces
 * and parentheses, so the split starts after the LAST `)` — the standard
 * way to parse this file, and not optional: a shell whose argv made its
 * comm contain a space would otherwise shift every field after it.
 */
function readProcStat(pid: number): { state: string; startTime: string } | undefined {
  let raw: string;
  try {
    raw = fs.readFileSync(`/proc/${pid}/stat`, "utf8");
  } catch {
    return undefined;
  }
  const after = raw.slice(raw.lastIndexOf(")") + 2).split(" ");
  // Fields are 1-based in proc(5): state is 3 and starttime is 22, so
  // after dropping pid and comm they sit at indices 0 and 19.
  return { state: after[0], startTime: after[19] };
}

/** The identity of a process that must be running right now. */
function processIdentity(pid: number): ProcessIdentity {
  const stat = readProcStat(pid);
  expect(stat, `process ${pid} must exist`).toBeDefined();
  return { pid, startTime: stat!.startTime };
}

/**
 * Whether the process `identity` names is still running — reaped and
 * recycled both count as gone.
 *
 * A ZOMBIE counts as gone on purpose: SPEC.md's promise is that closing a
 * tab kills the shell, and a zombie is a dead shell whose parent has not
 * collected its status yet. Treating it as alive would fail this suite for
 * a kill that fully succeeded, which is a test bug dressed as a product
 * bug. A start time that no longer matches counts as gone for the opposite
 * reason: the pid is live, but it is not the process we killed.
 */
function pidAlive(identity: ProcessIdentity): boolean {
  const stat = readProcStat(identity.pid);
  if (!stat) return false;
  if (stat.state === "Z") return false;
  return stat.startTime === identity.startTime;
}

// The headline tab contract, end to end: a tab is a real shell in the
// SESSION's working directory (SPEC.md, and PLAN_M4.md acceptance 1), and
// it is a genuinely separate terminal — the agent's own pane must be
// untouched by anything typed into it. Both halves are asserted, because a
// tab wired to the agent's pane would pass the first on its own.
test("a tab runs a real shell in the session's working directory, leaving the agent terminal alone", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `tab-shell-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");

    const tabId = await addTab(page, 0);
    const element = `terminal-${tabId}`;
    // Opening a tab selects it, so its pane is the visible one already;
    // this asserts that rather than assuming it.
    await expect(
      page.locator(`.terminal-pane[data-terminal="${tabId}"]`),
    ).toBeVisible();
    await waitForIslandMounted(page, element);

    // `$PWD` is what makes this about the SHELL's directory rather than
    // about the echo: the typed line carries the variable, only the
    // expansion carries the path (see `runInShell`).
    await runInShell(page, element, "echo \"TAB-CWD:$PWD\"", `TAB-CWD:${session.cwd}`);

    // The agent terminal is a different terminal, not a second view of the
    // same one: nothing typed into the tab may show up in it.
    expect(await termText(page)).not.toContain("TAB-CWD:");
    // ...and it is still live underneath, which is what makes the absence
    // above meaningful rather than merely a dead pane.
    await selectTerminal(page, "agent");
    await page.locator("#terminal").click();
    await page.keyboard.type("agent-still-live");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:agent-still-live", 15_000);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The strip's own contract: labels are positional and one-based
// (PLAN_M4.md item 6 — SPEC.md gives tabs no names, so this IS the naming
// rule), the agent terminal comes first, and it carries no close
// affordance at all. "No close affordance" is asserted structurally — the
// agent tab is not one of the closable slots, and the number of close
// buttons equals the number of TABS — rather than by looking for a
// disabled control, because an unclosable agent terminal means the button
// is absent, not merely inert.
test("the strip labels tabs positionally and gives the agent terminal no close control", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `tab-labels-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    await expect(page.locator(".tab-strip .tab-agent")).toHaveText("agent");
    await expect(page.locator(".tab-slot")).toHaveCount(0);
    await expect(page.locator(".tab-close")).toHaveCount(0);

    await addTab(page, 0);
    await addTab(page, 1);

    await expect(page.locator(".tab-slot .tab")).toHaveText([
      "Terminal 1",
      "Terminal 2",
    ]);
    // One close control per TAB, and the agent tab is not inside a slot —
    // together these say the agent terminal cannot be closed from here.
    await expect(page.locator(".tab-close")).toHaveCount(2);
    await expect(page.locator(".tab-slot .tab-agent")).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md's durability promise, extended to tabs: "Tabs survive client
// disconnects and supervisor restarts exactly like the agent terminal."
// A reload is the harshest client-side form of it — a brand-new page with
// empty buffers, so everything on screen afterwards came from replay.
//
// The reattached tab is checked while it is still HIDDEN, before anything
// selects it, which pins the other half of PLAN_M4.md item 6: every open
// tab attaches concurrently rather than on selection. A view that attached
// on select would show an empty buffer here and only fill it after the
// click below.
test("a tab survives a reload, reattaching with its scrollback while still unselected", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-reload-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    const tabId = await addTab(page, 0);
    const element = `terminal-${tabId}`;
    const before = shellMarker("BEFORE-RELOAD");
    await runInShell(page, element, before.command, before.expected);

    await page.reload();
    // A reload resets the app's navigation state, so it lands on the list
    // again — the same round trip the agent-terminal reload test makes.
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // The tab is listed again from the server's own rediscovery, not from
    // anything this client remembered across the reload.
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    await expect(page.locator(".tab-slot")).toHaveAttribute("data-tab-id", tabId);
    // Still on the agent terminal: selection is not persisted, which is
    // what makes the assertion below one about a HIDDEN, attached tab.
    await expect(page.locator(`.terminal-pane[data-terminal="${tabId}"]`)).toBeHidden();
    await waitForIslandText(page, element, before.expected, 30_000);

    // And it is genuinely live once shown, not just replaying history.
    await selectTerminal(page, tabId);
    const after = shellMarker("AFTER-RELOAD");
    await runInShell(page, element, after.command, after.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Close is the whole per-tab operation set in v1, and SPEC.md makes it a
// kill: "A tab can be closed individually, which kills that shell and its
// processes." The confirmation in front of it is in-page for the same
// reason delete's and restart's are (wry ships no native JS dialogs on
// macOS's WKWebView, where `window.confirm()` silently does nothing).
//
// Three things are pinned, in order of how badly a regression in each
// would hurt: cancel sends nothing, confirm actually kills the shell
// process (checked against the pid the shell itself printed, not merely
// against the UI's own state), and the tab is gone from the server's tab
// list afterwards.
test("closing a tab confirms in-page, then kills its shell and drops it from the session", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-close-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    const tabId = await addTab(page, 0);
    const element = `terminal-${tabId}`;
    // `sh -c` for `shellMarker`'s portability reason, and `$PPID` rather
    // than `$$` BECAUSE of it: inside the `sh` this spawns, `$$` would be
    // that short-lived child, while the parent is the tab's own login
    // shell — which is the process SPEC.md's "closing kills that shell"
    // is about. A regex, not the literal prefix, because the interactive
    // shell echoes the command line first and only the expanded digits
    // prove anything ran (see `runInShell`).
    await runInShell(page, element, "sh -c 'echo TAB-PID:$PPID'", /TAB-PID:\d+/);
    const pidMatch = (await islandText(page, element)).match(/TAB-PID:(\d+)/);
    expect(pidMatch, "the tab's shell must report its own pid").toBeTruthy();
    const shell = processIdentity(Number(pidMatch![1]));
    expect(pidAlive(shell)).toBe(true);

    // Cancel first: the prompt is the only thing the × does, and backing
    // out of it must leave the tab (and its shell) exactly as they were.
    await page.locator(".tab-close").click();
    await expect(page.locator(".tab-confirm .confirm-consequence")).toContainText(
      "kills this terminal's shell",
    );
    await expect(page.locator(".tab-confirm .confirm-title")).toHaveText("Terminal 1");
    await page.locator(".tab-confirm .confirm-cancel").click();
    await expect(page.locator(".tab-confirm")).toHaveCount(0);
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    expect(pidAlive(shell)).toBe(true);

    await page.locator(".tab-close").click();
    await page.locator(".confirm-close-tab").click();

    // Gone from the strip, and the view falls back to the agent terminal
    // rather than leaving a selection pointing at nothing.
    //
    // The generous timeout is not slack for a slow assertion, it is the
    // shape of the operation: the strip only drops the tab once the DELETE
    // returns, and that reply waits on the whole tab-scoped reap — M2's
    // stop ordering, which walks the process tree, quiesces with a grace
    // period, kills, and re-enumerates (up to `MAX_QUIESCE_PASSES` times),
    // plus a systemd scope teardown where a user manager exists. Several
    // seconds is a NORMAL close on a loaded host, so Playwright's 5s
    // default would make this a timing test rather than a behavior one.
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 30_000 });
    await expect(page.locator('.terminal-pane[data-terminal="agent"]')).toBeVisible();

    // The kill is the real contract, so it is checked against the OS, not
    // against the UI. Polled rather than asserted once: the close reply
    // only comes after the reap, but the process table is a separate
    // observer with its own timing.
    await expect
      .poll(() => pidAlive(shell), {
        timeout: 15_000,
        message: "closing a tab must kill its shell",
      })
      .toBe(false);

    // ...and the server agrees the tab is gone, which is what a client
    // opening this session later will see.
    const detail = await (await request.get(`/api/sessions/${id}`)).json();
    expect(detail.tabs ?? []).toEqual([]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md's changes-appear-automatically rule, on the tab list: the
// session view polls the session DETAIL at the list's own cadence
// (PLAN_M4.md item 6), so a tab opened from another client shows up
// without a reload. The HTTP API stands in for "another client" the same
// way the list-poll test above uses it for a session created elsewhere.
//
// Bounded at ~15s — comfortably above the 3s poll interval — so a
// regression to "never polls the detail" fails rather than hangs.
test("a tab opened from another client appears in the strip without a reload", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `tab-poll-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await expect(page.locator(".tab-slot")).toHaveCount(0);

    const opened = await request.post(`/api/sessions/${id}/tabs`);
    expect(opened.status(), await opened.text()).toBe(200);
    const tabId = (await opened.json()).tab.id;

    await expect(page.locator(".tab-slot")).toHaveCount(1, { timeout: 15_000 });
    await expect(page.locator(".tab-slot")).toHaveAttribute("data-tab-id", tabId);
    await expect(page.locator(".tab-slot .tab")).toHaveText("Terminal 1");
    // Discovered through polling, and then actually attached and live — a
    // strip entry with no working terminal behind it would be a worse bug
    // than not showing the tab at all.
    await waitForIslandMounted(page, `terminal-${tabId}`);
    await selectTerminal(page, tabId);
    const marker = shellMarker("POLLED-TAB");
    await runInShell(page, `terminal-${tabId}`, marker.command, marker.expected);

    // The counterpart: a tab closed from another client leaves the strip
    // the same way, with no reload and nothing left pointing at it.
    const closed = await request.delete(`/api/sessions/${id}/tabs/${tabId}`);
    expect(closed.status(), await closed.text()).toBe(200);
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 15_000 });
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// PLAN_M4.md item 3's session-scoped takeover, seen from the browser.
// SPEC.md's one-attached-client rule is per SESSION, not per terminal: a
// second view of the same session takes over ALL of the first view's
// terminals at once, because every terminal a view opens carries that
// view's single lease.
//
// This is the deliberate semantic change the lease introduces, so it is
// pinned on BOTH terminals rather than just the agent's — a build that
// leased only some of its terminals would still detach the agent here and
// look correct. Each losing terminal banners its own detach (the protocol
// sends one `Detached` per channel, with no session-wide message).
//
// The MECHANISM is asserted too, not just the outcome: the two terminals
// of one view must carry the SAME non-empty lease and the second view a
// different one. Without that, a build that simply failed to reuse leases
// would detach everything here for the wrong reason (each terminal taking
// over the last) and pass on the banners alone.
test("a second view of the same session detaches every terminal of the first", async ({
  browser,
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-takeover-${Date.now()}`;
  let id: string | undefined;
  let second: Awaited<ReturnType<typeof browser.newContext>> | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    const tabId = await addTab(page, 0);
    // A REAL attach signal, not merely a mounted island: terminal.js
    // publishes an island at the end of `mount()`, which is before the
    // socket's `onopen` and well before the supervisor-side attach it
    // triggers. Opening the second view against a first view whose tab had
    // not actually attached yet would test nothing — there would be no
    // second attachment to displace. A command the tab's own shell answers
    // is proof the whole path is up.
    const first = shellMarker("TAKEOVER-FIRST");
    await selectTerminal(page, tabId);
    await runInShell(page, `terminal-${tabId}`, first.command, first.expected);

    // Both of this view's terminals must be attached under ONE lease —
    // that shared identity is what makes the takeover below session-scoped
    // rather than per-terminal.
    const firstLeases = await islandLeases(page);
    expect(firstLeases.agent, "the agent terminal must carry a lease too").toBeTruthy();
    expect(firstLeases[tabId]).toBe(firstLeases.agent);

    second = await browser.newContext();
    const page2 = await second.newPage();
    await page2.goto("/");
    // Playwright's `use.baseURL` reaching a MANUALLY created context is
    // load-bearing for the line above and easy to assume wrongly, so it is
    // checked rather than trusted: the two pages must have resolved "/"
    // against the same origin. If a future Playwright stopped applying
    // config options to `browser.newContext()`, this fails here with a
    // clear reason instead of somewhere downstream as a mysterious
    // navigation.
    expect(new URL(page2.url()).origin).toBe(new URL(page.url()).origin);
    await page2.locator(`[data-session-id="${id}"]`).click();
    await page2.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // Both of the first view's terminals lost their attachment. Asserted
    // on text rather than visibility: only one pane is on screen at a
    // time, and the hidden one's banner is exactly as real (and as
    // load-bearing when the user switches back) as the visible one's.
    const agentBanner = page.locator('.terminal-pane[data-terminal="agent"] .banner');
    const tabBanner = page.locator(`.terminal-pane[data-terminal="${tabId}"] .banner`);
    await expect(agentBanner).toContainText("Detached", { timeout: 15_000 });
    await expect(tabBanner).toContainText("Detached", { timeout: 15_000 });

    // A DIFFERENT client, per the lease: same session, new view instance,
    // new identity. Equal leases would have been an ordinary reconnect
    // (farhelm-proto's `DETACH_REASON_REPLACED`), not the takeover this
    // test is named for.
    const secondLeases = await islandLeases(page2);
    expect(secondLeases.agent).toBeTruthy();
    expect(secondLeases.agent).not.toBe(firstLeases.agent);

    // The winner holds BOTH terminals, live. The agent terminal is
    // asserted as well as the tab: a takeover that handed over only the
    // terminal the winner happened to look at first would be a
    // half-transferred session, which is precisely what session-scoped
    // ownership is supposed to rule out.
    await selectTerminal(page2, tabId);
    const winner = shellMarker("TAKEOVER-WINNER");
    await runInShell(page2, `terminal-${tabId}`, winner.command, winner.expected);
    await selectTerminal(page2, "agent");
    await page2.locator("#terminal").click();
    await page2.keyboard.type("takeover-agent-live");
    await page2.keyboard.press("Enter");
    await waitForTermText(page2, "echo:takeover-agent-live", 15_000);
  } finally {
    if (second) await second.close();
    if (id) await cleanupSession(request, id);
  }
});

/**
 * Open a session in `page` with `count` tabs already added, returning the
 * session and its tab ids in strip order.
 *
 * Adds them through the UI rather than the API because that is also how a
 * user gets here, and because the returned ids are read back out of the
 * rendered strip either way (see `addTab`).
 */
async function openSessionWithTabs(
  page: Page,
  request: APIRequestContext,
  title: string,
  count: number,
): Promise<{ id: string; cwd: string; tabs: string[] }> {
  const session = await createTabSession(request, title);
  await page.goto("/");
  await page.locator(`[data-session-id="${session.id}"]`).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  const tabs: string[] = [];
  for (let i = 0; i < count; i++) {
    const id = await addTab(page, i);
    await waitForIslandMounted(page, `terminal-${id}`);
    tabs.push(id);
  }
  return { ...session, tabs };
}

/** The element ids terminal.js currently has mounted, sorted. */
async function mountedIslands(page: Page): Promise<string[]> {
  return page.evaluate(() => Object.keys((window as any).__farhelmIslands ?? {}).sort());
}

/**
 * THE takeover-reclaim contract (the reason terminal.js has a latch at
 * all), driven with two real clients against the real stack.
 *
 * The bug this pins is an eviction loop, and it is worth stating plainly
 * because the fix looks like mere politeness otherwise: a displaced view
 * keeps polling the session detail, so it LEARNS about a tab the winner
 * opens. Without a latch it hands that tab to `sync()`, which attaches it
 * under the displaced view's still-valid lease — and the supervisor,
 * seeing a different lease, detaches the winner. The user who lost the
 * session silently steals it back, triggered by the winner doing nothing
 * more provocative than opening a terminal. That inverts SPEC.md's
 * one-attached-client rule rather than enforcing it.
 *
 * So this asserts the negative (A discovers the tab and does NOT attach
 * it, and B keeps working) and then the positive (A's explicit "take
 * control" reattaches everything and displaces B) — the second half
 * matters because a latch that could not be released would just be a
 * different way to lose the session.
 */
test("a displaced view discovers the winner's new tab without attaching it, until take control", async ({
  browser,
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-reclaim-${Date.now()}`;
  let id: string | undefined;
  let second: Awaited<ReturnType<typeof browser.newContext>> | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [firstTab] = session.tabs;
    const alive = shellMarker("RECLAIM-A-ALIVE");
    await runInShell(page, `terminal-${firstTab}`, alive.command, alive.expected);

    // B takes the session.
    second = await browser.newContext();
    const page2 = await second.newPage();
    await page2.goto("/");
    await page2.locator(`[data-session-id="${id}"]`).click();
    await page2.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await expect(
      page.locator('.terminal-pane[data-terminal="agent"] .banner'),
    ).toContainText("Detached", { timeout: 15_000 });

    // B opens a second tab, through its own UI — the ordinary thing a
    // session's owner does, and the trigger for the whole bug.
    const secondTab = await addTab(page2, 1);
    await waitForIslandMounted(page2, `terminal-${secondTab}`);
    const winnerTab = shellMarker("RECLAIM-B-TAB");
    await runInShell(page2, `terminal-${secondTab}`, winnerTab.command, winnerTab.expected);

    // A's poll sees the new tab: it renders in A's strip, which is the
    // premise of the bug — A cannot attach what it never learned about.
    await expect(page.locator(".tab-slot")).toHaveCount(2, { timeout: 15_000 });

    // ...and A did NOT attach it. Both halves are checked, because either
    // alone could pass for the wrong reason: A holds no island for that
    // tab, and B — which a reattach would have evicted — is still live.
    expect(await mountedIslands(page)).not.toContain(`terminal-${secondTab}`);
    const stillB = shellMarker("RECLAIM-B-STILL");
    await runInShell(page2, `terminal-${secondTab}`, stillB.command, stillB.expected);
    await expect(
      page2.locator(`.terminal-pane[data-terminal="${secondTab}"] .banner`),
    ).toBeHidden();

    // The discovered-but-unattached tab says so rather than showing an
    // unexplained blank pane.
    await selectTerminal(page, secondTab);
    await expect(
      page.locator(`.terminal-pane[data-terminal="${secondTab}"] .banner`),
    ).toContainText("Detached");

    // Take control: an explicit act, in the banner where the loss was
    // reported. A must come back with EVERY terminal — including the one
    // it only ever saw listed — and B must lose them, which is the same
    // visible takeover B performed a moment ago.
    await page.locator(`.terminal-pane[data-terminal="${secondTab}"] .banner-reclaim`).click();
    await expect
      .poll(() => mountedIslands(page), {
        timeout: 20_000,
        message: "take control must reattach every terminal, the newly discovered tab included",
      })
      .toEqual(["terminal", `terminal-${firstTab}`, `terminal-${secondTab}`].sort());
    const reclaimed = shellMarker("RECLAIM-A-BACK");
    await runInShell(page, `terminal-${secondTab}`, reclaimed.command, reclaimed.expected);
    await expect(
      page2.locator('.terminal-pane[data-terminal="agent"] .banner'),
    ).toContainText("Detached", { timeout: 15_000 });
  } finally {
    if (second) await second.close();
    if (id) await cleanupSession(request, id);
  }
});

// PLAN_M4.md item 3's isolation claim, at the browser level: per-terminal
// flow control is what makes it safe to leave background tabs attached, so
// a viewer that stops draining ONE tab must pause only that tab.
//
// Driven with a real producer inside a real tab (the fake agent's flood
// script, run as an ordinary command — which is what a tab being a plain
// shell buys) and a write patch scoped to that one xterm instance, so the
// stall is genuinely one terminal's and not a page-wide freeze. The agent
// terminal and the sibling tab are proven unaffected by INPUT round trips,
// not merely by their counters staying zero: a page that had wedged
// entirely would also report zero pauses everywhere.
test("stalling one tab's writes pauses only that tab; the agent and a sibling stay live", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-isolation-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [stalled, sibling] = session.tabs;

    // Swallow completion callbacks for ONE island's terminal only. Applied
    // after mount (unlike the page-wide patches earlier in this file)
    // precisely so it can be scoped by instance identity — the producer
    // below has not started yet, so nothing is missed by patching late.
    await page.evaluate((el) => {
      const target = (window as any).__farhelmIslands[el].term;
      const real = (window as any).Terminal.prototype.write;
      (window as any).__heldWrites = [];
      (window as any).Terminal.prototype.write = function (data: unknown, cb?: () => void) {
        if (this === target && cb) {
          return real.call(this, data, () => (window as any).__heldWrites.push(cb));
        }
        return real.call(this, data, cb);
      };
      (window as any).__realWrite = real;
    }, `terminal-${stalled}`);

    // A tab is a plain shell, so the suite's own flood fixture runs in it
    // as an ordinary command — no special support needed on either side.
    await selectTerminal(page, stalled);
    await page.locator(`[id="terminal-${stalled}"]`).click();
    await page.keyboard.type(`${FLOOD_AGENT_COMMAND}`);
    await page.keyboard.press("Enter");

    await expect
      .poll(
        () =>
          page.evaluate(
            (el) => (window as any).__farhelmIslands[el].test.pauseCount,
            `terminal-${stalled}`,
          ),
        { timeout: 60_000, message: "the stalled tab must cross HIGH_WATER and pause" },
      )
      .toBeGreaterThanOrEqual(1);

    // The other two terminals never paused...
    const others = await page.evaluate(
      (els) =>
        els.map((el: string) => (window as any).__farhelmIslands[el].test.pauseCount),
      ["terminal", `terminal-${sibling}`],
    );
    expect(others).toEqual([0, 0]);
    // ...and, more to the point, still work. This is the assertion that
    // would fail if one wedged terminal had pinned the whole connection.
    await selectTerminal(page, "agent");
    await page.locator("#terminal").click();
    await page.keyboard.type("isolation-agent");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:isolation-agent", 20_000);
    const live = shellMarker("ISOLATION-SIBLING");
    await selectTerminal(page, sibling);
    await runInShell(page, `terminal-${sibling}`, live.command, live.expected);

    // Releasing the held callbacks drains the backlog and the paused tab
    // resumes on its own — the other half of the watermark contract.
    await page.evaluate(() => {
      (window as any).Terminal.prototype.write = (window as any).__realWrite;
      const held = (window as any).__heldWrites;
      (window as any).__heldWrites = [];
      for (const cb of held) cb();
    });
    await expect
      .poll(
        () =>
          page.evaluate(
            (el) => (window as any).__farhelmIslands[el].test.resumeCount,
            `terminal-${stalled}`,
          ),
        { timeout: 60_000, message: "draining must resume the paused tab" },
      )
      .toBeGreaterThanOrEqual(1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The multi-island version of the navigation-lifecycle test near the top of
// this file: leaving a session with tabs open must tear down EVERY island,
// not just the agent's, and reopening must build genuinely new ones.
//
// Asserted at depth (registry empty, every socket actually CLOSED, fresh
// objects afterwards) rather than by the agent's singletons alone, because
// a per-island teardown that missed the tabs would leave the agent's
// globals looking perfectly clean while orphaned tab sockets kept the
// supervisor's attachments alive behind the user's back.
test("leaving a session with tabs tears down every island and reopening builds new ones", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-teardown-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;

    const before = await page.evaluate(() => {
      const islands = (window as any).__farhelmIslands;
      (window as any).__sockets = Object.keys(islands).map((el) => islands[el].ws);
      (window as any).__terms = Object.keys(islands).map((el) => islands[el].term);
      return Object.keys(islands).sort();
    });
    expect(before).toEqual(
      ["terminal", `terminal-${session.tabs[0]}`, `terminal-${session.tabs[1]}`].sort(),
    );

    await page.locator(".back-button").click();
    await expect(page.locator(".session-list")).toBeVisible();

    expect(await mountedIslands(page)).toEqual([]);
    await expect
      .poll(() =>
        page.evaluate(() => (window as any).__sockets.map((ws: WebSocket) => ws.readyState))
      )
      // 3 is CLOSED; there is no browser `WebSocket` global in this
      // Node-side context to name the constant by.
      .toEqual([3, 3, 3]);

    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await expect(page.locator(".tab-slot")).toHaveCount(2);
    await expect
      .poll(() => mountedIslands(page), { timeout: 20_000 })
      .toEqual(before);
    // Genuinely new instances, not the old ones somehow surviving: the
    // same "fresh, not reused" property the single-terminal test pins,
    // extended across every island.
    expect(
      await page.evaluate(() => {
        const islands = (window as any).__farhelmIslands;
        const old = (window as any).__terms;
        return Object.keys(islands).every((el) => !old.includes(islands[el].term));
      }),
    ).toBe(true);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Restart touches the AGENT terminal alone (SPEC.md; the supervisor's
// `detach_for_restart` is scoped to it), and the UI has to respect that
// scope or it undoes the guarantee: rebuilding a tab's island would tear
// down an attachment the restart never touched, costing a full replay and
// interrupting a shell that was minding its own business.
//
// Pinned by IDENTITY, not by appearance: the tab's socket object must be
// the very same one after the restart, and it must still be interactive.
test("restarting the agent rebuilds only the agent island; a tab keeps its socket", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-restart-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [tabId] = session.tabs;
    const before = shellMarker("RESTART-TAB-BEFORE");
    await runInShell(page, `terminal-${tabId}`, before.command, before.expected);

    await page.evaluate((el) => {
      const islands = (window as any).__farhelmIslands;
      (window as any).__tabWs = islands[el].ws;
      (window as any).__agentWs = islands["terminal"].ws;
    }, `terminal-${tabId}`);

    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 20_000,
    });
    await restartButton.click();
    await page.locator(".restart-confirm").click();

    // The agent's socket really was replaced (otherwise "the tab's was
    // not" would be a claim about a restart that never remounted
    // anything).
    await expect
      .poll(
        () => page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws !== (window as any).__agentWs),
        { timeout: 60_000, message: "the restart must rebuild the agent island" },
      )
      .toBe(true);

    // ...and the tab's was not: same object, still open, still answering.
    expect(
      await page.evaluate(
        (el) => (window as any).__farhelmIslands[el].ws === (window as any).__tabWs,
        `terminal-${tabId}`,
      ),
    ).toBe(true);
    await selectTerminal(page, tabId);
    const after = shellMarker("RESTART-TAB-AFTER");
    await runInShell(page, `terminal-${tabId}`, after.command, after.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Selecting a tab must put the keyboard where the user is looking, without
// them having to click into the terminal first — otherwise every switch
// costs an extra click and the first keystroke after a switch goes nowhere.
//
// Typed with `page.keyboard` straight after the strip click, with no click
// into the pane at all: that is the whole point, and it only works because
// terminal.js moves focus as part of applying the selection.
test("selecting a terminal focuses it, so typing works without clicking the pane", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-focus-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [tabId] = session.tabs;

    await selectTerminal(page, "agent");
    await page.keyboard.type("focus-agent");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:focus-agent", 20_000);

    await selectTerminal(page, tabId);
    await page.keyboard.type("sh -c 'echo FOCUS-TAB-$((6*7))'");
    await page.keyboard.press("Enter");
    await waitForIslandText(page, `terminal-${tabId}`, "FOCUS-TAB-42", 30_000);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// One island failing to mount must not take its siblings down with it —
// with tabs, an exception during a mount would otherwise leave a session
// view with no terminals at all instead of one broken one.
//
// The failure is injected at `WebSocket` construction for exactly one
// path (the same technique the single-terminal rollback test uses, made
// selective), which also exercises the rollback: the failed island must
// leave nothing registered and must say why in its own banner.
test("a tab whose mount fails is rolled back and bannered while its siblings stay live", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-mount-fail-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    // Both tabs are opened through the API first, so both exist before the
    // page ever loads and BOTH mount in the same `sync()` — which is what
    // makes "the sibling survived" a statement about that sync rather than
    // about two unrelated mounts at different times.
    const doomed = (await (await request.post(`/api/sessions/${id}/tabs`)).json()).tab.id;
    const healthy = (await (await request.post(`/api/sessions/${id}/tabs`)).json()).tab.id;

    await page.goto("/");
    await page.addInitScript((bad) => {
      const Real = window.WebSocket;
      const Shim = function (url: string, protocols?: any) {
        if (String(url).includes(`tab=${bad}`)) {
          throw new Error("injected failure for one island");
        }
        return new Real(url, protocols);
      } as unknown as typeof WebSocket;
      Shim.prototype = Real.prototype;
      // The readyState CONSTANTS have to come along, and forgetting them
      // is not a cosmetic omission: terminal.js gates every send on
      // `ws.readyState === WebSocket.OPEN`, which against a shim missing
      // `OPEN` compares to `undefined` and silently swallows all input.
      // The healthy siblings this test is about would then look attached
      // and answer nothing — a test artifact indistinguishable from the
      // product bug it is meant to rule out.
      for (const key of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
        (Shim as any)[key] = (Real as any)[key];
      }
      window.WebSocket = Shim;
    }, doomed);
    await page.reload();
    await page.locator(`[data-session-id="${id}"]`).click();

    // The failed island rolled back: nothing registered under its id...
    await expect
      .poll(() => mountedIslands(page), {
        timeout: 30_000,
        message: "the healthy terminals must mount even though a sibling threw",
      })
      .toEqual(["terminal", `terminal-${healthy}`].sort());
    // ...and it says so where the user is looking, rather than showing an
    // unexplained blank pane.
    await expect(
      page.locator(`.terminal-pane[data-terminal="${doomed}"] .banner`),
    ).toContainText("Failed to start terminal");

    // The siblings are not merely mounted but working.
    await waitForTermText(page, "FAKE-AGENT READY", 30_000);
    const live = shellMarker("MOUNT-FAIL-SIBLING");
    await selectTerminal(page, healthy);
    await runInShell(page, `terminal-${healthy}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// terminal.js keys its pending mounts by element id so one terminal
// waiting on xterm's globals cannot hold up another — and so a wait for a
// terminal that has since LEFT the desired set is cancelled rather than
// left to fire into a view that no longer wants it. That second property
// is what this pins: the M2-era "stale mount retry" bug, in its per-tab
// form.
//
// `window.Terminal` is withheld to force a genuinely pending mount (an
// unloaded box otherwise resolves the first readiness check immediately),
// exactly as the single-terminal regression test does.
test("a pending tab mount is cancelled when the tab leaves the desired set", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-pending-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // Withheld AFTER the agent mounted, so only the tab below is left
    // pending — the agent terminal staying up is what lets this test keep
    // observing the page at all.
    await page.evaluate(() => {
      (window as any).__stashedTerminal = (window as any).Terminal;
      delete (window as any).Terminal;
    });

    const opened = await request.post(`/api/sessions/${id}/tabs`);
    const tabId = (await opened.json()).tab.id;
    await expect(page.locator(".tab-slot")).toHaveCount(1, { timeout: 15_000 });
    expect(await mountedIslands(page)).toEqual(["terminal"]);

    // Gone again before it could ever mount.
    const closed = await request.delete(`/api/sessions/${id}/tabs/${tabId}`);
    expect(closed.ok()).toBe(true);
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 20_000 });

    // Restoring the global would let any surviving retry fire. Nothing
    // may: the pending attempt was cancelled when the tab left the set,
    // and a mount now would attach a terminal the supervisor destroyed.
    await page.evaluate(() => {
      (window as any).Terminal = (window as any).__stashedTerminal;
      delete (window as any).__stashedTerminal;
    });
    await expect
      .poll(() => mountedIslands(page), { timeout: 5_000, intervals: [500, 500, 500, 500] })
      .toEqual(["terminal"]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The add control's re-entry guard: a slow open must not become several
// tabs because the user pressed the button again. Asserted on the WIRE
// (how many POSTs left the browser), which is the only place the answer is
// unambiguous — counting rendered tabs afterwards would be satisfied by a
// UI that merely deduplicated its own mistake.
test("repeated activations while an open is in flight produce exactly one tab", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-reentry-${Date.now()}`;
  let id: string | undefined;
  let posts = 0;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.route(`**/api/sessions/${session.id}/tabs`, async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      posts++;
      // Long enough that the extra activations below land while the first
      // request is genuinely still open.
      await new Promise((resolve) => setTimeout(resolve, 2_000));
      await route.continue();
    });

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // Three activations in ONE synchronous turn, so they all land before
    // any re-render can disable the control — which is the window the
    // signal-level guard exists for.
    await page.evaluate(() => {
      const add = document.querySelector(".tab-add") as HTMLButtonElement;
      add.click();
      add.click();
      add.click();
    });

    await expect(page.locator(".tab-slot")).toHaveCount(1, { timeout: 30_000 });
    expect(posts, "one intended open must send one request").toBe(1);
    // The control comes back once the operation finishes, or the user
    // could never open a second tab.
    await expect(page.locator(".tab-add")).toBeEnabled();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md requires an open that cannot happen to fail with a clear error
// naming the reason, and PLAN_M4.md item 2 gives two of them: a working
// directory that has vanished, and a session whose tmux session is gone
// (which must be restarted first rather than growing a tab-only terminal).
//
// This drives the first against the REAL supervisor by deleting the
// session's working directory out from under it, so the message asserted
// is the supervisor's own rather than a fixture's. The control must also
// come back: an error that left the button stuck disabled would make the
// failure permanent from the user's side.
test("an open the supervisor refuses shows its own words and leaves the control usable", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-refusal-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // The session's working directory disappears. Removed only now, with
    // the session already up, so this is the vanished-cwd case rather
    // than a create that never worked.
    fs.rmSync(session.cwd, { recursive: true, force: true });

    await page.locator(".tab-add").click();
    const error = page.locator('.tab-error[data-tab-error="open"]');
    await expect(error).toBeVisible({ timeout: 30_000 });
    // The supervisor's own message, naming the directory — not a generic
    // "could not open a tab".
    await expect(error).toContainText(session.cwd);
    await expect(page.locator(".tab-slot")).toHaveCount(0);
    await expect(page.locator(".tab-add")).toBeEnabled();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Two guards in one flow, both about a destructive action firing more than
// the user asked for. A confirmed close must send exactly ONE DELETE for
// its tab even when the confirm button is activated twice in the same turn
// (the re-render that removes the prompt has not happened yet), while a
// DIFFERENT tab's close is not blocked by it — the guard is per tab, not a
// global lock, or closing two tabs would mean waiting out each reap in
// turn.
test("a confirmed close sends one DELETE per tab, and a sibling can close alongside it", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-close-guard-${Date.now()}`;
  let id: string | undefined;
  const deletes: string[] = [];
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [a, b] = session.tabs;
    await page.route(`**/api/sessions/${session.id}/tabs/*`, async (route) => {
      if (route.request().method() === "DELETE") {
        deletes.push(new URL(route.request().url()).pathname.split("/").pop()!);
      }
      await route.continue();
    });

    // Tab A: confirm twice in one synchronous turn.
    await page.locator(`.tab-slot[data-tab-id="${a}"] .tab-close`).click();
    await page.evaluate(() => {
      const confirm = document.querySelector(".confirm-close-tab") as HTMLButtonElement;
      confirm.click();
      confirm.click();
    });
    await expect(page.locator(`.tab-slot[data-tab-id="${a}"]`)).toHaveCount(0, {
      timeout: 30_000,
    });

    // Tab B closes normally afterwards — the per-tab guard released, and
    // was never holding B in the first place.
    await page.locator(`.tab-slot[data-tab-id="${b}"] .tab-close`).click();
    await page.locator(".confirm-close-tab").click();
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 30_000 });

    expect(deletes.sort()).toEqual([a, b].sort());
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Cancel and confirm dispatched in the same tick must never destroy
// anything — the mirror of the session list's own "dispatching cancel and
// confirm in the same tick never deletes the session" test, for a control
// whose consequence is killing a shell and everything under it.
//
// The order is cancel-then-confirm on purpose: that is the dangerous one,
// where a confirm click already queued behind a cancel would act on a
// decision the user just reversed.
test("cancel and confirm in the same tick close nothing", async ({ page, request }) => {
  test.setTimeout(120_000);
  const title = `tab-close-race-${Date.now()}`;
  let id: string | undefined;
  let deletes = 0;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    await page.route(`**/api/sessions/${session.id}/tabs/*`, async (route) => {
      if (route.request().method() === "DELETE") deletes++;
      await route.continue();
    });

    await page.locator(".tab-close").click();
    await expect(page.locator(".tab-confirm")).toHaveCount(1);
    await page.evaluate(() => {
      (document.querySelector(".tab-confirm .confirm-cancel") as HTMLButtonElement).click();
      (document.querySelector(".confirm-close-tab") as HTMLButtonElement)?.click();
    });

    await expect(page.locator(".tab-confirm")).toHaveCount(0);
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    expect(deletes).toBe(0);
    // The tab is not merely still listed but still attached and working.
    const live = shellMarker("CANCEL-RACE-LIVE");
    await runInShell(page, `terminal-${session.tabs[0]}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A close that FAILS must say so and change nothing: the tab stays listed,
// its island stays attached, and the control comes back. The failure is
// injected (a healthy stack has no reason to refuse) for the same reason
// the list view's stop/delete failure tests inject theirs.
//
// The error is also checked to be keyed to ITS OWN tab, which is what
// keeps a later success on a sibling from wiping a message the user has
// not read.
test("a failed close surfaces on that tab and leaves it attached", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-close-fail-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [failing, other] = session.tabs;
    await page.route(`**/api/sessions/${session.id}/tabs/${failing}`, async (route) => {
      if (route.request().method() !== "DELETE") {
        await route.continue();
        return;
      }
      await route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: "injected close failure",
      });
    });

    await page.locator(`.tab-slot[data-tab-id="${failing}"] .tab-close`).click();
    await page.locator(".confirm-close-tab").click();

    const error = page.locator(`.tab-error[data-tab-error="${failing}"]`);
    await expect(error).toBeVisible({ timeout: 20_000 });
    await expect(error).toContainText("injected close failure");
    // Nothing was destroyed, and nothing is stuck: the tab is still there,
    // still attached, and its close control works again.
    await expect(page.locator(`.tab-slot[data-tab-id="${failing}"]`)).toHaveCount(1);
    expect(await mountedIslands(page)).toContain(`terminal-${failing}`);
    await expect(page.locator(`.tab-slot[data-tab-id="${failing}"] .tab-close`)).toBeEnabled();
    const live = shellMarker("CLOSE-FAIL-LIVE");
    await selectTerminal(page, failing);
    await runInShell(page, `terminal-${failing}`, live.command, live.expected);

    // A SUCCESSFUL close of the other tab must not erase the failure the
    // user has not acted on yet — the per-operation keying, made visible.
    await page.locator(`.tab-slot[data-tab-id="${other}"] .tab-close`).click();
    await page.locator(".confirm-close-tab").click();
    await expect(page.locator(`.tab-slot[data-tab-id="${other}"]`)).toHaveCount(0, {
      timeout: 30_000,
    });
    await expect(error).toBeVisible();
    await expect(error).toContainText("injected close failure");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Hidden panes are hidden with `visibility`, never `display: none`, and
// this is the assertion that pins the difference where it actually bites:
// a `display: none` element has no layout box, so `FitAddon.fit()` would
// size an unselected terminal to zero columns — and in a session with more
// than one terminal, every tab mounts while unselected.
//
// So the check is on the hidden terminal's GEOMETRY, not on its CSS: real
// pixel dimensions, and a real non-degenerate grid.
test("an unselected terminal keeps real geometry while hidden", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-hidden-geometry-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [tabId] = session.tabs;

    // Select the agent, so the TAB is the hidden one.
    await selectTerminal(page, "agent");
    await expect(page.locator(`.terminal-pane[data-terminal="${tabId}"]`)).toBeHidden();

    const hidden = await page.evaluate((el) => {
      const node = document.getElementById(el)!;
      const box = node.getBoundingClientRect();
      const term = (window as any).__farhelmIslands[el].term;
      return { width: box.width, height: box.height, cols: term.cols, rows: term.rows };
    }, `terminal-${tabId}`);
    expect(hidden.width).toBeGreaterThan(0);
    expect(hidden.height).toBeGreaterThan(0);
    // The floor is xterm.js's own minimum, so anything at it would mean
    // `fit()` measured nothing; a real pane is far above it.
    expect(hidden.cols).toBeGreaterThan(10);
    expect(hidden.rows).toBeGreaterThan(4);

    // And it is not merely sized but usable the instant it is shown.
    await selectTerminal(page, tabId);
    const live = shellMarker("HIDDEN-GEOMETRY");
    await runInShell(page, `terminal-${tabId}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The confirmation row and the tab-error lines sit above `.terminal-panes`
// in the same flex column, so opening either resizes every terminal while
// the window never moves. Before the per-island ResizeObserver, a terminal
// in that state kept stale geometry — the pane and the pty disagreeing
// about how many rows exist, which is what full-screen TUIs render as
// garbage.
//
// Toggling the confirmation is the cheapest real trigger for it, and its
// effect is asserted on the terminal's own row count rather than on
// pixels, because that is the number the pty is told.
test("opening the close confirmation refits the terminals below it", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-refit-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const rows = () =>
      page.evaluate(() => (window as any).__farhelmIslands["terminal"].term.rows);

    const before = await rows();
    await page.locator(".tab-close").click();
    await expect(page.locator(".tab-confirm")).toHaveCount(1);
    await expect
      .poll(rows, {
        timeout: 10_000,
        message: "a row appearing above the panes must shrink the terminals",
      })
      .toBeLessThan(before);

    // ...and closing it gives the rows back, so the refit is a live
    // response to the box rather than a one-way shrink.
    await page.locator(".tab-confirm .confirm-cancel").click();
    await expect(page.locator(".tab-confirm")).toHaveCount(0);
    await expect.poll(rows, { timeout: 10_000 }).toBe(before);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The window-resize path, extended across islands: every terminal must
// reflow, not just the one the user is looking at. A hidden tab that kept
// its old geometry would be wrong on screen the moment it was selected,
// and — worse — would have told the pty the wrong size in the meantime.
test("a viewport resize reflows every island, hidden ones included", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-resize-all-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const dims = () =>
      page.evaluate(() => {
        const islands = (window as any).__farhelmIslands;
        const out: Record<string, string> = {};
        for (const el of Object.keys(islands).sort()) {
          out[el] = `${islands[el].term.cols}x${islands[el].term.rows}`;
        }
        return out;
      });

    const before = await dims();
    expect(Object.keys(before)).toHaveLength(3);
    await page.setViewportSize({ width: 640, height: 480 });
    await expect
      .poll(
        async () => {
          const after = await dims();
          return Object.keys(before).every((el) => after[el] !== before[el]);
        },
        { timeout: 15_000, message: "every island must reflow, hidden ones included" },
      )
      .toBe(true);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A tab closed from ANOTHER client must be torn down here, not merely
// hidden: its island unmounted and its socket closed. A strip entry
// removed while its WebSocket stayed open would leave the supervisor
// holding an attachment for a window that no longer exists, invisible from
// this side. The sibling is checked too — teardown must be surgical.
test("a tab closed elsewhere is torn down here, leaving its sibling untouched", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-remote-close-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [gone, kept] = session.tabs;
    await page.evaluate((el) => {
      (window as any).__goneWs = (window as any).__farhelmIslands[el].ws;
    }, `terminal-${gone}`);

    const closed = await request.delete(`/api/sessions/${id}/tabs/${gone}`);
    expect(closed.ok(), await closed.text()).toBe(true);

    await expect(page.locator(`.tab-slot[data-tab-id="${gone}"]`)).toHaveCount(0, {
      timeout: 20_000,
    });
    await expect
      .poll(() => mountedIslands(page))
      .toEqual(["terminal", `terminal-${kept}`].sort());
    // 3 is CLOSED — the socket is genuinely gone, not merely unreferenced.
    await expect
      .poll(() => page.evaluate(() => (window as any).__goneWs.readyState))
      .toBe(3);

    const live = shellMarker("REMOTE-CLOSE-SIBLING");
    await selectTerminal(page, kept);
    await runInShell(page, `terminal-${kept}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A poll that FAILS is not evidence about anything. The strip must keep
// showing the tabs it knows about and their terminals must keep working —
// a view that emptied itself on a transient 500 would tear down live
// attachments over a dropped request, which is the opposite of what the
// poll is for.
test("a failing detail poll leaves the tabs and their terminals alone", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-poll-fail-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [tabId] = session.tabs;
    const before = await mountedIslands(page);

    let failures = 0;
    await page.route(`**/api/sessions/${session.id}`, async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      failures++;
      await route.fulfill({ status: 500, contentType: "text/plain", body: "injected" });
    });

    // Several poll intervals of nothing but failures.
    await expect.poll(() => failures, { timeout: 20_000 }).toBeGreaterThanOrEqual(3);

    await expect(page.locator(".tab-slot")).toHaveCount(1);
    expect(await mountedIslands(page)).toEqual(before);
    const live = shellMarker("POLL-FAIL-LIVE");
    await selectTerminal(page, tabId);
    await runInShell(page, `terminal-${tabId}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A 404 from the detail route is NOT proof the session is gone, and the
// view must neither act on it nor swallow it. The helm's detail route is
// not a per-session query at all — it fetches the listing and searches it
// (`get_session` in farhelm-helm), so it inherits the supervisor's listing
// cap and a perfectly healthy session past that cap answers 404 forever.
//
// A 404 is therefore ambiguous between "deleted elsewhere" and "not in
// this page", which is why the view says what it observed, keeps
// everything it has, and names both readings rather than picking one. The
// 404 is injected because provoking the real one would mean standing up
// hundreds of sessions to overflow the cap.
test("a session the helm stops listing is reported as stale, not torn down", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-stale-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [tabId] = session.tabs;
    const before = await mountedIslands(page);
    await expect(page.locator(".refresh-stale")).toHaveCount(0);

    let missing = true;
    await page.route(`**/api/sessions/${session.id}`, async (route) => {
      if (route.request().method() !== "GET" || !missing) {
        await route.continue();
        return;
      }
      await route.fulfill({
        status: 404,
        contentType: "text/plain",
        body: `no such session: ${session.id}\n`,
      });
    });

    const stale = page.locator(".refresh-stale");
    await expect(stale).toBeVisible({ timeout: 15_000 });
    // Both readings named, neither claimed.
    await expect(stale).toContainText("deleted from another client");
    await expect(stale).toContainText("more sessions than the helm lists");

    // Nothing was torn down on the strength of an ambiguous answer: the
    // strip, the islands, and the terminals all survive.
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    expect(await mountedIslands(page)).toEqual(before);
    const live = shellMarker("STALE-LIVE");
    await selectTerminal(page, tabId);
    await runInShell(page, `terminal-${tabId}`, live.command, live.expected);

    // And it clears itself once the helm answers again — a staleness
    // notice that outlived its cause would be its own lie.
    missing = false;
    await expect(stale).toHaveCount(0, { timeout: 15_000 });
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md's split between a tab whose SHELL exited and a tab whose WINDOW
// is gone: the first is an ordinary dead pane and stays viewable, exactly
// as the agent terminal does (`remain-on-exit` is the contract for both).
// Nothing in the UI may treat "the shell ended" as "the tab ended".
test("a tab whose shell exits stays listed with its scrollback readable", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-shell-exit-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [tabId] = session.tabs;
    const marker = shellMarker("BEFORE-EXIT");
    await runInShell(page, `terminal-${tabId}`, marker.command, marker.expected);

    await page.locator(`[id="terminal-${tabId}"]`).click();
    await page.keyboard.type("exit");
    await page.keyboard.press("Enter");

    // The tab is still a tab: listed by the server, listed in the strip,
    // and its history still on screen.
    await expect
      .poll(
        async () => {
          const detail = await (await request.get(`/api/sessions/${id}`)).json();
          return (detail.tabs ?? []).map((t: any) => t.id);
        },
        { timeout: 20_000, message: "a dead shell is still a tab" },
      )
      .toEqual([tabId]);
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    expect(await islandText(page, `terminal-${tabId}`)).toContain(marker.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The other half of that split: a tab whose WINDOW is gone renders the
// session view's existing no-terminal explanation rather than a blank
// pane. Reaching that state honestly is not possible from outside the
// supervisor — a window it lost track of is by definition one it no longer
// lists — so the tab list is intercepted to name an id the supervisor
// never minted, which takes the same attach-refused path a vanished window
// takes and produces the same relayed explanation. Only the LISTING is
// synthetic; the attach, the refusal, and the banner are all real.
test("a tab the supervisor cannot attach explains itself instead of showing a blank pane", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-window-gone-${Date.now()}`;
  const phantom = "00000000-0000-4000-8000-00000000dead";
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    // Snapshotted once and served statically, for the reason the island-cap
    // test below spells out: a per-request `route.fetch()` on a route the
    // view polls leaves several `APIResponse`s in flight, and Playwright
    // disposes them as their routes complete.
    const detail = await (await request.get(`/api/sessions/${session.id}`)).json();
    detail.tabs = [...(detail.tabs ?? []), { id: phantom }];
    await page.route(`**/api/sessions/${session.id}`, async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(detail),
      });
    });

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await expect(page.locator(".tab-slot")).toHaveCount(1, { timeout: 15_000 });

    await selectTerminal(page, phantom);
    const banner = page.locator(`.terminal-pane[data-terminal="${phantom}"] .banner`);
    await expect(banner).toBeVisible({ timeout: 20_000 });
    const text = await banner.textContent();
    expect(text).toMatch(/^Detached: .+/);
    expect(text).toContain(phantom);
    // Nothing was rendered into it — the explanation is instead of the
    // terminal's content, not on top of it.
    expect((await islandText(page, `terminal-${phantom}`)).trim()).toBe("");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The confirmation names a tab POSITIONALLY, and positions move: a
// lower-numbered sibling closed from another client renumbers everything
// after it. The label must follow, or the prompt would be asking about
// "Terminal 2" while pointing at what is now Terminal 1 — and, worse, the
// id it acts on must NOT follow, or the click would close whatever
// happened to land in that position.
test("a confirm prompt renumbers with the strip while still targeting its own tab", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-confirm-reorder-${Date.now()}`;
  let id: string | undefined;
  const deleted: string[] = [];
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [first, second] = session.tabs;
    await page.route(`**/api/sessions/${session.id}/tabs/*`, async (route) => {
      if (route.request().method() === "DELETE") {
        deleted.push(new URL(route.request().url()).pathname.split("/").pop()!);
      }
      await route.continue();
    });

    // Confirm a close on the SECOND tab, then remove the first from
    // elsewhere so the confirmed tab becomes Terminal 1 under the prompt.
    await page.locator(`.tab-slot[data-tab-id="${second}"] .tab-close`).click();
    await expect(page.locator(".tab-confirm .confirm-title")).toHaveText("Terminal 2");

    const closed = await request.delete(`/api/sessions/${id}/tabs/${first}`);
    expect(closed.ok(), await closed.text()).toBe(true);
    await expect(page.locator(".tab-confirm .confirm-title")).toHaveText("Terminal 1", {
      timeout: 20_000,
    });
    // The prompt survived the reshuffle rather than being dismissed by it:
    // the user is mid-decision, and a list refresh is not an answer.
    await expect(page.locator(".tab-confirm")).toHaveCount(1);

    await page.locator(".confirm-close-tab").click();
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 30_000 });
    // The click acted on the tab the user chose, not on whatever now
    // occupies the position its label showed.
    expect(deleted).toEqual([second]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The close prompt's exact wording, asserted verbatim: it is the last
// thing a user reads before destroying a shell and every process under it,
// so it is a contract rather than an implementation detail. The Rust side
// pins the same sentence at its source (`CLOSE_TAB_CONSEQUENCE`); this
// pins that the sentence actually reaches the screen.
test("the close confirmation shows its exact consequence sentence", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-confirm-copy-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    await page.locator(".tab-close").click();
    await expect(page.locator(".tab-confirm .confirm-consequence")).toHaveText(
      "closing kills this terminal's shell and every process it started:",
    );
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A tab opened and then closed before any poll ever observed it must not
// come back. The optimistic entry that renders it immediately is retired
// by the close itself; without that, `closed_tabs` — which is pruned as
// soon as the server stops listing the id, and for a never-listed tab that
// is at once — would stop suppressing it and the strip would show a tab
// that attaches to nothing for the rest of the view's life.
//
// The poll interval is waited out deliberately: the bug only appears once
// a reconciliation runs, so asserting immediately would pass either way.
test("a tab opened and closed before any poll observes it does not come back", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-phantom-local-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    const tabId = await addTab(page, 0);
    await page.locator(`.tab-slot[data-tab-id="${tabId}"] .tab-close`).click();
    await page.locator(".confirm-close-tab").click();
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 30_000 });

    // Several poll intervals later, still gone — and the server agrees.
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 12_000 });
    const detail = await (await request.get(`/api/sessions/${id}`)).json();
    expect(detail.tabs ?? []).toEqual([]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The same phantom, arrived at from the other direction: this view opens a
// tab, ANOTHER client closes it before any poll here has ever listed it,
// and nothing this view did retires the optimistic entry. Only a poll that
// STARTED after the open can settle it — which is what the sequence number
// on each optimistic entry exists to establish. Without it, "absent" is
// indistinguishable from "the reply predates the open", and the entry can
// never be retired at all.
test("a tab closed elsewhere before this view ever listed it stops being shown", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-phantom-remote-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    const tabId = await addTab(page, 0);
    // Closed from elsewhere immediately, so this view's own state never
    // learns about it except through polling.
    const closed = await request.delete(`/api/sessions/${id}/tabs/${tabId}`);
    expect(closed.ok(), await closed.text()).toBe(true);

    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 20_000 });
    expect(await mountedIslands(page)).toEqual(["terminal"]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The island cap (`MAX_MOUNTED_TAB_ISLANDS`) is a bound on what this client
// will do with a LIST IT DID NOT AUTHOR, so it is exercised the way the
// threat arrives: a tab list the supervisor never produced. Intercepting
// the detail reply is not a shortcut here, it IS the case — a supervisor
// that is compromised, or merely wrong, is exactly what the cap defends
// against, and no healthy stack will produce it on request.
//
// Both halves are asserted: the strip tells the truth about every tab the
// session claims, and the browser is not asked to build an island for all
// of them.
test("a tab list past the island cap is listed in full but only partly attached", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-cap-${Date.now()}`;
  // Mirrors `MAX_MOUNTED_TAB_ISLANDS` in farhelm-ui/src/lib.rs. Duplicated
  // across the language boundary for the same reason `FLOOD_RECORDS` is,
  // and used the same way — in an equality check, so drift fails loudly
  // rather than weakening the assertion.
  const CAP = 32;
  const EXTRA = 3;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    const phantoms = Array.from(
      { length: CAP + EXTRA },
      (_, i) => `00000000-0000-4000-8000-${String(i).padStart(12, "0")}`,
    );
    // Snapshotted ONCE and then served statically, rather than re-fetched
    // per request: the session view polls this route every few seconds, and
    // a handler that fetches on each call has several `APIResponse`s in
    // flight at a time — Playwright disposes those as their routes
    // complete, so a later handler can find the object it is reading
    // already gone. Nothing in this test needs the reply to stay live; it
    // needs the tab list to be a fixed, oversized fixture.
    const detail = await (await request.get(`/api/sessions/${session.id}`)).json();
    detail.tabs = phantoms.map((tabId) => ({ id: tabId }));
    await page.route(`**/api/sessions/${session.id}`, async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(detail),
      });
    });

    await page.goto("/");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // Every tab the session claims is listed — silently dropping some
    // would be its own lie about what the session holds.
    await expect(page.locator(".tab-slot")).toHaveCount(CAP + EXTRA, { timeout: 20_000 });
    // The ones past the cap say why they are not attached...
    await expect(page.locator(".terminal-not-mounted")).toHaveCount(EXTRA);
    // ...and no island was ever built for them. (The capped ones do mount
    // and then fail their attach, since these ids name no real window —
    // which is the ordinary refusal path, not what this test is about.)
    const islands = await mountedIslands(page);
    for (const tabId of phantoms.slice(CAP)) {
      expect(islands).not.toContain(`terminal-${tabId}`);
    }
    expect(islands.length).toBeLessThanOrEqual(CAP + 1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// ---------------------------------------------------------------------
// Paste and drop interception (PLAN_M4.md item 7).
//
// SPEC.md's attachments contract, end to end: a file dropped on ANY of a
// session's terminals is transferred to the session's host and its
// host-side path inserted at that terminal's cursor, a pasted image does
// the same under a generated name, plain text still pastes as text, and a
// dropped directory is rejected visibly.
//
// ## What is real here and what is synthesized, stated plainly
//
// PLAN_M4.md's testing decisions require this to be recorded rather than
// glossed, because the honest answer is "most of it, but not the event
// object":
//
// - REAL: the `File` objects (constructed in the page from real bytes),
//   the streamed `fetch` upload, the helm, the supervisor, the file that
//   lands on disk, the WebSocket, and the terminal the path is inserted
//   into. The assertions below read the actual published file off the
//   filesystem and the actual terminal buffer.
// - SYNTHESIZED: the DOM event and its `DataTransfer`/`clipboardData`
//   container. Playwright cannot make an OS-level drag or put a
//   screenshot on the clipboard, and the engines disagree about whether a
//   constructed `ClipboardEvent` even keeps the `clipboardData` it was
//   given — so the event is dispatched at the seam terminal.js listens on,
//   with a stand-in payload object. That is the deterministic path
//   PLAN_M4.md names for image paste, and it is used for drops too rather
//   than having two mechanisms.
// - NOT COVERED HERE: a genuine OS drag and a genuine clipboard image, on
//   the desktop build. Those are the recorded manual pass (PLAN_M4.md
//   acceptance 9), which exists because no browser run can vouch for them.
//
// The directory tests are the one place the synthesis is load-bearing
// rather than incidental: `webkitGetAsEntry` cannot be made to report a
// real directory under synthesis, so both rejection branches are driven
// through the payload stub — the entry-API branch by an item that reports
// `isDirectory`, and the no-entry-API branch by an item whose bytes refuse
// to be read, which is exactly how a directory reaches an engine without
// that API. What they pin is terminal.js's own handling; that a real
// directory produces one of those two shapes is the manual pass's to
// confirm.
// ---------------------------------------------------------------------

/**
 * One entry in a synthesized paste/drop payload.
 *
 * `content` becomes the file's real bytes. `directory` and `unreadable`
 * stand in for the two shapes a dropped folder takes across engines (see
 * this section's header); an entry with either set carries no uploadable
 * bytes by construction, which is the point.
 */
interface PayloadEntry {
  name: string;
  mime: string;
  content?: string;
  directory?: boolean;
  unreadable?: boolean;
  /**
   * `File.lastModified`, in ms. Left alone (so the engine stamps "now")
   * for the raw-clipboard-data cases; set to something old for the tests
   * that model a real file the user copied, which is half of what tells
   * those two apart (`classify` in farhelm-ui/src/attachments.rs).
   */
  lastModified?: number;
  /**
   * Put this entry only in the payload's `files` list, never in `items`.
   *
   * Some engines expose a payload as a bare `FileList` with no item list
   * at all, and terminal.js has a fallback for exactly that; without a
   * way to synthesize it, the fallback would be reachable only in
   * production.
   */
  filesOnly?: boolean;
}

/**
 * What a synthesized event did, as observed from the page.
 *
 * These are the assertions that separate "the handler ran" from "the
 * handler took responsibility for the event": an interception that
 * forgets `preventDefault` lets the engine navigate to the dropped file,
 * and one that forgets `stopPropagation` lets xterm paste the payload's
 * text on top of the upload.
 */
interface DispatchResult {
  /** Whether anything called `preventDefault()` on the event. */
  defaultPrevented: boolean;
  /** Whether anything called `stopPropagation()` on it. */
  propagationStopped: boolean;
  /**
   * Whether the event reached its own TARGET — xterm's helper textarea
   * for a paste. False means an ancestor's capture handler stopped it on
   * the way down, which is how "intercepted before xterm sees it" is
   * observed rather than assumed.
   */
  reachedTarget: boolean;
  /** For a `dragover`, the drop effect the handler asked for. */
  dropEffect: string;
}

/**
 * Dispatch a synthetic `paste` or `drop` at one island's own element,
 * carrying `entries` as file objects and `text` as `text/plain`, and
 * report what the handlers did with it.
 *
 * A paste is dispatched at xterm's hidden helper TEXTAREA rather than at
 * the mount element, and that is not incidental: xterm registers its own
 * paste handler there, so dispatching anywhere above it would test
 * terminal.js's interception while silently skipping the handler that has
 * to keep working for plain text. Capture-phase interception still sees
 * the event on the way down.
 *
 * A drop is preceded by a `dragover` at the same element, because that is
 * the order a real drag produces and because `drop` does not fire at all
 * on an element whose `dragover` was not prevented — dispatching the drop
 * alone would pass even against an island that is not a drop target.
 *
 * The payload is a stand-in object rather than a real `DataTransfer`
 * because a real one cannot express a directory entry, and mixing two
 * mechanisms would leave the interesting tests on the weaker one.
 */
async function dispatchPayload(
  page: Page,
  elementId: string,
  kind: "paste" | "drop",
  { entries = [], text = "" }: { entries?: PayloadEntry[]; text?: string },
): Promise<{ event: DispatchResult; dragover?: DispatchResult }> {
  return page.evaluate(
    ({ elementId, kind, entries, text }) => {
      const el = document.getElementById(elementId);
      if (!el) throw new Error(`no element ${elementId}`);
      const items: any[] = [];
      const files: any[] = [];
      for (const entry of entries) {
        if (entry.directory) {
          // A directory the drag-entry API reports up front: it has no
          // file behind it at all, which is what the entry API is for.
          items.push({
            kind: "file",
            type: "",
            getAsFile: () => null,
            webkitGetAsEntry: () => ({ isDirectory: true, name: entry.name }),
          });
          continue;
        }
        let file: any = new File([entry.content ?? ""], entry.name, {
          type: entry.mime,
          ...(entry.lastModified === undefined ? {} : { lastModified: entry.lastModified }),
        });
        if (entry.unreadable) {
          // A directory on an engine with NO entry API: it arrives looking
          // like a File and only fails when its bytes are read. Modelled
          // by handing back a file-like object whose `slice` yields
          // something a `FileReader` will not read, since a real `File`'s
          // bytes cannot be made to fail on demand.
          //
          // The failure therefore lands on the reader's synchronous throw
          // rather than its `onerror`; a real directory takes the async
          // path, which rejects one line later through the same handler.
          // Both are the probe refusing to vouch for the bytes, which is
          // all the caller acts on.
          file = {
            name: file.name,
            type: file.type,
            size: 4096,
            lastModified: file.lastModified,
            slice: () => ({ notABlob: true }),
          };
        }
        files.push(file);
        if (entry.filesOnly) continue;
        items.push({
          kind: "file",
          type: entry.mime,
          getAsFile: () => file,
          webkitGetAsEntry: () => null,
        });
      }
      const data = {
        items,
        files,
        types: [...(files.length ? ["Files"] : []), ...(text ? ["text/plain"] : [])],
        dropEffect: "none",
        getData: (type: string) => (type === "text/plain" ? text : ""),
      };

      // One dispatch, plus the bookkeeping that makes the handlers'
      // treatment of the event observable from Node.
      const fire = (type: string, target: EventTarget, payloadKey: string | null) => {
        const event = new Event(type, { bubbles: true, cancelable: true });
        if (payloadKey) Object.defineProperty(event, payloadKey, { value: data });
        let propagationStopped = false;
        const realStop = event.stopPropagation.bind(event);
        event.stopPropagation = () => {
          propagationStopped = true;
          realStop();
        };
        let reachedTarget = false;
        const witness = () => {
          reachedTarget = true;
        };
        target.addEventListener(type, witness);
        try {
          target.dispatchEvent(event);
        } finally {
          target.removeEventListener(type, witness);
        }
        return {
          defaultPrevented: event.defaultPrevented,
          propagationStopped,
          reachedTarget,
          dropEffect: data.dropEffect,
        };
      };

      if (kind === "paste") {
        const target = el.querySelector(".xterm-helper-textarea") ?? el;
        return { event: fire("paste", target, "clipboardData") };
      }
      const dragover = fire("dragover", el, "dataTransfer");
      return { event: fire("drop", el, "dataTransfer"), dragover };
    },
    { elementId, kind, entries, text },
  );
}

/**
 * One island's buffer as LOGICAL lines: xterm's wrapped continuation rows
 * are joined back onto the row they continue.
 *
 * `islandText` above joins every buffer row with a newline, which is right
 * for content assertions and wrong for these: an attachment path is ~70
 * characters before the session's own directory name, so it wraps in an
 * 80-column terminal and a newline lands in the middle of it. Every
 * assertion here matches a path, so every one of them would fail on a
 * terminal that is behaving perfectly.
 *
 * Rows are translated WITHOUT trailing-space trimming and the assembled
 * line is trimmed at the end instead — trimming each row would silently
 * eat a space that fell on a wrap boundary, which for these tests is the
 * separator between two attached paths.
 */
async function islandLogicalText(page: Page, elementId: string): Promise<string> {
  return page.evaluate((el) => {
    const island = (window as any).__farhelmIslands?.[el];
    if (!island) return "";
    const buf = island.term.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < buf.length; i++) {
      const line = buf.getLine(i);
      if (!line) continue;
      const text = line.translateToString(false);
      if (line.isWrapped && lines.length) lines[lines.length - 1] += text;
      else lines.push(text);
    }
    return lines.map((line) => line.replace(/\s+$/, "")).join("\n");
  }, elementId);
}

/**
 * A regular-expression fragment matching one inserted host path that ends
 * in `tail` (itself a regex fragment).
 *
 * Anchored at the leading `/`, which is not fussiness: an inserted path
 * arrives through `term.paste()`, so where this client has the pane's
 * bracketed-paste mode it is wrapped in the paste markers, and the pty
 * echoes those control bytes back in caret notation — the buffer really
 * does read `^[[200~/tmp/.../notes.txt ^[[201~`. A pattern starting with
 * `\S*` swallows the `^[[200~` into the path and hands the test a
 * filename that does not exist.
 *
 * The markers are deliberately tolerated rather than required, here and
 * in every `\S*?` that joins two of these fragments: whether THIS client
 * knows the pane is in bracketed-paste mode depends on the tmux underneath
 * (see `latchBracketedPaste`), and no assertion about where a path landed
 * should turn on that. The one test that does pin the markers latches the
 * mode first.
 */
function hostPathEnding(tail: string): string {
  return `\\/\\S*\\/attachments\\/\\S*${tail}`;
}

/**
 * Put this island's xterm into the bracketed-paste mode its pane is
 * already in, so a subsequent paste's markers are observable no matter
 * what the attach snapshot was able to carry.
 *
 * xterm wraps `term.paste()` in `\x1b[200~`/`\x1b[201~` only once it has
 * parsed a `\x1b[?2004h` of its own. The pane genuinely is in that mode —
 * the `basic` fake agent enables it before printing its ready banner — but
 * a client that attaches AFTER the agent said so never sees that byte
 * live: it can only learn the mode from the supervisor's replay, which
 * reconstructs it from tmux's `bracket_paste_flag`. That format variable
 * arrived in tmux 3.7, so against any older tmux the mode is not restored
 * — a degradation the supervisor knows about and warns about once per
 * process — and the paste goes out unwrapped even though the client took
 * exactly the paste path. CI's tmux is one of those, which is why the
 * marker assertion failed there on every run while passing on developer
 * machines with a newer one.
 *
 * So this writes a truth about the pane into the client rather than
 * inventing one, and it leaves the marker assertion pinning what it exists
 * to pin: that insertion went through `term.paste()` rather than straight
 * at the socket, which no amount of restored mode state would fake.
 *
 * Awaited on xterm's own write callback because `term.write` is
 * asynchronous — returning before the parser has run would reintroduce the
 * very race this removes.
 */
async function latchBracketedPaste(page: Page, elementId: string): Promise<void> {
  await page.evaluate(
    (el) =>
      new Promise<void>((resolve, reject) => {
        const island = (window as any).__farhelmIslands?.[el];
        if (!island) {
          reject(new Error(`no island ${el} to put into bracketed-paste mode`));
          return;
        }
        island.term.write("\x1b[?2004h", () => resolve());
      }),
    elementId,
  );
}

/**
 * Poll one island's logical buffer until `pattern` matches, and hand back
 * the match — which is how these tests learn the host path, since the
 * supervisor mints it and nothing client-side can predict it.
 */
async function waitForIslandMatch(
  page: Page,
  elementId: string,
  pattern: RegExp,
  timeout = 30_000,
): Promise<RegExpMatchArray> {
  // A holder rather than a bare `let`: `expect.poll` runs its callback on
  // its own schedule, so TypeScript cannot see that the variable is set
  // by the time the poll resolves.
  const found: { match: RegExpMatchArray | null } = { match: null };
  await expect
    .poll(
      async () => {
        found.match = (await islandLogicalText(page, elementId)).match(pattern);
        return found.match !== null;
      },
      { timeout, message: `waiting for ${pattern} in ${elementId}` },
    )
    .toBe(true);
  return found.match!;
}

/** `waitForIslandText`, over logical (unwrapped) lines. */
async function waitForIslandLogicalText(
  page: Page,
  elementId: string,
  needle: string,
  timeout = 20_000,
) {
  await expect
    .poll(() => islandLogicalText(page, elementId), {
      timeout,
      message: `waiting for ${needle} in ${elementId}`,
    })
    .toContain(needle);
}

/**
 * Count every attachment upload this page issues, so a test can assert
 * that NOTHING was uploaded — the assertion behind "pasted text is still
 * text" and "a directory is rejected", both of which are only interesting
 * if no bytes went anywhere.
 */
function countUploads(page: Page): {
  count: () => number;
  urls: () => string[];
  reset: () => void;
} {
  let seen: string[] = [];
  page.on("request", (req) => {
    if (req.url().includes("/attachments")) seen.push(req.url());
  });
  return {
    count: () => seen.length,
    urls: () => [...seen],
    // For the tests that care about what happened AFTER some point — a
    // remount, say — rather than about the whole page's history.
    reset: () => {
      seen = [];
    },
  };
}

/**
 * Create a session of this section's own and open its terminal, ready to
 * receive a drop.
 *
 * Every test here works on a fresh session rather than the shared
 * "e2e-session", and that is a hard requirement rather than tidiness. Two
 * earlier tests in this file deliberately flood that pane with megabytes
 * of output, so by the time this section runs its `FAKE-AGENT READY`
 * banner has scrolled out of the 12000-line buffer entirely and
 * `openTerminal`'s readiness wait can never succeed there — found exactly
 * that way, as a full-suite failure in tests that passed in isolation. A
 * fresh session also starts with an empty input line and a buffer holding
 * only the fake agent's own banner and prompt, so an assertion about "the
 * path that was inserted" cannot match one an earlier test left behind.
 *
 * Reuses `createTabSession` (from the tabs section) for the session
 * itself: it already mints a per-test working directory and registers it
 * for cleanup, and an attachment test wants exactly that.
 */
async function openAttachmentSession(
  page: Page,
  request: APIRequestContext,
  title: string,
): Promise<{ id: string; cwd: string }> {
  const session = await createTabSession(request, title);
  await page.goto("/");
  await page.locator(`[data-session-id="${session.id}"]`).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitForTermText(page, "FAKE-AGENT READY");
  return session;
}

// The headline flow, with nothing faked but the event container: a file
// dropped on the agent terminal is uploaded to the session's host, lands
// under that session's attachments directory with its bytes intact, and
// its path is inserted AT THE CURSOR — after the text already typed there,
// which is what "at the cursor" has to mean.
//
// Pressing Enter afterwards is not decoration: local echo alone would
// prove the path reached xterm, and the fake agent's `echo:` line is what
// proves those same bytes went out over the WebSocket as terminal input,
// which is the whole point of inserting a path.
test("a file dropped on the agent terminal uploads and inserts its host path at the cursor", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const name = `notes-${stamp}.txt`;
  const body = `dropped-body-${stamp}`;
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-drop-${stamp}`);
    id = session.id;

    await page.locator("#terminal").click();
    await page.keyboard.type("PRE:");
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name, mime: "text/plain", content: body }],
      // The text is synthesized alongside the file deliberately: some
      // drag sources do offer a text/plain copy of the path, and the
      // precedence rule exists for exactly that shape. It is not a claim
      // that every real drag carries one — it is how this test reaches
      // the branch where the file has to win.
      text: "/home/somebody/notes.txt",
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`PRE:\\S*?(${hostPathEnding(`${stamp}\\.txt`)})`),
    );
    const hostPath = match[1];
    expect(hostPath, "the path must live under this session's own attachments directory")
      .toContain(`/attachments/${id}/`);
    expect(hostPath.endsWith(name), `dropped files keep their name: ${hostPath}`).toBe(true);
    expect(fs.readFileSync(hostPath, "utf8")).toBe(body);

    await page.keyboard.press("Enter");
    const echoed = `echo:PRE:${hostPath}`;
    await waitForIslandLogicalText(page, "terminal", echoed);
    // One winning interpretation, one insertion: the payload's text/plain
    // copy of the dragged path must not ALSO have been pasted.
    expect(
      await islandLogicalText(page, "terminal"),
      "the file won the payload, so its text/plain sibling must not be pasted too",
    ).not.toContain("/home/somebody/notes.txt");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md quantifies the attachments contract over "any of a session's
// terminals — the agent's or a tab's", and this is that sentence made
// executable: the drop lands in a TAB, the path arrives in the tab's own
// input, and the agent terminal beside it never sees a thing.
//
// Both halves matter. Per-island hooks that all wrote to the agent
// terminal would pass a test that only checked "a path appeared
// somewhere".
test("a file dropped on a tab inserts its path in that tab, leaving the agent terminal untouched", async ({
  page,
  request,
}) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-tab-${stamp}`);
    id = session.id;
    const body = `tab-dropped-body-${stamp}`;

    const tabId = await addTab(page, 0);
    const island = `terminal-${tabId}`;
    await waitForIslandMounted(page, island);
    await selectTerminal(page, tabId);

    await dispatchPayload(page, island, "drop", {
      entries: [{ name: "from-tab.txt", mime: "text/plain", content: body }],
    });

    const match = await waitForIslandMatch(
      page,
      island,
      new RegExp(`(${hostPathEnding("from-tab\\.txt")})`),
    );
    const hostPath = match[1];
    expect(hostPath).toContain(`/attachments/${id}/`);
    expect(fs.readFileSync(hostPath, "utf8")).toBe(body);

    expect(
      await islandLogicalText(page, "terminal"),
      "the agent terminal received no drop, so nothing may have been inserted into it",
    ).not.toContain("/attachments/");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A pasted screenshot: image data on the clipboard is uploaded under a
// GENERATED name rather than the placeholder the engine invented for it
// (Chromium calls every pasted image `image.png`), which is why the
// payload below carries exactly that placeholder — using a nameless entry
// would test a case real engines do not produce.
//
// The extension comes from the MIME type, so the agent reading the path
// knows what it is holding.
test("a pasted image is uploaded under a generated name and its path inserted", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const body = `pasted-image-bytes-${stamp}`;
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-image-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "paste", {
      entries: [{ name: "image.png", mime: "image/png", content: body }],
      // The other half of the precedence order: a clipboard holding image
      // data very often holds a text rendering beside it (an HTML wrapper,
      // a source URL). The image has to win and the text must not also be
      // pasted, which is the "one winning interpretation, one insertion"
      // rule seen from the image side.
      text: "https://example.invalid/screenshot.png",
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding("pasted-\\d+(?:-\\d+)?\\.png")})`),
    );
    const hostPath = match[1];
    expect(hostPath).toContain(`/attachments/${id}/`);
    // The counter is per page and this is the page's first paste, so the
    // name is `pasted-1.png`; the optional numeric suffix is the
    // supervisor's collision resolution, which nothing here should hit but
    // which is not worth failing over if the directory is ever reused.
    expect(
      /\/pasted-1(-\d+)?\.png$/.test(hostPath),
      `a pasted image takes a generated, typed name, got ${hostPath}`,
    ).toBe(true);
    expect(fs.readFileSync(hostPath, "utf8")).toBe(body);
    expect(
      await islandLogicalText(page, "terminal"),
      "the image won the payload, so its text sibling must not be pasted too",
    ).not.toContain("example.invalid");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md is explicit that pasted text which merely LOOKS like a path is
// still text, and the way to keep that true is to have no path heuristic
// at all — the classifier looks at what the payload carries, never at what
// the text says. This pins both halves: the text arrives as terminal
// input, and no upload was attempted.
//
// It also exercises xterm's own paste handler, which an interception bug
// could break by swallowing every paste event indiscriminately.
test("pasted text that looks like a path arrives as text, with nothing uploaded", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-text-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "paste", { text: "/etc/hosts" });

    await waitForIslandMatch(page, "terminal", /\/etc\/hosts/);
    await page.keyboard.press("Enter");
    await waitForIslandLogicalText(page, "terminal", "echo:/etc/hosts");
    expect(uploads.count(), "text is not an attachment").toBe(0);
    await expect(page.locator('[data-terminal="agent"] .attach-error')).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md rejects dropped directories unconditionally in v1, and the
// drag-entry API is how that is caught before anything is read. The
// payload is synthesized with a `text/plain` copy of the folder's path
// beside it — a shape folder drags do produce on some sources — because
// the interesting failure is silently pasting THAT instead: a directory
// outranks text in the precedence order precisely so the rejection is
// what the user gets.
test("a dropped directory is rejected visibly, uploading nothing and pasting nothing", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-dir-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "some-folder", mime: "", directory: true }],
      text: "/home/somebody/some-folder",
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("some-folder");
    await expect(error).toContainText("is a directory");
    expect(uploads.count(), "nothing directory-shaped may reach the helm").toBe(0);
    expect(
      await islandLogicalText(page, "terminal"),
      "the folder's own path must not be pasted as a consolation prize",
    ).not.toContain("/home/somebody/some-folder");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The other half of the unconditional rejection: an engine with no
// drag-entry API hands a directory over looking like an ordinary `File`,
// and the only thing that gives it away is that its bytes cannot be read.
// terminal.js reads one byte before uploading for exactly this reason, so
// the refusal happens BEFORE anything is sent rather than arriving as a
// truncated upload the supervisor rejects for its size.
//
// The wording deliberately does not claim to know it was a directory — an
// unreadable file reaches this same path — but it does name what it saw.
test("an item whose bytes cannot be read is rejected before anything is uploaded", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-unreadable-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "looks-like-a-file", mime: "", unreadable: true }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("looks-like-a-file");
    await expect(error).toContainText("could not be read");
    expect(uploads.count(), "the probe read must fail before the upload starts").toBe(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md: "Upload failures must be visible; an attachment must never
// disappear silently." A failure is induced at the network seam because a
// healthy stack will not produce one on demand, and both obligations are
// checked — the user is told, in the supervisor's own words, and no path
// is inserted for a file that was never published.
test("an upload that fails surfaces the error and inserts no path", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: "storing the attachment failed: no space left on device\n",
      });
    });
    const session = await openAttachmentSession(page, request, `attach-fail-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "doomed.txt", mime: "text/plain", content: "never lands" }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("doomed.txt");
    await expect(
      error,
      "the response body — here injected, in production the supervisor's own words — is what \
makes the error actionable, so it has to reach the line the user reads",
    ).toContainText("no space left on device");
    await expect(error).toContainText("no path was inserted");
    expect(
      await islandLogicalText(page, "terminal"),
      "a failed upload published nothing, so there is no path to insert",
    ).not.toContain("/attachments/");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md: "Transfer must not block the terminal: you can keep typing
// while it runs, and the path is inserted at whatever cursor position is
// current when the transfer completes." Both clauses are one assertion
// here — the characters typed DURING the upload land first, and the path
// lands after them.
//
// The upload is throttled at the network seam rather than by uploading
// something genuinely large: what this test needs is a transfer that is
// still running while a human types, and a delayed `continue()` gives that
// deterministically while leaving the upload itself entirely real.
test("typing stays live during an upload, and the path lands at the cursor position it finds", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const name = `slow-${stamp}.txt`;
  const body = `slow-upload-${stamp}`;
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 3_000));
      await route.continue();
    });
    const session = await openAttachmentSession(page, request, `attach-slow-${stamp}`);
    id = session.id;

    await page.locator("#terminal").click();
    await page.keyboard.type("PRE:");
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name, mime: "text/plain", content: body }],
    });

    // The indicator is up while the transfer runs, and it names the file.
    const busy = page.locator('[data-terminal="agent"] .attach-busy');
    await expect(busy).toBeVisible();
    await expect(busy).toContainText(name);

    // Typed while the upload is still in flight: if the terminal were
    // blocked, these keystrokes would not be echoed until afterwards, and
    // the ordering assertion below would fail.
    await page.keyboard.type("LIVE");
    await waitForIslandMatch(page, "terminal", /PRE:LIVE/);

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`PRE:LIVE\\S*?(${hostPathEnding(`${stamp}\\.txt`)})`),
    );
    expect(match[1]).toContain(`/attachments/${id}/`);
    expect(fs.readFileSync(match[1], "utf8")).toBe(body);
    // The indicator goes away on its own once nothing is in flight.
    await expect(busy).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// "N dropped files insert N paths" (PLAN_M4.md acceptance 6), separated so
// a shell sees N words rather than one impossible one. The separator is a
// trailing space per path (`PATH_SEPARATOR` in
// farhelm-ui/src/attachments.rs), which is what also keeps a SECOND drop
// from fusing onto the end of the first one's path.
test("two dropped files insert both paths, each its own word", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-many-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [
        { name: `first-${stamp}.txt`, mime: "text/plain", content: "first body" },
        { name: `second-${stamp}.txt`, mime: "text/plain", content: "second body" },
      ],
    });

    // Each path is its own `term.paste()`, so each carries its own
    // bracketed-paste wrapper and the echoed markers land BETWEEN them:
    // `^[[200~<first> ^[[201~^[[200~<second> ^[[201~`. The space after the
    // first path is the separator under test; the `\S*?` after it is the
    // marker noise the pty echoed (see `hostPathEnding`).
    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(
        `(${hostPathEnding(`first-${stamp}\\.txt`)}) \\S*?(${
          hostPathEnding(`second-${stamp}\\.txt`)
        })`,
      ),
    );
    for (const [hostPath, expected] of [[match[1], "first body"], [match[2], "second body"]]) {
      expect(hostPath).toContain(`/attachments/${id}/`);
      expect(fs.readFileSync(hostPath, "utf8")).toBe(expected);
    }
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Two REAL files in one clipboard payload, one of them an image: both are
// file references, so both upload and both keep their own names.
//
// The regression this pins is a silent one. An earlier rule sorted every
// image-typed clipboard entry into the image bucket, so a payload holding
// a document and a picture classified as "file" and then uploaded only the
// document — the picture vanished with no error anywhere, which is exactly
// the silent loss SPEC.md forbids. The image entry here carries a real
// name and an old `lastModified`, which is what a copied FILE looks like
// (see `classify` in farhelm-ui/src/attachments.rs).
test("a clipboard payload of two real files uploads both, each under its own name", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-two-real-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "paste", {
      entries: [
        { name: `notes-${stamp}.txt`, mime: "text/plain", content: "document body" },
        {
          name: `holiday-${stamp}.png`,
          mime: "image/png",
          content: "picture body",
          // A week old: a file the user copied, not bytes the engine
          // synthesized a moment ago.
          lastModified: stamp - 7 * 24 * 60 * 60 * 1000,
        },
      ],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(
        `(${hostPathEnding(`notes-${stamp}\\.txt`)}) \\S*?(${
          hostPathEnding(`holiday-${stamp}\\.png`)
        })`,
      ),
    );
    for (const [hostPath, expected] of [[match[1], "document body"], [match[2], "picture body"]]) {
      expect(hostPath).toContain(`/attachments/${id}/`);
      expect(fs.readFileSync(hostPath, "utf8")).toBe(expected);
    }
    expect(
      await islandLogicalText(page, "terminal"),
      "a copied image FILE keeps its own name; only raw clipboard data is renamed",
    ).not.toContain("pasted-");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The precedence case with all three flavors present at once: a file
// reference, raw image DATA, and text.
//
// One upload, one insertion, and the file's own name — because the image
// data and the file are two REPRESENTATIONS of one copy (copying an image
// file offers both), so uploading both would publish the same picture
// twice under two names. That is what the precedence order is for, and it
// is a different question from the test above, where two distinct files
// both had to survive.
test("a file reference beside raw image data uploads the file alone", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-mixed-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "paste", {
      entries: [
        {
          name: `report-${stamp}.txt`,
          mime: "text/plain",
          content: "the file itself",
          lastModified: stamp - 60_000,
        },
        // No name at all: unambiguously the engine's rendering of the
        // clipboard's bytes rather than a file the user chose.
        { name: "", mime: "image/png", content: "the rendering" },
      ],
      text: `/home/somebody/report-${stamp}.txt`,
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`report-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("the file itself");
    const buffer = await islandLogicalText(page, "terminal");
    expect(buffer, "the rendering is not a second attachment").not.toContain("pasted-");
    expect(buffer, "and the text sibling is not pasted either").not.toContain("/home/somebody/");
    expect(uploads.count(), "one copy is one upload").toBe(1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The inserted path is TERMINAL INPUT, so a path a shell would split is a
// path the agent cannot open. The supervisor sanitizes the filename, but
// the directories above it come from the user's own `--state-dir`, which
// can be `~/Library/Application Support/…` or anything else a filesystem
// allows.
//
// Driven in a TAB, against a real shell, because that is the only way to
// assert the property that matters: not "the text looks quoted" but "the
// shell sees ONE argument". The published path is injected rather than
// produced, since the suite's own state directory is a plain mktemp name —
// making the stack use a hostile one would mean restarting it for a single
// test.
test("a path whose directories need quoting arrives at the shell as one word", async ({
  page,
  request,
}) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  const hostile = `/tmp/fh attach '${stamp}'/attachments/s/shot.png`;
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ path: hostile }),
      });
    });
    const session = await openAttachmentSession(page, request, `attach-quote-${stamp}`);
    id = session.id;

    const tabId = await addTab(page, 0);
    const island = `terminal-${tabId}`;
    await waitForIslandMounted(page, island);
    await selectTerminal(page, tabId);

    // `sh -c` for the same portability reason `shellMarker` uses it: the
    // tab runs the user's real login shell, which may be fish. `$1` is
    // printed whole, so a path the outer shell split would show up
    // truncated at the first space.
    await runInShell(page, island, `sh -c 'echo READY-${stamp}'`, `READY-${stamp}`);
    await page.locator(`[id="${island}"]`).click();
    await page.keyboard.type(`sh -c 'echo ARG:"$1"' _ `);
    await dispatchPayload(page, island, "drop", {
      entries: [{ name: "shot.png", mime: "image/png", content: "x" }],
    });
    await waitForIslandMatch(page, island, /shot\.png/);
    await page.keyboard.press("Enter");

    await waitForIslandLogicalText(page, island, `ARG:${hostile}`, 30_000);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md's "an attachment must never disappear silently", at the one seam
// where it is easiest to break: `term.paste()` into an island whose socket
// is gone drops the text with no error anywhere. A payload offered to a
// detached terminal is therefore refused up front, visibly, and nothing is
// uploaded for it.
//
// The socket is closed directly rather than by staging a real takeover:
// what this exercises is the liveness gate, and a takeover would add a
// second client's timing to a test that has nothing to say about it.
test("a payload dropped on a detached terminal is refused instead of uploaded", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-detached-${stamp}`);
    id = session.id;

    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());
    await page.waitForFunction(
      () => (window as any).__farhelmIslands["terminal"].ws.readyState !== WebSocket.OPEN,
    );

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "late.txt", mime: "text/plain", content: "nowhere to go" }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("not connected");
    expect(uploads.count(), "a path with nowhere to land is not worth the transfer").toBe(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The other side of the same seam: the terminal was live when the upload
// started and gone by the time it finished. The file is real and published
// by then, so the honest answer is to hand the user its path rather than
// paste it into a socket that will drop it.
test("an upload that outlives its socket reports the path it could not insert", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const landed = `/tmp/fh-landed-${stamp}/attachments/s/late.png`;
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 2_000));
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ path: landed }),
      });
    });
    const session = await openAttachmentSession(page, request, `attach-outlives-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "late.png", mime: "image/png", content: "bytes" }],
    });
    await expect(page.locator('[data-terminal="agent"] .attach-busy')).toBeVisible();
    // Killed while the upload is in flight, so completion lands on a
    // terminal that can no longer receive anything.
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible({ timeout: 20_000 });
    await expect(error).toContainText(landed);
    await expect(error).toContainText("not inserted");
    expect(
      await islandLogicalText(page, "terminal"),
      "the path was reported, not pasted into a socket that would swallow it",
    ).not.toContain(landed);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The failure message is assembled from a template and values that include
// the server's own words, and both halves of that assembly have bitten
// before.
//
// A string replacement would let `$&` and `$1` in the server's text
// rewrite the message around them, and a per-key sequence of replacements
// would re-scan what an earlier key inserted — so a file named
// `{reason}.txt` would have the error spliced into its own name. Both are
// asserted EXACTLY here, so a revert to either shape fails rather than
// producing something merely odd-looking.
test("error text carrying replacement tokens renders literally", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const hostileReason = "storage failed: $& and $1 and $` are literal";
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await route.fulfill({ status: 500, contentType: "text/plain", body: hostileReason });
    });
    const session = await openAttachmentSession(page, request, `attach-tokens-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "{reason}.txt", mime: "text/plain", content: "x" }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    expect(await error.textContent()).toBe(
      `attaching {reason}.txt failed: ${hostileReason} — no path was inserted`,
    );
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The indicator has to describe what is STILL RUNNING, which a count plus
// a remembered name cannot do: finish the second of two uploads and the
// count says one while the remembered name is the one that just landed, so
// the line names the wrong file for as long as the other runs.
//
// The two uploads are made to finish out of order deliberately — the
// second returns immediately, the first is held — which is the exact
// arrangement that exposed it.
test("the indicator names the upload still running, not the one that just finished", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      // The quick one is not instant: both have to be in flight together
      // long enough for the many-files form to be observable, or this
      // test would pass without ever seeing the state it is about.
      const slow = route.request().url().includes("slow");
      await new Promise((resolve) => setTimeout(resolve, slow ? 6_000 : 1_500));
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ path: `/tmp/fh-${stamp}/attachments/s/${slow ? "slow" : "quick"}` }),
      });
    });
    const session = await openAttachmentSession(page, request, `attach-order-${stamp}`);
    id = session.id;

    // Two payloads rather than one drop of two files: uploads within one
    // payload are sequential by design, so overlapping them takes two.
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "slow.txt", mime: "text/plain", content: "1" }],
    });
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "quick.txt", mime: "text/plain", content: "2" }],
    });

    const busy = page.locator('[data-terminal="agent"] .attach-busy');
    await expect(busy).toContainText("2 files");
    // The quick one lands first; the line must then name the slow one.
    await expect(busy).toContainText("slow.txt", { timeout: 15_000 });
    await expect(busy).toHaveCount(0, { timeout: 20_000 });
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// An error stays on screen until the user does something that supersedes
// it. A drag carrying nothing this UI can act on is not that: the default
// is still prevented (an unhandled drop navigates the page away, taking
// every terminal with it), but the failure the user has not read yet stays
// where it was.
test("an unsupported drop leaves an earlier failure on screen", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-none-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "some-folder", mime: "", directory: true }],
    });
    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toContainText("some-folder");

    const empty = await dispatchPayload(page, "terminal", "drop", {});
    expect(
      empty.event.defaultPrevented,
      "an unhandled drop is the engine's to act on, and its action is to navigate away",
    ).toBe(true);
    await expect(error).toContainText("some-folder");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A zero-byte file is an ordinary attachment: the pinned contract has no
// minimum size, and the readability probe — which reads one byte — must
// not mistake "there is no first byte" for "these bytes cannot be read".
test("a zero-byte file publishes and inserts its path", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-empty-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `empty-${stamp}.txt`, mime: "text/plain", content: "" }],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`empty-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("");
    await expect(page.locator('[data-terminal="agent"] .attach-error')).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The proposed name goes into a URL QUERY, so a filename carrying `&`,
// `#`, `?` or a space must not be able to add parameters, truncate the
// URL, or split it. Both ends are checked: the request leaves
// percent-encoded, and the file publishes with its bytes intact under the
// supervisor's own sanitized spelling of the name.
test("a filename full of URL syntax reaches the helm percent-encoded", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  const hostile = `a&b#c?d e-${stamp}.txt`;
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-urlname-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: hostile, mime: "text/plain", content: "encoded body" }],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`${stamp}.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("encoded body");
    const [url] = uploads.urls();
    expect(url).toContain(`filename=a%26b%23c%3Fd%20e-${stamp}.txt`);
    expect(
      new URL(url).searchParams.get("filename"),
      "every byte of the name must survive as ONE query value",
    ).toBe(hostile);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Two ways an upload can fail without the server ever saying so: the
// request never completes (a dropped connection), and it completes with a
// body that is not the contract's shape. Both have to surface visibly and
// insert nothing — a 200 with no usable path is not a success with
// nothing to show, it is a failure this client cannot explain further.
test("a dead request and a malformed reply both fail visibly", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let mode: "abort" | "garbage" = "abort";
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      if (mode === "abort") {
        await route.abort("connectionreset");
        return;
      }
      await route.fulfill({ status: 200, contentType: "application/json", body: "{\"ok\":true}" });
    });
    const session = await openAttachmentSession(page, request, `attach-broken-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "dropped.txt", mime: "text/plain", content: "x" }],
    });
    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("dropped.txt");
    await expect(error).toContainText("no path was inserted");

    mode = "garbage";
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "shapeless.txt", mime: "text/plain", content: "x" }],
    });
    await expect(error).toContainText("shapeless.txt");
    await expect(error).toContainText("no usable path");
    expect(
      await islandLogicalText(page, "terminal"),
      "neither failure published anything, so neither may insert anything",
    ).not.toContain("/attachments/");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// One bad file in a drop of several must not cost the user the rest. The
// first upload is refused and the second is left alone, so the queue has
// to report the failure and carry on rather than abandoning what follows.
test("a failure on the first of two dropped files does not stop the second", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      if (route.request().url().includes("doomed")) {
        await route.fulfill({ status: 500, contentType: "text/plain", body: "refused" });
        return;
      }
      await route.continue();
    });
    const session = await openAttachmentSession(page, request, `attach-partial-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [
        { name: `doomed-${stamp}.txt`, mime: "text/plain", content: "never" },
        { name: `survivor-${stamp}.txt`, mime: "text/plain", content: "landed anyway" },
      ],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`survivor-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("landed anyway");
    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toContainText(`doomed-${stamp}.txt`);
    expect(
      await islandLogicalText(page, "terminal"),
      "the refused file published nothing, so no path for it may appear",
    ).not.toContain("doomed-");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Leaving a session mid-upload tears the island down, and the remount that
// follows must be a clean one. The listeners live on an element Dioxus
// keeps, so a teardown that forgot to remove them would stack a second set
// on the remounted island — and the next drop would upload the same file
// twice and insert its path twice.
test("leaving mid-upload and reopening leaves exactly one set of hooks", async ({
  page,
  request,
}) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      if (route.request().url().includes("abandoned")) {
        await new Promise((resolve) => setTimeout(resolve, 30_000));
      }
      await route.continue();
    });
    const session = await openAttachmentSession(page, request, `attach-remount-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `abandoned-${stamp}.txt`, mime: "text/plain", content: "x" }],
    });
    await expect(page.locator('[data-terminal="agent"] .attach-busy')).toBeVisible();

    await page.locator(".back-button").click();
    await expect(page.locator(".session-row").first()).toBeVisible();
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");

    uploads.reset();
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `after-${stamp}.txt`, mime: "text/plain", content: "once" }],
    });
    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`after-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("once");
    expect(uploads.count(), "one drop is one upload, even after a remount").toBe(1);
    const buffer = await islandLogicalText(page, "terminal");
    expect(
      buffer.split(`after-${stamp}.txt`).length - 1,
      "and one upload is one insertion",
    ).toBe(1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The status line carries two strings this page did not author: a filename
// from the user's filesystem and an error body from a supervisor that
// under `--ssh` is another machine. Both are rendered as TEXT — the same
// promise the session list makes for titles and error details, checked the
// same way: the markup survives as characters and creates no element.
test("a filename and a server error carrying markup render as literal text", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const markupName = `<img src=x onerror="window.__pwned=1">-${stamp}.txt`;
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: '<script>window.__pwned = 2</script>refused',
      });
    });
    const session = await openAttachmentSession(page, request, `attach-xss-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: markupName, mime: "text/plain", content: "x" }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    const rendered = await error.textContent();
    expect(rendered).toContain(markupName);
    expect(rendered).toContain("<script>window.__pwned = 2</script>");
    expect(
      await error.locator("img, script").count(),
      "markup in a name or a server message is content, never structure",
    ).toBe(0);
    expect(await page.evaluate(() => (window as any).__pwned)).toBeUndefined();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Two properties of the moment an upload completes, both invisible until
// they break.
//
// The path must not steal FOCUS: an upload finishes whenever it finishes,
// and yanking the caret out of whatever the user moved to in the meantime
// would be worse than the wait. And it must arrive through the
// bracketed-paste path, which is what tells a full-screen agent that the
// text was pasted rather than typed — visible here as the markers the pty
// echoes back around it.
test("a completed upload keeps focus where it was and inserts through bracketed paste", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-focus-${stamp}`);
    id = session.id;

    // A precondition, not part of what is under test: this client attached
    // after the agent turned bracketed paste on, so on an old tmux it would
    // never have learned the mode the pane is in (see
    // `latchBracketedPaste`), and the markers below would be missing for a
    // reason that has nothing to do with the insertion path.
    await latchBracketedPaste(page, "terminal");
    // Focus deliberately parked outside the terminal before the drop.
    await page.locator(".back-button").focus();
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `focus-${stamp}.txt`, mime: "text/plain", content: "x" }],
    });
    await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`focus-${stamp}\\.txt`)})`),
    );

    expect(
      await page.evaluate(() => document.activeElement?.className ?? ""),
      "an upload completing must not move the caret",
    ).toContain("back-button");
    // `^[[200~` is the pty echoing the bracketed-paste start marker back
    // in caret notation; it is only there if the insertion went through
    // the paste path rather than being written straight at the socket.
    expect(await islandLogicalText(page, "terminal")).toContain("^[[200~");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The indicator is overlaid rather than placed in the pane's flex column,
// and that is a behavioral choice, not a cosmetic one: a line that took
// height from the terminal would resize the pty every time anyone attached
// anything — twice per upload — reflowing the scrollback and making a
// full-screen agent redraw itself.
test("the attachment indicator never resizes the terminal", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 2_000));
      await route.continue();
    });
    const session = await openAttachmentSession(page, request, `attach-geometry-${stamp}`);
    id = session.id;

    const geometry = () =>
      page.evaluate(() => {
        const term = (window as any).__farhelmIslands["terminal"].term;
        return { cols: term.cols, rows: term.rows };
      });
    const before = await geometry();

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `geometry-${stamp}.txt`, mime: "text/plain", content: "x" }],
    });
    await expect(page.locator('[data-terminal="agent"] .attach-busy')).toBeVisible();
    expect(await geometry(), "the indicator must not take rows from the terminal").toEqual(before);

    await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`geometry-${stamp}\\.txt`)})`),
    );
    expect(await geometry(), "and it must not give them back either").toEqual(before);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A transfer belongs to the terminal that received it, and so does its
// failure. With several terminals on screen, an error reported anywhere
// but the receiving pane would have the user looking for a file they
// dropped somewhere else.
test("an attachment failure stays in the terminal that received it", async ({ page, request }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await route.fulfill({ status: 500, contentType: "text/plain", body: "refused" });
    });
    const session = await openAttachmentSession(page, request, `attach-isolation-${stamp}`);
    id = session.id;

    const tabId = await addTab(page, 0);
    const island = `terminal-${tabId}`;
    await waitForIslandMounted(page, island);
    await selectTerminal(page, tabId);

    await dispatchPayload(page, island, "drop", {
      entries: [{ name: `tabbed-${stamp}.txt`, mime: "text/plain", content: "x" }],
    });

    await expect(page.locator(`.terminal-pane[data-terminal="${tabId}"] .attach-error`))
      .toContainText(`tabbed-${stamp}.txt`);
    await expect(page.locator('[data-terminal="agent"] .attach-error')).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Dropping TEXT is not a paste: there is no handler underneath to fall
// through to, so terminal.js inserts it itself — while still preventing
// the default, because the engine's own handling of a dropped URL is to
// navigate there, which would take every terminal on the page down with
// it.
test("a text-only drop is prevented, inserted, and leaves the page where it was", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-droptext-${stamp}`);
    id = session.id;
    const before = page.url();

    const result = await dispatchPayload(page, "terminal", "drop", {
      text: `/etc/hosts-${stamp}`,
    });

    expect(result.dragover?.defaultPrevented, "no dragover, no drop").toBe(true);
    expect(result.dragover?.dropEffect, "the island advertises itself as a copy target")
      .toBe("copy");
    expect(result.event.defaultPrevented, "an unprevented text drop navigates the page").toBe(true);
    await waitForIslandLogicalText(page, "terminal", `/etc/hosts-${stamp}`);
    expect(page.url(), "and the page stayed where it was").toBe(before);
    expect(uploads.count(), "dropped text is text").toBe(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Interception has to happen BEFORE xterm's own paste handler, and only
// for the flavors it takes responsibility for. A file paste that reached
// xterm would paste the payload's text on top of the upload; a text paste
// that did NOT reach it would lose the paste entirely.
//
// Both are observed rather than inferred: the helper reports whether the
// event was cancelled, whether propagation was stopped, and whether it
// ever arrived at xterm's own textarea.
test("file pastes are cancelled before xterm sees them; text pastes are not", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ path: `/tmp/fh-${stamp}/attachments/s/x.txt` }),
      });
    });
    const session = await openAttachmentSession(page, request, `attach-cancel-${stamp}`);
    id = session.id;

    const intercepted = await dispatchPayload(page, "terminal", "paste", {
      entries: [{ name: "doc.txt", mime: "text/plain", content: "x" }],
      text: "would have been pasted",
    });
    expect(intercepted.event.defaultPrevented).toBe(true);
    expect(intercepted.event.propagationStopped).toBe(true);
    expect(
      intercepted.event.reachedTarget,
      "xterm must never see a paste this island took responsibility for",
    ).toBe(false);

    const passed = await dispatchPayload(page, "terminal", "paste", { text: `plain-${stamp}` });
    expect(passed.event.defaultPrevented, "xterm's own handler owns a text paste").toBe(false);
    expect(
      passed.event.reachedTarget,
      "a text paste has to arrive at the textarea xterm listens on, or the paste is simply lost",
    ).toBe(true);
    // Propagation IS stopped for this one — by xterm itself, on the way
    // back up, which is the behavior terminal.js's capture-phase listeners
    // exist to get in front of. What matters here is that it was not
    // stopped BEFORE the target, which `reachedTarget` above is the
    // evidence for.
    await waitForIslandLogicalText(page, "terminal", `plain-${stamp}`);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Some engines expose a payload as a bare `FileList` with no item list at
// all. terminal.js has a fallback for exactly that, and without a test it
// would be reachable only in production — on whichever engine happens to
// take that shape, which is the worst place to discover a typo.
test("a payload with files but no items takes the same path", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-fileslist-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [
        { name: `listed-${stamp}.txt`, mime: "text/plain", content: "from files", filesOnly: true },
      ],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`listed-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("from files");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});
