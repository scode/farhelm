#!/usr/bin/env -S deno run -A
/**
 * Runs Farhelm's browser suite, or chosen projects of it, across Tensorlake sandboxes. Not the default way to test:
 * use it only when the maintainer asks for Tensorlake (`.agents/tensorlake.md` says when and how, and how to rebuild the
 * template by hand).
 *
 * Every browser run boots exactly three stacks (`e2e/stack-slots.ts`), so the suite takes hours on one shared machine.
 * Each sandbox runs its own three, so N sandboxes run 3N stacks at once.
 *
 * Subcommands (all take `--scratch <dir>`, where packs, the ledger and results live, outside the checkout):
 *   make-template --name <sandbox>     bootstrap a fresh sandbox, prove it with a probe shard, snapshot it, print the ID
 *   run --template <snapshot> --prefix <name-prefix> --shards <n> --out <dir> [--grep <title>] [--repeat <n>]
 *       [--suite-terminals tmux] [project ...]
 *                                      shard the projects (default: all) over clones of the template, collect results;
 *                                      --grep and --repeat narrow a rerun and apply to every shard; --reuse-pack
 *                                      sends the previous run's pack (the same build) instead of packing again;
 *                                      --suite-terminals tmux runs the suite's old-terminal set on tmux stacks
 *                                      (e2e/suite-terminals.ts), so only that set's projects exist
 *   sweep [--keep <snapshot>]          terminate ledger sandboxes still alive, delete snapshots of ledger sandboxes
 *
 * The sandbox-side work is in `bootstrap.sh` and `run-shard.sh` beside this file. They are Bash because they run on the
 * stock sandbox image, which has no Deno.
 *
 * Every sandbox this script creates is recorded in `<scratch>/ledger.jsonl` (name, ID, disk) before it is used:
 * snapshots outlive their sandboxes, and once a sandbox is gone its ID is the only way to tell which snapshots it left.
 */
import $ from "jsr:@david/dax@0.50.1";
import { parseArgs } from "jsr:@std/cli@1.0.32/parse-args";
import { assertEquals } from "jsr:@std/assert@1.0.19";

const HERE = import.meta.dirname!;
// The checkout this script lives in; `--repo` overrides it for a copy run from elsewhere.
let REPO = `${HERE}/../..`;
export const HOME = "/home/tl-user";
const CLONE_DISK_MB = "30720"; // room for several shards' retained traces; a clone cannot be smaller than the template
const TEMPLATE_DISK_MB = "10240"; // the platform minimum; the bootstrapped image uses about 4 GiB

type LedgerRow = { name: string; id: string; disk_mb: string; template?: string };

// --- packing: what a sandbox needs from the local checkout

/** Packs the tested tree (tracked files only: untracked leftovers in the checkout must not reach the tested tree), the
 * locally built binaries, web bundle and pinned tmux, and a one-commit Git checkout.
 * The sandbox image's glibc matches Ubuntu 24.04, so the debug binaries run there unchanged. The recorder identifies the
 * tested commit and the pinned tmux through Git, so a shallow clone of the local HEAD goes along; the sandbox never
 * contacts the repository's remote. */
async function pack(scratch: string) {
  const out = `${scratch}/pack`;
  await $`rm -rf ${out}/shallow`;
  await $`mkdir -p ${out}`;
  await $`git ls-files -z --cached | tar --null -czf ${out}/src.tar.gz -T -`.cwd(REPO)
    .pipefail();
  await $`tar -czf ${out}/bin.tar.gz target/debug/farhelm target/debug/farhelm-fixtures \
    target/dx/farhelm-ui/release/web/public .ci-tmux`.cwd(REPO);
  await $`git clone -q --depth 1 ${"file://" + Deno.realPathSync(REPO)} ${out}/shallow`;
  await $`tar -czf ${out}/git.tar.gz -C ${out}/shallow .git`;
  await $`rm -rf ${out}/shallow`;
  return out;
}

// --- sandboxes and the ledger

export async function create(scratch: string, name: string, disk: string, template?: string) {
  const fromSnapshot = template ? ["-s", template] : ["-c", "4", "-m", "8192"];
  const id = (await $`tl sbx create ${fromSnapshot} --timeout 60 --disk_mb ${disk} ${name}`.text()).trim().split(/\s+/)
    .pop()!;
  const row: LedgerRow = { name, id, disk_mb: disk, ...(template ? { template } : {}) };
  await Deno.writeTextFile(`${scratch}/ledger.jsonl`, JSON.stringify(row) + "\n", { append: true });
  return id;
}

