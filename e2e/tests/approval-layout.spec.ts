// Layout and selection are client behavior. Stage controlled approval
// replies over the real UI and owned session, while the spawn spec retains the
// real agent-to-helm approval path. The controlled feed makes expiry explicit.
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  pinAutoSelect,
  SESSION_LISTING,
  stubFeed,
  type SessionRow,
} from "./helpers/fleet";
import { attachSession, terminalInput } from "./helpers/term";
import { fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";
import { routeGate } from "./helpers/route-gate";

// Capture the real helm's build stamp before fabricating replies. Missing
// stamps deliberately put the UI into build-skew mode and close its feed.
installTerminalSuiteHooks();

/** Permit action-specific fields while keeping listing identity explicit in the fixture. */
type Approval = { id: string; host_name: string; [field: string]: unknown };

/** A complete launch card with a requester title that can exercise truncation. */
function launchRequest(id: string, host = "devbox", title = "fix-flaky-ci"): Approval {
  return {
    id,
    host_id: host === "devbox" ? 1 : 2,
    host_name: host,
    session: { id: "7c1e9a52-4b0d-4f7e-9a3c-2d81f6b0e413", title, host_name: host },
    expires_at_ms: Date.now() + 540_000,
    action: {
      kind: "launch",
      verb: "create",
      host_name: "buildbox",
      cwd: "~/src/app/services/ingest",
      title: null,
      source: null,
      launch: {
        kind: "agent",
        selection: { harness: "claude", model: "opus", effort: "high", permissions: "guarded" },
        start: [
          "claude", "--model", "opus", "--effort", "high", "--permission-mode", "auto", "{farhelm_args}",
          "--", "Investigate why tests/e2e/ingest_retry.rs fails on CI and propose a fix",
        ],
        resume: null,
      },
    },
  };
}

/** An add request uses the normal requester identity and editable form fields. */
function hostAddRequest(id: string): Approval {
  return { ...launchRequest(id, "build\u202ebox", "ask\u202eer"), action: {
    kind: "host_add", destination: "user@original.invalid", alias: "original alias",
  } };
}

/** Own the listing and answer replies; expiry is a new listing, never a client timer. */
async function stageApprovals(page: Page, initial: Approval[]) {
  let waiting = initial;
  const answers: Array<{ id: string; answer: string }> = [];
  let revision = 1;
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  /** Listing mutations need a connected feed so the UI must read the changed reply. */
  async function notifyListing() {
    await expect.poll(() => feed.openSockets(), { message: "listing change needs the owned live feed" }).toBe(1);
    feed.notify(++revision);
  }
  await page.route((url) => url.pathname === "/api/approvals", async (route) => {
    await fulfillAsHelm(route, { json: { approvals: waiting } });
  });
  await page.route((url) => url.pathname.startsWith("/api/approvals/"), async (route) => {
    const id = decodeURIComponent(new URL(route.request().url()).pathname.split("/").at(-1)!);
    answers.push({ id, answer: route.request().postDataJSON().answer });
    waiting = waiting.filter((card) => card.id !== id);
    await fulfillAsHelm(route, { status: 204 });
  });
  return {
    answers,
    async replace(next: Approval[]) {
      waiting = next;
      await notifyListing();
    },
    async expire(id: string) {
      waiting = waiting.filter((card) => card.id !== id);
      await notifyListing();
    },
  };
}

/** Control only the window-focus input at the DOM boundary. Playwright treats
 * every page as focused, so bringToFront cannot establish an inactive window
 * (https://playwright.dev/docs/pages#multiple-pages). The real application still
 * samples this API on arrival and admission; restoring focus removes the override
 * rather than replacing the native positive result. Visibility stays native. */
async function setWindowFocused(page: Page, focused: boolean) {
  await page.evaluate((active) => {
    if (active) Reflect.deleteProperty(document, "hasFocus");
    else {
      const samples = { count: 0 };
      Object.defineProperty(document, "__inactiveFocusSamples", { configurable: true, value: samples });
      Object.defineProperty(document, "hasFocus", { configurable: true, value: () => {
        samples.count++;
        return false;
      } });
    }
    window.dispatchEvent(new Event(active ? "focus" : "blur"));
  }, focused);
  await expect.poll(() => page.evaluate(() => ({
    focused: document.hasFocus(), visible: document.visibilityState !== "hidden",
  })), { message: "premise: document focus input changed without hiding the page" })
    .toEqual({ focused, visible: true });
}

/** Observe a fresh production read without calling the focus API from the assertion. */
async function inactiveFocusSamples(page: Page): Promise<number> {
  return page.evaluate(() => Reflect.get(document, "__inactiveFocusSamples").count);
}

let session: SessionRow | undefined;

// The real selected view supplies the top row and terminal whose bounds matter.
// Cleanup includes partially completed setup; no shared fixture is renamed.
test.beforeEach(async ({ page, request }) => {
  session = undefined;
  session = await createSession(request, { title: `approval-layout-${Date.now()}` });
  await pinAutoSelect(page, session.id);
});
test.afterEach(async ({ request }) => {
  if (session) await cleanupSession(request, session.id);
});

/** Establish the selected view before geometry or modal assertions use its chrome. */
async function openSelected(page: Page) {
  await page.goto("/");
  await attachSession(page, session!.id);
  await expect(page.locator(".app-main .terminal-panes")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".approval-card .approval-allow")).toBeEnabled();
}

/** Read just the rectangles needed to distinguish pane-relative placement from a window corner. */
async function placement(page: Page) {
  return page.evaluate(() => {
    const rect = (selector: string) => {
      const { x, y, width, height, bottom, right } = document.querySelector(selector)!.getBoundingClientRect();
      return { x, y, width, height, bottom, right };
    };
    return {
      main: rect(".app-main"),
      terminal: rect(".app-main .terminal-panes"),
      card: rect(".approval-card"),
      region: rect(".approval-cards"),
      actions: rect(".approval-card-actions"),
    };
  });
}

/** The positive answer must retain edits while its own listing entry disappears,
 * and must await the receipt response before any ordinary discovery request. */
test("focused host add preserves edited fields and waits for its own answer receipt", async ({ page }) => {
  const staged = await stageApprovals(page, []);
  const gate = routeGate();
  const answers: unknown[] = [];
  const probes: unknown[] = [];
  await page.route("**/api/approvals/add-edited", async (route) => {
    answers.push(route.request().postDataJSON());
    await gate.wait();
    await fulfillAsHelm(route, { status: 204 });
  });
  await page.route("**/api/hosts/probe", async (route) => {
    probes.push(route.request().postDataJSON());
    await fulfillAsHelm(route, { json: { result: "manual", reason: "controlled discovery refusal" } });
  });
  await page.goto("/");
  await page.bringToFront();
  await expect.poll(() => page.evaluate(() => document.hasFocus() && document.visibilityState !== "hidden"),
    { message: "premise: this window is live and focused before request arrival" }).toBe(true);
  const hostile = { ...hostAddRequest("add-edited"), action: {
    kind: "host_add", destination: "user@or\u202eiginal.invalid", alias: "or\u202eiginal alias",
  } };
  await staged.replace([hostile]);
  const form = page.locator(".add-host-form");
  try {
    await expect(form).toBeVisible();
    await expect(form.locator(".add-host-ssh")).toHaveValue("user@or\u202eiginal.invalid");
    await expect(form.locator(".add-host-alias")).toHaveValue("or\u202eiginal alias");
    await expect(form.locator(".agent-add-requester")).toContainText("build<U+202E>box");
    await expect(form.locator(".agent-add-requester")).toContainText("ask<U+202E>er");
    await expect(form.locator(".agent-add-approved-fields")).toContainText("user@or<U+202E>iginal.invalid");
    await expect(form.locator(".agent-add-approved-fields")).toContainText("or<U+202E>iginal alias");
    await expect(page.locator(".approval-always-allow")).toHaveCount(0);
    await form.locator(".add-host-ssh").fill("user@edited.invalid");
    await form.locator(".add-host-alias").fill("edited alias");
    await staged.replace([hostile]);
    await expect(form.locator(".add-host-ssh")).toHaveValue("user@edited.invalid");
    await expect(form.locator(".add-host-alias")).toHaveValue("edited alias");
    await expect(form.locator(".agent-add-approved-fields")).toContainText("user@edited.invalid");
    await form.locator(".add-host-submit").click();
    await expect.poll(() => answers.length).toBe(1);
    expect(answers).toEqual([{ answer: "allow", host_add: { destination: "user@edited.invalid" } }]);
    await staged.expire("add-edited");
    await expect(page.locator(".approval-card")).toHaveCount(0);
    await expect(form, "answering form survives disappearance caused by its own POST").toBeVisible();
    expect(probes, "no discovery before positive receipt").toHaveLength(0);
    gate.release();
    await expect.poll(() => probes.length).toBe(1);
    expect(probes[0]).toMatchObject({ target: { destination: "user@edited.invalid" }, new_entry: { alias: "edited alias" } });
    await expect(form).toContainText("controlled discovery refusal");
    await form.locator(".add-host-cancel").click();
    await expect(form).toHaveCount(0);
    expect(answers, "ordinary cancel after acceptance is not another Deny").toHaveLength(1);
  } finally { gate.release(); }
});

/** A request first observed in the background stays a card; explicitly opening
 * it never grants approval, and Escape sends the same route's Deny once. */
