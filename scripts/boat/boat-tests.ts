#!/usr/bin/env -S deno run -A
/**
 * Runs Farhelm's slow test suites on a pool of boat sandboxes (https://docs.boat.dev): hash partitions of a locally
 * built Rust nextest archive, and the GPUI app's end-to-end suites against locally built binaries. Builds and lints stay
 * on the development machine; only test execution moves. `.agents/boat.md` says when to use it and how.
 *
 * Why a pool rather than a sandbox per shard: boat charges per sandbox start (the $20 plan includes 12 a minute, 60 an
 * hour and 200 a day; past that each costs 3-7 cents or is refused), while running time is cheap, and boat has no
 * snapshot templates to clone from, so every new sandbox pays half a minute or more of bootstrap. So sandboxes are
 * created once, bootstrapped once (`sandbox/bootstrap.sh`), and reused by every later run until `down`; each run resets
 * the sandbox it uses (`sandbox/reset.sh`). The pool lives in `<scratch>/pool.json`; every sandbox in it was created by
 * this script for the caller, named `<prefix><n>`. Sandboxes the pool does not list are never touched.
 *
 * Concurrent runs (a Rust run and a GUI run at once) share the pool: each run claims members through lock files in
 * `<scratch>/locks/`, so they never use the same sandbox, and the pool never grows beyond `--budget` (default 4).
 *
 * Subcommands (all take `--scratch <dir>`, outside the checkout):
 *   up --count <n> [--type large]       make sure n pool members exist and are bootstrapped
 *   rust --shards <n> --out <dir> (--archive <file> | --reuse-stage) [-- <nextest filter>]
 *                                       hash partitions of a nextest archive over n members
 *   gui --out <dir> [--no-build] [smoke] [lifecycle]
 *                                       the GPUI app's end-to-end smoke and/or Linux lifecycle on one member
 *   docs --out <dir> [--no-build] [--only <page[/shot]>]... [--members <n>] | docs --out <dir> --build-there
 *                                       the docs website's screenshots (`scripts/docs-screenshots.sh`): the whole
 *                                       capture on one member, or the named pages spread over up to n members;
 *                                       --build-there builds on the member, for a publishable capture
 *   status | extend [--ttl <s>] | down  list, push out deadlines, delete every member
 *
 * Sandboxes are created with --no-env (no account secrets reach them), --no-snapshots and --no-monitoring, and only
 * when `boat limits` says the start is free; a refused or paid start is reported, never retried.
 */
import $ from "jsr:@david/dax@0.50.1";
import { parseArgs } from "jsr:@std/cli@1.0.32/parse-args";
import { assertEquals } from "jsr:@std/assert@1.0.19";

const HERE = import.meta.dirname!;
const SANDBOX_SCRIPTS = ["bootstrap.sh", "reset.sh", "run-rust-shard.sh", "run-gui.sh", "run-docs.sh"];
const H = "/home/user";
// The checkout whose build is tested; `--repo` overrides it for a copy of this script run from elsewhere.
let REPO = Deno.realPathSync(`${HERE}/../..`);
let PREFIX = "";
let BUDGET = 4;
// A run that starts with less than this left before a member's hard deadline first pushes the deadline out to it. A
// sandbox stops at its deadline even mid-command, and with snapshots off that loses its disk.
const MIN_TTL_S = 3 * 3600;

type Member = { name: string; id: string; type: string; created: string };

/** The default name prefix: `farhelm-boat-<checkout>-`, the checkout directory's name lowercased with everything
 * outside [a-z0-9] turned into `-`, so two checkouts' pools never share names. */
export function defaultPrefix(repo: string): string {
  const base = repo.split("/").filter(Boolean).pop() ?? "checkout";
  return `farhelm-boat-${base.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}-`;
}

// --- boat CLI helpers

type ExecResult = { code: number; out: string; sawExit: boolean };

/** Reads the JSON-line frames `boat exec --timeout 0` prints: the command's output, its exit code, and whether the
 * stream ended with an exit frame at all. Without one the remote command may still be running. */
export function readFrames(stdout: string, localCode: number): ExecResult {
  let out = "";
  let code = localCode;
  let sawExit = false;
  for (const line of stdout.split("\n")) {
    if (!line.trim()) continue;
    try {
      const f = JSON.parse(line);
      if (f.type === "stdout" || f.type === "stderr") out += f.data;
      if (f.type === "exit") {
        sawExit = true;
        code = f.exitCode ?? (f.signal ? 128 : code);
      }
    } catch {
      out += line + "\n";
    }
  }
  return { code, out, sawExit };
}

