/**
 * Stage a scenario's fleet and open it in the page: the part of a README
 * capture that is the same whether the result is the hero screenshot
 * (docs/readme-hero/SPEC.md) or the demo video (docs/readme-video/SPEC.md).
 *
 * What is real: the helm, every supervisor, every session, every terminal,
 * and every status the list shows, which staging waits for the supervisor
 * to classify before returning. What is rewritten in transit: each row's
 * last-activity stamp and displayed working directory, both taken from the
 * scenario (see the hero SPEC for why those two and no others).
 *
 * Shared rather than copied because both captures make the same honesty
 * claim about what is staged, and two copies of the staging would let one of
 * them quietly start rewriting more than the SPEC allows.
 */
import { expect, type APIRequestContext, type Page } from "@playwright/test";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { attachSession, cleanupSession, waitForTermText } from "../tests/helpers/term";
import { patchPreferences, SESSION_LISTING } from "../tests/helpers/fleet";
import type { Scenario, ScenarioSession } from "./scenario";

/** What the stack script (`start-stack.sh`) publishes about the fleet it booted. */
export interface StackInfo {
  farhelm: string;
  /** The `farhelm-fixtures` binary: the replay fixture runs from it. */
  fixtures: string;
  state: string;
  wrappers: string;
  work: string;
  remotes: Array<{ ssh: string; state: string }>;
}

interface HostView {
  id: number;
  kind: string;
  destination: string | null;
  alias: string | null;
  name: string;
  state: { phase: string };
}

interface SessionRow {
  id: string;
  title: string;
  cwd: string;
  status?: { state: string; exit_code?: number | null };
  last_activity_at: number;
  created_at: number;
  seen_activity_at?: number | null;
}

/** A staged fleet: session ids by scenario title, and the clock the listing rewrite uses. */
export interface StagedFleet {
  ids: Map<string, string>;
  /** Epoch seconds the scenario's `age` values count back from. */
  now: number;
  /** The id of the session the scenario marks `open: true`. */
  openId: string;
}

/** Read what the stack script booted. */
export function readStackInfo(stackInfoPath: string): StackInfo {
  return JSON.parse(readFileSync(stackInfoPath, "utf8")) as StackInfo;
}

/**
 * The `--then` mode and exit code a session needs from the replay fixture:
 * the scenario's explicit `then` when it has one, otherwise the shape that
 * holds its target status.
 */
function replayMode(session: ScenarioSession): string[] {
  const then = session.then ?? ({ running: "spin", waiting: "menu", idle: "quiet", exited: "exit" } as const)[session.status];
  return then === "exit" ? ["--then", "exit", "--exit-code", String(session.exit_code ?? 0)] : ["--then", then];
}

/** Quote one argv element for `sh`. */
function shellQuote(value: string): string {
  return `'${value.replaceAll("'", `'\\''`)}'`;
}

/**
 * Write the per-session replay script into the session's scratch working
 * directory. The session's command uses the wrapper name (`claude`
 * or `codex`, on PATH through the stack's private HOME) with optional
 * permission flags, and that wrapper
 * execs this file from `$PWD`, which is how sessions with identical
 * invocations replay different transcripts. The `"$@"` tail is where the
 * scenario's permission flags and supervisor's per-launch vendor flags
 * land; the fixture tolerates them.
 */
function writeAgentScript(info: StackInfo, scenarioDir: string, cwd: string, session: ScenarioSession): void {
  // The dialect makes the fixture's menus the shape the session's own
  // screen reader recognises as a question (see `ReplayDialect`).
  const argv = [info.fixtures, "fake-agent", "--script", "replay", "--dialect", session.wrapper];
  if (session.transcript) argv.push("--transcript", path.join(scenarioDir, session.transcript));
  argv.push(...replayMode(session));
  const file = path.join(cwd, ".hero-agent");
  writeFileSync(file, `#!/bin/sh\nexec ${argv.map(shellQuote).join(" ")} "$@"\n`, { mode: 0o700 });
  chmodSync(file, 0o700);
}

async function hosts(request: APIRequestContext): Promise<HostView[]> {
  const response = await request.get("/api/hosts");
  expect(response.ok(), "GET /api/hosts").toBeTruthy();
  return ((await response.json()) as { hosts: HostView[] }).hosts;
}

/** The helm's session listing, unrewritten: what the supervisor really reports. */
export async function sessions(request: APIRequestContext): Promise<SessionRow[]> {
  const response = await request.get("/api/sessions");
  expect(response.ok(), "GET /api/sessions").toBeTruthy();
  return ((await response.json()) as { sessions: SessionRow[] }).sessions;
}

/**
 * The last line a transcript prints before its first input-gated directive
 * or its end, whichever comes first, which is what "replayed as far as it
 * goes on its own" means in the terminal buffer. Plain `##` comments and
 * pacing directives print nothing and are skipped.
 */
