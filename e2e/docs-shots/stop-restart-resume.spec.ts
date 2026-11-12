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

/** Distinguish the session actions while leaving disabled Restart's explanation visible. */
test("header actions", async ({ page, request }) => {
  const { director } = await open(page, request);
  // The row at the top of the open session, which holds its terminal tabs
  // and, at its far end, these buttons.
  const header = page.locator("header.session-topbar");
  await expect(header).toBeVisible();
  await director.callout(header.locator(".header-stop"), "Stop: ends the agent and closes its shell tabs. The session stays.", {
    side: "bottom",
    dx: -210,
  });
  await director.callout(
    header.locator(".restart-primary"),
    "Restart: resumes this session's conversation. Greyed out, as here, when Farhelm cannot resume it; hover to see why.",
    { side: "bottom", dx: -90, dy: 100 },
  );
  await director.callout(header.locator(".header-replace"), "Replace: a fresh conversation with the same settings.", {
    side: "bottom",
    dx: -50,
    dy: 230,
  });
  await director.callout(header.locator(".header-clone"), "Clone: another session like this one, which stays.", {
    side: "bottom",
    dy: 340,
  });
  // Stagger the boxes diagonally, in button order: each box sits lower and
  // further right than the one before, so each arrow drops past the right
  // edge of every box above it without crossing one.
  // The action buttons sit at the row's far end; crop to them rather than
  // the whole row, which is wider than the docs' text column.
  await shot(page, `${PAGE}/header-actions`, [header.locator(".header-stop"), header.locator(".header-delete").first()]);
});

/** Show Stop's tab-closing behavior without claiming it contains detached work. */
test("row menu", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const row = rowOf(page, scenario, (session) => session.status === "waiting");
  await openRowMenu(row);
  const panel = page.locator(".session-row-menu-panel");
  await expect(panel).toBeVisible();
  await director.callout(
    panel.locator(".session-row-stop"),
    "Stop: ends the agent and closes its terminal tabs. Some detached jobs on Linux can keep running. The session stays.",
    { side: "right", dy: -50 },
  );
  await director.callout(panel.locator(".session-row-delete"), "Delete: removes the session altogether.", {
    side: "right",
    dy: 30,
  });
  await shot(page, `${PAGE}/row-menu`, [row, panel], { maxWidth: 930 });
});
