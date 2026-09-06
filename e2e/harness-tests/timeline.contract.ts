import { expect, test } from "../tests/helpers/evidence";
import {
  newObservedContext,
  observeContext,
  safeSocketFields,
  timelineConsolePrefix,
} from "../tests/helpers/evidence";
import { stubFeed } from "../tests/helpers/fleet";
import {
  BrowserTimeline,
  recordPage,
  registerObservedPage,
} from "../tests/helpers/timeline";
import { waitForSessionSocketOpen } from "../tests/helpers/terminal-readiness";
import { chromium, webkit } from "@playwright/test";
import type { Browser, BrowserContext, Page } from "@playwright/test";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import { closeSync, fstatSync } from "node:fs";
import { mkdir, symlink, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import type { Socket } from "node:net";
import { Readable } from "node:stream";
import { collectConsolePayloads, inspectTraceZip, openTraceZip, verifyIntentionalChild } from "./child-runner";

interface TimelineRecord {
  kind: string;
  receipt?: number;
  fields?: Record<string, unknown>;
  identity?: Record<string, unknown>;
  observed?: number;
  retained?: number;
  artifact_records?: number;
  loss?: Record<string, unknown>;
  incomplete?: boolean;
}

/** Parse only a finalized artifact, exactly as a failure investigator receives it. */
function records(timeline: BrowserTimeline): TimelineRecord[] {
  return timeline.toJSONL().toString("utf8").trimEnd().split("\n")
    .map((line) => JSON.parse(line) as TimelineRecord);
}

/** Freeze an isolated collector with representative allowlisted identity. */
function finish(timeline: BrowserTimeline): TimelineRecord[] {
  timeline.close({
    testTitle: "contract",
    testFile: "timeline.contract.ts",
    project: "standalone",
    retry: 0,
    repeatEachIndex: 0,
  });
  return records(timeline);
}

/** Select retained event records without confusing header/footer metadata. */
function events(timeline: BrowserTimeline, kind?: string): TimelineRecord[] {
  const all = records(timeline).filter((record) => record.receipt !== undefined);
  return kind === undefined ? all : all.filter((record) => record.kind === kind);
}

/**
 * Escaped strings consume encoded bytes much faster than source bytes. This
 * contract keeps all three storage limits honest while proving that neither
 * the early setup record nor the final tail disappears under pressure.
 */
test("collector enforces escaped UTF-8, record, artifact, and retention bounds", () => {
  const timeline = new BrowserTimeline();
  timeline.record("setup", [["value", "first"]]);
  timeline.record("utf8", [["value", `${"é".repeat(64)}tail`]]);
  const escaped = `"${"\u0000".repeat(128)}`;
  for (let index = 0; index < 3_000; index += 1) {
    timeline.record("tail", [
      ["index", index],
      ["escaped", escaped],
      ["unicode", `${"é".repeat(100)}${"😀".repeat(100)}`],
    ]);
  }
  timeline.close({
    testTitle: escaped,
    testFile: escaped,
    project: escaped,
    retry: 0,
    repeatEachIndex: 0,
  });

  const output = timeline.toJSONL();
  const lines = output.toString("utf8").trimEnd().split("\n");
  const parsed = records(timeline);
  const summary = parsed.at(-1)!;
  expect(output.byteLength).toBeLessThanOrEqual(BrowserTimeline.maxBytes);
  expect(lines).toHaveLength(parsed.length);
  expect(lines.length).toBeLessThanOrEqual(BrowserTimeline.maxRecords);
  expect(
    lines.every((line) =>
      Buffer.byteLength(line, "utf8") + 1 <= BrowserTimeline.maxRecordBytes
    ),
  ).toBe(true);
  expect(
    Buffer.byteLength(lines[0], "utf8") +
      Buffer.byteLength(lines.at(-1)!, "utf8") +
      2,
  ).toBeLessThanOrEqual(4096);
  expect(parsed[1]).toMatchObject({ kind: "setup", fields: { value: "first" } });
  expect(parsed[2]).toMatchObject({ kind: "utf8", fields: { value: "é".repeat(64) } });
  expect(parsed.find((record) => record.fields?.escaped)?.fields?.escaped).toBe(escaped.slice(0, 128));
  expect(parsed.at(-2)).toMatchObject({ kind: "tail", fields: { index: 2999 } });
  expect(parsed[0].identity).toMatchObject({ retry: 0, repeat: 0 });
  expect(Object.keys(parsed[0].identity ?? {}).sort()).toEqual([
    "file", "project", "repeat", "retry", "test",
  ]);
  expect(Object.values(parsed[0].identity ?? {}).filter((value) => typeof value === "string"))
    .toHaveLength(3);
  const receipts = parsed.flatMap((record) => record.receipt === undefined ? [] : [record.receipt]);
  expect(receipts).toEqual([...receipts].sort((left, right) => left - right));
  expect(new Set(receipts).size).toBe(receipts.length);
  expect(summary.observed).toBe(3_002);
  expect(summary.retained).toBe(parsed.length - 2);
  expect(summary.artifact_records).toBe(parsed.length);
  expect(summary.incomplete).toBe(true);
  expect(Number(summary.loss?.events)).toBeGreaterThan(0);
  expect(Number(summary.loss?.fieldsClipped)).toBeGreaterThan(0);
});

/**
 * Every loss source changes a distinct summary field, while helper data named
 * like record metadata remains nested and cannot corrupt receipt identity.
 */
test("collector separates every loss class and protects reserved record fields", () => {
  const timeline = new BrowserTimeline();
  const hostile = new Proxy<Array<[string, unknown]>>([], {
    get() {
      throw new Error("must become bounded loss");
    },
  });
  expect(() => timeline.record("hostile-fields", hostile)).not.toThrow();
  const fields: Array<[string, unknown]> = [
    ["kind", "readiness-boundary"],
    ["receipt", "event-data"],
    ["receipt_ns", "event-data"],
    ["clipped", "😀".repeat(100)],
    ["invalid", { secret: "must-not-serialize" }],
    ["infinite", Number.POSITIVE_INFINITY],
    ["undefinedValue", undefined],
  ];
  for (let index = 0; index < 20; index += 1) fields.push([`field-${index}`, index]);
  timeline.record("readiness-transition", fields);
  timeline.observerExhausted("page-1", "document-1");
  timeline.diagnostic("listener");
  for (let index = 0; index < BrowserTimeline.maxListenerGroups; index += 1) {
    const remove = index === 0 ? () => { throw new Error("fixed cleanup failure"); } : () => {};
    expect(timeline.reserveListenerGroup(remove)).toBe(true);
  }
  expect(timeline.reserveListenerGroup(() => {})).toBe(false);
  const parsed = finish(timeline);
  const transition = parsed.find((record) => record.kind === "readiness-transition");
  const summary = parsed.at(-1)!;

  expect(transition?.fields?.kind).toBe("readiness-boundary");
  expect(transition?.receipt).toEqual(expect.any(Number));
  expect(transition?.fields?.receipt).toBe("event-data");
  expect(Object.keys(transition?.fields ?? {}).length).toBeLessThanOrEqual(BrowserTimeline.maxFields);
  expect(JSON.stringify(parsed)).not.toContain("must-not-serialize");
  expect(Number(summary.loss?.fieldsClipped)).toBeGreaterThan(0);
  expect(Number(summary.loss?.fieldsOmitted)).toBeGreaterThan(0);
  expect(summary.loss?.fieldsOmittedExtentUnknown).toBe(true);
  expect(summary.loss?.listenerRefusals).toBe(1);
  expect(summary.loss?.observerExhaustions).toBe(1);
  expect(summary.loss?.diagnosticErrors).toBe(2);
  expect(summary.incomplete).toBe(true);
});

/** Socket parsing is the privacy boundary for lease and authentication data. */
test("socket evidence strips credentials and bounds hostile input", () => {
  expect(safeSocketFields("wss://user:pass@example.test/api/events?tab=chosen&lease=raw-secret#fragment"))
    .toEqual({
      pathname: "/api/events",
      tab: "chosen",
      valid: true,
      oversized: false,
      fieldsClipped: 0,
    });
  expect(safeSocketFields(
    `wss://example.test/${"é".repeat(100)}?tab=${"😀".repeat(100)}`,
  ).fieldsClipped).toBe(2);
  expect(JSON.stringify(safeSocketFields(`ws://example.test/${"x".repeat(5_000)}?tab=secret`)))
    .not.toContain("secret");
  expect(safeSocketFields("not a socket URL")).toEqual({
    pathname: "",
    valid: false,
    oversized: false,
    fieldsClipped: 0,
  });
});

/** A callback retained by a stale emitter must not mutate either adjacent test. */
test("late callbacks and page lookup cannot cross test ownership", () => {
  const page = {} as Parameters<typeof registerObservedPage>[0];
  const first = new BrowserTimeline();
  registerObservedPage(page, first, "page-1");
  recordPage(page, "first");
  const before = finish(first);
  const frozen = first.toJSONL();
  recordPage(page, "late");
  expect(records(first)).toEqual(before);
  expect(first.toJSONL().equals(frozen)).toBe(true);

  const second = new BrowserTimeline();
  registerObservedPage(page, second, "page-1");
  recordPage(page, "second");
  finish(second);
  expect(events(second).map((record) => record.kind)).toEqual(["second"]);
  expect(events(first).map((record) => record.kind)).toEqual(["first"]);
});

/**
 * Listener ownership is exact for installed groups and atomic at the cap: a
 * refused context gets no callbacks at all.
 */
test("bounded fake emitters lose all owned callbacks at freeze", async () => {
  const context = new FakeContext();
  const timeline = new BrowserTimeline();
  await observeContext(context as unknown as BrowserContext, timeline);
  expect(context.listenerCount()).toBe(2);
  const pageCallback = context.callback("page");
  timeline.close();
  expect(context.listenerCount()).toBe(0);
  pageCallback?.({} as never);
  expect(records(timeline).at(-1)?.observed).toBe(0);

  const refused = new BrowserTimeline();
  for (let index = 0; index < BrowserTimeline.maxListenerGroups; index += 1) {
    refused.reserveListenerGroup(() => {});
  }
  const untouched = new FakeContext();
  await observeContext(untouched as unknown as BrowserContext, refused);
  expect(untouched.listenerCount()).toBe(0);
  finish(refused);
  expect(records(refused).at(-1)?.loss?.listenerRefusals).toBe(1);
});

/** One broken off() call must not strand later listeners owned by the group. */
test("listener cleanup continues after an emitter removal failure", async () => {
  const context = new FakeContext({ throwOnOff: "page" });
  const timeline = new BrowserTimeline();
  await observeContext(context as unknown as BrowserContext, timeline);
  timeline.close();

  expect(context.removalAttempts).toEqual(["page", "close"]);
  expect(context.hasListener("page")).toBe(true);
  expect(context.hasListener("close")).toBe(false);
  expect(records(timeline).at(-1)?.loss?.diagnosticErrors).toBe(1);
});

/** Closing during async setup must prevent callbacks from attaching after freeze. */
test("pending context setup cannot attach listeners after timeline close", async () => {
  let resolveInit!: () => void;
  const init = new Promise<void>((resolve) => {
    resolveInit = resolve;
  });
  const context = new FakeContext({ addInitScript: init });
  const timeline = new BrowserTimeline();
  const observation = observeContext(context as unknown as BrowserContext, timeline);

  timeline.close();
  resolveInit();
  await observation;

  expect(context.listenerCount()).toBe(0);
  expect(records(timeline).at(-1)?.observed).toBe(0);
});

/** Context closure must be observed even before init-script acknowledgement. */
test("context close during pending setup removes listeners before resolution", async () => {
  let resolveInit!: () => void;
  const context = new FakeContext({ addInitScript: new Promise<void>((resolve) => {
    resolveInit = resolve;
  }) });
  const timeline = new BrowserTimeline();
  const observation = observeContext(context as unknown as BrowserContext, timeline);
  context.callback("close")?.();
  expect(context.listenerCount()).toBe(0);
  resolveInit();
  await observation;
  expect(context.listenerCount()).toBe(0);
  finish(timeline);
  expect(events(timeline, "context-close")).toHaveLength(1);
});

/**
 * A page can appear through both the event and the reconciliation snapshot.
 * It gets one listener group, and pre-init documents remain explicit loss.
 */
test("pages arriving during setup are observed once and rolled back on failure", async () => {
  for (const fail of [false, true]) {
    let resolveInit!: () => void;
    let rejectInit!: (error: Error) => void;
    const existing: Page[] = [];
    const context = new FakeContext({ pages: existing, addInitScript: new Promise<void>((resolve, reject) => {
      resolveInit = resolve;
      rejectInit = reject;
    }) });
    const page = new FakeContext();
    const timeline = new BrowserTimeline();
    const observation = observeContext(context as unknown as BrowserContext, timeline);
    existing.push(page as unknown as Page);
    context.callback("page")?.(page as never);
    expect(page.listenerCount()).toBe(4);
    const socket = Object.assign(new FakeContext(), { url: () => "ws://fixture.invalid/session" });
    const savedSocketCallback = page.callback("websocket");
    savedSocketCallback?.(socket as never);
    expect(socket.listenerCount()).toBe(2);
    if (fail) rejectInit(new Error("fixed setup rejection"));
    else resolveInit();
    await observation;
    if (fail) {
      expect(context.listenerCount()).toBe(0);
      expect(page.listenerCount()).toBe(0);
      expect(socket.listenerCount()).toBe(0);
      const lateSocket = Object.assign(new FakeContext(), { url: () => "ws://fixture.invalid/late" });
      savedSocketCallback?.(lateSocket as never);
      expect(lateSocket.listenerCount()).toBe(0);
    }
    finish(timeline);
    expect(page.listenerCount()).toBe(0);
    expect(socket.listenerCount()).toBe(0);
    expect(events(timeline, "page-observed")).toHaveLength(1);
    expect(events(timeline, "observation-incomplete")).toContainEqual(expect.objectContaining({
      fields: { reason: "existing-page-document" },
    }));
    expect(records(timeline).at(-1)?.incomplete).toBe(true);
  }
});

/** A large indexed source never triggers key enumeration or access beyond the admitted prefix. */
test("field admission reads only its indexed prefix and preserves the reserved page slot", () => {
  for (const withPage of [false, true]) {
    const timeline = new BrowserTimeline();
    const accessed: number[] = [];
    let enumerations = 0;
    let excessReads = 0;
    const cap = BrowserTimeline.maxFields - Number(withPage);
    const fields = new Proxy<Array<[string, unknown]>>(new Array(1_000_000), {
      ownKeys() {
        enumerations += 1;
        throw new Error("key enumeration is outside admission");
      },
      get(target, key) {
        if (key === "length") return target.length;
        const index = Number(key);
        accessed.push(index);
        if (index >= cap || !Number.isInteger(index)) {
          excessReads += 1;
          throw new Error("source prefix was exceeded");
        }
        return [index === 0 ? "page" : `field-${index}`, index === 0 ? "forged" : index];
      },
    });
    if (withPage) timeline.recordForPage("page-owned", "prefix", fields);
    else timeline.record("prefix", fields);
    const parsed = finish(timeline);
    expect(enumerations).toBe(0);
    expect(excessReads).toBe(0);
    expect(accessed).toEqual(Array.from({ length: cap }, (_, index) => index));
    expect(events(timeline, "prefix")[0].fields?.page).toBe(withPage ? "page-owned" : "forged");
    expect(parsed.at(-1)?.loss?.fieldsOmittedExtentUnknown).toBe(true);
  }
});

/** Overlapping session/feed identifiers still join to the page that emitted them. */
test("helper records preserve page ownership under overlapping caller identities", async ({ browser }) => {
  const timeline = new BrowserTimeline();
  const context = await newObservedContext(browser, timeline);
  try {
    const first = await context.newPage();
    const second = await context.newPage();
    const fields: Array<[string, unknown]> = [["page", "forged"], ["session", "same"], ["connection", 1]];
    for (let index = 0; index < 30; index += 1) fields.push([`extra-${index}`, "\u0000".repeat(128)]);
    recordPage(first, "feed-arrival", fields);
    recordPage(second, "feed-arrival", [["session", "same"], ["connection", 1]]);
    recordPage(first, "feed-close", [["session", "same"], ["connection", 1]]);
  } finally {
    await context.close();
  }
  finish(timeline);
  const pageIds = events(timeline, "page-observed").map((event) => event.fields?.page);
  expect(pageIds).toHaveLength(2);
  expect(new Set(pageIds).size).toBe(2);
  const arrivals = events(timeline, "feed-arrival");
  expect(arrivals.map((event) => event.fields?.page)).toEqual(pageIds);
  expect(events(timeline, "feed-close")[0].fields?.page).toBe(pageIds[0]);
  expect(Object.keys(arrivals[0].fields!).length).toBeLessThanOrEqual(BrowserTimeline.maxFields);
  expect(records(timeline).at(-1)?.incomplete).toBe(true);
});

/** A manually restarted persistent profile contributes both lifetimes to one test. */
test("persistent contexts retain observations before and after restart", async ({ browserName }, testInfo) => {
  const timeline = new BrowserTimeline();
  const browserType = browserName === "webkit" ? webkit : chromium;
  const profile = testInfo.outputPath("persistent-profile");
  for (const lifetime of ["first", "restarted"]) {
    const context = await browserType.launchPersistentContext(profile);
    try {
      await observeContext(context, timeline);
      const page = context.pages()[0] ?? await context.newPage();
      await page.goto(`data:text/html,%3Cinput%20id%3D${lifetime}%3E`);
      await page.locator(`#${lifetime}`).focus();
      recordPage(page, "persistent-lifetime", [["lifetime", lifetime]]);
    } finally {
      await context.close();
    }
  }
  finish(timeline);
  const lifetimes = events(timeline, "persistent-lifetime");
  expect(lifetimes.map((record) => record.fields?.lifetime)).toEqual(["first", "restarted"]);
  expect(new Set(lifetimes.map((record) => record.fields?.page)).size).toBe(2);
  for (const lifetime of lifetimes) {
    expect(events(timeline, "document-event")).toContainEqual(expect.objectContaining({
      fields: expect.objectContaining({ page: lifetime.fields?.page, event: "focusin", document_known: true }),
    }));
  }
  expect(events(timeline, "context-close")).toHaveLength(2);
  expect(records(timeline).at(-1)?.incomplete).toBe(true);
});

const requestOnly = test.extend<{}, { browser: Browser }>({
  browser: [async ({}, _use) => {
    throw new Error("request-only evidence acquired the browser fixture");
  }, { scope: "worker" }],
});

/** The automatic evidence fixture must remain usable when browser setup would fail. */
requestOnly("request-only evidence does not acquire a browser", async ({ request, timeline }) => {
  expect(request).toBeTruthy();
  timeline.record("request-only");
});

/**
 * Same-document receipt order answers the focus/input race. Document tokens
 * survive fragment/history changes and distinguish replacement under frozen time.
 */
test("passive input records preserve document order and redact text under frozen time", async ({
  browser,
}) => {
  const timeline = new BrowserTimeline();
  const context = await newObservedContext(browser, timeline);
  const page = await context.newPage();
  await page.clock.install({ time: new Date("2026-01-01T00:00:00Z") });
  await page.clock.pauseAt(new Date("2026-01-01T00:00:00Z"));
  await page.goto(
    "data:text/html,%3Cinput%20id%3Dfirst%20class%3D%22alpha%20beta%22%3E" +
      "%3Cbutton%20id%3Daway%3Eaway%3C%2Fbutton%3E",
  );
  await page.locator("#first").focus();
  await page.locator("#first").pressSequentially("raw-private-input");
  await page.evaluate(() => { location.hash = "fragment"; });
  await page.locator("#first").pressSequentially("x");
  await page.evaluate(() => { history.replaceState(null, "", "#history"); });
  await page.locator("#first").pressSequentially("y");
  await page.locator("#away").focus();
  await page.goto("data:text/html,%3Cinput%20id%3Dsecond%3E");
  await page.locator("#second").focus();
  await page.locator("#second").pressSequentially("second-private-input");
  await context.close();
  finish(timeline);

  const observed = events(timeline, "document-event");
  const inputEvents = observed.filter((record) => record.fields?.event === "input");
  expect(new Set(inputEvents.map((record) => record.fields?.document)).size).toBe(2);
  expect(inputEvents.every((record) => record.fields?.document_known === true)).toBe(true);
  for (const document of new Set(inputEvents.map((record) => record.fields?.document))) {
    const inDocument = observed.filter((record) => record.fields?.document === document);
    const focus = inDocument.findIndex((record) => record.fields?.event === "focusin");
    const input = inDocument.findIndex((record) => record.fields?.event === "input");
    expect(focus).toBeGreaterThanOrEqual(0);
    expect(input).toBeGreaterThan(focus);
    const sequences = inDocument.map((record) => Number(record.fields?.document_sequence));
    expect(sequences).toEqual([...sequences].sort((left, right) => left - right));
    expect(new Set(sequences).size).toBe(sequences.length);
  }
  expect(JSON.stringify(records(timeline))).not.toContain("raw-private-input");
  expect(JSON.stringify(records(timeline))).not.toContain("second-private-input");
});

/** Each document's 129th console marker makes later absence explicit incomplete evidence. */
test("browser exhaustion is explicit and every prefixed record stays bounded", async ({ browser }) => {
  const timeline = new BrowserTimeline();
  const context = await newObservedContext(browser, timeline);
  const page = await context.newPage();
  const consoleRecords: string[] = [];
  page.on("console", (message) => {
    if (message.text().startsWith("__farhelm_timeline__")) consoleRecords.push(message.text());
  });
  await page.goto("data:text/html,%3Cinput%20data-slot%3Done%3E%3Cinput%20data-slot%3Dtwo%3E");
  await page.evaluate(() => {
    const first = document.querySelector('[data-slot="one"]')!;
    first.id = "😀".repeat(100);
    for (let index = 0; index < 10; index += 1) first.classList.add(`class-${index}-${"x".repeat(80)}`);
  });
  for (let index = 0; index < 80; index += 1) {
    await page.locator(`[data-slot="${index % 2 === 0 ? "one" : "two"}"]`).focus();
  }
  await context.close();
  finish(timeline);
  const summary = records(timeline).at(-1)!;
  expect(summary.loss?.observerExhaustions).toBe(1);
  expect(Number(summary.loss?.fieldsClipped)).toBeGreaterThan(0);
  expect(Number(summary.loss?.fieldsOmitted)).toBeGreaterThan(0);
  expect(summary.incomplete).toBe(true);
  const documentEvents = events(timeline, "document-event");
  const exercisedDocument = documentEvents
    .filter((record) => record.fields?.event === "document-start")
    .at(-1)?.fields?.document;
  expect(exercisedDocument).toEqual(expect.any(String));
  const counts = new Map<unknown, number>();
  for (const record of documentEvents) {
    expect(record.fields?.document_known).toBe(true);
    const document = record.fields?.document;
    counts.set(document, (counts.get(document) ?? 0) + 1);
  }
  expect(counts.get(exercisedDocument)).toBe(128);
  expect([...counts.values()].every((count) => count <= 128)).toBe(true);
  expect(events(timeline, "observer-exhausted")).toContainEqual(expect.objectContaining({
    fields: expect.objectContaining({ document: exercisedDocument }),
  }));
  // WebKit can expose an initial about:blank document before this navigation. Match the full
  // stream to the independently bounded document groups instead of assuming there was only one.
  expect(consoleRecords).toHaveLength(
    [...counts.values()].reduce((total, count) => total + count, 0)
      + Number(summary.loss?.observerExhaustions),
  );
  expect(
    consoleRecords.every((record) =>
      Buffer.byteLength(record, "utf8") <= BrowserTimeline.maxRecordBytes
    ),
  ).toBe(true);
});

/** Malformed, unknown, and oversized console input becomes bounded loss evidence. */
test("console admission rejects records before they reach the collector", async ({ browser }) => {
  const timeline = new BrowserTimeline();
  const context = await newObservedContext(browser, timeline);
  const page = await context.newPage();
  await page.evaluate((prefix) => {
    console.log(`${prefix}{broken`);
    console.log(`${prefix}${JSON.stringify({
      event: "input",
      sequence: 2,
      timestamp: 0,
      unknown: "field",
    })}`);
    console.log(`${prefix}${"x".repeat(3_000)}`);
  }, timelineConsolePrefix);
  await context.close();
  finish(timeline);

  const summary = records(timeline).at(-1)!;
  expect(Number(summary.loss?.diagnosticErrors)).toBeGreaterThanOrEqual(3);
  expect(Number(summary.loss?.events)).toBeGreaterThanOrEqual(3);
  expect(JSON.stringify(records(timeline))).not.toContain("x".repeat(256));
});

/** Evidence installation must leave a test's replacement constructor untouched. */
test("a custom WebSocket constructor remains authoritative", async ({ browser }) => {
  const timeline = new BrowserTimeline();
  const context = await newObservedContext(browser, timeline);
  const page = await context.newPage();
  const result = await page.evaluate(() => {
    let calls = 0;
    class CustomSocket {
      readonly url: string;
      constructor(url: string) {
        calls += 1;
        this.url = url;
      }
    }
    (window as any).WebSocket = CustomSocket;
    const socket = new WebSocket("ws://constructor.test/custom") as any;
    return { calls, custom: socket instanceof CustomSocket, url: socket.url };
  });
  expect(result).toEqual({ calls: 1, custom: true, url: "ws://constructor.test/custom" });
  await context.close();
  finish(timeline);
});

/** Unchanged readiness polls are omitted without hiding either state transition. */
test("readiness records only changed snapshots with requested identity", async ({ browser }) => {
  const timeline = new BrowserTimeline();
  const context = await newObservedContext(browser, timeline);
  const page = await context.newPage();
  await page.setContent('<textarea id="terminal-input"></textarea>');
  await page.evaluate(() => {
    let observations = 0;
    const socket = {
      url: "ws://fixture.test/api/sessions/session-a/term",
      get readyState() {
        observations += 1;
        return observations < 4 ? WebSocket.CONNECTING : WebSocket.OPEN;
      },
    };
    (window as any).__farhelmIslands = {
      terminal: {
        term: { textarea: document.querySelector("#terminal-input") },
        ws: socket,
        test: { replay: { revealed: false } },
      },
    };
  });
  await waitForSessionSocketOpen(page, "session-a", { timeout: 5_000 });
  await context.close();
  finish(timeline);

  const transitions = events(timeline, "readiness-transition");
  expect(transitions).toHaveLength(2);
  expect(transitions[0].fields).toMatchObject({
    requested: "session-a",
    kind: "socket-open",
    socket_open: false,
    socket_matches: true,
    path_matches: true,
  });
  expect(transitions[1].fields).toMatchObject({ socket_open: true });
  expect(events(timeline).map((record) => record.kind)).toEqual(expect.arrayContaining([
    "readiness-begin",
    "readiness-success",
  ]));
});

/** A real native socket supplies arrival, safe routing identity, and close evidence. */
test("native socket observation retains routing identity and close", async ({ browser }) => {
  const peer = await openSocketPeer();
  const timeline = new BrowserTimeline();
  const context = await newObservedContext(browser, timeline);
  try {
    const page = await context.newPage();
    await page.setContent("<main>socket</main>");
    await page.evaluate((url) => new Promise<void>((resolve, reject) => {
      const socket = new WebSocket(url);
      socket.addEventListener("open", () => resolve(), { once: true });
      socket.addEventListener("error", () => reject(new Error("native socket did not open")), { once: true });
      (window as any).__contractSocket = socket;
    }), `${peer.url}/api/events?tab=chosen&lease=raw-lease&auth=raw-auth`);
    await peer.closeSockets();
    await expect.poll(() => page.evaluate(() => (window as any).__contractSocket.readyState)).toBe(3);
  } finally {
    await context.close();
    await peer.close();
  }
  finish(timeline);

  const socketEvents = events(timeline).filter((record) => record.kind.startsWith("socket-"));
  expect(socketEvents.map((record) => record.kind)).toContain("socket-observed");
  expect(socketEvents.map((record) => record.kind)).toContain("socket-close");
  expect(socketEvents.find((record) => record.kind === "socket-observed")?.fields)
    .toMatchObject({ pathname: "/api/events", tab: "chosen", url_valid: true });
  expect(JSON.stringify(socketEvents)).not.toContain("raw-lease");
  expect(JSON.stringify(socketEvents)).not.toContain("raw-auth");
});

/**
 * Replacing the live mock socket must not relabel the old socket's delayed
 * close, and every feed action retains the connection it actually addressed.
 */
test("mock feed records immutable connection and revision control points", async ({ browser }) => {
  const timeline = new BrowserTimeline();
  const context = await newObservedContext(browser, timeline);
  const page = await context.newPage();
  const feed = await stubFeed(page);
  await page.goto("data:text/html,%3Cmain%3Efeed%3C%2Fmain%3E");
  feed.notifyOnConnect(7);
  await page.evaluate(() => {
    (window as any).__feedFirst = new WebSocket("ws://feed.test/api/events?tab=mock&lease=private");
  });
  await expect.poll(feed.connections).toBe(1);
  await page.evaluate(() => {
    (window as any).__feedSecond = new WebSocket("ws://feed.test/api/events?tab=mock&lease=private");
  });
  await expect.poll(feed.connections).toBe(2);
  await page.evaluate(() => (window as any).__feedFirst.close());
  // The stub's existing onClose handler observes the client request without acknowledging it
  // back to the browser. Its owned-set removal proves the old callback ran after replacement;
  // waiting for browser CLOSED would test a handshake this fixture does not implement.
  await expect.poll(feed.openSockets).toBe(1);
  feed.notify(8);
  feed.kill();
  await expect.poll(() => page.evaluate(() => (window as any).__feedSecond.readyState)).toBe(3);
  feed.notifyOnConnect(undefined);
  await context.close();
  finish(timeline);

  const feedEvents = events(timeline).filter((record) => record.kind.startsWith("feed-"));
  expect(JSON.stringify(records(timeline))).not.toContain("private");
  expect(feedEvents).toEqual(expect.arrayContaining([
    expect.objectContaining({ kind: "feed-greeting-arm", fields: expect.objectContaining({ revision: 7 }) }),
    expect.objectContaining({ kind: "feed-arrival", fields: expect.objectContaining({ connection: 1 }) }),
    expect.objectContaining({
      kind: "feed-greeting",
      fields: expect.objectContaining({ connection: 1, revision: 7 }),
    }),
    expect.objectContaining({ kind: "feed-arrival", fields: expect.objectContaining({ connection: 2 }) }),
    expect.objectContaining({
      kind: "feed-notify",
      fields: expect.objectContaining({ connection: 2, revision: 8 }),
    }),
    expect.objectContaining({ kind: "feed-kill", fields: expect.objectContaining({ connection: 2 }) }),
    expect.objectContaining({ kind: "feed-close", fields: expect.objectContaining({ connection: 1 }) }),
    expect.objectContaining({ kind: "feed-greeting-disarm" }),
  ]));
});

/** Rejected types and ZIP errors cannot strand descriptors or transfer an unchecked file. */
test("trace descriptor ownership transfers only after successful ZIP admission", async ({}, testInfo) => {
  const root = testInfo.outputPath("trace-descriptors");
  await mkdir(root, { recursive: true });
  const file = `${root}/regular`;
  await writeFile(file, "fixed ZIP fixture input");
  await symlink(file, `${root}/symlink`);
  let descriptor: number | undefined;
  const accepted = {};
  const zip = {
    fromFd(fd: number, _options: unknown, callback: (error: Error | null, value?: object) => void) {
      descriptor = fd;
      callback(null, accepted);
    },
  };
  try {
    expect(await openTraceZip(file, zip)).toBe(accepted);
    expect(fstatSync(descriptor!).isFile()).toBe(true);
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
  descriptor = undefined;
  await expect(openTraceZip(root, zip)).rejects.toThrow("not a regular file");
  await expect(openTraceZip(`${root}/symlink`, zip)).rejects.toThrow();
  expect(descriptor).toBeUndefined();
  await expect(openTraceZip(file, {
    fromFd(fd: number, _options: unknown, callback: (error: Error) => void) {
      descriptor = fd;
      callback(new Error("fixed ZIP rejection"));
    },
  })).rejects.toThrow("fixed ZIP rejection");
  expect(descriptor).toEqual(expect.any(Number));
  expect(() => fstatSync(descriptor!)).toThrow(expect.objectContaining({ code: "EBADF" }));
});

/**
 * Tiny decompressed output cannot bypass a finite compressed-input budget.
 * Injection checks admission before opening a stream, including aggregate sizes.
 */
test("trace stream admission bounds compressed and declared output bytes", async () => {
  const limit = 8 * 1024 * 1024;
  for (const field of ["compressedSize", "uncompressedSize"]) {
    for (const sizes of [[limit + 1], [-1], [NaN], [Infinity], [0.5], [limit, 1]]) {
      const zip = new EventEmitter() as EventEmitter & {
        readEntry(): void;
        openReadStream(entry: unknown, callback: (error: null, stream: Readable) => void): void;
        close(): void;
      };
      let opened = 0;
      let closed = 0;
      let index = 0;
      zip.readEntry = () => {
        if (index === sizes.length) zip.emit("end");
        else zip.emit("entry", {
          fileName: "test.trace",
          compressedSize: 0,
          uncompressedSize: 0,
          [field]: sizes[index++],
        });
      };
      zip.openReadStream = (_entry, callback) => {
        opened += 1;
        callback(null, Readable.from([Buffer.from("{}\n")]));
      };
      zip.close = () => { closed += 1; };
      await expect(inspectTraceZip(zip)).rejects.toThrow("byte inspection bound");
      expect(opened).toBe(sizes.length - 1);
      expect(closed).toBe(1);
      expect(zip.eventNames()).toEqual([]);
    }
  }
});

/** Action arguments and malformed prefixed strings cannot stand in for emitted console events. */
test("trace evidence requires the console schema and a valid observer payload", () => {
  const text = timelineConsolePrefix + JSON.stringify({
    document: "0".repeat(32), event: "input", sequence: 1, timestamp: 1,
  });
  const payloads: string[] = [];
  collectConsolePayloads(JSON.stringify({ type: "before", params: { expression: text } }), payloads);
  collectConsolePayloads(JSON.stringify({ type: "console", args: [{ value: text }], text: "ordinary" }), payloads);
  expect(payloads).toEqual([]);
  for (const malformed of ["{}", "not JSON", JSON.stringify({ event: "input" })]) {
    expect(() => collectConsolePayloads(JSON.stringify({
      type: "console", text: timelineConsolePrefix + malformed,
    }), payloads)).toThrow();
  }
  expect(payloads).toEqual([]);
  collectConsolePayloads(JSON.stringify({ type: "console", text }), payloads);
  expect(payloads).toEqual([text]);
});

/** Each parent case validates one exact nonzero child outcome and its artifacts. */
for (const contract of [
  ["intentional timeline body failure", "failure"],
  ["intentional timeline timeout", "timeout"],
  ["intentional timeline unexpected pass", "unexpected-pass"],
  ["intentional timeline teardown failure", "teardown-failure"],
] as const) {
  test(`child contract: ${contract[1]}`, async ({}, testInfo) => {
    test.setTimeout(40_000);
    await verifyIntentionalChild(testInfo, contract[0], contract[1]);
  });
}

/**
 * Minimal context emitter for proving listener ownership without asking a
 * browser process to manufacture hundreds of resources.
 */
class FakeContext {
  private readonly listeners = new Map<string, Set<(...args: never[]) => void>>();
  readonly removalAttempts: string[] = [];

  constructor(private readonly options: {
    pages?: Page[];
    addInitScript?: Promise<void>;
    throwOnOff?: string;
  } = {}) {}

  pages(): Page[] {
    return this.options.pages ?? [];
  }

  async addInitScript(): Promise<void> {
    await this.options.addInitScript;
  }

  on(name: string, callback: (...args: never[]) => void): void {
    const callbacks = this.listeners.get(name) ?? new Set();
    callbacks.add(callback);
    this.listeners.set(name, callbacks);
  }

  off(name: string, callback: (...args: never[]) => void): void {
    this.removalAttempts.push(name);
    if (name === this.options.throwOnOff) throw new Error("fixed fake removal failure");
    this.listeners.get(name)?.delete(callback);
  }

  hasListener(name: string): boolean {
    return (this.listeners.get(name)?.size ?? 0) > 0;
  }

  callback(name: string): ((...args: never[]) => void) | undefined {
    return this.listeners.get(name)?.values().next().value;
  }

  listenerCount(): number {
    let count = 0;
    for (const callbacks of this.listeners.values()) count += callbacks.size;
    return count;
  }
}

interface SocketPeer {
  url: string;
  closeSockets(): Promise<void>;
  close(): Promise<void>;
}

/**
 * Serve only the RFC 6455 upgrade needed to observe the browser's native
 * socket. The peer carries no application protocol and owns every accepted
 * socket so teardown cannot leak a browser descendant or listening handle.
 */
async function openSocketPeer(): Promise<SocketPeer> {
  const sockets = new Set<Socket>();
  const server = createServer();
  server.on("upgrade", (request, socket) => {
    const key = request.headers["sec-websocket-key"];
    if (typeof key !== "string") {
      socket.destroy();
      return;
    }
    const accept = createHash("sha1")
      .update(`${key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`)
      .digest("base64");
    socket.write([
      "HTTP/1.1 101 Switching Protocols",
      "Upgrade: websocket",
      "Connection: Upgrade",
      `Sec-WebSocket-Accept: ${accept}`,
      "",
      "",
    ].join("\r\n"));
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (address === null || typeof address === "string") throw new Error("socket peer did not bind TCP");

  const closeSockets = async () => {
    for (const socket of sockets) socket.destroy();
    await new Promise<void>((resolve) => setImmediate(resolve));
  };
  return {
    url: `ws://127.0.0.1:${address.port}`,
    closeSockets,
    async close() {
      await closeSockets();
      await new Promise<void>((resolve, reject) => {
        server.close((error) => error ? reject(error) : resolve());
      });
    },
  };
}