export function lastTranscriptLine(scenarioDir: string, file: string): string {
  const printed: string[] = [];
  for (const line of readFileSync(path.join(scenarioDir, file), "utf8").split("\n")) {
    if (/^##@\s*(menu|prompt)\b/.test(line)) break;
    if (line.startsWith("##") || line.trim() === "") continue;
    printed.push(line.replaceAll("\\e", "\x1b").replace(/\x1b\[[0-9;]*m/g, "").trim());
  }
  const last = printed.at(-1);
  if (!last) throw new Error(`${file} prints nothing`);
  return last;
}

/** One session as the detail endpoint (`GET /api/sessions/{id}`) answers it. */
const SESSION_DETAIL = (url: URL) => /^\/api\/sessions\/[^/]+$/.test(url.pathname);

/**
 * Rewrite the two scenario-owned fields on every listing and detail reply.
 * Same shape as `hideSeenState` in the fleet helpers, including its
 * reasons for using node's own fetch and for dropping the length headers.
 * The detail route matters because the session header reads its own
 * fetch of the open session, not the list's row.
 */
async function rewriteSessions(page: Page, byTitle: Map<string, ScenarioSession>, now: number) {
  const rewrite = (row: SessionRow) => {
    const design = byTitle.get(row.title);
    if (!design) return;
    row.last_activity_at = now - design.age;
    row.cwd = design.cwd;
  };
  await page.route((url) => SESSION_LISTING(url) || SESSION_DETAIL(url), async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const url = new URL(route.request().url());
    const upstream = await fetch(url, { headers: await route.request().allHeaders() });
    const body = (await upstream.json()) as { sessions?: SessionRow[] } & Partial<SessionRow>;
    if (SESSION_LISTING(url)) {
      for (const row of body.sessions ?? []) rewrite(row);
      // The helm sorts server-side on the real stamps and the client
      // renders the reply in the order it arrives, so rewriting the stamps
      // alone would show scenario ages in the stack's own order. Re-sorting
      // by the rewritten stamp is the order the helm would have produced
      // had those stamps been real, which is the claim the rewrite makes.
      body.sessions?.sort((a, b) => b.last_activity_at - a.last_activity_at);
    } else if (typeof body.title === "string") {
      rewrite(body as SessionRow);
    }
    const headers = Object.fromEntries(upstream.headers.entries());
    delete headers["content-length"];
    delete headers["content-encoding"];
    delete headers["transfer-encoding"];
    await route.fulfill({ status: upstream.status, headers, json: body });
  });
}

/**
 * Measure the terminal the viewport gives a session, so the real sessions
 * can be created at that size. A pane created at the API's 80x24 default
 * is resized on first attach, and that resize repainted the pane while the
 * replay snapshot was being taken: the first capture attempts showed a
 * duplicated spinner line and a lost transcript line. Creating at the
 * final size means the attach that gets photographed resizes nothing.
 */
async function measureTerminal(page: Page, request: APIRequestContext, info: StackInfo, host: number) {
  const cwd = path.join(info.work, "probe");
  mkdirSync(cwd, { recursive: true });
  const created = await request.post("/api/sessions", {
    data: {
      cwd,
      command: { command: `${shellQuote(info.fixtures)} fake-agent --script basic`, yolo: false },
      title: "size probe",
      host,
    },
  });
  expect(created.ok(), `create the size probe: ${await created.text()}`).toBeTruthy();
  const id = ((await created.json()) as { id: string }).id;
  await page.goto("/");
  await attachSession(page, id);
  const read = () => page.evaluate(() => {
    const term = (window as any).__farhelmTerm;
    return { cols: (term?.cols as number) ?? 0, rows: (term?.rows as number) ?? 0 };
  });
  // xterm mounts at its 80x24 default and is fitted to the pane afterwards;
  // the viewport is far larger than that, so "bigger than the default" is
  // the fitted size.
  await expect.poll(async () => {
    const size = await read();
    return size.cols > 80 && size.rows > 24;
  }, { message: "the probe terminal must be fitted to the viewport" }).toBe(true);
  const size = await read();
  await cleanupSession(request, id);
  return size;
}

/**
 * Bring the scenario's fleet to life and wait until every session shows its
 * target status: hosts connected and aliased, sessions created at the
 * viewport's terminal size, statuses really classified, seen state set to
 * agree with the scenario's ages. Returns before anything is opened in the
 * page beyond the size probe.
 */
export async function stageFleet(
  page: Page,
  request: APIRequestContext,
  scenario: Scenario,
  scenarioDir: string,
  info: StackInfo,
): Promise<StagedFleet> {
  // Every host connected, then aliased. Waiting for `connected` rather than
  // for the row to exist is what keeps the creates below from racing the
  // ssh handshake.
  const hostIds = new Map<string, number>();
  await expect.poll(async () => {
    const view = await hosts(request);
    for (const host of scenario.hosts) {
      const match = host.kind === "local"
        ? view.find((row) => row.kind === "local")
        : view.find((row) => row.destination === host.ssh);
      if (!match || match.state.phase !== "connected") return `waiting for ${host.key}`;
      hostIds.set(host.key, match.id);
    }
    return "connected";
  }, { timeout: 60_000, message: "every scenario host must connect" }).toBe("connected");
  for (const host of scenario.hosts) {
    const response = await request.post(`/api/hosts/${hostIds.get(host.key)}/alias`, { data: { alias: host.alias } });
    expect(response.ok(), `alias ${host.key}`).toBeTruthy();
  }

  const localHost = hostIds.get(scenario.hosts.find((host) => host.kind === "local")?.key as string) as number;
  const size = await measureTerminal(page, request, info, localHost);

  // Sessions, in scenario order, each in its own scratch directory and at
  // the terminal's measured size. Each is a command launch declaring its
  // agent and stating its YOLO answer, which is what the UI's permission
  // mark shows; no listing rewrite manufactures badges.
  // start-stack.sh resolves these names to isolated transcript wrappers.
  const ids = new Map<string, string>();
  for (const [index, session] of scenario.sessions.entries()) {
    const cwd = path.join(info.work, String(index));
    mkdirSync(cwd, { recursive: true });
    writeAgentScript(info, scenarioDir, cwd, session);
    const permissionFlag = session.wrapper === "claude" ? "--dangerously-skip-permissions" : "--yolo";
    const response = await request.post("/api/sessions", {
      data: {
        cwd,
        command: {
          command: `${session.yolo ? `${session.wrapper} ${permissionFlag}` : session.wrapper} {farhelm_args}`,
          // Scenarios leave `yolo` out for an ordinary session, and the API
          // requires the answer either way.
          yolo: session.yolo === true,
          agent: session.wrapper,
        },
        title: session.title,
        host: hostIds.get(session.host),
        cols: size.cols,
        rows: size.rows,
        // Every host starts out asking before a YOLO launch. Confirming this
        // one launch is what a user clicking through that question does, and
        // unlike marking the host safe it leaves no setting behind that the
        // picture could show.
        ...(session.yolo ? { confirm_yolo: true } : {}),
      },
    });
    expect(response.ok(), `create ${session.title}: ${await response.text()}`).toBeTruthy();
    ids.set(session.title, ((await response.json()) as { id: string }).id);
  }

  // Real classification. The sampler visits every session round-robin on
  // a two-second tick and needs three quiet samples before it calls a pane
  // idle, so a fleet this size settles in well under a minute.
  await expect.poll(async () => {
    const rows = await sessions(request);
    const pending = scenario.sessions
      .filter((session) => rows.find((row) => row.id === ids.get(session.title))?.status?.state !== session.status)
      .map((session) => `${session.title}=${rows.find((row) => row.id === ids.get(session.title))?.status?.state}`);
    return pending.length === 0 ? "settled" : pending.join(", ");
  }, { timeout: 120_000, intervals: [2_000], message: "every session must reach its target status" }).toBe("settled");

  // Seen state through the API, consistent with the ages the listing will
  // show: a seen row's stamp equals its rewritten activity, an unseen row
  // has none. The open session is marked seen by the UI when it opens.
  const now = Math.floor(Date.now() / 1000);
  for (const session of scenario.sessions) {
    if (session.status !== "idle") continue;
    const response = await request.put(`/api/sessions/${ids.get(session.title)}/seen`, {
      data: { seen_activity_at: session.seen ? now - session.age : null },
    });
    expect(response.ok(), `seen ${session.title}`).toBeTruthy();
  }

  const open = scenario.sessions.find((session) => session.open) as ScenarioSession;
  return { ids, now, openId: ids.get(open.title) as string };
}

/**
 * Load the UI on the staged fleet with the scenario's open session showing
 * its replayed transcript and every row's permission glyph verified.
 */
export async function openStagedFleet(
  page: Page,
  request: APIRequestContext,
  scenario: Scenario,
  scenarioDir: string,
  fleet: StagedFleet,
): Promise<void> {
  // Open the flagged session by preference before the page loads, so the
  // page's own auto-select never lands on an idle row and marks it seen.
  await patchPreferences(request, { list_sort: "activity", last_selected: fleet.openId, compact: false });

  await rewriteSessions(page, new Map(scenario.sessions.map((session) => [session.title, session])), fleet.now);
  await page.goto("/");
  await attachSession(page, fleet.openId);
  const open = scenario.sessions.find((session) => session.open) as ScenarioSession;
  if (open.transcript) await waitForTermText(page, lastTranscriptLine(scenarioDir, open.transcript), 30_000);
  for (const session of scenario.sessions) {
    const row = page.locator(`[data-session-id="${fleet.ids.get(session.title)}"]`);
    await expect(row).toBeVisible();
    // Command launches show the YOLO answer they were created with. Pin the
    // exact mark before capture instead of merely checking for YOLO.
    await expect(row.locator(".permission-glyph")).toHaveCount(1);
    await expect(row.locator(".permission-glyph")).toHaveAttribute("data-glyph", session.yolo ? "yolo" : "shielded");
  }
}
