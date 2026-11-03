// The host-terminal run's premise, checked rather than assumed: on that run
// (e2e/host-terminal-specs.ts) every terminal of a session on the stack's
// local supervisor is a terminal in a terminal host and attaches as a grid
// stream. Only the stack's "remote" supervisor keeps old terminals, on
// purpose, and its sessions are recognized by their host, never inferred
// from a lookup that failed.
//
// Why: every spec helper reads whichever component an attachment holds, so a
// create, tab or remount path that fell back to tmux would pass its specs on
// xterm.js and the run would claim parity it never exercised. Global setup
// checks each stack once; this module checks every terminal socket opened in
// a browser context the test observes (the default context, every context
// `observeContext` observes, `newObservedContext`'s among them, and any a
// test registers with `watchHostPremise`), from the stream notice the helm
// sends on it (the one fact that decides the component), through an
// automatic fixture in evidence.ts, with the readiness waits in
// terminal-readiness.ts as an earlier signal.

import type { APIRequestContext, BrowserContext, Page, TestInfo, WebSocket } from "@playwright/test";
import { SUITE_TERMINALS_METADATA } from "../../host-terminal-specs";

/** How long one premise lookup may take: a stalled helm fails the check rather than the test's timing. */
const LOOKUP_TIMEOUT_MS = 5_000;

/**
 * Whether `config` is the main suite's host-terminal run: its config says so
 * in its metadata (playwright.config.ts), so a capture or other config whose
 * runner merely inherited an exported variable is never held to this premise.
 */
export function hostPremiseActive(config: { metadata?: Record<string, unknown> }): boolean {
  return config.metadata?.[SUITE_TERMINALS_METADATA] === "host";
}

/** The session and tab a terminal socket's URL names, or null for any other socket. */
export function terminalSocketTarget(url: string): { id: string; tabId?: string } | null {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return null;
  }
  const parts = parsed.pathname.split("/");
  const shaped = parts[1] === "api" && parts[2] === "sessions" && parts[4] === "term" &&
    (parts.length === 5 || (parts.length === 6 && parts[5] === "unowned"));
  if (!shaped) return null;
  let id: string;
  try {
    id = decodeURIComponent(parts[3]);
  } catch {
    return null;
  }
  const tab = parsed.searchParams.get("tab");
  return tab === null ? { id } : { id, tabId: tab };
}

/** Options a caller passes when its premise needs an exception it can name. */
export interface PremiseOptions {
  /**
   * The test serves this session itself (a fabricated helm reply), so the
   * helm cannot list it; its absence is then expected rather than a failed
   * lookup. Off by default: an absent row is otherwise a failure.
   */
  fabricated?: boolean;
}

/**
 * Why an old terminal (a byte stream, xterm.js) is not allowed for session
 * `id` on a host-terminal run, or null when it is: only for a session whose
 * row names a host that the helm lists as other than the local one (the
 * stack's "remote" supervisor). Both lookups must succeed and be well
 * formed; an answer that cannot establish the session's host is itself the
 * failure, with its diagnostics, never an exemption. A caller that declared
 * a fabricated session (`options.fabricated`) is allowed its absence.
 */
export async function oldTerminalProblem(
  request: APIRequestContext,
  target: { id: string; tabId?: string },
  options: PremiseOptions = {},
): Promise<string | null> {
  const what = `session ${target.id}${target.tabId === undefined ? "" : ` tab ${target.tabId}`} attached as an old terminal`;
  let row: any;
  try {
    const response = await request.get(`/api/sessions/${encodeURIComponent(target.id)}`, {
      timeout: LOOKUP_TIMEOUT_MS,
    });
    if (response.status() === 404 && options.fabricated) return null;
    if (!response.ok()) return `${what}, and its row answered ${response.status()}`;
    row = await response.json();
  } catch (error) {
    return `${what}, and its row could not be read: ${error}`;
  }
  if (!row || row.id !== target.id || typeof row.host !== "number") return `${what}, and its row is malformed`;
  let hosts: any[];
  try {
    const listed = await request.get("/api/hosts", { timeout: LOOKUP_TIMEOUT_MS });
    if (!listed.ok()) return `${what}, and the host listing answered ${listed.status()}`;
    const body = await listed.json();
    if (!body || !Array.isArray(body.hosts)) return `${what}, and the host listing is malformed`;
    hosts = body.hosts;
  } catch (error) {
    return `${what}, and the host listing failed: ${error}`;
  }
  const host = hosts.find((candidate) => candidate?.id === row.host);
  if (!host || typeof host.kind !== "string") return `${what}, and its host ${row.host} is not in the host listing`;
  if (host.kind !== "local") return null;
  return `${what} on the local host, whose supervisor creates terminals in terminal hosts on this run ` +
    `(its row: agent_terminal ${JSON.stringify(row.agent_terminal)}, supervisor_creates_host_terminals ` +
    `${JSON.stringify(row.supervisor_creates_host_terminals)})`;
}

/**
 * One test's watch over every terminal socket its pages open, collecting the
 * problems `oldTerminalProblem` finds for each byte-stream notice.
 *
 * Judged when the notice arrives, while the session still exists (a test
 * usually deletes its sessions before it ends), and reported at the end of
 * the test by `finish`.
 */
export class HostPremiseWatch {
  private readonly pending: Promise<string | null>[] = [];
  private readonly pages = new WeakSet<Page>();
  private readonly contexts = new WeakSet<BrowserContext>();

  constructor(private readonly request: APIRequestContext, readonly options: PremiseOptions) {}

  /** Watch every page this context has or opens. */
  watchContext(context: BrowserContext): void {
    if (this.contexts.has(context)) return;
    this.contexts.add(context);
    context.on("page", (page) => this.watchPage(page));
    for (const page of context.pages()) this.watchPage(page);
  }

  /** Watch every terminal socket this page opens, across its reloads. */
  watchPage(page: Page): void {
    if (this.pages.has(page)) return;
    this.pages.add(page);
    page.on("websocket", (socket: WebSocket) => {
      const target = terminalSocketTarget(socket.url());
      if (!target) return;
      socket.on("framereceived", ({ payload }) => {
        if (typeof payload !== "string") return;
        let message: any;
        try {
          message = JSON.parse(payload);
        } catch {
          return;
        }
        if (message?.type !== "stream") return;
        if (message.kind === "grid") return;
        this.pending.push(
          message.kind === "bytes"
            ? oldTerminalProblem(this.request, target, this.options)
            : Promise.resolve(`session ${target.id} sent a stream notice of an unknown kind`),
        );
      });
    });
  }

  /** Every problem found, after every pending lookup has answered. */
  async finish(testInfo: TestInfo): Promise<string[]> {
    const problems = (await Promise.all(this.pending)).filter((problem): problem is string => problem !== null);
    for (const problem of problems) testInfo.annotations.push({ type: "host premise", description: problem });
    return problems;
  }
}

/** The running test's watch, for contexts made outside the fixtures (`newObservedContext`). */
let activeWatch: HostPremiseWatch | null = null;

/** Make `watch` the running test's (null when it ends); one test runs per worker at a time. */
export function setActiveHostPremiseWatch(watch: HostPremiseWatch | null): void {
  activeWatch = watch;
}

/** The running test's watch, if this is a host-terminal run. */
export function activeHostPremiseWatch(): HostPremiseWatch | null {
  return activeWatch;
}

/**
 * Put `context` under the running test's watch, if this is a host-terminal
 * run: for a context a test makes itself without `observeContext`, before it
 * opens any page there. Idempotent.
 */
export function watchHostPremise(context: BrowserContext): void {
  activeWatch?.watchContext(context);
}
