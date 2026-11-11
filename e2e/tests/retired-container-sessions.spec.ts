/** Retirement is identified by the stored launch, not by the host's provider.
 * Both sessions below are real owned-stack sessions. HTTP rewrites change only
 * presentation: no old container or new Podman environment is created, and the
 * normal container's attachment still reaches its real remote supervisor. */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { drainedRoute } from "./helpers/drained-route";
import { cleanupSession, createSession, listSessions, openRowMenu, pinAutoSelect } from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { attachSession, waitForTermText } from "./helpers/term";

/** Old rows stay inspectable and deletable without mounting an input surface;
 * new provider-backed containers retain normal lifecycle and terminal controls.
 * A refused Delete proves the surviving action dispatches without removing the
 * real backing fixture; supervisor cleanup semantics have separate coverage. */
test("retired container rows offer Delete only while new containers keep normal controls", async ({ page, request }, info) => {
  const hostsReply = await request.get("/api/hosts");
  expect(hostsReply.ok()).toBe(true);
  const build = requireHelmBuild(hostsReply, "retired container presentation");
  const hosts = (await hostsReply.json()).hosts;
  const local = hosts.find((host: any) => host.kind === "local" && host.state.phase === "connected");
  const remote = hosts.find((host: any) => host.kind === "ssh" && host.state.phase === "connected");
  expect(local, "owned stack has its connected local supervisor").toBeTruthy();
  expect(remote, "owned stack has a connected remote supervisor").toBeTruthy();
  const stops: Array<() => Promise<void>> = [];
  const owned: string[] = [];
  const mutations: string[] = [];
  const attachments: string[] = [];
  try {
    const old = await createSession(request, { title: "retired-container-row", host: local.id });
    owned.push(old.id);
    const current = await createSession(request, { title: "new-container-row", host: remote.id });
    owned.push(current.id);
    const actual = (await listSessions(request)).sessions;
    expect(actual.find(row => row.id === old.id)).toBeTruthy();
    expect(actual.find(row => row.id === current.id)).toBeTruthy();
    const currentReply = await request.get("/api/sessions/" + encodeURIComponent(current.id));
    expect(currentReply.ok()).toBe(true);
    expect((await currentReply.json()).launch.kind).toBe("command");
    const retiredLaunch = { kind: "agent", selection: { harness: "codex", model: null, effort: null,
      permissions: null, container: { engine: "podman", image_file: "legacy.tar", api_proxy_port: 43210 } },
      start: ["codex", "{farhelm_args}"], resume: null };
    const oldPath = "/api/sessions/" + encodeURIComponent(old.id);
    const oldDecodedPath = "/api/sessions/" + old.id;
    page.on("request", sent => {
      const path = decodeURIComponent(new URL(sent.url()).pathname);
      if (path.startsWith(oldDecodedPath) && sent.method() !== "GET") mutations.push(sent.method() + " " + path);
    });
    page.on("websocket", socket => {
      if (decodeURIComponent(new URL(socket.url()).pathname).startsWith(oldDecodedPath + "/")) attachments.push(socket.url());
    });
    stops.push(await drainedRoute(page, "**/api/hosts", async route => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = body.hosts.map((host: any) => host.id === remote.id ? { ...host, kind: "podman", name: "New container",
        is_session_owned: true, tensorlake: null, environment: { name: "owned-container", place_name: "Build machine",
          target: { kind: "environment", recipe: "dev", place: { kind: "podman_host", host: local.id } },
          tunnels: null, credential_warning: null } } : host);
      await route.fulfill({ response, json: body });
    }));
    stops.push(await drainedRoute(page, url => url.pathname === "/api/sessions", async route => {
      if (route.request().method() !== "GET") return route.fallback();
      const response = await route.fetch();
      const body = await response.json();
      body.sessions = body.sessions.map((row: any) => row.id === old.id ? { ...row, launch: retiredLaunch } : row);
      await route.fulfill({ response, json: body });
    }));
    stops.push(await drainedRoute(page, url => url.pathname === oldPath || decodeURIComponent(url.pathname) === oldDecodedPath, async route => {
      if (route.request().method() === "DELETE") {
        return route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "retired fixture retained for retry" });
      }
      const response = await route.fetch();
      const body = await response.json();
      await route.fulfill({ response, json: { ...body, launch: retiredLaunch } });
    }));
    stops.push(await drainedRoute(page, "**/api/preferences", async route => {
      if (route.request().method() !== "GET") return route.fallback();
      const response = await route.fetch();
      await route.fulfill({ response, json: { ...await response.json(), skip_session_confirmations: false } });
    }));
    await pinAutoSelect(page, old.id);
    await page.goto("/");
    const oldRow = page.locator('[data-session-id="' + old.id + '"]');
    await expect(oldRow).toHaveAttribute("data-session-selected", "true");
    await expect(oldRow).toContainText("Retired container");
    await expect(page.locator(".retired-container-card")).toContainText("Delete only");
    await expect(page.locator(".terminal-pane, .tab-strip")).toHaveCount(0);
    await expect(page.locator(".header-stop, .restart-primary, .header-clone, .header-replace")).toHaveCount(0);
    await openRowMenu(oldRow);
    const menu = page.getByRole("menu", { name: "Session actions for retired-container-row", exact: true });
    await expect(menu.getByRole("menuitem")).toHaveCount(1);
    await expect(menu.getByRole("menuitem", { name: "Delete", exact: true })).toBeVisible();
    const deleteItem = menu.getByRole("menuitem", { name: "Delete", exact: true });
    await deleteItem.focus();
    await expect(deleteItem).toBeFocused();
    await deleteItem.press("Escape");
    await page.locator(".header-delete").click();
    const question = page.locator(".header-delete-confirm");
    await expect(question).toBeVisible();
    expect(mutations).toEqual([]);
    await question.locator(".header-delete-confirm-submit").click();
    await expect(page.locator(".session-action-error, .session-row-error").filter({ hasText: "retired fixture retained for retry" }).first()).toBeVisible();
    expect(mutations).toEqual(["DELETE " + oldDecodedPath]);
    await attachSession(page, current.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".header-stop")).toBeVisible();
    await expect(page.locator(".header-clone")).toBeVisible();
    await expect(page.locator(".restart-primary")).toBeVisible();
    await expect(page.locator(".header-replace")).toBeVisible();
    await expect(page.getByRole("button", { name: "Suspend", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "New terminal", exact: true })).toBeVisible();
    await expect(page.locator(".retired-container-card")).toHaveCount(0);
    expect(attachments, "retired rows must never open an agent or tab socket").toEqual([]);
  } finally {
    await cleanupOwnedSteps(info.status !== info.expectedStatus, [
      ...stops.map((stop, index): [string, () => Promise<void>] => ["retirement route " + index, stop]),
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});
