// M7 provisioning through the real authenticated helm REST surface.
//
// Host-side execution is the only injected layer. start-stack.sh points the
// helm at an E2E-only ProvisioningBackend whose control file this suite owns;
// routing, auth, one-use confirmation, registration, progress retention, and
// fleet-feed bumps remain production code. A few cases use narrower browser
// route seams for replies or connection transitions an executor cannot
// produce: unreadable successful bodies, progress-transport failure, and
// local connection-state changes without stopping the shared supervisor used
// by every other spec.

import { expect, test } from "./helpers/evidence";
import { Page, APIRequestContext, Locator, TestInfo } from "@playwright/test";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import {
  cleanupSession,
  createSession,
  hostRowByName,
  openHostMenu,
  openHostsPanel,
  showHostsPanel,
  patchPreferences,
  pinAutoSelect,
  readPreferences,
  stubFeed,
  type FeedStub,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { routeGate } from "./helpers/route-gate";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { disposeRelease, expectConnectedRelease, observeModalRelease, type ModalReleaseObservation } from "./helpers/modal-release";
import { expectKeyboardTooltip, expectQuietInitialFocus, expectCancelTooltipReturns } from "./helpers/host-dialog-tooltip";
import { expectKeyboardScroll, expectKeyboardScrollFromControl } from "./helpers/keyboard-scroll";
import { currentStackSlot } from "../stack-slots";
import { openSettings } from "./helpers/settings";

/** Live helm identity copied onto every route response this suite fabricates. */
let HELM_BUILD = "";

type Behavior = {
  probe?: "absent" | "supervisor" | "error";
  inspect?: "supported" | "manual" | "error";
  message?: string;
  build_version?: string;
  identity?: string | null;
  dial_farhelm?: string;
  dial_state_dir?: string | null;
  home?: string;
  user_unit_dir?: string;
  needs_tmux?: boolean;
  hold_actions?: boolean;
  fail_action?: string | null;
  action_delay_ms?: number;
};

type BackendConfig = {
  default?: Behavior;
  targets?: Record<string, Behavior>;
};

type BackendEvent = { event: string; target: string };

type Host = {
  id: number;
  kind: "local" | "ssh";
  destination: string | null;
  state: Record<string, unknown>;
};

type ProbeReply =
  | { result: "discovered"; host_id: number }
  | { result: "manual"; reason: string }
  | { result: "provisionable"; probe_id: string; confirmation: string; steps?: { lead: string; steps: { id: string; name: string; paths: { full: string; short: string }[] }[]; kept: string | null } };

type Accepted = { host_id: number; run_id: string };

type Progress = {
  run_id: string | null;
  operation: "add" | "update" | null;
  status: "running" | "completed" | "failed";
  steps: { step: string; status: string; message: string | null }[];
  message: string | null;
};

/** Mutable local-host facts snapshotted by each routed registry request. */
type InjectedLocalState = {
  down: boolean;
  lastError?: string;
};

type StackInfo = { provisioning_backend: string };

/** Read the backend path only after Playwright has started the stack. */
async function backendRoot(): Promise<string> {
  const raw = await readFile(currentStackSlot().stackInfoPath, "utf8");
  return (JSON.parse(raw) as StackInfo).provisioning_backend;
}

/** Atomically publish one backend behavior and reset its observable log. */
async function configureBackend(config: BackendConfig = {}): Promise<void> {
  const root = await backendRoot();
  const next = path.join(root, `config.${process.pid}.next`);
  await writeFile(next, `${JSON.stringify(config)}\n`, { mode: 0o600 });
  await rename(next, path.join(root, "config.json"));
  await writeFile(path.join(root, "events.jsonl"), "", { mode: 0o600 });
}

/** Read complete injected-executor events, tolerating an empty new log. */
async function backendEvents(): Promise<BackendEvent[]> {
  const root = await backendRoot();
  const raw = await readFile(path.join(root, "events.jsonl"), "utf8");
  return raw
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line) as BackendEvent);
}

/** A destination unique across both browser projects and every scenario. */
function destination(testInfo: TestInfo, suffix: string): string {
  return `e2e@${testInfo.project.name}-${suffix}.invalid`;
}

function target(destination: string): string {
  return `ssh:${destination}`;
}

async function responseBody(response: { text(): Promise<string>; status(): number }): Promise<string> {
  return `${response.status()}: ${await response.text()}`;
}

/** The whole real registry, with malformed or refused replies failing loudly. */
async function hosts(request: APIRequestContext): Promise<Host[]> {
  const response = await request.get("/api/hosts");
  expect(response.ok(), await responseBody(response)).toBe(true);
  return (await response.json()).hosts as Host[];
}

let baselineHostIds: ReadonlySet<number>;

/**
 * How long cleanup waits for a released run to let go of its host.
 *
 * A released run ends within milliseconds under the injected backend; the
 * bound is only there so one that never ends fails cleanup instead of hanging.
 */
const RUN_SETTLE_TIMEOUT_MS = 30_000;

/**
 * Remove one host, waiting first for any setup or update still running on it.
 *
 * Removal never waits for a run: while one holds the host, the helm refuses at
 * once with 409 "busy with a setup, update or uninstall" (`hosts::remove_host`, SPEC.md
 * "Waiting between operations on one host"). Cleanup reaches here right after
 * `configureBackend()` releases whatever action a test left held, and the run
 * behind it is still finishing at that moment. Deleting straight away lost
 * that race in a different handful of tests on every full run of this spec,
 * on both engines, each failing its cleanup with that 409 although its own
 * assertions had passed.
 *
 * So the busy refusal, and only that one, is retried until the run lets go.
 * The delete's own answer is the readiness oracle: a run records its final
 * status before it releases the host, so waiting for the progress view to
 * settle would still race the same lock. Any other refusal ends the wait and
 * is returned, an error from the request itself is rethrown, and a host still
 * busy when the wait ends is reported with its run's status, so a run that
 * never lets go still fails the test rather than passing as "eventually
 * removed". Returns `null` once the host is gone.
 */
async function removeHostOnceIdle(request: APIRequestContext, id: number): Promise<string | null> {
  // An object rather than a `let`: TypeScript does not see assignments made
  // inside the poll callback, and would narrow a `let` to its initial value.
  const last: { state: "removed" | "busy" | "refused" | "unasked"; body: string } = {
    state: "unasked",
    body: "",
  };
  try {
    await expect
      .poll(
        async () => {
          // Reset first, so a request that throws after an earlier busy
          // answer is rethrown below instead of reported as still busy.
          last.state = "unasked";
          const response = await request.delete(`/api/hosts/${id}`);
          last.body = response.ok() ? "" : await responseBody(response);
          last.state = response.ok()
            ? "removed"
            : response.status() === 409 && last.body.includes("busy with a setup, update or uninstall")
            ? "busy"
            : "refused";
          return last.state;
        },
        { timeout: RUN_SETTLE_TIMEOUT_MS },
      )
      .not.toBe("busy");
  } catch (error) {
    if (last.state !== "busy") throw error;
    let run = "unknown";
    try {
      run = (await progress(request, id)).status;
    } catch {
      // The run's status is diagnostic only; the busy refusal is the finding.
    }
    return `still busy when the wait ended (run status: ${run}): ${last.body}`;
  }
  return last.state === "refused" ? last.body : null;
}

/**
 * Restore the host registry this project inherited before provisioning ran.
 *
 * These tests exercise real registration, so a failed assertion can leave a
 * dialing actor behind even when the injected backend has already been reset.
 * Identity-based cleanup catches every row the suite created without assuming
 * that the test reached the point where it could record a destination or id.
 */
async function removeHostsBeyondBaseline(request: APIRequestContext): Promise<void> {
  const added = (await hosts(request)).filter((host) => !baselineHostIds.has(host.id));
  const failures: string[] = [];
  for (const host of added) {
    const refusal = await removeHostOnceIdle(request, host.id);
    if (refusal) failures.push(`${host.id}: ${refusal}`);
  }

  expect(failures, "every host registered by a provisioning test must be removable").toEqual([]);
  expect(
    (await hosts(request)).map((host) => host.id).sort((left, right) => left - right),
    "a provisioning test must leave the shared harness host registry unchanged",
  ).toEqual([...baselineHostIds].sort((left, right) => left - right));
}

async function hostFor(request: APIRequestContext, destination: string): Promise<Host> {
  await expect
    .poll(async () => (await hosts(request)).find((host) => host.destination === destination))
    .not.toBeUndefined();
  return (await hosts(request)).find((host) => host.destination === destination)!;
}

/** Ask the real ADD discovery handler for one plan. */
async function probe(request: APIRequestContext, destination: string): Promise<ProbeReply> {
  const response = await request.post("/api/hosts/probe", {
    data: { target: { kind: "ssh", destination } },
  });
  expect(response.ok(), await responseBody(response)).toBe(true);
  return (await response.json()) as ProbeReply;
}

/** Confirm one real ADD plan and return its durable host-scoped identity. */
async function startAdd(request: APIRequestContext, destination: string): Promise<Accepted> {
  const planned = await probe(request, destination);
  if (planned.result !== "provisionable") {
    throw new Error(`expected a plan for ${destination}, got ${JSON.stringify(planned)}`);
  }
  const response = await request.post("/api/hosts/provision", {
    data: { probe_id: planned.probe_id },
  });
  expect(response.status(), await responseBody(response)).toBe(202);
  return (await response.json()) as Accepted;
}

/**
 * Whether the update popup's current step name, count line, and clock line
 * are actually readable: every rectangle of each one's text lies inside the
 * popup's box, inside the step list's box for the step, and inside the
 * viewport. Measured on the text itself through a DOM `Range`, because the
 * elements' own `scrollWidth`/`clientWidth` read zero for inline spans and
 * so cannot tell clipped text from visible text.
 */
async function popupTextVisible(popup: Locator): Promise<{ current: boolean; count: boolean; elapsed: boolean }> {
  return await popup.evaluate((node) => {
    const within = (inner: DOMRect, outer: DOMRect) =>
      inner.left >= outer.left - 0.5 && inner.right <= outer.right + 0.5
      && inner.top >= outer.top - 0.5 && inner.bottom <= outer.bottom + 0.5;
    const viewport = new DOMRect(0, 0, window.innerWidth, window.innerHeight);
    const box = node.getBoundingClientRect();
    const readable = (element: Element | null, clip?: Element | null) => {
      if (!element) return false;
      const range = document.createRange();
      range.selectNodeContents(element);
      const rects = Array.from(range.getClientRects());
      const clipBox = clip ? clip.getBoundingClientRect() : box;
      return rects.length > 0
        && rects.every((rect) => within(rect, box) && within(rect, clipBox) && within(rect, viewport));
    };
    return {
      current: readable(node.querySelector(".host-update-popup-step"), node.querySelector(".host-update-popup-steps")),
      count: readable(node.firstElementChild),
      elapsed: readable(node.querySelector(".host-update-popup-elapsed")),
    };
  });
}

async function progress(request: APIRequestContext, host: number): Promise<Progress> {
  const response = await request.get(`/api/hosts/${host}/provisioning`);
  expect(response.ok(), await responseBody(response)).toBe(true);
  return (await response.json()) as Progress;
}

async function waitForProgress(
  request: APIRequestContext,
  host: number,
  status: Progress["status"],
): Promise<Progress> {
  await expect.poll(async () => (await progress(request, host)).status).toBe(status);
  return await progress(request, host);
}

/** Open the real add form and submit one destination with host details open.
 *
 * Most discovery tests inspect the expanded row after probing, so this helper
 * preserves that shared fixture state. Tests of folded disclosure open the
 * form directly.
 */
async function probeRemote(page: Page, destination: string): Promise<void> {
  await openHostsPanel(page);
  await page.locator(".host-rail-add").click();
  await page.locator(".add-host-ssh").fill(destination);
  await page.locator(".add-host-submit").click();
}

/**
 * Replace only the local row's connection state; every other host stays real.
 *
 * Most callers want a revision as soon as the feed opens. A test that needs an
 * exact notification boundary can suppress that greeting and send the first
 * revision itself.
 */
async function controlLocalState(
  page: Page,
  state: InjectedLocalState,
  options: { greetOnConnect?: boolean } = {},
): Promise<FeedStub> {
  const feed = await stubFeed(page);
  if (options.greetOnConnect !== false) feed.notifyOnConnect(1);
  await page.route("**/api/hosts", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    // A request belongs to the injected state generation at dispatch. Capture
    // it before fetching so a later test transition cannot rewrite a response
    // that was already in flight.
    const { down, lastError } = state;
    const response = await route.fetch();
    const body = await response.json();
    if (down) {
      const local = body.hosts.find((host: Host) => host.kind === "local");
      local.state = {
        phase: "unreachable-reprobing",
        cause: "local-supervisor-not-running",
        last_error:
          lastError ??
          "no supervisor is running; start it with `farhelm supervisor run --state-dir /tmp/fh-e2e-state`",
      };
    }
    await route.fulfill({ response, json: body });
  });
  return feed;
}

/** Publish only after the browser has established the injected feed socket. */
async function notifyFeed(feed: FeedStub, revision: number): Promise<void> {
  await expect.poll(() => feed.openSockets()).toBeGreaterThan(0);
  feed.notify(revision);
}

test.beforeAll(async ({ request }) => {
  const response = await request.get("/api/hosts");
  expect(response.ok(), await responseBody(response)).toBe(true);
  HELM_BUILD = requireHelmBuild(response, "provisioning fixtures");
  baselineHostIds = new Set(((await response.json()).hosts as Host[]).map((host) => host.id));
});

test.beforeEach(async () => {
  await configureBackend();
});

/** Rail actions keep their existing modal owner visible while management is
 * concealed. A real registered fixture and a live menu establish the origin
 * before each request; cancelling must return to that actual avatar.
 */
for (const action of ["setup", "uninstall", "refusal"] as const) {
  test(`rail ${action} modal stays visible over Sessions and returns to its avatar`, async ({ page, request }, testInfo) => {
    const remote = destination(testInfo, `hidden-${action}`);
    if (action === "setup") {
      await configureBackend({ targets: { [target(remote)]: { fail_action: "attach-supervisor" } } });
    }
    const accepted = await startAdd(request, remote);
    await waitForProgress(request, accepted.host_id, action === "setup" ? "failed" : "completed");
    await configureBackend();
    if (action !== "setup") {
      await page.route(`**/api/hosts/${accepted.host_id}/${action === "uninstall" ? "uninstall" : "update"}`, async (route) => {
        await route.fulfill(action === "refusal"
          ? { status: 409, headers: { "x-farhelm-build": HELM_BUILD }, body: "rail update refusal" }
          : { headers: { "x-farhelm-build": HELM_BUILD }, json: {
            probe_id: "rail-uninstall-plan", confirmation: "remove the fixture installation",
            plan: { state_dir: "/tmp/fixture-retained-state" },
          } });
      });
    }
    try {
      await page.goto("/");
      const avatar = page.locator(`.host-rail-avatar[data-rail-host-id="${accepted.host_id}"]`);
      await expect(avatar).toBeVisible();
      await expect(page.locator(".hosts-heading")).not.toBeVisible();
      await avatar.focus();
      await expect(avatar).toBeFocused();
      await page.keyboard.press("Shift+F10");
      const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
      const menu = row.locator(".host-row-menu-panel");
      await expect(menu).toBeVisible();
      await expect(row.locator(".host-row-main")).not.toBeVisible();
      await menu.locator(action === "setup" ? ".provisioning-rerun" : action === "uninstall" ? ".provisioning-uninstall" : ".provisioning-update").click();
      const modal = action === "refusal" ? page.locator(".host-refusal-dialog") : page.getByRole("dialog", {
        name: action === "setup" ? "host setup" : "uninstall Farhelm from this host", exact: true,
      });
      await expect(modal).toBeVisible();
      await expect(page.locator(".hosts-heading")).not.toBeVisible();
      if (action === "refusal") await expect(modal).toContainText("rail update refusal");
      await modal.getByRole("button", { name: action === "refusal" ? "close" : "cancel", exact: true }).click();
      await expect(modal).toHaveCount(0);
      await expect(avatar).toBeFocused();
      await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
    } finally {
      await page.unrouteAll({ behavior: "wait" });
    }
  });
}

/** Removing a rail host destroys its avatar; the persistent panel toggle is
 * the useful keyboard recovery, while cancelling keeps the originating host.
 */
test("rail removal returns to its avatar on cancel and Hosts on success", async ({ page, request }, testInfo) => {
  await patchPreferences(request, { skip_host_remove_confirmation: false });
  const accepted = await startAdd(request, destination(testInfo, "rail-remove"));
  await waitForProgress(request, accepted.host_id, "completed");
  await page.goto("/");
  const avatar = page.locator(`.host-rail-avatar[data-rail-host-id="${accepted.host_id}"]`);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  for (const confirm of [false, true]) {
    await expect(avatar).toBeVisible();
    await avatar.click({ button: "right" });
    await expect(row.locator(".host-row-menu-panel")).toBeVisible();
    await row.locator(".host-remove").click();
    const modal = page.locator(".host-remove-dialog");
    await expect(modal).toBeVisible();
    await modal.locator(confirm ? ".host-remove-confirm" : ".host-settings-close").click();
    await expect(modal).toHaveCount(0);
    if (confirm) {
      await expect(avatar).toHaveCount(0);
      await expect(page.locator(".hosts-panel-toggle")).toBeFocused();
    } else {
      await expect(avatar).toBeFocused();
    }
  }
});

/** A later menu opening can change both host and surface while an earlier
 * planning reply is held. The eventual refusal belongs to the accepted
 * action's origin, rather than the latest menu's trigger.
 */
for (const alternate of ["same host", "another host"] as const) {
  test(`held rail Update refusal keeps its avatar after ${alternate} panel menu opens`, async ({ page, request }, testInfo) => {
    const accepted = await startAdd(request, destination(testInfo, `origin-${alternate.replaceAll(" ", "-")}`));
    await waitForProgress(request, accepted.host_id, "completed");
    const gate = routeGate();
    let arrived = false;
    await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
      arrived = true;
      await gate.wait();
      await route.fulfill({ status: 409, headers: { "x-farhelm-build": HELM_BUILD }, body: "held origin refusal" });
    });
    try {
      await page.goto("/");
      const avatar = page.locator(`.host-rail-avatar[data-rail-host-id="${accepted.host_id}"]`);
      await expect(avatar).toBeVisible();
      await avatar.click({ button: "right" });
      const source = page.locator(`[data-host-id="${accepted.host_id}"]`);
      await expect(source.locator(".host-row-menu-panel")).toBeVisible();
      await source.locator(".provisioning-update").click();
      await expect.poll(() => arrived, { message: "the planning route is held before opening another trigger" }).toBe(true);
      expect((await progress(request, accepted.host_id)).status).toBe("completed");
      await showHostsPanel(page);
      const other = alternate === "same host" ? source : page.locator('.host-row[data-host-kind="local"]');
      await openHostMenu(other);
      await expect(other.locator(".host-row-menu-panel")).toBeVisible();
      expect(arrived, "the held route remains the source of the upcoming refusal").toBe(true);
      gate.release();
      const refusal = page.locator(".host-refusal-dialog");
      await expect(refusal).toBeVisible();
      await expect(refusal).toContainText("held origin refusal");
      await refusal.locator(".host-refusal-close").click();
      await expect(refusal).toHaveCount(0);
      await expect(avatar).toBeFocused();
    } finally {
      gate.release();
      await page.unrouteAll({ behavior: "wait" });
    }
  });
}

/** A planning request owns no mutation lock, but its eventual confirmation
 * will isolate the page. Settings must remain refused until that request
 * settles, or two independent focus traps can make both dialogs unusable. */
test("Settings waits while an uninstall confirmation is being planned", async ({ page, request }, testInfo) => {
  // Session questions are skipped, but an uninstall still owns its question.
  await patchPreferences(request, { skip_session_confirmations: true });
  const remote = destination(testInfo, "settings-plan");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const gate = routeGate();
  let planning = false;
  await page.route(`**/api/hosts/${accepted.host_id}/uninstall`, async (route) => {
    planning = true;
    await gate.wait();
    await route.fulfill({ headers: { "x-farhelm-build": HELM_BUILD }, json: { probe_id: "held-settings-plan", confirmation: "remove the fixture host's Farhelm installation", plan: { state_dir: "/tmp/fixture-retained-state" } } });
  });
  try {
    await page.goto("/");
    await openHostsPanel(page);
    const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
    const hosts = await (await request.get("/api/hosts")).json();
    const local = hosts.hosts.find((host: { kind: string }) => host.kind === "local");
    expect(local, "another host supplies an independent Settings entry point").toBeTruthy();
    const otherRow = page.locator(`[data-host-id="${local.id}"]`);
    await openHostMenu(otherRow);
    await otherRow.locator(".host-settings").click();
    await expect(page.locator(".host-settings-dialog")).toBeVisible();
    await page.locator(".host-settings-close").click();
    await openHostMenu(row);
    await row.locator(".provisioning-uninstall").click();
    await expect.poll(() => planning).toBe(true);
    await openHostMenu(row);
    const settings = row.locator(".host-settings");
    await expect(settings).toHaveAttribute("aria-disabled", "true");
    await settings.dispatchEvent("click");
    await expect(page.locator('.host-settings-dialog:not(.host-uninstall-dialog)')).toHaveCount(0);
    await openHostMenu(otherRow);
    await expect(otherRow.locator(".host-settings")).toHaveAttribute("aria-disabled", "true");
    await otherRow.locator(".host-settings").dispatchEvent("click");
    await expect(page.locator('.host-settings-dialog:not(.host-uninstall-dialog)')).toHaveCount(0);
    gate.release();
    const confirmation = page.locator(".host-uninstall-dialog");
    await expect(confirmation).toBeVisible();
    await expect(confirmation.locator(".uninstall-confirm")).toBeFocused();
    await confirmation.locator(".uninstall-cancel").click();
    await expect(confirmation).toHaveCount(0);
    await openHostMenu(row);
    await row.locator(".host-settings").click();
    await expect(page.locator('.host-settings-dialog:not(.host-uninstall-dialog)')).toBeVisible();
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
});

/** Delayed results wait for Settings and other hosts' confirmations; independent isolation must not overlap. */
for (const mode of ["plan", "refusal", "two plans"]) {
  test(`delayed host dialogs wait for Settings: ${mode}`, async ({ page, request }, testInfo) => {
    const gate = routeGate();
    const ids: number[] = [];
    const action = mode === "refusal" ? "update" : "uninstall";
    let arrived = 0;
    for (let index = 0; index < (mode === "two plans" ? 2 : 1); index++) {
      const accepted = await startAdd(request, destination(testInfo, `modal-${index}`));
      await waitForProgress(request, accepted.host_id, "completed");
      ids.push(accepted.host_id);
      await page.route(`**/api/hosts/${accepted.host_id}/${action}`, async (route) => {
        arrived++;
        await gate.wait();
        await route.fulfill(mode === "refusal"
          ? { status: 409, headers: { "x-farhelm-build": HELM_BUILD }, body: "held update refusal" }
          : { headers: { "x-farhelm-build": HELM_BUILD }, json: { probe_id: `held-modal-${index}`, confirmation: "remove fixture Farhelm", plan: { state_dir: "/tmp/fixture-state" } } });
      });
    }
    // Fake setup targets keep retrying SSH after setup completes. Their
    // changing detail lines legitimately dismiss lower menus; control those
    // unrelated notifications while the planning replies remain held below.
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    try {
      await page.goto("/");
      await showHostsPanel(page);
      await feed.waitForConnection(1);
      await openHostsPanel(page);
      for (const id of ids) {
        const row = page.locator(`[data-host-id="${id}"]`);
        await openHostMenu(row);
        await row.locator(`.provisioning-${action}`).click();
        await expect(row.locator('.host-update-pending[data-update-phase="planning"]')).toHaveCount(1);
      }
      await expect.poll(() => arrived).toBe(ids.length);
      await page.locator(".app-settings-toggle").click();
      const settings = page.locator(".app-settings-dialog");
      await expect.poll(() => settings.evaluate((dialog) => dialog.contains(document.activeElement))).toBe(true);
      gate.release();
      for (const id of ids) await expect(page.locator(`[data-host-id="${id}"] .host-update-pending[data-update-phase="planning"]`)).toHaveCount(0);
      const modal = page.locator(".host-uninstall-dialog, .host-refusal-dialog");
      await expect(modal).toHaveCount(0);
      await expect(page.locator('[role="dialog"][aria-modal="true"]')).toHaveCount(1);
      await settings.locator(".app-settings-close").first().click();
      for (const _ of ids) {
        await expect(modal).toHaveCount(1);
        const cancel = modal.locator(".uninstall-cancel, .host-refusal-close");
        await expect(modal.locator(".uninstall-confirm, .host-refusal-close")).toBeFocused();
        const owner = await modal.evaluate((dialog) => dialog.closest("[data-host-modal-admitted]")!.id);
        await cancel.click();
        await expect(page.locator(`[id="${owner}"]`)).toHaveCount(0);
      }
      await expect(page.locator('[role="dialog"][aria-modal="true"]')).toHaveCount(0);
    } finally {
      gate.release();
      await page.unrouteAll({ behavior: "ignoreErrors" });
    }
  });
}

for (const admission of ["visible", "waiting behind another host"] as const) {
  /**
   * Retargeting retires only the offer owned by that provisioning panel.
   * A waiting offer owns no isolation; retiring it must preserve the other
   * host's actual admitted modal, its native keyboard owner and its inert set.
   */
  test(`offer retirement releases only its owner: ${admission}`, async ({ page, request }, testInfo) => {
    const one = await startAdd(request, destination(testInfo, "release-retarget"));
    const two = await startAdd(request, destination(testInfo, "release-other"));
    await waitForProgress(request, one.host_id, "completed");
    await waitForProgress(request, two.host_id, "completed");
    const feed = await stubFeed(page);
    // Keep unrelated fake-host retry details from moving the menu during
    // admission. The explicit revision below still retires the owned offer.
    feed.notifyOnConnect(1);
    const gates = [routeGate(), routeGate()];
    const arrived = new Set<number>();
    let observation: ModalReleaseObservation | undefined;
    let bodyFailed = true;
    for (const [index, id] of [one.host_id, two.host_id].entries()) {
      await page.route(`**/api/hosts/${id}/uninstall`, async (route) => {
        expect(route.request().method()).toBe("POST");
        expect(route.request().postData(), "this test never confirms an uninstall").toBeNull();
        arrived.add(id);
        await gates[index].wait();
        // The fake remote cannot satisfy live uninstall preflight. Keep the
        // same bounded planning-reply seam as the delayed-admission cases.
        await route.fulfill({ headers: { "x-farhelm-build": HELM_BUILD }, json: {
          probe_id: `release-offer-${id}`,
          confirmation: "uninstall the fixture host's Farhelm",
          plan: { state_dir: "/tmp/fixture-retained-state" },
        } });
      });
    }
    try {
      await page.goto("/");
      await showHostsPanel(page);
      await feed.waitForConnection(1);
      await openHostsPanel(page);
      const source = page.locator(`[data-provisioning-host="${one.host_id}"]`);
      for (const id of admission === "visible" ? [one.host_id] : [one.host_id, two.host_id]) {
        const row = page.locator(`[data-host-id="${id}"]`);
        await openHostMenu(row);
        await row.locator(".provisioning-uninstall").click();
        await expect(row.locator('.host-update-pending[data-update-phase="planning"]')).toHaveCount(1);
        await expect.poll(() => arrived.has(id)).toBe(true);
      }
      const owner = admission === "visible" ? one.host_id : two.host_id;
      gates[admission === "visible" ? 0 : 1].release();
      const dialog = page.locator(`[data-provisioning-host="${owner}"] .host-uninstall-dialog`);
      const confirming = dialog.locator(".uninstall-confirm");
      await expect(confirming).toBeFocused();
      const backgroundSelector = `[data-host-id="${one.host_id}"] .host-row-menu`;
      observation = await observeModalRelease(dialog, backgroundSelector, backgroundSelector);
      if (admission !== "visible") {
        gates[0].release();
        // A real waiting HostModal slot proves the reply became an offer,
        // rather than merely reaching the browser before it was decoded.
        await expect(source.locator('[id^="host-modal-"]:not([data-host-modal-admitted])')).toHaveCount(1);
        await expect(source.locator(".host-uninstall-dialog")).toHaveCount(0);
      }
      const changed = `${destination(testInfo, "release-retarget")}-moved`;
      const retargeted = await request.post(`/api/hosts/${one.host_id}/destination`, { data: { ssh: changed } });
      expect(retargeted.ok(), await responseBody(retargeted)).toBe(true);
      expect((await hosts(request)).find((host) => host.id === one.host_id)?.destination).toBe(changed);
      await notifyFeed(feed, 2);
      await expect(source.locator('[id^="host-modal-"]'), "the owned stale offer must retire").toHaveCount(0);
      if (admission === "visible") {
        await expectConnectedRelease(observation);
        expect(await page.locator(backgroundSelector).evaluate((node) => node.closest("[inert]") !== null)).toBe(false);
        expect((await observation.evaluate(({ events }) => events)).some((event) => event.kind === "focus"),
          "retirement does not return to the old row").toBe(false);
      } else {
        await expect(dialog).toBeVisible();
        await expect(confirming).toBeFocused();
        expect(await page.locator(backgroundSelector).evaluate((node) => node.closest("[inert]") !== null)).toBe(true);
        expect(await dialog.evaluate((node) => (node as HTMLElement & { __farhelmModalIsolated?: boolean }).__farhelmModalIsolated)).toBe(true);
        expect(await observation.evaluate(({ events }) => events),
          "another host's retirement must not even address the visible release hook").toEqual([]);
        // Native containment must remain live, not only the installed marker.
        const close = dialog.locator(".host-settings-dismiss");
        await close.focus();
        await expect(close).toBeFocused();
        await page.keyboard.press("Shift+Tab");
        await expect(confirming).toBeFocused();
        await page.keyboard.press("Escape");
        await expect(dialog).toHaveCount(0);
        await expectConnectedRelease(observation);
        await expect(page.locator(".host-uninstall-dialog")).toHaveCount(0);
      }
      bodyFailed = false;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["held plans", () => gates.forEach((gate) => gate.release())],
        ["plan routes", () => page.unrouteAll({ behavior: "wait" })],
        ["release observation", () => disposeRelease(observation)],
      ]);
    }
  });
}