/** Runs a command on a sandbox in one attached exec with no time limit (boat's default kills a command after 30 s, and
 * any bound would end a slow run on a clock). A member whose exec ended without an exit frame must not be reused
 * (`markDirty`). */
async function bexec(id: string, cmd: string[]): Promise<ExecResult> {
  const r = await $`boat exec --timeout 0 ${id} -- ${cmd}`.noThrow().stdout("piped").stderr("piped");
  const res = readFrames(r.stdout, r.code);
  if (!res.sawExit) {
    res.out += `\n[boat exec ended without an exit frame; local code ${r.code}]\n${r.stderr.slice(-2000)}`;
  }
  return res;
}

async function bexecOk(m: Member, cmd: string[]) {
  const r = await bexec(m.id, cmd);
  if (!r.sawExit) await markDirty(m);
  if (r.code !== 0) throw new Error(`${m.name}: ${cmd.join(" ")} exited ${r.code}\n${r.out.slice(-3000)}`);
  return r.out;
}

async function put(m: Member, local: string, remote: string) {
  await $`boat scp ${local} ${m.id}:${remote}`.quiet();
}

async function get(m: Member, remote: string, local: string) {
  await $`boat scp ${m.id}:${remote} ${local}`.noThrow().quiet();
}

async function info(id: string): Promise<Record<string, unknown> | null> {
  const r = await $`boat info ${id} --json`.noThrow().stdout("piped").stderr("piped");
  if (r.code !== 0) return null;
  try {
    // `boat info --json` wraps the record: {"sandbox": {...}}.
    const j = JSON.parse(r.stdout);
    return j.sandbox ?? j;
  } catch {
    return null;
  }
}

// --- the pool file and member locks

let SCRATCH = "";
const poolFile = () => `${SCRATCH}/pool.json`;

function readPool(): Member[] {
  try {
    return JSON.parse(Deno.readTextFileSync(poolFile()));
  } catch (e) {
    if (e instanceof Deno.errors.NotFound) return [];
    throw e;
  }
}

function writePool(pool: Member[]) {
  Deno.writeTextFileSync(poolFile(), JSON.stringify(pool, null, 1));
}

function alive(pid: number) {
  try {
    Deno.statSync(`/proc/${pid}`);
    return true;
  } catch {
    return false;
  }
}

/** Pool updates go through one lock so concurrent runs do not lose each other's members. */
async function withPoolLock<T>(f: () => Promise<T>): Promise<T> {
  const lock = `${SCRATCH}/pool.lock`;
  for (;;) {
    try {
      Deno.writeTextFileSync(lock, String(Deno.pid), { createNew: true });
      break;
    } catch (e) {
      if (!(e instanceof Deno.errors.AlreadyExists)) throw e;
      const pid = Number(Deno.readTextFileSync(lock).trim());
      if (!pid || !alive(pid)) Deno.removeSync(lock);
      else await $.sleep(500);
    }
  }
  try {
    return await f();
  } finally {
    Deno.removeSync(lock);
  }
}

const lockPath = (m: Member) => `${SCRATCH}/locks/${m.name}`;

function tryLock(m: Member): boolean {
  Deno.mkdirSync(`${SCRATCH}/locks`, { recursive: true });
  try {
    Deno.writeTextFileSync(lockPath(m), String(Deno.pid), { createNew: true });
    return true;
  } catch (e) {
    if (!(e instanceof Deno.errors.AlreadyExists)) throw e;
    const pid = Number(Deno.readTextFileSync(lockPath(m)).trim());
    if (pid && alive(pid)) return false;
    Deno.writeTextFileSync(lockPath(m), String(Deno.pid));
    return true;
  }
}

function unlock(m: Member) {
  try {
    Deno.removeSync(lockPath(m));
  } catch { /* already gone */ }
}

/** A member whose exec ended without an exit frame may still be running that command: delete it rather than reuse it. */
async function markDirty(m: Member) {
  console.log(`${m.name}: an exec ended without its exit frame; deleting the sandbox rather than reusing it`);
  await $`boat delete ${m.id} --yes`.noThrow().quiet();
  await withPoolLock(async () => writePool(readPool().filter((x) => x.id !== m.id)));
}

// --- creating and bootstrapping members

