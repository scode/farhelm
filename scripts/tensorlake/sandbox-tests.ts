#!/usr/bin/env -S deno run -A
/**
 * Builds the Tensorlake sandbox template that Farhelm's Rust test runner (`rust-tests.ts` beside this file) clones, and
 * holds the sandbox bookkeeping that runner shares: creating sandboxes into the ledger, chunked uploads, and the sweep.
 * Not the default way to test: use it only when the maintainer asks for Tensorlake (`.agents/tensorlake.md` says when
 * and how, and how to rebuild the template by hand).
 *
 * Subcommands (all take `--scratch <dir>`, where the ledger lives, outside the checkout):
 *   make-template --name <sandbox>     bootstrap a fresh sandbox, snapshot it, print the snapshot ID
 *   sweep [--keep <snapshot>]          terminate ledger sandboxes still alive, delete snapshots of ledger sandboxes
 *
 * The sandbox-side work is in `bootstrap.sh` beside this file. It is Bash because it runs on the stock sandbox image,
 * which has no Deno.
 *
 * Every sandbox this script creates is recorded in `<scratch>/ledger.jsonl` (name, ID, disk) before it is used:
 * snapshots outlive their sandboxes, and once a sandbox is gone its ID is the only way to tell which snapshots it left.
 */
import $ from "jsr:@david/dax@0.50.1";
import { parseArgs } from "jsr:@std/cli@1.0.32/parse-args";
import { assertEquals } from "jsr:@std/assert@1.0.19";

const HERE = import.meta.dirname!;
export const HOME = "/home/tl-user";
const TEMPLATE_DISK_MB = "10240"; // the platform minimum; a clone cannot be smaller than the template

type LedgerRow = { name: string; id: string; disk_mb: string; template?: string };

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

export async function freeNames(prefix: string, n: number) {
  const taken = new Set((await $`tl sbx ls`.text()).split(/\s+/));
  const names: string[] = [];
  for (let i = 1; names.length < n; i++) if (!taken.has(`${prefix}${i}`)) names.push(`${prefix}${i}`);
  return names;
}

// --- subcommands

/** Bootstraps a fresh sandbox and snapshots it. The template sandbox boots Tensorlake's own init, not systemd, so it
 * cannot run a Rust shard (`run-rust-shard.sh` refuses one without systemd as PID 1). Nothing here proves the template
 * beyond `bootstrap.sh` finishing: the first Rust shard on a clone does, through what its `summary.txt` records. */
async function makeTemplate(a: Record<string, string>) {
  const id = await create(a.scratch, a.name, TEMPLATE_DISK_MB);
  try {
    await $`tl sbx exec ${a.name} mkdir -p ${HOME}/stage`;
    await $`tl sbx cp ${HERE}/bootstrap.sh ${a.name}:${HOME}/stage/bootstrap.sh`.quiet();
    await $`tl sbx exec -t 1800 ${a.name} bash ${HOME}/stage/bootstrap.sh`;
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
  const args = parseArgs(Deno.args, { string: ["scratch", "name", "keep"] });
  const [cmd] = args._.map(String);
  const a = args as unknown as Record<string, string>;
  if (!a.scratch) throw new Error("--scratch <dir> is required");
  await $`mkdir -p ${a.scratch}`;
  if (cmd === "make-template") await makeTemplate(a);
  else if (cmd === "sweep") await sweep(a.scratch, a.keep ? [a.keep] : []);
  else throw new Error("usage: sandbox-tests.ts make-template|sweep --scratch <dir> ...");
}

Deno.test("snapshotRows reads snapshot and source sandbox IDs from the CLI's table", () => {
  const listing = [
    "│ ID       ┆ Status    ┆ Kind       ┆ Store ┆ Sandbox ID ┆ Image │",
    "│ snapA    ┆ completed ┆ filesystem ┆ CAS   ┆ sbx1       ┆ img   │",
    "│ suspend-x ┆ completed ┆ memory    ┆ CAS   ┆ sbx2       ┆ img   │",
  ].join("\n");
  assertEquals(snapshotRows(listing), [["snapA", "sbx1", "completed"], ["suspend-x", "sbx2", "completed"]]);
});
