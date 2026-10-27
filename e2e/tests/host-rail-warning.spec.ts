/**
 * Closed Hosts must not hide a setup/Update outcome. These cases own real
 * registrations and runs through the existing injected executor; browser
 * routes alter only acceptance transport and retained read snapshots.
 */
import { expect, test } from "./helpers/evidence";
import { APIRequestContext, Page, TestInfo } from "@playwright/test";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { currentStackSlot } from "../stack-slots";
import { patchPreferences, readPreferences, showHostsPanel, showSessionsPanel, stubFeed, type FeedStub } from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { routeGate } from "./helpers/route-gate";

type Operation = "add" | "update";
type Accepted = { host_id: number; run_id: string };
type Progress = {
  run_id: string | null;
  operation: Operation | null;
  status: "running" | "completed" | "failed";
  steps: { step: string; status: string; message: string | null }[];
  message: string | null;
};
type Host = { id: number; destination: string | null };
type Behavior = { hold_actions?: boolean; fail_action?: string; message?: string; probe?: "supervisor"; identity?: string };
let baseline: Set<number>;
let helmBuild = "";
// One field is owned by these tests. Preserve the actual inherited value,
// including unset, rather than leaving this spec's null fixture behind.
let originalSetupConfirmation: boolean | null | undefined;

/** Atomically update the existing owned executor fixture, never process.env. */
async function configure(remote?: string, behavior: Behavior = {}): Promise<void> {
  const stack = JSON.parse(await readFile(currentStackSlot().stackInfoPath, "utf8"));
  const root = stack.provisioning_backend as string;
  const next = path.join(root, `warning.${process.pid}.next`);
  await writeFile(next, `${JSON.stringify({ targets: remote ? { [`ssh:${remote}`]: behavior } : {} })}\n`, { mode: 0o600 });
  await rename(next, path.join(root, "config.json"));
}

/** Registration identity, rather than alias or list position, owns cleanup. */
async function hosts(request: APIRequestContext): Promise<Host[]> {
  const response = await request.get("/api/hosts");
  expect(response.ok(), await response.text()).toBe(true);
  return (await response.json()).hosts;
}

/** Read the real retained run even when a browser route presents old history. */
async function progress(request: APIRequestContext, host: number): Promise<Progress> {
  const response = await request.get(`/api/hosts/${host}/provisioning`);
  expect(response.ok(), await response.text()).toBe(true);
  return await response.json();
}

/** A held action is a fixture premise only once the helm reports it Running. */
async function settled(request: APIRequestContext, host: number, status: Progress["status"]): Promise<Progress> {
  await expect.poll(async () => await progress(request, host)).toMatchObject({ status });
  return await progress(request, host);
}

/** Create a real host without relying on a dialog unrelated to the assertion. */
async function startAdd(request: APIRequestContext, remote: string): Promise<Accepted> {
  const probe = await request.post("/api/hosts/probe", { data: { target: { kind: "ssh", destination: remote } } });
  expect(probe.ok(), await probe.text()).toBe(true);
  const plan = await probe.json();
  expect(plan.result).toBe("provisionable");
  const accepted = await request.post("/api/hosts/provision", { data: { probe_id: plan.probe_id } });
  expect(accepted.status(), await accepted.text()).toBe(202);
  return await accepted.json();
}

/** Unique destinations keep both engines and all serial cases independent. */
function destination(info: TestInfo, suffix: string): string {
  return `e2e@${info.project.name}-rail-warning-${suffix}.invalid`;
}

/** Rail assertions follow immutable host identity through alias changes. */
function avatar(page: Page, host: number) {
  return page.locator(`.host-rail-avatar[data-rail-host-id="${host}"]`);
}

/** The row owner remains mounted while its ordinary presentation is hidden. */
function row(page: Page, host: number) {
  return page.locator(`[data-host-id="${host}"]`);
}

/**
 * Completed runs are normally folded out of the DOM. Enable their existing
 * details view before creating attention, so its rendered read markers remain
 * available without opening Hosts later and acknowledging the warning.
 */
async function retainRunDetails(page: Page): Promise<void> {
  await showHostsPanel(page);
  await page.locator(".host-details-toggle").check();
  await showSessionsPanel(page);
}

