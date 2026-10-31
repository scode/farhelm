/**
 * Screenshots for the docs page "Stop, restart, and resume"
 * (website/src/content/docs/docs/using/stop-restart-resume.mdx).
 *
 * Every action on this page ends or replaces a session, so the shots only
 * ever open menus, never use them. The staged sessions are command launches
 * with no resume command, so their restart is greyed out; the header shot
 * shows and explains that state rather than a working restart. Restart's
 * confirmation prompt is not shown at all: it needs a session Farhelm can
 * resume, which the staged fleet cannot honestly provide.
 */
import { expect, test } from "@playwright/test";
import { openRowMenu } from "../tests/helpers/fleet";
import { open, rowOf } from "./open";
import { shot } from "./shot";

const PAGE = "stop-restart-resume";

test("header actions", async ({ page, request }) => {
  const { director } = await open(page, request);
  // The row at the top of the open session, which holds its terminal tabs
  // and, at its far end, these buttons.
  const header = page.locator("header.session-topbar");
  await expect(header).toBeVisible();
  await director.callout(
    header.locator(".restart-primary"),
    "Restart: resumes this session's conversation. Greyed out, as here, when Farhelm cannot resume it; hover to see why.",
    { side: "bottom", dx: -200 },
  );
  await director.callout(header.locator(".header-replace"), "Replace: a fresh conversation with the same settings.", {
    side: "bottom",
    dx: -260,
    dy: 150,
  });
  await director.callout(header.locator(".header-clone"), "Clone: another session like this one, which stays.", {
    side: "bottom",
    dy: 150,
  });
  // An arrow ends at the point of its button nearest the box's centre, so the
  // clone box sits straight under clone: pushed right, its arrow met clone's
  // corner beside replace with. The replace box moves left to make room.
  // The action buttons sit at the row's far end; crop to them rather than
  // the whole row, which is wider than the docs' text column.
  await shot(page, `${PAGE}/header-actions`, [header.locator(".restart-primary"), header.locator(".header-delete").first()]);
});

test("row menu", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const row = rowOf(page, scenario, (session) => session.status === "waiting");
  await openRowMenu(row);
  const panel = page.locator(".session-row-menu-panel");
  await expect(panel).toBeVisible();
  await director.callout(
    panel.locator(".session-row-stop"),
    "Stop: ends the agent and everything it started. The session and its terminal stay.",
    { side: "right", dy: -5 },
  );
  await director.callout(panel.locator(".session-row-delete"), "Delete: removes the session altogether.", {
    side: "right",
    dy: 30,
  });
  await shot(page, `${PAGE}/row-menu`, [row, panel], { maxWidth: 930 });
});
