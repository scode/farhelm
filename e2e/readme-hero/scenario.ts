// A staged-fleet scenario as TypeScript sees it: one parser for
// `docs/readme-hero/scenario.json5`, `docs/readme-video/scenario.json5`, and
// `docs/docs-shots/scenario.json5`,
// shared by each Playwright config (which needs the host list and viewport
// before the stack boots) and each capture spec (which needs everything).
// One reader means a design file cannot mean two different things to its
// consumers, and the screenshot and the video cannot drift into two dialects
// of the same format.
//
// Validation is deliberately strict and loud. The scenario is hand-edited by
// a maintainer, and a typo that silently dropped a session or misread a
// status would produce a picture that looks plausible and is wrong.
import JSON5 from "json5";
import { readFileSync } from "node:fs";
import path from "node:path";

/** Where the hero screenshot's design lives, relative to this file. */
export const SCENARIO_DIR = path.resolve(__dirname, "../../docs/readme-hero");
/** Where the hero stack script publishes what it booted. Gitignored, per run. */
export const STACK_INFO_PATH = path.join(__dirname, ".stack-info.json");
/** The config's handoff of ssh destinations to the stack script: a JSON array. */
export const REMOTES_ENV = "FARHELM_HERO_REMOTES";

export type HostKind = "local" | "remote";
export type TargetStatus = "running" | "waiting" | "idle" | "exited";
/** The replay fixture's `--then` shapes; see `ReplayThen` in the fixture. */
export type ReplayThen = "spin" | "menu" | "quiet" | "exit";
export type Wrapper = "claude" | "codex";

export interface ScenarioHost {
  key: string;
  alias: string;
  kind: HostKind;
  /** The ssh destination, `$USER` already expanded. Remote hosts only. */
  ssh?: string;
  /**
   * The destination the UI is shown instead of `ssh` (docs screenshots only,
   * see docs/docs-shots/SPEC.md). The real one is a self-ssh spelling that may
   * name the capturing machine's account, which must never be photographed.
   */
  shown_ssh?: string;
}

/** The structured harness names the launcher's history speaks (`LaunchHarness` in farhelm-proto). */
export type LaunchHarness = "codex" | "claude" | "muse" | "cursor" | "grok" | "goose" | "pi" | "omp" | "open_code";

/**
 * One remembered launch, shown in the session launcher's recent setups
 * (docs screenshots only). A fresh stack has no launch history, and staged
 * sessions are not structured launches, so the docs capture rewrites the
 * history reply from these instead; see docs/docs-shots/SPEC.md.
 */
export interface ScenarioLaunch {
  host: string;
  cwd: string;
  harness: LaunchHarness;
  model?: string;
  effort?: string;
  permissions?: string;
  workspace_trust?: boolean;
  /** Seconds before the capture that this launch happened; orders the history. */
  age: number;
}

/** One remembered folder, offered under the launcher's folder field (docs screenshots only). */
export interface ScenarioFolder {
  host: string;
  cwd: string;
  age: number;
}

export interface ScenarioSession {
  title: string;
  host: string;
  cwd: string;
  wrapper: Wrapper;
  /** Use the harness's permission-bypass flag in the real staged invocation. */
  yolo?: boolean;
  status: TargetStatus;
  seen?: boolean;
  exit_code?: number;
  age: number;
  open?: boolean;
  /** Transcript file name, relative to the scenario directory. */
  transcript?: string;
  /**
   * What the fake agent does once its transcript ends, when that differs
   * from what `status` implies. `status` is the classification the staging
   * waits for; for a transcript that stops mid-way on a `##@` directive
   * (the demo video's interactive sessions), that is not what happens after
   * the end, so the end shape is named separately.
   */
  then?: ReplayThen;
}

export interface Scenario {
  viewport: { width: number; height: number; scale: number };
  hosts: ScenarioHost[];
  sessions: ScenarioSession[];
  launch_history?: ScenarioLaunch[];
  folders?: ScenarioFolder[];
}

