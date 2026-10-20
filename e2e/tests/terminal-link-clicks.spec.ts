/** The program-observed half of terminal link activation. One browser open
 * is insufficient evidence: the program may receive the same click and open
 * its own tab. The real fake-agent input scanner acknowledges completed mouse
 * reports after an ordered sync cue, avoiding an absence-by-sleep assertion.
 * Preference restoration is armed before writes and independent of session
 * cleanup, since every later spec in the stack observes the same link policy.
 */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import type { Page } from "@playwright/test";
import path from "node:path";
import { createSession, cleanupSession, patchPreferences, readPreferences, stubFeed } from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";
import { addTab, selectTerminal, islandText, waitForIslandText, fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";

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

/** An agent TUI can consume a drag without creating any local selection.
 * Keep the real mouse-reporting fixture alive and admit file lookup with a
 * controlled reply, then observe all transfer/open entry points. Positive
 * clicks on the same links prove these negatives did not disable links.
 */
test("mouse-reporting drags never open web or file links", async ({ page, request }) => {
  await page.addInitScript(() => {
    (window as any).__linkOpens = [];
    window.open = ((url: string) => { (window as any).__linkOpens.push(String(url)); return null; }) as typeof window.open;
  });
  let sessionId: string | undefined;
  let bodyFailed = false;
  let transfers = 0;
  try {
    await patchPreferences(request, { terminal_link_click: "both" });
    const session = await createSession(request, { title: `link-drag-${Date.now()}`, cwd: "/tmp", invocation });
    sessionId = session.id;
    await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/stat`, route => route.fulfill({
      json: { status: "ready", host: "fixture", path: "/output/report.txt", size: 8, can_open: true },
    }));
    await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/download`, route => {
      transfers++;
      return route.fulfill({ status: 403, body: "fixture transfer observed" });
    });
    await page.goto("/");
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await cue(page, "drag");
    await waitForTermText(page, "MOUSE-MODE:drag");
    await expect.poll(() => page.evaluate(() => (window as any).__farhelmTerm.modes.mouseTrackingMode)).toBe("drag");
    const reports = reportBarrier(page);
    expect(await reports()).toBe(0);
    for (const label of ["https://example.test/link-click", "OSC-LINK", "relative/report.txt", "OSC-FILE"]) {
      await cue(page, "links");
      // Add only a display label in the fixture's fixed header. Input still
      // belongs to the live fake-agent, never to a shell left in mouse mode.
      await page.evaluate(() => new Promise<void>(resolve => {
        (window as any).__farhelmTerm.write("\x1b7\x1b[5;1H\x1b]8;;file:///output/report.txt\x07OSC-FILE\x1b]8;;\x07\x1b8", resolve);
      }));
      const point = await cell(page, label);
      const file = label === "relative/report.txt" || label === "OSC-FILE";
      if (file) await expect(page.locator("#terminal .terminal-file-target")).toContainText("8 B");
      await expectLinkUnderPointer(page, point, label);
      // The release point must still be on the same link, or this negative
      // would not show xterm would otherwise have activated it.
      await expectLinkUnderPointer(page, { x: point.x + 18, y: point.y }, label);
      const beforeReports = await reports();
      // The report acknowledgement stays below the header, but reestablish
      // hover after it to pin the target under this exact press.
      await cell(page, label);
      // OSC file links show a pointer before their lookup answers; wait for the
      // ready disclosure so the drag, not a pending lookup, is what is refused.
      if (file) await expect(page.locator("#terminal .terminal-file-target")).toContainText("8 B");
      await page.keyboard.down("Control");
      await page.mouse.down();
      await page.mouse.move(point.x + 18, point.y, { steps: 5 });
      await page.mouse.up();
      await page.keyboard.up("Control");
      expect(await reports()).toBeGreaterThan(beforeReports);
      expect(await page.evaluate(() => (window as any).__farhelmTerm.getSelection())).toBe("");
      expect(await page.evaluate(() => (window as any).__linkOpens)).toEqual([]);
      expect(transfers).toBe(0);
      const click = await cell(page, label);
      if (file) await expect(page.locator("#terminal .terminal-file-target")).toContainText("8 B");
      await page.mouse.click(click.x, click.y);
      if (file) await expect.poll(() => transfers).toBe(1);
      else await expect.poll(() => page.evaluate(() => (window as any).__linkOpens.length)).toBe(1);
      await page.evaluate(() => { (window as any).__linkOpens = []; });
      transfers = 0;
    }
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["mouse button", () => page.mouse.up()],
      ["Control key", () => page.keyboard.up("Control")],
      ["session", () => sessionId && cleanupSession(request, sessionId)],
      ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
    ]);
  }
});

