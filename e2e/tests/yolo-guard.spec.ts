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
import { type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession,
  createResumableYoloSession,
  listHosts,
  listSessions,
  localHostId,
  openRowMenu,
  patchPreferences,
  setLocalYoloWithoutAsking,
  stopSession,
} from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";
import { fulfillAsHelm } from "./helpers/terminal-suite";

/** Open the launcher on a Codex YOLO launch into `cwd`, ready to submit. */
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
  const cwd = stackScratchDir("yolo-guard-");
  const created: string[] = [];
  try {
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
    await expect(confirmation).toContainText("YOLO means the agent runs with no approval prompts");
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
    await patchPreferences(request, { remembered_permissions: null });
    for (const id of created) await cleanupSession(request, id);
  }
});

/**
 * Cancel and an answer to the launcher's YOLO question delivered in one
 * burst, before the render that removes the question, start nothing. Why:
 * both answers launch an agent with no approval prompts (and "don't ask
 * again" also turns the host's question off), and the answers used to be
 * the form's submit buttons recording consent from their own copy of the
 * question, so an answer queued behind Cancel restored the consent and
 * resubmitted the cancelled draft. Specifies, for each answer: after the
 * burst no create carries the override, the host is not marked, nothing
 * starts, and a following plain Launch is asked about again; a genuine
 * answer afterwards still retries the same request with the override.
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

    for (const answer of [".yolo-confirm", ".yolo-confirm-stop-asking"]) {
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
    ).toEqual([false, false, false]);
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
        await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "held by the test" });
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
    const exitedBadge = page.locator(".titlebar .status-badge.exited");
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
          candidate.request().method() === "POST" && candidate.url().endsWith(`/api/sessions/${sourceId}/replace`),
      ),
      header.locator(".header-replace-confirm .btn-danger").click(),
    ]);
    expect(refused.status(), "the replace is refused as a YOLO launch on a host that asks before YOLO launches").toBe(409);
    const confirmation = page.locator(".yolo-confirmation");
    await expect(confirmation).toBeVisible();

    // While the question is open, the session comes back to life from
    // outside the prompt, through a restart that resumes it.
    const restarted = await request.post(`/api/sessions/${sourceId}/restart`, { data: {} });
    expect(restarted.ok(), `restarting the source: ${await restarted.text()}`).toBe(true);
    // And this page has seen it: code that re-read the session when the
    // answer arrived would now find it alive and send no precondition.
    await expect(exitedBadge).toHaveCount(0, { timeout: 20_000 });

    const [answered] = await Promise.all([
      page.waitForRequest(
        (candidate) => candidate.method() === "POST" && candidate.url().endsWith(`/api/sessions/${sourceId}/replace`),
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
          candidate.request().method() === "POST" && candidate.url().endsWith(`/api/sessions/${sourceId}/replace`),
      ),
      sourceRow.locator(".confirm-replace").click(),
    ]);
    expect(refused.status(), "the replace is refused as a YOLO launch on a host that asks before YOLO launches").toBe(409);
    const confirmation = page.locator(".yolo-confirmation");
    await expect(confirmation).toBeVisible();

    // While the question is open, the session comes back to life from
    // outside the prompt, through a restart that resumes it.
    const restarted = await request.post(`/api/sessions/${sourceId}/restart`, { data: {} });
    expect(restarted.ok(), `restarting the source: ${await restarted.text()}`).toBe(true);
    // And the list has shown it: code that looked the row up again when the
    // answer arrived would now find it alive and send no precondition.
    await expect(exitedBadge).toHaveCount(0, { timeout: 20_000 });

    const [answered] = await Promise.all([
      page.waitForRequest(
        (candidate) => candidate.method() === "POST" && candidate.url().endsWith(`/api/sessions/${sourceId}/replace`),
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
