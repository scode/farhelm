/** Observe the real modal's release boundary without changing its closure identity. */
import { expect } from "./evidence";
import type { JSHandle, Locator } from "@playwright/test";

type ReleaseEvent = {
  kind: "read" | "retired" | "removed" | "focus";
  connected: boolean;
  isolated: boolean;
  backgroundInert: boolean;
  backgroundFocused: boolean;
};

type ReleaseState = {
  events: ReleaseEvent[];
  dispose(): void;
};

/** The retained root remains observable after the renderer removes it. */
export type ModalReleaseObservation = JSHandle<ReleaseState>;

/**
 * Distinguish connected release from the removal observer's fallback cleanup.
 *
 * The fallback calls its private closure directly, whose identity guard still
 * reads the public hook. Returning the original function preserves that guard;
 * wrapping it would disable fallback cleanup. Duplicate reads inside one call
 * are expected, so the evidence describes ordering rather than invocation count.
 */
export async function observeModalRelease(
  dialog: Locator,
  backgroundSelector: string,
  focusSelector: string,
): Promise<ModalReleaseObservation> {
  return dialog.evaluateHandle((node, { backgroundSelector, focusSelector }) => {
    const root = node as HTMLElement & {
      __farhelmModalRelease?: (() => void) | null;
      __farhelmModalIsolated?: boolean;
    };
    const background = document.querySelector(backgroundSelector);
    const descriptor = Object.getOwnPropertyDescriptor(root, "__farhelmModalRelease");
    if (!root.isConnected || !root.__farhelmModalIsolated || !background?.closest("[inert]") ||
      typeof descriptor?.value !== "function" || !descriptor.configurable) {
      throw new Error("release observation requires a connected isolated root and an inert background");
    }
    let current = descriptor.value as (() => void) | null;
    const events: ReleaseEvent[] = [];
    const record = (kind: ReleaseEvent["kind"]) => events.push({
      kind,
      connected: root.isConnected,
      isolated: root.__farhelmModalIsolated === true,
      backgroundInert: background.closest("[inert]") !== null,
      backgroundFocused: document.activeElement === background,
    });
    Object.defineProperty(root, "__farhelmModalRelease", {
      configurable: true,
      enumerable: descriptor.enumerable,
      get() { record("read"); return current; },
      set(value: (() => void) | null) { current = value; if (value === null) record("retired"); },
    });
    const removed = new MutationObserver(() => {
      if (!root.isConnected) { record("removed"); removed.disconnect(); }
    });
    removed.observe(document.body, { childList: true, subtree: true });
    const onFocus = (event: FocusEvent) => {
      if (event.target instanceof Element && event.target.matches(focusSelector)) record("focus");
    };
    document.addEventListener("focusin", onFocus, true);
    return {
      events,
      dispose() {
        removed.disconnect();
        document.removeEventListener("focusin", onFocus, true);
        // Keep the current value: restoring the original function after close
        // would resurrect a hook that production has already retired.
        Object.defineProperty(root, "__farhelmModalRelease", { ...descriptor, value: current });
      },
    };
  }, { backgroundSelector, focusSelector });
}

/**
 * Require actual hook retirement before root removal, not merely a final inert state.
 *
 * A lookup without invocation cannot pass the connected-null-write assertion.
 * Fallback cleanup reads and retires only after disconnection, so it cannot
 * supply the connected read/retirement pair either.
 */
export async function expectConnectedRelease(observation: ModalReleaseObservation): Promise<ReleaseEvent[]> {
  const events = await observation.evaluate(({ events }) => events);
  const explicit = events.findIndex((event) =>
    event.kind === "read" && event.connected && event.isolated && event.backgroundInert);
  const retired = events.findIndex((event) => event.kind === "retired" && event.connected);
  const removed = events.findIndex((event) => event.kind === "removed");
  expect(explicit, JSON.stringify(events)).toBeGreaterThanOrEqual(0);
  expect(retired, JSON.stringify(events)).toBeGreaterThan(explicit);
  expect(removed, JSON.stringify(events)).toBeGreaterThan(retired);
  return events;
}

/** Remove observation before owned-fixture cleanup can trigger unrelated closes. */
export async function disposeRelease(observation: ModalReleaseObservation | undefined): Promise<void> {
  if (observation) {
    await observation.evaluate((state) => state.dispose());
    await observation.dispose();
  }
}
