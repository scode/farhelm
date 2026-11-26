#!/usr/bin/env -S deno run -A
/**
 * Wait for the landing monitor's next real event, handling idle timeouts without waking the model.
 *
 * The landing monitor waits between rounds on `scripts/plans-watch.sh --wake-for lander --max-wait 6600`
 * (plans/.agents/landing.md, "Waiting between rounds"). The watcher should wake it for every plan that becomes complete
 * or blocked; the idle exit, about every 110 minutes, exists only to catch one it missed. Waking the model for that
 * would cost a full re-read of its conversation each time to find nothing, so this wrapper does the idle check itself:
 * it runs `queue status` and exits (waking the model) only when a round is due or something else needs it:
 *
 * - the watcher exits `changed` (a plan became complete or blocked), or with a usage error, an unexpected status, or
 *   an `error:` line naming HTTP 401 or 403 (the GitHub login needs the maintainer) or 404 (a baseline commit GitHub
 *   does not know, which retrying cannot fix);
 * - an idle check's `queue status` shows a `[complete]` plan, one of the flags the landing instructions say to act on
 *   (complete with no open PRs, landing with no open PRs), or a `[blocked]` line for a plan named with
 *   `--wake-on-blocked`, or, with `--wake-on-any-blocked`, any `[blocked]` line not named with `--ignore-blocked` (the
 *   monitor answers blocks itself unless they are huge, landing.md, "Answering blocks", so a fresh block is work for
 *   it; `--ignore-blocked` names the blocks it left for the maintainer);
 * - `queue status` itself fails.
 *
 * Otherwise it restarts the watcher with the same baseline. `--blocks-only` is the mode for while a landing round runs
 * (landing.md, "A landing round"). A round's plans are fixed when it starts, so the only thing worth waking the monitor
 * for mid-round is a block it can answer beside the round. In this mode the watcher runs with `--wake-for
 * lander-blocks`, which wakes only for a plan newly blocked since the baseline, and an idle check wakes only for a
 * `[blocked]` line (as with `--wake-on-any-blocked`, still minus `--ignore-blocked`). It never wakes for a `[complete]`
 * plan, which includes the round's own, or for the status flags: both belong to the round boundary, which looks at
 * them anyway.
 *
 * Other `error:` exits (a GitHub outage, say) are retried with `--interval 1800 --max-wait 3600`, as the executors'
 * watcher is (plans/.agents/watching.md), until a watcher exits `changed` or `idle`, which restores the default
 * interval and `--max-wait`. The shorter cap makes the slow interval end on its own once GitHub answers again. An
 * outage being retried does not need the maintainer, so each retry is reported on this script's own stderr only.
 *
 * NOTE: This script never writes the monitor log. The log lives in the agent records (`cargo xtask records path
 * landing`), where the monitor and `landing.py` edit it and push it before GitHub changes; a background writer racing
 * those edits could drop one, and nothing reads the idle checks as a sign of life any more, since nothing unattended
 * judges whether the monitor is alive. It prints exactly one line on stdout before exiting: `wake <reason>`.
 *
 * The checkout defaults to the one holding this script and the repository to its `origin` remote.
 *
 * Usage: idle-watch.ts --baseline-from <commit> [--checkout <dir>] [--repo OWNER/NAME] [--wake-on-blocked <slug>]...
 *        [--wake-on-any-blocked] [--blocks-only] [--ignore-blocked <slug>]... [--max-wait <seconds>]
 */
import $, { KillController } from "jsr:@david/dax@0.50.1";
import { parseArgs } from "jsr:@std/cli@1.0.32/parse-args";
import { dirname, fromFileUrl, join, resolve } from "jsr:@std/path@1.1.6";

// Flags from plans/.agents/landing.md, "A landing round", that the monitor acts on. Other flags (a pending plan's
// unlanded work from an earlier round, a claim too old for its age to be known) are informational and do not wake it.
const ACTIONABLE_FLAGS = ["no open PRs"];