/** Creates `count` members after checking that every start is free. Never retries a refused or paid start. */
async function create(count: number, type: string): Promise<Member[]> {
  const lim = JSON.parse(await $`boat limits --json`.text());
  const remaining = Math.min(lim.starts.minute.remaining, lim.starts.hour.remaining, lim.starts.day.remaining);
  if (!lim.canStart || lim.nextStartDollars !== 0 || remaining < count) {
    throw new Error(
      `boat starts are not free for ${count} sandboxes (canStart ${lim.canStart}, next start $${lim.nextStartDollars}, ` +
        `remaining ${remaining}, window frees at ${lim.startWindowFull?.freesAt ?? "n/a"}); not creating`,
    );
  }
  const listing = JSON.parse(await $`boat list --all --json`.text());
  const taken = new Set<string>(listing.sandboxes.map((s: { name: string }) => s.name));
  for (const m of readPool()) taken.add(m.name);
  const names: string[] = [];
  for (let i = 1; names.length < count; i++) if (!taken.has(`${PREFIX}${i}`)) names.push(`${PREFIX}${i}`);
  return await Promise.all(names.map(async (name) => {
    const r = await $`boat new --no-env --no-snapshots --no-monitoring --type ${type} --ttl ${
      String(MIN_TTL_S)
    } --name ${name} --json`.noThrow().stdout("piped").stderr("piped");
    const frames = r.stdout.split("\n").filter((l) => l.trim()).map((l) => JSON.parse(l));
    const id = frames.find((f) => f.id)?.id;
    if (!id) throw new Error(`boat new ${name} failed: ${r.stderr.slice(-1000)}`);
    const m = { name, id, type, created: new Date().toISOString() };
    await withPoolLock(async () => writePool([...readPool(), m]));
    if (!frames.some((f) => f.event === "ready")) throw new Error(`boat new ${name} (${id}) never reported ready`);
    console.log(`created ${name} ${id} (${type})`);
    return m;
  }));
}

async function pushScripts(m: Member) {
  await bexecOk(m, ["mkdir", "-p", `${H}/bin`]);
  for (const f of SANDBOX_SCRIPTS) await put(m, `${HERE}/sandbox/${f}`, `${H}/bin/${f}`);
}

async function bootstrap(m: Member) {
  await pushScripts(m);
  const out = await bexecOk(m, ["bash", `${H}/bin/bootstrap.sh`]);
  console.log(`${m.name}: bootstrapped\n${out.trim().split("\n").slice(-6).join("\n")}`);
}

/** Drops members boat no longer has running (deleted, stopped at their deadline) from the pool. */
async function prune() {
  const pool = readPool();
  const live: Member[] = [];
  for (const m of pool) {
    const state = String((await info(m.id))?.state ?? "gone");
    if (["ready", "idle", "running", "provisioned"].includes(state)) live.push(m);
    else console.log(`${m.name} ${m.id}: ${state}; dropped from the pool`);
  }
  if (live.length !== pool.length) {
    const ids = new Set(live.map((m) => m.id));
    await withPoolLock(async () => writePool(readPool().filter((m) => ids.has(m.id))));
  }
}

/** Claims n free members, creating (and bootstrapping) new ones within the budget when too few are free. */
async function claim(n: number, type: string): Promise<Member[]> {
  await prune();
  const got: Member[] = [];
  for (const m of readPool()) if (got.length < n && m.type === type && tryLock(m)) got.push(m);
  const missing = n - got.length;
  if (missing > 0) {
    const room = BUDGET - readPool().length;
    if (room < missing) {
      got.forEach(unlock);
      throw new Error(`need ${missing} more ${type} sandboxes, but the budget of ${BUDGET} leaves room for ${room}`);
    }
    let fresh: Member[];
    try {
      fresh = await create(missing, type);
    } catch (e) {
      got.forEach(unlock);
      throw e;
    }
    for (const m of fresh) tryLock(m);
    got.push(...fresh);
  }
  // bootstrap.sh is a no-op on a member that finished it, so every claim runs it: a member whose bootstrap failed gets
  // it again then.
  try {
    await Promise.all(got.map(async (m) => {
      await ensureTtl(m);
      await bootstrap(m);
    }));
  } catch (e) {
    got.forEach(unlock);
    throw e;
  }
  return got;
}

async function ensureTtl(m: Member) {
  const i = await info(m.id);
  const left = (Date.parse(String(i?.archiveAfter ?? 0)) - Date.now()) / 1000;
  if (left < MIN_TTL_S) await $`boat extend ${m.id} --ttl ${String(MIN_TTL_S)}`.quiet();
}

/** Brings back a run's summary and logs, and for a failed run the recorder's retained run directories. */
async function collect(m: Member, label: string, files: string[], r: ExecResult, out: string) {
  const dir = `${out}/${label}`;
  await $`mkdir -p ${dir}`;
  await Deno.writeTextFile(`${dir}/exec.log`, r.out.slice(-20000));
  for (const f of files) await get(m, `${H}/out/${label}/${f}`, `${dir}/${f}`);
  if (r.code !== 0 && r.sawExit) {
    await bexec(m.id, ["tar", "-czf", `${H}/out/${label}/runs.tar.gz`, "-C", "/tmp/farhelm-tmp", "test-runs"]);
    await get(m, `${H}/out/${label}/runs.tar.gz`, `${dir}/runs.tar.gz`);
  }
}

