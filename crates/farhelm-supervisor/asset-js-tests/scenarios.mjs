import { copyFileSync, mkdtempSync, writeFileSync, readFileSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

/** One controlled scenario against the OMP conversation reporter asset.
 *
 * Run as a CHILD PROCESS (`node scenarios.mjs <case> <workspace> [variant]`)
 * with FARHELM_OMP_REPORTER_EXE provided in the child's environment by the
 * spawner — the scenario never mutates its own environment, and the spawner
 * never mutates the test process's. Prints a JSON verdict on stdout and
 * exits nonzero on any violated expectation.
 *
 * The asset is copied to a `.mjs` sibling of a temp workspace because its
 * content is plain ESM that Node can import, while its shipped name carries
 * the `.ts` extension OMP's loader is happy with. Two dispatch substrates
 * are available:
 *
 * - the shell reporter (default): a script that marks its start — the
 *   readiness oracle the scenarios wait on, bounded — and then appends each
 *   received payload, one JSON line, to a log: the exact shape of the real
 *   `farhelm internal hook` input, minus the supervisor;
 * - the deterministic mock (`variant` = "mock-child-process"): the copied
 *   asset's `node:child_process` import is rewritten to the controlled
 *   stand-in in `mock-child-process.mjs`, whose completion callbacks the
 *   scenario holds and releases, making dispatch order directly observable.
 *
 * `variant` = "nonserialized" additionally rewrites the copied asset's
 * serialization chain into per-report independent dispatch — the deliberately
 * broken implementation the ordering proof must reject.
 */

const scenario = process.argv[2];
const workspace = process.argv[3];
const variant = process.argv[4] ?? "";
if (!scenario || !workspace) {
    console.error("usage: node scenarios.mjs <case> <workspace> [variant]");
    process.exit(2);
}

const problems = [];
function check(condition, message) {
    if (!condition) problems.push(message);
}

// Import the asset through a temp .mjs copy, transformed per the variant.
const modulePath = join(workspace, "farhelm-conversation-v1.mjs");
copyFileSync(
    new URL("../assets/omp-conversation-v1.ts", import.meta.url),
    modulePath,
);
if (variant === "mock-child-process" ||
    variant === "nonserialized" ||
    scenario === "delayed-old-serialization") {
    const mockUrl = new URL("./mock-child-process.mjs", import.meta.url).href;
    const text = readFileSync(modulePath, "utf8");
    assert_in_text(text, 'import { execFile } from "node:child_process";');
    writeFileSync(
        modulePath,
        text.replace(
            'import { execFile } from "node:child_process";',
            `import { execFile } from "${mockUrl}";`,
        ),
    );
}
if (variant === "nonserialized") {
    // The deliberately broken implementation the ordering proof must
    // reject: each report dispatches off a FRESH resolved promise instead
    // of chaining behind the queue, so a report queued while another is
    // pending dispatches immediately, in parallel.
    const text = readFileSync(modulePath, "utf8");
    const chain = "pending = pending.then(() => new Promise((resolve) => {";
    const parallel = "pending = Promise.resolve().then(() => new Promise((resolve) => {";
    assert_in_text(text, chain);
    writeFileSync(modulePath, text.replace(chain, parallel));
}
const { default: factory } = await import(pathToFileURL(modulePath));

function assert_in_text(text, needle) {
    if (!text.includes(needle)) {
        console.error(`scenario transform failed: ${needle.slice(0, 60)} not found`);
        process.exit(2);
    }
}

/** A scripted session manager: the id and file the events observe, plus an
 * optional one-shot throwing getter used to pin the failure contract.
 *
 * The default ctx is the ROOT TUI shape (`hasUI: true, mode: "tui"`) the
 * gated asset accepts — the harness simulates the foreground parent unless
 * a scenario builds a child-shaped ctx instead. `makeChildCtx` builds an
 * ineligible ctx with its OWN session manager, mirroring upstream's
 * per-run child manager (`sessionManagerForRun`): a child's stale-id check
 * is self-consistent, so only the context gate can refuse it. */
function makeHarness() {
    const handlers = {};
    const omp = {
        on(name, handler) {
            handlers[name] = handler;
        },
    };
    const state = {
        id: "unset",
        file: undefined,
        throwOnGetSessionId: 0,
    };
    const manager = {
        getSessionId() {
            if (state.throwOnGetSessionId > 0) {
                state.throwOnGetSessionId -= 1;
                throw new Error("sessionManager contract violation");
            }
            return state.id;
        },
        getSessionFile() {
            return state.file;
        },
    };
    const ctx = { hasUI: true, mode: "tui", sessionManager: manager };
    return { omp, handlers, state, ctx };
}

/** An ineligible child-shaped ctx with an independent session manager.
 * `mode` varies per scenario: upstream pins `hasUI: false` for built-in
 * task/workpool/revival children while the exact child mode value is not
 * established, so the gate must refuse on the hasUI leg for every mode. */
function makeChildCtx(mode) {
    const state = { id: "child-9", file: undefined };
    const manager = {
        getSessionId() {
            return state.id;
        },
        getSessionFile() {
            return state.file;
        },
    };
    const ctx = { sessionManager: manager };
    if (mode !== undefined) {
        ctx.hasUI = false;
        ctx.mode = mode;
    }
    return { ctx, state };
}

function readLog(path) {
    try {
        return readFileSync(path, "utf8")
            .split("\n")
            .filter((line) => line.length > 0)
            .map((line) => JSON.parse(line));
    } catch {
        return [];
    }
}

const logPath = join(workspace, "reports.log");

/** One report per event; every handler's returned promise is awaited so the
 * scenario is deterministic about which queued reports have run. An explicit
 * ctx fires the event as another context (a child shape) sharing the same
 * factory import — the same-process delegation the gate must refuse. */
async function fire(harness, name, event, ctx) {
    const returned = harness.handlers[name](event ?? {}, ctx ?? harness.ctx);
    if (returned && typeof returned.then === "function") {
        await returned;
    }
}

/** Wait, bounded, until the reporter named by `marker` has STARTED — the
 * readiness oracle that lets a scenario act inside a report's execution
 * window (while its child holds the queue) instead of racing the microtask
 * that would run the queued work. Never polls past the bound: a timeout is
 * a recorded failure, not a silent pass. */
async function waitUntilStarted(marker) {
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
        if (readFileSyncSafe(marker)) return true;
        await new Promise((resolve) => setTimeout(resolve, 5));
    }
    return false;
}