test("inactive host add stays a card and explicit opening cancels through Deny", async ({ page }) => {
  const staged = await stageApprovals(page, []);
  await page.goto("/");
  await setWindowFocused(page, false);
  try {
    await staged.replace([hostAddRequest("add-inactive")]);
    const card = page.locator('[data-approval-id="add-inactive"]');
    await expect(card.locator(".approval-allow")).toBeEnabled();
    await expect(page.locator(".add-host-form")).toHaveCount(0);
    await setWindowFocused(page, true);
    await card.getByRole("button", { name: "Open add dialog", exact: true }).click();
    const form = page.locator(".add-host-form");
    await expect(form).toBeVisible();
    expect(staged.answers, "opening is not approval").toEqual([]);
    await expect(form.locator(".add-host-ssh")).toBeFocused();
    await form.locator(".add-host-ssh").press("Escape");
    await expect(form).toHaveCount(0);
    expect(staged.answers).toEqual([{ id: "add-inactive", answer: "deny" }]);
  } finally { await setWindowFocused(page, true); }
});

/** Automatic admission must preserve a manual draft and recheck focus after
 * that draft closes. Returning focus later does not auto-open an old card. */
test("host add admission keeps a manual draft and rechecks live focus", async ({ page }) => {
  const staged = await stageApprovals(page, []);
  await page.goto("/");
  await page.bringToFront();
  await expect.poll(() => page.evaluate(() => document.hasFocus())).toBe(true);
  await page.locator(".host-rail-add").click();
  const form = page.locator(".add-host-form");
  await form.locator(".add-host-ssh").fill("user@manual-draft.invalid");
  await form.locator(".add-host-alias").fill("manual alias");
  await staged.replace([hostAddRequest("add-deferred")]);
  await expect(page.locator(".approval-allow")).toBeEnabled();
  await expect(form).toHaveCount(1);
  await expect(form.locator(".add-host-ssh")).toHaveValue("user@manual-draft.invalid");
  await expect(form.locator(".add-host-alias")).toHaveValue("manual alias");
  await setWindowFocused(page, false);
  try {
    const beforeAdmission = await inactiveFocusSamples(page);
    // Invoke the owned manual cancellation without bringing its background
    // window forward; the queued agent opening must now fail admission.
    await form.locator(".add-host-cancel").evaluate((button: HTMLButtonElement) => button.click());
    await expect(form).toHaveCount(0);
    // Rejection must finish before native focus is restored; otherwise a late
    // admission sample could observe the restored value and open the old card.
    await expect.poll(() => inactiveFocusSamples(page),
      { message: "queued admission sampled inactive focus after the manual dialog closed" }).toBeGreaterThan(beforeAdmission);
    await expect(page.locator('[id^="host-modal-"]')).toHaveCount(0);
    await expect(page.locator('[data-approval-id="add-deferred"]')).toBeVisible();
    await setWindowFocused(page, true);
    await expect(form).toHaveCount(0);
    await page.locator(".approval-allow").click();
    await expect(form.locator(".add-host-ssh")).toHaveValue("user@original.invalid");
    expect(staged.answers).toEqual([]);
    await form.locator(".add-host-cancel").click();
    await expect(form).toHaveCount(0);
    expect(staged.answers).toEqual([{ id: "add-deferred", answer: "deny" }]);
  } finally { await setWindowFocused(page, true); }
});

/** A focused listing may contain several arrivals. Present them in order
 * without replacing the current draft, and omit a queued request once a
 * successful refresh proves that it expired. */
test("simultaneous host adds open in order and omit expired queued requests", async ({ page }) => {
  const staged = await stageApprovals(page, []);
  await page.goto("/");
  await page.bringToFront();
  await expect.poll(() => page.evaluate(() => document.hasFocus())).toBe(true);
  const cards = ["first", "expired", "last"].map((id) => ({
    ...hostAddRequest(id), action: { kind: "host_add", destination: `user@${id}.invalid`, alias: id },
  }));
  await staged.replace(cards);
  const form = page.locator(".add-host-form");
  await expect(form.locator(".add-host-ssh")).toHaveValue("user@first.invalid");
  await expect(form.locator(".add-host-cancel")).toBeEnabled();
  await staged.expire("expired");
  await expect(page.locator('[data-approval-id="expired"]')).toHaveCount(0);
  await expect(form.locator(".add-host-ssh")).toHaveValue("user@first.invalid");
  await form.locator(".add-host-cancel").click();
  await expect(form.locator(".add-host-ssh")).toHaveValue("user@last.invalid");
  expect(staged.answers).toEqual([{ id: "first", answer: "deny" }]);
  await form.locator(".add-host-cancel").click();
  await expect(form).toHaveCount(0);
  expect(staged.answers).toEqual([{ id: "first", answer: "deny" }, { id: "last", answer: "deny" }]);
});

/** Definite rejection and expiry cannot start discovery. The refused form stays
 * editable; a later Cancel may still answer a request the helm kept waiting. */
for (const status of [200, 409, 410]) {
  test(`host add answer ${status} starts zero probes`, async ({ page }) => {
    const staged = await stageApprovals(page, []);
    let answers = 0;
    let probes = 0;
    await page.route("**/api/approvals/add-refused", async (route) => {
      answers++;
      await fulfillAsHelm(route, { status, body: "controlled add answer refusal" });
    });
    await page.route("**/api/hosts/probe", async (route) => {
      probes++;
      await route.abort();
    });
    await page.goto("/");
    await page.bringToFront();
    await expect.poll(() => page.evaluate(() => document.hasFocus())).toBe(true);
    await staged.replace([hostAddRequest("add-refused")]);
    const form = page.locator(".add-host-form");
    await expect(form).toBeVisible();
    await form.locator(".add-host-submit").click();
    await expect(form.locator(".add-host-error")).toContainText(status === 410 ? "no longer waiting"
      : status === 200 ? "did not confirm acceptance" : "controlled add answer refusal");
    expect(answers).toBe(1);
    expect(probes).toBe(0);
    await staged.expire("add-refused");
    await expect(form).toHaveCount(0);
  });
}

/** The server's sole answer winner is simulated here; helm tests exercise
 * its real take/receipt. Each page must start discovery only after its own
 * positive response, even when both had opened and edited the same request. */
test("only the winning host add window probes its captured edited fields", async ({ page, context }) => {
  const other = await context.newPage();
  const gate = routeGate();
  const windows = [page, other];
  const staged = await Promise.all(windows.map((window) => stageApprovals(window, [])));
  const answers: number[] = [];
  const probes: Array<{ window: number; body: unknown }> = [];
  let winner: number | undefined;
  try {
    for (const [index, window] of windows.entries()) {
      await window.route("**/api/approvals/add-race", async (route) => {
        answers.push(index);
        if (winner === undefined) {
          winner = index;
          await gate.wait();
          await fulfillAsHelm(route, { status: 204 });
        } else { await fulfillAsHelm(route, { status: 410 }); }
      });
      await window.route("**/api/hosts/probe", async (route) => {
        probes.push({ window: index, body: route.request().postDataJSON() });
        await fulfillAsHelm(route, { json: { result: "manual", reason: "winner discovery complete" } });
      });
      await window.goto("/");
    }
    await other.bringToFront();
    await expect.poll(() => other.evaluate(() => document.hasFocus())).toBe(true);
    await setWindowFocused(page, false);
    await staged[0].replace([hostAddRequest("add-race")]);
    await staged[1].replace([hostAddRequest("add-race")]);
    await expect(other.locator(".add-host-form")).toBeVisible();
    await expect(page.locator(".approval-allow")).toBeEnabled();
    await expect(page.locator(".add-host-form")).toHaveCount(0);
    await setWindowFocused(page, true);
    await page.locator(".approval-allow").click();
    for (const [index, window] of windows.entries()) {
      await expect(window.locator(".add-host-form")).toBeVisible();
      await window.locator(".add-host-ssh").fill(`user@window-${index}.invalid`);
      await window.locator(".add-host-alias").fill(`window ${index}`);
    }
    await page.locator(".add-host-submit").click();
    await expect.poll(() => answers.length).toBe(1);
    await other.locator(".add-host-submit").click();
    await expect(other.locator(".add-host-error")).toContainText("no longer waiting");
    await staged[0].expire("add-race");
    await staged[1].expire("add-race");
    await expect(page.locator(".approval-card")).toHaveCount(0);
    await expect(page.locator(".add-host-form")).toBeVisible();
    expect(probes, "neither window probes before the winner's receipt").toEqual([]);
    gate.release();
    await expect(page.locator(".add-host-error")).toContainText("winner discovery complete");
    expect(answers).toEqual([0, 1]);
    expect(probes).toEqual([{ window: 0, body: {
      target: { kind: "ssh", destination: "user@window-0.invalid" },
      remote_farhelm: null, remote_state_dir: null, new_entry: { alias: "window 0" },
    } }]);
  } finally { gate.release(); await other.close(); }
});

/** Exercise the product's decision shapes with text large enough to expose accidental wrapping.
 * The wire fixtures deliberately include optional facts that a compact example
 * would omit. Rust row tests audit content; these cases audit the browser's
 * containment and scrolling contract for each shape of decision.
 */
