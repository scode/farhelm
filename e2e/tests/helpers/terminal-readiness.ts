// Readiness boundaries for one terminal attachment. These helpers observe the
// client-side facts a test needs without manufacturing input, focus, or replay.

import { expect, type Page } from "@playwright/test";
import { recordPage } from "./timeline";

/** One island's catch-up record, as terminal.js publishes it. */
export interface ReplayRecord {
  buffering: boolean;
  bufferedBytes: number;
  bufferedChunks: number;
  writesWhileHidden: number;
  revealReason: string | null;
  revealed: boolean;
  revealedInWriteCallback: boolean;
  viewportAtTailOnReveal: boolean | null;
  holdMarker: boolean;
  heldReason: string | null;
  limits: { bufferBytes: number; bufferChunks: number; idleMs: number };
}

/** One island's catch-up record right now, without waiting for anything. */
export async function replayRecord(page: Page, elementId: string): Promise<ReplayRecord> {
  return page.evaluate(
    (el) => (window as any).__farhelmIslands[el].test.replay,
    elementId,
  );
}

/**
 * Wait until this island has REVEALED, then return its catch-up record.
 *
 * This is intentionally narrower than session readiness. It does not prove
 * that a socket belongs to a requested session or that its terminal took
 * focus, so held-replay tests can use it without releasing their own fixture
 * control. `revealed`, rather than `revealReason`, is the witness because the
 * reason is recorded one asynchronous write completion before the reveal.
 */
export async function waitForReplayReveal(
  page: Page,
  elementId: string,
  timeout = 60_000,
): Promise<ReplayRecord> {
  await expect
    .poll(
      () =>
        page.evaluate(
          (el) => !!(window as any).__farhelmIslands?.[el]?.test?.replay?.revealed,
          elementId,
        ),
      { timeout, message: `waiting for ${elementId} to reveal` },
    )
    .toBe(true);
  return replayRecord(page, elementId);
}

/**
 * Wait until the island at `elementId` is MOUNTED — an xterm instance and a
 * socket exist for it.
 *
 * The registry's terminal, socket, and test record together are the mount
 * boundary. It does not require OPEN, replay reveal, or focus, which is why
 * fixtures that hold replay or deliberately refuse a socket use this weaker
 * boundary.
 */
export async function waitForIslandMounted(page: Page, elementId: string) {
  await expect
    .poll(
      () =>
        page.evaluate((el) => {
          const island = (window as any).__farhelmIslands?.[el];
          return !!(island?.term && island?.ws && island?.test);
        }, elementId),
      { timeout: 20_000, message: `waiting for the island at ${elementId} to mount` },
    )
    .toBe(true);
}

/**
 * The identity an attachment wait must prove independently of its registry key.
 *
 * A primary `terminal` id survives session changes, so it is never enough to
 * identify the session whose socket and replay a caller asked to observe.
 */
interface SessionTerminal {
  id: string;
  tabId?: string;
}

/** A bounded, serializable snapshot of the attachment facts readiness needs. */
interface SessionObservation {
  pathname: string;
  pathnameMatches: boolean;
  islandMounted: boolean;
  socketOpen: boolean;
  socketMatches: boolean;
  replayRevealed: boolean;
  focused: boolean;
}

type SessionWaitKind = "mounted" | "socket-open" | "revealed" | "ready";

/** Options for a requested agent terminal or one of its terminal tabs. */
export interface SessionReadinessOptions {
  /** The terminal tab's id. Omit this for the session's primary agent pane. */
  tabId?: string;
  /** The whole host-driven polling budget, rather than an evaluation argument. */
  timeout?: number;
}

/**
 * Read the requested attachment's state from terminal.js in one browser turn.
 *
 * Element ids are only registry keys and can be reused after navigation or
 * reconnect. The socket's path and `tab` selector therefore establish which
 * session that island actually attaches before its state can satisfy a wait.
 */
async function observeSession(
  page: Page,
  terminal: SessionTerminal,
): Promise<SessionObservation> {
  return page.evaluate(({ id, tabId }) => {
    const elementId = tabId === undefined ? "terminal" : `terminal-${tabId}`;
    const island = (window as any).__farhelmIslands?.[elementId];
    const pathnameLimit = 240;
    let pathname = "(no socket)";
    let pathnameMatches = false;
    let socketMatches = false;

    if (island?.ws?.url) {
      try {
        const url = new URL(island.ws.url, location.href);
        pathname = url.pathname.slice(0, pathnameLimit);
        const parts = url.pathname.split("/");
        const encodedSession = parts[3];
        let sessionMatches = false;
        try {
          sessionMatches = decodeURIComponent(encodedSession ?? "") === id;
        } catch {
          sessionMatches = false;
        }
        pathnameMatches =
          sessionMatches &&
          ((parts.length === 5 &&
            parts[1] === "api" &&
            parts[2] === "sessions" &&
            parts[4] === "term") ||
            (parts.length === 6 &&
              parts[1] === "api" &&
              parts[2] === "sessions" &&
              parts[4] === "term" &&
              parts[5] === "unowned"));
        const tabs = url.searchParams.getAll("tab");
        const tabMatches =
          tabId === undefined ? tabs.length === 0 : tabs.length === 1 && tabs[0] === tabId;
        socketMatches = pathnameMatches && tabMatches;
      } catch {
        pathname = "(invalid socket URL)";
      }
    }

    // The public xterm textarea is the actual keyboard-input destination.
    // Being somewhere inside a terminal container does not prove that keys
    // reach xterm; another focusable descendant could own them instead.
    return {
      pathname,
      pathnameMatches,
      islandMounted: !!(island?.term && island?.ws && island?.test),
      socketOpen: island?.ws?.readyState === WebSocket.OPEN,
      socketMatches,
      replayRevealed: !!island?.test?.replay?.revealed,
      focused: document.activeElement === island?.term?.textarea,
    };
  }, terminal);
}