for (const reason of ["Remove success", "authoritative Settings disappearance"] as const) {
  /**
   * A host retirement releases its connected root and owes no old-row return.
   * The newer owner is an actual enabled approval answer: an inert background
   * button cannot make this preservation assertion pass vacuously.
   */
  test(`host retirement releases before removal and preserves approval focus: ${reason}`, async ({ page, request }, testInfo) => {
    const remote = destination(testInfo, `release-${reason === "Remove success" ? "delete" : "gone"}`);
    const accepted = await startAdd(request, remote);
    await waitForProgress(request, accepted.host_id, "completed");
    const previous = await readPreferences(request);
    let session: Awaited<ReturnType<typeof createSession>> | undefined;
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    const gate = routeGate();
    let deleteArrived = false;
    let waiting = false;
    const answers: string[] = [];
    const approvalId = `release-newer-owner-${Date.now()}`;
    let observation: ModalReleaseObservation | undefined;
    let bodyFailed = true;
    try {
      const requester = await createSession(request, { title: `release-approval-${Date.now()}` });
      session = requester;
      await patchPreferences(request, { skip_host_remove_confirmation: false });
      await page.route((url) => url.pathname === "/api/approvals", (route) => route.fulfill({
        headers: { "x-farhelm-build": HELM_BUILD }, json: { approvals: waiting ? [{
          id: approvalId, host_id: 1, host_name: "fixture-host",
          session: { id: requester.id, title: requester.title, host_name: "fixture-host" },
          expires_at_ms: Date.now() + 540_000,
          action: {
            kind: "launch", verb: "create", host_name: "fixture-host", cwd: "/work/project",
            title: null, source: null,
            launch: { kind: "agent", selection: { harness: "claude", model: "opus", effort: "high", permissions: "guarded" },
              start: ["claude", "--model", "opus", "--effort", "high", "--permission-mode", "auto", "{farhelm_args}"], resume: null },
          },
        }] : [] },
      }));
      await page.route((url) => url.pathname === `/api/approvals/${approvalId}`, async (route) => {
        expect(route.request().method()).toBe("POST");
        answers.push(route.request().postDataJSON().answer);
        waiting = false;
        await route.fulfill({ status: 204, headers: { "x-farhelm-build": HELM_BUILD } });
      });
      if (reason === "Remove success") {
        await page.route(`**/api/hosts/${accepted.host_id}`, async (route) => {
          if (route.request().method() !== "DELETE") return route.continue();
          deleteArrived = true;
          await gate.wait();
          const response = await route.fetch();
          expect(response.ok(), await response.text()).toBe(true);
          await route.fulfill({ response });
        });
      }
      await pinAutoSelect(page, requester.id);
      await page.goto("/");
      await showHostsPanel(page);
      await feed.waitForConnection(1);
      await openHostsPanel(page);
      const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
      await expect(row).toBeVisible();
      await openHostMenu(row);
      await row.locator(reason === "Remove success" ? ".host-remove" : ".host-settings").click();
      const dialog = page.locator(reason === "Remove success" ? '.host-remove-dialog[role="dialog"]' : '.host-settings-refreshed[role="dialog"]');
      await expect(dialog).toBeVisible();
      observation = await observeModalRelease(dialog, ".add-host-button", ".add-host-button");
      if (reason === "Remove success") {
        await expect(dialog.locator(".host-remove-confirm")).toBeFocused();
        await page.keyboard.press("Enter");
        await expect.poll(() => deleteArrived).toBe(true);
        await expect(dialog.locator(".host-settings-close")).toBeDisabled();
        expect((await hosts(request)).some((host) => host.id === accepted.host_id),
          "the DELETE is held before the real handler").toBe(true);
      }
      waiting = true;
      await notifyFeed(feed, 2);
      const card = page.locator(`.approval-card[data-approval-id="${approvalId}"]`);
      const answer = card.getByRole("button", { name: "allow", exact: true });
      await expect(answer).toBeVisible();
      await expect(answer).toBeEnabled();
      expect(await answer.evaluate((node) => node.closest("[data-modal-exempt]") !== null)).toBe(true);
      expect(await answer.evaluate((node) => node.closest("[inert]") !== null)).toBe(false);
      await answer.focus();
      await expect(answer).toBeFocused();
      if (reason === "Remove success") gate.release();
      else {
        const removed = await request.delete(`/api/hosts/${accepted.host_id}`);
        expect(removed.ok(), await responseBody(removed)).toBe(true);
        await notifyFeed(feed, 3);
      }
      await expect(dialog).toHaveCount(0);
      await expect(row).toHaveCount(0);
      await expect(answer, "retirement must leave the newer valid owner alone").toBeFocused();
      await expectConnectedRelease(observation);
      expect(await page.locator(".add-host-button").evaluate((node) => node.closest("[inert]") !== null)).toBe(false);
      expect(answers, "preserving focus must not answer an approval").toEqual([]);
      expect((await hosts(request)).some((host) => host.id === accepted.host_id)).toBe(false);
      bodyFailed = false;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["held DELETE", () => gate.release()],
        ["owned routes", () => page.unrouteAll({ behavior: "wait" })],
        ["release observation", () => disposeRelease(observation)],
        ["remove preference", () => patchPreferences(request, { skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null })],
        ["approval requester", () => session && cleanupSession(request, session.id)],
      ]);
    }
  });
}

/** Update planning has not acquired the page lock and opens no confirmation.
 * It must exclude its own Settings, including a dispatched click, without
 * blocking another host's dialog. Hold the actual planning request so the
 * assertions cannot accidentally observe the later tracked-run busy flag. */
test("Update planning keeps only its own Settings unavailable", async ({ page, request }, testInfo) => {
  const remote = destination(testInfo, "settings-update-plan");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const gate = routeGate();
  let planning = false;
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (route.request().postData()) {
      await route.continue();
      return;
    }
    planning = true;
    await gate.wait();
    // Released only at teardown. Failing the held plan, rather than letting
    // it through, keeps the page from claiming it and submitting a real
    // Update while the test shuts down, as the uninstall test cancels its own.
    await route.abort().catch(() => {});
  });
  try {
    await page.goto("/");
    await openHostsPanel(page);
    const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
    const local = (await hosts(request)).find((host) => host.kind === "local");
    expect(local, "an independent Settings entry point exists before Update").toBeTruthy();
    const otherRow = page.locator(`[data-host-id="${local!.id}"]`);
    await openHostMenu(row);
    await row.locator(".provisioning-update").click();
    await expect.poll(() => planning).toBe(true);
    expect((await progress(request, accepted.host_id)).status, "the held request has not started Update").toBe("completed");
    await openHostMenu(row);
    await expect(row.locator(".host-settings")).toHaveAttribute("aria-disabled", "true");
    await row.locator(".host-settings").dispatchEvent("click");
    await expect(page.locator(".host-settings-dialog")).toHaveCount(0);
    await openHostMenu(otherRow);
    await expect(otherRow.locator(".host-settings")).not.toHaveAttribute("aria-disabled", "true");
    await otherRow.locator(".host-settings").click();
    await expect(page.locator(".host-settings-dialog")).toBeVisible();
    await page.locator(".host-settings-close").click();
  } finally {
    gate.release();
    await page.unrouteAll({ behavior: "wait" });
  }
});

/** Planning refusals leave the narrow row and retain the helm's typed remedy.
 * The Remove button must enter the ordinary confirmation, never remove a host
 * from the refusal dialog itself. */
test("uninstall and Update planning refusals open framed dialogs", async ({ page, request }, testInfo) => {
  const remote = destination(testInfo, "action-error-update");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  for (const [action, body] of [["uninstall", "old uninstall refusal"], ["update", "new update refusal"]]) {
    await page.route(`**/api/hosts/${accepted.host_id}/${action}`, async (route) => {
      await route.fulfill({
        status: 409,
        headers: {
          "x-farhelm-build": HELM_BUILD,
          "content-type": "text/plain",
          ...(action === "uninstall" ? { "x-farhelm-uninstall-remedy": "remove-host" } : {}),
        },
        body,
      });
    });
  }
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-uninstall").click();
  const refusal = page.locator(".host-refusal-dialog");
  await expect(refusal).toContainText("old uninstall refusal");
  await expect(refusal.locator(".host-refusal-close")).toBeFocused();
  await expect(row.locator(".provisioning-error")).toHaveCount(0);
  await refusal.locator(".host-refusal-remove").click();
  await expect(page.locator(".host-remove-dialog")).toBeVisible();
  await page.locator(".host-remove-dialog .host-settings-close").click();
  await openHostMenu(row);
  await row.locator(".provisioning-update").click();
  await expect(refusal).toContainText("new update refusal");
  await expect(refusal.locator(".host-refusal-remove")).toHaveCount(0);
  await expect(row.locator(".provisioning-error")).toHaveCount(0);
});

/** Two planning refusals can arrive together from Update all. Dismissing one
 * must hand modal focus and background isolation to the next refusal before
 * the user can send keys to the hosts panel. */
test("queued Update refusals retain modal focus and isolation", async ({ page, request }, testInfo) => {
  const first = destination(testInfo, "queued-refusal-one");
  const second = destination(testInfo, "queued-refusal-two");
  const one = await startAdd(request, first);
  const two = await startAdd(request, second);
  await waitForProgress(request, one.host_id, "completed");
  await waitForProgress(request, two.host_id, "completed");
  const refused = new Set<number>();
  for (const [id, reason] of [[one.host_id, "first update refused"], [two.host_id, "second update refused"]] as const) {
    await page.route(`**/api/hosts/${id}/update`, async (route) => {
      if (route.request().postData()) return route.continue();
      refused.add(id);
      await route.fulfill({
        status: 409,
        headers: { "x-farhelm-build": HELM_BUILD, "content-type": "text/plain" },
        body: reason,
      });
    });
  }
  await page.goto("/");
  await showHostsPanel(page);
  await page.getByRole("button", { name: "update all" }).click();
  await expect.poll(() => refused.size).toBe(2);
  const refusal = page.locator(".host-refusal-dialog");
  await expect(refusal).toBeVisible();
  const firstShown = await refusal.locator(".host-refusal-reason").textContent();
  expect(["first update refused", "second update refused"]).toContain(firstShown?.trim());
  await refusal.locator(".host-refusal-close").click();
  await expect(refusal.locator(".host-refusal-reason")).not.toHaveText(firstShown ?? "");
  await expect(refusal.locator(".host-refusal-close")).toBeFocused();
  const backgroundIsolated = await page.locator(`[data-host-id="${one.host_id}"]`).evaluate(
    (row) => !!row.closest("[inert]"),
  );
  expect(backgroundIsolated, "the second modal still isolates the host list").toBe(true);
  await page.keyboard.press("Escape");
  await expect(refusal).toHaveCount(0);
});

/** A one-use plan can expire after the user has read it. Its definite POST
 * refusal must reopen a dialog, while the provisional uninstall notice is
 * withdrawn because no run was accepted. */
test("confirmed uninstall refusal reopens its dialog", async ({ page, request }, testInfo) => {
  const remote = destination(testInfo, "uninstall-submit-refusal");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await page.route(`**/api/hosts/${accepted.host_id}/uninstall`, async (route) => {
    if (route.request().postData()) {
      await route.fulfill({
        status: 409,
        headers: { "x-farhelm-build": HELM_BUILD, "content-type": "text/plain" },
        body: "the provisioning plan has expired",
      });
    } else {
      // This fixture's simulated remote cannot answer the helm's live
      // connectivity recheck. The planning reply is the premise for the
      // confirm-time refusal, which is the boundary this test exercises.
      await route.fulfill({
        status: 200,
        headers: { "x-farhelm-build": HELM_BUILD, "content-type": "application/json" },
        json: {
          probe_id: "expiring-uninstall-plan",
          confirmation: `Farhelm will uninstall from ${remote}.`,
          plan: { state_dir: "/home/farhelm-e2e/.local/state/farhelm" },
        },
      });
    }
  });
  await page.goto("/");
  await showHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-uninstall").click();
  await expect(page.locator(".host-uninstall-dialog .provisioning-plan")).toBeVisible();
  await page.locator(".host-uninstall-dialog .uninstall-confirm").click();
  const refusal = page.locator(".host-refusal-dialog");
  await expect(refusal).toContainText("the provisioning plan has expired");
  await expect(refusal.locator(".host-refusal-close")).toBeFocused();
  await expect(row.locator(".provisioning-error")).toHaveCount(0);
  await expect(page.locator(".uninstall-notice")).toHaveCount(0);
});

/** A remote host can vanish from another window while its confirmed POST is
 * pending. The definite refusal still belongs to the clicked host, even
 * though the row that submitted it has unmounted. */
test("confirmed uninstall refusal survives a vanished host row", async ({ page, request }, testInfo) => {
  const remote = destination(testInfo, "vanished-uninstall-refusal");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const feed = await stubFeed(page);
  const replyGate = routeGate();
  let hideHost = false;
  let postArrived = false;
  await page.route("**/api/hosts", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const response = await route.fetch();
    const body = await response.json();
    if (hideHost) body.hosts = body.hosts.filter((host: Host) => host.id !== accepted.host_id);
    await route.fulfill({ response, json: body });
  });
  await page.route(`**/api/hosts/${accepted.host_id}/uninstall`, async (route) => {
    if (!route.request().postData()) {
      // The fake remote cannot satisfy the live uninstall preflight; this
      // reply establishes a confirmed plan before the host disappears.
      await route.fulfill({
        status: 200,
        headers: { "x-farhelm-build": HELM_BUILD, "content-type": "application/json" },
        json: {
          probe_id: "vanishing-host-plan",
          confirmation: `Farhelm will uninstall from ${remote}.`,
          plan: { state_dir: "/home/farhelm-e2e/.local/state/farhelm" },
        },
      });
      return;
    }
    postArrived = true;
    await replyGate.wait();
    await route.fulfill({
      status: 409,
      headers: { "x-farhelm-build": HELM_BUILD, "content-type": "text/plain" },
      body: "the host disappeared before uninstall began",
    });
  });
  try {
    await page.goto("/");
    await showHostsPanel(page);
    await feed.waitForConnection(1);
    const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
    await expect(row, "the host exists before confirmation").toBeVisible();
    await openHostMenu(row);
    await row.locator(".provisioning-uninstall").click();
    await expect(page.locator(".host-uninstall-dialog .provisioning-plan")).toBeVisible();
    await page.locator(".host-uninstall-dialog .uninstall-confirm").click();
    await expect.poll(() => postArrived).toBe(true);
    hideHost = true;
    await notifyFeed(feed, 2);
    await expect(row, "the row has unmounted before its POST reply").toHaveCount(0);
    replyGate.release();
    const refusal = page.locator(".host-refusal-dialog");
    await expect(refusal).toContainText("the host disappeared before uninstall began");
    await expect(refusal).toContainText(remote);
    await expect(refusal.locator(".host-refusal-close")).toBeFocused();
  } finally {
    replyGate.release();
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
});

/** An open, unscrollable popup must follow progress and retain warning detail.
 * A controlled long step list makes the next step start outside its clip;
 * the geometry oracle distinguishes scrolling from text repaint. A degraded
 * prior step must keep the helm's reason when the row is folded. */
test("an open Update popup follows the new current step in a short viewport", async ({ page, request }, testInfo) => {
  const remote = destination(testInfo, "popup-follow");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  await page.goto("/");
  await showHostsPanel(page);
  await feed.waitForConnection(1);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").click();
  await expect(row.locator(".host-update-step")).toHaveText(" create directories");
  const live = await progress(request, accepted.host_id);
  expect(live.status).toBe("running");
  expect(live.operation).toBe("update");
  let current = 0;
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    await route.fulfill({ headers: { "x-farhelm-build": HELM_BUILD }, json: {
      ...live,
      steps: Array.from({ length: 16 }, (_, index) => ({
        step: `fixture-step-${index}`,
        status: index === current ? "running" : index === 14 && current === 15 ? "degraded" : index < current ? "completed" : "pending",
        message: index === 14 && current === 15 ? "Lingering denied; supervisor starts at login instead of boot." : null,
      })),
    } });
  });
  feed.notify(1);
  await expect(row.locator(".host-update-step")).toHaveText(" fixture-step-0");
  await page.setViewportSize({ width: 1280, height: 300 });
  await row.locator(".host-update-count").hover();
  const popup = row.locator(".host-update-popup");
  await expect(popup).toBeVisible();
  await expect.poll(() => popupTextVisible(popup)).toEqual({ current: true, count: true, elapsed: true });
  expect(await popup.locator(".host-update-popup-steps").evaluate((node) => node.scrollHeight > node.clientHeight), "the step list must actually be clipped").toBe(true);
  current = 15;
  feed.notify(2);
  await expect(popup.locator(".host-update-popup-step")).toHaveText("fixture-step-15");
  await expect(popup.locator('.provisioning-step[data-status="degraded"] .provisioning-step-message'))
    .toHaveText("Lingering denied; supervisor starts at login instead of boot.");
  await expect.poll(() => popupTextVisible(popup), { message: "the already-open popup must scroll to step 15" }).toEqual({ current: true, count: true, elapsed: true });
});

test.afterEach(async ({ request }) => {
  // A test that failed while `holdLockWithAdd` still held its setup POST
  // must not let that POST reach the helm later. WebKit forwards a held
  // request when the context closes, which registered the lock host after
  // the cleanup below had already run and leaked it into every later spec
  // sharing this helm. Abort it here, and wait for the abort to land,
  // before the registry is checked.
  await Promise.all(heldAddConfirms.splice(0).map((abort) => abort()));
  // Release an action held by a failed assertion so it cannot poison the
  // next serial test or the second browser project.
  await configureBackend();
  await patchPreferences(request, { skip_host_setup_confirmation: null, skip_session_confirmations: null });
  await removeHostsBeyondBaseline(request);
});

/**
 * Compare painted answer geometry, rather than inferring layout from classes.
 * The desktop row must share its baseline; on a phone every answer must span
 * the footer in DOM order (Cancel first), and the permanent hint must follow
 * the whole stack. Visual and keyboard answer order must agree at both widths.
 */
async function expectHostAnswers(dialog: Locator, labels: string[], phone: boolean): Promise<void> {
  const answers = dialog.locator(".host-dialog-answers");
  await expect(answers.getByRole("button")).toHaveText(labels);
  const measured = await answers.evaluate((element) => {
    const box = element.getBoundingClientRect();
    return {
      left: box.left,
      width: box.width,
      buttons: Array.from(element.querySelectorAll("button"), (button) => {
        const rect = button.getBoundingClientRect();
        return {
          left: rect.left, top: rect.top, bottom: rect.bottom, width: rect.width, height: rect.height,
          background: getComputedStyle(button).backgroundColor,
        };
      }),
    };
  });
  const buttons = measured.buttons;
  expect(Math.max(...buttons.map((b) => b.height)) - Math.min(...buttons.map((b) => b.height))).toBeLessThan(1);
  if (labels.length === 3) {
    expect(buttons[1].background, "permanent answer stays outlined rather than drawing the reflex click").toBe("rgba(0, 0, 0, 0)");
    expect(buttons[2].background, "one-off action retains its fill").not.toBe("rgba(0, 0, 0, 0)");
  }
  if (phone) {
    for (const button of buttons) {
      expect(Math.abs(button.width - measured.width)).toBeLessThan(1);
      expect(Math.abs(button.left - measured.left)).toBeLessThan(1);
    }
    for (let i = 1; i < buttons.length; i++) expect(buttons[i].top).toBeGreaterThan(buttons[i - 1].bottom);
  } else {
    expect(Math.max(...buttons.map((b) => b.bottom)) - Math.min(...buttons.map((b) => b.bottom))).toBeLessThan(1);
    for (let i = 1; i < buttons.length; i++) expect(buttons[i].left).toBeGreaterThan(buttons[i - 1].left);
  }
  const hint = dialog.locator(".host-dialog-permanent-hint");
  if (await hint.count()) {
    const hintBox = (await hint.boundingBox())!;
    if (phone) expect(hintBox.y).toBeGreaterThan(Math.max(...buttons.map((b) => b.bottom)));
    else expect(hintBox.y + hintBox.height).toBeLessThan(Math.min(...buttons.map((b) => b.top)));
  }
}

/**
 * The numbered plan still copies the helm's exact text, including its final
 * newline. Short path chips must stay inside the step column, even when the
 * full host path is much longer than the available dialog width.
 */
async function expectPlanCopyAndRail(page: Page, plan: Locator, confirmation: string): Promise<void> {
  expect(confirmation.endsWith("\n"), "fixture plan retains the helm's terminator").toBe(true);
  await page.evaluate(() => {
    (window as any).__planCopies = [];
    (window as any).__farhelmNativeClipboardWrite = (value: string) => {
      (window as any).__planCopies.push(value);
    };
  });
  await plan.locator(".provisioning-copy-plan").click();
  await expect.poll(() => page.evaluate(() => (window as any).__planCopies),
    { message: "Copy plan sends the exact helm confirmation" }).toEqual([confirmation]);
  const result = await plan.evaluate((element) => {
    const chip = element.querySelector(".provisioning-plan-path")!;
    const detail = chip.closest(".provisioning-plan-detail")!;
    const number = detail.parentElement!.querySelector(".provisioning-plan-number")!;
    const chipBox = chip.getBoundingClientRect();
    const detailBox = detail.getBoundingClientRect();
    return { chipLeft: chipBox.left, chipRight: chipBox.right, detailLeft: detailBox.left, detailRight: detailBox.right, numberRight: number.getBoundingClientRect().right };
  });
  expect(result.detailLeft, "the step's details stay to the right of its number").toBeGreaterThan(result.numberRight);
  expect(result.chipLeft, "the chip starts within the step column").toBeGreaterThanOrEqual(result.detailLeft - 1);
  expect(result.chipRight, "the chip fits within the step column").toBeLessThanOrEqual(result.detailRight + 1);
}

