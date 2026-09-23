/**
 * Mounted composer authority tests. The real helm supplies the host identity
 * and catalog; intercepted checkout replies exercise UI races without network
 * clones or vendor agents. Supervisor allocation and offline Git execution
 * have separate integration coverage; these tests make no allocation claim.
 */
import { expect, test } from "./helpers/evidence";
import { observeFeedReaders, readFeedReaders, stubFeed } from "./helpers/fleet";
import type { APIRequestContext, Page, Route } from "@playwright/test";
import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";

type LocalHost = {
  id: number; identity: string; incarnation: number; state: { phase: string }; kind: string;
  checkoutRevision: number;
};
type PreviewInput = { host: number; expected_incarnation: number; repo: string; title: string | null };

const fixtureBuilds = new WeakMap<Page, string>();

/** Mocked API replies must preserve the real build contract: an absent stamp
 * latches skew and disables the feed this suite needs to exercise. */
test.beforeEach(async ({ page, request }) => {
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  fixtureBuilds.set(page, build);
});

/** Stamp refusals and successes alike, matching the helm's outer middleware. */
async function fulfill(route: Route, options: Parameters<Route["fulfill"]>[0]) {
  const build = fixtureBuilds.get(route.request().frame().page());
  expect(build).toBeTruthy();
  await route.fulfill({ ...options, headers: { ...options?.headers, "x-farhelm-build": build! } });
}

/** Establish the actual installation claim before constructing wire fixtures. */
async function localHost(request: APIRequestContext): Promise<LocalHost> {
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const body = await response.json() as { hosts: LocalHost[] };
  const host = body.hosts.find((candidate) => candidate.kind === "local");
  expect(host, "the owned stack must expose its local host").toBeTruthy();
  expect(host!.state.phase).toBe("connected");
  expect(host!.identity).toBeTruthy();
  expect(host!.incarnation).toBeGreaterThan(0);
  const history = await request.get(`/api/launch-history?host=${host!.id}`);
  expect(history.ok()).toBe(true);
  host!.checkoutRevision = (await history.json()).checkout_config_revision;
  expect(Number.isInteger(host!.checkoutRevision)).toBe(true);
  return host!;
}

/** Path sequences are fixture labels, independent of the monotonic config
 * epoch: clearing a prior test's settings does not reset that epoch. */
function previewFor(input: PreviewInput, host: LocalHost, sequence: number, configRevision = host.checkoutRevision) {
  expect(input.host).toBe(host.id);
  expect(input.expected_incarnation).toBeGreaterThan(0);
  const basename = `bar-${input.title || sequence}`;
  return {
    canonical_root: "/checkout-fixture", basename, cwd: `/checkout-fixture/${basename}`,
    config_revision: configRevision, host: String(host.id), incarnation: input.expected_incarnation,
    installation_identity: host.identity,
  };
}

/** An unavailable scan must leave manual repo input usable and visible. */
async function unavailableDiscovery(page: Page, host: LocalHost) {
  await page.route("**/api/github-repositories", async (route) => {
    const body = route.request().postDataJSON() as { expected_incarnation: number };
    await fulfill(route, { json: {
      host: String(host.id), incarnation: body.expected_incarnation, installation_identity: host.identity,
      repos: [], truncated: true, scan_error: "fixture scan unavailable",
    } });
  });
}

/** Open a complete agent choice while leaving the destination unselected. */
async function openComposer(page: Page, host: LocalHost) {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator('.create-session-form[role="dialog"]');
  await expect(form).toBeVisible();
  await expect(form.getByRole("combobox", { name: "host", exact: true })).toHaveValue(String(host.id));
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  return form;
}

/** Accept exactly the manual fresh action and prove the shared focus handoff. */
async function selectRepo(page: Page) {
  const form = page.locator('.create-session-form[role="dialog"]');
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("gh:acme/bar");
  const choice = form.getByRole("option", { name: "Fresh checkout: acme/bar", exact: true });
  await expect(choice).toHaveAttribute("aria-selected", "true");
  await search.press("Enter");
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();
}

