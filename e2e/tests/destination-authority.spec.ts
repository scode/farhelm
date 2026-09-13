// Destination snapshots must survive sidebar projection changes without
// letting a remembered path follow a registry row onto another installation.
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Page } from "@playwright/test";
import { cleanupSession, createSession, localHostId, pinAutoSelect, stubFeed } from "./helpers/fleet";
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
   * as a complete host/home pair, never carry the remote folder to local. */
  test(`New inherits a filtered selected destination with replacement=${replace}`, async ({ page, request }) => {
    const fixture = await registryFixture(request);
    const active = await createSession(request, {
      title: `filtered-destination-${Date.now()}`,
      cwd: "/tmp",
      host: fixture.remote.id,
    });
    let replacement = false;
    const feed = await stubFeed(page);
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
      await page.locator(".filter-host").selectOption(String(fixture.local));
      await expect(row, "the server-backed filter must actually remove the selected row").toHaveCount(0);
      await expect(page.locator(".titlebar .title")).toHaveText(active.title);
      expect(await terminal!.evaluate((node) => node === document.querySelector("#terminal")), "filtering must preserve the same terminal element").toBe(true);
      if (replace) {
        replacement = true;
        feed.notify(1);
        await expect(page.locator(`.filter-host option[value="${fixture.remote.id}"]`), "the replacement registry must be rendered before opening New").toContainText("replacement destination");
      }
      await page.locator(".new-session-button").click();
      const form = page.locator(".create-session-form");
      await expect(form.locator(".create-session-host")).toHaveValue(String(replace ? fixture.local : fixture.remote.id));
      await expect(form.getByLabel("folder", { exact: true })).toHaveValue(replace ? "~" : active.cwd);
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
 * component would submit after the explicit correction. */
async function historyFixture(page: Page, request: APIRequestContext) {
  const fixture = await registryFixture(request);
  const state = { reconnects: 0, replaced: false };
  const posts: Array<{ host: number; cwd: string; expected_incarnation: number }> = [];
  const feed = await stubFeed(page);
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
      launches: [{ host: fixture.remote.id, cwd: "/tmp", canonical_cwd: "/tmp", selection: { harness: "codex", model: null, effort: null, permissions: null }, created_at: 2, creation_seq: 2 }],
      folders: ["/tmp", "/var/tmp"].map((cwd, index) => ({ host: fixture.remote.id, canonical_cwd: cwd, canonical_proven: true, display_cwd: cwd, created_at: 2 - index, creation_seq: 2 - index })),
    }),
  }));
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts.push(route.request().postDataJSON());
    await route.fulfill({ status: 409, headers, body: JSON.stringify("destination fixture refuses launch") });
  });
  await page.goto("/");
  await feed.waitForConnection(1);
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await form.locator(".create-session-host").selectOption(String(fixture.remote.id));
  await expect(form, "the actual remote connection must reach the mounted composer").toHaveAttribute("data-browse-live-connection", String(fixture.remote.incarnation));
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
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
    const folder = form.getByLabel("folder", { exact: true });
    const launch = form.locator(".create-session-submit");
    if (source === "ordinary recent") {
      await form.locator(".launch-composer-recent-slots button").first().click();
    } else if (source === "search recent") {
      await form.getByRole("combobox", { name: /search/i }).fill("codex");
      await form.getByRole("option", { name: /^Recent setup:.*\/tmp/ }).click();
    } else {
      await form.locator(".launch-composer-folder-links").getByRole("button", { name: "/tmp", exact: true }).click();
      await form.locator(".launch-composer-harness-choice").getByRole("button", { name: /Codex$/ }).click();
    }
    await expect(form).toHaveAttribute("data-history-activation-attempts", "1");
    await expect(folder).toHaveValue("/tmp");
    await expect(launch).toBeEnabled();
    state.reconnects = 1;
    feed.notify(1);
    await expect(form, "same-install reconnect must be consumed before testing continuity").toHaveAttribute("data-browse-live-connection", String(remote.incarnation + 1));
    await expect(form).toHaveAttribute("data-remembered-destination-valid", "true");
    await expect(launch).toBeEnabled();
    state.replaced = true;
    state.reconnects = 2;
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
    // Native selects need a changed value to emit change. Choose away and
    // back, as a person can, instead of synthesizing change on the same value.
    await form.locator(".create-session-host").selectOption(String(local));
    await expect(form.locator(".create-session-host")).toHaveValue(String(local));
    await form.locator(".create-session-host").selectOption(String(remote.id));
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
  const saved = form.locator(".launch-composer-folder-links").getByRole("button", { name: "/var/tmp", exact: true });
  await expect(saved).toBeVisible();
  const attempts = Number(await form.getAttribute("data-history-activation-attempts"));
  await form.evaluate((node, local) => {
    const host = node.querySelector<HTMLSelectElement>(".create-session-host")!;
    const saved = [...node.querySelectorAll<HTMLButtonElement>(".launch-composer-folder-links button")].find((button) => button.textContent?.includes("/var/tmp"))!;
    host.value = String(local);
    host.dispatchEvent(new Event("change", { bubbles: true }));
    saved.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  }, local);
  await expect(form).toHaveAttribute("data-history-activation-attempts", String(attempts + 1));
  await expect(form.locator(".create-session-host")).toHaveValue(String(local));
  await expect(form.getByLabel("folder", { exact: true })).toHaveValue("/tmp");
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
    feed.notify(1);
    await expect(form).toHaveAttribute("data-browse-live-connection", String(remote.incarnation + 1));
    await expect(form).toHaveAttribute("data-remembered-destination-valid", "false");
    await page.evaluate(() => (window as any).__destinationMint.release());
    await expect(form.locator(".create-session-error")).toContainText("remembered folder belongs to a different installation");
    expect(posts).toHaveLength(0);
    // A fresh explicit choice also proves the refusal released the shared
    // operation guard; a leaked guard would leave this selector disabled.
    await form.locator(".create-session-host").selectOption(String(local));
    await expect(form.locator(".create-session-host")).toHaveValue(String(local));
    await form.locator(".create-session-host").selectOption(String(remote.id));
    await expect(launch).toBeEnabled();
  } finally {
    await page.evaluate(() => (window as any).__destinationMint.release());
  }
});
