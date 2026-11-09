/**
 * Opt-in process-memory measurement of the shipped grid component.
 * Node TypeScript keeps this beside the existing Playwright harness and uses
 * its installed, lockfile-selected runtime. No normal *.spec.ts run finds it.
 */
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { freemem, loadavg, totalmem } from "node:os";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, webkit } from "playwright";
import type { BrowserServer, BrowserType } from "playwright";

type Engine = "chromium" | "webkit";
type Scenario = "typical" | "indexed" | "truecolor";
const SCENARIOS: Scenario[] = ["typical", "indexed", "truecolor"];
const ASSETS = [
  "app.css",
  "terminal-theme.js",
  "copy-on-select.js",
  "grid-model.js",
  "grid-style.js",
  "grid-input.js",
  "grid-buffer.js",
  "grid-terminal.js",
  "fonts/JetBrainsMonoNerdFont-Regular.woff2",
  "fonts/JetBrainsMonoNerdFont-Bold.woff2",
];

/** Incremental private envelope decoder; a frame never borrows a larger input chunk.
 * Real sockets give the component one payload at a time. Owning that exact slice
 * keeps benchmark transport buffers out of retained compact-history storage.
 */
export class FrameDecoder {
  pending: Uint8Array = new Uint8Array();
  frames = 0;
  bytes = 0;

  /** Return complete payloads and retain only the next incomplete envelope. */
  push(chunk: Uint8Array): Uint8Array[] {
    const joined = new Uint8Array(this.pending.length + chunk.length);
    joined.set(this.pending);
    joined.set(chunk, this.pending.length);
    const frames: Uint8Array[] = [];
    let offset = 0;
    while (joined.length - offset >= 4) {
      const length = new DataView(joined.buffer, joined.byteOffset + offset, 4).getUint32(0);
      if (length === 0 || length > 1024 * 1024) throw new Error("invalid benchmark frame length");
      if (joined.length - offset - 4 < length) break;
      frames.push(joined.slice(offset + 4, offset + 4 + length));
      this.frames++;
      this.bytes += length;
      offset += 4 + length;
    }
    this.pending = joined.slice(offset);
    return frames;
  }

  /** EOF is valid only at a frame boundary, even when no grid message was decoded. */
  finish(): void {
    if (this.pending.length) throw new Error("truncated benchmark frame");
  }
}

/** Linux process identity survives spaces and parentheses in a process's comm.
 * starttime is a decimal string, preserving all kernel ticks without JS rounding.
 */
export function parseStat(text: string): { pid: number; parent: number; start: string } {
  const open = text.indexOf("(");
  const close = text.lastIndexOf(")");
  const pid = Number(text.slice(0, open).trim());
  const fields = text.slice(close + 1).trim().split(/\s+/);
  if (open < 1 || close <= open || !Number.isSafeInteger(pid) || pid < 1 || !/^\d+$/.test(fields[19] || "")) {
    throw new Error("malformed process stat");
  }
  const parent = Number(fields[1]);
  if (!Number.isSafeInteger(parent) || parent < 0) throw new Error("malformed process parent");
  return { pid, parent, start: fields[19] };
}

/** Required kernel readings, in bytes; a missing field is not a zero sample. */
export function parseMemory(text: string): { rss: number; hwm: number } {
  const field = (name: string) => {
    const value = text.match(new RegExp(`^${name}:\\s+(\\d+) kB$`, "m"));
    if (!value) throw new Error(`missing ${name} in process status`);
    return Number(value[1]) * 1024;
  };
  return { rss: field("VmRSS"), hwm: field("VmHWM") };
}

type Process = ReturnType<typeof parseStat> & { args: string[] };

/** Refuse ambiguity instead of selecting whichever browser helper grew most.
 * Chromium rewrites its Linux process title into one argv entry containing
 * the command line. Recognize a whole flag in either spelling; array
 * membership alone misses a renderer whose title was rewritten.
 */
