// Byte-level proof for the Shift+Enter "insert a newline" chord
// (shift-enter-key.js, wired into terminal.js's `attachCustomKeyEventHandler`
// — see that module's own header for the ESC-prefix design and why it
// PREFIXES rather than replaces xterm's own `\r`).
//
// `js-tests/shift-enter-key.test.js` already pins the pure decision function
// exhaustively — each modifier boundary the decision uses, IME composition,
// the `keyCode === 229` sentinel. What that unit suite cannot see is the
// browser-level wiring: a real `KeyboardEvent` reaching xterm's
// `attachCustomKeyEventHandler` through a real DOM, encoded and written down
// a real WebSocket, landing as real bytes in a real pty. This file is that
// proof, end to end, for the three chords that matter: Shift+Enter (must
// send `\x1b\r`, nothing else), plain Enter (must send bare `\r`, no leading
// ESC), and Ctrl+Shift+Enter — the actual boundary the fix's modifier check
// claims (shift held, but disqualified by a second modifier) — which must
// NOT trigger this file's own ESC injection, whatever xterm sends for it
// natively.
//
// ## Why a raw-mode `od` dump, not a WebSocket `send` patch
//
// terminal.spec.ts's DECRPM regression proves an INPUT-shape claim by
// patching `WebSocket.prototype.send` and inspecting frames the browser
// queued to leave — a fine proof that the CLIENT never emitted a byte, but
// silent on what the server side actually received: encoding and pty
// line-discipline translation could still be wrong on the wire, and that
// pattern would not catch it. This file instead proves the byte arrived by
// having the PTY ITSELF report what it read: the session's own invocation
// puts the pty in a noncanonical, no-echo mode and hex-dumps every byte it
// receives back onto the screen, so the assertions below read the actual
// terminal buffer rather than a client-side interception.
//
// ## The exact `stty`/`od` incantation, and why each flag is there
//
// `stty raw` alone is the wrong tool here even though it is the usual
// one-liner for "show me raw keystrokes": it disables BOTH input processing
// (good — no ICRNL translating a genuine `\r` into `\n` before this test can
// see it) AND output processing (bad — with `opost`/`onlcr` off, `od`'s own
// bare `\n` moves the cursor down a row without returning it to column 0,
// so consecutive one-line dumps drift diagonally across the screen and can
// end up split across rows by the terminal's autowrap). The incantation
// below disables only what has to go — echo, canonical (line-buffered)
// input, and CR/NL input translation — while leaving `opost`/`onlcr` alone,
// so every dumped byte lands on its own clean row. This is a noncanonical,
// no-echo mode, NOT `stty raw`'s full raw mode — the byte-exactness this
// file claims is scoped to what these tests actually send (ESC, CR, and
// printable sentinel characters), not every possible control byte:
//
//   stty -echo -icanon -icrnl -inlcr -igncr min 1 time 0
//
// `od`'s own defaults are equally unsafe for a byte-at-a-time proof: any
// dump width greater than one buffers input until it has a FULL row before
// printing anything — verified directly against a real pty (an 8-byte
// width, fed a two-byte write, produced no output at all until an eighth
// byte arrived) — so a two- or one-byte keystroke would simply never
// appear on its own. `-w1` fixes that (one byte, one line, flushed
// immediately), but introduces a second trap: `od` silently collapses a run
// of IDENTICAL consecutive lines into a bare `*`, which would make "exactly
// one `\r` arrived" indistinguishable from "several arrived, collapsed to
// one glyph" — exactly the ambiguity the shift-enter fix's whole promise
// (one `\r`, not a duplicate) has to rule out. `-v` (`--output-duplicates`)
// disables that collapsing. All four flags were confirmed against a real
// pty pair (a Python `pty.openpty()` harness) before writing this file.
//
// ## Reading GROWTH, not a text SLICE
//
// An early version of this file computed a "before" snapshot as the
// buffer's own STRING LENGTH and looked for new content past that offset —
// which is wrong, and confirmed wrong against a real browser run: xterm's
// buffer reports `buf.length` rows for the WHOLE viewport, not just the
// rows actually written to, so a freshly attached terminal already carries
// dozens of blank trailing rows before a single keystroke is sent. Their
// length is real length, so "before" already points near the tail of a
// mostly-empty buffer — and a keystroke's own dump lands a couple of rows
// below the `RAWREADY` marker, in the middle of the string, never past
// that tail offset. `bytesIn` (below) sidesteps the whole problem: it scans
// the CURRENT FULL text for every od-shaped byte LINE, in the order they
// appear top to bottom — which is also temporal order, since `od` only
// ever appends the next byte to the next unused row — so comparing the
// CUMULATIVE array before and after a keystroke is safe regardless of how
// much blank padding the viewport still carries.
//
// ## Sentinel fencing
//
// Every chord below is followed by a distinct, typed printable byte before
// its assertion runs, and the assertion checks the COMPLETE ordered delta
// through that sentinel rather than stopping the instant it first sees a
// match. Without the sentinel, a poll that resolves as soon as it observes
// the expected prefix cannot rule out one more byte — a duplicate `\r`,
// say — landing a few milliseconds later: the assertion would already have
// passed and stopped looking. Waiting for the sentinel first, THEN reading
// the whole delta through it in one shot, closes that trailing-duplicate
// window.
import { expect, test, type Page } from "@playwright/test";
import { cleanupSession, createSession } from "./helpers/fleet";
import { attachSession, termText, waitForTermText } from "./helpers/term";