/** Enter the actual rail menu without opening Hosts or changing selection. */
async function action(page: Page, host: number, operation: Operation, retry = false): Promise<void> {
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
  await avatar(page, host).click({ button: "right" });
  const menu = row(page, host).locator(".host-row-menu-panel");
  await expect(menu).toBeVisible();
  await menu.locator(retry ? ".provisioning-rerun" : ".provisioning-update").click();
  if (operation === "add") {
    const question = page.getByRole("dialog", { name: "host setup", exact: true });
    await expect(question).toBeVisible();
    await question.locator(".provisioning-confirm").click();
    await expect(question).toHaveCount(0);
  }
}

/** Feed revision is the existing progress-read trigger, not an extra loop. */
async function notify(feed: FeedStub, revision: number): Promise<void> {
  await expect.poll(() => feed.openSockets()).toBeGreaterThan(0);
  feed.notify(revision);
}

test.beforeAll(async ({ request }) => {
  const response = await request.get("/api/hosts");
  expect(response.ok(), await response.text()).toBe(true);
  helmBuild = requireHelmBuild(response, "host rail warning fixtures");
  baseline = new Set(((await response.json()).hosts as Host[]).map((host) => host.id));
});

test.beforeEach(async ({ request }) => {
  originalSetupConfirmation = undefined;
  originalSetupConfirmation = (await readPreferences(request)).skip_host_setup_confirmation ?? null;
  await configure();
  await patchPreferences(request, { skip_host_setup_confirmation: null });
});

/**
 * Release every held action before deleting this spec's registrations. Busy
 * refusal is the deletion oracle: terminal progress can precede lock release.
 * Keep failed fixture diagnostics inside its lifetime, before removing it.
 */
test.afterEach(async ({ page, request }, info) => {
  await cleanupOwnedSteps(info.status !== info.expectedStatus, [
    ["remove browser seams", () => page.unrouteAll({ behavior: "wait" })],
    ["release executor", () => configure()],
    ["restore setup preference", async () => {
      if (originalSetupConfirmation !== undefined) {
        await patchPreferences(request, { skip_host_setup_confirmation: originalSetupConfirmation });
      }
    }],
    ["remove owned hosts", async () => {
      for (const host of (await hosts(request)).filter((host) => !baseline.has(host.id))) {
        await expect.poll(async () => {
          const response = await request.delete(`/api/hosts/${host.id}`);
          const body = await response.text();
          if (response.ok()) return "removed";
          if (response.status() === 409 && body.includes("busy with a setup, update or uninstall")) {
            return `busy: ${JSON.stringify(await progress(request, host.id))}`;
          }
          throw new Error(`owned host ${host.id} removal refused: ${response.status()}: ${body}`);
        }, { timeout: 30_000, message: "owned run must release its host before cleanup" }).toBe("removed");
      }
      expect(new Set((await hosts(request)).map((host) => host.id))).toEqual(baseline);
    }],
  ]);
});

/**
 * The same visible error text must represent two different failures when a
 * fresh run fails. Acknowledgement remembers the first run; the final retry
 * keeps its warning through planning/acceptance/Running and clears on success.
 */
