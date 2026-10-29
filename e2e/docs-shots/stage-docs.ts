/**
 * The docs screenshots' layer over the README staging (docs/docs-shots/SPEC.md):
 * the docs overlay theme, the staged fleet handed from the staging project to
 * the shot projects, and the extra in-transit rewrites the docs need on
 * top of the hero's two.
 *
 * Why more rewrites than the hero allows: docs pages show parts of the UI the
 * hero never does. The session launcher's recent setups and folders come from
 * launch history, which a fresh stack does not have and staged sessions do not
 * create; and host settings show each host's ssh destination, which for a
 * self-ssh spelling is the capturing machine's account name. The SPEC lists
 * these rewrites by name; anything else rewritten here would be a SPEC change.
 */
import { expect, type APIRequestContext, type Page } from "@playwright/test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { OverlayTheme } from "../readme-video/overlay";
import type { Scenario, ScenarioSession } from "../readme-hero/scenario";
import { openStagedFleet, readStackInfo, type StagedFleet, writeAgentScript } from "../readme-hero/stage";
import { requireHelmBuild } from "../tests/helpers/helm-build";
import { DOCS_FLEET_PATH, DOCS_SHOTS_DIR, DOCS_STACK_INFO_PATH } from "./paths";

/**
 * Red and small: docs images are cropped, shown at half their pixel size, and
 * sit inside a page of text, where the video's large pink callouts would
 * dominate. Sizes are CSS pixels at the capture's scale of 2.
 */
export const DOCS_THEME: OverlayTheme = {
  accent: "#ef4444",
  glow: "rgba(239, 68, 68, 0.55)",
  calloutPx: 15,
  calloutMaxPx: 270,
  strokePx: 3,
  headPx: 12,
  gapPx: 40,
  ringPx: 3,
};

/** The staged fleet as the staging project writes it to disk. */
interface FleetFile {
  ids: Array<[string, string]>;
  now: number;
  openId: string;
  /** Development-only metadata for the same two approved cwd/age rewrites. */
  toolbarReview?: ScenarioSession;
  /** Owned requester; only its cwd/age join the existing listing rewrites. */
  spawnReview?: ScenarioSession;
}

/** Hand real IDs and declared display metadata to the separate shot processes.
 * Development sources remain in the fleet until their owning test removes
 * them; metadata adds only the same cwd/age rewrites as ordinary rows.
 */
export function writeFleet(fleet: StagedFleet, toolbarReview?: ScenarioSession, spawnReview?: ScenarioSession): void {
  const file: FleetFile = { ids: [...fleet.ids.entries()], now: fleet.now, openId: fleet.openId, toolbarReview, spawnReview };
  writeFileSync(DOCS_FLEET_PATH, JSON.stringify(file));
}

/** Read the fleet the staging project staged in this run. */
export function readFleet(): StagedFleet & { toolbarReview?: ScenarioSession; spawnReview?: ScenarioSession } {
  const file = JSON.parse(readFileSync(DOCS_FLEET_PATH, "utf8")) as FleetFile;
  return { ids: new Map(file.ids), now: file.now, openId: file.openId, toolbarReview: file.toolbarReview, spawnReview: file.spawnReview };
}

/** Stage one requester whose inherited command is safe to show on a real card.
 * The existing private Claude wrapper executes .hero-agent from this cwd;
 * only that file knows the fixture's path. No request is emitted until the
 * dedicated shot test opens the real GUI, and that test must deny it and
 * remove this source. Ordinary docs and hero staging never use this fixture.
 */
export async function stageSpawnReviewSession(request: APIRequestContext, scenario: Scenario, fleet: StagedFleet) {
  const local = scenario.hosts.find((host) => host.kind === "local");
  if (!local) throw new Error("approval review requires the owned local host");
  const review: ScenarioSession = {
    title: "Review agent request", host: local.key, cwd: "/tmp", wrapper: "claude",
    status: "idle", age: 0, yolo: false, seen: true, open: false,
  };
  const info = readStackInfo(DOCS_STACK_INFO_PATH);
  const cwd = path.join(info.work, "spawn-review");
  mkdirSync(cwd, { recursive: true });
  // Quote the private executable as one shell word; forwarded launch flags
  // belong to the existing fixture, just as in the ordinary replay wrapper.
  const fixture = `'${info.fixtures.replaceAll("'", `'\\''`)}'`;
  writeFileSync(path.join(cwd, ".hero-agent"), `#!/bin/sh\nexec ${fixture} fake-agent --script spawn "$@"\n`, { mode: 0o700 });
  const response = await request.post("/api/sessions", {
    data: {
      cwd, title: review.title, host: (await hostIds(request, scenario)).get(local.key),
      command: { command: "claude {farhelm_args}", agent: "claude", yolo: false },
    },
  });
  expect(response.ok(), `create approval review source: ${await response.text()}`).toBe(true);
  const { id } = await response.json();
  fleet.ids.set(review.title, id);
  return review;
}

