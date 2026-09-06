import type { Page } from "@playwright/test";

type Scalar = boolean | number | string;
/** Indexed pairs let admission stop before touching an arbitrarily wide caller input. */
export type TimelineFields = ReadonlyArray<readonly [string, unknown]>;

export interface TimelineIdentity {
  testTitle?: unknown;
  testFile?: unknown;
  project?: unknown;
  retry?: unknown;
  repeatEachIndex?: unknown;
}

type DiagnosticCategory = "setup" | "listener" | "cleanup";

interface LossCounts {
  events: number;
  fieldsClipped: number;
  fieldsOmitted: number;
  fieldsOmittedExtentUnknown: boolean;
  listenerRefusals: number;
  observerExhaustions: number;
  diagnosticErrors: number;
}

interface EncodedRecord {
  text: string;
  bytes: number;
}

const MAX_COUNTER = Number.MAX_SAFE_INTEGER;
const HEAD_RECORDS = 64;
const STRUCTURAL_RECORDS = 2;
const STRUCTURAL_RESERVE_BYTES = 4 * 1024;

/**
 * Retains failure evidence without letting diagnostic traffic become another
 * unbounded workload. Callers must close it before serialization; closing
 * normalizes identity, removes owned listeners, and freezes every counter.
 */
export class BrowserTimeline {
  static readonly maxRecords = 1024;
  static readonly maxBytes = 256 * 1024;
  static readonly maxRecordBytes = 2 * 1024;
  static readonly maxScalarBytes = 128;
  static readonly maxFields = 16;
  static readonly maxListenerGroups = 256;

  private readonly head: EncodedRecord[] = [];
  private readonly tail: EncodedRecord[] = [];
  private readonly cleanup: Array<() => void> = [];
  private eventBytes = 0;
  private receiptSequence = 0;
  private observed = 0;
  private retained = 0;
  private closed = false;
  private frozenOutput: Buffer | undefined;
  private pageSequence = 0;
  private socketSequence = 0;
  private readonly loss: LossCounts = {
    events: 0,
    fieldsClipped: 0,
    fieldsOmitted: 0,
    fieldsOmittedExtentUnknown: false,
    listenerRefusals: 0,
    observerExhaustions: 0,
    diagnosticErrors: 0,
  };

  /** Record one event from explicitly bounded primitive fields. */
  record(kind: unknown, fields: TimelineFields = []): void {
    this.recordEvent(kind, fields);
  }

  /** Keep observer-owned page identity ahead of caller fields and byte trimming. */
  recordForPage(pageId: string, kind: unknown, fields: TimelineFields = []): void {
    this.recordEvent(kind, fields, pageId);
  }

  /** Admit bounded caller fields without enumerating or copying their source collection. */
  private recordEvent(kind: unknown, fields: TimelineFields, pageId?: string): void {
    if (this.closed) return;
    this.observed = increment(this.observed);

    if (this.receiptSequence === MAX_COUNTER) {
      this.loss.events = increment(this.loss.events);
      return;
    }

    const safeKind = this.normalizeScalar(kind, "invalid-kind");
    const safeFields = this.normalizeFields(fields, pageId);
    const receipt = ++this.receiptSequence;
    const receiptNs = process.hrtime.bigint().toString();

    let text = encodeEvent(safeKind, receipt, receiptNs, safeFields);
    while (lineBytes(text) > BrowserTimeline.maxRecordBytes && safeFields.length > 0) {
      safeFields.pop();
      this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
      text = encodeEvent(safeKind, receipt, receiptNs, safeFields);
    }

    const bytes = lineBytes(text);
    if (bytes > BrowserTimeline.maxRecordBytes) {
      this.loss.events = increment(this.loss.events);
      return;
    }
    this.insert({ text, bytes });
  }

  /** Record a fixed error class without retaining an error message or stack. */
  diagnostic(category: DiagnosticCategory): void {
    if (this.closed) return;
    this.loss.diagnosticErrors = increment(this.loss.diagnosticErrors);
    this.record("diagnostic", [["category", category]]);
  }

