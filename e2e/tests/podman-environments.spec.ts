/** Fresh environments reuse the normal launcher and Settings surfaces. These
 * boundary fixtures refuse launch/delete deliberately: they establish placement,
 * consent and error recovery without pretending to create a dialable container. */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Page } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { drainedRoute } from "./helpers/drained-route";
import { routeGate } from "./helpers/route-gate";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { cleanupSession, createSession, openRowMenu, pinAutoSelect, stubFeed } from "./helpers/fleet";
import { answerYolo } from "./helpers/term";
import { openSettings } from "./helpers/settings";

/** Every displayed place is a real registry entry; only provider observations
 * are replaced. Request errors remain explicit rather than fake successes. */
async function environmentChoices(page: Page, request: APIRequestContext) {
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const hosts = (await response.json()).hosts;
  const local = hosts.find((host: any) => host.kind === "local");
  expect(local, "the owned stack has a registered local place").toBeTruthy();
  const headers = { "x-farhelm-build": build };
  const stops: Array<() => Promise<void>> = [];
  stops.push(
    await drainedRoute(page, `**/api/hosts/${local.id}/podman-probe`, async (route) => {
      expect(route.request().method()).toBe("POST");
      await route.fulfill({ status: 202, headers });
    }),
  );
  stops.push(
    await drainedRoute(page, "**/api/environment-places", async (route) =>
      route.fulfill({
        status: 200,
        headers,
        json: [{
          place: { kind: "podman_host", host: local.id },
          name: "Build machine",
          reason: null,
          refreshing: false,
        }],
      })),
  );
  stops.push(
    await drainedRoute(
      page,
      "**/api/environment-launch-history",
      async (route) => route.fulfill({ status: 200, headers, json: [] }),
    ),
  );
  stops.push(
    await drainedRoute(page, "**/api/recipe-places", async (route) =>
      route.fulfill({
        status: 200,
        headers,
        json: [{
          recipe: "container-dev",
          place: { kind: "podman_host", host: local.id },
          ready: true,
          building: false,
          reason: null,
          built_at: 1,
          prepared_at: 2,
          step: null,
          cleanup_pending: false,
        }],
      })),
  );
  stops.push(
    await drainedRoute(page, "**/api/tensorlake/recipes", async (route) =>
      route.fulfill({
        status: 200,
        headers,
        json: {
          recipes: [{
            name: "container-dev",
            size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
            base: { kind: "tensorlake_ubuntu" },
            packages: [],
            claude: null,
            codex: null,
            claude_login_credential: null,
            codex_login_credential: null,
            github_token_credential: null,
            tunnels: [],
            snapshot: null,
          }],
        },
      })),
  );
  return { local, headers, stops };
}

/** Active placement segments keep the explicit choices already shown.
 * Real local and remote hosts distinguish the ordinary default from a picked
 * host. Two built recipes and multiple places expose resets to provider defaults.
 * Provider availability, recipes and remembered defaults are controlled.
 * This case makes no launch request. */
