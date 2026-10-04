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
