// The GUI half of the YOLO guard (SPEC.md, Sessions → Creation): a YOLO launch on a host
// that asks before YOLO launches is refused by the helm, the create form answers the
// refusal with a confirmation instead of an error, cancelling starts nothing, and
// confirming retries the same launch with the override. A host set to allow YOLO without
// asking launches without asking.
//
// The helm's own tests pin the refusal and the override on the wire. What
// only a browser can show is that the page reads the refusal as a question
// (from the helm's header, not from the text) and that the confirmed retry
// really carries the override the helm reads; a UI that dropped it would
// loop on the confirmation forever, and one that skipped the question would
// defeat the guard.
import { type APIRequestContext, type Locator, type Page, type Route } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { chooseLauncherHost } from "./helpers/host-picker";
import {
  cleanupSession,
  createResumableYoloSession,
  listHosts,
  listSessions,
  localHostId,
  openRowMenu,
  patchPreferences,
  readPreferences,
  setLocalYoloWithoutAsking,
  stopSession,
} from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";
import { requireHelmBuild } from "./helpers/helm-build";
import { routeGate } from "./helpers/route-gate";

// Two tests here turn session confirmations off. The preference is helm-wide
// and later specs share this helm, so reset it even when a test body is
// abandoned before its own `finally` restores it. Null is the default.
test.afterEach(async ({ request }) => {
  await patchPreferences(request, { skip_session_confirmations: null });
});

/** Establish an explicit Codex YOLO draft so the helm's refusal has a known reason. */
async function openYoloLaunch(page: Page, cwd: string): Promise<Locator> {
  const form = page.locator(".create-session-form");
  await page.locator(".new-session-button").click();
  await expect(form).toBeVisible();
  await form.getByLabel("folder", { exact: true }).fill(cwd);
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  return form;
}

test("a YOLO launch asks first and starts only when confirmed", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const cwd = stackScratchDir("yolo-guard-");
  const created: string[] = [];
  try {
    // The session opt-out cannot supply the helm's separate YOLO consent.
    await patchPreferences(request, { skip_session_confirmations: true });
    // Premise, made true rather than assumed: the suite shares one helm,
    // and a spec that marks the host safe puts it back, but say so here.
    await setLocalYoloWithoutAsking(request, false);
    await page.goto("/");
    const form = page.locator(".create-session-form");
    const confirmation = form.locator(".yolo-confirmation");
    const createPosts: string[] = [];
    page.on("request", (candidate) => {
      if (candidate.method() === "POST" && candidate.url().endsWith("/api/sessions")) {
        createPosts.push(candidate.postData() ?? "");
      }
    });

    await page.locator(".new-session-button").click();
    await expect(form).toBeVisible();
    await form.getByLabel("folder", { exact: true }).fill(cwd);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();

    // The first submit is refused and turned into the question.
    const [refused] = await Promise.all([
      page.waitForResponse(
        (candidate) => candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions"),
      ),
      form.locator(".create-session-submit").click(),
    ]);
    expect(refused.status(), "the helm must refuse a YOLO launch on a host that asks before YOLO launches").toBe(409);
    await expect(confirmation).toBeVisible();
    // The GUI's own explanation: what YOLO means, why this launch is one,
    // and that nothing started. The helm's sentence is for the command line
    // and is not shown.
    await expect(confirmation).toContainText("Confirm YOLO launch");
    await expect(confirmation).toContainText("YOLO means the agent bypasses its safety checks");
    await expect(confirmation).toContainText("This launch uses YOLO permissions.");
    await expect(confirmation).toContainText("asks before every YOLO launch. Nothing has been started.");
    await expect(confirmation).not.toContainText("--confirm-yolo");
    // Right under Launch, where the click happened, and on screen: it used to
    // render after every other section, below the launcher's visible edge.
    expect(
      await confirmation.evaluate((node) => node.previousElementSibling?.classList.contains("launch-composer-actions")),
      "the question renders directly under the Launch row",
    ).toBe(true);
    await expect(confirmation).toBeInViewport();
    expect(
      (await listSessions(request)).sessions.filter((row) => row.cwd === cwd),
      "a refused launch must start nothing",
    ).toHaveLength(0);

    // Cancel leaves the form open and still starts nothing.
    await confirmation.locator(".yolo-cancel").click();
    await expect(confirmation).toHaveCount(0);
    await expect(form).toBeVisible();

    // A question belongs to the draft it was asked about: asking again and
    // then editing the draft retires it, so its confirmation can never ride
    // along with a launch the user was not asked about.
    await form.locator(".create-session-submit").click();
    await expect(confirmation).toBeVisible();
    await form.getByLabel("name (optional)").fill("edited after the question");
    await expect(confirmation).toHaveCount(0);
    await form.getByLabel("name (optional)").fill("");

    // Asking again and confirming retries with the override and launches.
    await form.locator(".create-session-submit").click();
    await expect(confirmation).toBeVisible();
    const [confirmed] = await Promise.all([
      page.waitForResponse(
        (candidate) => candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions"),
      ),
      confirmation.locator(".yolo-confirm").click(),
    ]);
    expect(confirmed.ok(), `the confirmed launch must be admitted: ${await confirmed.text()}`).toBe(true);
    const session = await confirmed.json();
    created.push(session.id);
    expect(session.launch.selection).toMatchObject({ harness: "codex", permissions: "yolo" });
    await expect(form, "a successful launch closes the composer").toHaveCount(0);
    const bodies = createPosts.map((body) => JSON.parse(body));
    expect(bodies.map((body) => body.confirm_yolo ?? false)).toEqual([false, false, false, true]);

    // Once the host allows YOLO without asking, the same launch does not ask.
    await setLocalYoloWithoutAsking(request, true);
    await page.locator(".new-session-button").click();
    await expect(form).toBeVisible();
    await form.getByLabel("folder", { exact: true }).fill(cwd);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
    const [safe] = await Promise.all([
      page.waitForResponse(
        (candidate) => candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions"),
      ),
      form.locator(".create-session-submit").click(),
    ]);
    expect(safe.ok(), `a YOLO launch on a safe host must be admitted: ${await safe.text()}`).toBe(true);
    created.push((await safe.json()).id);
    await expect(form).toHaveCount(0);
  } finally {
    await setLocalYoloWithoutAsking(request, false);
    // The confirmed launches leave YOLO remembered helm-wide; later specs
    // on the shared helm expect a fresh composer to start on "default".
    await patchPreferences(request, { remembered_permissions: null, skip_session_confirmations: previous.skip_session_confirmations ?? null });
    for (const id of created) await cleanupSession(request, id);
  }
});

/**
 * Guarded is a new launch choice, not approval of the refused YOLO request.
 * A held create makes its permission change observable before success
 * closes the form. The real helm then proves one guarded session results,
 * the host still asks, and the ordinary successful-launch memory is guarded.
 */
