/** Contained UI contracts whose failure needs the actual reactive browser.
 * Each case owns its session or routes; the fixture helm and supervisor stay
 * real, and injected responses carry the build of that same helm. */
import { expect, test } from "./helpers/evidence";
import { cleanupSession, createResumableSession, createSession, forceBuildSkew, listSessions, openHostMenu, openRowMenu, stopSession, stubFeed } from "./helpers/fleet";
import { waitForTermText } from "./helpers/term";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";
import { FAKE_AGENT_INVOCATION, runInShell, shellMarker } from "./helpers/terminal-suite";

/** Header writes remain attended work under build skew. A successful restart
 * must update the sidebar's ended state, and Replace must remove its source
 * and show its replacement without relying on background invalidations. */
test("header restart and replace refresh the sidebar under build skew", async ({ page, request }) => {
  const source = await createResumableSession(request, { title: `header-skew-${Date.now()}`, cwd: "/tmp" });
  let agentAttachments = 0;
  page.on("websocket", (socket) => {
    const url = new URL(socket.url());
    if (url.pathname === `/api/sessions/${encodeURIComponent(source.id)}/term` && !url.searchParams.has("tab")) agentAttachments++;
  });
  let replacement: string | undefined;
  try {
    await stopSession(request, source.id);
    await expect.poll(async () => (await listSessions(request)).sessions.find((s) => s.id === source.id)?.status.state, { timeout: 20_000 }).toBe("exited");
    await forceBuildSkew(page, "9.9.9-ui-plan");
    await stubFeed(page);
    await page.goto("/");
    await expect(page.locator(".build-skew")).toBeVisible();
    const sourceRow = page.locator(`[data-session-id="${source.id}"]`);
    await expect(sourceRow.locator(".status-badge")).toHaveClass(/\bexited\b/);
    await sourceRow.locator(".session-row-open").click();
    await waitForSessionRevealed(page, source.id);
    await expect(page.locator(".titlebar .status-badge")).toHaveClass(/\bexited\b/);
    const beforeRestart = agentAttachments;
    await page.locator(".restart-primary").click();
    await expect.poll(() => agentAttachments, { timeout: 20_000 }).toBeGreaterThan(beforeRestart);
    await waitForSessionRevealed(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator("#term-banner")).toBeHidden();
    await expect(sourceRow.locator(".status-badge")).not.toHaveClass(/\bexited\b/, { timeout: 20_000 });
    await page.locator(".header-replace").click();
    const reply = page.waitForResponse((r) => r.url().endsWith(`/api/sessions/${source.id}/replace`) && r.request().method() === "POST");
    await page.locator(".header-replace-confirm").getByRole("button", { name: "replace", exact: true }).click();
    const response = await reply;
    expect(response.ok()).toBe(true);
    const body = await response.json();
    replacement = body.id;
    await expect(sourceRow).toHaveCount(0, { timeout: 20_000 });
    await expect(page.locator(`[data-session-id="${replacement}"]`)).toBeVisible();
    await waitForSessionRevealed(page, replacement!);
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await cleanupSession(request, source.id);
    if (replacement) await cleanupSession(request, replacement);
  }
});

/** Removing automatic capability must not make a discovered tab take the
 * displacing route. The view has no takeover latch here, so that separate
 * safeguard cannot conceal the mount-policy bug; an explicit banner action
 * must still attach a working shell. */
test("a newly discovered tab waits for intent under build skew", async ({ page, request }) => {
  const session = await createSession(request, { title: `tab-skew-${Date.now()}`, cwd: "/tmp", invocation: FAKE_AGENT_INVOCATION });
  const socketUrls: string[] = [];
  page.on("websocket", (socket) => socketUrls.push(socket.url()));
  try {
    await forceBuildSkew(page, "9.9.9-tab-plan");
    await stubFeed(page);
    await page.goto("/");
    await page.locator(`[data-session-id="${session.id}"] .session-row-open`).click();
    await waitForSessionRevealed(page, session.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".build-skew")).toBeVisible();
    await expect(page.locator("#term-banner")).toBeHidden();
    await page.locator(".tab-add").click();
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    const tab = (await page.locator(".tab-slot").getAttribute("data-tab-id"))!;
    expect(tab, "the new tab has an opaque id").toBeTruthy();
    const pane = page.locator(`.terminal-pane[data-terminal="${tab}"]`);
    await expect(pane.locator(".banner")).toContainText("automatic attachment unavailable");
    expect(await page.evaluate((id) => Boolean((window as any).__farhelmIslands?.[`terminal-${id}`]), tab)).toBe(false);
    expect(socketUrls.some((url) => new URL(url).searchParams.get("tab") === tab)).toBe(false);
    await pane.locator(".banner-reclaim").click();
    const live = shellMarker("SKEW-MANUAL-TAB");
    await runInShell(page, `terminal-${tab}`, live.command, live.expected);
    expect(socketUrls.some((url) => new URL(url).searchParams.get("tab") === tab)).toBe(true);
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await cleanupSession(request, session.id);
  }
});