function readFileSyncSafe(path) {
    try {
        return readFileSync(path, "utf8").length > 0;
    } catch {
        return false;
    }
}

const results = {};

if (scenario === "transition-events") {
    const dir = join(workspace, "files");
    mkdirSync(dir);
    const file = join(dir, "conv-1.jsonl");
    writeFileSync(file, "{\"type\":\"session\",\"version\":3,\"id\":\"conv-1\"}\n");
    const harness = makeHarness();
    factory(harness.omp);
    harness.state.id = "conv-1";
    harness.state.file = file;
    await fire(harness, "session_start");
    await fire(harness, "session_switch", { reason: "fork" });
    await fire(harness, "session_branch");
    await fire(harness, "agent_end");
    const reports = readLog(logPath);
    check(
        reports.map((r) => r.source).join(",") ===
            "session_start,session_switch:fork,session_branch,agent_end",
        `subscribed transitions must each report in order: ${reports.map((r) => r.source)}`,
    );
    check(
        reports.every((r) => r.vendor === "omp" && r.session_id === "conv-1"),
        `every report carries vendor omp and the current id: ${JSON.stringify(reports)}`,
    );
    check(
        reports.every((r) => r.session_file === file),
        "an existing session file is reported as the path",
    );
    results.reports = reports.length;
} else if (scenario === "file-recheck-at-execution") {
    const dir = join(workspace, "files");
    mkdirSync(dir);
    const fileB = join(dir, "conv-b.jsonl");
    const marker = join(workspace, "started.marker");
    const harness = makeHarness();
    factory(harness.omp);
    // A slow first report holds the queue; the marker says its child has
    // STARTED, so the scenario acts inside the execution window rather than
    // racing the queue.
    harness.state.id = "conv-a";
    harness.state.file = undefined;
    const slow = fire(harness, "session_start");
    check(
        await waitUntilStarted(marker),
        "the first report's child must start for the window to exist",
    );
    // While that window holds: switch to conv-b, queue its report — the
    // manager has ALLOCATED its path (the shape OMP's getSessionFile has
    // before persistence) but the file does not exist yet — and only THEN
    // create the file. The existence re-read happens when the queued report
    // executes; it must observe the file that did not exist when the event
    // fired.
    harness.state.id = "conv-b";
    harness.state.file = fileB;
    const queued = fire(harness, "session_switch", { reason: "new" });
    writeFileSync(fileB, "{\"type\":\"session\",\"version\":3,\"id\":\"conv-b\"}\n");
    await slow;
    await queued;
    let reports = readLog(logPath);
    check(
        reports.length === 2 &&
            reports[1].session_id === "conv-b" &&
            reports[1].session_file === fileB,
        `the file created between event and execution is reported: ${JSON.stringify(reports)}`,
    );
    // Withdrawal: the manager still NAMES the file, but it is gone — the
    // next report must carry null, not a stale path.
    rmSync(fileB);
    harness.state.file = fileB;
    await fire(harness, "agent_end");
    reports = readLog(logPath);
    check(
        reports[reports.length - 1].session_id === "conv-b" &&
            reports[reports.length - 1].session_file === null,
        "a missing file reports null, withdrawing the old target",
    );
} else if (scenario === "delayed-old-serialization") {
    // The DISPATCH-ORDER proof, deterministic end to end: the reporter
    // subprocess is the controlled mock, whose completion callbacks the
    // scenario holds and releases. The premise a parallel implementation
    // cannot satisfy: while A's report is pending, B's report must NOT be
    // dispatched at all — B dispatches only once A's completion has been
    // observed by the queue.
    const { dispatches, setCompletionController } = await import(
        new URL("./mock-child-process.mjs", import.meta.url)
    );
    // One macrotask turn: every queued MICROTASK — the report executor
    // chained by an event — has run by the time it resolves, while nothing
    // else can sneak in between.
    const macrotask = () => new Promise((resolve) => setImmediate(resolve));
    let held = null;
    setCompletionController((record, callback) => {
        if (dispatches[0] === record) {
            held = callback;
            return;
        }
        callback();
    });
    const harness = makeHarness();
    factory(harness.omp);
    harness.state.id = "conv-a";
    harness.state.file = undefined;
    const queuedBehindA = fire(harness, "session_start");
    await macrotask();
    check(
        dispatches.length === 1,
        `exactly one report dispatched for the first event: ${dispatches.length}`,
    );
    check(
        typeof held === "function",
        "the first report's completion must be held by the scenario for the window to exist",
    );
    // Fire B's event while A's completion is still held, then let the queue
    // advance as far as it can.
    harness.state.id = "conv-b";
    const queuedBehindB = fire(harness, "session_switch", { reason: "resume" });
    await macrotask();
    check(
        dispatches.length === 1,
        `B's report must NOT dispatch while A's completion is pending — a parallel \
         implementation would have dispatched it: ${JSON.stringify(dispatches.map((d) => d.payload))}`,
    );
    // Release A; observe B's dispatch and completion.
    if (typeof held === "function") {
        held();
    }
    await queuedBehindA;
    await queuedBehindB;
    check(
        dispatches.length === 2,
        `B dispatches exactly once, after A's completion: ${dispatches.length}`,
    );
    check(
        dispatches[0]?.payload && JSON.parse(dispatches[0].payload).session_id === "conv-a",
        `A's payload dispatched first: ${dispatches[0]?.payload}`,
    );
    check(
        dispatches[1]?.payload && JSON.parse(dispatches[1].payload).session_id === "conv-b",
        `B's payload dispatched second: ${dispatches[1]?.payload}`,
    );
} else if (scenario === "stale-old-report-cancelled") {
    const marker = join(workspace, "started.marker");
    const harness = makeHarness();
    factory(harness.omp);
    // The queue is busy (conv-a0's child running) when conv-a's event fires;
    // before conv-a's queued report executes, the current id is conv-b. The
    // stale-id check must CANCEL it — no report for conv-a at all — and the
    // current identity then reports normally.
    harness.state.id = "conv-a0";
    harness.state.file = undefined;
    const slow = fire(harness, "session_start");
    check(
        await waitUntilStarted(marker),
        "the first report's child must start for the window to exist",
    );
    harness.state.id = "conv-a";
    const stale = fire(harness, "session_switch", { reason: "new" });
    harness.state.id = "conv-b";
    const current = fire(harness, "session_switch", { reason: "fork" });
    await slow;
    await stale;
    await current;
    const reports = readLog(logPath);
    check(
        !reports.some((r) => r.session_id === "conv-a"),
        `a stale queued report is cancelled, not reported: ${JSON.stringify(reports)}`,
    );
    check(
        reports.length === 2 &&
            reports[0].session_id === "conv-a0" &&
            reports[1].session_id === "conv-b",
        `the cancelled id's slot is taken by the current id's report: ${JSON.stringify(reports)}`,
    );
} else if (scenario === "throwing-getter") {
    const harness = makeHarness();
    factory(harness.omp);
    harness.state.id = "conv-1";
    harness.state.file = undefined;
    harness.state.throwOnGetSessionId = 1;
    let escaped = false;
    try {
        await fire(harness, "session_start");
    } catch {
        escaped = true;
    }
    check(!escaped, "a throwing getter must not escape into OMP");
    check(readLog(logPath).length === 0, "a throwing getter produces no report");
    // A later healthy event still reports: the failure cost the event, not
    // the reporter.
    await fire(harness, "agent_end");
    const reports = readLog(logPath);
    check(
        reports.length === 1 &&
            reports[0].session_id === "conv-1" &&
            reports[0].source === "agent_end",
        `a later healthy event reports normally: ${JSON.stringify(reports)}`,
    );
} else if (scenario === "child-contexts-silent") {
    // Every non-interactive context shape stays silent across every
    // subscribed event: hasUI:false under each known mode (the child mode
    // value itself is not pinned upstream, so all four are covered), plus
    // a ctx missing the fields entirely. Each child ctx carries its OWN
    // session manager whose stale-id check is self-consistent — the gate,
    // not the id check, must refuse. Afterwards the parent still reports:
    // refused children cost nothing, not even queue position.
    const harness = makeHarness();
    factory(harness.omp);
    harness.state.id = "conv-1";
    harness.state.file = undefined;
    const events = ["session_start", "session_switch", "session_branch", "agent_end"];
    for (const mode of ["print", "rpc", "json", "tui", undefined]) {
        const child = makeChildCtx(mode);
        for (const name of events) {
            await fire(harness, name, { reason: "new" }, child.ctx);
        }
    }
    check(
        readLog(logPath).length === 0,
        `all child-shaped contexts must stay silent: ${JSON.stringify(readLog(logPath))}`,
    );
    await fire(harness, "session_start");
    const reports = readLog(logPath);
    check(
        reports.length === 1 && reports[0].session_id === "conv-1",
        `the parent still reports after refused children: ${JSON.stringify(reports)}`,
    );
    results.reports = reports.length;
} else if (scenario === "child-session-start-no-dispatch") {
    // A child-shaped session_start sharing the parent's factory import
    // dispatches nothing — while the parent's own start and a later switch
    // both report in order. The child manager names a live id of its own,
    // so a stale-id check alone would accept it.
    const harness = makeHarness();
    factory(harness.omp);
    harness.state.id = "conv-1";
    harness.state.file = undefined;
    const child = makeChildCtx("rpc");
    await fire(harness, "session_start");
    await fire(harness, "session_start", {}, child.ctx);
    harness.state.id = "conv-2";
    await fire(harness, "session_switch", { reason: "resume" });
    await fire(harness, "agent_end", {}, child.ctx);
    const reports = readLog(logPath);
    check(
        reports.length === 2 &&
            reports[0].session_id === "conv-1" &&
            reports[0].source === "session_start" &&
            reports[1].session_id === "conv-2" &&
            reports[1].source === "session_switch:resume",
        `only the parent's events report, in order: ${JSON.stringify(reports)}`,
    );
} else if (scenario === "two-reporter-ordering") {
    // Same-process parent + child factories (two extension instances, one
    // import — the built-in delegation shape): the child's events must
    // neither dispatch nor disturb the parent's queue. A parent report
    // queued AROUND child events still dispatches promptly and in order.
    const parent = makeHarness();
    factory(parent.omp);
    const childHandlers = {};
    const recording = { on: (name, handler) => { childHandlers[name] = handler; } };
    factory(recording);
    parent.state.id = "conv-a";
    parent.state.file = undefined;
    const child = makeChildCtx("print");
    const first = fire(parent, "session_start");
    await first;
    await childHandlers["session_start"]({}, child.ctx);
    await childHandlers["session_switch"]({ reason: "new" }, child.ctx);
    parent.state.id = "conv-b";
    await fire(parent, "session_switch", { reason: "fork" });
    await childHandlers["agent_end"]({}, child.ctx);
    const reports = readLog(logPath);
    check(
        reports.length === 2 &&
            reports[0].session_id === "conv-a" &&
            reports[1].session_id === "conv-b",
        `the parent queue is unpoisoned and ordered: ${JSON.stringify(reports)}`,
    );
    check(
        !reports.some((r) => r.session_id === "child-9"),
        `the child factory dispatched nothing: ${JSON.stringify(reports)}`,
    );
} else if (scenario === "throwing-context") {
    // A ctx whose eligibility read throws is a complete no-op — the gate
    // runs before identity, so the throwing getter below is never even
    // reached for dispatch — and the reporter recovers for later events.
    const harness = makeHarness();
    factory(harness.omp);
    harness.state.id = "conv-1";
    harness.state.file = undefined;
    const hostile = {
        get hasUI() {
            throw new Error("context contract violation");
        },
        get mode() {
            throw new Error("context contract violation");
        },
        sessionManager: {
            getSessionId() {
                throw new Error("must not be read past the gate");
            },
            getSessionFile() {
                throw new Error("must not be read past the gate");
            },
        },
    };
    let escaped = false;
    try {
        await fire(harness, "session_start", {}, hostile);
    } catch {
        escaped = true;
    }
    check(!escaped, "a throwing context must not escape into OMP");
    check(readLog(logPath).length === 0, "a throwing context produces no report");
    await fire(harness, "agent_end");
    const reports = readLog(logPath);
    check(
        reports.length === 1 && reports[0].session_id === "conv-1",
        `a later healthy event reports normally: ${JSON.stringify(reports)}`,
    );
} else {
    console.error(`unknown scenario: ${scenario}`);
    process.exit(2);
}

if (problems.length > 0) {
    console.log(JSON.stringify({ scenario, ok: false, problems }));
    process.exit(1);
}
console.log(JSON.stringify({ scenario, ok: true }));