/** Keep every create inside the browser fixture; no clone can reach GitHub. */
async function unaccepted(route: Route) {
  await fulfill(route, { status: 409, headers: { "x-farhelm-create-outcome": "definitely-unaccepted" },
    json: { error: "fixture preview conflict" } });
}

/** Invalid gh text cannot submit the dormant folder, and a first proven
 * refusal refreshes the offer without automatically sending another create.
 * The displayed path follows that offer until an explicit existing-folder
 * choice restores the editable directory. */
test("manual checkout needs selection and explicit resubmit after a proven refusal", async ({ page, request }) => {
  const host = await localHost(request);
  await unavailableDiscovery(page, host);
  let previews = 0;
  const creates: unknown[] = [];
  await page.route("**/api/github-checkout-preview", async (route) => {
    previews += 1;
    await fulfill(route, { json: previewFor(route.request().postDataJSON(), host, previews) });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates.push(route.request().postDataJSON());
    await unaccepted(route);
  });
  const form = await openComposer(page, host);
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("gh:invalid!");
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  await search.press("Enter");
  await expect(search).toHaveValue("gh:invalid!");
  expect(creates).toHaveLength(0);
  await selectRepo(page);
  await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-1");
  const folder = form.getByLabel("folder", { exact: true });
  await expect(folder).toHaveJSProperty("readOnly", true);
  await expect(folder).toHaveValue("/checkout-fixture/bar-1");
  await expect(form.getByRole("button", { name: "browse existing folders" })).toBeEnabled();
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await form.locator(".create-session-submit").click();
  await expect(form.locator(".create-session-error")).toContainText("nothing was accepted");
  await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-2");
  await expect(folder).toHaveValue("/checkout-fixture/bar-2");
  expect(creates).toHaveLength(1);
  expect(creates[0]).toMatchObject({ github_checkout: { repo: "acme/bar" }, launch: { harness: "codex" } });
  await form.getByRole("button", { name: "use existing folder" }).click();
  await expect(folder).toHaveJSProperty("readOnly", false);
  await folder.fill("/tmp");
  await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
  await expect(form.locator(".create-session-submit")).toContainText("/tmp");
});

/** A preview can be slow or fail after checkout mode has replaced an editable
 * folder. Until the helm supplies an accepted path, the folder control must
 * disclose that absence instead of showing the unrelated old `cwd` seed. */
test("checkout folder shows pending and failed preview states without a stale path", async ({ page, request }) => {
  const host = await localHost(request);
  await unavailableDiscovery(page, host);
  let release!: () => void;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  let started = false;
  await page.route("**/api/github-checkout-preview", async (route) => {
    started = true;
    await gate;
    try {
      await fulfill(route, { status: 503, json: { error: "fixture preview unavailable" } });
    } catch (error) {
      if (!route.request().failure()) throw error;
    }
  });
  try {
    const form = await openComposer(page, host);
    const priorFolder = await form.getByLabel("folder", { exact: true }).inputValue();
    expect(priorFolder, "the existing-folder seed must be present before checkout mode hides it").not.toBe("");
    await selectRepo(page);
    await expect.poll(() => started).toBe(true);
    const folder = form.getByLabel("folder", { exact: true });
    await expect(folder).toHaveJSProperty("readOnly", true);
    await expect(folder).toHaveValue("");
    await expect(folder).toHaveAttribute("placeholder", "waiting for checkout preview");
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    release();
    await expect(folder).toHaveAttribute("placeholder", "checkout preview unavailable");
    await expect(form.locator(".launch-composer-checkout-preview .create-session-error")).toBeVisible();
    await expect(folder).toHaveValue("");
    await form.getByRole("button", { name: "use existing folder" }).click();
    await expect(folder).toHaveJSProperty("readOnly", false);
    await expect(folder).toHaveValue(priorFolder);
    await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
  } finally {
    release();
  }
});

