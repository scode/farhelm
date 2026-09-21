/** Real checkout acceptance: every API, clone, hook and terminal is live.
 * Only the vendor agent is fake. An exact private Git URL rewrite and a
 * file-only transport policy keep repository traffic inside the owned stack. */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Locator, Page } from "@playwright/test";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { cleanupProfile, cleanupSession, createProfile, createSession, FAKE_AGENT, localHostId } from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";
import { attachSession, waitForTermText } from "./helpers/term";

const run = promisify(execFile);

/** Compare every owned byte, link and directory across an archival rename.
 * Git metadata and untracked work are included: checking only the tracked
 * sentinel would miss a destructive re-clone or partial copy. */
async function contents(root: string): Promise<Record<string, string>> {
  const result: Record<string, string> = {};
  async function visit(directory: string) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      const absolute = path.join(directory, entry.name);
      const relative = path.relative(root, absolute);
      if (entry.isDirectory()) {
        result[relative] = "directory";
        await visit(absolute);
      } else if (entry.isSymbolicLink()) {
        result[relative] = `link:${await fs.readlink(absolute)}`;
      } else {
        expect(entry.isFile(), `unexpected fixture entry ${relative}`).toBe(true);
        result[relative] = `file:${(await fs.readFile(absolute)).toString("base64")}`;
      }
    }
  }
  await visit(root);
  return result;
}

/** Require a new echo through this session's own attached terminal. */
async function assertLive(page: Page, id: string) {
  // API-driven deletion can leave the page with no selected row. Start a
  // new attachment lifecycle instead of inheriting that deleted selection.
  await page.goto("/");
  await attachSession(page, id);
  await waitForTermText(page, "FAKE-AGENT READY", 20_000);
  const probe = `checkout-live-${id}-${Date.now()}`;
  await page.keyboard.type(probe);
  await page.keyboard.press("Enter");
  await waitForTermText(page, `echo:${probe}`, 20_000);
}

/** Read durable membership independently from an admission response. */
async function sessionState(request: APIRequestContext, id: string) {
  const response = await request.get(`/api/sessions/${id}`);
  expect(response.ok(), await response.text()).toBe(true);
  return response.json();
}

/** A test action must succeed; unlike cleanup, an absent source is a failure. */
async function deleteSession(request: APIRequestContext, id: string) {
  const response = await request.delete(`/api/sessions/${id}`);
  expect(response.ok(), await response.text()).toBe(true);
  expect((await request.get(`/api/sessions/${id}`)).status()).toBe(404);
}

/** Own the source and destination until all sessions have released them.
 * The stack runs one browser test at a time, so its private Git config can
 * be restored exactly without coordinating with another test's clone. */
