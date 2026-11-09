#!/usr/bin/env -S deno run -A
/**
 * Wait for the landing monitor's next real event, handling idle timeouts without waking the model.
 *
 * The landing monitor waits between rounds on `scripts/plans-watch.sh --wake-for lander --max-wait 6600`
 * (plans/.agents/landing.md, "Waiting between rounds"). Its `idle` exit, about every 110 minutes, used to wake the
 * model only to run `queue status`, refresh the monitor log's Status line, and restart the watcher. This wrapper does
 * that idle check itself and exits (waking the model) only when something needs it:
 *
 * - the watcher exits `changed` (a plan became complete or blocked), or with a usage error, an unexpected status, or
 *   an `error:` line naming HTTP 401 or 403 (the GitHub login needs the maintainer) or 404 (a baseline commit GitHub
 *   does not know, which retrying cannot fix);
 * - an idle check's `queue status` shows a `[complete]` plan, one of the flags the landing instructions say to act on
 *   (possibly abandoned, complete with no open PRs, landing with no open PRs), or a `[blocked]` line for a plan named
 *   with `--wake-on-blocked`, or, with `--wake-on-any-blocked`, any `[blocked]` line not named with
 *   `--ignore-blocked` (the monitor answers blocks itself unless they are huge, landing.md, "Answering blocks", so a
 *   fresh block is work for it; `--ignore-blocked` names the blocks it left for the maintainer);
 * - `queue status` itself fails.
 *
 * `--blocks-only` is the mode for while a landing round runs (landing.md, "A landing round"). A round's plans are fixed
 * when it starts, so the only thing worth waking the monitor for mid-round is a block it can answer beside the round.
 * In this mode the watcher runs with `--wake-for lander-blocks`, which wakes only for a plan newly blocked since the
 * baseline, and an idle check wakes only for a `[blocked]` line (as with `--wake-on-any-blocked`, still minus
 * `--ignore-blocked`). It never wakes for a `[complete]` plan, which includes the round's own, or for the status
 * flags: both belong to the round boundary, which looks at them anyway.
 *
 * Other `error:` exits are retried with `--interval 1800`, as the instructions say, until a poll succeeds. Each idle
 * check appends a History entry naming its mode to the monitor log, so the review flow still sees a live monitor;
 * between rounds it also rewrites the log's `running, last checked` line. It prints exactly one line before exiting:
 * `wake <reason>`.
 *
 * The checkout defaults to the one holding this script, the repository to its `origin` remote, and the monitor log to
 * `farhelm-plans-monitor-log.md` beside the checkout (plans/.agents/outside-files.md).
 *
 * Usage: idle-watch.ts --baseline-from <commit> [--checkout <dir>] [--repo OWNER/NAME] [--wake-on-blocked <slug>]...
 *        [--wake-on-any-blocked] [--blocks-only] [--ignore-blocked <slug>]... [--max-wait <seconds>]
 */
import $, { KillController } from "jsr:@david/dax@0.50.1";
import { parseArgs } from "jsr:@std/cli@1.0.32/parse-args";
import { dirname, fromFileUrl, join, resolve } from "jsr:@std/path@1.1.6";

// Flags from plans/.agents/landing.md, "A landing round", that the monitor acts on. Other flags (such as a pending
// plan's unlanded work from an earlier round) are informational and do not wake it.
const ACTIONABLE_FLAGS = ["possibly abandoned", "no open PRs"];

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

/**
 * Append one History entry stamped with the current UTC minute, and with `refreshStatus` also rewrite the Status line.
 *
 * Between rounds the model is asleep and nothing else writes the log, so rewriting the whole file is safe. During a
 * round the monitor and `landing.py` edit the log all the time (claim lines among them, which a resume depends on),
 * and a whole-file rewrite racing one of those edits would drop it. So the mid-round mode only appends: at worst a
 * concurrent rewrite loses this entry, never the monitor's. The Status line is the monitor's to keep during a round.
 */
