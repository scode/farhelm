// Clone's browser contract: the "clone" menu item opens the create form
// pre-filled from the clicked row (crates/farhelm-ui/src/list/row.rs's
// `.session-row-clone`, `create_form::CreatePrefill`), the prefill reflects
// the row's own agent — a profile id when the row was created from one
// still `Present` in the catalog, the raw invocation otherwise. A selected
// profile displays its own invocation while the raw value remains the seed
// for custom mode. Submitting the edited copy creates a SEPARATE session
// while leaving the cloned row exactly as it was. Also covers an archived
// row, since clone is deliberately offered there: it is the one way an
// archived session gets a fresh, running agent again without touching the
// archived original (row.rs's `row_control_visibility`).

import { expect, Locator, Page, test } from "@playwright/test";
import {
  cleanupProfile,
  cleanupSession,
  createProfile,
  createSession,
  FAKE_AGENT,
  localHostId,
  openFilterBar,
  openRowMenu,
  type SessionRow,
} from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";

/** Find one session by its opaque server id, independent of title changes. */
function row(page: Page, id: string) {
  return page.locator(`.session-row[data-session-id="${id}"]`);
}

/**
 * Fill the create form's changed working directory and submit it, waiting
 * on the real `POST /api/sessions` response for the new session's id.
 *
 * Kept separate from the create-form fill sequence in `real-agent.spec.ts`'s
 * own helper: that one fills every field from scratch for an ordinary
 * create, while a clone's whole point is that only ONE field — the
 * directory — needs touching, with the rest already carrying the row's own
 * values into the request.
 */
async function submitClonedCwd(page: Page, form: Locator, cwd: string) {
  await form.locator('input[type="text"]').nth(0).fill(cwd);
  const [response] = await Promise.all([
    page.waitForResponse(
      (r) => r.request().method() === "POST" && r.url().endsWith("/api/sessions"),
    ),
    form.locator(".create-session-submit").click(),
  ]);
  const body = await response.json();
  return body.id as string;
}

