/**
 * A held launch reply is visibly pending, and a client deadline says nothing
 * about whether the helm accepted it. Drive the shipped timers with Clock;
 * aborting a route would exercise a different transport failure.
 */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Page } from "@playwright/test";
import { fillCreateForm } from "./helpers/term";
import { cleanupSession, createSession, listHosts, localHostId, patchPreferences, pinAutoSelect, readPreferences } from "./helpers/fleet";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { cleanUpSessionsTitled, fulfillAsHelm, installTerminalSuiteHooks } from "./helpers/terminal-suite";
import { routeGate } from "./helpers/route-gate";
import { drainedRoute } from "./helpers/drained-route";

installTerminalSuiteHooks();

const HOST = "launch-status-host";

/** Keep real connection authority while giving the captured host a stable name. */
async function nameLocalHost(page: Page, request: APIRequestContext) {
  const local = await localHostId(request);
  await page.route("**/api/hosts", async route => {
    const response = await route.fetch();
    const body = await response.json();
    const host = body.hosts.find((host: { id: number }) => host.id === local);
    expect(host, "the status fixture uses the stack's actual local connection").toBeTruthy();
    host.name = HOST;
    await route.fulfill({ response, json: body });
  });
  return local;
}

/** Freeze after automatic form observations finish, before any launch timer exists. */
async function freezeLauncher(page: Page) {
  const form = page.locator(".create-session-form");
  await expect(form).toHaveAttribute("data-folder-check-pending", "false");
  await page.clock.pauseAt(new Date(Date.now() + 5_000));
  return form;
}

/** The request is held until each timing assertion has observed its boundary.
 * A server refusal whose text mentions timeouts must keep the refusal prefix;
 * neither that text nor the unresolved retry outcome is deadline evidence. */
test("launcher status waits two seconds, counts up and ends on a real refusal", async ({ page, request }, testInfo) => {
  await page.clock.install();
  const local = await nameLocalHost(page, request);
  const gate = routeGate();
  let calls = 0;
  const stop = await drainedRoute(page, "**/api/sessions", async route => {
    if (route.request().method() !== "POST") return route.continue();
    expect(route.request().postDataJSON().host).toBe(local);
    calls++;
    await gate.wait();
    await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "The host reported a request timeout" });
  });
  try {
    await page.goto("/");
    await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title: "slow-refusal" });
    const form = await freezeLauncher(page);
    await form.locator(".create-session-submit").click();
    await expect.poll(() => calls, { message: "the create is captured and its reply remains held" }).toBe(1);
    await expect(form).toHaveCount(1);
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    const status = page.locator(".launch-progress-pill .launch-progress");
    await page.clock.runFor(1_900);
    await expect(status).toHaveCount(0);
    await page.clock.runFor(300);
    await expect(status).toContainText(`Launching “slow-refusal” on ${HOST}`);
    await expect(status).toContainText("2 s");
    await expect(status.locator(".delete-spinner")).toBeVisible();
    const announcement = status.getByRole("status");
    const once = await announcement.textContent();
    await page.clock.runFor(1_000);
    await expect(status).toContainText("3 s");
    await expect(announcement).toHaveText(once!);
    await expect(form.locator(".launch-composer-cancel")).toBeDisabled();
    await page.setViewportSize({ width: 390, height: 500 });
    await testInfo.attach("launch-progress-phone", { body: await page.screenshot(), contentType: "image/png" });
    gate.release();
    await expect(form).toBeVisible();
    await expect(form).toHaveAttribute("aria-modal", "true");
    await expect(form.getByRole("alert")).toContainText("Not launched. The host reported a request timeout");
    await expect(form.getByLabel("Name (optional)")).toHaveValue("slow-refusal");
    // The returned DOM precedes its queued focus/isolation paint. Drive that
    // frame explicitly because the request-timing clock is still paused.
    await page.clock.runFor(20);
    await expect.poll(() => form.evaluate(node => node.contains(document.activeElement)),
      { message: "a refused launch restores focus to its modal editor" }).toBe(true);
    await expect(status).toHaveCount(0);
  } finally {
    gate.release();
    await stop();
  }
});