test("the YOLO question can launch guarded and remember that permission", async ({ page, request }) => {
  const cwd = stackScratchDir("yolo-guarded-");
  const template = `e2e-yolo-guarded-${Date.now()}`;
  const gate = routeGate();
  const createBodies: any[] = [];
  const markPosts: unknown[] = [];
  let guardedDone: Promise<unknown> | undefined;
  const isCreate = (candidate: { request(): { method(): string }; url(): string }) =>
    candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions");
  try {
    await setLocalYoloWithoutAsking(request, false);
    const local = await localHostId(request);
    const saved = await request.put(`/api/templates/${template}`, {
      data: { kind: "agent", agent: "codex", permissions: "yolo" },
    });
    expect(saved.ok(), "the YOLO template must exist before the launcher loads it").toBe(true);
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const body = route.request().postDataJSON();
      createBodies.push(body);
      if (body.launch?.permissions === "guarded") {
        // Drain this actual request before listing sessions during cleanup,
        // including when a held-boundary assertion fails before release.
        guardedDone = route.request().response();
        await gate.wait();
      }
      await route.continue();
    });
    page.on("request", (candidate) => {
      if (candidate.method() === "POST" && candidate.url().endsWith(`/api/hosts/${local}/yolo-without-asking`)) {
        markPosts.push(candidate.postData());
      }
    });
    await page.goto("/");
    const form = await openYoloLaunch(page, cwd);
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill(`tl:${template}`);
    await form.getByRole("option", { name: `Template: ${template}`, exact: true }).click();
    const permissions = form.locator(".launch-composer-permissions-choice");
    await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute("aria-pressed", "true");
    const [refused] = await Promise.all([page.waitForResponse(isCreate), form.locator(".create-session-submit").click()]);
    expect(refused.status()).toBe(409);
    const confirmation = form.locator(".yolo-confirmation");
    await expect(confirmation).toBeVisible();
    await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
    const guarded = confirmation.locator(".yolo-launch-guarded");
    await expect(guarded).toHaveText("Start guarded session instead");
    await expect(guarded).toHaveAttribute("type", "button");
    await expect(guarded).toHaveClass(/\bbtn-primary\b/);
    await expect(confirmation.locator(".dialog-actions button")).toHaveText([
      "Cancel", "Start guarded session instead", "Start, and don't ask again on this host", "Start YOLO session anyway",
    ]);
    await expect(confirmation.locator(".yolo-confirm-stop-asking")).toHaveClass(/\bbtn-danger btn-outline\b/);
    // These are still inline native answers. Starting from the established
    // safe focus must reach guarded, permanent YOLO, then the one-time answer.
    await page.keyboard.press("Tab");
    await expect(guarded).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(confirmation.locator(".yolo-confirm-stop-asking")).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(confirmation.locator(".yolo-confirm")).toBeFocused();
    expect(createBodies).toHaveLength(1);
    expect((await listSessions(request)).sessions.filter((row) => row.cwd === cwd)).toHaveLength(0);

    const launched = page.waitForResponse(isCreate);
    await guarded.click();
    await expect.poll(() => createBodies.length).toBe(2);
    await expect(permissions.getByRole("button", { name: "guarded", exact: true })).toHaveAttribute("aria-pressed", "true");
    // The launcher has no stored-launch baseline marker. Its selected
    // permission and the held request expose the actual template edit.
    expect(createBodies[1].launch).toMatchObject({ harness: "codex", permissions: "guarded" });
    expect(createBodies[1].launch).not.toHaveProperty("permission_flags");
    expect(createBodies[1]).not.toHaveProperty("confirm_yolo");
    expect(createBodies[0].intent_key).toBeTruthy();
    expect(createBodies[1].intent_key).toBeTruthy();
    expect(createBodies[1].intent_key, "changing permission retires the refused request").not.toBe(createBodies[0].intent_key);
    expect(markPosts).toHaveLength(0);
    gate.release();
    const response = await launched;
    expect(response.ok(), `guarded launch: ${await response.text()}`).toBe(true);
    const session = await response.json();
    expect(session.launch.selection).toMatchObject({ harness: "codex", permissions: "guarded" });
    await expect(form).toHaveCount(0);
    expect((await listSessions(request)).sessions.filter((row) => row.cwd === cwd).map((row) => row.id)).toEqual([session.id]);
    expect(createBodies, "one answer sends exactly one new create").toHaveLength(2);
    expect(markPosts).toHaveLength(0);
    expect((await listHosts(request)).find((host) => host.id === local)?.yolo_without_asking).toBe(false);

    await page.locator(".new-session-button").click();
    await expect(form).toBeVisible();
    await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }))
      .toHaveAttribute("aria-pressed", "true");
    await form.locator(".launch-composer-cancel").click();
  } finally {
    gate.release();
    await guardedDone?.catch(() => undefined);
    await setLocalYoloWithoutAsking(request, false);
    await patchPreferences(request, { remembered_permissions: null });
    try {
      for (const row of (await listSessions(request)).sessions.filter((candidate) => candidate.cwd === cwd)) {
        await cleanupSession(request, row.id);
      }
    } finally {
      const removed = await request.delete(`/api/templates/${template}`);
      expect(removed.ok() || removed.status() === 404).toBe(true);
    }
  }
});

/**
 * A launch whose reply was lost may already have started its session, and
 * its retry keeps the same request so the helm can match it. If the host has
 * meanwhile been set to ask before YOLO launches, that retry is refused with
 * the YOLO question, and a guarded answer there would be a new request that
 * could start a second session. Spec: after a lost reply, the question says
 * an earlier attempt may already have started the launch and offers no
 * guarded answer.
 */
test("the YOLO question after a lost launch reply offers no guarded answer", async ({ page, request }) => {
  const cwd = stackScratchDir("yolo-uncertain-");
  const keys: string[] = [];
  const inCwd = async () => (await listSessions(request)).sessions.filter((row) => row.cwd === cwd);
  try {
    await setLocalYoloWithoutAsking(request, true);
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      keys.push(route.request().postDataJSON().intent_key);
      if (keys.length > 1) return route.continue();
      // The first create reaches the helm and starts; only its reply is lost.
      await route.fetch();
      await route.abort("failed");
    });
    await page.goto("/");
    const form = await openYoloLaunch(page, cwd);
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".create-session-error"), "premise: the lost reply left the form open").toBeVisible();
    await expect.poll(async () => (await inCwd()).length, { message: "premise: the first attempt started" }).toBe(1);

    await setLocalYoloWithoutAsking(request, false);
    await form.locator(".create-session-submit").click();
    const confirmation = form.locator(".yolo-confirmation");
    await expect(confirmation).toContainText("An earlier attempt at this launch may already have started it.");
    await expect(confirmation.locator(".yolo-launch-guarded")).toHaveCount(0);
    expect(keys, "the refused retry is the same request as the lost attempt").toEqual([keys[0], keys[0]]);
    expect(await inCwd()).toHaveLength(1);
  } finally {
    await setLocalYoloWithoutAsking(request, false);
    for (const row of await inCwd()) await cleanupSession(request, row.id);
  }
});

/** Custom words cannot be silently dropped by a guarded answer, even when
 * the words themselves request a vendor safety mode. The real guard refuses
 * them as custom; the browser must keep only its existing answers. */
