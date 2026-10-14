// Authenticate the browser projects once against the real harness helm.
//
// start-stack.sh uses the same CLI and exchange for its curl bootstrap, but
// browser state is minted independently here. The resulting device secret is
// persisted once for both sides of Playwright's transport: storageState
// supplies page localStorage, and the config reads the same file for request
// headers when each worker starts.
import { expect, FullConfig, request } from "@playwright/test";
import { execFile } from "node:child_process";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import {
  AUTH_STORAGE_STATE_PATH,
  DEVICE_SECRET_KEY,
} from "./tests/helpers/device-auth";

const run = promisify(execFile);

interface StackInfo {
  farhelm: string;
  state: string;
}

/**
 * Where the stack script published what it booted. The ordinary suite's
 * `start-stack.sh` writes `e2e/.stack-info.json`; a config that boots a
 * different stack (the README hero capture, `readme-hero.config.ts`) names
 * its own file through this variable so the two never read each other's
 * state directory. That config publishes the variable into the runner's
 * own environment at load time, which is where this setup reads it.
 */
export const STACK_INFO_ENV = "FARHELM_E2E_STACK_INFO";

/** Authenticate and mark the shared welcome seen before any harness page opens.
 * All four capture/test configs use this setup, so a fresh helm's introduction
 * cannot cover their UI. The welcome's own tests explicitly restore unseen.
 */
export default async function globalSetup(config: FullConfig) {
  const infoPath = process.env[STACK_INFO_ENV] || path.join(__dirname, ".stack-info.json");
  const info = JSON.parse(await readFile(infoPath, "utf8")) as StackInfo;
  const { stdout } = await run(info.farhelm, [
    "helm",
    "token",
    "show",
    "--state-dir",
    info.state,
  ]);
  const token = stdout.trim();
  if (!token) throw new Error("farhelm helm token show returned an empty token");

  const baseURL = config.projects[0]?.use.baseURL as string | undefined;
  if (!baseURL) throw new Error("the Playwright project has no baseURL");
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

  const directory = path.dirname(AUTH_STORAGE_STATE_PATH);
  await mkdir(directory, { recursive: true, mode: 0o700 });
  await chmod(directory, 0o700);
  const state = await client.storageState();
  state.origins = [{
    origin: new URL(baseURL).origin,
    localStorage: [{ name: DEVICE_SECRET_KEY, value: body.device_secret }],
  }];
  await writeFile(AUTH_STORAGE_STATE_PATH, JSON.stringify(state), { mode: 0o600 });
  await chmod(AUTH_STORAGE_STATE_PATH, 0o600);
  await client.dispose();
}
