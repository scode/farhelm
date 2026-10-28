import { chooseLauncherHost } from "./helpers/host-picker";
import { fillLauncherFolder } from "./helpers/launcher";
/**
 * Mounted composer authority tests. The real helm supplies the host identity
 * and catalog; intercepted checkout replies exercise UI races without network
 * clones or vendor agents. Supervisor allocation and offline Git execution
 * have separate integration coverage; these tests make no allocation claim.
 */
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { drainedRoute } from "./helpers/drained-route";
import { routeGate } from "./helpers/route-gate";
import {
  cleanupSession,
  createSession,
  listSessions,
  observeFeedReaders,
  openRowMenu,
  readFeedReaders,
  stopSession,
  stubFeed,
} from "./helpers/fleet";
import { LIVE_BADGE, LIVE_STATES } from "./helpers/terminal-suite";
import type { APIRequestContext, Page, Request, Route } from "@playwright/test";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, realpath, rm } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { currentStackSlot } from "../stack-slots";

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
  await expect(form.getByRole("combobox", { name: "host", exact: true })).toHaveAttribute("data-host-id", String(host.id));
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  return form;
}

/** Accept exactly the manual fresh action and prove the shared focus handoff. */
async function selectRepo(page: Page) {
  const form = page.locator('.create-session-form[role="dialog"]');
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("gh:acme/bar");
  const choice = form.getByRole("option", { name: "Managed checkout: acme/bar", exact: true });
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

/** Arrow navigation must reveal the row Enter would choose in an overflowing
 * repository list. Both wrap directions and forward movement keep browser
 * focus on the input; a selected-but-clipped option is not usable feedback.
 */
test("repository keyboard navigation keeps the active suggestion visible through wrapping", async ({ page, request }) => {
  const host = await localHost(request);
  const repos = Array.from({ length: 24 }, (_, index) => ({ owner: "acme", name: `repo-${String(index).padStart(2, "0")}` }));
  await page.route("**/api/github-repositories", async (route) => {
    const input = route.request().postDataJSON() as { host: number; expected_incarnation: number };
    expect(input.host).toBe(host.id);
    await fulfill(route, { json: {
      host: String(host.id), incarnation: input.expected_incarnation, installation_identity: host.identity,
      repos, truncated: false, scan_error: null, needs_checkout_root: false,
    } });
  });
  const form = await openComposer(page, host);
  await form.getByRole("group", { name: "destination type", exact: true })
    .getByRole("button", { name: "managed checkout", exact: true }).click();
  const repository = form.getByRole("combobox", { name: "repository", exact: true });
  await repository.focus();
  await expect(repository).toBeFocused();
  const list = form.getByRole("listbox", { name: "repositories", exact: true });
  await expect(list.getByRole("option")).toHaveCount(repos.length);
  expect(await list.evaluate((element) => element.scrollHeight > element.clientHeight), "fixture suggestions must overflow").toBe(true);

  /** Selection alone is insufficient: require the entire active row inside
   * the list's viewport after the post-render scroll has had a chance to run. */
  async function expectVisibleActive(name: string) {
    const active = list.getByRole("option", { selected: true });
    await expect(active).toHaveText(name);
    await expect.poll(() => active.evaluate((element) => {
      const row = element.getBoundingClientRect();
      const viewport = element.parentElement!.getBoundingClientRect();
      return row.top >= viewport.top && row.bottom <= viewport.bottom;
    })).toBe(true);
    await expect(repository).toBeFocused();
  }
  await expectVisibleActive("acme/repo-00");
  await repository.press("ArrowUp");
  await expectVisibleActive("acme/repo-23");
  await repository.press("ArrowDown");
  await expectVisibleActive("acme/repo-00");
  for (let index = 0; index < 12; index++) await repository.press("ArrowDown");
  await expectVisibleActive("acme/repo-12");
});

/** Inline setup must refresh the same launcher's discovery and failed preview.
 * The real supervisor scans a local Git fixture; a greeted, unchanging feed
 * prevents the revision watcher from supplying the refresh under test. No
 * launch or network clone is performed. A later notification exposes an
 * external clear to the same open launcher. An unrelated epoch change must keep
 * the setup editor's DOM identity, draft and focus through a pending recheck.
 * Title edits must keep configured
 * discovery idle. A command-line host override then
 * proves the field follows effective host configuration, not the global GET. */
test("inline checkout folder setup refreshes discovery and preview and respects host overrides", async ({ page, request }, testInfo) => {
  const host = await localHost(request);
  const stack = JSON.parse(await readFile(currentStackSlot().stackInfoPath, "utf8")) as {
    farhelm: string; state: string;
  };
  const run = promisify(execFile);
  // A Git hook can pass repository selectors that override -C. Keep these
  // children inside the owned clone and independent of ambient Git config.
  const gitEnv = Object.fromEntries(Object.entries(process.env).filter(([name]) =>
    !name.startsWith("GIT_") && !name.startsWith("FARHELM_")
  ));
  Object.assign(gitEnv, {
    GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_SYSTEM: "/dev/null", GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_CONFIG_COUNT: "0", GIT_TERMINAL_PROMPT: "0",
  });
  const cli = (...args: string[]) => run(stack.farhelm,
    ["helm", "checkout-config", ...args, "--state-dir", stack.state],
    { timeout: 10_000, maxBuffer: 64 * 1024 });
  const previousReply = await request.get("/api/checkout-root");
  expect(previousReply.ok(), await previousReply.text()).toBe(true);
  const previous = await previousReply.json();
  const previousCommandReply = await request.get("/api/checkout-post-clone");
  expect(previousCommandReply.ok()).toBe(true);
  const previousCommand = await previousCommandReply.json();
  let releaseDiscovery: (() => void) | undefined;
  const root = await mkdtemp(path.join(stack.state, "inline-checkouts-"));
  let overrideSet = false;
  let submittedSave: Request | undefined;
  let discoveryRequests = 0;
  let bodyFailed = false;
  page.on("request", (outgoing) => {
    if (outgoing.url().endsWith("/api/github-repositories")) discoveryRequests += 1;
    if (outgoing.url().endsWith("/api/checkout-root") && outgoing.method() === "PUT") {
      submittedSave = outgoing;
    }
  });
  try {
    const cleared = await request.put("/api/checkout-root", { data: { root: null } });
    expect(cleared.ok(), await cleared.text()).toBe(true);
    expect((await cli("show", "--host", String(host.id))).stdout).toMatch(/^root: unset$/m);
    const clone = path.join(root, "local-clone");
    await mkdir(clone);
    await run("git", ["-C", clone, "init", "--quiet"], { env: gitEnv });
    await run("git", ["-C", clone, "remote", "add", "origin", "https://github.com/acme/bar.git"], { env: gitEnv });
    expect((await run("git", ["-C", clone, "config", "--get", "remote.origin.url"], { env: gitEnv })).stdout.trim())
      .toBe("https://github.com/acme/bar.git");
    const feed = await stubFeed(page);
    feed.notifyOnConnect(1);
    const form = await openComposer(page, host);
    await feed.waitForConnection(1);
    expect(feed.openSockets()).toBeGreaterThan(0);
    await form.getByRole("group", { name: "destination type", exact: true })
      .getByRole("button", { name: "managed checkout", exact: true }).click();
    await expect(form.locator(".launch-composer-repository-note")).toBeVisible();
    const field = form.locator(".checkout-root-field");
    const folder = field.getByRole("textbox", { name: "checkout folder", exact: true });
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue("");
    await expect(field).toContainText("Settings");
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 900 });
      await field.screenshot({ path: testInfo.outputPath(`inline-checkout-setup-${width}.png`) });
      expect(await field.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
    }
    await folder.fill("relative-folder");
    await field.getByRole("button", { name: "save", exact: true }).click();
    await expect(field.getByRole("alert")).toContainText("not the relative path 'relative-folder'");
    expect(await (await request.get("/api/checkout-root")).json()).toEqual({ root: null });
    await page.setViewportSize({ width: 1280, height: 900 });

    // A repository can be selected before setup. Keep its folder editor and
    // refused draft through the failed preview instead of hiding the remedy.
    const selectedDiscovery = page.waitForResponse(async response =>
      response.url().endsWith("/api/github-repositories") && response.ok()
      && (await response.json()).needs_checkout_root === true
    );
    await selectRepo(page);
    await selectedDiscovery;
    await expect(form.locator(".launch-composer-checkout-preview")).toHaveAttribute("data-preview-state", "failed");
    await expect(folder).toHaveValue("relative-folder");
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    await folder.focus();
    await expect(folder).toBeFocused();
    const originalEditor = await folder.elementHandle();
    expect(originalEditor).not.toBeNull();
    const held = new Promise<void>(resolve => { releaseDiscovery = resolve; });
    await page.route("**/api/github-repositories", async route => { await held; await route.continue(); });
    const rechecking = page.waitForRequest(outgoing => outgoing.url().endsWith("/api/github-repositories"));
    const historyBefore = await request.get(`/api/launch-history?host=${host.id}`);
    expect(historyBefore.ok()).toBe(true);
    const revision = (await historyBefore.json()).checkout_config_revision;
    const unrelated = await request.put("/api/checkout-post-clone", { data: { command: `true # ${root}` } });
    expect(unrelated.ok()).toBe(true);
    const historyAfter = await request.get(`/api/launch-history?host=${host.id}`);
    expect(historyAfter.ok()).toBe(true);
    expect((await historyAfter.json()).checkout_config_revision).toBeGreaterThan(revision);
    expect(await (await request.get("/api/checkout-root")).json()).toEqual({ root: null });
    expect(feed.openSockets()).toBeGreaterThan(0);
    feed.notify(2);
    await rechecking;
    expect(await originalEditor!.evaluate(element => element.isConnected)).toBe(true);
    await expect(folder).toHaveValue("relative-folder");
    await expect(folder).toBeFocused();
    const refreshed = page.waitForResponse(async response =>
      response.url().endsWith("/api/github-repositories") && response.ok()
      && (await response.json()).needs_checkout_root === true
    );
    releaseDiscovery!();
    await refreshed;
    expect(await originalEditor!.evaluate(element => element.isConnected)).toBe(true);
    await expect(folder).toHaveValue("relative-folder");
    await expect(folder).toBeFocused();
    const discoveryAfterSave = page.waitForResponse(async (response) =>
      response.url().endsWith("/api/github-repositories") && response.ok()
      && (await response.json()).needs_checkout_root === false
    );
    await folder.fill(root);
    await field.getByRole("button", { name: "save", exact: true }).click();
    const discovered = await (await discoveryAfterSave).json();
    expect(discovered.repos).toContainEqual({ owner: "acme", name: "bar" });
    await expect(field).toHaveCount(0);
    const preview = form.locator(".launch-composer-checkout-preview");
    await expect(preview).toHaveAttribute("data-preview-state", "ready");
    await expect(preview).toContainText(await realpath(root));
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 900 });
      await form.screenshot({ path: testInfo.outputPath(`inline-checkout-ready-${width}.png`) });
    }
    // A new title needs a new preview, but the accepted folder observation
    // remains valid until configuration changes. No rescan should start.
    const scansBeforeTitle = discoveryRequests;
    await form.getByLabel("name (optional)", { exact: true }).fill("edited title");
    await expect(preview).toContainText("bar-edited-title");
    await expect(preview).toHaveAttribute("data-preview-state", "ready");
    // sleep-ok: observe beyond discovery's 150ms debounce; a title-triggered scan must stay absent.
    await page.waitForTimeout(300);
    expect(discoveryRequests).toBe(scansBeforeTitle);

    const beforeClear = await request.get(`/api/launch-history?host=${host.id}`);
    expect(beforeClear.ok(), await beforeClear.text()).toBe(true);
    const configuredRevision = (await beforeClear.json()).checkout_config_revision;
    await cli("clear-root");
    expect((await cli("show", "--host", String(host.id))).stdout).toMatch(/^root: unset$/m);
    const afterClear = await request.get(`/api/launch-history?host=${host.id}`);
    expect(afterClear.ok(), await afterClear.text()).toBe(true);
    expect((await afterClear.json()).checkout_config_revision).toBeGreaterThan(configuredRevision);
    // The stub intentionally withheld earlier invalidations to isolate save's
    // explicit refresh. Deliver this external edit only after proving it exists.
    expect(feed.openSockets()).toBeGreaterThan(0);
    feed.notify(3);
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue("");
    await expect(preview).toHaveAttribute("data-preview-state", "failed");
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    await form.getByRole("button", { name: "cancel", exact: true }).click();

    overrideSet = true;
    await cli("set-root", root, "--host", String(host.id));
    const unset = await request.put("/api/checkout-root", { data: { root: null } });
    expect(unset.ok(), await unset.text()).toBe(true);
    expect(await (await request.get("/api/checkout-root")).json()).toEqual({ root: null });
    const discovery = await request.post("/api/github-repositories", {
      data: { host: host.id, expected_incarnation: host.incarnation, query: "" },
    });
    expect(discovery.ok(), await discovery.text()).toBe(true);
    expect((await discovery.json()).needs_checkout_root).toBe(false);
    const overriddenForm = await openComposer(page, host);
    await overriddenForm.getByRole("group", { name: "destination type", exact: true })
      .getByRole("button", { name: "managed checkout", exact: true }).click();
    await overriddenForm.getByRole("combobox", { name: "repository", exact: true }).focus();
    await expect(overriddenForm.getByRole("option", { name: "acme/bar", exact: true })).toBeVisible();
    await expect(overriddenForm.locator(".checkout-root-field")).toHaveCount(0);
    expect((await cli("show", "--host", String(host.id))).stdout).toContain(`${root} (host override)`);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    // Settle the app-owned save before restoring configuration. A failure in
    // any release is reported, while the remaining restoration still runs.
    await cleanupOwnedSteps(bodyFailed, [
      ["held discovery", async () => { releaseDiscovery?.(); }],
      ["pending checkout save", async () => {
        const settled = await submittedSave?.response();
        if (settled) await settled.finished();
      }],
      ["host checkout override", async () => {
        if (overrideSet) await cli("clear-root", "--host", String(host.id));
      }],
      ["global checkout folder", async () => {
        const restored = await request.put("/api/checkout-root", { data: previous });
        expect(restored.ok(), await restored.text()).toBe(true);
      }],
      ["post-clone command", async () => {
        const restored = await request.put("/api/checkout-post-clone", { data: previousCommand });
        expect(restored.ok()).toBe(true);
      }],
      ["checkout fixture directory", () => rm(root, { recursive: true, force: true })],
    ]);
  }
});