function layoutRequests(): Approval[] {
  const host = `remote-${"build".repeat(8)}`;
  const title = `Investigate ${"long session title ".repeat(8)}`;
  const targetBare = `target-${"x".repeat(32)}`;
  expect(new TextEncoder().encode(targetBare).length, "premise: target ID fits the helm's 40-byte admission bound").toBe(39);
  const target = { id: `12345678-1234-1234-1234-123456789abc:${targetBare}`, title, host_name: host };
  const command = `agent ${"--review-long-command-argument ".repeat(60)}`;
  const prompt = `${"A complete line of the requested message.\n".repeat(40)}PROMPT-END`;
  const folder = `/workspace/${"long-directory/".repeat(22)}`;
  const repo = `example-org/${"long-repository-name-".repeat(14)}`;
  const launch = {
    kind: "agent",
    selection: {
      harness: "claude", model: "opus", effort: "high", permissions: "custom",
      permission_flags: ["--allowedTools", "Read", `--security-tail=${"flag".repeat(65)}`],
      workspace_trust: true,
    },
    start: ["claude", "{farhelm_args}", command],
    resume: ["claude", "{farhelm_args}", "--resume", "{conversation}", command],
  };
  const fields = {
    kind: "command", agent: "claude", model: "opus", effort: "high", permissions: "custom",
    permission_flags: ["--allowedTools", `--security-tail=${"flag".repeat(65)}`],
    command, resume_command: command, yolo: true, prompt,
    wrapper_prefix: ["env", `WRAPPER=${"wrapper".repeat(90)}`],
    container: { engine: "podman", image: "example.invalid/agent:latest", api_proxy_port: 43210, github_token_file: folder },
    host: "12345678-1234-1234-1234-123456789abc", destination: { github: repo }, name: title, name_kind: "fixed",
  };
  const actions: Array<Record<string, unknown>> = [];
  for (const kind of ["launch", "launch_with_prompt", "launch_with_agent_wrapper"]) {
    for (const verb of ["spawn", "spawn_inherited", "create", "clone"]) {
      actions.push({
        kind, verb, host_name: host, cwd: folder, repo, title,
        source: verb === "clone" ? target : null,
        launch: kind === "launch_with_agent_wrapper"
          ? { ...launch, selection: { ...launch.selection, wrapper_prefix: fields.wrapper_prefix },
            start: [...fields.wrapper_prefix, ...launch.start], resume: [...fields.wrapper_prefix, ...launch.resume] }
          : launch,
        ...(kind !== "launch" ? { prompt } : {}),
      });
    }
  }
  actions.push(
    { kind: "rename", target, title },
    { kind: "stop", target },
    { kind: "delete", target, guard: "unconditional" },
    { kind: "restart", target, stop_if_running: true, launch },
    { kind: "prompt", target, launch, screen_yolo: true, attached: true, prompt },
    { kind: "launch", verb: "create", host_name: host, cwd: folder, repo: null, title, source: null,
      launch: { kind: "command", command, resume: command, agent: "claude", yolo: false } },
    { kind: "template_write", name: title, replaces_existing: false, fields, host_name: host },
    { kind: "template_write_with_shortcuts", name: title, replaces_existing: true, fields, host_name: host,
      shortcuts: { quick_launch: { change: "set", moved_from: title }, key: { change: "set", key: "2", replaced_key: "1", moved_from: title } } },
    { kind: "template_delete", name: title, fields: { ...fields, destination: { folder } }, host_name: host },
    { kind: "host_add", destination: `operator@${"destination".repeat(30)}.invalid`, alias: title },
    { kind: "host_edit", host_name: host, destination: `operator@${"destination".repeat(30)}.invalid`,
      alias: { old: title, new: `${title} revised` }, icon: { old: "cloud", new: "rocket" }, color: { old: "default", new: "teal" } },
    { kind: "host_remove", host_name: host, destination: `operator@${"destination".repeat(30)}.invalid` },
  );
  for (const preapproved of [false, true]) {
    for (const local_network of [false, true]) {
      actions.push({ kind: "open_url", preapproved, local_network, tunnel_available: false,
        url: `https://example.test/${"long-path/".repeat(35)}?security-tail=END` });
    }
  }
  // A web renderer must retain the link even when the helm offers a native
  // tunnel. The desktop-only action itself has no browser rendering path.
  actions.push({ kind: "open_url", preapproved: false, local_network: true, tunnel_available: true,
    url: `http://127.0.0.1:8765/${"long-path/".repeat(35)}?security-tail=END` });
  return actions.map((action, index) => ({
    ...launchRequest(`audit-${index}`, host, title),
    own_host_session_action: ["launch", "launch_with_prompt", "rename", "stop", "delete", "restart", "prompt"].includes(String(action.kind)),
    action,
  }));
}

/** Set the real pane's width without assuming the sidebar's current size.
 * At its 320px minimum the shell may scroll; reveal the pane's own left edge
 * before taking viewport-dependent measurements. The pane can settle before
 * the approval ResizeObserver runs, so the card's centered width and position
 * are a separate readiness oracle before callers record an anchor.
 */
async function setPaneWidth(page: Page, width: number) {
  await page.setViewportSize({ width: 1600, height: 1200 });
  const main = (await placement(page)).main;
  await page.setViewportSize({ width: Math.round(main.x + width), height: 1200 });
  await page.locator(".app-shell").evaluate((node) => {
    node.scrollLeft = (node.querySelector(".app-main") as HTMLElement).offsetLeft;
  });
  await expect.poll(async () => (await placement(page)).main.width, { message: "premise: measured main-pane width" })
    .toBeCloseTo(width, 0);
  await expect.poll(async () => {
    const { main, card } = await placement(page);
    const wantedWidth = Math.min(660, main.width - 24);
    return Math.abs(card.width - wantedWidth) < 0.5
      && Math.abs(card.x - (main.x + (main.width - wantedWidth) / 2)) < 0.5;
  }, { message: "premise: the approval observer applied this pane's current width and center" }).toBe(true);
}

/** Sample containment, not just CSS declarations that could describe a broken layout.
 * Horizontal overflow is permitted only in the declared single-line fact
 * values and text boxes. Clipped requester text is checked separately.
 */
async function cardGeometry(page: Page) {
  return page.locator(".approval-cards").evaluate((region) => {
    const bounds = region.getBoundingClientRect();
    const card = region.querySelector(".approval-card")!;
    const header = card.querySelector(".approval-card-header")!.getBoundingClientRect();
    const from = card.querySelector(".approval-card-from")!.getBoundingClientRect();
    const actions = card.querySelector(".approval-card-actions")!.getBoundingClientRect();
    const inside = (rect: DOMRect) => rect.left >= bounds.left - 1 && rect.right <= bounds.right + 1
      && rect.top >= bounds.top - 1 && rect.bottom <= bounds.bottom + 1;
    const horizontalLeaks = Array.from(card.querySelectorAll<HTMLElement>(".approval-card-body, .approval-card-facts, .approval-card-row, .approval-card-command, .approval-card-block-row, .approval-card-actions"))
      .filter(node => node.scrollWidth > node.clientWidth + 1).map(node => node.className);
    return {
      regionOverflowX: region.scrollWidth - region.clientWidth,
      regionScrollTop: region.scrollTop,
      headerInside: inside(header), fromInside: inside(from), actionsInside: inside(actions),
      buttonsInside: Array.from(card.querySelectorAll(".approval-card-actions button"), button => inside(button.getBoundingClientRect())),
      horizontalLeaks,
      bodyOverflowX: (card.querySelector(".approval-card-body") as HTMLElement).scrollWidth
        - (card.querySelector(".approval-card-body") as HTMLElement).clientWidth,
    };
  });
}