for (const operation of ["setup", "uninstall"] as const) {
  /**
   * Older planning replies have no structured Copy Plan control. Their exact
   * plain text still needs a named body stop and native scrolling, while the
   * same renderers omit that stop when a structured plan supplies Copy entry.
   * These offers are cancelled, never used as authority for a real mutation.
   */
  test(`unstructured ${operation} plan keeps a named keyboard entry`, async ({ page, request }, testInfo) => {
    const remote = destination(testInfo, `unstructured-${operation}`);
    const confirmation = Array.from({ length: 45 }, (_, index) =>
      `Plan line ${index + 1}: inspect the complete host changes before deciding.`).join("\n") + "\n";
    let plans = 0;
    let confirmations = 0;
    const accepted = operation === "uninstall" ? await startAdd(request, remote) : undefined;
    if (accepted) await waitForProgress(request, accepted.host_id, "completed");
    if (operation === "setup") {
      await page.route("**/api/hosts/probe", async (route) => {
        expect(route.request().method()).toBe("POST");
        expect(route.request().postDataJSON()).toMatchObject({ target: { kind: "ssh", destination: remote } });
        plans++;
        await route.fulfill({ headers: { "x-farhelm-build": HELM_BUILD }, json: {
          result: "provisionable", probe_id: "unstructured-setup-plan", confirmation,
        } });
      });
      await page.route("**/api/hosts/provision", async (route) => {
        confirmations++;
        await route.fulfill({ status: 409, headers: { "x-farhelm-build": HELM_BUILD }, body: "this plan must only be cancelled" });
      });
    } else {
      await page.route(`**/api/hosts/${accepted!.host_id}/uninstall`, async (route) => {
        expect(route.request().method()).toBe("POST");
        if (route.request().postData()) {
          confirmations++;
          await route.fulfill({ status: 409, headers: { "x-farhelm-build": HELM_BUILD }, body: "this plan must only be cancelled" });
          return;
        }
        plans++;
        // As in the existing uninstall fixtures, the fake remote cannot
        // perform the live preflight. Only its read-only planning reply is owned.
        await route.fulfill({ headers: { "x-farhelm-build": HELM_BUILD }, json: {
          probe_id: "unstructured-uninstall-plan", confirmation, plan: { state_dir: "/tmp/fixture-retained-state" },
        } });
      });
    }
    try {
      await patchPreferences(request, { skip_host_setup_confirmation: null });
      await page.setViewportSize({ width: 390, height: 420 });
      await page.goto("/");
      await showHostsPanel(page);
      if (operation === "setup") await probeRemote(page, remote);
      else {
        await openHostsPanel(page);
        const row = page.locator(`[data-host-id="${accepted!.host_id}"]`);
        await expect(row).toBeVisible();
        await openHostMenu(row);
        await row.locator(".provisioning-uninstall").click();
      }
      const dialog = page.locator(operation === "setup" ? ".host-add-dialog" : ".host-uninstall-dialog");
      const body = dialog.locator(".host-settings-body");
      await expect(dialog).toBeVisible();
      await expect.poll(() => plans).toBe(1);
      expect(await dialog.locator(".provisioning-plan").textContent(),
        "the plain renderer preserves every line and the final LF").toBe(confirmation);
      await expect(body.locator(".provisioning-copy-plan")).toHaveCount(0);
      await expect(body.locator("button, input, select, textarea, a[href], summary")).toHaveCount(0);
      await expect(body).toHaveAccessibleName(`host ${operation} plan`);
      await expect(body).toHaveAttribute("tabindex", "0");
      await expectKeyboardScroll(page, dialog.locator(".host-settings-dismiss"), body);
      await dialog.locator(".host-settings-dismiss").click();
      await expect(dialog).toHaveCount(0);
      expect(confirmations, "reading and cancelling supplies no confirmation authority").toBe(0);
      if (operation === "setup") expect((await hosts(request)).some((host) => host.destination === remote)).toBe(false);
    } finally {
      // All offers are read-only, and route teardown precedes the spec's
      // backend/registry cleanup so no late browser request can reach it.
      await page.unrouteAll({ behavior: "wait" });
    }
  });
}

for (const phone of [false, true]) {
  /**
   * Keyboard submission must keep the same dialog width and land on Set up,
   * while the plan remains readable and copyable at both supported layouts.
   * Initial focus keeps its ring without a covering tooltip, but subsequent
   * hover and keyboard navigation must still explain the answers.
   * Dismissing through the new header must retire the offer without a write.
   * A short viewport also proves keyboard access through the plan's Copy
   * control, without adding a redundant stop to its enclosing scrollport.
   * Header controls must also scroll the whole card when fixed chrome cannot fit.
   */
  test(`host-setup-layout ${phone ? "phone" : "desktop"} keeps answers and exact plan text`, async ({ page, request }, testInfo) => {
    const remote = destination(testInfo, `layout-${phone ? "phone" : "desktop"}`);
    await configureBackend({ targets: { [target(remote)]: { home: `/home/${"long-path-".repeat(8)}` } } });
    await patchPreferences(request, { skip_host_setup_confirmation: null, skip_session_confirmations: true });
    await page.setViewportSize({ width: phone ? 390 : 1100, height: 850 });
    await page.goto("/");
    await openHostsPanel(page);
    await page.locator(".host-rail-add").click();
    const dialog = page.getByRole("dialog", { name: "add host", exact: true });
    await expect(dialog.getByRole("heading", { name: "add host", exact: true })).toBeVisible();
    await expectHostAnswers(dialog, ["cancel", "add"], phone);
    await testInfo.attach(`host-add-${phone ? "phone" : "desktop"}`, { body: await dialog.screenshot(), contentType: "image/png" });
    const before = (await dialog.boundingBox())!;
    await expectKeyboardTooltip(page, dialog.getByRole("button", { name: "cancel", exact: true }));
    const reply = page.waitForResponse((response) => response.url().endsWith("/api/hosts/probe") && response.request().method() === "POST");
    await page.locator(".add-host-ssh").fill(remote);
    await expect(page.locator(".add-host-ssh")).toBeFocused();
    await page.mouse.move(0, 0);
    await page.locator(".add-host-ssh").press("Enter");
    const planned = await (await reply).json() as ProbeReply;
    expect(planned.result, "real discovery must produce the plan this test measures").toBe("provisionable");
    if (planned.result !== "provisionable") throw new Error("fixture did not offer setup");
    await expect(dialog.getByRole("heading")).toHaveText(`set up host · ${remote}`);
    await expect(dialog.getByRole("button", { name: "set up", exact: true })).toBeFocused();
    await expectQuietInitialFocus(page, dialog.getByRole("button", { name: "set up", exact: true }));
    expect((await dialog.boundingBox())!.width).toBe(before.width);
    await expectHostAnswers(dialog, ["cancel", "set up, don't ask again", "set up"], phone);
    const plan = dialog.locator(".provisioning-plan");
    expect(planned.steps, "the real helm sends the ordered step descriptions").toBeDefined();
    await expect(plan.locator(".provisioning-plan-step")).toHaveCount(planned.steps!.steps.length);
    await expect(plan.locator(".provisioning-plan-step-name")).toHaveText(planned.steps!.steps.map((step) => step.name));
    await expect(plan.locator(".provisioning-plan-path").first()).toHaveAttribute("data-tooltip", planned.steps!.steps[0].paths[0].full);
    // Host paths may contain repeated ordinary spaces, which must remain
    // visible as separate characters instead of naming a different path.
    expect(await plan.locator(".provisioning-plan-path").first().evaluate((node) => getComputedStyle(node).whiteSpace)).toBe("pre-wrap");
    await expectPlanCopyAndRail(page, plan, planned.confirmation);
    await testInfo.attach(`host-setup-${phone ? "phone" : "desktop"}`, { body: await dialog.screenshot(), contentType: "image/png" });
    await page.setViewportSize({ width: phone ? 390 : 1100, height: 420 });
    const body = dialog.locator(".host-settings-body");
    await expect(body).not.toHaveAttribute("tabindex");
    await expect(body).not.toHaveAttribute("role");
    await expect(body).not.toHaveAttribute("aria-label");
    await expectKeyboardScrollFromControl(page, dialog.locator(".host-settings-dismiss"), plan.locator(".provisioning-copy-plan"), body);
    await page.setViewportSize({ width: phone ? 390 : 1100, height: 160 });
    await dialog.locator(".host-settings-dismiss").focus();
    await expect(dialog.locator(".host-settings-dismiss")).toBeFocused();
    await page.keyboard.press("Home");
    await expect.poll(() => dialog.evaluate((node) => node.scrollTop)).toBe(0);
    await page.keyboard.press("PageDown");
    await expect.poll(() => dialog.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
    await page.setViewportSize({ width: phone ? 390 : 1100, height: 850 });
    await expectCancelTooltipReturns(page, dialog.getByRole("button", { name: "cancel", exact: true }), dialog.getByRole("button", { name: "set up", exact: true }));
    await dialog.getByRole("button", { name: "cancel host setup", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect((await hosts(request)).some((host) => host.destination === remote)).toBe(false);
    expect((await backendEvents()).map((event) => event.event)).toEqual(["probe", "inspect"]);
    expect((await readPreferences(request)).skip_host_setup_confirmation).not.toBe(true);
  });
}

/**
 * Setup leaves its confirmation immediately, then exposes live work in the
 * row without taking over the global details preference. A user who folds
 * that one run must still get its compact status and visible verb while it continues.
 */
test("confirmed setup expands its row once and can be folded", async ({ page, request }, testInfo) => {
  test.setTimeout(120_000);
  const remote = destination(testInfo, "setup-progress-rail");
  await configureBackend({ targets: { [target(remote)]: { action_delay_ms: 1_500 } } });
  await patchPreferences(request, { skip_host_setup_confirmation: null });
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".hosts-panel")).toBeVisible();
  await page.locator(".host-rail-add").click();
  await page.locator(".add-host-ssh").fill(remote);
  await page.locator(".add-host-submit").click();
  const dialog = page.getByRole("dialog", { name: "add host", exact: true });
  await expect(dialog.locator(".provisioning-plan")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await dialog.getByRole("button", { name: "set up", exact: true }).click();
  await expect(dialog).toHaveCount(0);

  const row = hostRowByName(page, remote);
  await expect(row.locator('.provisioning-run[data-provisioning-operation="setup"]')).toBeVisible({ timeout: 30_000 });
  await expect(row.locator('.provisioning-step[data-step="create-directories"] .provisioning-step-name'))
    .toHaveText("create directories");
  await testInfo.attach("setup-progress-rail", { body: await row.screenshot(), contentType: "image/png" });
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await row.locator(".provisioning-run-fold").click();
  await expect(row.locator(".provisioning-run")).toHaveCount(0);
  await expect(row.locator(".host-update-verb")).toBeVisible();
  await expect(row.locator(".host-update-verb")).toHaveText("setting up");
  const compact = await row.evaluate((node) => {
    const sidebar = node.closest(".app-sidebar")!;
    const menu = node.querySelector(".host-row-menu")!.getBoundingClientRect();
    const verb = node.querySelector(".host-update-verb")!.getBoundingClientRect();
    const clock = node.querySelector(".host-update-elapsed")!.getBoundingClientRect();
    return {
      verbLeft: verb.left,
      sidebarLeft: sidebar.getBoundingClientRect().left,
      clockRight: clock.right,
      menuLeft: menu.left,
    };
  });
  expect(compact.verbLeft).toBeGreaterThanOrEqual(compact.sidebarLeft);
  expect(compact.clockRight).toBeLessThanOrEqual(compact.menuLeft);
  await expect(row.locator(".host-update-running")).toHaveCount(0, { timeout: 90_000 });
  await expect(row.locator(".provisioning-run")).toHaveCount(0);
});

/**
 * Discovery must display the helm's concrete steps without acting on them.
 * A home path containing direction and zero-width controls tests the full-path
 * hover boundary even when the visible chip uses the shorter ~/… spelling.
 */
test("real discovery offers one peer-safe concrete plan and mutates nothing before confirmation", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "offer");
  const hostileHome = "/home/plan\u202Espoof\u200B";
  await configureBackend({ targets: { [target(remote)]: { home: hostileHome } } });
  await page.goto("/");
  await openHostsPanel(page);
  await probeRemote(page, remote);

  const plan = page.locator(".add-host-form .provisioning-plan");
  await expect(plan).toContainText(remote);
  // Home paths shrink to ~/… in the rail, but the full path remains on hover.
  // That attribute crosses the same peer-escaping boundary as visible text.
  await expect(plan.locator(".provisioning-plan-path").first()).toHaveAttribute(
    "data-tooltip", /<U\+202E>.*<U\+200B>/,
  );
  await expect(page.getByRole("button", { name: "set up", exact: true })).toBeVisible();
  expect((await hosts(request)).some((host) => host.destination === remote)).toBe(false);
  expect((await backendEvents()).map((event) => event.event)).toEqual(["probe", "inspect"]);
});

/** An accepted agent request retains its origin through a definite setup
 * refusal/reopen. The user's saved shortcut must never auto-submit either
 * setup offer, and approval itself carries only the edited destination. */
test("agent host add asks before setup despite preference and after refusal reopen", async ({ page, request }, testInfo) => {
  const original = destination(testInfo, "agent-original");
  const edited = destination(testInfo, "agent-edited");
  await configureBackend({ targets: { [target(edited)]: {} } });
  await patchPreferences(request, { skip_host_setup_confirmation: true });
  expect((await readPreferences(request)).skip_host_setup_confirmation).toBe(true);
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  let waiting = false;
  const answers: unknown[] = [];
  const order: string[] = [];
  const probes: unknown[] = [];
  let provisions = 0;
  await page.route("**/api/approvals", async (route) => {
    await route.fulfill({ headers: { "x-farhelm-build": HELM_BUILD }, json: { approvals: waiting ? [{
      id: "agent-setup", host_id: 1, host_name: "asking host",
      session: { id: "00000001-0000-4000-8000-000000000000:asking", title: "asking session", host_name: "asking host" },
      expires_at_ms: Date.now() + 540_000,
      action: { kind: "host_add", destination: original, alias: "original alias" },
    }] : [] } });
  });
  await page.route("**/api/approvals/agent-setup", async (route) => {
    answers.push(route.request().postDataJSON());
    order.push("allow");
    waiting = false;
    feed.notify(3);
    await route.fulfill({ status: 204, headers: { "x-farhelm-build": HELM_BUILD } });
  });
  page.on("request", (candidate) => {
    if (candidate.method() === "POST" && new URL(candidate.url()).pathname === "/api/hosts/probe") {
      order.push("probe"); probes.push(candidate.postDataJSON());
    }
  });
  await page.route("**/api/hosts/provision", async (route) => {
    provisions++;
    await route.fulfill({ status: 409, headers: { "x-farhelm-build": HELM_BUILD }, body: "controlled definite setup refusal" });
  });
  await page.goto("/");
  await page.bringToFront();
  await expect.poll(() => page.evaluate(() => document.hasFocus() && document.visibilityState !== "hidden")).toBe(true);
  await expect.poll(() => feed.openSockets()).toBe(1);
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator(".hosts-heading")).not.toBeVisible();
  waiting = true; feed.notify(2);
  let form = page.locator(".add-host-form");
  await expect(form).toBeVisible();
  await form.locator(".add-host-ssh").fill(edited);
  await form.locator(".add-host-alias").fill("edited agent alias");
  await form.locator(".add-host-submit").click();
  await expect(form.locator(".provisioning-plan")).toBeVisible();
  expect(order).toEqual(["allow", "probe"]);
  expect(answers).toEqual([{ answer: "allow", host_add: { destination: edited } }]);
  expect(probes[0]).toMatchObject({ target: { destination: edited }, new_entry: { alias: "edited agent alias" } });
  expect(provisions, "preference did not auto-submit agent setup").toBe(0);
  expect((await backendEvents()).map((event) => event.event)).toEqual(["probe", "inspect"]);
  await form.getByRole("button", { name: "set up", exact: true }).click();
  form = page.locator(".add-host-form");
  await expect(form.locator(".add-host-error")).toContainText("controlled definite setup refusal");
  await expect(form.locator(".add-host-ssh")).toHaveValue(edited);
  await expect(form.locator(".add-host-alias")).toHaveValue("edited agent alias");
  expect((await readPreferences(request)).skip_host_setup_confirmation).toBe(true);
  await form.locator(".add-host-submit").click();
  await expect(form.locator(".provisioning-plan")).toBeVisible();
  expect(provisions, "origin retained after pending card is gone and form remounted").toBe(1);
  expect(answers, "reprobe is ordinary flow, not a new approval").toHaveLength(1);
  expect((await backendEvents()).map((event) => event.event)).toEqual(["probe", "inspect", "probe", "inspect"]);
  await form.getByRole("button", { name: "cancel", exact: true }).click();
  await expect(form).toHaveCount(0);
  expect(answers, "post-acceptance cancel is not Deny").toHaveLength(1);
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator(".host-rail-add")).toBeFocused();
  expect((await hosts(request)).some((host) => host.destination === edited)).toBe(false);
});

/** The host answer skips the next setup question, and the app-wide checkbox
 * restores that question in this same client without a preference reload.
 */
test("a permanent setup answer is reversible through settings after the next add", async ({
  page,
  request,
}, testInfo) => {
  const first = destination(testInfo, "remember-setup-first");
  const second = destination(testInfo, "remember-setup-second");
  const third = destination(testInfo, "remember-setup-third");
  await configureBackend({
    targets: {
      [target(first)]: {},
      [target(second)]: {},
      [target(third)]: {},
    },
  });
  await patchPreferences(request, { skip_host_setup_confirmation: null });
  await page.goto("/");
  await openHostsPanel(page);

  await page.locator(".host-rail-add").click();
  await page.locator(".add-host-ssh").fill(first);
  await page.locator(".add-host-submit").click();
  await expect(page.locator(".provisioning-plan")).toBeVisible();
  await page.getByRole("button", { name: "set up, don't ask again", exact: true }).click();
  await expect(hostRowByName(page, first)).toBeVisible();
  await expect.poll(async () => (await readPreferences(request)).skip_host_setup_confirmation).toBe(true);

  await page.locator(".host-rail-add").click();
  await page.locator(".add-host-ssh").fill(second);
  await page.locator(".add-host-submit").click();
  await expect(hostRowByName(page, second)).toBeVisible();
  await expect(page.getByRole("button", { name: "set up", exact: true })).toHaveCount(0);
  await expect(page.locator(".add-host-form")).toHaveCount(0);

  await openSettings(page, "hosts");
  const settings = page.getByRole("dialog", { name: "settings", exact: true });
  const toggle = settings.getByRole("checkbox", { name: "set up new hosts without asking", exact: true });
  await expect(toggle).toBeChecked();
  await toggle.uncheck();
  await expect(settings.locator("#host-setup-preference-help")).toContainText("asks before doing it");
  await expect.poll(async () => (await readPreferences(request)).skip_host_setup_confirmation).toBe(false);
  await settings.getByRole("button", { name: "Close", exact: true }).click();
  await probeRemote(page, third);
  await expect(page.locator(".provisioning-plan")).toBeVisible();
  await expect(page.getByText(`undo "don't ask again" with the gear at the top of the sidebar`)).toBeVisible();
  expect((await hosts(request)).some((host) => host.destination === third)).toBe(false);
  await page.getByRole("button", { name: "cancel", exact: true }).click();
  await openSettings(page, "hosts");
  await toggle.check();
  await expect.poll(async () => (await readPreferences(request)).skip_host_setup_confirmation).toBe(true);
  await settings.getByRole("button", { name: "Close", exact: true }).click();
  await page.locator(".host-rail-add").click();
  await page.locator(".add-host-ssh").fill(third);
  await page.locator(".add-host-submit").click();
  await expect(hostRowByName(page, third)).toBeVisible();
  await expect(page.locator(".add-host-form")).toHaveCount(0);
});

/**
 * Record every setup-preference write the page sends. The page saves
 * preferences from a background writer that nothing on screen waits for, so
 * a single read of the helm's preferences can come before an unwanted write
 * lands; watching the writes themselves cannot.
 */
function recordSetupPreferenceWrites(page: Page): unknown[] {
  const writes: unknown[] = [];
  page.on("request", (candidate) => {
    if (candidate.method() !== "PUT" || new URL(candidate.url()).pathname !== "/api/preferences") return;
    const body = candidate.postDataJSON() as Record<string, unknown>;
    if ("skip_host_setup_confirmation" in body) writes.push(body);
  });
  return writes;
}

/** The settings dialog's "set up new hosts without asking" checkbox, which
 * reflects this client's own preference at once. */
async function expectSetupStillAsks(page: Page): Promise<void> {
  await openSettings(page, "hosts");
  const settings = page.getByRole("dialog", { name: "settings", exact: true });
  await expect(
    settings.getByRole("checkbox", { name: "set up new hosts without asking", exact: true }),
    "the setup question is still asked",
  ).not.toBeChecked();
  await settings.getByRole("button", { name: "Close", exact: true }).click();
}

/**
 * Cancel and an answer to the add dialog's setup question delivered in one
 * burst, before the render that removes the question, set nothing up. Why:
 * either answer installs Farhelm on the remote machine, and "set up, don't
 * ask again" also turns the setup question off for every later
 * host. Cancel only told the hosts panel to close the dialog, so the offer
 * stayed live until the next render and an answer queued behind Cancel still
 * submitted it; the permanent answer also saved its preference before
 * anything checked the offer. Specifies, for each answer: no provisioning
 * request, no host registered, and the preference untouched.
 */
test("cancel and a setup answer in one burst add nothing and change no preference", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "add-burst");
  await configureBackend({ targets: { [target(remote)]: {} } });
  await patchPreferences(request, { skip_host_setup_confirmation: null });
  const preferenceWrites = recordSetupPreferenceWrites(page);
  const provisions: string[] = [];
  page.on("request", (candidate) => {
    if (candidate.method() === "POST" && new URL(candidate.url()).pathname.endsWith("/provision")) {
      provisions.push(candidate.url());
    }
  });
  await page.goto("/");
  await showHostsPanel(page);

  for (const answer of ["set up", "set up, don't ask again"]) {
    await probeRemote(page, remote);
    const form = page.locator(".add-host-form");
    await expect(form.locator(".provisioning-plan")).toBeVisible();
    const answerWasLive = await form.evaluate((node, label) => {
      const buttons = [...node.querySelectorAll<HTMLButtonElement>("button")];
      const cancel = buttons.find((button) => button.textContent?.trim() === "cancel")!;
      const yes = buttons.find((button) => button.textContent?.trim() === label)!;
      cancel.click();
      const live = yes.isConnected && !yes.disabled;
      yes.click();
      return live;
    }, answer);
    expect(answerWasLive, `premise: "${answer}" was still live when clicked`).toBe(true);
    await expect(page.locator(".add-host-form"), "cancel closes the dialog").toHaveCount(0);
    // The panel's add button is usable again only once nothing holds the
    // page; an admitted setup would hold it while it posts.
    await expect(page.locator(".host-rail-add")).toBeEnabled();
  }
  expect(provisions, "no setup request after a cancelled question").toHaveLength(0);
  expect((await hosts(request)).some((host) => host.destination === remote)).toBe(false);
  await expectSetupStillAsks(page);
  expect(preferenceWrites, "no setup preference was written").toHaveLength(0);
});

/**
 * The same burst on a host row's own setup question (a failed setup run
 * again) leaves the setup preference alone. Why: the row's confirmation
 * already refused a plan that Cancel had withdrawn, but its permanent answer
 * saved "set up new hosts without asking" first, turning the question off for
 * every later host on an answer the user had just cancelled. Specifies: after
 * Cancel then the permanent answer in one burst, no setup request is sent and
 * the preference is unchanged; a genuine answer afterwards still runs setup.
 * The separately mounted row question also keeps initial Set up focus quiet
 * without losing later keyboard or pointer help.
 */
test("cancel and a permanent answer on a host row's setup question change no preference", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "rerun-burst");
  await configureBackend({
    targets: {
      [target(remote)]: {
        fail_action: "attach-supervisor",
        message: "supervisor started but attachment failed",
      },
    },
  });
  await patchPreferences(request, { skip_host_setup_confirmation: null });
  const preferenceWrites = recordSetupPreferenceWrites(page);
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "failed");
  // The rerun probes again and, finding nothing, asks before setting up.
  await configureBackend({ targets: { [target(remote)]: {} } });
  const provisions: string[] = [];
  page.on("request", (candidate) => {
    if (candidate.method() === "POST" && new URL(candidate.url()).pathname.endsWith("/provision")) {
      provisions.push(candidate.url());
    }
  });
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-rerun").click();
  const question = page.getByRole("dialog", { name: "host setup", exact: true });
  await expect(question).toBeVisible();

  const answerWasLive = await question.evaluate((node) => {
    const buttons = [...node.querySelectorAll<HTMLButtonElement>("button")];
    const cancel = buttons.find((button) => button.textContent?.trim() === "cancel")!;
    const forever = buttons.find((button) => button.textContent?.trim() === "set up, don't ask again")!;
    cancel.click();
    const live = forever.isConnected && !forever.disabled;
    forever.click();
    return live;
  });
  expect(answerWasLive, "premise: the permanent answer was still live when clicked").toBe(true);
  await expect(question).toHaveCount(0);
  expect(provisions, "no setup request after a cancelled question").toHaveLength(0);
  await expectSetupStillAsks(page);
  expect(preferenceWrites, "no setup preference was written").toHaveLength(0);

  // The row's header owns a separate dismissal handler from Add's. Prove
  // that it returns focus and retires the offer before the positive answer.
  // Establish keyboard help on the persistent toggle before opening its
  // menu: Tab from a menu item dismisses the menu and removes that item.
  await expectKeyboardTooltip(page, row.locator(".host-row-menu"));
  await row.locator(".host-row-menu").press("Enter");
  await expect(row.locator(".host-row-menu-panel")).toBeVisible();
  await row.locator(".provisioning-rerun").press("Enter");
  await expect(question).toBeVisible();
  await expectQuietInitialFocus(page, question.getByRole("button", { name: "set up", exact: true }));
  await expect(question.getByRole("heading")).toHaveText(`set up host · ${remote}`);
  await expectHostAnswers(question, ["cancel", "set up, don't ask again", "set up"], false);
  await testInfo.attach("host-row-setup", { body: await question.screenshot(), contentType: "image/png" });
  await expectCancelTooltipReturns(page, question.getByRole("button", { name: "cancel", exact: true }), question.getByRole("button", { name: "set up", exact: true }));
  await question.getByRole("button", { name: "cancel host setup", exact: true }).click();
  await expect(question).toHaveCount(0);
  await expect(row.locator(".host-row-menu")).toBeFocused();
  expect(provisions, "header dismissal sends no setup").toHaveLength(0);
  expect(preferenceWrites, "header dismissal writes no permanent answer").toHaveLength(0);

  // Positive control: answering the question for real runs setup.
  await openHostMenu(row);
  await row.locator(".provisioning-rerun").click();
  await expect(question).toBeVisible();
  await expect(question.getByRole("button", { name: "set up", exact: true })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect.poll(() => provisions.length).toBe(1);
});

test("manual discovery preserves the concrete peer-safe reason and never provisions", async ({
  page,
}, testInfo) => {
  const remote = destination(testInfo, "manual");
  await configureBackend({
    targets: {
      [target(remote)]: {
        inspect: "manual",
        message: "unsupported target \u202Emanual\u200B",
      },
    },
  });
  await page.goto("/");
  await openHostsPanel(page);
  await probeRemote(page, remote);

  await expect(page.locator(".add-host-error")).toHaveText(
    "unsupported target <U+202E>manual<U+200B>",
  );
  await expect(page.locator(".provisioning-plan")).toHaveCount(0);
  expect((await backendEvents()).map((event) => event.event)).not.toContain("create-directories");
});