// --- packing the tree

/** The tracked source, and a one-commit clone for the scripts that read git metadata. The tree is unpacked on the
 * sandbox at this checkout's own absolute path, since test binaries have it compiled in. `untracked` adds files git
 * does not track yet but would not ignore, for a docs capture of page modules still being written. `withMain` deepens
 * the clone down to where HEAD leaves main and records main as `origin/main` there, for a docs capture that checks the
 * app code it builds against main's (`scripts/docs-screenshots.sh`). */
async function packSource(out: string, untracked = false, withMain = false) {
  const others = untracked ? ["--others", "--exclude-standard"] : [];
  await $`git ls-files -z --cached ${others} | tar --null -czf ${out}/src.tar.gz -T -`.cwd(REPO).pipefail();
  let depth = "1";
  let main = "";
  if (withMain) {
    main = (await $`git merge-base HEAD refs/remotes/origin/main`.cwd(REPO).text()).trim();
    depth = String(Number((await $`git rev-list --count ${main + "..HEAD"}`.cwd(REPO).text()).trim()) + 1);
  }
  await $`git clone -q --depth ${depth} ${"file://" + REPO} ${out}/shallow`;
  if (withMain) await $`git -C ${out}/shallow update-ref refs/remotes/origin/main ${main}`;
  await $`tar -czf ${out}/git.tar.gz -C ${out}/shallow .git`;
  await $`rm -rf ${out}/shallow`;
}

/** Replaces the sandbox's tree with the packs in `<H>/stage` (src, git, and the named extra tars, unpacked in the tree's
 * root). Any tree an earlier run left there goes, Rust or GUI alike. */
async function unpack(m: Member, extra: string[]) {
  const cmd = [
    `sudo -n rm -rf ${REPO}`,
    `sudo -n install -d -o user -g user ${REPO}`,
    `cd ${REPO}`,
    `tar -xzf ${H}/stage/src.tar.gz`,
    `tar -xzf ${H}/stage/git.tar.gz`,
    ...extra.map((f) => `tar -xzf ${H}/stage/${f}`),
  ].join(" && ");
  await bexecOk(m, ["sh", "-c", cmd]);
}

// --- Rust runs

async function nextestDir() {
  const dir = (await $`python3 scripts/install-pinned-nextest.py`.cwd(REPO).text()).trim();
  if (!dir.startsWith(`${REPO}/`)) throw new Error(`nextest installed outside the checkout: ${dir}`);
  return dir.slice(REPO.length + 1);
}

async function packRust(nextest: string, skipPodman: boolean) {
  const out = `${SCRATCH}/rust-pack`;
  await $`rm -rf ${out}`;
  await $`mkdir -p ${out}`;
  await packSource(out);
  // The e2e tests run the fixture binary beside `farhelm`; nextest's build-fixtures setup builds it locally, but the
  // shards drop that setup (no build there) and an archive does not contain it, so make sure it is built: after a
  // `cargo clean` nothing else has.
  await $`cargo build --locked -p farhelm-fixtures`.cwd(REPO);
  await $`tar -czf ${out}/tools.tar.gz ${nextest} target/debug/farhelm-fixtures`.cwd(REPO);
  // An opt-out packs an empty directory: the shard then reports the Podman image as SKIPPED.
  let payloads = `${out}/no-payloads`;
  if (skipPodman) await $`mkdir -p ${payloads}`;
  else payloads = (await $`bash scripts/build-podman-test-payloads.sh`.cwd(REPO).text()).trim();
  // Unpacked in the tree's root like the other packs, so the payloads land where the shard script points the setup.
  await $`tar -czf ${out}/payloads.tar.gz --transform ${"s,^\\./,target/podman-test-payloads/,"} -C ${payloads} .`;
  return out;
}

const RUST_STAGE = () => `${SCRATCH}/rust-stage.json`;

/** Members holding a staged Rust tree (`--reuse-stage`), minus any a later GUI run replaced. */
function rustStage(): string[] {
  try {
    return JSON.parse(Deno.readTextFileSync(RUST_STAGE()));
  } catch {
    return [];
  }
}