async function checkoutFixture(repoName: string) {
  const stack = JSON.parse(await fs.readFile(path.resolve(__dirname, "../.stack-info.json"), "utf8"));
  const scratch = stackScratchDir("github-browser-");
  const root = path.join(scratch, "checkouts");
  const seed = path.join(scratch, "seed");
  const bare = path.join(scratch, "source.git");
  const repo = `fixture/${repoName}`;
  const url = `https://github.com/${repo}.git`;
  const originalConfig = await fs.readFile(stack.checkout_git_config);
  // Inherited Git overrides must not redirect fixture commands to an operator
  // repository. This is a child environment, never process.env mutation.
  const env = Object.fromEntries(Object.entries(process.env).filter(([name]) =>
    !name.startsWith("GIT_") && !name.startsWith("FARHELM_")
  ));
  Object.assign(env, {
    HOME: stack.structured_home, GIT_CONFIG_GLOBAL: stack.checkout_git_config,
    GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_SYSTEM: "/dev/null", GIT_CONFIG_COUNT: "0",
    GIT_ALLOW_PROTOCOL: "file", GIT_TERMINAL_PROMPT: "0",
  });
  const git = async (...args: string[]) => (await run("git", args, { env, timeout: 10_000 })).stdout.trim();
  const config = async (...args: string[]) => {
    await run(stack.farhelm, ["helm", "checkout-config", ...args, "--state-dir", stack.state],
      { env, timeout: 10_000 });
  };
  await fs.mkdir(root);
  await fs.mkdir(seed);
  await git("init", "--initial-branch=main", seed);
  await fs.writeFile(path.join(seed, "checkout-sentinel"), `${repo}\n`);
  await git("-C", seed, "add", "checkout-sentinel");
  await git("-C", seed, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
    "-c", "commit.gpgSign=false", "commit", "-m", "fixture");
  const commit = await git("-C", seed, "rev-parse", "HEAD");
  await git("clone", "--bare", seed, bare);
  expect(await git("--git-dir", bare, "rev-parse", "--is-bare-repository")).toBe("true");
  await git("config", "--file", stack.checkout_git_config, `url.${pathToFileURL(bare).href}.insteadOf`, url);
  expect(await git("ls-remote", url, "HEAD")).toBe(`${commit}\tHEAD`);
  await config("set-root", root);
  await config("set-post-clone", "test -f checkout-sentinel && printf 'CHECKOUT-HOOK-READY\\n' && printf 'once\\n' >> hook-count");
  return {
    root, repo, repoName, url, commit, git,
    /** Call only after cleanupSession: Delete must archive intact content
     * while both the original root and its recorded identity still exist. */
    async close() {
      await config("clear-post-clone");
      await config("clear-root");
      await fs.writeFile(stack.checkout_git_config, originalConfig);
      await fs.rm(scratch, { recursive: true, force: true });
    },
  };
}

/** Opening a fresh dialog has no accepted destination or agent. */
async function openComposer(page: Page, host: number): Promise<Locator> {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator('.create-session-form[role="dialog"]');
  await expect(form).toBeVisible();
  await expect(form.locator(".create-session-host")).toHaveValue(String(host));
  return form;
}

/** Explicitly select fresh intent through the user's scoped search action. */
async function selectRepo(form: Locator, repo: string) {
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill(`gh:${repo}`);
  const option = form.getByRole("option", { name: `Fresh checkout: ${repo}`, exact: true });
  await expect(option).toHaveAttribute("aria-selected", "true");
  await search.press("Enter");
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();
}

/** A successful create response is only admission; callers separately prove
 * the persisted session and attach to its own live terminal. */
async function launch(page: Page, form: Locator, ids: string[]) {
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  const [response] = await Promise.all([
    page.waitForResponse((r) => r.request().method() === "POST" && new URL(r.url()).pathname === "/api/sessions"),
    form.locator(".create-session-submit").click(),
  ]);
  expect(response.ok(), await response.text()).toBe(true);
  const session = await response.json();
  ids.push(session.id);
  return { session, body: response.request().postDataJSON() };
}

/** Read server state and terminal output independently of the create reply.
 * The hook requires the tracked file, and the agent banner can appear only
 * after preparation; file and commit checks prove that a real clone ran. */
async function assertCheckout(
  page: Page, request: APIRequestContext, fixture: Awaited<ReturnType<typeof checkoutFixture>>,
  id: string, cwd: string, structured: boolean,
) {
  const response = await request.get(`/api/sessions/${id}`);
  expect(response.ok(), await response.text()).toBe(true);
  const session = await response.json();
  expect(session.cwd).toBe(cwd);
  expect(session.github_repo).toEqual({ owner: "fixture", name: fixture.repoName });
  expect(session.working_copy).toBeTruthy();
  await attachSession(page, id);
  await waitForTermText(page, "CHECKOUT-HOOK-READY", 20_000);
  if (structured) await waitForTermText(page, `STRUCTURED-LAUNCH-GENERATION:${id}:1`, 20_000);
  await waitForTermText(page, "FAKE-AGENT READY", 20_000);
  // A banner can be replayed from a dead terminal. This attachment must
  // also receive a fresh agent response before preparation counts as live.
  const probe = `checkout-live-${id}`;
  await page.keyboard.type(probe);
  await page.keyboard.press("Enter");
  await waitForTermText(page, `echo:${probe}`, 20_000);
  expect(await fs.readFile(path.join(cwd, "checkout-sentinel"), "utf8")).toBe(`${fixture.repo}\n`);
  expect(await fs.readFile(path.join(cwd, "hook-count"), "utf8")).toBe("once\n");
  expect(await fixture.git("-C", cwd, "rev-parse", "HEAD")).toBe(fixture.commit);
  expect(await fixture.git("-C", cwd, "config", "--local", "--get", "remote.origin.url")).toBe(fixture.url);
  return session;
}

