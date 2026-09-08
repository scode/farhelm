// The sidebar's one host selector: registry identity on screen, server-side
// narrowing on the wire, and recovery when that identity leaves the registry.
import { Page, Route } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession,
  countReads,
  createSession,
  forceBuildSkew,
  listHosts,
  localHostId,
  openRowMenu,
  stubFeed,
} from "./helpers/fleet";

/** The row for one session id, as the list renders it. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/** Load a settled sidebar while keeping unrelated fleet revisions quiet. */
async function settledSidebar(page: Page) {
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);
  await expect(page.locator(".host-row").first()).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".hosts-status", { hasText: "loading hosts" })).toHaveCount(0, {
    timeout: 20_000,
  });
  await expect(page.locator(".host-select")).toBeVisible();
  return feed;
}

test.describe("session host selector", () => {
  const created: string[] = [];

  test.afterEach(async ({ request }) => {
    while (created.length) {
      const id = created.pop();
      if (id) await cleanupSession(request, id);
    }
  });

  /**
   * Every configured machine remains selectable regardless of connection
   * state, and locality comes from the registry kind rather than a name.
   */
  test("offers ALL, the registry local machine, and every configured remote", async ({
    page,
    request,
  }) => {
    const hosts = await listHosts(request);
    const local = await localHostId(request);
    const remotes = hosts.filter((host) => host.id !== local);
    expect(remotes.length, "the e2e fleet must include a configured remote").toBeGreaterThan(0);
    const disconnected = remotes[0];
    await page.route(
      (url) => url.pathname === "/api/hosts",
      async (route: Route) => {
        const response = await route.fetch();
        const body = await response.json();
        const host = body.hosts.find((candidate: { id: number }) => candidate.id === disconnected.id);
        host.state = {
          phase: "unreachable-reprobing",
          cause: "transport-failure",
          last_error: "host-selector-disconnected-sentinel",
        };
        await route.fulfill({ response, json: body });
      },
    );

    await settledSidebar(page);
    const selector = page.locator(".host-select");
    await expect(selector).toHaveValue("");
    await expect(selector.locator('option[value=""]')).toHaveText("ALL");
    await expect(selector.locator(`option[value="${local}"]`)).toHaveText("This machine");
    for (const remote of remotes) {
      await expect(
        selector.locator(`option[value="${remote.id}"]`),
        `configured host ${remote.id} must remain available even when disconnected`,
      ).toHaveCount(1);
    }
    await expect(selector.locator(`option[value="${disconnected.id}"]`)).toHaveText(
      disconnected.name,
    );
    await expect(
      page.locator(`.host-row[data-host-id="${disconnected.id}"] .host-status-label`),
      "connection state remains visible in the host panel without destabilizing the selector label",
    ).toHaveText("unreachable, retrying");
    await expect(page.locator(".filter-toggle, .filter-popover")).toHaveCount(0);
  });

  /**
   * A change is a server query, not a client-side pass over the current rows.
   * The open terminal remains selected while its row is outside the choice.
   */
  test("applies immediately while preserving the selected session", async ({ page, request }) => {
    const session = await createSession(request, { title: `host-choice-${Date.now()}` });
    created.push(session.id);
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the e2e fleet must include a configured remote").toBeTruthy();
    const reads = countReads(page);
    const feed = await settledSidebar(page);
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await row(page, session.id).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toContainText("host-choice-");

    // Native keyboard operation chooses the local host and immediately sends
    // the same server query as a pointer selection. The fixture session makes
    // the matching count nonzero, which distinguishes a real host-scoped
    // answer from an empty response that happened to use the right URL.
    const selector = page.locator(".host-select");
    const localBefore = reads.count("listing");
    await selector.focus();
    await expect(selector).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(selector).toHaveValue(String(local));
    await expect.poll(() => reads.count("listing"), { timeout: 20_000 }).toBeGreaterThan(localBefore);
    await expect(row(page, session.id)).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".session-count")).toHaveText(/^[1-9]\d* matching of \d+ sessions$/);
    const localCount = /^([1-9]\d*) matching of (\d+) sessions$/.exec(
      (await page.locator(".session-count").textContent())?.trim() ?? "",
    );
    expect(localCount, "local selection must report a nonzero server-side match count").toBeTruthy();
    expect(Number(localCount![2])).toBeGreaterThanOrEqual(Number(localCount![1]));

    // Sorting and host scope are independent: a sort change must keep the
    // selected source, and returning to ALL below must keep the chosen order.
    await page.locator(".sort-select").selectOption("title");
    await expect.poll(() => reads.urls("listing").at(-1)).toContain("sort=title");
    expect(new URL(reads.urls("listing").at(-1)!).searchParams.get("host")).toBe(String(local));
    await expect(selector).toHaveValue(String(local));

    // The transient row menu must not resurrect after its row leaves the
    // selected host. An unsent rename draft, in contrast, remains owned by
    // the session: host exclusion says nothing about whether it still exists.
    await openRowMenu(row(page, session.id));
    await row(page, session.id).locator(".session-row-rename").click();
    const draft = `${session.title}-unsent`;
    await row(page, session.id).locator(".rename-input").fill(draft);

    const before = reads.count("listing");
    await selector.selectOption(String(remote!.id));
    await expect.poll(() => reads.count("listing"), { timeout: 20_000 }).toBeGreaterThan(before);
    const asked = reads.urls("listing").slice(before);
    expect(
      asked.some((url) => new URL(url).searchParams.get("host") === String(remote!.id)),
      `the selected registry id must reach the helm; saw ${asked.join(", ")}`,
    ).toBe(true);
    await expect(row(page, session.id)).toHaveCount(0, { timeout: 20_000 });
    await expect(page.locator(".host-selection-empty")).toBeVisible();
    await expect(page.locator(".session-count")).toHaveText(/^0 matching of \d+ sessions$/);
    await expect(page.locator(".titlebar .title")).toContainText("host-choice-");

    await selector.selectOption("");
    await expect(row(page, session.id)).toBeVisible();
    await expect(row(page, session.id).locator(".session-row-menu-panel")).toHaveCount(0);
    await expect(page.locator(".sort-select")).toHaveValue("title");
    await openRowMenu(row(page, session.id));
    await expect(row(page, session.id).locator(".rename-input")).toHaveValue(draft);
    await selector.selectOption(String(remote!.id));
    await expect(row(page, session.id)).toHaveCount(0);

    // Host scope is intentionally page-local: feed refreshes keep the current
    // choice, while a full navigation starts from ALL rather than reviving a
    // stale machine choice from browser storage.
    feed.notify(2);
    await expect(selector).toHaveValue(String(remote!.id));
    await page.reload();
    await feed.waitForConnection(2);
    feed.notify(3);
    await expect(page.locator(".host-select")).toHaveValue("", { timeout: 20_000 });
  });

  /**
   * A registry refresh in flight or failed cannot prove that a configured
   * host disappeared. The retained snapshot keeps both the option and the
   * user's concrete selection until a successful reply says otherwise.
   */
  test("retains the selected host through a pending and failed registry refresh", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the e2e fleet must include a configured remote").toBeTruthy();
    const hostProbe = await request.get("/api/hosts");
    const helmBuild = hostProbe.headers()["x-farhelm-build"];
    expect(helmBuild, "fabricated failures need the live helm compatibility stamp").toBeTruthy();

    const feed = await settledSidebar(page);
    const selector = page.locator(".host-select");
    await selector.selectOption(String(remote!.id));
    await expect(selector).toHaveValue(String(remote!.id));

    let entered!: () => void;
    const requestEntered = new Promise<void>((resolve) => {
      entered = resolve;
    });
    let release: () => void = () => {};
    const released = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => url.pathname === "/api/hosts",
      async (route: Route) => {
        entered();
        await released;
        await route.fulfill({
          status: 500,
          body: "host-selector-refresh-failure",
          headers: {
            "content-type": "text/plain",
            "x-farhelm-build": helmBuild,
          },
        });
      },
    );

    try {
      feed.notify(2);
      await requestEntered;
      await expect(selector).toHaveValue(String(remote!.id));
      await expect(selector.locator(`option[value="${remote!.id}"]`)).toHaveCount(1);

      release();
      await expect(page.locator(".hosts-refresh-error")).toContainText(
        "host-selector-refresh-failure",
        { timeout: 20_000 },
      );
      await expect(selector).toHaveValue(String(remote!.id));
      await expect(selector.locator(`option[value="${remote!.id}"]`)).toHaveCount(1);
    } finally {
      release();
    }
  });

  /**
   * Only a successful registry reply can prove removal. Once it does, the
   * selector returns to ALL and the follow-up listing drops the host query.
   */
  test("resets a removed host to ALL after an authoritative registry read", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the e2e fleet must include a configured remote").toBeTruthy();
    const removedId = remote!.id;

    let removed = false;
    await page.route(
      (url) => url.pathname === "/api/hosts",
      async (route: Route) => {
        if (route.request().method() !== "GET") {
          await route.continue();
          return;
        }
        const response = await route.fetch();
        const body = await response.json();
        if (removed) {
          body.hosts = body.hosts.filter((host: { id: number }) => host.id !== removedId);
        }
        await route.fulfill({ response, json: body });
      },
    );

    const reads = countReads(page);
    const feed = await settledSidebar(page);
    await page.locator(".host-select").selectOption(String(removedId));
    await expect(page.locator(".host-select")).toHaveValue(String(removedId));

    removed = true;
    const before = reads.count("listing");
    feed.notify(2);
    await expect(page.locator(".host-select")).toHaveValue("", { timeout: 20_000 });
    // A scoped read may already be in flight when the registry reply resets
    // the choice. Wait for the coalesced ALL request, not merely any read.
    await expect.poll(
      () => reads.urls("listing").slice(before).some((url) => !new URL(url).searchParams.has("host")),
      { timeout: 20_000, message: "resetting to ALL must issue an unscoped listing" },
    ).toBe(true);
  });

  /**
   * A version mismatch withdraws background activity, but an explicit host
   * choice still asks the helm. Otherwise the selector would appear usable
   * while silently showing the old scope until the client was upgraded.
   */
  test("a host choice remains an explicit read under build skew", async ({ page, request }) => {
    const local = await localHostId(request);
    const session = await createSession(request, { title: `skew-host-${Date.now()}` });
    created.push(session.id);
    await forceBuildSkew(page, "9.9.9-host-selector-skew");
    const reads = countReads(page);
    await page.goto("/");
    await expect(page.locator(`.host-select option[value="${local}"]`)).toHaveCount(1);
    await expect(page.locator(".build-skew")).toBeVisible();
    await expect(row(page, session.id)).toBeVisible();
    const before = reads.count("listing");
    await page.locator(".host-select").selectOption(String(local));
    await expect.poll(() => reads.count("listing")).toBeGreaterThan(before);
    expect(reads.urls("listing").slice(before).every(
      (url) => new URL(url).searchParams.get("host") === String(local),
    )).toBe(true);
    await expect(page.locator(".session-count")).toHaveText(/^[1-9]\d* matching of \d+ sessions$/);
    await expect(row(page, session.id)).toBeVisible();
    await expect(page.locator(".build-skew")).toBeVisible();
  });
});