/** A lost reply followed by a Conflict is still ambiguous. Even a newer
 * preview for the same selection must not replace the original body or key. */
test("ambiguous checkout retries retain the exact original body after a later refusal", async ({ page, request }) => {
  const host = await localHost(request);
  await unavailableDiscovery(page, host);
  let revision = 1;
  let previews = 0;
  const creates: unknown[] = [];
  await page.route("**/api/github-checkout-preview", async (route) => {
    previews += 1;
    await fulfill(route, { json: previewFor(route.request().postDataJSON(), host, revision) });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates.push(route.request().postDataJSON());
    if (creates.length === 1) await route.abort("failed");
    else await fulfill(route, { status: 409, json: { error: "fixture unresolved conflict" } });
  });
  const form = await openComposer(page, host);
  await selectRepo(page);
  await expect(form.getByLabel("folder", { exact: true })).toHaveValue("/checkout-fixture/bar-1");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await form.locator(".create-session-submit").click();
  await expect(form.locator(".create-session-error")).toContainText("original request is retained");
  expect(creates).toHaveLength(1);
  const before = previews;
  revision = 2;
  await selectRepo(page);
  await expect.poll(() => previews).toBeGreaterThan(before);
  await expect(form.locator(".launch-composer-checkout-preview")).toContainText("retry reconciles the original request");
  await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-1");
  await expect(form.getByLabel("folder", { exact: true })).toHaveValue("/checkout-fixture/bar-1");
  for (const count of [2, 3]) {
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    await form.locator(".create-session-submit").click();
    await expect.poll(() => creates.length).toBe(count);
    await expect(form.locator(".create-session-error")).toContainText("original request is retained");
  }
  expect(creates[1]).toEqual(creates[0]);
  expect(creates[2]).toEqual(creates[0]);
});

/** Recent repo intent must never restore its old ephemeral directory. The
 * offered setup selects the agent and obtains a new preview before submission. */
test("a repository recent selects fresh intent rather than its previous cwd", async ({ page, request }) => {
  const host = await localHost(request);
  await unavailableDiscovery(page, host);
  const creates: unknown[] = [];
  await page.route("**/api/launch-history?*", (route) => fulfill(route, { json: { folders: [], launches: [{
    host: host.id, cwd: "/previous/bar-99", canonical_cwd: "/previous/bar-99",
    github_repo: { owner: "acme", name: "bar" }, created_at: 1, creation_seq: 1,
    selection: { harness: "codex", model: null, effort: null, permissions: null },
  }] } }));
  await page.route("**/api/github-checkout-preview", (route) => fulfill(route, {
    json: previewFor(route.request().postDataJSON(), host, 1),
  }));
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates.push(route.request().postDataJSON());
    await unaccepted(route);
  });
  const form = await openComposer(page, host);
  const recent = form.locator(".launch-composer-recents").getByRole("button", { name: /gh:acme\/bar/ });
  await expect(recent).toBeVisible();
  await recent.click();
  await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-1");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await form.locator(".create-session-submit").click();
  await expect.poll(() => creates.length).toBe(1);
  expect(creates[0]).toMatchObject({ cwd: "/checkout-fixture/bar-1", github_checkout: { repo: "acme/bar" } });
});

/** Two controlled responses cross a title edit. The earlier offer cannot
 * replace the newer path, and neither completion may move focus out of Name. */