test("the YOLO question offers no guarded answer for custom permission flags", async ({ page, request }) => {
  const cwd = stackScratchDir("yolo-custom-answer-");
  try {
    await setLocalYoloWithoutAsking(request, false);
    await page.goto("/");
    const form = await openYoloLaunch(page, cwd);
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "custom", exact: true }).click();
    await form.locator(".custom-permission-flags").fill("--sandbox workspace-write");
    const [refused] = await Promise.all([
      page.waitForResponse((candidate) => candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions")),
      form.locator(".create-session-submit").click(),
    ]);
    expect(refused.status()).toBe(409);
    expect(refused.request().postDataJSON().launch).toMatchObject({ permissions: "custom", permission_flags: ["--sandbox", "workspace-write"] });
    const confirmation = form.locator(".yolo-confirmation");
    await expect(confirmation).toContainText("uses custom permission flags");
    await expect(confirmation.locator(".yolo-launch-guarded")).toHaveCount(0);
    await expect(confirmation.locator(".yolo-confirm")).toBeVisible();
    await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
    await confirmation.locator(".yolo-cancel").click();
    await expect(confirmation).toHaveCount(0);
  } finally {
    await setLocalYoloWithoutAsking(request, false);
    await patchPreferences(request, { remembered_permissions: null });
    for (const row of (await listSessions(request)).sessions.filter((candidate) => candidate.cwd === cwd)) {
      await cleanupSession(request, row.id);
    }
  }
});

/**
 * Cancel and an answer to the launcher's YOLO question delivered in one
 * burst, before the render that removes the question, start nothing. Why:
 * the YOLO answers launch an agent with no approval prompts (and "don't ask
 * again" also turns the host's question off), and the answers used to be
 * the form's submit buttons recording consent from their own copy of the
 * question, so an answer queued behind Cancel restored the consent and
 * resubmitted the cancelled draft. Specifies, for each answer: after the
 * burst no create carries the override, the host is not marked, nothing
 * starts, and a following plain Launch is asked about again; a genuine
 * answer afterwards still retries the same request with the override.
 * A stale guarded answer must also leave the original YOLO draft alone.
 */
test("cancel and an answer to the launcher's YOLO question in one burst start nothing", async ({ page, request }) => {
  const cwd = stackScratchDir("yolo-burst-");
  const created: string[] = [];
  try {
    await setLocalYoloWithoutAsking(request, false);
    const local = await localHostId(request);
    const createPosts: string[] = [];
    page.on("request", (candidate) => {
      if (candidate.method() === "POST" && candidate.url().endsWith("/api/sessions")) {
        createPosts.push(candidate.postData() ?? "");
      }
    });
    const markPosts: unknown[] = [];
    page.on("request", (candidate) => {
      if (candidate.method() === "POST" && candidate.url().endsWith(`/api/hosts/${local}/yolo-without-asking`)) {
        markPosts.push(candidate.postData());
      }
    });
    const isCreate = (candidate: { request(): { method(): string }; url(): string }) =>
      candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions");

    await page.goto("/");
    const form = await openYoloLaunch(page, cwd);
    const confirmation = form.locator(".yolo-confirmation");
    const [refused] = await Promise.all([page.waitForResponse(isCreate), form.locator(".create-session-submit").click()]);
    expect(refused.status()).toBe(409);
    await expect(confirmation).toBeVisible();

    for (const answer of [".yolo-confirm", ".yolo-confirm-stop-asking", ".yolo-launch-guarded"]) {
      // Both buttons are captured before either is clicked, then clicked in
      // one synchronous block: Cancel first, the answer second, before any
      // render can remove the answer (the technique the session list's
      // cancel-and-confirm test uses). The answer must still be connected
      // and enabled when clicked, or the click would reach no handler and
      // prove nothing.
      const answerWasLive = await confirmation.evaluate((node, selector) => {
        const cancel = node.querySelector<HTMLButtonElement>(".yolo-cancel")!;
        const start = node.querySelector<HTMLButtonElement>(selector)!;
        cancel.click();
        const live = start.isConnected && !start.disabled;
        start.click();
        return live;
      }, answer);
      expect(answerWasLive, `premise: ${answer} was still live when clicked`).toBe(true);
      await expect(confirmation).toHaveCount(0);
      await expect(form, `cancel keeps the launcher open (${answer})`).toBeVisible();
      await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }))
        .toHaveAttribute("aria-pressed", "true");

      // A plain Launch is the observation point: its reply proves the page
      // has handled everything the burst could have started, and it must be
      // asked about again rather than ride on restored consent.
      const [again] = await Promise.all([
        page.waitForResponse(isCreate),
        form.locator(".create-session-submit").click(),
      ]);
      expect(again.status(), `after cancel then ${answer}, Launch is not confirmed`).toBe(409);
      await expect(confirmation).toBeVisible();
    }
    const bodies = createPosts.map((body) => JSON.parse(body));
    expect(
      bodies.map((body) => body.confirm_yolo ?? false),
      "no create after a burst carries the override",
    ).toEqual([false, false, false, false]);
    expect(bodies.map((body) => body.launch.permissions), "stale answers do not change or submit a guarded draft")
      .toEqual(["yolo", "yolo", "yolo", "yolo"]);
    expect(markPosts, "the host is never marked").toHaveLength(0);
    expect((await listHosts(request)).find((host) => host.id === local)?.yolo_without_asking).toBe(false);
    expect((await listSessions(request)).sessions.filter((row) => row.cwd === cwd)).toHaveLength(0);

    // Positive control: a genuine answer retries the same request with the
    // override and launches.
    const [confirmed] = await Promise.all([
      page.waitForResponse(isCreate),
      confirmation.locator(".yolo-confirm").click(),
    ]);
    expect(confirmed.ok(), `the confirmed launch must be admitted: ${await confirmed.text()}`).toBe(true);
    created.push((await confirmed.json()).id);
    const last = JSON.parse(createPosts[createPosts.length - 1]);
    expect(last.confirm_yolo).toBe(true);
    expect(last.intent_key, "the confirmation retries the refused request").toBe(
      JSON.parse(createPosts[createPosts.length - 2]).intent_key,
    );
  } finally {
    await setLocalYoloWithoutAsking(request, false);
    await patchPreferences(request, { remembered_permissions: null });
    for (const id of created) await cleanupSession(request, id);
    // A regression would start sessions this test never recorded; leave
    // none of them on the shared helm.
    for (const row of (await listSessions(request)).sessions.filter((candidate) => candidate.cwd === cwd)) {
      await cleanupSession(request, row.id);
    }
  }
});

/**
 * "Start, and don't ask again on this host" lets the host start YOLO
 * launches and then starts the launch, and when marking fails it starts
 * nothing. Why: the button turns a safety question off for good, so the
 * order matters. A launch that went out before (or without) the host being
 * marked would leave the user asked again next time for a setting they
 * believe they changed, and a failed mark that still launched would start a
 * YOLO session on an answer the helm never recorded. Specifies, on the
 * launcher: a refused mark shows its reason inside the confirmation, sends no
 * create, leaves the host asking, and withdraws the override (a plain Launch
 * asks again); a mark that succeeds is followed by exactly one create with
 * the override, the host now reads safe, and the launcher closes.
 */