for (const paneWidth of [960, 650, 320]) {
  /** All action shapes share one shell. Stepping the listing also catches
   * geometry left behind by the previously selected kind, unlike isolated
   * snapshots that mount each kind into a fresh region.
   */
  test(`every approval kind keeps its header and answers visible in a ${paneWidth}px pane`, async ({ page }) => {
    const fixtures = layoutRequests();
    expect([...new Set(fixtures.map(card => (card.action as { kind: string }).kind))].sort(), "premise: the audit includes every wire kind")
      .toEqual(["delete", "host_add", "host_edit", "host_remove", "launch", "launch_with_agent_wrapper", "launch_with_prompt", "open_url", "prompt", "rename", "restart", "stop", "template_delete", "template_write", "template_write_with_shortcuts"]);
    const staged = await stageApprovals(page, [launchRequest("audit-start")]);
    await openSelected(page);
    await setPaneWidth(page, paneWidth);
    await page.evaluate(() => document.fonts.ready);
    const anchor = (await placement(page)).card;
    await setWindowFocused(page, false);
    try {
      for (const fixture of fixtures) {
        await test.step(`${fixture.id}: ${(fixture.action as { kind: string }).kind}`, async () => {
          await staged.replace([fixture]);
          const card = page.locator(".approval-card");
          await expect(card).toHaveAttribute("data-approval-id", fixture.id);
          await expect(card).toHaveAttribute("data-approval-kind", (fixture.action as { kind: string }).kind);
          await expect(card.locator(".approval-card-actions .approval-allow")).toHaveCount(1);
          await expect(card.locator(".approval-card-actions .approval-deny")).toHaveCount(1);
          await expect.poll(() => cardGeometry(page), { message: `${fixture.id}: header, From and every answer stay inside the unscrolled region` }).toMatchObject({
            regionOverflowX: 0, regionScrollTop: 0, headerInside: true, fromInside: true,
            actionsInside: true, horizontalLeaks: [], bodyOverflowX: 0,
          });
          const geometry = await cardGeometry(page);
          const current = (await placement(page)).card;
          expect(current.x, "kind changes preserve the horizontal anchor").toBeCloseTo(anchor.x, 0);
          expect(current.y, "kind changes preserve the terminal anchor").toBeCloseTo(anchor.y, 0);
          expect(current.width, "all kinds use the same card width").toBeCloseTo(anchor.width, 0);
          expect(geometry.buttonsInside.length).toBeGreaterThanOrEqual(2);
          expect(geometry.buttonsInside.every(Boolean), "every answer is fully visible before any scrolling").toBe(true);
          for (const block of await card.locator(".approval-card-block").all()) {
            await expect(block).toHaveAttribute("tabindex", "0");
            await expect(block).toHaveAttribute("aria-label", /.+/);
            expect(await block.evaluate(node => getComputedStyle(node).overflowY), "long text owns its scroll").toBe("auto");
          }
          const singleLines = await card.locator(".approval-fact-scroll").evaluateAll(nodes => nodes.map(node => ({
            label: node.closest(".approval-card-row")!.querySelector("dt")!.textContent,
            overflowing: node.scrollWidth > node.clientWidth,
            wrap: getComputedStyle(node).whiteSpace,
            overflow: getComputedStyle(node).overflowX,
            tabStop: node.hasAttribute("tabindex"),
            mono: getComputedStyle(node.querySelector(".peer-value")!).fontFamily.toLowerCase().includes("mono"),
          })));
          for (const value of singleLines) {
            expect(value).toMatchObject({ wrap: "nowrap", overflow: "auto", tabStop: false, mono: true });
            expect(["Folder", "Managed checkout of", "SSH destination", "Destination", "Container GitHub token file",
              "Session ID", "Copied session ID", "Site", "Host install"], "sideways scrolling is reserved for technical identities and destinations")
              .toContain(value.label);
          }
          if ((fixture.action as { kind: string }).kind !== "open_url") {
            expect(singleLines.some(value => value.overflowing), "premise: a long path, repository, destination or target ID needs its own sideways scroll").toBe(true);
          }
          const flags = approvalValue(page, "permission flags");
          if (await flags.count()) {
            await expect(flags).toContainText(`--security-tail=${"flag".repeat(65)}`);
            expect(await flags.evaluate(node => node.scrollWidth - node.clientWidth), "permission flag tails wrap in full").toBe(0);
          }
          if ((fixture.action as { kind: string }).kind === "open_url") {
            const url = approvalValue(page, "URL");
            await expect(url).toContainText("security-tail=END");
            expect(await url.evaluate(node => node.scrollWidth - node.clientWidth), "URL tails wrap inside the fact").toBe(0);
          }
        });
      }
      expect(staged.answers, "layout inspection cannot approve a request").toEqual([]);
    } finally { await setWindowFocused(page, true); }
  });
}

/** Read the CSS's actual text budgets so this test follows deliberate tuning.
 * Rectangle heights include borders and padding, while the min-height budget
 * must use the box's own box-sizing convention to mean the same thing.
 */
async function textBudgets(page: Page) {
  return page.locator(".approval-card-block").evaluateAll(nodes => nodes.map(node => {
    const style = getComputedStyle(node);
    const extra = style.boxSizing === "border-box" ? 0
      : parseFloat(style.paddingTop) + parseFloat(style.paddingBottom) + parseFloat(style.borderTopWidth) + parseFloat(style.borderBottomWidth);
    return {
      height: node.getBoundingClientRect().height,
      minimum: parseFloat(style.minHeight) + extra,
      maximum: parseFloat(style.maxHeight) + extra,
      overflowing: node.scrollHeight > node.clientHeight,
    };
  }));
}

/** Text consumes its shrink allowance before the shared facts start scrolling.
 * The first resize removes only half that allowance, so an implementation
 * that immediately scrolls an unshrunk body cannot satisfy the oracle. The
 * second resize exhausts it and must keep the answers pinned nevertheless.
 */
test("long boxes shrink to their computed minima before the body scrolls", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 2400 });
  const fixture = layoutRequests().find(card => (card.action as { kind: string; verb?: string }).kind === "launch_with_agent_wrapper"
    && (card.action as { verb?: string }).verb === "create")!;
  await stageApprovals(page, [fixture]);
  await openSelected(page);
  await page.evaluate(() => document.fonts.ready);
  const body = page.locator(".approval-card-body");
  const initial = await textBudgets(page);
  expect(initial.length, "premise: wrapper, command, resume and first prompt all compete for space").toBe(4);
  for (const box of initial) {
    expect(Number.isFinite(box.minimum) && box.minimum > 0, "premise: computed two-line minimum exists").toBe(true);
    expect(box.maximum).toBeGreaterThan(box.minimum);
    expect(box.overflowing, "premise: long text overflows even at its cap").toBe(true);
    expect(box.height).toBeCloseTo(box.maximum, 0);
  }
  expect(await body.evaluate(node => node.scrollHeight - node.clientHeight), "premise: the tall pane fits facts plus capped boxes").toBeLessThanOrEqual(1);
  const before = await placement(page);
  const cap = await page.locator(".approval-cards").evaluate(node => parseFloat(getComputedStyle(node).maxHeight));
  const allowance = initial.reduce((sum, box) => sum + box.height - box.minimum, 0);
  const desiredCap = before.card.height - allowance / 2;
  const paneRatio = cap / before.main.height;
  await page.setViewportSize({ width: 1280, height: Math.round(desiredCap / paneRatio + before.main.y) });
  await expect.poll(async () => (await textBudgets(page)).reduce((sum, box) => sum + box.height, 0),
    { message: "text boxes yield actual height when the cap becomes smaller" })
    .toBeLessThan(initial.reduce((sum, box) => sum + box.height, 0) - allowance / 4);
  expect(await body.evaluate(node => node.scrollHeight - node.clientHeight), "shrinking boxes preserve a non-scrolling fact list").toBeLessThanOrEqual(1);
  expect((await textBudgets(page)).every(box => box.height >= box.minimum - 1), "no text box crosses its computed minimum").toBe(true);
  await expect.poll(() => cardGeometry(page)).toMatchObject({ headerInside: true, actionsInside: true, regionScrollTop: 0 });

  await page.setViewportSize({ width: 1280, height: 500 });
  await expect.poll(() => body.evaluate(node => node.scrollHeight - node.clientHeight), {
    message: "only after text reaches its minimum does the body need to scroll",
  }).toBeGreaterThan(1);
  for (const box of await textBudgets(page)) expect(box.height).toBeCloseTo(box.minimum, 0);
  const pinned = await placement(page);
  await body.evaluate(node => { node.scrollTop = node.scrollHeight; });
  await expect.poll(() => body.evaluate(node => node.scrollTop), { message: "the overflowing body really scrolls" }).toBeGreaterThan(0);
  const scrolled = await placement(page);
  expect(scrolled.actions.y).toBeCloseTo(pinned.actions.y, 0);
  expect(scrolled.actions.bottom).toBeLessThanOrEqual(scrolled.region.bottom + 1);
  await expect.poll(() => cardGeometry(page)).toMatchObject({ headerInside: true, actionsInside: true, regionScrollTop: 0 });
});

/** A late receipt and a large queue each consume a bounded part of the region.
 * Moving the queue must not move the notice, card header or answers; the last
 * waiting request must still be selectable rather than merely present in DOM.
 */
test("waiting requests scroll independently below a tall card and fixed late notice", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  const staged = await stageApprovals(page, [launchRequest("late-receipt")]);
  await page.route("**/api/approvals/late-receipt", route => fulfillAsHelm(route, { status: 410 }));
  await openSelected(page);
  await page.locator(".approval-deny").click();
  const notice = page.locator(".approval-late");
  await expect(notice).toContainText("did not take effect");
  const tall = layoutRequests().find(card => (card.action as { kind: string }).kind === "launch_with_prompt")!;
  const queue = Array.from({ length: 12 }, (_, index) => launchRequest(`waiting-${index}`));
  await staged.replace([tall, ...queue]);
  await expect(page.locator(".approval-card")).toHaveAttribute("data-approval-id", tall.id);
  const waiting = page.locator(".approval-waiting-list");
  await expect(waiting.locator(".approval-waiting")).toHaveCount(queue.length);
  await expect.poll(() => waiting.evaluate(node => node.scrollHeight - node.clientHeight), {
    message: "premise: the queue exceeds its own visible allocation",
  }).toBeGreaterThan(1);
  const initial = await placement(page);
  const noticeBefore = (await notice.boundingBox())!;
  expect(noticeBefore.y).toBeGreaterThanOrEqual(initial.region.y);
  expect(noticeBefore.y + noticeBefore.height).toBeLessThanOrEqual(initial.region.bottom);
  await waiting.evaluate(node => { node.scrollTop = node.scrollHeight; });
  await expect.poll(() => waiting.evaluate(node => node.scrollTop)).toBeGreaterThan(0);
  await expect(page.locator('.approval-waiting[data-approval-id="waiting-11"]')).toBeInViewport();
  expect((await notice.boundingBox())!.y).toBeCloseTo(noticeBefore.y, 0);
  expect((await placement(page)).card.y).toBeCloseTo(initial.card.y, 0);
  expect((await placement(page)).actions.y).toBeCloseTo(initial.actions.y, 0);
  await expect.poll(() => cardGeometry(page)).toMatchObject({ headerInside: true, actionsInside: true, regionScrollTop: 0 });
  await page.locator('.approval-waiting[data-approval-id="waiting-11"]').click();
  await expect(page.locator(".approval-card")).toHaveAttribute("data-approval-id", "waiting-11");
  await expect(notice).toContainText("did not take effect");
});