test(
  "active placement segments preserve explicit host recipe and place",
  async ({ page, request }, testInfo) => {
    const fixture = await environmentChoices(page, request);
    const response = await request.get("/api/hosts");
    expect(response.ok()).toBe(true);
    const remote = (await response.json()).hosts.find((host: any) =>
      host.kind === "ssh" && host.state.phase === "connected"
    );
    expect(remote, "the owned stack has a connected ordinary remote host").toBeTruthy();
    expect(remote.id).not.toBe(fixture.local.id);
    const source = await createSession(request, {
      title: "active-placement-source",
      host: fixture.local.id,
    });
    const listing = await request.get("/api/sessions");
    expect(listing.ok()).toBe(true);
    const listedSource = (await listing.json()).sessions.find((row: any) => row.id === source.id);
    expect(listedSource, "the created source appears in the helm listing").toBeTruthy();
    expect(listedSource.host).toBe(fixture.local.id);
    const base = { kind: "tensorlake_ubuntu" };
    const recipes = ["container-dev", "selected-recipe"].map((name) => ({
      name,
      size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
      base,
      packages: [],
      claude: null,
      codex: null,
      claude_login_credential: null,
      codex_login_credential: null,
      github_token_credential: null,
      tunnels: [],
      snapshot: {
        id: "built-" + name,
        built_at: 1,
        base,
        disk_gb: 20,
        packages: [],
        claude_version: null,
        codex_version: null,
      },
    }));
    const places = [
      {
        place: { kind: "podman_host", host: fixture.local.id },
        name: "Default container host",
        reason: null,
        refreshing: false,
      },
      {
        place: { kind: "podman_host", host: remote.id },
        name: "Selected container host",
        reason: null,
        refreshing: false,
      },
      {
        place: { kind: "tensorlake_account" },
        name: "Tensorlake",
        reason: null,
        refreshing: false,
      },
    ];
    let creates = 0;
    fixture.stops.push(
      await drainedRoute(page, "**/api/hosts/" + remote.id + "/podman-probe", async (route) => {
        expect(route.request().method()).toBe("POST");
        await route.fulfill({ status: 202, headers: fixture.headers });
      }),
    );
    fixture.stops.push(
      await drainedRoute(
        page,
        "**/api/environment-places",
        async (route) => route.fulfill({ status: 200, headers: fixture.headers, json: places }),
      ),
    );
    fixture.stops.push(
      await drainedRoute(
        page,
        "**/api/tensorlake/recipes",
        async (route) => route.fulfill({ status: 200, headers: fixture.headers, json: { recipes } }),
      ),
    );
    fixture.stops.push(
      await drainedRoute(page, "**/api/recipe-places", async (route) =>
        route.fulfill({
          status: 200,
          headers: fixture.headers,
          json: recipes.flatMap((recipe) =>
            places.map((place) => ({
              recipe: recipe.name,
              place: place.place,
              ready: true,
              building: false,
              reason: null,
              built_at: 1,
              prepared_at: 2,
              step: null,
              cleanup_pending: false,
            }))
          ),
        })),
    );
    fixture.stops.push(
      await drainedRoute(page, "**/api/preferences", async (route) => {
        if (route.request().method() !== "GET") return route.fallback();
        const reply = await route.fetch();
        await route.fulfill({
          response: reply,
          json: { ...await reply.json(), last_container_host: fixture.local.id },
        });
      }),
    );
    fixture.stops.push(
      await drainedRoute(page, "**/api/tensorlake", async (route) => {
        const reply = await route.fetch();
        await route.fulfill({
          response: reply,
          json: {
            ...await reply.json(),
            api_key_set: true,
            ssh_key: {
              state: "ready",
              public_key: "ssh-ed25519 fixture",
              public_key_path: "fixture.pub",
              name: "fixture",
              register_command: "tl sbx ssh keys add --name fixture fixture.pub",
            },
          },
        });
      }),
    );
    for (const endpoint of ["sessions", "environment-sessions"]) {
      fixture.stops.push(
        await drainedRoute(page, "**/api/" + endpoint, async (route) => {
          if (route.request().method() !== "POST") return route.fallback();
          creates++;
          await route.fulfill({
            status: 409,
            headers: fixture.headers,
            body: "placement fixture does not launch",
          });
        }),
      );
    }
    try {
      await pinAutoSelect(page, source.id);
      await page.goto("/");
      await page.locator(".new-session-button").click();
      const form = page.locator(".create-session-form");
      const existing = form.getByRole("button", { name: "Existing host", exact: true });
      const host = form.getByRole("combobox", { name: "Host", exact: true });
      await expect(existing).toHaveAttribute("aria-pressed", "true");
      await expect(host).toHaveAttribute("data-host-id", String(fixture.local.id));
      await host.click();
      await form.locator('.launcher-host-option[data-host-id="' + remote.id + '"]').click();
      await expect(host).toHaveAttribute("data-host-id", String(remote.id));
      await existing.click();
      await expect(host).toHaveAttribute("data-host-id", String(remote.id));

      const container = form.getByRole("button", { name: "New container", exact: true });
      await container.click();
      const recipe = form.getByRole("combobox", { name: "Recipe", exact: true });
      const containerHost = form.getByRole("combobox", { name: "Container host", exact: true });
      await expect(recipe).toHaveValue("container-dev");
      await expect(containerHost).toHaveValue(String(fixture.local.id));
      await recipe.selectOption("selected-recipe");
      await containerHost.selectOption(String(remote.id));
      await expect(recipe).toHaveValue("selected-recipe");
      await expect(containerHost).toHaveValue(String(remote.id));
      await expect(container).toHaveAttribute("aria-pressed", "true");
      await container.click();
      await expect(recipe).toHaveValue("selected-recipe");
      await expect(containerHost).toHaveValue(String(remote.id));

      const sandbox = form.getByRole("button", { name: "New sandbox", exact: true });
      await sandbox.click();
      await expect(recipe).toHaveValue("container-dev");
      await recipe.selectOption("selected-recipe");
      await expect(recipe).toHaveValue("selected-recipe");
      await expect(sandbox).toHaveAttribute("aria-pressed", "true");
      await sandbox.click();
      await expect(recipe).toHaveValue("selected-recipe");
      await expect(sandbox).toHaveAttribute("aria-pressed", "true");
      expect(creates).toBe(0);
    } finally {
      await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
        ["page", () => page.goto("about:blank").then(() => {})],
        ...fixture.stops.map((
          stop,
          index,
        ): [string, () => Promise<void>] => ["route " + index, stop]),
        ["source session", () => cleanupSession(request, source.id)],
      ]);
    }
  },
);

/** Restoring a target mounts both native selects with non-first choices. The
 * visible recipe and host must agree with the protected launch request before
 * any interactive selectOption can repair the browser's initial fallback. */