/** Stage an enabled Restart without inventing a launch or conversation reply.
 * The existing transcript wrapper keeps the photographed invocation portable;
 * its fixture reports through the real Claude hook. The stack owns cleanup,
 * just as for every other staged session. Only cwd and age use scenario values.
 */
export async function stageToolbarReviewSession(request: APIRequestContext, scenario: Scenario, fleet: StagedFleet) {
  const source = scenario.sessions.find((session) => session.status === "idle" && session.wrapper === "claude")!;
  const local = scenario.hosts.find((host) => host.kind === "local")!;
  const review: ScenarioSession = {
    ...source,
    title: "Review session toolbar",
    host: local.key,
    wrapper: "claude",
    status: "idle",
    then: "quiet",
    yolo: false,
    seen: true,
    open: false,
  };
  const info = readStackInfo(DOCS_STACK_INFO_PATH);
  const cwd = path.join(info.work, "toolbar-review");
  mkdirSync(cwd, { recursive: true });
  writeAgentScript(info, DOCS_SHOTS_DIR, cwd, review, "toolbar-review-conversation");
  const response = await request.post("/api/sessions", {
    data: {
      cwd, title: review.title, host: (await hostIds(request, scenario)).get(local.key),
      command: { command: "claude {farhelm_args}", resume: "claude --resume {conversation} {farhelm_args}", agent: "claude", yolo: false },
    },
  });
  expect(response.ok(), `create toolbar review: ${await response.text()}`).toBe(true);
  const { id } = await response.json();
  fleet.ids.set(review.title, id);
  await expect.poll(async () => {
    const listing = await request.get("/api/sessions");
    expect(listing.ok()).toBe(true);
    const current = (await listing.json()).sessions.find((session: { id: string }) => session.id === id);
    return { offer: current?.restart_offer, state: current?.status?.state };
  }, { timeout: 60_000, message: "the review fixture reports a real resumable conversation and settles idle" })
    .toEqual({ offer: "resume", state: "idle" });
  return review;
}

interface HostRow {
  id: number;
  kind: string;
  destination: string | null;
  identity: string | null;
}

/** The helm's host id for every scenario host key, matched the way staging matched them. */
async function hostIds(request: APIRequestContext, scenario: Scenario): Promise<Map<string, number>> {
  const response = await request.get("/api/hosts");
  expect(response.ok(), "GET /api/hosts").toBeTruthy();
  const rows = ((await response.json()) as { hosts: HostRow[] }).hosts;
  const ids = new Map<string, number>();
  for (const host of scenario.hosts) {
    const row = host.kind === "local"
      ? rows.find((candidate) => candidate.kind === "local")
      : rows.find((candidate) => candidate.destination === host.ssh);
    if (!row) throw new Error(`scenario host ${host.key} is not registered`);
    ids.set(host.key, row.id);
  }
  return ids;
}

/**
 * Create the scenario's launch templates on the staged helm, through the same
 * API the templates panel uses, so they are real templates and not a rewrite.
 * A template's `host` is a scenario host key until here, where it becomes
 * that host's install identity.
 */
export async function stageTemplates(request: APIRequestContext, scenario: Scenario): Promise<void> {
  const response = await request.get("/api/hosts");
  expect(response.ok(), "GET /api/hosts").toBeTruthy();
  const rows = ((await response.json()) as { hosts: HostRow[] }).hosts;
  for (const template of scenario.templates ?? []) {
    const fields: Record<string, unknown> = { ...template.fields };
    if (template.fields.host !== undefined) {
      const host = scenario.hosts.find((candidate) => candidate.key === template.fields.host);
      const row = host?.kind === "local"
        ? rows.find((candidate) => candidate.kind === "local")
        : rows.find((candidate) => candidate.destination === host?.ssh);
      if (!row?.identity) throw new Error(`template ${template.name}: host ${template.fields.host} has no identity yet`);
      fields.host = row.identity;
    }
    const put = await request.put(`/api/templates/${encodeURIComponent(template.name)}`, { data: fields });
    expect(put.ok(), `create template ${template.name}: ${await put.text()}`).toBeTruthy();
  }
}

