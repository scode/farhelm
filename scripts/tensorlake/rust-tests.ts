#!/usr/bin/env -S deno run -A
/**
 * Runs a locally built nextest archive of Farhelm's Rust tests across Tensorlake sandboxes, one hash partition per
 * sandbox. Not the default way to test: use it only when the maintainer asks for Tensorlake (`.agents/tensorlake.md`).
 * The template it clones is built by `sandbox-tests.ts make-template` beside this file, whose ledger and sweep this one
 * shares.
 *
 * The sandboxes have no Rust toolchain, so the tests are built here (`cargo nextest archive`) and only run there. That
 * shapes the rest:
 * - The archive holds every test binary, about a gigabyte compressed, so it is uploaded once, to a staging sandbox
 *   that is then snapshotted; each shard clones that snapshot in seconds rather than receiving its own copy. The
 *   staging snapshot is kept after the run, so a rerun of the same archive passes `--staged <snapshot>` and uploads
 *   nothing; `sandbox-tests.ts sweep` removes it with the rest.
 * - Test binaries have absolute paths compiled in (fixture directories through CARGO_MANIFEST_DIR, CARGO_BIN_EXE_*,
 *   the target directory), so the checkout is unpacked on the sandbox at this checkout's own absolute path and the
 *   archive is extracted over it. Nothing needs remapping.
 * - Nextest's setup script for the e2e tests runs `cargo build -p farhelm-fixtures`, which cannot run there, so the
 *   shard removes only that setup hook. Matching static Podman provisioning payloads are prepared locally and staged
 *   with the tools. The retained Podman setup exposes those files to its matching test on the clone.
 *
 * Usage:
 *   rust-tests.ts run --scratch <dir> --template <snapshot> --prefix <name-prefix> --shards <n>
 *       --archive <file> --out <dir> [--staged <snapshot>] [--repo <checkout>] [-- <nextest filter arguments>]
 * Build the archive first, from an empty working copy of the revision under test, with the pinned nextest on PATH:
 *   cargo nextest archive --locked [-p <package> ...] --archive-file <file>
 * and `cargo build` beforehand, so target/debug/farhelm-fixtures matches. Use a `--scratch` of its own when another
 * run is going at the same time: each run's sweep terminates every live sandbox in its scratch's ledger.
 */
import $ from "jsr:@david/dax@0.50.1";
import { parseArgs } from "jsr:@std/cli@1.0.32/parse-args";
import { create, freeNames, HOME, retainSandbox, snapshotRows, sweep, uploadChunked } from "./sandbox-tests.ts";

const HERE = import.meta.dirname!;
// The checkout whose build is tested; `--repo` overrides it for a copy of this script run from elsewhere.
let REPO = Deno.realPathSync(`${HERE}/../..`);
// The staged checkout plus the extracted archive take about 25 GiB; a clone cannot be smaller than its snapshot.
const DISK_MB = "61440";

/** The pinned nextest's install directory, relative to the checkout, as the installer reports it. */
async function nextestDir() {
  const dir = (await $`python3 scripts/install-pinned-nextest.py`.cwd(REPO).text()).trim();
  if (!dir.startsWith(`${REPO}/`)) throw new Error(`nextest installed outside the checkout: ${dir}`);
  return dir.slice(REPO.length + 1);
}

/** Stage this source's ordinary static installer inputs beside the archive tools. Cargo is unavailable on clones.
 * Published releases can carry an older protocol, so matching payloads are prepared locally. An explicit Podman opt-out
 * skips this build and is forwarded to every shard. The payload producer also accepts an operator-supplied matching
 * directory. Payloads travel with the staging snapshot for reuse by later filters of the same archive. */
