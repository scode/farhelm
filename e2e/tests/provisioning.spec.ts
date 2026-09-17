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
import { Page, APIRequestContext, TestInfo } from "@playwright/test";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import {
  openHostMenu,
  openHostsPanel,
  stubFeed,
  type FeedStub,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";

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
  | { result: "provisionable"; probe_id: string; confirmation: string };

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
  const raw = await readFile(path.join(__dirname, "..", ".stack-info.json"), "utf8");
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
    const response = await request.delete(`/api/hosts/${host.id}`);
    if (!response.ok()) failures.push(`${host.id}: ${await responseBody(response)}`);
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

/** Open the real add form and submit one destination.
 *
 * Ensuring host details first keeps any resulting plan or diagnostic visible
 * without making callers repeat the disclosure prerequisite.
 */
async function probeRemote(page: Page, destination: string): Promise<void> {
  await openHostsPanel(page);
  await page.getByRole("button", { name: "add host" }).click();
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

test.afterEach(async ({ request }) => {
  // Release an action held by a failed assertion so it cannot poison the
  // next serial test or the second browser project.
  await configureBackend();
  await removeHostsBeyondBaseline(request);
});

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
  await expect(plan).toContainText("<U+202E>");
  await expect(plan).toContainText("<U+200B>");
  await expect(page.getByRole("button", { name: "confirm setup" })).toBeVisible();
  expect((await hosts(request)).some((host) => host.destination === remote)).toBe(false);
  expect((await backendEvents()).map((event) => event.event)).toEqual(["probe", "inspect"]);
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
  await expect(page.getByRole("button", { name: "confirm setup" })).toHaveCount(0);
  expect((await backendEvents()).map((event) => event.event)).toEqual(["probe"]);
});

test("blank optional fields and same-task double submit produce one real probe", async ({
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
  await page.getByRole("button", { name: "add host" }).click();
  await page.locator(".add-host-ssh").fill(remote);
  await page.evaluate(() => {
    const form = document.querySelector(".add-host-form")!;
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  });
  await expect(page.locator(".add-host-form .provisioning-plan")).toBeVisible();

  expect(body?.remote_farhelm ?? null).toBeNull();
  expect(body?.remote_state_dir ?? null).toBeNull();
  expect((await backendEvents()).filter((event) => event.event === "probe")).toHaveLength(1);
});

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
  await page.getByRole("button", { name: "confirm setup" }).click();

  const registered = await hostFor(request, remote);
  await expect(page.locator(`[data-host-id="${registered.id}"]`)).toBeVisible();
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-run`)).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
  // `.host-edit` lives inside the row's own "⋯" menu now, and it is
  // `aria-disabled` there rather than natively `disabled` (see `HostRow`'s
  // own doc — the menu leaves busy items focusable, like the session row's
  // menu) — `toBeDisabled()` honours `aria-disabled="true"` the same way.
  const registeredRow = page.locator(`[data-host-id="${registered.id}"]`);
  await openHostMenu(registeredRow);
  await expect(registeredRow.locator(".host-edit")).toBeDisabled();
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-update`)).toHaveCount(
    0,
  );
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-rerun`)).toHaveCount(
    0,
  );
  await expect.poll(async () => (await backendEvents()).some((event) => event.event === "create-directories")).toBe(true);

  await configureBackend();
  await waitForProgress(request, registered.id, "completed");
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-run`)).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  await expect(page.locator(`[data-host-id="${registered.id}"] .provisioning-update`)).toBeVisible();
  expect(documents).toBe(1);
});

