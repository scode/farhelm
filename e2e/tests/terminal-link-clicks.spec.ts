/** The program-observed half of terminal link activation. One browser open
 * is insufficient evidence: the program may receive the same click and open
 * its own tab. The real fake-agent input scanner acknowledges completed mouse
 * reports after an ordered sync cue, avoiding an absence-by-sleep assertion.
 */
import { expect, test } from "./helpers/evidence";
import type { Page } from "@playwright/test";
import path from "node:path";
import { createSession, cleanupSession, patchPreferences, readPreferences } from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";
import { addTab, selectTerminal, islandText, waitForIslandText } from "./helpers/terminal-suite";

const invocation = `"${path.resolve(__dirname, "../../target/debug/farhelm-fixtures")}" fake-agent --script mouse-modes`;
const labels = {
  intercept: "open in Farhelm, don't pass the click on",
  both: "open in Farhelm and also pass the click on",
  program: "pass the click on only",
} as const;

/** Match the existing mouse-mode suite's negative notice window: beyond the
 * 1.5-second OSC 52 grace, while a wrongly shown notice would still be visible.
 */
const NO_NOTICE_WINDOW_MS = 3_000;

/** Focus the actual terminal input before sending a program cue. */
async function cue(page: Page, word: string, element = "terminal") {
  // Link redraws repeat the same visible URLs. A fresh sequence prevents a
  // previous row from satisfying readiness before this cue reaches the program.
  const nextLinks = word === "links"
    ? Math.max(0, ...Array.from((await islandText(page, element)).matchAll(/LINKS-READY:(\d+)/g), (match) => Number(match[1]))) + 1
    : null;
  const input = page.locator(`#${element} .xterm-helper-textarea`);
  await input.focus();
  await expect(input).toBeFocused();
  await page.keyboard.type(word);
  await page.keyboard.press("Enter");
  if (nextLinks !== null) await waitForIslandText(page, element, `LINKS-READY:${nextLinks}`);
}

/** Coordinates come from live grid geometry, after the fixture output is
 * visibly present. Visit a different cell inside the screen first: xterm
 * keeps its last buffer-cell cache on mouseleave, so returning from outside
 * to the same fixed link cell would not ask its provider for a fresh hover. */
async function cell(page: Page, text: string, link = true, element = "terminal") {
  const geometry = await page.evaluate(({ needle, element }) => {
    const term = (window as any).__farhelmIslands[element].term;
    for (let row = term.rows - 1; row >= 0; row--) {
      const line = term.buffer.active.getLine(term.buffer.active.viewportY + row)?.translateToString(true) || "";
      const col = line.indexOf(needle);
      if (col >= 0) return { row, col, cols: term.cols, rows: term.rows };
    }
    return null;
  }, { needle: text, element });
  expect(geometry, `fixture text must remain in the live viewport: ${text}`).not.toBeNull();
  const screen = page.locator(`#${element} .xterm-screen`);
  const box = (await screen.boundingBox())!;
  const point = {
    x: box.x + (geometry!.col + text.length / 2) * box.width / geometry!.cols,
    y: box.y + (geometry!.row + 0.5) * box.height / geometry!.rows,
  };
  // The fixture's log corner is ordinary text or blank, never a link. A held
  // secondary button may report this setup move; the overlap baseline includes it.
  await page.mouse.move(
    box.x + box.width * (1 - 0.5 / geometry!.cols),
    box.y + box.height * (1 - 0.5 / geometry!.rows),
  );
  await expect(screen).not.toHaveClass(/\bxterm-cursor-pointer\b/);
  await page.mouse.move(point.x, point.y);
  if (link) {
    try {
      await expect(screen).toHaveClass(/\bxterm-cursor-pointer\b/);
    } catch (error) {
      // Collect inside the fixture lifetime, before finally tears its session
      // down. A hover failure must distinguish lost text from stale decoration.
      const premise = await page.evaluate(({ point, element, text }) => {
        const term = (window as any).__farhelmIslands[element]?.term;
        const screen = document.querySelector(`#${element} .xterm-screen`);
        if (!term || !screen) return { element, expected: text, terminalPresent: false };
        const box = screen.getBoundingClientRect();
        const row = Math.floor((point.y - box.y) * term.rows / box.height);
        const col = Math.floor((point.x - box.x) * term.cols / box.width);
        return {
          element, expected: text, pointerCell: { row, col }, grid: { cols: term.cols, rows: term.rows },
          liveRowText: term.buffer.active.getLine(term.buffer.active.viewportY + row)?.translateToString(true),
          cursorClass: screen.className,
        };
      }, { point, element, text });
      throw new Error(`link hover fixture premise: ${JSON.stringify(premise)}`, { cause: error });
    }
  }
  return point;
}

