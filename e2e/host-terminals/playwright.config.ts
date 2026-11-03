// Playwright config for the browser specs of terminals in terminal hosts,
// against a real stack whose local supervisor creates new terminals in hosts.
//
// Separate from playwright.config.ts because the switch belongs to a whole
// supervisor, not to one spec: the main suite's stacks keep their
// supervisors on tmux (start-stack.sh passes `--new-terminals tmux` unless
// asked otherwise), and every spec there was written against it. This config
// boots one stack of its own through the same start-stack.sh, asking it for
// `farhelm supervisor run --new-terminals host` (FARHELM_E2E_NEW_TERMINALS),
// so the startup session and every session these specs create run in
// terminal hosts and reach the browser as grid streams. Its specs stay out of
// the main config's discovery (that config's `testDir` is `tests/`), as the
// static grid-client specs do; both engines run, WebKit standing in for the
// desktop app's.
//
// One stack and one worker: the specs share the stack's startup session and
// run one at a time. Everything else is the main suite's own machinery: the
// port choice, the stack script, the global setup that authenticates it, and
// the credential handoff.
import { defineConfig, devices } from "@playwright/test";
import path from "node:path";
import { STACK_INFO_ENV } from "../global-setup";
import { harnessStackPort, STACK_PORT_ENV } from "../stack-port";
import { AUTH_STATE_ENV, harnessAuthorizationHeaders } from "../tests/helpers/device-auth";

/** The stack script's input naming the local supervisor's `--new-terminals`. */
const NEW_TERMINALS_ENV = "FARHELM_E2E_NEW_TERMINALS";

const port = harnessStackPort();
const baseURL = `http://127.0.0.1:${port}`;
// Beside the main suite's slot directories (`.stacks/<index>`, gitignored),
// under a name of its own, so neither config reads the other's stack.
const stackDir = path.join(__dirname, "..", ".stacks", "host-terminals");
const stackInfoPath = path.join(stackDir, "stack-info.json");
const authStatePath = path.join(stackDir, "storage-state.json");

export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  fullyParallel: false,
  workers: 1,
  // The main suite's bound: a first page load on a loaded machine takes
  // most of a shorter one.
  timeout: 60_000,
  use: {
    baseURL,
    storageState: authStatePath,
    extraHTTPHeaders: harnessAuthorizationHeaders(authStatePath),
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  globalSetup: "../global-setup.ts",
  // Global setup authenticates the stacks this lists, as for the main
  // suite's slots.
  metadata: {
    stackSlots: [{ baseURL, stackInfoPath, authStatePath }],
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: {
    // `exec` and the graceful SIGTERM for the main config's reasons (see
    // there): the stack script's orphan watcher waits on its parent, and its
    // trap is what ends its supervisors, tmux servers and terminal hosts.
    command: "exec bash ./start-stack.sh",
    cwd: path.join(__dirname, ".."),
    env: {
      [STACK_PORT_ENV]: String(port),
      [STACK_INFO_ENV]: stackInfoPath,
      [AUTH_STATE_ENV]: authStatePath,
      [NEW_TERMINALS_ENV]: "host",
    },
    url: `${baseURL}/`,
    reuseExistingServer: false,
    stdout: "pipe",
    stderr: "pipe",
    timeout: 30_000,
    gracefulShutdown: { signal: "SIGTERM", timeout: 5_000 },
  },
});
