import { expect, test as base } from "@playwright/test";
import type {
  Browser,
  BrowserContext,
  BrowserContextOptions,
  ConsoleMessage,
  Frame,
  Page,
  WebSocket,
} from "@playwright/test";
import path from "node:path";
import {
  bounded,
  BrowserTimeline,
  registerObservedPage,
} from "./timeline";

export { expect };

type EvidenceFixtures = { timeline: BrowserTimeline };

/**
 * Preserves a bounded browser timeline when the final result is unexpected.
 *
 * The automatic collector has no browser dependency. The context wrapper is
 * separate so request-only tests keep their existing resource profile, while
 * the built-in page fixture still receives an instrumented default context.
 * A forcibly killed Playwright worker cannot run fixture teardown, so its
 * enclosing test-run recorder remains the only evidence source in that case.
 */
export const test = base.extend<EvidenceFixtures>({
  timeline: [async ({}, use, testInfo) => {
    const timeline = new BrowserTimeline();
    await use(timeline);

    try {
      timeline.close({
        testTitle: testInfo.title,
        testFile: path.relative(testInfo.project.testDir, testInfo.file),
        project: testInfo.project.name,
        retry: testInfo.retry,
        repeatEachIndex: testInfo.repeatEachIndex,
      });
    } catch {
      process.stderr.write("browser timeline finalization failed\n");
      return;
    }
    if (testInfo.status !== testInfo.expectedStatus) {
      try {
        await testInfo.attach("browser-timeline.jsonl", {
          body: timeline.toJSONL(),
          contentType: "application/x-ndjson",
        });
      } catch {
        // The timeline is already frozen, so stderr is the only remaining
        // bounded channel that cannot rewrite the result being diagnosed.
        process.stderr.write("browser timeline attachment failed\n");
      }
    }
  }, { auto: true }],
  context: async ({ context, timeline }, use) => {
    await observeContext(context, timeline);
    await use(context);
  },
});

/** Create a caller-owned context with exactly its original options, then observe it. */
export async function newObservedContext(
  browser: Browser,
  timeline: BrowserTimeline,
  options?: BrowserContextOptions,
): Promise<BrowserContext> {
  const context = await browser.newContext(options);
  await observeContext(context, timeline);
  return context;
}

/**
 * Install passive document, page, and socket observation on one context.
 *
 * A listener group is reserved before any callback in that group is attached.
 * Refused or failed registration leaves the product-facing context untouched
 * apart from a fixed incomplete-evidence diagnostic.
 */
export async function observeContext(
  context: BrowserContext,
  timeline: BrowserTimeline,
): Promise<void> {
  let initReady = false;
  let removed = false;
  const seen = new WeakSet<Page>();
  const pageRemovals = new Set<() => void>();
  const onPage = (page: Page) => {
    if (removed || seen.has(page)) return;
    seen.add(page);
    try {
      if (!initReady) timeline.observationLost("existing-page-document");
      const removePage = observePage(page, timeline);
      if (removePage) pageRemovals.add(removePage);
    } catch {
      timeline.diagnostic("listener");
    }
  };
  const onClose = () => {
    try {
      timeline.record("context-close");
      removeOwnedGroup(remove, timeline);
    } catch {
      timeline.diagnostic("listener");
    }
  };
  const remove = () => {
    if (removed) return;
    removed = true;
    removeAll(timeline, [
      () => context.off("page", onPage),
      () => context.off("close", onClose),
    ]);
    for (const removePage of pageRemovals) removeOwnedGroup(removePage, timeline);
    pageRemovals.clear();
  };

  if (!timeline.reserveListenerGroup(remove)) return;
  try {
    // Lifecycle callbacks must cover the await itself. Existing or concurrently
    // created documents may predate the init script, so their evidence is partial.
    context.on("page", onPage);
    context.on("close", onClose);
    for (const page of context.pages()) onPage(page);
    await context.addInitScript({ content: documentObserverSource });
    if (removed) return;
    // Reconcile before declaring installation complete: pages found here may
    // have been created before the browser acknowledged the script.
    for (const page of context.pages()) onPage(page);
    initReady = true;
  } catch {
    removeListenerGroup(remove, timeline, "setup");
    return;
  }
}

