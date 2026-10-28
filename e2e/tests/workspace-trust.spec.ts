import type { APIRequestContext, Locator, Page, Route } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { openSettings, selectSettingsSection } from "./helpers/settings";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { agentLaunchRow, cleanupSession, createSession, resetPreferences } from "./helpers/fleet";
import { attachSession } from "./helpers/term";
import { waitForSessionReady } from "./helpers/terminal-readiness";
import { requireHelmBuild } from "./helpers/helm-build";

type TrustHarness = "codex" | "muse" | "pi";
const AGENTS = [
  { harness: "codex", name: "Codex" },
  { harness: "muse", name: "Muse" },
  { harness: "pi", name: "Pi" },
] as const;

/** Mutable HTTP observations describe UI-only facts. The real sessions still
 * run the ordinary fake agent; no vendor process or trust-file edit is needed. */
interface TrustObservation {
  harness: TrustHarness | null;
  prompt: boolean;
  managed: boolean;
  detailTitle?: string;
}

/** Read the real shared setting, rather than treating a checkbox render as
 * proof that a configuration save reached the helm. */
async function trustState(request: APIRequestContext, harness: TrustHarness | "claude") {
  const response = await request.get(`/api/checkout-agent-trust/${harness}`);
  expect(response.ok(), await response.text()).toBe(true);
  return await response.json() as { enabled: boolean; hint_dismissed: boolean };
}

/** Refuse an operation while preserving the real helm's response identity.
 * Even failures carry the build stamp; omitting it tests version-skew withdrawal
 * instead of trust-read recovery. Fetch only the GET so a refused PUT never
 * changes the setting before its synthetic error reaches the browser. */
async function refuseTrust(route: Route, message: string) {
  const response = await route.fetch({ method: "GET", postData: "" });
  expect(response.ok(), await response.text()).toBe(true);
  requireHelmBuild(response, "trust refusal fixture");
  await route.fulfill({ response, status: 503, body: message });
}

/** Each spec owns the shared settings it changes. Clear all hint dismissals
 * explicitly because the new fields must not leak into another spec or engine. */
async function resetTrust(request: APIRequestContext) {
  await resetPreferences(request);
  const preferences = await request.put("/api/preferences", {
    data: {
      claude_trust_hint_dismissed: null,
      codex_trust_hint_dismissed: null,
      muse_trust_hint_dismissed: null,
      pi_trust_hint_dismissed: null,
    },
  });
  expect(preferences.ok(), await preferences.text()).toBe(true);
  for (const { harness } of AGENTS) {
    const response = await request.put(`/api/checkout-agent-trust/${harness}`, { data: { enabled: false } });
    expect(response.ok(), await response.text()).toBe(true);
  }
  const claude = await request.put("/api/checkout-claude-trust", { data: { enabled: false } });
  expect(claude.ok(), await claude.text()).toBe(true);
}

/** Preserve the upstream response stamp and real terminal substrate. Only list
 * and detail JSON are decorated; tabs and terminal connections reach the stack.
 * A detail-only title is a visible witness that a refresh was admitted, rather
 * than merely that the transport completed while an old question stayed drawn. */