for (const operation of ["add", "update"] as const) {
  test(`${operation} warning acknowledges once, relatches a new failure and clears on correlated retry`, async ({ page, request }, info) => {
    const remote = destination(info, operation);
    if (operation === "add") await configure(remote, { hold_actions: true });
    const initial = await startAdd(request, remote);
    if (operation === "update") {
      await settled(request, initial.host_id, "completed");
      await configure(remote, { hold_actions: true });
    }
    const feed = await stubFeed(page);
    await page.goto("/");
    await expect(avatar(page, initial.host_id)).toBeVisible();
    if (operation === "update") await action(page, initial.host_id, operation);
    const running = await settled(request, initial.host_id, "running");
    expect(running.operation).toBe(operation);
    await notify(feed, 1);
    await expect(avatar(page, initial.host_id)).toHaveAttribute("data-host-progress", "true");
    await expect(avatar(page, initial.host_id)).toHaveAttribute("data-host-attention", "false");
    const fail = { fail_action: operation === "add" ? "attach-supervisor" : "restart-supervisor", message: "same failure" };
    await configure(remote, fail);
    const first = await settled(request, initial.host_id, "failed");
    expect(first.message).toContain("same failure");
    await notify(feed, 2);
    await expect(avatar(page, initial.host_id)).toHaveAttribute("data-host-attention", "true");
    await expect(avatar(page, initial.host_id)).toHaveAttribute("aria-label", /needs attention; open Hosts for details/);
    await expect(avatar(page, initial.host_id).locator(".host-rail-attention")).toBeVisible();
    await expect(page.locator(".session-heading")).toBeVisible();
    await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");

    await showHostsPanel(page);
    await expect(avatar(page, initial.host_id)).toHaveAttribute("data-host-attention", "false");
    await showSessionsPanel(page);
    // Change only the diagnostic to prove the browser consumed another read
    // of this retained run before checking that acknowledgement survives it.
    await page.route(`**/api/hosts/${initial.host_id}/provisioning`, async (route) => {
      await route.fulfill({ headers: { "x-farhelm-build": helmBuild }, json: { ...first, message: "retained read after acknowledgement" } });
    });
    await notify(feed, 3);
    await expect(row(page, initial.host_id).locator(".provisioning-run-message")).toHaveText("retained read after acknowledgement");
    await expect(avatar(page, initial.host_id)).toHaveAttribute("data-host-attention", "false");
    await page.unroute(`**/api/hosts/${initial.host_id}/provisioning`);

    await configure(remote, { hold_actions: true });
    await action(page, initial.host_id, operation, true);
    const second = await settled(request, initial.host_id, "running");
    expect(second.run_id).not.toBe(first.run_id);
    await configure(remote, fail);
    const secondFailure = await settled(request, initial.host_id, "failed");
    expect(secondFailure.message, "a fresh failure deliberately repeats the earlier diagnostic").toBe(first.message);
    await notify(feed, 4);
    await expect(avatar(page, initial.host_id)).toHaveAttribute("data-host-attention", "true");

    await configure(remote, { hold_actions: true });
    await action(page, initial.host_id, operation, true);
    const retry = await settled(request, initial.host_id, "running");
    expect(retry.run_id).not.toBe(second.run_id);
    await notify(feed, 5);
    await expect(avatar(page, initial.host_id)).toHaveAttribute("data-host-progress", "true");
    await expect(avatar(page, initial.host_id)).toHaveAttribute("data-host-attention", "true");
    await configure();
    await settled(request, initial.host_id, "completed");
    await notify(feed, 6);
    await expect(avatar(page, initial.host_id)).toHaveAttribute("data-host-attention", "false");
    await expect(page.locator(".session-heading")).toBeVisible();
  });
}

/**
 * An unreadable initial acceptance still really registered a host. Its exact
 * inspected destination must receive the warning, while an unrelated host's
 * successful setup and later retained Completed history cannot acknowledge it.
 */
for (const transport of ["malformed", "lost"] as const) {
  test(`initial Add ${transport} acceptance warns only its registered avatar`, async ({ page, request }, info) => {
    const remote = destination(info, transport);
    const other = await startAdd(request, destination(info, `${transport}-other`));
    await settled(request, other.host_id, "completed");
    await configure(remote, { hold_actions: true });
    let accepted: Accepted | undefined;
    await page.route("**/api/hosts/provision", async (route) => {
      const response = await route.fetch();
      expect(response.status()).toBe(202);
      accepted = await response.json();
      if (transport === "lost") await route.abort("failed");
      else await route.fulfill({ response, body: "{malformed acceptance" });
    });
    const feed = await stubFeed(page);
    await page.goto("/");
    await expect(avatar(page, other.host_id)).toBeVisible();
    await retainRunDetails(page);
    await page.locator(".host-rail-add").click();
    await page.locator(".add-host-ssh").fill(remote);
    await page.locator(".add-host-submit").click();
    const question = page.locator(".host-add-dialog");
    await expect(question).toBeVisible();
    await question.locator(".provisioning-confirm").click();
    await expect(question).toHaveCount(0);
    await expect.poll(() => accepted).not.toBeUndefined();
    const own = accepted!;
    expect((await hosts(request)).find((host) => host.id === own.host_id)?.destination).toBe(remote);
    await notify(feed, 1);
    await expect(avatar(page, own.host_id)).toHaveAttribute("data-host-attention", "true");
    await expect(avatar(page, own.host_id)).toHaveAttribute("data-host-progress", "true");
    await expect(avatar(page, other.host_id)).toHaveAttribute("data-host-attention", "false");
    await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");

    const completed = await progress(request, other.host_id);
    await page.route(`**/api/hosts/${own.host_id}/provisioning`, async (route) => {
      await route.fulfill({ headers: { "x-farhelm-build": helmBuild }, json: { ...completed, message: "unrelated completed history" } });
    });
    await notify(feed, 2);
    await expect(row(page, own.host_id).locator(".provisioning-run-message")).toHaveText("unrelated completed history");
    await expect(avatar(page, own.host_id)).toHaveAttribute("data-host-attention", "true");
    await showHostsPanel(page);
    await expect(avatar(page, own.host_id)).toHaveAttribute("data-host-attention", "false");
    await showSessionsPanel(page);
    await page.unroute(`**/api/hosts/${own.host_id}/provisioning`);
    await configure();
  });
}

