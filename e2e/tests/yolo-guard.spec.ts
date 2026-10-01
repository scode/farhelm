// The GUI half of the YOLO guard (SPEC.md, Sessions → Creation): a YOLO
// launch on a host marked sensitive is refused by the helm, the create form
// answers the refusal with a confirmation instead of an error, cancelling
// starts nothing, and confirming retries the same launch with the override.
// A host marked safe launches without asking.
//
// The helm's own tests pin the refusal and the override on the wire. What
// only a browser can show is that the page reads the refusal as a question
// (from the helm's header, not from the text) and that the confirmed retry
// really carries the override the helm reads; a UI that dropped it would
// loop on the confirmation forever, and one that skipped the question would
// defeat the guard.
import { type Locator, type Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import {
  cleanupSession,
  listHosts,
  listSessions,
  localHostId,
  patchPreferences,
  setLocalYoloSafe,
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

test("a YOLO launch on a sensitive host asks first, and starts only when confirmed", async ({ page, request }) => {
  const cwd = stackScratchDir("yolo-guard-");
  const created: string[] = [];
  try {
    // Premise, made true rather than assumed: the suite shares one helm,
    // and a spec that marks the host safe puts it back, but say so here.
    await setLocalYoloSafe(request, false);
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
    expect(refused.status(), "the helm must refuse a YOLO launch on a sensitive host").toBe(409);
    await expect(confirmation).toBeVisible();
    // The GUI's own explanation: what YOLO means, why this launch is one,
    // and that nothing started. The helm's sentence is for the command line
    // and is not shown.
    await expect(confirmation).toContainText("Confirm YOLO launch");
    await expect(confirmation).toContainText("YOLO means the agent runs with no approval prompts");
    await expect(confirmation).toContainText("This launch uses YOLO permissions.");
    await expect(confirmation).toContainText("asks before every YOLO launch. Nothing has been started.");
    await expect(confirmation).not.toContainText("--allow-yolo-on-sensitive-host");
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
    expect(session.launch).toMatchObject({ harness: "codex", permissions: "yolo" });
    await expect(form, "a successful launch closes the composer").toHaveCount(0);
    const bodies = createPosts.map((body) => JSON.parse(body));
    expect(bodies.map((body) => body.allow_yolo_on_sensitive_host ?? false)).toEqual([false, false, false, true]);

    // Once the host is marked safe, the same launch does not ask.
    await setLocalYoloSafe(request, true);
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
    await setLocalYoloSafe(request, false);
    // The confirmed launches leave YOLO remembered helm-wide; later specs
    // on the shared helm expect a fresh composer to start on "default".
    await patchPreferences(request, { remembered_permissions: null });
    for (const id of created) await cleanupSession(request, id);
  }
});

/**
 * "Start, and don't ask again on this host" marks the host safe for YOLO
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
    await setLocalYoloSafe(request, false);
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
    await page.route(`**/api/hosts/${local}/yolo-safe`, async (route) => {
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

    // A refused mark: its reason in the confirmation, which stays up, and no create.
    await confirmation.locator(".yolo-confirm-stop-asking").click();
    await expect(confirmation.locator(".yolo-confirmation-error")).toContainText("could not stop asking for");
    await expect(confirmation.locator(".yolo-confirmation-error")).toContainText("held by the test");
    expect(markPosts).toEqual([{ yolo_safe: true }]);
    expect(createPosts, "a failed mark sends no create").toHaveLength(1);
    expect((await listHosts(request)).find((host) => host.id === local)?.yolo_safe).toBe(false);
    // The pressed button was disabled while the mark ran; focus comes back
    // to the question's safe answer rather than staying lost.
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
    expect((await listHosts(request)).find((host) => host.id === local)?.yolo_safe).toBe(true);
    const bodies = createPosts.map((body) => JSON.parse(body));
    expect(bodies.map((body) => body.allow_yolo_on_sensitive_host ?? false)).toEqual([false, false, true]);
  } finally {
    await setLocalYoloSafe(request, false);
    await patchPreferences(request, { remembered_permissions: null });
    for (const id of created) await cleanupSession(request, id);
  }
});