/**
 * Every hex byte VALUE `od -v -An -tx1 -w1` has printed so far, in order,
 * read off the CURRENT full terminal buffer.
 *
 * Each of `od`'s lines is matched WHOLE — `/^ ?[0-9a-f]{2}$/` against the
 * complete (already right-trimmed) row text — rather than searching for a
 * hex-shaped substring anywhere in the buffer: a substring match would
 * treat any OTHER line that happened to end in two hex-looking characters
 * as one of this fixture's bytes, which a line-anchored match cannot do by
 * construction. Reading the WHOLE buffer on every poll, rather than trying
 * to isolate what changed since some earlier snapshot, is what makes this
 * safe to call repeatedly — see this file's header for why a length-based
 * "what's new" slice does not work here.
 */
function bytesIn(text: string): string[] {
  const bytes: string[] = [];
  for (const line of text.split("\n")) {
    const match = /^ ?([0-9a-f]{2})$/.exec(line);
    if (match) bytes.push(match[1]);
  }
  return bytes;
}

/** Whether `bytes` contains an ESC (`1b`) immediately followed by a CR
 * (`0d`) anywhere — this file's own injected prefix's exact shape, checked
 * as a SUBSEQUENCE-of-two rather than requiring it to start at index 0, so
 * it can be asked of a delta that also carries a sentinel byte after it. */
function hasEscCrPair(bytes: string[]): boolean {
  return bytes.some((b, i) => b === "1b" && bytes[i + 1] === "0d");
}

/**
 * The fixture invocation: a noncanonical, no-echo, byte-exact hex dump of
 * everything the pty receives — see this file's header for why each `stty`
 * and `od` flag is there.
 *
 * Gated behind `read _gate`, the same idiom terminal-clipboard.spec.ts's
 * OSC 52 test uses and for the analogous reason, but for a DIFFERENT race:
 * that test guards against its printf racing this test's own attach; this
 * one guards against a chord racing the shell's OWN startup — the login
 * shell wrapping every invocation (launch.rs) can print its own banner text
 * before reaching this script, and more importantly the pty is still in the
 * shell's default cooked/echoing mode until `stty` actually runs. Releasing
 * the gate with an ordinary typed line (in that default cooked mode) then
 * waiting for the `RAWREADY` marker — chained with `&&` so it can only print
 * once `stty` itself has SUCCEEDED, rather than after it merely ran — is
 * what proves noncanonical mode has already taken effect before any test
 * sends a single real keystroke. `RAWREADY` itself prints with `opost`
 * still enabled, so it renders as ordinary text regardless of anything
 * downstream.
 */
const RAW_DUMP_INVOCATION =
  "sh -c 'read _gate && stty -echo -icanon -icrnl -inlcr -igncr min 1 time 0 && " +
  "printf \"RAWREADY\\n\" && od -v -An -tx1 -w1'";

/**
 * The read-boundary variant of [`RAW_DUMP_INVOCATION`]: same gate and
 * `stty` preamble, but the dump prints one LINE per `read(2)` — each
 * iteration's `dd bs=4096 count=1` performs exactly one read, and the
 * bytes it got are printed as concatenated hex terminated by `|`.
 *
 * This is the fixture for the single-write delivery claim: `od` alone can
 * never show where one pty write ended and the next began, and the whole
 * Shift+Enter bug was ABOUT that boundary (a split pair reads as
 * Escape-then-Enter to boundary-sensitive line editors; see
 * shift-enter-key.js's header). One caveat keeps this observation honest
 * in one direction only: bytes arriving while `dd` is between reads
 * coalesce in the pty buffer, so a split COULD read as one line on a slow
 * machine — but a genuinely single write can never read as two. The
 * DETERMINISTIC half of the claim therefore lives beside it in the same
 * test: a websocket-frame capture asserting the chord leaves the browser
 * as exactly one two-byte frame, which no pty timing can blur.
 */
