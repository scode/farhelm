// The move to new terminals, as the session view shows it (SPEC.md,
// "Terminal experience"; crates/farhelm-ui/src/migration.rs): the migration
// band at the top of an old session, and the terminal kinds in the details
// bar under the terminal.
//
// Two kinds of evidence, kept apart on purpose:
//
// - The real stack's own replies. Its supervisors run with their internal
//   switch off, as every shipped supervisor does until the switch-on
//   release and as the media harnesses' stacks keep running after it, so
//   their sessions must look exactly as they always have: no band, no
//   terminal kinds. This is also the end-to-end check that the kinds and the
//   switch report reach the browser through the helm unchanged.
// - Seeded listings. A stack whose supervisor creates new terminals needs
//   the supervisor's `--new-terminals host` option, which this suite's
//   ordinary runs do not pass, and an old session beside it needs that
//   supervisor to have created one before the switch. So the band's cases,
//   its dismissal and its return are driven through the real UI and the
//   real stack, with only the session rows' terminal fields rewritten on the
//   way to the page, the way claude-trust.spec.ts seeds its trust question.
//   On the suite's host-terminal run (host-terminal-specs.ts) the real
//   terminals underneath are new ones; the seeded cases hold there too.
//
// A real-host leg (a stack whose supervisor creates new terminals) belongs
// with the browser configuration that starts such a stack, not here.

import type { Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createSession, renameSession } from "./helpers/fleet";
import { attachSession, onHostTerminals } from "./helpers/term";

/** What a seeded row says about its terminals; `undefined` leaves the real
 * stack's value. */
interface TerminalSeed {
  supervisor_creates_host_terminals?: boolean;
  agent_terminal?: "tmux" | "host";
  restart_offer?: string;
  /** Each listed tab's kind, by position. */
  tab_kinds?: ("tmux" | "host")[];
}

/** Rewrite one session's terminal fields in every listing and detail read
 * the page makes, leaving every other session and field as the helm sent
 * it. `seed()` is read per request, so a test changes it between visits. */
async function seedTerminals(page: Page, sessionId: string, seed: () => TerminalSeed) {
  const apply = (row: Record<string, unknown>) => {
    if (row.id !== sessionId) return row;
    const wanted = seed();
    const next: Record<string, unknown> = { ...row };
    if (wanted.supervisor_creates_host_terminals !== undefined) {
      next.supervisor_creates_host_terminals = wanted.supervisor_creates_host_terminals;
    }
    if (wanted.agent_terminal !== undefined) next.agent_terminal = wanted.agent_terminal;
    if (wanted.restart_offer !== undefined) next.restart_offer = wanted.restart_offer;
    if (wanted.tab_kinds !== undefined && Array.isArray(row.tabs)) {
      next.tabs = (row.tabs as Record<string, unknown>[]).map((tab, index) => ({
        ...tab,
        kind: wanted.tab_kinds![index] ?? tab.kind,
      }));
    }
    return next;
  };
  await page.route(
    (url) => url.pathname === "/api/sessions" || url.pathname === `/api/sessions/${encodeURIComponent(sessionId)}`,
    async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      try {
        const response = await route.fetch();
        if (!response.ok()) return route.fulfill({ response });
        const body = await response.json();
        await route.fulfill({
          response,
          json: Array.isArray(body.sessions) ? { ...body, sessions: body.sessions.map(apply) } : apply(body),
        });
      } catch (error) {
        // A reload (the band test's "a reload is a visit too") can leave a
        // listing in flight: Playwright then disposes its response, and the
        // page that asked for it is gone, so there is nobody left to answer.
        // Seen once in WebKit as "apiResponse.json: Response has been
        // disposed". Anything else is a real failure.
        if (/has been disposed|has been closed/.test(String(error))) return;
        throw error;
      }
    },
  );
}

/** Open a real tab through the API, as a second client would. */
async function openTab(request: Parameters<typeof cleanupSession>[0], sessionId: string): Promise<{ id: string; kind?: string }> {
  const opened = await request.post(`/api/sessions/${encodeURIComponent(sessionId)}/tabs`);
  expect(opened.ok(), await opened.text()).toBe(true);
  return (await opened.json()).tab;
}

/** The real stack's switch-off reply for one session, read past the UI. */
async function listedRow(request: Parameters<typeof cleanupSession>[0], sessionId: string) {
  const listed = await request.get("/api/sessions");
  expect(listed.ok()).toBe(true);
  const row = (await listed.json()).sessions.find((session: { id: string }) => session.id === sessionId);
  expect(row, "the session is listed").toBeTruthy();
  return row as Record<string, unknown> & { tabs: { id: string; kind?: string }[] };
}

const band = (page: Page) => page.locator(".migration-band");
const kinds = (page: Page) => page.locator(".session-details .session-details-terminals");

/** A supervisor that does not create new terminals reports its terminals as
 * old and the switch as off, and its sessions show neither the band nor the
 * terminal kinds. Why: that is every shipped supervisor until switch-on, and
 * the media harnesses' captures afterwards; any change there would show up
 * on every user's screen and in every published screenshot. */
