// Exercise terminal output under sustained load: watermark flow control,
// stalled-client detachment, replay after detachment, and bounded reconnect
// behavior all run against the real terminal stack.

import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import path from "node:path";
import { DEVICE_SECRET_KEY, requireProductPageAuth } from "./helpers/device-auth";
import { cleanupSession, termText, waitForTermText } from "./helpers/term";
import {
  installTerminalSuiteHooks,
  openTerminal,
  reconnectTimingsFromNextLoad,
  sharedSessionRow,
} from "./helpers/terminal-suite";

installTerminalSuiteHooks();

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
    // Start the verifier's hard cap in this same page turn, before the gate
    // opens, so a delayed first Node-side sample cannot extend the budget.
    const verifier = (window as any).__farhelmFloodVerify;
    if (verifier) verifier.monitorStartedAt = performance.now();
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
 * The caller clears Playwright's context-level bearer header, and this
 * socket supplies the same device subprotocol as terminal.js. That pairing
 * is deliberate: Chromium otherwise authenticates the upgrade from the
 * ambient header while WebKit does not, masking a missing product credential
 * in one engine and failing it in the other.
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
    ({ id, cols, rows, secretKey }) =>
      new Promise<void>((resolve, reject) => {
        const secret = localStorage.getItem(secretKey);
        if (!secret) {
          reject(new Error("the drain page has no device secret"));
          return;
        }
        const ws = new WebSocket(
          `ws://${location.host}/api/sessions/${id}/term?cols=${cols}&rows=${rows}`,
          ["farhelm", `farhelm-device-${secret}`],
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
    { id, ...geometry, secretKey: DEVICE_SECRET_KEY },
  );
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
      // The gate sender arms this clock immediately before releasing the
      // producer. Recording progress where bytes are verified, rather than
      // when Node later samples the count, preserves stalls hidden by a busy
      // browser or delayed debugging-protocol round trip.
      monitorStartedAt: null as number | null,
      lastProgressAt: null as number | null,
      maxProgressGap: 0,
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
        const recordsBefore = state.recordsSeen;
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
        if (state.recordsSeen > recordsBefore || state.sawDone) {
          const now = performance.now();
          if (state.monitorStartedAt !== null) {
            const previous = Math.max(
              state.lastProgressAt ?? state.monitorStartedAt,
              state.monitorStartedAt,
            );
            state.maxProgressGap = Math.max(state.maxProgressGap, now - previous);
          }
          state.lastProgressAt = now;
        }
      }
      return real.call(this, data, cb);
    };
  });
}

/**
 * Wait for the whole-stream verifier to reach the flood's terminal marker.
 *
 * Progress, rather than total throughput, is the useful liveness signal for
 * this load test. WebKit can keep consuming the stream correctly while a
 * loaded runner falls behind the fixed completion budget that a buffer-text
 * poll would impose. Each newly verified record therefore renews a bounded
 * no-progress budget, while an independent absolute cap still prevents a
 * merely slow producer from holding the suite forever. The verifier is also
 * the cheapest observation seam: unlike `termText`, reading its constant-size
 * state does not reconstruct the terminal's 12,000-line retained buffer on
 * every poll.
 */
