// The helm-wide "last permissions used" memory (SPEC.md's launch-composer
// carve-out): the permissions mode of the last successful STRUCTURED launch
// preselects the segment on every client's next "New" open, "reset choices"
// returns to that remembered value rather than to "default", and a
// following structured launch with default permissions clears it again.
//
// This is a fresh spec file rather than an addition to sidebar.spec.ts or
// clone.spec.ts on purpose: those files' composer tests intentionally
// exercise unrelated mechanics (search, reconciliation, clone reseeding) and
// this feature's own contract — what a FRESH, unprefilled dialog preselects,
// across two real launches — reads clearest as its own linear story with
// the shared stack's memory cleared at its own start.
import { expect, test } from "./helpers/evidence";
import { cleanupSession, patchPreferences, readPreferences } from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";

test("the remembered structured-launch permissions mode survives an open, a reset, and a later default launch", async ({
  page,
  request,
}) => {
  const cwd = stackScratchDir("remembered-permissions-");
  const created: string[] = [];
  try {
    // Premise, MADE true rather than assumed: every project of one
    // `playwright test` invocation shares one helm, so an earlier spec's
    // real yolo launch (clone.spec.ts makes one) may already be remembered
    // here. Clear the memory BEFORE the page loads — the client reads the
    // preference row once, at load — then confirm the very first "New"
    // open, before this test has launched anything, preselects "default".
    await patchPreferences(request, { remembered_permissions: null });
    await page.goto("/");
    const form = page.locator(".create-session-form");
    const permissions = () => form.locator(".launch-composer-permissions-choice");

    await page.locator(".new-session-button").click();
    await expect(form).toBeVisible();
    expect(
      (await readPreferences(request)).remembered_permissions,
      "the fixture premise: nothing remembered before this test's first launch",
    ).toBeUndefined();
    await expect(permissions().getByRole("button", { name: "default", exact: true })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    // A successful structured launch with yolo.
    await form.getByLabel("folder", { exact: true }).fill(cwd);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await permissions().getByRole("button", { name: "yolo", exact: true }).click();
    const [yoloResponse] = await Promise.all([
      page.waitForResponse(
        (candidate) => candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions"),
      ),
      form.locator(".create-session-submit").click(),
    ]);
    expect(yoloResponse.ok(), `the yolo launch must be admitted: ${await yoloResponse.text()}`).toBe(true);
    const yoloSession = await yoloResponse.json();
    created.push(yoloSession.id);
    expect(yoloSession.launch).toMatchObject({ harness: "codex", permissions: "yolo" });
    await expect(form, "a successful launch closes the composer").toHaveCount(0);
    await expect
      .poll(async () => (await readPreferences(request)).remembered_permissions, {
        message: "the helm must remember the launched permissions choice",
      })
      .toBe("yolo");

    // Reopening New — a fresh dialog, no clone/replace prefill — preselects
    // the remembered mode instead of always starting on "default".
    await page.locator(".new-session-button").click();
    await expect(form).toBeVisible();
    await expect(permissions().getByRole("button", { name: "yolo", exact: true })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    // "Reset choices" returns the segment to the REMEMBERED value, not to
    // "default", and does not itself clear the memory. Clicked before any
    // harness is chosen: reset also clears harness/model/effort back to
    // unpreselected (SPEC.md's harness/model rule, unchanged by this
    // feature), so choosing Codex AFTER reset is what proves the segment's
    // own remembered value survived that same reset independently.
    await form.getByRole("button", { name: "reset choices", exact: true }).click();
    await expect(permissions().getByRole("button", { name: "yolo", exact: true })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(
      (await readPreferences(request)).remembered_permissions,
      "reset choices must not itself write anything",
    ).toBe("yolo");

    // A following structured launch with explicit default permissions
    // clears the memory again.
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await form.getByLabel("folder", { exact: true }).fill(cwd);
    await permissions().getByRole("button", { name: "default", exact: true }).click();
    const [defaultResponse] = await Promise.all([
      page.waitForResponse(
        (candidate) => candidate.request().method() === "POST" && candidate.url().endsWith("/api/sessions"),
      ),
      form.locator(".create-session-submit").click(),
    ]);
    expect(defaultResponse.ok(), `the default-permissions launch must be admitted: ${await defaultResponse.text()}`).toBe(true);
    const defaultSession = await defaultResponse.json();
    created.push(defaultSession.id);
    expect(defaultSession.launch).toMatchObject({ harness: "codex", permissions: null });
    await expect(form, "a successful launch closes the composer").toHaveCount(0);
    await expect
      .poll(async () => (await readPreferences(request)).remembered_permissions, {
        message: "a structured launch with default permissions must clear the remembered mode",
      })
      .toBeUndefined();

    // Reopening New once more preselects "default" again.
    await page.locator(".new-session-button").click();
    await expect(form).toBeVisible();
    await expect(permissions().getByRole("button", { name: "default", exact: true })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  } finally {
    for (const id of created) await cleanupSession(request, id);
  }
});