test("a refused ADD attempt consumes the displayed offer and refreshes committed state", async ({
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

  await page.getByRole("button", { name: "confirm setup" }).click();
  await expect(page.locator(".add-host-error")).toContainText("already been used");
  await expect(page.locator(".add-host-form .provisioning-plan")).toHaveCount(0);
  await expect(page.locator(".add-host-ssh")).toBeVisible();
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
  await page.getByRole("button", { name: "confirm setup" }).click();

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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  release();
  await expect(page.locator('[data-host-kind="local"] .provisioning-error')).toContainText(
    "the helm accepted the local probe, but its reply could not be read",
  );
  await expect(page.locator(".host-details-toggle")).toBeChecked();
});

/**
 * Automatic setup has no initiating click that can reveal its confirmation.
 * Releasing a held plan from the collapsed state must open Details through
 * the child-to-parent reveal bridge.
 */
test("an automatic local plan reveals its confirmation", async ({ page }) => {
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  release();
  await expect(page.locator('[data-host-kind="local"] .provisioning-plan')).toContainText(
    "install the local supervisor",
  );
  await expect(page.locator(".host-details-toggle")).toBeChecked();
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
// and prove no replay. Disclosure cases start with global details UNCHECKED
// (never `openHostsPanel()`) and assert it stays unchecked: only the chosen
// row expands, through automatic per-host disclosure, and only authoritative
// success collapses it.

/**
 * Hold the page operation lock through an ADD confirm, without touching details.
 *
 * Discovery stays outside the lock, so the held request is the confirm POST
 * itself: its arrival proves the claim is taken, and releasing it completes
 * the ADD and frees the token. Unlike retry/adopt/edit, confirming an add
 * writes no global disclosure, so checkbox assertions stay meaningful.
 */
async function holdLockWithAdd(
  page: Page,
  destination: string,
): Promise<{ release: () => void }> {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let resolveConfirmed!: () => void;
  const confirmed = new Promise<void>((resolve) => {
    resolveConfirmed = resolve;
  });
  // Predicate, not a glob: the held POST must match exactly this path.
  await page.route(
    (url) => url.pathname === "/api/hosts/provision",
    async (route) => {
      resolveConfirmed();
      await gate;
      await route.continue();
    },
  );
  const addHost = page.getByRole("button", { name: "add host" });
  await addHost.click();
  await page.locator(".add-host-ssh").fill(destination);
  await page.locator(".add-host-submit").click();
  await expect(page.locator(".add-host-form .provisioning-plan")).toBeVisible();
  await page.getByRole("button", { name: "confirm setup" }).click();
  await confirmed;
  // The held confirm owns the page token: unrelated adds disable until
  // the release below, which is the UI-side proof the lock is really held.
  await expect(addHost).toBeDisabled();
  return { release };
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

test("single Update plans and submits once with no confirmation, expanding only its row", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update");
  const bystander = destination(testInfo, "update-bystander");
  const accepted = await startAdd(request, remote);
  const other = await startAdd(request, bystander);
  await waitForProgress(request, accepted.host_id, "completed");
  await waitForProgress(request, other.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
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
  // request, no confirmation rendered, and the row expands on its own.
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-confirm")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);

  // Only the chosen host expands; the bystander keeps no detail content.
  await expect(idle.locator(".host-detail")).toHaveCount(0);
  await expect(idle.locator(".provisioning-run")).toHaveCount(0);

  await openHostMenu(row);
  await expect(row.locator(".provisioning-update")).toHaveCount(0);
  await expect(row.locator(".provisioning-rerun")).toHaveCount(0);
  await expect(row.locator(".provisioning-auto-setup")).toHaveCount(0);
  await page.keyboard.press("Escape");

  // Acceptance releases the page lock while the run continues: unrelated
  // controls stay usable, while this row's own edit waits for its run.
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
  // `.host-edit` lives inside the row's own "⋯" menu now — see the
  // earlier ADD case's comment for why `toBeDisabled()` still applies.
  await openHostMenu(row);
  await expect(row.locator(".host-edit")).toBeDisabled();
  await page.keyboard.press("Escape");

  // Authoritative success collapses the automatic disclosure; the checkbox
  // the user never touched stays unchecked, and nothing replays.
  await configureBackend();
  await waitForProgress(request, accepted.host_id, "completed");
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".provisioning-trace")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

/**
 * Running, failed, and cleared traces each alter the collapsed host-list
 * shape. A fixed row menu must close at every boundary, including
 * running-to-failed where the traced host set itself is unchanged.
 *
 * The retained run is an ADD: observed UPDATE runs auto-expand their row
 * instead of tracing, so only a setup run keeps this geometry coverage
 * about the collapsed shape.
 */
test("collapsed trace transitions invalidate fixed-surface geometry", async ({
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
      await expect(row.locator(".provisioning-trace")).toHaveCount(0);
    } else {
      await expect(row.locator(".provisioning-trace")).toHaveAttribute(
        "data-provisioning-status",
        status,
      );
    }
  }
});

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
  let holdPlanning = true;
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (holdPlanning && !route.request().postData()) {
      await planningGate;
    }
    await route.continue();
  });
  await page.goto("/");
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);

  // Retarget before plan completion: the held planning reply arrives for a
  // row that no longer matches, so the intent dies and nothing submits.
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-planning")).toBeVisible();
  const changed = `${remote}-moved`;
  const response = await request.post(`/api/hosts/${accepted.host_id}/destination`, {
    data: { ssh: changed },
  });
  expect(response.ok(), await responseBody(response)).toBe(true);
  await notifyFeed(feed, 2);
  // The refresh delivering the new binding invalidates the intent while
  // planning is still held: the indicator going away proves the watcher
  // ran, before the stale reply is even released.
  await expect(row.locator(".provisioning-planning")).toHaveCount(0);
  holdPlanning = false;
  releasePlanning();
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
  await expect(row.locator(".provisioning-planning")).toBeVisible();
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
  holdPlanning = false;
  releasePlanning();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(2);
  expect(updates.confirms).toBe(0);
});

