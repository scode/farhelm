#!/usr/bin/env -S deno run -A
/**
 * Runs a locally built nextest archive of Farhelm's Rust tests across Tensorlake sandboxes, one hash partition per
 * sandbox. Not the default way to test: use it only when the maintainer asks for Tensorlake (`.agents/tensorlake.md`).
 * The browser suite has its own runner beside this file, `sandbox-tests.ts`, whose template, ledger and sweep this one
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
 * and `cargo build` beforehand, so target/debug/farhelm-fixtures matches. Use a `--scratch` of its own when a browser
 * run is going at the same time: each run's sweep terminates every live sandbox in its scratch's ledger.
 */
import $ from "jsr:@david/dax@0.50.1";
import { parseArgs } from "jsr:@std/cli@1.0.32/parse-args";
import { create, freeNames, HOME, snapshotRows, sweep, uploadChunked } from "./sandbox-tests.ts";

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
  const out = `${scratch}/rust-pack`;
  await $`rm -rf ${out}`;
  await $`mkdir -p ${out}`;
  await $`git ls-files -z --cached | tar --null -czf ${out}/src.tar.gz -T -`.cwd(REPO).pipefail();
  await $`git clone -q --depth 1 ${"file://" + REPO} ${out}/shallow`;
  await $`tar -czf ${out}/git.tar.gz -C ${out}/shallow .git`;
  await $`rm -rf ${out}/shallow`;
  await $`tar -czf ${out}/tools.tar.gz .ci-tmux ${nextest} target/debug/farhelm-fixtures`.cwd(REPO);
  if (skipPodman) {
    await $`tar -czf ${out}/payloads.tar.gz --files-from /dev/null`;
  } else {
    const payloads = (await $`bash scripts/build-podman-test-payloads.sh`.cwd(REPO).text()).trim();
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
    await $`tl sbx exec ${name} sh -c ${`mkdir -p ${HOME}/stage && sudo -n install -d -o tl-user -g tl-user ${REPO}`}`;
    for (const f of ["src.tar.gz", "git.tar.gz", "tools.tar.gz", "payloads.tar.gz"]) {
      await uploadChunked(name, `${p}/${f}`, `${HOME}/stage/${f}`);
    }
    const t0 = Date.now();
    await uploadChunked(name, archive, `${HOME}/stage/archive.tar.zst`);
    console.log(`archive uploaded in ${Math.round((Date.now() - t0) / 1000)} s`);
    await $`tl sbx cp ${HERE}/run-rust-shard.sh ${name}:${HOME}/stage/run-rust-shard.sh`.quiet();
    const unpack = `cd ${REPO} && tar -xzf ${HOME}/stage/src.tar.gz && tar -xzf ${HOME}/stage/git.tar.gz ` +
      `&& tar -xzf ${HOME}/stage/tools.tar.gz && mkdir -p target/podman-test-payloads ` +
      `&& tar -xzf ${HOME}/stage/payloads.tar.gz -C target/podman-test-payloads ` +
      `&& rm ${HOME}/stage/src.tar.gz ${HOME}/stage/git.tar.gz ${HOME}/stage/tools.tar.gz ${HOME}/stage/payloads.tar.gz`;
    await $`tl sbx exec -t 1800 ${name} sh -c ${unpack}`;
    const snap = (await $`tl sbx checkpoint ${name}`.text()).trim().split(/\s+/).pop()!;
    while (!snapshotRows(await $`tl sbx checkpoint ls`.text()).some(([s, , st]) => s === snap && st === "completed")) {
      await $.sleep(3000);
    }
    return snap;
  } finally {
    await $`tl sbx terminate ${name}`.noThrow().quiet();
  }
}

/** One partition on its own clone, in one attached exec; brings back its summary and console output, and for a failed
 * shard its retained run directories, then terminates the clone. */
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
) {
  await create(scratch, name, DISK_MB, snap);
  try {
    const r = await $`tl sbx exec -t 14400 ${name} env FARHELM_TEST_SKIP_PODMAN=${
      skipPodman ? "1" : "0"
    } bash ${HOME}/stage/run-rust-shard.sh ${REPO} ${HOME}/stage/archive.tar.zst ${nextest}/cargo-nextest ${k} ${n} ${filter}`
      .noThrow().captureCombined();
    const dir = `${out}/shard${k}`;
    await $`mkdir -p ${dir}`;
    await Deno.writeTextFile(`${dir}/exec.log`, r.combined.slice(-20000));
    for (const f of ["summary.txt", "console.log"]) {
      await $`tl sbx cp ${name}:${HOME}/out/shard${k}/${f} ${dir}/${f}`.noThrow().quiet();
    }
    if (r.code !== 0) {
      await $`tl sbx exec ${name} tar -czf ${HOME}/out/shard${k}/runs.tar.gz -C /tmp/farhelm-tmp test-runs`.noThrow();
      await $`tl sbx cp ${name}:${HOME}/out/shard${k}/runs.tar.gz ${dir}/runs.tar.gz`.noThrow().quiet();
    }
    console.log(`shard${k} (${name}): exit ${r.code}`);
    return r.code;
  } finally {
    await $`tl sbx terminate ${name}`.noThrow().quiet();
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
  const filter = (args["--"] ?? []).map(String);
  const skipPodman = Deno.env.get("FARHELM_TEST_SKIP_PODMAN") === "1";
  await $`mkdir -p ${args.scratch!}`;
  const nextest = await nextestDir();
  const names = await freeNames(args.prefix!, n + 1);
  let snap = args.staged ?? "";
  try {
    if (!snap) {
      console.log(`staging on ${names[0]}`);
      snap = await stage(args.scratch!, args.template!, names[0], args.archive!, nextest, skipPodman);
    }
    console.log(`staging snapshot ${snap}; ${n} shards`);
    const codes = await Promise.all(
      Array.from(
        { length: n },
        (_, i) => shard(args.scratch!, snap, names[i + 1], i + 1, n, nextest, filter, args.out!, skipPodman),
      ),
    );
    console.log(codes.every((c) => c === 0) ? "ALL SHARDS PASSED" : "SOME SHARDS FAILED");
  } finally {
    // Keep the template and the staging snapshot: a rerun of the same archive reuses the latter (--staged).
    await sweep(args.scratch!, [args.template!, ...(snap ? [snap] : [])]);
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