/** A fresh helm must explain managed-checkout setup without letting the note
 * dominate the launcher. This uses the real missing-root reply and proves the
 * fixture has no root before measuring helper text at desktop and phone widths.
 */
test("managed checkout setup notes use ordinary helper styling", async ({ page, request }, testInfo) => {
  const host = await localHost(request);
  const stack = JSON.parse(await readFile(currentStackSlot().stackInfoPath, "utf8")) as {
    farhelm: string; state: string;
  };
  const result = await promisify(execFile)(stack.farhelm,
    ["helm", "checkout-config", "show", "--state-dir", stack.state],
    { timeout: 10_000, maxBuffer: 64 * 1024 });
  expect(result.stdout).toMatch(/^root: unset$/m);
  const discovery = await request.post("/api/github-repositories", {
    data: { host: host.id, expected_incarnation: host.incarnation, query: "" },
  });
  expect(discovery.ok(), await discovery.text()).toBe(true);
  expect((await discovery.json()).scan_error).toMatch(/^no checkout (root|folder) is configured/);
  const form = await openComposer(page, host);
  await form.getByRole("group", { name: "destination type", exact: true })
    .getByRole("button", { name: "managed checkout", exact: true }).click();
  const note = form.locator(".launch-composer-repository-note");
  await expect(note).toBeVisible();
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  await page.evaluate(() => document.fonts.ready);
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await form.screenshot({ path: testInfo.outputPath(`checkout-setup-note-${width}.png`) });
    const style = await note.evaluate((el) => {
      const help = el.closest("form")!.querySelector(".launch-composer-checkout-explanation")!;
      return {
        size: getComputedStyle(el).fontSize,
        color: getComputedStyle(el).color,
        lineHeight: getComputedStyle(el).lineHeight,
        helpColor: getComputedStyle(help).color,
        overflowing: el.scrollWidth > el.clientWidth,
      };
    });
    expect(style.size).toBe("12px");
    expect(style.lineHeight).toBe("18px");
    expect(style.color).toBe(style.helpColor);
    expect(style.overflowing).toBe(false);
  }
});