/** A setup retry can discover the supervisor and complete the same retained
 * failed Add. Requiring a fresh run id would strand this warning forever. */
test("setup discovery clears the same failed run without opening Hosts", async ({ page, request }, info) => {
  const remote = destination(info, "discovery");
  await configure(remote, { fail_action: "attach-supervisor", message: "attachment failed" });
  const added = await startAdd(request, remote);
  const failed = await settled(request, added.host_id, "failed");
  await page.goto("/");
  await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "true");
  await configure(remote, { probe: "supervisor", identity: `recovered-${info.project.name}` });
  await avatar(page, added.host_id).click({ button: "right" });
  await row(page, added.host_id).locator(".provisioning-rerun").click();
  const recovered = await settled(request, added.host_id, "completed");
  expect(recovered.run_id).toBe(failed.run_id);
  await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "false");
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
});

/**
 * Submission uncertainty must outlive stale same-action success, a retry that
 * cannot plan, and a retry whose submission is refused. Only the final real
 * retry's own completion is evidence that recovers the warning.
 */
test("uncertain Update survives old success and failed retries until correlated success", async ({ page, request }, info) => {
  const remote = destination(info, "uncertain-update");
  const added = await startAdd(request, remote);
  await settled(request, added.host_id, "completed");
  const planResponse = await request.post(`/api/hosts/${added.host_id}/update`);
  expect(planResponse.ok(), await planResponse.text()).toBe(true);
  const plan = await planResponse.json();
  const oldResponse = await request.post(`/api/hosts/${added.host_id}/update`, { data: { probe_id: plan.probe_id } });
  expect(oldResponse.status(), await oldResponse.text()).toBe(202);
  const old = await settled(request, added.host_id, "completed");
  expect(old.operation).toBe("update");
  await configure(remote, { hold_actions: true });
  let phase: "uncertain" | "plan-refused" | "submit-refused" | "recover" = "uncertain";
  await page.route(`**/api/hosts/${added.host_id}/update`, async (route) => {
    const submitting = Boolean(route.request().postDataJSON()?.probe_id);
    if (phase === "plan-refused" && !submitting || phase === "submit-refused" && submitting) {
      await route.fulfill({ status: 409, headers: { "x-farhelm-build": helmBuild }, body: "controlled retry refusal" });
    } else if (phase === "uncertain" && submitting) {
      const response = await route.fetch();
      expect(response.status()).toBe(202);
      await route.fulfill({ response, body: "{unreadable update acceptance" });
    } else await route.continue();
  });
  // Keep the submitted run genuinely unseen by this browser. If its Running
  // and matching Completed were observed, that would be a correlated recovery
  // rather than the unrelated history this test exists to reject.
  let staleMessage = "initial retained Update history";
  await page.route(`**/api/hosts/${added.host_id}/provisioning`, async (route) => {
    await route.fulfill({ headers: { "x-farhelm-build": helmBuild }, json: { ...old, message: staleMessage } });
  });
  const feed = await stubFeed(page);
  await page.goto("/");
  await expect(avatar(page, added.host_id)).toBeVisible();
  await action(page, added.host_id, "update");
  const uncertain = await settled(request, added.host_id, "running");
  expect(uncertain.run_id).not.toBe(old.run_id);
  await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "true");

  staleMessage = "old Update success after uncertainty";
  await notify(feed, 1);
  await expect(row(page, added.host_id).locator(".provisioning-run-message")).toHaveText(staleMessage);
  await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "true");
  await configure();
  await settled(request, added.host_id, "completed");

  phase = "plan-refused";
  await action(page, added.host_id, "update");
  const refusal = page.locator(".host-refusal-dialog");
  await expect(refusal).toContainText("controlled retry refusal");
  await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "true");
  await refusal.getByRole("button", { name: "close", exact: true }).click();
  await expect(refusal).toHaveCount(0);

  phase = "submit-refused";
  await action(page, added.host_id, "update");
  await expect(row(page, added.host_id).locator(".provisioning-error")).toContainText("controlled retry refusal");
  await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "true");
  phase = "recover";
  await page.unroute(`**/api/hosts/${added.host_id}/provisioning`);
  await configure(remote, { hold_actions: true });
  await action(page, added.host_id, "update");
  const recovery = await settled(request, added.host_id, "running");
  expect(recovery.run_id).not.toBe(uncertain.run_id);
  await notify(feed, 2);
  await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-progress", "true");
  await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "true");
  await configure();
  await settled(request, added.host_id, "completed");
  await notify(feed, 3);
  await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "false");
  await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
});

