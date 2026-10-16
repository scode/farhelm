import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { cleanupSession, createSession, patchPreferences, readPreferences, resetPreferences } from "./helpers/fleet";
import { attachSession } from "./helpers/term";

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
      await target.route((url) => url.pathname === "/api/sessions" || url.pathname === `/api/sessions/${session.id}`, async (route) => {
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

    await band.getByRole("button", { name: "Settings and caveats", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    const field = dialog.locator("#checkout-claude-trust");
    const toggle = field.getByRole("checkbox", { name: 'answer Claude\'s "trust this folder?" for new checkouts', exact: true });
    await expect(field).toBeFocused();
    await expect(toggle).toBeEnabled();
    await expect(toggle).not.toBeChecked();
    expect(await toggle.evaluate((element) => {
      const box = element.getBoundingClientRect();
      const body = element.closest(".host-settings-body")!.getBoundingClientRect();
      return box.top >= body.top && box.bottom <= body.bottom;
    }), "settings opens scrolled to the opt-in").toBe(true);
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
    await page.getByRole("button", { name: "settings", exact: true }).click();
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
    await page.getByRole("button", { name: "settings", exact: true }).click();
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