/** Register only the listeners this helper owns on one page. */
function observePage(page: Page, timeline: BrowserTimeline): (() => void) | undefined {
  const pageId = timeline.nextPageId();
  if (pageId === undefined) return;
  let startedDocument: string | undefined;
  const socketRemovals = new Set<() => void>();
  let removed = false;

  const onConsole = (message: ConsoleMessage) => {
    if (removed) return;
    try {
      const text = message.text();
      if (!text.startsWith(timelineConsolePrefix)) return;
      if (utf8Exceeds(text, BrowserTimeline.maxRecordBytes)) {
        timeline.observationLost("oversized-browser-record");
        timeline.diagnostic("listener");
        return;
      }

      const parsed: unknown = JSON.parse(text.slice(timelineConsolePrefix.length));
      if (!acceptedBrowserEvent(parsed)) {
        timeline.observationLost("invalid-browser-record");
        timeline.diagnostic("listener");
        return;
      }

      if (parsed.event === "document-start") {
        startedDocument = parsed.document;
      }
      // The document supplies its identity on every record. Same-document
      // navigation and delayed console delivery cannot relabel it as a new page.
      const attributedDocument = `document-${parsed.document}`;
      const documentKnown = startedDocument === parsed.document;
      if (!documentKnown) timeline.observationLost("missing-document-start");

      if (parsed.event === "exhausted") {
        timeline.observerExhausted(pageId, attributedDocument);
        return;
      }
      timeline.noteFieldLoss(parsed.fieldsClipped ?? 0, parsed.fieldsOmitted ?? 0);
      const fields: Array<[string, unknown]> = [
        ["page", pageId],
        ["document", attributedDocument],
        ["document_known", documentKnown],
        ["document_sequence", parsed.sequence],
        ["browser_ms", parsed.timestamp],
        ["event", parsed.event],
      ];
      if (parsed.tag !== undefined) fields.push(["tag", parsed.tag]);
      if (parsed.id !== undefined) fields.push(["id", parsed.id]);
      if (parsed.classes !== undefined) fields.push(["classes", parsed.classes]);
      if (parsed.inputType !== undefined) fields.push(["input_type", parsed.inputType]);
      if (parsed.fieldsClipped !== undefined) fields.push(["browser_fields_clipped", parsed.fieldsClipped]);
      if (parsed.fieldsOmitted !== undefined) fields.push(["browser_fields_omitted", parsed.fieldsOmitted]);
      timeline.record("document-event", fields);
    } catch {
      timeline.observationLost("malformed-browser-record");
      timeline.diagnostic("listener");
    }
  };
  const onSocket = (socket: WebSocket) => {
    if (removed) return;
    try {
      const removeSocket = observeSocket(socket, pageId, timeline);
      if (removeSocket) socketRemovals.add(removeSocket);
    } catch {
      timeline.diagnostic("listener");
    }
  };
  const onFrameNavigated = (frame: Frame) => {
    if (removed) return;
    try {
      if (frame !== page.mainFrame()) return;
      timeline.record("main-document-navigation", [["page", pageId]]);
    } catch {
      timeline.diagnostic("listener");
    }
  };
  const onClose = () => {
    if (removed) return;
    try {
      timeline.record("page-close", [["page", pageId]]);
      removeOwnedGroup(remove, timeline);
    } catch {
      timeline.diagnostic("listener");
    }
  };
  const remove = () => {
    if (removed) return;
    removed = true;
    removeAll(timeline, [
      () => page.off("console", onConsole),
      () => page.off("websocket", onSocket),
      () => page.off("framenavigated", onFrameNavigated),
      () => page.off("close", onClose),
    ]);
    // Socket listeners belong to the page even when context initialization
    // fails before that page closes. The timeline's group cap also bounds this set.
    for (const removeSocket of socketRemovals) removeOwnedGroup(removeSocket, timeline);
    socketRemovals.clear();
  };

  if (!timeline.reserveListenerGroup(remove)) return;
  try {
    page.on("console", onConsole);
    page.on("websocket", onSocket);
    page.on("framenavigated", onFrameNavigated);
    page.on("close", onClose);
    registerObservedPage(page, timeline, pageId);
    timeline.record("page-observed", [["page", pageId]]);
    return remove;
  } catch {
    removeListenerGroup(remove, timeline, "listener");
  }
}