/** A monotonic acknowledgement observes the scanner's received report count,
 * including reports split across reads. The cue follows pointer input on the
 * same ordered terminal connection, so a returned zero is a processed boundary. */
function reportBarrier(page: Page, element = "terminal") {
  let sequence = 0;
  return async () => {
    sequence += 1;
    await cue(page, "sync", element);
    await waitForIslandText(page, element, `INPUT-SYNC:${sequence}:`);
    const match = (await islandText(page, element)).match(new RegExp(`INPUT-SYNC:${sequence}:(\\d+)`));
    expect(match, "the program must acknowledge this exact cue").not.toBeNull();
    return Number(match![1]);
  };
}

/** Verify the measured press still lands on live link cells, after every
 * output-producing acknowledgement. Cursor decoration alone could outlive
 * a scrolled row. The fixture's separate log region keeps these cells fixed.
 */
async function expectLinkUnderPointer(page: Page, point: { x: number; y: number }, text: string) {
  await expect(page.locator("#terminal .xterm-screen")).toHaveClass(/\bxterm-cursor-pointer\b/);
  expect(await page.evaluate(({ point, text }) => {
    const screen = document.querySelector("#terminal .xterm-screen")!.getBoundingClientRect();
    const term = (window as any).__farhelmTerm;
    const row = Math.floor((point.y - screen.y) * term.rows / screen.height);
    const col = Math.floor((point.x - screen.x) * term.cols / screen.width);
    const line = term.buffer.active.getLine(term.buffer.active.viewportY + row)?.translateToString(true) || "";
    const start = line.indexOf(text);
    return start >= 0 && col >= start && col < start + text.length;
  }, { point, text }), "the primary press must still land on this live link").toBe(true);
}

/** Real pointer input through xterm, websocket, supervisor and tmux must obey
 * each setting immediately. Every negative is followed by a beside-link click
 * that the same running program receives, proving reporting stayed enabled. */
