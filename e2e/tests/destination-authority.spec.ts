import { chooseHost, hostChoice } from "./helpers/host-rail";
import { chooseLauncherHost } from "./helpers/host-picker";
import { fillLauncherFolder } from "./helpers/launcher";
// Destination snapshots must survive sidebar projection changes without
// letting a remembered path follow a registry row onto another installation.
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Locator, type Page, type Route } from "@playwright/test";
import { cleanupSession, createSession, localHostId, patchPreferences, pinAutoSelect, SESSION_LISTING, stubFeed } from "./helpers/fleet";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";

/** Preserve the real connected registry and build stamp before controlling its
 * presentation. These tests replace browser-visible authority, never adopt or
 * retarget a real host or launch a process on the synthetic installation. */
async function registryFixture(request: APIRequestContext) {
  const local = await localHostId(request);
  await expect.poll(async () => {
    const response = await request.get("/api/hosts");
    expect(response.ok()).toBe(true);
    const listing = await response.json();
    return listing.hosts.find((host: { id: number }) => host.id !== local)?.state.phase;
  }, { message: "the owned remote supervisor must be connected before testing its installation authority" }).toBe("connected");
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build, "controlled replies must retain the stack build identity").toBeTruthy();
  const listing = await response.json();
  const remote = listing.hosts.find((host: { id: number }) => host.id !== local);
  expect(remote, "the owned browser stack must supply its remote fixture").toBeTruthy();
  expect(remote.identity, "installation replacement needs a known predecessor identity").toBeTruthy();
  expect(remote.incarnation, "the remote fixture must have a connection claim").toBeGreaterThan(0);
  expect(remote.state.phase, "the registry snapshot must still describe a connected remote").toBe("connected");
  return { listing, local, remote, build };
}

for (const replace of [false, true]) {
  /** Filtering removes only a sidebar projection. The selected terminal owns
   * the destination snapshot; a replacement identity must instead fall back
   * to the local host and its own default folder, never carry the remote
   * folder to local. Recent setups are stubbed empty, so that default is home
   * whatever earlier tests launched on the shared helm. */
  test(`New inherits a filtered selected destination with replacement=${replace}`, async ({ page, request }) => {
    const fixture = await registryFixture(request);
    const active = await createSession(request, {
      title: `filtered-destination-${Date.now()}`,
      cwd: "/tmp",
      host: fixture.remote.id,
    });
    let replacement = false;
    await page.route("**/api/launch-history**", (route) => route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": fixture.build },
      body: JSON.stringify({ launches: [], folders: [] }),
    }));
    const feed = await stubFeed(page);
    // These cases require a healthy feed, not an ungreeted outage. Match the
    // helm's greeting so its silence deadline cannot remove our notification peer.
    feed.notifyOnConnect(0);
    await page.route("**/api/hosts", async (route) => {
      const listing = structuredClone(fixture.listing);
      if (replacement) {
        const remote = listing.hosts.find((host: { id: number }) => host.id === fixture.remote.id);
        remote.identity = `${fixture.remote.identity}-replacement`;
        remote.incarnation += 1;
        remote.name = "replacement destination";
      }
      await route.fulfill({ status: 200, headers: { "content-type": "application/json", "x-farhelm-build": fixture.build }, body: JSON.stringify(listing) });
    });
    try {
      expect(active.cwd, "the created remote session must carry its canonical temporary folder").toMatch(/\/tmp$/);
      await pinAutoSelect(page, active.id);
      await page.goto("/");
      await feed.waitForConnection(1);
      const row = page.locator(`[data-session-id="${active.id}"]`);
      await expect(row).toBeVisible();
      await row.locator(".session-row-open").click();
      await waitForSessionRevealed(page, active.id);
      const terminal = await page.locator("#terminal").elementHandle();
      expect(terminal).toBeTruthy();
      await chooseHost(page, String(fixture.local));
      await expect(row, "the server-backed filter must actually remove the selected row").toHaveCount(0);
      await expect(page.locator(".session-details .title")).toHaveText(active.title);
      expect(await terminal!.evaluate((node) => node === document.querySelector("#terminal")), "filtering must preserve the same terminal element").toBe(true);
      if (replace) {
        replacement = true;
        expect(feed.openSockets(), "replacement notification requires a live feed peer").toBe(1);
        feed.notify(1);
        await expect(hostChoice(page, fixture.remote.id), "the replacement registry must be rendered before opening New").toHaveAttribute("aria-label", /replacement destination/);
      }
      await page.locator(".new-session-button").click();
      const form = page.locator(".create-session-form");
      await expect(form.locator(".create-session-host")).toHaveAttribute("data-host-id", String(replace ? fixture.local : fixture.remote.id));
      await expect(form.getByLabel("Folder", { exact: true })).toHaveValue(replace ? "~" : active.cwd);
      await expect(form.locator(".launch-composer-harness-choice button[aria-pressed=true]")).toHaveCount(0);
      await expect(form.locator(".create-session-submit")).toBeDisabled();
    } finally {
      await page.close();
      await cleanupSession(request, active.id);
    }
  });
}