async function ledger(scratch: string): Promise<LedgerRow[]> {
  const text = await Deno.readTextFile(`${scratch}/ledger.jsonl`).catch((e) => {
    if (e instanceof Deno.errors.NotFound) return "";
    throw e;
  });
  return text.split("\n").filter((l) => l.trim()).map((l) => JSON.parse(l));
}

/** Pushes the packs and the sandbox scripts, and unpacks them over any earlier tree (a clone carries the template's). */
async function provision(name: string, pack: string) {
  await $`tl sbx exec ${name} mkdir -p ${HOME}/stage ${HOME}/farhelm`;
  for (const f of ["src.tar.gz", "bin.tar.gz", "git.tar.gz"]) {
    await uploadChunked(name, `${pack}/${f}`, `${HOME}/stage/${f}`);
  }
  for (const f of ["bootstrap.sh", "run-shard.sh"]) {
    await $`tl sbx cp ${HERE}/${f} ${name}:${HOME}/stage/${f}`.quiet();
  }
  const unpack = `cd ${HOME}/farhelm && rm -rf crates e2e/tests .git && tar -xzf ../stage/src.tar.gz ` +
    `&& tar -xzf ../stage/bin.tar.gz && tar -xzf ../stage/git.tar.gz`;
  await $`tl sbx exec ${name} sh -c ${unpack}`;
}

/** Uploads a file to a sandbox as 100 MiB chunks over up to 12 parallel `tl sbx cp` calls, then reassembles it and
 * checks its size. Measured from a development host on 2026-11-08, one `tl sbx cp` moved about 0.7 MiB/s however fast
 * the link was, and eleven at once moved about 8 MiB/s in total: the cap is per transfer, so a 1 GiB file takes minutes
 * this way instead of half an hour. Files of one chunk or less go in a single call. */