test("a late checkout preview cannot overwrite a newer title or steal focus", async ({ page, request }) => {
  const host = await localHost(request);
  await unavailableDiscovery(page, host);
  let releaseOld!: () => void;
  const oldGate = new Promise<void>((resolve) => { releaseOld = resolve; });
  let oldStarted = false;
  let oldFinished = false;
  let currentFinished = false;
  await page.route("**/api/github-checkout-preview", async (route) => {
    const input = route.request().postDataJSON() as PreviewInput;
    if (!input.title) {
      oldStarted = true;
      await oldGate;
    }
    try {
      await fulfill(route, { json: previewFor(input, host, 1) });
    } catch (error) {
      // A cancelled obsolete fetch is an allowed way to discard the reply;
      // a failure delivering the current request is a broken fixture.
      if (input.title || !route.request().failure()) throw error;
    } finally {
      if (!input.title) oldFinished = true;
      else currentFinished = true;
    }
  });
  try {
    const form = await openComposer(page, host);
    await selectRepo(page);
    await expect.poll(() => oldStarted).toBe(true);
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    const name = form.getByLabel("name (optional)", { exact: true });
    await name.fill("new-title");
    await expect(name).toBeFocused();
    await expect.poll(() => currentFinished).toBe(true);
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-new-title");
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue("/checkout-fixture/bar-new-title");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    releaseOld();
    await expect.poll(() => oldFinished).toBe(true);
    await expect(name).toBeFocused();
    await expect(name).toHaveValue("new-title");
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-new-title");
  } finally {
    releaseOld();
  }
});

/** An old fresh offer must not resurrect checkout intent after the user
 * explicitly returns to an existing folder. The submitted body is the oracle:
 * a hidden stale preview must not turn the ordinary launch into a clone. */
test("a late checkout preview cannot replace an existing folder destination", async ({ page, request }) => {
  const host = await localHost(request);
  await unavailableDiscovery(page, host);
  let release!: () => void;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  let started = false;
  let finished = false;
  const creates: Record<string, unknown>[] = [];
  await page.route("**/api/github-checkout-preview", async (route) => {
    started = true;
    await gate;
    try {
      await fulfill(route, { json: previewFor(route.request().postDataJSON(), host, 1) });
    } catch (error) {
      if (!route.request().failure()) throw error;
    } finally {
      finished = true;
    }
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates.push(route.request().postDataJSON());
    await fulfill(route, { status: 400, json: { error: "fixture ordinary create refused" } });
  });
  try {
    const form = await openComposer(page, host);
    await selectRepo(page);
    await expect.poll(() => started).toBe(true);
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    const folder = form.getByLabel("folder", { exact: true });
    await expect(folder).toHaveJSProperty("readOnly", true);
    await expect(folder).toHaveValue("");
    await expect(folder).toHaveAttribute("placeholder", "waiting for checkout preview");
    await form.getByRole("button", { name: "use existing folder" }).click();
    await expect(folder).toHaveJSProperty("readOnly", false);
    await folder.fill("/tmp");
    await expect(folder).toBeFocused();
    await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
    release();
    await expect.poll(() => finished).toBe(true);
    await expect(folder).toBeFocused();
    await expect(folder).toHaveValue("/tmp");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    await form.locator(".create-session-submit").click();
    await expect.poll(() => creates.length).toBe(1);
    expect(creates[0]).toMatchObject({ cwd: "/tmp", launch: { harness: "codex" } });
    expect(creates[0].github_checkout ?? null).toBeNull();
  } finally {
    release();
  }
});

/** Escape first dismisses repository suggestions without accepting one or
 * cancelling the dialog. Once suggestions are closed, Escape belongs to the
 * dialog. Neither keyboard action may dispatch a create. */
test("Escape dismisses repository suggestions before closing the composer", async ({ page, request }) => {
  const host = await localHost(request);
  await unavailableDiscovery(page, host);
  const creates: unknown[] = [];
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates.push(route.request().postDataJSON());
    await unaccepted(route);
  });
  const form = await openComposer(page, host);
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("gh:acme/bar");
  await expect(search).toBeFocused();
  await expect(form.getByRole("option", { name: "Fresh checkout: acme/bar", exact: true })).toBeVisible();
  await search.press("Escape");
  await expect(form).toBeVisible();
  await expect(search).toBeFocused();
  await expect(search).toHaveAttribute("aria-expanded", "false");
  await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
  expect(creates).toHaveLength(0);
  await search.press("Escape");
  await expect(form).toHaveCount(0);
  expect(creates).toHaveLength(0);
});

