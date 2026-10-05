/**
 * The docs screenshots' layer over the README staging (docs/docs-shots/SPEC.md):
 * the docs overlay theme, the staged fleet handed from the staging project to
 * the shot projects, and the three extra in-transit rewrites the docs need on
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
import { readFileSync, writeFileSync } from "node:fs";
import type { OverlayTheme } from "../readme-video/overlay";
import type { Scenario } from "../readme-hero/scenario";
import { openStagedFleet, type StagedFleet } from "../readme-hero/stage";
import { DOCS_FLEET_PATH, DOCS_SHOTS_DIR } from "./paths";

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
}

/** Hand the staged fleet to the shot projects, which run as separate tests. */
export function writeFleet(fleet: StagedFleet): void {
  const file: FleetFile = { ids: [...fleet.ids.entries()], now: fleet.now, openId: fleet.openId };
  writeFileSync(DOCS_FLEET_PATH, JSON.stringify(file));
}

/** Read the fleet the staging project staged in this run. */
export function readFleet(): StagedFleet {
  const file = JSON.parse(readFileSync(DOCS_FLEET_PATH, "utf8")) as FleetFile;
  return { ids: new Map(file.ids), now: file.now, openId: file.openId };
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
 * Load the UI on the staged fleet with every rewrite in place: the hero's two
 * (ages and folders, installed by `openStagedFleet`) and the docs' own.
 */
export async function openDocsFleet(page: Page, request: APIRequestContext, scenario: Scenario): Promise<StagedFleet> {
  const fleet = readFleet();
  await installDocsRewrites(page, request, scenario, fleet.now);
  await openStagedFleet(page, request, scenario, DOCS_SHOTS_DIR, fleet);
  return fleet;
}