/** Offer one installation-bound history row and expose registry transitions
 * through a controlled feed. A later POST is refused at this test boundary;
 * the body, not a synthetic filesystem failure, proves which destination the
 * component would submit after the explicit correction. The ordinary retry
 * mode adds complete Guarded choices and numbered unmarked refusals while
 * retaining the same live registry and installation authority. */
async function historyFixture(page: Page, request: APIRequestContext, ordinaryRetry = false) {
  const fixture = await registryFixture(request);
  const state = { reconnects: 0, replaced: false };
  const posts: Array<Record<string, unknown> & { host: number; cwd: string; expected_incarnation: number }> = [];
  const feed = await stubFeed(page);
  // Holding key creation must not also stage a silent-feed outage. The real
  // helm greets every subscription; preserve that independent fixture premise.
  feed.notifyOnConnect(0);
  const headers = { "content-type": "application/json", "x-farhelm-build": fixture.build };
  await page.route("**/api/hosts", async (route) => {
    const listing = structuredClone(fixture.listing);
    const remote = listing.hosts.find((host: { id: number }) => host.id === fixture.remote.id);
    remote.incarnation += state.reconnects;
    if (state.replaced) remote.identity = `${fixture.remote.identity}-replacement`;
    await route.fulfill({ status: 200, headers, body: JSON.stringify(listing) });
  });
  await page.route("**/api/launch-catalog", (route) => route.fulfill({ status: 200, headers, body: "[]" }));
  await page.route("**/api/launch-history**", (route) => route.fulfill({
    status: 200,
    headers,
    body: JSON.stringify(state.replaced ? { launches: [], folders: [] } : {
      launches: ordinaryRetry ? [
        { host: fixture.remote.id, cwd: "/tmp", canonical_cwd: "/tmp", selection: { harness: "codex", model: null, effort: null, permissions: "guarded" }, created_at: 2, creation_seq: 2 },
        { host: fixture.remote.id, cwd: "/tmp", canonical_cwd: "/tmp", selection: { harness: "codex", model: "placement-model", effort: null, permissions: "guarded" }, created_at: 1, creation_seq: 1 },
      ] : [{ host: fixture.remote.id, cwd: "/tmp", canonical_cwd: "/tmp", selection: { harness: "codex", model: null, effort: null, permissions: null }, created_at: 2, creation_seq: 2 }],
      folders: ["/tmp", "/var/tmp"].map((cwd, index) => ({ host: fixture.remote.id, canonical_cwd: cwd, canonical_proven: true, display_cwd: cwd, created_at: 2 - index, creation_seq: 2 - index })),
    }),
  }));
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts.push(route.request().postDataJSON());
    if (ordinaryRetry) {
      // No definite-outcome header: this completed refusal leaves an
      // unresolved retry pair in the mounted launcher.
      await route.fulfill({ status: 400, headers: { ...headers, "content-type": "text/plain" }, body: `placement fixture refused create ${posts.length}` });
    } else {
      await route.fulfill({ status: 409, headers, body: JSON.stringify("destination fixture refuses launch") });
    }
  });
  await page.goto("/");
  await feed.waitForConnection(1);
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await chooseLauncherHost(form.locator(".create-session-host"), String(fixture.remote.id));
  await expect(form, "the actual remote connection must reach the mounted composer").toHaveAttribute("data-browse-live-connection", String(fixture.remote.incarnation));
  // Retry cases next use controls outside the floating suggestions. The
  // authority cases deliberately retain those options for saved callbacks.
  if (ordinaryRetry) await fillLauncherFolder(form, "/tmp");
  else await form.getByLabel("Folder", { exact: true }).fill("/tmp");
  return { ...fixture, state, posts, feed, form };
}

