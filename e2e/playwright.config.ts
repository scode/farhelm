// Playwright drives the WEB build against a real helm, supervisor,
// private tmux, and fake agent — no mocks anywhere in the stack
// (PLAN_M1.md: this is the canonical GUI verification path for agents).
//
// Two engines run: Chromium, and WebKit as a stand-in for the desktop
// app's actual renderer family (WKWebView on macOS, WebKitGTK on Linux).
// This covers the WEB build under that engine — rendering, input, JS
// API behavior — not the desktop shell: wry-layer integration bugs
// (like the eval-channel death manual testing found on macOS) live
// outside anything a browser run can reach, and stay with the desktop
// smoke harness and manual passes. Firefox is not included: nothing in
// the desktop app embeds Gecko, so it would add runtime without
// covering a real target.
import { defineConfig, devices } from "@playwright/test";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import {
  AUTH_STORAGE_STATE_PATH,
  harnessAuthorizationHeaders,
} from "./tests/helpers/device-auth";

const authorizationHeaders = harnessAuthorizationHeaders();
const testsDir = join(__dirname, "tests");

/** Preserve Playwright's recursive spec discovery while assigning one file per project. */
function discoverSpecFiles(directory = testsDir): string[] {
  return readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) return discoverSpecFiles(path);
      if (!entry.name.endsWith(".spec.ts")) return [];

      return [path.slice(testsDir.length + 1)];
    })
    .sort();
}

const specFiles = discoverSpecFiles();
const terminalMultihostSpec = "terminal-multihost.spec.ts";

// The multihost suite stops its replacement remote supervisor when it exits,
// and nothing puts one back. That teardown must therefore be last across the
// WHOLE run, not merely last within its own engine: an engine-local ordering
// left chromium's multihost teardown ahead of every webkit project — and the
// specs that need the second host (`agent-relay`, `filters`, `profiles`) can
// only wait for it, never restore it. The symptom was a WebKit relay suite
// that failed in its `beforeAll` or skipped its whole point.
//
// WHAT THIS RELIES ON, precisely: that a run with `workers: 1` executes
// projects in the order this array lists them. That is Playwright's observed
// scheduling — it builds one test group per project in configuration order and
// hands them to the single worker in that order — but it is not a documented
// contract the way project `dependencies` is, and the docs describe projects
// as running in parallel subject to the worker limit. So this is an
// implementation behavior we depend on, not a guarantee. If a Playwright
// upgrade ever reorders projects, the failure is loud and localized: every
// spec above that needs the ssh host fails or skips in its own `beforeAll`
// after the multihost teardown has already run.
//
// Expressed as an ordering rather than through project `dependencies`, which
// would make a directly selected multihost project drag every unrelated
// project into the run. The two ways out, if this ever breaks, are both
// bigger than a config change: teach `terminal-multihost.spec.ts` to restore
// the supervisor in its `afterAll` (which needs a reaper for the restored
// process, since a supervisor spawned by a worker outlives the run and
// `start-stack.sh` reaps only the pids it started itself), or give every spec
// that needs the second host the restore-if-absent `beforeAll` the multihost
// suite already has.
const specFilesNeedingTheRemote = specFiles.filter((spec) => spec !== terminalMultihostSpec);
const specFilesTearingDownTheRemote = specFiles.filter((spec) => spec === terminalMultihostSpec);

const engines = [
  { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  { name: "webkit", use: { ...devices["Desktop Safari"] } },
];

export default defineConfig({
  testDir: "./tests",
  // The stack is one shared helm with ONE session; tests interfere with
  // each other by design (takeover semantics), so run serially. `workers`
  // caps the whole run, including across projects, and every project points
  // at the one webServer below. A second worker would race the first on the
  // shared session server-side.
  //
  // One project per (engine, spec file) is also a browser-process lifetime
  // boundary. The old one-project-per-engine shape reused one WebKit process
  // for all 294 cases. Its network subprocess eventually crashed in CI and
  // took healthy terminal and feed WebSockets down with it. A fresh project
  // keeps the suite serial and the stack shared while replacing the browser
  // between files. Do not collapse these projects merely to save launches;
  // that restores the multi-thousand-second process lifetime behind the
  // crash. The largest file still sets the residual lifetime bound.
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  use: {
    baseURL: "http://127.0.0.1:7434",
    // These two options are one credential in two browser transports.
    // Playwright injects `use` options into the request fixture and into
    // manual browser.newContext() calls as well as its default page context,
    // so secondary clients cannot silently fall back to unauthenticated I/O.
    storageState: AUTH_STORAGE_STATE_PATH,
    extraHTTPHeaders: authorizationHeaders,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  globalSetup: "./global-setup.ts",
  // The per-engine stack reset is registered by
  // `tests/helpers/terminal-suite.ts`'s `installTerminalSuiteHooks`, not by a
  // setup project or a name-ordered spec file:
  // Playwright does not guarantee alphabetical file ordering, and
  // project `dependencies` cannot express "webkit's reset runs AFTER
  // chromium's tests" without dragging the whole chromium suite into a
  // `--project='webkit-*'` run. A `beforeAll` runs at each project's entry
  // into the file, in order by construction, and its failure fails the
  // file's tests rather than letting them run against half-reset state.
  // Every engine's ordinary specs first, then every engine's remote-teardown
  // spec — see `specFilesTearingDownTheRemote` for why the split is global
  // rather than per engine.
  projects: [
    ...engines.flatMap((engine) =>
      specFilesNeedingTheRemote.map((spec) => ({
        name: `${engine.name}-${spec.replace(/\.spec\.ts$/, "")}`,
        testMatch: spec,
        use: engine.use,
      })),
    ),
    ...engines.flatMap((engine) =>
      specFilesTearingDownTheRemote.map((spec) => ({
        name: `${engine.name}-${spec.replace(/\.spec\.ts$/, "")}`,
        testMatch: spec,
        use: engine.use,
      })),
    ),
  ],
  webServer: {
    command: "bash ./start-stack.sh",
    // The API correctly answers 401 before global setup has exchanged the
    // token. The public static bundle is therefore the readiness probe.
    url: "http://127.0.0.1:7434/",
    reuseExistingServer: false,
    stdout: "pipe",
    stderr: "pipe",
    timeout: 30_000,
    // Without this Playwright SIGKILLs the process group, which no trap
    // can catch — and the private tmux server (which daemonizes out of
    // the group) would survive every run. SIGTERM lets start-stack.sh
    // reap its own supervisor and tmux server.
    gracefulShutdown: { signal: "SIGTERM", timeout: 5_000 },
  },
});