test("probe failure is concrete, peer-safe, and cannot become a setup offer", async ({
  page,
}, testInfo) => {
  const remote = destination(testInfo, "probe-error");
  await configureBackend({
    targets: {
      [target(remote)]: { probe: "error", message: "ssh denied \u202Ekey\u200B" },
    },
  });
  await page.goto("/");
  await openHostsPanel(page);
  await probeRemote(page, remote);

  await expect(page.locator(".add-host-error")).toContainText(
    'injected provisioning probe failure: host stderr "ssh denied \\u{202e}key\\u{200b}"',
  );
  await expect(page.locator(".provisioning-plan")).toHaveCount(0);
  await expect(page.locator(".add-host-submit")).toBeEnabled();

  await configureBackend();
  await page.locator(".add-host-submit").click();
  await expect(page.locator(".add-host-form .provisioning-plan")).toBeVisible();
});

// A 2xx is a commit boundary even when its body is unreadable. The form must
// report uncertainty while the registry refresh reveals any discovered host.
test("an unreadable successful probe refreshes a registration without inventing an offer", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "probe-unvalidated");
  await configureBackend({
    targets: {
      [target(remote)]: {
        probe: "supervisor",
        identity: `unvalidated-${testInfo.project.name}`,
      },
    },
  });
  await page.route("**/api/hosts/probe", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: "{not-json" });
  });
  await page.goto("/");
  await openHostsPanel(page);
  await probeRemote(page, remote);

  await expect(page.locator(".add-host-error")).toContainText(
    "the helm accepted the host probe, but its reply could not be read",
  );
  await expect(page.locator(".add-host-form .provisioning-plan")).toHaveCount(0);
  await expect(page.locator(`.host-row:has-text("${remote}")`)).toBeVisible();
  expect((await hosts(request)).some((host) => host.destination === remote)).toBe(true);
});

test("discovery registers an answering supervisor through the real handler", async ({
  page,
}, testInfo) => {
  const remote = destination(testInfo, "discovered");
  await configureBackend({
    targets: {
      [target(remote)]: {
        probe: "supervisor",
        identity: `identity-${testInfo.project.name}`,
      },
    },
  });
  await page.goto("/");
  await openHostsPanel(page);
  await probeRemote(page, remote);

  await expect(page.locator(`.host-row:has-text("${remote}")`)).toBeVisible();
  await expect(page.getByRole("button", { name: "set up", exact: true })).toHaveCount(0);
  expect((await backendEvents()).map((event) => event.event)).toEqual(["probe"]);
});

/**
 * Adding a host with an alias: the add dialog's alias is on the host it adds,
 * and adding the same destination again is refused by that name before
 * Farhelm contacts the host.
 *
 * Why it matters: the alias used to need a second trip to the host's
 * settings, and a repeated add quietly set the existing host up again. The
 * maintainer decided a repeated add is almost certainly a mistake.
 * Specifies: the probe carries `new_entry` with the alias; the discovered host
 * is listed under the alias; a second add of the same destination shows
 * "already added as" with the alias, leaves both fields as typed, and reaches
 * the backend with no second probe.
 */
test("the add dialog adds a host under its alias and refuses the same destination again", async ({
  page,
}, testInfo) => {
  const remote = destination(testInfo, "aliased");
  const alias = `alias-${testInfo.project.name}`;
  await configureBackend({ targets: { [target(remote)]: { probe: "supervisor", identity: `identity-${alias}` } } });
  const bodies: Array<Record<string, unknown>> = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/hosts/probe") {
      bodies.push(request.postDataJSON() as Record<string, unknown>);
    }
  });
  await page.goto("/");
  await openHostsPanel(page);
  const addHost = async () => {
    await page.locator(".host-rail-add").click();
    await page.locator(".add-host-ssh").fill(remote);
    await page.locator(".add-host-alias").fill(alias);
    await page.locator(".add-host-submit").click();
  };
  await addHost();
  await expect(page.locator(".add-host-form")).toHaveCount(0);
  await expect(page.locator(`.host-row:has-text("${alias}")`)).toBeVisible();
  expect(bodies[0]?.new_entry).toEqual({ alias });
  expect((await backendEvents()).filter((event) => event.event === "probe")).toHaveLength(1);

  await addHost();
  const form = page.locator(".add-host-form");
  await expect(form.locator(".add-host-error")).toContainText(`already added as "${alias}"`);
  await expect(form.locator(".add-host-ssh")).toHaveValue(remote);
  await expect(form.locator(".add-host-alias")).toHaveValue(alias);
  expect(bodies).toHaveLength(2);
  expect((await backendEvents()).filter((event) => event.event === "probe"), "the duplicate never reached ssh")
    .toHaveLength(1);
});

/**
 * The add dialog's alias is checked before Farhelm contacts the host: one
 * that is already a host's name is refused by the helm, and a malformed one
 * never leaves the browser.
 *
 * Why it matters: a probe can take seconds and lead into a setup
 * confirmation; finding out about the alias only at the end wastes all of it.
 * Specifies: an alias equal to an existing host's destination shows "already
 * displayed as" with no backend probe; a 65-character alias shows the
 * settings dialog's length message and sends no probe request at all.
 */
test("the add dialog refuses a taken or malformed alias before contacting the host", async ({
  page,
  request,
}, testInfo) => {
  const existing = destination(testInfo, "alias-owner");
  const remote = destination(testInfo, "alias-wanted");
  await configureBackend({ targets: { [target(existing)]: { probe: "supervisor", identity: `identity-${existing}` } } });
  expect((await probe(request, existing)).result, "premise: the name is taken by a real host").toBe("discovered");
  await configureBackend({ targets: { [target(remote)]: { probe: "supervisor" } } });
  let probeRequests = 0;
  page.on("request", (candidate) => {
    if (new URL(candidate.url()).pathname === "/api/hosts/probe") probeRequests += 1;
  });
  await page.goto("/");
  await openHostsPanel(page);
  await page.locator(".host-rail-add").click();
  const form = page.locator(".add-host-form");
  await form.locator(".add-host-ssh").fill(remote);
  await form.locator(".add-host-alias").fill(existing);
  await form.locator(".add-host-submit").click();
  await expect(form.locator(".add-host-error")).toContainText(`already displayed as "${existing}"`);
  expect(probeRequests).toBe(1);

  await form.locator(".add-host-alias").fill("x".repeat(65));
  await form.locator(".add-host-submit").click();
  await expect(form.locator(".add-host-error")).toHaveText("host alias must be at most 64 characters");
  expect(probeRequests, "a malformed alias is refused in the browser").toBe(1);
  expect((await backendEvents()).filter((event) => event.event === "probe")).toHaveLength(0);
  expect((await hosts(request)).some((host) => host.destination === remote)).toBe(false);
});

/**
 * The alias typed in the add dialog survives the setup question for a host
 * without Farhelm and is on the host the confirmed setup adds.
 *
 * Why it matters: the setup question replaces the form, so the alias lives
 * only in the helm's retained plan while the person reads it; dropping it
 * there would leave them renaming the host right after adding it.
 * Specifies: confirming setup lists the new host under the alias.
 */
test("the add dialog's alias survives the setup question", async ({ page }, testInfo) => {
  const remote = destination(testInfo, "aliased-setup");
  const alias = `setup-alias-${testInfo.project.name}`;
  await configureBackend({ targets: { [target(remote)]: { probe: "absent" } } });
  await page.goto("/");
  await openHostsPanel(page);
  await page.locator(".host-rail-add").click();
  await page.locator(".add-host-ssh").fill(remote);
  await page.locator(".add-host-alias").fill(alias);
  await page.locator(".add-host-submit").click();
  await expect(page.locator(".add-host-form .provisioning-plan")).toBeVisible();
  await page.getByRole("button", { name: "set up", exact: true }).click();
  await expect(page.locator(`.host-row:has-text("${alias}")`)).toBeVisible();
});

/**
 * The add dialog asks only for the ssh destination and an optional alias, so
 * its probe must carry no remote binary or state directory, and two submits in
 * one browser task must still retain only one setup plan.
 *
 * Specifies: the probe body's `remote_farhelm` and `remote_state_dir` are null,
 * its `new_entry` names no alias when none was typed, and the backend sees
 * exactly one probe for a doubled submit. The dialog once had optional fields
 * for both paths; null is what lets discovery and setup choose them instead of
 * a value nobody adding a host knew how to fill in.
 */
test("the add dialog's probe sends no install paths, and a doubled submit produces one real probe", async ({
  page,
}, testInfo) => {
  const remote = destination(testInfo, "probe-guard");
  let body: Record<string, unknown> | undefined;
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/hosts/probe") {
      body = request.postDataJSON() as Record<string, unknown>;
    }
  });
  await page.goto("/");
  await openHostsPanel(page);
  await page.locator(".host-rail-add").click();
  await page.locator(".add-host-ssh").fill(remote);
  await page.evaluate(() => {
    const form = document.querySelector(".add-host-form")!;
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  });
  await expect(page.locator(".add-host-form .provisioning-plan")).toBeVisible();

  expect(body?.remote_farhelm ?? null).toBeNull();
  expect(body?.remote_state_dir ?? null).toBeNull();
  expect(body?.new_entry).toEqual({ alias: null });
  expect((await backendEvents()).filter((event) => event.event === "probe")).toHaveLength(1);
});

/**
 * The add dialog must never hold the user behind a pending request, so its
 * cancel stays live while discovery is out and closing it frees the page.
 *
 * Specifies: with the probe held, cancel is enabled and closes the dialog;
 * the page behind it is reachable again at once; and releasing the probe
 * afterwards reopens nothing and registers nothing. Closing drops the
 * dialog's wait for the reply, and the browser may abandon the request with
 * it, so the release barrier is the route handler finishing, not a response.
 */
test("the add dialog's cancel stays live while discovery is pending", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "cancel-while-probing");
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let resolveArrived!: () => void;
  const arrived = new Promise<void>((resolve) => {
    resolveArrived = resolve;
  });
  let resolveHandled!: () => void;
  const handled = new Promise<void>((resolve) => {
    resolveHandled = resolve;
  });
  await page.route(
    (url) => url.pathname === "/api/hosts/probe",
    async (route) => {
      resolveArrived();
      await gate;
      try {
        await route.continue();
      } catch {
        // The page abandoned the request when the dialog closed.
      } finally {
        resolveHandled();
      }
    },
  );
  await page.goto("/");
  await showHostsPanel(page);
  await probeRemote(page, remote);
  await arrived;
  await expect(page.locator(".add-host-submit")).toHaveText("checking…");

  const cancel = page.locator(".add-host-cancel");
  await expect(cancel).toBeEnabled();
  await cancel.click();
  await expect(page.locator(".host-add-dialog")).toHaveCount(0);
  await expect(page.locator(".host-rail-add")).toBeEnabled();

  release();
  await handled;
  await expect(page.locator(".host-add-dialog")).toHaveCount(0);
  await expect(page.locator(".provisioning-plan")).toHaveCount(0);
  expect((await hosts(request)).some((host) => host.destination === remote)).toBe(false);
});

/** Acceptance registers the host and releases unrelated page actions, while
 * this row stays busy until its numbered setup run reaches completion. */
