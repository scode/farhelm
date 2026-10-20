// The one place that says how many stacks the browser suite runs and how a
// worker finds its own.
//
// The suite runs one stack (helm, both supervisors, private tmux servers,
// startup session; see start-stack.sh) per Playwright worker slot, and each
// worker drives only its slot's stack. Playwright's `parallelIndex` is the
// slot: it is stable for the lifetime of the run (a replacement worker after a
// crash, or the fresh worker every (engine, spec file) project gets, inherits
// the slot it replaces), and it never reaches or exceeds the worker count.
// That is what lets Playwright hand any project to any free worker while the
// stacks themselves stay long-lived, booted once per run by the config's
// `webServer` entries.
//
// Everything a test needs to reach its stack is derived here and nowhere
// else: the port, the base URL, where the stack script publishes what it
// booted, and where the harness credential for that stack lives. Keeping that
// description in one place is deliberate beyond tidiness: running stacks or
// whole groups of work on another machine is a likely later change, and it
// should only have to teach this module where a slot's stack is, not hunt
// down per-spec assumptions that a slot means a localhost port or a file
// under e2e/.
//
// Single-stack capture configs (readme-hero, readme-video, docs-shots) do not
// use slots; they keep their own stack-info paths and the legacy credential
// path in tests/helpers/device-auth.ts.
import path from "node:path";
import { harnessStackPorts } from "./stack-port";

/** How many stacks, and therefore Playwright workers, the browser suite runs.
 *
 * Picked rather than measured: the maintainer chose three to start with, to
 * get the suite running in parallel rather than to find an optimum, on a
 * machine whose CPU other agents share. Each stack is a helm, two
 * supervisors and their tmux servers, and each worker drives its own browser,
 * so the suite's CPU and memory use grow with this number.
 * `scripts/test_run_playwright.py` pins the recorder's worker count to the
 * same value and its tests check that the two agree; change both together.
 */
export const STACK_COUNT = 3;

/** What a worker needs to reach one slot's stack. */
export interface StackSlot {
  /** The slot index: Playwright's `parallelIndex` of the workers that use it. */
  index: number;
  /** The helm's loopback port, chosen once per run (stack-port.ts). */
  port: number;
  /** The helm's base URL, without a trailing slash. */
  baseURL: string;
  /** Where this slot's start-stack.sh publishes what it booted (state dirs, binaries, pids). */
  stackInfoPath: string;
  /** This slot's Playwright storage state holding the harness device credential. */
  authStatePath: string;
}

/** Every slot of this run, resolving (and on first use publishing) their ports. */
export function harnessStackSlots(): StackSlot[] {
  const ports = harnessStackPorts(STACK_COUNT);
  return ports.map((port, index) => {
    const dir = path.join(__dirname, ".stacks", String(index));
    return {
      index,
      port,
      baseURL: `http://127.0.0.1:${port}`,
      stackInfoPath: path.join(dir, "stack-info.json"),
      authStatePath: path.join(dir, "storage-state.json"),
    };
  });
}

/** Whether this process is a Playwright test worker.
 *
 * Playwright sets `TEST_WORKER_INDEX` (with `TEST_PARALLEL_INDEX`) in each
 * worker's environment. Every other process that loads the config has neither:
 * the runner, and the separate loader processes UI mode and editor
 * integrations use to discover tests. Those have no slot and drive no page.
 *
 * What this cannot catch is a Playwright that stops setting both variables
 * before a worker loads the config: every worker would then look like a
 * loader and use slot 0. The recorder pins the Playwright version
 * (scripts/test_run_playwright.py), so that can only arrive with a deliberate
 * upgrade, which should recheck it.
 */
export function isWorkerProcess(): boolean {
  return process.env.TEST_WORKER_INDEX !== undefined;
}

/** The slot of the worker this code runs in.
 *
 * Playwright 1.64 sets `TEST_PARALLEL_INDEX` in a worker's environment before
 * the worker re-imports the config, and keeps it there for the tests. That
 * ordering is observed behavior, not a documented contract, so in a worker
 * its absence is an error rather than a default: a worker that silently fell
 * back to slot 0 would drive another worker's stack and fail in ways that look
 * like ordinary flakes.
 */
export function currentStackSlot(): StackSlot {
  const raw = process.env.TEST_PARALLEL_INDEX;
  const slots = harnessStackSlots();
  const index = raw === undefined || !/^[0-9]+$/.test(raw) ? NaN : Number(raw);
  if (!(index >= 0 && index < slots.length)) {
    throw new Error(
      `this Playwright worker has TEST_PARALLEL_INDEX=${JSON.stringify(raw)}, which names none of the ` +
        `${slots.length} stack slots; the browser suite relies on Playwright setting it before the worker loads ` +
        `the config (stack-slots.ts)`,
    );
  }
  return slots[index];
}
