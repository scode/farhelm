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
 *       [project ...]                  shard the projects (default: all) over clones of the template, collect results;
 *                                      --grep and --repeat narrow a rerun and apply to every shard
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
const HOME = "/home/tl-user";
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

async function create(scratch: string, name: string, disk: string, template?: string) {
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
    await $`tl sbx cp ${pack}/${f} ${name}:${HOME}/stage/${f}`.quiet();
  }
  for (const f of ["bootstrap.sh", "run-shard.sh"]) {
    await $`tl sbx cp ${HERE}/${f} ${name}:${HOME}/stage/${f}`.quiet();
  }
  const unpack = `cd ${HOME}/farhelm && rm -rf crates e2e/tests .git && tar -xzf ../stage/src.tar.gz ` +
    `&& tar -xzf ../stage/bin.tar.gz && tar -xzf ../stage/git.tar.gz`;
  await $`tl sbx exec ${name} sh -c ${unpack}`;
}

/** Snapshot table rows as [snapshot ID, source sandbox ID, status]. The CLI prints a box-drawn table. */
export function snapshotRows(listing: string): [string, string, string][] {
  return listing.split("\n").filter((l) => l.includes("┆")).map((l) => l.replace(/^[│\s]+|[│\s]+$/g, "").split("┆"))
    .map((cells) => cells.map((c) => c.trim())).filter((c) => c.length > 4 && c[0] !== "ID")
    .map((c) => [c[0], c[4], c[1]]);
}

async function sweep(scratch: string, keep: string[]) {
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

function allProjects() {
  const specs = [...Deno.readDirSync(`${REPO}/e2e/tests`)].map((e) => e.name).filter((n) => n.endsWith(".spec.ts"))
    .map((n) => n.slice(0, -".spec.ts".length)).sort();
  return ["chromium", "webkit"].flatMap((e) => specs.map((s) => `${e}-${s}`));
}

const specLines = (spec: string) => Deno.readTextFileSync(`${REPO}/e2e/tests/${spec}.spec.ts`).split("\n").length;

/** One shard on its own clone: run it in one attached exec (services and processes do not outlive an exec), bring back
 * the summary and console output, and for a failed shard its retained run directories, then terminate the clone.
 * `extra` holds Playwright arguments for every shard (a grep, a repeat count), passed after `--`. */
async function shard(
  scratch: string,
  pack: string,
  template: string,
  name: string,
  label: string,
  projects: string[],
  extra: string[],
  out: string,
) {
  await create(scratch, name, CLONE_DISK_MB, template);
  try {
    await provision(name, pack);
    const tail = extra.length ? ["--", ...extra] : [];
    const r = await $`tl sbx exec -t 14400 ${name} bash ${HOME}/stage/run-shard.sh ${label} ${projects} ${tail}`
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
    console.log(`${label} (${name}): exit ${r.code}, ${projects.length} projects`);
    return r.code;
  } finally {
    await $`tl sbx terminate ${name}`.noThrow().quiet();
  }
}

async function freeNames(prefix: string, n: number) {
  const taken = new Set((await $`tl sbx ls`.text()).split(/\s+/));
  const names: string[] = [];
  for (let i = 1; names.length < n; i++) if (!taken.has(`${prefix}${i}`)) names.push(`${prefix}${i}`);
  return names;
}

// --- subcommands

async function run(a: Record<string, string>, projects: string[]) {
  const pack_ = await pack(a.scratch);
  const shards = balance(projects.length ? projects : allProjects(), Number(a.shards), specLines);
  const names = await freeNames(a.prefix, shards.length);
  const extra = [...(a.grep ? ["-g", a.grep] : []), ...(a.repeat ? [`--repeat-each=${a.repeat}`] : [])];
  console.log(`${shards.length} shards`);
  try {
    const codes = await Promise.all(
      shards.map((s, i) => shard(a.scratch, pack_, a.template, names[i], `shard${i + 1}`, s, extra, a.out)),
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
    string: ["scratch", "name", "template", "prefix", "shards", "out", "keep", "repo", "grep", "repeat"],
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