test("UPDATE planning refusal shows the concrete reason and stays expanded", async ({
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  const refusal = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/hosts/${accepted.host_id}/update`
    && response.request().method() === "POST",
  );
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await refusal;
  await expect(row.locator(".provisioning-error")).toContainText(
    'injected provisioning inspection failure: host stderr "inspection refused \\u{202e}host\\u{200b}"',
    { timeout: 10_000 },
  );
  // Planning failure submits nothing and mints no confirmation, but the
  // diagnostic keeps its own row expanded without touching the checkbox.
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-confirm")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toBeVisible();
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const lock = await holdLockWithAdd(page, lockHost);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-waiting")).toBeVisible();
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
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${from.host_id}"]`);
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
  await expect(row.locator(".host-edit")).toBeEnabled();
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");

  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toContainText("accepted");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
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
  await expect(row.locator(".provisioning-error")).toContainText("its reply was lost");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(2);
  expect(updates.confirms).toBe(1);

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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );

  // The failure keeps the tracked run's expansion and adds its own
  // diagnostic; the checkbox stays out of it.
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
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
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
  // `.host-edit` lives inside each row's own "⋯" menu now, and only one
  // row menu is ever open at a time (see `HostsPanel`'s own "one row menu
  // open" doc) — opening the SECOND row's closes the first's, so the two
  // rows are checked one at a time rather than simultaneously. That is a
  // change to how this test OBSERVES the two rows, not to what it proves:
  // each row's own busy state still reconciles independently of the
  // other's, which is what every assertion below still pins.
  await openHostMenu(rowOne);
  await expect(rowOne.locator(".host-edit")).toBeDisabled();
  await openHostMenu(rowTwo);
  await expect(rowTwo.locator(".host-edit")).toBeDisabled();
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();

  await configureBackend({ targets: { [target(second)]: { hold_actions: true } } });
  await waitForProgress(request, one.host_id, "completed");
  await openHostMenu(rowOne);
  await expect(rowOne.locator(".host-edit")).toBeEnabled();
  await openHostMenu(rowTwo);
  await expect(rowTwo.locator(".host-edit")).toBeDisabled();
});

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
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await expect(row.locator(".provisioning-trace")).toHaveText("setup provisioning failed");
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
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  // Automatic, like a fresh update: no plan rendered, no confirmation.
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-confirm")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const running = await waitForProgress(request, added.host_id, "running");
  expect(running.operation).toBe("update");
  expect(updateRequests).toEqual(["plan", "confirm"]);
  expect(addProbeRequests).toBe(0);
});