/** The visible type must own an empty or invalid repository draft. Selection
 * grants preview authority; Enter in the repository field never launches. The
 * ordinary folder survives a round trip without becoming a hidden fallback. */
test("destination type keeps unselected repositories separate from the folder", async ({ page, request }) => {
  const host = await localHost(request);
  await unavailableDiscovery(page, host);
  const creates: unknown[] = [];
  await page.route("**/api/github-checkout-preview", async (route) => {
    await fulfill(route, { json: previewFor(route.request().postDataJSON(), host, 1) });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates.push(route.request().postDataJSON());
    await unaccepted(route);
  });
  const form = await openComposer(page, host);
  const types = form.getByRole("group", { name: "destination type", exact: true });
  const folder = form.getByLabel("folder", { exact: true });
  await folder.fill("/tmp/retained-folder");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await types.getByRole("button", { name: "managed checkout", exact: true }).click();
  await expect(types.getByRole("button", { name: "managed checkout", exact: true })).toHaveAttribute("aria-pressed", "true");
  const repository = form.getByRole("combobox", { name: "repository", exact: true });
  await expect(repository).toHaveValue("");
  await expect(folder).toHaveValue("");
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  const globalSearch = form.locator(".launch-composer-search input");
  await globalSearch.fill("host:no-host-for-repository-dismissal");
  await expect(form.locator("#launch-composer-search-results")).toHaveText("no host matches");
  await repository.click();
  await expect(form.locator("#launch-composer-search-results")).toHaveCount(0);
  await expect(form.locator("#checkout-repository-options")).toBeVisible();
  // Retire the dismissed query before the later destination-readiness checks.
  await globalSearch.fill("");
  await repository.fill("invalid!");
  const repositoryArea = form.locator("#checkout-repository-options");
  await expect(repositoryArea).toHaveText("no repository matches; enter owner/repo");
  await repository.press("Enter");
  await expect(repositoryArea).toHaveText("no repository matches; enter owner/repo");
  await form.getByLabel("name (optional)", { exact: true }).click();
  await expect(repositoryArea).toHaveCount(0);
  await repository.focus();
  await expect(repositoryArea).toHaveText("no repository matches; enter owner/repo");
  await repository.press("Escape");
  await expect(repositoryArea).toHaveCount(0);
  await repository.press("Enter");
  await expect(repositoryArea).toHaveText("no repository matches; enter owner/repo");
  await expect(form.locator(".launch-composer-refusal")).toHaveCount(0);
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  await repository.fill("acme/bar");
  // A typed owner/repo suggestion exists before discovery answers. Observe
  // the fixture warning first so its disappearance actually tests selection.
  await expect(form.locator(".launch-composer-destination .launch-composer-repository-note")).toContainText("fixture scan unavailable");
  await expect(form.getByRole("listbox", { name: "repositories" }).getByRole("option", { name: "acme/bar", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(form.locator(".launch-composer-refusal")).toHaveCount(0);
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  // A hidden suggestion must become visible before a separate Enter can
  // select it and grant checkout-preview authority.
  await repository.press("Escape");
  await expect(repositoryArea).toHaveCount(0);
  await repository.press("Enter");
  await expect(form.getByRole("option", { name: "acme/bar", exact: true })).toBeVisible();
  await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
  await repository.press("Enter");
  await expect(form.locator(".launch-composer-refusal")).toHaveCount(0);
  await expect(repository).toBeFocused();
  await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-1");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await expect(form.locator(".launch-composer-refusal")).toHaveCount(0);
  await expect(form.locator(".launch-composer-checkout-explanation")).toContainText("last session using it is deleted");
  await expect(form.locator(".launch-composer-repository-note")).toHaveCount(0);
  await form.getByLabel("name (optional)", { exact: true }).fill("renamed");
  await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-renamed");
  await expect(form.locator(".launch-composer-repository-note")).toHaveCount(0);
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  expect(creates).toHaveLength(0);
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("gh:x");
  await expect(form.locator(".launch-composer-repository-note")).toHaveCount(1);
  await expect(form.locator(".launch-composer-repository-note")).toContainText("fixture scan unavailable");
  await expect(form.locator(".launch-composer-destination .launch-composer-repository-note")).toHaveCount(0);
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  // A deliberate field selection retires the older global gh: query. The
  // ready preview alone must not leave the launcher blocked on hidden search.
  await repository.fill("acme/bar");
  await expect(form.getByRole("listbox", { name: "repositories" }).getByRole("option", { name: "acme/bar", exact: true })).toBeVisible();
  await repository.press("Enter");
  await expect(search).toHaveValue("");
  await expect(form.locator(".launch-composer-checkout-preview")).toHaveAttribute("data-preview-state", "ready");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await repository.fill("");
  await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  await types.getByRole("button", { name: "folder", exact: true }).click();
  await expect(folder).toHaveValue("/tmp/retained-folder");
  await expect(folder).toHaveJSProperty("readOnly", false);
  await expect(repository).toHaveCount(0);
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  expect(creates).toHaveLength(0);
});

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
  await expect(form.getByRole("group", { name: "destination type" }).getByRole("button", { name: "managed checkout", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(form.getByRole("button", { name: "browse this path" })).toHaveCount(0);
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await form.locator(".create-session-submit").click();
  await expect(form.locator(".create-session-error")).toContainText("nothing was accepted");
  await expect(form.locator(".launch-composer-checkout-preview")).toContainText("/checkout-fixture/bar-2");
  await expect(folder).toHaveValue("/checkout-fixture/bar-2");
  expect(creates).toHaveLength(1);
  expect(creates[0]).toMatchObject({ github_checkout: { repo: "acme/bar" }, launch: { harness: "codex" } });
  await form.getByRole("button", { name: "folder", exact: true }).click();
  await expect(folder).toHaveJSProperty("readOnly", false);
  await folder.fill("/tmp");
  await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
  await expect(form.locator(".launch-composer-launch-context")).toContainText("/tmp");
  await expect(form.locator(".create-session-submit")).toHaveAccessibleName(/ · \/tmp$/);
});

for (const outcome of ["submit", "edit", "close"] as const) {
  /** A guarded answer changes the checkout draft and therefore needs a new
   * authoritative preview. Holding that reply proves the answer cannot send
   * the refused YOLO body or submit before the new preview is ready. A later
   * draft edit or dialog close withdraws the pending automatic submission.
   * All creates stay intercepted: these cases allocate no checkout.
   */
  test(`guarded checkout answer waits for preview and ${outcome === "submit" ? "submits once" : `is cancelled by ${outcome}`}`, async ({ page, request }) => {
    const host = await localHost(request);
    await unavailableDiscovery(page, host);
    const gate = routeGate();
    const previews: PreviewInput[] = [];
    const creates: any[] = [];
    let holdRefresh = false;
    let heldStarted = false;
    let heldFinished = false;
    await page.route("**/api/github-checkout-preview", async (route) => {
      const input = route.request().postDataJSON() as PreviewInput;
      previews.push(input);
      const sequence = previews.length;
      const held = holdRefresh && !input.title;
      if (held) {
        heldStarted = true;
        await gate.wait();
      }
      try {
        await fulfill(route, { json: previewFor(input, host, sequence) });
      } catch (error) {
        // Editing or closing may abort the obsolete held request. A current
        // preview failure is a broken fixture, not evidence of cancellation.
        if (!held || !route.request().failure()) throw error;
      } finally {
        if (held) heldFinished = true;
      }
    });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const body = route.request().postDataJSON();
      creates.push(body);
      if (body.launch?.permissions === "yolo") {
        await fulfill(route, { status: 409, contentType: "text/plain",
          headers: { "x-farhelm-yolo-confirmation": "confirmation-required" }, body: "fixture YOLO confirmation" });
      } else {
        await fulfill(route, { status: 400, json: { error: "fixture guarded create received" } });
      }
    });

    try {
      const form = await openComposer(page, host);
      const name = form.getByLabel("name (optional)", { exact: true });
      await name.fill("");
      await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
      await selectRepo(page);
      const preview = form.locator(".launch-composer-checkout-preview");
      await expect(preview).toHaveAttribute("data-preview-state", "ready");
      await expect(preview).toContainText("/checkout-fixture/bar-1");
      const submit = form.locator(".create-session-submit");
      await expect(submit).toBeEnabled();
      await submit.click();
      const confirmation = form.locator(".yolo-confirmation");
      await expect(confirmation).toBeVisible();
      await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
      expect(creates).toHaveLength(1);
      expect(creates[0]).toMatchObject({ github_checkout: { repo: "acme/bar" }, launch: { permissions: "yolo" } });
      expect(creates[0].intent_key).toBeTruthy();
      const previewsBefore = previews.length;
      holdRefresh = true;
      await confirmation.locator(".yolo-launch-guarded").click();
      await expect.poll(() => heldStarted).toBe(true);
      expect(previews.length, "changing permission requests a fresh checkout preview").toBeGreaterThan(previewsBefore);
      await expect(preview).toHaveAttribute("data-preview-state", "pending");
      await expect(submit).toBeDisabled();
      await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }))
        .toHaveAttribute("aria-pressed", "true");
      expect(creates, "a guarded answer waits while its fresh preview is held").toHaveLength(1);

      if (outcome === "edit") {
        await name.fill("edited-before-preview");
        await expect(name).toBeFocused();
        await expect(preview).toHaveAttribute("data-preview-state", "ready");
        await expect(preview).toContainText("/checkout-fixture/bar-edited-before-preview");
      } else if (outcome === "close") {
        await form.locator(".launch-composer-cancel").click();
        await expect(form).toHaveCount(0);
      }

      gate.release();
      await expect.poll(() => heldFinished).toBe(true);
      if (outcome === "submit") {
        await expect(form.locator(".create-session-error")).toContainText("fixture guarded create received");
        expect(creates, "the refreshed preview admits one automatic guarded create").toHaveLength(2);
        expect(creates[1]).toMatchObject({ github_checkout: { repo: "acme/bar", preview: { cwd: "/checkout-fixture/bar-2" } }, launch: { harness: "codex", permissions: "guarded" } });
      } else {
        if (outcome === "edit") {
          await expect(preview).toContainText("/checkout-fixture/bar-edited-before-preview");
          await expect(name).toHaveValue("edited-before-preview");
          await expect(submit).toBeEnabled();
        } else {
          await page.locator(".new-session-button").click();
          await expect(form).toBeVisible();
          await fillLauncherFolder(form, "/tmp");
          await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
          await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
          await expect(submit).toBeEnabled();
        }
        expect(creates, "the obsolete preview cannot submit after an edit or close").toHaveLength(1);
        // A deliberate current launch is the positive control and ordering
        // boundary: it must be the only additional request after the late reply.
        await submit.click();
        await expect(form.locator(".create-session-error")).toContainText("fixture guarded create received");
        expect(creates).toHaveLength(2);
        if (outcome === "edit") {
          expect(creates[1]).toMatchObject({ github_checkout: { title: "edited-before-preview", preview: { cwd: "/checkout-fixture/bar-edited-before-preview" } } });
        } else {
          expect(creates[1]).toMatchObject({ cwd: "/tmp" });
          expect(creates[1].github_checkout ?? null).toBeNull();
        }
      }
      expect(creates[1].launch).toMatchObject({ permissions: "guarded" });
      expect(creates[1]).not.toHaveProperty("confirm_yolo");
      expect(creates[1].intent_key).toBeTruthy();
      expect(creates[1].intent_key).not.toBe(creates[0].intent_key);
    } finally {
      gate.release();
    }
  });
}

