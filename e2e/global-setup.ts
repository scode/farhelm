// Authenticate the browser projects once per stack against the real harness
// helm.
//
// start-stack.sh uses the same CLI and exchange for its curl bootstrap, but
// browser state is minted independently here. The resulting device secret is
// persisted once for both sides of Playwright's transport: storageState
// supplies page localStorage, and the config reads the same file for request
// headers when each worker starts.
import { type APIRequestContext, expect, FullConfig, request } from "@playwright/test";
import { execFile } from "node:child_process";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import {
  AUTH_STORAGE_STATE_PATH,
  DEVICE_SECRET_KEY,
} from "./tests/helpers/device-auth";
import { type SuiteTerminals } from "./suite-terminals";
import { suiteTerminalsOf } from "./tests/helpers/host-premise";

const run = promisify(execFile);

interface StackInfo {
  farhelm: string;
  state: string;
}

/**
 * Where a stack script publishes what it booted. The browser suite hands
 * each slot's `start-stack.sh` its own file through this variable
 * (stack-slots.ts); a run of the script by hand without it writes
 * `e2e/.stack-info.json`. A config that boots a different stack (the README
 * hero capture, `readme-hero.config.ts`) names its own file the same way, so
 * no two stacks read each other's state directory. Those configs also
 * publish the variable into the runner's own environment at load time, which
 * is where this setup reads it for them.
 */
export const STACK_INFO_ENV = "FARHELM_E2E_STACK_INFO";

/** One stack global setup must authenticate, as the suite's config describes it. */
interface StackTarget {
  baseURL: string;
  stackInfoPath: string;
  authStatePath: string;
}

/** Authenticate and mark the shared welcome seen before any harness page opens.
 * All four capture/test configs use this setup, so a fresh helm's introduction
 * cannot cover their UI. The welcome's own tests explicitly restore unseen.
 *
 * The browser suite boots one stack per worker slot and lists them in its
 * config's `metadata.stackSlots`; each gets its own credential. The
 * single-stack capture configs list nothing and keep their one stack-info
 * file (named by {@link STACK_INFO_ENV}) and the shared credential path.
 */
export default async function globalSetup(config: FullConfig) {
  const slots = (config.metadata as { stackSlots?: StackTarget[] }).stackSlots;
  // Only the main config names its run's terminals in its metadata; a
  // capture config's runner that merely inherited the variable has none, and
  // its stacks are not checked.
  const terminals = suiteTerminalsOf(config);
  if (terminals) console.log(`suite terminals: ${terminals}`);
  if (slots) {
    for (const slot of slots) await authenticateStack(slot, terminals);
    return;
  }
  const baseURL = config.projects[0]?.use.baseURL as string | undefined;
  if (!baseURL) throw new Error("the Playwright project has no baseURL");
  await authenticateStack({
    baseURL,
    stackInfoPath: process.env[STACK_INFO_ENV] || path.join(__dirname, ".stack-info.json"),
    authStatePath: AUTH_STORAGE_STATE_PATH,
  });
}

/** Exchange one stack's harness token for a device secret and persist it for its workers. */
async function authenticateStack(
  { baseURL, stackInfoPath, authStatePath }: StackTarget,
  terminals: SuiteTerminals | null = null,
) {
  const info = JSON.parse(await readFile(stackInfoPath, "utf8")) as StackInfo;
  const { stdout } = await run(info.farhelm, [
    "helm",
    "token",
    "show",
    "--state-dir",
    info.state,
  ]);
  const token = stdout.trim();
  if (!token) throw new Error("farhelm helm token show returned an empty token");

  const client = await request.newContext({ baseURL });
  const exchanged = await client.post("/api/auth/token", { data: { token } });
  expect(exchanged, "the harness token must exchange before tests start").toBeOK();
  const body = (await exchanged.json()) as { device_secret?: unknown };
  if (typeof body.device_secret !== "string" || body.device_secret === "") {
    throw new Error("the harness token exchange returned no device secret");
  }

  const seen = await client.put("/api/preferences", {
    headers: { Authorization: `Bearer ${body.device_secret}` },
    data: { welcome_seen: true },
  });
  expect(seen, "the welcome must be marked seen before harness pages open").toBeOK();
  if (terminals) await expectStackTerminals(client, body.device_secret, baseURL, terminals);

  const directory = path.dirname(authStatePath);
  await mkdir(directory, { recursive: true, mode: 0o700 });
  await chmod(directory, 0o700);
  const state = await client.storageState();
  state.origins = [{
    origin: new URL(baseURL).origin,
    localStorage: [{ name: DEVICE_SECRET_KEY, value: body.device_secret }],
  }];
  await writeFile(authStatePath, JSON.stringify(state), { mode: 0o600 });
  await chmod(authStatePath, 0o600);
  await client.dispose();
}

/**
 * The main browser suite's stack really creates the run's kind of terminal
 * (suite-terminals.ts), checked once per stack from the helm's listing of its
 * startup session before any spec runs: a terminal host with the switch on
 * for the ordinary run, tmux with the switch off for the old-terminal set.
 *
 * Why here: every spec helper reads whichever component a terminal holds, so
 * a stack that silently kept tmux (a dropped `--new-terminals host`, a broken
 * stack-script branch) would pass the ordinary run on xterm.js, and one that
 * silently made hosts would fail the old-terminal set in confusing ways. The
 * ordinary run's attachments are checked too (tests/helpers/host-premise.ts);
 * this stops a wrong stack before any of them starts.
 */
async function expectStackTerminals(
  client: APIRequestContext,
  secret: string,
  baseURL: string,
  terminals: SuiteTerminals,
) {
  // Playwright's webServer probe sees the helm before start-stack.sh finishes
  // creating its startup session. Wait for that creation, then judge its kind:
  // an existing session of the wrong kind remains an immediate setup failure.
  let startup: Record<string, unknown> | undefined;
  await expect.poll(async () => {
    const listed = await client.get("/api/sessions", { headers: { Authorization: `Bearer ${secret}` } });
    expect(listed, "the session listing must answer").toBeOK();
    const sessions = ((await listed.json()) as { sessions: Record<string, unknown>[] }).sessions;
    startup = sessions.find((session) => session.title === "e2e-session");
    return startup !== undefined;
  }, { timeout: 30_000, message: `the stack at ${baseURL} must list its startup session` }).toBe(true);
  // An old row may omit the switch rather than report it off.
  expect(
    {
      agent_terminal: startup!.agent_terminal,
      supervisor_creates_host_terminals: startup!.supervisor_creates_host_terminals ?? false,
    },
    `the ${terminals === "host" ? "new" : "old"}-terminal run's stack at ${baseURL} must create ` +
      `${terminals === "host" ? "terminals in terminal hosts" : "tmux terminals"}`,
  ).toEqual({ agent_terminal: terminals, supervisor_creates_host_terminals: terminals === "host" });
}