async function installObservations(page: Page, observations: Map<string, TrustObservation>) {
  await page.route((url) =>
    url.pathname === "/api/sessions" ||
    observations.has(decodeURIComponent(url.pathname.replace(/^\/api\/sessions\//, ""))), async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const response = await route.fetch();
    if (!response.ok()) return route.fulfill({ response });
    const body = await response.json();
    const detail = new URL(route.request().url()).pathname !== "/api/sessions";
    const seed = (row: Record<string, unknown>) => {
      const observation = observations.get(String(row.id));
      if (!observation) return row;
      return {
        ...row,
        ...(detail && observation.detailTitle ? { title: observation.detailTitle } : {}),
        claude_trust_prompt: false,
        workspace_trust_prompt: observation.prompt,
        launch: observation.harness
          ? agentLaunchRow({
            harness: observation.harness,
            model: null,
            effort: null,
            permissions: null,
            workspace_trust: null,
          })
          : row.launch,
        // Muse shares the generic integration kind. Its structured launch,
        // rather than that kind or its command text, must name the band.
        agent_kind: observation.harness === "muse" ? "generic" : observation.harness ?? "codex",
        github_repo: null,
        working_copy: observation.managed
          ? {
            id: `browser-checkout-${row.id}`,
            repo: { owner: "example", name: "project" },
            canonical_path: row.cwd,
            origin_session_id: "another-session",
          }
          : null,
      };
    };
    await route.fulfill({
      response,
      json: Array.isArray(body.sessions) ? { ...body, sessions: body.sessions.map(seed) } : seed(body),
    });
  });
}

/** A band has its own agent identity even though it reuses Claude's visual
 * styling. Scoping the locator prevents Claude's compatibility band from
 * accidentally satisfying an assertion about the new neutral signal. */
function bandFor(page: Page, harness: TrustHarness): Locator {
  return page.locator(`.workspace-trust-surface[data-agent="${harness}"] .workspace-trust-band`);
}

/** The three real settings start off, report refusals, and survive reopening
 * independently. Exercising the legacy Claude route beside them protects older
 * clients while the newer generic route is introduced. */
test("managed-checkout trust switches persist independently of Claude's legacy route", async ({ page, request }) => {
  let bodyFailed = false;
  try {
    await resetTrust(request);
    for (const { harness } of AGENTS) expect(await trustState(request, harness)).toMatchObject({ enabled: false });
    await page.goto("/");
    await openSettings(page, "agent trust");
    const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
    const choices = AGENTS.map(({ harness }) =>
      dialog.locator(`#checkout-${harness}-trust`).getByRole("checkbox").first()
    );
    for (const choice of choices) {
      await expect(choice).toBeEnabled();
      await expect(choice).not.toBeChecked();
    }
    await expect(dialog.locator("#checkout-pi-trust")).toContainText("approve project settings and extensions");
    await expect(dialog.locator("#checkout-pi-trust")).toContainText("project resources");
    await expect(dialog.locator("#checkout-pi-trust").getByRole("checkbox")).toHaveCount(1);
    await expect(dialog.locator("#checkout-codex-trust")).toContainText("hooks, settings and commands");
    await expect(page.locator(".build-skew")).toHaveCount(0);

    await page.route("**/api/checkout-agent-trust/codex", async (route) => {
      if (route.request().method() === "PUT") return refuseTrust(route, "fixture trust save refusal");
      return route.fallback();
    });
    // A refusal may restore the checkbox before Playwright checks the click's
    // postcondition. Observe that restoration explicitly instead of requiring
    // check() to keep the deliberately refused value checked.
    await choices[0].click();
    await expect(dialog.getByRole("alert")).toContainText("fixture trust save refusal");
    await expect(page.locator(".build-skew")).toHaveCount(0);
    await expect(choices[0]).not.toBeChecked();
    await expect(choices[0]).toBeEnabled();
    expect((await trustState(request, "codex")).enabled).toBe(false);
    await page.unroute("**/api/checkout-agent-trust/codex");

    for (const [index, { harness }] of AGENTS.entries()) {
      await choices[index].check();
      await expect.poll(async () => (await trustState(request, harness)).enabled).toBe(true);
      for (const [otherIndex, other] of AGENTS.entries()) {
        expect((await trustState(request, other.harness)).enabled).toBe(otherIndex <= index);
      }
    }
    const legacyBefore = await request.get("/api/checkout-claude-trust");
    expect(legacyBefore.ok(), await legacyBefore.text()).toBe(true);
    expect((await legacyBefore.json()).enabled).toBe(false);
    const legacySave = await request.put("/api/checkout-claude-trust", { data: { enabled: true } });
    expect(legacySave.ok(), await legacySave.text()).toBe(true);
    expect((await trustState(request, "claude")).enabled).toBe(true);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await page.reload();
    await openSettings(page, "agent trust");
    for (const choice of choices) await expect(choice).toBeChecked();
    await expect(dialog.locator("#checkout-claude-trust").getByRole("checkbox").first()).toBeChecked();
    await choices[0].uncheck();
    await expect.poll(async () => (await trustState(request, "codex")).enabled).toBe(false);
    expect((await trustState(request, "muse")).enabled).toBe(true);
    expect((await trustState(request, "pi")).enabled).toBe(true);
    expect((await trustState(request, "claude")).enabled).toBe(true);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["settings routes", () => page.unrouteAll({ behavior: "wait" })],
      ["shared trust settings", () => resetTrust(request)],
    ]);
  }
});