  /** Mark an observation that the helper knows it could not recover. */
  observationLost(reason: string): void {
    if (this.closed) return;
    this.loss.events = increment(this.loss.events);
    this.record("observation-incomplete", [["reason", reason]]);
  }

  /** Mark the browser-side cap that makes later event absence inconclusive. */
  observerExhausted(page: string, document: string): void {
    if (this.closed) return;
    this.loss.observerExhaustions = increment(this.loss.observerExhaustions);
    this.record("observer-exhausted", [["page", page], ["document", document]]);
  }

  /** Carry pre-admission clipping and omission into the frozen host summary. */
  noteFieldLoss(clipped: number, omitted: number): void {
    if (this.closed) return;
    this.loss.fieldsClipped = incrementBy(this.loss.fieldsClipped, clipped);
    this.loss.fieldsOmitted = incrementBy(this.loss.fieldsOmitted, omitted);
  }

  /**
   * Reserve one resource's listener cleanup before any listener is attached.
   * Refusal is itself frozen loss evidence and installs no partial group.
   */
  reserveListenerGroup(remove: () => void): boolean {
    if (this.closed) return false;
    if (this.cleanup.length >= BrowserTimeline.maxListenerGroups) {
      this.loss.listenerRefusals = increment(this.loss.listenerRefusals);
      return false;
    }
    this.cleanup.push(remove);
    return true;
  }

  /** Allocate identities in the owning test's clock domain. */
  nextPageId(): string | undefined {
    if (this.closed || this.pageSequence === MAX_COUNTER) {
      if (!this.closed) this.loss.events = increment(this.loss.events);
      return undefined;
    }
    return `page-${++this.pageSequence}`;
  }

  /** Allocate identities in the owning test's clock domain. */
  nextSocketId(): string | undefined {
    if (this.closed || this.socketSequence === MAX_COUNTER) {
      if (!this.closed) this.loss.events = increment(this.loss.events);
      return undefined;
    }
    return `socket-${++this.socketSequence}`;
  }

  /**
   * Normalize identity, remove every owned listener, then freeze the artifact.
   * Cleanup failures remain diagnostics but never replace the test's outcome.
   */
  close(identity: TimelineIdentity = {}): void {
    if (this.closed) return;

    const safeIdentity = this.normalizeIdentity(identity);
    for (const remove of this.cleanup) {
      try {
        remove();
      } catch {
        this.diagnostic("cleanup");
      }
    }
    this.cleanup.length = 0;
    const header = this.fitHeader(safeIdentity.fields, safeIdentity.clipped);
    this.closed = true;
    const footer = JSON.stringify({
      kind: "summary",
      observed: this.observed,
      retained: this.retained,
      artifact_records: this.retained + STRUCTURAL_RECORDS,
      loss: { ...this.loss },
      incomplete: hasLoss(this.loss),
    });
    const lines = [
      header,
      ...this.head.map((entry) => entry.text),
      ...this.tail.map((entry) => entry.text),
      footer,
    ];
    this.frozenOutput = Buffer.from(`${lines.join("\n")}\n`, "utf8");

    if (
      this.frozenOutput.byteLength > BrowserTimeline.maxBytes ||
      lines.length > BrowserTimeline.maxRecords ||
      lines.some((line) => lineBytes(line) > BrowserTimeline.maxRecordBytes)
    ) {
      throw new Error("browser timeline violated its frozen artifact bounds");
    }
  }

  /** Return a copy of the already-frozen artifact without changing counters. */
  toJSONL(): Buffer {
    if (!this.frozenOutput) throw new Error("browser timeline must be closed before serialization");
    return Buffer.from(this.frozenOutput);
  }

  /** Normalize one string without ever traversing bytes beyond its retained prefix. */
  private normalizeScalar(value: unknown, replacement = "invalid"): string {
    if (typeof value !== "string") {
      this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
      return replacement;
    }
    const clipped = clipUtf8(value, BrowserTimeline.maxScalarBytes);
    if (clipped.clipped) this.loss.fieldsClipped = increment(this.loss.fieldsClipped);
    return clipped.value;
  }