/** Render a real clipped hover rather than inferring fit from path length.
 * A later resize must restore normal admission, proving that refusal follows
 * current disclosure geometry rather than a permanent flag from the lookup.
 */
test("a clipped file disclosure refuses a click until the window fits", async ({ page, request }) => {
  await page.setViewportSize({ width: 900, height: 350 });
  const session = await createSession(request, { title: `file-fit-${Date.now()}`, cwd: "/tmp", invocation });
  let transfers = 0;
  await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/stat`, route => route.fulfill({
    json: { status: "ready", host: "fixture", path: "/" + "directory/".repeat(300) + "report.txt", size: 8, can_open: true },
  }));
  await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/download`, route => {
    transfers++;
    return route.fulfill({ status: 403, body: "fixture transfer observed" });
  });
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await cue(page, "links");
    const point = await cell(page, "relative/report.txt");
    const disclosure = page.locator("#terminal .terminal-file-target");
    await expect(disclosure).toContainText("8 B");
    await expect.poll(() => disclosure.evaluate(el => el.scrollHeight > el.clientHeight)).toBe(true);
    await page.mouse.click(point.x, point.y);
    const notice = page.locator("#terminal .terminal-file-message.showing");
    await expect(notice).toContainText("full file details are not visible");
    await expect(notice).toBeVisible();
    // Repaint the link row without moving the pointer. xterm re-enters links
    // on redraw, so hiding a hover once cannot establish readable refusal.
    await page.evaluate(async ({ x, y }) => {
      const term = (window as any).__farhelmTerm;
      const box = term.element.querySelector('.xterm-screen').getBoundingClientRect();
      const row = Math.floor((y - box.y) * term.rows / box.height) + 1;
      const column = Math.floor((x - box.x) * term.cols / box.width) + 20;
      await new Promise<void>(resolve => term.write(`\x1b7\x1b[${row};${column}HX\x1b8`, resolve));
    }, point);
    await expect(disclosure).toBeVisible();
    await expect.poll(() => notice.evaluate((el) => {
      const hover = el.parentElement!.querySelector('.terminal-file-target')!;
      return Number(getComputedStyle(el).zIndex) > Number(getComputedStyle(hover).zIndex);
    })).toBe(true);
    expect(transfers).toBe(0);
    await page.keyboard.press("Control+Shift+k");
    const switcher = page.getByRole("dialog", { name: "quick switcher" });
    await expect(switcher).toBeVisible();
    await expect.poll(() => notice.evaluate(el => !!el.closest('[inert]'))).toBe(true);
    await expect.poll(() => notice.evaluate(el => Number(getComputedStyle(el).zIndex))).toBe(12);
    await expect.poll(() => page.locator('.quick-switcher-backdrop').evaluate(el => Number(getComputedStyle(el).zIndex))).toBe(40);
    await page.keyboard.press("Escape");
    await expect(switcher).toBeHidden();
    await expect.poll(() => notice.evaluate(el => !!el.closest('[inert]'))).toBe(false);
    // Whether the dialog ended the hover depends on the engine: Chromium ends
    // it (the refusal then stays on the ordinary failure layer), WebKit keeps
    // it. Either way the refusal keeps its text and is never under a visible
    // hover.
    await expect(notice).toContainText("full file details are not visible");
    await expect.poll(() => notice.evaluate((el) => {
      const hover = el.parentElement!.querySelector('.terminal-file-target') as HTMLElement;
      return hover.hidden || Number(getComputedStyle(el).zIndex) > Number(getComputedStyle(hover).zIndex);
    })).toBe(true);
    const shortRows = await page.evaluate(() => (window as any).__farhelmTerm.rows);
    await page.setViewportSize({ width: 1280, height: 1200 });
    // Viewport sizing returns before WebKit delivers ResizeObserver. Wait
    // for the real fit before deriving cells from the enlarged screen box.
    await expect.poll(() => page.evaluate(() => (window as any).__farhelmTerm.rows)).toBeGreaterThan(shortRows);
    await cue(page, "links");
    const fitting = await cell(page, "relative/report.txt");
    await expect(disclosure).toContainText("8 B");
    await expect.poll(() => disclosure.evaluate(el => el.scrollHeight <= el.clientHeight)).toBe(true);
    await page.mouse.click(fitting.x, fitting.y);
    await expect.poll(() => transfers).toBe(1);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** Disclosure and refusal text share one escape policy; CSS must preserve
 * the spaces in both, otherwise two different host paths can look alike.
 */
test("file disclosure and transfer messages preserve path differences", async ({ page, request }) => {
  const session = await createSession(request, { title: `file-text-${Date.now()}`, cwd: "/tmp", invocation });
  const raw = "/output/a  b\u034F<U+034F>.txt";
  const shown = "a  b<U+034F><U+003C>U+034F>.txt";
  await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/stat`, route => route.fulfill({
    json: { status: "ready", host: "fixture", path: raw, size: 8, can_open: true },
  }));
  await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/download`, route => route.fulfill({ status: 403, body: raw }));
  try {
    await page.goto("/");
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await cue(page, "links");
    const point = await cell(page, "relative/report.txt");
    const name = page.locator("#terminal .terminal-file-name");
    await expect(name).toContainText(shown);
    expect(await name.evaluate(el => getComputedStyle(el).whiteSpace)).toBe("pre-wrap");
    await page.mouse.click(point.x, point.y);
    const notice = page.locator("#terminal .terminal-file-message.showing");
    await expect(notice).toHaveText(`download failed: /output/${shown}`);
    expect(await notice.evaluate(el => getComputedStyle(el).whiteSpace)).toBe("pre-wrap");
  } finally {
    await cleanupSession(request, session.id);
  }
});

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
  let sessionId: string | undefined;
  let bodyFailed = false;
  try {
    await patchPreferences(request, { terminal_link_click: null });
    const session = await createSession(request, { title: `link-ownership-${Date.now()}`, cwd: "/tmp", invocation });
    sessionId = session.id;
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
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["session", () => sessionId && cleanupSession(request, sessionId)],
      ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
    ]);
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
  let sessionId: string | undefined;
  let bodyFailed = false;
  try {
    await patchPreferences(request, { terminal_link_click: null });
    const session = await createSession(request, { title: `link-drag-${Date.now()}`, cwd: "/tmp", invocation });
    sessionId = session.id;
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
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["session", () => sessionId && cleanupSession(request, sessionId)],
      ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
    ]);
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
    let sessionId: string | undefined;
    let bodyFailed = false;
    try {
      await patchPreferences(request, { terminal_link_click: null });
      const session = await createSession(request, { title: `link-overlap-${Date.now()}`, cwd: "/tmp", invocation });
      sessionId = session.id;
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
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["left button", () => page.mouse.up({ button: "left" })],
        ["right button", () => page.mouse.up({ button: "right" })],
        ["session", () => sessionId && cleanupSession(request, sessionId)],
        ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
      ]);
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
  let sessionId: string | undefined;
  let bodyFailed = false;
  try {
    await patchPreferences(request, { terminal_link_click: "program" });
    const session = await createSession(request, { title: `link-no-mouse-${Date.now()}`, cwd: "/tmp", invocation });
    sessionId = session.id;
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
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["session", () => sessionId && cleanupSession(request, sessionId)],
      ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
    ]);
  }
});