/** A displayed escape is an explanation, never an implicit rename. Saving
 * untouched preserves the original bytes; deliberately typing that visible
 * spelling submits literal text, even though it looks like the seed. */
test("rename shows hidden title characters and preserves an untouched seed", async ({ page, request }) => {
  const raw = `rename-${Date.now()}-\u202Ereverse\u200B`;
  const session = await createSession(request, { title: raw, invocation: "sleep 300" });
  const sent: string[] = [];
  await page.route(`**/api/sessions/${session.id}/rename`, async (route) => {
    if (route.request().method() === "POST") sent.push(route.request().postDataJSON().title);
    await route.fallback();
  });
  try {
    await page.goto("/");
    const row = page.locator(`[data-session-id="${session.id}"]`);
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    const field = page.locator(".rename-input");
    const shown = await field.inputValue();
    expect(shown).not.toBe(raw);
    expect(shown).not.toMatch(/[\u202E\u200B]/u);
    await page.locator(".rename-submit").click();
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    expect(sent).toEqual([raw]);
    await openRowMenu(row);
    await row.locator(".session-row-rename").click();
    await field.fill(`${shown} edited`);
    await field.fill(shown);
    await page.locator(".rename-submit").click();
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    expect(sent).toEqual([raw, shown]);
    expect((await listSessions(request)).sessions.find((s) => s.id === session.id)?.title).toBe(shown);
  } finally {
    await cleanupSession(request, session.id);
  }
});

/** The longest ordinary phase wording must not push the only host action
 * entry point outside a 240px sidebar. Use the real registry's row identity,
 * replacing only the phase so this needs no unreachable network endpoint. */
test("an unreachable host keeps its menu inside the minimum sidebar", async ({ page, request }) => {
  const hosts = await (await request.get("/api/hosts")).json();
  const remote = hosts.hosts.find((h: any) => h.kind === "ssh");
  expect(remote, "the fixture has a remote host row").toBeTruthy();
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.hosts.find((h: any) => h.id === remote.id).state = { phase: "unreachable-reprobing", cause: "transport-failure", last_error: "fixture unreachable" };
    await route.fulfill({ response, json: body });
  });
  await page.goto("/");
  const host = page.locator(`[data-host-id="${remote.id}"]`);
  await expect(host.locator(".host-status-label")).toHaveText("unreachable, retrying");
  const handle = page.getByRole("separator", { name: "Sidebar width" });
  await expect(handle).toHaveAttribute("data-sidebar-bound", "true");
  await handle.focus();
  await expect(handle).toBeFocused();
  // The separator supports ten-pixel arrow steps. Reach the minimum through
  // that public gesture before checking the host row's actual geometry.
  for (let step = 0; step < 10; step++) await page.keyboard.press("ArrowLeft");
  await expect(handle).toHaveAttribute("aria-valuenow", "240");
  await expect(host.locator(".host-status-label")).toHaveAttribute("data-tooltip", "unreachable, retrying");
  const sidebar = (await page.locator(".app-sidebar").boundingBox())!;
  const button = host.locator(".host-row-menu");
  const box = (await button.boundingBox())!;
  expect(box.x + box.width).toBeLessThanOrEqual(sidebar.x + sidebar.width);
  await openHostMenu(host);
  await expect(host.locator(".host-settings")).toBeVisible();
});

/** Refused writes restore both native switches and leave stored settings
 * unchanged. This is browser smoke coverage of the refusal outcome; network
 * replies cannot force the missed-busy-render case, which needs source review. */
test("refused host permission switches restore their stored values", async ({ page, request }) => {
  const reply = await request.get("/api/hosts");
  const local = (await reply.json()).hosts.find((h: any) => h.kind === "local");
  expect(local).toBeTruthy();
  let refusals = 0;
  for (const endpoint of ["yolo-without-asking", "commands-without-asking"]) {
    await page.route(`**/api/hosts/${local.id}/${endpoint}`, (route) => {
      refusals++;
      return route.fulfill({ status: 409, headers: { "x-farhelm-build": reply.headers()["x-farhelm-build"] }, body: "permission refused" });
    });
  }
  await page.goto("/");
  const row = page.locator(`[data-host-id="${local.id}"]`);
  await openHostMenu(row);
  await row.locator(".host-settings").click();
  const dialog = page.locator(".host-settings-dialog");
  for (const [selector, value] of [[".host-yolo-without-asking-toggle", local.yolo_without_asking], [".host-commands-without-asking-toggle", local.commands_without_asking]] as const) {
    const toggle = dialog.locator(selector);
    await expect(toggle).toBeChecked({ checked: value });
    const before = refusals;
    await toggle.click();
    await expect.poll(() => refusals).toBe(before + 1);
    await expect(dialog).toContainText("permission refused");
    await expect(toggle).toBeChecked({ checked: value });
    await expect(toggle).toBeEnabled();
  }
  const after = (await (await request.get("/api/hosts")).json()).hosts.find((h: any) => h.id === local.id);
  expect([after.yolo_without_asking, after.commands_without_asking]).toEqual([local.yolo_without_asking, local.commands_without_asking]);
});