/** Fetch the real reply for a routed GET, minus the headers a rewritten body invalidates. */
async function upstream(route: import("@playwright/test").Route) {
  const response = await fetch(route.request().url(), { headers: await route.request().allHeaders() });
  const headers = Object.fromEntries(response.headers.entries());
  delete headers["content-length"];
  delete headers["content-encoding"];
  delete headers["transfer-encoding"];
  return { response, headers };
}

/**
 * Replace every string in `value` that equals a key of `replacements`,
 * at any depth. Exact matches only: a destination embedded in a longer
 * string (an error message, say) is caught by the shot's leak check instead,
 * which fails the capture rather than guessing at a rewrite.
 */
function replaceStrings(value: unknown, replacements: Map<string, string>): unknown {
  if (typeof value === "string") return replacements.get(value) ?? value;
  if (Array.isArray(value)) return value.map((item) => replaceStrings(item, replacements));
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, replaceStrings(item, replacements)]));
  }
  return value;
}

/**
 * Install the docs rewrites: every host reply shows each remote's
 * `shown_ssh` instead of its real destination, and the launch history reply
 * carries the scenario's launches and folders for the host it was asked
 * about. The history keeps the helm's own checkout revision, since that is
 * real state the launcher compares against.
 */
async function installDocsRewrites(page: Page, request: APIRequestContext, scenario: Scenario, now: number) {
  const ids = await hostIds(request, scenario);
  const keyById = new Map([...ids.entries()].map(([key, id]) => [id, key]));
  const destinations = new Map(
    scenario.hosts.filter((host) => host.ssh && host.shown_ssh).map((host) => [host.ssh as string, host.shown_ssh as string]),
  );

  await page.route((url) => url.pathname === "/api/hosts" || url.pathname.startsWith("/api/hosts/"), async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const { response, headers } = await upstream(route);
    if (!(response.headers.get("content-type") ?? "").includes("json")) {
      await route.fulfill({ status: response.status, headers, body: Buffer.from(await response.arrayBuffer()) });
      return;
    }
    await route.fulfill({ status: response.status, headers, json: replaceStrings(await response.json(), destinations) });
  });

  // A canonical path has to look absolute to the launcher; the home it
  // claims is invented like everything else the scenario shows.
  const canonical = (cwd: string) => cwd.replace(/^~(?=\/|$)/, "/home/you");
  await page.route((url) => url.pathname === "/api/launch-history", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const { response, headers } = await upstream(route);
    const body = (await response.json()) as Record<string, unknown>;
    const host = Number(new URL(route.request().url()).searchParams.get("host"));
    const key = keyById.get(host);
    body.launches = (scenario.launch_history ?? []).filter((launch) => launch.host === key).map((launch) => ({
      host,
      github_repo: null,
      canonical_cwd: canonical(launch.cwd),
      cwd: launch.cwd,
      selection: {
        harness: launch.harness,
        model: launch.model ?? null,
        effort: launch.effort ?? null,
        permissions: launch.permissions ?? null,
        ...(launch.workspace_trust === undefined ? {} : { workspace_trust: launch.workspace_trust }),
      },
      created_at: now - launch.age,
      creation_seq: 1_000_000 - launch.age,
    }));
    body.folders = (scenario.folders ?? []).filter((folder) => folder.host === key).map((folder) => ({
      host,
      canonical_cwd: canonical(folder.cwd),
      canonical_proven: true,
      display_cwd: folder.cwd,
      created_at: now - folder.age,
      creation_seq: 1_000_000 - folder.age,
    }));
    await route.fulfill({ status: response.status, headers, json: body });
  });
}

/**
 * Answer the managed checkout trash listing from the scenario's
 * `checkout_trash`, for the host each request names. Opt-in per shot (see
 * `openDocsFleet`): with it, the trash button beside new shows a count, which
 * every other shot should not. The archive time, name and path are built the
 * way Farhelm records them, so the dialog shows what a real archive would.
 * Requests to delete are refused outright rather than passed on: no shot
 * empties the trash, and a fake listing must never reach real deletion. Footer
 * review holds requests above this route and drains them into this same refusal;
 * its build stamp matters even on a refusal, since the client classifies both.
 */