export function contentProcess(engine: Engine, processes: Process[]): Process {
  const matches = processes.filter(({ args }) =>
    engine === "chromium"
      ? args.some((arg) => /(?:^|\s)--type=renderer(?:\s|$)/.test(arg))
      : args.some((arg) => ["WPEWebProcess", "WebKitWebProcess"].includes(basename(arg)))
  );
  if (matches.length !== 1) {
    throw new Error(
      `expected one owned ${engine} content process; found ${matches.length}: ${matches.map((p) => p.pid)}; ` +
        `owned processes: ${JSON.stringify(processes)}`,
    );
  }
  return matches[0];
}

/** Walk only this browser's owned descendants, including children spawned by worker threads.
 * A helper may exit during discovery. Losing the root or the measured content
 * process is checked separately and never accepted as an ordinary helper exit.
 */
async function descendants(root: number): Promise<Process[]> {
  const found: Process[] = [];
  const queue = [root];
  const seen = new Set<number>();
  while (queue.length) {
    const pid = queue.shift()!;
    if (seen.has(pid)) continue;
    seen.add(pid);
    if (seen.size > 256) throw new Error("owned browser process bound exceeded");
    try {
      const stat = parseStat(await readFile(`/proc/${pid}/stat`, "utf8"));
      const args = (await readFile(`/proc/${pid}/cmdline`, "utf8")).split("\0").filter(Boolean);
      found.push({ ...stat, args });
      const threads = await readdir(`/proc/${pid}/task`);
      if (threads.length > 2048) throw new Error("owned browser thread bound exceeded");
      for (const thread of threads) {
        let children: string;
        try {
          children = await readFile(`/proc/${pid}/task/${thread}/children`, "utf8");
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code === "ENOENT") continue;
          throw error;
        }
        queue.push(...children.trim().split(/\s+/).filter(Boolean).map(Number));
      }
    } catch (error) {
      if (pid !== root && (error as NodeJS.ErrnoException).code === "ENOENT") continue;
      throw error;
    }
  }
  return found;
}

/** Bind both attribution and readings to a process that has not been replaced. */
async function memory(process: Process): Promise<ReturnType<typeof parseMemory>> {
  const before = parseStat(await readFile(`/proc/${process.pid}/stat`, "utf8"));
  const status = parseMemory(await readFile(`/proc/${process.pid}/status`, "utf8"));
  const after = parseStat(await readFile(`/proc/${process.pid}/stat`, "utf8"));
  if (before.start !== process.start || after.start !== process.start) {
    throw new Error("content process identity changed");
  }
  return status;
}