test("clone pre-fills the create form from a profile-backed row, and the edited copy leaves the original untouched", async ({
  page,
  request,
}) => {
  const local = await localHostId(request);
  const invocationA = FAKE_AGENT;
  const invocationB = FAKE_AGENT.replace("--script basic", "--script altscreen");
  const profileA = await createProfile(request, local, {
    name: `clone-profile-a-${Date.now()}`,
    invocation: invocationA,
  });
  const profileB = await createProfile(request, local, {
    name: `clone-profile-b-${Date.now()}`,
    invocation: invocationB,
  });
  const title = `clone-source-${Date.now()}`;
  const originalCwd = "/tmp";
  const original = await createSession(request, {
    title,
    cwd: originalCwd,
    profile_id: profileA.id,
  });
  // A THROWAWAY session created from a DIFFERENT profile afterwards, purely
  // to move the host's remembered default onto B while the row this test
  // clones still names A. Without this, "the row's own profile" and "the
  // host's remembered default" would be the same id, and a broken
  // implementation that discards the clone's own choice and seeds only
  // from the remembered default would select the identical profile — the
  // exact regression this fixture exists to distinguish from the correct
  // behavior.
  const rememberedDefaultShift = await createSession(request, {
    title: `clone-remembered-default-${Date.now()}`,
    cwd: "/tmp",
    profile_id: profileB.id,
  });
  let cloneId: string | undefined;
  try {
    await page.goto("/");
    const source = row(page, original.id);
    await expect(source).toBeVisible({ timeout: 20_000 });

    await openRowMenu(source);
    await source.locator(".session-row-clone").click();

    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator('input[type="text"]').nth(0)).toHaveValue(originalCwd);
    await expect(form.locator('input[type="text"]').nth(2)).toHaveValue(title);
    // The row's OWN profile (A) wins the picker over the host's remembered
    // default (B, moved there by the throwaway session above) — see that
    // fixture's own comment for why the two must differ for this
    // assertion to mean anything.
    await expect(form.locator(".create-session-profile")).toHaveValue(profileA.id, {
      timeout: 20_000,
    });
    const command = form.locator('input[type="text"]').nth(1);
    // The label is located from the input upward: a `has` filter rooted at the
    // form can never match, because the inner locator would be re-rooted at
    // each candidate label and the form is not inside its own label.
    const commandLabel = command.locator("xpath=..");
    await expect(command).toBeDisabled();
    await expect(command).toHaveValue(invocationA);
    await expect(commandLabel).toContainText(
      'agent command (the selected profile\'s own; choose "custom command" above to edit)',
    );

    await form.locator(".create-session-profile").selectOption(profileB.id);
    await expect(command).toHaveValue(invocationB);
    await expect(command).toBeDisabled();

    await form.locator(".create-session-profile").selectOption("");
    await expect(command).toBeEnabled();
    await expect(command).toHaveValue(invocationA);
    await expect(commandLabel).toHaveText("agent command");

    // Keep the original profile-backed submit assertion below meaningful after
    // the custom-mode display assertions above.
    await form.locator(".create-session-profile").selectOption(profileA.id);

    const newCwd = stackScratchDir("clone-e2e-");
    await form.locator('input[type="text"]').nth(0).fill(newCwd);
    const [response] = await Promise.all([
      page.waitForResponse(
        (r) => r.request().method() === "POST" && r.url().endsWith("/api/sessions"),
      ),
      form.locator(".create-session-submit").click(),
    ]);
    // The wire body itself, not just what the picker showed — the two
    // could disagree if the reseed effect wrote the picker's display
    // without also writing what submit actually reads.
    expect(response.request().postDataJSON().profile_id).toBe(profileA.id);
    const body = await response.json();
    cloneId = body.id as string;

    const cloned = row(page, cloneId);
    await expect(cloned).toBeVisible({ timeout: 20_000 });
    await expect(cloned.locator(".session-title")).toHaveText(title);
    await expect(cloned.locator(".session-cwd")).toHaveAttribute("title", newCwd);

    // The original row: same directory, same title, still there — cloning
    // must not have touched it.
    await expect(source).toBeVisible();
    await expect(source.locator(".session-cwd")).toHaveAttribute("title", originalCwd);
    await expect(source.locator(".session-title")).toHaveText(title);
  } finally {
    if (cloneId) await cleanupSession(request, cloneId);
    await cleanupSession(request, rememberedDefaultShift.id);
    await cleanupSession(request, original.id);
    await cleanupProfile(request, local, profileA.id);
    await cleanupProfile(request, local, profileB.id);
  }
});

test("clone reaches an archived row, pre-filling its raw invocation, and the archived original stays archived", async ({
  page,
  request,
}) => {
  const title = `clone-archived-${Date.now()}`;
  const cwd = "/tmp";
  const session = await createSession(request, { title, cwd });
  let cloneId: string | undefined;
  try {
    await page.goto("/");
    const target = row(page, session.id);
    await expect(target).toBeVisible({ timeout: 20_000 });

    // Archive it through the real UI flow (archive.spec.ts's own pattern),
    // so the row this test clones is genuinely in the state clone is
    // offered for, not a fixture standing in for it.
    await openRowMenu(target);
    await target.locator(".session-row-archive").click();
    await target.locator(".confirm-archive").click();
    await expect(target).toHaveCount(0, { timeout: 20_000 });

    await openFilterBar(page);
    await page.locator(".filter-include-archived").check();
    const archived = row(page, session.id);
    await expect(archived).toBeVisible({ timeout: 20_000 });
    await expect(archived).toHaveAttribute("data-session-archived", "true");

    await openRowMenu(archived);
    // The point of the coverage: stop and archive are gone from an
    // archived row's menu, but clone is not.
    await expect(archived.locator(".session-row-stop")).toHaveCount(0);
    await expect(archived.locator(".session-row-archive")).toHaveCount(0);
    await archived.locator(".session-row-clone").click();

    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator('input[type="text"]').nth(0)).toHaveValue(cwd);
    await expect(form.locator('input[type="text"]').nth(2)).toHaveValue(title);
    // No profile on this session, so the picker reflects the raw-command
    // path and the invocation itself carries into the command field —
    // the fallback `PrefillAgent` takes when there is no profile to trust.
    await expect(form.locator(".create-session-profile")).toHaveValue("");
    await expect(form.locator('input[type="text"]').nth(1)).toHaveValue(FAKE_AGENT);

    const newCwd = stackScratchDir("clone-archived-e2e-");
    cloneId = await submitClonedCwd(page, form, newCwd);

    const cloned = row(page, cloneId);
    await expect(cloned).toBeVisible({ timeout: 20_000 });
    await expect(cloned).not.toHaveAttribute("data-session-archived", "true");
    await expect(cloned.locator(".session-title")).toHaveText(title);
    await expect(cloned.locator(".session-cwd")).toHaveAttribute("title", newCwd);

    // The archived original: still archived, still at its own directory —
    // cloning it must not have resurrected or otherwise touched it.
    await expect(archived).toBeVisible();
    await expect(archived).toHaveAttribute("data-session-archived", "true");
    await expect(archived.locator(".session-cwd")).toHaveAttribute("title", cwd);
  } finally {
    if (cloneId) await cleanupSession(request, cloneId);
    await cleanupSession(request, session.id);
  }
});