async function installCheckoutTrash(page: Page, request: APIRequestContext, scenario: Scenario, now: number) {
  const ids = await hostIds(request, scenario);
  // The body is staged, but the reply still belongs to this live helm. A
  // missing stamp latches build skew across every later picture on the page.
  const identity = await request.get("/api/hosts");
  expect(identity.ok(), "GET /api/hosts for staged trash identity").toBeTruthy();
  const build = requireHelmBuild(identity, "docs staged checkout trash");
  const keyById = new Map([...ids.entries()].map(([key, id]) => [id, key]));
  const stamp = (seconds: number) => new Date(seconds * 1000).toISOString().replace(/[-:]/g, "").replace(/\.\d+Z$/, "Z");
  await page.route((url) => url.pathname === "/api/checkout-trash/delete", (route) =>
    route.fulfill({ headers: { "x-farhelm-build": build }, status: 409, body: "docs capture: the staged trash cannot be emptied" }),
  );
  await page.route((url) => url.pathname === "/api/checkout-trash/list", async (route) => {
    const { host } = route.request().postDataJSON() as { host: number };
    const key = keyById.get(host);
    const checkouts = (scenario.checkout_trash ?? [])
      .filter((checkout) => checkout.host === key)
      .map((checkout) => {
        const archivedAt = now - checkout.age;
        const name = `${checkout.checkout}-${stamp(archivedAt)}`;
        return {
          id: `docs-${key}-${checkout.checkout}`,
          name,
          repository: checkout.repository,
          path: `${checkout.root}/.farhelm-trash/${name}`,
          archived_at: archivedAt,
          bytes: checkout.bytes,
        };
      });
    await route.fulfill({ headers: { "x-farhelm-build": build }, json: { checkouts, issues: [] } });
  });
}

/**
 * Answer the terminal's file check from the scenario's `terminal_files`: the
 * host it names (by its alias, as the helm reports it), the invented full
 * path, and the size. A path the scenario does not list reads as not found.
 * Downloads are refused outright, so a click in a shot fetches nothing.
 */
async function installTerminalFiles(page: Page, scenario: Scenario) {
  const alias = new Map(scenario.hosts.map((host) => [host.key, host.alias]));
  await page.route(
    (url) => /^\/api\/sessions\/[^/]+\/files\/download$/.test(url.pathname),
    (route) => route.fulfill({ status: 409, body: "docs capture: downloads are not staged" }),
  );
  await page.route(
    (url) => /^\/api\/sessions\/[^/]+\/files\/stat$/.test(url.pathname),
    async (route) => {
      const { path } = route.request().postDataJSON() as { path: string };
      const file = (scenario.terminal_files ?? []).find((candidate) => candidate.path === path);
      await route.fulfill({
        json: file
          ? { host: alias.get(file.host), path: file.shown_path, size: file.size, status: "ready" }
          : { host: "", path, size: 0, status: "not_found" },
      });
    },
  );
}

/** What a shot can ask `openDocsFleet` to stage beyond the fleet every shot sees. */
export interface DocsFleetOptions {
  /** Fill the managed checkout trash from the scenario's `checkout_trash`. */
  checkoutTrash?: boolean;
  /** Answer the terminal's file hover from the scenario's `terminal_files`. */
  terminalFiles?: boolean;
}

/**
 * Load the UI on the staged fleet with every rewrite in place: the hero's two
 * (ages and folders, installed by `openStagedFleet`) and the docs' own.
 * Development sources join that declaration so their real scratch cwd never
 * enters a pictured row; their statuses and approval replies are untouched.
 */
export async function openDocsFleet(
  page: Page,
  request: APIRequestContext,
  scenario: Scenario,
  options: DocsFleetOptions = {},
): Promise<StagedFleet> {
  const fleet = readFleet();
  if (fleet.toolbarReview) scenario.sessions.push(fleet.toolbarReview);
  if (fleet.spawnReview) scenario.sessions.push(fleet.spawnReview);
  await installDocsRewrites(page, request, scenario, fleet.now);
  if (options.checkoutTrash) await installCheckoutTrash(page, request, scenario, fleet.now);
  if (options.terminalFiles) await installTerminalFiles(page, scenario);
  await openStagedFleet(page, request, scenario, DOCS_SHOTS_DIR, fleet);
  return fleet;
}