async function logIdleCheck(log: string, entry: string, refreshStatus: boolean) {
  const now = new Date().toISOString().slice(0, 16) + "Z";
  const text = await Deno.readTextFile(log);
  if (refreshStatus) {
    const updated = text.replace(/^running, last checked .*$/m, `running, last checked ${now}`);
    await Deno.writeTextFile(log, updated.replace(/\n*$/, "\n") + `- ${now}: ${entry}\n`);
    return;
  }
  const sep = text === "" || text.endsWith("\n") ? "" : "\n";
  await Deno.writeTextFile(log, `${sep}- ${now}: ${entry}\n`, { append: true });
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
  const log = join(dirname(checkout), "farhelm-plans-monitor-log.md");
  const repo = args.repo ?? githubRepo(await $`git -C ${checkout} remote get-url origin`.noThrow().text());
  if (!repo) throw new Error("cannot tell the repository from the origin remote; pass --repo OWNER/NAME");
  const on: WakeOn = {
    wakeOnBlocked: args["wake-on-blocked"] as string[],
    anyBlocked: args["wake-on-any-blocked"] as boolean,
    ignoreBlocked: args["ignore-blocked"] as string[],
    blocksOnly: args["blocks-only"] as boolean,
  };
  const role = on.blocksOnly ? "lander-blocks" : "lander";
  const mode = on.blocksOnly ? "mid-round, blocks only" : "between rounds";
  const record = (entry: string) => logIdleCheck(log, `idle watcher (idle-watch, ${mode}): ${entry}`, !on.blocksOnly);
  // 6600 s is the landing instructions' idle cap; a smaller value is only for exercising the idle path.
  const maxWait = args["max-wait"] ?? "6600";
  const kill = new KillController();
  for (const sig of ["SIGINT", "SIGTERM"] as const) Deno.addSignalListener(sig, () => kill.kill());
  let slow = false;
  while (true) {
    const interval = slow ? ["--interval", "1800"] : [];
    const watch = await $`${checkout}/scripts/plans-watch.sh --repo ${repo} --baseline-from ${baseline} \
      --wake-check ${checkout}/scripts/plans-queue.py --wake-for ${role} --max-wait ${maxWait} ${interval}`
      .cwd(checkout).noThrow().stdout("piped").signal(kill.signal);
    const line = watch.stdout.trim();
    if (watch.code === 3 && !/HTTP 40[134]/.test(line)) {
      slow = true;
      await record(`watcher error, retrying at the slow interval: ${line}`);
      continue;
    }
    if (watch.code !== 10) wake(`watcher exit ${watch.code}: ${line}`);
    slow = false;
    const status = await $`python3 scripts/plans-queue.py --repo ${repo} status`.cwd(checkout).noThrow()
      .stdout("piped").stderr("piped");
    if (status.code !== 0) {
      wake(`queue status failed (exit ${status.code}): ${status.stderr.trim().split("\n").at(-1)}`);
    }
    const head = status.stdout.match(/^head: (\w+)/m)?.[1] ?? "unknown";
    const reasons = actionable(status.stdout, on);
    if (reasons.length > 0) {
      await record(`status head ${head.slice(0, 8)} needs the monitor: ${reasons.join("; ")}`);
      wake(reasons.join("; "));
    }
    const inFlight = status.stdout.split("\n").filter((l) => l.startsWith("[in-flight")).length;
    const nothing = on.blocksOnly ? "no block to answer" : "no flags, no [complete]";
    await record(
      `status head ${head.slice(0, 8)}: ${nothing}, ${inFlight} in flight; ` +
        `restarting the watcher with baseline ${baseline.slice(0, 8)} without waking the model.`,
    );
  }
}

// The status every `actionable` test reads: one line per state, with flags under the plans they belong to.
const SAMPLE_STATUS = [
  "head: 0123456789abcdef",
  "[complete] alpha | PRs #1 | log quiet 0.1 h",
  "[in-flight abc123] beta | claimed 2026-10-23T00:00:00Z | log quiet 9.0 h",
  "    ! possibly abandoned",
  "[pending] gamma",
  "    ! unlanded work from an earlier round",
  "[blocked] delta | PRs #2",
  "[blocked] epsilon",
  "[landing ff00ff] zeta | log quiet 1.0 h",
  "    ! landing, no open PRs",
  "[landed 2026-10-20] eta | PRs #3 | report awaiting review",
].join("\n");

Deno.test("actionable finds complete plans, acted-on flags and pre-answered blocks only", async () => {
  const { assertEquals } = await import("jsr:@std/assert@1.0.19");
  assertEquals(actionable(SAMPLE_STATUS, { wakeOnBlocked: ["delta"] }), [
    "complete alpha",
    "flag beta: possibly abandoned",
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

Deno.test("githubRepo reads https and ssh remotes and nothing else", async () => {
  const { assertEquals } = await import("jsr:@std/assert@1.0.19");
  assertEquals(githubRepo("https://github.com/owner/name\n"), "owner/name");
  assertEquals(githubRepo("https://github.com/owner/name.git"), "owner/name");
  assertEquals(githubRepo("git@github.com:owner/some.name.git"), "owner/some.name");
  assertEquals(githubRepo("https://example.com/owner/name"), undefined);
});
