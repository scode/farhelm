// Playwright config for the README hero capture (docs/readme-hero/SPEC.md).
//
// Separate from playwright.config.ts on purpose: that config discovers every
// spec under tests/ and runs each against the shared one-session stack, and
// the capture is neither a test nor something that can run against that
// stack. It boots its own fleet (readme-hero/start-stack.sh, one supervisor
// per scenario host), runs exactly one spec in Chromium, and produces a PNG
// rather than a verdict.
//
// Everything shared with the ordinary suite is imported rather than copied:
// the port choice, the auth handoff, and the global setup. The stack-info
// path is the one thing that differs, so the two stacks can never read each
// other's state directory.
import { defineConfig, devices } from "@playwright/test";
import { STACK_INFO_ENV } from "./global-setup";
import { loadScenario, remoteDestinations, REMOTES_ENV, STACK_INFO_PATH } from "./readme-hero/scenario";
import { harnessStackPort, STACK_PORT_ENV } from "./stack-port";
import { AUTH_STORAGE_STATE_PATH, harnessAuthorizationHeaders } from "./tests/helpers/device-auth";

const scenario = loadScenario();
const stackPort = harnessStackPort();
const stackBaseURL = `http://127.0.0.1:${stackPort}`;
// Published for global-setup.ts, which runs in this same runner process
// after the config is loaded and reads the variable rather than a path.
process.env[STACK_INFO_ENV] = STACK_INFO_PATH;

export default defineConfig({
  testDir: "./readme-hero",
  testMatch: "capture.spec.ts",
  fullyParallel: false,
  workers: 1,
  // Staging seven sessions and waiting for real classification is slower
  // than any single test in the ordinary suite; the wait budget lives in
  // the spec, this only has to be larger than it.
  timeout: 180_000,
  retries: 0,
  use: {
    baseURL: stackBaseURL,
    storageState: AUTH_STORAGE_STATE_PATH,
    extraHTTPHeaders: harnessAuthorizationHeaders(),
    screenshot: "off",
    trace: "retain-on-failure",
  },
  globalSetup: "./global-setup.ts",
  projects: [{
    name: "chromium",
    use: {
      ...devices["Desktop Chrome"],
      viewport: { width: scenario.viewport.width, height: scenario.viewport.height },
      deviceScaleFactor: scenario.viewport.scale,
    },
  }],
  webServer: {
    // `exec` for the same reason the ordinary config gives: the orphan
    // watcher in the stack script waits on its parent pid, and a dash
    // wrapper that outlives the Playwright leader would defeat it.
    command: "exec bash ./readme-hero/start-stack.sh",
    env: {
      [STACK_PORT_ENV]: String(stackPort),
      [REMOTES_ENV]: JSON.stringify(remoteDestinations(scenario)),
      [STACK_INFO_ENV]: STACK_INFO_PATH,
    },
    url: `${stackBaseURL}/`,
    reuseExistingServer: false,
    stdout: "pipe",
    stderr: "pipe",
    timeout: 60_000,
    gracefulShutdown: { signal: "SIGTERM", timeout: 5_000 },
  },
});