async function pack(scratch: string, nextest: string, skipPodman: boolean) {
  const { target_directory } = await $`cargo metadata --locked --no-deps --format-version 1`.cwd(REPO).json();
  if (target_directory !== `${REPO}/target`) {
    throw new Error(
      "Tensorlake archive staging needs this checkout's target/ directory. " +
        "Build the fixtures and archive with Cargo's target directory set to this checkout's target/, then stage again.",
    );
  }
  const out = `${scratch}/rust-pack`;
  await $`rm -rf ${out}`;
  await $`mkdir -p ${out}`;
  await $`git ls-files -z --cached | tar --null -czf ${out}/src.tar.gz -T -`.cwd(REPO).pipefail();
  await $`git clone -q --no-local --depth 1 ${REPO} ${out}/shallow`;
  await $`tar -czf ${out}/git.tar.gz -C ${out}/shallow .git`;
  await $`rm -rf ${out}/shallow`;
  await $`tar -czf ${out}/tools.tar.gz ${nextest} target/debug/farhelm-fixtures`.cwd(REPO);
  if (skipPodman) {
    await $`tar -czf ${out}/payloads.tar.gz --files-from /dev/null`;
  } else {
    // The producer prints the exact directory and one newline. dax text() removes only the line ending.
    const payloads = await $`bash scripts/build-podman-test-payloads.sh`.cwd(REPO).text();
    await $`tar -czf ${out}/payloads.tar.gz -C ${payloads} .`;
  }
  return out;
}

/** Uploads everything once to a staging sandbox, unpacks it at this checkout's path, and snapshots it. */
async function stage(
  scratch: string,
  template: string,
  name: string,
  archive: string,
  nextest: string,
  skipPodman: boolean,
) {
  const p = await pack(scratch, nextest, skipPodman);
  await create(scratch, name, DISK_MB, template);
  try {
    const makeDirs = 'mkdir -p "$1/stage" && sudo -n install -d -o tl-user -g tl-user "$2"';
    await $`tl sbx exec ${name} sh -c ${makeDirs} sh ${HOME} ${REPO}`;
    for (const f of ["src.tar.gz", "git.tar.gz", "tools.tar.gz", "payloads.tar.gz"]) {
      await uploadChunked(name, `${p}/${f}`, `${HOME}/stage/${f}`);
    }
    const t0 = Date.now();
    await uploadChunked(name, archive, `${HOME}/stage/archive.tar.zst`);
    console.log(`archive uploaded in ${Math.round((Date.now() - t0) / 1000)} s`);
    await $`tl sbx cp ${HERE}/run-rust-shard.sh ${name}:${HOME}/stage/run-rust-shard.sh`.quiet();
    // The checkout is an argument to the remote shell, so spaces in its name survive both parsers.
    const unpack = `cd "$1" && tar -xzf "$2/src.tar.gz" && tar -xzf "$2/git.tar.gz" ` +
      `&& tar -xzf "$2/tools.tar.gz" && mkdir -p target/podman-test-payloads ` +
      `&& tar -xzf "$2/payloads.tar.gz" -C target/podman-test-payloads ` +
      `&& rm "$2/src.tar.gz" "$2/git.tar.gz" "$2/tools.tar.gz" "$2/payloads.tar.gz"`;
    await $`tl sbx exec -t 1800 ${name} sh -c ${unpack} sh ${REPO} ${HOME}/stage`;
    const snap = (await $`tl sbx checkpoint ${name}`.text()).trim().split(/\s+/).pop()!;
    while (!snapshotRows(await $`tl sbx checkpoint ls`.text()).some(([s, , st]) => s === snap && st === "completed")) {
      await $.sleep(3000);
    }
    return snap;
  } finally {
    await $`tl sbx terminate ${name}`.noThrow().quiet();
  }
}

/** Each partition may select zero tests. The existing worker summary includes nextest's final count on both new
 * and reused staging snapshots. A success without a summary cannot establish that tests ran. */
function completedTests(summary: string) {
  const clean = summary.replace(/\x1b\[[0-9;]*m/g, "");
  const count = clean.match(/^\s*Summary[ \t]+\[[^\]\n]+\][ \t]+(\d+) tests? run:/m)?.[1];
  if (count === undefined) throw new Error("shard finished without nextest's completed test count");
  return Number(count);
}

/** One partition on its own clone. Keep a failed shard's run directories with its console and summary. If any
 * required collection fails, the clone still holds the evidence and both cleanup paths must leave it available. */