/**
 * The generation latch that lets an already-open form reseed (`create_form
 * .rs`'s `prefill_applied`) proven behaviorally: cloning row B while the
 * form opened by row A's clone is still on screen must replace EVERY
 * field, not merely the ones B's own values happen to differ on, and
 * cloning the SAME row a second time must restore the full prefill rather
 * than being a silent no-op because a prefill was already showing.
 *
 * The pure `is_fresh_clone`-style unit coverage this used to rely on only
 * proved the generation COMPARISON, never that Dioxus actually reruns the
 * effect and repaints every signal without unmounting the form — this is
 * the real regression for a broken generation bump, a missed reactive
 * dependency, or a reseed that only overwrote some of the fields.
 */
test("cloning a second row without closing the form replaces every field, and re-cloning it restores that", async ({
  page,
  request,
}) => {
  const local = await localHostId(request);
  const titleA = `clone-reseed-a-${Date.now()}`;
  const titleB = `clone-reseed-b-${Date.now()}`;
  // Allocated inside the stack's own state dir rather than a literal
  // `/tmp/...` path: a clean runner never creates that directory, and
  // farhelm's create precondition would refuse the fixture session before
  // this test ever reached the browser.
  let cwdA: string | undefined;
  let cwdB: string | undefined;
  let sessionA: SessionRow | undefined;
  let sessionB: SessionRow | undefined;
  let cloneId: string | undefined;
  try {
    cwdA = stackScratchDir("clone-reseed-a-");
    sessionA = await createSession(request, { title: titleA, cwd: cwdA });
    cwdB = stackScratchDir("clone-reseed-b-");
    sessionB = await createSession(request, { title: titleB, cwd: cwdB });

    await page.goto("/");
    const rowA = row(page, sessionA.id);
    const rowB = row(page, sessionB.id);
    await expect(rowA).toBeVisible({ timeout: 20_000 });
    await expect(rowB).toBeVisible({ timeout: 20_000 });

    await openRowMenu(rowA);
    await rowA.locator(".session-row-clone").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-host")).toHaveValue(String(local));
    await expect(form.locator('input[type="text"]').nth(0)).toHaveValue(cwdA);
    await expect(form.locator('input[type="text"]').nth(2)).toHaveValue(titleA);
    await expect(form.locator('input[type="text"]').nth(1)).toHaveValue(FAKE_AGENT);

    // Edit every field before cloning again — a partial reseed (one field
    // replaced, another left at this edit) would otherwise be invisible
    // to an assertion that only checked the fields B's clone changes.
    await form.locator('input[type="text"]').nth(0).fill("/tmp/edited-in-between");
    await form.locator('input[type="text"]').nth(1).fill("sleep 999");
    await form.locator('input[type="text"]').nth(2).fill("edited in between");

    // Clone row B WITHOUT closing the form: a new generation must reseed
    // the still-mounted form wholesale.
    await openRowMenu(rowB);
    await rowB.locator(".session-row-clone").click();
    await expect(form.locator(".create-session-host")).toHaveValue(String(local));
    await expect(form.locator('input[type="text"]').nth(0)).toHaveValue(cwdB);
    await expect(form.locator('input[type="text"]').nth(2)).toHaveValue(titleB);
    await expect(form.locator('input[type="text"]').nth(1)).toHaveValue(FAKE_AGENT);

    // Edit again, then clone B a SECOND time — a same-row reclone, which
    // still bumps the generation and must restore the full prefill rather
    // than being a no-op because B's own prefill was already on screen.
    await form.locator('input[type="text"]').nth(0).fill("/tmp/edited-again");
    await form.locator('input[type="text"]').nth(2).fill("edited again");
    await openRowMenu(rowB);
    await rowB.locator(".session-row-clone").click();
    await expect(form.locator('input[type="text"]').nth(0)).toHaveValue(cwdB);
    await expect(form.locator('input[type="text"]').nth(2)).toHaveValue(titleB);

    const newCwd = stackScratchDir("clone-reseed-e2e-");
    cloneId = await submitClonedCwd(page, form, newCwd);
    await expect(row(page, cloneId)).toBeVisible({ timeout: 20_000 });
  } finally {
    if (cloneId) await cleanupSession(request, cloneId);
    if (sessionA) await cleanupSession(request, sessionA.id);
    if (sessionB) await cleanupSession(request, sessionB.id);
  }
});

