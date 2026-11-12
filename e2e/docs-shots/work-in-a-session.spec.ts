/**
 * Screenshots for the docs page "Work in a session"
 * (website/src/content/docs/docs/using/work-in-a-session.mdx).
 *
 * Each test is one named shot; the annotation wording is part of the page's
 * content. Nothing here is clicked except what only opens or shows something.
 */
import { expect, test } from "@playwright/test";
import { terminalScreen, terminalViewport } from "../tests/helpers/term";
import { open } from "./open";
import { shot } from "./shot";

const PAGE = "work-in-a-session";

test("header", async ({ page, request }) => {
  const { director } = await open(page, request);
  // The session's details sit in a slim bar under its terminal; the shot
  // keeps the name "header" from when they headed the session.
  const details = page.locator(".session-details");
  await expect(details).toBeVisible();
  const copies = details.locator(".header-copy");
  // Above the bar, over the terminal's empty lower half: under it there is
  // nothing but the window's edge.
  await director.callout(copies.first(), "Click to copy the session's directory, or next to it, the command it runs.", {
    side: "top",
    dx: 40,
  });
  await shot(page, `${PAGE}/header`, [details.locator(".status-badge"), copies.last()], {
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
  // would land on it: the text has no element of its own to hover, only a
  // position, found from the terminal's visible rows and its screen's box
  // (the browser suite's file tests find it the same way).
  let cell: { x: number; y: number } | null = null;
  await expect
    .poll(async () => {
      const view = await terminalViewport(page);
      const box = await terminalScreen(page).boundingBox();
      cell = null;
      if (!view || !box) return false;
      for (let i = 0; i < view.rows; i++) {
        const column = (view.lines[i] ?? "").indexOf(needle);
        if (column >= 0) {
          cell = {
            x: box.x + (column + needle.length / 2) * box.width / view.cols,
            y: box.y + (i + 0.5) * box.height / view.rows,
          };
          break;
        }
      }
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