async function shard(
  scratch: string,
  snap: string,
  name: string,
  k: number,
  n: number,
  nextest: string,
  filter: string[],
  out: string,
  skipPodman: boolean,
  retained: Set<string>,
) {
  const dir = `${out}/shard${k}`;
  await Deno.mkdir(dir);
  const id = await create(scratch, name, DISK_MB, snap);
  const remote = `${HOME}/out/shard${k}`;
  let collected = false;
  try {
    const r = await $`tl sbx exec -t 14400 ${name} env FARHELM_TEST_SKIP_PODMAN=${
      skipPodman ? "1" : "0"
    } bash ${HOME}/stage/run-rust-shard.sh ${REPO} ${HOME}/stage/archive.tar.zst ${nextest}/cargo-nextest ${k} ${n} ${filter}`
      .noThrow().captureCombined();
    await Deno.writeTextFile(`${dir}/exec.log`, r.combined.slice(-20000));
    await $`tl sbx cp ${name}:${remote}/summary.txt ${dir}/summary.txt`.quiet();
    const summary = await Deno.readTextFile(`${dir}/summary.txt`);
    // The worker can refuse an unusable clone before starting the recorder or writing a console log.
    const setupFailed = /^setup failed: /m.test(summary);
    if (!setupFailed) {
      await $`tl sbx cp ${name}:${remote}/console.log ${dir}/console.log`.quiet();
    }
    // The preamble is already on disk while nextest is running. A successful copy after a lost exec
    // says nothing about whether the console and recorder files were finished.
    const workerExit = setupFailed ? "1" : summary.match(/^exit: (\d+)$/m)?.[1];
    if (workerExit === undefined) throw new Error(`${name}: shard summary ended before the worker verdict`);
    const code = r.code || Number(workerExit);
    // The recorder may have written evidence even if its path never reached the summary.
    if (code !== 0 && !setupFailed) {
      const archiveRuns = 'tar -czf "$1" -C /tmp/farhelm-tmp test-runs';
      await $`tl sbx exec ${name} sh -c ${archiveRuns} sh ${remote}/runs.tar.gz`;
      await $`tl sbx cp ${name}:${remote}/runs.tar.gz ${dir}/runs.tar.gz`.quiet();
    }
    const tests = code === 0 ? completedTests(summary) : 0;
    collected = true;
    if (code !== 0) throw new Error(`${name}: shard exited ${code}. Evidence in ${dir}`);
    console.log(`shard${k} (${name}): ${tests} tests run`);
    return tests;
  } finally {
    if (collected) {
      await $`tl sbx terminate ${name}`.noThrow().quiet();
    } else {
      retained.add(id);
      console.error(`Evidence collection incomplete. Kept sandbox ${name} (${id}), evidence at ${remote}.`);
      await retainSandbox(scratch, id).catch((error) => console.error("Could not record the evidence hold:", error));
    }
  }
}

if (import.meta.main) {
  const args = parseArgs(Deno.args, {
    string: ["scratch", "template", "prefix", "shards", "archive", "out", "staged", "repo"],
    "--": true,
  });
  const [cmd] = args._.map(String);
  if (cmd !== "run") throw new Error("usage: rust-tests.ts run --scratch <dir> ... (see the header)");
  if (args.repo) REPO = Deno.realPathSync(args.repo);
  for (const k of ["scratch", "template", "prefix", "shards", "out"] as const) {
    if (!args[k]) throw new Error(`--${k} is required`);
  }
  if (!args.staged && !args.archive) throw new Error("--archive is required unless --staged names a staging snapshot");
  const n = Number(args.shards);
  if (!Number.isSafeInteger(n) || n < 1) throw new Error("--shards must be a positive integer");
  const filter = (args["--"] ?? []).map(String);
  const skipPodman = Deno.env.get("FARHELM_TEST_SKIP_PODMAN") === "1";
  await $`mkdir -p ${args.scratch!}`;
  await Deno.mkdir(args.out!, { recursive: true });
  const nextest = await nextestDir();
  const names = await freeNames(args.prefix!, n + 1);
  let snap = args.staged ?? "";
  const retained = new Set<string>();
  try {
    if (!snap) {
      console.log(`staging on ${names[0]}`);
      snap = await stage(args.scratch!, args.template!, names[0], args.archive!, nextest, skipPodman);
    }
    console.log(`staging snapshot ${snap}; ${n} shards`);
    const results = await Promise.allSettled(
      Array.from(
        { length: n },
        (_, i) => shard(args.scratch!, snap, names[i + 1], i + 1, n, nextest, filter, args.out!, skipPodman, retained),
      ),
    );
    for (const r of results) if (r.status === "rejected") console.error(r.reason);
    if (results.some((r) => r.status === "rejected")) {
      console.error("SOME SHARDS FAILED");
      Deno.exitCode = 1;
    } else if (results.every((r) => r.status === "fulfilled" && r.value === 0)) {
      console.error("NO TESTS RAN across all shards");
      Deno.exitCode = 1;
    } else {
      console.log("ALL SHARDS PASSED");
    }
  } finally {
    // Keep the template and the staging snapshot: a rerun of the same archive reuses the latter (--staged).
    await sweep(args.scratch!, [args.template!, ...(snap ? [snap] : [])], { holds: retained });
  }
}