for (const source of ["ordinary recent", "search recent", "saved folder"] as const) {
  /** The draft path stays fixed across connected registry snapshots. The
   * form's local refusal before any POST distinguishes lost authority from
   * a downstream filesystem error. A same-install reconnect is the control;
   * only replacement requires an explicit new destination choice. */
  test(`applied ${source} retains installation authority through Launch`, async ({ page, request }) => {
    const fixture = await historyFixture(page, request);
    const { form, state, feed, remote, local, posts } = fixture;
    const folder = form.getByLabel("Folder", { exact: true });
    const launch = form.locator(".create-session-submit");
    if (source === "ordinary recent") {
      await form.locator(".launch-composer-recent-slots button").first().click();
    } else if (source === "search recent") {
      await form.getByRole("combobox", { name: /search/i }).fill("codex");
      await form.getByRole("option", { name: /^Recent setup:.*\/tmp/ }).click();
    } else {
      await folder.focus();
      await form.getByRole("listbox", { name: "Folders" }).getByRole("option", { name: "/tmp", exact: true }).click();
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: /Codex$/ }).click();
    }
    await expect(form).toHaveAttribute("data-history-activation-attempts", "1");
    await expect(folder).toHaveValue("/tmp");
    await expect(launch).toBeEnabled();
    state.reconnects = 1;
    expect(feed.openSockets(), "reconnect notification requires a live feed peer").toBe(1);
    feed.notify(1);
    await expect(form, "same-install reconnect must be consumed before testing continuity").toHaveAttribute("data-browse-live-connection", String(remote.incarnation + 1));
    await expect(form).toHaveAttribute("data-remembered-destination-valid", "true");
    await expect(launch).toBeEnabled();
    state.replaced = true;
    state.reconnects = 2;
    expect(feed.openSockets(), "replacement notification requires a live feed peer").toBe(1);
    feed.notify(2);
    await expect(form, "connected replacement must be consumed before checking refusal").toHaveAttribute("data-browse-live-connection", String(remote.incarnation + 2));
    await expect(form).toHaveAttribute("data-remembered-destination-valid", "false");
    await expect(folder).toHaveValue("/tmp");
    await expect(launch).toBeDisabled();
    await expect(form.locator(".create-session-host-note")).toContainText("remembered folder belongs to a different installation");
    // requestSubmit bypasses only the disabled button. The component's own
    // refusal is the oracle that its submit handler ran and sent no POST.
    await form.evaluate((node) => (node as HTMLFormElement).requestSubmit());
    await expect(form.locator(".create-session-error")).toContainText("remembered folder belongs to a different installation");
    expect(posts).toHaveLength(0);
    // Choosing the committed host is a no-op. Choose away and back to make
    // an explicit correction, rather than manufacturing a same-value change.
    await chooseLauncherHost(form.locator(".create-session-host"), String(local));
    await expect(form.locator(".create-session-host")).toHaveAttribute("data-host-id", String(local));
    await chooseLauncherHost(form.locator(".create-session-host"), String(remote.id));
    await expect(form).toHaveAttribute("data-remembered-destination-valid", "true");
    await expect(launch).toBeEnabled();
    await launch.click();
    await expect(form.locator(".create-session-error")).toContainText("destination fixture refuses launch");
    expect(posts).toHaveLength(1);
    expect(posts[0]).toMatchObject({ host: remote.id, cwd: "/tmp", expected_incarnation: remote.incarnation + 2 });
  });
}

/** A captured history callback must observe a host change from the same event
 * turn. The handler counter proves this exercised refusal rather than merely
 * dispatching at a node already withdrawn from the document. */
test("queued saved-folder activation cannot follow an explicit host change", async ({ page, request }) => {
  const { form, local } = await historyFixture(page, request);
  const folder = form.getByLabel("Folder", { exact: true });
  await folder.focus();
  const saved = form.getByRole("listbox", { name: "Folders" }).getByRole("option", { name: "/var/tmp", exact: true });
  await expect(saved).toBeVisible();
  // A native pointer press blurs the folder and withdraws its options. Open
  // the host picker with an unfocused key event instead: both callbacks stay
  // mounted until the one event turn that changes host then picks old history.
  const attempts = Number(await form.getAttribute("data-history-activation-attempts"));
  await form.locator(".create-session-host").evaluate((node) => node.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })));
  await expect(form.locator(`.launcher-host-option[data-host-id="${local}"]`)).toBeVisible();
  await expect(folder, "unfocused key setup must leave the folder callback mounted").toBeFocused();
  await expect(saved).toBeVisible();
  await form.evaluate((node, local) => {
    const host = node.querySelector<HTMLButtonElement>(`.launcher-host-option[data-host-id="${local}"]`)!;
    const saved = node.querySelector<HTMLButtonElement>('.launch-composer-folder-options button[aria-label="/var/tmp"]')!;
    host.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    saved.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  }, local);
  await expect(form).toHaveAttribute("data-history-activation-attempts", String(attempts + 1));
  await expect(form.locator(".create-session-host")).toHaveAttribute("data-host-id", String(local));
  await expect(form.getByLabel("Folder", { exact: true })).toHaveValue("/tmp");
});