/** A host entry owns its warning. Removing it and later registering the same
 * destination must not hand that warning to the new host entry. */
test("removing a warned host does not warn a later registration of its destination", async ({ page, request }, info) => {
  const remote = destination(info, "remove");
  await configure(remote, { fail_action: "attach-supervisor", message: "owned setup failure" });
  const failed = await startAdd(request, remote);
  await settled(request, failed.host_id, "failed");
  await page.goto("/");
  await expect(avatar(page, failed.host_id)).toHaveAttribute("data-host-attention", "true");
  await expect.poll(async () => {
    const response = await request.delete(`/api/hosts/${failed.host_id}`);
    const body = await response.text();
    if (response.status() === 409 && body.includes("busy with a setup, update or uninstall")) return false;
    expect(response.ok(), body).toBe(true);
    return true;
  }).toBe(true);
  await expect(avatar(page, failed.host_id)).toHaveCount(0);
  await configure();
  const later = await startAdd(request, remote);
  expect(later.host_id).not.toBe(failed.host_id);
  await settled(request, later.host_id, "completed");
  await expect(avatar(page, later.host_id)).toBeVisible();
  await expect(avatar(page, later.host_id)).toHaveAttribute("data-host-attention", "false");
});

/**
 * A decoded 202 establishes a known run even when no Running snapshot ever
 * reaches the browser. Hold all post-click reads until acceptance has visibly
 * installed, then return idle while that exact real run remains held Running.
 */
test("an accepted Update whose first applicable read is idle warns without ever observing Running", async ({ page, request }, info) => {
  const remote = destination(info, "accepted-idle");
  const added = await startAdd(request, remote);
  await settled(request, added.host_id, "completed");
  await configure(remote, { hold_actions: true });
  await stubFeed(page);
  const reads = routeGate();
  let holdReads = false;
  await page.route(`**/api/hosts/${added.host_id}/provisioning`, async (route) => {
    if (!holdReads) return route.continue();
    await reads.wait();
    await route.fulfill({ headers: { "x-farhelm-build": helmBuild }, json: {
      run_id: null, operation: null, status: "completed", steps: [], message: null,
    } });
  });
  try {
    await page.goto("/");
    await retainRunDetails(page);
    await expect(row(page, added.host_id).locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "completed");
    const acceptance = page.waitForResponse((response) =>
      new URL(response.url()).pathname === `/api/hosts/${added.host_id}/update`
      && response.request().postData() !== null
    );
    holdReads = true;
    await action(page, added.host_id, "update");
    const response = await acceptance;
    expect(response.status(), await response.text()).toBe(202);
    const accepted = await response.json() as Accepted;
    const actual = await settled(request, added.host_id, "running");
    expect(actual.run_id).toBe(accepted.run_id);
    // This is the renderer's acceptance barrier, rather than mere network
    // arrival. The warning must still be absent before the idle read commits.
    await expect(row(page, added.host_id).locator('.host-update-pending[data-update-phase="awaiting-progress"]')).toHaveCount(1);
    await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "false");
    reads.release();
    await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "true");
    expect(await progress(request, added.host_id)).toMatchObject({ run_id: accepted.run_id, status: "running" });
    await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
  } finally {
    reads.release();
  }
});

/**
 * A changed destination belongs to the same logical host entry. Its detached
 * run becomes uncertain whether acceptance was already installed or arrives
 * late. Connected/current-version facts can settle the existing advice text,
 * but only opening Hosts (or correlated recovery) acknowledges its rail mark.
 */