test("accepted ADD registers before execution and releases the page lock while running", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "accepted-add");
  await configureBackend({
    targets: { [target(remote)]: { hold_actions: true } },
  });
  let documents = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documents += 1;
  });
  await page.goto("/");
  await openHostsPanel(page);
  await probeRemote(page, remote);
  await page.getByRole("button", { name: "set up", exact: true }).click();

  const registered = await hostFor(request, remote);
  await expect(page.locator(`[data-host-id="${registered.id}"]`)).toBeVisible();
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-run`)).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(page.locator(".host-rail-add")).toBeEnabled();
  // `.host-settings` lives inside the row's own "⋯" menu now, and it is
  // `aria-disabled` there rather than natively `disabled` (see `HostRow`'s
  // own doc — the menu leaves busy items focusable, like the session row's
  // menu) — `toBeDisabled()` honours `aria-disabled="true"` the same way.
  const registeredRow = page.locator(`[data-host-id="${registered.id}"]`);
  await openHostMenu(registeredRow);
  await expect(registeredRow.locator(".host-settings")).toBeDisabled();
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-update`)).toHaveCount(
    0,
  );
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-rerun`)).toHaveCount(
    0,
  );
  await expect.poll(async () => (await backendEvents()).some((event) => event.event === "create-directories")).toBe(true);

  // The executor is demonstrably held in its first live step before hiding
  // management. Completion while concealed must reach the same mounted owner.
  const running = await progress(request, registered.id);
  expect(running.status).toBe("running");
  expect(running.run_id).toBeTruthy();
  await page.locator(".hosts-panel-toggle").click();
  await expect(page.locator(".hosts-heading")).not.toBeVisible();
  await expect(registeredRow.locator(".provisioning-run")).not.toBeVisible();
  const avatar = page.locator(`.host-rail-avatar[data-rail-host-id="${registered.id}"]`);
  await expect(avatar).toHaveAttribute("data-host-progress", "true");
  const local = (await hosts(request)).find((host) => host.kind === "local");
  expect(local?.state.phase, "the independent rail anchor must be a connected local host").toBe("connected");
  const localAvatar = page.locator(`.host-rail-avatar[data-rail-host-id="${local!.id}"]`);
  await localAvatar.focus();
  await expect(localAvatar).toBeFocused();
  await page.keyboard.press("Shift+F10");
  const localMenu = page.locator(`[data-host-id="${local!.id}"] .host-row-menu-panel`);
  await expect(localMenu).toBeVisible();

  await configureBackend();
  await waitForProgress(request, registered.id, "completed");
  await expect(avatar).toHaveAttribute("data-host-progress", "false");
  await expect(localMenu, "concealed remote progress cannot move the local rail anchor").toBeVisible();
  await page.keyboard.press("Escape");
  await expect(localMenu).toHaveCount(0);
  await showHostsPanel(page);
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-run`)).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  // Completion can fold the one-off rail and invalidate the menu's measured
  // position. Open it again before checking the now-available Update action.
  await openHostMenu(registeredRow);
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-update`)).toBeVisible();
  expect(documents).toBe(1);
});

/** A definite setup submission refusal reopens the add form with its
 * destination intact, but cannot restore the consumed one-use plan. */
test("a refused ADD attempt reopens the form and refreshes committed state", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "add-refusal");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  await page.goto("/");
  await openHostsPanel(page);
  const probeResponse = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/hosts/probe" && response.request().method() === "POST",
  );
  await probeRemote(page, remote);
  const planned = (await (await probeResponse).json()) as Extract<ProbeReply, { result: "provisionable" }>;
  const consumed = await request.post("/api/hosts/provision", { data: { probe_id: planned.probe_id } });
  expect(consumed.status()).toBe(202);

  await page.getByRole("button", { name: "set up", exact: true }).click();
  const form = page.locator(".add-host-form");
  await expect(form.locator(".add-host-error")).toContainText("already been used");
  await expect(form.locator(".add-host-ssh")).toHaveValue(remote);
  await expect(form.locator(".provisioning-plan")).toHaveCount(0);
  await expect(page.locator(".hosts-notices > .add-host-error")).toHaveCount(0);
  await form.locator(".add-host-cancel").click();
  await expect(form).toHaveCount(0);
  await expect(page.locator(".host-rail-add")).toBeEnabled();
  await expect(page.locator(`.host-row:has-text("${remote}")`)).toBeVisible();
});

test("a malformed accepted ADD closes the form and warns without retrying", async ({
  page,
}, testInfo) => {
  const remote = destination(testInfo, "add-unvalidated");
  await page.route("**/api/hosts/provision", async (route) => {
    await route.fulfill({
      status: 202,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: "{not-json",
    });
  });
  await page.goto("/");
  await openHostsPanel(page);
  await probeRemote(page, remote);
  await page.getByRole("button", { name: "set up", exact: true }).click();

  await expect(page.locator(".add-host-form")).toHaveCount(0);
  await expect(page.locator(".add-host-warning")).toContainText("accepted");
});

// The helm's own machine is `farhelm helm setup`'s to own (plan D1), so an
// absent local supervisor is answered with that instruction instead of an
// install plan. What still has to work is the state machine around it: the
// answer arrives on the down transition, clears when the supervisor comes
// back, and one probe is spent per transition rather than per render.
test("local setup answers an absent supervisor with run-setup, and reacts down, connected, then down again", async ({
  page,
}) => {
  const state = { down: false };
  const feed = await controlLocalState(page, state);
  await page.goto("/");
  await openHostsPanel(page);
  await expect(page.locator('[data-host-kind="local"] .provisioning-error')).toHaveCount(0);

  state.down = true;
  await notifyFeed(feed, 2);
  await expect(page.locator('[data-host-kind="local"] .provisioning-error')).toContainText(
    "run farhelm helm setup here instead of provisioning from the panel",
  );
  await expect(page.locator('[data-host-kind="local"] .provisioning-plan')).toHaveCount(0);
  await expect(page.locator('[data-host-kind="local"] .provisioning-manual')).toContainText(
    "farhelm supervisor run",
  );
  state.down = false;
  await notifyFeed(feed, 3);
  await expect(page.locator('[data-host-kind="local"] .provisioning-error')).toHaveCount(0);
  state.down = true;
  await notifyFeed(feed, 4);
  await expect.poll(async () =>
    (await backendEvents()).filter((event) => event.target === "local" && event.event === "probe").length,
  ).toBe(2);
  await expect(page.locator('[data-host-kind="local"] .provisioning-error')).toContainText(
    "run farhelm helm setup here instead of provisioning from the panel",
  );
});

// Step status is a forward-compatible string even though aggregate run state
// is closed. A newer helm must not make an older UI drop the whole progress
// view merely because it introduced another step state.
test("an unknown future progress-step status renders verbatim", async ({ page }) => {
  await page.route("**/api/hosts/1/provisioning", async (route) => {
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: 1,
        run_id: "future-step-status",
        operation: "update",
        status: "running",
        steps: [{ step: "restart-supervisor", status: "awaiting-reboot", message: null }],
        message: null,
      }),
    });
  });
  await page.goto("/");
  await openHostsPanel(page);

  const step = page.locator('[data-host-id="1"] [data-step="restart-supervisor"]');
  await expect(step).toHaveAttribute("data-status", "awaiting-reboot");
  await expect(step.locator(".provisioning-step-status")).toHaveText("awaiting-reboot");
});

// A transport failure is not the same answer as "this machine is the helm's
// own". Keep retry available so a temporary failure cannot strand the row
// until reload, and let the retry reach the real answer.
test("a transient local probe error can be retried into the run-setup answer", async ({ page }) => {
  const state = { down: true };
  await controlLocalState(page, state);
  await configureBackend({
    targets: {
      local: { probe: "error", message: "temporary local probe failure" },
    },
  });
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/hosts/probe", async (route) => {
    const body = route.request().postDataJSON() as { target?: { kind?: string } };
    if (body.target?.kind !== "local") return route.continue();
    await held;
    await route.continue();
  });
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  release();
  const local = page.locator('[data-host-kind="local"]');
  await expect(local.locator(".provisioning-error")).toContainText("temporary local probe failure");
  await expect(page.locator(".host-details-toggle")).toBeChecked();
  await openHostMenu(local);
  await expect(local.locator(".provisioning-auto-setup")).toBeVisible();

  await configureBackend();
  await local.locator(".provisioning-auto-setup").click();
  await expect(local.locator(".provisioning-error")).toContainText(
    "run farhelm helm setup here instead of provisioning from the panel",
  );
  await expect(local.locator(".provisioning-plan")).toHaveCount(0);
});

test("local automatic discovery does not start before the authoritative idle view", async ({
  page,
}) => {
  const state = { down: true };
  await controlLocalState(page, state);
  let release!: () => void;
  const allowed = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/hosts/*/provisioning", async (route) => {
    await allowed;
    await route.continue();
  });
  await page.goto("/");
  await openHostsPanel(page);
  await expect(page.locator('[data-host-kind="local"]')).toBeVisible();
  expect((await backendEvents()).some((event) => event.target === "local")).toBe(false);

  release();
  await expect(page.locator('[data-host-kind="local"] .provisioning-error')).toContainText(
    "run farhelm helm setup here instead of provisioning from the panel",
  );
  expect((await backendEvents()).some((event) => event.target === "local" && event.event === "probe")).toBe(true);
});

// A manual-only answer must not leave the row half-offering automation: no
// plan, and no "set up automatically" button whose only outcome would be the
// same refusal again. Every helm machine WITHOUT a local supervisor reaches
// this state, so its rendering is the one most users see first; the state
// clears as soon as a supervisor is running, which the tail of this test
// checks.
test("manual-only local setup leaves the manual command primary", async ({ page }) => {
  const state = { down: true };
  const feed = await controlLocalState(page, state);
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/hosts/probe", async (route) => {
    const body = route.request().postDataJSON() as { target?: { kind?: string } };
    if (body.target?.kind !== "local") return route.continue();
    await held;
    await route.continue();
  });
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  release();
  const local = page.locator('[data-host-kind="local"]');
  await expect(local.locator(".provisioning-manual:not(.secondary)")).toContainText(
    "farhelm supervisor run",
  );
  await expect(local.locator(".provisioning-error")).toContainText(
    "this is the helm's own machine; run farhelm helm setup here instead of provisioning from the panel",
  );
  await expect(page.locator(".host-details-toggle")).toBeChecked();
  await expect(local.locator(".provisioning-plan")).toHaveCount(0);
  await openHostMenu(local);
  await expect(local.locator(".provisioning-auto-setup")).toHaveCount(0);

  state.down = false;
  await notifyFeed(feed, 2);
  await expect(local.locator(".provisioning-error")).toHaveCount(0);
});

/**
 * A committed local probe with an unreadable body still needs a person to
 * reconcile what happened. The disclosure must therefore open from its
 * resting state even though no confirmation can be rendered.
 */
test("an unvalidated automatic local probe reveals its feedback", async ({ page }) => {
  const state = { down: true };
  await controlLocalState(page, state);
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/hosts/probe", async (route) => {
    const body = route.request().postDataJSON() as { target?: { kind?: string } };
    if (body.target?.kind !== "local") return route.continue();
    await held;
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: "{not-json",
    });
  });

  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  release();
  await expect(page.locator('[data-host-kind="local"] .provisioning-error')).toContainText(
    "the helm accepted the local probe, but its reply could not be read",
  );
  await expect(page.locator(".host-details-toggle")).toBeChecked();
});

/**
 * A confirmation must mount from a folded row without rewriting the user's
 * global details choice. This stub reaches a defensive local plan path;
 * production local probes hand off to the manual setup command.
 */
test("a held local plan opens its confirmation while details stay folded", async ({ page }, testInfo) => {
  const state = { down: true };
  await controlLocalState(page, state);
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/hosts/probe", async (route) => {
    const body = route.request().postDataJSON() as { target?: { kind?: string } };
    if (body.target?.kind !== "local") return route.continue();
    await held;
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        result: "provisionable",
        probe_id: "held-local-plan",
        confirmation: "install the local supervisor",
      }),
    });
  });

  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  release();
  await expect(page.locator('[data-host-kind="local"] .provisioning-plan')).toContainText(
    "install the local supervisor",
  );
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const question = page.getByRole("dialog", { name: "host setup", exact: true });
  await expect(question.getByRole("heading")).toHaveText("set up host · local (this machine)");
  await expect(question.locator(".host-settings-summary")).toHaveText("Farhelm is not running here yet");
  await testInfo.attach("host-local-setup", { body: await question.screenshot(), contentType: "image/png" });
});

/**
 * Feed-driven host props must republish menu offers even when retained
 * progress does not change. This pins the local-setup and host-kind inputs as
 * reactive dependencies rather than values captured on the first render.
 */
test("a local setup phase transition republishes provisioning commands", async ({ page }) => {
  const state = { down: false };
  const feed = await controlLocalState(page, state, { greetOnConnect: false });
  await configureBackend({
    targets: { local: { probe: "error", message: "retry automatic setup" } },
  });
  await page.route("**/api/hosts/*/provisioning", async (route) => {
    const id = Number(new URL(route.request().url()).pathname.split("/").at(-2));
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: id,
        run_id: null,
        operation: null,
        status: "completed",
        steps: [],
        message: null,
      }),
    });
  });

  await page.goto("/");
  await showHostsPanel(page);
  const local = page.locator('[data-host-kind="local"]');
  await openHostMenu(local);
  await expect(local.locator(".provisioning-update")).toBeVisible();
  await page.keyboard.press("Escape");

  state.down = true;
  await notifyFeed(feed, 2);
  await expect(local.locator(".provisioning-error")).toContainText("retry automatic setup");
  await openHostMenu(local);
  await expect(local.locator(".provisioning-update")).toHaveCount(0);
  await expect(local.locator(".provisioning-auto-setup")).toBeVisible();
});

/**
 * The helm refuses to update its own machine, and that refusal used to stick
 * under the local row with no way to dismiss it. So the local row's menu shows
 * Update greyed out, pointing at the installer, and activating it by mouse or
 * keyboard sends no update request at all. The item stays in the menu (greyed
 * out, not hidden) so the menu still says how this machine is updated.
 */
test("the local row's update is greyed out, points at the installer, and sends nothing", async ({ page }) => {
  const state = { down: false };
  await controlLocalState(page, state, { greetOnConnect: false });
  await page.route("**/api/hosts/*/provisioning", async (route) => {
    const id = Number(new URL(route.request().url()).pathname.split("/").at(-2));
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: id,
        run_id: null,
        operation: null,
        status: "completed",
        steps: [],
        message: null,
      }),
    });
  });

  await page.goto("/");
  await showHostsPanel(page);
  const local = page.locator('[data-host-kind="local"]');
  await expect(local).toBeVisible();
  const localId = Number(await local.getAttribute("data-host-id"));
  const updates = countUpdateRequests(page, localId);
  await openHostMenu(local);
  const item = local.locator(".provisioning-update");
  await expect(item).toBeVisible();
  await expect(item).toHaveAttribute("aria-disabled", "true");
  await expect(item.locator(".session-row-menu-description")).toHaveText(
    "run the installer again to update this machine",
  );

  // A sent update is not a POST from the handler itself: the click queues the
  // row's request, a reactive effect consumes it, and a spawned task then
  // POSTs the update plan. That is a few render turns, so each window allows a
  // second for them on a loaded machine; nothing else in the page would
  // announce the request's absence. Checking after each activation keeps a
  // regression failing on the count rather than on the menu an accepted update
  // closes. `force` because Playwright treats an aria-disabled button as not
  // clickable, and the point is to click it anyway.
  await item.click({ force: true });
  // sleep-ok: observation window for an update request that must not be sent.
  await page.waitForTimeout(1000);
  expect(updates, "a click on the greyed-out item").toEqual({ plans: 0, confirms: 0 });
  await item.focus();
  await expect(item).toBeFocused();
  await page.keyboard.press("Enter");
  // sleep-ok: observation window for an update request that must not be sent.
  await page.waitForTimeout(1000);
  expect(updates, "Enter on the greyed-out item").toEqual({ plans: 0, confirms: 0 });
  await expect(local.locator(".provisioning-error")).toHaveCount(0);
});

test("a failed local ADD keeps its rerun action in the local setup state", async ({ page }) => {
  const state: InjectedLocalState = { down: true };
  const feed = await controlLocalState(page, state, { greetOnConnect: false });
  await page.route("**/api/hosts/*/provisioning", async (route) => {
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: 1,
        run_id: "failed-local",
        operation: "add",
        status: "failed",
        steps: [{ step: "enable-supervisor", status: "failed", message: "user manager failed" }],
        message: "rerun provisioning to continue",
      }),
    });
  });
  await page.goto("/");
  await openHostsPanel(page);
  const local = page.locator('[data-host-kind="local"]');
  await openHostMenu(local);
  await expect(local.locator(".provisioning-rerun")).toBeVisible();
  await local.locator(".provisioning-rerun").click();
  await expect(local.locator(".provisioning-error")).toContainText(
    "run farhelm helm setup here instead of provisioning from the panel",
  );

  // A feed notification only starts reconciliation. Register its request
  // boundary first, then publish the marker and revision without yielding;
  // the route's state snapshot prevents an older in-flight request from
  // acquiring it. Rendering the marker proves WebKit decoded this refresh.
  await expect.poll(() => feed.openSockets()).toBeGreaterThan(0);
  const refresh = page.waitForRequest(
    (request) =>
      new URL(request.url()).pathname === "/api/hosts" && request.method() === "GET",
  );
  const refreshedError = "feed-driven host refresh completed";
  state.lastError = refreshedError;
  feed.notify(2);
  try {
    await refresh;
    await expect(local.locator(".provisioning-manual")).toContainText(refreshedError);
    await expect(local.locator(".provisioning-error")).toContainText(
      "run farhelm helm setup here instead of provisioning from the panel",
    );
  } finally {
    // Feed and fallback refreshes can coalesce after the visible assertion.
    // Drain and remove their handlers here so context teardown never disposes
    // a route-fetched response while `controlLocalState` is decoding it.
    await page.unrouteAll({ behavior: "wait" });
  }
});

// These UPDATE cases run while the shared fleet is reconnecting. Dispatching
// the already-visible row action atomically keeps an unrelated card replacement
// from splitting Playwright's pointer-down/pointer-up pair; earlier cases cover
// ordinary pointer actionability.
//
// Remote updates submit automatically: the Update click is the authorization,
// so these cases never wait for `.provisioning-confirm` — they count the
// bodyless planning POST separately from the token-bearing submission POST
// and prove no replay. These cases start with global details UNCHECKED (never
// `openHostsPanel()`) and assert it stays unchecked: healthy updates show
// inline progress, while failures and uncertain outcomes open their own row.
// Authoritative success returns the status spot to its ordinary label.

/**
 * Aborts for `holdLockWithAdd` gates; `afterEach` fires whatever is left.
 * Each resolves once a held confirm has been aborted, or at once when its
 * gate was already released (the handler then continued long ago and the
 * abort changes nothing) or the confirm never arrived.
 */
const heldAddConfirms: Array<() => Promise<void>> = [];

/**
 * Hold the page operation lock through an ADD confirm, without touching details.
 *
 * Discovery stays outside the lock, so the held request is the confirm POST
 * itself: its arrival proves the claim is taken, and releasing it completes
 * the ADD and frees the token. Unlike retry/adopt/edit, confirming an add
 * writes no global disclosure, so checkbox assertions stay meaningful.
 *
 * Confirming also closes the add dialog while the POST is still held: the
 * submission belongs to the hosts panel from then on, so the rest of the
 * page (other hosts' menus above all) stays reachable instead of sitting
 * under a modal that waits on the request.
 */
async function holdLockWithAdd(
  page: Page,
  destination: string,
): Promise<{ release: () => void }> {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let abandoned = false;
  let arrived = false;
  let settle!: () => void;
  const settled = new Promise<void>((resolve) => {
    settle = resolve;
  });
  heldAddConfirms.push(async () => {
    abandoned = true;
    release();
    if (arrived) await settled;
  });
  let resolveConfirmed!: () => void;
  const confirmed = new Promise<void>((resolve) => {
    resolveConfirmed = resolve;
  });
  // Predicate, not a glob: the held POST must match exactly this path.
  await page.route(
    (url) => url.pathname === "/api/hosts/provision",
    async (route) => {
      arrived = true;
      resolveConfirmed();
      await gate;
      try {
        if (abandoned) await route.abort();
        else await route.continue();
      } finally {
        settle();
      }
    },
  );
  const addHost = page.locator(".host-rail-add");
  await addHost.click();
  await page.locator(".add-host-ssh").fill(destination);
  await page.locator(".add-host-submit").click();
  await expect(page.locator(".add-host-form .provisioning-plan")).toBeVisible();
  await page.getByRole("button", { name: "set up", exact: true }).click();
  await confirmed;
  await expect(page.locator(".host-add-dialog")).toHaveCount(0);
  // The held confirm owns the page token: unrelated adds disable until
  // the release below, which is the UI-side proof the lock is really held.
  await expect(addHost).toBeDisabled();
  return { release };
}

/**
 * The inline status names the helm's current step, which may contain spaces.
 * The `updating: ` prefix is visually hidden but kept for assistive technology.
 */
const RUNNING_PROGRESS = /updating: \d+\/\d+ .+ \d+:\d{2}/;

/**
 * The row's pending `updating…` status at one exact lifecycle stage.
 *
 * Updates keep their row folded, so the planning, waiting-for-claim, and
 * submitting indicators inside the details never render. Every stage shows
 * the same `updating…` text; `data-update-phase` is the narrow handle that
 * tells a held boundary apart, and a response arriving would not prove the
 * UI had processed it.
 */
function pendingUpdate(
  row: Locator,
  phase: "planning" | "waiting" | "submitting" | "awaiting-progress",
): Locator {
  return row.locator(`.host-update-pending[data-update-phase="${phase}"]`);
}

/**
 * Wait for a settled update to hand the status spot back to the ordinary label.
 *
 * A backend completion barrier proves only that the run ended, and a folded
 * row looks the same while an update runs as after it ends, so settlement is
 * observed here: both inline forms disappear and the plain phase label
 * returns. The provisioning hosts dial `.invalid` destinations and never
 * connect, so a plain label is always present at rest; its text is not
 * compared because reprobing can move it between phases.
 */
async function expectOrdinaryStatus(row: Locator): Promise<void> {
  await expect(row.locator(".host-update-pending")).toHaveCount(0);
  await expect(row.locator(".host-update-running")).toHaveCount(0);
  await expect(row.locator(".host-status")).not.toHaveClass(/\bupdating\b/);
  await expect(row.locator(".host-status > .host-status-label")).toHaveClass("host-status-label");
}

/** Classify update-route POSTs into bodyless plans and token submissions. */
function countUpdateRequests(page: Page, host: number): { plans: number; confirms: number } {
  const counts = { plans: 0, confirms: 0 };
  page.on("request", (outgoing) => {
    const pathname = new URL(outgoing.url()).pathname;
    if (pathname === `/api/hosts/${host}/update` && outgoing.method() === "POST") {
      if (outgoing.postData()) {
        counts.confirms += 1;
      } else {
        counts.plans += 1;
      }
    }
  });
  return counts;
}

/**
 * A healthy update is followed from the host row's status spot alone.
 *
 * Why this matters: the row used to open for every update, and the user asked
 * for it to stay compact. This pins the whole folded path: one plan and one
 * submission with no confirmation, `N/M step m:ss` inline with a pulse that
 * stands down under reduced motion, no trace line, the menu toggle still
 * inside the sidebar with the longest real step name, a hover popup that
 * spells out what the narrow label truncates, and success returning the
 * ordinary status without ever touching the details checkbox.
 */
for (const appearance of ["dark", "light"] as const) {
test(`single Update shows inline progress without opening its row in ${appearance}`, async ({
  page,
  request,
}, testInfo) => {
  await page.emulateMedia({ colorScheme: appearance, reducedMotion: "no-preference" });
  const remote = destination(testInfo, "update");
  const bystander = destination(testInfo, "update-bystander");
  const accepted = await startAdd(request, remote);
  const other = await startAdd(request, bystander);
  await waitForProgress(request, accepted.host_id, "completed");
  await waitForProgress(request, other.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".hosts-panel")).toBeVisible();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  const idle = page.locator(`[data-host-id="${other.host_id}"]`);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  // Completed retained runs are resting history, not collapsed traces.
  await expect(row.locator(".provisioning-trace")).toHaveCount(0);
  await openHostMenu(row);
  await expect(row.locator(".provisioning-update")).toBeVisible();
  const toggle = row.locator(".host-row-menu");
  await row.locator(".provisioning-update").focus();
  await page.evaluate((id) => {
    const button = document.querySelector(`[data-host-id="${id}"] .provisioning-update`)!;
    button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  }, accepted.host_id);
  await expect(row.locator(".host-row-menu-panel")).toHaveCount(0);
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  await expect(toggle).toBeFocused();
  // The duplicate click coalesces into the accepted intent: one planning
  // request, no confirmation, and live progress stays in the status spot.
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  // The inline status replaces the folded row's one-line run trace rather
  // than sitting above a second line that repeats it.
  await expect(row.locator(".provisioning-trace")).toHaveCount(0);

  // The held `create-directories` step has a multiword helm name. It must
  // not push the menu toggle past the sidebar's visible right edge (the
  // sidebar clips horizontal overflow): the name truncates, while the count
  // and clock stay whole, left of the toggle.
  await expect(row.locator(".host-update-step")).toHaveText(" create directories");
  // The visible count is bare, and `updating: ` survives only as hidden text:
  // in a sidebar this narrow the prefix was most of the step name's room.
  await expect(row.locator(".host-update-count .visually-hidden")).toHaveText("updating: ");
  const geometry = await row.evaluate((node) => {
    const sidebar = node.closest(".app-sidebar")!;
    const sidebarBox = sidebar.getBoundingClientRect();
    const toggleBox = node.querySelector(".host-row-menu")!.getBoundingClientRect();
    const clockBox = node.querySelector(".host-update-elapsed")!.getBoundingClientRect();
    const nameBox = node.querySelector(".host-name")!.getBoundingClientRect();
    const statusBox = node.querySelector(".host-status")!.getBoundingClientRect();
    return {
      visibleRight: sidebarBox.left + sidebar.clientLeft + sidebar.clientWidth,
      toggleLeft: toggleBox.left,
      toggleRight: toggleBox.right,
      clockRight: clockBox.right,
      nameWidth: nameBox.width,
      nameBottom: nameBox.bottom,
      statusTop: statusBox.top,
    };
  });
  expect(geometry.toggleRight).toBeLessThanOrEqual(geometry.visibleRight);
  expect(geometry.clockRight).toBeLessThanOrEqual(geometry.toggleLeft);
  // Progress yields its own line so the host remains identifiable while
  // the operation runs, without opening the detailed run panel.
  expect(geometry.nameWidth).toBeGreaterThan(150);
  expect(geometry.statusTop).toBeGreaterThanOrEqual(geometry.nameBottom);

  // Hovering anywhere on the label spells out the whole status. The popup is
  // fixed-position so the sidebar's clipping cannot cut off the step name the
  // inline label had to truncate, and it goes away when the pointer leaves.
  const popup = row.locator(".host-update-popup");
  await expect(popup).toHaveCount(0);
  await row.locator(".host-update-count").hover();
  await expect(popup).toBeVisible();
  await expect(popup).toContainText(/updating: \d+ of \d+ steps done/);
  await expect(popup.locator(".host-update-popup-step")).toHaveText("create directories");
  await expect(popup).toContainText(/\d+:\d{2} elapsed/);
  // Both live clocks keep their technical face; the popup's explanatory
  // suffix stays in Inter. Resolve the actual weight-matched bundled face.
  for (const [clock, family] of [
    [row.locator(".host-update-elapsed"), "JetBrains Mono"],
    [popup.locator(".host-update-popup-elapsed .font-mono"), "JetBrains Mono"],
    [popup.locator(".host-update-popup-elapsed"), "Inter"],
  ] as const) {
    await expect.poll(() => clock.evaluate(async (element, family) => {
      const style = getComputedStyle(element);
      if (style.fontFamily.split(",")[0].replace(/["']/g, "").trim() !== family) return false;
      const faces = await document.fonts.load(`${style.fontWeight} ${style.fontSize} "${family}"`, element.textContent || "M");
      return faces.length > 0 && faces.every((face) => face.status === "loaded");
    }, family), { message: `${family} loads for the held operation clock role` }).toBe(true);
  }
  // Every step of the held run is listed, in executor order with the
  // statuses the helm reports, and exactly one is marked as in progress:
  // the held `create-directories`.
  const held = await progress(request, accepted.host_id);
  const listed = await popup.locator(".provisioning-step").evaluateAll((items) =>
    items.map((item) => ({ step: item.getAttribute("data-step"), status: item.getAttribute("data-status") })),
  );
  expect(listed, "the popup lists the run's own steps").toEqual(
    held.steps.map(({ step, status }) => ({ step, status })),
  );
  await expect(popup.locator('.provisioning-step[data-current="true"]')).toHaveCount(1);
  await expect(popup.locator('.provisioning-step[data-current="true"]')).toHaveAttribute("data-step", "create-directories");
  await expect(popup.locator('.provisioning-step[data-current="true"]')).toHaveAttribute("data-status", "running");
  const marker = popup.locator('.provisioning-step[data-current="true"] .provisioning-step-marker');
  // Fading a whole numbered marker makes its text unreadable. Only the ring
  // may pulse; its pseudo-element must retain the original outer diameter.
  const markerPaint = await marker.evaluate((element) => {
    const style = getComputedStyle(element);
    const number = getComputedStyle(element, "::before");
    const ring = getComputedStyle(element, "::after");
    return { textAnimation: style.animationName, textOpacity: style.opacity, numeralOpacity: number.opacity,
      ringAnimation: ring.animationName, ringTiming: ring.animationTimingFunction, ringDuration: ring.animationDuration,
      color: style.color, ringColor: ring.borderTopColor, width: style.width, height: style.height,
      ringWidth: ring.width, ringHeight: ring.height, ringLeft: ring.left, ringTop: ring.top };
  });
  expect(markerPaint).toEqual({ textAnimation: "none", textOpacity: "1", numeralOpacity: "1",
    ringAnimation: "farhelm-status-pulse", ringTiming: "steps(4)", ringDuration: "2s",
    color: appearance === "dark" ? "rgb(229, 187, 110)" : "rgb(121, 83, 27)",
    ringColor: appearance === "dark" ? "rgb(229, 187, 110)" : "rgb(121, 83, 27)",
    width: "17px", height: "17px", ringWidth: "15px", ringHeight: "15px", ringLeft: "-1px", ringTop: "-1px" });
  await page.emulateMedia({ reducedMotion: "reduce" });
  expect(await marker.evaluate((element) => getComputedStyle(element, "::after").animationName)).toBe("none");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  expect(await popup.evaluate((node) => getComputedStyle(node).position)).toBe("fixed");
  expect(await popupTextVisible(popup), "the current step, count, and clock are readable").toEqual({
    current: true,
    count: true,
    elapsed: true,
  });
  await page.mouse.move(0, 0);
  await expect(popup).toHaveCount(0);

  // With the label near the viewport's bottom edge there is no room below it,
  // so the popup opens above it, or, when the run's step list is too tall for
  // that too, is pinned inside the viewport over the label: fixed positioning
  // escapes the sidebar's clipping but not the viewport's, and a popup past
  // either edge would hide the steps it exists to show.
  const originalViewport = page.viewportSize()!;
  const labelBottom = await row
    .locator(".host-update-running")
    .evaluate((node) => node.getBoundingClientRect().bottom);
  await page.setViewportSize({
    width: originalViewport.width,
    height: Math.ceil(labelBottom) + 20,
  });
  await row.locator(".host-update-count").hover();
  await expect(popup).toBeVisible();
  const placement = await popup.evaluate((node) => {
    const box = node.getBoundingClientRect();
    const label = node.parentElement!.getBoundingClientRect();
    return {
      inside: box.top >= 0 && box.bottom <= window.innerHeight,
      notBelow: box.top < label.bottom,
    };
  });
  expect(placement).toEqual({ inside: true, notBelow: true });
  // However short the window, the list gives up height, not the lines that
  // matter: the current step (scrolled into the list's view), the count,
  // and the clock stay readable.
  expect(await popupTextVisible(popup), "a short window keeps the current step, count, and clock").toEqual({
    current: true,
    count: true,
    elapsed: true,
  });
  await page.mouse.move(0, 0);
  await expect(popup).toHaveCount(0);
  await page.setViewportSize(originalViewport);

  const progressDot = row.locator(".host-update-dot");
  expect(await progressDot.evaluate((dot) => getComputedStyle(dot).animationName)).toBe(
    "farhelm-status-pulse",
  );
  await page.emulateMedia({ reducedMotion: "reduce" });
  expect(await progressDot.evaluate((dot) => getComputedStyle(dot).animationName)).toBe("none");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-confirm")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);

  // Only the chosen host shows inline progress; the bystander stays ordinary.
  await expect(idle.locator(".host-detail")).toHaveCount(0);
  await expect(idle.locator(".provisioning-run")).toHaveCount(0);

  await openHostMenu(row);
  await expect(row.locator(".provisioning-update")).toHaveCount(0);
  await expect(row.locator(".provisioning-rerun")).toHaveCount(0);
  await expect(row.locator(".provisioning-auto-setup")).toHaveCount(0);
  await page.keyboard.press("Escape");

  // Acceptance releases the page lock while the run continues: unrelated
  // controls stay usable, while this row's own edit waits for its run.
  await expect(page.locator(".host-rail-add")).toBeEnabled();
  // `.host-settings` lives inside the row's own "⋯" menu now — see the
  // earlier ADD case's comment for why `toBeDisabled()` still applies.
  await openHostMenu(row);
  await expect(row.locator(".host-settings")).toBeDisabled();
  await page.keyboard.press("Escape");

  // The run remains held, but has released the page lock and cannot open a
  // confirmation. Another host's Settings must actually open, not merely look
  // enabled; this distinguishes the row's own busy guard from a global one.
  expect((await progress(request, accepted.host_id)).status).toBe("running");
  await openHostMenu(idle);
  await expect(idle.locator(".host-settings")).not.toHaveAttribute("aria-disabled", "true");
  await idle.locator(".host-settings").click();
  await expect(page.locator(".host-settings-dialog")).toBeVisible();
  await page.locator(".host-settings-close").click();

  // Authoritative success hands the status spot back to the ordinary label;
  // the row stays folded, the checkbox stays untouched, and nothing replays.
  await configureBackend();
  await waitForProgress(request, accepted.host_id, "completed");
  await expectOrdinaryStatus(row);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".provisioning-trace")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});
}

/**
 * Setup's automatic expansion, failure reopening and success folding move
 * rows below it. Each boundary must close a fixed menu measured before that
 * move, even though the host list itself has not changed.
 */
test("setup disclosure transitions invalidate fixed-surface geometry", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "trace-shape");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  let view: Progress = {
    run_id: "trace-shape",
    operation: "add",
    status: "completed",
    steps: [],
    message: null,
  };
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({ host_id: accepted.host_id, ...view }),
    });
  });

  await page.goto("/");
  await showHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();

  for (const status of ["running", "failed", "completed"] as const) {
    await openHostMenu(row);
    await expect(row.locator(".host-row-menu-panel")).toBeVisible();
    view = { ...view, status };
    await notifyFeed(feed, status === "running" ? 2 : status === "failed" ? 3 : 4);
    await expect(row.locator(".host-row-menu-panel")).toHaveCount(0);
    await expect(row.locator(".host-row-menu")).toHaveAttribute("aria-expanded", "false");
    if (status === "completed") {
      await expect(row.locator(".host-detail")).toHaveCount(0);
    } else {
      await expect(row.locator(".provisioning-run")).toHaveAttribute(
        "data-provisioning-status",
        status,
      );
      if (status === "running") {
        // A user fold persists for this run. Failure must reopen it, giving
        // the next iteration a real layout change to invalidate the menu.
        await row.locator(".provisioning-run-fold").click();
        await expect(row.locator(".host-detail")).toHaveCount(0);
      }
    }
    await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  }
});

/**
 * Invalidation must survive delivery of a plan already made for the old target.
 * Holding the request before server execution would plan the new target instead;
 * both legs wait for their released reply, then observe a renderer window before
 * checking for an unintended submission. Network completion alone could let an
 * already-satisfied negative assertion run before the UI handles the reply.
 */
test("a retarget and a foreign run each invalidate an unsubmitted update", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "binding");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  const updates = countUpdateRequests(page, accepted.host_id);
  let releasePlanning!: () => void;
  const planningGate = new Promise<void>((resolve) => {
    releasePlanning = resolve;
  });
  let planningCaptured!: () => void;
  const oldPlanCaptured = new Promise<void>((resolve) => { planningCaptured = resolve; });
  let holdPlanning = true;
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (holdPlanning && !route.request().postData()) {
      const response = await route.fetch();
      expect(response.ok(), "the old-target plan must exist before retargeting").toBe(true);
      planningCaptured();
      await planningGate;
      await route.fulfill({ response });
      return;
    }
    await route.continue();
  });
  try {
    await page.goto("/");
    await showHostsPanel(page);
    await expect(page.locator(".host-details-toggle")).not.toBeChecked();
    const row = page.locator(`[data-host-id="${accepted.host_id}"]`);

    /**
     * Observe stale-plan handling without mistaking the foreign run's legitimate
     * progress panel for an old plan. Both legs forbid a pending intent or POST;
     * only the retarget leg requires the row's details to stay hidden.
     */
    const observeNoLatePlan = async (hiddenDetails: boolean) => {
      for (let observation = 0; observation < 10; observation++) {
        // sleep-ok: after positive reply receipt, sample a 500 ms decoding/reactive-submission window with the request counter continuously armed.
        await page.waitForTimeout(50);
        expect(updates.confirms).toBe(0);
        await expect(row.locator(".host-update-pending")).toHaveCount(0);
        if (hiddenDetails) await expect(row.locator(".host-detail")).toHaveCount(0);
        else await expect(row.locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "running");
        await expect(page.locator(".host-details-toggle")).not.toBeChecked();
      }
    };

    // Retarget before plan completion: the held planning reply arrives for a
    // row that no longer matches, so the intent dies and nothing submits.
    await openHostMenu(row);
    await row.locator(".provisioning-update").dispatchEvent("click");
    await expect(pendingUpdate(row, "planning")).toBeVisible();
    await oldPlanCaptured;
    const changed = `${remote}-moved`;
    const response = await request.post(`/api/hosts/${accepted.host_id}/destination`, {
      data: { ssh: changed },
    });
    expect(response.ok(), await responseBody(response)).toBe(true);
    await notifyFeed(feed, 2);
    // The refresh delivering the new binding invalidates the intent while
    // planning is still held: the pending status going away proves the
    // watcher ran, before the stale reply is even released.
    await expect(row.locator(".host-update-pending")).toHaveCount(0);
    const oldReply = page.waitForResponse((reply) =>
      new URL(reply.url()).pathname === `/api/hosts/${accepted.host_id}/update`
      && reply.request().method() === "POST" && !reply.request().postData());
    holdPlanning = false;
    releasePlanning();
    expect(await (await oldReply).finished(), "the captured old-target reply must finish arriving").toBeNull();
    await observeNoLatePlan(true);
    await expect(row.locator(".host-detail")).toHaveCount(0);
    await expect(page.locator(".host-details-toggle")).not.toBeChecked();
    expect(updates.plans).toBe(1);
    expect(updates.confirms).toBe(0);

    // A foreign running update during planning invalidates the local intent
    // instead of queueing behind it: the row follows the observed run with
    // an explanation, and still submits nothing of its own.
    await configureBackend({ targets: { [target(changed)]: { hold_actions: true } } });
    holdPlanning = true;
    const secondGate = new Promise<void>((resolve) => {
      releasePlanning = resolve;
    });
    // A promise gate cannot be re-armed, so the second hold re-registers
    // the same route shape around a fresh gate.
    await page.unroute(`**/api/hosts/${accepted.host_id}/update`);
    await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
      if (holdPlanning && !route.request().postData()) {
        await secondGate;
      }
      await route.continue();
    });
    await openHostMenu(row);
    await row.locator(".provisioning-update").dispatchEvent("click");
    await expect(pendingUpdate(row, "planning")).toBeVisible();
    const competingPlan = await request.post(`/api/hosts/${accepted.host_id}/update`);
    expect(competingPlan.ok(), await responseBody(competingPlan)).toBe(true);
    const competing = (await competingPlan.json()) as { probe_id: string };
    const competingRun = await request.post(`/api/hosts/${accepted.host_id}/update`, {
      data: { probe_id: competing.probe_id },
    });
    expect(competingRun.status()).toBe(202);
    await notifyFeed(feed, 3);
    await expect(row.locator(".provisioning-warning")).toContainText(
      "another provisioning run started on this host first",
    );
    await expect(row.locator(".provisioning-run")).toHaveAttribute(
      "data-provisioning-status",
      "running",
    );
    const foreignReply = page.waitForResponse((reply) =>
      new URL(reply.url()).pathname === `/api/hosts/${accepted.host_id}/update`
      && reply.request().method() === "POST" && !reply.request().postData());
    holdPlanning = false;
    releasePlanning();
    expect(await (await foreignReply).finished(), "the released foreign-run planning reply must finish arriving").toBeNull();
    await observeNoLatePlan(false);
    await expect(page.locator(".host-details-toggle")).not.toBeChecked();
    expect(updates.plans).toBe(2);
    expect(updates.confirms).toBe(0);
  } finally {
    // Neither captured reply may strand route teardown after an assertion fails.
    releasePlanning();
  }
});

/** A failed Update plan must show the helm's concrete, peer-safe reason in a
 * modal without treating planning as a submitted run or changing Details. */
test("UPDATE planning refusal shows the concrete reason in a dialog", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update-plan-refusal");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({
    targets: {
      [target(remote)]: { inspect: "error", message: "inspection refused \u202Ehost\u200B" },
    },
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  const refusal = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/hosts/${accepted.host_id}/update`
    && response.request().method() === "POST",
  );
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await refusal;
  await expect(page.locator(".host-refusal-dialog .host-refusal-reason")).toContainText(
    'injected provisioning inspection failure: host stderr "inspection refused \\u{202e}host\\u{200b}"',
    { timeout: 10_000 },
  );
  // Planning failure submits nothing and mints no confirmation. Its modal
  // does not change the user's global row disclosure preference.
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-confirm")).toHaveCount(0);
  await expect(row.locator(".provisioning-error")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(0);
});