  /**
   * Read at most sixteen indexed pairs, reserving the first output slot for page identity.
   * Object-key enumeration can allocate a whole key list before an early loop break;
   * indexed input makes the admission bound independent of the caller's collection size.
   */
  private normalizeFields(fields: TimelineFields, pageId?: string): Array<[string, Scalar]> {
    const result: Array<[string, Scalar]> = pageId === undefined
      ? [] : [["page", this.normalizeScalar(pageId)]];
    if (!Array.isArray(fields)) {
      this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
      return result;
    }
    try {
      const length = fields.length;
      if (!Number.isSafeInteger(length) || length < 0) throw new Error("invalid field collection length");
      const admitted = Math.min(length, BrowserTimeline.maxFields - Number(pageId !== undefined));
      if (length > admitted) {
        this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
        this.loss.fieldsOmittedExtentUnknown = true;
      }
      for (let index = 0; index < admitted; index += 1) {
        let key: unknown;
        let value: unknown;
        try {
          const pair = fields[index];
          key = pair[0];
          value = pair[1];
        } catch {
          this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
          continue;
        }
        const safeKey = this.normalizeScalar(key);
        let safeValue: Scalar;
        if (typeof value === "string") {
          safeValue = this.normalizeScalar(value);
        } else if (typeof value === "boolean") {
          safeValue = value;
        } else if (typeof value === "number" && Number.isFinite(value)) {
          safeValue = value;
        } else {
          this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
          continue;
        }

        if (result.some(([acceptedKey]) => acceptedKey === safeKey)) {
          this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
          continue;
        }
        result.push([safeKey, safeValue]);
      }
    } catch {
      this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
      this.loss.fieldsOmittedExtentUnknown = true;
    }
    return result;
  }

  /** Read only the identity allowlist and account for every rejected value. */
  private normalizeIdentity(identity: TimelineIdentity): {
    fields: Array<[string, Scalar]>;
    clipped: Set<string>;
  } {
    const result: Array<[string, Scalar]> = [];
    const clippedFields = new Set<string>();
    const strings: Array<[string, unknown]> = [
      ["test", identity.testTitle],
      ["file", identity.testFile],
      ["project", identity.project],
    ];
    for (const [key, value] of strings) {
      if (value === undefined) continue;
      if (typeof value !== "string") {
        this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
        continue;
      }
      const clipped = clipUtf8(value, BrowserTimeline.maxScalarBytes);
      if (clipped.clipped) {
        clippedFields.add(key);
        this.loss.fieldsClipped = increment(this.loss.fieldsClipped);
      }
      result.push([key, clipped.value]);
    }
    for (const [key, value] of [["retry", identity.retry], ["repeat", identity.repeatEachIndex]] as const) {
      if (value === undefined) continue;
      if (typeof value === "number" && Number.isFinite(value)) result.push([key, value]);
      else this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
    }
    return { fields: result, clipped: clippedFields };
  }

  /** Reduce bounded identity values until escaping also fits the line limit. */
  private fitHeader(identity: Array<[string, Scalar]>, clippedFields: Set<string>): string {
    let header = encodeHeader(identity);
    while (lineBytes(header) > BrowserTimeline.maxRecordBytes) {
      const index = longestStringValue(identity);
      if (index === -1) {
        const removed = identity.pop();
        if (!removed) throw new Error("browser timeline header cannot fit its fixed schema");
        this.loss.fieldsOmitted = increment(this.loss.fieldsOmitted);
      } else {
        const [key, value] = identity[index];
        identity[index] = [key, removeLastCodePoint(value as string)];
        if (!clippedFields.has(key)) {
          clippedFields.add(key);
          this.loss.fieldsClipped = increment(this.loss.fieldsClipped);
        }
      }
      header = encodeHeader(identity);
    }
    return header;
  }

