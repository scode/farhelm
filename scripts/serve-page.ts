#!/usr/bin/env -S deno run -A
/**
 * Serve an agent-written page to the maintainer's browser, on loopback only, with every response uncacheable.
 *
 * This is the one way agents in this repository serve a local page: the review-triage decision page, the planning
 * flow's up-front questions, and any page a later flow adds. What goes on the page is the agent's to decide; this
 * script only serves the directory it is given, which must hold the page as `index.html` (the printed URL is `/`, and
 * directories are never listed). Give it a directory of its own rather than a whole scratch directory: any local
 * account that guesses a file name can read what is served. Usage:
 *
 *     scripts/serve-page.ts <dir> [--port <n>]
 *
 * It prints `serving http://127.0.0.1:<port>/` once it listens, then one log line per request, and serves until killed.
 * Run it as a background command and stop it when the page is no longer needed. The request log is kept on purpose:
 * it is how the incident below was diagnosed.
 *
 * Every response carries `Cache-Control: no-store`, and that is the reason this script exists. Agents used to serve
 * these pages with `python3 -m http.server`, which sends `Last-Modified` and no `Cache-Control`, so the browser picks
 * its own freshness. On 2026-10-22 that showed the maintainer a planning session's page, cached hours earlier, when a
 * triage session was serving a different page at the same URL: the server's log showed the browser fetching only the
 * favicon. Every flow shares one URL, so a cached copy can be another session's page or an older version of this one.
 * Conditional request headers are dropped before serving, so the server never answers 304 and lets the browser keep a
 * copy it cached from an earlier server, and redirects get the header too.
 *
 * The port defaults to 14003, the todo picker's port, because the maintainer keeps that one URL open and may forward
 * that one port from another machine. Like `scripts/todo-picker.py`, this is not a test harness, so the root AGENTS.md
 * rule against fixed ports does not apply. When the port is taken, the script moves up to the next free port, within
 * 20, and prints the URL it actually got, because the flows that use it say to move on without asking. A moved port is
 * not reachable through a forward of 14003, and a port squatter can exploit the move
 * (`review_feedback_queue/todo-picker-port-squatter-steers-planner.md`); the flows have the agent say plainly that the
 * port changed.
 *
 * It never stops whatever holds a port, lists no directories, and binds nothing but `127.0.0.1`.
 */

import { serveDir } from "jsr:@std/http@1.1.4/file-server";

export const DEFAULT_PORT = 14003;
const PORTS_TRIED = 20;
const CONDITIONAL_HEADERS = ["if-modified-since", "if-none-match", "if-match", "if-unmodified-since", "if-range"];

/**
 * Serve one request from `dir`, uncacheable whatever the status.
 *
 * `serveDir`'s own `headers` option skips every 3xx (so a 304 or a trailing-slash 301 would go out cacheable) and every
 * 405, which is why the header is set here on a copy of the response instead. The copy is needed because a redirect's
 * headers are immutable.
 */
export async function handle(req: Request, dir: string): Promise<Response> {
  const headers = new Headers(req.headers);
  for (const name of CONDITIONAL_HEADERS) headers.delete(name);
  const res = await serveDir(new Request(req.url, { method: req.method, headers }), { fsRoot: dir });
  const out = new Response(res.body, res);
  out.headers.set("Cache-Control", "no-store");
  out.headers.delete("ETag");
  out.headers.delete("Last-Modified");
  return out;
}

/**
 * Listen on `127.0.0.1` at `firstPort`, or the next free port above it, serving `dir`.
 *
 * Throws when every port in the range is taken, or for any bind error other than the port being in use.
 */
export function startServer(dir: string, firstPort: number): { server: Deno.HttpServer; port: number } {
  const last = Math.min(firstPort + PORTS_TRIED - 1, 65535);
  for (let port = firstPort; port <= last; port++) {
    try {
      const server = Deno.serve({ hostname: "127.0.0.1", port, onListen: () => {} }, (req) => handle(req, dir));
      return { server, port };
    } catch (err) {
      if (!(err instanceof Deno.errors.AddrInUse)) throw err;
    }
  }
  throw new Error(`no free port in ${firstPort}-${last} on 127.0.0.1`);
}