test("two overlapping remote updates track and settle independently", async ({
  page,
  request,
}, testInfo) => {
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
  await page.goto("/");
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const rowOne = page.locator(`[data-host-id="${one.host_id}"]`);
  const rowTwo = page.locator(`[data-host-id="${two.host_id}"]`);
  await openHostMenu(rowOne);
  await rowOne.locator(".provisioning-update").dispatchEvent("click");
  await openHostMenu(rowTwo);
  await rowTwo.locator(".provisioning-update").dispatchEvent("click");
  await expect(rowOne.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(rowTwo.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();

  await configureBackend({ targets: { [target(second)]: { hold_actions: true } } });
  await waitForProgress(request, one.host_id, "completed");
  // The finished row collapses on its own authoritative success while the
  // still-running row stays expanded: per-host disclosure, not a page mode.
  await expect(rowOne.locator(".host-detail")).toHaveCount(0);
  await expect(rowTwo.locator(".host-detail")).toBeVisible();
  await expect(rowTwo.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updatesOne.plans).toBe(1);
  expect(updatesOne.confirms).toBe(1);
  expect(updatesTwo.plans).toBe(1);
  expect(updatesTwo.confirms).toBe(1);
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const lock = await holdLockWithAdd(page, lockHost);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  // Planning ignores the held lock; the minted plan then waits for the
  // claim visibly, sending nothing until the token frees.
  await expect(row.locator(".provisioning-waiting")).toBeVisible();
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
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".provisioning-waiting")).toHaveCount(0);
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const hold = await holdLockWithAdd(page, lockHost);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);

  // Retarget while waiting: the stored plan belongs to the old row, so the
  // intent dies and the released lock submits nothing.
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-waiting")).toBeVisible();
  const changed = `${remote}-moved`;
  const response = await request.post(`/api/hosts/${accepted.host_id}/destination`, {
    data: { ssh: changed },
  });
  expect(response.ok(), await responseBody(response)).toBe(true);
  await notifyFeed(feed, 2);
  await expect(row.locator(".provisioning-waiting")).toHaveCount(0);
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
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(0);

  // Removal while waiting: the row (and its intent) is gone before the
  // lock frees, so again nothing submits.
  const holdAgain = await holdLockWithAdd(page, `${lockHost}-again`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-waiting")).toBeVisible();
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
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
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
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-planning")).toBeVisible();
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  // A is admitted and held running on the backend while its 202 waits. The
  // submitting indicator plus the disabled add-host button prove the
  // submission is in flight and the page lock is held — the premise the
  // acceptance release below needs. Request counts here prove dispatch,
  // nothing more.
  await submissionAdmitted;
  await expect(row.locator(".provisioning-submitting")).toBeVisible();
  await expect(page.getByRole("button", { name: "add host" })).toBeDisabled();
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
  await expect(row.locator(".provisioning-submitting")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
  // The acceptance's own reread observes held running A, so the accepted
  // run is reconciled before the stale detour begins.
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );

  // The stale older-run success lands on a read dispatched after that
  // installation. It renders, yet settles nothing: the row stays expanded
  // for the live submission.
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
  await expect(row.locator(".host-edit")).toBeDisabled();
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
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

test("an update expands at acceptance and collapses on immediate completion", async ({
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  // Expansion happens at click acceptance, before planning returns: the
  // planning indicator is showing, only the old completed setup run is
  // visible (no new run exists yet), and the checkbox is still unchecked.
  await expect(row.locator(".provisioning-planning")).toBeVisible();
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  await expect(row.locator(".host-detail")).toBeVisible();
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
  releaseProgress();
  // Matching completion without ever observing Running still settles.
  await expect(row.locator(".host-detail")).toHaveCount(0);
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(1);
  expect(updates.confirms).toBe(1);
});

test("global detail toggles never move another row's automatic disclosure", async ({
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
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  const idle = page.locator(`[data-host-id="${other.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );

  // Toggling global details on details every row; toggling it back off
  // collapses the bystander but leaves the running row's automatic
  // disclosure exactly where it was.
  await page.locator(".host-details-toggle").click();
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(idle.locator(".host-detail")).toBeVisible();
  await page.locator(".host-details-toggle").click();
  await expect(idle.locator(".host-detail")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );

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
  // The alias marker is the render barrier: the destination-detail line
  // appears only once the rewritten entry has rendered.
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
      if (seen === "moved") entry.destination = moved;
    }
    await route.fulfill({ response, json: body });
  });
  const updates = countUpdateRequests(page, accepted.host_id);
  await page.goto("/");
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );

  // A reconnect mid-run (fresh incarnation, same target) is normal: the
  // run stays followed and the row stays expanded.
  phase = "reconnected";
  await notifyFeed(feed, 2);
  await expect(row.locator(".host-destination-detail")).toBeVisible();
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "running",
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(row.locator(".provisioning-warning")).toHaveCount(0);

  // A real target change detaches the run evidence with an explicit
  // unresolved diagnostic instead of following it onto the new target —
  // and the old target's retained run is never re-adopted.
  phase = "moved";
  await notifyFeed(feed, 3);
  await expect(row.locator(".host-destination-detail")).toContainText(moved);
  await expect(row.locator(".provisioning-warning")).toContainText(
    "the host changed while its update was being followed",
  );
  await expect(row.locator(".host-detail")).toBeVisible();
  await notifyFeed(feed, 4);
  await expect(row.locator(".provisioning-warning")).toContainText(
    "the host changed while its update was being followed",
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
    "the host changed while its update was being followed",
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
  // Suppressed: the row still shows the old completed ADD view, with no
  // adopted run and no warning, while the submission is outstanding.
  await expect(row.locator(".provisioning-run")).toHaveAttribute(
    "data-provisioning-status",
    "completed",
  );
  await expect(row.locator(".provisioning-warning")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();

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
  await expect(row.locator(".provisioning-submitting")).toBeVisible();

  // Retarget while the 202 is held: the submitting intent belongs to the old
  // row, so the watcher drops it with nothing tracked and no diagnostic —
  // the row goes quiet, which is the premise this branch needs.
  moved = true;
  await notifyFeed(feed, 2);
  await expect(row.locator(".provisioning-submitting")).toHaveCount(0);
  await expect(row.locator(".host-detail")).toHaveCount(0);

  // The late acceptance lands on the mismatch branch: unknown outcome, this
  // row claims nothing — and the old run id retires before the follow-up
  // reread that would otherwise adopt it as new-target work.
  releaseAcceptance();
  await expect(row.locator(".provisioning-warning")).toContainText(
    "the host changed while the update was submitting",
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
    "the host changed while the update was submitting",
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
    "the host changed while the update was submitting",
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
    "the host changed while the update was submitting",
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
  await expect(row.locator(".provisioning-submitting")).toBeVisible();

  // Retarget while the 202 is held: the submitting intent belongs to the old
  // row, so the watcher drops it with nothing tracked and no diagnostic —
  // the row goes quiet, which is the premise this ordering needs.
  moved = true;
  await notifyFeed(feed, 2);
  await expect(row.locator(".provisioning-submitting")).toHaveCount(0);
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
  // A with the new binding and owns the row.
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
    "the host changed while the update was submitting",
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
    "the host changed while the update was submitting",
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
    "the host changed while the update was submitting",
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
    "the host changed while the update was submitting",
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
  await openHostMenu(row);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-planning")).toBeVisible();
  await expect(row.locator(".provisioning-planning")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toContainText("accepted the update");
  await expect(row.locator(".host-detail")).toBeVisible();
  await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  expect(updates.plans).toBe(3);
  expect(updates.confirms).toBe(1);
});