/** Hash exact bytes without retaining the entire generated stream in Node. */
async function sha256(path: string): Promise<string> {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

/** A generated file's identity and payload totals, checked again by the page. */
type Input = { scenario: Scenario; file: string; sha256: string; frames: number; payload_bytes: number };
/** The fixed workload shape and the protocol used by the generating checkout. */
type Manifest = {
  format: number;
  protocol: number;
  cols: number;
  screen_rows: number;
  history_lines: number;
  inputs: Input[];
};

/** Only the fixed benchmark scenarios are served; the manifest also binds their exact payload files. */
async function inputs(directory: string): Promise<Manifest> {
  const manifest = JSON.parse(await readFile(join(directory, "inputs.json"), "utf8")) as Manifest;
  if (
    manifest.format !== 1 || manifest.cols !== 200 || manifest.screen_rows !== 60 || manifest.history_lines !== 12000 ||
    !Array.isArray(manifest.inputs) || manifest.inputs.length !== 3
  ) throw new Error("unexpected benchmark input shape");
  for (const scenario of SCENARIOS) {
    const matching = manifest.inputs.filter((input) => input.scenario === scenario);
    if (matching.length !== 1) throw new Error(`missing or repeated ${scenario} input`);
    const input = matching[0];
    if (
      input.file !== `${scenario}.frames` || !Number.isSafeInteger(input.frames) || input.frames < 1 ||
      !Number.isSafeInteger(input.payload_bytes) || input.payload_bytes < 1 ||
      await sha256(join(directory, input.file)) !== input.sha256
    ) throw new Error(`invalid ${scenario} input identity`);
  }
  return manifest;
}

/** The only inline helper is the benchmark envelope parser, not another grid decoder. */
function html(): string {
  const scripts = ASSETS.filter((name) => name.endsWith(".js")).map((name) => `<script src="/assets/${name}"></script>`)
    .join("");
  return `<!doctype html><html><head><meta charset="utf-8"><link rel="stylesheet" href="/assets/app.css">${scripts}
<script>${FrameDecoder.toString()};window.BenchmarkFrames=FrameDecoder;</script></head>
<body style="margin:0"><div id="term" style="position:relative;width:1700px;height:1000px"></div></body></html>`;
}

/** One private loopback origin serves real assets and streams files with ordinary HTTP backpressure. */
async function serve(root: string, directory: string): Promise<{ origin: string; close: () => Promise<void> }> {
  const assets = join(root, "crates/farhelm-ui/assets");
  const routes = new Map(ASSETS.map((name) => [`/assets/${basename(name)}`, join(assets, name)]));
  for (const scenario of SCENARIOS) routes.set(`/stream/${scenario}`, join(directory, `${scenario}.frames`));
  const server = createServer((request, response) => {
    const path = new URL(request.url || "/", "http://localhost").pathname;
    response.setHeader("Cache-Control", "no-store");
    if (path === "/") {
      response.setHeader("Content-Type", "text/html");
      response.end(html());
      return;
    }
    const file = routes.get(path);
    if (!file) {
      response.writeHead(404);
      response.end();
      return;
    }
    const type = file.endsWith(".js")
      ? "application/javascript"
      : file.endsWith(".css")
      ? "text/css"
      : file.endsWith(".woff2")
      ? "font/woff2"
      : "application/octet-stream";
    response.setHeader("Content-Type", type);
    const stream = createReadStream(file);
    stream.on("error", (error) => response.destroy(error));
    response.on("close", () => stream.destroy());
    stream.pipe(response);
  });
  await new Promise<void>((done, fail) => {
    server.once("error", fail);
    server.listen(0, "127.0.0.1", done);
  });
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("no benchmark server address");
  return {
    origin: `http://127.0.0.1:${address.port}`,
    close: () =>
      new Promise<void>((done, fail) => {
        server.closeAllConnections();
        server.close((error) => error ? fail(error) : done());
      }),
  };
}

/** Small shared-host context, not a reading of other agents' individual processes. */
function load() {
  return { load: loadavg(), free: freemem(), total: totalmem() };
}

/** Measure exactly one fresh page/process, retaining raw readings instead of declaring a performance threshold. */
async function measure(engine: Engine, type: BrowserType, origin: string, input: Input): Promise<unknown> {
  let server: BrowserServer | undefined;
  try {
    server = await type.launchServer({
      headless: true,
      args: engine === "chromium" ? ["--disable-features=SpareRendererForSitePerProcess"] : [],
    });
    const root = server.process().pid;
    if (!root) throw new Error("browser server has no process PID");
    const rootIdentity = parseStat(await readFile(`/proc/${root}/stat`, "utf8"));
    const browser = await type.connect(server.wsEndpoint());
    const page = await browser.newPage({ viewport: { width: 1800, height: 1100 } });
    const errors: string[] = [];
    page.on("pageerror", (error) => {
      if (errors.length < 8) errors.push(error.message);
    });
    await page.goto(origin);
    await page.evaluate(async () => {
      const fonts = await Promise.all([
        document.fonts.load('14px "JetBrains Mono"'),
        document.fonts.load('700 14px "JetBrains Mono"'),
      ]);
      if (
        fonts.some((faces) => faces.length === 0) || !document.fonts.check('14px "JetBrains Mono"') ||
        !document.fonts.check('700 14px "JetBrains Mono"') ||
        document.visibilityState !== "visible"
      ) throw new Error("font or visible-page premise failed");
      const w = window as any;
      const errors: string[] = [];
      w.benchmark = {
        grid: w.farhelmGridTerminal.create(document.getElementById("term"), {
          fontSize: 14,
          fontFamily: '"JetBrains Mono", monospace',
          theme: w.farhelmTerminalTheme,
          platform: navigator.platform,
          send() {},
          onProtocolError: (error: Error) => errors.push(error.message),
        }),
        errors,
      };
    });
    // The same bounded observation follows both GC requests. It never waits
    // for RSS to reach a favorable minimum, and is outside attach timing.
    const settle = async () => {
      await page.requestGC();
      await page.evaluate(async () => {
        for (let i = 0; i < 3; i++) await new Promise<void>((done) => requestAnimationFrame(() => done()));
      });
    };
    await settle();
    const process = contentProcess(engine, await descendants(root));
    const before = await memory(process);
    const hostBefore = load();
    await page.evaluate((scenario) => {
      const w = window as any;
      const bench = w.benchmark;
      bench.start = performance.now();
      bench.eof = false;
      bench.done = false;
      const decoder = new w.BenchmarkFrames();
      const observe = () => {
        if (bench.errors.length) return;
        const model = bench.grid.model;
        if (
          bench.eof && model.historyLength === 12000 && !model.historyPending && model.cols === 200 && model.rows === 60
        ) {
          bench.done = true;
          bench.milliseconds = performance.now() - bench.start;
          bench.frames = decoder.frames;
          bench.bytes = decoder.bytes;
          return;
        }
        requestAnimationFrame(observe);
      };
      requestAnimationFrame(observe);
      void (async () => {
        const response = await fetch(`/stream/${scenario}`);
        if (!response.ok || !response.body) throw new Error("benchmark stream unavailable");
        const reader = response.body.getReader();
        try {
          while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            for (const frame of decoder.push(value)) bench.grid.receive(frame);
          }
          decoder.finish();
          bench.eof = true;
        } finally {
          reader.releaseLock();
        }
      })().catch((error) => bench.errors.push(error.message));
    }, input.scenario);
    try {
      await page.waitForFunction(
        () => (window as any).benchmark.done || (window as any).benchmark.errors.length,
        undefined,
        { timeout: 120_000 },
      );
    } catch (error) {
      const state = await page.evaluate(() => {
        const b = (window as any).benchmark;
        return {
          eof: b.eof,
          history: b.grid.model.historyLength,
          pending: b.grid.model.historyPending,
          cols: b.grid.model.cols,
          rows: b.grid.model.rows,
          errors: b.errors,
        };
      });
      throw new Error(`attach did not complete: ${JSON.stringify({ state, pageErrors: errors })}`, { cause: error });
    }
    const result = await page.evaluate(() => {
      const b = (window as any).benchmark;
      return {
        done: b.done,
        eof: b.eof,
        frames: b.frames,
        bytes: b.bytes,
        milliseconds: b.milliseconds,
        history: b.grid.model.historyLength,
        pending: b.grid.model.historyPending,
        cols: b.grid.model.cols,
        rows: b.grid.model.rows,
        errors: b.errors,
      };
    });
    if (
      !result.done || result.errors.length || errors.length || result.frames !== input.frames ||
      result.bytes !== input.payload_bytes
    ) {
      throw new Error(`benchmark completion failed: ${JSON.stringify({ result, pageErrors: errors })}`);
    }
    const atDrain = await memory(process);
    await settle();
    const after = await memory(process);
    const current = contentProcess(engine, await descendants(root));
    const rootAfter = parseStat(await readFile(`/proc/${root}/stat`, "utf8"));
    if (current.pid !== process.pid || current.start !== process.start || rootAfter.start !== rootIdentity.start) {
      throw new Error("owned browser/content process changed during measurement");
    }
    return {
      engine,
      scenario: input.scenario,
      browser: browser.version(),
      process: { pid: process.pid, start: process.start },
      before,
      atDrain,
      after,
      rss_growth: after.rss - before.rss,
      peak_above_baseline_rss: atDrain.hwm - before.rss,
      preattach_peak_not_exceeded: atDrain.hwm <= before.hwm,
      attach: result,
      hostBefore,
      hostAfter: load(),
    };
  } finally {
    if (server) await server.close();
  }
}