/** Fixed queue and notice space counts against the card's pinned minimum.
 * At this boundary the viewport exceeds the narrow card's fallback constant,
 * but its siblings leave too little room for the card. Either ordinary
 * containment or the declared whole-region exception must retain every answer
 * and the last waiting request; hidden overflow is not a reachable fallback.
 */
test("a narrow short queue and late notice keep every answer reachable", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  const staged = await stageApprovals(page, [launchRequest("late-boundary")]);
  await page.route("**/api/approvals/late-boundary", route => fulfillAsHelm(route, { status: 410 }));
  await openSelected(page);
  await page.locator(".approval-deny").click();
  await expect(page.locator(".approval-late")).toContainText("did not take effect");
  const tall = layoutRequests().find(card => (card.action as { kind: string }).kind === "launch_with_prompt")!;
  const queue = Array.from({ length: 12 }, (_, index) => launchRequest(`boundary-waiting-${index}`));
  await staged.replace([tall, ...queue]);
  await expect(page.locator(".approval-card")).toHaveAttribute("data-approval-id", tall.id);
  await expect(page.locator(".approval-waiting")).toHaveCount(queue.length);
  await expect(page.locator(".approval-card-actions button")).toHaveCount(4);
  await setPaneWidth(page, 320);
  await page.setViewportSize({ width: page.viewportSize()!.width, height: 430 });
  const region = page.locator(".approval-cards");
  await expect.poll(async () => ({
    width: Math.round((await placement(page)).main.width),
    viewportHeight: page.viewportSize()!.height,
  }), { message: "premise: the 320px pane has only the 430px viewport available" })
    .toEqual({ width: 320, viewportHeight: 430 });
  await expect.poll(() => region.evaluate(node => node.getBoundingClientRect().bottom), {
    message: "the resized approval region has settled within the short viewport",
  }).toBeLessThanOrEqual(430);
  await expect.poll(async () => {
    const geometry = await cardGeometry(page);
    return geometry.buttonsInside.every(Boolean) || await region.getAttribute("data-tiny-pane") === "true";
  }, { message: "all answers fit, or insufficient sibling-adjusted card space declares the scrollable tiny-pane exception" }).toBe(true);

  if (await region.getAttribute("data-tiny-pane") === "true") {
    await expect.poll(() => region.evaluate(node => node.scrollHeight - node.clientHeight), {
      message: "the declared tiny-pane exception supplies real whole-region scrolling",
    }).toBeGreaterThan(1);
    for (const answer of await page.locator(".approval-card-actions button").all()) {
      await answer.evaluate(button => {
        const region = button.closest(".approval-cards")!;
        const buttonBounds = button.getBoundingClientRect();
        const regionBounds = region.getBoundingClientRect();
        // WebKit rounds scrollTop to device pixels. Move just inside the
        // edge rather than leaving a fractional button border clipped.
        region.scrollTop += Math.ceil(buttonBounds.bottom - regionBounds.bottom) + 2;
      });
      await expect(answer, "each answer can be fully revealed by the region's own scroll").toBeInViewport({ ratio: 1 });
    }
  } else {
    for (const answer of await page.locator(".approval-card-actions button").all()) {
      await expect(answer, "the ordinary pinned layout fully exposes each answer").toBeInViewport({ ratio: 1 });
    }
  }
  // The queue keeps its own scrollport even when the surrounding region must
  // also scroll. Reveal that list, then prove its final request remains usable.
  const waiting = page.locator(".approval-waiting-list");
  await region.evaluate(node => { node.scrollTop = node.scrollHeight; });
  await waiting.evaluate(node => { node.scrollTop = node.scrollHeight; });
  const last = page.locator('.approval-waiting[data-approval-id="boundary-waiting-11"]');
  await expect.poll(() => last.evaluate(button => {
    const bounds = button.getBoundingClientRect();
    const waiting = button.closest(".approval-waiting-list")!.getBoundingClientRect();
    const region = button.closest(".approval-cards")!.getBoundingClientRect();
    // At its scroll limit WebKit can round away a fraction of a border pixel.
    // Check every clipping boundary with the same 1px geometry tolerance as
    // the ordinary pinned tests, then actually select this request below.
    const inside = (box: { left: number; right: number; top: number; bottom: number }) =>
      bounds.left >= box.left - 1 && bounds.right <= box.right + 1
      && bounds.top >= box.top - 1 && bounds.bottom <= box.bottom + 1;
    return {
      queue: inside(waiting), region: inside(region),
      viewport: inside({ left: 0, right: innerWidth, top: 0, bottom: innerHeight }),
    };
  }), { message: "the last request fits its queue, region and viewport at the real scroll limits" })
    .toEqual({ queue: true, region: true, viewport: true });
  await expect(page.locator(".approval-waiting")).toHaveCount(queue.length);
  await last.click();
  await expect(page.locator(".approval-card")).toHaveAttribute("data-approval-id", "boundary-waiting-11");
  await expect(page.locator(".approval-late")).toContainText("did not take effect");
});

/** The shortened requester is a presentation only, never a clipboard target.
 * Spaces, bidi controls and literal escape-looking text must retain distinct
 * full spellings in the accessible identity while Copy sends the raw bytes.
 */
test("the compact From line keeps isolated full identity and raw Copy ID", async ({ page }) => {
  const bare = "begin \u202eopaque\\n-middle-identity-end";
  expect(new TextEncoder().encode(bare).length, "premise: raw requester ID fits the helm's admission bound").toBeLessThanOrEqual(40);
  const raw = `12345678-1234-1234-1234-123456789abc:${bare}`;
  const full = "12345678-1234-1234-1234-123456789abc:begin<U+0020><U+202E>opaque\\n-middle-identity-end";
  const fixture = launchRequest("requester-copy", "build\u202ebox", "a \"quoted\" session\u202e");
  fixture.session = { id: raw, title: "a \"quoted\" session\u202e", host_name: "build\u202ebox" };
  await stageApprovals(page, [fixture]);
  await openSelected(page);
  const from = page.locator(".approval-card-from");
  const id = from.locator('[aria-label^="From session ID:"]');
  await expect(from.locator('[aria-label="From host"]')).toHaveText("build<U+202E>box");
  await expect(from.locator('[aria-label="From session"]')).toHaveText('a "quoted" session<U+202E>');
  await expect(from.locator(".approval-from-title")).toHaveText('“a "quoted" session<U+202E>”');
  await expect(id).toHaveAttribute("data-tooltip", full);
  await expect(id).toHaveAttribute("aria-label", `From session ID: ${full}`);
  const short = (await id.textContent())!;
  expect(short).not.toContain("12345678-1234-1234-1234-123456789abc:");
  expect(short).toMatch(/^begin.*….*end$/);
  expect(short, "abbreviation cannot leave a partial visible escape").not.toMatch(/<[^>]*…|…[^<]*>/);
  expect(await from.locator(".peer-value").evaluateAll(values =>
    values.length === 3 && values.every(node => node.getAttribute("dir") === "ltr")
  ), "host, title and identity each remain isolated").toBe(true);
  await page.evaluate(() => {
    const copies: string[] = [];
    Object.assign(window, { __approvalCopies: copies });
    navigator.clipboard.writeText = async text => { copies.push(text); };
  });
  const copy = from.getByRole("button", { name: "Copy from session ID", exact: true });
  await expect(copy).toHaveText("Copy ID");
  await copy.click();
  await expect.poll(() => page.evaluate(() => Reflect.get(window, "__approvalCopies"))).toEqual([raw]);
});