/** Lose a real, durably refused response after a late path collision. The
 * same request must recover that refusal, refresh its offer and await another
 * click before allocating. Only delivery of the first reply is interrupted;
 * refusal, replay, the final clone and hook all use the assembled backend. */
test("lost durable refusal refreshes the preview before an explicit new launch", async ({ page, request }) => {
  const fixture = await checkoutFixture("browser-refusal");
  const ids: string[] = [];
  const bodies: { intent_key: string; github_checkout: { preview: { cwd: string } } }[] = [];
  let durableRefusals = 0;
  const occupied = path.join(fixture.root, `${fixture.repoName}-1`);
  const next = path.join(fixture.root, `${fixture.repoName}-2`);
  try {
    const host = await localHostId(request);
    const form = await openComposer(page, host);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await selectRepo(form, fixture.repo);
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText(occupied);
    expect(await fs.readdir(fixture.root), "the accepted preview must precede the collision").toEqual([]);
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      bodies.push(route.request().postDataJSON());
      if (bodies.length === 1) {
        // Occupy the exact offer only after the browser dispatched its frozen
        // request. The backend must refuse rather than choose a new path.
        await fs.mkdir(occupied);
        await fs.writeFile(path.join(occupied, "foreign"), "preserve this collision\n");
      }
      const response = await route.fetch();
      if (bodies.length <= 2) {
        expect(response.status(), await response.text()).toBe(409);
        expect(response.headers()["x-farhelm-create-outcome"]).toBe("definitely-unaccepted");
        durableRefusals += 1;
      } else {
        expect(response.ok(), await response.text()).toBe(true);
        ids.push((await response.json()).id);
      }
      if (bodies.length === 1) await route.abort("failed");
      else await route.fulfill({ response });
    });
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".create-session-error")).toContainText("original request is retained");
    expect(durableRefusals).toBe(1);
    expect(bodies).toHaveLength(1);
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".create-session-error")).toContainText("nothing was accepted");
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText(next);
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    expect(durableRefusals).toBe(2);
    expect(bodies).toHaveLength(2);
    expect(bodies[1]).toEqual(bodies[0]);
    expect(await fs.readdir(fixture.root), "refresh must not create the second checkout").toEqual([`${fixture.repoName}-1`]);
    const { session, body } = await launch(page, form, ids);
    expect(bodies).toHaveLength(3);
    expect(body.intent_key).not.toBe(bodies[0].intent_key);
    expect(body.github_checkout.preview.cwd).toBe(next);
    await assertCheckout(page, request, fixture, session.id, next, true);
    expect(await fs.readFile(path.join(occupied, "foreign"), "utf8")).toBe("preserve this collision\n");
  } finally {
    for (const id of new Set(ids.reverse())) await cleanupSession(request, id);
    await fixture.close();
  }
});

/** A name edit affects the preview only; saved setup reuse carries repo
 * intent into a second real allocation rather than borrowing the first cwd. */