test("don't ask again marks the host safe before launching, and a failed mark launches nothing", async ({
  page,
  request,
}) => {
  const cwd = stackScratchDir("yolo-stop-asking-");
  // This spec does not install the terminal family's reset hook. Capture
  // the real stamp here so the refusal tests recovery without latching skew.
  const build = requireHelmBuild(await request.get("/api/sessions"), "YOLO refusal fixture");
  const created: string[] = [];
  try {
    await setLocalYoloWithoutAsking(request, false);
    const local = await localHostId(request);
    const createPosts: string[] = [];
    page.on("request", (candidate) => {
      if (candidate.method() === "POST" && candidate.url().endsWith("/api/sessions")) {
        createPosts.push(candidate.postData() ?? "");
      }
    });
    // The first mark is refused by the route, the second reaches the helm.
    let refuseMark = true;
    const markPosts: unknown[] = [];
    await page.route(`**/api/hosts/${local}/yolo-without-asking`, async (route) => {
      markPosts.push(route.request().postDataJSON());
      if (refuseMark) {
        await route.fulfill({
          status: 409,
          headers: { "content-type": "text/plain", "x-farhelm-build": build },
          body: "held by the test",
        });
      } else {
        await route.continue();
      }
    });

    await page.goto("/");
    const form = await openYoloLaunch(page, cwd);
    const confirmation = form.locator(".yolo-confirmation");
    const isCreate = (candidate: { request(): { method(): string }; url(): string }) =>
      candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions");

    const [refused] = await Promise.all([page.waitForResponse(isCreate), form.locator(".create-session-submit").click()]);
    expect(refused.status()).toBe(409);
    await expect(confirmation).toBeVisible();

    // A refused mark: the question, which the answer took down, comes back
    // with its reason, and no create is sent.
    await confirmation.locator(".yolo-confirm-stop-asking").click();
    await expect(confirmation.locator(".yolo-confirmation-error")).toContainText("could not stop asking for");
    await expect(confirmation.locator(".yolo-confirmation-error")).toContainText("held by the test");
    await expect(page.locator(".build-skew"), "a current-build refusal must preserve the normal page lifecycle").toHaveCount(0);
    expect(markPosts).toEqual([{ yolo_without_asking: true }]);
    expect(createPosts, "a failed mark sends no create").toHaveLength(1);
    expect((await listHosts(request)).find((host) => host.id === local)?.yolo_without_asking).toBe(false);
    // The question returns as a fresh one, so focus lands on its safe answer
    // rather than staying lost with the button that was pressed.
    await expect(confirmation.locator(".yolo-cancel")).toBeFocused();

    // The override went with the failure: a plain Launch asks again.
    const [askedAgain] = await Promise.all([
      page.waitForResponse(isCreate),
      form.locator(".create-session-submit").click(),
    ]);
    expect(askedAgain.status(), "after a failed mark, Launch is not confirmed").toBe(409);
    await expect(confirmation).toBeVisible();
    await expect(confirmation.locator(".yolo-confirmation-error"), "a new question starts clean").toHaveCount(0);

    // A mark that succeeds, then the launch with the override.
    refuseMark = false;
    const [launched] = await Promise.all([
      page.waitForResponse(isCreate),
      confirmation.locator(".yolo-confirm-stop-asking").click(),
    ]);
    expect(launched.ok(), `the launch after marking must be admitted: ${await launched.text()}`).toBe(true);
    created.push((await launched.json()).id);
    await expect(form, "a successful launch closes the composer").toHaveCount(0);
    expect(markPosts).toHaveLength(2);
    expect((await listHosts(request)).find((host) => host.id === local)?.yolo_without_asking).toBe(true);
    const bodies = createPosts.map((body) => JSON.parse(body));
    expect(bodies.map((body) => body.confirm_yolo ?? false)).toEqual([false, false, true]);
  } finally {
    await setLocalYoloWithoutAsking(request, false);
    await patchPreferences(request, { remembered_permissions: null });
    for (const id of created) await cleanupSession(request, id);
  }
});

/**
 * Drive one Replace started from an open session's top bar into the YOLO
 * question, bring the session back to life while the question is open, and
 * answer it with `answer` (one of the confirmation's two YOLO buttons).
 * Asserts that the answered request still carries the confirmed prompt's
 * `only_if_nothing_alive: true` and that the precondition refused the source
 * delete, so the restarted session survives.
 */
async function headerReplaceKeepsAnswer(page: Page, request: APIRequestContext, answer: string): Promise<void> {
  const cwd = stackScratchDir("yolo-header-replace-");
  const local = await localHostId(request);
  // A YOLO launch that can resume: restart, the only way to bring the
  // stopped source back to life below, only ever resumes.
  const sourceId = await createResumableYoloSession(request, {
    cwd,
    title: `yolo-header-replace-${Date.now()}`,
    host: local,
  });
  try {
    await setLocalYoloWithoutAsking(request, false);
    await stopSession(request, sourceId);
    await page.goto("/");
    await page.locator(`.session-row[data-session-id="${sourceId}"]`).click();
    const exitedBadge = page.locator(".session-details .status-badge.exited");
    await expect(exitedBadge).toBeVisible({ timeout: 20_000 });
    const header = page.locator(".header-replace-anchor");
    await header.locator(".header-replace").click();
    // Premise: the prompt the user confirms says nothing is alive (an exited
    // agent and no open tabs, which would add a clause of their own).
    await expect(header.locator(".confirm-consequence")).toHaveText(
      "replacing discards the conversation; a fresh session with the same settings takes its place:",
    );

    const [refused] = await Promise.all([
      page.waitForResponse(
        (candidate) =>
          candidate.request().method() === "POST" && candidate.url().endsWith(`/api/sessions/${encodeURIComponent(sourceId)}/replace`),
      ),
      header.locator(".header-replace-confirm .btn-danger").click(),
    ]);
    expect(refused.status(), "the replace is refused as a YOLO launch on a host that asks before YOLO launches").toBe(409);
    const confirmation = page.locator(".yolo-confirmation");
    await expect(confirmation).toBeVisible();

    // While the question is open, the session comes back to life from
    // outside the prompt, through a restart that resumes it.
    const restarted = await request.post(`/api/sessions/${encodeURIComponent(sourceId)}/restart`, { data: {} });
    expect(restarted.ok(), `restarting the source: ${await restarted.text()}`).toBe(true);
    // And this page has seen it: code that re-read the session when the
    // answer arrived would now find it alive and send no precondition.
    await expect(exitedBadge).toHaveCount(0, { timeout: 20_000 });

    const [answered] = await Promise.all([
      page.waitForRequest(
        (candidate) => candidate.method() === "POST" && candidate.url().endsWith(`/api/sessions/${encodeURIComponent(sourceId)}/replace`),
      ),
      confirmation.locator(answer).click(),
    ]);
    const body = JSON.parse(answered.postData() ?? "{}");
    expect(body.only_if_nothing_alive, "the confirmed prompt said nothing was alive").toBe(true);
    expect(body.confirm_yolo, "the answer carries the YOLO override").toBe(true);
    // The precondition refuses the source delete: the replacement is made,
    // the restarted source is kept, and the page says both exist.
    const reply = await answered.response();
    expect(reply?.ok(), "the precondition refuses the source delete").toBe(false);
    expect(await reply?.text()).toContain("both sessions still exist");
    await expect(page.locator(".replace-error")).toContainText("both sessions still exist");
    expect((await listSessions(request)).sessions.some((row) => row.id === sourceId)).toBe(true);
  } finally {
    await setLocalYoloWithoutAsking(request, false);
    await patchPreferences(request, { remembered_permissions: null });
    for (const row of (await listSessions(request)).sessions.filter((candidate) => candidate.cwd === cwd)) {
      await cleanupSession(request, row.id);
    }
  }
}

