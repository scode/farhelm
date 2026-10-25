// Emoji widths must agree with the two-cell cursor coordinates agents use.
// Direct writes exercise the mounted terminal's parser and DOM renderer; the
// reconnect case separately exercises tmux's capture and cursor restoration.
// Geometry checks the cell layout, so these tests need no colour-emoji font.

import { type APIRequestContext, type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { attachSession, cleanupSession, waitForTermText } from "./helpers/term";
import { installTerminalSuiteHooks, reconnectTimingsFromNextLoad } from "./helpers/terminal-suite";
import { waitForReplayReveal, waitForSessionRevealed } from "./helpers/terminal-readiness";

installTerminalSuiteHooks();

const EMOJI = ["✅", "❌", "🔄", "⚠️", "✔️", "⏭️"];
const CLEAR = "\x1b[3J\x1b[2J\x1b[H";
const REPLAY_ROW = "⚠️R✅S✔️T⏭️U";
const CURSOR_PREFIX = "REPLAY-CURSOR:";

/**
 * Keep an owned command quiet between explicit requests, without a shell prompt
 * or input echo changing the rows under measurement. The ready reply proves
 * live output reached this test's post-replay attachment. The replay request
 * leaves tmux's cursor immediately after a VS16 emoji, where a one-cell width
 * disagreement becomes observable when the next request prints X.
 */
const UNICODE_COMMAND = `sh -c 'stty -echo && printf "UNICODE-READY\\n" && while IFS= read -r input; do case "$input" in ready) printf "UNICODE-LIVE\\n" ;; replay) printf "\\033[2J\\033[H${REPLAY_ROW}\\r\\n${CURSOR_PREFIX}⚠️" ;; continue) printf "X\\r\\nREPLAY-LIVE\\r\\n" ;; esac; done'`;

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
    const size = await page.evaluate(() => {
      const term = (window as any).__farhelmIslands.terminal.term;
      return { cols: term.cols as number, rows: term.rows as number };
    });
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
  await page.evaluate((command) => {
    const ws = (window as any).__farhelmIslands.terminal.ws as WebSocket;
    if (ws.readyState !== WebSocket.OPEN) throw new Error("the owned terminal socket closed before its request");
    ws.send(new TextEncoder().encode(`${command}\n`));
  }, command);
}

/**
 * Wait for xterm to parse a direct write before reading its public buffer API.
 * Rendering is asynchronous beyond this callback, so DOM assertions still poll
 * their own rows. These bytes deliberately bypass tmux, just as live output
 * reaches xterm without first being rendered by tmux.
 */
async function writeTerminal(page: Page, bytes: string): Promise<void> {
  await page.evaluate((bytes) => new Promise<void>((resolve) => {
    (window as any).__farhelmIslands.terminal.term.write(bytes, resolve);
  }), bytes);
}

/**
 * Compare the same visible screen rows in the buffer and DOM. Returning only
 * the requested rows keeps polling and timeout diagnostics bounded; trimming
 * trailing blanks preserves interior spaces where cursor misalignment appears.
 * Replay can serialize blank padding as printable cells; normalize that padding
 * in both views instead of assuming the buffer API's trim flag removes it all.
 */
async function expectRows(page: Page, expected: string[]): Promise<void> {
  await expect.poll(() => page.evaluate((count) => {
    const term = (window as any).__farhelmIslands?.terminal?.term;
    const buffer = term?.buffer.active;
    const rows = document.querySelector("#terminal .xterm-rows");
    return {
      buffer: Array.from({ length: count }, (_, row) =>
        (buffer?.getLine(buffer.baseY + row)?.translateToString(true) ?? "").trimEnd()),
      rendered: Array.from({ length: count }, (_, row) => rows?.children[row]?.textContent?.trimEnd() ?? ""),
      atTail: !!buffer && buffer.viewportY === buffer.baseY,
    };
  }, expected.length), { timeout: 10_000, message: "the buffer and painted emoji rows must agree" }).toEqual({
    buffer: expected,
    rendered: expected,
    atTail: true,
  });
}

/**
 * A wide cell owns its continuation column; inspecting both it and the next
 * printable cell catches tables that draw the glyph but still advance by one.
 */
test("emoji and VS16 sequences occupy two cells before the following character", async ({ page, request }) => {
  const id = await openUnicodeTerminal(page, request);
  try {
    await writeTerminal(page, CLEAR + EMOJI.map((emoji) => `${emoji}X`).join("\r\n"));
    const cells = await page.evaluate((count) => {
      const buffer = (window as any).__farhelmIslands.terminal.term.buffer.active;
      return Array.from({ length: count }, (_, row) => {
        const line = buffer.getLine(buffer.baseY + row);
        return [0, 1, 2].map((column) => ({
          chars: line.getCell(column).getChars() as string,
          width: line.getCell(column).getWidth() as number,
        }));
      });
    }, EMOJI.length);
    expect(cells).toEqual(EMOJI.map((emoji) => [
      { chars: emoji, width: 2 },
      { chars: "", width: 0 },
      { chars: "X", width: 1 },
    ]));
    await expectRows(page, EMOJI.map((emoji) => `${emoji}X`));
  } finally {
    await cleanupSession(request, id);
  }
});