test("structured checkout previews, launches, and reuses a recent as a fresh clone", async ({ page, request }) => {
  const fixture = await checkoutFixture("browser-structured");
  const ids: string[] = [];
  try {
    const host = await localHostId(request);
    let form = await openComposer(page, host);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await selectRepo(form, fixture.repo);
    const unnamed = path.join(fixture.root, `${fixture.repoName}-1`);
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText(unnamed);
    await form.getByLabel("name (optional)").fill("fix");
    const named = path.join(fixture.root, `${fixture.repoName}-fix`);
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText(named);
    expect(await fs.readdir(fixture.root), "preview must not allocate either offered path").toEqual([]);
    const first = await launch(page, form, ids);
    expect(first.body).toMatchObject({ cwd: named, title: "fix", launch: { harness: "codex" }, github_checkout: { repo: fixture.repo } });
    const persisted = await assertCheckout(page, request, fixture, first.session.id, named, true);
    expect(persisted.launch).toEqual(first.body.launch);

    form = await openComposer(page, host);
    const recent = form.locator(".launch-composer-recents").getByRole("button", { name: new RegExp(`gh:${fixture.repo}`) });
    await expect(recent).toBeVisible();
    await recent.click();
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText(unnamed);
    const second = await launch(page, form, ids);
    expect(second.session.id).not.toBe(first.session.id);
    expect(second.body.github_checkout.repo).toBe(fixture.repo);
    await assertCheckout(page, request, fixture, second.session.id, unnamed, true);
    expect((await fs.readdir(fixture.root)).sort()).toEqual([`${fixture.repoName}-1`, `${fixture.repoName}-fix`]);
  } finally {
    for (const id of ids.reverse()) await cleanupSession(request, id);
    await fixture.close();
  }
});

/** Command mode and profile mode retain their selector when gh is selected.
 * The same real clone/hook/live-terminal oracles cover both legacy branches. */
for (const mode of ["raw", "profile"] as const) {
  test(`${mode} agent choice survives a real fresh checkout`, async ({ page, request }) => {
    const fixture = await checkoutFixture(`browser-${mode}`);
    const ids: string[] = [];
    let profileId: string | undefined;
    try {
      const host = await localHostId(request);
      if (mode === "profile") profileId = (await createProfile(request, { name: `checkout-${mode}` })).id;
      const form = await openComposer(page, host);
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "other / command", exact: true }).click();
      await form.locator(".create-session-profile").selectOption(profileId ?? "");
      if (mode === "raw") await form.getByLabel("agent command").fill(FAKE_AGENT);
      await selectRepo(form, fixture.repo);
      await expect(form).toHaveAttribute("data-composer-mode", "command");
      const cwd = path.join(fixture.root, `${fixture.repoName}-1`);
      await expect(form.locator(".launch-composer-checkout-preview")).toContainText(cwd);
      expect(await fs.readdir(fixture.root)).toEqual([]);
      const created = await launch(page, form, ids);
      if (profileId) expect(created.body.profile_id).toBe(profileId);
      else expect(created.body.invocation).toBe(FAKE_AGENT);
      const persisted = await assertCheckout(page, request, fixture, created.session.id, cwd, false);
      expect(persisted.invocation).toBe(FAKE_AGENT);
      if (profileId) expect(persisted.source_profile.id).toBe(profileId);
    } finally {
      for (const id of ids.reverse()) await cleanupSession(request, id);
      if (profileId) await cleanupProfile(request, profileId);
      await fixture.close();
    }
  });
}

/** An unlabeled folder result explicitly leaves fresh mode and preserves
 * the agent selection. Ordinary launches must not run the configured hook
 * or acquire checkout ownership merely because a root is configured. */
