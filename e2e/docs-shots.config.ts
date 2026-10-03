// Playwright config for the docs website's screenshots (docs/docs-shots/SPEC.md).
//
// A sibling of readme-hero.config.ts and readme-video.config.ts, separate for
// the same reasons: it is not a test, it boots its own fleet, and it produces
// files rather than a verdict. The fleet comes from the same stack script,
// fed the docs scenario and its own stack-info path, so a docs run and a
// README run never read each other's state.
//
// The fleet is staged once, by the `stage` project, and every docs page's
// shots run afterwards against it as the `shots` project. Selecting one page
// (`scripts/docs-screenshots.sh --only <page>`) still runs the staging first,
// because Playwright always runs a selected project's dependencies.
import { defineConfig, devices } from "@playwright/test";
import { STACK_INFO_ENV } from "./global-setup";
import { loadScenario, remoteDestinations, REMOTES_ENV } from "./readme-hero/scenario";
import { DOCS_SHOTS_DIR, DOCS_STACK_INFO_PATH } from "./docs-shots/paths";
import { harnessStackPort, STACK_PORT_ENV } from "./stack-port";
import { AUTH_STORAGE_STATE_PATH, harnessAuthorizationHeaders } from "./tests/helpers/device-auth";

const scenario = loadScenario(DOCS_SHOTS_DIR);
const stackPort = harnessStackPort();
const stackBaseURL = `http://127.0.0.1:${stackPort}`;
// Published for global-setup.ts, which runs in this same runner process
// after the config is loaded and reads the variable rather than a path.
process.env[STACK_INFO_ENV] = DOCS_STACK_INFO_PATH;

const browser = {
  ...devices["Desktop Chrome"],
  viewport: { width: scenario.viewport.width, height: scenario.viewport.height },
  deviceScaleFactor: scenario.viewport.scale,
};

export default defineConfig({
  testDir: "./docs-shots",
  fullyParallel: false,
  workers: 1,
  // The staging test waits up to two minutes for classification on its own.
  timeout: 240_000,
  retries: 0,
  use: {
    baseURL: stackBaseURL,
    storageState: AUTH_STORAGE_STATE_PATH,
    extraHTTPHeaders: harnessAuthorizationHeaders(),
    screenshot: "off",
    trace: "retain-on-failure",
  },
  globalSetup: "./global-setup.ts",
  projects: [
    { name: "stage", testMatch: "stage.setup.ts", use: browser },
    { name: "shots", testMatch: "*.spec.ts", dependencies: ["stage"], use: browser },
  ],
  webServer: {
    // `exec` for the reason readme-hero.config.ts gives.
    command: "exec bash ./readme-hero/start-stack.sh",
    env: {
      [STACK_PORT_ENV]: String(stackPort),
      [REMOTES_ENV]: JSON.stringify(remoteDestinations(scenario)),
      [STACK_INFO_ENV]: DOCS_STACK_INFO_PATH,
    },
    url: `${stackBaseURL}/`,
    reuseExistingServer: false,
    stdout: "pipe",
    stderr: "pipe",
    timeout: 60_000,
    gracefulShutdown: { signal: "SIGTERM", timeout: 5_000 },
  },
});