  /** Preserve the fixed beginning and evict only the bounded diagnostic tail. */
  private insert(record: EncodedRecord): void {
    const maxEvents = BrowserTimeline.maxRecords - STRUCTURAL_RECORDS;
    const maxEventBytes = BrowserTimeline.maxBytes - STRUCTURAL_RESERVE_BYTES;
    if (this.head.length < HEAD_RECORDS) {
      this.head.push(record);
      this.eventBytes += record.bytes;
      this.retained = increment(this.retained);
      return;
    }

    while (
      this.tail.length > 0 &&
      (this.retained >= maxEvents || this.eventBytes + record.bytes > maxEventBytes)
    ) {
      const removed = this.tail.shift()!;
      this.eventBytes -= removed.bytes;
      this.retained -= 1;
      this.loss.events = increment(this.loss.events);
    }
    if (this.retained >= maxEvents || this.eventBytes + record.bytes > maxEventBytes) {
      this.loss.events = increment(this.loss.events);
      return;
    }
    this.tail.push(record);
    this.eventBytes += record.bytes;
    this.retained = increment(this.retained);
  }
}

const pages = new WeakMap<Page, { timeline: BrowserTimeline; pageId: string }>();

/** Associate a page with one timeline without retaining the page after teardown. */
export function registerObservedPage(page: Page, timeline: BrowserTimeline, pageId: string): void {
  pages.set(page, { timeline, pageId });
}

/** Record a helper control point only while its owning test remains open. */
export function recordPage(page: Page, kind: string, fields?: TimelineFields): void {
  const owner = pages.get(page);
  owner?.timeline.recordForPage(owner.pageId, kind, fields);
}

/** Bound a string by UTF-8 bytes without splitting a code point. */
export function bounded(value: string): string {
  return clipUtf8(value, BrowserTimeline.maxScalarBytes).value;
}

/** Stop decoding at the byte boundary instead of slicing a surrogate pair. */
function clipUtf8(value: string, limit: number): { value: string; clipped: boolean } {
  let output = "";
  let bytes = 0;
  let clipped = false;
  for (const character of value) {
    const width = Buffer.byteLength(character);
    if (bytes + width > limit) {
      clipped = true;
      break;
    }
    output += character;
    bytes += width;
  }
  return { value: output, clipped };
}

/** Build an event only from the already bounded pair list. */
function encodeEvent(
  kind: string,
  receipt: number,
  receiptNs: string,
  fields: Array<[string, Scalar]>,
): string {
  const object: Record<string, Scalar> = Object.create(null);
  for (const [key, value] of fields) object[key] = value;
  return JSON.stringify({ kind, receipt, receipt_ns: receiptNs, fields: object });
}

/** Build fixed allowlisted metadata without spreading a caller object. */
function encodeHeader(identity: Array<[string, Scalar]>): string {
  const object: Record<string, Scalar> = Object.create(null);
  for (const [key, value] of identity) object[key] = value;
  return JSON.stringify({ version: 1, kind: "browser-timeline", identity: object });
}

function lineBytes(text: string): number {
  return Buffer.byteLength(text, "utf8") + 1;
}

function increment(value: number): number {
  return value === MAX_COUNTER ? value : value + 1;
}

/** Saturate only validated positive integer loss reports from pre-admission code. */
function incrementBy(value: number, amount: number): number {
  if (!Number.isSafeInteger(amount) || amount <= 0) return value;
  return Math.min(MAX_COUNTER, value + amount);
}

/** Any loss class makes absence in the retained evidence inconclusive. */
function hasLoss(loss: LossCounts): boolean {
  return loss.events > 0 ||
    loss.fieldsClipped > 0 ||
    loss.fieldsOmitted > 0 ||
    loss.fieldsOmittedExtentUnknown ||
    loss.listenerRefusals > 0 ||
    loss.observerExhaustions > 0 ||
    loss.diagnosticErrors > 0;
}

/** Select the identity value whose escaped representation costs the most bytes. */
function longestStringValue(fields: Array<[string, Scalar]>): number {
  let selected = -1;
  let selectedBytes = -1;
  for (let index = 0; index < fields.length; index += 1) {
    const value = fields[index][1];
    if (typeof value !== "string") continue;
    const bytes = Buffer.byteLength(JSON.stringify(value), "utf8");
    if (bytes > selectedBytes && value.length > 0) {
      selected = index;
      selectedBytes = bytes;
    }
  }
  return selected;
}

/** Remove one complete code point from a string already bounded to 128 bytes. */
function removeLastCodePoint(value: string): string {
  let previous = "";
  for (const character of value) {
    const next = previous + character;
    if (next.length === value.length) return previous;
    previous = next;
  }
  return "";
}