/** The UUID eval is awaited by the production web renderer. Hold that return
 * after a valid submit, consume a replacement registry, then release it. This
 * isolates the post-mint guard: the initial submit saw a valid installation.
 * Resolving the held promise in cleanup keeps a failed assertion from leaving
 * a pending operation alive while the browser context is torn down. */
test("remembered destination is rechecked after key minting", async ({ page, request }) => {
  const { form, state, feed, remote, local, posts } = await historyFixture(page, request);
  await form.locator(".launch-composer-recent-slots button").first().click();
  await expect(form).toHaveAttribute("data-remembered-destination-valid", "true");
  const launch = form.locator(".create-session-submit");
  await expect(launch).toBeEnabled();
  await page.evaluate(() => {
    const original = Object.getOwnPropertyDescriptor(Crypto.prototype, "randomUUID")!;
    const id = crypto.randomUUID();
    const state = { entered: 0, release: () => {} };
    (window as any).__destinationMint = state;
    Object.defineProperty(Crypto.prototype, "randomUUID", {
      configurable: true,
      value: () => {
        state.entered += 1;
        return new Promise<string>((resolve) => {
          state.release = () => {
            Object.defineProperty(Crypto.prototype, "randomUUID", original);
            resolve(id);
          };
        });
      },
    });
  });
  try {
    await launch.click();
    await expect.poll(() => page.evaluate(() => (window as any).__destinationMint.entered), {
      message: "the create must enter and hold its UUID eval before replacing the registry",
    }).toBe(1);
    expect(posts).toHaveLength(0);
    state.replaced = true;
    state.reconnects = 1;
    expect(feed.openSockets(), "post-mint replacement requires a live feed peer").toBe(1);
    feed.notify(1);
    await expect(form).toHaveAttribute("data-browse-live-connection", String(remote.incarnation + 1));
    await expect(form).toHaveAttribute("data-remembered-destination-valid", "false");
    await page.evaluate(() => (window as any).__destinationMint.release());
    await expect(form.locator(".create-session-error")).toContainText("remembered folder belongs to a different installation");
    expect(posts).toHaveLength(0);
    // A fresh explicit choice also proves the refusal released the shared
    // operation guard; a leaked guard would leave this selector disabled.
    await chooseLauncherHost(form.locator(".create-session-host"), String(local));
    await expect(form.locator(".create-session-host")).toHaveAttribute("data-host-id", String(local));
    await chooseLauncherHost(form.locator(".create-session-host"), String(remote.id));
    await expect(launch).toBeEnabled();
  } finally {
    await page.evaluate(() => (window as any).__destinationMint.release());
  }
});

/** Reopen without editing the path: refocusing an already focused field
 * need not fire onfocus, and typing would independently retire the key.
 */
async function placementFolderOptions(form: Locator) {
  const anchor = form.locator(".create-session-submit");
  await anchor.focus();
  await expect(anchor).toBeFocused();
  const folder = form.getByRole("combobox", { name: "Folder", exact: true });
  await folder.focus();
  await expect(folder).toBeFocused();
  const list = form.getByRole("listbox", { name: "Folders", exact: true });
  await expect(list).toBeVisible();
  return list;
}

/** Observe completion of this request, not an earlier refusal or busy draft. */
async function submitPlacementRetry(form: Locator, posts: Record<string, unknown>[]) {
  const before = posts.length;
  const launch = form.locator(".create-session-submit");
  await expect(launch).toHaveAttribute("aria-disabled", "false");
  await launch.click();
  await expect(form.locator(".create-session-error")).toContainText(`placement fixture refused create ${before + 1}`);
  await expect(launch).toHaveAttribute("aria-disabled", "false");
  expect(posts).toHaveLength(before + 1);
  return posts[before];
}

/** The whole body and key must survive, not merely the displayed path. */
function expectPlacementRetry(previous: Record<string, unknown>, next: Record<string, unknown>) {
  expect(next.intent_key).toEqual(expect.any(String));
  expect(next.intent_key).not.toBe("");
  expect(next.intent_key).toBe(previous.intent_key);
  expect(next).toEqual(previous);
}

/** A real path or recent-selection edit changes substantive request data,
 * not only a needlessly reminted key.
 */
function expectPlacementChange(previous: Record<string, unknown>, next: Record<string, unknown>) {
  const { intent_key: oldKey, ...oldRequest } = previous;
  const { intent_key: newKey, ...newRequest } = next;
  expect(newKey).toEqual(expect.any(String));
  expect(newKey).not.toBe("");
  expect(newKey).not.toBe(oldKey);
  expect(newRequest).not.toEqual(oldRequest);
}

