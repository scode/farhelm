/**
 * Stage the README hero fleet and photograph it (docs/readme-hero/SPEC.md).
 *
 * Not a test in the ordinary sense: it produces a PNG, not a verdict, and
 * nothing in CI runs it. It still uses Playwright's test runner because
 * that is what already knows how to boot the stack, authenticate, drive
 * the real UI, and wait on the terminal buffer.
 *
 * What is real: the helm, every supervisor, every session, every terminal,
 * and every status the list shows, which the spec waits for the supervisor
 * to classify before shooting. What is rewritten in transit: each row's
 * last-activity stamp and displayed working directory, both taken from the
 * scenario (see the SPEC for why those two and no others).
 */
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { attachSession, cleanupSession, waitForTermText } from "../tests/helpers/term";
import { patchPreferences, SESSION_LISTING } from "../tests/helpers/fleet";
import { loadScenario, SCENARIO_DIR, STACK_INFO_PATH, type ScenarioSession } from "./scenario";

/** Where the PNG goes. The capture script sets it; a bare run lands under target/. */
const OUTPUT_ENV = "FARHELM_HERO_OUTPUT";

interface StackInfo {
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

/** The `--then` mode and exit code a target status needs from the replay fixture. */
function replayMode(session: ScenarioSession): string[] {
  switch (session.status) {
    case "running": return ["--then", "spin"];
    case "waiting": return ["--then", "menu"];
    case "idle": return ["--then", "quiet"];
    case "exited": return ["--then", "exit", "--exit-code", String(session.exit_code ?? 0)];
  }
}

/** Quote one argv element for `sh`. */
function shellQuote(value: string): string {
  return `'${value.replaceAll("'", `'\\''`)}'`;
}

/**
 * Write the per-session replay script into the session's scratch working
 * directory. The session's invocation uses the wrapper name (`claude`
 * or `codex`, on PATH through the stack's private HOME) with optional
 * permission flags, and that wrapper
 * execs this file from `$PWD`, which is how sessions with identical
 * invocations replay different transcripts. The `"$@"` tail is where the
 * scenario's permission flags and supervisor's per-launch vendor flags
 * land; the fixture tolerates them.
 */
function writeAgentScript(info: StackInfo, cwd: string, session: ScenarioSession): void {
  const argv = [info.fixtures, "fake-agent", "--script", "replay"];
  if (session.transcript) argv.push("--transcript", path.join(SCENARIO_DIR, session.transcript));
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

async function sessions(request: APIRequestContext): Promise<SessionRow[]> {
  const response = await request.get("/api/sessions");
  expect(response.ok(), "GET /api/sessions").toBeTruthy();
  return ((await response.json()) as { sessions: SessionRow[] }).sessions;
}

/** The last line the transcript prints, which is what "replayed to the end" means in the terminal buffer. */
function lastTranscriptLine(file: string): string {
  const lines = readFileSync(path.join(SCENARIO_DIR, file), "utf8")
    .split("\n")
    .filter((line) => !line.startsWith("##") && line.trim() !== "")
    .map((line) => line.replaceAll("\\e", "\x1b").replace(/\x1b\[[0-9;]*m/g, "").trim());
  const last = lines.at(-1);
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
    data: { cwd, invocation: `${shellQuote(info.fixtures)} fake-agent --script basic`, title: "size probe", host },
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

test("capture the README hero", async ({ page, request }) => {
  const scenario = loadScenario();
  const info = JSON.parse(readFileSync(STACK_INFO_PATH, "utf8")) as StackInfo;
  const output = process.env[OUTPUT_ENV] ||
    path.resolve(__dirname, "../../target/readme-hero/readme-hero.png");

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
  // the terminal's measured size. Permission flags reach the stored invocation
  // so the UI derives its own glyph; no listing rewrite manufactures badges.
  // start-stack.sh resolves these names to isolated transcript wrappers.
  const ids = new Map<string, string>();
  for (const [index, session] of scenario.sessions.entries()) {
    const cwd = path.join(info.work, String(index));
    mkdirSync(cwd, { recursive: true });
    writeAgentScript(info, cwd, session);
    const permissionFlag = session.wrapper === "claude" ? "--dangerously-skip-permissions" : "--yolo";
    const response = await request.post("/api/sessions", {
      data: {
        cwd,
        invocation: session.yolo ? `${session.wrapper} ${permissionFlag}` : session.wrapper,
        title: session.title,
        host: hostIds.get(session.host),
        agent_kind: session.wrapper,
        cols: size.cols,
        rows: size.rows,
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

  // Open the flagged session by preference before the page loads, so the
  // page's own auto-select never lands on an idle row and marks it seen.
  const open = scenario.sessions.find((session) => session.open) as ScenarioSession;
  const openId = ids.get(open.title) as string;
  await patchPreferences(request, { list_sort: "activity", last_selected: openId, compact: false });

  await rewriteSessions(page, new Map(scenario.sessions.map((session) => [session.title, session])), now);
  await page.goto("/");
  await attachSession(page, openId);
  if (open.transcript) await waitForTermText(page, lastTranscriptLine(open.transcript), 30_000);
  for (const session of scenario.sessions) {
    const row = page.locator(`[data-session-id="${ids.get(session.title)}"]`);
    await expect(row).toBeVisible();
    // Verify both marked and ordinary rows before photographing them. Otherwise
    // a broken flag-to-glyph mapping could silently publish the wrong design.
    await expect(row.locator('[data-glyph="yolo"]')).toHaveCount(session.yolo ? 1 : 0);
  }
  // Park the pointer where nothing has a hover state.
  await page.mouse.move(0, 0);
  // One settle beat so the terminal's last paint and the list's status
  // dots are on screen together; nothing here is a readiness oracle, the
  // waits above were.
  await page.waitForTimeout(1_500); // sleep-ok: observation window after every readiness wait passed

  mkdirSync(path.dirname(output), { recursive: true });
  await page.screenshot({ path: output, fullPage: false });
  console.log(`README hero written to ${output}`);
});
