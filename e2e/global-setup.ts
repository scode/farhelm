// Authenticate the browser projects once against the real harness helm.
//
// start-stack.sh uses the same CLI and exchange for its curl bootstrap, but
// browser state is minted independently here. The resulting device secret is
// handed to both sides of Playwright's transport: storageState supplies page
// localStorage, while the process environment supplies request headers when
// the config is reloaded in each worker.
import { expect, FullConfig, request } from "@playwright/test";
import { execFile } from "node:child_process";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import {
  AUTH_STORAGE_STATE_PATH,
  DEVICE_SECRET_KEY,
  exposeHarnessDeviceSecret,
} from "./tests/helpers/device-auth";

const run = promisify(execFile);

interface StackInfo {
  farhelm: string;
  state: string;
}

/** Mint one device identity and publish it to every Playwright transport. */
export default async function globalSetup(config: FullConfig) {
  const info = JSON.parse(
    await readFile(path.join(__dirname, ".stack-info.json"), "utf8"),
  ) as StackInfo;
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
  exposeHarnessDeviceSecret(body.device_secret);
  await client.dispose();
}