/** The stable connection and valid installation claim are premises of the
 * no-op comparison, rather than a synthetic retarget treated as equivalent.
 */
async function expectPlacementAuthority(form: Locator, remote: { id: number; incarnation: number }, cwd: string) {
  await expect(form.locator(".create-session-host")).toHaveAttribute("data-host-id", String(remote.id));
  await expect(form).toHaveAttribute("data-browse-live-connection", String(remote.incarnation));
  await expect(form).toHaveAttribute("data-remembered-destination-valid", "true");
  await expect(form.getByLabel("Folder", { exact: true })).toHaveValue(cwd);
}

/** Selecting Home, saved history or the same literal path must not replace
 * an unresolved operation. A different raw path remains a new operation.
 */
test("unchanged Home and folder selections preserve the unresolved create", async ({ page, request }) => {
  const { form, posts, remote } = await historyFixture(page, request, true);
  await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "Guarded", exact: true }).click();
  let list = await placementFolderOptions(form);
  await list.getByRole("option", { name: "Reset folder to home", exact: true }).click();
  await expect(list).toHaveCount(0);
  await expectPlacementAuthority(form, remote, "~");
  expect(posts).toEqual([]);
  let previous = await submitPlacementRetry(form, posts);
  expect(previous.intent_key).toEqual(expect.any(String));
  expect(previous.intent_key).not.toBe("");
  expect(previous).toMatchObject({ host: remote.id, cwd: "~", expected_incarnation: remote.incarnation });

  let before = posts.length;
  list = await placementFolderOptions(form);
  await list.getByRole("option", { name: "Reset folder to home", exact: true }).click();
  await expect(list).toHaveCount(0);
  await expectPlacementAuthority(form, remote, "~");
  expect(posts, "Home only reapplies the folder").toHaveLength(before);
  let next = await submitPlacementRetry(form, posts);
  expectPlacementRetry(previous, next);
  previous = next;

  // The first saved-folder choice is a real path change and establishes
  // admitted history provenance before either history no-op is tested.
  let attempts = Number(await form.getAttribute("data-history-activation-attempts"));
  before = posts.length;
  list = await placementFolderOptions(form);
  await list.getByRole("option", { name: "/tmp", exact: true }).click();
  await expect(list).toHaveCount(0);
  await expect(form).toHaveAttribute("data-history-activation-attempts", String(attempts + 1));
  await expectPlacementAuthority(form, remote, "/tmp");
  expect(posts).toHaveLength(before);
  next = await submitPlacementRetry(form, posts);
  expect(next).toMatchObject({ host: remote.id, cwd: "/tmp" });
  expectPlacementChange(previous, next);
  previous = next;

  for (const surface of ["dropdown", "search"] as const) {
    await test.step(`same admitted saved folder via ${surface}`, async () => {
      await expectPlacementAuthority(form, remote, "/tmp");
      attempts = Number(await form.getAttribute("data-history-activation-attempts"));
      before = posts.length;
      if (surface === "dropdown") {
        list = await placementFolderOptions(form);
        await list.getByRole("option", { name: "/tmp", exact: true }).click();
        await expect(list).toHaveCount(0);
      } else {
        const search = form.locator('.launch-composer-search input[role="combobox"]');
        await search.focus();
        await expect(search).toBeFocused();
        // A non-path query cannot offer the literal UsePath action. This
        // uniquely reaches the installation-bound saved Folder callback.
        await search.fill("folder:tmp");
        const saved = form.getByRole("group", { name: "Folders", exact: true })
          .getByRole("option", { name: "Use this path: /tmp", exact: true });
        await expect(saved).toHaveCount(1);
        await saved.click();
        await expect(search).toHaveValue("");
        await expect(search).toBeFocused();
      }
      await expect(form).toHaveAttribute("data-history-activation-attempts", String(attempts + 1));
      await expectPlacementAuthority(form, remote, "/tmp");
      expect(posts, "saved-folder acceptance must not launch").toHaveLength(before);
      next = await submitPlacementRetry(form, posts);
      expectPlacementRetry(previous, next);
      previous = next;
    });
  }

  for (const [path, changed] of [["/placement-plain", true], ["/placement-plain", false], ["/placement-changed", true]] as const) {
    await test.step(`literal ${path}, changed=${changed}`, async () => {
      before = posts.length;
      attempts = Number(await form.getAttribute("data-history-activation-attempts"));
      const search = form.locator('.launch-composer-search input[role="combobox"]');
      await search.focus();
      await expect(search).toBeFocused();
      await search.fill(path);
      const direct = form.getByRole("option", { name: `Use this path: ${path}`, exact: true });
      await expect(direct).toHaveCount(1);
      await direct.click();
      await expect(search).toHaveValue("");
      await expect(search).toBeFocused();
      await expectPlacementAuthority(form, remote, path);
      await expect(form).toHaveAttribute("data-history-activation-attempts", String(attempts));
      expect(posts, "literal path acceptance must not launch").toHaveLength(before);
      next = await submitPlacementRetry(form, posts);
      expect(next).toMatchObject({ host: remote.id, cwd: path });
      if (changed) expectPlacementChange(previous, next);
      else expectPlacementRetry(previous, next);
      previous = next;
    });
  }
});