for (const { harness, name } of AGENTS.filter((agent) => agent.harness !== "pi")) {
  /** Each recognized agent gets the same integrated lifecycle proof. Saved
   * opt-outs are shared; occurrence closes are page-local and retire only on
   * an admitted ended-question observation, never navigation or a failed read. */
  test(`${name} trust hint stays scoped to its managed agent terminal and own dismissal`, async ({ page, request, context }) => {
    test.setTimeout(120_000);
    const created: string[] = [];
    let observer: Page | undefined;
    let bodyFailed = false;
    try {
      await resetTrust(request);
      const session = await createSession(request, { title: `trust-${harness}-${Date.now()}` });
      created.push(session.id);
      const otherHarness = harness === "codex" ? "muse" : "codex";
      const neighbor = await createSession(request, { title: `trust-other-${Date.now()}` });
      created.push(neighbor.id);
      const observation: TrustObservation = { harness, prompt: true, managed: true };
      const observations = new Map<string, TrustObservation>([
        [session.id, observation],
        [neighbor.id, { harness: otherHarness, prompt: true, managed: true }],
      ]);
      await installObservations(page, observations);
      let failReads = true;
      await page.route(`**/api/checkout-agent-trust/${harness}`, async (route) => {
        if (failReads && route.request().method() === "GET") {
          return refuseTrust(route, "fixture trust read refusal");
        }
        return route.fallback();
      });
      // A real configuration invalidation reuses the product's admitted-read
      // boundary; the fixture needs no timer or private application callback.
      const nudge = async () => {
        const response = await request.put(`/api/checkout-agent-trust/${harness}`, { data: { enabled: false } });
        expect(response.ok(), await response.text()).toBe(true);
      };
      await page.goto("/");
      await attachSession(page, session.id);
      const band = bandFor(page, harness);
      const surface = page.locator(`.workspace-trust-surface[data-agent="${harness}"]`);
      const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
      const field = dialog.locator(`#checkout-${harness}-trust`);
      const toggle = field.getByRole("checkbox").first();
      await expect(page.locator(".app-main .layout")).toHaveAttribute("data-workspace-trust-prompt", "true");
      await expect(page.locator(".build-skew")).toHaveCount(0);
      await openSettings(page, "agent trust");
      await expect(field.getByRole("alert")).toContainText("fixture trust read refusal");
      await expect(page.locator(".build-skew")).toHaveCount(0);
      await expect(toggle).toBeDisabled();
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
      await expect(surface).toHaveAttribute("data-enabled", "");
      await expect(band).toHaveCount(0);

      failReads = false;
      await page.locator("#terminal").click();
      const terminal = page.locator("#terminal .xterm-helper-textarea");
      await expect(terminal).toBeFocused();
      await nudge();
      await expect(page.locator(".build-skew")).toHaveCount(0);
      await expect(band).toBeVisible();
      await expect(band).toContainText(`${name} asks about trust`);
      await expect(band).toHaveAttribute("role", "status");
      await expect(terminal).toBeFocused();
      await expect(page.locator(".claude-trust-surface .claude-trust-band")).toHaveCount(0);
      // The real band must reveal and focus its own agent block without a
      // helper repairing the selection, including the phone's one-pane view.
      for (const width of [390, 1280]) {
        await page.setViewportSize({ width, height: 900 });
        await band.getByRole("button", { name: "Settings", exact: true }).click();
        await expect(dialog.locator('.app-settings-panel[aria-label="Agent trust"]')).toBeVisible();
        await expect(dialog.locator('.app-settings-panel[aria-label="Sessions"]')).toBeHidden();
        await expect(field).toBeFocused();
        await expect(toggle).toBeEnabled();
        await expect(toggle).not.toBeChecked();
        expect(
          await toggle.evaluate((element) => {
            const box = element.getBoundingClientRect();
            const body = element.closest(".host-settings-body")!.getBoundingClientRect();
            return box.top >= body.top && box.bottom <= body.bottom;
          }),
          "the matching switch is scrolled into the dialog viewport",
        ).toBe(true);
        if (width === 390) {
          await expect(dialog.getByRole("button", { name: "‹ Settings", exact: true })).toBeVisible();
          await expect(dialog.getByRole("navigation", { name: "Settings sections", exact: true })).toBeHidden();
          await dialog.getByRole("button", { name: "Close", exact: true }).click();
          await openSettings(page);
          await expect(dialog.getByRole("checkbox", { name: "Skip session confirmations", exact: true })).toBeFocused();
          await expect(dialog.locator('.app-settings-panel[aria-label="Agent trust"]')).toBeHidden();
          await dialog.getByRole("button", { name: "Close", exact: true }).click();
        }
      }
      await toggle.check();
      await expect.poll(async () => (await trustState(request, harness)).enabled).toBe(true);
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
      await expect(band).toHaveCount(0);
      await openSettings(page);
      await expect(dialog.locator('.app-settings-panel[aria-label="Sessions"]')).toBeVisible();
      await selectSettingsSection(page, "agent trust");
      await expect(toggle).toBeChecked();
      await toggle.uncheck();
      await expect.poll(async () => (await trustState(request, harness)).enabled).toBe(false);
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
      await expect(band).toBeVisible();

      await expect(page.locator(".tab-slot")).toHaveCount(0);
      await page.locator(".tab-add").click();
      await expect(page.locator(".tab-slot")).toHaveCount(1);
      const tabId = await page.locator(".tab-slot").getAttribute("data-tab-id");
      expect(tabId, "the shell tab has its own terminal identity").toBeTruthy();
      await waitForSessionReady(page, session.id, { tabId: tabId! });
      await expect(page.locator(`.terminal-pane[data-terminal="${tabId}"]`)).toBeVisible();
      await expect(band).toHaveCount(0);
      await page.locator('.tab-strip [data-terminal="agent"]').click();
      await waitForSessionReady(page, session.id);
      await expect(band).toBeVisible();

      observation.managed = false;
      await nudge();
      await expect(surface).toHaveCount(0);
      await expect(page.locator(".app-main .layout")).toHaveAttribute("data-workspace-trust-prompt", "true");
      await expect(band).toHaveCount(0);
      observation.managed = true;
      await nudge();
      await expect(band).toBeVisible();
      await band.getByRole("button", { name: "hide this trust hint for this occurrence", exact: true }).click();
      await expect(band).toHaveCount(0);
      observation.detailTitle = `closed-refresh-${harness}`;
      await nudge();
      await expect(page.locator(".app-main .layout .title")).toHaveText(observation.detailTitle);
      await expect(surface).toHaveAttribute("data-enabled", "false");
      await expect(surface).toHaveAttribute("data-hint-dismissed", "false");
      await expect(band).toHaveCount(0);
      await attachSession(page, neighbor.id);
      await expect(bandFor(page, otherHarness)).toBeVisible();
      await attachSession(page, session.id);
      await expect(surface).toHaveAttribute("data-enabled", "false");
      await expect(surface).toHaveAttribute("data-hint-dismissed", "false");
      await expect(band).toHaveCount(0);
      observation.prompt = false;
      await nudge();
      await expect(page.locator(".app-main .layout")).toHaveAttribute("data-workspace-trust-prompt", "false");
      observation.prompt = true;
      await nudge();
      await expect(band).toBeVisible();

      const other = await context.newPage();
      observer = other;
      await installObservations(other, observations);
      await other.goto("/");
      await attachSession(other, session.id);
      await expect(bandFor(other, harness)).toBeVisible();
      await band.getByRole("button", { name: "Don't show again", exact: true }).click();
      await expect.poll(async () => (await trustState(request, harness)).hint_dismissed).toBe(true);
      await expect(band).toHaveCount(0);
      await expect(other.locator(`.workspace-trust-surface[data-agent="${harness}"]`)).toHaveAttribute(
        "data-hint-dismissed",
        "true",
      );
      await expect(other.locator(".app-main .layout")).toHaveAttribute("data-workspace-trust-prompt", "true");
      await expect(bandFor(other, harness)).toHaveCount(0);
      await attachSession(page, neighbor.id);
      await expect(bandFor(page, otherHarness)).toBeVisible();
      expect((await trustState(request, otherHarness)).hint_dismissed).toBe(false);
      await attachSession(page, session.id);
      await expect(surface).toHaveAttribute("data-hint-dismissed", "true");
      await openSettings(page, "agent trust");
      await field.getByRole("checkbox", {
        name: `Show the hint when ${name} asks about a managed checkout`,
        exact: true,
      }).check();
      await expect.poll(async () => (await trustState(request, harness)).hint_dismissed).toBe(false);
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
      await expect(band).toBeVisible();
      await expect(bandFor(other, harness)).toBeVisible();
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        ["main page routes", () => page.unrouteAll({ behavior: "wait" })],
        ["observer routes", () => observer?.unrouteAll({ behavior: "wait" })],
        ["observer page", () => observer?.close()],
        ["shared trust settings", () => resetTrust(request)],
        ...created.map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
      ]);
    }
  });
}

/** Pi has a resource-approval setting but no recognized question. A command
 * launch with a Codex integration kind also has no structured harness identity;
 * neither may borrow another agent's band from the neutral sampled boolean. */
test("Pi and command launches do not borrow a workspace trust hint", async ({ page, request }) => {
  const created: string[] = [];
  let bodyFailed = false;
  try {
    await resetTrust(request);
    const observations = new Map<string, TrustObservation>();
    for (const harness of ["pi", null] as const) {
      const session = await createSession(request, { title: `trust-no-band-${harness}-${Date.now()}` });
      created.push(session.id);
      observations.set(session.id, { harness, prompt: true, managed: true });
    }
    await installObservations(page, observations);
    await page.goto("/");
    for (const id of created) {
      await attachSession(page, id);
      await expect(page.locator(".app-main .layout")).toHaveAttribute("data-workspace-trust-prompt", "true");
      await expect(page.locator(".workspace-trust-surface")).toHaveCount(0);
      await expect(page.locator(".claude-trust-surface .claude-trust-band")).toHaveCount(0);
    }
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["session routes", () => page.unrouteAll({ behavior: "wait" })],
      ["shared trust settings", () => resetTrust(request)],
      ...created.map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});
