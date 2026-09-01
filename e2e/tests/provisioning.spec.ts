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

import { expect, Page, APIRequestContext, test, TestInfo } from "@playwright/test";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import { openHostMenu, openHostsPanel, stubFeed, type FeedStub } from "./helpers/fleet";
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

/** Open the real add form and submit one destination — opening the
 * hosts panel itself first (idempotent), so callers need no per-call
 * prerequisite. */
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
  await page.goto("/");
  await openHostsPanel(page);
  const local = page.locator('[data-host-kind="local"]');
  await expect(local.locator(".provisioning-error")).toContainText("temporary local probe failure");
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
  await page.goto("/");
  await openHostsPanel(page);
  const local = page.locator('[data-host-kind="local"]');
  await expect(local.locator(".provisioning-manual:not(.secondary)")).toContainText(
    "farhelm supervisor run",
  );
  await expect(local.locator(".provisioning-error")).toContainText(
    "this is the helm's own machine; run farhelm helm setup here instead of provisioning from the panel",
  );
  await expect(local.locator(".provisioning-plan")).toHaveCount(0);
  await expect(local.locator(".provisioning-auto-setup")).toHaveCount(0);

  state.down = false;
  await notifyFeed(feed, 2);
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
test("UPDATE plans once, binds to the row, and releases OpLock at acceptance", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await expect(row.locator(".provisioning-update")).toBeVisible();
  await page.evaluate((id) => {
    const button = document.querySelector(`[data-host-id="${id}"] .provisioning-update`)!;
    button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  }, accepted.host_id);
  await expect(row.locator(".provisioning-plan")).toBeVisible();
  expect((await backendEvents()).filter((event) => event.event === "probe")).toHaveLength(1);

  await row.locator(".provisioning-confirm").dispatchEvent("click");
  await expect(row.locator(".provisioning-run")).toHaveAttribute("data-provisioning-status", "running");
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
  // `.host-edit` lives inside the row's own "⋯" menu now — see the
  // earlier ADD case's comment for why `toBeDisabled()` still applies.
  await openHostMenu(row);
  await expect(row.locator(".host-edit")).toBeDisabled();
});

test("a row change and an observed foreign run each discard a pending plan", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "binding");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-plan")).toBeVisible();

  const changed = `${remote}-moved`;
  const response = await request.post(`/api/hosts/${accepted.host_id}/destination`, {
    data: { ssh: changed },
  });
  expect(response.ok(), await responseBody(response)).toBe(true);
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);

  await configureBackend({ targets: { [target(changed)]: { hold_actions: true } } });
  await row.locator(".provisioning-update").dispatchEvent("click");
  await expect(row.locator(".provisioning-plan")).toBeVisible();
  const competingPlan = await request.post(`/api/hosts/${accepted.host_id}/update`);
  expect(competingPlan.ok(), await responseBody(competingPlan)).toBe(true);
  const competing = (await competingPlan.json()) as { probe_id: string };
  const competingRun = await request.post(`/api/hosts/${accepted.host_id}/update`, {
    data: { probe_id: competing.probe_id },
  });
  expect(competingRun.status()).toBe(202);
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
});

test("UPDATE planning refusal shows the concrete reason without minting a confirmation", async ({
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
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  const refusal = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/hosts/${accepted.host_id}/update`
    && response.request().method() === "POST",
  );
  await row.locator(".provisioning-update").dispatchEvent("click");
  await refusal;
  await expect(row.locator(".provisioning-error")).toContainText(
    'injected provisioning inspection failure: host stderr "inspection refused \\u{202e}host\\u{200b}"',
    { timeout: 10_000 },
  );
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
});

test("a refused UPDATE consumes its plan and leaves unrelated controls usable", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update-refusal");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await configureBackend({ targets: { [target(remote)]: { hold_actions: true } } });
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // This is the stale-client refusal case. Keep this page on its last
  // completed view while the API client starts a competing run; otherwise
  // ordinary progress reconciliation correctly withdraws the plan before
  // confirmation can exercise the helm's synchronous Busy refusal.
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
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  const planResponse = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/hosts/${accepted.host_id}/update`,
  );
  await row.locator(".provisioning-update").dispatchEvent("click");
  const displayed = (await (await planResponse).json()) as { probe_id: string };
  const competingPlan = await request.post(`/api/hosts/${accepted.host_id}/update`);
  const competing = (await competingPlan.json()) as { probe_id: string };
  const competingRun = await request.post(`/api/hosts/${accepted.host_id}/update`, {
    data: { probe_id: competing.probe_id },
  });
  expect(competingRun.status()).toBe(202);
  expect(displayed.probe_id).not.toBe(competing.probe_id);

  await row.locator(".provisioning-confirm").dispatchEvent("click");
  await expect(row.locator(".provisioning-error")).toContainText("in flight");
  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
});