/** Which `queue status` lines wake the monitor; the command-line flags of the same names. */
export interface WakeOn {
  /** Blocked plans to wake for by name. */
  wakeOnBlocked?: string[];
  /** Wake for every blocked plan not in `ignoreBlocked`. */
  anyBlocked?: boolean;
  /** Blocks the monitor left for the maintainer, which must not wake it again. */
  ignoreBlocked?: string[];
  /** The mid-round mode: blocks only (implying `anyBlocked`), never a `[complete]` plan or a flag. */
  blocksOnly?: boolean;
}

/**
 * The reasons a `queue status` output needs the monitor, or an empty list when it does not.
 *
 * A status line starts with `[<state>...] <slug>`; flags follow on indented lines starting with `!`. A flag is
 * attributed to the plan line above it so the reason names the plan.
 */
export function actionable(status: string, on: WakeOn = {}): string[] {
  const { wakeOnBlocked = [], ignoreBlocked = [], blocksOnly = false } = on;
  const anyBlocked = blocksOnly || (on.anyBlocked ?? false);
  const reasons: string[] = [];
  let plan = "";
  for (const line of status.split("\n")) {
    const head = line.match(/^\[(\w[\w-]*)[^\]]*\] (\S+)/);
    if (head) {
      const [, state, slug] = head;
      plan = slug;
      if (state === "complete" && !blocksOnly) reasons.push(`complete ${slug}`);
      const wanted = wakeOnBlocked.includes(slug) || (anyBlocked && !ignoreBlocked.includes(slug));
      if (state === "blocked" && wanted) reasons.push(`blocked ${slug}`);
      continue;
    }
    if (blocksOnly) continue;
    const flag = line.match(/^\s+!\s*(.*)$/);
    if (flag && ACTIONABLE_FLAGS.some((f) => flag[1].includes(f))) reasons.push(`flag ${plan}: ${flag[1].trim()}`);
  }
  return reasons;
}

/** `OWNER/NAME` from an https or ssh GitHub remote URL, or undefined for anything else. */
export function githubRepo(url: string): string | undefined {
  return url.trim().match(
    /^(?:https:\/\/github\.com\/|git@github\.com:|ssh:\/\/git@github\.com\/)([\w.-]+\/[\w.-]+?)(?:\.git)?\/?$/,
  )
    ?.[1];
}

/** The idle cap the landing instructions give the watcher between rounds: about 110 minutes. */
export const DEFAULT_MAX_WAIT = "6600";

/**
 * The watcher's polling arguments. Normal runs pass only the idle cap (`maxWait`, default `DEFAULT_MAX_WAIT`). Slow
 * mode, after a transient error, polls every 30 minutes with the executors' shorter cap of 3600 seconds
 * (plans/.agents/watching.md), so that it ends by itself with an `idle` exit once GitHub answers again.
 */
export function watcherTiming(slow: boolean, maxWait: string = DEFAULT_MAX_WAIT): string[] {
  return slow ? ["--interval", "1800", "--max-wait", "3600"] : ["--max-wait", maxWait];
}

/**
 * What one watcher exit means for the loop, and whether the next watcher runs in slow mode.
 *
 * - `retry`: an `error:` exit (status 3) that retrying can fix; the next watcher runs slow. HTTP 401 and 403 (the
 *   GitHub login) and 404 (a baseline GitHub does not know) are not retried.
 * - `check`: `idle` (status 10); run the idle check, and the next watcher (if any) is back to normal.
 * - `wake`: anything else (`changed`, a usage error, an unexpected status, an unretryable error).
 */
export function afterWatcherExit(code: number, line: string): { action: "retry" | "check" | "wake"; slow: boolean } {
  if (code === 3 && !/HTTP 40[134]/.test(line)) return { action: "retry", slow: true };
  if (code === 10) return { action: "check", slow: false };
  return { action: "wake", slow: false };
}