test("restored container placement displays the recipe and host it submits", async ({ page, request }, testInfo) => {
  const fixture = await environmentChoices(page, request);
  const fleet = await request.get("/api/hosts");
  expect(fleet.ok()).toBe(true);
  const remote = (await fleet.json()).hosts.find((host: any) => host.kind === "ssh" && host.state.phase === "connected");
  expect(remote, "a second real registry host distinguishes the restored place from the first option").toBeTruthy();
  expect(remote.id).not.toBe(fixture.local.id);
  const recipes = ["container-dev", "restored-recipe"].map((name) => ({
    name, size: { cpus: 2, memory_gb: 4, disk_gb: 20 }, base: { kind: "tensorlake_ubuntu" }, packages: [],
    claude: null, codex: null, claude_login_credential: null, codex_login_credential: null, github_token_credential: null,
    tunnels: [], snapshot: null,
  }));
  const places = [fixture.local, remote].map((host, index) => ({
    place: { kind: "podman_host", host: host.id }, name: index === 0 ? "First host" : "Restored host", reason: null, refreshing: false,
  }));
  const target = { kind: "environment", recipe: "restored-recipe", place: { kind: "podman_host", host: remote.id } };
  const name = "restored-container-" + randomUUID();
  let ownsTemplate = false;
  const bodies: any[] = [];
  let ordinaryCreates = 0;
  fixture.stops.push(
    await drainedRoute(page, "**/api/tensorlake/recipes", async (route) => route.fulfill({ headers: fixture.headers, json: { recipes } })),
    await drainedRoute(page, "**/api/environment-places", async (route) => route.fulfill({ headers: fixture.headers, json: places })),
    await drainedRoute(page, `**/api/hosts/${remote.id}/podman-probe`, async (route) => route.fulfill({ status: 202, headers: fixture.headers })),
    await drainedRoute(page, "**/api/recipe-places", async (route) => route.fulfill({ headers: fixture.headers, json: recipes.flatMap((recipe) => places.map((place) => ({
      recipe: recipe.name, place: place.place, ready: true, building: false, reason: null, built_at: 1, prepared_at: 2,
      step: null, cleanup_pending: false,
    }))) })),
    await drainedRoute(page, "**/api/environment-sessions", async (route) => {
      expect(route.request().method()).toBe("POST");
      bodies.push(route.request().postDataJSON());
      await route.fulfill({ status: 409, headers: fixture.headers, body: "restored container fixture refused" });
    }),
    await drainedRoute(page, "**/api/sessions", async (route) => {
      if (route.request().method() === "POST") ordinaryCreates++;
      await route.fallback();
    }),
  );
  try {
    const saved = await request.put("/api/templates/" + name, {
      data: { kind: "command", command: "sleep 300", yolo: false, target, destination: { folder: "~" } },
    });
    expect(saved.ok(), await saved.text()).toBe(true);
    ownsTemplate = true;
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await form.getByRole("combobox", { name: "Search folders, harnesses, models, and efforts", exact: true }).fill(`tl:${name}`);
    await form.getByRole("option", { name: `Template: ${name}`, exact: true }).click();
    const recipe = form.getByRole("combobox", { name: "Recipe", exact: true });
    const host = form.getByRole("combobox", { name: "Container host", exact: true });
    await expect(recipe.locator("option")).toHaveCount(2);
    await expect(host.locator("option")).toHaveCount(2);
    await expect(recipe.locator("option").first()).toHaveAttribute("value", "container-dev");
    await expect(host.locator("option").first()).toHaveAttribute("value", String(fixture.local.id));
    await expect(recipe).toHaveValue(target.recipe);
    await expect(host).toHaveValue(String(target.place.host));
    await expect(recipe.locator("option:checked")).toHaveText(target.recipe);
    await expect(host.locator("option:checked")).toHaveText("Restored host");
    expect(bodies, "applying a template only restores the draft").toHaveLength(0);
    await form.locator(".create-session-submit").click();
    await expect(form.getByRole("alert")).toContainText("restored container fixture refused");
    expect(bodies).toHaveLength(1);
    expect(bodies[0]).toMatchObject({ recipe: target.recipe, place: target.place, request: { cwd: "~" } });
    expect(ordinaryCreates, "the displayed environment target never falls through to ordinary create").toBe(0);
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ["page", () => page.goto("about:blank").then(() => {})],
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => ["route " + index, stop]),
      ["template", async () => {
        if (!ownsTemplate) return;
        const removed = await request.delete("/api/templates/" + name);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
    ]);
  }
});

/** A feed reply may change ranking, but cannot change the Recent that Enter
 * will launch. The component's consumed-history revision distinguishes a stable
 * offered snapshot from a response the browser has not rendered yet. Equal tab
 * choice, direct Recent acceptance and search input each promote deliberately. */