/**
 * `clone_prefill` is stored above the create form so it survives while the
 * form stays open (`list::view::ListView`), which leaves it exactly two
 * cleanup paths: cancelling through the "new session" toggle, and a
 * successful create. Either one skipped, or ordered wrong, would let the
 * NEXT unrelated "new session" open silently inherit a clone's host,
 * directory, title, and agent while looking like an ordinary fresh create.
 */
test("closing a clone without submitting, or submitting it, both leave the next New Session with fresh defaults", async ({
  page,
  request,
}) => {
  const local = await localHostId(request);
  const title = `clone-prefill-cleanup-${Date.now()}`;
  const newSessionButton = page.locator(".new-session-button");
  let sourceCwd: string | undefined;
  let session: SessionRow | undefined;
  let cloneId: string | undefined;
  try {
    // Allocated inside the stack's own state dir, like every other fixture
    // directory in this file — a literal `/tmp/...` path is never created
    // for the test, so the source session's create would be refused before
    // the browser had anything to clone.
    sourceCwd = stackScratchDir("clone-prefill-cleanup-");
    session = await createSession(request, { title, cwd: sourceCwd });

    await page.goto("/");
    const source = row(page, session.id);
    await expect(source).toBeVisible({ timeout: 20_000 });

    const form = page.locator(".create-session-form");
    const assertFreshDefaults = async () => {
      await expect(form).toBeVisible();
      await expect(form.locator(".create-session-host")).toHaveValue(String(local));
      await expect(form.locator('input[type="text"]').nth(0)).toHaveValue("~");
      await expect(form.locator('input[type="text"]').nth(1)).toHaveValue("");
      await expect(form.locator('input[type="text"]').nth(2)).toHaveValue("");
    };

    // (a) Clone, then cancel through the "new session" toggle — the one
    // path that actually clears `clone_prefill` on a form the user backs
    // out of. Reopening must show the ordinary blank-form defaults, not
    // the cancelled clone's fields.
    await openRowMenu(source);
    await source.locator(".session-row-clone").click();
    await expect(form).toBeVisible();
    await expect(form.locator('input[type="text"]').nth(0)).toHaveValue(sourceCwd);
    await newSessionButton.click();
    await expect(form).toHaveCount(0);
    await newSessionButton.click();
    await assertFreshDefaults();
    await newSessionButton.click();
    await expect(form).toHaveCount(0);

    // (b) Clone, submit it successfully, then reopen: the same fresh
    // defaults, not the just-submitted clone's fields either — the other
    // path that clears `clone_prefill` (`on_created`).
    await openRowMenu(source);
    await source.locator(".session-row-clone").click();
    await expect(form).toBeVisible();
    const newCwd = stackScratchDir("clone-prefill-cleanup-e2e-");
    cloneId = await submitClonedCwd(page, form, newCwd);
    await expect(row(page, cloneId)).toBeVisible({ timeout: 20_000 });

    await newSessionButton.click();
    await assertFreshDefaults();
  } finally {
    if (cloneId) await cleanupSession(request, cloneId);
    if (session) await cleanupSession(request, session.id);
  }
});
