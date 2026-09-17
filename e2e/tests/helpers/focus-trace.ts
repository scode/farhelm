// A bounded in-page record of the focus operations around a composer opening.
//
// The composer-focus report is a RACE claim — some claimant takes focus that
// the opening's own search-focus request should have kept — and a bare
// `toBeFocused` cannot name the loser: it passes during a transient success
// and fails without saying who won. This helper records the operations
// themselves (`focusin`/`focusout`, every `HTMLElement.focus` call with its
// target and call site, trusted pointer/keyboard activation around them) plus
// the search input's receivability (present, connected, disabled, visible)
// at each step, so a failing run can answer "search became active, then THIS
// displaced it" versus "the request ran against a missing/disabled target
// and never landed."
//
// The trace lives on the page under test (`window.__farhelmFocusTrace`) and
// is read back with `readFocusTrace`, typically attached to the test report.
// It is bounded (newest events are dropped past the cap, counted in
// `dropped`) and page-scoped: nothing persists past the test's own page.

import { Page, TestInfo } from "@playwright/test";
import fs from "node:fs";

/** One recorded focus/activation observation; see `installFocusTrace`. */
export type FocusTraceEvent = {
  /** `performance.now()` milliseconds, for ordering against page activity. */
  t: number;
  kind:
    | "focusin"
    | "focusout"
    | "focus-call"
    | "pointerdown"
    | "mousedown"
    | "click"
    | "keydown"
    | "keyup";
  /** What the event targeted, or what `focus()` was called on. */
  target: string;
  /** `document.activeElement` when the observation was recorded. */
  active: string;
  /** The DOM event's own `isTrusted`, where the observation is an event. */
  trusted?: boolean;
  /** The key for keyboard observations. */
  key?: string;
  /** Trimmed call-site frames for `focus()` calls. */
  stack?: string[];
  /** Set on `focus-call` entries the armed gate held instead of delivering. */
  held?: boolean;
  /** Whether an originating row-menu panel is still mounted. */
  menuPresent: boolean;
  /** Whether the composer dialog is mounted. */
  dialogPresent: boolean;
  /** The shared search input's receivability at this observation. */
  search: {
    present: boolean;
    connected: boolean;
    disabled: boolean;
    visible: boolean;
    node: string;
  };
};

/** The installed trace store, as `readFocusTrace` returns it. */
export type FocusTrace = {
  events: FocusTraceEvent[];
  dropped: number;
};

/**
 * Record focus operations on the page from now until it is read or closed.
 *
 * Idempotent: reinstalling resets the event list but never double-wraps
 * `HTMLElement.prototype.focus`, so a reopen case can restart the record
 * between openings without duplicating `focus-call` entries.
 */
