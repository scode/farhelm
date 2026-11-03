// The move to new terminals on a host whose supervisor creates them (this
// directory's playwright.config.ts starts it with `--new-terminals host`):
// the session details name the session's terminals as new, and no migration
// band shows, since nothing about the session is old.
//
// The band's own cases (an old agent, old tabs, Restart unavailable,
// dismissal) need an old terminal on such a host, which this stack cannot
// make; e2e/tests/terminal-migration.spec.ts drives them on the main stack
// with the session rows' terminal fields seeded, and the process test
// `host_processes::an_old_session_after_the_switch_to_hosts_is_reported_as_one_to_move`
// shows a real supervisor reporting those fields for such a session. This
// spec is the real-host half: every field comes from the supervisor as it is.
import type { APIRequestContext, Locator, Page } from "@playwright/test";
import { expect, test } from "../tests/helpers/evidence";

/** The startup session's row, matched on its whole title. */
function startupRow(page: Page): Locator {
  return page.locator(".session-row").filter({
    has: page.locator(".session-title", { hasText: /^e2e-session$/ }),
  });
}

/** The startup session as the helm lists it, read past the UI. */
async function startupSession(request: APIRequestContext): Promise<Record<string, any>> {
  const listed = await request.get("/api/sessions");
  expect(listed.ok()).toBe(true);
  const row = (await listed.json()).sessions.find((session: { title: string }) => session.title === "e2e-session");
  expect(row, "the startup session is listed").toBeTruthy();
  return row;
}

/**
 * A session created on a host that creates new terminals says so in its
 * details, for its agent and for a tab opened beside it, and shows no
 * migration band.
 *
 * Why: this is the only place the terminal kinds and the switch report are
 * checked end to end from a supervisor that really creates new terminals,
 * through the helm, into the page; and a session already on new terminals
 * must never be told to move. Spec: the helm lists the startup session with
 * `agent_terminal: host` and `supervisor_creates_host_terminals: true`; its
 * details read "New terminal" with the tooltip "Agent: new terminal."; once a
 * tab is opened (in a host too), they read "New terminals" and name
 * "Terminal 1: new terminal."; there is no `.migration-band` throughout.
 */
test("a session on new terminals names their kind and shows no band", async ({ page, request }) => {
  const session = await startupSession(request);
  expect(session.agent_terminal).toBe("host");
  expect(session.supervisor_creates_host_terminals).toBe(true);

  await page.goto("/");
  const row = startupRow(page);
  await expect(row).toBeVisible({ timeout: 20_000 });
  if ((await row.getAttribute("data-session-selected")) !== "true") {
    await row.locator(".session-row-open").click();
  }
  const details = page.locator(".session-details .session-details-terminals");
  await expect(details).toHaveText(/^New terminal/, { timeout: 30_000 });
  await expect(details).toHaveAttribute("data-tooltip", "Agent: new terminal.");
  await expect(page.locator(".migration-band")).toHaveCount(0);

  const opened = await request.post(`/api/sessions/${encodeURIComponent(session.id)}/tabs`);
  expect(opened.ok(), await opened.text()).toBe(true);
  const tab = (await opened.json()).tab;
  try {
    expect(tab.kind, "the tab-open reply names a host tab").toBe("host");
    await expect(details).toHaveText(/^New terminals/, { timeout: 30_000 });
    await expect(details).toHaveAttribute("data-tooltip", "Agent: new terminal. Terminal 1: new terminal.");
    await expect(page.locator(".migration-band")).toHaveCount(0);
  } finally {
    const closed = await request.delete(`/api/sessions/${encodeURIComponent(session.id)}/tabs/${tab.id}`);
    expect(closed.ok(), await closed.text()).toBe(true);
  }
});