const READ_GROUPED_DUMP_INVOCATION =
  "sh -c 'read _gate && stty -echo -icanon -icrnl -inlcr -igncr min 1 time 0 && " +
  "printf \"RAWREADY\\n\" && while :; do dd bs=4096 count=1 2>/dev/null | od -v -An -tx1 | " +
  "tr -d \" \\n\"; printf \"|\\n\"; done'";

/**
 * Attach a raw-dump fixture session, release its `read _gate`, and wait for
 * `RAWREADY` — the identical three-step preamble every test in this file
 * needs before it may send a single real keystroke, folded into one call
 * rather than repeated at each call site.
 */
async function attachRawDumpSession(page: Page, id: string): Promise<void> {
  await attachSession(page, id);
  // (per terminal-clipboard.spec.ts's own hard-won lesson) past the RARE
  // backstop path terminal.js falls back to when its pre-mount font
  // settling gives up before the real load lands (its own "## Font
  // settling before mount" header): that backstop triggers a real resize
  // of the pty (a fresh `fit()` call), and letting it settle before typing
  // keeps a resize's own SIGWINCH-driven repaint from landing in the
  // middle of a byte-level dump this file is about to start asserting on
  // exactly.
  await page.evaluate(() => (document as any).fonts.ready.then(() => undefined));
  // Release the gate in the shell's default cooked/echoing mode — any
  // single line does, its content is unimportant — then wait for the
  // marker `stty` itself gates (see `RAW_DUMP_INVOCATION`'s own doc).
  await page.keyboard.type("go");
  await page.keyboard.press("Enter");
  await waitForTermText(page, "RAWREADY");
}

test.describe("Shift+Enter's ESC-prefix reaches the pty as exact bytes", () => {
  test("Shift+Enter sends ESC then CR — one pair, no duplicate CR", async ({ page, request }) => {
    const session = await createSession(request, {
      title: `keys-shift-enter-${Date.now()}`,
      cwd: "/tmp",
      invocation: RAW_DUMP_INVOCATION,
    });
    try {
      await page.goto("/");
      await attachRawDumpSession(page, session.id);

      const before = bytesIn(await termText(page));
      await page.keyboard.press("Shift+Enter");
      // `shiftEnterKeyAction` returns "arm" for this chord and
      // `mergeArmedPrefix` glues the `\x1b` onto xterm's own ordinary `\r`
      // (see shift-enter-key.js's docs): one message carrying both bytes,
      // down the same path as ordinary input. This test pins the byte
      // VALUES; the read-boundary test below pins the one-write shape.
      await page.keyboard.press("z");
      await waitForTermText(page, " 7a");

      // The COMPLETE delta through the sentinel, not merely its prefix —
      // see this file's header on sentinel fencing for why that closes the
      // trailing-duplicate window a "contains" or first-match check leaves
      // open.
      const delta = bytesIn(await termText(page)).slice(before.length);
      expect(delta).toEqual(["1b", "0d", "7a"]);
    } finally {
      await cleanupSession(request, session.id);
    }
  });

  test("plain Enter sends bare CR — no ESC prefix", async ({ page, request }) => {
    const session = await createSession(request, {
      title: `keys-plain-enter-${Date.now()}`,
      cwd: "/tmp",
      invocation: RAW_DUMP_INVOCATION,
    });
    try {
      await page.goto("/");
      await attachRawDumpSession(page, session.id);

      const before = bytesIn(await termText(page));
      await page.keyboard.press("Enter");
      // `shiftEnterKeyAction` returns "pass" for a plain Enter and the
      // unarmed merge is byte-transparent — see shift-enter-key.js's docs —
      // so nothing here should ever send a leading ESC; only xterm's own
      // ordinary `\r` should land.
      await page.keyboard.press("z");
      await waitForTermText(page, " 7a");

      const delta = bytesIn(await termText(page)).slice(before.length);
      expect(delta).toEqual(["0d", "7a"]);
    } finally {
      await cleanupSession(request, session.id);
    }
  });

  test("Ctrl+Shift+Enter does not trigger this fix's own ESC injection", async ({
    page,
    request,
  }) => {
    const session = await createSession(request, {
      title: `keys-ctrl-shift-enter-${Date.now()}`,
      cwd: "/tmp",
      invocation: RAW_DUMP_INVOCATION,
    });
    try {
      await page.goto("/");
      await attachRawDumpSession(page, session.id);

      const before = bytesIn(await termText(page));
      // The actual boundary the fix's modifier check claims — shift IS
      // held, but a second modifier disqualifies it (shift-enter-key.js's
      // exact-match check, `!ev.ctrlKey`) — unlike plain Ctrl+Enter, which
      // never has `shiftKey` true in the first place and so never reaches
      // that check at all.
      await page.keyboard.press("Control+Shift+Enter");
      // A NEGATIVE claim, unlike the two tests above: this chord must fall
      // straight through to whatever xterm does with it natively — nothing,
      // its own default `\r`, or an application-mode sequence (which MAY
      // legitimately contain an ESC byte of xterm's own, unrelated to this
      // fix) are all fine; only OUR ESC-then-CR pair is not, which is why
      // this checks for that adjacent PAIR specifically rather than the
      // mere presence of `1b` anywhere.
      await page.keyboard.press("z");
      await waitForTermText(page, " 7a");

      const delta = bytesIn(await termText(page)).slice(before.length);
      // Checked before the sentinel, not the whole delta: the sentinel
      // itself is a plain "z" and could never form the pair, but slicing
      // it off keeps this assertion legible as "nothing the CHORD sent
      // forms the pair" rather than "nothing anywhere in the delta does".
      expect(
        hasEscCrPair(delta.slice(0, -1)),
        "no keystroke here should ever produce this fix's ESC-then-CR pair",
      ).toBe(false);
      expect(
        delta[delta.length - 1],
        "the sentinel must be the last byte observed, proving the pty stayed live through the chord",
      ).toBe("7a");
    } finally {
      await cleanupSession(request, session.id);
    }
  });
});