export async function installFocusTrace(page: Page): Promise<void> {
  await page.evaluate(() => {
    const store = (window as unknown as Record<string, unknown>).__farhelmFocusTrace as
      | { events: unknown[]; dropped: number }
      | undefined;
    if (store) {
      store.events = [];
      store.dropped = 0;
      return;
    }
    const MAX_EVENTS = 600;
    const trace: { events: FocusTraceEvent[]; dropped: number } = { events: [], dropped: 0 };
    (window as unknown as Record<string, unknown>).__farhelmFocusTrace = trace;
    // Node identities, so "the same search input" is checkable across a
    // rerender: a stable number per DOM node for this page's lifetime.
    const nodeIds = new WeakMap<object, number>();
    let nextNodeId = 1;
    const SEARCH_SELECTOR = '.create-session-form[role="dialog"] .launch-composer-search input';
    const describe = (node: unknown): string => {
      if (!(node instanceof Element)) return node === null ? "null" : String(node);
      let id = nodeIds.get(node);
      if (id === undefined) {
        id = nextNodeId;
        nextNodeId += 1;
        nodeIds.set(node, id);
      }
      const parts = [`${node.tagName.toLowerCase()}#${id}`];
      if (node.id) parts.push(`id=${node.id}`);
      const classes = node.className && typeof node.className === "string"
        ? node.className.trim().split(/\s+/).slice(0, 3).join(".")
        : "";
      if (classes) parts.push(`.${classes}`);
      for (const attr of ["role", "aria-label", "data-session-id", "type"]) {
        const value = node.getAttribute(attr);
        if (value !== null) parts.push(`${attr}=${JSON.stringify(value.slice(0, 48))}`);
      }
      return parts.join(" ");
    };
    const searchState = () => {
      const input = document.querySelector(SEARCH_SELECTOR);
      if (!(input instanceof HTMLInputElement)) {
        return { present: false, connected: false, disabled: false, visible: false, node: "absent" };
      }
      // `offsetParent` is null for fixed-position elements too; the search
      // input is statically positioned inside the dialog, so null here
      // means hidden or detached rather than fixed.
      return {
        present: true,
        connected: input.isConnected,
        disabled: input.disabled,
        visible: input.offsetParent !== null,
        node: describe(input),
      };
    };
    const record = (event: Omit<FocusTraceEvent, "menuPresent" | "dialogPresent" | "search" | "t" | "active"> & {
      t?: number;
      active?: string;
    }) => {
      if (trace.events.length >= MAX_EVENTS) {
        trace.dropped += 1;
        return;
      }
      trace.events.push({
        ...event,
        t: Math.round(performance.now() * 10) / 10,
        active: describe(document.activeElement),
        menuPresent: document.querySelector(".session-row-menu-panel") !== null,
        dialogPresent: document.querySelector('.create-session-form[role="dialog"]') !== null,
        search: searchState(),
      } as FocusTraceEvent);
    };
    for (const kind of ["focusin", "focusout", "pointerdown", "mousedown", "click", "keydown", "keyup"] as const) {
      document.addEventListener(
        kind,
        (evt) => {
          record({
            kind,
            target: describe(evt.target),
            trusted: evt.isTrusted,
            ...(evt instanceof KeyboardEvent ? { key: evt.key } : {}),
          });
        },
        true,
      );
    }
    const originalFocus = HTMLElement.prototype.focus;
    (window as unknown as Record<string, unknown>).__farhelmFocusOriginal = originalFocus;
    HTMLElement.prototype.focus = function (...args: Parameters<typeof originalFocus>) {
      // The stack names the claimant family: terminal.js frames versus the
      // renderer's eval dispatch. Six frames is enough to see past the
      // patch itself without retaining megabytes of wasm glue.
      const stack = new Error().stack?.split("\n").slice(1, 7).map((line) => line.trim().slice(0, 160));
      // An armed gate holds REAL production focus deliveries at the DOM
      // boundary — the eval or mounted-focus call the application issued
      // — recording them exactly like delivered ones (plus `held`) while
      // skipping the browser effect, so a test can establish a premise
      // and release them later. Nothing is injected: with no gate armed,
      // or for a call the gate's predicate does not match, this is a
      // pure observer that calls straight through.
      const gate = (window as unknown as Record<string, unknown>).__farhelmFocusGate as
        | { mode: string; frozenDialog: Element | null; held: unknown[]; released: boolean }
        | null
        | undefined;
      let hold = false;
      if (gate && !gate.released) {
        if (gate.mode === "toggle") {
          hold = this instanceof HTMLElement && this.classList.contains("session-row-menu");
        } else if (gate.mode === "dialog") {
          const dialog = gate.frozenDialog
            ?? document.querySelector('.create-session-form[role="dialog"]');
          hold = !!(dialog && dialog.contains(this));
        }
      }
      record({ kind: "focus-call", target: describe(this), stack, held: hold || undefined });
      if (hold && gate) {
        (gate.held as { el: unknown; target: string; stack: string[] | undefined; t: number }[]).push({
          el: this,
          target: describe(this),
          stack,
          t: Math.round(performance.now() * 10) / 10,
        });
        return undefined;
      }
      return (originalFocus as (...callArgs: unknown[]) => unknown).apply(this, args);
    };
  });
}

/** One focus delivery the armed gate held, as `releaseFocusGate` reports it. */
export type FocusGateReceipt = {
  /** What the production call targeted. */
  target: string;
  /** Whether that node was still connected when the gate released it. */
  connectedBefore: boolean;
  /** `performance.now()` milliseconds when the call was held. */
  t: number;
};

/**
 * Arm the installed trace's intercept as a gate holding production focus
 * deliveries instead of merely recording them.
 *
 * Requires `installFocusTrace` first (the gate is a mode of its intercept,
 * not a second patch). `toggle` holds calls on row-menu toggles — the
 * originating dismissal's return; `dialog` holds calls inside the current
 * composer dialog — one opening's mount work. Held calls are recorded in
 * the trace with `held` set and skipped, never injected or reordered.
 * Always pair with `releaseFocusGate` in a `finally`.
 */
export async function armFocusGate(page: Page, mode: "toggle" | "dialog"): Promise<void> {
  await page.evaluate((gateMode) => {
    const store = (window as unknown as Record<string, unknown>).__farhelmFocusTrace;
    if (!store) throw new Error("installFocusTrace must run before armFocusGate");
    (window as unknown as Record<string, unknown>).__farhelmFocusGate = {
      mode: gateMode,
      frozenDialog: null,
      held: [],
      released: false,
    };
  }, mode);
}

/**
 * Pin a `dialog` gate to the composer dialog mounted right now.
 *
 * Call after the held opening's work has arrived and before closing it:
 * later calls are judged against this (possibly since detached) element
 * rather than whatever dialog is current, so a reopened dialog's own
 * mount work passes through while stragglers from the closed one stay
 * held.
 */