/**
 * A Replace started from an open session's top bar that meets the YOLO
 * question keeps the "nothing is alive" answer of the prompt the user
 * confirmed, even when the session comes back to life while the question is
 * open, for both of the question's YOLO answers. Why: the prompt that said
 * nothing was running is what the user agreed to, so the source delete must
 * carry the supervisor-side "only if nothing is alive" precondition.
 * Recomputed when the YOLO answer arrived, a session restarted in the
 * meantime (from another window, an agent, or this view) sent no
 * precondition, and the delete killed the freshly started agent after a
 * prompt that said nothing was alive.
 */
test("a header replace keeps its confirmed nothing-alive answer when confirmed once", async ({ page, request }) => {
  await headerReplaceKeepsAnswer(page, request, ".yolo-confirm");
});

/**
 * The same as the test above, answered with "Start, and don't ask again on
 * this host": that answer also re-sends the Replace, so it must carry the
 * same confirmed answer. Specifies the same request and survival outcome.
 */
test("a header replace keeps its confirmed nothing-alive answer when told not to ask again", async ({
  page,
  request,
}) => {
  await headerReplaceKeepsAnswer(page, request, ".yolo-confirm-stop-asking");
});

/**
 * The sidebar's version of `headerReplaceKeepsAnswer`: start Replace from a
 * session row's menu, bring the session back to life while the YOLO question
 * is open, and answer with `answer`. Asserts the same request and survival
 * outcome.
 */
async function sidebarReplaceKeepsAnswer(page: Page, request: APIRequestContext, answer: string): Promise<void> {
  const cwd = stackScratchDir("yolo-sidebar-replace-");
  const local = await localHostId(request);
  // A YOLO launch that can resume: restart, the only way to bring the
  // stopped source back to life below, only ever resumes.
  const sourceId = await createResumableYoloSession(request, {
    cwd,
    title: `yolo-sidebar-replace-${Date.now()}`,
    host: local,
  });
  try {
    await setLocalYoloWithoutAsking(request, false);
    await stopSession(request, sourceId);
    await page.goto("/");
    const sourceRow = page.locator(`.session-row[data-session-id="${sourceId}"]`);
    const exitedBadge = sourceRow.locator(".status-badge.exited");
    await expect(exitedBadge).toBeVisible({ timeout: 20_000 });
    await openRowMenu(sourceRow);
    await sourceRow.locator(".session-row-replace").click();
    // Premise: the prompt the user confirms says nothing is alive.
    await expect(sourceRow.locator(".confirm-consequence")).toHaveText(
      "replacing discards the conversation; a fresh session with the same settings takes its place:",
    );

    const [refused] = await Promise.all([
      page.waitForResponse(
        (candidate) =>
          candidate.request().method() === "POST" && candidate.url().endsWith(`/api/sessions/${encodeURIComponent(sourceId)}/replace`),
      ),
      sourceRow.locator(".confirm-replace").click(),
    ]);
    expect(refused.status(), "the replace is refused as a YOLO launch on a host that asks before YOLO launches").toBe(409);
    const confirmation = page.locator(".yolo-confirmation");
    await expect(confirmation).toBeVisible();

    // While the question is open, the session comes back to life from
    // outside the prompt, through a restart that resumes it.
    const restarted = await request.post(`/api/sessions/${encodeURIComponent(sourceId)}/restart`, { data: {} });
    expect(restarted.ok(), `restarting the source: ${await restarted.text()}`).toBe(true);
    // And the list has shown it: code that looked the row up again when the
    // answer arrived would now find it alive and send no precondition.
    await expect(exitedBadge).toHaveCount(0, { timeout: 20_000 });

    const [answered] = await Promise.all([
      page.waitForRequest(
        (candidate) => candidate.method() === "POST" && candidate.url().endsWith(`/api/sessions/${encodeURIComponent(sourceId)}/replace`),
      ),
      confirmation.locator(answer).click(),
    ]);
    const body = JSON.parse(answered.postData() ?? "{}");
    expect(body.only_if_nothing_alive, "the confirmed prompt said nothing was alive").toBe(true);
    expect(body.confirm_yolo, "the answer carries the YOLO override").toBe(true);
    const reply = await answered.response();
    expect(reply?.ok(), "the precondition refuses the source delete").toBe(false);
    expect(await reply?.text()).toContain("both sessions still exist");
    await expect(sourceRow.locator(".action-error")).toContainText("both sessions still exist");
    expect((await listSessions(request)).sessions.some((row) => row.id === sourceId)).toBe(true);
  } finally {
    await setLocalYoloWithoutAsking(request, false);
    await patchPreferences(request, { remembered_permissions: null });
    for (const row of (await listSessions(request)).sessions.filter((candidate) => candidate.cwd === cwd)) {
      await cleanupSession(request, row.id);
    }
  }
}

/**
 * A Replace started from a session row's menu that meets the YOLO question
 * keeps the "nothing is alive" answer of the row's prompt, even when the
 * session comes back to life while the question is open. Why: as for the
 * top-of-session Replace, the source delete must carry the precondition the
 * confirmed prompt implied. The sidebar looked the row up again at answer
 * time, so a restarted session lost the precondition and was killed after a
 * prompt that said nothing was running.
 */
test("a sidebar replace keeps its confirmed nothing-alive answer when confirmed once", async ({ page, request }) => {
  await sidebarReplaceKeepsAnswer(page, request, ".yolo-confirm");
});

/**
 * The same as the test above, answered with "Start, and don't ask again on
 * this host", which also re-sends the Replace and must carry the same
 * confirmed answer.
 */
test("a sidebar replace keeps its confirmed nothing-alive answer when told not to ask again", async ({
  page,
  request,
}) => {
  await sidebarReplaceKeepsAnswer(page, request, ".yolo-confirm-stop-asking");
});

/**
 * With session confirmations skipped, a Replace of a YOLO session on a host
 * that asks before YOLO launches still meets the YOLO question, with Cancel
 * focused, and Cancel replaces nothing. Why: skipping covers the everyday
 * "are you sure" question only; the YOLO question is a separate guard the
 * helm enforces, and a refactor that let a skipped first click count as YOLO
 * consent would launch YOLO without asking. Specifies that the one skipped
 * click sends a single Replace without the YOLO override, that the helm
 * refuses it as a YOLO launch, and that the source session survives Cancel.
 */