async function rustShard(
  m: Member,
  pack: string | null,
  archive: string | null,
  nextest: string,
  k: number,
  n: number,
  filter: string[],
  out: string,
  skipPodman: boolean,
) {
  await pushScripts(m);
  await bexecOk(m, ["bash", `${H}/bin/reset.sh`, ...(pack ? [] : ["keep-stage"])]);
  if (pack && archive) {
    for (const f of ["src.tar.gz", "git.tar.gz", "tools.tar.gz", "payloads.tar.gz"]) {
      await put(m, `${pack}/${f}`, `${H}/stage/${f}`);
    }
    const t0 = Date.now();
    await put(m, archive, `${H}/stage/archive.tar.zst`);
    console.log(`shard${k} (${m.name}): archive uploaded in ${Math.round((Date.now() - t0) / 1000)} s`);
    await unpack(m, ["tools.tar.gz", "payloads.tar.gz"]);
  }
  const r = await bexec(m.id, [
    "env",
    `FARHELM_TEST_SKIP_PODMAN=${skipPodman ? "1" : "0"}`,
    "bash",
    `${H}/bin/run-rust-shard.sh`,
    REPO,
    `${H}/stage/archive.tar.zst`,
    `${nextest}/cargo-nextest`,
    String(k),
    String(n),
    ...filter,
  ]);
  await collect(m, `shard${k}`, ["summary.txt", "console.log"], r, out);
  if (!r.sawExit) await markDirty(m);
  console.log(`shard${k} (${m.name}): exit ${r.code}`);
  return r.code;
}

async function runRust(a: Record<string, string>, filter: string[]) {
  const n = Number(a.shards);
  if (!(n >= 1)) throw new Error("--shards <n> is required");
  const skipPodman = Deno.env.get("FARHELM_TEST_SKIP_PODMAN") === "1";
  const nextest = await nextestDir();
  const reuse = Boolean(a["reuse-stage"]);
  if (!reuse && !a.archive) throw new Error("--archive is required unless --reuse-stage");
  const pack = reuse ? null : await packRust(nextest, skipPodman);
  let members: Member[];
  if (reuse) {
    // A reused stage must be on the members that hold it: the ones the previous Rust run used, recorded below.
    const names = rustStage();
    const pool = readPool();
    members = names.map((nm) => pool.find((m) => m.name === nm)).filter((m): m is Member => !!m);
    if (members.length !== n || !members.every(tryLock)) {
      members.forEach(unlock);
      throw new Error(`the staged members (${names.join(", ")}) are not all free and present`);
    }
    await Promise.all(members.map(ensureTtl));
  } else {
    members = await claim(n, a.type ?? "large");
  }
  try {
    const codes = await Promise.all(
      members.map((m, i) =>
        rustShard(m, pack, a.archive ?? null, nextest, i + 1, n, filter, a.out, skipPodman).catch((e) => {
          console.log(`shard${i + 1} (${m.name}): harness error: ${e.message}`);
          return 99;
        })
      ),
    );
    if (!reuse) Deno.writeTextFileSync(RUST_STAGE(), JSON.stringify(members.map((m) => m.name)));
    console.log(codes.every((c) => c === 0) ? "ALL SHARDS PASSED" : "SOME SHARDS FAILED");
  } finally {
    members.forEach(unlock);
  }
}

// --- GPUI app runs

/** The binaries `scripts/gui-agent.py start` uses, built by its own `build` (so the agent feature and the bridge's
 * command line come out exactly as a local run builds them), relative to the checkout. */
const GUI_BINARIES = ["farhelm-desktop", "farhelm", "farhelm-fixtures", "gpui-agent"].map((b) => `target/debug/${b}`);

async function packGui(build: boolean, untracked = false, buildThere = false) {
  // One pack directory per process: concurrent docs runs from several agents must not repack over each other.
  const out = `${SCRATCH}/gui-pack-${Deno.pid}`;
  await $`rm -rf ${out}`;
  await $`mkdir -p ${out}`;
  if (build) {
    const py = "import importlib.util as u, sys; s = u.spec_from_file_location('ga', sys.argv[1]); " +
      "m = u.module_from_spec(s); s.loader.exec_module(m); m.build(m.target_dir())";
    await $`python3 -c ${py} scripts/gui-agent.py`.cwd(REPO);
  }
  await packSource(out, untracked, buildThere);
  // A run that builds on the sandbox gets no binaries from here: what it builds is what it vouches for.
  if (buildThere) await $`tar -czf ${out}/bin.tar.gz --files-from /dev/null`;
  else await $`tar -czf ${out}/bin.tar.gz ${GUI_BINARIES}`.cwd(REPO);
  return out;
}

/** Assign all selectors for one page together so one member owns its outputs. Each share supports one app start. */
export function spread(selections: string[], members: number): string[][] {
  const pages = new Map<string, string[]>();
  for (const selection of selections) {
    const page = selection.split("/", 1)[0];
    const group = pages.get(page) ?? [];
    group.push(selection);
    pages.set(page, group);
  }
  const lists: string[][] = Array.from({ length: Math.max(1, Math.min(members, pages.size)) }, () => []);
  [...pages.values()].forEach((page, i) => lists[i % lists.length].push(...page));
  return lists;
}