test("a Busy-refused UPDATE consumes its plan and leaves unrelated controls usable", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update-refusal");
  const lockHost = destination(testInfo, "update-refusal-lock");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // This is the stale-client refusal case. Keep this page on its last
  // completed view while the API client starts a competing run; otherwise
  // ordinary progress reconciliation correctly invalidates the waiting
  // intent before submission can exercise the helm's synchronous Busy
  // refusal. The held ADD confirm is what keeps the plan waiting.
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: accepted.host_id,
        run_id: "stale-completed-run",
        operation: "add",
        status: "completed",
        steps: [],
        message: null,
      }),
    });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const lock = await holdLockWithAdd(page, lockHost);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  // The plan is minted and stored, waiting behind the held lock: the
  // submission below is this exact stale plan, not a fresh one.
  await expect(pendingUpdate(row, "waiting")).toBeVisible();
  await expect(row.locator(".host-detail")).toHaveCount(0);
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(0);

  const competingPlan = await request.post(`/api/hosts/${accepted.host_id}/update`);
  const competing = (await competingPlan.json()) as { probe_id: string };
  const competingRun = await request.post(`/api/hosts/${accepted.host_id}/update`, {
    data: { probe_id: competing.probe_id },
  });
  expect(competingRun.status()).toBe(202);

  lock.release();
  await expect(row.locator(".provisioning-error")).toContainText("in flight");
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-confirm")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await expect(page.locator(".host-rail-add")).toBeEnabled();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

test("a wrong-host UPDATE acceptance warns on its source row and claims nothing", async ({
  page,
  request,
}, testInfo) => {
  const first = destination(testInfo, "returned-from");
  const second = destination(testInfo, "returned-to");
  const from = await startAdd(request, first);
  const to = await startAdd(request, second);
  await waitForProgress(request, from.host_id, "completed");
  await waitForProgress(request, to.host_id, "completed");
  // A current connection may clear change-of-host advice only. A wrong-host
  // acceptance remains uncertain even when this source host is healthy.
  await page.route("**/api/hosts", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const response = await route.fetch();
    const body = await response.json();
    const entry = body.hosts.find((host: Host) => host.id === from.host_id);
    if (entry) {
      entry.state = {
        phase: "connected",
        identity: entry.identity,
        build_version: HELM_BUILD,
        refresh: { status: "ok", sessions: 0 },
      };
    }
    await route.fulfill({ response, json: body });
  });
  await page.route(`**/api/hosts/${from.host_id}/update`, async (route) => {
    if (!route.request().postData()) return route.continue();
    await route.fulfill({
      status: 202,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({ host_id: to.host_id, run_id: "rerouted-run" }),
    });
  });
  const updates = countUpdateRequests(page, from.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${from.host_id}"]`);
  await expect(row).toHaveAttribute("data-host-phase", "connected");
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  // UPDATE is host-bound, so a foreign acceptance is a protocol warning on
  // the row that submitted — never routed progress, never claimed success.
  // ADD keeps its legitimate returned-row routing; this test pins UPDATE's.
  await expect(row.locator(".provisioning-warning")).toContainText(
    `accepted this update for host ${to.host_id} instead of this row`,
  );
  await expect(row.locator(".provisioning-run[data-provisioning-status='running']")).toHaveCount(0);
  await expect(row.locator(".provisioning-error")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  // No run was claimed, so this row's own edit stays usable throughout.
  await openHostMenu(row);
  await expect(row.locator(".host-settings")).toBeEnabled();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

test("a malformed accepted UPDATE warns, survives old success, and never replays", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update-unvalidated");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (!route.request().postData()) return route.continue();
    await route.fulfill({
      status: 202,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: "{not-json",
    });
  });
  // After the lost acceptance, serve one stale success for an older run,
  // then the real backend state. Neither may clear the uncertainty.
  let serveStale = true;
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    if (!serveStale) return route.continue();
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: accepted.host_id,
        run_id: "older-update-run",
        operation: "update",
        status: "completed",
        steps: [],
        message: "stale success marker",
      }),
    });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");

  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toContainText("accepted");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await expect(page.locator(".host-rail-add")).toBeEnabled();
  // The stale older-run success lands and changes the snapshot, but the
  // lost acceptance stays unresolved and the row stays expanded.
  await expect(row.locator(".provisioning-run-message")).toContainText("stale success marker");
  await expect(row.locator(".provisioning-warning")).toContainText("accepted");
  await expect(row.locator(".host-detail")).toBeVisible();
  serveStale = false;
  await notifyFeed(feed, 2);
  // The recovered backend view is the retained ADD run with no message, so
  // its arrival removes the message element rather than changing its text:
  // barrier on the recovered view first, then assert the absence.
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-operation",
    "setup",
  );
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  await expect(row.locator(".provisioning-run-message")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toContainText("accepted");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

test("a lost submission reply survives a failed retry plan, a refused retry submission, and unrelated completed progress", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update-lost-reply");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // The first submission POST loses its reply in transport. From the
  // client's side that is indistinguishable from a run that committed while
  // its answer was lost, so the uncertainty must outlive everything below.
  // Plans pass through, except the failed retries', which are held until the
  // planning indicator proves each attempt is really in flight. The second
  // submission is held until the submitting indicator proves it is in
  // flight, then answered with an explicit refusal.
  let submissions = 0;
  let plans = 0;
  let releaseRetryPlan!: () => void;
  const retryPlanGate = new Promise<void>((resolve) => {
    releaseRetryPlan = resolve;
  });
  let releaseSecondRetryPlan!: () => void;
  const secondRetryPlanGate = new Promise<void>((resolve) => {
    releaseSecondRetryPlan = resolve;
  });
  let releaseRefusal!: () => void;
  const refusalGate = new Promise<void>((resolve) => {
    releaseRefusal = resolve;
  });
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (route.request().postData()) {
      submissions += 1;
      if (submissions === 1) {
        await route.abort("failed");
        return;
      }
      if (submissions === 2) {
        await refusalGate;
        await route.fulfill({
          status: 409,
          headers: { "content-type": "text/plain", "x-farhelm-build": HELM_BUILD },
          body: "explicit test refusal marker",
        });
        return;
      }
      await route.continue();
      return;
    }
    plans += 1;
    if (plans === 2) {
      await retryPlanGate;
    }
    if (plans === 4) {
      await secondRetryPlanGate;
    }
    await route.continue();
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);

  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-error")).toContainText("its reply was lost");
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-confirm")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);

  // The retry's own plan fails. Its indicator appearing and then clearing is
  // the UI-side proof the failure was processed — and the processed failure
  // must not replace the earlier uncertainty, because the failed plan
  // provably submitted nothing while the first request may have committed.
  await configureBackend({
    targets: {
      [target(remote)]: { inspect: "error", message: "retry inspection refused" },
    },
  });
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-planning")).toBeVisible();
  releaseRetryPlan();
  await expect(row.locator(".provisioning-planning")).toHaveCount(0);
  await expect(page.locator(".host-refusal-dialog")).toContainText("retry inspection refused");
  await expect(row.locator(".provisioning-error")).toContainText("its reply was lost");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(2);
  expect(updates.confirms).toBe(1);
  await page.locator(".host-refusal-dialog .host-refusal-close").click();

  // The next retry plans successfully but its submission is definitely
  // refused. The submitting indicator proves the second POST is in flight;
  // its response plus the indicator clearing proves the refusal was
  // processed. That definite refusal describes only its own attempt, which
  // provably submitted nothing, so the earlier sticky uncertainty must
  // survive it — not be replaced by a clearable error.
  await configureBackend();
  const refusedSubmission = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/hosts/${accepted.host_id}/update`
    && response.request().method() === "POST"
    && !!response.request().postData()
  );
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-submitting")).toBeVisible();
  releaseRefusal();
  await refusedSubmission;
  await expect(row.locator(".provisioning-submitting")).toHaveCount(0);
  await expect(row.locator(".provisioning-error")).toContainText("its reply was lost");
  await expect(row.locator(".provisioning-error")).not.toContainText("explicit test refusal marker");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(3);
  expect(updates.confirms).toBe(2);

  // Another click after the refusal would clear a merely displayed refusal.
  // Its planning indicator appearing and clearing proves the fourth attempt
  // was processed, and the original uncertainty must still stand.
  await configureBackend({
    targets: {
      [target(remote)]: { inspect: "error", message: "second retry inspection refused" },
    },
  });
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-planning")).toBeVisible();
  releaseSecondRetryPlan();
  await expect(row.locator(".provisioning-planning")).toHaveCount(0);
  await expect(page.locator(".host-refusal-dialog")).toContainText("second retry inspection refused");
  await expect(row.locator(".provisioning-error")).toContainText("its reply was lost");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(4);
  expect(updates.confirms).toBe(2);

  // Unrelated completed progress arrives last. It renders, but the original
  // uncertainty stays beside it.
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: accepted.host_id,
        run_id: "older-update-run",
        operation: "update",
        status: "completed",
        steps: [],
        message: "stale success marker",
      }),
    });
  });
  await notifyFeed(feed, 2);
  await expect(row.locator(".provisioning-run-message")).toContainText("stale success marker");
  await expect(row.locator(".provisioning-error")).toContainText("its reply was lost");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(4);
  expect(updates.confirms).toBe(2);
});

test("a progress read failure recovers on the next feed-driven real read", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "read-recovery");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  let fail = true;
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    if (fail) {
      await route.fulfill({
        status: 502,
        headers: { "content-type": "text/plain", "x-farhelm-build": HELM_BUILD },
        body: "injected progress read failure",
      });
    } else {
      await route.continue();
    }
  });
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await expect(row.locator(".provisioning-read-error")).toContainText(
    "injected progress read failure",
  );
  fail = false;
  await notifyFeed(feed, 2);
  await expect(row.locator(".provisioning-read-error")).toHaveCount(0);
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
});

test("a progress read failure during an update recovers on retry demand alone", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update-read-retry");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  let fail = false;
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    if (fail) {
      await route.fulfill({
        status: 502,
        headers: { "content-type": "text/plain", "x-farhelm-build": HELM_BUILD },
        body: "injected progress read failure",
      });
    } else {
      await route.continue();
    }
  });
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(row.locator(".host-detail")).toHaveCount(0);

  // A failed read opens the row to expose its diagnostic; the checkbox stays
  // out of that disclosure.
  fail = true;
  await notifyFeed(feed, 2);
  await expect(row.locator(".provisioning-read-error")).toContainText(
    "injected progress read failure",
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  // Recovery with no second notification: the reader's own retry demand
  // re-reads, clears only the read diagnostic, and the run is still
  // followed exactly where it was.
  fail = false;
  await expect(row.locator(".provisioning-read-error")).toHaveCount(0);
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
});

/** An already-expanded failure adds text without changing disclosure state.
 * A menu below it must close rather than keep acting beside a different row. */
test("expanded setup failure closes the menu below it", async ({ page, request }, testInfo) => {
  // Keep both hosts in view: a scroll caused by growth would exercise the
  // existing scroll-close path instead of the panel's resize observer.
  await page.setViewportSize({ width: 1280, height: 1600 });
  const first = destination(testInfo, "height-failure");
  const second = destination(testInfo, "height-menu");
  await configureBackend({ targets: { [target(first)]: { hold_actions: true } } });
  const one = await startAdd(request, first);
  const two = await startAdd(request, second);
  await waitForProgress(request, two.host_id, "completed");
  await page.goto("/");
  await openHostsPanel(page);
  const rowOne = page.locator(`[data-host-id="${one.host_id}"]`);
  const rowTwo = page.locator(`[data-host-id="${two.host_id}"]`);
  await expect(rowOne.locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "running");
  await expect(rowOne.locator(".host-detail")).toBeVisible();
  const panel = page.locator(".hosts-panel");
  const sidebar = page.locator(".sidebar-content");
  const scrollTop = await sidebar.evaluate((node) => node.scrollTop);
  const height = await panel.evaluate((node) => node.getBoundingClientRect().height);
  await openHostMenu(rowTwo);
  await expect(rowTwo.locator(".host-row-menu-panel")).toBeVisible();
  expect(await panel.evaluate((node) => node.getBoundingClientRect().height)).toBe(height);

  await configureBackend({ targets: { [target(first)]: {
    fail_action: "attach-supervisor",
    message: "attachment failed after setup was already expanded",
  } } });
  await waitForProgress(request, one.host_id, "failed");
  await expect(rowOne.locator(".provisioning-run-message")).toContainText("attachment failed");
  await expect.poll(() => panel.evaluate((node) => node.getBoundingClientRect().height)).toBeGreaterThan(height);
  expect(await sidebar.evaluate((node) => node.scrollTop)).toBe(scrollTop);
  await expect(rowTwo.locator(".host-row-menu-panel")).toHaveCount(0);
  await expect(rowTwo.locator(".host-row-menu")).toHaveAttribute("aria-expanded", "false");
});

test("two provisioning-busy rows reconcile independently", async ({
  page,
  request,
}, testInfo) => {
  const first = destination(testInfo, "busy-one");
  const second = destination(testInfo, "busy-two");
  await configureBackend({
    targets: {
      [target(first)]: { hold_actions: true },
      [target(second)]: { hold_actions: true },
    },
  });
  const one = await startAdd(request, first);
  const two = await startAdd(request, second);
  await page.goto("/");
  await openHostsPanel(page);
  const rowOne = page.locator(`[data-host-id="${one.host_id}"]`);
  const rowTwo = page.locator(`[data-host-id="${two.host_id}"]`);
  // `.host-settings` lives inside each row's own "⋯" menu now, and only one
  // row menu is ever open at a time (see `HostsPanel`'s own "one row menu
  // open" doc) — opening the SECOND row's closes the first's, so the two
  // rows are checked one at a time rather than simultaneously. That is a
  // change to how this test OBSERVES the two rows, not to what it proves:
  // each row's own busy state still reconciles independently of the
  // other's, which is what every assertion below still pins.
  //
  // An open host menu now closes when a row above it changes its visible
  // lines, or when the panel resizes or a setup changes status, since the
  // menu would otherwise point at stale geometry. Both hosts here are
  // unreachable, so the helm's connection retries keep rewriting row one's
  // attempt line (Details is on), and row one's completion reshapes the
  // panel; a menu opened just before such a change can close before its
  // item is read. The page's own setup status is the premise for each
  // phase, and each open-and-check is retried as one unit, well inside the
  // backend's 30 s action hold. A Settings item in the wrong state fails
  // every attempt.
  const running = (row: Locator) => row.locator('[data-provisioning-status="running"]');
  const settingsReads = async (row: Locator, enabled: boolean) => {
    await expect(async () => {
      await openHostMenu(row);
      const settings = row.locator(".host-settings");
      if (enabled) await expect(settings).toBeEnabled({ timeout: 1_000 });
      else await expect(settings).toBeDisabled({ timeout: 1_000 });
    }).toPass({ timeout: 8_000 });
  };
  await expect(running(rowOne), "premise: row one's setup is running on the page").not.toHaveCount(0);
  await expect(running(rowTwo), "premise: row two's setup is running on the page").not.toHaveCount(0);
  await settingsReads(rowOne, false);
  await settingsReads(rowTwo, false);
  await expect(page.locator(".host-rail-add")).toBeEnabled();

  await configureBackend({ targets: { [target(second)]: { hold_actions: true } } });
  await waitForProgress(request, one.host_id, "completed");
  await expect(running(rowOne), "the page has caught up with row one's completion").toHaveCount(0);
  await settingsReads(rowOne, true);
  await expect(running(rowTwo), "premise: row two's setup is still held").not.toHaveCount(0);
  await settingsReads(rowTwo, false);
});

/** A retained setup failure opens its rail without a global Details change;
 * rerun probes the saved destination and can recover by discovery alone. */
test("failed ADD rerun probes the registered destination and discovery resolves the run", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "failed-rerun");
  await configureBackend({
    targets: {
      [target(remote)]: {
        fail_action: "attach-supervisor",
        message: "supervisor started but attachment failed",
      },
    },
  });
  const accepted = await startAdd(request, remote);
  const failed = await waitForProgress(request, accepted.host_id, "failed");
  expect(failed.steps.find((step) => step.step === "attach-supervisor")?.status).toBe("failed");
  await configureBackend({
    targets: {
      [target(remote)]: {
        probe: "supervisor",
        identity: `recovered-${testInfo.project.name}`,
      },
    },
  });
  await page.goto("/");
  await showHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await expect(row.locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "failed");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await openHostsPanel(page);
  await expect(row.locator(".provisioning-run-message")).toContainText(
    "supervisor started but attachment failed",
  );
  await openHostMenu(row);
  await row.locator(".provisioning-rerun").click();

  await expect.poll(async () => (await progress(request, accepted.host_id)).status).toBe("completed");
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  expect((await backendEvents()).some((event) =>
    event.event === "probe" && event.target === target(remote),
  )).toBe(true);
});

// A retained failure remembers which operation produced it. This keeps the
// UPDATE arm honest: rerun must mint and consume an UPDATE plan without
// falling back to discovery or the ADD confirmation route — and a failed
// remote UPDATE reruns down the same automatic path as a fresh update.
test("failed UPDATE rerun submits automatically through the host update route", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "failed-update-rerun");
  const added = await startAdd(request, remote);
  await waitForProgress(request, added.host_id, "completed");
  await configureBackend({
    targets: {
      [target(remote)]: {
        fail_action: "restart-supervisor",
        message: "restart failed during update",
      },
    },
  });
  const firstPlanResponse = await request.post(`/api/hosts/${added.host_id}/update`);
  expect(firstPlanResponse.ok(), await responseBody(firstPlanResponse)).toBe(true);
  const firstPlan = (await firstPlanResponse.json()) as { probe_id: string };
  const firstRunResponse = await request.post(`/api/hosts/${added.host_id}/update`, {
    data: { probe_id: firstPlan.probe_id },
  });
  expect(firstRunResponse.status(), await responseBody(firstRunResponse)).toBe(202);
  const failed = await waitForProgress(request, added.host_id, "failed");
  expect(failed.operation).toBe("update");

  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const updateRequests: ("plan" | "confirm")[] = [];
  let addProbeRequests = 0;
  page.on("request", (outgoing) => {
    const pathname = new URL(outgoing.url()).pathname;
    if (pathname === "/api/hosts/probe" && outgoing.method() === "POST") {
      addProbeRequests += 1;
    }
    if (pathname === `/api/hosts/${added.host_id}/update` && outgoing.method() === "POST") {
      updateRequests.push(outgoing.postData() ? "confirm" : "plan");
    }
  });
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${added.host_id}"]`);
  // The retained failed UPDATE keeps its own row expanded from mount, with
  // the checkbox still unchecked.
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "failed",
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await openHostMenu(row);
  const toggle = row.locator(".host-row-menu");
  await row.locator(".provisioning-rerun").focus();
  await row.locator(".provisioning-rerun").dispatchEvent("click");
  await expect(row.locator(".host-row-menu-panel")).toHaveCount(0);
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  await expect(toggle).toBeFocused();
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  // Automatic, like a fresh update: no plan rendered, no confirmation.
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-confirm")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const running = await waitForProgress(request, added.host_id, "running");
  expect(running.operation).toBe("update");
  expect(updateRequests).toEqual(["plan", "confirm"]);
  expect(addProbeRequests).toBe(0);
});

/**
 * One fleet click must retain the individual Update authority and result for
 * every remote host. The local row is deliberately present: touching it would
 * be a new provisioning path, not a harmless extra request.
 */
test("update all dispatches remote hosts and keeps their results independent", async ({
  page,
  request,
}, testInfo) => {
  // The shared stack already has a registered SSH host. The batch contract
  // includes it too, so capture that fixture before this test adds rows.
  const inheritedRemoteIds = (await hosts(request))
    .filter((host) => host.kind === "ssh")
    .map((host) => host.id);
  const first = destination(testInfo, "overlap-one");
  const second = destination(testInfo, "overlap-two");
  const one = await startAdd(request, first);
  const two = await startAdd(request, second);
  await waitForProgress(request, one.host_id, "completed");
  await waitForProgress(request, two.host_id, "completed");
  await configureBackend({
    targets: {
      [target(first)]: { hold_actions: true },
      [target(second)]: { hold_actions: true },
    },
  });
  const updatesOne = countUpdateRequests(page, one.host_id);
  const updatesTwo = countUpdateRequests(page, two.host_id);
  const inheritedUpdates = inheritedRemoteIds.map((id) => ({
    id,
    counts: countUpdateRequests(page, id),
  }));
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const local = page.locator('.host-row[data-host-kind="local"]');
  await expect(local).toHaveCount(1);
  const localIdAttribute = await local.getAttribute("data-host-id");
  expect(localIdAttribute).not.toBeNull();
  const localId = Number(localIdAttribute);
  expect(Number.isInteger(localId)).toBe(true);
  const localUpdates = countUpdateRequests(page, localId);
  const rowOne = page.locator(`[data-host-id="${one.host_id}"]`);
  const rowTwo = page.locator(`[data-host-id="${two.host_id}"]`);
  const updateAll = page.getByRole("button", { name: "update all" });
  await expect(updateAll).toBeEnabled();
  // Both activations land before a reactive rerender. Per-row ownership must
  // consume only one request each, even when the header is clicked twice.
  await page.evaluate(() => {
    const button = document.querySelector(".update-all-button")!;
    button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
  await expect(rowOne.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(rowTwo.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(rowOne.locator(".host-detail")).toHaveCount(0);
  await expect(rowTwo.locator(".host-detail")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();

  // The inherited remote can finish while the held test rows remain busy;
  // that makes the header action available again and is expected. Its own
  // plan and submission still prove this click covered every remote row.
  for (const inherited of inheritedUpdates) {
    await waitForProgress(request, inherited.id, "completed");
    expect(inherited.counts).toEqual({ plans: 1, confirms: 1 });
  }

  await configureBackend({ targets: { [target(second)]: { hold_actions: true } } });
  await waitForProgress(request, one.host_id, "completed");
  // Success returns the first row's ordinary status; the second keeps
  // showing progress without opening its details.
  await expectOrdinaryStatus(rowOne);
  await expect(rowOne.locator(".host-detail")).toHaveCount(0);
  await expect(rowTwo.locator(".host-detail")).toHaveCount(0);
  await expect(rowTwo.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updatesOne.plans).toBe(1);
  expect(updatesOne.confirms).toBe(1);
  expect(updatesTwo.plans).toBe(1);
  expect(updatesTwo.confirms).toBe(1);
  expect(localUpdates).toEqual({ plans: 0, confirms: 0 });

  // The second host fails after the first has already succeeded. Its row
  // retains the failure while the first row stays folded after its success.
  await configureBackend({
    targets: {
      [target(second)]: { fail_action: "restart-supervisor" },
    },
  });
  await waitForProgress(request, two.host_id, "failed");
  await expect(rowTwo.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "failed",
  );
  await expect(rowTwo.locator(".host-detail")).toBeVisible();
  await expect(rowOne.locator(".host-detail")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
});

/**
 * A fleet click takes only the Update offers available at that instant.
 * A setup run already accepted by the helm must never inherit a delayed
 * UPDATE after it finishes, while another remote host can still proceed.
 */
test("update all skips a host already busy with setup", async ({ page, request }, testInfo) => {
  const ready = destination(testInfo, "update-all-ready");
  const busy = destination(testInfo, "update-all-busy");
  const readyHost = await startAdd(request, ready);
  await waitForProgress(request, readyHost.host_id, "completed");
  await configureBackend({ targets: { [target(busy)]: { hold_actions: true } } });
  const busyHost = await startAdd(request, busy);
  await waitForProgress(request, busyHost.host_id, "running");
  const readyUpdates = countUpdateRequests(page, readyHost.host_id);
  const busyUpdates = countUpdateRequests(page, busyHost.host_id);

  await page.goto("/");
  await showHostsPanel(page);
  const busyRow = page.locator(`[data-host-id="${busyHost.host_id}"]`);
  // Observing an accepted ADD opens that run's rail even in a new window.
  // Its compact status remains the same authority that suppresses Update.
  await expect(busyRow.locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "running");
  await expect(busyRow.locator(".host-update-verb")).toHaveText("setting up");
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await openHostMenu(busyRow);
  await expect(busyRow.locator(".provisioning-update")).toHaveCount(0);
  const updateAll = page.getByRole("button", { name: "update all" });
  await expect(updateAll).toBeEnabled();
  await updateAll.click();
  await expect.poll(() => readyUpdates.confirms).toBe(1);
  await waitForProgress(request, readyHost.host_id, "completed");
  expect(readyUpdates).toEqual({ plans: 1, confirms: 1 });
  expect(busyUpdates).toEqual({ plans: 0, confirms: 0 });

  await configureBackend();
  await waitForProgress(request, busyHost.host_id, "completed");
  await openHostMenu(busyRow);
  await expect(busyRow.locator(".provisioning-update")).toBeVisible();
  expect(busyUpdates).toEqual({ plans: 0, confirms: 0 });
});

test("a plan held under a held OpLock submits exactly once on release", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "held-claim");
  const lockHost = destination(testInfo, "held-claim-lock");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const lock = await holdLockWithAdd(page, lockHost);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  // Planning ignores the held lock; the minted plan then waits for the
  // claim, sending nothing until the token frees. The folded row reports
  // that stage through its pending status.
  await expect(pendingUpdate(row, "waiting")).toBeVisible();
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-confirm")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(0);
  // No second intent is offerable through the wait.
  await openHostMenu(row);
  await expect(row.locator(".provisioning-update")).toHaveCount(0);
  await expect(row.locator(".provisioning-rerun")).toHaveCount(0);
  await page.keyboard.press("Escape");

  lock.release();
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".host-update-pending")).toHaveCount(0);
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

test("a retarget and a removal before the claim each submit nothing", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "claim-race");
  const lockHost = destination(testInfo, "claim-race-lock");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const hold = await holdLockWithAdd(page, lockHost);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);

  // Retarget while waiting: the stored plan belongs to the old row, so the
  // intent dies and the released lock submits nothing.
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(pendingUpdate(row, "waiting")).toBeVisible();
  const changed = `${remote}-moved`;
  const response = await request.post(`/api/hosts/${accepted.host_id}/destination`, {
    data: { ssh: changed },
  });
  expect(response.ok(), await responseBody(response)).toBe(true);
  await notifyFeed(feed, 2);
  await expect(row.locator(".host-update-pending")).toHaveCount(0);
  // The intent is dead while the lock is still held, so the claim is
  // impossible on both sides of the release. Subscribe before resolving:
  // the released confirm's response plus the add-host button re-enabling
  // prove the release round-tripped and the UI observed it, so the
  // zero-submission count is not checked in a dead window.
  const claimRelease = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/api/hosts/provision" &&
      response.request().method() === "POST",
  );
  hold.release();
  await claimRelease;
  await expect(page.locator(".host-rail-add")).toBeEnabled();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(0);

  // Removal while waiting: the row (and its intent) is gone before the
  // lock frees, so again nothing submits.
  const holdAgain = await holdLockWithAdd(page, `${lockHost}-again`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(pendingUpdate(row, "waiting")).toBeVisible();
  const removed = await request.delete(`/api/hosts/${accepted.host_id}`);
  expect(removed.ok(), await responseBody(removed)).toBe(true);
  await notifyFeed(feed, 3);
  await expect(row).toHaveCount(0);
  // Same release barrier as above: the released confirm's response plus
  // the add-host button re-enabling precede the zero-submission count.
  const claimReleaseAgain = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/api/hosts/provision" &&
      response.request().method() === "POST",
  );
  holdAgain.release();
  await claimReleaseAgain;
  await expect(page.locator(".host-rail-add")).toBeEnabled();
  expect(updates.plans).toBe(2);
  expect(updates.confirms).toBe(0);
});

