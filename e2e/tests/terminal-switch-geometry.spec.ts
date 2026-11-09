import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { cleanupSession, createSession } from "./helpers/fleet";
import { terminalFit } from "./helpers/term";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";

// The program redraws only when its PTY changes size. Its last word and rule
// reach the final column, where an unnecessary resize is easiest to see.
const DRAWING_PROGRAM = String.raw`python3 -u -c '
import os, signal, sys
def draw(*args):
    cols = os.get_terminal_size().columns
    sys.stdout.write("\x1b[?25l\x1b[2J\x1b[H" + "x" * (cols - 3) + "\x1b[31myou\x1b[0m")
    sys.stdout.write("\x1b[3;1H" + "─" * cols + "\x1b[5;1HGEOMETRY READY " + str(cols))
    sys.stdout.flush()
signal.signal(signal.SIGWINCH, draw)
draw()
while True:
    signal.pause()
'`;

for (const font of [13, 14]) {
  /** Reselecting a quiet session must keep its already-drawn right edge still.
   * The program reports the size it actually received, independently of the
   * client's fitted proposal. Per-frame samples include transient visible
   * layouts after reveal, rather than waiting away a resize before comparing.
   */
  test(`session switching keeps full-width text and rules still at ${font}px`, async ({ page, request }, testInfo) => {
    let bodyFailed = true;
    const owned: string[] = [];
    try {
      await page.addInitScript((font) => {
        localStorage.setItem("farhelm.terminal-font-size", String(font));
        const w = window as any;
        w.__switchGeometry = [];
        w.__switchSizing = [];
        let api: any;
        let mount = 0;
        Object.defineProperty(window, "farhelmGridTerminal", {
          configurable: true,
          get: () => api,
          set(value) {
            api = { ...value, create(container: HTMLElement, options: any) {
              const id = ++mount;
              const grid = value.create(container, options);
              let alive = true;
              const record = (event: string) => {
                if (w.__switchSizing.length < 200) w.__switchSizing.push({
                  event, mount: id, time: performance.now(), fitted: grid.size(),
                  cols: grid.model.cols, visible: getComputedStyle(grid.element).visibility,
                });
              };
              for (const method of ["fit", "receive", "flush"]) {
                const original = grid[method];
                grid[method] = (...args: any[]) => {
                  const result = original.apply(grid, args);
                  record(method);
                  return result;
                };
              }
              const dispose = grid.dispose;
              grid.dispose = () => { alive = false; dispose(); };
              const rect = (node: Element) => {
                const box = node.getBoundingClientRect();
                return { x: box.x, y: box.y, width: box.width, height: box.height };
              };
              const sample = () => {
                if (!alive) return;
                const rows = grid.element.querySelectorAll(".grid-terminal-row");
                const tail = rows[0] && [...rows[0].children].find((span: any) => span.textContent === "you");
                const rule = rows[2] && [...rows[2].children];
                if (tail && rule?.length && rule.map((span: any) => span.textContent).join("").includes("─") &&
                    getComputedStyle(grid.element).visibility === "visible" && w.__switchGeometry.length < 200) {
                  w.__switchGeometry.push({
                    mount: id, time: performance.now(), cols: grid.model.cols,
                    screen: rect(grid.element.querySelector(".grid-terminal-screen")),
                    row: rect(rows[0]), tail: rect(tail),
                    rule: rule.map((span: any) => ({ className: span.className, ...rect(span) })),
                  });
                }
                requestAnimationFrame(sample);
              };
              requestAnimationFrame(sample);
              return grid;
            } };
          },
        });
      }, font);
      const first = await createSession(request, { title: `geometry-${font}-${Date.now()}`, invocation: DRAWING_PROGRAM });
      owned.push(first.id);
      const other = await createSession(request, { title: `geometry-away-${font}-${Date.now()}`, invocation: "sleep 300" });
      owned.push(other.id);
      await page.setViewportSize({ width: 1280, height: 900 });
      await page.goto("/");
      await page.locator(`[data-session-id="${first.id}"]`).click();
      await waitForSessionRevealed(page, first.id);
      // Select a real font-rounding boundary where the engine has one; the
      // ordinary width still checks switching where its metrics are exact.
      const pane = await page.locator("#terminal").evaluate((element) => {
        const grid = element.querySelector(".grid-terminal")!;
        const style = getComputedStyle(grid);
        const context = new OffscreenCanvas(100, 100).getContext("2d")!;
        context.font = `${style.fontSize} ${style.fontFamily}`;
        const advance = context.measureText("W").width;
        const initial = Math.round(advance * 80) / 80;
        let room = 811;
        for (let candidate = 650; candidate <= 1400; candidate++) {
          const cols = Math.floor(candidate / initial);
          if (Math.floor(candidate / (Math.round(advance * cols) / cols)) !== cols) { room = candidate; break; }
        }
        return { current: parseInt(getComputedStyle(element).width, 10), desired: room + 22, advance };
      });
      await page.evaluate(() => { (window as any).__switchGeometry = []; });
      await page.setViewportSize({ width: 1280 + pane.desired - pane.current, height: 900 });
      await expect.poll(async () => {
        const size = await terminalFit(page);
        if (!size) return false;
        return page.locator(".grid-terminal-screen").textContent().then((text) => text?.includes(`GEOMETRY READY ${size.cols}`));
      }).toBe(true);
      // Wait for this pane's completed paint, not an earlier sample from
      // before the viewport changed. The return trace below stays intact
      // throughout its readiness wait so an intermediate resize cannot hide.
      const paintedAtFit = async () => {
        const size = await terminalFit(page);
        return page.evaluate(({ size, advance }) => {
          const sample = (window as any).__switchGeometry.at(-1);
          return !!size && sample?.cols === size.cols && sample.screen.width === Math.round(advance * size.cols) &&
            document.querySelector(".grid-terminal-screen")?.textContent?.includes(`GEOMETRY READY ${size.cols}`);
        }, { size, advance: pane.advance });
      };
      await expect.poll(paintedAtFit).toBe(true);
      /** Exercise genuine resize history before comparing with a fresh session view.
       * Observe the actual fit callback for each changed pane so a pending
       * ResizeObserver cannot leave the old dimensions as a false baseline.
       * Keep the program/paint oracle: a proposed size alone is not a redraw.
       */
      const resizeAndPaint = async (width: number, height: number) => {
        const fits = await page.evaluate(() => (window as any).__switchSizing.filter((sample: any) => sample.event === "fit").length);
        await page.setViewportSize({ width, height });
        await expect.poll(() => page.evaluate(() => (window as any).__switchSizing.filter((sample: any) => sample.event === "fit").length))
          .toBeGreaterThan(fits);
        await expect.poll(paintedAtFit).toBe(true);
      };
      const finalWidth = 1280 + pane.desired - pane.current;
      await resizeAndPaint(finalWidth - 2, 898);
      await resizeAndPaint(finalWidth, 900);
      const baseline = await page.evaluate(() => (window as any).__switchGeometry.at(-1));
      const content = await page.locator(".grid-terminal-screen").evaluate((screen) => {
        const rows = screen.querySelectorAll(".grid-terminal-row");
        return { text: rows[0].textContent, rule: rows[2].textContent };
      });
      expect(content, "premise: both lines fill the program's current PTY width").toEqual({
        text: "x".repeat(baseline.cols - 3) + "you",
        rule: "─".repeat(baseline.cols),
      });
      await page.locator(`[data-session-id="${other.id}"]`).click();
      await waitForSessionRevealed(page, other.id);
      await page.evaluate(() => { (window as any).__switchGeometry = []; (window as any).__switchSizing = []; });
      await page.locator(`[data-session-id="${first.id}"]`).click();
      await waitForSessionRevealed(page, first.id);
      await expect.poll(paintedAtFit).toBe(true);
      await expect.poll(() => page.evaluate(() => (window as any).__switchGeometry.length)).toBeGreaterThanOrEqual(4);
      const evidence = await page.evaluate(() => ({ geometry: (window as any).__switchGeometry, sizing: (window as any).__switchSizing }));
      await testInfo.attach("switch-geometry.json", { body: JSON.stringify({ font, pane, baseline, ...evidence }), contentType: "application/json" });
      const { mount: _mount, time: _time, ...expected } = baseline;
      for (const { mount: _id, time: _at, ...geometry } of evidence.geometry) expect(geometry).toEqual(expected);
      bodyFailed = false;
    } finally {
      await cleanupOwnedSteps(bodyFailed, owned.map((id): [string, () => Promise<void>] => [`delete ${id}`, () => cleanupSession(request, id)]));
    }
  });
}