/** A preview can be slow or fail after checkout mode has replaced an editable
 * folder. Until the helm supplies an accepted path, the folder control must
 * disclose that absence instead of showing the unrelated old `cwd` seed. */
test("checkout folder shows pending and failed preview states without a stale path", async ({ page, request }, testInfo) => {
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
    const error = form.locator(".launch-composer-checkout-preview .create-session-error");
    await expect(error).toBeVisible();
    await form.screenshot({ path: testInfo.outputPath("checkout-preview-error.png") });
    // Form-level error padding would indent this line away from the preview
    // it explains. Keep the refusal small and aligned with those details.
    const errorStyle = await error.evaluate((el) => ({
      size: getComputedStyle(el).fontSize,
      paddingLeft: getComputedStyle(el).paddingLeft,
      lineHeight: getComputedStyle(el).lineHeight,
    }));
    expect(errorStyle.size).toBe("12px");
    expect(errorStyle.paddingLeft).toBe("0px");
    expect(errorStyle.lineHeight).toBe("18px");
    await expect(folder).toHaveValue("");
    await form.getByRole("button", { name: "folder", exact: true }).click();
    await expect(folder).toHaveJSProperty("readOnly", false);
    await expect(folder).toHaveValue(priorFolder);
    await expect(form.locator(".launch-composer-checkout-preview")).toHaveCount(0);
  } finally {
    release();
  }
});

