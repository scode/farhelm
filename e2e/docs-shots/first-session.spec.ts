/**
 * Screenshots for the docs page "Your first session"
 * (website/src/content/docs/docs/get-started/first-session.mdx).
 *
 * Each test is one named shot; the annotation wording is part of the page's
 * content and carries as much of it as the picture can (website/EDITORIAL_RULES.md,
 * "Rules from feedback"). Nothing here launches a session or changes a
 * setting: the staged fleet already has a working session to show.
 */
import { expect, test } from "@playwright/test";
import { open } from "./open";
import { shot } from "./shot";
import { readFleet } from "./stage-docs";

const PAGE = "first-session";

test("this machine", async ({ page, request }) => {
  const { director } = await open(page, request);
  const local = page.locator('.host-row[data-host-kind="local"]');
  await expect(local).toBeVisible();
  await director.highlight(local, { pad: 2 });
  await director.callout(local, "The Mac you are on is already a host, so sessions can start here straight away.", {
    side: "right",
  });
  await shot(page, `${PAGE}/this-machine`, [page.locator(".host-list")], { maxWidth: 900 });
});

test("a running session", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const openSession = scenario.sessions.find((session) => session.open);
  if (!openSession) throw new Error("the docs scenario needs an open session");
  const row = page.locator(".session-row.selected");
  await expect(row).toBeVisible();
  await director.callout(
    row,
    "Your session in the session list. The dot shows what its agent is doing: green and pulsing while it works.",
    { side: "right", dy: 90 },
  );
  // The terminal fills the window's height; anchoring on a line of it keeps
  // the crop to the part that shows something.
  const line = await director.terminalText("I'll read the spec");
  await director.callout(line, "The agent's own terminal. Type your first prompt here, as you would in any terminal.", {
    side: "right",
    dy: 40,
  });
  await shot(page, `${PAGE}/running-session`, [row, line], { maxWidth: 930 });
});

test("statuses", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const fleet = readFleet();
  const rowOf = (status: string, seen?: boolean) => {
    const session = scenario.sessions.find(
      (candidate) => candidate.status === status && (seen === undefined || candidate.seen === seen),
    );
    if (!session) throw new Error(`the docs scenario needs a ${status} session`);
    return page.locator(`[data-session-id="${fleet.ids.get(session.title)}"]`);
  };
  const waiting = rowOf("waiting");
  const unread = rowOf("idle", false);
  await director.callout(waiting, "Waiting: the agent is asking you something.", { side: "right" });
  await director.callout(unread, "Idle with a blue dot: it finished since you last looked.", { side: "right", dy: 40 });
  await shot(page, `${PAGE}/statuses`, [page.locator(".session-list")], { maxWidth: 900 });
});