test("removal before plan completion submits nothing", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "plan-race-remove");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // Fetch the planning reply first and hold the already-obtained response:
  // holding the request before it reaches the backend would let the removal
  // below turn it into a 404, exercising the error path instead of the
  // claimed stale successful plan.
  let releasePlanning!: () => void;
  const planningGate = new Promise<void>((resolve) => {
    releasePlanning = resolve;
  });
  let planningFetched!: () => void;
  const planningReady = new Promise<void>((resolve) => {
    planningFetched = resolve;
  });
  let planningStatus = 0;
  // Set once the test observes the page abort the held planning request;
  // fulfilling a dead interception would throw, so the handler skips it.
  let planningAborted = false;
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (route.request().postData()) return route.continue();
    const planning = await route.fetch();
    planningStatus = planning.status();
    planningFetched();
    await planningGate;
    if (!planningAborted) await route.fulfill({ response: planning });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(pendingUpdate(row, "planning")).toBeVisible();
  // The held reply is an already-obtained successful plan before the row is
  // removed: the stale-plan premise, not a removal 404.
  await planningReady;
  expect(planningStatus).toBe(200);
  // Subscribe before the removal: unmounting the row aborts the in-flight
  // planning request, and that abort is the observable boundary — the held
  // reply can never be delivered, so awaiting a response would hang.
  const planningRequestDead = page.waitForEvent("requestfailed", (request) =>
    new URL(request.url()).pathname === `/api/hosts/${accepted.host_id}/update` &&
    request.method() === "POST" &&
    !request.postData()
  );
  const removed = await request.delete(`/api/hosts/${accepted.host_id}`);
  expect(removed.ok(), await responseBody(removed)).toBe(true);
  await notifyFeed(feed, 2);
  await expect(row).toHaveCount(0);
  // The page tore down the planning request while the successful reply was
  // still held: the stale reply has no consumer because the row — and its
  // request — are gone.
  await planningRequestDead;
  planningAborted = true;
  releasePlanning();
  // One full refresh round-trip after the release proves the UI ran
  // reactive cycles past it instead of the count being checked in a dead
  // window; the removed row stays gone throughout.
  const refreshed = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/api/hosts" &&
      response.request().method() === "GET",
  );
  await notifyFeed(feed, 3);
  await refreshed;
  await expect(row).toHaveCount(0);
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(0);
});

test("a stale Completed around submission settles nothing and replays nothing", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "stale-around-submit");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // Hold the submission's 202 in transport: run A is admitted and held
  // running on the backend while the UI still waits for its acceptance.
  // Plans pass through; only the token submission waits for the release.
  let releaseAcceptance!: () => void;
  const acceptGate = new Promise<void>((resolve) => {
    releaseAcceptance = resolve;
  });
  let resolveAdmitted!: () => void;
  const submissionAdmitted = new Promise<void>((resolve) => {
    resolveAdmitted = resolve;
  });
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (!route.request().postData()) return route.continue();
    const response = await route.fetch();
    resolveAdmitted();
    await acceptGate;
    await route.fulfill({ response });
  });
  // Real backend reads until the test flips to a uniquely marked stale
  // success AFTER acceptance installs. The marker can only arrive on a read
  // dispatched after that installation, so awaiting it proves a
  // post-acceptance stale commit — not a mount-time view, and not a read
  // that raced the outstanding POST.
  let serveStale = false;
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    if (!serveStale) return route.continue();
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: accepted.host_id,
        run_id: "stale-update-run-post-acceptance",
        operation: "update",
        status: "completed",
        steps: [],
        message: "post-acceptance stale success marker",
      }),
    });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  // A is admitted and held running on the backend while its 202 waits. The
  // submitting stage plus the disabled add-host button prove the submission
  // is in flight and the page lock is held — the premise the acceptance
  // release below needs. Request counts here prove dispatch, nothing more.
  await submissionAdmitted;
  await expect(pendingUpdate(row, "submitting")).toBeVisible();
  await expect(page.locator(".host-rail-add")).toBeDisabled();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);

  // Release the 202 and establish that the UI processed acceptance: the
  // response arrives, the intent ends, and the page lock frees, while the
  // backend still holds A running.
  const submitted = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === `/api/hosts/${accepted.host_id}/update` &&
      response.request().method() === "POST" &&
      response.request().postData() != null,
  );
  releaseAcceptance();
  const submitResponse = await submitted;
  expect(submitResponse.status(), await responseBody(submitResponse)).toBe(202);
  const submittedRun = ((await submitResponse.json()) as Accepted).run_id;
  await expect(pendingUpdate(row, "submitting")).toHaveCount(0);
  await expect(page.locator(".host-rail-add")).toBeEnabled();
  // The acceptance's own reread observes held running A, so the accepted
  // run is reconciled before the stale detour begins: only the tracked
  // run's own running view produces full inline progress.
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(row.locator(".host-detail")).toHaveCount(0);

  // The stale older-run success lands on a read dispatched after that
  // installation. It renders, yet settles nothing: the tracked run's unseen
  // end becomes an uncertainty warning, which is what opens the row.
  serveStale = true;
  const provisioningPath = `/api/hosts/${accepted.host_id}/provisioning`;
  const staleRead = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === provisioningPath &&
      response.request().method() === "GET",
  );
  await notifyFeed(feed, 2);
  await staleRead;
  await expect(row.locator(".provisioning-run-message")).toContainText(
    "post-acceptance stale success marker",
  );
  // The stale view names a different run than the tracked one, so the
  // tracked run's unseen end is retained as uncertainty — reconciliation
  // evidence beyond the rendered marker.
  await expect(row.locator(".provisioning-warning")).toContainText(
    `update run ${submittedRun} ended without reporting its outcome`,
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  // Ownership survives the stale Completed: the accepted run is retained
  // with no live intent, so the row stays busy and offers no second
  // Update. A repeated attempt mints nothing. Synchronous admission for
  // this owned-without-intent state is covered independently of menu
  // hiding by the `update_ownership_survives_acceptance_until_a_terminal_state`
  // unit test (accepted gap and Running both owned, terminal states free)
  // plus `decide_request`'s coalesce-behind-ownership case.
  await openHostMenu(row);
  await expect(row.locator(".provisioning-update")).toHaveCount(0);
  await expect(row.locator(".host-settings")).toBeDisabled();
  await page.keyboard.press("Escape");
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
  serveStale = false;
  await notifyFeed(feed, 3);
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
  // The stale detour poisons nothing: the tracked run's own completion
  // still settles the row, including the uncertainty it caused.
  await configureBackend();
  await waitForProgress(request, accepted.host_id, "completed");
  await notifyFeed(feed, 4);
  await expectOrdinaryStatus(row);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

/**
 * An update whose run completes before the UI ever reads it still settles.
 *
 * Why this matters: the accepted-but-unread gap is real (a fast update can
 * finish between the 202 and the next progress read), and the row must not
 * stay stuck on `updating…` or report the outcome as unknown just because
 * Running was never observed. Specifies: `updating…` from the click, through
 * planning and the unread gap, with the row folded; then the first read
 * already reports the exact accepted run completed, and that alone returns
 * the ordinary status, opens nothing, and offers Update again.
 */
test("an update shows pending status from acceptance and settles on immediate completion", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "immediate");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  // The backend runs unheld; the test — not timing — establishes that the
  // accepted run is never observed Running. Progress reads flow until the
  // Update click, then hold: every read that could observe the new run
  // fetches only after that run completes, so the first applicable
  // response already carries its completion.
  let releasePlanning!: () => void;
  const planningGate = new Promise<void>((resolve) => {
    releasePlanning = resolve;
  });
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (!route.request().postData()) {
      await planningGate;
    }
    await route.continue();
  });
  let holdProgress = false;
  let releaseProgress!: () => void;
  const progressGate = new Promise<void>((resolve) => {
    releaseProgress = resolve;
  });
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    if (holdProgress) {
      await progressGate;
    }
    await route.continue();
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  // The status spot says `updating…` from click acceptance, before planning
  // returns, while the row stays folded and the checkbox unchecked. The
  // held planning request is what guarantees no new run exists yet.
  await expect(pendingUpdate(row, "planning")).toBeVisible();
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  // No new run can exist before the planning release below, so every read
  // that passed so far saw only the ADD run; every later read waits for
  // the new run's completion.
  holdProgress = true;
  const submitted = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === `/api/hosts/${accepted.host_id}/update` &&
      response.request().method() === "POST" &&
      response.request().postData() != null,
  );
  releasePlanning();
  const submitResponse = await submitted;
  expect(submitResponse.status(), await responseBody(submitResponse)).toBe(202);
  const submittedRun = ((await submitResponse.json()) as Accepted).run_id;
  // Correlate completion with the new accepted run id: polling for bare
  // "completed" could return the earlier ADD run before submission lands.
  await expect
    .poll(async () => await progress(request, accepted.host_id))
    .toMatchObject({ run_id: submittedRun, status: "completed" });
  // Acceptance installed while every progress read is still held: the row
  // follows the accepted run without any snapshot of it. This is the state
  // the release below must visibly leave.
  await expect(pendingUpdate(row, "awaiting-progress")).toBeVisible();
  releaseProgress();
  // Matching completion without ever observing Running still settles: the
  // ordinary status returns, no uncertainty warning opens the row, and the
  // released ownership offers Update again.
  await expectOrdinaryStatus(row);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await openHostMenu(row);
  await expect(row.locator(".provisioning-update")).toBeVisible();
  await page.keyboard.press("Escape");
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

test("global details alone control disclosure while update progress stays inline", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "toggle-row");
  const bystander = destination(testInfo, "toggle-bystander");
  const accepted = await startAdd(request, remote);
  const other = await startAdd(request, bystander);
  await waitForProgress(request, accepted.host_id, "completed");
  await waitForProgress(request, other.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  const idle = page.locator(`[data-host-id="${other.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(row.locator(".host-detail")).toHaveCount(0);

  // Global details opens every row while checked. Turning it back off folds
  // both rows, while the running update keeps its inline status.
  await page.locator(".host-details-toggle").click();
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(idle.locator(".host-detail")).toBeVisible();
  await page.locator(".host-details-toggle").click();
  await expect(idle.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);

  // Success with global details on stays detailed; turning them off then
  // collapses the now-resting row.
  await page.locator(".host-details-toggle").click();
  await configureBackend();
  await waitForProgress(request, accepted.host_id, "completed");
  await expect(row.locator(".host-detail")).toBeVisible();
  await page.locator(".host-details-toggle").click();
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".provisioning-trace")).toHaveCount(0);
});

// First contact after a protocol-skewed host is updated learns its identity.
// Exercise both continuity checks, with success proving the false alarm is
// gone and replacement proving the first learned identity becomes an anchor.
for (const boundary of ["following", "submitting"] as const) {
  for (const outcome of ["completion", "replacement", "replacement-before-acceptance"] as const) {
    if (boundary === "following" && outcome === "replacement-before-acceptance") continue;
    test(`a first identity learned while ${boundary}: ${outcome}`, async ({
      page,
      request,
    }, testInfo) => {
      const remote = destination(testInfo, `first-identity-${boundary}-${outcome}`);
      const added = await startAdd(request, remote);
      await waitForProgress(request, added.host_id, "completed");
      await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
      const feed = await stubFeed(page);
      feed.notifyOnConnect(1);
      const learnedIdentity = `learned-${testInfo.project.name}`;
      let phase: "unknown" | "learned" | "replaced" = "unknown";
      // Snapshot each request's phase. The visible name is a render barrier,
      // so a served response alone cannot masquerade as a changed UI binding.
      await page.route("**/api/hosts", async (route) => {
        if (route.request().method() !== "GET") return route.continue();
        const seen = phase;
        const response = await route.fetch();
        const body = await response.json();
        const entry = body.hosts.find((host: Host) => host.id === added.host_id);
        if (entry) {
          entry.identity = seen === "unknown" ? null : seen === "learned" ? learnedIdentity : "replacement-identity";
          entry.alias = `${seen}-identity-marker`;
          entry.name = entry.alias;
          if (seen === "learned") {
            entry.state = {
              phase: "connected",
              identity: learnedIdentity,
              build_version: HELM_BUILD,
              refresh: { status: "ok", sessions: 0 },
            };
          } else if (seen === "replaced") {
            // Keep the replacement disconnected so the warning describes
            // an unsettled outcome rather than a healthy current version.
            entry.state = {
              phase: "unreachable-reprobing",
              cause: "transport-failure",
              last_error: "replacement connection not established",
            };
          }
        }
        await route.fulfill({ response, json: body });
      });
      const acceptance = routeGate();
      const admission = routeGate();
      let accepted: Accepted | undefined;
      if (boundary === "following") acceptance.release();
      await page.route(`**/api/hosts/${added.host_id}/update`, async (route) => {
        if (!route.request().postData()) return route.continue();
        const response = await route.fetch();
        expect(response.status(), await responseBody(response)).toBe(202);
        accepted = (await response.json()) as Accepted;
        admission.release();
        await acceptance.wait();
        await route.fulfill({ response });
      });
      const updates = countUpdateRequests(page, added.host_id);
      // Ignore older in-flight registry reads; readiness names the phase
      // actually served, not merely the next response to arrive.
      const registryRead = (expectedPhase: string) => page.waitForResponse(async (response) => {
        if (new URL(response.url()).pathname !== "/api/hosts" || response.request().method() !== "GET") return false;
        const body = await response.json();
        return body.hosts.some((host: Host & { name: string }) =>
          host.id === added.host_id && host.name === `${expectedPhase}-identity-marker`
        );
      });
      try {
        const initialRead = registryRead("unknown");
        await page.goto("/");
        await showHostsPanel(page);
        const initialResponse = await initialRead;
        expect(initialResponse.ok()).toBe(true);
        const initialHost = (await initialResponse.json()).hosts.find((host: Host) => host.id === added.host_id);
        expect(initialHost?.identity, "the browser must start with no recorded identity").toBeNull();
        const row = page.locator(`[data-host-id="${added.host_id}"]`);
        await expect(row.locator(".host-name")).toHaveText("unknown-identity-marker");
        await expect(page.locator(".host-details-toggle")).not.toBeChecked();
        await openHostMenu(row);
        await row.locator(".provisioning-update").dispatchEvent("click");
        await admission.wait();
        const running = await waitForProgress(request, added.host_id, "running");
        expect(accepted?.host_id).toBe(added.host_id);
        expect(running.run_id).toBe(accepted?.run_id);
        expect(running.run_id).not.toBeNull();
        if (boundary === "following") {
          await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
        } else {
          await expect(pendingUpdate(row, "submitting")).toBeVisible();
        }

        const learnedRead = registryRead("learned");
        phase = "learned";
        await notifyFeed(feed, 2);
        const learnedResponse = await learnedRead;
        expect(learnedResponse.ok()).toBe(true);
        const learnedHost = (await learnedResponse.json()).hosts.find((host: Host) => host.id === added.host_id);
        expect(learnedHost?.identity).toBe(learnedIdentity);
        expect(learnedHost?.state.phase).toBe("connected");
        expect(learnedHost?.state.build_version).toBe(HELM_BUILD);
        await expect(row.locator(".host-name")).toHaveText("learned-identity-marker");
        await expect(row).toHaveAttribute("data-host-phase", "connected");
        if (outcome === "replacement-before-acceptance") {
          // The intent is already invalidated by first contact. Its still-
          // outstanding POST must remember A rather than accepting B as
          // another first contact when the delayed 202 finally arrives.
          await expect(row.locator(".host-update-pending")).toHaveCount(0);
          phase = "replaced";
          await notifyFeed(feed, 3);
          await expect(row.locator(".host-name")).toHaveText("replaced-identity-marker");
          await expect(row).toHaveAttribute("data-host-phase", "unreachable-reprobing");
          await expect(row.locator(".provisioning-warning")).toHaveCount(0);
          acceptance.release();
          await expect(row.locator(".provisioning-warning")).toContainText(
            `this host's identity changed from ‹${learnedIdentity}› to ‹replacement-identity› while its update was being sent`,
          );
          await expect(row.locator(".host-update-running")).toHaveCount(0);
          await expect(row.locator(".host-detail")).toBeVisible();
          const stillRunning = await waitForProgress(request, added.host_id, "running");
          expect(stillRunning.run_id).toBe(running.run_id);
          expect(updates.plans).toBe(1);
          expect(updates.confirms).toBe(1);
          return;
        }
        // In the submitting case the full-binding watcher drops the intent,
        // but the POST still owns suppression until its checked reply lands.
        acceptance.release();
        await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
        await expect(row.locator(".host-detail")).toHaveCount(0);
        await expect(row.locator(".provisioning-warning")).toHaveCount(0);

        if (outcome === "completion") {
          await configureBackend();
          const completed = await waitForProgress(request, added.host_id, "completed");
          expect(completed.run_id).toBe(running.run_id);
          await notifyFeed(feed, 3);
          await expect(row.locator(".host-update-running")).toHaveCount(0);
          await expect(row.locator(".host-detail")).toHaveCount(0);
          await expect(row.locator(".provisioning-warning")).toHaveCount(0);
        } else {
          // This second transition distinguishes pinning the learned identity
          // from allowing every later identity against a stale unknown one.
          phase = "replaced";
          await notifyFeed(feed, 3);
          await expect(row.locator(".host-name")).toHaveText("replaced-identity-marker");
          await expect(row).toHaveAttribute("data-host-phase", "unreachable-reprobing");
          await expect(row.locator(".provisioning-warning")).toContainText(
            `this host's identity changed from ‹${learnedIdentity}› to ‹replacement-identity› while it was being updated`,
          );
          await expect(row.locator(".host-update-running")).toHaveCount(0);
          await expect(row.locator(".host-detail")).toBeVisible();
          const stillRunning = await waitForProgress(request, added.host_id, "running");
          expect(stillRunning.run_id).toBe(running.run_id);
        }
        expect(updates.plans).toBe(1);
        expect(updates.confirms).toBe(1);
      } finally {
        acceptance.release();
      }
    });
  }
}

// The warning is version advice after continuity breaks, not proof about the
// detached update. Cover both constructors and both facts through real row
// effects, including a 202 that arrives after current-version facts rendered.
for (const boundary of ["following", "submitting", "submitting-current"] as const) {
  for (const change of ["destination", "identity"] as const) {
    if (boundary === "submitting-current" && change === "identity") continue;
    test(`changed-host advice while ${boundary}: ${change}`, async ({ page, request }, testInfo) => {
      const remote = destination(testInfo, `advice-${boundary}-${change}`);
      const added = await startAdd(request, remote);
      await waitForProgress(request, added.host_id, "completed");
      await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
      const feed = await stubFeed(page);
      feed.notifyOnConnect(1);
      const oldIdentity = "recorded\u2066-identity";
      const newIdentity = "replacement\u202e-identity";
      const moved = `${remote}\u202e-other`;
      type Phase = "initial" | "changed" | "older" | "newer" | "current";
      let phase: Phase = "initial";
      await page.route("**/api/hosts", async (route) => {
        if (route.request().method() !== "GET") return route.continue();
        const seen = phase;
        const response = await route.fetch();
        const body = await response.json();
        const entry = body.hosts.find((host: Host) => host.id === added.host_id);
        if (entry) {
          entry.alias = `${seen}-${change}-advice-marker`;
          entry.name = entry.alias;
          entry.destination = seen !== "initial" && change === "destination" ? moved : remote;
          entry.identity = seen !== "initial" && change === "identity" ? newIdentity : oldIdentity;
          entry.state = seen === "initial" || seen === "changed"
            ? { phase: "unreachable-reprobing", cause: "transport-failure", last_error: "fixture disconnected" }
            : {
              phase: "connected",
              identity: entry.identity,
              build_version: seen === "current" ? HELM_BUILD : `${HELM_BUILD}-${seen}`,
              old_version: seen === "older",
              newer_version: seen === "newer",
              refresh: { status: "ok", sessions: 0 },
            };
        }
        await route.fulfill({ response, json: body });
      });
      const acceptance = routeGate();
      const admission = routeGate();
      let accepted: Accepted | undefined;
      if (boundary === "following") acceptance.release();
      await page.route(`**/api/hosts/${added.host_id}/update`, async (route) => {
        if (!route.request().postData()) return route.continue();
        const response = await route.fetch();
        expect(response.status(), await responseBody(response)).toBe(202);
        accepted = (await response.json()) as Accepted;
        admission.release();
        await acceptance.wait();
        await route.fulfill({ response });
      });
      const updates = countUpdateRequests(page, added.host_id);
      const row = page.locator(`[data-host-id="${added.host_id}"]`);
      /** A served generation and its visible marker jointly prove the row's new facts. */
      const publishPhase = async (next: Phase, revision: number) => {
        const read = page.waitForResponse(async (response) => {
          if (new URL(response.url()).pathname !== "/api/hosts" || response.request().method() !== "GET") return false;
          const body = await response.json();
          return body.hosts.some((host: Host & { name: string }) =>
            host.id === added.host_id && host.name === `${next}-${change}-advice-marker`
          );
        });
        phase = next;
        await notifyFeed(feed, revision);
        const response = await read;
        expect(response.ok()).toBe(true);
        const current = (await response.json()).hosts.find((host: Host) => host.id === added.host_id);
        expect(current?.identity).toBe(change === "identity" ? newIdentity : oldIdentity);
        expect(current?.destination).toBe(change === "destination" ? moved : remote);
        expect(current?.state.phase).toBe(next === "changed" ? "unreachable-reprobing" : "connected");
        if (next !== "changed") {
          expect(current?.state.build_version).toBe(next === "current" ? HELM_BUILD : `${HELM_BUILD}-${next}`);
        }
        await expect(row.locator(".host-name")).toHaveText(`${next}-${change}-advice-marker`);
        await expect(row).toHaveAttribute("data-host-phase", current.state.phase);
      };
      try {
        await page.goto("/");
        await showHostsPanel(page);
        await expect(row.locator(".host-name")).toHaveText(`initial-${change}-advice-marker`);
        await expect(row).toHaveAttribute("data-host-phase", "unreachable-reprobing");
        await expect(page.locator(".host-details-toggle")).not.toBeChecked();
        await openHostMenu(row);
        await row.locator(".provisioning-update").dispatchEvent("click");
        await admission.wait();
        const running = await waitForProgress(request, added.host_id, "running");
        expect(accepted?.host_id).toBe(added.host_id);
        expect(running.run_id).toBe(accepted?.run_id);
        if (boundary === "following") {
          await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
        } else {
          await expect(pendingUpdate(row, "submitting")).toBeVisible();
        }
        if (boundary === "submitting-current") {
          await publishPhase("current", 2);
          await expect(row.locator(".host-update-pending")).toHaveCount(0);
          await expect(row.locator(".provisioning-warning")).toHaveCount(0);
          const reply = page.waitForResponse((response) =>
            new URL(response.url()).pathname === `/api/hosts/${added.host_id}/update` &&
            response.request().postData() !== null
          );
          acceptance.release();
          expect((await reply).status()).toBe(202);
          // No host change follows the acceptance. Its own warning signal
          // must wake the clearing effect against the already-rendered facts.
          await expect(page.locator(".host-rail-add")).toBeEnabled();
          await expect(row.locator(".host-detail")).toHaveCount(0);
          await expect(row.locator(".provisioning-warning")).toHaveCount(0);
        } else {
          await publishPhase("changed", 2);
          if (boundary === "submitting") acceptance.release();
          const warning = row.locator(".provisioning-warning");
          const changedValues = change === "destination"
            ? `ssh destination changed from ‹${remote}› to ‹${remote}<U+202E>-other›`
            : "identity changed from ‹recorded<U+2066>-identity› to ‹replacement<U+202E>-identity›";
          await expect(warning).toContainText(changedValues);
          await expect(warning).toContainText(boundary === "following"
            ? "can't tell how it ended or which install it reached"
            : "can't tell whether the update started");
          await expect(warning).toContainText("check the farhelm version this host shows now");
          await expect(row.locator(".host-detail")).toBeVisible();
          // Both unequal builds stay connected. A connection or an age hint
          // alone must not erase the advice before exact agreement is known.
          await publishPhase("older", 3);
          await expect(warning).toContainText(changedValues);
          await publishPhase("newer", 4);
          await expect(warning).toContainText(changedValues);
          await publishPhase("current", 5);
          await expect(warning).toHaveCount(0);
          await expect(row.locator(".host-detail")).toHaveCount(0);
        }
        await expect(row.locator(".host-update-running")).toHaveCount(0);
        const stillRunning = await waitForProgress(request, added.host_id, "running");
        expect(stillRunning.run_id).toBe(running.run_id);
        expect(updates.plans).toBe(1);
        expect(updates.confirms).toBe(1);
      } finally {
        acceptance.release();
      }
    });
  }
}