test("a supervisor still on old terminals shows no band and no terminal kinds", async ({ page, request }) => {
  // About a stack whose supervisor keeps old terminals; on a host-terminal
  // run every stack creates new ones, and host-terminals/migration.spec.ts
  // checks that side (new kinds named, no band).
  test.skip(onHostTerminals(), "needs a supervisor on old terminals; host-terminals/migration.spec.ts covers new ones");
  const session = await createSession(request, { title: `migration-off-${Date.now()}` });
  try {
    const tab = await openTab(request, session.id);
    expect(tab.kind, "the tab-open reply names the tab's kind").toBe("tmux");
    const row = await listedRow(request, session.id);
    expect(row.agent_terminal).toBe("tmux");
    expect(row.supervisor_creates_host_terminals ?? false).toBe(false);
    expect(row.tabs.map((listed) => listed.kind)).toEqual(["tmux"]);

    await page.goto("/");
    await attachSession(page, session.id);
    await expect(page.locator(".session-details")).toBeVisible();
    await expect(band(page)).toHaveCount(0);
    await expect(kinds(page)).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** The band's first case: an old agent terminal on a host that creates new
 * ones is told to restart when convenient, or, when Restart is unavailable,
 * to clone or replace; the details name the kind; × hides the band for the
 * rest of the visit, a refresh included, and the next visit brings it back.
 * Why: the band is the only reminder an old session gets, so it must be
 * easy to put aside and must not stay away. */
test("an old session's band hides until the next visit and offers only what the session can do", async ({ page, request }) => {
  const session = await createSession(request, { title: `migration-old-${Date.now()}` });
  const neighbor = await createSession(request, { title: `migration-neighbor-${Date.now()}` });
  try {
    let seed: TerminalSeed = {
      supervisor_creates_host_terminals: true,
      agent_terminal: "tmux",
      restart_offer: "resume",
    };
    await seedTerminals(page, session.id, () => seed);
    await page.goto("/");
    await attachSession(page, session.id);

    await expect(band(page)).toBeVisible();
    await expect(band(page)).toHaveAttribute("role", "status");
    await expect(band(page)).toHaveAttribute("data-migration", "agent");
    await expect(band(page)).toContainText("This session still runs in an old terminal.");
    await expect(band(page)).toContainText("restart this session");
    await expect(band(page)).toContainText("when convenient");
    const link = band(page).getByRole("link", { name: "What's changing ↗" });
    await expect(link).toHaveAttribute("href", "https://farhelm.io/docs/using/new-terminals/");
    // The band sits between the top row and the terminal, taking space.
    const bandBox = (await band(page).boundingBox())!;
    const panes = (await page.locator(".terminal-panes").boundingBox())!;
    expect(panes.y).toBeGreaterThanOrEqual(bandBox.y + bandBox.height - 1);
    await expect(kinds(page)).toHaveText(/Old terminal/);
    await expect(kinds(page)).toHaveAttribute("data-tooltip", "Agent: old terminal (tmux).");

    await band(page).getByRole("button", { name: "Hide this note until you next open this session" }).click();
    await expect(band(page)).toHaveCount(0);
    // A detail refresh of the same session keeps it dismissed: rename it
    // from another client and wait for the view to show the new title.
    await renameSession(request, session.id, `migration-renamed-${Date.now()}`);
    await expect(page.locator(".session-details .title")).toContainText("migration-renamed-");
    await expect(band(page)).toHaveCount(0);
    await expect(kinds(page)).toHaveText(/Old terminal/);

    // Restart becomes unavailable meanwhile; the next visit shows the band
    // again, offering Clone and Replace only.
    seed = { ...seed, restart_offer: "no_conversation_reporting" };
    await attachSession(page, neighbor.id);
    await expect(band(page)).toHaveCount(0);
    await attachSession(page, session.id);
    await expect(band(page)).toBeVisible();
    await expect(band(page)).not.toContainText("restart this session");
    await expect(band(page)).toContainText("clone or replace it when convenient");
    await expect(band(page)).toContainText("fresh conversation");
    await expect(page.locator(".restart-primary")).toHaveAttribute("aria-disabled", "true");

    // A reload is a visit too.
    await band(page).getByRole("button", { name: "Hide this note until you next open this session" }).click();
    await expect(band(page)).toHaveCount(0);
    await page.reload();
    await attachSession(page, session.id);
    await expect(band(page)).toBeVisible();
  } finally {
    await cleanupSession(request, neighbor.id);
    await cleanupSession(request, session.id);
  }
});

/** The band's second case: the agent already runs in a new terminal but a
 * tab is old, so the band names that tab and says restarting the agent will
 * not move it; the details say the session has both kinds. Why: Restart is
 * the obvious move and here it does nothing for the tabs, so the band must
 * say what does. */
test("a new agent with an old tab is told to reopen that tab", async ({ page, request }) => {
  const session = await createSession(request, { title: `migration-tabs-${Date.now()}` });
  try {
    await openTab(request, session.id);
    await openTab(request, session.id);
    await seedTerminals(page, session.id, () => ({
      supervisor_creates_host_terminals: true,
      agent_terminal: "host",
      tab_kinds: ["host", "tmux"],
    }));
    await page.goto("/");
    await attachSession(page, session.id);

    await expect(band(page)).toBeVisible();
    await expect(band(page)).toHaveAttribute("data-migration", "tabs");
    await expect(band(page)).toContainText("Terminal 2 is still an old terminal.");
    await expect(band(page)).toContainText("restarting it does not move");
    await expect(band(page)).toContainText("close it and open a new tab.");
    await expect(kinds(page)).toHaveText(/Old and new terminals/);
    await expect(kinds(page)).toHaveAttribute(
      "data-tooltip",
      "Agent: new terminal. Terminal 1: new terminal. Terminal 2: old terminal (tmux).",
    );
  } finally {
    await cleanupSession(request, session.id);
  }
});