/** A preview for host A has no authority on host B, even while B's own
 * response is still pending. Both requests are latched so a stale success
 * cannot hide behind an already-ready replacement response. */
test("switching hosts rejects the old checkout preview while the new host is pending", async ({ page, request }) => {
  const local = await localHost(request);
  let remote: LocalHost | undefined;
  await expect.poll(async () => {
    const response = await request.get("/api/hosts");
    expect(response.ok()).toBe(true);
    const body = await response.json() as { hosts: LocalHost[] };
    remote = body.hosts.find((candidate) => candidate.kind === "ssh");
    return remote?.state.phase;
  }, { message: "the owned second host must be connected for the host-transition race" }).toBe("connected");
  expect(remote!.identity).toBeTruthy();
  expect(remote!.id).not.toBe(local.id);
  // The epoch is helm-global, including edits to per-host overrides.
  remote!.checkoutRevision = local.checkoutRevision;
  await page.route("**/api/github-repositories", async (route) => {
    const input = route.request().postDataJSON();
    const host = input.host === local.id ? local : remote!;
    await fulfill(route, { json: {
      host: String(host.id), incarnation: input.expected_incarnation, installation_identity: host.identity,
      repos: [], truncated: false, scan_error: null,
    } });
  });
  let releaseLocal!: () => void;
  let releaseRemote!: () => void;
  const localGate = new Promise<void>((resolve) => { releaseLocal = resolve; });
  const remoteGate = new Promise<void>((resolve) => { releaseRemote = resolve; });
  let localStarted = false;
  let localFinished = false;
  let remoteStarted = false;
  await page.route("**/api/github-checkout-preview", async (route) => {
    const input = route.request().postDataJSON() as PreviewInput;
    const isLocal = input.host === local.id;
    if (isLocal) localStarted = true;
    else {
      expect(input.host).toBe(remote!.id);
      remoteStarted = true;
    }
    await (isLocal ? localGate : remoteGate);
    try {
      await fulfill(route, { json: previewFor(input, isLocal ? local : remote!, isLocal ? 1 : 2) });
    } catch (error) {
      if (!isLocal || !route.request().failure()) throw error;
    } finally {
      if (isLocal) localFinished = true;
    }
  });
  try {
    const form = await openComposer(page, local);
    await selectRepo(page);
    await expect.poll(() => localStarted).toBe(true);
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    const hostPicker = form.getByRole("combobox", { name: "host", exact: true });
    await hostPicker.selectOption(String(remote!.id));
    await expect(hostPicker).toHaveValue(String(remote!.id));
    await expect.poll(() => remoteStarted).toBe(true);
    releaseLocal();
    await expect.poll(() => localFinished).toBe(true);
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    await expect(form.locator(".launch-composer-checkout-preview")).not.toContainText("/checkout-fixture/bar-1");
    releaseRemote();
    await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-2");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
  } finally {
    releaseLocal();
    releaseRemote();
  }
});

