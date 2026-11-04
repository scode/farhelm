// Shared terminal-island test surface: only stable cross-spec helpers live
// here. Divergent one-off helpers remain local to the specs that need them.
//
// ## Two kinds of terminal
//
// A session's terminal is old (tmux, drawn by xterm.js from a byte stream)
// or new (a terminal host, drawn by the grid component from a grid stream).
// terminal.js picks the component per attachment from the helm's stream
// notice, and publishes it in its island registry (`__farhelmIslands[el]`,
// whose `kind` is `bytes` or `grid`) and, for the agent's terminal, as
// `window.__farhelmTerm` or `window.__farhelmGrid`. The readers here
// dispatch on what the island actually holds, never on what a spec expects,
// so a product-level assertion (text arrived, the terminal has the keyboard)
// runs unchanged against either kind. The xterm.js path is the one these
// helpers always had and stays exactly as it was for the whole transition
// to terminal hosts; the grid path reads the component's own test surface
// (its `model`, `snapshot()`, `viewport()` and `size()`), not an imitation of
// xterm.js's buffer API. Where a spec needs a fact only one kind has, it
// asks `terminalKind` and says why in place.
//
// Which kind a run's stacks create is the run's choice, not a spec's:
// `playwright.config.ts` reads `SUITE_TERMINALS_ENV` (e2e/host-terminal-specs.ts;
// unset: the supervisor's default, tmux). Specs that assert something only
// the old kind has skip themselves on host terminals with `onHostTerminals()`.

