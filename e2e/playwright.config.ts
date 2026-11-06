// Playwright drives the WEB build against a real helm, supervisors, and
// fake agent — no mocks anywhere in the stack (PLAN_M1.md: this is the
// canonical GUI verification path for agents). The stacks create new
// terminals (terminal hosts, drawn by the grid component) unless the run
// asks for the old-terminal set (suite-terminals.ts).
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
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { STACK_INFO_ENV } from "./global-setup";
import { STACK_PORT_ENV } from "./stack-port";
import { currentStackSlot, harnessStackSlots, isWorkerProcess, STACK_COUNT } from "./stack-slots";
import { AUTH_STATE_ENV, harnessAuthorizationHeaders } from "./tests/helpers/device-auth";
import {
  OLD_TERMINAL_FILES,
  OLD_TERMINAL_GREP,
  OLD_TERMINAL_TAG,
  OLD_TERMINAL_TAGGED,
  SUITE_TERMINALS_ENV,
  SUITE_TERMINALS_METADATA,
  suiteTerminalsFrom,
} from "./suite-terminals";

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

/**
 * A use of the old-terminal tag in a spec's source, through the constant or
 * as the literal tag in quotes: what puts a file in `OLD_TERMINAL_TAGGED`'s
 * account. The literal counts too, since a test tagged with it leaves the
 * ordinary run all the same.
 */
const OLD_TERMINAL_TAG_USE = new RegExp(`\\bOLD_TERMINAL_TAG\\b|["'\`]${OLD_TERMINAL_TAG}["'\`]`);

// Which kind of terminal this run's stacks create (suite-terminals.ts;
// tests/helpers/term.ts, "Two kinds of terminal"). Unset is the ordinary run:
// every stack's supervisors create new terminals, and every spec runs except
// the old-terminal set. `tmux` starts them on old terminals and runs only that
// set. Any other value is refused here rather than run as either.
const suiteTerminals = suiteTerminalsFrom(process.env[SUITE_TERMINALS_ENV]);

const discovered = discoverSpecFiles();
// The old-terminal set's account must stay complete on both runs: a listed
// file that does not exist (renamed, moved), a file in both lists, a spec
// that tags tests without being listed as tagged (a whole file that also tags
// is reported as such), and a listed tagged file that tags nothing would each
// move tests between the runs without a word.
{
  const listed = [...OLD_TERMINAL_FILES.keys(), ...OLD_TERMINAL_TAGGED];
  const missing = listed.filter((spec) => !discovered.includes(spec));
  const both = [...OLD_TERMINAL_TAGGED].filter((spec) => OLD_TERMINAL_FILES.has(spec));
  const tagging = discovered.filter((spec) => OLD_TERMINAL_TAG_USE.test(readFileSync(join(testsDir, spec), "utf8")));
  const unlisted = tagging.filter((spec) => !OLD_TERMINAL_TAGGED.has(spec) && !OLD_TERMINAL_FILES.has(spec));
  const taggedWhole = tagging.filter((spec) => OLD_TERMINAL_FILES.has(spec));
  const untagged = [...OLD_TERMINAL_TAGGED].filter((spec) => discovered.includes(spec) && !tagging.includes(spec));
  if (missing.length || both.length || unlisted.length || taggedWhole.length || untagged.length) {
    throw new Error(
      `suite-terminals.ts names spec files that do not exist (${missing.join(", ") || "none"}), ` +
        `lists files as both whole and tagged (${both.join(", ") || "none"}), leaves files that tag ` +
        `old-terminal tests unlisted (${unlisted.join(", ") || "none"}), has whole files that also use the ` +
        `tag (${taggedWhole.join(", ") || "none"}), and lists tagged files that tag nothing ` +
        `(${untagged.join(", ") || "none"})`,
    );
  }
}

/**
 * The run's spec files, each with the test filter its projects carry: the
 * ordinary run takes every file but the whole old-terminal ones and leaves
 * the tagged tests out; the old-terminal run takes the whole files as they
 * are and only the tagged tests of the others. Project-level filters, so a
 * `-g` on the command line narrows further instead of replacing them.
 */
const specFiles: { spec: string; grep?: RegExp; grepInvert?: RegExp }[] = suiteTerminals === "host"
  ? discovered.filter((spec) => !OLD_TERMINAL_FILES.has(spec)).map((spec) => ({ spec, grepInvert: OLD_TERMINAL_GREP }))
  : discovered.filter((spec) => OLD_TERMINAL_FILES.has(spec) || OLD_TERMINAL_TAGGED.has(spec))
    .map((spec) => (OLD_TERMINAL_FILES.has(spec) ? { spec } : { spec, grep: OLD_TERMINAL_GREP }));

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
    [SUITE_TERMINALS_METADATA]: suiteTerminals,
    stackSlots: stackSlots.map(({ baseURL, stackInfoPath, authStatePath }) => ({
      baseURL,
      stackInfoPath,
      authStatePath,
    })),
  },
  // Projects carry no ordering: any of them may run on any stack, in any
  // order, because a spec that disturbs shared stack state puts it back
  // before it ends. Two kinds of state matter. The terminal-family specs
  // reset the stack in their own `beforeAll` (`installTerminalSuiteHooks`
  // in `tests/helpers/terminal-suite.ts`), which runs at each project's entry
  // into the file and fails that file's tests rather than letting them run
  // against half-reset state. And `terminal-multihost.spec.ts`, which takes
  // the second host down on purpose, restores it before it ends, so the
  // specs that only wait for that host (`agent-relay`, `filters`) find it up
  // whatever ran before them.
  projects: engines.flatMap((engine) =>
    specFiles.map(({ spec, grep, grepInvert }) => ({
      name: `${engine.name}-${spec.replace(/\.spec\.ts$/, "")}`,
      testMatch: spec,
      ...(grep ? { grep } : {}),
      ...(grepInvert ? { grepInvert } : {}),
      use: engine.use,
    })),
  ),
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
      // Pinned to the run's kind: Playwright merges its own environment into
      // this, and a FARHELM_E2E_NEW_TERMINALS exported in the runner's shell
      // must not decide this suite's terminals. Only this suite's own switch
      // (`SUITE_TERMINALS_ENV`, above) chooses them.
      FARHELM_E2E_NEW_TERMINALS: suiteTerminals,
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
    // can catch — and the terminal hosts (and, on the old-terminal run,
    // the private tmux servers), which outlive the processes that started
    // them, would survive every run. SIGTERM lets start-stack.sh reap its own supervisors, hosts
    // and tmux servers.
    gracefulShutdown: { signal: "SIGTERM", timeout: 5_000 },
  })),
});