/** A fast reply must not flash a spinner or leave an elapsed counter behind. */
test("a launch answered below the threshold keeps the ordinary refusal", async ({ page, request }) => {
  await page.clock.install();
  await nameLocalHost(page, request);
  await page.route("**/api/sessions", async route => {
    if (route.request().method() !== "POST") return route.continue();
    await fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: "fixture launch refused" });
  });
  await page.goto("/");
  await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title: "fast-refusal" });
  const form = await freezeLauncher(page);
  await form.locator(".create-session-submit").click();
  await expect(form.getByRole("alert")).toContainText("Not launched. fixture launch refused");
  await page.clock.runFor(3_000);
  await expect(page.locator(".launch-progress")).toHaveCount(0);
});

/** The first reply is known accepted before it is held past the actual 60 s
 * deadline. An unchanged retry must carry the same key and reach that one
 * session, proving the alert's promise against a successful late outcome.
 * Switching to another install while it waits must preserve the implicit
 * original destination and every fingerprint input through the retry. */
test("deadline expiry reports an unknown outcome and keeps the accepted create key", async ({ page, request }, testInfo) => {
  await page.clock.install();
  const local = await nameLocalHost(page, request);
  const title = `deadline-create-${testInfo.project.name}-${Date.now()}`;
  const gate = routeGate();
  const keys: string[] = [];
  const bodies: any[] = [];
  const owned: string[] = [];
  let accepted = false;
  const stop = await drainedRoute(page, "**/api/sessions", async route => {
    if (route.request().method() !== "POST") return route.continue();
    const body = route.request().postDataJSON();
    bodies.push(body);
    keys.push(body.intent_key);
    const response = await route.fetch();
    expect(response.ok(), await response.text()).toBe(true);
    if (keys.length === 1) {
      accepted = true;
      await gate.wait();
    }
    await route.fulfill({ response });
  });
  try {
    const hosts = await listHosts(request);
    const origin = hosts.find(host => host.id === local);
    const remote = hosts.find(host => host.kind === "ssh" && host.state.phase === "connected" && host.id !== local);
    expect(origin?.identity, "the implicit launch origin has a recorded installation identity").toBeTruthy();
    expect(origin?.state.phase).toBe("connected");
    expect(remote?.identity, "the switch target belongs to the stack's connected remote install").toBeTruthy();
    expect(remote!.identity).not.toBe(origin!.identity);
    const sessionsResponse = await request.get("/api/sessions");
    expect(sessionsResponse.ok()).toBe(true);
    const sessions = await sessionsResponse.json() as { sessions: Array<{ id: string; host: number }> };
    const source = sessions.sessions.find(row => row.host === local);
    expect(source, "the owned stack supplies a local startup session for the implicit New target").toBeDefined();
    const other = await createSession(request, { title: `${title}-other-host`, host: remote!.id });
    owned.push(other.id);
    await pinAutoSelect(page, source!.id);
    await page.goto("/");
    await expect(page.locator(`[data-session-id="${source!.id}"]`)).toHaveAttribute("data-session-selected", "true");
    await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title });
    const form = await freezeLauncher(page);
    await form.locator(".create-session-submit").click();
    await expect.poll(() => accepted, { message: "the helm accepted the create; only its GUI reply is held" }).toBe(true);
    expect(bodies[0]).toMatchObject({ host: local, expected_incarnation: origin!.incarnation });
    // Selection is deliberately observed before B's terminal is ready: the
    // deadline must survive the source view's teardown, not wait for a new
    // attachment. The navigation/input spec covers B's live terminal too.
    const otherRow = page.locator(`[data-session-id="${other.id}"]`);
    await otherRow.locator(".session-row-open").click();
    await expect(otherRow).toHaveAttribute("data-session-selected", "true");
    await page.clock.fastForward(61_000);
    await expect(form).toBeVisible();
    await expect(form).toHaveAttribute("aria-modal", "true");
    await expect(form.getByLabel("Name (optional)")).toHaveValue(title);
    await expect(form.getByLabel("Folder", { exact: true })).toHaveValue("/tmp");
    await expect(form.getByLabel("Agent command")).toHaveValue("true");
    await expect(form.getByRole("combobox", { name: "Host", exact: true })).toContainText(HOST);
    await expect(otherRow).toHaveAttribute("data-session-selected", "true");
    const alert = form.getByRole("alert");
    await expect(alert).toContainText(`Still waiting on ${HOST}.`);
    await expect(alert).toContainText("The session may still start; it will appear in the sidebar if it does.");
    await expect(alert).toContainText("Launching again with the same settings will not start a second copy.");
    await expect(alert).not.toContainText("Not launched.");
    // Expiring the request and rendering its alert do not run the reveal's
    // next paint while Clock is paused; focus is observable after that frame.
    await page.clock.runFor(20);
    await expect.poll(() => form.evaluate(node => node.contains(document.activeElement)),
      { message: "an unknown outcome returns keyboard ownership to the retained draft" }).toBe(true);
    await expect(page.locator(".launch-progress")).toHaveCount(0);
    gate.release();
    await stop();
    // Retire the cancelled interception before retrying through a fresh one.
    await page.route("**/api/sessions", async route => {
      if (route.request().method() === "POST") {
        const body = route.request().postDataJSON();
        bodies.push(body);
        keys.push(body.intent_key);
      }
      await route.continue();
    });
    await form.locator(".create-session-submit").click();
    await expect(form).toHaveCount(0);
    expect(keys).toHaveLength(2);
    expect(keys[0]).toBeTruthy();
    expect(keys[1]).toBe(keys[0]);
    expect(bodies).toHaveLength(2);
    expect(bodies[1]).toEqual(bodies[0]);
    const listing = await request.get("/api/sessions");
    expect(listing.ok()).toBe(true);
    expect((await listing.json()).sessions.filter((row: { title: string }) => row.title === title)).toHaveLength(1);
  } finally {
    gate.release();
    await stop();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ["accepted create", () => cleanUpSessionsTitled(request, title)],
      ...owned.map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** Successful headers do not mean the reply has been read. Reqwest's browser
 * body decoder loses the timeout type, so exercise that boundary separately
 * from a held fetch. The native fetch still creates a real session; only the
 * body returned to the GUI is held in a stream that rejects on its abort. */
test("a deadline after successful headers still reports an unknown launch outcome", async ({ page, request }, testInfo) => {
  await page.clock.install();
  await nameLocalHost(page, request);
  const title = `deadline-body-${testInfo.project.name}-${Date.now()}`;
  try {
    await page.goto("/");
    await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title });
    const form = await freezeLauncher(page);
    await page.evaluate(() => {
      const fetch = window.fetch.bind(window);
      window.fetch = async (input, init) => {
        const request = new Request(input, init);
        const response = await fetch(request);
        if (request.method !== "POST" || new URL(request.url).pathname !== "/api/sessions") return response;
        if (!response.ok) throw new Error(`body fixture create refused: ${response.status}`);
        // Drain the real reply, then hand the client its success headers with
        // an unfinished body. The same AbortSignal owns that pending read.
        await response.arrayBuffer();
        const body = new ReadableStream({ start(controller) {
          request.signal.addEventListener("abort", () => controller.error(request.signal.reason), { once: true });
        } });
        document.documentElement.dataset.launchBodyHeaders = "held";
        const held = new Response(body, { status: response.status, headers: response.headers });
        // Reqwest validates the response URL before decoding. A constructed
        // Response has none, so retain the native reply's actual URL too.
        Object.defineProperty(held, "url", { value: response.url });
        return held;
      };
    });
    await form.locator(".create-session-submit").click();
    await expect(page.locator("html"), "successful headers have been handed to the GUI, but its body is still held")
      .toHaveAttribute("data-launch-body-headers", "held");
    await page.clock.runFor(2_200);
    await expect(form).toBeHidden();
    await expect(page.locator(".launch-progress-pill")).toContainText(HOST);
    await expect(form.getByRole("alert")).toHaveCount(0);
    await page.clock.runFor(57_800);
    await expect(form).toBeVisible();
    await expect(form).toHaveAttribute("aria-modal", "true");
    const alert = form.getByRole("alert");
    await expect(alert).toContainText(`Still waiting on ${HOST}.`);
    await expect(alert).not.toContainText("Not launched.");
    await expect(form.locator(".launch-progress")).toHaveCount(0);
  } finally {
    await cleanUpSessionsTitled(request, title);
  }
});