/** Observe native socket lifecycle without wrapping the page's constructor. */
function observeSocket(socket: WebSocket, pageId: string, timeline: BrowserTimeline): (() => void) | undefined {
  const socketId = timeline.nextSocketId();
  if (socketId === undefined) return;
  const safeUrl = safeSocketFields(socket.url());
  timeline.noteFieldLoss(safeUrl.fieldsClipped, 0);
  if (!safeUrl.valid) {
    timeline.observationLost(safeUrl.oversized ? "oversized-socket-url" : "invalid-socket-url");
  }
  if (safeUrl.tab === undefined) {
    timeline.record("socket-observed", [
      ["page", pageId],
      ["socket", socketId],
      ["pathname", safeUrl.pathname],
      ["url_valid", safeUrl.valid],
    ]);
  } else {
    timeline.record("socket-observed", [
      ["page", pageId],
      ["socket", socketId],
      ["pathname", safeUrl.pathname],
      ["tab", safeUrl.tab],
      ["url_valid", safeUrl.valid],
    ]);
  }

  let removed = false;
  const onClose = () => {
    if (removed) return;
    try {
      timeline.record("socket-close", [["page", pageId], ["socket", socketId]]);
      removeOwnedGroup(remove, timeline);
    } catch {
      timeline.diagnostic("listener");
    }
  };
  const onError = () => {
    if (removed) return;
    try {
      timeline.record("socket-error", [["page", pageId], ["socket", socketId]]);
    } catch {
      timeline.diagnostic("listener");
    }
  };
  const remove = () => {
    if (removed) return;
    removed = true;
    removeAll(timeline, [
      () => socket.off("close", onClose),
      () => socket.off("socketerror", onError),
    ]);
  };

  if (!timeline.reserveListenerGroup(remove)) return;
  try {
    socket.on("close", onClose);
    socket.on("socketerror", onError);
    return remove;
  } catch {
    removeListenerGroup(remove, timeline, "listener");
  }
}

/** Roll back a partially installed group while preserving the original result. */
function removeListenerGroup(
  remove: () => void,
  timeline: BrowserTimeline,
  category: "setup" | "listener",
): void {
  try {
    remove();
  } catch {
    timeline.diagnostic("cleanup");
  }
  timeline.diagnostic(category);
}

/** Remove a resource-closed group without letting emitter cleanup escape. */
function removeOwnedGroup(remove: () => void, timeline: BrowserTimeline): void {
  try {
    remove();
  } catch {
    timeline.diagnostic("cleanup");
  }
}

/** Attempt every owned listener removal even when one emitter operation fails. */
function removeAll(timeline: BrowserTimeline, removals: Array<() => void>): void {
  for (const remove of removals) {
    try {
      remove();
    } catch {
      timeline.diagnostic("cleanup");
    }
  }
}

interface BrowserEventRecord {
  document: string;
  event: "document-start" | "focusin" | "focusout" | "beforeinput" | "input" | "exhausted";
  sequence: number;
  timestamp: number;
  tag?: string;
  id?: string;
  classes?: string;
  inputType?: string;
  fieldsClipped?: number;
  fieldsOmitted?: number;
}