/** Clone trusts the session's repository association without the search
 * parser. A supervisor can therefore supply formatting controls in that
 * identity. Hold its actual preview request to check the pending boundary,
 * then refuse it: both states must disclose the same escaped repository,
 * retain its raw request bytes, and require an explicit clear before editing.
 * The real source lives in the owned stack; intercepted checkout replies
 * prevent any clone from reaching GitHub. */
test("Clone discloses unsafe inherited repository text without changing its preview identity", async ({ page, request }) => {
  const host = await localHost(request);
  const source = await createSession(request, { title: "checkout-peer-source" });
  const repo = { owner: "acme", name: "repo\u202e\u200b" };
  const raw = `${repo.owner}/${repo.name}`;
  const shown = "acme/repo<U+202E><U+200B>";
  let release!: () => void;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  const previews: PreviewInput[] = [];
  let sourceListed = false;
  let creates = 0;
  let bodyFailed = true;
  let stopListing = async () => {};
  try {
    await unavailableDiscovery(page, host);
    stopListing = await drainedRoute(page, "**/api/sessions?*", async (route) => {
      // The listing helper uses independent HTTP for body rewrites: Playwright's
      // APIResponse can be disposed while json() still reads it. Drain this
      // callback before context teardown as well, retaining live listing data.
      const response = await fetch(route.request().url(), { headers: await route.request().allHeaders() });
      expect(response.ok).toBe(true);
      const body = await response.json() as { sessions: Record<string, unknown>[] };
      const row = body.sessions.find((candidate) => candidate.id === source.id);
      if (row) {
        // Exercise the association path and its precedence over launch provenance.
        row.github_repo = { owner: "other", name: "provenance" };
        row.working_copy = {
          id: "fixture-checkout", repo, canonical_path: "/checkout-fixture/source",
          origin_session_id: source.id,
        };
        sourceListed = true;
      }
      await fulfill(route, { status: response.status, json: body });
    });
    await page.route("**/api/github-checkout-preview", async (route) => {
      const input = route.request().postDataJSON() as PreviewInput;
      previews.push(input);
      if (input.repo === raw) await gate;
      try {
        await fulfill(route, { status: 400, json: { error: "fixture invalid repository" } });
      } catch (error) {
        if (!route.request().failure()) throw error;
      }
    });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      creates += 1;
      await unaccepted(route);
    });
    await page.goto("/");
    const sourceRow = page.locator(`.session-row[data-session-id="${source.id}"]`);
    await expect(sourceRow).toBeVisible();
    await expect.poll(() => sourceListed, { message: "the source listing must carry the hostile association" }).toBe(true);
    await expect(sourceRow).toContainText(shown);
    await openRowMenu(sourceRow);
    await sourceRow.locator(".session-row-clone").click();
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    const repository = form.getByRole("combobox", { name: "repository", exact: true });
    const preview = form.locator(".launch-composer-checkout-preview");
    const launch = form.locator(".create-session-submit");
    await expect.poll(() => previews.length).toBeGreaterThan(0);
    expect(previews.every((input) => input.repo === raw)).toBe(true);
    await expect(preview).toHaveAttribute("data-preview-state", "pending");
    await expect(preview).toContainText(`managed checkout of ${shown} on`);
    // The compact action and its visible footer context disclose the same
    // escaped target; the literal repository in the request remains separate.
    await expect(form.locator(".launch-composer-launch-context")).toContainText(`gh:${shown} (preview pending)`);
    expect(await launch.getAttribute("aria-label")).toContain(`gh:${shown} (preview pending)`);
    await expect(repository).toHaveValue(shown);
    await expect(repository).toHaveJSProperty("readOnly", true);
    await repository.focus();
    await expect(repository).toBeFocused();
    await repository.press("End");
    await repository.press("X");
    await expect(repository).toHaveValue(shown);
    await expect(launch).toBeDisabled();
    expect(await form.textContent()).not.toContain(repo.name);
    release();
    await expect(preview).toHaveAttribute("data-preview-state", "failed");
    await expect(preview).toContainText(`managed checkout of ${shown} on`);
    await expect(preview).toContainText("fixture invalid repository");
    await expect(form.locator(".launch-composer-launch-context")).toContainText(`gh:${shown} (preview pending)`);
    expect(await launch.getAttribute("aria-label")).toContain(`gh:${shown} (preview pending)`);
    await expect(repository).toHaveValue(shown);
    await expect(repository).toHaveJSProperty("readOnly", true);
    await expect(launch).toBeDisabled();
    expect(previews.every((input) => input.repo === raw)).toBe(true);
    expect(creates).toBe(0);

    await form.getByRole("button", { name: "clear repository", exact: true }).click();
    await expect(repository).toHaveValue("");
    await expect(repository).toBeEditable();
    await expect(preview).toHaveCount(0);
    await repository.fill("acme/replacement");
    await expect(repository).toBeFocused();
    await expect(form.getByRole("option", { name: "acme/replacement", exact: true })).toBeVisible();
    await repository.press("Enter");
    await expect.poll(() => previews.some((input) => input.repo === "acme/replacement")).toBe(true);
    await expect(repository).toHaveValue("acme/replacement");
    await expect(repository).toBeEditable();
    expect(previews.every((input) => input.repo === raw || input.repo === "acme/replacement")).toBe(true);
    expect(creates).toBe(0);
    bodyFailed = false;
  } finally {
    release();
    // Release the response reader while its context is still alive. Attempt
    // session cleanup even if draining fails, without hiding a body assertion.
    await cleanupOwnedSteps(bodyFailed, [
      ["session listing rewrite", stopListing],
      ["source session", () => cleanupSession(request, source.id)],
    ]);
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

/**
 * Why this matters: a fresh-checkout "Replace with" retries by resending its
 * retained request, and the source delete it ends with must still authorize
 * only what the launcher shows at that press (SPEC.md "Replace with"); a
 * retry that reused the first press's permission could kill a session the
 * launcher no longer warned about. Spec: the first press on a running source
 * sends neither precondition flag; after an ambiguous failure and the
 * source's exit reaching the page, the launcher's warning is gone and the
 * retry sends `only_if_nothing_alive` with the identical retained create body.
 */
test("a fresh-checkout replace-with retry carries the precondition shown at that press", async ({ page, request }) => {
  const host = await localHost(request);
  const title = `fresh-replace-retry-${Date.now()}`;
  const source = await createSession(request, { title });
  try {
    await expect
      .poll(
        async () =>
          LIVE_STATES.includes((await listSessions(request)).sessions.find((one) => one.id === source.id)?.status?.state ?? ""),
        { timeout: 20_000 },
      )
      .toBe(true);
    await unavailableDiscovery(page, host);
    await page.route("**/api/github-checkout-preview", async (route) => {
      await fulfill(route, { json: previewFor(route.request().postDataJSON(), host, 1) });
    });
    const replaces: { only_if_nothing_alive: boolean; only_if_agent_ended: boolean; with: unknown }[] = [];
    await page.route(`**/api/sessions/${encodeURIComponent(source.id)}/replace`, async (route) => {
      replaces.push(route.request().postDataJSON());
      if (replaces.length === 1) await route.abort("failed");
      else await fulfill(route, { status: 409, json: { error: "fixture unresolved conflict" } });
    });
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    const sourceRow = page.locator(`.session-row[data-session-id="${source.id}"]`);
    await expect(sourceRow.locator(".status-badge")).toHaveText(LIVE_BADGE, { timeout: 20_000 });
    await openRowMenu(sourceRow);
    await sourceRow.locator(".session-row-replace-with").click();
    const form = page.locator('.create-session-form[role="dialog"]');
    await expect(form).toBeVisible();
    // A command-launch source opens the command tab.
    await form.getByRole("tab", { name: "agent", exact: true }).click();
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await selectRepo(page);
    // The copied source title is not a name for a fresh checkout, so the
    // fixture offers its unnamed path rather than one named after the source.
    await expect(form.getByLabel("folder", { exact: true })).toHaveValue("/checkout-fixture/bar-1");
    const warning = form.locator(".launch-composer-replace-warning");
    await expect(warning).toContainText("still running");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".create-session-error")).toContainText("original request is retained");
    expect(replaces).toHaveLength(1);
    expect(replaces[0]).toMatchObject({ only_if_nothing_alive: false, only_if_agent_ended: false });

    await stopSession(request, source.id);
    await expect
      .poll(async () => (await listSessions(request)).sessions.find((one) => one.id === source.id)?.status?.state, {
        timeout: 20_000,
      })
      .toBe("exited");
    feed.notify(2);
    await expect(warning).toBeEmpty({ timeout: 20_000 });
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    await form.locator(".create-session-submit").click();
    await expect.poll(() => replaces.length).toBe(2);
    expect(replaces[1]).toMatchObject({ only_if_nothing_alive: true, only_if_agent_ended: false });
    expect(replaces[1].with).toEqual(replaces[0].with);
  } finally {
    await cleanupSession(request, source.id);
  }
});