test("environment recents hold focused intent until deliberate promotion", async ({ page, request }, testInfo) => {
  const fixture = await environmentChoices(page, request);
  const feed = await stubFeed(page);
  feed.notifyOnConnect(0);
  const target = { kind: "environment", recipe: "container-dev", place: { kind: "podman_host", host: fixture.local.id } };
  const selection = (model: string) => ({ harness: "codex", model, effort: "high", permissions: "guarded" });
  const entry = (model: string, at: number) => ({
    target, host: fixture.local.id, canonical_cwd: "~", cwd: "~", selection: selection(model), created_at: at, creation_seq: at,
  });
  const old = entry("environment-original", 1);
  const fresh = entry("environment-fresh", 2);
  let entries = [old];
  let held: ReturnType<typeof routeGate> | undefined;
  let reached: ReturnType<typeof routeGate> | undefined;
  let reads = 0;
  const bodies: any[] = [];
  fixture.stops.push(
    await drainedRoute(page, "**/api/launch-history**", async (route) => route.fulfill({ headers: fixture.headers, json: { launches: [], folders: [] } })),
    await drainedRoute(page, "**/api/launch-catalog", async (route) => route.fulfill({ headers: fixture.headers, json: [
      { id: "environment-original", harness: "codex", efforts: ["high"] },
      { id: "environment-fresh", harness: "codex", efforts: ["high"] },
    ] })),
    await drainedRoute(page, "**/api/environment-launch-history", async (route) => {
      reads++;
      const answer = entries;
      const release = held;
      reached?.release();
      await release?.wait();
      await route.fulfill({ headers: fixture.headers, json: answer });
    }),
    await drainedRoute(page, "**/api/environment-sessions", async (route) => {
      bodies.push(route.request().postDataJSON());
      await route.fulfill({ status: 409, headers: fixture.headers, body: "environment recent fixture refused" });
    }),
  );
  try {
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    const recents = form.locator(".launch-composer-recent-slots > button");
    await expect(recents).toHaveCount(1);
    await expect(recents.first()).toContainText(old.selection.model);
    await expect(form).toHaveAttribute("data-template-catalogs-ready", "true");
    await expect(form.getByRole("button", { name: "New container", exact: true })).toBeEnabled();
    /** Publish a distinct response through the live feed, holding it until the
     * existing offer is observable. Only the mounted consumption revision ends
     * this boundary; an HTTP response or request count alone is insufficient. */
    const publish = async (next: typeof entries, revision: number) => {
      const before = Number(await form.getAttribute("data-environment-history-fetched-revision"));
      entries = next;
      held = routeGate();
      reached = routeGate();
      await expect.poll(() => feed.openSockets()).toBe(1);
      feed.notify(revision);
      await reached.wait();
      await expect(recents).toHaveCount(1);
      held.release();
      await expect.poll(async () => Number(await form.getAttribute("data-environment-history-fetched-revision"))).toBeGreaterThan(before);
    };
    await page.mouse.move(0, 0);
    await recents.first().focus();
    await expect(recents.first()).toBeFocused();
    const original = await recents.first().getAttribute("aria-label");
    await publish([fresh], 41);
    await expect(recents.first()).toBeFocused();
    await expect(recents.first()).toHaveAttribute("aria-label", original!);
    const afterFreshRead = reads;
    await form.getByRole("tab", { name: "Agent", exact: true }).click();
    await expect(recents.first()).toContainText(fresh.selection.model);
    expect(reads, "an equal Agent choice promotes without another history request").toBe(afterFreshRead);

    await publish([old], 42);
    await expect(recents.first()).toContainText(fresh.selection.model);
    await form.getByRole("tab", { name: "Agent", exact: true }).click();
    await expect(recents.first()).toContainText(old.selection.model);
    await page.mouse.move(0, 0);
    await recents.first().focus();
    await expect(recents.first()).toBeFocused();
    await publish([fresh], 43);
    await expect(recents.first()).toBeFocused();
    await expect(recents.first()).toHaveAttribute("aria-label", original!);
    await recents.first().press("Enter");
    await expect(form.getByRole("alert")).toContainText("environment recent fixture refused");
    expect(bodies).toHaveLength(1);
    expect(bodies[0]).toMatchObject({ recipe: target.recipe, place: target.place, request: { launch: selection(old.selection.model), cwd: "~" } });
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toHaveCount(0);
    await expect(form.getByRole("combobox", { name: "Recipe", exact: true })).toHaveValue(target.recipe);

    // With no ordinary target, search must still use the offered environment
    // projection for its immediate active result, then apply that exact row.
    const search = form.getByRole("combobox", { name: "Search folders, harnesses, models, and efforts", exact: true });
    await search.fill("recent:");
    const results = form.getByRole("group", { name: "Recent setups", exact: true }).getByRole("option");
    await expect(results).toHaveCount(1);
    await expect(results.first()).toContainText(fresh.selection.model);
    await expect(search).toHaveAttribute("aria-activedescendant", await results.first().getAttribute("id") as string);
    await search.press("Enter");
    await form.locator(".create-session-submit").click();
    await expect.poll(() => bodies.length).toBe(2);
    expect(bodies[1]).toMatchObject({ recipe: target.recipe, place: target.place, request: { launch: selection(fresh.selection.model), cwd: "~" } });
  } finally {
    held?.release();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ["page", () => page.goto("about:blank").then(() => {})],
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => ["route " + index, stop]),
    ]);
  }
});

/** Environment history may load while an ordinary host's default folder is
 * still unread. It must offer reusable intent without treating its home Folder
 * as that host's remembered folder or settling the ordinary launch wait. */