function wake(reason: string): never {
  console.log(`wake ${reason}`);
  Deno.exit(0);
}

if (import.meta.main) {
  const args = parseArgs(Deno.args, {
    string: ["baseline-from", "checkout", "repo", "wake-on-blocked", "ignore-blocked", "max-wait"],
    boolean: ["wake-on-any-blocked", "blocks-only"],
    collect: ["wake-on-blocked", "ignore-blocked"],
  });
  const baseline = args["baseline-from"];
  if (!baseline) throw new Error("--baseline-from is required");
  const checkout = resolve(args.checkout ?? join(dirname(fromFileUrl(import.meta.url)), "..", ".."));
  const repo = args.repo ?? githubRepo(await $`git -C ${checkout} remote get-url origin`.noThrow().text());
  if (!repo) throw new Error("cannot tell the repository from the origin remote; pass --repo OWNER/NAME");
  const on: WakeOn = {
    wakeOnBlocked: args["wake-on-blocked"] as string[],
    anyBlocked: args["wake-on-any-blocked"] as boolean,
    ignoreBlocked: args["ignore-blocked"] as string[],
    blocksOnly: args["blocks-only"] as boolean,
  };
  const role = on.blocksOnly ? "lander-blocks" : "lander";
  // A value other than the default is only for exercising the idle path.
  const maxWait = args["max-wait"] ?? DEFAULT_MAX_WAIT;
  const kill = new KillController();
  for (const sig of ["SIGINT", "SIGTERM"] as const) Deno.addSignalListener(sig, () => kill.kill());
  let slow = false;
  while (true) {
    const watch = await $`${checkout}/scripts/plans-watch.sh --repo ${repo} --baseline-from ${baseline} \
      --wake-check ${checkout}/scripts/plans-queue.py --wake-for ${role} ${watcherTiming(slow, maxWait)}`
      .cwd(checkout).noThrow().stdout("piped").signal(kill.signal);
    const line = watch.stdout.trim();
    const exit = afterWatcherExit(watch.code, line);
    slow = exit.slow;
    if (exit.action === "retry") {
      console.error(`idle-watch: watcher error, retrying at the slow interval: ${line}`);
      continue;
    }
    if (exit.action === "wake") wake(`watcher exit ${watch.code}: ${line}`);
    const status = await $`python3 scripts/plans-queue.py --repo ${repo} status`.cwd(checkout).noThrow()
      .stdout("piped").stderr("piped");
    if (status.code !== 0) {
      wake(`queue status failed (exit ${status.code}): ${status.stderr.trim().split("\n").at(-1)}`);
    }
    const reasons = actionable(status.stdout, on);
    if (reasons.length > 0) wake(reasons.join("; "));
  }
}

// The status every `actionable` test reads: one line per state, with flags under the plans they belong to.
const SAMPLE_STATUS = [
  "head: 0123456789abcdef",
  "[complete] alpha | PRs #1",
  "[in-flight abc123] beta | claimed 2026-10-23T00:00:00Z",
  "[in-flight def456] theta",
  "    ! claim commit not in recent history: its age is unknown",
  "[pending] gamma",
  "    ! unlanded work from an earlier round",
  "[blocked] delta | PRs #2",
  "[blocked] epsilon",
  "[landing ff00ff] zeta",
  "    ! landing, no open PRs",
  "[landed 2026-10-20] eta | PRs #3 | report awaiting review",
].join("\n");

/**
 * Between rounds the idle check wakes the monitor only for a round that is due or a flag it acts on. An old claim is
 * not one: nothing unattended judges whether a holder is alive, so neither an old claim nor the informational
 * unknown-age flag may wake it.
 */