test("mismatched accepted identity routes progress to the returned host", async ({
  page,
  request,
}, testInfo) => {
  const first = destination(testInfo, "returned-from");
  const second = destination(testInfo, "returned-to");
  const from = await startAdd(request, first);
  const to = await startAdd(request, second);
  await waitForProgress(request, from.host_id, "completed");
  await waitForProgress(request, to.host_id, "completed");
  let returnedRead = false;
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === `/api/hosts/${to.host_id}/provisioning`) {
      returnedRead = true;
    }
  });
  await page.route(`**/api/hosts/${from.host_id}/update`, async (route) => {
    if (!route.request().postData()) return route.continue();
    await route.fulfill({
      status: 202,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: JSON.stringify({ host_id: to.host_id, run_id: "rerouted-run" }),
    });
  });
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${from.host_id}"]`);
  const planResponse = page.waitForResponse(
    (response) => new URL(response.url()).pathname === `/api/hosts/${from.host_id}/update`,
  );
  await row.locator(".provisioning-update").dispatchEvent("click");
  expect((await planResponse).status()).toBe(200);
  await row.locator(".provisioning-confirm").dispatchEvent("click");
  await expect.poll(() => returnedRead).toBe(true);
  await expect(row.locator(".provisioning-error")).toHaveCount(0);
});

test("a malformed accepted UPDATE consumes the plan, warns, and releases page controls", async ({
  page,
  request,
}, testInfo) => {
  const remote = destination(testInfo, "update-unvalidated");
  const accepted = await startAdd(request, remote);
  await waitForProgress(request, accepted.host_id, "completed");
  await page.route(`**/api/hosts/${accepted.host_id}/update`, async (route) => {
    if (!route.request().postData()) return route.continue();
    await route.fulfill({
      status: 202,
      headers: { "content-type": "application/json", "x-farhelm-build": HELM_BUILD },
      body: "{not-json",
    });
  });
  await page.goto("/");
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await row.locator(".provisioning-update").dispatchEvent("click");
  await row.locator(".provisioning-confirm").dispatchEvent("click");

  await expect(row.locator(".provisioning-plan")).toHaveCount(0);
  await expect(row.locator(".provisioning-warning")).toContainText("accepted");
  await expect(page.getByRole("button", { name: "add host" })).toBeEnabled();
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
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${accepted.host_id}"]`);
  await expect(row.locator(".provisioning-run-message")).toContainText(
    "supervisor started but attachment failed",
  );
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
// falling back to discovery or the ADD confirmation route.
test("failed UPDATE rerun plans and confirms through the host update route", async ({
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
  await openHostsPanel(page);
  const row = page.locator(`[data-host-id="${added.host_id}"]`);
  const rerunPlanResponse = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/hosts/${added.host_id}/update`
    && response.request().method() === "POST",
  );
  await row.locator(".provisioning-rerun").dispatchEvent("click");
  expect((await rerunPlanResponse).status()).toBe(200);
  await expect(row.locator(".provisioning-plan")).toBeVisible();
  expect(updateRequests).toEqual(["plan"]);
  expect(addProbeRequests).toBe(0);

  const rerunResponse = page.waitForResponse((response) =>
    new URL(response.url()).pathname === `/api/hosts/${added.host_id}/update`
    && response.request().method() === "POST",
  );
  await row.locator(".provisioning-confirm").dispatchEvent("click");
  expect((await rerunResponse).status()).toBe(202);
  const running = await waitForProgress(request, added.host_id, "running");
  expect(running.operation).toBe("update");
  expect(updateRequests).toEqual(["plan", "confirm"]);
  expect(addProbeRequests).toBe(0);
});
