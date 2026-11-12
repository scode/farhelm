// The browser suite's premise, checked rather than assumed: both of every
// stack's supervisors create terminals in terminal hosts, so every terminal
// socket for a real session attaches as a grid stream. A byte stream (an old
// terminal, drawn by xterm.js) is a failure, whichever host the session is on.
//
// Why: every spec helper reads whichever component an attachment holds, so
// a create, tab or remount path that ended up on a byte stream would pass
// its specs on xterm.js and the run would claim coverage of new terminals it
// never exercised. This module checks every terminal socket opened in a browser
// context the test observes (the default context, every context
// `observeContext` observes, `newObservedContext`'s among them, and any a
// test registers with `watchHostPremise`), from the stream notice the helm
// sends on it (the one fact that decides the component), through an
// automatic fixture in evidence.ts, with the readiness waits in
// terminal-readiness.ts as an earlier signal.

import type { APIRequestContext, BrowserContext, Page, TestInfo, WebSocket } from "@playwright/test";

/** How long one premise lookup may take: a stalled helm fails the check rather than the test's timing. */
const LOOKUP_TIMEOUT_MS = 5_000;

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
 * Why session `id`'s old terminal (a byte stream, xterm.js) breaks the
 * browser suite's premise, or null when it does not: only a session the
 * caller declared fabricated (`options.fabricated`), which the helm then does
 * not list, is allowed one. Every real session is on a supervisor that
 * creates terminals in hosts, so the answer is otherwise always a problem;
 * the session's row is read only to say what it reported, and a row that
 * cannot be read is reported as such, never taken as an exemption.
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
  if (!row || row.id !== target.id) return `${what}, and its row is malformed`;
  return `${what} on a stack whose supervisors create terminals in terminal hosts ` +
    `(its row: host ${JSON.stringify(row.host)}, agent_terminal ${JSON.stringify(row.agent_terminal)}, ` +
    `supervisor_creates_host_terminals ${JSON.stringify(row.supervisor_creates_host_terminals)})`;
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
  /** Made at the first byte-stream notice: an ordinary test never needs it. */
  private request: Promise<APIRequestContext> | null = null;

  /**
   * `newRequest` makes the authorized request context lookups use. It is
   * called at most once, and only when a byte-stream notice arrives, so the
   * watch costs nothing beyond its listeners on a test whose terminals are
   * all new; `finish` disposes what it made.
   */
  constructor(private readonly newRequest: () => Promise<APIRequestContext>, readonly options: PremiseOptions) {}

  private lookup(): Promise<APIRequestContext> {
    this.request ??= this.newRequest();
    return this.request;
  }

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
            ? this.lookup().then((request) => oldTerminalProblem(request, target, this.options))
            : Promise.resolve(`session ${target.id} sent a stream notice of an unknown kind`),
        );
      });
    });
  }

  /**
   * Every problem found, after every pending lookup has answered, with the
   * lookups' request context disposed. A context that could not be made is
   * itself a problem: the premise could not be judged.
   */
  async finish(testInfo: TestInfo): Promise<string[]> {
    const settled = await Promise.allSettled(this.pending);
    const problems = settled.flatMap((result) =>
      result.status === "rejected"
        ? [`a premise lookup failed: ${result.reason}`]
        : result.value === null
        ? []
        : [result.value]
    );
    if (this.request) await this.request.then((request) => request.dispose(), () => undefined);
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

/** The running test's watch, or null between tests. */
export function activeHostPremiseWatch(): HostPremiseWatch | null {
  return activeWatch;
}

/**
 * Put `context` under the running test's watch: for a context a test makes
 * itself without `observeContext`, before it opens any page there.
 * Idempotent, and nothing between tests.
 */
export function watchHostPremise(context: BrowserContext): void {
  activeWatch?.watchContext(context);
}
