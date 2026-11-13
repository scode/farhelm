// Emoji widths must agree with the two-cell cursor coordinates agents use.
// A real program prints every case; the reconnect case separately exercises
// the supervisor's replay of its own copy of the screen and the cursor.
// Geometry checks the cell layout, so these tests need no colour-emoji font.

import { type APIRequestContext, type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { attachSession, cleanupSession, sendTerminalInput, terminalSize, waitForTermText } from "./helpers/term";
import { installTerminalSuiteHooks, reconnectTimingsFromNextLoad } from "./helpers/terminal-suite";
import { waitForReplayReveal, waitForSessionRevealed } from "./helpers/terminal-readiness";

installTerminalSuiteHooks();

const REPLAY_ROW = "⚠️R✅S✔️T⏭️U";
const CURSOR_PREFIX = "REPLAY-CURSOR:";

/**
 * Keep an owned command quiet between explicit requests, without a shell prompt
 * or input echo changing the rows under measurement. The ready reply proves
 * live output reached this test's post-replay attachment. The replay request
 * leaves the terminal's cursor immediately after a VS16 emoji, where a one-cell
 * width disagreement becomes observable when the next request prints X.
 */
const UNICODE_COMMAND = `sh -c 'stty -echo && printf "UNICODE-READY\\n" && while IFS= read -r input; do case "$input" in ready) printf "UNICODE-LIVE\\n" ;; replay) printf "\\033[2J\\033[H${REPLAY_ROW}\\r\\n${CURSOR_PREFIX}⚠️" ;; narrow) printf "\\033[2J\\033[H⌚︎\\033[41mX\\033[0m" ;; continue) printf "X\\r\\nREPLAY-LIVE\\r\\n" ;; esac; done'`;

/**
 * Create and reveal a quiet terminal, retaining cleanup ownership until the
 * caller receives its id. A parsed ready reply, rather than an open socket or
 * replayed banner alone, establishes that the command is alive and reading.
 */
async function openUnicodeTerminal(page: Page, request: APIRequestContext): Promise<string> {
  const created = await request.post("/api/sessions", {
    data: {
      cwd: "/tmp",
      title: `terminal-unicode-${Date.now()}`,
      command: { command: UNICODE_COMMAND, yolo: false },
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  const id: string = (await created.json()).id;
  try {
    await page.goto("/");
    await attachSession(page, id);
    await waitForTermText(page, "UNICODE-READY");
    await sendCommand(page, id, "ready");
    await waitForTermText(page, "UNICODE-LIVE");
    const size = (await terminalSize(page))!;
    expect(size.cols, "the fixture needs room for the whole status line without wrapping").toBeGreaterThanOrEqual(40);
    expect(size.rows, "the emoji rows must all be visible for DOM assertions").toBeGreaterThanOrEqual(8);
    return id;
  } catch (error) {
    await cleanupSession(request, id);
    throw error;
  }
}

/**
 * Send one request only to this session's revealed attachment. The command's
 * request/reply protocol establishes peer liveness before each dependent step;
 * the socket check also catches a disconnect between readiness and this send.
 */
async function sendCommand(page: Page, id: string, command: string): Promise<void> {
  await waitForSessionRevealed(page, id);
  await sendTerminalInput(page, `${command}\n`);
}

/**
 * Compare the same visible screen rows in the buffer and DOM. Returning only
 * the requested rows keeps polling and timeout diagnostics bounded; trimming
 * trailing blanks preserves interior spaces where cursor misalignment appears.
 * Replay can serialize blank padding as printable cells; normalize that padding
 * in both views instead of assuming the buffer API's trim flag removes it all.
 * A grid terminal is read the same way: its model's screen rows and its
 * drawn row elements, at the bottom when its view follows the output.
 */
async function expectRows(page: Page, expected: string[]): Promise<void> {
  await expect.poll(() => page.evaluate((count) => {
    const grid = (window as any).__farhelmIslands?.terminal?.grid;
    const snapshot = grid?.snapshot();
    const drawn = document.querySelectorAll("#terminal .grid-terminal-row");
    return {
      buffer: Array.from({ length: count }, (_, row) => (snapshot?.lines[row] ?? "").trimEnd()),
      rendered: Array.from({ length: count }, (_, row) => drawn[row]?.textContent?.trimEnd() ?? ""),
      atTail: !!grid && grid.viewport().following,
    };
  }, expected.length), { timeout: 10_000, message: "the buffer and painted emoji rows must agree" }).toEqual({
    buffer: expected,
    rendered: expected,
    atTail: true,
  });
}

/**
 * A text selector can narrow a normally wide glyph. The supervisor supplies
 * that cell's width; the browser must respect it even when the font's default
 * glyph is wide, or the coloured next cell covers part of the glyph's box.
 */
test("a narrowed text-presentation glyph owns one grid cell", async ({ page, request }) => {
  const id = await openUnicodeTerminal(page, request);
  try {
    await sendCommand(page, id, "narrow");
    await waitForTermText(page, "⌚︎X");
    await expectRows(page, ["⌚︎X"]);
    await expect.poll(() => page.evaluate(() => {
      const row = document.querySelector("#terminal .grid-terminal-row");
      const spans = Array.from(row?.children ?? []) as HTMLElement[];
      const glyph = spans.find((span) => span.textContent === "⌚︎");
      const next = spans.find((span) => span.textContent === "X");
      if (!glyph || !next) return null;
      const a = glyph.getBoundingClientRect();
      const b = next.getBoundingClientRect();
      const grid = (window as any).__farhelmIslands.terminal.grid;
      return {
        positive: b.width > 0,
        equalWidth: Math.abs(a.width - b.width) < 1,
        adjacent: Math.abs(b.left - a.right) < 1,
        cursor: grid.snapshot().cursor.x,
        coloured: !/^(transparent|rgba\(0, 0, 0, 0\))$/.test(getComputedStyle(next).backgroundColor),
      };
    }), { message: "the narrowed glyph and coloured neighbour need adjacent one-cell boxes" }).toEqual({
      positive: true,
      equalWidth: true,
      adjacent: true,
      cursor: 2,
      coloured: true,
    });
  } finally {
    await cleanupSession(request, id);
  }
});

/**
 * Reconnect reconstructs rows from the supervisor's own copy of the screen
 * rather than replaying the live output. A new socket, a marker-completed
 * replay, and a subsequent command reply establish that both the replayed
 * VS16 widths and the restored cursor agree with the newly mounted terminal.
 * (The title still says tmux, from when it ran on old terminals; reports
 * cite it by that title, so it is kept.)
 */
test("tmux reconnect replay restores emoji rows and the cursor column", async ({ page, request }) => {
  await reconnectTimingsFromNextLoad(page, { delaysMs: [50, 100, 200, 400, 800, 1600] });
  const id = await openUnicodeTerminal(page, request);
  try {
    await sendCommand(page, id, "replay");
    await waitForTermText(page, `${CURSOR_PREFIX}⚠️`);
    await expectRows(page, [REPLAY_ROW, `${CURSOR_PREFIX}⚠️`]);
    await page.evaluate(() => {
      const win = window as any;
      win.__unicodePriorSocket = win.__farhelmIslands.terminal.ws;
      win.__unicodePriorSocket.close();
    });
    await expect.poll(() => page.evaluate(() => {
      const win = window as any;
      const island = win.__farhelmIslands?.terminal;
      return !!island && island.ws !== win.__unicodePriorSocket;
    }), { timeout: 20_000, message: "reconnect must replace the original terminal attachment" }).toBe(true);
    await waitForSessionRevealed(page, id);
    const replay = await waitForReplayReveal(page, "terminal");
    // The catch-up ends at the replay's end marker; a grid replay is frames
    // the component applies as they arrive.
    expect(replay.revealReason).toBe("marker");
    await expectRows(page, [REPLAY_ROW, `${CURSOR_PREFIX}⚠️`]);
    const cursor = await page.evaluate(
      () => (window as any).__farhelmIslands.terminal.grid.snapshot().cursor as { x: number; y: number },
    );
    // The restored grid must keep the selector's two-cell layout, so live
    // output below continues at the same column as uninterrupted output.
    expect(cursor, "replayed VS16 emoji must leave the restored cursor two cells later").toMatchObject({
      x: CURSOR_PREFIX.length + 2,
      y: 1,
    });
    await sendCommand(page, id, "continue");
    await waitForTermText(page, "REPLAY-LIVE");
    await expectRows(page, [REPLAY_ROW, `${CURSOR_PREFIX}⚠️X`, "REPLAY-LIVE"]);
    // The grid's rows above place X right after the emoji; its cells have
    // no separate reader.
  } finally {
    await cleanupSession(request, id);
  }
});