/** Accept only the browser schema whose fields have already passed byte bounds. */
export function acceptedBrowserEvent(value: unknown): value is BrowserEventRecord {
  if (!isPlainRecord(value)) return false;
  const allowedKeys = new Set([
    "document",
    "event",
    "sequence",
    "timestamp",
    "tag",
    "id",
    "classes",
    "inputType",
    "fieldsClipped",
    "fieldsOmitted",
  ]);
  const keys = Object.keys(value);
  if (keys.length > allowedKeys.size || keys.some((key) => !allowedKeys.has(key))) return false;
  if (typeof value.document !== "string" || !/^[0-9a-f]{32}$/.test(value.document)) return false;
  if (
    typeof value.event !== "string" ||
    !["document-start", "focusin", "focusout", "beforeinput", "input", "exhausted"].includes(value.event)
  ) return false;
  if (
    typeof value.sequence !== "number" ||
    !Number.isFinite(value.sequence) ||
    typeof value.timestamp !== "number" ||
    !Number.isFinite(value.timestamp)
  ) return false;
  for (const key of ["tag", "id", "classes", "inputType"] as const) {
    const field = value[key];
    if (
      field !== undefined &&
      (typeof field !== "string" ||
        Buffer.byteLength(field, "utf8") > BrowserTimeline.maxScalarBytes)
    ) {
      return false;
    }
  }
  for (const key of ["fieldsClipped", "fieldsOmitted"] as const) {
    const count = value[key];
    if (
      count !== undefined &&
      (typeof count !== "number" || !Number.isInteger(count) || count < 0 || count > 4)
    ) return false;
  }
  return true;
}

/** Reject arrays before bounded schema inspection treats numeric keys as fields. */
function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** The entire socket URL shape permitted to cross the evidence privacy boundary. */
export interface SafeSocketFields {
  pathname: string;
  tab?: string;
  valid: boolean;
  oversized: boolean;
  fieldsClipped: number;
}

/** Keep only bounded socket routing identity; credentials never leave the parser. */
export function safeSocketFields(raw: string): SafeSocketFields {
  if (utf8Exceeds(raw, 4096)) {
    return { pathname: "", valid: false, oversized: true, fieldsClipped: 0 };
  }
  try {
    const url = new URL(raw);
    const tab = url.searchParams.get("tab");
    const pathname = bounded(url.pathname);
    const safeTab = tab === null ? undefined : bounded(tab);
    return {
      pathname,
      tab: safeTab,
      valid: true,
      oversized: false,
      fieldsClipped: Number(pathname !== url.pathname) + Number(tab !== null && safeTab !== tab),
    };
  } catch {
    return { pathname: "", valid: false, oversized: false, fieldsClipped: 0 };
  }
}

/** Detect an oversized URL without first allocating an encoded copy of it. */
function utf8Exceeds(value: string, limit: number): boolean {
  let bytes = 0;
  for (const character of value) {
    const point = character.codePointAt(0)!;
    bytes += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
    if (bytes > limit) return true;
  }
  return false;
}

export const timelineConsolePrefix = "__farhelm_timeline__";

/*
 * This script deliberately has no captures other than fixed source literals:
 * Playwright installs it in every future main document, including after
 * navigation and under fake clocks. It never changes focus, input, sockets,
 * storage, constructors, or time.
 */