test("terminal link ownership opens once and preserves program clicks in all mounted terminals", async ({ page, request }) => {
  test.setTimeout(120_000);
  await page.addInitScript(() => {
    (window as any).__linkOpens = [];
    window.open = ((url: string) => { (window as any).__linkOpens.push(String(url)); return null; }) as typeof window.open;
  });
  await patchPreferences(request, { terminal_link_click: null });
  const session = await createSession(request, { title: `link-ownership-${Date.now()}`, cwd: "/tmp", invocation });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await cue(page, "sgr");
    await waitForTermText(page, "MOUSE-MODE:sgr");
    await expect.poll(() => page.evaluate(() => (window as any).__farhelmTerm.modes.mouseTrackingMode)).toBe("vt200");
    await page.evaluate(() => { (window as any).__linkTermBefore = (window as any).__farhelmTerm; });
    const reports = reportBarrier(page);
    let expectedReports = await reports();
    expect(expectedReports).toBe(0);
    const tab = await addTab(page, 0);
    await selectTerminal(page, tab);
    const tabElement = `terminal-${tab}`;
    const tabInput = page.locator(`#${tabElement} .xterm-helper-textarea`);
    await tabInput.focus();
    await expect(tabInput).toBeFocused();
    await page.keyboard.type(invocation);
    await page.keyboard.press("Enter");
    await waitForIslandText(page, tabElement, "FAKE-AGENT READY");
    await cue(page, "sgr", tabElement);
    await waitForIslandText(page, tabElement, "MOUSE-MODE:sgr");
    await expect.poll(() => page.evaluate((el) => (window as any).__farhelmIslands[el].term.modes.mouseTrackingMode, tabElement)).toBe("vt200");
    const tabReports = reportBarrier(page, tabElement);
    let expectedTabReports = await tabReports();
    expect(expectedTabReports).toBe(0);
    await page.evaluate((el) => { (window as any).__linkTabBefore = (window as any).__farhelmIslands[el].term; }, tabElement);
    const opens = () => page.evaluate(() => (window as any).__linkOpens as string[]);
    let expectedOpens: string[] = [];
    for (const mode of ["both", "intercept", "program"] as const) {
      await page.getByRole("button", { name: "settings", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "settings", exact: true });
      await dialog.getByRole("radio", { name: labels[mode], exact: true }).check();
      await expect.poll(() => readPreferences(request)).toMatchObject({ terminal_link_click: mode });
      await expect.poll(() => page.evaluate(() => (window as any).farhelmTerminalLinkClickMode)).toBe(mode);
      await dialog.getByRole("button", { name: "close settings", exact: true }).click();
      expect(await page.evaluate(() => (window as any).__farhelmTerm === (window as any).__linkTermBefore)).toBe(true);
      expect(await page.evaluate((el) => (window as any).__farhelmIslands[el].term === (window as any).__linkTabBefore, tabElement)).toBe(true);
      await selectTerminal(page, "agent");
      for (const [text, url] of [["https://example.test/link-click", "https://example.test/link-click"], ["OSC-LINK", "https://example.test/osc-click"]]) {
        await cue(page, "links");
        await waitForTermText(page, "LINKS-READY");
        const point = await cell(page, text);
        await page.mouse.click(point.x, point.y);
        if (mode !== "program") expectedOpens.push(url);
        if (mode !== "intercept") expectedReports += 2;
        expect(await reports()).toBe(expectedReports);
        expect(await opens()).toEqual(expectedOpens);
        // A nearby ordinary cell must still reach the program after interception.
        await cue(page, "links");
        const beside = await cell(page, "PLAIN", false);
        await page.mouse.click(beside.x, beside.y);
        expectedReports += 2;
        expect(await reports()).toBe(expectedReports);
        expect(await opens()).toEqual(expectedOpens);
      }
      await cue(page, "links");
      const file = await cell(page, "relative/report.txt", false);
      await expect(page.locator("#terminal .terminal-file-target")).toBeVisible();
      await page.mouse.click(file.x, file.y);
      expectedReports += 2;
      expect(await reports()).toBe(expectedReports);
      expect(await opens()).toEqual(expectedOpens);
      await selectTerminal(page, tab);
      await cue(page, "links", tabElement);
      const tabLink = await cell(page, "OSC-LINK", true, tabElement);
      await page.mouse.click(tabLink.x, tabLink.y);
      if (mode !== "program") expectedOpens.push("https://example.test/osc-click");
      if (mode !== "intercept") expectedTabReports += 2;
      expect(await tabReports()).toBe(expectedTabReports);
      expect(await opens()).toEqual(expectedOpens);
    }
    // Reload seeds the shared choice again, rather than retaining a device copy.
    await page.reload();
    await expect.poll(() => page.evaluate(() => (window as any).farhelmTerminalLinkClickMode)).toBe("program");
  } finally {
    await cleanupSession(request, session.id);
    await patchPreferences(request, { terminal_link_click: null });
  }
});

/** A forced-selection click must update xterm's pressed-link bookkeeping.
 * Its following plain click cannot reuse stale state and open twice. A
 * withheld drag belongs to the link and must neither send reports nor show
 * program-copy guidance; modifier drags still select locally. */