import { expect, type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { SUITE_TERMINALS_ENV } from "../../host-terminal-specs";
export { type InputFrame, programInput, recordInput } from "./program-input";
import { waitForSessionReady } from "./terminal-readiness";
import { recordPage } from "./timeline";

/**
 * Whether this run's stacks create terminals in terminal hosts.
 *
 * For `test.skip(onHostTerminals(), reason)` on a test whose assertion is
 * about the old terminal's implementation (xterm.js's DOM, its write
 * callbacks, tmux's own behavior), or about something terminals in terminal
 * hosts deliberately do not have yet (the drag-copy notice):
 * such a test stays in the suite and runs on every ordinary run, and the
 * reason says why it has no grid counterpart or where that counterpart lives.
 */
export function onHostTerminals(): boolean {
  return process.env[SUITE_TERMINALS_ENV] === "host";
}

/**
 * The keyboard target of the terminal mounted at `elementId` (`terminal`
 * for the agent, `terminal-<tab id>` for a tab), whichever component holds
 * it: xterm.js's helper textarea or the grid's input field. Both are hidden
 * text fields that take focus when the terminal does, so "the terminal has
 * the keyboard" is the same observation for either kind.
 */
export function terminalInput(page: Page, elementId = "terminal"): Locator {
  return page.locator(`[id="${elementId}"] :is(.xterm-helper-textarea, .grid-terminal-input)`);
}

/**
 * The CSS selector form of `terminalInput` for the agent's terminal, for
 * the helpers that take selectors rather than locators (`observeModalRelease`).
 */
export const AGENT_TERMINAL_INPUT = "#terminal :is(.xterm-helper-textarea, .grid-terminal-input)";

/**
 * The drawing surface of the terminal mounted at `elementId`, whichever
 * component holds it: xterm.js's `.xterm-screen` or the grid's
 * `.grid-terminal-screen`. Both cover exactly the terminal's cells (the
 * grid sizes its screen to its model's columns and rows of cells), so a
 * point computed from this box and `terminalSize` lands on the same cell
 * for either.
 */
export function terminalScreen(page: Page, elementId = "terminal"): Locator {
  return page.locator(`[id="${elementId}"] :is(.xterm-screen, .grid-terminal-screen)`);
}

/**
 * The rows and columns the terminal mounted at `elementId` draws: xterm.js's
 * `cols` and `rows` (its fit, which the program learns through a resize),
 * or the size the grid's model holds (the one the supervisor last committed
 * in a frame, which is also what its screen element measures). After a
 * pane resize the grid's size changes once the program's terminal did, so
 * pollers see the new size a little later than with xterm.js but never a
 * size the program has not taken. Null before anything mounted.
 */
export async function terminalSize(page: Page, elementId = "terminal"): Promise<{ cols: number; rows: number } | null> {
  return page.evaluate((el) => {
    const island = (window as any).__farhelmIslands?.[el];
    if (island?.grid) return { cols: island.grid.model.cols, rows: island.grid.model.rows };
    if (island?.term) return { cols: island.term.cols, rows: island.term.rows };
    return null;
  }, elementId);
}

/**
 * Send `text` to the program of the terminal mounted at `elementId` as if
 * typed, straight down its socket rather than through the keyboard: raw
 * UTF-8 bytes on a byte stream, or grid `text` input messages (which the
 * supervisor writes as the same bytes) on a grid stream, whose binary frames
 * must be input messages, split as the grid splits typed text. Throws when
 * the socket is not open, so a request cannot vanish into a closed
 * attachment.
 */
export async function sendTerminalInput(page: Page, text: string, elementId = "terminal"): Promise<void> {
  await page.evaluate(({ el, text }) => {
    const w = window as any;
    const island = w.__farhelmIslands?.[el];
    const ws = island?.ws as WebSocket | undefined;
    if (!ws || ws.readyState !== WebSocket.OPEN) throw new Error(`the terminal socket at ${el} is not open`);
    if (!island.grid) {
      ws.send(new TextEncoder().encode(text));
      return;
    }
    // The grid's own split into messages within the format's bounds; every
    // message is encoded before any is sent, so an encoding the format
    // refuses (null) fails here instead of sending part of the text.
    const encoded = w.farhelmGridInput.textMessages(text).map((message: unknown) => w.farhelmGridInput.encodeInput(message));
    if (encoded.some((bytes: unknown) => bytes === null)) throw new Error("a grid text message over the input bound");
    for (const bytes of encoded) ws.send(bytes);
  }, { el: elementId, text });
}

/**
 * The rows and columns the terminal mounted at `elementId` fits its pane to,
 * the size the page asks the program for: xterm.js's `cols` and `rows`, or
 * the grid's own fit (`size()`), which the page measures independently of
 * what the supervisor sends back. A resize test compares this with what the
 * program reports, so that a size sent wrong (swapped, off by one) cannot
 * agree with itself the way the supervisor's committed size would.
 */
export async function terminalFit(page: Page, elementId = "terminal"): Promise<{ cols: number; rows: number } | null> {
  return page.evaluate((el) => {
    const island = (window as any).__farhelmIslands?.[el];
    if (island?.grid) return island.grid.size();
    if (island?.term) return { cols: island.term.cols, rows: island.term.rows };
    return null;
  }, elementId);
}

/**
 * The text size (CSS pixels) and font family the terminal mounted at
 * `elementId` draws with: xterm.js's options, or the grid element's own
 * style, which the grid sets from the same values terminal.js hands either
 * component. Null before anything mounted.
 */
export async function terminalFont(page: Page, elementId = "terminal"): Promise<{ fontSize: number; fontFamily: string } | null> {
  return page.evaluate((el) => {
    const island = (window as any).__farhelmIslands?.[el];
    if (island?.grid) {
      const style = island.grid.element.style;
      return { fontSize: parseFloat(style.fontSize), fontFamily: style.fontFamily };
    }
    if (island?.term) return { fontSize: island.term.options.fontSize, fontFamily: island.term.options.fontFamily };
    return null;
  }, elementId);
}

/**
 * The rows the terminal mounted at `elementId` shows right now, top to
 * bottom, each trimmed of trailing blanks, with its size: xterm.js's
 * viewport over its buffer, or the grid's view (its screen while it
 * follows the output, or the history and screen lines it is scrolled to,
 * with the lost-history notice line read as an empty row). A row's index
 * here is its row on screen, so a caller can turn it and a column into a
 * point with `terminalScreen`'s box. `all` is every line the view can
 * scroll over, in the same form, and `top` the index in it of the first
 * visible row. Null before anything mounted.
 */
export async function terminalViewport(
  page: Page,
  elementId = "terminal",
): Promise<{ rows: number; cols: number; lines: string[]; all: string[]; top: number } | null> {
  return page.evaluate((el) => {
    const w = window as any;
    const island = w.__farhelmIslands?.[el];
    if (island?.grid) {
      const grid = island.grid;
      const model = grid.model;
      const rowText = w.farhelmGridBuffer.rowText;
      const view = grid.viewport();
      // The view's lines as grid-terminal.js's `viewShape` counts them: the
      // notice line when lost history is marked, then history (both hidden
      // on the alternate screen), then the screen; `top` indexes them.
      const historyShown = !model.state.altScreen;
      const all: string[] = [];
      if (historyShown && model.state.historyTrimmed) all.push("");
      if (historyShown) {
        for (let i = 0; i < model.historyLength; i++) {
          const row = model.historyLine(i);
          all.push(rowText(row, row.cols).replace(/ +$/, ""));
        }
      }
      for (const row of model.screen) all.push(rowText(row, model.cols).replace(/ +$/, ""));
      const lines = all.slice(view.top, view.top + model.rows);
      return { rows: model.rows, cols: model.cols, lines, all, top: view.top };
    }
    const term = island?.term;
    if (!term) return null;
    const buffer = term.buffer.active;
    const all: string[] = [];
    for (let i = 0; i < buffer.length; i++) all.push(buffer.getLine(i)?.translateToString(true) ?? "");
    const lines = all.slice(buffer.viewportY, buffer.viewportY + term.rows);
    return { rows: term.rows, cols: term.cols, lines, all, top: buffer.viewportY as number };
  }, elementId);
}

/**
 * The element that shows the pointer is over a live link in the terminal
 * mounted at `elementId`, matched only while it does: xterm.js's screen
 * with its `xterm-cursor-pointer` class, or the grid element with its
 * `grid-terminal-link-hover` class. Count 1 is "a link is under the
 * pointer", count 0 is "none is".
 */
export function terminalLinkPointer(page: Page, elementId = "terminal"): Locator {
  return page.locator(
    `[id="${elementId}"] :is(.xterm-screen.xterm-cursor-pointer, .grid-terminal.grid-terminal-link-hover)`,
  );
}

/**
 * Which component draws the terminal mounted at `elementId`: `xterm` for a
 * byte stream, `grid` for a grid stream, or null before anything mounted.
 * A fresh mount is xterm.js until the stream notice says otherwise, so a
 * caller that needs the settled kind waits for the attachment's reveal
 * first (`waitForSessionRevealed`), which follows the notice.
 */
export async function terminalKind(page: Page, elementId = "terminal"): Promise<"xterm" | "grid" | null> {
  return page.evaluate((el) => {
    const island = (window as any).__farhelmIslands?.[el];
    if (island?.grid) return "grid";
    if (island?.term) return "xterm";
    return null;
  }, elementId);
}

/**
 * The complete text of the terminal mounted at `elementId`, history and
 * screen, one string per row with trailing blanks trimmed, joined by
 * newlines; plus enough context for a failed wait to say why.
 *
 * Runs in the page, so it is self-contained: `page.evaluate` ships the
 * function's source, not its closure. `elementId` null reads the primary
 * agent terminal through its `window.__farhelmTerm` or
 * `window.__farhelmGrid` handle, which is what `termText` always read;
 * otherwise the island registry. `needle` null asks for the text; a
 * needle asks only whether the text contains it. A grid is read from its
 * model (history, then the screen, with no history on the alternate screen
 * as xterm.js's active buffer has none), row by row as `snapshot()` reads it. For a grid, history the component has
 * not applied yet (it applies a backlog in slices) is not there yet, as
 * xterm.js's buffer lacks bytes it has not parsed yet.
 */
function terminalTextInPage({ elementId, needle }: { elementId: string | null; needle: string | null }) {
  const w = window as any;
  const island = elementId === null ? null : w.__farhelmIslands?.[elementId];
  const term = elementId === null ? w.__farhelmTerm : island?.term;
  const grid = elementId === null ? w.__farhelmGrid : island?.grid;
  const lines: string[] = [];
  // A grid's rows at their full width, blanks kept: see `found` below.
  const padded: string[] = [];
  if (grid) {
    const model = grid.model;
    const rowText = w.farhelmGridBuffer.rowText;
    // The normal screen's history is not this screen's while the alternate
    // screen shows, as xterm.js's active (alternate) buffer has none.
    if (!model.state.altScreen) {
      for (let i = 0; i < model.historyLength; i++) {
        const row = model.historyLine(i);
        padded.push(rowText(row, row.cols));
      }
    }
    for (const row of model.screen) padded.push(rowText(row, model.cols));
    for (const row of padded) lines.push(row.replace(/ +$/, ""));
  } else if (term) {
    const buffer = term.buffer.active;
    for (let i = 0; i < buffer.length; i++) {
      lines.push(buffer.getLine(i)?.translateToString(true) ?? "");
    }
  }
  const text = lines.join("\n");
  const ws = elementId === null ? w.__farhelmWs : island?.ws;
  return {
    // Only a read asks for the text itself: a wait searches here, in the
    // page, rather than moving megabytes of scrollback on every poll.
    text: needle === null ? text : "",
    // xterm.js keeps a blank the program wrote and trims only untouched
    // cells, so a needle can end in a space ("> " after a prompt). The
    // emulator behind a grid, like tmux, cannot tell those apart, so a
    // grid's trimmed rows lose that space; its rows padded to their width
    // still hold it, and a needle is found in either form.
    // The padded form is searched only for a needle that ends in a space,
    // the one case where it can match and the trimmed form cannot.
    found: needle === null
      ? false
      : text.includes(needle) || (padded.length > 0 && / $/.test(needle) && padded.join("\n").includes(needle)),
    mounted: !!(term || grid),
    kind: grid ? "grid" : term ? "xterm" : null,
    socketState: ws?.readyState ?? null,
    characters: text.length,
    tail: text.slice(-1024),
  };
}

/**
 * Read the complete text of the terminal mounted at `elementId`, history
 * and screen, whichever component draws it; "" before it mounts. The
 * island-addressed form of `termText`.
 */
export async function terminalText(page: Page, elementId: string): Promise<string> {
  return (await page.evaluate(terminalTextInPage, { elementId, needle: null })).text;
}

/**
 * Poll the terminal mounted at `elementId` until `needle` arrives, as
 * `waitForTermText` does for the agent's.
 */
export async function waitForTerminalText(page: Page, elementId: string, needle: string, timeout = 15_000) {
  await expect
    .poll(() => page.evaluate(terminalTextInPage, { elementId, needle }), {
      timeout,
      message: `waiting for ${needle} in ${elementId}`,
    })
    .toMatchObject({ found: true });
}

/**
 * Restart the open session from its header while its agent reads idle, so
 * the click restarts directly instead of opening the confirmation.
 *
 * Restart asks for confirmation only while the agent is working (SPEC.md,
 * Lifecycle operations). The fixture agents these specs launch go quiet
 * after printing, and the activity sampler reads a quiet screen as idle only
 * after several samples, so a spec that restarts as a step toward something
 * else waits for that reading first: clicking earlier can still find the
 * agent reading working and open the prompt instead. The badge is the
 * oracle rather than `data-confirms` alone, because the create reply's
 * `Unknown` placeholder also leaves `data-confirms` false. Pass
 * `afterInput` when the spec typed into the agent just before; see the
 * option's comment below.
 */
export async function restartIdleAgent(page: Page, options: { afterInput?: boolean } = {}): Promise<void> {
  const badge = page.locator(".session-details .status-badge");
  if (options.afterInput) {
    // A spec that has just typed into the agent may still see the idle
    // reading from BEFORE that input, while the supervisor has already
    // sampled the echo and reads working; restarting then is refused as
    // unconfirmed. The changed screen reads working for several samples,
    // so wait for that reading first, then for the idle one after it.
    await expect(badge).toHaveClass(/\brunning\b/, { timeout: 30_000 });
  }
  await expect(badge).toHaveClass(/\bidle\b/, { timeout: 30_000 });
  const restartButton = page.locator(".restart-primary");
  // Premise: Restart is available. It only ever resumes, and a page whose
  // copy says the session cannot resume turns the click into a silent no-op
  // that would only surface later as an unexplained timeout.
  await expect(restartButton).not.toHaveAttribute("aria-disabled", "true");
  await expect(restartButton).toHaveAttribute("data-confirms", "false");
  await restartButton.click();
}

/**
 * Read the complete terminal buffer, including scrollback rather than only
 * the rows the terminal currently renders in the DOM.
 *
 * This is the terminal island's test-surface contract: for xterm.js,
 * `window.__farhelmTerm` and `term.buffer.active`; for the grid,
 * `window.__farhelmGrid` and its `snapshot()` (history, then the screen),
 * so the terminal tests keep asserting replayed or off-screen output on
 * either kind. It returns an empty string before mounting, so polling
 * callers can start before the island is ready.
 */
export async function termText(page: Page): Promise<string> {
  return (await page.evaluate(terminalTextInPage, { elementId: null, needle: null })).text;
}

/**
 * Poll the terminal buffer until `needle` arrives.
 *
 * Terminal output arrives asynchronously over a WebSocket with no DOM event
 * to await, so a one-shot buffer read cannot establish that output landed.
 * Search inside the page: transferring megabytes of scrollback on every
 * mismatch makes the observation itself expensive and floods failure reports.
 * The bounded tail, the component and the attachment state remain available
 * when the wait fails; containment still covers the complete buffer,
 * including line separators.
 */
export async function waitForTermText(page: Page, needle: string, timeout = 15_000) {
  await expect
    .poll(() => page.evaluate(terminalTextInPage, { elementId: null, needle }), {
      timeout,
      message: `waiting for ${needle}`,
    })
    .toMatchObject({ found: true });
}

/**
 * Select a session and wait until its terminal can receive keyboard input.
 *
 * Opening a row is not enough: an island can exist while its socket is still
 * connecting or its replay remains hidden, and the row click must leave the
 * terminal's own focus intact for keyboard-driven callers. This does not
 * prove that a fixture agent reached a prompt; callers that need one keep
 * their own agent-specific witness. This ordinary fixture expects an
 * unfiltered session list and the initial focus handoff. Tests that already
 * moved focus into another control must choose a focus-neutral boundary.
 */
export async function attachSession(page: Page, id: string): Promise<void> {
  recordPage(page, "attach-session-intent", [["session", id]]);
  const target = page.locator(`[data-session-id="${id}"]`);
  await expect(target).toBeVisible({ timeout: 20_000 });
  // A fresh page auto-selects before any test action. Wait for that choice
  // to commit before deciding whether a click is needed: clicking the same
  // row after its terminal revealed would move focus back to the row, with
  // no new reveal to hand it to xterm again.
  await expect(page.locator('.session-row[data-session-selected="true"]')).toHaveCount(1, {
    timeout: 20_000,
  });
  if ((await target.getAttribute("data-session-selected")) !== "true") {
    recordPage(page, "attach-session-selection", [["session", id], ["branch", "open"]]);
    await target.locator(".session-row-open").click();
  } else {
    recordPage(page, "attach-session-selection", [["session", id], ["branch", "already-selected"]]);
  }
  await waitForSessionReady(page, id);
}

/**
 * Answer a command launch's required YOLO question in the launcher (`yolo`,
 * default no). The question has no default, so a command-mode create cannot
 * be submitted without it.
 */
export async function answerYolo(form: Locator, yolo = false) {
  await form
    .getByRole("group", { name: "Runs in YOLO mode" })
    .getByLabel(yolo ? "Yes (YOLO)" : "No", { exact: true })
    .check();
}

/**
 * Open the inline create form and fill its required fields without submitting.
 *
 * This is the list view's plain toggled `<div>`, not a modal. `title` is
 * required because every caller needs a known title, including the explicit
 * empty-title case. Filling and submitting stay separate because callers need
 * to inspect the form while a request is pending or after it fails. Only the
 * command tab accepts an arbitrary command, so the helper selects it before
 * the command field becomes the request's source of intent, and
 * answers the command's required YOLO question (`yolo`, default no).
 */
export async function fillCreateForm(
  page: Page,
  { cwd, invocation, title, yolo = false }: { cwd: string; invocation: string; title: string; yolo?: boolean },
) {
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByRole("tab", { name: "Command", exact: true }).click();
  await form.getByLabel("Folder", { exact: true }).fill(cwd);
  await form.getByLabel("Agent command").fill(invocation);
  // A command launch has no default YOLO answer: the form asks every time.
  await answerYolo(form, yolo);
  // The name field sits on the top action row now, visible without opening
  // anything — fill it by its label directly rather than by DOM position.
  await form.getByLabel("Name (optional)").fill(title);
  return form;
}

/**
 * Stop then delete a session, tolerating only a session that is already gone.
 *
 * This deliberately differs from `helpers/fleet.ts`'s `cleanupSession`:
 * fleet issues stop and delete eagerly, so a failed stop does not block the
 * delete; this terminal helper stops at the first non-404 failure. That
 * fail-fast policy is inherited from the terminal specs. Changing it would be
 * observable fixture behavior, not part of moving the duplicate helpers.
 */
export async function cleanupSession(request: APIRequestContext, id: string): Promise<void> {
  const stopped = await request.post(`/api/sessions/${encodeURIComponent(id)}/stop`);
  if (!stopped.ok() && stopped.status() !== 404) {
    throw new Error(
      `cleanup: stopping session ${id} failed (${stopped.status()}): ${await stopped.text()}`,
    );
  }
  const deleted = await request.delete(`/api/sessions/${encodeURIComponent(id)}`);
  if (!deleted.ok() && deleted.status() !== 404) {
    throw new Error(
      `cleanup: deleting session ${id} failed (${deleted.status()}): ${await deleted.text()}`,
    );
  }
}
