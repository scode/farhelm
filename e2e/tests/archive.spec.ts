// Archive's browser contract against the real authenticated stack: both
// entry points, the default-off list switch, the count banner's
// denominator, the terminal-less retained view, and restart as the route
// back.

import { expect, Page, test } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  openFilterBar,
  openRowMenu,
} from "./helpers/fleet";

/** Find one session by its opaque server id, independent of title changes. */
function row(page: Page, id: string) {
  return page.locator(`.session-row[data-session-id="${id}"]`);
}

let helmBuild = "";

test.beforeAll(async ({ request }) => {
  const probe = await request.get("/api/sessions");
  helmBuild = probe.headers()["x-farhelm-build"] ?? "";
  expect(helmBuild).not.toBe("");
});

/** Fulfil an intercepted response with the compatibility stamp a helm owns. */
async function refuseArchive(route: { fulfill: (options: object) => Promise<void> }, body: string) {
  await route.fulfill({
    status: 500,
    body,
    headers: {
      "content-type": "text/plain",
      "x-farhelm-build": helmBuild,
    },
  });
}

test("the row confirmation names every live thing and the toggle reveals the archive", async ({
  page,
  request,
}) => {
  const session = await createSession(request, { title: `archive-row-${Date.now()}` });
  try {
    const tab = await request.post(`/api/sessions/${session.id}/tabs`);
    expect(tab.ok(), await tab.text()).toBeTruthy();

    await page.goto("/");
    await openFilterBar(page);
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await openRowMenu(target);
    await target.locator(".session-row-archive").click();
    await expect(target.locator(".confirm-consequence")).toContainText("agent");
    await expect(target.locator(".confirm-consequence")).toContainText("whole process tree");
    await expect(target.locator(".confirm-consequence")).toContainText("1 terminal tab");
    await expect(target.locator(".confirm-consequence")).toContainText("removes the terminal");
    await target.locator(".confirm-archive").click();

    await expect(target).toHaveCount(0, { timeout: 20_000 });
    // Focus left the popover for the row above (a row menu, a confirm), which
    // closes it; reopen before touching its controls again.
    await openFilterBar(page);
    const include = page.locator(".filter-include-archived");
    await include.check();
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, session.id)).toHaveAttribute("data-session-archived", "true");
    await expect(row(page, session.id).locator(".archived-badge")).toHaveText("archived");
    await expect(row(page, session.id).locator(".session-row-open")).toBeEnabled();
    // The lifecycle controls live in the actions menu now; opening it is
    // what makes "stop and archive are gone, rename and delete remain"
    // observable rather than vacuously true of a closed panel.
    await openRowMenu(row(page, session.id));
    await expect(row(page, session.id).locator(".session-row-stop")).toHaveCount(0);
    await expect(row(page, session.id).locator(".session-row-archive")).toHaveCount(0);
    await expect(row(page, session.id).locator(".session-row-rename")).toBeEnabled();
    await expect(row(page, session.id).locator(".session-row-delete")).toBeEnabled();
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * Read the count banner, insisting on the UNFILTERED wording, and return its
 * one number.
 *
 * The shape is half the assertion: the ordinary list and the archive switch
 * are both views rather than filters, so the banner must never reach for "N
 * matching of M sessions" in either. The absolute number belongs to a shared
 * stack, so every test below compares it against itself.
 */
async function unfilteredCount(page: Page): Promise<number> {
  const banner = page.locator(".session-count");
  await expect(banner).toBeVisible({ timeout: 20_000 });
  const text = (await banner.textContent())?.trim() ?? "";
  const match = /^(\d+) sessions$/.exec(text);
  expect(
    match,
    `an unapplied filter must produce the plain count sentence; saw ${JSON.stringify(text)}`,
  ).toBeTruthy();
  return Number(match![1]);
}

/**
 * The count banner's denominator is the view's own size: archiving takes a
 * session out of it, and the inclusion switch puts it back.
 *
 * This is the browser half of the 2026-08-22 verdict, and the reason it
 * needs a browser at all is that the number reaching the user is the end of
 * a chain no unit test spans — the helm counts a column, the walk carries
 * the count, and the banner picks a sentence for it. The bug it rules out
 * was visible on a fresh install: the default list hid archived sessions
 * while its count included them, so ten rows sat under "10 matching of 12
 * sessions" with nothing typed into any filter.
 *
 * The row count is asserted alongside the number because that is the whole
 * claim — a denominator nobody can see is not the thing that was wrong.
 *
 * The count wording and Clear button are pinned in the same test because
 * they read the switch differently ON PURPOSE, and only a rendered page
 * shows both answers at once: the switch keeps ordinary count wording (it
 * chose a view, it narrowed nothing) while still being something Clear can undo.
 * Collapsing the two is the natural-looking mistake in either direction.
 */
test("the count banner counts the view the archive switch selects", async ({ page, request }) => {
  const session = await createSession(request, { title: `archive-count-${Date.now()}` });
  try {
    await page.goto("/");
    await openFilterBar(page);
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    await expect(
      page.locator(".filter-clear"),
      "and nothing for Clear to undo either",
    ).toBeDisabled();

    const before = await unfilteredCount(page);
    await expect(
      page.locator(".session-row"),
      "a complete walk shows every session its own count claims",
    ).toHaveCount(before);

    await openRowMenu(target);
    await target.locator(".session-row-archive").click();
    await target.locator(".confirm-archive").click();
    await expect(target).toHaveCount(0, { timeout: 20_000 });

    await expect
      .poll(() => unfilteredCount(page), {
        timeout: 20_000,
        message: "an archived session leaves the default view's count with its row",
      })
      .toBe(before - 1);
    await expect(page.locator(".session-row")).toHaveCount(before - 1);

    // The switch widens the view, so it widens the count with it — and the
    // wording stays unfiltered, because turning it on is not a narrowing
    // anybody applied.
    // Focus left the popover for the row above (a row menu, a confirm), which
    // closes it; reopen before touching its controls again.
    await openFilterBar(page);
    await page.locator(".filter-include-archived").check();
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await expect.poll(() => unfilteredCount(page), { timeout: 20_000 }).toBe(before);
    await expect(page.locator(".session-row")).toHaveCount(before);

    // The switch alone keeps the unfiltered count wording, but Clear is
    // live because the switch is still a setting the user turned on.
    await expect(
      page.locator(".filter-clear"),
      "while Clear is the only way back to the default view",
    ).toBeEnabled();
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("cancelling a row archive restores every competing control without a request", async ({
  page,
  request,
}) => {
  const session = await createSession(request, { title: `archive-row-cancel-${Date.now()}` });
  let archiveRequests = 0;
  page.on("request", (outgoing) => {
    if (
      outgoing.method() === "POST" &&
      new URL(outgoing.url()).pathname === `/api/sessions/${session.id}/archive`
    ) {
      archiveRequests++;
    }
  });
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await openRowMenu(target);
    await target.locator(".session-row-archive").click();

    await expect(target.locator(".confirm-archive")).toBeVisible();
    // The confirm replaces the menu's items inside the panel; the open
    // button stays visible (the panel floats over the list instead of
    // competing for the row's space) but is inert until the prompt is
    // answered — cancel is the only way back to normal.
    await expect(target.locator(".session-row-open")).toBeDisabled();
    await expect(target.locator(".session-row-stop")).toHaveCount(0);
    await expect(target.locator(".session-row-archive")).toHaveCount(0);
    await expect(target.locator(".session-row-rename")).toHaveCount(0);
    await expect(target.locator(".session-row-delete")).toHaveCount(0);

    await target.locator(".archive-cancel").click();
    expect(archiveRequests).toBe(0);
    await expect(target.locator(".session-row-open")).toBeVisible();
    await expect(target.locator(".session-row-open")).toBeEnabled();
    await expect(target.locator(".session-row-stop")).toBeEnabled();
    await expect(target.locator(".session-row-archive")).toBeEnabled();
    await expect(target.locator(".session-row-rename")).toBeEnabled();
    await expect(target.locator(".session-row-delete")).toBeEnabled();
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("an included row stays visible while its archive state changes", async ({ page, request }) => {
  const session = await createSession(request, { title: `archive-retained-${Date.now()}` });
  try {
    await page.goto("/");
    await openFilterBar(page);
    const include = page.locator(".filter-include-archived");
    await include.check();

    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await openRowMenu(target);
    await target.locator(".session-row-archive").click();
    await target.locator(".confirm-archive").click();

    await expect(target).toHaveCount(1);
    await expect(target).toHaveAttribute("data-session-archived", "true", { timeout: 20_000 });
    await expect(target.locator(".archived-badge")).toHaveText("archived");
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("the detail action leaves metadata without a terminal and restart restores one", async ({
  page,
  request,
}) => {
  const title = `archive-view-${Date.now()}`;
  const session = await createSession(request, { title });
  try {
    await page.goto("/");
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toHaveText(title);

    await page.locator(".archive-primary").click();
    await expect(page.locator(".archive-offer .confirm-consequence")).toContainText("agent");
    await expect(page.locator(".restart-primary")).toBeDisabled();
    await expect(page.locator(".tab-add")).toBeDisabled();
    await page.locator(".archive-confirm").click();

    const notice = page.locator(".archived-notice");
    await expect(notice).toContainText("metadata and attachments remain", { timeout: 20_000 });
    await expect(notice).toContainText("agent, tabs, and terminal were removed");
    await expect(page.locator(".titlebar .title")).toHaveText(title);
    await expect(page.locator(".titlebar .meta")).toContainText("/tmp");
    await expect(page.locator(".terminal")).toHaveCount(0);

    const restart = page.locator(".restart-primary");
    await expect(restart).toBeVisible();
    await expect(restart).toHaveAttribute("data-confirms", "false");
    await restart.click();
    await expect(notice).toHaveCount(0, { timeout: 20_000 });
    await expect(page.locator("#terminal")).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".tab-strip .tab-agent")).toHaveCount(1);
    await expect(page.locator(".tab-strip .tab:not(.tab-agent)")).toHaveCount(0);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a refused row archive keeps the row and restores its controls", async ({ page, request }) => {
  const session = await createSession(request, { title: `archive-row-refusal-${Date.now()}` });
  try {
    await page.route(`**/api/sessions/${session.id}/archive`, (route) =>
      refuseArchive(route, "archive-row-refusal-sentinel"),
    );
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await openRowMenu(target);
    await target.locator(".session-row-archive").click();
    await target.locator(".confirm-archive").click();
    await expect(target.locator(".action-error")).toContainText("archive-row-refusal-sentinel");
    await expect(target).toHaveCount(1);
    await expect(target.locator(".session-row-archive")).toBeEnabled();
    await expect(target.locator(".session-row-open")).toBeEnabled();
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a refused detail archive leaves the terminal usable", async ({ page, request }) => {
  const session = await createSession(request, { title: `archive-view-refusal-${Date.now()}` });
  try {
    await page.route(`**/api/sessions/${session.id}/archive`, (route) =>
      refuseArchive(route, "archive-detail-refusal-sentinel"),
    );
    await page.goto("/");
    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator("#terminal")).toBeVisible({ timeout: 20_000 });
    await page.locator(".archive-primary").click();
    await page.locator(".archive-confirm").click();
    await expect(page.locator(".archive-error")).toContainText(
      "archive-detail-refusal-sentinel",
    );
    await expect(page.locator("#terminal")).toBeVisible();
    await expect(page.locator(".tab-add")).toBeEnabled();
    await page.locator(".tab-add").click();
    await expect(page.locator(".tab-strip .tab:not(.tab-agent)")).toHaveCount(1, {
      timeout: 20_000,
    });
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("cancelling a detail archive sends no request and leaves the terminal mounted", async ({
  page,
  request,
}) => {
  const session = await createSession(request, { title: `archive-view-cancel-${Date.now()}` });
  let archiveRequests = 0;
  page.on("request", (outgoing) => {
    if (
      outgoing.method() === "POST" &&
      new URL(outgoing.url()).pathname === `/api/sessions/${session.id}/archive`
    ) {
      archiveRequests++;
    }
  });
  try {
    await page.goto("/");
    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator("#terminal")).toBeVisible({ timeout: 20_000 });

    await page.locator(".archive-primary").click();
    await expect(page.locator(".archive-confirm")).toBeVisible();
    // The prompt is a popover anchored to the trigger since the header
    // consolidation, so the trigger does NOT vanish behind it — it stays on
    // screen. `aria-expanded` records that SOMETHING owned by this button
    // is open (it says nothing about which panel), so `aria-controls` is
    // asserted alongside it — that is the attribute that actually ties this
    // one panel to the button that opened it, explicit rather than implied
    // by DOM proximity.
    await expect(page.locator(".archive-primary")).toHaveAttribute("aria-expanded", "true");
    await expect(page.locator(".archive-primary")).toHaveAttribute(
      "aria-controls",
      "archive-confirm-panel",
    );
    await expect(page.locator("#archive-confirm-panel")).toBeVisible();
    await page.locator(".archive-cancel").click();

    expect(archiveRequests).toBe(0);
    await expect(page.locator(".archive-primary")).toBeVisible();
    await expect(page.locator(".archive-primary")).toHaveAttribute("aria-expanded", "false");
    await expect(page.locator("#terminal")).toBeVisible();
    await expect(page.locator(".tab-strip .tab-agent")).toHaveCount(1);
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("a pending detail archive blocks navigation and competing mutations", async ({
  page,
  request,
}) => {
  const session = await createSession(request, { title: `archive-view-pending-${Date.now()}` });
  let releaseArchive!: () => void;
  const archiveRelease = new Promise<void>((resolve) => {
    releaseArchive = resolve;
  });
  let archiveEntered!: () => void;
  const archiveRequest = new Promise<void>((resolve) => {
    archiveEntered = resolve;
  });
  const competing: string[] = [];
  page.on("request", (outgoing) => {
    const url = new URL(outgoing.url()).pathname;
    if (
      outgoing.method() === "POST" &&
      (url === `/api/sessions/${session.id}/restart` ||
        url === `/api/sessions/${session.id}/rename`)
    ) {
      competing.push(url);
    }
  });
  await page.route(`**/api/sessions/${session.id}/archive`, async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    archiveEntered();
    await archiveRelease;
    await route.continue();
  });
  try {
    await page.goto("/");
    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator("#terminal")).toBeVisible({ timeout: 20_000 });

    await page.locator(".archive-primary").click();
    await page.locator(".archive-confirm").click();
    await archiveRequest;

    // Navigation is the sidebar now (back button and titlebar rename are
    // gone with the redesign): while the archive is pending, every row's
    // open control is nav-locked, and a dispatched click on a DIFFERENT
    // row — the shared session's, which is provably not the one this
    // archive selected — must not swap the view away from the session
    // whose archive owns the gate. (A click on the selected row's own
    // button would be a no-op even with the guard deleted.)
    const sharedOpen = page
      .locator(".session-row")
      .filter({ has: page.locator(".session-title", { hasText: /^e2e-session$/ }) })
      .locator(".session-row-open");
    await expect(sharedOpen).toBeDisabled();
    await expect(page.locator(".restart-primary")).toBeDisabled();
    await sharedOpen.dispatchEvent("click");
    await page.locator(".restart-primary").dispatchEvent("click");
    // The competing RENAME is attempted for real, not merely asserted
    // absent: the row menu opens (the toggle is deliberately not
    // nav-locked) but its rename control is disabled and a dispatched
    // click on it must produce neither a form nor a request while the
    // archive holds the shared gate.
    const ownRow = page.locator(`[data-session-id="${session.id}"]`);
    await openRowMenu(ownRow);
    await expect(ownRow.locator(".session-row-rename")).toBeDisabled();
    await ownRow.locator(".session-row-rename").dispatchEvent("click");
    expect(competing).toEqual([]);
    await expect(page.locator(".rename-form")).toHaveCount(0);
    await expect(page.locator(".titlebar .title")).toHaveText(session.title);
    await expect(page.locator("#terminal")).toBeVisible();

    releaseArchive();
    await expect(page.locator(".archived-notice")).toBeVisible({ timeout: 20_000 });
  } finally {
    releaseArchive();
    await cleanupSession(request, session.id);
  }
});

test("archive stays unavailable while a terminal tab is opening", async ({ page, request }) => {
  const session = await createSession(request, { title: `archive-tab-race-${Date.now()}` });
  try {
    await page.route(`**/api/sessions/${session.id}/tabs`, async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, 2_000));
      await route.continue();
    });
    await page.goto("/");
    await row(page, session.id).locator(".session-row-open").click();

    await page.locator(".tab-add").click();
    await expect(page.locator(".archive-primary")).toBeDisabled();
    await expect(page.locator(".tab-strip .tab:not(.tab-agent)")).toHaveCount(1, {
      timeout: 20_000,
    });
    await expect(page.locator(".archive-primary")).toBeEnabled();
  } finally {
    await cleanupSession(request, session.id);
  }
});

test("an external archive invalidates an open detail confirmation", async ({ page, request }) => {
  const session = await createSession(request, { title: `archive-external-${Date.now()}` });
  try {
    await page.goto("/");
    await row(page, session.id).locator(".session-row-open").click();
    await page.locator(".archive-primary").click();
    await expect(page.locator(".archive-confirm")).toBeVisible();

    const archived = await request.post(`/api/sessions/${session.id}/archive`);
    expect(archived.ok(), await archived.text()).toBeTruthy();
    await expect(page.locator(".archive-confirm")).toHaveCount(0, { timeout: 20_000 });
    await expect(page.locator(".archived-notice")).toBeVisible();
    await expect(page.locator(".restart-primary")).toBeEnabled();
  } finally {
    await cleanupSession(request, session.id);
  }
});

/**
 * A rename in progress survives another client archiving the session, and
 * is still there when the archive switch brings the row back.
 *
 * This is the reconciliation half of the 2026-08-22 split, end to end, and
 * it is the reason `omits_fleet_members` exists as a second flag rather
 * than a second reading of `filtered`. The default view now reads as
 * UNFILTERED — the banner says "N sessions" — while still withholding every
 * archived row, so a listing poll that treated its own absences as
 * departures would retire an answer the user is in the middle of giving.
 * Collapse the two predicates and this test fails by the editor closing
 * under the user's hands the moment somebody else archives the session.
 *
 * The row itself does leave the sidebar, and must: the default view is not
 * showing archived sessions, and the actions menu is a lens that closes
 * with its row. What has to survive is the STATE — the open editor and its
 * unsent draft — which is what reopening the menu after the switch is
 * flipped puts back on screen.
 */
test("an external archive does not close an open rename editor", async ({ page, request }) => {
  const session = await createSession(request, { title: `archive-rename-${Date.now()}` });
  const draft = `${session.title}-unsent`;
  try {
    await page.goto("/");
    await openFilterBar(page);
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    await openRowMenu(target);
    await target.locator(".session-row-rename").click();
    await expect(target.locator(".rename-form")).toBeVisible();
    // Typed and deliberately NOT submitted: an answer in progress is
    // exactly what a poll must not be allowed to throw away.
    await target.locator(".rename-input").fill(draft);

    const archived = await request.post(`/api/sessions/${session.id}/archive`);
    expect(archived.ok(), await archived.text()).toBeTruthy();
    await expect(
      target,
      "the default view withholds archived rows, which is the premise of this test",
    ).toHaveCount(0, { timeout: 20_000 });
    // Several polls' worth, so this is "the reconciliation declined to run"
    // rather than "the first read had not landed yet".
    await page.waitForTimeout(6_000);

    // Focus left the popover for the row above (a row menu, a confirm), which
    // closes it; reopen before touching its controls again.
    await openFilterBar(page);
    await page.locator(".filter-include-archived").check();
    const restored = row(page, session.id);
    await expect(restored).toBeVisible({ timeout: 20_000 });

    await openRowMenu(restored);
    await expect(
      restored.locator(".rename-form"),
      "the rename was still in progress; only the row went away",
    ).toBeVisible();
    await expect(
      restored.locator(".rename-input"),
      "and the unsent draft went with it, character for character",
    ).toHaveValue(draft);
  } finally {
    await cleanupSession(request, session.id);
  }
});