export async function uploadChunked(name: string, file: string, dest: string) {
  const size = Deno.statSync(file).size;
  const chunk = 100 * 1024 * 1024;
  if (size <= chunk) {
    await $`tl sbx cp ${file} ${name}:${dest}`.quiet();
    return;
  }
  const dir = await Deno.makeTempDir({ prefix: "tl-upload-" });
  try {
    await $`split -b ${String(chunk)} -d -a 3 ${file} ${dir}/part.`;
    const parts = [...Deno.readDirSync(dir)].map((e) => e.name).sort();
    await $`tl sbx exec ${name} mkdir -p ${dest}.parts`;
    const queue = [...parts];
    await Promise.all(Array.from({ length: Math.min(12, parts.length) }, async () => {
      for (let p = queue.shift(); p; p = queue.shift()) await $`tl sbx cp ${dir}/${p} ${name}:${dest}.parts/${p}`.quiet();
    }));
    await $`tl sbx exec ${name} sh -c ${`cat ${dest}.parts/part.* > ${dest} && rm -rf ${dest}.parts`}`;
    const got = Number((await $`tl sbx exec ${name} stat -c %s ${dest}`.text()).trim());
    if (got !== size) throw new Error(`${dest} on ${name}: reassembled ${got} bytes, expected ${size}`);
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
}

/** Snapshot table rows as [snapshot ID, source sandbox ID, status]. The CLI prints a box-drawn table. */
export function snapshotRows(listing: string): [string, string, string][] {
  return listing.split("\n").filter((l) => l.includes("┆")).map((l) => l.replace(/^[│\s]+|[│\s]+$/g, "").split("┆"))
    .map((cells) => cells.map((c) => c.trim())).filter((c) => c.length > 4 && c[0] !== "ID")
    .map((c) => [c[0], c[4], c[1]]);
}

export async function sweep(scratch: string, keep: string[]) {
  const rows = (await ledger(scratch)).filter((r) => r.id); // template rows record a snapshot, not a sandbox
  const alive = await $`tl sbx ls`.text();
  for (const r of rows) if (alive.includes(r.id)) await $`tl sbx terminate ${r.name}`.noThrow();
  const ids = new Set(rows.map((r) => r.id));
  for (const [snap, source] of snapshotRows(await $`tl sbx checkpoint ls`.text())) {
    if (ids.has(source) && !keep.includes(snap)) await $`tl sbx checkpoint rm ${snap}`.noThrow();
  }
}

// --- sharding

/** Spreads projects over at most n shards, heaviest first onto the lightest shard. A project's weight is its spec
 * file's length in lines, a rough proxy for run time, and WebKit counts heavier because it runs the same spec slower. */
export function balance(projects: string[], n: number, lines: (spec: string) => number): string[][] {
  const weight = (p: string) => lines(p.slice(p.indexOf("-") + 1)) * (p.startsWith("webkit-") ? 1.5 : 1);
  const shards: string[][] = Array.from({ length: n }, () => []);
  const loads = new Array(n).fill(0);
  for (const p of [...projects].sort((a, b) => weight(b) - weight(a))) {
    const i = loads.indexOf(Math.min(...loads));
    shards[i].push(p);
    loads[i] += weight(p);
  }
  return shards.filter((s) => s.length);
}

/** The old-terminal set's account (e2e/suite-terminals.ts), as much of it as the project list needs. */
export type OldTerminalSet = { files: ReadonlySet<string>; tagged: ReadonlySet<string> };

/** Loads the old-terminal set's account from the tested checkout (`REPO`, after any `--repo`), not from beside this
 * script: a copy run from elsewhere must shard the target checkout's suite, and `sweep` must not need a suite at all. */
async function oldTerminalSet(): Promise<OldTerminalSet> {
  const m = await import(`file://${Deno.realPathSync(`${REPO}/e2e/suite-terminals.ts`)}`);
  return { files: new Set(m.OLD_TERMINAL_FILES.keys()), tagged: m.OLD_TERMINAL_TAGGED };
}

/** Every project of a run, as playwright.config.ts makes them: the ordinary run has every spec file but the whole
 * old-terminal ones, and the old-terminal run (`tmux`) has only the whole ones and those with tagged tests. `specs`
 * are the spec file names under e2e/tests. */
export function allProjects(specs: string[], tmux: boolean, old: OldTerminalSet): string[] {
  const names = specs.filter((n) => n.endsWith(".spec.ts"))
    .filter((n) => tmux ? old.files.has(n) || old.tagged.has(n) : !old.files.has(n))
    .map((n) => n.slice(0, -".spec.ts".length)).sort();
  return ["chromium", "webkit"].flatMap((e) => names.map((s) => `${e}-${s}`));
}

const specLines = (spec: string) => Deno.readTextFileSync(`${REPO}/e2e/tests/${spec}.spec.ts`).split("\n").length;

/** One shard on its own clone: run it in one attached exec (what an exec leaves running cannot be relied on
 * afterwards), bring back the summary and console output, and for a failed shard its retained run directories, then
 * terminate the clone.
 * `extra` holds Playwright arguments for every shard (a grep, a repeat count), passed after `--`; `env` holds
 * `KEY=VALUE` settings for the shard script's environment (the suite's terminal kind), which it hands on to the run. */
async function shard(
  scratch: string,
  pack: string,
  template: string,
  name: string,
  label: string,
  projects: string[],
  extra: string[],
  env: string[],
  out: string,
) {
  await create(scratch, name, CLONE_DISK_MB, template);
  try {
    await provision(name, pack);
    const tail = extra.length ? ["--", ...extra] : [];
    const envArgs = env.flatMap((e) => ["-e", e]);
    const r = await $`tl sbx exec -t 14400 ${envArgs} ${name} bash ${HOME}/stage/run-shard.sh ${label} ${projects} ${tail}`
      .noThrow().captureCombined();
    await $`mkdir -p ${out}/${label}`;
    await Deno.writeTextFile(`${out}/${label}/exec.log`, r.combined.slice(-20000));
    for (const f of ["summary.txt", "console.log"]) {
      await $`tl sbx cp ${name}:${HOME}/out/${label}/${f} ${out}/${label}/${f}`.noThrow().quiet();
    }
    if (r.code !== 0) {
      await $`tl sbx exec ${name} tar -czf ${HOME}/out/${label}/runs.tar.gz -C /tmp/farhelm-tmp test-runs`.noThrow();
      await $`tl sbx cp ${name}:${HOME}/out/${label}/runs.tar.gz ${out}/${label}/runs.tar.gz`.noThrow().quiet();
    }
    // An old-terminal shard whose script never saw the variable runs the ordinary suite's projects of those files on
    // new terminals and can pass; its summary's `suite terminals:` line is the shard script's own word that it kept
    // the variable for the run.
    let code = r.code;
    const kind = env.find((e) => e.startsWith("FARHELM_E2E_SUITE_TERMINALS="))?.split("=")[1];
    if (kind && !summaryNamesSuiteTerminals(await Deno.readTextFile(`${out}/${label}/summary.txt`).catch(() => ""), kind)) {
      console.log(`${label} (${name}): its summary does not say "suite terminals: ${kind}"; failing the shard`);
      code = code || 1;
    }
    console.log(`${label} (${name}): exit ${code}, ${projects.length} projects`);
    return code;
  } finally {
    await $`tl sbx terminate ${name}`.noThrow().quiet();
  }
}

/** Whether a shard's summary records that its run kept the suite's terminal kind (`run-shard.sh` writes the line). */
export function summaryNamesSuiteTerminals(summary: string, kind: string): boolean {
  return summary.split("\n").some((line) => line.trim() === `suite terminals: ${kind}`);
}

export async function freeNames(prefix: string, n: number) {
  const taken = new Set((await $`tl sbx ls`.text()).split(/\s+/));
  const names: string[] = [];
  for (let i = 1; names.length < n; i++) if (!taken.has(`${prefix}${i}`)) names.push(`${prefix}${i}`);
  return names;
}

// --- subcommands

async function run(a: Record<string, string>, projects: string[]) {
  // The main suite's terminal kind (e2e/suite-terminals.ts): only `tmux`, the old-terminal set, is a choice; unset is
  // the ordinary new-terminal run.
  if (a["suite-terminals"] && a["suite-terminals"] !== "tmux") throw new Error("--suite-terminals takes only tmux");
  // A rerun of failures must test the same build. Rust test builds relink `target/debug/farhelm` with test features, so
  // packing again while or after they run would send a different binary; --reuse-pack keeps the earlier pack.
  const pack_ = a["reuse-pack"] ? `${a.scratch}/pack` : await pack(a.scratch);
  const specs = [...Deno.readDirSync(`${REPO}/e2e/tests`)].map((e) => e.name);
  const chosen = projects.length ? projects : allProjects(specs, !!a["suite-terminals"], await oldTerminalSet());
  const shards = balance(chosen, Number(a.shards), specLines);
  const names = await freeNames(a.prefix, shards.length);
  const extra = [...(a.grep ? ["-g", a.grep] : []), ...(a.repeat ? [`--repeat-each=${a.repeat}`] : [])];
  const env = a["suite-terminals"] ? [`FARHELM_E2E_SUITE_TERMINALS=${a["suite-terminals"]}`] : [];
  console.log(`${shards.length} shards`);
  try {
    const codes = await Promise.all(
      shards.map((s, i) => shard(a.scratch, pack_, a.template, names[i], `shard${i + 1}`, s, extra, env, a.out)),
    );
    console.log(codes.every((c) => c === 0) ? "ALL SHARDS PASSED" : "SOME SHARDS FAILED");
  } finally {
    await sweep(a.scratch, [a.template]);
  }
}

async function makeTemplate(a: Record<string, string>) {
  const pack_ = await pack(a.scratch);
  const id = await create(a.scratch, a.name, TEMPLATE_DISK_MB);
  try {
    await provision(a.name, pack_);
    await $`tl sbx exec -t 1800 ${a.name} bash ${HOME}/stage/bootstrap.sh`;
    const probe = await $`tl sbx exec -t 1800 ${a.name} bash ${HOME}/stage/run-shard.sh probe \
      chromium-welcome webkit-welcome`.noThrow().captureCombined();
    console.log(probe.combined.slice(-2000));
    if (probe.code !== 0) throw new Error("the probe shard failed; the sandbox is left for inspection until sweep");
    await $`tl sbx exec ${a.name} rm -rf ${HOME}/out /tmp/farhelm-tmp`;
    const snap = (await $`tl sbx checkpoint ${a.name}`.text()).trim().split(/\s+/).pop()!;
    while (!snapshotRows(await $`tl sbx checkpoint ls`.text()).some(([s, , st]) => s === snap && st === "completed")) {
      await $.sleep(2000);
    }
    await Deno.writeTextFile(`${a.scratch}/ledger.jsonl`, JSON.stringify({ template: snap, from: id }) + "\n", {
      append: true,
    });
    console.log(`template snapshot: ${snap}`);
    await sweep(a.scratch, [snap]);
  } catch (e) {
    console.error(`template not made; run sweep when done inspecting ${a.name}`);
    throw e;
  }
}

if (import.meta.main) {
  const args = parseArgs(Deno.args, {
    string: ["scratch", "name", "template", "prefix", "shards", "out", "keep", "repo", "grep", "repeat", "suite-terminals"],
    boolean: ["reuse-pack"],
  });
  const [cmd, ...rest] = args._.map(String);
  const a = args as unknown as Record<string, string>;
  if (!a.scratch) throw new Error("--scratch <dir> is required");
  if (a.repo) REPO = a.repo;
  await $`mkdir -p ${a.scratch}`;
  if (cmd === "make-template") await makeTemplate(a);
  else if (cmd === "run") await run(a, rest);
  else if (cmd === "sweep") await sweep(a.scratch, a.keep ? [a.keep] : []);
  else throw new Error("usage: sandbox-tests.ts make-template|run|sweep --scratch <dir> ...");
}

Deno.test("balance puts the heaviest projects on separate shards and weights WebKit heavier", () => {
  const lines = (s: string) => ({ a: 100, b: 100, c: 10 } as Record<string, number>)[s];
  assertEquals(balance(["chromium-a", "webkit-b", "chromium-c"], 2, lines), [["webkit-b"], [
    "chromium-a",
    "chromium-c",
  ]]);
  assertEquals(balance(["chromium-c"], 3, lines), [["chromium-c"]]);
});

Deno.test("snapshotRows reads snapshot and source sandbox IDs from the CLI's table", () => {
  const listing = [
    "│ ID       ┆ Status    ┆ Kind       ┆ Store ┆ Sandbox ID ┆ Image │",
    "│ snapA    ┆ completed ┆ filesystem ┆ CAS   ┆ sbx1       ┆ img   │",
    "│ suspend-x ┆ completed ┆ memory    ┆ CAS   ┆ sbx2       ┆ img   │",
  ].join("\n");
  assertEquals(snapshotRows(listing), [["snapA", "sbx1", "completed"], ["suspend-x", "sbx2", "completed"]]);
});

Deno.test("summaryNamesSuiteTerminals accepts only the shard script's exact line for the kind", () => {
  const summary = "systemd --user: running\nself-ssh: ok\nsuite terminals: tmux\nexit: 0\n";
  assertEquals(summaryNamesSuiteTerminals(summary, "tmux"), true);
  assertEquals(summaryNamesSuiteTerminals("systemd --user: running\nexit: 0\n", "tmux"), false);
  assertEquals(summaryNamesSuiteTerminals("suite terminals: host\n", "tmux"), false);
});

/** Why: a shard handed a project the config does not make fails with "project not found", and one missing a project
 * silently drops its coverage, so the list must match playwright.config.ts's for both runs. Spec: the ordinary run takes
 * every spec file but the whole old-terminal ones (a file with tagged tests stays); the tmux run takes only the whole
 * and the tagged files; non-spec entries never become projects. */
Deno.test("allProjects leaves the old-terminal set's whole files out of the ordinary run and runs only the set on tmux", () => {
  const specs = ["a.spec.ts", "old.spec.ts", "mixed.spec.ts", "helpers", "notes.md"];
  const old: OldTerminalSet = { files: new Set(["old.spec.ts"]), tagged: new Set(["mixed.spec.ts"]) };
  assertEquals(allProjects(specs, false, old), ["chromium-a", "chromium-mixed", "webkit-a", "webkit-mixed"]);
  assertEquals(allProjects(specs, true, old), ["chromium-mixed", "chromium-old", "webkit-mixed", "webkit-old"]);
});

/** Why: the account is loaded at run time from the tested checkout by a dynamic import, which nothing else exercises
 * before a sandbox is paid for. Spec: this checkout's e2e/suite-terminals.ts loads and every file it names exists. */
Deno.test("the checkout's old-terminal set loads and names spec files that exist", async () => {
  const old = await oldTerminalSet();
  const specs = new Set([...Deno.readDirSync(`${REPO}/e2e/tests`)].map((e) => e.name));
  for (const name of [...old.files, ...old.tagged]) assertEquals(specs.has(name), true, name);
});
