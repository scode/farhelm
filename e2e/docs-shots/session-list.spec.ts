/**
 * Screenshots for the docs page "Read the session list"
 * (website/src/content/docs/docs/using/session-list.mdx).
 *
 * Each test is one named shot; the annotation wording is part of the page's
 * content and carries as much of it as the picture can. Nothing here changes
 * a session or a setting: menus are opened and photographed, never used.
 */
import { expect, test } from "@playwright/test";
import { openRowMenu } from "../tests/helpers/fleet";
import { open, rowOf } from "./open";
import { shot } from "./shot";

const PAGE = "session-list";

test("a row", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const row = rowOf(page, scenario, (session) => session.status === "waiting");
  await expect(row).toBeVisible();
  // Three callouts on one row, pushed right past the list (dx) so they sit in
  // the terminal's space and stack without covering other rows.
  await director.callout(
    row.locator(".session-status-slot"),
    "Status: green and pulsing while the agent works, red when it waits for you, blue when it finished since you last looked, grey when idle.",
    { side: "right", dx: 330, dy: -110 },
  );
  await director.callout(
    row.locator(".session-agent"),
    "Which agent, and its permissions: an amber slashed shield is YOLO; a green shield is guarded, where a safety check can refuse an action or ask you.",
    { side: "right", dx: 70, dy: 60 },
  );
  // Below the row rather than beside it: the right-hand space already holds
  // the other two, and an arrow from there would cross one of them.
  await director.callout(row.locator(".session-host"), "The host and directory it runs in.", {
    side: "bottom",
    dy: 10,
  });
  await shot(page, `${PAGE}/row`, [row], { maxWidth: 930 });
});

test("statuses", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const waiting = rowOf(page, scenario, (session) => session.status === "waiting");
  const unread = rowOf(page, scenario, (session) => session.status === "idle" && session.seen === false);
  const exited = rowOf(page, scenario, (session) => session.status === "exited");
  await director.callout(waiting, "Waiting: the agent is asking you something. Answer it in its terminal.", {
    side: "right",
    dy: -30,
  });
  await director.callout(unread, "Idle and unread: it finished since you last looked. Opening it marks it read.", {
    side: "right",
    dy: 20,
  });
  await director.callout(exited, "Ended: the agent quit, here with exit code 0. Its terminal stays readable.", {
    side: "right",
    dy: 60,
  });
  await shot(page, `${PAGE}/statuses`, [page.locator(".session-list")], { maxWidth: 930 });
});

test("list controls", async ({ page, request }) => {
  const { director } = await open(page, request);
  // The menu stays closed: its choices carry their own descriptions, and
  // opening it hides the status filter behind it. The title's and the status
  // filter's callouts sit right of the list, the host's under the rail.
  const filter = page.locator(".host-rail-avatar").first();
  const sort = page.locator(".sort-title");
  const status = page.locator(".session-status-filter");
  await expect(status).toBeVisible();
  await director.callout(
    sort,
    "The list's order. Click it for Recently active, Newest created or Title A–Z, or for Compact: one line per session.",
    { side: "right", dx: 320, dy: 0 },
  );
  await director.callout(status, "Show only running, waiting or idle sessions. Ended sessions appear under All.", {
    side: "right",
    dx: 40,
    dy: 120,
  });
  // Below the rail's top rather than beside it, so its arrow runs up the rail
  // instead of across the list.
  await director.callout(filter, "Show only this host's sessions; All above it shows every host.", {
    side: "bottom",
    dx: 120,
    dy: 300,
  });
  await shot(page, `${PAGE}/controls`, [filter, sort, status], { maxWidth: 930 });
});

test("row menu", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const row = rowOf(page, scenario, (session) => session.status === "idle" && session.seen === false);
  await openRowMenu(row);
  const panel = page.locator(".session-row-menu-panel");
  await expect(panel).toBeVisible();
  await director.callout(panel.locator(".session-row-rename"), "Give the session a name you will recognise.", {
    side: "right",
  });
  await director.callout(panel.locator(".session-row-mark-seen"), "Mark it read, or unread again to come back to later.", {
    side: "right",
    dy: 90,
  });
  await shot(page, `${PAGE}/row-menu`, [row, panel], { maxWidth: 930 });
});

/**
 * The trash button beside new, holding the scenario's archived managed
 * checkouts. The trash is staged for this shot alone (`checkoutTrash`), so
 * every other shot shows the fleet's real, empty one.
 */
test("trash button", async ({ page, request }) => {
  const { director } = await open(page, request, { checkoutTrash: true });
  const button = page.locator(".checkout-trash-button");
  await expect(button.locator(".checkout-trash-count")).toHaveText("3");
  await director.callout(
    button,
    "Managed checkouts whose last session was deleted. The lid opens and a count shows while it holds any.",
    { side: "right", dx: 160, dy: -70 },
  );
  await shot(page, `${PAGE}/trash-button`, [button, page.locator(".new-session-button")], { maxWidth: 930 });
});

/**
 * The trash dialog over the same staged trash. Stops at the listing: delete
 * would only ask to confirm, and the staging refuses deletion outright anyway.
 */
test("trash dialog", async ({ page, request }) => {
  // About the docs' text column wide, with room below the dialog for the
  // callout. The delete buttons get no callout: they sit at the dialog's top
  // and right edges, where a callout would cover the listing or the dialog's
  // own explanation, so the page's text explains them instead.
  await page.setViewportSize({ width: 960, height: 1150 });
  const { director } = await open(page, request, { checkoutTrash: true });
  await page.locator(".checkout-trash-button").click();
  const dialog = page.locator(".checkout-trash-dialog");
  await expect(dialog.locator(".checkout-trash-entry")).toHaveCount(3);
  await director.callout(
    dialog.locator(".checkout-trash-path").last(),
    "Where each host keeps them. Copy out anything you want back before deleting.",
    { side: "bottom", dx: -40, dy: 60 },
  );
  await shot(page, `${PAGE}/trash-dialog`, [dialog], { maxWidth: 930 });
});
