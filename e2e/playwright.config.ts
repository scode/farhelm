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
import { STACK_INFO_ENV } from "./global-setup";
import { STACK_PORT_ENV } from "./stack-port";
import { currentStackSlot, harnessStackSlots, isWorkerProcess, STACK_COUNT } from "./stack-slots";
import { AUTH_STATE_ENV, harnessAuthorizationHeaders } from "./tests/helpers/device-auth";

// One stack per worker slot (stack-slots.ts). Ports are chosen once per run
// and pinned through the environment; see stack-port.ts for why a fixed port
// was a collision between concurrent runs and why the choice cannot live in
// start-stack.sh.
const stackSlots = harnessStackSlots();
// Playwright re-imports this config in every worker, with the worker's slot
// already in its environment, so `use` below points each worker at its own
// stack. The runner (and any other process that only loads the config to
// discover tests) has no slot and never drives a page; slot 0 only gives its
// copy of `use` valid values.
const slot = isWorkerProcess() ? currentStackSlot() : stackSlots[0];
const authorizationHeaders = harnessAuthorizationHeaders(slot.authStatePath);
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
// specs that need the second host (`agent-relay`, `filters`) can
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
  // Each stack is one shared helm with ONE startup session; tests interfere
  // with each other by design (takeover semantics), so a stack serves one
  // worker at a time. `workers` equals the number of stacks, and every worker
  // drives only its own slot's stack (stack-slots.ts), so workers run in
  // parallel without ever sharing a stack. Within a project, tests stay
  // serial.
  //
  // One project per (engine, spec file) is also the unit Playwright hands to
  // whichever worker is free, and a browser-process lifetime
  // boundary. The old one-project-per-engine shape reused one WebKit process
  // for all 294 cases. Its network subprocess eventually crashed in CI and
  // took healthy terminal and feed WebSockets down with it. A fresh project
  // keeps each file's tests serial on its worker's long-lived stack while
  // replacing the browser between files. Do not collapse these projects merely to save launches;
  // that restores the multi-thousand-second process lifetime behind the
  // crash. The largest file still sets the residual lifetime bound.
  fullyParallel: false,
  workers: STACK_COUNT,
  timeout: 60_000,
  use: {
    baseURL: slot.baseURL,
    // These two options are one credential in two browser transports.
    // Playwright injects `use` options into the request fixture and into
    // manual browser.newContext() calls as well as its default page context,
    // so secondary clients cannot silently fall back to unauthenticated I/O.
    storageState: slot.authStatePath,
    extraHTTPHeaders: authorizationHeaders,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  globalSetup: "./global-setup.ts",
  // Global setup runs in the runner, which has no slot of its own, so it is
  // told about every stack it has to authenticate here.
  metadata: {
    stackSlots: stackSlots.map(({ baseURL, stackInfoPath, authStatePath }) => ({
      baseURL,
      stackInfoPath,
      authStatePath,
    })),
  },
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
  // One entry per stack slot. Each is an independent stack with its own port,
  // state directory and credential; Playwright starts them all before global
  // setup and shuts each down with its own graceful SIGTERM.
  //
  // They boot even when the selection is one test that only one worker will
  // run. Booting lazily from a worker instead would make that worker the
  // stack script's parent, and the orphan watcher below would tear the stack
  // down as soon as the worker's (engine, spec file) project ended, since
  // every project gets a fresh worker process. The idle stacks cost seconds.
  webServer: stackSlots.map((stack) => ({
    // The `exec` is load-bearing, not cosmetic. Playwright launches this
    // as `/bin/sh -c "<command>"`, and on Linux that shell is dash, which
    // stays resident instead of replacing itself — so without `exec` the
    // script's parent is a dash wrapper in the detached session, and that
    // wrapper survives the Playwright leader's death. start-stack.sh's
    // orphan watcher waits on its parent pid, so it would wait on a pid
    // that never goes away and the stack would leak exactly as before.
    // With `exec` the shell becomes the script and the leader is the
    // direct parent on every platform. scripts/test-start-stack-cleanup.sh
    // pins this layering; do not drop the prefix without updating it.
    command: "exec bash ./start-stack.sh",
    // The stack script is a shell child with no view of this module, so the
    // port it must bind, and the slot's paths it publishes to, reach it
    // through its environment.
    env: {
      [STACK_PORT_ENV]: String(stack.port),
      [STACK_INFO_ENV]: stack.stackInfoPath,
      [AUTH_STATE_ENV]: stack.authStatePath,
    },
    // The API correctly answers 401 before global setup has exchanged the
    // token. The public static bundle is therefore the readiness probe.
    url: `${stack.baseURL}/`,
    name: `stack ${stack.index}`,
    // A port that already answers is someone else's stack (another agent's
    // run on this machine), never this run's; refuse it rather than test
    // against a helm whose token this run does not hold.
    reuseExistingServer: false,
    stdout: "pipe",
    stderr: "pipe",
    timeout: 30_000,
    // Without this Playwright SIGKILLs the process group, which no trap
    // can catch — and the private tmux server (which daemonizes out of
    // the group) would survive every run. SIGTERM lets start-stack.sh
    // reap its own supervisor and tmux server.
    gracefulShutdown: { signal: "SIGTERM", timeout: 5_000 },
  })),
});