/** Existing-folder recents share one retry through pointer, Enter and search.
 * Enter intentionally submits exactly once; pointer and search only select.
 */
test("unchanged existing-folder recents retain retry identity across their controls", async ({ page, request }) => {
  const { form, posts, remote } = await historyFixture(page, request, true);
  const rows = form.locator(".launch-composer-recent-slots");
  const original = rows.getByRole("button", { name: /Model: Default · Effort: Default · Permissions: Guarded$/ });
  await expect(original).toHaveCount(1);
  let attempts = Number(await form.getAttribute("data-history-activation-attempts"));
  await original.click();
  await expect(form).toHaveAttribute("data-history-activation-attempts", String(attempts + 1));
  await expectPlacementAuthority(form, remote, "/tmp");
  await expect(form.getByRole("combobox", { name: "Model", exact: true })).toHaveValue("Harness default");
  await expect(form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "Guarded", exact: true })).toHaveAttribute("aria-pressed", "true");
  expect(posts).toEqual([]);
  let previous = await submitPlacementRetry(form, posts);
  expect(previous.intent_key).toEqual(expect.any(String));
  expect(previous.intent_key).not.toBe("");
  expect(previous).toMatchObject({ host: remote.id, cwd: "/tmp", launch: { harness: "codex", model: null, effort: null, permissions: "guarded" } });

  for (const surface of ["pointer", "Enter", "search"] as const) {
    await test.step(`same admitted recent via ${surface}`, async () => {
      await expectPlacementAuthority(form, remote, "/tmp");
      await expect(original).toBeVisible();
      attempts = Number(await form.getAttribute("data-history-activation-attempts"));
      const before = posts.length;
      let next: Record<string, unknown>;
      if (surface === "Enter") {
        await original.focus();
        await expect(original).toBeFocused();
        await original.press("Enter");
        await expect(form.locator(".create-session-error")).toContainText(`placement fixture refused create ${before + 1}`);
        await expect(form.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "false");
        expect(posts, "recent Enter invokes ordinary Launch exactly once").toHaveLength(before + 1);
        next = posts[before];
      } else {
        if (surface === "pointer") await original.click();
        else {
          const search = form.locator('.launch-composer-search input[role="combobox"]');
          await search.focus();
          await expect(search).toBeFocused();
          await search.fill("recent:codex");
          const saved = form.getByRole("option", { name: /^Recent setup:.*Model: Default · Effort: Default · Permissions: Guarded$/ });
          await expect(saved).toHaveCount(1);
          await saved.click();
          await expect(search).toHaveValue("");
          await expect(search).toBeFocused();
        }
        await expect(form).toHaveAttribute("data-history-activation-attempts", String(attempts + 1));
        expect(posts, "pointer/search recent acceptance only selects").toHaveLength(before);
        next = await submitPlacementRetry(form, posts);
      }
      await expect(form).toHaveAttribute("data-history-activation-attempts", String(attempts + 1));
      await expectPlacementAuthority(form, remote, "/tmp");
      expectPlacementRetry(previous, next);
      previous = next;
    });
  }

  const changed = rows.getByRole("button", { name: /Model: placement-model · Effort: Default · Permissions: Guarded$/ });
  await expect(changed, "the unfiltered model dimension must still offer the changed recent").toBeVisible();
  const before = posts.length;
  attempts = Number(await form.getAttribute("data-history-activation-attempts"));
  await changed.click();
  await expect(form).toHaveAttribute("data-history-activation-attempts", String(attempts + 1));
  await expect(form.getByRole("combobox", { name: "Model", exact: true })).toHaveValue("placement-model");
  await expectPlacementAuthority(form, remote, "/tmp");
  expect(posts).toHaveLength(before);
  const next = await submitPlacementRetry(form, posts);
  expect(next).toMatchObject({ launch: { model: "placement-model" } });
  expectPlacementChange(previous, next);
});