/** Archive workers consume staged payloads through nextest's scoped setup environment. Exercise the worker's actual
 * rewrite against the repository configuration to verify fixture-builder removal and retention of Podman's setup,
 * experimental support and long-test resource budget. This test operates on local temporary files. */
Deno.test("the archive config removes only the local fixture builder and keeps scoped Podman payload setup", async () => {
  const worker = await Deno.readTextFile(`${HERE}/run-rust-shard.sh`);
  const rewrite = worker.match(/<<'PY'[^\n]*\n([\s\S]*?)\nPY\n/)?.[1];
  if (!rewrite) throw new Error("the worker's config rewrite was not found");
  const scratch = await Deno.makeTempDir({ prefix: "farhelm-rust-config-" });
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
      ]
    ) {
      if (!retained.includes(required)) throw new Error(`archive configuration lost ${required}`);
    }
    if (retained.includes("[scripts.setup.build-fixtures]") || retained.includes('setup = "build-fixtures"')) {
      throw new Error("the archive worker would still try to build fixtures with Cargo");
    }
    // The worker consumes the complete configuration. Parse its syntax as well as checking retained settings.
    await $`python3 -c 'import sys,tomllib; tomllib.load(open(sys.argv[1], "rb"))' ${output}`;
  } finally {
    await Deno.remove(scratch, { recursive: true });
  }
});

