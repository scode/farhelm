/** Rendering proofs over real Settings saves and controlled host listings.
 * Rust tests cover transport, actor ordering and the actual failure response;
 * these cases establish what both browser engines show after that response.
 */
import { expect, test } from "./helpers/evidence";
import { openHostsPanel } from "./helpers/fleet";
import { openSettings } from "./helpers/settings";

/** A successful save must close and clear its secret draft even when some
 * hosts could not receive it. A later successful delivery clears the notice.
 */
test("saved credentials report delivery failures without retaining the secret draft", async ({ page, request }, info) => {
  const name = `delivery-${info.workerIndex}`;
  const created = await request.post("/api/credentials", {
    data: { name, kind: "endpoint_token", secret: "fictional-browser-first-token" },
  });
  expect(created.status(), await created.text()).toBe(201);
  let failed = true;
  const url = `**/api/credentials/${name}`;
  await page.route(url, async (route) => {
    if (route.request().method() !== "PUT") return route.continue();
    const response = await route.fetch();
    expect(response.ok(), await response.text()).toBe(true);
    const body = await response.json();
    expect(body.credential.name).toBe(name);
    body.delivery_failures = failed ? [{ host: "Example host", reason: "could not install the agent settings" }] : [];
    await route.fulfill({ response, json: body });
  });
  try {
    await page.goto("/");
    const dialog = await openSettings(page, "credentials");
    const panel = dialog.locator("#settings-credentials");
    const row = panel.locator(".credential-row").filter({ hasText: name });
    await expect(row).toContainText("Saved");
    await row.getByRole("button", { name: `Edit ${name}`, exact: true }).click();
    const editor = panel.locator(".credential-editor");
    await expect(editor.getByLabel("Token", { exact: true })).toHaveValue("");
    await editor.getByLabel("Token", { exact: true }).fill("fictional-browser-replaced-token");
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(editor).toHaveCount(0);
    await expect(row).toContainText("Replaced");
    const warning = panel.locator(".credential-delivery-warning[role=status]");
    await expect(warning).toHaveText("Saved; not delivered to Example host: could not install the agent settings");
    const metadata = await request.get("/api/credentials");
    const text = await metadata.text();
    expect(text).not.toContain("fictional-browser-replaced-token");
    expect((JSON.parse(text) as { name: string; revision: number }[]).find((row) => row.name === name)?.revision).toBe(2);
    await row.getByRole("button", { name: `Edit ${name}`, exact: true }).click();
    await expect(editor.getByLabel("Token", { exact: true })).toHaveValue("");
    failed = false;
    await editor.getByLabel("Token", { exact: true }).fill("fictional-browser-final-token");
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(editor).toHaveCount(0);
    await expect(warning).toHaveCount(0);
  } finally {
    if (!page.isClosed()) await page.goto("about:blank");
    await page.unroute(url);
    expect((await request.delete(`/api/credentials/${name}`)).ok()).toBe(true);
  }
});

/** A connected host remains usable while a credential warning makes its
 * status and navigation mark amber. The reason must stay readable in either
 * palette; clearing it restores normal presentation without changing phase.
 */
for (const palette of ["dark", "light"] as const) {
  test(`connected credential warnings remain readable in ${palette}`, async ({ page }, info) => {
    let warning: string | null = "could not install the agent settings";
    await page.emulateMedia({ colorScheme: palette });
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts.push({
        ...body.hosts[0], id: 9002, kind: "tensorlake", name: "Credential host", alias: "Credential host",
        destination: "fixture@fixture.sandbox.tensorlake.ai", identity: null, incarnation: 1,
        state: { phase: "connected", identity: null, build_version: "0.1.0", old_version: false, newer_version: false, refresh: { status: "ok", sessions: 0 } },
        tensorlake: { sandbox: "fixture", recipe: "inference", size: { cpus: 2, memory_gb: 4, disk_gb: 20 }, tunnels: null, credential_warning: warning },
      });
      await route.fulfill({ response, json: body });
    });
    await page.goto("/");
    await expect(page.locator("html")).toHaveAttribute("data-appearance", palette);
    await openHostsPanel(page);
    const host = page.locator('.host-row[data-host-id="9002"]');
    await expect(host.locator(".host-status")).toHaveClass(/unreachable/);
    const mark = page.locator('[data-rail-host-id="9002"] .host-rail-state');
    await expect(mark).toHaveClass(/unreachable/);
    const notice = host.locator(".credential-delivery-warning[role=status]");
    await expect(notice).toHaveText("Credentials not delivered: could not install the agent settings");
    await expect(host).toContainText("Connected");
    await notice.scrollIntoViewIfNeeded();
    await host.screenshot({ path: info.outputPath(`credential-host-${palette}.png`) });
    warning = null;
    await page.reload();
    await openHostsPanel(page);
    await expect(host.locator(".host-status")).toHaveClass(/connected/);
    await expect(mark).toHaveClass(/connected/);
    await expect(notice).toHaveCount(0);
  });
}