/** Equal BrowsePath still reads the selected installation, and selecting the
 * current directory still runs its guarded callback, without replacing an
 * unresolved create. Real child/parent path changes must replace that intent.
 */
test("equal browse and current-directory choices preserve the unresolved create", async ({ page, request }) => {
  const { form, posts, remote, build } = await historyFixture(page, request, true);
  const root = "/tmp";
  const child = "/tmp/retry-child";
  const browseBodies: Record<string, unknown>[] = [];
  const endpoint = "**/api/browse-directory";
  const browseRoute = async (route: Route) => {
    const body = route.request().postDataJSON();
    // These counters and held-intent assertions describe explicit Browse.
    // Advisory checks have a separate generation and never activate a path.
    if (body.membership_only) return route.continue();
    browseBodies.push(body);
    // These bounded replies observe paths; they grant no new registry or
    // installation identity. The real mounted connection remains authority.
    const listing = body.cwd === child
      ? { cwd: child, parent: root, children: [], truncated: false }
      : { cwd: root, parent: "/", children: [child], truncated: false };
    await route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": build },
      body: JSON.stringify(listing),
    });
  };
  await page.route(endpoint, browseRoute);
  try {
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "Guarded", exact: true }).click();
    // historyFixture typed this path directly, without accepting history.
    // An equal browse therefore clears no previously invalid saved claim.
    await expect(form).toHaveAttribute("data-history-activation-attempts", "0");
    await expectPlacementAuthority(form, remote, root);
    await expect(form).toHaveAttribute("data-browse-reply-completions", /^\d+$/);
    await expect(form).toHaveAttribute("data-browse-activation-attempts", /^\d+$/);
    let previous = await submitPlacementRetry(form, posts);
    expect(previous.intent_key).toEqual(expect.any(String));
    expect(previous.intent_key).not.toBe("");
    expect(previous).toMatchObject({ host: remote.id, cwd: root, expected_incarnation: remote.incarnation });

    /** Wait for this read's completion and accepted listing. A counter alone
     * also advances for discarded replies, so the current button is required.
     */
    const readDirectory = async (cwd: string, action: () => Promise<unknown>) => {
      await expectPlacementAuthority(form, remote, cwd);
      const before = browseBodies.length;
      const completions = Number(await form.getAttribute("data-browse-reply-completions"));
      const creates = posts.length;
      await action();
      await expect.poll(() => browseBodies.length, { message: "the actual browse callback must dispatch this directory read" }).toBe(before + 1);
      expect(browseBodies[before]).toEqual({ host: remote.id, cwd, expected_incarnation: remote.incarnation });
      await expect(form).toHaveAttribute("data-browse-reply-completions", String(completions + 1));
      const browser = form.locator(".launch-composer-browser");
      await expect(browser.getByRole("button", { name: `Use ${cwd}`, exact: true })).toBeVisible();
      await expectPlacementAuthority(form, remote, cwd);
      expect(posts, "directory observation must not create a session").toHaveLength(creates);
      return browser;
    };

    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.focus();
    await expect(search).toBeFocused();
    await search.fill(root);
    const browseSearch = form.getByRole("option", { name: `Browse this path: ${root}`, exact: true });
    await expect(browseSearch).toHaveCount(1);
    await expect(browseSearch).toBeVisible();
    await readDirectory(root, () => browseSearch.click());
    await expect(search).toHaveValue("");
    await expect(search).toBeFocused();
    await expect(form).toHaveAttribute("data-browse-activation-attempts", "0");
    // Submit before any result activation: otherwise a second callback's
    // retirement could hide which equal operation lost the original key.
    let next = await submitPlacementRetry(form, posts);
    expectPlacementRetry(previous, next);
    previous = next;

    const browseButton = form.getByRole("button", { name: "Browse this path", exact: true });
    let browser = await readDirectory(root, () => browseButton.click());
    let attempts = Number(await form.getAttribute("data-browse-activation-attempts"));
    let creates = posts.length;
    await browser.getByRole("button", { name: `Use ${root}`, exact: true }).click();
    await expect(form).toHaveAttribute("data-browse-activation-attempts", String(attempts + 1));
    await expect(browser, "an accepted current-folder pick invalidates its observation").toHaveCount(0);
    await expectPlacementAuthority(form, remote, root);
    expect(posts, "the current-folder pick only selects").toHaveLength(creates);
    next = await submitPlacementRetry(form, posts);
    expectPlacementRetry(previous, next);
    previous = next;

    browser = await readDirectory(root, () => browseButton.click());
    const childChoice = browser.getByRole("button", { name: child, exact: true });
    await expect(childChoice).toBeVisible();
    attempts = Number(await form.getAttribute("data-browse-activation-attempts"));
    creates = posts.length;
    await childChoice.click();
    await expect(form).toHaveAttribute("data-browse-activation-attempts", String(attempts + 1));
    await expect(browser).toHaveCount(0);
    await expectPlacementAuthority(form, remote, child);
    expect(posts, "a child choice still only selects").toHaveLength(creates);
    next = await submitPlacementRetry(form, posts);
    expect(next).toMatchObject({ host: remote.id, cwd: child });
    expectPlacementChange(previous, next);
    previous = next;

    browser = await readDirectory(child, () => browseButton.click());
    const parentChoice = browser.getByRole("button", { name: `Parent: ${root}`, exact: true });
    await expect(parentChoice).toBeVisible();
    attempts = Number(await form.getAttribute("data-browse-activation-attempts"));
    creates = posts.length;
    await parentChoice.click();
    await expect(form).toHaveAttribute("data-browse-activation-attempts", String(attempts + 1));
    await expect(browser).toHaveCount(0);
    await expectPlacementAuthority(form, remote, root);
    expect(posts, "a parent choice still only selects").toHaveLength(creates);
    next = await submitPlacementRetry(form, posts);
    expect(next).toMatchObject({ host: remote.id, cwd: root });
    expectPlacementChange(previous, next);
    expect(browseBodies).toHaveLength(4);
    await expect(form).toHaveAttribute("data-history-activation-attempts", "0");
  } finally {
    // Every route on this test-owned page belongs to historyFixture or this
    // bounded browse reply. Drain entered handlers before page teardown;
    // creates were all refused, so there is no allocated session to delete.
    await page.unrouteAll({ behavior: "wait" });
  }
});