const STATUSES: TargetStatus[] = ["running", "waiting", "idle", "exited"];
const THENS: ReplayThen[] = ["spin", "menu", "quiet", "exit"];
const WRAPPERS: Wrapper[] = ["claude", "codex"];
const HARNESSES: LaunchHarness[] = ["codex", "claude", "muse", "cursor", "grok", "goose", "pi", "omp", "open_code"];
const EFFORTS = ["off", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"];
const PERMISSIONS = ["yolo", "approve", "smart_approve", "chat"];

/** Read and validate `scenario.json5` in `dir`, expanding `$USER` in ssh destinations.
 *
 * `$USER` is the one substitution allowed, because a self-ssh spelling that
 * names the account is the cheapest second destination a machine has, and
 * the account name must not be written into a public file.
 */
export function loadScenario(dir: string = SCENARIO_DIR): Scenario {
  const scenarioPath = path.join(dir, "scenario.json5");
  // Annotated rather than inferred: TypeScript only narrows on a call to a
  // `never`-returning function when the callee's own declaration says so.
  const fail: (message: string) => never = (message) => {
    throw new Error(`${scenarioPath}: ${message}`);
  };
  const raw = JSON5.parse(readFileSync(scenarioPath, "utf8")) as Record<string, unknown>;
  const viewport = raw.viewport as Scenario["viewport"] | undefined;
  if (
    !viewport || !Number.isInteger(viewport.width) || !Number.isInteger(viewport.height) ||
    !Number.isInteger(viewport.scale) || viewport.scale < 1
  ) {
    fail("viewport needs integer width, height, and scale >= 1");
  }

  const user = process.env.USER;
  const hosts = (raw.hosts as ScenarioHost[] | undefined) ?? [];
  if (hosts.length === 0) fail("at least one host is required");
  const keys = new Set<string>();
  const destinations = new Set<string>();
  let locals = 0;
  for (const host of hosts) {
    if (!host.key || !host.alias) fail(`host ${JSON.stringify(host)} needs key and alias`);
    if (keys.has(host.key)) fail(`duplicate host key ${host.key}`);
    keys.add(host.key);
    if (host.kind === "local") {
      locals += 1;
      continue;
    }
    if (host.kind !== "remote") fail(`host ${host.key}: kind must be local or remote`);
    if (!host.ssh) fail(`host ${host.key}: remote hosts need an ssh destination`);
    if (host.ssh.includes("$USER")) {
      if (!user) fail(`host ${host.key} uses $USER but USER is not set`);
      host.ssh = host.ssh.replaceAll("$USER", user);
    }
    if (destinations.has(host.ssh)) fail(`two remote hosts share the destination ${host.ssh}`);
    destinations.add(host.ssh);
    if (host.shown_ssh !== undefined && (typeof host.shown_ssh !== "string" || host.shown_ssh.includes("$USER"))) {
      fail(`host ${host.key}: shown_ssh must be a literal destination (it is what gets photographed)`);
    }
  }
  if (locals !== 1) fail("exactly one host must be kind: local");

  const sessions = (raw.sessions as ScenarioSession[] | undefined) ?? [];
  if (sessions.length === 0) fail("at least one session is required");
  let open = 0;
  let previousAge = -1;
  for (const session of sessions) {
    if (!session.title) fail("every session needs a title");
    if (!keys.has(session.host)) fail(`session ${session.title}: unknown host ${session.host}`);
    if (!session.cwd) fail(`session ${session.title}: cwd is required`);
    if (!WRAPPERS.includes(session.wrapper)) fail(`session ${session.title}: wrapper must be claude or codex`);
    if (session.yolo !== undefined && typeof session.yolo !== "boolean") {
      fail(`session ${session.title}: yolo must be a boolean`);
    }
    if (!STATUSES.includes(session.status)) fail(`session ${session.title}: unknown status ${session.status}`);
    if (session.then !== undefined && !THENS.includes(session.then)) {
      fail(`session ${session.title}: then must be one of ${THENS.join(", ")}`);
    }
    if (session.status === "idle" && typeof session.seen !== "boolean") {
      fail(`session ${session.title}: idle sessions need seen: true or false`);
    }
    if (!Number.isInteger(session.age) || session.age < 0) fail(`session ${session.title}: age must be a non-negative integer`);
    // The list sorts by recent activity, so a scenario whose ages disagree
    // with its order would draw rows in an order the file does not show.
    if (session.age < previousAge) fail(`session ${session.title}: ages must be non-decreasing down the list`);
    previousAge = session.age;
    if (session.open) open += 1;
  }
  if (open !== 1) fail("exactly one session must carry open: true");

  const launchHistory = (raw.launch_history as ScenarioLaunch[] | undefined) ?? [];
  for (const launch of launchHistory) {
    const name = `launch_history entry ${JSON.stringify(launch)}`;
    if (!keys.has(launch.host)) fail(`${name}: unknown host`);
    if (!launch.cwd) fail(`${name}: cwd is required`);
    if (!HARNESSES.includes(launch.harness)) fail(`${name}: harness must be one of ${HARNESSES.join(", ")}`);
    if (launch.effort !== undefined && !EFFORTS.includes(launch.effort)) fail(`${name}: unknown effort`);
    if (launch.permissions !== undefined && !PERMISSIONS.includes(launch.permissions)) fail(`${name}: unknown permissions`);
    if (launch.workspace_trust !== undefined && typeof launch.workspace_trust !== "boolean") {
      fail(`${name}: workspace_trust must be a boolean`);
    }
    if (!Number.isInteger(launch.age) || launch.age < 0) fail(`${name}: age must be a non-negative integer`);
  }
  const folders = (raw.folders as ScenarioFolder[] | undefined) ?? [];
  for (const folder of folders) {
    const name = `folders entry ${JSON.stringify(folder)}`;
    if (!keys.has(folder.host)) fail(`${name}: unknown host`);
    if (!folder.cwd) fail(`${name}: cwd is required`);
    if (!Number.isInteger(folder.age) || folder.age < 0) fail(`${name}: age must be a non-negative integer`);
  }

  return { viewport, hosts, sessions, launch_history: launchHistory, folders };
}

/** The ssh destinations of the remote hosts, in scenario order. */
export function remoteDestinations(scenario: Scenario): string[] {
  return scenario.hosts.filter((host) => host.kind === "remote").map((host) => host.ssh as string);
}