test("environment recents stay independent of a held ordinary folder default", async ({ page, request }, testInfo) => {
  const fixture = await environmentChoices(page, request);
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const remote = (await response.json()).hosts.find((host: any) => host.kind === "ssh" && host.state.phase === "connected");
  expect(remote, "changing to another real installation must rearm its default-folder read").toBeTruthy();
  const source = await createSession(request, { title: "environment-default-source", host: fixture.local.id });
  const reached = routeGate();
  const release = routeGate();
  const selection = { harness: "codex", model: null, effort: null, permissions: "guarded" };
  fixture.stops.push(
    await drainedRoute(page, "**/api/environment-launch-history", async (route) => route.fulfill({ headers: fixture.headers, json: [{
      target: { kind: "environment", recipe: "container-dev", place: { kind: "podman_host", host: fixture.local.id } },
      host: fixture.local.id, cwd: "~", canonical_cwd: "~", selection, created_at: 2, creation_seq: 2,
    }] })),
    await drainedRoute(page, "**/api/launch-history**", async (route) => {
      if (new URL(route.request().url()).searchParams.get("host") !== String(remote.id)) {
        return route.fulfill({ headers: fixture.headers, json: { launches: [], folders: [] } });
      }
      reached.release();
      await release.wait();
      await route.fulfill({ headers: fixture.headers, json: { launches: [{
        host: remote.id, cwd: "/ordinary-remembered", canonical_cwd: "/ordinary-remembered", selection, created_at: 1, creation_seq: 1,
      }], folders: [] } });
    }),
  );
  try {
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    const environmentRecent = form.locator(".launch-composer-recent-slots > button").filter({ hasText: "container-dev container" });
    await expect(environmentRecent).toHaveCount(1);
    await form.getByRole("combobox", { name: "Host", exact: true }).click();
    await form.locator(`.launcher-host-option[data-host-id="${remote.id}"]`).click();
    await reached.wait();
    await expect(environmentRecent, "a held ordinary read cannot withdraw helm-wide environment intent").toBeVisible();
    await expect(form.locator(".create-session-submit")).toHaveAttribute("data-tooltip", "Wait for this host's last folder before launching");
    release.release();
    await expect(form.getByLabel("Folder", { exact: true })).toHaveValue("/ordinary-remembered");
    await expect(environmentRecent).toBeVisible();
    await environmentRecent.click();
    await expect(form.getByRole("combobox", { name: "Recipe", exact: true })).toHaveValue("container-dev");
    await expect(form.getByRole("combobox", { name: "Container host", exact: true })).toHaveValue(String(fixture.local.id));
  } finally {
    release.release();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ["page", () => page.goto("about:blank").then(() => {})],
      ...fixture.stops.map((stop, index): [string, () => Promise<void>] => ["route " + index, stop]),
      ["source session", () => cleanupSession(request, source.id)],
    ]);
  }
});

/** Container placement must never use ordinary create's default host or cloud
 * consent. The refused protected request returns to the same editable target. */
test(
  "container launch keeps recipe and place through refusal",
  async ({ page, request }, testInfo) => {
    const fixture = await environmentChoices(page, request);
    const source = await createSession(request, { title: "container-placement-source" });
    const bodies: any[] = [];
    let ordinaryCreates = 0;
    fixture.stops.push(
      await drainedRoute(page, "**/api/environment-sessions", async (route) => {
        bodies.push(route.request().postDataJSON());
        await route.fulfill({
          status: 409,
          headers: fixture.headers,
          body: "container fixture refused launch",
        });
      }),
    );
    fixture.stops.push(
      await drainedRoute(page, "**/api/sessions", async (route) => {
        if (route.request().method() === "POST") ordinaryCreates++;
        await route.continue();
      }),
    );
    try {
      await pinAutoSelect(page, source.id);
      await page.goto("/");
      await page.locator(".new-session-button").click();
      const form = page.locator(".create-session-form");
      await expect(form).toBeVisible();
      await form.getByRole("button", { name: "New container", exact: true }).click();
      await expect(form.getByRole("combobox", { name: "Recipe", exact: true })).toHaveValue(
        "container-dev",
      );
      await expect(form.getByRole("combobox", { name: "Container host", exact: true })).toHaveValue(
        String(fixture.local.id),
      );
      await expect(form).toContainText("Ready on Build machine");
      await form.getByRole("tab", { name: "Command", exact: true }).click();
      await form.getByLabel("Agent command", { exact: true }).fill("sleep 300");
      await answerYolo(form, false);
      await form.locator(".create-session-submit").click();
      await expect(form.getByRole("alert")).toContainText("container fixture refused launch");
      expect(bodies).toHaveLength(1);
      expect(bodies[0]).toMatchObject({
        recipe: "container-dev",
        place: { kind: "podman_host", host: fixture.local.id },
        request: { cwd: "~" },
      });
      expect(bodies[0].request).not.toHaveProperty("host");
      expect(bodies[0].request).not.toHaveProperty("expected_incarnation");
      expect(ordinaryCreates).toBe(0);
      await expect(
        form.getByRole("alertdialog", { name: "Create a sandbox for this session?", exact: true }),
      ).toHaveCount(0);
      await expect(form.getByRole("combobox", { name: "Container host", exact: true })).toHaveValue(
        String(fixture.local.id),
      );
    } finally {
      await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
        ...fixture.stops.map((
          stop,
          index,
        ): [string, () => Promise<void>] => ["route " + index, stop]),
        ["source session", () => cleanupSession(request, source.id)],
      ]);
    }
  },
);