/** The launcher's folder default is per host (SPEC.md, "Working directory"):
 * the newest recent setup that launched into a plain folder on that host, so
 * a newer fresh managed checkout is skipped, else the host's home, `~`.
 * Switching hosts re-seeds an untouched folder in both directions, and once
 * the user types a folder a host switch keeps it. Recent setups are stubbed
 * per host so each host's memory is unambiguous whatever earlier tests ran,
 * and an empty session listing with no remembered selection keeps a selected
 * session's folder, which would outrank history, out of the premise. */
test("the launcher's folder follows each host's last launch until the user types one", async ({ page, request }) => {
  const fixture = await registryFixture(request);
  const headers = { "content-type": "application/json", "x-farhelm-build": fixture.build };
  await page.route("**/api/launch-history**", (route) => {
    const host = Number(new URL(route.request().url()).searchParams.get("host"));
    const launch = (cwd: string, repo: { owner: string; name: string } | null, seq: number) => ({
      host, github_repo: repo, cwd, canonical_cwd: cwd, created_at: seq, creation_seq: seq,
      selection: { harness: "codex", model: null, effort: null, permissions: null },
    });
    const launches = host === fixture.remote.id
      ? [launch("/srv/fresh-checkout", { owner: "acme", name: "repo" }, 3), launch("/srv/remote-work", null, 2)]
      : [];
    return route.fulfill({ status: 200, headers, body: JSON.stringify({ launches, folders: [] }) });
  });
  await page.route(SESSION_LISTING, (route) => route.request().method() === "GET"
    ? route.fulfill({ status: 200, headers, body: JSON.stringify({ sessions: [], total: 0, matching: 0, truncated: false }) })
    : route.fallback());
  await patchPreferences(request, { last_selected: null });
  await page.goto("/");
  await expect(page.locator(".session-count")).toHaveText("0");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  const host = form.locator(".create-session-host");
  const folder = form.getByLabel("Folder", { exact: true });
  await chooseLauncherHost(host, String(fixture.remote.id));
  await expect(folder).toHaveValue("/srv/remote-work");
  await chooseLauncherHost(host, String(fixture.local));
  await expect(folder, "a host without recent setups opens at its home").toHaveValue("~");
  await chooseLauncherHost(host, String(fixture.remote.id));
  await expect(folder).toHaveValue("/srv/remote-work");
  await folder.fill("/srv/typed");
  const fetched = Number(await form.getAttribute("data-history-fetched-revision"));
  await chooseLauncherHost(host, String(fixture.local));
  await expect(host).toHaveAttribute("data-host-id", String(fixture.local));
  // The local host's reply must have landed, or the folder could merely be unseeded yet.
  await expect.poll(async () => Number(await form.getAttribute("data-history-fetched-revision"))).toBeGreaterThan(fetched);
  await expect(folder, "a typed folder survives a host change").toHaveValue("/srv/typed");
});