const documentObserverSource = `(() => {
  if (window.top !== window) return;
  const prefix = ${JSON.stringify(timelineConsolePrefix)};
  const maxScalarBytes = 128;
  const maxRecordBytes = 2048;
  // Random document identity is independent of fake clocks and survives history
  // changes without retaining a host-side map of every document ever visited.
  const documentId = Array.from(crypto.getRandomValues(new Uint32Array(4)),
    (value) => value.toString(16).padStart(8, "0")).join("");
  let sequence = 0;
  let remaining = 128;
  let exhausted = false;

  const utf8Width = (character) => {
    const point = character.codePointAt(0);
    if (point <= 0x7f) return 1;
    if (point <= 0x7ff) return 2;
    if (point <= 0xffff) return 3;
    return 4;
  };
  const clip = (value, limit = maxScalarBytes) => {
    if (typeof value !== "string") return { value: "", clipped: false };
    let output = "";
    let bytes = 0;
    let clipped = false;
    for (const character of value) {
      const width = utf8Width(character);
      if (bytes + width > limit) {
        clipped = true;
        break;
      }
      output += character;
      bytes += width;
    }
    return { value: output, clipped };
  };
  const encodedBytes = (value) => new TextEncoder().encode(value).byteLength;
  const fit = (record, alreadyClipped = []) => {
    let text = JSON.stringify(record);
    const shrinkOrder = ["classes", "id", "inputType", "tag"];
    const shrunk = new Set(alreadyClipped);
    for (const field of shrinkOrder) {
      while (encodedBytes(prefix + text) > maxRecordBytes && record[field]) {
        record[field] = clip(
          record[field],
          Math.max(0, encodedBytes(record[field]) - 8),
        ).value;
        if (!shrunk.has(field)) {
          shrunk.add(field);
          record.fieldsClipped = Math.min(4, record.fieldsClipped + 1);
        }
        text = JSON.stringify(record);
      }
    }
    return encodedBytes(prefix + text) <= maxRecordBytes ? text : undefined;
  };
  const write = (record, alreadyClipped) => {
    record.document = documentId;
    const text = fit(record, alreadyClipped);
    if (text !== undefined) console.log(prefix + text);
  };
  const classNames = (target) => {
    if (!target) return { value: "", clipped: false, omitted: false };
    let result = "";
    let clipped = false;
    const count = Math.min(target.classList.length, 4);
    for (let index = 0; index < count; index += 1) {
      const token = clip(target.classList.item(index) || "");
      clipped ||= token.clipped;
      const joined = clip(result ? result + " " + token.value : token.value);
      result = joined.value;
      clipped ||= joined.clipped;
    }
    return { value: result, clipped, omitted: target.classList.length > count };
  };
  const exhaustAfterError = () => {
    remaining = 0;
    if (exhausted) return;
    exhausted = true;
    try {
      write({ event: "exhausted", sequence: ++sequence, timestamp: performance.now() });
    } catch {}
  };
  const emit = (event) => {
    try {
      if (remaining <= 0) {
        if (!exhausted) {
          exhausted = true;
          write({ event: "exhausted", sequence: ++sequence, timestamp: performance.now() });
        }
        return;
      }
      remaining -= 1;
      const target = event.target instanceof Element ? event.target : null;
      const tag = clip(target ? target.tagName : "");
      const id = clip(target ? target.id : "");
      const classes = classNames(target);
      const inputType = clip(typeof event.inputType === "string" ? event.inputType : "");
      const clippedFields = [];
      if (tag.clipped) clippedFields.push("tag");
      if (id.clipped) clippedFields.push("id");
      if (classes.clipped) clippedFields.push("classes");
      if (inputType.clipped) clippedFields.push("inputType");
      write({
        event: event.type,
        sequence: ++sequence,
        timestamp: performance.now(),
        tag: tag.value,
        id: id.value,
        classes: classes.value,
        inputType: inputType.value,
        fieldsClipped: clippedFields.length,
        fieldsOmitted: classes.omitted ? 1 : 0,
      }, clippedFields);
    } catch {
      exhaustAfterError();
    }
  };

  try {
    remaining -= 1;
    write({ event: "document-start", sequence: ++sequence, timestamp: performance.now() });
  } catch {
    exhaustAfterError();
  }
  try {
    for (const name of ["focusin", "focusout", "beforeinput", "input"]) {
      addEventListener(name, emit, true);
    }
  } catch {
    exhaustAfterError();
  }
})()`;