/** The common launch fits above the prompt; sidebar changes and narrow widths still follow the pane. */
test("a compact card follows the main pane and keeps the answer buttons visible", async ({ page }, info) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  const staged = await stageApprovals(page, [launchRequest("layout")]);
  await openSelected(page);
  await expect.poll(async () => {
    const { main, card, terminal } = await placement(page);
    return Math.abs(card.x + card.width / 2 - main.x - main.width / 2) < 1
      && card.y >= terminal.y + 11 && card.x >= main.x + 11;
  }, { message: "card centered inside the terminal area and clear of the sidebar" }).toBe(true);
  const wide = await placement(page);
  expect(wide.card.width).toBeCloseTo(660, 0);
  expect(wide.card.height, "a request grows only as far as the available cap").toBeLessThanOrEqual(wide.region.height);
  expect(wide.actions.bottom, "answers visible without scrolling").toBeLessThanOrEqual(wide.region.bottom);
  expect(wide.region.bottom, "the terminal's bottom remains clear").toBeLessThan(wide.main.bottom - 100);
  await page.screenshot({ path: info.outputPath("approval-common.png") });

  // A band between chrome and terminal distinguishes the terminal anchor
  // from the now-higher tab strip. Only its height is synthetic: the
  // anchor and approval region remain the application's production nodes.
  await page.locator(".terminal-panes").evaluate((terminal) => {
    const band = document.createElement("div");
    band.dataset.approvalBandFixture = "true";
    band.style.cssText = "height: 64px; flex-shrink: 0";
    band.textContent = "conditional notice geometry fixture";
    terminal.before(band);
  });
  await expect.poll(async () => {
    const { card, terminal } = await placement(page);
    return Math.abs(card.y - terminal.y - 12);
  }, { message: "a conditional band moves the approval below itself with the terminal" }).toBeLessThan(1);
  await page.locator("[data-approval-band-fixture]").evaluate((band) => band.remove());
  await expect.poll(async () => {
    const { card, terminal } = await placement(page);
    return Math.abs(card.y - terminal.y - 12);
  }, { message: "removing a band restores the terminal anchor" }).toBeLessThan(1);

  // Change actual pane geometry, without depending on the separate sidebar
  // resizing feature. A hardcoded sidebar offset fails this observation.
  await page.locator(".app-sidebar").evaluate((node) => { (node as HTMLElement).style.width = "500px"; });
  await expect.poll(async () => {
    const { main, card } = await placement(page);
    return Math.abs(card.x + card.width / 2 - main.x - main.width / 2);
  }).toBeLessThan(1);
  await page.locator(".app-sidebar").evaluate((node) => { (node as HTMLElement).style.removeProperty("width"); });
  await page.setViewportSize({ width: 520, height: 800 });
  await page.locator(".app-shell").evaluate((node) => {
    // Header actions can extend the shell's scroll range beyond the pane.
    // Reveal the pane's left edge, not the far end of that overflow.
    node.scrollLeft = (node.querySelector(".app-main") as HTMLElement).offsetLeft;
  });
  await expect.poll(async () => {
    const { main, card } = await placement(page);
    return card.x >= Math.max(11, main.x + 11) && card.right <= main.right - 11 && card.width <= 296;
  }, { message: "320px pane contains the whole card" }).toBe(true);
  await expect(page.locator(".approval-card-facts")).toHaveCount(1);
  expect(await page.locator(".approval-cards").evaluate((node) => node.scrollWidth - node.clientWidth)).toBe(0);
  await page.screenshot({ path: info.outputPath("approval-narrow.png") });

  // A minimum-width pane moves when its sidebar grows, but does not resize.
  // Observing the pane's size alone would leave the overlay behind.
  const beforeResize = await placement(page);
  await page.locator(".app-sidebar").evaluate((node) => { (node as HTMLElement).style.width = "500px"; });
  await expect.poll(async () => {
    const { main, card } = await placement(page);
    return Math.abs(card.x + card.width / 2 - main.x - main.width / 2);
  }, { message: "sidebar resize moves the minimum-width pane and its overlay together" }).toBeLessThan(1);
  const afterResize = await placement(page);
  expect(afterResize.main.width).toBeCloseTo(beforeResize.main.width, 0);
  expect(afterResize.main.x - beforeResize.main.x).toBeGreaterThan(100);
  await page.locator(".app-sidebar").evaluate((node) => { (node as HTMLElement).style.removeProperty("width"); });
  await expect.poll(async () => {
    const { main, card } = await placement(page);
    return Math.abs(card.x + card.width / 2 - main.x - main.width / 2);
  }).toBeLessThan(1);

  // A flex row can wrap whole buttons while leaving a long host token wider
  // than the card. Both permanent-allow choices must contain the complete
  // escaped host token inside their buttons, even with four actions present.
  const longHost = `remote-${"x".repeat(80)}\u202e`;
  const shownHost = `remote-${"x".repeat(80)}<U+202E>`;
  await staged.replace([{ ...launchRequest("long-host", longHost), own_host_session_action: true }]);
  await expect(page.locator(".approval-card")).toHaveAttribute("data-approval-id", "long-host");
  await expect(page.locator(".approval-card-actions button")).toHaveCount(4);
  for (const selector of [".approval-always-allow-own-host", ".approval-always-allow"]) {
    const button = page.locator(selector);
    await expect(button).toContainText(shownHost);
    await expect(button.locator(".peer-value")).toHaveText(shownHost);
    await expect(button.locator(".peer-value")).toHaveAttribute("dir", "ltr");
    await expect.poll(() => button.evaluate((node) => node.scrollWidth - node.clientWidth)).toBe(0);
  }
  await expect(page.locator(".approval-always-allow-own-host")).toHaveText(`Always allow on ${shownHost} only`);
  await expect(page.locator(".approval-always-allow-own-host")).toHaveAttribute(
    "data-tooltip",
    `Always allow: let agents in ${shownHost}'s sessions start, stop, restart, rename and delete sessions on ${shownHost} without asking. Requests for other hosts still ask.`,
  );
  await expect.poll(() => page.locator(".approval-cards").evaluate((node) => node.scrollWidth - node.clientWidth)).toBe(0);
  await page.screenshot({ path: info.outputPath("approval-narrow-long-host.png") });
});

/**
 * The compact requester line truncates presentation without losing identity.
 * A narrow card must ellipsize the title rather than spending its answer
 * space on wrapped requester prose; the complete spelling remains in hover.
 */
test("a requester session title stays on one line with its full hover text", async ({ page }, info) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  const title = "migrate authentication tokens";
  await stageApprovals(page, [launchRequest("requester-width", "devbox", title)]);
  await openSelected(page);
  const value = page.locator('.approval-card-from [aria-label="From session"]');
  await expect(value).toHaveText(title);
  await expect(value).toHaveAttribute("data-tooltip", title);
  // The vendored UI font is a layout input; a fallback's smaller glyphs can
  // otherwise hide the cramped column before the real face finishes loading.
  await page.evaluate(() => document.fonts.ready);
  expect((await placement(page)).card.width).toBeCloseTo(660, 0);
  /** Count text lines, independent of the containing row's height or padding. */
  const lines = () => value.evaluate((node) => {
    const range = document.createRange();
    range.selectNodeContents(node);
    return new Set(Array.from(range.getClientRects(), (rect) => Math.round(rect.top))).size;
  });
  await page.screenshot({ path: info.outputPath("approval-requester-wide.png") });
  await expect.poll(lines, { message: "ordinary requester title fits one line at the usual card width" }).toBe(1);

  await page.setViewportSize({ width: 520, height: 800 });
  // The shell can scroll horizontally at this width; reveal the pane so the
  // narrow screenshot shows the same card whose text geometry is measured.
  await page.locator(".app-shell").evaluate((node) => {
    node.scrollLeft = (node.querySelector(".app-main") as HTMLElement).offsetLeft;
  });
  await expect.poll(async () => (await placement(page)).card.width).toBeLessThanOrEqual(296);
  await expect.poll(lines, { message: "a narrow requester title cannot grow the pinned header" }).toBe(1);
  await expect.poll(() => value.evaluate((node) => ({
    overflow: node.scrollWidth > node.clientWidth,
    ellipsis: getComputedStyle(node).textOverflow,
  })), { message: "the full title is retained behind a visible ellipsis" }).toEqual({ overflow: true, ellipsis: "ellipsis" });
  await expect.poll(() => page.locator(".approval-cards").evaluate((node) => node.scrollWidth - node.clientWidth)).toBe(0);
  await page.screenshot({ path: info.outputPath("approval-requester-narrow.png") });
});

/** The sibling region must follow both the empty pane and a newly mounted session view. */
test("placement follows the empty pane, session mount, and approval remount", async ({ page }) => {
  const staged = await stageApprovals(page, [launchRequest("empty-pane")]);
  await page.route(SESSION_LISTING, (route) => fulfillAsHelm(route, {
    status: 200,
    contentType: "application/json",
    body: JSON.stringify({ sessions: [], total: 0, matching: 0, truncated: false }),
  }));
  await page.goto("/");
  await expect(page.locator(".main-empty")).toHaveText("No sessions — create one");
  await expect(page.locator(".app-main .tab-strip")).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => {
    const main = document.querySelector(".app-main")!.getBoundingClientRect();
    const card = document.querySelector(".approval-card")!.getBoundingClientRect();
    return Math.abs(card.top - main.top - 12) < 1
      && Math.abs(card.x + card.width / 2 - main.x - main.width / 2) < 1;
  }), { message: "no-session card uses the actual pane top" }).toBe(true);

  await page.unroute(SESSION_LISTING);
  await staged.replace([launchRequest("empty-pane")]);
  await attachSession(page, session!.id);
  await expect(page.locator(".app-main .tab-strip")).toBeVisible();
  await expect.poll(async () => {
    const { card, terminal } = await placement(page);
    return Math.abs(card.y - terminal.y - 12);
  }, { message: "new session chrome replaces the no-session anchor" }).toBeLessThan(1);

  await staged.replace([]);
  await expect(page.locator(".approval-cards")).toHaveCount(0);
  await staged.replace([launchRequest("remounted")]);
  await expect(page.locator(".approval-card")).toHaveAttribute("data-approval-id", "remounted");
  await expect.poll(async () => {
    const { card, terminal } = await placement(page);
    return Math.abs(card.y - terminal.y - 12);
  }, { message: "remounted region owns a fresh measured anchor" }).toBeLessThan(1);
});

/**
 * A rebooted session has chrome and a restart explanation but no terminal
 * surface. Approval placement must use the top row's bottom in that state,
 * rather than the bottom details bar or the empty pane's top.
 */