test("a skipped header replace still asks the YOLO question", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const cwd = stackScratchDir("yolo-skip-replace-");
  const local = await localHostId(request);
  const sourceId = await createResumableYoloSession(request, {
    cwd,
    title: `yolo-skip-replace-${Date.now()}`,
    host: local,
  });
  try {
    await setLocalYoloWithoutAsking(request, false);
    await patchPreferences(request, { skip_session_confirmations: true });
    expect((await readPreferences(request)).skip_session_confirmations, "premise: confirmations are skipped").toBe(true);
    await stopSession(request, sourceId);
    const replacePosts: string[] = [];
    page.on("request", (candidate) => {
      if (candidate.method() === "POST" && candidate.url().endsWith(`/api/sessions/${encodeURIComponent(sourceId)}/replace`)) {
        replacePosts.push(candidate.postData() ?? "{}");
      }
    });
    await page.goto("/");
    await page.locator(`.session-row[data-session-id="${sourceId}"]`).click();
    await expect(page.locator(".session-details .status-badge.exited")).toBeVisible({ timeout: 20_000 });

    const [refused] = await Promise.all([
      page.waitForResponse(
        (candidate) =>
          candidate.request().method() === "POST" && candidate.url().endsWith(`/api/sessions/${encodeURIComponent(sourceId)}/replace`),
      ),
      page.locator(".header-replace-anchor .header-replace").click(),
    ]);
    await expect(page.locator(".header-replace-confirm"), "premise: the session question was skipped").toHaveCount(0);
    expect(refused.status(), "the skipped replace is refused as a YOLO launch").toBe(409);
    const confirmation = page.locator(".yolo-confirmation");
    await expect(confirmation).toBeVisible();
    await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
    expect(replacePosts).toHaveLength(1);
    expect(JSON.parse(replacePosts[0]).confirm_yolo ?? false, "a skipped click is not YOLO consent").toBe(false);

    await confirmation.locator(".yolo-cancel").click();
    await expect(confirmation).toHaveCount(0);
    expect(replacePosts, "cancel sends nothing").toHaveLength(1);
    const remaining = (await listSessions(request)).sessions.filter((row) => row.cwd === cwd);
    expect(remaining.map((row) => row.id), "only the source session exists").toEqual([sourceId]);
  } finally {
    await patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null });
    await setLocalYoloWithoutAsking(request, false);
    for (const row of (await listSessions(request)).sessions.filter((candidate) => candidate.cwd === cwd)) {
      await cleanupSession(request, row.id);
    }
  }
});

/** A template preserves consent only while its completed request is unchanged.
 * Real helm YOLO refusals establish the question; confirmed and Guarded attempts
 * receive stamped unresolved fixture replies so this test starts no vendor agent.
 */