/** The dispatcher must preserve the only failed-run bundle even while it lets a sibling complete and sweep safely. */
Deno.test("shard verdicts and evidence survive the automatic sweep", async () => {
  const scratch = await Deno.makeTempDir({ prefix: "farhelm-shard-test-" });
  try {
    await Deno.mkdir(`${scratch}/scripts`);
    await Deno.writeTextFile(
      `${scratch}/scripts/install-pinned-nextest.py`,
      'from pathlib import Path\nprint(Path.cwd() / "nextest")\n',
    );
    // This CLI stand-in records cleanup calls and produces only the worker files the dispatcher takes home.
    const cli = `${scratch}/tl`;
    await Deno.writeTextFile(
      cli,
      `#!/bin/sh
case "$2" in
  ls) cat "$FAKE_ROOT/live" ;;
  create) for name do :; done; printf 'id-%s\\n' "$name" | tee -a "$FAKE_ROOT/live" ;;
  exec)
    case "$*" in
      *run-rust-shard.sh*)
        if [ "$5" = review2 ]; then
          case "$MODE" in failed) exit 100 ;; incomplete) exit 124 ;; esac
        fi ;;
      *runs.tar.gz*) exit 0 ;;
      *) exit 2 ;;
    esac ;;
  cp)
    case "$3" in
      */summary.txt)
        if [ "$MODE" = incomplete ] && [ "\${3%%:*}" = review2 ]; then
          printf 'pid 1: systemd\\n' >"$4"
        elif [ "$MODE" = failed ] && [ "\${3%%:*}" = review2 ]; then
          printf 'exit: 100\\n' >"$4"
        elif [ "$MODE" = empty ] || [ "\${3%%:*}" = review2 ]; then
          printf 'exit: 0\\n    Summary [0.001s] 0 tests run: 0 passed\\n' >"$4"
        else
          printf 'exit: 0\\n    Summary [0.001s] 1 test run: 1 passed\\n' >"$4"
        fi ;;
      */console.log)
        if [ "$MODE" = failed ] && [ "\${3%%:*}" = review3 ]; then
          # Hold the sibling's copy until the other shard recorded its copy failure.
          attempt=0
          until grep -q 'retained_sandbox.*id-review2' "$FAKE_ROOT/failed-state/ledger.jsonl"; do
            attempt=$((attempt + 1)); [ "$attempt" -lt 150 ] || exit 2
            sleep 0.02
          done
          # Leave a window for any premature sweep to terminate this still-copying clone.
          sleep 0.1
          grep -qx 'terminate review3' "$FAKE_ROOT/events" && exit 2
        fi
        printf '%s\\n' "$3" >>"$FAKE_ROOT/events"; touch "$4" ;;
      */runs.tar.gz) exit 1 ;;
      *) exit 2 ;;
    esac ;;
  terminate) printf 'terminate %s\\n' "$3" >>"$FAKE_ROOT/events" ;;
  checkpoint) [ "$3" = ls ] && echo sweep >>"$FAKE_ROOT/events" ;;
  *) exit 2 ;;
esac
`,
      { mode: 0o700 },
    );
    const run = async (mode: string, shards = "2") => {
      const state = `${scratch}/${mode}-state`;
      await Deno.mkdir(state);
      await Deno.writeTextFile(
        `${state}/ledger.jsonl`,
        '{"name":"previous","id":"id-previous","disk_mb":"61440"}\n{"retained_sandbox":"id-previous"}\n',
      );
      await Deno.writeTextFile(`${scratch}/live`, "id-previous\n");
      await Deno.writeTextFile(`${scratch}/events`, "");
      const result = await new Deno.Command(Deno.execPath(), {
        args: [
          "run",
          "-A",
          import.meta.filename!,
          "run",
          "--repo",
          scratch,
          "--scratch",
          state,
          "--out",
          `${scratch}/${mode}-out`,
          "--prefix",
          "review",
          "--template",
          "template",
          "--staged",
          "stage",
          "--shards",
          shards,
        ],
        env: { PATH: `${scratch}:${Deno.env.get("PATH")}`, FAKE_ROOT: scratch, MODE: mode },
      }).output();
      const output = new TextDecoder().decode(result.stdout) + new TextDecoder().decode(result.stderr);
      return { code: result.code, output, events: await Deno.readTextFile(`${scratch}/events`) };
    };
    const mixed = await run("mixed");
    if (mixed.code !== 0 || !mixed.output.includes("ALL SHARDS PASSED") || !mixed.output.includes("0 tests run")) {
      throw new Error(mixed.output);
    }
    const empty = await run("empty");
    if (empty.code === 0 || !empty.output.includes("NO TESTS RAN")) throw new Error(empty.output);
    const invalid = await run("invalid", "0");
    if (invalid.code === 0 || !invalid.output.includes("--shards must be a positive integer") || invalid.events) {
      throw new Error(invalid.output + invalid.events);
    }
    const failed = await run("failed");
    const { code, output, events } = failed;
    if (code === 0 || !output.includes("SOME SHARDS FAILED") || !output.includes("Kept sandbox review2 (id-review2)")) {
      throw new Error(output);
    }
    if (
      !events.includes("terminate review3\n") || /terminate (review2|previous)\n/.test(events) ||
      events.indexOf("review3:/home/tl-user/out/shard2/console.log") === -1 ||
      !events.endsWith("sweep\n")
    ) {
      throw new Error(`cleanup did not follow sibling collection or discarded the failed clone:\n${events}`);
    }
    const ledger = await Deno.readTextFile(`${scratch}/failed-state/ledger.jsonl`);
    if (!ledger.includes('{"retained_sandbox":"id-review2"}')) throw new Error("the evidence hold was not recorded");
    const incomplete = await run("incomplete");
    if (
      incomplete.code === 0 || !incomplete.output.includes("before the worker verdict") ||
      incomplete.events.includes("terminate review2\n")
    ) {
      throw new Error(incomplete.output + incomplete.events);
    }
  } finally {
    await Deno.remove(scratch, { recursive: true });
  }
});