for (const boundary of ["following", "submitting", "submitting-unvalidated"] as const) {
  test(`a held Update losing its target while ${boundary} warns the logical entry with Hosts closed`, async ({ page, request }, info) => {
    const remote = destination(info, `target-${boundary}`);
    const added = await startAdd(request, remote);
    await settled(request, added.host_id, "completed");
    await configure(remote, { hold_actions: true });
    const feed = await stubFeed(page);
    const acceptance = routeGate();
    const admission = routeGate();
    if (boundary === "following") acceptance.release();
    let accepted: Accepted | undefined;
    await page.route(`**/api/hosts/${added.host_id}/update`, async (route) => {
      if (!route.request().postData()) return route.continue();
      const response = await route.fetch();
      expect(response.status(), await response.text()).toBe(202);
      accepted = await response.json();
      admission.release();
      await acceptance.wait();
      if (boundary === "submitting-unvalidated") {
        await route.fulfill({ response, body: "{unreadable old-target acceptance" });
      } else await route.fulfill({ response });
    });
    let phase: "initial" | "changed" | "current" | "repeat" = "initial";
    const movedTo = `${remote}-moved`;
    await page.route("**/api/hosts", async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      const seen = phase;
      const response = await route.fetch();
      const body = await response.json();
      const entry = body.hosts.find((host: Host) => host.id === added.host_id);
      expect(entry, "the real owned entry must exist before its browser-only target changes").toBeDefined();
      entry.alias = entry.name = `warning-target-${seen}`;
      entry.destination = seen === "initial" ? remote : movedTo;
      if (seen === "current" || seen === "repeat") {
        entry.state = { phase: "connected", identity: entry.identity, build_version: helmBuild, refresh: { status: "ok", sessions: 0 } };
      } else if (seen === "changed") {
        entry.state = { phase: "unreachable-reprobing", cause: "transport-failure", last_error: "changed target not connected" };
      }
      await route.fulfill({ response, json: body });
    });
    try {
      await page.goto("/");
      await expect(row(page, added.host_id).locator(".host-name")).toHaveText("warning-target-initial");
      await action(page, added.host_id, "update");
      await admission.wait();
      const running = await settled(request, added.host_id, "running");
      expect(running.run_id).toBe(accepted?.run_id);
      expect(accepted?.host_id).toBe(added.host_id);
      const pending = row(page, added.host_id).locator('.host-update-pending[data-update-phase="submitting"]');
      if (boundary === "following") {
        await expect(row(page, added.host_id).locator(".host-update-running")).toHaveCount(1);
      } else {
        await expect(pending).toHaveCount(1);
      }
      await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "false");
      phase = "changed";
      await notify(feed, 1);
      await expect(row(page, added.host_id).locator(".host-name")).toHaveText("warning-target-changed");
      if (boundary !== "following") {
        // The watcher invalidated the intent, but no accepted run was owned
        // yet. This leg specifically needs the late acceptance boundary.
        await expect(pending).toHaveCount(0);
        await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "false");
        acceptance.release();
      }
      const warning = row(page, added.host_id).locator(".provisioning-warning");
      await expect(warning).toContainText(boundary === "submitting-unvalidated" ? "run identity could not be read" : "ssh destination changed");
      await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "true");
      if (boundary !== "submitting-unvalidated") {
        await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-progress", "false");
      }
      const stillRunning = await progress(request, added.host_id);
      expect(stillRunning).toMatchObject({ run_id: running.run_id, status: "running" });

      phase = "current";
      await notify(feed, 2);
      await expect(row(page, added.host_id).locator(".host-name")).toHaveText("warning-target-current");
      await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-phase", "connected");
      if (boundary === "submitting-unvalidated") {
        // An unreadable receipt has no run id to retire. Its existing sticky
        // diagnostic and progress adoption rules remain authoritative.
        await expect(warning).toContainText("run identity could not be read");
      } else await expect(warning).toHaveCount(0);
      await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "true");
      await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "false");
      await showHostsPanel(page);
      await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "false");
      await showSessionsPanel(page);
      phase = "repeat";
      await notify(feed, 3);
      await expect(row(page, added.host_id).locator(".host-name")).toHaveText("warning-target-repeat");
      await expect(avatar(page, added.host_id)).toHaveAttribute("data-host-attention", "false");
    } finally {
      acceptance.release();
    }
  });
}