/** Require explicit input identities; command-line defaults choose scope, not a hidden source revision. */
async function main() {
  if (process.platform !== "linux") throw new Error("this benchmark requires Linux /proc process readings");
  const options = new Map<string, string>();
  const allowed = new Set(["root", "input", "label", "output", "engine", "scenario", "repeats"]);
  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i].replace(/^--/, "");
    if (!args[i].startsWith("--") || !allowed.has(key) || options.has(key) || !args[i + 1]) {
      throw new Error("invalid benchmark arguments");
    }
    options.set(key, args[i + 1]);
  }
  for (const key of ["root", "input", "label", "output"]) if (!options.get(key)) throw new Error(`missing --${key}`);
  const engines: Engine[] = options.get("engine") === undefined || options.get("engine") === "all"
    ? ["chromium", "webkit"]
    : [options.get("engine") as Engine];
  const scenarios: Scenario[] = options.get("scenario") === undefined || options.get("scenario") === "all"
    ? SCENARIOS
    : [options.get("scenario") as Scenario];
  if (
    engines.some((name) => !["chromium", "webkit"].includes(name)) ||
    scenarios.some((name) => !SCENARIOS.includes(name))
  ) throw new Error("unknown engine or scenario");
  const repeats = Number(options.get("repeats") || 1);
  if (!Number.isSafeInteger(repeats) || repeats < 1 || repeats > 100) throw new Error("repeats must be 1..100");
  const root = resolve(options.get("root")!);
  const directory = resolve(options.get("input")!);
  const report: { [key: string]: unknown; samples: unknown[] } = {
    source: options.get("label"),
    node: process.version,
    playwright: createRequire(import.meta.url)("playwright/package.json").version,
    measurement: "GC-requested process RSS; lifetime VmHWM includes preattach peaks; no threshold",
    samples: [],
  };
  const output = resolve(options.get("output")!);
  await writeFile(output, JSON.stringify(report, null, 2), { flag: "wx" });
  let server: Awaited<ReturnType<typeof serve>> | undefined;
  try {
    const manifest = await inputs(directory);
    report.inputs = manifest;
    report.assets = Object.fromEntries(
      await Promise.all(ASSETS.map(async (name) => [name, await sha256(join(root, "crates/farhelm-ui/assets", name))])),
    );
    report.driver_sha256 = await sha256(fileURLToPath(import.meta.url));
    report.page_sha256 = createHash("sha256").update(html()).digest("hex");
    await writeFile(output, JSON.stringify(report, null, 2));
    server = await serve(root, directory);
    for (let repeat = 0; repeat < repeats; repeat++) {
      for (const engine of engines) {
        for (const scenario of scenarios) {
          const input = manifest.inputs.find((item) => item.scenario === scenario)!;
          const sample = await measure(engine, engine === "chromium" ? chromium : webkit, server.origin, input);
          report.samples.push({ repeat, ...(sample as object) });
          await writeFile(output, JSON.stringify(report, null, 2));
          console.log(JSON.stringify(sample));
        }
      }
    }
  } catch (error) {
    report.failure = error instanceof Error ? error.message : String(error);
    await writeFile(output, JSON.stringify(report, null, 2));
    throw error;
  } finally {
    if (server) await server.close();
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