/** SPEC.md, "Container sessions": a user's Delete follows the sandbox rule
 * with container wording. A connected container's Delete states both outcomes
 * (no snapshots: containers have none), asks the container's Farhelm with
 * `?delete_environment_session=true`, and, when the container stays, says so
 * with the count that Farhelm reported. The stack's remote host is presented
 * as a session-owned container; its helm would refuse the environment Delete
 * for what is really an ordinary host, so the answer is given here in the
 * helm's shape, after the session is really deleted so the next listing
 * agrees with it. tensorlake-sandbox-sessions.spec.ts covers the unasked
 * answer, bulk Delete and the being-deleted state for the shared code. */
test("a connected container's Delete asks its Farhelm and reports the container that stays", async ({ page, request }, testInfo) => {
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  const hosts = (await response.json()).hosts;
  const local = hosts.find((host: any) => host.kind === "local");
  const remote = hosts.find((host: any) => host.kind === "ssh" && host.state.phase === "connected");
  expect(remote, "the owned stack has a connected remote supervisor").toBeTruthy();
  const stops: Array<() => Promise<void>> = [];
  const owned: string[] = [];
  const sent: string[] = [];
  try {
    const kept = await createSession(request, { title: "container-delete-kept-" + randomUUID(), host: remote.id });
    owned.push(kept.id);
    const gone = await createSession(request, { title: "container-delete-gone-" + randomUUID(), host: remote.id });
    owned.push(gone.id);
    stops.push(await drainedRoute(page, "**/api/hosts", async (route) => {
      const reply = await route.fetch();
      const json = await reply.json();
      json.hosts = json.hosts.map((host: any) => host.id === remote.id ? {
        ...host, kind: "podman", name: "New container", is_session_owned: true, tensorlake: null,
        environment: {
          name: "owned-container", place_name: "Build machine",
          target: { kind: "environment", recipe: "dev", place: { kind: "podman_host", host: local.id } },
          tunnels: null, credential_warning: null,
        },
      } : host);
      await route.fulfill({ response: reply, json });
    }));
    const path = "/api/sessions/" + encodeURIComponent(gone.id);
    stops.push(await drainedRoute(page, (url) => url.pathname === path, async (route) => {
      if (route.request().method() !== "DELETE") return route.fallback();
      sent.push(new URL(route.request().url()).search);
      await cleanupSession(request, gone.id);
      await route.fulfill({ status: 200, headers: { "x-farhelm-build": build }, json: { environment_remaining: 1 } });
    }));
    await page.goto("/");
    const row = page.locator('.session-row[data-session-id="' + gone.id + '"]');
    const keptRow = page.locator('.session-row[data-session-id="' + kept.id + '"]');
    await expect(row).toHaveCount(1);
    await expect(keptRow).toHaveCount(1);
    await openRowMenu(row);
    await expect(row.locator(".session-row-delete")).toContainText("the container goes too if this is its last session");
    await row.locator(".session-row-delete").click();
    const prompt = row.locator(".session-row-menu-panel");
    await expect(prompt).toContainText(
      "If this is the last session in its container, the container goes too, with every process and file in it.");
    await expect(prompt).toContainText("container: owned-container");
    await expect(prompt).not.toContainText("snapshot");
    expect(sent).toEqual([]);
    await expect(row.locator(".confirm-delete")).toHaveText("Delete session");
    await row.locator(".confirm-delete").click();
    await expect.poll(() => sent).toEqual(["?delete_environment_session=true"]);
    await expect(page.locator(".delete-notice .status")).toHaveText("The container stays: 1 other session is in it");
    await expect(row).toHaveCount(0);
    await expect(keptRow).toHaveCount(1);
  } finally {
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...stops.map((stop, index): [string, () => Promise<void>] => ["container delete route " + index, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** Leftovers remain deliberate whole-container deletion. A refusal must remain
 * visible and retryable, and merely opening or cancelling must send no delete. */
test("Containers Settings keeps failed cleanup explicit", async ({ page, request }, testInfo) => {
  const fixture = await environmentChoices(page, request);
  let deletes = 0;
  fixture.stops.push(
    await drainedRoute(page, "**/api/container-orphans", async (route) => {
      if (route.request().method() === "DELETE") {
        deletes++;
        expect(route.request().postDataJSON()).toMatchObject({
          id: "owned-container",
          place: { kind: "podman_host", host: fixture.local.id },
        });
        return route.fulfill({
          status: 409,
          headers: fixture.headers,
          body: "provider fixture refused cleanup",
        });
      }
      await route.fulfill({
        status: 200,
        headers: fixture.headers,
        json: [{
          id: "owned-container",
          name: "Interrupted launch",
          place: { kind: "podman_host", host: fixture.local.id },
          role: "session",
          host: null,
        }],
      });
    }),
  );
  try {
    await page.goto("/");
    const dialog = await openSettings(page, "containers");
    await expect(dialog.getByRole("region", { name: "Container hosts", exact: true }))
      .toContainText("Build machine");
    const leftovers = dialog.getByRole("region", {
      name: "Containers not on this helm",
      exact: true,
    });
    await expect(leftovers).toContainText("Interrupted launch");
    await leftovers.getByRole("button", { name: "Delete", exact: true }).click();
    const question = dialog.getByRole("alertdialog", { name: "Delete container?", exact: true });
    await expect(question).toContainText("Every session, process, file and unpushed change");
    expect(deletes).toBe(0);
    await question.getByRole("button", { name: "Cancel", exact: true }).click();
    expect(deletes).toBe(0);
    await leftovers.getByRole("button", { name: "Delete", exact: true }).click();
    await question.getByRole("button", { name: "Delete container", exact: true }).click();
    await expect(dialog.getByRole("alert")).toContainText("provider fixture refused cleanup");
    expect(deletes).toBe(1);
    await expect(leftovers).toContainText("Interrupted launch");
  } finally {
    await cleanupOwnedSteps(
      testInfo.status !== testInfo.expectedStatus,
      fixture.stops.map((stop, index): [string, () => Promise<void>] => ["route " + index, stop]),
    );
  }
});

/** Number shortcuts wait for the saved container place before immediate submit.
 * The held first place answer keeps the loading boundary observable. Its release
 * must admit exactly one provider request carrying the stored recipe and place. */
test(
  "container template quick launch retains its saved place",
  async ({ page, request }, testInfo) => {
    const fixture = await environmentChoices(page, request);
    const name = "container-template-" + randomUUID();
    const saved = await request.put("/api/templates/" + name, {
      data: {
        kind: "command",
        command: "sleep 300",
        yolo: false,
        target: {
          kind: "environment",
          recipe: "container-dev",
          place: { kind: "podman_host", host: fixture.local.id },
        },
        destination: { folder: "~" },
      },
    });
    expect(saved.ok(), await saved.text()).toBe(true);
    const bodies: any[] = [];
    const placesGate = routeGate();
    let heldPlaceReads = 0;
    let ordinaryCreates = 0;
    fixture.stops.push(
      await drainedRoute(page, "**/api/sessions", async (route) => {
        if (route.request().method() !== "POST") return route.fallback();
        ordinaryCreates++;
        await route.fulfill({ status: 409, headers: fixture.headers, body: "ordinary host fixture refused" });
      }),
    );
    fixture.stops.push(
      await drainedRoute(page, "**/api/environment-places", async (route) => {
        heldPlaceReads++;
        await placesGate.wait();
        await route.fallback();
      }),
    );
    fixture.stops.push(
      await drainedRoute(page, "**/api/preferences", async (route) => {
        if (route.request().method() !== "GET") return route.fallback();
        const response = await route.fetch();
        await route.fulfill({
          response,
          json: { ...await response.json(), template_shortcuts: { "1": name } },
        });
      }),
    );
    fixture.stops.push(
      await drainedRoute(page, "**/api/environment-sessions", async (route) => {
        bodies.push(route.request().postDataJSON());
        await route.fulfill({
          status: 409,
          headers: fixture.headers,
          body: "container template fixture refused",
        });
      }),
    );
    try {
      await page.goto("/");
      await expect(page.locator(".template-number-trigger")).toHaveCount(10);
      await page.keyboard.press("Control+Shift+Digit1");
      const form = page.locator(".create-session-form");
      await expect.poll(() => heldPlaceReads, {
        message: "the saved container place is waiting for its first cached answer",
      }).toBeGreaterThan(0);
      await expect(form).toHaveAttribute("data-template-catalogs-ready", "true");
      await expect(form.getByText("Loading session setup…", { exact: true })).toBeAttached();
      await expect(form).toHaveJSProperty("hidden", true);
      await expect(form.getByRole("alert", { includeHidden: true })).toHaveCount(0);
      expect(bodies, "a pending place answer cannot launch").toHaveLength(0);
      expect(ordinaryCreates).toBe(0);
      placesGate.release();
      await expect(form.getByRole("alert")).toContainText("container template fixture refused");
      expect(bodies).toHaveLength(1);
      expect(bodies[0]).toMatchObject({
        recipe: "container-dev",
        place: { kind: "podman_host", host: fixture.local.id },
      });
      expect(bodies[0].request).not.toHaveProperty("host");
      expect(ordinaryCreates, "the saved container target cannot fall through to an ordinary host").toBe(0);
      await expect(form.getByRole("combobox", { name: "Container host", exact: true })).toHaveValue(
        String(fixture.local.id),
      );
    } finally {
      await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
        ["place-answer gate", () => placesGate.release()],
        ...fixture.stops.map((
          stop,
          index,
        ): [string, () => Promise<void>] => ["route " + index, stop]),
        ["template", async () => {
          const response = await request.delete("/api/templates/" + name);
          expect(response.ok() || response.status() === 404).toBe(true);
        }],
      ]);
    }
  },
);

for (const surface of ["launcher", "recipes"] as const) {
  /** The background probe completes through cached place observations only.
   * A healthy, silent fleet feed cannot repair the stale recipe-place response;
   * the mounted consumer must invalidate it when the probe settles. No rebuild,
   * launch or real Podman probe belongs to this presentation fixture. */
  test(
    surface + " refreshes recipe places after a delayed probe without a fleet notice",
    async ({ page, request }, info) => {
      const fixture = await environmentChoices(page, request);
      const feed = await stubFeed(page);
      feed.notifyOnConnect(1);
      const name = "probe-placement-" + randomUUID();
      const oldReason = "Previous cached Podman check was unusable";
      let settled = false;
      let probes = 0;
      let readyPlaceReads = 0;
      let recipeReads = 0;
      let creates = 0;
      fixture.stops.push(
        await drainedRoute(
          page,
          "**/api/hosts/" + fixture.local.id + "/podman-probe",
          async (route) => {
            expect(route.request().method()).toBe("POST");
            probes++;
            // Admission returns before the asynchronous probe changes the cache.
            await route.fulfill({ status: 202, headers: fixture.headers });
          },
        ),
      );
      fixture.stops.push(
        await drainedRoute(page, "**/api/environment-places", async (route) => {
          if (settled) readyPlaceReads++;
          await route.fulfill({
            status: 200,
            headers: fixture.headers,
            json: [{
              place: { kind: "podman_host", host: fixture.local.id },
              name: "Build machine",
              reason: settled ? null : oldReason,
              refreshing: !settled,
            }],
          });
        }),
      );
      fixture.stops.push(
        await drainedRoute(page, "**/api/recipe-places", async (route) => {
          recipeReads++;
          await route.fulfill({
            status: 200,
            headers: fixture.headers,
            json: [{
              recipe: "container-dev",
              place: { kind: "podman_host", host: fixture.local.id },
              ready: settled && surface === "launcher",
              building: false,
              reason: settled ? null : oldReason,
              built_at: 1700000000,
              prepared_at: surface === "launcher" ? 1700000001 : null,
              step: null,
              cleanup_pending: false,
            }],
          });
        }),
      );
      fixture.stops.push(
        await drainedRoute(page, "**/api/container-orphans", async (route) => {
          await route.fulfill({ status: 200, headers: fixture.headers, json: [] });
        }),
      );
      for (const endpoint of ["sessions", "environment-sessions"]) {
        fixture.stops.push(
          await drainedRoute(page, "**/api/" + endpoint, async (route) => {
            if (route.request().method() !== "POST") return route.fallback();
            creates++;
            await route.fulfill({
              status: 409,
              headers: fixture.headers,
              body: "probe fixture does not launch",
            });
          }),
        );
      }
      try {
        if (surface === "launcher") {
          const saved = await request.put("/api/templates/" + name, {
            data: {
              kind: "command",
              command: "sleep 300",
              yolo: false,
              destination: { folder: "~" },
              target: {
                kind: "environment",
                recipe: "container-dev",
                place: { kind: "podman_host", host: fixture.local.id },
              },
            },
          });
          expect(saved.ok(), await saved.text()).toBe(true);
        }
        await page.goto("/");
        await feed.waitForConnection(1);
        let status: ReturnType<Page["locator"]>;
        if (surface === "launcher") {
          await page.locator(".new-session-button").click();
          const form = page.locator(".create-session-form");
          // A stored explicit place remains selected even while it is unusable.
          const search = form.locator('.launch-composer-search input[role="combobox"]');
          await search.fill("tl:" + name);
          await form.getByRole("option", { name: "Template: " + name, exact: true }).click();
          await expect(form.getByRole("combobox", { name: "Container host", exact: true }))
            .toHaveValue(String(fixture.local.id));
          status = form.locator("p.host-settings-help[role=status]");
        } else {
          const dialog = await openSettings(page, "recipes");
          const row = dialog.locator(".recipe-list > .recipe-row").filter({
            has: page.locator(".recipe-row-name").filter({ hasText: /^container-dev$/ }),
          });
          status = row.locator(".recipe-row-main > .recipe-row-status");
        }
        await expect(status).toContainText(oldReason);
        // Settings retains Recipes and Containers as separate mounted consumers;
        // both opening probes must be admitted before completing the fixture job.
        await expect.poll(() => probes).toBe(surface === "recipes" ? 2 : 1);
        const probesBefore = probes;
        const recipeReadsBefore = recipeReads;
        expect(recipeReadsBefore, "premise: a stale recipe-place response was displayed")
          .toBeGreaterThan(0);
        expect(feed.openSockets(), "the silent healthy feed remains connected").toBe(1);
        settled = true;
        await expect.poll(() => readyPlaceReads, {
          timeout: 20_000,
          message: "cached place polling observes the completed probe",
        }).toBeGreaterThan(0);
        await expect.poll(() => recipeReads, {
          timeout: 20_000,
          message: "changed cached places invalidate the stale recipe projection",
        }).toBeGreaterThan(recipeReadsBefore);
        await expect(status).not.toContainText(oldReason);
        if (surface === "launcher") await expect(status).toContainText("Ready on Build machine");
        else await expect(status).toContainText("Built 2023-11-14");
        expect(probes, "projection refresh must not force another probe").toBe(probesBefore);
        expect(feed.connections(), "no reconnect greeting supplies a replacement fleet notice")
          .toBe(1);
        expect(creates).toBe(0);
      } finally {
        await cleanupOwnedSteps(info.status !== info.expectedStatus, [
          ...fixture.stops.map((
            stop,
            index,
          ): [string, () => Promise<void>] => ["probe route " + index, stop]),
          ["probe placement template", async () => {
            const response = await request.delete("/api/templates/" + name);
            expect(response.ok() || response.status() === 404).toBe(true);
          }],
        ]);
      }
    },
  );
}