// Run A ending unseen and observed run B are deliberately different evidence.
// Neither constructor may replace A's sticky warning with advice that a
// current connection would clear; B still has to detach on the retarget.
for (const boundary of ["following", "submitting"] as const) {
  test(`a changed host preserves earlier update uncertainty while ${boundary}`, async ({ page, request }, testInfo) => {
    const remote = destination(testInfo, `sticky-retarget-${boundary}`);
    const added = await startAdd(request, remote);
    await waitForProgress(request, added.host_id, "completed");
    await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    let moved = false;
    await page.route("**/api/hosts", async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      const seen = moved;
      const response = await route.fetch();
      const body = await response.json();
      const entry = body.hosts.find((host: Host) => host.id === added.host_id);
      if (entry && seen) {
        entry.destination = `${remote}-moved`;
        entry.name = entry.alias = "sticky-retarget-marker";
        entry.state = {
          phase: "connected", identity: entry.identity, build_version: HELM_BUILD,
          refresh: { status: "ok", sessions: 0 },
        };
      }
      await route.fulfill({ response, json: body });
    });
    // These are observed progress snapshots, not claims about the injected
    // executor. A→B proves A ended without a terminal result in this client.
    let observed: "A" | "B" | "B-failed" = "A";
    await page.route(`**/api/hosts/${added.host_id}/provisioning`, async (route) => {
      const seen = observed;
      await route.fulfill({
        headers: { "x-farhelm-build": HELM_BUILD },
        json: {
          host_id: added.host_id, run_id: seen === "A" ? "unresolved-A" : "observed-B",
          operation: "update", status: seen === "B-failed" ? "failed" : "running",
          steps: [], message: `${seen} progress marker`,
        },
      });
    });
    const admission = routeGate();
    const acceptance = routeGate();
    await page.route(`**/api/hosts/${added.host_id}/update`, async (route) => {
      if (!route.request().postData()) return route.continue();
      const response = await route.fetch();
      expect(response.status(), await responseBody(response)).toBe(202);
      admission.release();
      await acceptance.wait();
      await route.fulfill({ response });
    });
    const updates = countUpdateRequests(page, added.host_id);
    try {
      await page.goto("/");
      await showHostsPanel(page);
      const row = page.locator(`[data-host-id="${added.host_id}"]`);
      await expect(page.locator(".host-details-toggle")).not.toBeChecked();
      await expect(row.locator(".host-update-running")).toBeVisible();
      await expect(row.locator(".provisioning-warning")).toHaveCount(0);
      observed = "B";
      await notifyFeed(feed, 2);
      await expect(row.locator(".provisioning-run-message")).toHaveText("B progress marker");
      await expect(row.locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "running");
      await expect(row.locator(".host-update-running")).toBeVisible();
      const warning = row.locator(".provisioning-warning");
      await expect(warning).toContainText("update run unresolved-A ended without reporting its outcome");
      const original = await warning.textContent();
      if (boundary === "submitting") {
        observed = "B-failed";
        await notifyFeed(feed, 3);
        await expect(row.locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "failed");
        await expect(warning).toHaveText(original!);
        await openHostMenu(row);
        await row.locator(".provisioning-update").dispatchEvent("click");
        await admission.wait();
        await waitForProgress(request, added.host_id, "running");
        await expect(pendingUpdate(row, "submitting")).toBeVisible();
      }
      moved = true;
      await notifyFeed(feed, 4);
      await expect(row.locator(".host-name")).toHaveText("sticky-retarget-marker");
      await expect(row).toHaveAttribute("data-host-phase", "connected");
      await expect(row.locator(".host-update-running")).toHaveCount(0);
      await expect(row.locator(".host-update-pending")).toHaveCount(0);
      if (boundary === "submitting") {
        const reply = page.waitForResponse((response) =>
          new URL(response.url()).pathname === `/api/hosts/${added.host_id}/update` &&
          response.request().postData() !== null
        );
        acceptance.release();
        expect((await reply).status()).toBe(202);
        await expect(page.locator(".host-rail-add")).toBeEnabled();
      }
      await expect(warning).toHaveText(original!);
      await expect(row.locator(".host-detail")).toBeVisible();
      expect(updates.plans).toBe(boundary === "submitting" ? 1 : 0);
      expect(updates.confirms).toBe(boundary === "submitting" ? 1 : 0);
    } finally {
      acceptance.release();
    }
  });
}

test("incarnation churn keeps a followed run while a target change detaches it", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "continuity");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // Rewrite only this row's registry entry, snapshotting the test's phase
  // at dispatch so an in-flight request keeps its own generation's facts.
  // The alias marker is the render barrier. The helm reports an alias as
  // the host's `name`, so the rewrite sets both, and the marker then shows
  // in the folded row's visible name only once the rewritten entry has
  // rendered.
  let phase: "live" | "reconnected" | "moved" = "live";
  const moved = `${remote}-moved`;
  await page.route("**/api/hosts", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const seen = phase;
    const response = await route.fetch();
    const body = await response.json();
    const entry = body.hosts.find((host: Host) => host.id === accepted.host_id);
    if (entry && seen !== "live") {
      entry.incarnation += 1;
      entry.alias = seen === "reconnected" ? "reconnected-marker" : "moved-marker";
      entry.name = entry.alias;
      if (seen === "moved") entry.destination = moved;
    }
    await route.fulfill({ response, json: body });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);

  // A reconnect mid-run (fresh incarnation, same target) is normal: the
  // run stays followed and the row stays folded.
  const registryRefresh = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/api/hosts" &&
      response.request().method() === "GET",
  );
  phase = "reconnected";
  await notifyFeed(feed, 2);
  const refreshed = await registryRefresh;
  expect(refreshed.ok(), "the feed-triggered host refresh must succeed").toBe(true);
  const refreshedHosts = (await refreshed.json()).hosts as Host[];
  expect(JSON.stringify(refreshedHosts.find((host) => host.id === accepted.host_id))).toContain(
    "reconnected-marker",
  );
  // The response only proves the route served the new incarnation; the
  // visible name proves the row rendered it, so the provisioning child has
  // the new binding before continued progress is checked.
  await expect(row.locator(".host-name")).toHaveText("reconnected-marker");
  await expect(row.locator(".host-update-running")).toContainText(RUNNING_PROGRESS);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toHaveCount(0);

  // A real target change detaches the run evidence with an explicit
  // unresolved diagnostic instead of following it onto the new target —
  // and the old target's retained run is never re-adopted.
  phase = "moved";
  await notifyFeed(feed, 3);
  await expect(row.locator(".host-destination-detail")).toContainText(moved);
  await expect(row.locator(".provisioning-warning")).toContainText(
    `this host's ssh destination changed from ‹${remote}› to ‹${moved}› while it was being updated`,
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await notifyFeed(feed, 4);
  await expect(row.locator(".provisioning-warning")).toContainText(
    `this host's ssh destination changed from ‹${remote}› to ‹${moved}› while it was being updated`,
  );

  // Recovery is a fresh explicit attempt. The old held run finishes first
  // (its retired completion adopts nothing and clears nothing); only then
  // does the menu offer a new update. Note the backend still dials the real
  // destination — the rewrite is browser-side only.
  await configureBackend();
  await waitForProgress(request, accepted.host_id, "completed");
  await notifyFeed(feed, 5);
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  await expect(row.locator(".provisioning-warning")).toContainText(
    `this host's ssh destination changed from ‹${remote}› to ‹${moved}› while it was being updated`,
  );
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  expect(updates.plans).toBe(2);
  expect(updates.confirms).toBe(2);
  // The new run's correlated success supersedes the sticky diagnostic and
  // collapses the row.
  await configureBackend();
  await waitForProgress(request, accepted.host_id, "completed");
  await notifyFeed(feed, 6);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(2);
  expect(updates.confirms).toBe(2);
});

test("a progress read racing submission commits nothing; the delayed acceptance reconciles against the later run", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "submit-race");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // Delay acceptance A in transport: the submission POST reaches the backend
  // and A is admitted, but its 202 waits for the test's release.
  let releaseAcceptance!: () => void;
  const acceptGate = new Promise<void>((resolve) => {
    releaseAcceptance = resolve;
  });
  let resolveAdmitted!: () => void;
  const submissionAdmitted = new Promise<void>((resolve) => {
    resolveAdmitted = resolve;
  });
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (!route.request().postData()) return route.continue();
    const response = await route.fetch();
    resolveAdmitted();
    await acceptGate;
    await route.fulfill({ response });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  // A is admitted and held running on the backend while its 202 waits.
  await submissionAdmitted;
  // A completes unseen and another client starts B, which stays running.
  await configureBackend();
  const completedA = await waitForProgress(request, accepted.host_id, "completed");
  expect(completedA.run_id).not.toBeNull();
  const runA = completedA.run_id!;
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const competingPlan = await request.post(`/api/hosts/${accepted.host_id}/update`);
  const competing = (await competingPlan.json()) as { probe_id: string };
  const competingRun = await request.post(`/api/hosts/${accepted.host_id}/update`, {
    data: { probe_id: competing.probe_id },
  });
  expect(competingRun.status()).toBe(202);
  await waitForProgress(request, accepted.host_id, "running");

  // A feed read completes inside the POST interval and sees running B. The
  // follow-up GET proves the first read was fully processed — the
  // single-flight reader dispatches it only after finishing — and that
  // suppression kept retry demand alive rather than going idle.
  const provisioningPath = `/api/hosts/${accepted.host_id}/provisioning`;
  const firstRead = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === provisioningPath
      && response.request().method() === "GET",
  );
  await notifyFeed(feed, 2);
  await firstRead;
  await page.waitForRequest(
    (outgoing) =>
      new URL(outgoing.url()).pathname === provisioningPath && outgoing.method() === "GET",
  );
  // The folded row cannot show which view is committed, and the submitting
  // stage would read the same either way. Global details are opened only for
  // this observation: suppressed, the row still shows the old completed ADD
  // view, with no adopted run and no warning, while the submission is
  // outstanding. Closing them again restores the folded premise.
  await expect(pendingUpdate(row, "submitting")).toBeVisible();
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await page.locator(".host-details-toggle").click();
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  await expect(row.locator(".provisioning-warning")).toHaveCount(0);
  await page.locator(".host-details-toggle").click();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await expect(row.locator(".host-detail")).toHaveCount(0);

  // The delayed acceptance installs first; the fresh reread then observes
  // running B and follows it, keeping A's unseen end visible.
  releaseAcceptance();
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-warning")).toContainText(
    `update run ${runA} ended without reporting its outcome`,
  );
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);

  // B's own completion settles the followed run, but the sticky uncertainty
  // about A survives it: no unrelated completion resolves a lost outcome.
  await configureBackend();
  await waitForProgress(request, accepted.host_id, "completed");
  await notifyFeed(feed, 3);
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  await expect(row.locator(".provisioning-warning")).toContainText(
    `update run ${runA} ended without reporting its outcome`,
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

test("a held acceptance across a target change retires the old run instead of adopting it", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "accept-retarget");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // Browser-side retarget only: the backend keeps running the old target's
  // run, which is exactly what the host-scoped progress route reports after
  // a real retarget. Snapshot the phase at dispatch so an in-flight request
  // keeps its own generation's facts.
  let moved = false;
  const movedTo = `${remote}-moved`;
  await page.route("**/api/hosts", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const seen = moved;
    const response = await route.fetch();
    const body = await response.json();
    if (seen) {
      const entry = body.hosts.find((host: Host) => host.id === accepted.host_id);
      if (entry) entry.destination = movedTo;
    }
    await route.fulfill({ response, json: body });
  });
  // Hold the submission's 202 in transport: run A is admitted and held
  // running on the backend while the UI still waits for its acceptance.
  let releaseAcceptance!: () => void;
  const acceptGate = new Promise<void>((resolve) => {
    releaseAcceptance = resolve;
  });
  let resolveAdmitted!: () => void;
  const submissionAdmitted = new Promise<void>((resolve) => {
    resolveAdmitted = resolve;
  });
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (!route.request().postData()) return route.continue();
    const response = await route.fetch();
    resolveAdmitted();
    await acceptGate;
    await route.fulfill({ response });
  });
  // Hold this row's progress reads from admission until the acceptance
  // installs: the retarget watcher's own reread must still be in flight when
  // the late 202 lands (the install fence then rejects it), so the first
  // read to commit is the acceptance's own follow-up — the branch under
  // test. Afterwards, optionally serve a different running run instead.
  let holdProgress = false;
  let releaseProgress!: () => void;
  const progressGate = new Promise<void>((resolve) => {
    releaseProgress = resolve;
  });
  let serveNewRun = false;
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    if (holdProgress) await progressGate;
    if (!serveNewRun) return route.continue();
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: accepted.host_id,
        run_id: "update-run-on-new-target",
        operation: "update",
        status: "running",
        steps: [],
        message: "run-on-new-target marker",
      }),
    });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  const provisioningPath = `/api/hosts/${accepted.host_id}/provisioning`;
  const isProvisioningGet = (url: string, method: string) =>
    new URL(url).pathname === provisioningPath && method === "GET";
  const mountRead = page.waitForResponse((response) =>
    isProvisioningGet(response.url(), response.request().method())
  );
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  // The mount read's response arrived before the click — its commit follows
  // in the same reader task with no await in between — so the progress
  // reader is idle and the retarget's reread is genuinely its own.
  await mountRead;
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await submissionAdmitted;
  holdProgress = true;
  const runningA = await waitForProgress(request, accepted.host_id, "running");
  expect(runningA.run_id).not.toBeNull();
  await expect(pendingUpdate(row, "submitting")).toBeVisible();

  // Retarget while the 202 is held: the submitting intent belongs to the old
  // row, so the watcher drops it with nothing tracked and no diagnostic —
  // the row goes quiet (no inline status, nothing opened), which is the
  // premise this branch needs.
  moved = true;
  await notifyFeed(feed, 2);
  await expect(row.locator(".host-update-pending")).toHaveCount(0);
  await expect(row.locator(".host-update-running")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toHaveCount(0);

  // The late acceptance lands on the mismatch branch: unknown outcome, this
  // row claims nothing — and the old run id retires before the follow-up
  // reread that would otherwise adopt it as new-target work.
  releaseAcceptance();
  await expect(row.locator(".provisioning-warning")).toContainText(
    `ssh destination changed from ‹${remote}› to ‹${movedTo}› while its update was being sent`,
  );

  // The held pre-acceptance read completes into the install fence and commits
  // nothing; the acceptance's own reread then returns the old run, and a
  // feed-driven repeat returns it again. Each response is awaited; each
  // commit is barriered on what it renders.
  const heldRead = page.waitForResponse((response) =>
    isProvisioningGet(response.url(), response.request().method())
  );
  releaseProgress();
  await heldRead;
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-warning")).toContainText(
    `ssh destination changed from ‹${remote}› to ‹${movedTo}› while its update was being sent`,
  );
  const repeatRead = page.waitForResponse((response) =>
    isProvisioningGet(response.url(), response.request().method())
  );
  await notifyFeed(feed, 3);
  await repeatRead;
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-warning")).toContainText(
    `ssh destination changed from ‹${remote}› to ‹${movedTo}› while its update was being sent`,
  );
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);

  // A different run observed afterwards exposes whether the old one was
  // adopted: adoption rewrites the warning into a guessed resolution of the
  // old run's end, while retirement keeps the true submitting-uncertainty
  // beside the newly followed run.
  serveNewRun = true;
  await notifyFeed(feed, 4);
  await expect(row.locator(".provisioning-run-message")).toContainText("run-on-new-target marker");
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-warning")).toContainText(
    `ssh destination changed from ‹${remote}› to ‹${movedTo}› while its update was being sent`,
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

test("a progress read across a held acceptance and target change cannot adopt the old run", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "accept-retarget-progress-first");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // Browser-side retarget only: the backend keeps running the old target's
  // run, which is exactly what the host-scoped progress route reports after
  // a real retarget. Snapshot the phase at dispatch so an in-flight request
  // keeps its own generation's facts.
  let moved = false;
  const movedTo = `${remote}-moved`;
  await page.route("**/api/hosts", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const seen = moved;
    const response = await route.fetch();
    const body = await response.json();
    if (seen) {
      const entry = body.hosts.find((host: Host) => host.id === accepted.host_id);
      if (entry) entry.destination = movedTo;
    }
    await route.fulfill({ response, json: body });
  });
  // Hold the submission's 202 in transport: run A is admitted and held
  // running on the backend while the UI still waits for its acceptance.
  let releaseAcceptance!: () => void;
  const acceptGate = new Promise<void>((resolve) => {
    releaseAcceptance = resolve;
  });
  let resolveAdmitted!: () => void;
  const submissionAdmitted = new Promise<void>((resolve) => {
    resolveAdmitted = resolve;
  });
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (!route.request().postData()) return route.continue();
    const response = await route.fetch();
    resolveAdmitted();
    await acceptGate;
    await route.fulfill({ response });
  });
  // The opposite ordering from the acceptance-first case above: hold this
  // row's progress reads only until the retarget watcher has dropped the
  // intent, then release them BEFORE the acceptance installs. The watcher's
  // successful old-A progress response must commit nothing while the POST is
  // still outstanding. Afterwards, optionally serve a different running run
  // instead.
  let holdProgress = false;
  let releaseProgress!: () => void;
  const progressGate = new Promise<void>((resolve) => {
    releaseProgress = resolve;
  });
  let serveNewRun = false;
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    if (holdProgress) await progressGate;
    if (!serveNewRun) return route.continue();
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: accepted.host_id,
        run_id: "update-run-on-new-target",
        operation: "update",
        status: "running",
        steps: [],
        message: "run-on-new-target marker",
      }),
    });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  const provisioningPath = `/api/hosts/${accepted.host_id}/provisioning`;
  const isProvisioningGet = (url: string, method: string) =>
    new URL(url).pathname === provisioningPath && method === "GET";
  const mountRead = page.waitForResponse((response) =>
    isProvisioningGet(response.url(), response.request().method())
  );
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  // The mount read's response arrived before the click — its commit follows
  // in the same reader task with no await in between — so the progress
  // reader is idle and the retarget's reads are genuinely its own.
  await mountRead;
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await submissionAdmitted;
  holdProgress = true;
  const runningA = await waitForProgress(request, accepted.host_id, "running");
  expect(runningA.run_id).not.toBeNull();
  const runA = runningA.run_id!;
  await expect(pendingUpdate(row, "submitting")).toBeVisible();

  // Retarget while the 202 is held: the submitting intent belongs to the old
  // row, so the watcher drops it with nothing tracked and no diagnostic —
  // the row goes quiet (no inline status, nothing opened), which is the
  // premise this ordering needs.
  moved = true;
  await notifyFeed(feed, 2);
  await expect(row.locator(".host-update-pending")).toHaveCount(0);
  await expect(row.locator(".host-update-running")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toHaveCount(0);

  // The held reads complete before the acceptance installs. The first
  // response is either the fenced pre-retarget read (with the watcher's
  // reread coalesced behind it) or the watcher's own read (with the feed
  // notice coalesced behind it); either way the second response is a
  // post-fence read naming old running A. The request after it proves that
  // read was fully processed — the single-flight reader dispatches it only
  // after finishing — and that suppression kept retry demand alive rather
  // than going idle.
  const firstRead = page.waitForResponse((response) =>
    isProvisioningGet(response.url(), response.request().method())
  );
  releaseProgress();
  await firstRead;
  const secondRead = page.waitForResponse((response) =>
    isProvisioningGet(response.url(), response.request().method())
  );
  await secondRead;
  // Suppressed: the old run acquires no new-target ownership — no adopted
  // run, no disclosure, no warning — while the submission is outstanding,
  // and the menu still offers Update. Without suppression this read adopts
  // A with the new binding and owns the row, which would show as inline
  // progress for A (an adopted running update opens nothing) and a busy
  // menu; a committed but unadopted running view would draw the trace.
  await expect(row.locator(".host-update-pending")).toHaveCount(0);
  await expect(row.locator(".host-update-running")).toHaveCount(0);
  await expect(row.locator(".provisioning-trace")).toHaveCount(0);
  await expect(row.locator(".provisioning-run")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await openHostMenu(row);
  await expect(row.locator(".provisioning-update")).toBeVisible();
  await page.keyboard.press("Escape");
  await page.waitForRequest((outgoing) =>
    isProvisioningGet(outgoing.url(), outgoing.method())
  );
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);

  // The late acceptance lands on the mismatch branch: unknown outcome, this
  // row claims nothing — and the old run id retires (detaching nothing,
  // since the suppressed reads adopted nothing) before the follow-up reread.
  releaseAcceptance();
  await expect(row.locator(".provisioning-warning")).toContainText(
    `ssh destination changed from ‹${remote}› to ‹${movedTo}› while its update was being sent`,
  );

  // The acceptance's own reread returns the old run, and a feed-driven
  // repeat returns it again. Each response is awaited; each commit is
  // barriered on what it renders. Retirement holds: the view renders, but
  // nothing is adopted as new-target work.
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-warning")).toContainText(
    `ssh destination changed from ‹${remote}› to ‹${movedTo}› while its update was being sent`,
  );
  const repeatRead = page.waitForResponse((response) =>
    isProvisioningGet(response.url(), response.request().method())
  );
  await notifyFeed(feed, 3);
  await repeatRead;
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-warning")).toContainText(
    `ssh destination changed from ‹${remote}› to ‹${movedTo}› while its update was being sent`,
  );
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);

  // A different run observed afterwards exposes whether the old one was
  // adopted: adoption rewrites the warning into a guessed resolution of the
  // old run's end, while retirement keeps the true submitting-uncertainty
  // beside the newly followed run.
  serveNewRun = true;
  await notifyFeed(feed, 4);
  await expect(row.locator(".provisioning-run-message")).toContainText("run-on-new-target marker");
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-warning")).toContainText(
    `ssh destination changed from ‹${remote}› to ‹${movedTo}› while its update was being sent`,
  );
  await expect(row.locator(".provisioning-warning")).not.toContainText(
    `update run ${runA} ended without reporting its outcome`,
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

/**
 * A competing run that invalidates a retry must not erase the sticky warning.
 *
 * Why this matters: update A commits with an unreadable acceptance, leaving a
 * sticky "outcome unknown" warning that only a correlated success may clear.
 * When retry C is still planning and competing run B is observed, the intent
 * dies and B is followed — but A's uncertainty must survive that, B's own
 * completion, and a later failed retry. Replacing it with a clearable
 * "another run started" explanation would let the next click discard a run
 * that may have committed, with nothing left to resolve it.
 *
 * What it does: unreadable 202, a processed stale Completed view, a held
 * retry plan, then a marked Running view that invalidates the intent before
 * the held plan is released. Asserts no retry submission and the original
 * warning text through B's completion and one more planning failure.
 */
test("a competing run during retry planning preserves the sticky update warning", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update-sticky-competing");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // Attempt A's submission commits with an unreadable body; retry C's plan
  // waits for the test's release so the competing view lands mid-planning.
  let submissions = 0;
  let plans = 0;
  let releaseRetryPlan!: () => void;
  const retryPlanGate = new Promise<void>((resolve) => {
    releaseRetryPlan = resolve;
  });
  // The last attempt's plan is held too. Its injected inspection error
  // answers in a few milliseconds, so without a gate the planning indicator
  // can come and go before the assertion that it was shown gets to look.
  let releaseFailedPlan!: () => void;
  const failedPlanGate = new Promise<void>((resolve) => {
    releaseFailedPlan = resolve;
  });
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (route.request().postData()) {
      submissions += 1;
      if (submissions === 1) {
        await route.fulfill({
          status: 202,
          headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
          body: "{not-json",
        });
        return;
      }
      await route.continue();
      return;
    }
    plans += 1;
    if (plans === 2) {
      await retryPlanGate;
    }
    if (plans === 3) {
      await failedPlanGate;
    }
    await route.continue();
  });
  // Stale history first, then the marked competing run and its completion.
  // Each marker proves the view that carried it was fully processed.
  let phase: "stale" | "competing-running" | "competing-completed" = "stale";
  await page.route(`**/api/hosts/${accepted.host_id}/provisioning`, async (route) => {
    if (phase === "stale") {
      await route.fulfill({
        status: 200,
        headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
        body: JSON.stringify({
          host_id: accepted.host_id,
          run_id: "older-update-run",
          operation: "update",
          status: "completed",
          steps: [],
          message: "stale success marker",
        }),
      });
      return;
    }
    if (phase === "competing-running") {
      await route.fulfill({
        status: 200,
        headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
        body: JSON.stringify({
          host_id: accepted.host_id,
          run_id: "competing-update-run",
          operation: "update",
          status: "running",
          steps: [],
          message: "competing run marker",
        }),
      });
      return;
    }
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({
        host_id: accepted.host_id,
        run_id: "competing-update-run",
        operation: "update",
        status: "completed",
        steps: [],
        message: "competing completed marker",
      }),
    });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await showHostsPanel(page);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);

  // Attempt A installs the sticky uncertainty; the stale Completed view
  // renders beside it without clearing it.
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-warning")).toContainText("accepted the update");
  await expect(row.locator(".provisioning-run-message")).toContainText("stale success marker");
  await expect(row.locator(".provisioning-warning")).toContainText("accepted the update");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);

  // Retry C plans behind the held gate; the indicator proves the intent is
  // really in flight before the competing view arrives.
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-planning")).toBeVisible();

  // Competing B lands mid-planning: the intent dies, B is followed, but the
  // original warning — not the clearable invalidation explanation — stands.
  // Every assertion here commits before the held plan is released.
  phase = "competing-running";
  await notifyFeed(feed, 2);
  await expect(row.locator(".provisioning-run-message")).toContainText("competing run marker");
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-planning")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toContainText("accepted the update");
  await expect(row.locator(".provisioning-warning")).not.toContainText(
    "another provisioning run started",
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();

  // The stale plan reply arrives into a dead intent and submits nothing.
  const stalePlanReply = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/hosts/${accepted.host_id}/update`
    && response.request().method() === "POST"
    && !response.request().postData()
  );
  releaseRetryPlan();
  await stalePlanReply;
  await expect(row.locator(".provisioning-planning")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toContainText("accepted the update");
  await expect(row.locator(".provisioning-warning")).not.toContainText(
    "another provisioning run started",
  );
  expect(updates.plans).toBe(2);
  expect(updates.confirms).toBe(1);

  // B's own completion settles the followed run, not A's uncertainty.
  phase = "competing-completed";
  await notifyFeed(feed, 3);
  await expect(row.locator(".provisioning-run-message")).toContainText(
    "competing completed marker",
  );
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  await expect(row.locator(".provisioning-warning")).toContainText("accepted the update");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(2);
  expect(updates.confirms).toBe(1);

  // One more unsuccessful attempt: its planning failure provably submitted
  // nothing, so the sticky warning outlives it as well.
  await configureBackend({
    targets: {
      [target(remote)]: { inspect: "error", message: "retry inspection refused" },
    },
  });
  const failedPlanReply = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/hosts/${accepted.host_id}/update`
    && response.request().method() === "POST"
    && !response.request().postData()
  );
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-planning")).toBeVisible();
  releaseFailedPlan();
  await failedPlanReply;
  await expect(row.locator(".provisioning-planning")).toHaveCount(0);
  await expect(page.locator(".host-refusal-dialog")).toContainText("retry inspection refused");
  await expect(row.locator(".provisioning-warning")).toContainText("accepted the update");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(3);
  expect(updates.confirms).toBe(1);
});