/** Discard a prior publish record before merging this run's archives. Clear PNGs owned by the accepted selectors
 * so a failed or skipped shot cannot be mistaken for a fresh one. Keep the rest of a draft's previews. */
async function clearDocsOutputs(root: string, selections: string[]) {
  await $`mkdir -p ${root}`;
  await $`rm -f ${root}/capture.json`;
  for (const selection of selections.length ? selections : [""]) {
    const path = `${root}/${selection}`;
    if (selection.includes("/")) await $`rm -f ${path}.png`;
    else if ((await $`test -d ${path}`.noThrow()).code === 0) {
      await $`find ${path} -type f -name "*.png" -delete`;
    }
  }
}

/** Claims up to `n` free members, settling for fewer (at least one) when the rest of the budget is busy: a docs run with
 * several pages runs them on whatever members it can get rather than failing. */
async function claimUpTo(n: number, type: string): Promise<Member[]> {
  for (let k = n; k > 1; k--) {
    try {
      return await claim(k, type);
    } catch { /* fewer */ }
  }
  return await claim(1, type);
}

/** Each member captures its entire share with one staged app. Logs identify the share by its first page, or "all"
 * for a full capture. The common shots tree can merge archives because a page belongs to exactly one share. */
async function docsOnMember(
  m: Member,
  pack: string,
  selections: string[],
  out: string,
  mode: string,
): Promise<boolean> {
  await pushScripts(m);
  await bexecOk(m, ["bash", `${H}/bin/reset.sh`]);
  Deno.writeTextFileSync(RUST_STAGE(), JSON.stringify(rustStage().filter((nm) => nm !== m.name)));
  for (const f of ["src.tar.gz", "git.tar.gz", "bin.tar.gz"]) await put(m, `${pack}/${f}`, `${H}/stage/${f}`);
  await unpack(m, ["bin.tar.gz"]);
  const r = await bexec(m.id, ["bash", `${H}/bin/run-docs.sh`, REPO, mode, ...selections]);
  const dir = `${out}/docs/${selections[0]?.split("/", 1)[0] ?? "all"}`;
  await $`rm -rf ${dir}`;
  await $`mkdir -p ${dir} ${out}/docs/shots`;
  await Deno.writeTextFile(`${dir}/exec.log`, r.out.slice(-20000));
  for (const f of ["summary.txt", "docs.log", "shots.tar.gz"]) await get(m, `${H}/out/docs/${f}`, `${dir}/${f}`);
  const extracted = (await $`tar -xzf ${dir}/shots.tar.gz -C ${out}/docs/shots`.noThrow()).code === 0;
  const outcome = r.code !== 0 ? `FAILED (exit ${r.code})` : extracted ? "captured" : "FAILED to collect screenshots";
  console.log(
    `${m.name} ${selections.join(", ") || "all pages"}: ${outcome}`,
  );
  console.log(r.out.trim().split("\n").slice(-12).join("\n"));
  if (!r.sawExit) {
    await markDirty(m);
    return false;
  }
  return r.code === 0 && extracted;
}

/** The docs screenshots on the pool: a full capture on one member, or selected pages and shots spread over up to
 * `--members` members (default: one per selected page, within the budget). The PNGs come back into
 * `<out>/docs/shots/<page>/<shot>.png`. A run from shipped binaries is never
 * publishable, since `scripts/docs-screenshots.sh` writes its record only when it built the binaries itself; so
 * `--build-there` (whole capture only) ships no binaries and has the member build them from the shipped source, with
 * main recorded beside it for the capture's check that the app code is main's. Its record comes back as
 * `<out>/docs/shots/capture.json`, ready to publish. */
async function runDocs(a: Record<string, string>, selections: string[]) {
  const buildThere = Boolean(a["build-there"]);
  if (buildThere && (selections.length || a["no-build"])) {
    throw new Error("--build-there takes neither --only nor --no-build");
  }
  if (selections.length) {
    const flags = selections.flatMap((name) => ["--only", name]);
    await $`python3 -B ${REPO}/scripts/docs_shots.py --check-selection ${flags}`;
  }
  await clearDocsOutputs(`${a.out}/docs/shots`, selections);
  const pack = await packGui(!a["no-build"] && !buildThere, true, buildThere);
  const want = spread(selections, Math.min(Number(a.members ?? BUDGET), BUDGET)).length;
  let members: Member[] = [];
  try {
    members = await claimUpTo(want, a.type ?? "large");
    const shares = spread(selections, members.length);
    const mode = buildThere ? "build" : "shipped";
    // A failed transfer must leave the other members their locks and source until their captures finish.
    const results = await Promise.allSettled(
      shares.map((share, i) => docsOnMember(members[i], pack, share, a.out, mode)),
    );
    for (const [i, result] of results.entries()) {
      if (result.status === "rejected") console.error(`${members[i].name}: ${result.reason}`);
    }
    const ok = results.every((result) => result.status === "fulfilled" && result.value);
    console.log(ok ? `DOCS SHOTS CAPTURED into ${a.out}/docs/shots` : "DOCS SHOTS FAILED (see the lines above)");
    if (!ok) Deno.exitCode = 1;
  } finally {
    members.forEach(unlock);
    await $`rm -rf ${pack}`;
  }
}