/** Render a timeout-safe account of the last composite readiness observation. */
function describeObservation(observation: SessionObservation): string {
  return [
    `path=${observation.pathname}`,
    `pathMatches=${observation.pathnameMatches}`,
    `mounted=${observation.islandMounted}`,
    `open=${observation.socketOpen}`,
    `socketMatches=${observation.socketMatches}`,
    `revealed=${observation.replayRevealed}`,
    `focused=${observation.focused}`,
  ].join(", ");
}

/**
 * Poll one requested attachment from the host until `accepts` holds.
 *
 * `expect.poll` schedules outside the page, so a test's fake browser clock
 * cannot freeze the readiness oracle. Capturing the last primitive snapshot
 * keeps a timeout actionable without serializing a DOM or terminal object.
 */
async function waitForSession(
  page: Page,
  terminal: SessionTerminal,
  timeout: number,
  kind: SessionWaitKind,
  accepts: (observation: SessionObservation) => boolean,
): Promise<void> {
  recordPage(page, "readiness-begin", [["requested", terminal.id], ["kind", kind]]);
  let last: SessionObservation = {
    pathname: "(not observed)",
    pathnameMatches: false,
    islandMounted: false,
    socketOpen: false,
    socketMatches: false,
    replayRevealed: false,
    focused: false,
  };
  const requested = `session ${terminal.id}${terminal.tabId === undefined ? " agent" : ` tab ${terminal.tabId}`}`;
  try {
    await expect
      .poll(
        async () => {
          const next = await observeSession(page, terminal);
          if (!sameObservation(next, last)) {
            recordPage(page, "readiness-transition", [
              ["requested", terminal.id],
              ["kind", kind],
              ["mounted", next.islandMounted],
              ["socket_open", next.socketOpen],
              ["revealed", next.replayRevealed],
              ["focused", next.focused],
              ["tab", terminal.tabId ?? ""],
              ["socket_matches", next.socketMatches],
              ["path_matches", next.pathnameMatches],
            ]);
          }
          last = next;
          return accepts(last);
        },
        { timeout, message: `waiting for ${requested} to become ${kind}` },
      )
      .toBe(true);
  } catch (cause) {
    recordPage(page, "readiness-failure", [["requested", terminal.id], ["kind", kind]]);
    throw new Error(
      `waiting for ${requested} to become ${kind} failed with a ${timeout}ms budget; last observation: ${describeObservation(last)}`,
      { cause },
    );
  }
  recordPage(page, "readiness-success", [["requested", terminal.id], ["kind", kind]]);
}

/** Compare the fixed readiness snapshot without serializing browser-derived data for diagnostics. */
function sameObservation(left: SessionObservation, right: SessionObservation): boolean {
  return left.pathname === right.pathname &&
    left.pathnameMatches === right.pathnameMatches &&
    left.islandMounted === right.islandMounted &&
    left.socketOpen === right.socketOpen &&
    left.socketMatches === right.socketMatches &&
    left.replayRevealed === right.replayRevealed &&
    left.focused === right.focused;
}

/**
 * Wait for the requested attachment's mount without requiring its socket to open.
 *
 * Held-replay and never-connected tests need an independently identified island
 * before inspecting their own boundary. A reused DOM id alone can still name
 * the previous session while reconciliation is pending. This wait checks the
 * socket URL but leaves connection, replay and focus state to the caller.
 */
export async function waitForSessionMounted(
  page: Page,
  id: string,
  { tabId, timeout = 20_000 }: SessionReadinessOptions = {},
): Promise<void> {
  await waitForSession(
    page,
    { id, tabId },
    timeout,
    "mounted",
    (observation) => observation.islandMounted && observation.socketMatches,
  );
}

/**
 * Wait until the requested session attachment has an open, revealed socket.
 *
 * This does not require focus. An inactive tab can complete replay safely,
 * and callers that only need that replay must not steal or wait on focus.
 */
export async function waitForSessionRevealed(
  page: Page,
  id: string,
  { tabId, timeout = 20_000 }: SessionReadinessOptions = {},
): Promise<void> {
  await waitForSession(
    page,
    { id, tabId },
    timeout,
    "revealed",
    (observation) =>
      observation.islandMounted &&
      observation.socketMatches &&
      observation.socketOpen &&
      observation.replayRevealed,
  );
}

/**
 * Wait until the requested attachment is mounted and its matching socket opens.
 *
 * This observes the requested socket open without awaiting replay or focus.
 * It still rejects a reused element id whose socket belongs to another
 * session or tab; OPEN alone does not prove supervisor attachment.
 */
export async function waitForSessionSocketOpen(
  page: Page,
  id: string,
  { tabId, timeout = 20_000 }: SessionReadinessOptions = {},
): Promise<void> {
  await waitForSession(
    page,
    { id, tabId },
    timeout,
    "socket-open",
    (observation) => observation.islandMounted && observation.socketMatches && observation.socketOpen,
  );
}

/**
 * Wait until the requested attachment is revealed and owns its terminal focus.
 *
 * This is an observation only. It never clicks or focuses an element, because
 * doing so would hide the product's focus-protection behavior from callers.
 */
export async function waitForSessionReady(
  page: Page,
  id: string,
  { tabId, timeout = 20_000 }: SessionReadinessOptions = {},
): Promise<void> {
  await waitForSession(
    page,
    { id, tabId },
    timeout,
    "ready",
    (observation) =>
      observation.islandMounted &&
      observation.socketMatches &&
      observation.socketOpen &&
      observation.replayRevealed &&
      observation.focused,
  );
}
