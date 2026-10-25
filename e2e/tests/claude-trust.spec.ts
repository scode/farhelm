import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { cleanupSession, createSession, patchPreferences, readPreferences, resetPreferences } from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";
import { openSettings, selectSettingsSection } from "./helpers/settings";
import { routeGate } from "./helpers/route-gate";
import { drainedRoute } from "./helpers/drained-route";
import { requireHelmBuild } from "./helpers/helm-build";

/** The real setting and preference routes must survive reload and reach another
 * client. Only the sampled dialog flag and managed association are seeded: no
 * Claude process, vendor login or trust edit is involved in this browser proof.
 * A borrower has no github_repo provenance, so membership alone must suffice. */
test("Claude trust hint preserves the terminal and opens the opt-in caveats", async ({ page, request, context }, testInfo) => {
  const created: string[] = [];
  let observer: typeof page | undefined;
  let bodyFailed = false;
  // Shared settings and each successful acquisition are owned before the
  // next await; a second create or observer failure must release the first.
  try {
    await resetPreferences(request);
    expect((await request.put("/api/checkout-claude-trust", { data: { enabled: false } })).ok()).toBe(true);
    const session = await createSession(request, { title: `trust-hint-${Date.now()}` });
    created.push(session.id);
    const neighbor = await createSession(request, { title: `trust-neighbor-${Date.now()}` });
    created.push(neighbor.id);
    let prompt = false;
    const seed = (row: Record<string, unknown>) => row.id === session.id
      ? {
        ...row,
        claude_trust_prompt: prompt,
        github_repo: null,
        working_copy: {
          id: "browser-checkout", repo: { owner: "example", name: "project" }, canonical_path: "/tmp",
          origin_session_id: "another-session",
        },
      }
      : row;
    const installSeed = async (target: typeof page) => {
      await target.route((url) => url.pathname === "/api/sessions" || url.pathname === `/api/sessions/${encodeURIComponent(session.id)}`, async (route) => {
        if (route.request().method() !== "GET") return route.continue();
        const response = await route.fetch();
        if (!response.ok()) return route.fulfill({ response });
        const body = await response.json();
        await route.fulfill({ response, json: Array.isArray(body.sessions) ? { ...body, sessions: body.sessions.map(seed) } : seed(body) });
      });
    };
    // An ordinary saved-setting invalidation asks mounted readers to fetch the
    // newly seeded observation; no model-driven timing or terminal input is used.
    const nudge = async () => expect((await request.put("/api/checkout-claude-trust", { data: { enabled: false } })).ok()).toBe(true);
    const other = await context.newPage();
    observer = other;
    await installSeed(page);
    await installSeed(other);
    await page.goto("/");
    await attachSession(page, session.id);
    const terminal = page.locator("#terminal .xterm-helper-textarea");
    await expect(terminal).toBeFocused();
    const panes = page.locator(".terminal-panes");
    const before = (await panes.boundingBox())!;
    expect(before.height, "fixture has a revealed terminal before the hint").toBeGreaterThan(0);
    prompt = true;
    await nudge();
    const band = page.locator(".claude-trust-band");
    await expect(band).toBeVisible();
    await expect(band).toHaveAttribute("role", "status");
    await expect(terminal).toBeFocused();
    const hintBox = (await band.boundingBox())!;
    const after = (await panes.boundingBox())!;
    expect(after.y).toBeGreaterThanOrEqual(hintBox.y + hintBox.height - 1);
    expect(after.height).toBeLessThan(before.height);
    await testInfo.attach("trust-hint", { body: await page.locator(".app-main").screenshot(), contentType: "image/png" });

    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    const field = dialog.locator("#checkout-claude-trust");
    const toggle = field.getByRole("checkbox", { name: 'answer Claude\'s "trust this folder?" for new checkouts', exact: true });
    // Exercise the band's own entry point at each layout. Selecting trust
    // through the gear helper here would hide a broken initial selection.
    for (const width of [390, 1280]) {
      await page.setViewportSize({ width, height: 900 });
      await band.getByRole("button", { name: "Settings and caveats", exact: true }).click();
      await expect(dialog.locator('.app-settings-panel[aria-label="agent trust"]')).toBeVisible();
      await expect(dialog.locator('.app-settings-panel[aria-label="sessions"]')).toBeHidden();
      await expect(field).toBeFocused();
      await expect(toggle).toBeEnabled();
      await expect(toggle).not.toBeChecked();
      expect(await toggle.evaluate((element) => {
        const box = element.getBoundingClientRect();
        const body = element.closest(".host-settings-body")!.getBoundingClientRect();
        return box.top >= body.top && box.bottom <= body.bottom;
      }), "settings opens scrolled to the opt-in").toBe(true);
      if (width === 390) {
        await expect(dialog.getByRole("button", { name: "‹ settings", exact: true })).toBeVisible();
        await expect(dialog.getByRole("navigation", { name: "settings sections", exact: true })).toBeHidden();
        await dialog.getByRole("button", { name: "close", exact: true }).click();
        await openSettings(page);
        await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
        await expect(dialog.locator('.app-settings-panel[aria-label="agent trust"]')).toBeHidden();
        await dialog.getByRole("button", { name: "close", exact: true }).click();
      }
    }
    await expect(field).toContainText("How this works, and why it's off by default");
    await expect(field).toContainText("Claude session exiting at the same instant");
    await testInfo.attach("trust-caveats", { body: await dialog.screenshot(), contentType: "image/png" });
    // A refused opt-in must restore the native checkbox, not merely show an
    // error while leaving a misleading checked control beside the caveats.
    await page.route("**/api/checkout-claude-trust", async (route) => {
      if (route.request().method() === "PUT") return route.fulfill({ status: 500, body: "fixture save refusal" });
      return route.fallback();
    });
    await toggle.click();
    await expect(dialog.getByRole("alert")).toContainText("fixture save refusal");
    await expect(toggle).not.toBeChecked();
    await expect(toggle).toBeEnabled();
    expect((await (await request.get("/api/checkout-claude-trust")).json()).enabled).toBe(false);
    await page.unroute("**/api/checkout-claude-trust");
    await toggle.check();
    await expect.poll(async () => (await (await request.get("/api/checkout-claude-trust")).json()).enabled).toBe(true);
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await expect(band).toHaveCount(0);
    await page.reload();
    await openSettings(page);
    await expect(dialog.locator('.app-settings-panel[aria-label="sessions"]')).toBeVisible();
    await selectSettingsSection(page, "agent trust");
    await expect(toggle).toBeChecked();
    await toggle.uncheck();
    await expect.poll(async () => (await (await request.get("/api/checkout-claude-trust")).json()).enabled).toBe(false);
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await expect(band).toBeVisible();

    await band.getByRole("button", { name: "hide this trust hint for this occurrence", exact: true }).click();
    await expect(band).toHaveCount(0);
    await attachSession(page, neighbor.id);
    await attachSession(page, session.id);
    await expect(page.locator(".app-main .layout")).toHaveAttribute("data-claude-trust-prompt", "true");
    await expect(page.locator(".claude-trust-surface")).toHaveAttribute("data-enabled", "false");
    await expect(page.locator(".claude-trust-surface")).toHaveAttribute("data-hint-dismissed", "false");
    await expect(band).toHaveCount(0);
    prompt = false;
    await nudge();
    // Observe the accepted session state, rather than just a completed GET:
    // the hidden band alone cannot distinguish this reset from its local ×.
    await expect(page.locator(".app-main .layout")).toHaveAttribute("data-claude-trust-prompt", "false");
    prompt = true;
    await nudge();
    await expect(band).toBeVisible();
    await other.goto("/");
    await attachSession(other, session.id);
    await expect(other.locator(".claude-trust-band")).toBeVisible();
    await band.getByRole("button", { name: "Don't show again", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).claude_trust_hint_dismissed).toBe(true);
    await expect(band).toHaveCount(0);
    await expect(other.locator(".app-main .layout")).toHaveAttribute("data-claude-trust-prompt", "true");
    await expect(other.locator(".claude-trust-surface")).toHaveAttribute("data-enabled", "false");
    await expect(other.locator(".claude-trust-surface")).toHaveAttribute("data-hint-dismissed", "true");
    await expect(other.locator(".claude-trust-band")).toHaveCount(0);
    await openSettings(page, "agent trust");
    await field.getByRole("checkbox", { name: "show the hint when Claude asks about a managed checkout", exact: true }).check();
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await expect(band).toBeVisible();
    await expect(other.locator(".claude-trust-band")).toBeVisible();
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      // Shared-state cleanup triggers fresh reads. Remove interceptors and
      // settle their current handlers before either page's context can close.
      ["main page routes", () => page.unrouteAll({ behavior: "wait" })],
      ["observer page routes", () => observer?.unrouteAll({ behavior: "wait" })],
      ["observer page", () => observer?.close()],
      ["trust hint preference", () => patchPreferences(request, { claude_trust_hint_dismissed: null })],
      ["checkout trust", async () => {
        expect((await request.put("/api/checkout-claude-trust", { data: { enabled: false } })).ok()).toBe(true);
      }],
      ...created.map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});
/** The band can copy a newer Settings choice after its own dismissal was
 * superseded. Refusing that newer choice must repair the band independently
 * of the now-closed Settings component, without another user action or feed
 * nudge. Raw GETs and stored values distinguish the pending overlay from a
 * backend that merely happened to contain the visible choice. */
test("Claude trust band discards a newer Settings overlay after its origin closes", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const previousTrust = await request.get("/api/checkout-claude-trust");
  expect(previousTrust.ok(), await previousTrust.text()).toBe(true);
  const savedTrust = await previousTrust.json() as { enabled: boolean };
  const build = requireHelmBuild(previousTrust, "cross-component trust refusal");
  const created: string[] = [];
  const firstReached = routeGate();
  const firstRelease = routeGate();
  const secondReached = routeGate();
  const secondRelease = routeGate();
  let stopSessionRoute = async () => {};
  let stopPreferenceRoute = async () => {};
  const dismissals: boolean[] = [];
  let bodyFailed = true;
  try {
    await patchPreferences(request, { welcome_seen: true, claude_trust_hint_dismissed: false });
    expect((await readPreferences(request)).claude_trust_hint_dismissed).toBe(false);
    expect((await request.put("/api/checkout-claude-trust", { data: { enabled: false } })).ok()).toBe(true);
    const session = await createSession(request, { title: `trust-cross-component-${Date.now()}` });
    created.push(session.id);
    const origin = await createSession(request, { title: `trust-checkout-origin-${Date.now()}` });
    created.push(origin.id);
    // Seed only the sampled question and managed membership, as in the
    // existing lifecycle proof. The owned fake agent supplies live terminal
    // attachment; no production visibility or question gate is bypassed.
    stopSessionRoute = await drainedRoute(page, (url) => url.pathname === "/api/sessions"
      || url.pathname === `/api/sessions/${encodeURIComponent(session.id)}`, async (route) => {
      if (route.request().method() !== "GET") return route.fallback();
      const reply = await route.fetch();
      if (!reply.ok()) return route.fulfill({ response: reply });
      const body = await reply.json();
      const seed = (row: Record<string, unknown>) => row.id === session.id ? {
        ...row,
        claude_trust_prompt: true,
        github_repo: null,
        working_copy: {
          id: `browser-checkout-${session.id}`, repo: { owner: "example", name: "project" }, canonical_path: "/tmp",
          origin_session_id: origin.id,
        },
      } : row;
      await route.fulfill({ response: reply, json: Array.isArray(body.sessions)
        ? { ...body, sessions: body.sessions.map(seed) } : seed(body) });
    });
    stopPreferenceRoute = await drainedRoute(page, "**/api/preferences", async (route) => {
      const outgoing = route.request();
      if (outgoing.method() !== "PUT" || !("claude_trust_hint_dismissed" in outgoing.postDataJSON())) return route.fallback();
      dismissals.push(outgoing.postDataJSON().claude_trust_hint_dismissed);
      if (dismissals.length === 1) {
        expect(dismissals).toEqual([true]);
        firstReached.release();
        await firstRelease.wait();
        const saved = await route.fetch();
        expect(saved.status(), "the superseded band dismissal really commits before the second request").toBe(204);
        await route.fulfill({ response: saved });
      } else {
        expect(dismissals).toEqual([true, false]);
        secondReached.release();
        await secondRelease.wait();
        await route.fulfill({ status: 503, headers: { "x-farhelm-build": build }, body: "fixture newer Settings dismissal refused" });
      }
    });
    await page.goto("/");
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const selected = page.locator(`.session-row[data-session-id="${session.id}"]`);
    const surface = page.locator(".claude-trust-surface");
    const band = page.locator(".claude-trust-band");
    await expect(selected).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(".app-main .layout")).toHaveAttribute("data-claude-trust-prompt", "true");
    await expect(surface).toHaveAttribute("data-enabled", "false");
    await expect(surface).toHaveAttribute("data-hint-dismissed", "false");
    await expect(band).toBeVisible();
    await band.getByRole("button", { name: "Don't show again", exact: true }).click();
    await firstReached.wait();
    await expect(band).toHaveCount(0);
    const dialog = await openSettings(page, "agent trust");
    const field = dialog.locator("#checkout-claude-trust");
    const toggle = field.getByRole("checkbox", { name: "show the hint when Claude asks about a managed checkout", exact: true });
    await expect(toggle).toBeEnabled();
    await expect(toggle).not.toBeChecked();
    expect((await readPreferences(request)).claude_trust_hint_dismissed).toBe(false);
    await toggle.click();
    await expect(toggle).toBeChecked();
    await expect(toggle).toBeEnabled();
    firstRelease.release();
    await secondReached.wait();
    expect(dismissals).toEqual([true, false]);
    expect((await readPreferences(request)).claude_trust_hint_dismissed).toBe(true);

    // A real configuration invalidation refreshes the current session view.
    // Its trust GET must now read stored true while the live clients still
    // present the newer pending false, including the independently owned band.
    const refreshed = page.waitForResponse(async (reply) => reply.request().method() === "GET"
      && new URL(reply.url()).pathname === "/api/checkout-claude-trust"
      && reply.ok() && (await reply.json()).hint_dismissed === true);
    expect((await request.put("/api/checkout-claude-trust", { data: { enabled: false } })).ok()).toBe(true);
    const raw = await refreshed;
    expect((await raw.json()).hint_dismissed).toBe(true);
    await expect(surface).toHaveAttribute("data-enabled", "false");
    await expect(surface).toHaveAttribute("data-hint-dismissed", "false");
    await expect(toggle).toBeChecked();
    await expect(toggle).toBeEnabled();
    await dialog.getByRole("button", { name: "close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(band).toBeVisible();
    await expect(selected).toHaveAttribute("data-session-selected", "true");
    const refused = page.waitForResponse((reply) => reply.request().method() === "PUT"
      && new URL(reply.url()).pathname === "/api/preferences"
      && reply.request().postDataJSON().claude_trust_hint_dismissed === false);
    secondRelease.release();
    const reply = await refused;
    expect(reply.status()).toBe(503);
    await reply.finished();
    // No post-failure click, nudge or navigation may repair the copied value.
    await expect(surface).toHaveAttribute("data-hint-dismissed", "true");
    await expect(surface).toHaveAttribute("data-enabled", "false");
    await expect(band).toHaveCount(0);
    await expect(surface.getByRole("alert")).toHaveCount(0);
    await expect(selected).toHaveAttribute("data-session-selected", "true");
    expect((await readPreferences(request)).claude_trust_hint_dismissed).toBe(true);
    expect(dismissals).toEqual([true, false]);
    await expect(page.locator(".build-skew")).toHaveCount(0);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["cross-component page", () => page.goto("about:blank")],
      ["cross-component routes", async () => {
        firstRelease.release();
        secondRelease.release();
        const results = await Promise.allSettled([stopPreferenceRoute(), stopSessionRoute()]);
        expect(results.filter((result) => result.status === "rejected"), "both cross-component routes must drain").toEqual([]);
      }],
      ["trust dismissal preferences", () => patchPreferences(request, {
        welcome_seen: previous.welcome_seen ?? null,
        claude_trust_hint_dismissed: previous.claude_trust_hint_dismissed ?? null,
      })],
      ["checkout trust", async () => {
        expect((await request.put("/api/checkout-claude-trust", { data: { enabled: savedTrust.enabled } })).ok()).toBe(true);
      }],
      ...created.map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** An idle band read can span an entire Settings save without owning that
 * click's completion. Its real old body must not become an indefinitely
 * retained success after the field settles. Cancellation of the old read is
 * valid; a fresh authoritative read and repaired band are the outcome. */
test("Claude trust band repairs an idle read crossing a Settings dismissal save", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const previousTrust = await request.get("/api/checkout-claude-trust");
  expect(previousTrust.ok(), await previousTrust.text()).toBe(true);
  const savedTrust = await previousTrust.json() as { enabled: boolean };
  const created: string[] = [];
  const oldCaptured = routeGate();
  const oldRelease = routeGate();
  const oldFinished = routeGate();
  const currentRead = routeGate();
  let stopSessionRoute = async () => {};
  let stopTrustRoute = async () => {};
  let stopWriteRoute = async () => {};
  let captureOld = true;
  let writes = 0;
  let bodyFailed = true;
  try {
    await patchPreferences(request, { welcome_seen: true, claude_trust_hint_dismissed: false });
    expect((await readPreferences(request)).claude_trust_hint_dismissed).toBe(false);
    expect((await request.put("/api/checkout-claude-trust", { data: { enabled: false } })).ok()).toBe(true);
    const session = await createSession(request, { title: `trust-idle-read-${Date.now()}` });
    created.push(session.id);
    const origin = await createSession(request, { title: `trust-idle-origin-${Date.now()}` });
    created.push(origin.id);
    // Only sampled question and membership facts are supplied. The actual
    // session, terminal replay and explicit-off trust read still establish
    // the ordinary band's visibility gates before a response is held.
    stopSessionRoute = await drainedRoute(page, (url) => url.pathname === "/api/sessions"
      || url.pathname === `/api/sessions/${encodeURIComponent(session.id)}`, async (route) => {
      if (route.request().method() !== "GET") return route.fallback();
      const response = await route.fetch();
      if (!response.ok()) return route.fulfill({ response });
      const body = await response.json();
      const seed = (row: Record<string, unknown>) => row.id === session.id ? {
        ...row,
        claude_trust_prompt: true,
        github_repo: null,
        working_copy: {
          id: `browser-checkout-${session.id}`, repo: { owner: "example", name: "project" }, canonical_path: "/tmp",
          origin_session_id: origin.id,
        },
      } : row;
      await route.fulfill({ response, json: Array.isArray(body.sessions)
        ? { ...body, sessions: body.sessions.map(seed) } : seed(body) });
    });
    stopWriteRoute = await drainedRoute(page, "**/api/preferences", async (route) => {
      const outgoing = route.request();
      if (outgoing.method() !== "PUT" || !("claude_trust_hint_dismissed" in outgoing.postDataJSON())) return route.fallback();
      expect(outgoing.postDataJSON().claude_trust_hint_dismissed).toBe(true);
      writes++;
      const response = await route.fetch();
      expect(response.status(), "the Settings dismissal really commits while old delivery stays held").toBe(204);
      await route.fulfill({ response });
    });
    await page.goto("/");
    await attachSession(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const selected = page.locator(`.session-row[data-session-id="${session.id}"]`);
    const surface = page.locator(".claude-trust-surface");
    const band = page.locator(".claude-trust-band");
    await expect(selected).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(".app-main .layout")).toHaveAttribute("data-claude-trust-prompt", "true");
    await expect(surface).toHaveAttribute("data-enabled", "false");
    await expect(surface).toHaveAttribute("data-hint-dismissed", "false");
    await expect(band).toBeVisible();
    await expect(page.getByRole("dialog", { name: "settings", exact: true })).toHaveCount(0);
    stopTrustRoute = await drainedRoute(page, "**/api/checkout-claude-trust", async (route) => {
      if (route.request().method() !== "GET") return route.fallback();
      const response = await route.fetch();
      expect(response.ok()).toBe(true);
      const raw = await response.json();
      let held = false;
      if (captureOld) {
        captureOld = false;
        held = true;
        expect(writes, "this band read captures its backend body before Settings creates a write").toBe(0);
        expect(raw.hint_dismissed).toBe(false);
        oldCaptured.release();
        await oldRelease.wait();
      }
      // A newer automatic refresh may cancel this held delivery. The shared
      // drained route owns that cancellation path; stale bytes need not render.
      try {
        await route.fulfill({ response });
        if (raw.hint_dismissed === true) currentRead.release();
      } finally {
        if (held) oldFinished.release();
      }
    });
    // This sole pre-write nudge starts the independently owned band's idle
    // GET. No configuration write or navigation is used after the save commits.
    expect((await request.put("/api/checkout-claude-trust", { data: { enabled: false } })).ok()).toBe(true);
    await oldCaptured.wait();
    expect(writes).toBe(0);
    expect((await readPreferences(request)).claude_trust_hint_dismissed).toBe(false);
    const dialog = await openSettings(page, "agent trust");
    const field = dialog.locator("#checkout-claude-trust");
    const toggle = field.getByRole("checkbox", { name: "show the hint when Claude asks about a managed checkout", exact: true });
    await expect(toggle).toBeEnabled();
    await expect(toggle).toBeChecked();
    const committed = page.waitForResponse((reply) => reply.request().method() === "PUT"
      && new URL(reply.url()).pathname === "/api/preferences"
      && reply.request().postDataJSON().claude_trust_hint_dismissed === true);
    await toggle.click();
    const saved = await committed;
    expect(saved.status()).toBe(204);
    await saved.finished();
    expect((await readPreferences(request)).claude_trust_hint_dismissed).toBe(true);
    oldRelease.release();
    await oldFinished.wait();
    await currentRead.wait();
    await expect(surface).toHaveAttribute("data-hint-dismissed", "true");
    await expect(surface).toHaveAttribute("data-enabled", "false");
    await expect(band).toHaveCount(0);
    await expect(toggle).not.toBeChecked();
    await expect(toggle).toBeEnabled();
    await expect(field.getByRole("alert")).toHaveCount(0);
    await expect(surface.getByRole("alert")).toHaveCount(0);
    await expect(selected).toHaveAttribute("data-session-selected", "true");
    expect(writes).toBe(1);
    expect((await readPreferences(request)).claude_trust_hint_dismissed).toBe(true);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["idle-crossing page", () => page.goto("about:blank")],
      ["idle-crossing routes", async () => {
        oldRelease.release();
        const results = await Promise.allSettled([stopWriteRoute(), stopTrustRoute(), stopSessionRoute()]);
        expect(results.filter((result) => result.status === "rejected"), "all idle-crossing routes must drain").toEqual([]);
      }],
      ["trust dismissal preferences", () => patchPreferences(request, {
        welcome_seen: previous.welcome_seen ?? null,
        claude_trust_hint_dismissed: previous.claude_trust_hint_dismissed ?? null,
      })],
      ["checkout trust", async () => {
        expect((await request.put("/api/checkout-claude-trust", { data: { enabled: savedTrust.enabled } })).ok()).toBe(true);
      }],
      ...created.map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