test("a session without a terminal anchors approval below its top row", async ({ page }) => {
  const interrupted = { ...session!, status: { state: "interrupted" }, stale: false };
  await page.route(SESSION_LISTING, (route) => fulfillAsHelm(route, {
    json: { sessions: [interrupted], total: 1, matching: 1, truncated: false },
  }));
  await page.route(`**/api/sessions/${encodeURIComponent(session!.id)}`, (route) => fulfillAsHelm(route, { json: interrupted }));
  await stageApprovals(page, [launchRequest("no-terminal")]);
  await page.goto("/");
  const target = page.locator(`[data-session-id="${session!.id}"] .session-row-open`);
  await expect(target).toBeVisible();
  await target.click();
  await expect(page.locator(".session-details .status-badge")).toHaveText("Interrupted");
  await expect(page.locator(".terminal-panes")).toHaveCount(0);
  // The recovery card replaces the flexible terminal area. A full-pane
  // minimum height would leave the session identity below the window edge.
  const details = (await page.locator(".session-details").boundingBox())!;
  const recovery = (await page.locator(".interrupted-card").boundingBox())!;
  expect(details.y).toBeGreaterThanOrEqual(recovery.y + recovery.height - 1);
  expect(details.y + details.height).toBeLessThanOrEqual(page.viewportSize()!.height + 1);
  await expect(page.locator(".approval-card .approval-allow")).toBeEnabled();
  await expect.poll(() => page.evaluate(() => {
    const top = document.querySelector(".session-topbar")!.getBoundingClientRect();
    const card = document.querySelector(".approval-card")!.getBoundingClientRect();
    return Math.abs(card.top - top.bottom - 12);
  }), { message: "no-terminal approval uses the top row fallback" }).toBeLessThan(1);
});