export async function freezeDialogGate(page: Page): Promise<void> {
  await page.evaluate(() => {
    const gate = (window as unknown as Record<string, unknown>).__farhelmFocusGate as
      | { mode: string; frozenDialog: Element | null }
      | null
      | undefined;
    if (!gate || gate.mode !== "dialog") throw new Error("no dialog gate is armed");
    gate.frozenDialog = document.querySelector('.create-session-form[role="dialog"]');
    if (!gate.frozenDialog) throw new Error("no composer dialog is mounted to freeze the gate on");
  });
}

/** How many production focus deliveries the armed gate currently holds. */
export async function focusGateHeldCount(page: Page): Promise<number> {
  return page.evaluate(() => {
    const gate = (window as unknown as Record<string, unknown>).__farhelmFocusGate as
      | { held: unknown[] }
      | null
      | undefined;
    if (!gate) throw new Error("no focus gate is armed");
    return gate.held.length;
  });
}

/**
 * Deliver every held call in arrival order, restore the original
 * `HTMLElement.prototype.focus`, and report what was released.
 *
 * Delivery replays the application's own calls against the nodes they
 * named: a call held for a since-detached node is a browser no-op, and
 * its receipt says so (`connectedBefore: false`). Idempotent — a second
 * call reports nothing — so `finally` cleanup is safe unconditionally.
 */
export async function releaseFocusGate(page: Page): Promise<FocusGateReceipt[]> {
  return page.evaluate(() => {
    const win = window as unknown as Record<string, unknown>;
    const gate = win.__farhelmFocusGate as
      | { held: { el: HTMLElement; target: string; t: number }[]; released: boolean }
      | null
      | undefined;
    if (!gate || gate.released) return [];
    gate.released = true;
    const original = win.__farhelmFocusOriginal as HTMLElement["focus"];
    const receipts: FocusGateReceipt[] = [];
    for (const held of gate.held) {
      receipts.push({ target: held.target, connectedBefore: held.el.isConnected, t: held.t });
      original.apply(held.el);
    }
    gate.held = [];
    HTMLElement.prototype.focus = original;
    return receipts;
  });
}

/** Read back the trace `installFocusTrace` recorded, in event order. */
export async function readFocusTrace(page: Page): Promise<FocusTrace> {
  return page.evaluate(() => {
    const trace = (window as unknown as Record<string, unknown>).__farhelmFocusTrace as FocusTrace | undefined;
    if (!trace) throw new Error("focus trace was never installed on this page");
    return { events: trace.events, dropped: trace.dropped };
  });
}

/**
 * One line per recorded focus operation: the claimant, its target, and the
 * resulting active element. Pointer/keyboard activation is elided — the
 * full JSON keeps it — so a failing run's retained output shows the
 * handoff sequence at a glance without scrolling through clicks.
 */
export function summarizeFocusTrace(trace: FocusTrace): string {
  const short = (value: string) => value.length > 90 ? `${value.slice(0, 87)}…` : value;
  const lines = trace.events
    .filter((event) => event.kind === "focusin" || event.kind === "focusout" || event.kind === "focus-call")
    .map((event) => {
      const search = event.search.present
        ? `search#${event.search.node} connected=${event.search.connected} disabled=${event.search.disabled} visible=${event.search.visible}`
        : "search=absent";
      const site = event.kind === "focus-call" && event.stack?.length ? ` @ ${event.stack[0]}` : "";
      const held = event.held ? " [HELD]" : "";
      return `${event.t}ms ${event.kind} target=${short(event.target)} active=${short(event.active)} menu=${event.menuPresent} dialog=${event.dialogPresent} ${search}${site}${held}`;
    });
  if (trace.dropped > 0) lines.push(`(${trace.dropped} oldest events dropped past the trace cap)`);
  return lines.join("\n");
}

/**
 * Persist the full trace JSON to this test's own output directory and
 * attach it, then log the compact handoff summary to retained output.
 *
 * Written through `node:fs` to `testInfo.outputPath` rather than as a
 * body-only attachment, so the JSON exists on disk even if the reporter's
 * attachment handling drops it.
 */
export async function attachFocusTrace(page: Page, testInfo: TestInfo, name: string): Promise<void> {
  const trace = await readFocusTrace(page);
  const file = testInfo.outputPath(name);
  fs.writeFileSync(file, JSON.stringify(trace, null, 1));
  await testInfo.attach(name, { path: file, contentType: "application/json" });
  console.log(`focus trace (${trace.events.length} events, ${trace.dropped} dropped):\n${summarizeFocusTrace(trace)}`);
}
