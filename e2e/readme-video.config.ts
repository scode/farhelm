// Playwright config for the README demo video (docs/readme-video/SPEC.md).
//
// The video's twin of readme-hero.config.ts, and separate for the same
// reasons: it is not a test, it boots its own fleet, and it produces a file
// rather than a verdict. The fleet comes from the same stack script as the
// hero screenshot (readme-hero/start-stack.sh), fed the video's own
// scenario; only the stack-info path differs, so a screenshot run and a
// video run can never read each other's state directory.
import { defineConfig, devices } from "@playwright/test";
import { STACK_INFO_ENV } from "./global-setup";
import { loadScenario, remoteDestinations, REMOTES_ENV } from "./readme-hero/scenario";
import { VIDEO_DIR, VIDEO_STACK_INFO_PATH } from "./readme-video/paths";
import { harnessStackPort, STACK_PORT_ENV } from "./stack-port";
import { AUTH_STORAGE_STATE_PATH, harnessAuthorizationHeaders } from "./tests/helpers/device-auth";

const scenario = loadScenario(VIDEO_DIR);
const stackPort = harnessStackPort();
const stackBaseURL = `http://127.0.0.1:${stackPort}`;
// Published for global-setup.ts, which runs in this same runner process
// after the config is loaded and reads the variable rather than a path.
process.env[STACK_INFO_ENV] = VIDEO_STACK_INFO_PATH;

export default defineConfig({
  testDir: "./readme-video",
  testMatch: "capture.spec.ts",
  fullyParallel: false,
  workers: 1,
  // Staging, the choreography's own waits, and encoding all happen inside
  // the one test. The staging budget alone is two minutes.
  timeout: 420_000,
  retries: 0,
  use: {
    baseURL: stackBaseURL,
    storageState: AUTH_STORAGE_STATE_PATH,
    extraHTTPHeaders: harnessAuthorizationHeaders(),
    screenshot: "off",
    // Trace screenshots come from the same per-page screencast the recorder
    // uses, and the first client's frame size wins: with them on, the video
    // came out at the trace's 800x450 instead of the viewport's size. The
    // trace keeps its actions and DOM snapshots, which is what a failed
    // beat needs.
    trace: { mode: "retain-on-failure", screenshots: false },
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
    // `exec` for the reason readme-hero.config.ts gives.
    command: "exec bash ./readme-hero/start-stack.sh",
    env: {
      [STACK_PORT_ENV]: String(stackPort),
      [REMOTES_ENV]: JSON.stringify(remoteDestinations(scenario)),
      [STACK_INFO_ENV]: VIDEO_STACK_INFO_PATH,
    },
    url: `${stackBaseURL}/`,
    reuseExistingServer: false,
    stdout: "pipe",
    stderr: "pipe",
    timeout: 60_000,
    gracefulShutdown: { signal: "SIGTERM", timeout: 5_000 },
  },
});
