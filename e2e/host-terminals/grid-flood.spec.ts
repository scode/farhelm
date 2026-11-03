// A flood in a terminal in a terminal host, seen through the grid terminal
// in the real app: the supervisor's emulator takes the program's output at
// full speed, and the browser gets the emulator's rows and history as a grid
// stream, paced by the helm and the supervisor rather than by the page's own
// write callbacks (which is what tests/terminal-flood.spec.ts checks for
// xterm.js). The Rust tests of the supervisor's grid streams and of the
// helm's relay check the pause itself against stand-ins; this is the same
// path end to end, judged by what a viewer ends up with.
import path from "node:path";
import type { Page } from "@playwright/test";
import { expect, test } from "../tests/helpers/evidence";
import { cleanupSession, createSession } from "../tests/helpers/fleet";
import { attachSession, terminalKind, waitForTermText } from "../tests/helpers/term";

/**
 * The `flood_gated` fake agent (crates/farhelm-fixtures/src/fake_agent.rs):
 * after `FAKE-AGENT READY` it waits for one byte, then prints `FLOOD_RECORDS`
 * numbered lines (`FLOOD-00000000`, ...) unpaced, then `FLOOD-DONE`, and
 * stays alive so its terminal keeps its history.
 */
const FLOOD_GATED = `"${path.resolve(__dirname, "../../target/debug/farhelm-fixtures")}" fake-agent --script flood-gated`;

/** `fake_agent::FLOOD_RECORDS`, duplicated as tests/terminal-flood.spec.ts does. */
const FLOOD_RECORDS = 800_000;

/**
 * The scrollback a terminal in a terminal host keeps, which its grid stream
 * announces as its history capacity: the supervisor's
 * `hosts::terminal::DEFAULT_SCROLLBACK_LINES`, which is tmux's
 * `HISTORY_LIMIT` (crates/farhelm-supervisor/src/tmux.rs). Duplicated so the
 * test can say how much history a flood must leave, independently of what
 * the stream announces.
 */
const SCROLLBACK_LINES = 12_000;

/**
 * Keep the page's main thread busy for `ms`, so it reads nothing from its
 * sockets meanwhile: a viewer that stops reading, as a slow or briefly
 * frozen tab does. Output keeps arriving at the browser and then the helm
 * behind it.
 */
async function stopReading(page: Page, ms: number): Promise<void> {
  await page.evaluate((ms) => {
    const end = performance.now() + ms;
    while (performance.now() < end) {
      // Deliberately busy: a timer would let the page read its sockets.
    }
  }, ms);
}

/**
 * The flood records the grid holds, history then screen, as a summary
 * computed in the page: how many, the first and last record numbers, the
 * first place the numbers do not go up by one (a lost or repeated line), how
 * many history lines are not records, and the history's length against the
 * capacity the stream announced, so a failure says where without moving the
 * whole history.
 */
async function floodRecords(page: Page) {
  return page.evaluate(() => {
    const w = window as any;
    const grid = w.__farhelmGrid;
    const snapshot = grid.snapshot();
    let count = 0;
    let first = -1;
    let last = -1;
    let broken: string | null = null;
    let historyOthers = 0;
    const lines: string[] = [...snapshot.history, ...snapshot.lines];
    lines.forEach((line, index) => {
      const match = /^FLOOD-(\d{8})$/.exec(line);
      if (!match) {
        if (index < snapshot.history.length) historyOthers++;
        return;
      }
      const n = Number(match[1]);
      if (count > 0 && n !== last + 1 && broken === null) broken = `record ${n} after ${last}`;
      if (count === 0) first = n;
      last = n;
      count++;
    });
    const screen: string[] = snapshot.lines;
    return {
      count,
      first,
      last,
      broken,
      historyOthers,
      historyLength: snapshot.history.length as number,
      historyCap: grid.model.historyCap as number,
      historyPending: snapshot.historyPending as boolean,
      // The final screen's shape: how many of its rows above its last two
      // are records, and those last two rows.
      screenRecords: screen.slice(0, -2).filter((line) => /^FLOOD-\d{8}$/.test(line)).length,
      screenTail: screen.slice(-2),
    };
  });
}

/**
 * Open the flood's gate and stop reading in one page turn: send the gate
 * byte down the grid's own socket (as a `text` input message, as
 * `sendTerminalInput` would), then keep the main thread busy for `ms`.
 *
 * Why one turn: the flood starts only once the gate byte arrives and runs
 * for seconds, and the page cannot read a single frame of it before the
 * stall ends, so the stall overlaps the flood by construction rather than by
 * a race with a sample of the output. (The send itself is the browser's
 * network side's, which a busy main thread does not hold back.) An earlier
 * version waited to see an early record before stalling; the records it
 * looked for exist only in the flood's first fraction of a second, and a
 * page that sampled later found them already scrolled out of the 12,000-line
 * history for good.
 */
