// Terminal sockets and input at the edges of the real terminal stack: an
// attach to an unknown session, a paste of more than a megabyte into a grid
// terminal, and real Backspace and Ctrl+C keypresses. A live flood's
// scrollback is in grid-flood.spec.ts.

import { expect, test } from "./helpers/evidence";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { DEVICE_SECRET_KEY, requireProductPageAuth } from "./helpers/device-auth";
import { attachSession, cleanupSession, termText, waitForTermText } from "./helpers/term";
import { stackScratchDir } from "./helpers/scratch";
import { installTerminalSuiteHooks, sharedSessionRow } from "./helpers/terminal-suite";

installTerminalSuiteHooks();

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
          `ws://${location.host}/api/sessions/00000000-0000-4000-8000-000000000000%3Ano-such-session/term`,
          ["farhelm", `farhelm-device-${secret}`],
        );
        // sleep-ok: fail if the peer never delivers a detach notice; elapsed time is not success.
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

/**
 * The receiver for the grid's large-paste test: once it says it is ready, it
 * reads exactly the number of bytes its argument names in raw mode, reports
 * how many arrived and their SHA-256 on two short lines (so neither wraps in
 * a terminal 80 columns wide), then echoes each line typed after it as
 * `echo:<line>`. Owned by that test, so a lost, cut short or altered paste
 * shows in its own report rather than in a shared agent's state.
 */
const PASTE_RECEIVER = `import hashlib, os, sys, termios, tty
want = int(sys.argv[1])
saved = termios.tcgetattr(0)
tty.setraw(0)
os.write(1, b"PASTE-RECEIVER-READY\\r\\n")
data = bytearray()
while len(data) < want:
    chunk = os.read(0, want - len(data))
    if not chunk:
        break
    data += chunk
os.write(1, ("PASTE-GOT %d\\r\\nPASTE-SHA %s\\r\\n" % (len(data), hashlib.sha256(data).hexdigest())).encode())
termios.tcsetattr(0, termios.TCSADRAIN, saved)
for line in sys.stdin:
    print("echo:" + line.rstrip("\\n"), flush=True)
`;

/**
 * A paste of more than 1 MiB into a grid terminal arrives whole, byte for
 * byte, and the attachment that carried it stays.
 *
 * Why: a grid terminal never sends one message that large (an input message over 64 KiB ends the
 * attachment by design); its paste splits into messages within the bound,
 * which the supervisor writes in order. A paste that lost, repeated or
 * reordered a part, or that the attachment dropped, would still let a later
 * line through, so the receiver's own count and digest are the oracle. The
 * paste is a 23-letter cycle, which no part boundary (the grid splits a
 * paste into 32 KiB parts) lines up with, so every part's text differs from
 * its neighbours' and a reordered or repeated part changes the digest.
 * Spec: in a session running `PASTE_RECEIVER`, a grid paste of 1 MiB + 1
 * bytes of that cycle is reported as exactly that many bytes with the
 * expected SHA-256, then a line typed after it is echoed, on the same, still
 * open socket.
 */
test("a paste of more than one megabyte into a grid terminal arrives whole and keeps the socket", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const pasteOverOneMiB = 1024 * 1024 + 1;
  const cycle = "abcdefghijklmnopqrstuvw";
  const payload = cycle.repeat(Math.ceil(pasteOverOneMiB / cycle.length)).slice(0, pasteOverOneMiB);
  const digest = createHash("sha256").update(payload, "utf8").digest("hex");
  const dir = stackScratchDir("paste-receiver-");
  const script = path.join(dir, "receiver.py");
  fs.writeFileSync(script, PASTE_RECEIVER);
  const created = await request.post("/api/sessions", {
    data: {
      cwd: "/tmp",
      command: { command: `python3 ${script} ${pasteOverOneMiB}`, yolo: false },
      title: `grid-big-paste-${Date.now()}`,
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  const id: string = (await created.json()).id;
  try {
    await page.goto("/");
    await attachSession(page, id);
    await waitForTermText(page, "PASTE-RECEIVER-READY");
    await page.evaluate((text) => {
      const w = window as any;
      w.__farhelmPasteSocket = w.__farhelmWs;
      if (!w.__farhelmGrid.paste(text)) throw new Error("the grid refused the paste");
    }, payload);
    await expect
      .poll(
        () =>
          page.evaluate(() => {
            const ws = (window as any).__farhelmWs as WebSocket;
            return { state: ws.readyState, buffered: ws.bufferedAmount };
          }),
        { timeout: 30_000, message: "socket must stay open and drain" },
      )
      .toEqual({ state: WebSocket.OPEN, buffered: 0 });
    await waitForTermText(page, `PASTE-GOT ${pasteOverOneMiB}\nPASTE-SHA ${digest}`, 60_000);
    await page.keyboard.type("after-big-message");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:after-big-message");
    expect(await page.evaluate(() => {
      const original = (window as any).__farhelmPasteSocket as WebSocket;
      return original === (window as any).__farhelmWs && original.readyState === WebSocket.OPEN;
    }), "the paste must arrive on the same, still open attachment").toBe(true);
  } finally {
    await cleanupSession(request, id);
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// Regression test for the tmux paste-buffer input-mangling bug at the browser
// level: real Backspace and Ctrl+C keypresses go through the grid's own key
// handling (grid key messages the supervisor encodes as DEL and ETX — no custom
// key binding intercepts them, see terminal.js), the WebSocket, and the framing
// protocol exactly like any other keystroke, landing on `basic`'s pty in its
// ordinary canonical/cooked mode (nothing here puts it in raw mode, unlike the
// `hexecho` fixture a Rust e2e test once used for byte-exact coverage of every
// mangled control byte, including ESC/arrow-up).
//
// ArrowUp is deliberately NOT exercised here, after checking what a
// canonical-mode pty actually does with it: Linux's ECHOCTL local-echo
// renders ANY control byte with no special canonical role — ESC included —
// as two-character caret notation ("^["), regardless of whether it arrived
// as a genuine 0x1b byte or as the bug's literal caret text. The pane's
// rendered output is identical either way, so `basic`'s canonical pty gives
// no browser-observable signal for ESC at all; that gap is exactly what
// `hexecho`'s raw mode (no ECHOCTL, no canonical processing) exists to
// close. The Rust e2e test that covered it end to end ran on tmux terminals
// and is gone, so that end-to-end check is not covered any more; the grid
// input encoder's key table still pins the bytes each key sends.
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
// fifteen seconds for a banner that a multi-megabyte flood test, which then
// ran before them in this file, had already pushed out of the pane's history. It failed exactly once per run,
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
  // banner, which this test does not need, and which a multi-megabyte
  // flood test that used to run just before it in this file had pushed out
  // of the shared session's history. Liveness is proven below with a fresh
  // marker instead. Still has to go through the list view to get there,
  // same as openTerminal does.
  await page.goto("/");
  const row = sharedSessionRow(page);
  await expect(row).toBeVisible({ timeout: 20_000 });
  const id = await row.getAttribute("data-session-id");
  expect(id, "the shared session row must identify the terminal it opens").toBeTruthy();
  await attachSession(page, id!);
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
  // SIGINT kills it. Typing a fresh marker and checking that it does not echo
  // during the observation window catches that failure; an absence needs repeated observation
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
    // sleep-ok: sample forbidden echo during a finite window; the deadline is checked between reads.
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
});
