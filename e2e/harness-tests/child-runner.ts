import type { TestInfo } from "@playwright/test";
import { spawn } from "node:child_process";
import { close as closeFd, constants, fstat, open as openFd } from "node:fs";
import { mkdir, open, realpath, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { acceptedBrowserEvent, timelineConsolePrefix } from "../tests/helpers/evidence";
import { BrowserTimeline } from "../tests/helpers/timeline";

const STREAM_LIMIT = 1024 * 1024;
const CHILD_DEADLINE_MS = 25_000;
const STDIO_DRAIN_GRACE_MS = 2_000;
const TRACE_ENTRY_LIMIT = 128;
const TRACE_BYTES_LIMIT = 8 * 1024 * 1024;
const TRACE_LINE_LIMIT = 256 * 1024;

export type ChildOutcome = "failure" | "timeout" | "unexpected-pass" | "teardown-failure";

interface ChildArtifacts {
  directory: string;
  exitCode: number | null;
  killedByParent: boolean;
  outputOverflow: boolean;
  report: JsonReport;
  timeline: Buffer;
  traceConsolePayloads: string[];
}

interface JsonReport {
  suites?: JsonSuite[];
}

interface JsonSuite {
  specs?: JsonSpec[];
  suites?: JsonSuite[];
}

interface JsonSpec {
  title?: unknown;
  tests?: JsonTest[];
}

interface JsonTest {
  projectName?: unknown;
  expectedStatus?: unknown;
  status?: unknown;
  results?: JsonResult[];
}

interface JsonResult {
  status?: unknown;
  error?: unknown;
  errors?: unknown[];
  attachments?: JsonAttachment[];
}

interface JsonAttachment {
  name?: unknown;
  path?: unknown;
  body?: unknown;
  contentType?: unknown;
}

/**
 * Run one exact intentional child and retain its evidence if any assertion fails.
 *
 * The direct child is reaped and its local pipes are released before
 * verification begins. A failed verification deliberately leaves its bounded
 * artifacts under the parent test's output directory for diagnosis.
 * A Python supervisor retains the CLI's wait identity until group cleanup finishes;
 * Linux subreaping additionally owns detached browser descendants after CLI exit.
 */
export async function verifyIntentionalChild(
  testInfo: TestInfo,
  title: string,
  outcome: ChildOutcome,
): Promise<void> {
  const directory = testInfo.outputPath(`child-${slug(title)}`);
  await mkdir(directory, { recursive: true });
  let verified = false;
  try {
    const artifacts = await runChild(testInfo.project.name, title, directory);
    verifySelectionAndOutcome(artifacts, testInfo.project.name, title, outcome);
    verifyTimeline(artifacts.timeline, outcome, testInfo.project.name, title);
    verifyTrace(artifacts.traceConsolePayloads, outcome);
    verified = true;
  } catch (error) {
    throw new Error(`child evidence retained at ${directory}: ${fixedError(error)}`);
  } finally {
    if (verified) await rm(directory, { recursive: true, force: true }).catch(() => {});
  }
}

/** Spawn, bound, reap, and decode one child before returning any evidence. */
async function runChild(project: string, title: string, directory: string): Promise<ChildArtifacts> {
  const cli = path.join(__dirname, "../node_modules/@playwright/test/cli.js");
  const child = spawn("python3", [
    path.join(__dirname, "supervise-child.py"),
    `--timeout=${CHILD_DEADLINE_MS / 1000}`,
    "--",
    process.execPath,
    cli,
    "test",
    "timeline-child.failure.ts",
    "--config=child.config.ts",
    `--project=${project}`,
    `--grep=${escapeRegex(title)}$`,
    "--reporter=json",
    "--workers=1",
    `--output=${directory}`,
  ], {
    cwd: __dirname,
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = Buffer.alloc(0);
  let stderr = Buffer.alloc(0);
  let outputOverflow = false;
  let exited = false;
  let terminationRequested = false;
  let drainTimedOut = false;
  const leaderHasExited = () =>
    exited || child.exitCode !== null || child.signalCode !== null;
  const requestTermination = () => {
    if (leaderHasExited() || terminationRequested) return;
    terminationRequested = true;
    // EOF is a cancellation lease, so Node never signals a numeric process group
    // after libuv may have reaped its leader. Parent death closes this pipe too.
    child.stdin.end();
  };
  child.stdin.on("error", () => {}); // An already-exited supervisor has no lease to cancel.
  const collect = (current: Buffer, chunk: Buffer): Buffer => {
    if (current.byteLength >= STREAM_LIMIT) {
      outputOverflow = true;
      return current;
    }
    const remaining = STREAM_LIMIT - current.byteLength;
    if (chunk.byteLength > remaining) outputOverflow = true;
    return Buffer.concat([current, chunk.subarray(0, remaining)]);
  };
  child.stdout.on("data", (chunk: Buffer) => {
    stdout = collect(stdout, chunk);
    if (outputOverflow) requestTermination();
  });
  child.stderr.on("data", (chunk: Buffer) => {
    stderr = collect(stderr, chunk);
    if (outputOverflow) requestTermination();
  });

  let killedByParent = false;
  const exit = new Promise<{ code: number | null; signal: NodeJS.Signals | null }>((resolve, reject) => {
    child.once("error", reject);
    // `close` follows both process exit and stdio drain, so the bounded report
    // cannot be parsed while its final bytes are still in a pipe.
    child.once("close", (code, signal) => resolve({ code, signal }));
  });
  const deadline = setTimeout(() => {
    killedByParent = true;
    requestTermination();
  }, CHILD_DEADLINE_MS);
  let drainDeadline: NodeJS.Timeout | undefined;
  child.once("exit", () => {
    // The supervisor has already cleaned its owned descendants before exiting.
    // A bounded drain still protects against unsupported escaped-pipe cases.
    exited = true;
    clearTimeout(deadline);
    drainDeadline = setTimeout(() => {
      drainTimedOut = true;
      child.stdout.destroy();
      child.stderr.destroy();
    }, STDIO_DRAIN_GRACE_MS);
  });

  let result: { code: number | null; signal: NodeJS.Signals | null };
  try {
    result = await exit;
  } finally {
    clearTimeout(deadline);
    child.stdin.end();
    if (drainDeadline !== undefined) clearTimeout(drainDeadline);
    child.stdout.removeAllListeners();
    child.stderr.removeAllListeners();
  }

  const reportPath = path.join(directory, "report.json");
  await Promise.all([
    writeFile(reportPath, stdout),
    writeFile(path.join(directory, "stdout.log"), stdout),
    writeFile(path.join(directory, "stderr.log"), stderr),
  ]);
  if (killedByParent || outputOverflow || drainTimedOut || result.signal !== null) {
    throw new Error("child did not reach its intended Playwright outcome");
  }

  let report: JsonReport;
  try {
    report = JSON.parse(stdout.toString("utf8")) as JsonReport;
  } catch {
    throw new Error("child JSON reporter output was absent or malformed");
  }
  const selected = flattenTests(report);
  if (selected.length !== 1) throw new Error(`child reporter contained ${selected.length} tests`);
  const resultRecord = selected[0].test.results?.at(-1);
  if (!resultRecord) throw new Error("child reporter omitted its final result");
  const timelineAttachment = uniqueAttachment(resultRecord, "browser-timeline.jsonl");
  const traceAttachment = uniqueAttachment(resultRecord, "trace");
  const timeline = await readAttachment(timelineAttachment, directory);
  await writeFile(path.join(directory, "browser-timeline.jsonl"), timeline);
  const tracePath = await attachmentPath(traceAttachment, directory);
  const traceConsolePayloads = await inspectTrace(tracePath);

  return {
    directory,
    exitCode: result.code,
    killedByParent,
    outputOverflow,
    report,
    timeline,
    traceConsolePayloads,
  };
}

/** Refuse every nonzero result that did not come from the requested test outcome. */
function verifySelectionAndOutcome(
  artifacts: ChildArtifacts,
  project: string,
  title: string,
  outcome: ChildOutcome,
): void {
  if (artifacts.exitCode !== 1) {
    throw new Error("intentional child did not reach Playwright's normal failure exit");
  }
  if (artifacts.killedByParent || artifacts.outputOverflow) {
    throw new Error("parent bounds, rather than the selected test, ended the child");
  }
  const selected = flattenTests(artifacts.report);
  if (selected.length !== 1 || selected[0].title !== title || selected[0].test.projectName !== project) {
    throw new Error("JSON reporter did not identify the exact requested test and engine");
  }
  const test = selected[0].test;
  const result = test.results?.at(-1);
  if (!result || test.status !== "unexpected") {
    throw new Error("selected child was not an unexpected outcome");
  }

  const expected = {
    failure: { status: "failed", expectedStatus: "passed" },
    timeout: { status: "timedOut", expectedStatus: "passed" },
    "unexpected-pass": { status: "passed", expectedStatus: "failed" },
    "teardown-failure": { status: "failed", expectedStatus: "passed" },
  }[outcome];
  if (result.status !== expected.status || test.expectedStatus !== expected.expectedStatus) {
    throw new Error("selected child reported the wrong structured outcome");
  }
  if (outcome !== "unexpected-pass" && result.error === undefined && !(result.errors?.length)) {
    throw new Error("selected failing child carried no reporter error");
  }
}

/** Check the copied attachment's byte bounds, identity, lifecycle, and privacy. */
function verifyTimeline(
  buffer: Buffer,
  outcome: ChildOutcome,
  project: string,
  title: string,
): void {
  if (buffer.byteLength > 256 * 1024) throw new Error("attached timeline exceeded its artifact bound");
  const lines = buffer.toString("utf8").trimEnd().split("\n");
  if (lines.length > 1024 || lines.some((line) => Buffer.byteLength(line, "utf8") + 1 > 2048)) {
    throw new Error("attached timeline exceeded its record bounds");
  }
  const records = lines.map((line) => JSON.parse(line) as Record<string, unknown>);
  const identity = records[0]?.identity as Record<string, unknown> | undefined;
  if (
    records[0]?.kind !== "browser-timeline" ||
    identity?.test !== title ||
    identity?.file !== "timeline-child.failure.ts" ||
    identity?.project !== project
  ) {
    throw new Error("timeline header did not retain the selected child's bounded identity");
  }
  const kinds = records.map((record) => record.kind);
  const afterEach = kinds.lastIndexOf("after-each");
  const contextClose = kinds.lastIndexOf("context-close");
  if (afterEach < 0 || contextClose <= afterEach) {
    throw new Error("afterEach and default context close were not retained before attachment");
  }
  if (records.at(-1)?.kind !== "summary") throw new Error("timeline footer was not retained");

  const serialized = buffer.toString("utf8");
  if (serialized.includes("private-value") || serialized.includes("private-timeout-value")) {
    throw new Error("timeline retained private input text");
  }
  if (outcome === "timeout" && !kinds.includes("timeout-premise")) {
    throw new Error("timeout occurred before its observed premise");
  }
  if (outcome === "failure" && !serialized.includes('"outcome":"failure"')) {
    throw new Error("body failure did not retain its completed input premise");
  }
  if (outcome === "unexpected-pass" && !serialized.includes('"outcome":"unexpected-pass"')) {
    throw new Error("unexpected pass did not retain its successful body premise");
  }
  if (outcome === "teardown-failure" && !serialized.includes('"outcome":"teardown"')) {
    throw new Error("teardown failure did not follow a successful body");
  }
}

/** Restrict trace assertions to the observer's own console payloads. */
function verifyTrace(payloads: string[], outcome: ChildOutcome): void {
  if (payloads.length === 0) throw new Error("retained trace contained no prefixed browser observations");
  for (const payload of payloads) {
    if (!payload.startsWith(timelineConsolePrefix)) {
      throw new Error("trace inspection admitted a non-timeline console payload");
    }
    if (payload.includes("private-value") || payload.includes("private-timeout-value")) {
      throw new Error("prefixed trace console evidence retained private input");
    }
  }
  if (outcome === "timeout" && !payloads.some((payload) => {
    const event: unknown = JSON.parse(payload.slice(timelineConsolePrefix.length));
    return acceptedBrowserEvent(event) && event.event === "input";
  })) {
    throw new Error("timeout trace did not retain the input premise");
  }
}

/** Flatten the reporter hierarchy without accepting tests from hidden siblings. */
function flattenTests(report: JsonReport): Array<{ title: string; test: JsonTest }> {
  const found: Array<{ title: string; test: JsonTest }> = [];
  const visit = (suite: JsonSuite) => {
    for (const spec of suite.specs ?? []) {
      if (typeof spec.title !== "string") continue;
      for (const test of spec.tests ?? []) found.push({ title: spec.title, test });
    }
    for (const child of suite.suites ?? []) visit(child);
  };
  for (const suite of report.suites ?? []) visit(suite);
  return found;
}

/** Require one named artifact so stale or duplicate output cannot satisfy a case. */
function uniqueAttachment(result: JsonResult, name: string): JsonAttachment {
  const matches = (result.attachments ?? []).filter((attachment) => attachment.name === name);
  if (matches.length !== 1) throw new Error(`expected one ${name} attachment, found ${matches.length}`);
  return matches[0];
}

/** Read an inline or copied reporter attachment through the same bounded scope. */
async function readAttachment(attachment: JsonAttachment, directory: string): Promise<Buffer> {
  if (typeof attachment.body === "string") {
    const decoded = Buffer.from(attachment.body, "base64");
    if (decoded.byteLength > 256 * 1024) {
      throw new Error("inline timeline attachment exceeded its byte bound");
    }
    return decoded;
  }
  const handle = await open(await attachmentPath(attachment, directory), "r");
  try {
    const metadata = await handle.stat();
    if (!metadata.isFile()) throw new Error("timeline attachment was not a regular file");
    return await readBounded(handle, 256 * 1024);
  } finally {
    await handle.close();
  }
}

/** Keep reporter-controlled file targets inside the invocation's real directory. */
async function attachmentPath(attachment: JsonAttachment, directory: string): Promise<string> {
  if (typeof attachment.path !== "string") throw new Error("reporter attachment had no readable path");
  const supplied = path.isAbsolute(attachment.path)
    ? attachment.path
    : path.resolve(__dirname, attachment.path);
  const [resolved, root] = await Promise.all([
    realpath(supplied),
    realpath(path.resolve(directory)),
  ]);
  if (resolved !== root && !resolved.startsWith(`${root}${path.sep}`)) {
    throw new Error("reporter attachment escaped the child output directory");
  }
  if (!(await stat(resolved)).isFile()) {
    throw new Error("reporter attachment was not a regular file");
  }
  return resolved;
}

/** Read through one descriptor and stop after the configured limit plus one byte. */
async function readBounded(
  handle: Awaited<ReturnType<typeof open>>,
  limit: number,
): Promise<Buffer> {
  const buffer = Buffer.alloc(limit + 1);
  let offset = 0;
  while (offset < buffer.byteLength) {
    const { bytesRead } = await handle.read(buffer, offset, buffer.byteLength - offset, null);
    if (bytesRead === 0) break;
    offset += bytesRead;
  }
  if (offset > limit) throw new Error("timeline attachment exceeded its byte bound");
  return buffer.subarray(0, offset);
}

/** Read only bounded trace event streams and extract actual console payloads. */
async function inspectTrace(tracePath: string): Promise<string[]> {
  const bundle = await import("playwright-core/lib/utilsBundle");
  const yauzl = (bundle as any).yauzl ?? (bundle as any).default?.yauzl;
  if (!yauzl || typeof yauzl.fromFd !== "function") {
    throw new Error("pinned Playwright does not expose yauzl");
  }

  const zip = await openTraceZip(tracePath, yauzl);
  return inspectTraceZip(zip);
}

/** Own an admitted ZIP until its bounded selected streams finish or fail. */
export function inspectTraceZip(zip: any): Promise<string[]> {
  return new Promise<string[]>((resolve, reject) => {
    const payloads: string[] = [];
    let entries = 0;
    let selectedBytes = 0;
    let selectedCompressedBytes = 0;
    let streamedBytes = 0;
    let settled = false;
    let releaseActiveStream: (() => void) | undefined;
    let onZipError: (error: Error) => void;
    let onZipEnd: () => void;
    let onEntry: (entry: any) => void;
    const finish = (error?: Error) => {
      if (settled) return;
      settled = true;
      const release = releaseActiveStream;
      releaseActiveStream = undefined;
      release?.();
      zip.off("error", onZipError);
      zip.off("end", onZipEnd);
      zip.off("entry", onEntry);
      zip.close();
      if (error) reject(error);
      else resolve(payloads);
    };
    onZipError = (error) => finish(error);
    onZipEnd = () => finish();
    onEntry = (entry) => {
      entries += 1;
      if (entries > TRACE_ENTRY_LIMIT) {
        return finish(new Error("trace ZIP exceeded the entry inspection bound"));
      }
      if (typeof entry.fileName !== "string" || !entry.fileName.endsWith(".trace")) {
        zip.readEntry();
        return;
      }
      if (
        !Number.isSafeInteger(entry.uncompressedSize) || entry.uncompressedSize < 0 ||
        !Number.isSafeInteger(entry.compressedSize) || entry.compressedSize < 0 ||
        entry.uncompressedSize > TRACE_BYTES_LIMIT - selectedBytes ||
        entry.compressedSize > TRACE_BYTES_LIMIT - selectedCompressedBytes
      ) {
        finish(new Error("selected trace data exceeded the byte inspection bound"));
        return;
      }
      selectedBytes += entry.uncompressedSize;
      // Deflate can consume arbitrarily many empty blocks before producing
      // output. Bound compressed input independently of declared/actual output.
      selectedCompressedBytes += entry.compressedSize;
      zip.openReadStream(entry, (streamError: Error | null, stream: any) => {
        if (settled) {
          stream?.destroy();
          return;
        }
        if (streamError || !stream) return finish(streamError ?? new Error("trace entry did not open"));
        const chunks: Buffer[] = [];
        const onData = (chunk: Buffer) => {
          streamedBytes += chunk.byteLength;
          if (streamedBytes > TRACE_BYTES_LIMIT) {
            finish(new Error("trace stream exceeded the byte inspection bound"));
            return;
          }
          chunks.push(chunk);
        };
        const onStreamError = (error: Error) => finish(error);
        const onStreamEnd = () => {
          const release = releaseActiveStream;
          releaseActiveStream = undefined;
          release?.();
          if (settled) return;
          try {
            collectConsolePayloads(Buffer.concat(chunks).toString("utf8"), payloads);
            zip.readEntry();
          } catch (error) {
            finish(error instanceof Error ? error : new Error("trace event parsing failed"));
          }
        };
        releaseActiveStream = () => {
          stream.off("data", onData);
          stream.off("error", onStreamError);
          stream.off("end", onStreamEnd);
          if (!stream.destroyed) stream.destroy();
        };
        stream.on("data", onData);
        stream.on("error", onStreamError);
        stream.on("end", onStreamEnd);
      });
    };
    zip.on("error", onZipError);
    zip.on("end", onZipEnd);
    zip.on("entry", onEntry);
    zip.readEntry();
  });
}

/**
 * Transfer one verified regular descriptor to yauzl, whose close owns it from
 * then on. Nonblocking/nofollow open refuses a replaced FIFO or final symlink;
 * output-directory containment still assumes the reaped child's tree is quiescent.
 */
export async function openTraceZip(tracePath: string, yauzl: any): Promise<any> {
  const fd = await new Promise<number>((resolve, reject) => {
    openFd(tracePath, constants.O_RDONLY | constants.O_NONBLOCK | constants.O_NOFOLLOW,
      (error, descriptor) => error ? reject(error) : resolve(descriptor));
  });
  try {
    const metadata = await new Promise<import("node:fs").Stats>((resolve, reject) => {
      fstat(fd, (error, value) => error ? reject(error) : resolve(value));
    });
    if (!metadata.isFile()) throw new Error("trace attachment was not a regular file");
    return await new Promise((resolve, reject) => {
      yauzl.fromFd(fd, { lazyEntries: true, autoClose: false }, (error: Error | null, zip: any) => {
        if (error || !zip) reject(error ?? new Error("trace ZIP did not open"));
        else resolve(zip);
      });
    });
  } catch (error) {
    await new Promise<void>((resolve) => closeFd(fd, () => resolve()));
    throw error;
  }
}

/** Parse bounded trace-event lines and keep only this observer's console records. */
export function collectConsolePayloads(trace: string, payloads: string[]): void {
  for (const line of trace.split("\n")) {
    if (!line.includes(timelineConsolePrefix)) continue;
    if (Buffer.byteLength(line, "utf8") > TRACE_LINE_LIMIT) {
      throw new Error("trace event containing timeline evidence exceeded its line bound");
    }
    const parsed: unknown = JSON.parse(line);
    // Pinned Playwright emits console events with top-level type and text.
    // An action expression or argument containing the prefix is not evidence
    // that the browser emitted an observer record.
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) continue;
    const event = parsed as Record<string, unknown>;
    if (event.type !== "console" || typeof event.text !== "string" ||
      !event.text.startsWith(timelineConsolePrefix)) continue;
    if (Buffer.byteLength(event.text, "utf8") > BrowserTimeline.maxRecordBytes ||
      !acceptedBrowserEvent(JSON.parse(event.text.slice(timelineConsolePrefix.length)))) {
      throw new Error("trace console contained an invalid browser observation");
    }
    payloads.push(event.text);
    if (payloads.length > 1024) throw new Error("trace contained too many timeline console payloads");
  }
}

function slug(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^$()|[\]\\]/g, "\\$&");
}

function fixedError(error: unknown): string {
  return error instanceof Error ? error.message : "unknown child verification failure";
}