for (const numbered of [false, true]) {
  /** A dispatched quick launch releases page isolation while retaining its
   * named progress. A refusal restores the same editor as a modal, so a hidden
   * origin does not lose either its request or the explanation of its result. */
  test(`${numbered ? "template shortcut" : "quick launch"} shows a named pending pill`, async ({ page, request }, testInfo) => {
    await page.clock.install();
    await nameLocalHost(page, request);
    const name = `progress-template-${testInfo.project.name}-${Date.now()}`;
    const previous = await readPreferences(request);
    const gate = routeGate();
    let calls = 0;
    const stop = await drainedRoute(page, "**/api/sessions", async route => {
      if (route.request().method() !== "POST") return route.continue();
      expect(route.request().postDataJSON().title).toBe(name);
      calls++;
      await gate.wait();
      await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "hidden launch refused" });
    });
    try {
      const saved = await request.put(`/api/templates/${name}`, { data: {
        kind: "command", command: "true", yolo: false, name, destination: { folder: "/tmp" },
      } });
      expect(saved.ok(), await saved.text()).toBe(true);
      await patchPreferences(request, numbered ? { template_shortcuts: { "1": name } } : { quick_launch_template: name });
      const stored = await readPreferences(request);
      expect(numbered ? stored.template_shortcuts?.["1"] : stored.quick_launch_template).toBe(name);
      await page.goto("/");
      await expect(page.locator(".quick-launch-trigger")).toHaveCount(1);
      await page.clock.pauseAt(new Date(Date.now() + 5_000));
      await page.keyboard.press(numbered ? "Control+Shift+Digit1" : "Control+Shift+p");
      await expect.poll(() => calls, { message: "the hidden create is held after dispatch" }).toBe(1);
      await expect(page.locator(".create-session-form")).toBeHidden();
      await expect(page.locator(".create-session-form")).not.toHaveAttribute("aria-modal", "true");
      await page.keyboard.press("Control+Shift+k");
      const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
      await expect(switcher).toBeVisible();
      await switcher.getByRole("combobox").press("Escape");
      await expect(switcher).toHaveCount(0);
      const pill = page.locator(".launch-progress-pill");
      await page.clock.runFor(1_900);
      await expect(pill.locator(".launch-progress")).toHaveCount(0);
      await page.clock.runFor(300);
      await expect(pill).toBeVisible();
      await expect(pill).toContainText(name);
      await expect(pill).toContainText(HOST);
      await expect(pill).toContainText("2 s");
      await expect(pill.getByRole("status")).toHaveCount(1);
      expect(await pill.evaluate(node => node.closest("[inert]") === null),
        "the display-only pill must remain in the accessibility tree outside its hidden modal").toBe(true);
      gate.release();
      await expect(page.locator(".create-session-form")).toBeVisible();
      await expect(page.locator(".create-session-form")).toHaveAttribute("aria-modal", "true");
      await expect(page.locator(".create-session-form").getByRole("alert")).toContainText("Not launched. hidden launch refused");
      await expect(pill).toHaveCount(0);
    } finally {
      gate.release();
      await stop();
      await patchPreferences(request, { quick_launch_template: previous.quick_launch_template ?? null, template_shortcuts: previous.template_shortcuts ?? {} });
      const removed = await request.delete(`/api/templates/${name}`);
      expect(removed.ok() || removed.status() === 404).toBe(true);
    }
  });
}