Deno.test("actionable finds complete plans, acted-on flags and pre-answered blocks only", async () => {
  const { assertEquals } = await import("jsr:@std/assert@1.0.19");
  assertEquals(actionable(SAMPLE_STATUS, { wakeOnBlocked: ["delta"] }), [
    "complete alpha",
    "blocked delta",
    "flag zeta: landing, no open PRs",
  ]);
  assertEquals(actionable("head: 01\n[pending] gamma\n    ! unlanded work from an earlier round\n"), []);
  // Any-blocked mode wakes for every block except the ones deliberately left for the maintainer.
  assertEquals(actionable("[blocked] delta\n[blocked] epsilon\n", { anyBlocked: true, ignoreBlocked: ["epsilon"] }), [
    "blocked delta",
  ]);
  assertEquals(actionable("[blocked] delta\n", { anyBlocked: false }), []);
});

/**
 * Mid-round, the monitor may answer blocks but must not take on plans that complete, nor act on the round-boundary
 * flags (landing.md, "A landing round"). Waking it for those would hand it work the round forbids it to start, so the
 * blocks-only mode must report every block not left for the maintainer and nothing else, even from a status full of
 * `[complete]` plans and flags.
 */
Deno.test("actionable in blocks-only mode wakes for blocks alone", async () => {
  const { assertEquals } = await import("jsr:@std/assert@1.0.19");
  assertEquals(actionable(SAMPLE_STATUS, { blocksOnly: true, ignoreBlocked: ["epsilon"] }), ["blocked delta"]);
  assertEquals(actionable(SAMPLE_STATUS, { blocksOnly: true }), ["blocked delta", "blocked epsilon"]);
  assertEquals(actionable(SAMPLE_STATUS, { blocksOnly: true, ignoreBlocked: ["delta", "epsilon"] }), []);
});

/**
 * After a GitHub outage the watcher must poll slowly but still end the slow mode by itself, and an `idle` exit must
 * put it back on the normal cap; otherwise the monitor would either hammer GitHub during an outage or keep the
 * 30-minute interval forever. Specified: slow mode passes `--interval 1800 --max-wait 3600` whatever cap was asked
 * for, normal mode only the cap (6600 by default); a retryable error turns slow mode on, an `idle` exit turns it off,
 * and login or unknown-baseline errors and every other exit wake the monitor instead of retrying.
 */
Deno.test("the watcher's timing goes slow after a transient error and back to normal after idle", async () => {
  const { assertEquals } = await import("jsr:@std/assert@1.0.19");
  assertEquals(watcherTiming(false), ["--max-wait", "6600"]);
  assertEquals(watcherTiming(false, "60"), ["--max-wait", "60"]);
  assertEquals(watcherTiming(true, "60"), ["--interval", "1800", "--max-wait", "3600"]);
  let slow = false;
  const timings: string[][] = [];
  for (const [code, line] of [[3, "error: HTTP 502"], [3, "error: timeout"], [10, "idle"]] as const) {
    timings.push(watcherTiming(slow));
    slow = afterWatcherExit(code, line).slow;
  }
  timings.push(watcherTiming(slow));
  assertEquals(timings, [
    ["--max-wait", "6600"],
    ["--interval", "1800", "--max-wait", "3600"],
    ["--interval", "1800", "--max-wait", "3600"],
    ["--max-wait", "6600"],
  ]);
  assertEquals(afterWatcherExit(3, "error: HTTP 401 from GitHub").action, "wake");
  assertEquals(afterWatcherExit(3, "error: HTTP 404 baseline").action, "wake");
  assertEquals(afterWatcherExit(0, "changed abc").action, "wake");
  assertEquals(afterWatcherExit(2, "usage: bad").action, "wake");
});

Deno.test("githubRepo reads https and ssh remotes and nothing else", async () => {
  const { assertEquals } = await import("jsr:@std/assert@1.0.19");
  assertEquals(githubRepo("https://github.com/owner/name\n"), "owner/name");
  assertEquals(githubRepo("https://github.com/owner/name.git"), "owner/name");
  assertEquals(githubRepo("git@github.com:owner/some.name.git"), "owner/some.name");
  assertEquals(githubRepo("https://example.com/owner/name"), undefined);
});