for (const { feedUnavailable, ambiguous } of [
  { feedUnavailable: false, ambiguous: false },
  { feedUnavailable: false, ambiguous: true },
  { feedUnavailable: true, ambiguous: false },
  { feedUnavailable: true, ambiguous: true },
]) {
  /** The shipped CLI changes the owned helm database while this dialog stays
   * open. Its first history refresh fails deliberately: successful recovery
   * must come from retained reader demand, without a second configuration edit.
   * The unavailable-feed variant never sends a notification, so only the
   * bounded fallback can discover that edit. A dispatched ambiguous intent
   * remains bound to its original snapshot under either refresh mechanism. */
  test(`CLI configuration changes refresh ${ambiguous ? "an ambiguous retry without replacing its body" : "an undispatched preview after a failed history read"} with ${feedUnavailable ? "an unavailable" : "a healthy"} feed`, async ({ page, request }) => {
    await observeFeedReaders(page);
    const unavailableFeed = feedUnavailable ? await stubFeed(page) : undefined;
    const host = await localHost(request);
    const stack = JSON.parse(await readFile(path.join(__dirname, "..", ".stack-info.json"), "utf8")) as {
      farhelm: string; state: string;
    };
    const run = promisify(execFile);
    const config = async (...args: string[]) => run(stack.farhelm,
      ["helm", "checkout-config", ...args, "--state-dir", stack.state],
      { timeout: 10_000, maxBuffer: 64 * 1024 });
    expect((await config("show")).stdout).toMatch(/^root: unset$/m);
    const historyUrl = `/api/launch-history?host=${host.id}`;
    const initial = await request.get(historyUrl);
    expect(initial.ok()).toBe(true);
    const baseline = (await initial.json()).checkout_config_revision as number;
    expect(Number.isInteger(baseline)).toBe(true);
    await unavailableDiscovery(page, host);
    let failHistory = false;
    let historyFailed = false;
    await page.route("**/api/launch-history?*", async (route) => {
      if (failHistory) {
        failHistory = false;
        historyFailed = true;
        await fulfill(route, { status: 503, json: { error: "fixture transient history failure" } });
      } else await route.continue();
    });
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    let changedPreviewStarted = false;
    let changedRevision = baseline;
    await page.route("**/api/github-checkout-preview", async (route) => {
      const response = await request.get(historyUrl);
      expect(response.ok()).toBe(true);
      const revision = (await response.json()).checkout_config_revision as number;
      if (revision > baseline) {
        changedRevision = revision;
        changedPreviewStarted = true;
        await gate;
      }
      await fulfill(route, { json: previewFor(route.request().postDataJSON(), host, revision, revision) });
    });
    const creates: unknown[] = [];
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      creates.push(route.request().postDataJSON());
      if (ambiguous && creates.length === 1) await route.abort("failed");
      else await unaccepted(route);
    });
    try {
      const form = await openComposer(page, host);
      await selectRepo(page);
      await expect(form.locator(".launch-composer-checkout-preview")).toContainText(`/checkout-fixture/bar-${baseline}`);
      await expect(form.locator(".create-session-submit")).toBeEnabled();
      if (ambiguous) {
        await form.locator(".create-session-submit").click();
        await expect(form.locator(".create-session-error")).toContainText("original request is retained");
        expect(creates).toHaveLength(1);
      }
      if (unavailableFeed) await unavailableFeed.waitForConnection(1);
      await expect.poll(async () => {
        const list = (await readFeedReaders(page)).find((reader) => reader.role === "list");
        return { healthy: list?.healthy, skew: list?.skew };
      }).toEqual({ healthy: !feedUnavailable, skew: false });
      failHistory = true;
      await config("set-root", path.join(stack.state, "changed-checkouts"));
      const current = await request.get(historyUrl);
      expect(current.ok()).toBe(true);
      expect((await current.json()).checkout_config_revision).toBeGreaterThan(baseline);
      await expect.poll(() => historyFailed, { timeout: 10_000 }).toBe(true);
      await expect.poll(() => changedPreviewStarted, { timeout: 10_000 }).toBe(true);
      if (ambiguous) {
        await expect(form.locator(".launch-composer-checkout-preview")).toContainText("retry reconciles the original request");
        await expect(form.locator(".create-session-submit")).toBeEnabled();
      } else await expect(form.locator(".create-session-submit")).toBeDisabled();
      expect(creates).toHaveLength(ambiguous ? 1 : 0);
      release();
      if (!ambiguous) {
        await expect(form.locator(".launch-composer-checkout-preview")).toContainText(`/checkout-fixture/bar-${changedRevision}`);
      }
      await expect(form.locator(".create-session-submit")).toBeEnabled();
      await form.locator(".create-session-submit").click();
      await expect.poll(() => creates.length).toBe(ambiguous ? 2 : 1);
      if (ambiguous) expect(creates[1]).toEqual(creates[0]);
      else expect(creates[0]).toMatchObject({ github_checkout: { preview: { config_revision: changedRevision } } });
    } finally {
      release();
      await page.close();
      await config("clear-root");
    }
  });
}