/** Why: Clone and Replace with copy the source's title, and a fresh checkout
 * named after that title collides with the source's own checkout, so every
 * such launch used to be refused. Spec (SPEC.md, Managed checkouts):
 * while a fresh checkout is the destination and the copied title is
 * unedited, the preview and the launch body both carry an empty title, the
 * name field is empty with the unnamed `repo-N` as its placeholder, a return
 * to a folder restores the copied title, and a typed title is sent as typed.
 * The request bodies are the oracle; the intercepted replies keep any clone
 * from running. */
for (const action of ["clone", "replace-with"] as const) {
  test(`${action} into a fresh checkout drops the copied title unless the user edits it`, async ({ page, request }) => {
    const host = await localHost(request);
    const title = `copied-title-${action}-${Date.now()}`;
    const source = await createSession(request, { title });
    try {
      await unavailableDiscovery(page, host);
      const previews: PreviewInput[] = [];
      await page.route("**/api/github-checkout-preview", async (route) => {
        const input = route.request().postDataJSON() as PreviewInput;
        previews.push(input);
        await fulfill(route, { json: previewFor(input, host, 1) });
      });
      // Clone creates and Replace with replaces; both are refused before any
      // allocation so the test can launch twice from one dialog.
      const launches: Record<string, unknown>[] = [];
      const endpoint = action === "clone" ? "**/api/sessions" : `**/api/sessions/${encodeURIComponent(source.id)}/replace`;
      await page.route(endpoint, async (route) => {
        if (route.request().method() !== "POST") return route.continue();
        const body = route.request().postDataJSON();
        launches.push(action === "clone" ? body : body.with);
        await unaccepted(route);
      });
      await page.goto("/");
      const sourceRow = page.locator(`.session-row[data-session-id="${source.id}"]`);
      await expect(sourceRow).toBeVisible({ timeout: 20_000 });
      await openRowMenu(sourceRow);
      await sourceRow.locator(`.session-row-${action}`).click();
      const form = page.locator('.create-session-form[role="dialog"]');
      await expect(form).toBeVisible();
      // A command-launch source opens the command tab.
      await form.getByRole("tab", { name: "agent", exact: true }).click();
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
      const name = form.getByLabel("name (optional)", { exact: true });
      const folder = form.getByLabel("folder", { exact: true });
      // Premise: the dialog opened as a Clone/Replace with, carrying the title.
      await expect(name).toHaveValue(title);

      await selectRepo(page);
      await expect(folder).toHaveValue("/checkout-fixture/bar-1");
      await expect(name).toHaveValue("");
      await expect(name).toHaveAttribute("placeholder", "bar-1");
      expect(previews.length).toBeGreaterThan(0);
      expect(previews.every((input) => input.title === ""), JSON.stringify(previews)).toBe(true);
      await expect(form.locator(".create-session-submit")).toBeEnabled();
      await form.locator(".create-session-submit").click();
      await expect.poll(() => launches.length).toBe(1);
      expect(launches[0]).toMatchObject({
        title: null, cwd: "/checkout-fixture/bar-1", github_checkout: { repo: "acme/bar", title: "" },
      });

      // A folder destination makes the copied title the session's name again.
      await form.getByRole("button", { name: "folder", exact: true }).click();
      await expect(folder).toHaveJSProperty("readOnly", false);
      await expect(name).toHaveValue(title);
      await expect(name).toHaveJSProperty("placeholder", "");

      // Re-entering the checkout drops the copy again; the preview for that
      // selection, not just the field, must carry the empty title.
      const previewsBefore = previews.length;
      await selectRepo(page);
      await expect(name).toHaveValue("");
      await expect(name).toHaveAttribute("placeholder", "bar-1");
      expect(previews.length).toBeGreaterThan(previewsBefore);
      expect(previews.slice(previewsBefore).every((input) => input.title === ""), JSON.stringify(previews)).toBe(true);
      await name.fill("typed");
      await expect(folder).toHaveValue("/checkout-fixture/bar-typed");
      await expect(name).toHaveJSProperty("placeholder", "");
      await expect(form.locator(".create-session-submit")).toBeEnabled();
      await form.locator(".create-session-submit").click();
      await expect.poll(() => launches.length).toBe(2);
      expect(launches[1]).toMatchObject({
        title: "typed", cwd: "/checkout-fixture/bar-typed", github_checkout: { repo: "acme/bar", title: "typed" },
      });
    } finally {
      await cleanupSession(request, source.id);
    }
  });
}

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
    await form.getByRole("button", { name: "folder", exact: true }).click();
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
  await expect(form.getByRole("option", { name: "Managed checkout: acme/bar", exact: true })).toBeVisible();
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
 * cannot hide behind an already-ready replacement response. Retaining the
 * selected repository on B must still discover its missing-root flag and
 * offer setup independently of that held preview. */
test("switching hosts rejects the old checkout preview and checks setup for the selected repository", async ({ page, request }) => {
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
      repos: [], truncated: false, scan_error: null, needs_checkout_root: host.id === remote!.id,
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
    await expect(form.locator(".checkout-root-field")).toHaveCount(0);
    const hostPicker = form.getByRole("combobox", { name: "host", exact: true });
    await chooseLauncherHost(hostPicker, String(remote!.id));
    await expect(hostPicker).toHaveAttribute("data-host-id", String(remote!.id));
    await expect.poll(() => remoteStarted).toBe(true);
    // Keeping the repository selected must not suppress the new host's first
    // setup check. Its pending preview supplies no setup signal; discovery's
    // host-bound flag is the only authority for this field.
    await expect(form.locator(".checkout-root-field")
      .getByRole("textbox", { name: "checkout folder", exact: true })).toBeEditable();
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
    const stack = JSON.parse(await readFile(currentStackSlot().stackInfoPath, "utf8")) as {
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
