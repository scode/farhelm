import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

/** Controlled extension harness for the OMP conversation reporter asset.
 *
 * Each test spawns `scenarios.mjs` as a CHILD PROCESS with the reporter
 * executable supplied through the child's environment — the test process's
 * own environment is never mutated. The scenario stands in a scripted
 * session manager and either a payload-appending reporter shell script or a
 * controlled `execFile` mock for OMP and the supervisor, so the promise-chain
 * ordering, stale-id cancellation, file-existence recheck timing, subscribed
 * transition events, and the silent-failure boundary are all exercised
 * against the REAL asset module.
 *
 * Two kinds of waiting appear here, and they are different on purpose:
 *
 * - The shell reporter's `sleep 0.2` is a serialization stimulus: it holds
 *   a report long enough for a later event to be queued behind it. The
 *   scenario's start marker is a readiness oracle polled by a bounded
 *   helper (5 s bound, 5 ms steps) — it waits for a CONDITION, never for a
 *   fixed duration, and timing out is a recorded failure.
 * - The dispatch-order scenario (`delayed-old-serialization`) uses none of
 *   that: it substitutes a controlled `execFile` mock whose completion
 *   callbacks the scenario holds and releases, so the assertion is
 *   structural — B must not dispatch while A's completion is pending — and
 *   it is the one the suite proves against a deliberately nonserialized
 *   asset variant.
 */

const scenarioRunner = fileURLToPath(new URL("./scenarios.mjs", import.meta.url));

function runScenario(name, variant) {
    const workspace = mkdtempSync(join(tmpdir(), "omp-asset-"));
    const reporter = join(workspace, "reporter.sh");
    writeFileSync(
        reporter,
        [
            "#!/bin/sh",
            // Mark the start so a scenario can act INSIDE this report's
            // execution window (while its child holds the queue); the sleep
            // itself is the serialization stimulus that makes a later event
            // land in the queue behind this one.
            `printf started > '${join(workspace, "started.marker")}'`,
            "sleep 0.2",
            `cat >> '${join(workspace, "reports.log")}'`,
            `printf '\\n' >> '${join(workspace, "reports.log")}'`,
            "",
        ].join("\n"),
        { mode: 0o700 },
    );
    const args = variant ? [scenarioRunner, name, workspace, variant] : [scenarioRunner, name, workspace];
    const result = spawnSync(
        process.execPath,
        args,
        {
            encoding: "utf8",
            timeout: 30000,
            // The child's environment is built HERE, explicitly; the parent's
            // is read but never written.
            env: {
                PATH: process.env.PATH ?? "/usr/bin:/bin",
                HOME: process.env.HOME ?? workspace,
                FARHELM_OMP_REPORTER_EXE: reporter,
            },
        },
    );
    // A scenario's failures travel on STDOUT as a JSON problem list; stderr
    // carries crashes. Both belong in the diagnostic.
    const diagnostics = `stdout: ${result.stdout}\nstderr: ${result.stderr}`;
    return { result, diagnostics };
}

function expectScenarioPass(name, variant) {
    const { result, diagnostics } = runScenario(name, variant);
    assert.equal(
        result.status,
        0,
        `scenario ${name} failed:\n${diagnostics}`,
    );
    assert.equal(
        JSON.parse(result.stdout).ok,
        true,
        `scenario ${name} reported problems:\n${diagnostics}`,
    );
}

test("the extension reports every subscribed conversation transition", () => {
    expectScenarioPass("transition-events");
});

test("the session file's existence is re-read when the queued report executes", () => {
    expectScenarioPass("file-recheck-at-execution");
});

test("a queued report cannot dispatch while an earlier completion is pending", () => {
    expectScenarioPass("delayed-old-serialization");
});

test("the ordering proof rejects a deliberately nonserialized asset variant", () => {
    // Fail-on-broken: the same proof, run against a copy of the asset whose
    // serialization chain has been rewritten into per-report parallel
    // dispatch, must FAIL — otherwise the proof above would pass on a
    // broken implementation too.
    const { result, diagnostics } = runScenario("delayed-old-serialization", "nonserialized");
    assert.notEqual(
        result.status,
        0,
        `the nonserialized variant must fail the ordering proof:\n${diagnostics}`,
    );
    const verdict = JSON.parse(result.stdout);
    assert.equal(verdict.ok, false, `verdict: ${JSON.stringify(verdict)}`);
    assert.ok(
        verdict.problems.some((problem) => problem.includes("must NOT dispatch")),
        `the parallel dispatch is what the proof names:\n${JSON.stringify(verdict.problems)}`,
    );
});

test("a stale queued report is cancelled and leaves the current identity alone", () => {
    expectScenarioPass("stale-old-report-cancelled");
});

test("a throwing sessionManager getter is absorbed silently and reporting recovers", () => {
    expectScenarioPass("throwing-getter");
});