async function openGateAndStopReading(page: Page, ms: number): Promise<void> {
  await page.evaluate((ms) => {
    const w = window as any;
    const ws = w.__farhelmWs as WebSocket;
    if (!w.__farhelmGrid || !ws || ws.readyState !== WebSocket.OPEN) throw new Error("no open grid terminal socket");
    const messages = w.farhelmGridInput.textMessages("g").map((message: unknown) => w.farhelmGridInput.encodeInput(message));
    if (messages.length !== 1 || messages[0] === null) throw new Error("the gate byte does not encode as one input message");
    ws.send(messages[0]);
    const end = performance.now() + ms;
    while (performance.now() < end) {
      // Deliberately busy: a timer would let the page read its sockets.
    }
  }, ms);
}

/** Whether the grid already drew the flood's end, read in the page. */
async function floodDone(page: Page): Promise<boolean> {
  return page.evaluate(() => (window as any).__farhelmGrid.snapshot().lines.some((line: string) => line === "FLOOD-DONE"));
}

/**
 * A flood's scrollback arrives whole and in order in a grid terminal, also
 * when the viewer stops reading partway.
 *
 * Why: a grid stream is not the program's bytes but the emulator's rows,
 * sent as history lines once they scroll off the screen, and paced by the
 * helm and the supervisor when the browser falls behind. A lost, repeated or
 * reordered history batch anywhere on that path (the supervisor's bookkeeping
 * of what it sent, the helm's pause and resume, the grid model's history
 * queue) shows as a gap or a repeat in exactly this kind of output, which no
 * screen-only check would see.
 * Spec: the page opens the gate and stops reading in the same turn, for a
 * few seconds, so the flood runs while it reads nothing; the flood is then
 * seen to have started, and the page stops reading again if it has not yet
 * drawn the flood's end; once `FLOOD-DONE` is drawn, the grid comes to hold exactly the last
 * `SCROLLBACK_LINES + rows - 2` records, consecutive: a history full to the
 * configured scrollback (as the stream announces), all records, and a
 * screen of records above `FLOOD-DONE` and the cursor's empty row, with
 * nothing left to apply. History still owed when the screen arrives may
 * follow it.
 */
test("a flood's scrollback arrives whole and in order, through a viewer that stops reading", async ({ page, request }) => {
  test.setTimeout(240_000);
  const session = await createSession(request, {
    title: `grid-flood-${Date.now()}`,
    cwd: "/tmp",
    invocation: FLOOD_GATED,
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    // Premise: this session's terminal is a host terminal drawn as a grid.
    expect(await terminalKind(page)).toBe("grid");
    // The first stall starts with the gate, so output is owed to the page
    // throughout it (see `openGateAndStopReading`).
    await openGateAndStopReading(page, 3_000);
    // Premise: the gate opened the flood. Any record line will do: which
    // records the grid holds at a given moment is a window that moves at
    // the flood's speed, and the early ones are soon gone.
    await expect
      .poll(() => floodRecords(page).then((records) => records.count), {
        timeout: 30_000,
        message: "the gate must start the flood",
      })
      .toBeGreaterThan(0);
    // A second stall only while output is still owed; a fast machine may
    // have delivered it all during the first.
    if (!(await floodDone(page))) await stopReading(page, 3_000);
    else test.info().annotations.push({ type: "note", description: "the flood's end was drawn during the first stall" });
    await waitForTermText(page, "FLOOD-DONE", 180_000);
    // What the flood must leave, worked out from the fixture rather than
    // read back from the grid: a history full to the configured scrollback,
    // every line of it a record, and a screen of records down to its last
    // two rows, `FLOOD-DONE` and the empty row the cursor waits on; so the
    // records held are the last `SCROLLBACK_LINES + rows - 2`, consecutive.
    // The screen can show the end of the flood before all the history that
    // scrolled off it has arrived (after the stalls, several thousand lines
    // were seen to follow it within seconds), so this is polled; a line that
    // never comes fails here with where it is missing.
    const rows = await page.evaluate(() => (window as any).__farhelmGrid.model.rows as number);
    const held = SCROLLBACK_LINES + rows - 2;
    await expect
      .poll(() => floodRecords(page), {
        timeout: 60_000,
        message: "the history must arrive whole: full to the scrollback, all records, consecutive to the last",
      })
      .toEqual({
        count: held,
        first: FLOOD_RECORDS - held,
        last: FLOOD_RECORDS - 1,
        broken: null,
        historyOthers: 0,
        historyLength: SCROLLBACK_LINES,
        historyCap: SCROLLBACK_LINES,
        historyPending: false,
        screenRecords: rows - 2,
        screenTail: ["FLOOD-DONE", ""],
      });
  } finally {
    await cleanupSession(request, session.id);
  }
});