test.describe("Shift+Enter's ESC CR reaches the pane as one write", () => {
  test("the pane reads ESC CR in a single read, never a lone ESC", async ({ page, request }) => {
    const session = await createSession(request, {
      title: `keys-shift-enter-one-write-${Date.now()}`,
      cwd: "/tmp",
      invocation: READ_GROUPED_DUMP_INVOCATION,
    });
    try {
      await page.goto("/");
      await attachRawDumpSession(page, session.id);

      // The deterministic half of the claim: capture what the browser
      // actually sends. Read-boundary observation below is inherently
      // one-sided (bytes can coalesce in the pty buffer between the
      // fixture's reads), but the frame boundary is not — one websocket
      // frame carrying both bytes IS the single-write contract at its
      // source, and a regression back to the two-producer design shows up
      // here as two frames no matter how the pty timing falls.
      await page.evaluate(() => {
        const ws = (window as any).__farhelmWs;
        (window as any).__sentFrames = [];
        const orig = ws.send.bind(ws);
        ws.send = (payload: any) => {
          (window as any).__sentFrames.push(
            Array.from(new Uint8Array(payload))
              .map((b: number) => b.toString(16).padStart(2, "0"))
              .join(""),
          );
          orig(payload);
        };
      });
      await page.keyboard.press("Shift+Enter");
      const chordFrames = await page.evaluate(() => (window as any).__sentFrames);
      expect(
        chordFrames,
        "the chord must leave the browser as exactly one two-byte frame",
      ).toEqual(["1b0d"]);

      await page.keyboard.press("z");
      await waitForTermText(page, "7a|");

      const text = await termText(page);
      // The plumbing half: both bytes inside one read line. Matched as a
      // pattern rather than the exact line `1b0d|` because the sentinel
      // (or any later byte) can legitimately coalesce into the SAME read
      // while the fixture's `dd` is between reads — `1b0d7a|` is still a
      // pass, since the claim is "never split", not "read in isolation".
      expect(
        text,
        "the chord's two bytes must arrive within one read",
      ).toMatch(/1b0d[0-9a-f]*\|/);
      // The failure shape, named exactly: a read that ENDED on the lone
      // ESC. (`1b\|` cannot match inside `1b0d…|`, so this is a clean
      // negative.)
      expect(
        text,
        "no read may ever end on the chord's lone ESC",
      ).not.toMatch(/1b\|/);
    } finally {
      await cleanupSession(request, session.id);
    }
  });

  test("a plain Enter after the chord stays bare — the arm does not linger", async ({
    page,
    request,
  }) => {
    const session = await createSession(request, {
      title: `keys-shift-enter-disarm-${Date.now()}`,
      cwd: "/tmp",
      invocation: RAW_DUMP_INVOCATION,
    });
    try {
      await page.goto("/");
      await attachRawDumpSession(page, session.id);

      const before = bytesIn(await termText(page));
      // Same terminal, chord then plain Enter: the regression this pins is
      // wiring-level, not merge-level — an arm that survived its dispatch
      // (the microtask expiry in terminal.js dropped, or `merged.armed` no
      // longer assigned back) would ESC-prefix the SECOND Enter too, and
      // no fresh-terminal test can see that.
      await page.keyboard.press("Shift+Enter");
      await page.keyboard.press("Enter");
      await page.keyboard.press("z");
      await waitForTermText(page, " 7a");

      const delta = bytesIn(await termText(page)).slice(before.length);
      expect(
        delta,
        "one prefixed pair, then a bare CR — a phantom second ESC means a stale arm",
      ).toEqual(["1b", "0d", "0d", "7a"]);
    } finally {
      await cleanupSession(request, session.id);
    }
  });
});
