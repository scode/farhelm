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
import { expect, test } from "./helpers/evidence";
import { cleanupSession, listSessions, patchPreferences, setLocalYoloSafe } from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";

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
    await expect(confirmation).toContainText("YOLO launch on a sensitive host");
    await expect(confirmation).toContainText("--allow-yolo-on-sensitive-host");
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