async function runGui(a: Record<string, string>, suites: string[]) {
  const chosen = suites.length ? suites : ["smoke", "lifecycle"];
  const pack = await packGui(!a["no-build"]);
  const [m] = await claim(1, a.type ?? "large");
  try {
    await pushScripts(m);
    await bexecOk(m, ["bash", `${H}/bin/reset.sh`]);
    // The GUI tree replaces any staged Rust tree on this member.
    Deno.writeTextFileSync(RUST_STAGE(), JSON.stringify(rustStage().filter((nm) => nm !== m.name)));
    for (const f of ["src.tar.gz", "git.tar.gz", "bin.tar.gz"]) await put(m, `${pack}/${f}`, `${H}/stage/${f}`);
    await unpack(m, ["bin.tar.gz"]);
    const r = await bexec(m.id, ["bash", `${H}/bin/run-gui.sh`, REPO, ...chosen]);
    await collect(m, "gui", ["summary.txt", ...chosen.map((s) => `${s}.log`)], r, a.out);
    if (!r.sawExit) await markDirty(m);
    console.log(r.out.trim().split("\n").slice(-30).join("\n"));
    console.log(r.code === 0 ? "GUI SUITES PASSED" : `GUI SUITES FAILED (exit ${r.code})`);
  } finally {
    unlock(m);
    await $`rm -rf ${pack}`;
  }
}

// --- pool management

async function status() {
  await prune();
  for (const m of readPool()) {
    const i = await info(m.id);
    let locked = "";
    try {
      locked = Deno.readTextFileSync(lockPath(m)).trim();
    } catch { /* free */ }
    console.log(
      `${m.name} ${m.id} ${m.type} ${i?.state} until ${i?.archiveAfter}${locked ? ` (in use by ${locked})` : ""}`,
    );
  }
}

async function down() {
  const pool = readPool();
  await Promise.all(pool.map((m) => $`boat delete ${m.id} --yes`.noThrow().quiet()));
  writePool([]);
  Deno.writeTextFileSync(RUST_STAGE(), "[]");
  const listing = JSON.parse(await $`boat list --all --json`.text());
  const ids = new Set(pool.map((m) => m.id));
  const left = listing.sandboxes.filter((s: { id: string; state: string }) => ids.has(s.id) && s.state !== "deleted");
  console.log(
    left.length
      ? `STILL PRESENT: ${JSON.stringify(left.map((s: { name: string; id: string }) => [s.name, s.id]))}`
      : `deleted ${pool.length}`,
  );
}

if (import.meta.main) {
  const args = parseArgs(Deno.args, {
    string: ["scratch", "count", "type", "shards", "out", "archive", "ttl", "repo", "prefix", "budget", "members"],
    collect: ["only"],
    boolean: ["reuse-stage", "no-build", "build-there"],
    "--": true,
  });
  const [cmd, ...rest] = args._.map(String);
  const a = args as unknown as Record<string, string>;
  if (!a.scratch) throw new Error("--scratch <dir> is required");
  SCRATCH = a.scratch;
  if (a.repo) REPO = Deno.realPathSync(a.repo);
  PREFIX = a.prefix ?? defaultPrefix(REPO);
  if (a.budget) BUDGET = Number(a.budget);
  await $`mkdir -p ${SCRATCH}`;
  if ((cmd === "rust" || cmd === "gui" || cmd === "docs") && !a.out) throw new Error("--out <dir> is required");
  if (cmd === "up") {
    const ms = await claim(Number(a.count ?? "1"), a.type ?? "large");
    ms.forEach(unlock);
    await status();
  } else if (cmd === "rust") await runRust(a, (args["--"] ?? []).map(String));
  else if (cmd === "gui") await runGui(a, rest);
  else if (cmd === "docs") await runDocs(a, (args.only ?? []).map(String));
  else if (cmd === "status") await status();
  else if (cmd === "extend") {
    for (const m of readPool()) await $`boat extend ${m.id} --ttl ${a.ttl ?? String(MIN_TTL_S)}`.quiet();
    await status();
  } else if (cmd === "down") await down();
  else throw new Error("usage: boat-tests.ts up|rust|gui|docs|status|extend|down --scratch <dir> ... (see the header)");
}

