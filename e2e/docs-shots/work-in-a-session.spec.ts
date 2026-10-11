/**
 * Screenshots for the docs page "Work in a session"
 * (website/src/content/docs/docs/using/work-in-a-session.mdx).
 *
 * Each test is one named shot; the annotation wording is part of the page's
 * content. Nothing here is clicked except what only opens or shows something.
 */
import { expect, test } from "@playwright/test";
import { open } from "./open";
import { shot } from "./shot";

const PAGE = "work-in-a-session";

test("header", async ({ page, request }) => {
  const { director } = await open(page, request);
  const header = page.locator("header.titlebar");
  await expect(header).toBeVisible();
  const copies = header.locator(".header-copy");
  await director.callout(copies.first(), "Click to copy the session's directory, or next to it, the command it runs.", {
    side: "bottom",
    dx: 40,
  });
  // Only the left part of the header: the action buttons at its far end are
  // the Stop, restart, and resume page's to show.
  await shot(page, `${PAGE}/header`, [header.locator(".title"), copies.last()], {
    maxWidth: 930,
  });
});

test("tabs", async ({ page, request }) => {
  const { director } = await open(page, request);
  const strip = page.locator(".tab-strip");
  await expect(strip).toBeVisible();
  await director.callout(strip.locator(".tab-add"), "Opens a shell in the session's directory, as another tab beside the agent.", {
    side: "bottom",
    dx: 60,
  });
  await shot(page, `${PAGE}/tabs`, [strip.locator(".tab-agent"), strip.locator(".tab-add")], { maxWidth: 930 });
});

/**
 * Hovering a file path the agent printed, with the hover's check answered
 * from the scenario (`terminalFiles`): the staged session's real directory is
 * the run's temporary state, which no shot may show. Stops at the hover; a
 * click would only start a download, which the staging refuses anyway.
 */
test("download hover", async ({ page, request }) => {
  const { director } = await open(page, request, { terminalFiles: true });
  const needle = "e2e/attach.spec.ts";
  // Find the path's cell in the live terminal the way a person's pointer
  // would land on it: the terminal draws on a canvas, so its text has no
  // element to hover, only a position.
  let cell: { x: number; y: number } | null = null;
  await expect
    .poll(async () => {
      cell = await page.evaluate((needle) => {
        const term = (window as any).__farhelmTerm;
        if (!term) return null;
        const box = term.element.querySelector(".xterm-screen").getBoundingClientRect();
        for (let i = 0; i < term.rows; i++) {
          const text = term.buffer.active.getLine(term.buffer.active.viewportY + i)?.translateToString(true) || "";
          const column = text.indexOf(needle);
          if (column >= 0) {
            return {
              x: box.x + (column + needle.length / 2) * box.width / term.cols,
              y: box.y + (i + 0.5) * box.height / term.rows,
            };
          }
        }
        return null;
      }, needle);
      return cell !== null;
    }, { message: `the open session's terminal must show ${needle}` })
    .toBe(true);
  await page.mouse.move(cell!.x, cell!.y);
  const target = page.locator("#terminal .terminal-file-target");
  await expect(target).toContainText("click to download");
  await director.callout(
    target,
    "Hover a path to check it on the session's host: which host, the full path, and its size. Click to download it.",
    { side: "right", dx: 40 },
  );
  await shot(page, `${PAGE}/download-hover`, [target], { pad: 60, maxWidth: 930, keepPointer: true });
});