test("link interception keeps forced selection and withheld drags local", async ({ page, request }) => {
  await page.addInitScript(() => {
    (window as any).__linkOpenCount = 0;
    window.open = (() => { (window as any).__linkOpenCount++; return null; }) as typeof window.open;
  });
  await patchPreferences(request, { terminal_link_click: null });
  const session = await createSession(request, { title: `link-drag-${Date.now()}`, cwd: "/tmp", invocation });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await cue(page, "drag");
    await waitForTermText(page, "MOUSE-MODE:drag");
    await expect.poll(() => page.evaluate(() => (window as any).__farhelmTerm.modes.mouseTrackingMode)).toBe("drag");
    const reports = reportBarrier(page);
    expect(await reports()).toBe(0);
    await cue(page, "links");
    const point = await cell(page, "OSC-LINK");
    await page.keyboard.down("Shift");
    await page.mouse.click(point.x, point.y);
    await page.keyboard.up("Shift");
    expect(await page.evaluate(() => (window as any).__linkOpenCount)).toBe(1);
    await page.mouse.click(point.x, point.y);
    expect(await page.evaluate(() => (window as any).__linkOpenCount)).toBe(2);
    expect(await reports()).toBe(0);
    await cue(page, "links");
    const notice = page.locator("#drag-copy-notice-terminal");
    await expect(notice).toHaveCount(1);
    const screen = (await page.locator("#terminal .xterm-screen").boundingBox())!;
    // A program-owned drag in the log area establishes a real visible notice,
    // away from the link rows, before the intercepted press must dismiss it.
    await page.mouse.move(screen.x + screen.width - 60, screen.y + screen.height - 40);
    await expect(page.locator("#terminal .xterm-screen")).not.toHaveClass(/\bxterm-cursor-pointer\b/);
    await page.mouse.down();
    await page.mouse.move(screen.x + screen.width - 40, screen.y + screen.height - 40, { steps: 5 });
    await page.mouse.up();
    const programReports = await reports();
    expect(programReports).toBeGreaterThan(0);
    await expect(notice).toBeVisible({ timeout: 10_000 });
    const drag = await cell(page, "https://example.test/link-click");
    await page.mouse.down();
    await expect(notice, "an intercepted press must dismiss the existing notice").toBeHidden();
    await page.mouse.move(drag.x + 20, drag.y, { steps: 5 });
    await page.mouse.up();
    expect(await reports()).toBe(programReports);
    // sleep-ok: observe beyond the OSC 52 grace before another press could cancel an incorrectly scheduled notice.
    await page.waitForTimeout(NO_NOTICE_WINDOW_MS);
    expect(await page.evaluate(() => (window as any).__farhelmTerm.modes.mouseTrackingMode)).toBe("drag");
    expect(await reports()).toBe(programReports);
    await expect(notice, "a withheld drag must not raise guidance after the grace period").toBeHidden();
    await cue(page, "links");
    const forced = await cell(page, "https://example.test/link-click");
    await page.keyboard.down("Shift");
    await page.mouse.down();
    await page.mouse.move(forced.x + 20, forced.y, { steps: 5 });
    await page.mouse.up();
    await page.keyboard.up("Shift");
    await expect.poll(() => page.evaluate(() => (window as any).__farhelmTerm.getSelection())).not.toBe("");
    expect(await reports()).toBe(programReports);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** A different held button can install xterm's document reporters before or
 * during a withheld link gesture. Primary release and drag must still stay
 * local, while the secondary button and a later ordinary click keep flowing.
 * Each count is acknowledged by the program, including the release outside
 * the screen, where a screen-only event stop cannot establish ownership.
 */
for (const mode of ["drag", "legacy"] as const) {
  test(`overlapping buttons cannot leak a withheld link gesture (${mode})`, async ({ page, request }) => {
    await page.addInitScript(() => {
      window.open = (() => null) as typeof window.open;
    });
    await patchPreferences(request, { terminal_link_click: null });
    const session = await createSession(request, { title: `link-overlap-${Date.now()}`, cwd: "/tmp", invocation });
    try {
      await page.goto("/");
      await attachSession(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await cue(page, mode);
      await waitForTermText(page, `MOUSE-MODE:${mode}`);
      await expect.poll(() => page.evaluate(() => (window as any).__farhelmTerm.modes.mouseTrackingMode)).toBe(mode === "drag" ? "drag" : "vt200");
      const reports = reportBarrier(page);
      expect(await reports()).toBe(0);
      await cue(page, "links");
      await cell(page, "PLAIN", false);
      await page.mouse.down({ button: "right" });
      const link = await cell(page, "OSC-LINK");
      const beforePrimary = await reports();
      expect(beforePrimary).toBeGreaterThan(0);
      await expectLinkUnderPointer(page, link, "OSC-LINK");
      await page.mouse.down({ button: "left" });
      await page.mouse.move(link.x + 3, link.y);
      await page.mouse.up({ button: "left" });
      expect(await reports()).toBe(beforePrimary);
      await page.mouse.up({ button: "right" });
      expect(await reports()).toBe(beforePrimary + 1);

      // Reversing the order installs reporters after Farhelm takes ownership.
      // A sync cue prints output and can scroll the link away from the pointer;
      // obtain the count first, then establish fresh geometry and hover.
      await cue(page, "links");
      const beforeOverlap = await reports();
      const nextLink = await cell(page, "OSC-LINK");
      await expectLinkUnderPointer(page, nextLink, "OSC-LINK");
      await page.mouse.down({ button: "left" });
      await page.mouse.down({ button: "right" });
      expect(await reports()).toBe(beforeOverlap + 1);
      const screen = (await page.locator("#terminal .xterm-screen").boundingBox())!;
      await page.mouse.move(nextLink.x, screen.y - 2);
      await page.mouse.up({ button: "left" });
      expect(await reports()).toBe(beforeOverlap + 1);
      await page.mouse.up({ button: "right" });
      expect(await reports()).toBe(beforeOverlap + 2);

      await cue(page, "links");
      const finalOrdinary = await cell(page, "PLAIN", false);
      await page.mouse.click(finalOrdinary.x, finalOrdinary.y);
      expect(await reports()).toBe(beforeOverlap + 4);
    } finally {
      await page.mouse.up({ button: "left" });
      await page.mouse.up({ button: "right" });
      await cleanupSession(request, session.id);
    }
  });
}

/** With no program mouse reporting, interception has nothing to withhold.
 * Only program-only mode disables Farhelm activation; switching to either
 * other choice must restore opening on the same mounted terminal. */
test("program-only link clicks do nothing without mouse reporting", async ({ page, request }) => {
  await page.addInitScript(() => {
    (window as any).__linkOpenCount = 0;
    window.open = (() => { (window as any).__linkOpenCount++; return null; }) as typeof window.open;
  });
  await patchPreferences(request, { terminal_link_click: "program" });
  const session = await createSession(request, { title: `link-no-mouse-${Date.now()}`, cwd: "/tmp", invocation });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    expect(await page.evaluate(() => (window as any).__farhelmTerm.modes.mouseTrackingMode)).toBe("none");
    const reports = reportBarrier(page);
    for (const [mode, count] of [["program", 0], ["both", 1], ["intercept", 2]] as const) {
      await page.getByRole("button", { name: "settings", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "settings", exact: true });
      await dialog.getByRole("radio", { name: labels[mode], exact: true }).check();
      await expect.poll(() => page.evaluate(() => (window as any).farhelmTerminalLinkClickMode)).toBe(mode);
      await dialog.getByRole("button", { name: "close settings", exact: true }).click();
      await cue(page, "links");
      const point = await cell(page, "OSC-LINK");
      await page.mouse.click(point.x, point.y);
      expect(await reports()).toBe(0);
      expect(await page.evaluate(() => (window as any).__linkOpenCount)).toBe(count);
    }
  } finally {
    await cleanupSession(request, session.id);
    await patchPreferences(request, { terminal_link_click: null });
  }
});