test("an unlabeled folder result replaces fresh intent without cloning", async ({ page, request }) => {
  const fixture = await checkoutFixture("browser-folder");
  const ids: string[] = [];
  try {
    const host = await localHostId(request);
    const seed = await createSession(request, { title: "folder-history", cwd: fixture.root, host });
    ids.push(seed.id);
    const history = await request.get(`/api/launch-history?host=${host}`);
    expect(history.ok()).toBe(true);
    expect((await history.json()).folders.some((folder: any) => folder.display_cwd === fixture.root)).toBe(true);
    const form = await openComposer(page, host);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "other / command", exact: true }).click();
    await form.locator(".create-session-profile").selectOption("");
    await form.getByLabel("agent command").fill(FAKE_AGENT);
    await selectRepo(form, fixture.repo);
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText(`${fixture.repoName}-1`);
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill(path.basename(path.dirname(fixture.root)));
    await form.getByRole("option", { name: `Use this path: ${fixture.root}`, exact: true }).click();
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue(fixture.root);
    await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
    await expect(form).toHaveAttribute("data-composer-mode", "command");
    const created = await launch(page, form, ids);
    expect(created.body.github_checkout).toBeUndefined();
    expect(created.body.invocation).toBe(FAKE_AGENT);
    expect(created.session.cwd).toBe(fixture.root);
    expect(created.session.github_repo).toBeNull();
    expect(created.session.working_copy).toBeNull();
    await attachSession(page, created.session.id);
    await waitForTermText(page, "FAKE-AGENT READY", 20_000);
    expect(await fs.readdir(fixture.root), "an ordinary launch runs neither clone nor post-clone hook").toEqual([]);
  } finally {
    for (const id of ids.reverse()) await cleanupSession(request, id);
    await fixture.close();
  }
});

/** Exercise the last-reference rule with real processes and a real clone.
 * Stop retains a reference after ending its process. Only Delete of the
 * final borrower may move the complete checkout, including dirty work. */
test("borrowers retain the checkout until the final stopped session is deleted", async ({ page, request }) => {
  const fixture = await checkoutFixture("browser-lifetime");
  const ids: string[] = [];
  try {
    const host = await localHostId(request);
    const form = await openComposer(page, host);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "other / command", exact: true }).click();
    await form.locator(".create-session-profile").selectOption("");
    await form.getByLabel("agent command").fill(FAKE_AGENT);
    await selectRepo(form, fixture.repo);
    const { session: origin } = await launch(page, form, ids);
    const original = await assertCheckout(page, request, fixture, origin.id, origin.cwd, false);
    const identity = await fs.stat(origin.cwd);
    const subdir = path.join(origin.cwd, "dirty-subdir");
    await fs.mkdir(subdir);
    await fs.writeFile(path.join(subdir, "untracked"), "keep my uncommitted work\n");
    await fs.symlink("../checkout-sentinel", path.join(subdir, "sentinel-link"));
    const unmanaged = path.join(fixture.root, "unmanaged");
    await fs.mkdir(unmanaged);
    await fs.writeFile(path.join(unmanaged, "foreign"), "unmanaged content\n");
    const foreign = await createSession(request, { host, title: "unmanaged", cwd: unmanaged });
    ids.push(foreign.id);
    expect((await sessionState(request, foreign.id)).working_copy).toBeNull();
    await assertLive(page, foreign.id);
    await deleteSession(request, foreign.id);

    // Ordinary Clone must retain the actual cwd, not inherit fresh repo intent.
    await assertLive(page, origin.id);
    const sourceRow = page.locator(`.session-row[data-session-id="${origin.id}"]`);
    await sourceRow.locator(".session-row-menu").click();
    await sourceRow.locator(".session-row-clone").click();
    const cloneForm = page.locator('.create-session-form[role="dialog"]');
    await expect(cloneForm.getByLabel("folder", { exact: true })).toHaveValue(origin.cwd);
    await expect(cloneForm.locator(".launch-composer-checkout-preview")).toHaveCount(0);
    await cloneForm.getByLabel("name (optional)").fill("same-cwd-borrower");
    const cloned = await launch(page, cloneForm, ids);
    expect(cloned.body.github_checkout).toBeUndefined();
    const same = cloned.session;
    const nested = await createSession(request, { host, title: "nested-borrower", cwd: subdir });
    ids.push(nested.id);
    for (const borrower of [same, nested]) {
      const state = await sessionState(request, borrower.id);
      expect(state.github_repo).toBeNull();
      expect(state.working_copy).toEqual(original.working_copy);
      await assertLive(page, borrower.id);
    }
    const borrowerRow = page.locator(`.session-row[data-session-id="${nested.id}"]`);
    await borrowerRow.locator(".session-row-menu").click();
    await borrowerRow.locator(".session-row-delete").click();
    await expect(borrowerRow.locator(".confirm-checkout-consequence")).toHaveText(
      "The checkout stays while another session uses it. Deleting its last session moves it into the working-copy archive; no files are deleted.",
    );
    await borrowerRow.locator(".confirm-cancel").click();
    const before = await contents(origin.cwd);
    for (const id of [origin.id, same.id]) {
      await deleteSession(request, id);
      expect(await contents(origin.cwd)).toEqual(before);
      expect((await fs.stat(origin.cwd)).ino).toBe(identity.ino);
      expect((await fs.readdir(fixture.root)).sort()).toEqual([path.basename(origin.cwd), "unmanaged"].sort());
    }
    await assertLive(page, nested.id);
    const stopped = await request.post(`/api/sessions/${nested.id}/stop`);
    expect(stopped.ok(), await stopped.text()).toBe(true);
    const retained = await sessionState(request, nested.id);
    expect(retained.working_copy).toEqual(original.working_copy);
    expect(await contents(origin.cwd)).toEqual(before);
    await deleteSession(request, nested.id);
    await expect(fs.stat(origin.cwd)).rejects.toMatchObject({ code: "ENOENT" });
    const archive = path.join(fixture.root, "farhelm-archived-working-copies");
    const moved = await fs.readdir(archive);
    expect(moved).toHaveLength(1);
    const destination = path.join(archive, moved[0]);
    const movedIdentity = await fs.stat(destination);
    expect([movedIdentity.dev, movedIdentity.ino]).toEqual([identity.dev, identity.ino]);
    expect(await contents(destination)).toEqual(before);
    expect(await fs.readFile(path.join(unmanaged, "foreign"), "utf8")).toBe("unmanaged content\n");
  } finally {
    for (const id of ids.reverse()) await cleanupSession(request, id);
    await fixture.close();
  }
});