/** Shards consume staged payloads through nextest's scoped setup environment and cannot run Cargo. Exercise the shard
 * runner's actual config rewrite against the repository's configuration: it must drop the fixture builder and keep
 * Podman's setup, the setup-scripts opt-in and the long test's budget. Operates on local temporary files only. */
Deno.test("the shard config removes only the local fixture builder and keeps the scoped Podman setup", async () => {
  const worker = await Deno.readTextFile(`${HERE}/sandbox/run-rust-shard.sh`);
  const rewrite = worker.match(/<<'PY'[^\n]*\n([\s\S]*?)\nPY\n/)?.[1];
  if (!rewrite) throw new Error("the shard runner's config rewrite was not found");
  const scratch = await Deno.makeTempDir({ prefix: "farhelm-boat-config-" });
  try {
    const input = `${scratch}/nextest.toml`;
    const output = `${scratch}/archive.toml`;
    await Deno.writeTextFile(input, await Deno.readTextFile(`${REPO}/.config/nextest.toml`));
    await $`python3 -c ${rewrite} ${input} ${output}`;
    const retained = await Deno.readTextFile(output);
    for (
      const required of [
        "[scripts.setup.podman-payloads]",
        'setup = "podman-payloads"',
        'experimental = ["setup-scripts"]',
        "test(/^podman_environments::/)",
        "terminate-after = 40",
        // The shard's only way to see a passing test's runtime SKIPPED line.
        "store-success-output = true",
      ]
    ) {
      if (!retained.includes(required)) throw new Error(`shard configuration lost ${required}`);
    }
    if (retained.includes("[scripts.setup.build-fixtures]") || retained.includes('setup = "build-fixtures"')) {
      throw new Error("the shard would still try to build fixtures with Cargo");
    }
    await $`python3 -c 'import sys,tomllib; tomllib.load(open(sys.argv[1], "rb"))' ${output}`;
  } finally {
    await Deno.remove(scratch, { recursive: true });
  }
});

Deno.test("a docs run's pages spread round-robin over the members it got, in page order within each", () => {
  assertEquals(spread(["a", "b", "c", "d", "e"], 3), [["a", "d"], ["b", "e"], ["c"]]);
  assertEquals(spread(["a", "b"], 3), [["a"], ["b"]]);
  assertEquals(spread(["a"], 0), [["a"]]);
  assertEquals(spread(["a/one", "b", "a/two", "c"], 2), [["a/one", "a/two", "c"], ["b"]]);
  assertEquals(spread(["a/one", "a/two"], 2), [["a/one", "a/two"]]);
});

Deno.test("a selected docs run clears only its outputs and invalidates a prior full capture", async () => {
  const root = await Deno.makeTempDir({ prefix: "farhelm-docs-output-" });
  try {
    await Deno.mkdir(`${root}/a`);
    await Deno.mkdir(`${root}/b`);
    const previous = ["capture.json", "a/one.png", "a/keep.png", "b/old.png", "b/notes.txt"];
    for (const path of previous) await Deno.writeTextFile(`${root}/${path}`, "earlier capture");
    await clearDocsOutputs(root, ["a/one", "b"]);
    assertEquals([...Deno.readDirSync(root)].map((entry) => entry.name).sort(), ["a", "b"]);
    assertEquals([...Deno.readDirSync(`${root}/a`)].map((entry) => entry.name), ["keep.png"]);
    assertEquals([...Deno.readDirSync(`${root}/b`)].map((entry) => entry.name), ["notes.txt"]);
  } finally {
    await Deno.remove(root, { recursive: true });
  }
});

Deno.test("the default prefix is derived from the checkout's directory name", () => {
  assertEquals(defaultPrefix("/home/u/git/Farhelm_Main.2"), "farhelm-boat-farhelm-main-2-");
});

Deno.test("an exec's frames give its output and exit code, and say when the exit frame never came", () => {
  const frames = [
    '{"type":"attached"}',
    '{"type":"stdout","data":"a\\n"}',
    '{"type":"stderr","data":"b\\n"}',
    '{"type":"exit","exitCode":3}',
  ].join("\n");
  assertEquals(readFrames(frames, 3), { code: 3, out: "a\nb\n", sawExit: true });
  assertEquals(readFrames('{"type":"stdout","data":"x"}', 143), { code: 143, out: "x", sawExit: false });
});