/**
 * Agents update counts with cell coordinates rather than reprinting the whole
 * line. More output must preserve those edits in both parsed and painted rows;
 * a full repaint that happens to repair an earlier overlap is insufficient.
 */
test("cursor-addressed status redraws stay aligned as more output arrives", async ({ page, request }) => {
  const id = await openUnicodeTerminal(page, request);
  try {
    await writeTerminal(page, CLEAR + EMOJI.map((emoji) => `${emoji} 0`).join(" "));
    await expectRows(page, [EMOJI.map((emoji) => `${emoji} 0`).join(" ")]);
    for (let index = 0; index < EMOJI.length; index++) {
      // Each segment occupies five columns: a two-cell emoji, space, count,
      // and separator. Addressing the count models the agent's width contract.
      await writeTerminal(page, `\x1b[1;${index * 5 + 4}H${index + 1}`);
    }
    await writeTerminal(page, "\x1b[2;1Hstream-one\r\nstream-two");
    await expectRows(page, [EMOJI.map((emoji, index) => `${emoji} ${index + 1}`).join(" "), "stream-one", "stream-two"]);
  } finally {
    await cleanupSession(request, id);
  }
});

/**
 * A neighbouring background can cover the right half of a one-cell emoji
 * box. Box widths and adjacency test that mechanism without depending on which
 * fallback font supplies the glyph, or comparing rendered pixels.
 */
test("a coloured neighbouring cell starts after the emoji's full two-cell box", async ({ page, request }) => {
  const id = await openUnicodeTerminal(page, request);
  try {
    await writeTerminal(page, CLEAR + EMOJI.map((emoji) => `${emoji}\x1b[41mX\x1b[0m`).join("\r\n"));
    await expectRows(page, EMOJI.map((emoji) => `${emoji}X`));
    const boxes = await page.evaluate((emoji) => {
      const rows = document.querySelector("#terminal .xterm-rows")!;
      return emoji.map((text, index) => {
        const spans = Array.from(rows.children[index].children) as HTMLElement[];
        const wide = spans.find((span) => span.textContent === text);
        const next = spans.find((span) => span.textContent === "X");
        if (!wide || !next) return null;
        const wideBox = wide.getBoundingClientRect();
        const nextBox = next.getBoundingClientRect();
        return {
          emojiWidth: wideBox.width,
          cellWidth: nextBox.width,
          gap: nextBox.left - wideBox.right,
          background: getComputedStyle(next).backgroundColor,
        };
      });
    }, EMOJI);
    for (let index = 0; index < boxes.length; index++) {
      const box = boxes[index];
      expect(box, `DOM spans for ${EMOJI[index]} and its coloured neighbour`).not.toBeNull();
      expect(box!.cellWidth).toBeGreaterThan(0);
      expect(Math.abs(box!.emojiWidth - 2 * box!.cellWidth), `${EMOJI[index]} needs a two-cell box`).toBeLessThan(1);
      expect(Math.abs(box!.gap), "the coloured cell must begin at the emoji box's right edge").toBeLessThan(1);
      expect(box!.background, "the neighbour must actually paint a background").not.toMatch(/^(transparent|rgba\(0, 0, 0, 0\))$/);
    }
  } finally {
    await cleanupSession(request, id);
  }
});

/**
 * Reconnect reconstructs rows from tmux rather than replaying the live byte
 * stream. A new socket, a marker-completed replay, and a subsequent command
 * reply establish that both tmux's VS16 widths and the restored cursor agree
 * with the newly mounted terminal.
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
    expect(replay.revealReason).toBe("marker");
    expect(replay.revealedInWriteCallback).toBe(true);
    await expectRows(page, [REPLAY_ROW, `${CURSOR_PREFIX}⚠️`]);
    const cursor = await page.evaluate(() => {
      const buffer = (window as any).__farhelmIslands.terminal.term.buffer.active;
      return { x: buffer.cursorX as number, y: buffer.cursorY as number };
    });
    expect(cursor, "replayed VS16 emoji must leave the restored cursor two cells later").toEqual({
      x: CURSOR_PREFIX.length + 2,
      y: 1,
    });
    await sendCommand(page, id, "continue");
    await waitForTermText(page, "REPLAY-LIVE");
    await expectRows(page, [REPLAY_ROW, `${CURSOR_PREFIX}⚠️X`, "REPLAY-LIVE"]);
    expect(await page.evaluate((column) => {
      const buffer = (window as any).__farhelmIslands.terminal.term.buffer.active;
      return buffer.getLine(buffer.baseY + 1).getCell(column).getChars();
    }, CURSOR_PREFIX.length + 2)).toBe("X");
  } finally {
    await cleanupSession(request, id);
  }
});