/** Very short windows must still expose a scrollable route to the request and its answer. */
test("a short viewport keeps approval content and answers reachable", async ({ page }, info) => {
  const staged = await stageApprovals(page, [launchRequest("short-viewport")]);
  await openSelected(page);
  // At 48px the real 30px top row and placement margins exhausted the old
  // region. A positive height alone would miss an answer still outside it.
  await page.setViewportSize({ width: 1280, height: 48 });
  const region = page.locator(".approval-cards");
  await expect.poll(() => region.evaluate((node) => node.getBoundingClientRect().height), {
    message: "a short window must not collapse waiting requests to zero height",
  }).toBeGreaterThan(0);
  await expect.poll(() => region.evaluate((node) => {
    const rect = node.getBoundingClientRect();
    return rect.top >= 0 && rect.bottom <= innerHeight;
  })).toBe(true);
  await region.hover();
  await page.mouse.wheel(0, 2000);
  await expect.poll(() => region.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
  const deny = page.locator(".approval-card .approval-deny");
  await expect(deny).toBeInViewport();
  await page.screenshot({ path: info.outputPath("approval-short-answer.png") });
  await deny.click();
  await expect(page.locator(".approval-card")).toHaveCount(0);
  expect(staged.answers).toEqual([{ id: "short-viewport", answer: "deny" }]);
});

/** A stale delay must not arm a different request after rapid switches across hosts. */
test("one request stays selected across refreshes and rapid switches rearm the buttons", async ({ page }) => {
  await page.clock.install();
  const staged = await stageApprovals(page, [launchRequest("first"), launchRequest("second", "laptop"), launchRequest("third")]);
  await openSelected(page);
  const card = page.locator(".approval-card");
  await expect(card).toHaveCount(1);
  await expect(card).toHaveAttribute("data-approval-id", "first");
  await expect(page.locator(".approval-waiting")).toHaveCount(2);
  await expect(card.locator(".approval-card-count")).toHaveText("1 of 3 waiting");
  await page.clock.pauseAt(new Date(Date.now() + 5_000));
  await page.locator('.approval-waiting[data-approval-id="second"]').click();
  await expect(card).toHaveAttribute("data-approval-id", "second");
  await expect(card.locator(".approval-allow")).toBeDisabled();
  await page.clock.runFor(400);
  await page.locator('.approval-waiting[data-approval-id="first"]').click();
  await expect(card).toHaveAttribute("data-approval-id", "first");
  await page.clock.runFor(300);
  await expect(card.locator(".approval-allow"), "the previous card's timer cannot arm this card").toBeDisabled();
  await page.clock.runFor(400);
  await expect(card.locator(".approval-allow")).toBeEnabled();
  const second = page.locator('.approval-waiting[data-approval-id="second"]');
  await second.focus();
  await expect(second).toBeFocused();
  await second.press("Enter");
  await expect(card).toHaveAttribute("data-approval-id", "second");
  await page.clock.runFor(700);
  await staged.expire("third");
  await expect(page.locator(".approval-waiting")).toHaveCount(1);
  await expect(card).toHaveAttribute("data-approval-id", "second");
  await page.clock.runFor(700);
  await expect(card.locator(".approval-deny")).toBeEnabled();
  await card.locator(".approval-deny").click();
  await expect(card).toHaveAttribute("data-approval-id", "first");
  expect(staged.answers).toEqual([{ id: "second", answer: "deny" }]);
  await page.clock.runFor(700);
  await staged.expire("first");
  await expect(page.locator(".approval-cards")).toHaveCount(0);
});

/** A dialog cannot make an agent's visible approval request unanswerable. */
test("approval remains answerable while the quick switcher is open", async ({ page }) => {
  const staged = await stageApprovals(page, [launchRequest("dialog")]);
  await openSelected(page);
  const trigger = page.locator(".quick-switcher-trigger");
  await expect(trigger).toHaveCount(1);
  const terminal = terminalInput(page);
  await terminal.focus();
  await expect(terminal).toBeFocused();
  const mac = await page.evaluate(() => navigator.platform.includes("Mac"));
  await page.keyboard.press(mac ? "Meta+k" : "Control+Shift+k");
  const dialog = page.getByRole("dialog", { name: "Quick switcher" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("combobox")).toBeFocused();
  await expect(page.locator(".app-main")).toHaveAttribute("inert", "");
  await page.locator(".approval-card .approval-allow").click();
  await expect(page.locator(".approval-card")).toHaveCount(0);
  await expect(dialog).toBeVisible();
  expect(staged.answers).toEqual([{ id: "dialog", answer: "allow" }]);
});

/** A link's classification and permission variant are supplied by the helm, never inferred by the UI. */
function linkRequest(id: string, url: string, preapproved = false, localNetwork = false): Approval {
  return { ...launchRequest(id), action: { kind: "open_url", url, preapproved, local_network: localNetwork } };
}

/** Record user activation at the real synchronous browser call, without opening an external site. */
async function installLinkOpenProbe(page: Page) {
  await page.addInitScript(() => {
    const opened: Array<{ url: string; active: boolean }> = [];
    Object.assign(window, { __farhelmOpenedLinks: opened });
    window.open = (url) => {
      opened.push({ url: String(url), active: navigator.userActivation.isActive });
      return null;
    };
  });
}

/** Read the bounded observations owned by this page's opener stand-in. */
async function openedLinks(page: Page) {
  return page.evaluate(() =>
    (window as unknown as { __farhelmOpenedLinks: Array<{ url: string; active: boolean }> }).__farhelmOpenedLinks
  );
}

/** Allow must open inside the click, after answer admission, while full peer URL text stays escaped. */
test("a link approval opens synchronously with user activation and displays the entire URL as data", async ({ page }) => {
  const url = "https://example.test/path?note=<b>&next=%2Faccount#section";
  await installLinkOpenProbe(page);
  const staged = await stageApprovals(page, [linkRequest("link-allow", url)]);
  await openSelected(page);
  const card = page.locator(".approval-card");
  await expect(card).toContainText(url);
  await expect(card.locator(".approval-card-facts b"), "URL markup must remain text").toHaveCount(0);
  await card.getByRole("button", { name: "Allow", exact: true }).click();
  await expect(card).toHaveCount(0);
  expect(staged.answers).toEqual([{ id: "link-allow", answer: "allow" }]);
  expect(await openedLinks(page)).toEqual([{ url, active: true }]);
});

for (const preapproved of [false, true]) {
  /** Deny and the preapproved variant's Dismiss settle the request without reaching the opener. */
  test(`a ${preapproved ? "preapproved link dismiss" : "link denial"} opens nothing`, async ({ page }) => {
    await installLinkOpenProbe(page);
    const staged = await stageApprovals(page, [linkRequest("link-deny", "http://127.0.0.1:8765/", preapproved, true)]);
    await openSelected(page);
    const card = page.locator(".approval-card");
    if (preapproved) {
      await expect(card.getByRole("button", { name: "Open", exact: true })).toBeEnabled();
      await expect(card.locator(".approval-always-allow")).toHaveCount(0);
    } else {
      await expect(card).toContainText("It needs its own host permission, even when public links are allowed.");
    }
    await card.getByRole("button", { name: preapproved ? "Dismiss" : "Deny", exact: true }).click();
    await expect(card).toHaveCount(0);
    expect(staged.answers).toEqual([{ id: "link-deny", answer: "deny" }]);
    expect(await openedLinks(page)).toEqual([]);
  });
}

/** A host grant still needs a web click; a double click may not open a second request now under the pointer. */
test("a preapproved link opens on click and a double click cannot open the next link", async ({ page }) => {
  await installLinkOpenProbe(page);
  const first = "https://example.test/first";
  const staged = await stageApprovals(page, [linkRequest("link-first", first, true), linkRequest("link-second", "https://example.test/second", true)]);
  await openSelected(page);
  await page.locator(".approval-card .approval-allow").dblclick();
  await expect(page.locator(".approval-card")).toHaveAttribute("data-approval-id", "link-second");
  expect(staged.answers).toEqual([{ id: "link-first", answer: "allow" }]);
  expect(await openedLinks(page)).toEqual([{ url: first, active: true }]);
  await page.locator(".approval-card .approval-deny").click();
});

/** Recheck immediately before opening even if a malformed card somehow bypassed the helm's validator. */
test("an invalid URL on a card is neither opened nor answered as allowed", async ({ page }) => {
  await installLinkOpenProbe(page);
  const staged = await stageApprovals(page, [linkRequest("link-invalid", "javascript:alert(1)")]);
  await openSelected(page);
  await page.locator(".approval-card .approval-allow").click();
  await expect(page.locator(".approval-card [role=alert]")).toContainText("could not be opened safely");
  expect(staged.answers).toEqual([]);
  expect(await openedLinks(page)).toEqual([]);
  await page.locator(".approval-card .approval-deny").click();
});

/**
 * Prompt approval must preserve structural blank lines without inserting the
 * single-value renderer's labels. Unsafe characters still have visible escapes.
 * The card must also keep the target's shield, attachment notice, and answer
 * controls reachable while showing a long message as peer data.
 */
test("a running prompt card keeps its full message and target facts readable", async ({ page }, info) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  const prompt = `\n<b>agent text, not an instruction from Farhelm</b>\n\nunsafe:\u202e\n \t \n${"line of message data\n".repeat(80)}END-OF-PROMPT\n`;
  const staged = await stageApprovals(page, [{
    ...launchRequest("running-prompt"),
    action: {
      kind: "prompt",
      target: { id: "target-id", title: "target session", host_name: "buildbox" },
      launch: {
        kind: "agent",
        selection: { harness: "claude", permissions: "guarded" },
        start: ["claude", "--permission-mode", "auto", "{farhelm_args}"],
        resume: null,
      },
      screen_yolo: true,
      attached: true,
      prompt,
    },
  }]);
  await openSelected(page);
  const card = page.locator('.approval-card[data-approval-kind="prompt"]');
  const message = card.locator(".approval-card-prompt");
  await expect(message).toContainText("END-OF-PROMPT");
  expect(await message.textContent(), "blank lines remain while unsafe characters are escaped")
    .toBe(prompt.replace("\u202e", "<U+202E>").replace("\t", "<U+0009>"));
  expect(await message.locator(".peer-line").evaluateAll((lines) =>
    lines.length > 0 && lines.every((line) => line.getAttribute("dir") === "ltr")
  ), "each message line keeps its direction isolated").toBe(true);
  await expect(message.locator("b"), "agent markup stays literal text").toHaveCount(0);
  await expect(card).toContainText("target session");
  await expect(card).toContainText("buildbox");
  await expect(card).toContainText("session is open in a client");
  // Claude has an observed bypass footer; Codex deliberately has no such detector.
  await expect(card.locator('.permission-glyph[data-glyph="yolo"]')).toBeVisible();
  await expect(message).toHaveAttribute("aria-label", "Prompt");
  await expect.poll(() => message.evaluate((node) => node.scrollHeight > node.clientHeight), {
    message: "the complete message stays available in its bounded scrolling block",
  }).toBe(true);
  await expect.poll(() => page.locator(".approval-cards").evaluate((node) => node.scrollWidth - node.clientWidth))
    .toBe(0);
  await expect.poll(async () => {
    const { main, card, actions } = await placement(page);
    return card.x >= main.x && card.right <= 1280 && actions.bottom <= 900;
  }, { message: "the card and its answer controls fit in the visible session pane" }).toBe(true);
  await page.screenshot({ path: info.outputPath("approval-running-prompt.png") });
  await card.getByRole("button", { name: "Allow", exact: true }).click();
  await expect(card).toHaveCount(0);
  expect(staged.answers).toEqual([{ id: "running-prompt", answer: "allow" }]);
});

/** Read semantic requester roles or a labelled fact without confusing their hosts.
 * The From line uses accessible roles rather than fact rows; callers retain
 * the same vocabulary when comparing a requester with a changed host.
 */
function approvalValue(page: Page, label: string) {
  // Callers name semantic facts; the rendered label also preserves ID's acronym.
  const displayLabel = label.replace(/\bid\b/g, "ID").replace(/^[a-z]/, letter => letter.toUpperCase());
  if (displayLabel.startsWith("From ")) {
    return page.locator(`.approval-card-from [aria-label${displayLabel === "From session ID" ? "^" : ""}="${displayLabel}${displayLabel === "From session ID" ? ":" : ""}"]`);
  }
  return page.locator(".approval-card .approval-card-row")
    .filter({ has: page.locator("dt").filter({ hasText: new RegExp(`^${displayLabel}$`) }) }).locator("dd");
}

/** Host edits need the complete before/after decision, with every peer value kept outside UI prose. */
test("a host edit card shows changed fields as escaped data and answers only this request", async ({ page }) => {
  const requester = "<b>requester</b>\u202e";
  const title = "<b>asking session</b>\t";
  const target = "<em>builder</em>\u202e";
  const destination = "operator@<b>builder</b>\u001b";
  const staged = await stageApprovals(page, [{
    ...launchRequest("host-edit", requester, title),
    action: {
      kind: "host_edit",
      host_name: target,
      destination,
      alias: { old: "<b>old alias</b>\t", new: "<b>new alias</b>\u202e" },
      icon: { old: "cloud", new: "rocket" },
      color: { old: "default", new: "teal" },
    },
  }]);
  await openSelected(page);
  const card = page.locator('.approval-card[data-approval-kind="host_edit"]');
  await expect(card.getByRole("heading")).toHaveText("Change a host's alias, icon or color");
  for (const [label, text] of [
    ["from host", "<b>requester</b><U+202E>"],
    ["from session", "<b>asking session</b><U+0009>"],
    ["host", "<em>builder</em><U+202E>"],
    ["destination", "operator@<b>builder</b><U+001B>"],
    ["alias now", "<b>old alias</b><U+0009>"],
    ["new alias", "<b>new alias</b><U+202E>"],
    ["icon now", "Cloud"],
    ["new icon", "Rocket"],
    ["color now", "Default"],
    ["new color", "Teal"],
  ]) {
    await expect(approvalValue(page, label)).toHaveText(text);
  }
  // The compact ID keeps its full identity in the accessible spelling.
  await expect(approvalValue(page, "from session id"))
    .toHaveAttribute("data-tooltip", "7c1e9a52-4b0d-4f7e-9a3c-2d81f6b0e413");
  await expect(card.locator("b, em"), "peer markup cannot create elements").toHaveCount(0);
  expect(await card.locator(".peer-value").evaluateAll((values) =>
    values.length >= 7 && values.every((value) => value.getAttribute("dir") === "ltr")
  ), "requester and changed host values retain direction isolation").toBe(true);
  await expect(card).toContainText(
    "Each host change needs your approval, even when the requesting host can run Farhelm commands without asking.",
  );
  await expect(card.locator(".approval-always-allow")).toHaveCount(0);
  await expect(card.locator(".approval-card-actions button")).toHaveText(["Allow", "Deny"]);
  expect(staged.answers, "showing the decision is not approval").toEqual([]);
  await card.getByRole("button", { name: "Allow", exact: true }).click();
  await expect(card).toHaveCount(0);
  expect(staged.answers).toEqual([{ id: "host-edit", answer: "allow" }]);
});

for (const answer of ["allow", "deny"] as const) {
  /** Forgetting leaves sessions running; either answer must settle only the request whose consequence was shown. */
  test(`a host remove card explains the consequence and sends ${answer}`, async ({ page }) => {
    const staged = await stageApprovals(page, [{
      ...launchRequest("host-remove", "requesting host", "asking session"),
      action: {
        kind: "host_remove",
        host_name: "<b>target host</b>\u202e",
        destination: "operator@<em>target</em>\t",
      },
    }]);
    await openSelected(page);
    const card = page.locator('.approval-card[data-approval-kind="host_remove"]');
    await expect(card.getByRole("heading")).toHaveText("Remove a host");
    await expect(approvalValue(page, "from host")).toHaveText("requesting host");
    await expect(approvalValue(page, "from session")).toHaveText("asking session");
    await expect(approvalValue(page, "host")).toHaveText("<b>target host</b><U+202E>");
    await expect(approvalValue(page, "destination")).toHaveText("operator@<em>target</em><U+0009>");
    for (const label of ["host", "destination"]) {
      await expect(approvalValue(page, label).locator(".peer-value")).toHaveAttribute("dir", "ltr");
    }
    await expect(card.locator("b, em"), "peer markup cannot create elements").toHaveCount(0);
    await expect(card.locator("p.approval-card-note")).toHaveText([
      "Farhelm will forget this host. Its supervisor and sessions keep running, and adding the destination again finds them.",
      "Each host change needs your approval, even when the requesting host can run Farhelm commands without asking.",
    ]);
    await expect(card.locator(".approval-always-allow")).toHaveCount(0);
    await expect(card.locator(".approval-card-actions button")).toHaveText(["Allow", "Deny"]);
    expect(staged.answers, "showing the consequence is not approval").toEqual([]);
    await card.getByRole("button", { name: answer === "allow" ? "Allow" : "Deny", exact: true }).click();
    await expect(card).toHaveCount(0);
    expect(staged.answers).toEqual([{ id: "host-remove", answer }]);
  });
}