/** These tests keep the real terminal and link providers. Approval listings,
 * file replies and the desktop-only tunnel notice are controlled; every
 * disclosure still comes from the real provider after pointer movement.
 */
test.describe("covered terminal disclosures", () => {
  // Scope the reset/build-stamp capture to these additions. The cleanup hook
  // drains routed replies before Playwright disposes their page context.
  installTerminalSuiteHooks();

  /** One unrelated live approval makes the actual overlay region measurable.
   * A URL request keeps this fixture compact without hiding an approval's
   * rendering or layout behind a hand-made overlay.
   */
  async function stageCoveringApproval(page: Page) {
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    await page.route(url => url.pathname === "/api/approvals", route => fulfillAsHelm(route, {
      json: {
        approvals: [{
          id: "covering-link-disclosure",
          host_id: 1,
          host_name: "fixture",
          session: { id: "7c1e9a52-4b0d-4f7e-9a3c-2d81f6b0e413", title: "requester", host_name: "fixture" },
          expires_at_ms: Date.now() + 540_000,
          action: { kind: "open_url", url: "https://example.test/approval", preapproved: false, local_network: false },
        }],
      },
    }));
  }

  /** Observe synchronous browser opening without navigating away from the
   * fixture. The refusal message supplies the negative assertion's boundary.
   */
  async function probeDisclosureOpens(page: Page) {
    await page.addInitScript(() => {
      (window as any).__disclosureOpens = [];
      window.open = ((url: string) => {
        (window as any).__disclosureOpens.push(String(url));
        return null;
      }) as typeof window.open;
    });
    return () => page.evaluate(() => (window as any).__disclosureOpens as string[]);
  }

  /** Add display bytes to the attached terminal while the real fake-agent
   * keeps input ownership. Saving the cursor preserves its log area; this is
   * the same display-only seam already used by this spec's OSC file case.
   */
  async function paintDisclosureLink(page: Page, label: string, target: string | null, row: number, col: number) {
    await page.evaluate(({ label, target, row, col }) => new Promise<void>(resolve => {
      const text = target ? `\x1b]8;;${target}\x07${label}\x1b]8;;\x07` : label;
      (window as any).__farhelmTerm.write(`\x1b7\x1b[${row};${col}H\x1b[2K${text}\x1b8`, resolve);
    }), { label, target, row, col });
    return cell(page, label);
  }

  /** Put the pointer left of a cover while its ordinary below-right hover
   * reaches beneath it. For approval cards the allowed location is below
   * the same live card, so the positive control keeps approvals present.
   */
  async function besideCover(page: Page, label: string, target: string | null, covered: boolean, coverSelector = ".approval-cards") {
    await page.evaluate(() => document.fonts.ready);
    const grid = await page.evaluate(({ label, covered, coverSelector }) => {
      const term = (window as any).__farhelmTerm;
      const screen = document.querySelector("#terminal .xterm-screen")!.getBoundingClientRect();
      const region = document.querySelector(coverSelector)!.getBoundingClientRect();
      const width = screen.width / term.cols;
      const height = screen.height / term.rows;
      return {
        row: Math.floor(((covered ? region.top + 12 : region.bottom + 28) - screen.top) / height) + 1,
        col: Math.floor((region.left - 24 - screen.left) / width - label.length / 2) + 1,
        rows: term.rows,
        cols: term.cols,
      };
    }, { label, covered, coverSelector });
    expect(grid.row, "the target row must be in this live terminal").toBeGreaterThan(0);
    expect(grid.row).toBeLessThanOrEqual(grid.rows);
    expect(grid.col, "the entire label must fit beside the covering surface").toBeGreaterThan(0);
    expect(grid.col + label.length).toBeLessThanOrEqual(grid.cols);
    return paintDisclosureLink(page, label, target, grid.row, grid.col);
  }

  /** Establish both causes independently: the link receives real pointer
   * input, and its otherwise fitting hover intersects the higher cover region.
   * Reading the rectangles also gives useful fixture state on failure.
   */
  async function assertDisclosureGeometry(page: Page, selector: string, point: { x: number; y: number }, covered: boolean, coverSelector = ".approval-cards") {
    await expect(page.locator(selector)).toBeVisible();
    await expect.poll(() => page.locator(selector).evaluate((hover, { point, coverSelector }) => {
      const card = document.querySelector(coverSelector)!;
      const h = hover.getBoundingClientRect();
      const c = card.getBoundingClientRect();
      return {
        fits: hover.clientWidth > 0 && hover.clientHeight > 0 && hover.scrollHeight <= hover.clientHeight &&
          hover.scrollWidth <= hover.clientWidth && h.top >= 0 && h.left >= 0 && h.bottom <= innerHeight && h.right <= innerWidth,
        overlaps: h.left < c.right && h.right > c.left && h.top < c.bottom && h.bottom > c.top,
        pointerUncovered: !!document.elementFromPoint(point.x, point.y)?.closest("#terminal"),
        coverAboveHover: Number(getComputedStyle(card).zIndex) > Number(getComputedStyle(hover).zIndex),
      };
    }, { point, coverSelector }), { message: "live pointer, fitting disclosure and covering-region intersection" }).toEqual({
      fits: true, overlaps: covered, pointerUncovered: true, coverAboveHover: true,
    });
  }

  /** Refusal must be a new, readable outcome above the surface that covered
   * the destination, not an ordinary low-layer transfer failure.
   */
  async function assertCoveredRefusal(page: Page, operation: "download" | "open", coverSelector = ".approval-cards") {
    const notice = page.locator('#terminal .terminal-file-message[data-outcome="disclosure-refusal"]');
    await expect(notice).toContainText(`cannot ${operation}:`);
    await expect(notice).toContainText("covered by another Farhelm surface");
    await expect(notice).toBeVisible();
    await expect.poll(() => notice.evaluate((el, coverSelector) => {
      const card = document.querySelector(coverSelector)!;
      const box = el.getBoundingClientRect();
      return Number(getComputedStyle(el).zIndex) > Number(getComputedStyle(card).zIndex) &&
        box.top >= 0 && box.bottom <= innerHeight && el.scrollHeight <= el.clientHeight;
    }, coverSelector), { message: "the full refusal is inside the window and above the covering surface" }).toBe(true);
  }

  /** The desktop renders this notice outside browser builds. Inject its real
   * class and child structure, retaining production CSS while fixing a tall
   * geometry that crosses both the hover and the terminal's refusal banner.
   */
  async function stageCoveringTunnelNotice(page: Page) {
    await page.evaluate(() => {
      const screen = document.querySelector("#terminal .xterm-screen")!.getBoundingClientRect();
      const term = (window as any).__farhelmTerm;
      const notice = document.createElement("div");
      notice.className = "tunnel-error-notice";
      notice.setAttribute("role", "alert");
      notice.style.top = `${screen.top}px`;
      // Keep the cell helper's bottom-right reset inside the terminal. A
      // covered reset leaves xterm's last link-cell cache unchanged, so the
      // second click form would never obtain a fresh file hover or lookup.
      notice.style.bottom = `${innerHeight - screen.bottom + 2 * screen.height / term.rows}px`;
      notice.style.width = "480px";
      const text = document.createElement("span");
      text.textContent = "fixture tunnel failure";
      const dismiss = document.createElement("button");
      dismiss.type = "button";
      dismiss.className = "btn btn-neutral";
      dismiss.textContent = "dismiss";
      dismiss.addEventListener("click", () => notice.remove());
      notice.append(text, dismiss);
      document.body.appendChild(notice);
    });
    await expect(page.locator(".tunnel-error-notice")).toBeVisible();
  }

  /** A layer number alone does not prove readability across stacking contexts.
   * Sample the actual refusal/notice intersection without moving the pointer.
   * Refusals normally ignore hit testing, so enable it only during this sample
   * and restore the exact inline property before any further user input.
   */
  async function assertRefusalAboveTunnelNotice(page: Page) {
    const refusal = page.locator('#terminal .terminal-file-message[data-outcome="disclosure-refusal"]');
    await expect.poll(() => refusal.evaluate(el => {
      const notice = document.querySelector(".tunnel-error-notice")!;
      const r = el.getBoundingClientRect();
      const n = notice.getBoundingClientRect();
      const left = Math.max(r.left, n.left);
      const top = Math.max(r.top, n.top);
      const right = Math.min(r.right, n.right);
      const bottom = Math.min(r.bottom, n.bottom);
      if (right <= left || bottom <= top) return false;
      const style = (el as HTMLElement).style;
      const previous = style.getPropertyValue("pointer-events");
      const priority = style.getPropertyPriority("pointer-events");
      try {
        style.setProperty("pointer-events", "auto");
        return document.elementFromPoint((left + right) / 2, (top + bottom) / 2) === el;
      } finally {
        if (previous) style.setProperty("pointer-events", previous, priority);
        else style.removeProperty("pointer-events");
      }
    }), { message: "a tall tunnel notice intersects the refusal but paints below it" }).toBe(true);
  }

  /** Both browser click forms must reach admission, refuse without a request,
   * then permit a real Blob download once the same file's hover is uncovered.
   * Browser modifiers exercise the shared gate; native opening is not covered.
   */
  test("a covered file hover refuses plain and modifier clicks before transfer", async ({ page, request }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const opens = await probeDisclosureOpens(page);
    let sessionId: string | undefined;
    let bodyFailed = false;
    let transfers = 0;
    try {
      await patchPreferences(request, { terminal_link_click: null });
      const session = await createSession(request, { title: `file-covered-${Date.now()}`, cwd: "/tmp", invocation });
      sessionId = session.id;
      await stageCoveringApproval(page);
      page.on("request", request => {
        if (new URL(request.url()).pathname === `/api/sessions/${encodeURIComponent(session.id)}/files/download`) transfers++;
      });
      await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/stat`, route => fulfillAsHelm(route, {
        json: { status: "ready", host: "fixture", path: "/output/report.txt", size: 8, can_open: true },
      }));
      await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/download`, route => fulfillAsHelm(route, {
        status: 200, contentType: "application/octet-stream", body: "fixture\n",
      }));
      await page.goto("/");
      await attachSession(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await cue(page, "links");
      await expect(page.locator(".approval-card .approval-allow")).toBeEnabled();
      const modifier = await page.evaluate(() => navigator.platform.includes("Mac") ? "Meta" : "Control");
      const disclosure = "#terminal .terminal-file-target";
      for (const modified of [false, true]) {
        const point = await besideCover(page, "/output/report.txt", null, true);
        await expect(page.locator(disclosure)).toContainText("8 B");
        await expectLinkUnderPointer(page, point, "/output/report.txt");
        await assertDisclosureGeometry(page, disclosure, point, true);
        await expect(page.locator('#terminal .terminal-file-message[data-outcome="disclosure-refusal"]')).toHaveCount(0);
        if (modified) await page.keyboard.down(modifier);
        await page.mouse.click(point.x, point.y);
        if (modified) await page.keyboard.up(modifier);
        await assertCoveredRefusal(page, "download");
        expect(transfers, "refusal is the completed admission boundary").toBe(0);
        expect(await opens()).toEqual([]);
      }
      const point = await besideCover(page, "/output/report.txt", null, false);
      await expect(page.locator(disclosure)).toContainText("8 B");
      await assertDisclosureGeometry(page, disclosure, point, false);
      const downloaded = page.waitForEvent("download");
      await page.mouse.click(point.x, point.y);
      expect((await downloaded).suggestedFilename()).toBe("report.txt");
      expect(transfers).toBe(1);
      await expect(page.locator("#terminal .terminal-file-message.showing")).toHaveText("downloaded report.txt");
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["Control key", () => page.keyboard.up("Control")],
        ["Meta key", () => page.keyboard.up("Meta")],
        ["session", () => sessionId && cleanupSession(request, sessionId)],
        ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
      ]);
    }
  });

  /** OSC output owes target disclosure even for an innocent-looking label.
   * Plain URL text owes no hover: the adjacent real card must not disable it.
   */
  test("covered OSC web targets refuse while plain URLs and uncovered OSC targets open", async ({ page, request }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const opens = await probeDisclosureOpens(page);
    let sessionId: string | undefined;
    let bodyFailed = false;
    try {
      await patchPreferences(request, { terminal_link_click: null });
      const session = await createSession(request, { title: `web-covered-${Date.now()}`, cwd: "/tmp", invocation });
      sessionId = session.id;
      await stageCoveringApproval(page);
      await page.goto("/");
      await attachSession(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await cue(page, "links");
      await expect(page.locator(".approval-card .approval-allow")).toBeEnabled();
      const target = "https://example.test/covered-osc";
      const disclosure = "#terminal .terminal-link-target:not(.terminal-file-target)";
      const point = await besideCover(page, "COVERED-WEB", target, true);
      await expect(page.locator(disclosure)).toContainText(target);
      await expectLinkUnderPointer(page, point, "COVERED-WEB");
      await assertDisclosureGeometry(page, disclosure, point, true);
      await page.mouse.click(point.x, point.y);
      await assertCoveredRefusal(page, "open");
      expect(await opens()).toEqual([]);

      const plain = "https://p.test";
      const plainPoint = await besideCover(page, plain, null, true);
      await expect(page.locator(disclosure)).toBeHidden();
      expect(await page.evaluate(point => !!document.elementFromPoint(point.x, point.y)?.closest("#terminal"), plainPoint)).toBe(true);
      await expectLinkUnderPointer(page, plainPoint, plain);
      await page.mouse.click(plainPoint.x, plainPoint.y);
      expect(await opens()).toEqual([plain]);

      const uncovered = await besideCover(page, "UNCOVERED-WEB", target, false);
      await expect(page.locator(disclosure)).toContainText(target);
      await assertDisclosureGeometry(page, disclosure, uncovered, false);
      await page.mouse.click(uncovered.x, uncovered.y);
      expect(await opens()).toEqual([plain, target]);
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["session", () => sessionId && cleanupSession(request, sessionId)],
        ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
      ]);
    }
  });

  /** A persistent tunnel failure can hide ready file details without taking
   * the pointer off the path. Both click forms must refuse before requesting
   * a transfer, and the tall failure must not conceal that refusal in turn.
   */
  test("a tunnel notice covering a file hover refuses plain and modifier clicks", async ({ page, request }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const opens = await probeDisclosureOpens(page);
    let sessionId: string | undefined;
    let bodyFailed = false;
    let transfers = 0;
    try {
      await patchPreferences(request, { terminal_link_click: null });
      const session = await createSession(request, { title: `file-tunnel-covered-${Date.now()}`, cwd: "/tmp", invocation });
      sessionId = session.id;
      page.on("request", request => {
        if (new URL(request.url()).pathname === `/api/sessions/${encodeURIComponent(session.id)}/files/download`) transfers++;
      });
      await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/stat`, route => fulfillAsHelm(route, {
        json: { status: "ready", host: "fixture", path: "/output/report.txt", size: 8, can_open: true },
      }));
      await page.route(`**/api/sessions/${encodeURIComponent(session.id)}/files/download`, route => fulfillAsHelm(route, {
        status: 200, contentType: "application/octet-stream", body: "fixture\n",
      }));
      await page.goto("/");
      await attachSession(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await cue(page, "links");
      await stageCoveringTunnelNotice(page);
      const modifier = await page.evaluate(() => navigator.platform.includes("Mac") ? "Meta" : "Control");
      const disclosure = "#terminal .terminal-file-target";
      const label = "/output/report.txt";
      for (const modified of [false, true]) {
        const point = await besideCover(page, label, null, true, ".tunnel-error-notice");
        await expect(page.locator(disclosure)).toContainText("8 B");
        await expectLinkUnderPointer(page, point, label);
        await assertDisclosureGeometry(page, disclosure, point, true, ".tunnel-error-notice");
        await expect(page.locator('#terminal .terminal-file-message[data-outcome="disclosure-refusal"]')).toHaveCount(0);
        if (modified) await page.keyboard.down(modifier);
        await page.mouse.click(point.x, point.y);
        if (modified) await page.keyboard.up(modifier);
        await assertCoveredRefusal(page, "download", ".tunnel-error-notice");
        await assertRefusalAboveTunnelNotice(page);
        expect(transfers, "the completed refusal must precede any transfer request").toBe(0);
        expect(await opens()).toEqual([]);
      }
      // Fresh hover of the same cells proves admission recovers when only the
      // covering notice is removed, without a new lookup target or terminal.
      await page.locator(".tunnel-error-notice").evaluate(el => el.remove());
      const point = await cell(page, label);
      await expect(page.locator(disclosure)).toContainText("8 B");
      await expectLinkUnderPointer(page, point, label);
      const downloaded = page.waitForEvent("download");
      await page.mouse.click(point.x, point.y);
      expect((await downloaded).suggestedFilename()).toBe("report.txt");
      expect(transfers).toBe(1);
      await expect(page.locator("#terminal .terminal-file-message.showing")).toHaveText("downloaded report.txt");
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["Control key", () => page.keyboard.up("Control")],
        ["Meta key", () => page.keyboard.up("Meta")],
        ["session", () => sessionId && cleanupSession(request, sessionId)],
        ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
      ]);
    }
  });

  /** OSC labels do not reveal their destinations. A notice beside the live
   * link must block opening while its hover is hidden, then permit the same
   * target after dismissal; its refusal must remain readable meanwhile.
   */
  test("a tunnel notice covering an OSC web hover refuses opening", async ({ page, request }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const opens = await probeDisclosureOpens(page);
    let sessionId: string | undefined;
    let bodyFailed = false;
    try {
      await patchPreferences(request, { terminal_link_click: null });
      const session = await createSession(request, { title: `web-tunnel-covered-${Date.now()}`, cwd: "/tmp", invocation });
      sessionId = session.id;
      await page.goto("/");
      await attachSession(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await cue(page, "links");
      await stageCoveringTunnelNotice(page);
      const target = "https://example.test/tunnel-covered-osc";
      const label = "COVERED-WEB";
      const disclosure = "#terminal .terminal-link-target:not(.terminal-file-target)";
      const point = await besideCover(page, label, target, true, ".tunnel-error-notice");
      await expect(page.locator(disclosure)).toContainText(target);
      await expectLinkUnderPointer(page, point, label);
      await assertDisclosureGeometry(page, disclosure, point, true, ".tunnel-error-notice");
      await page.mouse.click(point.x, point.y);
      await assertCoveredRefusal(page, "open", ".tunnel-error-notice");
      await assertRefusalAboveTunnelNotice(page);
      expect(await opens()).toEqual([]);

      await page.locator(".tunnel-error-notice").evaluate(el => el.remove());
      const uncovered = await cell(page, label);
      await expect(page.locator(disclosure)).toContainText(target);
      await expectLinkUnderPointer(page, uncovered, label);
      await page.mouse.click(uncovered.x, uncovered.y);
      expect(await opens()).toEqual([target]);
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["session", () => sessionId && cleanupSession(request, sessionId)],
        ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
      ]);
    }
  });

  /** The web hover renders its host without eliding it. A long real host in
   * a short window exceeds the viewport without any fixture CSS or fake DOM;
   * enlarging the window must restore opening of that exact target.
   */
  test("an OSC web hover clipped by the window refuses until it fits", async ({ page, request }) => {
    await page.setViewportSize({ width: 900, height: 220 });
    const opens = await probeDisclosureOpens(page);
    let sessionId: string | undefined;
    let bodyFailed = false;
    try {
      await patchPreferences(request, { terminal_link_click: null });
      const session = await createSession(request, { title: `web-clipped-${Date.now()}`, cwd: "/tmp", invocation });
      sessionId = session.id;
      await page.goto("/");
      await attachSession(page, session.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await cue(page, "links");
      const target = `https://${"segment.".repeat(180)}example.test/path`;
      const label = "https://claimed.test/safe";
      const point = await paintDisclosureLink(page, label, target, 1, 1);
      const disclosure = page.locator("#terminal .terminal-link-target:not(.terminal-file-target)");
      await expect(disclosure).toContainText("link text does not match where it goes");
      await expect.poll(() => disclosure.evaluate(el => el.getBoundingClientRect().bottom > innerHeight), {
        message: "the real host disclosure must extend below the small window",
      }).toBe(true);
      await expectLinkUnderPointer(page, point, label);
      await page.mouse.click(point.x, point.y);
      const notice = page.locator('#terminal .terminal-file-message[data-outcome="disclosure-refusal"]');
      await expect(notice).toContainText("cannot open:");
      await expect(notice).toContainText("not visible");
      await expect(notice).toBeVisible();
      expect(await opens()).toEqual([]);

      const shortRows = await page.evaluate(() => (window as any).__farhelmTerm.rows);
      await page.setViewportSize({ width: 1280, height: 1200 });
      await expect.poll(() => page.evaluate(() => (window as any).__farhelmTerm.rows)).toBeGreaterThan(shortRows);
      const fitting = await paintDisclosureLink(page, label, target, 1, 1);
      await expect.poll(() => disclosure.evaluate(el => {
        const rect = el.getBoundingClientRect();
        return rect.top >= 0 && rect.left >= 0 && rect.bottom <= innerHeight && rect.right <= innerWidth &&
          el.scrollHeight <= el.clientHeight && el.scrollWidth <= el.clientWidth;
      })).toBe(true);
      await page.mouse.click(fitting.x, fitting.y);
      expect(await opens()).toEqual([target]);
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["session", () => sessionId && cleanupSession(request, sessionId)],
        ["link preference", () => patchPreferences(request, { terminal_link_click: null })],
      ]);
    }
  });
});