test("template edits retain an unchanged YOLO question and retire a changed one", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const local = await localHostId(request);
  const previousHost = (await listHosts(request)).find((host) => host.id === local);
  expect(previousHost, "the owned stack must expose its local host before policy is changed").toBeTruthy();
  if (!previousHost) throw new Error("the local host disappeared before the YOLO fixture started");
  const build = requireHelmBuild(await request.get("/api/sessions"), "template YOLO fixture");
  const cwd = stackScratchDir("yolo-template-edit-");
  const suffix = Date.now();
  const yoloTemplate = `e2e-question-yolo-${suffix}`;
  const guardedTemplate = `e2e-question-guarded-${suffix}`;
  const ownedTemplates: string[] = [];
  const bodies: any[] = [];
  const policyWrites: string[] = [];
  const completions: Promise<void>[] = [];
  const endpoint = "**/api/sessions";
  const observeCreate = async (route: Route) => {
    if (route.request().method() !== "POST") return route.continue();
    const body = route.request().postDataJSON();
    const attempt = bodies.push(body);
    const completion = (async () => {
      if (body.confirm_yolo !== true && body.launch?.permissions === "yolo" && body.host === local) {
        // Only this host's unconfirmed YOLO request reaches the real
        // guard. Every other request is intercepted before it can launch.
        await route.continue();
      } else {
        // There is no definitely-unaccepted marker: the form must retain
        // its key after this completed attempt, as for an ambiguous reply.
        await route.fulfill({
          status: 503,
          headers: { "x-farhelm-build": build, "content-type": "text/plain" },
          body: `template YOLO fixture refused create ${attempt}`,
        });
      }
    })();
    completions.push(completion);
    await completion;
  };
  const observePolicy = (candidate: { method(): string; url(): string }) => {
    if (candidate.method() === "POST" && new URL(candidate.url()).pathname === `/api/hosts/${local}/yolo-without-asking`) {
      policyWrites.push(candidate.url());
    }
  };
  let bodyFailed = false;
  try {
    await setLocalYoloWithoutAsking(request, false);
    for (const [name, fields] of [
      [yoloTemplate, { kind: "agent", agent: "codex", permissions: "yolo" }],
      [guardedTemplate, { kind: "agent", permissions: "guarded" }],
    ] as const) {
      const saved = await request.put(`/api/templates/${name}`, { data: fields });
      expect(saved.ok(), `create owned template ${name}: ${await saved.text()}`).toBe(true);
      ownedTemplates.push(name);
      const stored = await request.get("/api/templates");
      expect(stored.ok()).toBe(true);
      expect((await stored.json()).templates.find((row: { name: string }) => row.name === name)?.fields).toEqual(fields);
    }
    await page.route(endpoint, observeCreate);
    page.on("request", observePolicy);
    await page.goto("/");
    const form = await openYoloLaunch(page, cwd);
    await chooseLauncherHost(form.locator(".create-session-host"), String(local));
    await expect(form.locator(".create-session-host")).toHaveAttribute("data-host-id", String(local));
    await expect(form).toHaveAttribute("data-initial-templates-ready", "true");
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    const permissions = form.locator(".launch-composer-permissions-choice");
    const yolo = permissions.getByRole("button", { name: "yolo", exact: true });
    const guarded = permissions.getByRole("button", { name: "guarded", exact: true });
    const question = form.locator(".yolo-confirmation");
    const launch = form.locator(".create-session-submit");
    const isCreate = (candidate: { request(): { method(): string }; url(): string }) =>
      candidate.request().method() === "POST" && new URL(candidate.url()).pathname === "/api/sessions";

    await expect(yolo).toHaveAttribute("aria-pressed", "true");
    const [firstRefusal] = await Promise.all([page.waitForResponse(isCreate), launch.click()]);
    expect(firstRefusal.status()).toBe(409);
    expect(firstRefusal.headers()["x-farhelm-yolo-confirmation"]).toBe("confirmation-required");
    await expect(question).toBeVisible();
    await expect(question.locator(".yolo-cancel")).toBeFocused();
    const firstOpening = await question.getAttribute("data-yolo-confirm-id");
    expect(firstOpening).toBeTruthy();
    expect(bodies).toHaveLength(1);
    expect(bodies[0].intent_key).toEqual(expect.any(String));
    expect(bodies[0].intent_key).not.toBe("");
    expect(bodies[0]).toMatchObject({ host: local, cwd, launch: { harness: "codex", permissions: "yolo" } });
    expect(bodies[0]).not.toHaveProperty("confirm_yolo");

    await search.focus();
    await expect(search).toBeFocused();
    await search.fill(`tl:${yoloTemplate}`);
    await form.getByRole("option", { name: `Template: ${yoloTemplate}`, exact: true }).click();
    await expect(search).toHaveValue("");
    await expect(search).toBeFocused();
    await expect(yolo).toHaveAttribute("aria-pressed", "true");
    await expect(question).toBeVisible();
    await expect(question).toHaveAttribute("data-yolo-confirm-id", firstOpening!);
    expect(bodies, "unchanged template application must not submit").toHaveLength(1);
    expect(policyWrites).toHaveLength(0);

    // This is the positive control for retaining the retry pair: merely
    // leaving a question visible while minting another key must fail too.
    await question.locator(".yolo-confirm").click();
    await expect(form.locator(".create-session-error")).toContainText("template YOLO fixture refused create 2");
    await expect(launch).toHaveAttribute("aria-disabled", "false");
    expect(bodies).toHaveLength(2);
    expect(bodies[1]).toEqual({ ...bodies[0], confirm_yolo: true });
    expect(policyWrites).toHaveLength(0);

    // A new name ends the confirmed request before obtaining a fresh real
    // question. Otherwise the retained one-off consent would bypass it.
    const name = form.getByLabel("name (optional)", { exact: true });
    await name.focus();
    await expect(name).toBeFocused();
    await name.fill(`template-question-${suffix}`);
    const [secondRefusal] = await Promise.all([page.waitForResponse(isCreate), launch.click()]);
    expect(secondRefusal.status()).toBe(409);
    expect(secondRefusal.headers()["x-farhelm-yolo-confirmation"]).toBe("confirmation-required");
    await expect(question).toBeVisible();
    await expect(question.locator(".yolo-cancel")).toBeFocused();
    const refusedOpening = await question.getAttribute("data-yolo-confirm-id");
    expect(refusedOpening).toBeTruthy();
    expect(refusedOpening).not.toBe(firstOpening);
    expect(bodies).toHaveLength(3);
    expect(bodies[2].intent_key).toEqual(expect.any(String));
    expect(bodies[2].intent_key).not.toBe("");
    expect(bodies[2].intent_key).not.toBe(bodies[0].intent_key);
    expect(bodies[2]).toEqual({ ...bodies[0], title: `template-question-${suffix}`, intent_key: bodies[2].intent_key });

    await search.focus();
    await expect(search).toBeFocused();
    await search.fill(`tl:${guardedTemplate}`);
    await form.getByRole("option", { name: `Template: ${guardedTemplate}`, exact: true }).click();
    await expect(search).toHaveValue("");
    await expect(search).toBeFocused();
    await expect(guarded).toHaveAttribute("aria-pressed", "true");
    await expect(question, "the old question must end before a new Launch press").toHaveCount(0);
    expect(bodies, "changing a template edits the draft without submitting").toHaveLength(3);
    expect(policyWrites).toHaveLength(0);

    // A visibility predicate alone can hide a still-live slot. Returning
    // to YOLO must not resurrect that old opening without a new refusal.
    await search.focus();
    await expect(search).toBeFocused();
    await search.fill(`tl:${yoloTemplate}`);
    await form.getByRole("option", { name: `Template: ${yoloTemplate}`, exact: true }).click();
    await expect(search).toHaveValue("");
    await expect(search).toBeFocused();
    await expect(yolo).toHaveAttribute("aria-pressed", "true");
    await expect(question).toHaveCount(0);
    expect(bodies).toHaveLength(3);
    expect(policyWrites).toHaveLength(0);

    await search.focus();
    await expect(search).toBeFocused();
    await search.fill(`tl:${guardedTemplate}`);
    await form.getByRole("option", { name: `Template: ${guardedTemplate}`, exact: true }).click();
    await expect(search).toHaveValue("");
    await expect(search).toBeFocused();
    await expect(guarded).toHaveAttribute("aria-pressed", "true");
    await expect(question).toHaveCount(0);
    await launch.click();
    await expect(form.locator(".create-session-error")).toContainText("template YOLO fixture refused create 4");
    await expect(launch).toHaveAttribute("aria-disabled", "false");
    expect(bodies).toHaveLength(4);
    expect(bodies[3].intent_key).toEqual(expect.any(String));
    expect(bodies[3].intent_key).not.toBe("");
    expect(bodies[3].intent_key).not.toBe(bodies[2].intent_key);
    expect(bodies[3]).toEqual({ ...bodies[2], intent_key: bodies[3].intent_key, launch: { ...bodies[2].launch, permissions: "guarded" } });
    expect(policyWrites).toHaveLength(0);
    expect((await listHosts(request)).find((host) => host.id === local)?.yolo_without_asking).toBe(false);
    expect((await listSessions(request)).sessions.filter((row) => row.cwd === cwd)).toHaveLength(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    page.off("request", observePolicy);
    await cleanupOwnedSteps(bodyFailed, [
      ["create route", async () => {
        await page.unrouteAll({ behavior: "wait" });
        await Promise.all(completions);
      }],
      ["host YOLO policy", () => setLocalYoloWithoutAsking(request, previousHost.yolo_without_asking)],
      ["remembered permissions", () => patchPreferences(request, { remembered_permissions: previous.remembered_permissions ?? null })],
      ...ownedTemplates.map((name): [string, () => Promise<void>] => [name, async () => {
        const removed = await request.delete(`/api/templates/${name}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }]),
    ]);
  }
});

/** A Command question belongs to its literal request, just as an Agent
 * question does. Real guard refusals establish consent openings; confirmed
 * attempts are intercepted so preserving one cannot execute the command.
 */
test("command templates preserve equal YOLO questions and retire changed commands", async ({ page, request }) => {
  const local = await localHostId(request);
  const host = (await listHosts(request)).find((candidate) => candidate.id === local);
  expect(host, "the owned stack must expose a local guard target").toBeTruthy();
  expect(host?.yolo_without_asking, "fixture premise: the local host already asks before YOLO launches").toBe(false);
  const build = requireHelmBuild(await request.get("/api/sessions"), "Command template YOLO fixture");
  const cwd = stackScratchDir("yolo-command-template-");
  const suffix = Date.now();
  const original = { name: `e2e-question-command-a-${suffix}`, command: "printf first" };
  const changed = { name: `e2e-question-command-b-${suffix}`, command: "printf second" };
  const owned: string[] = [];
  const bodies: any[] = [];
  const policyWrites: string[] = [];
  const endpoint = "**/api/sessions";
  const observeCreate = async (route: Route) => {
    if (route.request().method() !== "POST") return route.continue();
    const body = route.request().postDataJSON();
    const attempt = bodies.push(body);
    if (body.host === local && body.command?.yolo === true && body.confirm_yolo !== true) {
      // The real helm's guard is the sole source of questions. Only an
      // unconfirmed YOLO Command reaches it; no confirmed command can run.
      await route.continue();
    } else {
      await route.fulfill({
        status: 503,
        headers: { "x-farhelm-build": build, "content-type": "text/plain" },
        body: `Command template fixture refused create ${attempt}`,
      });
    }
  };
  const observePolicy = (candidate: { method(): string; url(): string }) => {
    if (candidate.method() === "POST" && new URL(candidate.url()).pathname === `/api/hosts/${local}/yolo-without-asking`) {
      policyWrites.push(candidate.url());
    }
  };
  let bodyFailed = false;
  try {
    for (const template of [original, changed]) {
      const fields = { kind: "command", command: template.command, yolo: true };
      const saved = await request.put(`/api/templates/${template.name}`, { data: fields });
      expect(saved.ok(), `store Command template: ${await saved.text()}`).toBe(true);
      owned.push(template.name);
      const listing = await request.get("/api/templates");
      expect(listing.ok()).toBe(true);
      expect((await listing.json()).templates.find((row: { name: string }) => row.name === template.name)?.fields).toEqual(fields);
    }
    await page.route(endpoint, observeCreate);
    page.on("request", observePolicy);
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await chooseLauncherHost(form.locator(".create-session-host"), String(local));
    await expect(form.locator(".create-session-host")).toHaveAttribute("data-host-id", String(local));
    await form.getByLabel("folder", { exact: true }).fill(cwd);
    await expect(form).toHaveAttribute("data-initial-templates-ready", "true");
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    const question = form.locator(".yolo-confirmation");
    const launch = form.locator(".create-session-submit");
    const isCreate = (candidate: { request(): { method(): string }; url(): string }) =>
      candidate.request().method() === "POST" && new URL(candidate.url()).pathname === "/api/sessions";

    /** Completed template acceptance must expose the active Command value,
     * not merely dismiss a search option while leaving another draft active.
     */
    const apply = async (template: typeof original) => {
      const count = bodies.length;
      await search.focus();
      await expect(search).toBeFocused();
      await search.fill(`tl:${template.name}`);
      await form.getByRole("option", { name: `Template: ${template.name}`, exact: true }).click();
      await expect(search).toHaveValue("");
      await expect(search).toBeFocused();
      await expect(form.getByRole("tab", { name: "command", exact: true })).toHaveAttribute("aria-selected", "true");
      await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(template.command);
      await expect(form.getByRole("group", { name: "runs in YOLO mode" }).getByLabel("yes (YOLO)", { exact: true })).toBeChecked();
      expect(bodies, "template acceptance itself cannot launch").toHaveLength(count);
      expect(policyWrites, "template acceptance cannot change host consent policy").toHaveLength(0);
    };

    await apply(original);
    const [first] = await Promise.all([page.waitForResponse(isCreate), launch.click()]);
    expect(first.status()).toBe(409);
    expect(first.headers()["x-farhelm-yolo-confirmation"]).toBe("confirmation-required");
    await expect(question).toBeVisible();
    await expect(question.locator(".yolo-cancel")).toBeFocused();
    const firstOpening = await question.getAttribute("data-yolo-confirm-id");
    expect(firstOpening).toBeTruthy();
    expect(bodies).toHaveLength(1);
    expect(bodies[0]).toMatchObject({ host: local, cwd, command: { command: original.command, yolo: true } });
    expect(bodies[0].intent_key).toEqual(expect.any(String));
    expect(bodies[0].intent_key).not.toBe("");
    expect(bodies[0]).not.toHaveProperty("confirm_yolo");

    await apply(original);
    await expect(question).toHaveAttribute("data-yolo-confirm-id", firstOpening!);
    await expect(question).toBeVisible();
    await apply(changed);
    await expect(question, "a different literal Command retires the opening before Launch").toHaveCount(0);
    const [second] = await Promise.all([page.waitForResponse(isCreate), launch.click()]);
    expect(second.status()).toBe(409);
    expect(second.headers()["x-farhelm-yolo-confirmation"]).toBe("confirmation-required");
    await expect(question).toBeVisible();
    // Each new question schedules its safe-answer focus after becoming usable.
    // Finish that handoff before deliberately moving back to template search.
    await expect(question.locator(".yolo-cancel")).toBeFocused();
    const secondOpening = await question.getAttribute("data-yolo-confirm-id");
    expect(secondOpening).toBeTruthy();
    expect(secondOpening).not.toBe(firstOpening);
    expect(bodies).toHaveLength(2);
    expect(bodies[1].intent_key).toEqual(expect.any(String));
    expect(bodies[1].intent_key).not.toBe("");
    expect(bodies[1].intent_key).not.toBe(bodies[0].intent_key);
    expect(bodies[1]).toEqual({ ...bodies[0], intent_key: bodies[1].intent_key, command: { ...bodies[0].command, command: changed.command } });

    // Returning to old bytes must not revive the original opening or its
    // key. It needs a fresh real refusal before a new answer can be offered.
    await apply(original);
    await expect(question).toHaveCount(0);
    expect(bodies).toHaveLength(2);
    const [third] = await Promise.all([page.waitForResponse(isCreate), launch.click()]);
    expect(third.status()).toBe(409);
    expect(third.headers()["x-farhelm-yolo-confirmation"]).toBe("confirmation-required");
    await expect(question).toBeVisible();
    await expect(question.locator(".yolo-cancel")).toBeFocused();
    const thirdOpening = await question.getAttribute("data-yolo-confirm-id");
    expect(thirdOpening).toBeTruthy();
    expect(thirdOpening).not.toBe(firstOpening);
    expect(thirdOpening).not.toBe(secondOpening);
    expect(bodies).toHaveLength(3);
    expect(bodies[2].intent_key).toEqual(expect.any(String));
    expect(bodies[2].intent_key).not.toBe("");
    expect(bodies[2].intent_key).not.toBe(bodies[0].intent_key);
    expect(bodies[2].intent_key).not.toBe(bodies[1].intent_key);
    expect(bodies[2]).toEqual({ ...bodies[0], intent_key: bodies[2].intent_key });

    // Keep a real Command question through another equal template batch,
    // then prove that its answer uses the same complete request and key.
    await apply(original);
    await expect(question).toBeVisible();
    await expect(question).toHaveAttribute("data-yolo-confirm-id", thirdOpening!);
    const [confirmed] = await Promise.all([page.waitForResponse(isCreate), question.locator(".yolo-confirm").click()]);
    expect(confirmed.status()).toBe(503);
    await expect(form.locator(".create-session-error")).toContainText("Command template fixture refused create 4");
    await expect(launch).toHaveAttribute("aria-disabled", "false");
    expect(bodies).toHaveLength(4);
    expect(bodies[3]).toEqual({ ...bodies[2], confirm_yolo: true });
    expect(policyWrites).toHaveLength(0);
    expect((await listHosts(request)).find((candidate) => candidate.id === local)?.yolo_without_asking).toBe(false);
    expect((await listSessions(request)).sessions.filter((row) => row.cwd === cwd)).toHaveLength(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    try {
      await cleanupOwnedSteps(bodyFailed, [
        ["entered create routes", () => page.unrouteAll({ behavior: "wait" })],
        ...owned.map((name): [string, () => Promise<void>] => [name, async () => {
          const removed = await request.delete(`/api/templates/${name}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }]),
      ]);
    } finally {
      page.off("request", observePolicy);
    }
  }
});