/** Replacement must register its successor before deleting a same-cwd source.
 * Fresh and different-directory overrides then exercise both nonfinal and
 * final release of the old checkout through the assembled helm/supervisor. */
test("replacement preserves borrowers and archives only the released checkout", async ({ page, request }) => {
  const fixture = await checkoutFixture("browser-replace");
  const ids: string[] = [];
  /** Keep actual response IDs for cleanup, then prove the old ID is gone. */
  async function replace(source: string, withBody?: Record<string, unknown>) {
    const response = await request.post(`/api/sessions/${source}/replace`, {
      data: { intent_key: `replace-${source}`, ...(withBody ? { with: withBody } : {}) },
    });
    expect(response.ok(), await response.text()).toBe(true);
    const successor = await response.json();
    ids.push(successor.id);
    expect(successor.id).not.toBe(source);
    expect((await request.get(`/api/sessions/${source}`)).status()).toBe(404);
    return successor;
  }
  try {
    const host = await localHostId(request);
    const form = await openComposer(page, host);
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "other / command", exact: true }).click();
    await form.locator(".create-session-profile").selectOption("");
    await form.getByLabel("agent command").fill(FAKE_AGENT);
    await selectRepo(form, fixture.repo);
    const { session: origin } = await launch(page, form, ids);
    const owned = await assertCheckout(page, request, fixture, origin.id, origin.cwd, false);
    const originalIdentity = await fs.stat(origin.cwd);
    const plain = await replace(origin.id);
    await assertLive(page, plain.id);
    expect(plain.cwd).toBe(origin.cwd);
    expect(plain.github_repo).toBeNull();
    expect(plain.working_copy).toEqual(owned.working_copy);

    const selection = { harness: "claude", effort: "medium" };
    const edited = await replace(plain.id, { host, cwd: origin.cwd, title: "changed-agent", launch: selection });
    await assertLive(page, edited.id);
    const editedState = await sessionState(request, edited.id);
    expect(editedState.launch).toEqual({ ...selection, model: null, permissions: null });
    expect(editedState.cwd).toBe(origin.cwd);
    expect(editedState.working_copy).toEqual(owned.working_copy);
    expect(editedState.github_repo).toBeNull();
    expect((await fs.stat(origin.cwd)).ino).toBe(originalIdentity.ino);
    expect(await fs.readFile(path.join(origin.cwd, "hook-count"), "utf8")).toBe("once\n");
    expect(await fs.readdir(fixture.root)).toEqual([path.basename(origin.cwd)]);

    const borrower = await createSession(request, { host, title: "replacement-borrower", cwd: origin.cwd });
    ids.push(borrower.id);
    expect((await sessionState(request, borrower.id)).working_copy).toEqual(owned.working_copy);
    await assertLive(page, borrower.id);
    const before = await contents(origin.cwd);
    const hosts = await request.get("/api/hosts");
    expect(hosts.ok(), await hosts.text()).toBe(true);
    const claim = (await hosts.json()).hosts.find((row: { id: number }) => row.id === host);
    expect(claim.state.phase).toBe("connected");
    const previewResponse = await request.post("/api/github-checkout-preview", {
      data: { host, expected_incarnation: claim.incarnation, repo: fixture.repo, title: "second" },
    });
    expect(previewResponse.ok(), await previewResponse.text()).toBe(true);
    const preview = await previewResponse.json();
    expect(preview.cwd).toBe(path.join(fixture.root, `${fixture.repoName}-second`));
    await expect(fs.stat(preview.cwd)).rejects.toMatchObject({ code: "ENOENT" });
    const fresh = await replace(edited.id, {
      host, expected_incarnation: claim.incarnation, cwd: preview.cwd, title: "second", invocation: FAKE_AGENT,
      github_checkout: { repo: fixture.repo, title: "second", preview },
    });
    await page.goto("/");
    const freshState = await assertCheckout(page, request, fixture, fresh.id, preview.cwd, false);
    expect(freshState.working_copy.id).not.toBe(owned.working_copy.id);
    expect(await contents(origin.cwd)).toEqual(before);
    expect((await fs.readdir(fixture.root)).sort()).toEqual([path.basename(origin.cwd), path.basename(fresh.cwd)].sort());
    await assertLive(page, borrower.id);
    await deleteSession(request, borrower.id);
    await expect(fs.stat(origin.cwd)).rejects.toMatchObject({ code: "ENOENT" });
    const archive = path.join(fixture.root, "farhelm-archived-working-copies");
    const firstArchive = await fs.readdir(archive);
    expect(firstArchive).toHaveLength(1);
    expect(await contents(path.join(archive, firstArchive[0]))).toEqual(before);

    const unmanaged = path.join(fixture.root, "unmanaged-replacement");
    await fs.mkdir(unmanaged);
    await fs.writeFile(path.join(unmanaged, "foreign"), "keep unrelated directory\n");
    const freshBefore = await contents(fresh.cwd);
    const outside = await replace(fresh.id, { host, title: "outside", cwd: unmanaged, invocation: FAKE_AGENT });
    await assertLive(page, outside.id);
    expect(outside.cwd).toBe(unmanaged);
    expect(outside.working_copy).toBeNull();
    expect(outside.github_repo).toBeNull();
    await expect(fs.stat(fresh.cwd)).rejects.toMatchObject({ code: "ENOENT" });
    const allArchives = await fs.readdir(archive);
    expect(allArchives).toHaveLength(2);
    const secondArchive = allArchives.find((name) => !firstArchive.includes(name));
    expect(secondArchive).toBeTruthy();
    expect(await contents(path.join(archive, secondArchive!))).toEqual(freshBefore);
    await deleteSession(request, outside.id);
    expect(await fs.readFile(path.join(unmanaged, "foreign"), "utf8")).toBe("keep unrelated directory\n");
  } finally {
    for (const id of ids.reverse()) await cleanupSession(request, id);
    await fixture.close();
  }
});