if (import.meta.main) {
  const usage = "usage: scripts/serve-page.ts <dir> [--port <n>]";
  const args = [...Deno.args];
  let port = DEFAULT_PORT;
  const portAt = args.indexOf("--port");
  if (portAt !== -1) {
    const value = args[portAt + 1] ?? "";
    port = /^\d+$/.test(value) ? Number(value) : NaN;
    args.splice(portAt, 2);
  }
  if (args.length !== 1 || !Number.isInteger(port) || port < 1 || port > 65535) {
    console.error(usage);
    Deno.exit(2);
  }
  // Without index.html the printed URL is a 404, which reads to the maintainer as a broken page.
  try {
    Deno.statSync(`${args[0]}/index.html`);
  } catch (err) {
    if (!(err instanceof Deno.errors.NotFound)) throw err;
    console.error(`serve-page: no index.html in ${args[0]}`);
    Deno.exit(2);
  }
  const { port: bound } = startServer(args[0], port);
  console.log(`serving http://127.0.0.1:${bound}/`);
}

// ---- Tests --------------------------------------------------------------------------------------------------------

/** Run `body` against a server for a fresh page directory, cleaning both up whatever happens. */
async function withServer(body: (base: string) => Promise<void>): Promise<void> {
  const dir = await Deno.makeTempDir();
  try {
    await Deno.writeTextFile(`${dir}/index.html`, "<p>page</p>");
    await Deno.mkdir(`${dir}/sub`);
    const probe = Deno.listen({ hostname: "127.0.0.1", port: 0 });
    const firstPort = probe.addr.port;
    probe.close();
    const { server, port } = startServer(dir, firstPort);
    try {
      await body(`http://127.0.0.1:${port}`);
    } finally {
      await server.shutdown();
    }
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
}

function expectNoStore(res: Response, what: string): void {
  if (res.headers.get("cache-control") !== "no-store") {
    throw new Error(`${what}: Cache-Control was ${res.headers.get("cache-control")}`);
  }
}

Deno.test("the page is served with no-store, so a browser never shows another session's cached page", async () => {
  await withServer(async (base) => {
    const res = await fetch(`${base}/`);
    const body = await res.text();
    if (res.status !== 200 || body !== "<p>page</p>") throw new Error(`unexpected response ${res.status}: ${body}`);
    expectNoStore(res, "page");
  });
});

Deno.test("a browser revalidating an old cached copy gets the page, never a 304 that would keep the old copy", async () => {
  await withServer(async (base) => {
    const res = await fetch(`${base}/`, { headers: { "If-Modified-Since": "Fri, 01 Jan 2100 00:00:00 GMT" } });
    await res.body?.cancel();
    if (res.status !== 200) throw new Error(`status ${res.status}, expected 200`);
    expectNoStore(res, "revalidation");
  });
});

Deno.test("redirects are uncacheable too, so a later session's layout is never shadowed by a cached 301", async () => {
  await withServer(async (base) => {
    const res = await fetch(`${base}/sub`, { redirect: "manual" });
    await res.body?.cancel();
    if (res.status < 300 || res.status >= 400) throw new Error(`status ${res.status}, expected a redirect`);
    expectNoStore(res, "redirect");
  });
});

Deno.test("a taken port moves the server to a later free one instead of failing", async () => {
  const dir = await Deno.makeTempDir();
  const taken = Deno.listen({ hostname: "127.0.0.1", port: 0 });
  try {
    const { server, port } = startServer(dir, taken.addr.port);
    await server.shutdown();
    if (port <= taken.addr.port) throw new Error(`bound ${port}, expected a port above ${taken.addr.port}`);
  } finally {
    taken.close();
    await Deno.remove(dir, { recursive: true });
  }
});
