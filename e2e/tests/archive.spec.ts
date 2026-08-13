// Archive's browser contract against the real authenticated stack: both
// entry points, the default-off list switch, the terminal-less retained
// view, and restart as the route back.

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
    const include = page.locator(".filter-include-archived");
    await include.check();
    await page.locator(".filter-apply").click();
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
    await page.locator(".filter-apply").click();

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
    await expect(page.locator(".session-rename")).toBeDisabled();
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
    await page.locator(".archive-cancel").click();

    expect(archiveRequests).toBe(0);
    await expect(page.locator(".archive-primary")).toBeVisible();
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

    await expect(page.locator(".back-button")).toBeDisabled();
    await expect(page.locator(".session-rename")).toBeDisabled();
    await expect(page.locator(".restart-primary")).toBeDisabled();
    await page.locator(".back-button").dispatchEvent("click");
    await page.locator(".session-rename").dispatchEvent("click");
    await page.locator(".restart-primary").dispatchEvent("click");
    expect(competing).toEqual([]);
    await expect(page.locator(".rename-form")).toHaveCount(0);
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