async function waitForFloodStreamComplete(page: Page) {
  const noProgressTimeout = 15_000;
  const absoluteTimeout = 90_000;
  const pollInterval = 250;
  let recordsSeen = -1;
  let progressAge = 0;
  let elapsed = 0;

  for (;;) {
    const remainingProgress = Math.max(1, noProgressTimeout - progressAge);
    const remainingAbsolute = Math.max(1, absoluteTimeout - elapsed);
    const readBudget = Math.min(remainingProgress, remainingAbsolute);
    let readTimer: ReturnType<typeof setTimeout> | undefined;
    const verify = await Promise.race([
      page.evaluate(() => {
        const state = (window as any).__farhelmFloodVerify;
        if (!state) return null;
        const now = performance.now();
        state.monitorStartedAt ??= now;
        const lastProgress = Math.max(
          state.lastProgressAt ?? state.monitorStartedAt,
          state.monitorStartedAt,
        );
        return {
          recordsSeen: state.recordsSeen as number,
          sawDone: state.sawDone as boolean,
          error: state.error as string | null,
          progressAge: now - lastProgress,
          maxProgressGap: state.maxProgressGap as number,
          elapsed:
            (state.sawDone && state.lastProgressAt !== null
              ? state.lastProgressAt
              : now) - state.monitorStartedAt,
        };
      }),
      new Promise<never>((_, reject) => {
        readTimer = setTimeout(() => {
          const deadline =
            remainingProgress <= remainingAbsolute
              ? `flood verifier made no observable progress for ${noProgressTimeout}ms`
              : `flood verifier was not observable within its ${absoluteTimeout}ms hard cap`;
          reject(
            new Error(
              `${deadline}; last verified ${recordsSeen}/${FLOOD_RECORDS} records`,
            ),
          );
        }, readBudget);
      }),
    ]).finally(() => {
      if (readTimer !== undefined) clearTimeout(readTimer);
    });
    if (!verify) {
      throw new Error("whole-stream flood verifier was not installed");
    }
    if (verify.error) {
      throw new Error(`whole-stream flood verifier failed: ${verify.error}`);
    }
    recordsSeen = verify.recordsSeen;
    progressAge = verify.progressAge;
    elapsed = verify.elapsed;
    if (elapsed >= absoluteTimeout) {
      throw new Error(
        `flood did not reach FLOOD-DONE within ${absoluteTimeout}ms; verified ${recordsSeen}/${FLOOD_RECORDS} records`,
      );
    }
    if (
      progressAge >= noProgressTimeout ||
      verify.maxProgressGap >= noProgressTimeout
    ) {
      throw new Error(
        `flood made no progress for ${noProgressTimeout}ms; verified ${recordsSeen}/${FLOOD_RECORDS} records`,
      );
    }
    if (verify.sawDone) return;

    await page.waitForTimeout(
      Math.min(
        pollInterval,
        noProgressTimeout - progressAge,
        absoluteTimeout - elapsed,
      ),
    );
  }
}

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
  test.setTimeout(150_000);
  const title = `flood-complete-${Date.now()}`;
  let id: string | undefined;
  try {
    id = await openFloodSession(page, request, title, {
      holdWrites: true,
      verifyStream: true,
    });

    await waitForFloodStreamComplete(page);
    // The verifier sees the marker immediately before handing its chunk to
    // xterm. Keep one buffer-level wait after throughput is no longer part of
    // the deadline so the retained-tail assertion still covers what rendered.
    await waitForTermText(page, "FLOOD-DONE");

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
    const retainedText = await termText(page);
    expect(retainedText).toContain("FLOOD-DONE");
    const records = parseFloodRecords(retainedText);
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
  // Short rungs so the stall carve-out asserted after the detach is
  // asserted against a ladder that would have fired several times over by
  // then — with the shipped 0.5s-first rung the "nothing reattached"
  // window would be a claim about half a second.
  await reconnectTimingsFromNextLoad(page, {
    delaysMs: [50, 50, 50, 50, 50, 50],
    probeIntervalMs: 100,
  });
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

    // PLAN_M6.md item 7's stall CARVE-OUT, asserted here because this is
    // the only place in the suite where a real stall detach exists: a
    // client detached for stalling must NOT auto-reconnect. Reconnecting
    // into the same wedge helps nobody — this client is still swallowing
    // its write callbacks, so a fresh attachment would stall out again on
    // its own — and the user is supposed to act first.
    //
    // The ladder was tuned short before this page loaded, so waiting a
    // second here is waiting out several rungs of it; a client that was
    // going to bounce back has had many chances by now. Three
    // observations, because each fails differently: no fresh socket (the
    // island still holds the one it was detached on), no recovery surface,
    // and no manual control offering a way out of a state that is not a
    // connection failure.
    const stalledSocketUnchanged = await page.evaluate(
      () => ((window as any).__farhelmStalledWs = (window as any).__farhelmIslands["terminal"].ws)
        !== undefined,
    );
    expect(stalledSocketUnchanged).toBe(true);
    await page.waitForTimeout(1_000);
    expect(
      await page.evaluate(
        () =>
          (window as any).__farhelmIslands["terminal"].ws
            === (window as any).__farhelmStalledWs,
      ),
      "a stall detach is a decision, not a dropped connection: nothing may reattach on its own",
    ).toBe(true);
    // The placeholder is asserted by its ROLE, not its visibility: this
    // fixture swallows every write callback, so the original attach's
    // catch-up placeholder is still up (nothing ever revealed it) — which
    // is unrelated to, and would mask, the thing under test. What must not
    // be there is the RECOVERY surface, which carries its phase as an
    // attribute.
    await expect(page.locator("#term-connecting")).not.toHaveAttribute(
      "data-reconnect-phase",
      /.*/,
    );
    await expect(page.locator(".terminal-reconnect-now")).toHaveCount(0);
    await expect(page.locator("#term-banner")).toContainText("stalled");

    // Restore real rendering before reattaching, or the replay below
    // would be exactly as invisible as the stall that produced it.
    await page.evaluate(() => {
      (window as any).Terminal.prototype.write = (window as any).__testRealWrite;
      delete (window as any).__testRealWrite;
    });

    // Reattach the same way terminal.spec.ts's "switching sessions tears
    // down the mounted terminal; reselecting mounts a fresh one" test does.
    // Bounce to another session, then back. The
    // session survived the detach (SPEC.md: no viewer can affect a
    // session it stalls out of), so replay brings back its own tail —
    // already complete in tmux history well before the stall elapsed, so
    // no second gate byte is needed.
    // A real unmount/remount: bounce through the shared session (there
    // is no back), then reselect.
    await sharedSessionRow(page).click();
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
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
  await requireProductPageAuth(page.context());
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

    // Leaving means selecting another session now: bounce to the shared
    // row, which detaches the flood session (the delete-own-object teardown
    // property itself is pinned by terminal.spec.ts's "switching sessions
    // tears down the mounted terminal; reselecting mounts a fresh one" test;
    // here only the DETACH matters).
    await sharedSessionRow(page).click();
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);

    // The flood session has no attachment now — the page holds the shared
    // session's — so the flood can finish without any of it landing on
    // the attachment this test is about to measure.
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
// opens sockets for sessions the API listed. The test clears Playwright's
// ambient page header and supplies the product device subprotocol itself,
// so Chromium cannot pass through a credential WebKit never sends.
test("a terminal socket for an unknown session reports why", async ({
  page,
}) => {
  await requireProductPageAuth(page.context());
  await page.goto("/");
  const notice = await page.evaluate(
    (secretKey) =>
      new Promise<string>((resolve, reject) => {
        const secret = localStorage.getItem(secretKey);
        if (!secret) {
          reject(new Error("the unknown-session page has no device secret"));
          return;
        }
        const ws = new WebSocket(
          `ws://${location.host}/api/sessions/no-such-session/term`,
          ["farhelm", `farhelm-device-${secret}`],
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
    DEVICE_SECRET_KEY,
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
// This flood pair lives in its own spec file because this payload's PTY
// echo pollutes the terminal state — observed directly: a fresh page's
// READY wait found only a wall of echoed 'a's. (How much echoes is bounded
// by canonical-mode input handling and was not pinned down; the containment
// rule, not the mechanism, is the contract.) This file's per-file stack
// reset contains that damage before another terminal area runs.
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
// The containment rule has teeth, and PLAN_M6.md item 7's reconnect tests
// learned it the hard way: they were written against the shared session,
// passed every targeted run (where `beforeAll` hands them a fresh one), and
// failed in the FULL suite on both engines — the first of them waiting
// fifteen seconds for a banner that the multi-megabyte test above had
// already pushed out of the pane's history. It failed exactly once per run,
// which made it look like a flake: Playwright restarts its worker after a
// failure, and the new worker's `beforeAll` recreates the fixture for
// everything that follows. This file's per-file stack reset now contains
// that damage before the next terminal spec starts.
//
// This does NOT end the tmux session, despite ending the fake-agent
// process: `remain-on-exit on` (SPEC.md) keeps a dead pane's session and
// window around so its terminal stays viewable, it just stops accepting
// input. Killing the agent is still sufficient for the assertion; this
// file's per-file stack reset contains its destruction of the shared
// fixture before another terminal spec begins.
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
