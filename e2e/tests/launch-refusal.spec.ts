/**
 * How the session launcher presents a refused launch (SPEC.md, the launcher's
 * working-directory field and its precondition failures): a bordered,
 * announced "Not launched." alert beside Launch for every refusal, and the
 * folder field marked while a folder refusal still describes the form's
 * destination.
 *
 * Why: the refusal used to be one plain red line under the launch row, with
 * nothing pointing at the folder, and the maintainer launched into a missing
 * folder without noticing the message (2026-10-22).
 */
import { expect, test } from "./helpers/evidence";
import { type Page, type Route } from "@playwright/test";
import { chooseLauncherHost } from "./helpers/host-picker";
import { localHostId } from "./helpers/fleet";
import { fillCreateForm } from "./helpers/term";
import { routeGate } from "./helpers/route-gate";
import { drainedRoute } from "./helpers/drained-route";

const builds = new WeakMap<Page, string>();

/** Routed observations retain the real build stamp, so fixtures cannot latch
 * build skew and accidentally withdraw the reads they intend to exercise. */
test.beforeEach(async ({ page, request }) => {
  const response = await request.get("/api/hosts");
  expect(response.ok()).toBe(true);
  const build = response.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();
  builds.set(page, build);
});

/** Answer only an automatic check, preserving its normal HTTP response shape. */
async function checkReply(route: Route, error?: string) {
  const build = builds.get(route.request().frame().page());
  expect(build).toBeTruthy();
  await route.fulfill({ status: error ? 400 : 200, headers: { "x-farhelm-build": build! }, json: error
    ? { error }
    : { cwd: "/tmp", parent: null, children: [], truncated: false } });
}

/** Open New and return its form, waiting until it is on screen. */
async function openNew(page: Page) {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  return form;
}

/**
 * Spec: a create into a folder that does not exist is refused with the
 * supervisor's own words inside an alert that starts "Not launched."; the
 * folder field is marked invalid and says the folder does not exist on the
 * host; the marking goes away when the folder changes, comes back for the
 * same folder on the same host, and goes away when the host changes. Nothing
 * is created.
 */
test("a missing folder is refused as an alert and marks the folder until the destination changes", async ({ page, request }, testInfo) => {
  // Isolate the launch refusal from advisory observations. Separate tests
  // below exercise real and delayed pre-launch answers.
  await page.route("**/api/browse-directory", async route => {
    if (route.request().postDataJSON().membership_only) await checkReply(route);
    else await route.continue();
  });
  const local = await localHostId(request);
  // The remote row only matters for the host switch, but it must be live
  // there, or the launcher would not offer it as a destination.
  let remote: number | undefined;
  await expect.poll(async () => {
    const listing = await (await request.get("/api/hosts")).json();
    const row = listing.hosts.find((host: { id: number }) => host.id !== local);
    remote = row?.id;
    return row?.state.phase;
  }, { message: "the owned browser stack must supply a connected remote host" }).toBe("connected");
  const missing = `/nonexistent/launch-refusal-${Date.now()}`;
  const title = `launch-refusal-${Date.now()}`;
  await page.goto("/");
  const form = await fillCreateForm(page, { cwd: missing, invocation: "true", title });
  const hostPicker = form.locator(".create-session-host");
  await expect(hostPicker, "the refusal must come from the local host").toHaveAttribute("data-host-id", String(local));
  const folder = form.getByLabel("Folder", { exact: true });
  await expect(folder, "nothing is marked before an attempt").not.toHaveAttribute("aria-invalid", "true");

  await form.locator(".create-session-submit").click();

  const alert = form.locator(".launch-composer-refusal-alert");
  await expect(alert).toHaveAttribute("role", "alert");
  // The helm wraps the supervisor's sentence in its own words, so the
  // refusal is pinned by its start and the supervisor's part, not verbatim.
  const refusalText = alert.locator(".launch-composer-refusal-text");
  await expect(refusalText).toHaveText(/^Not launched\. /);
  await expect(refusalText).toContainText(`working directory does not exist: ${missing}`);
  await expect(alert.locator(".launch-composer-refusal-icon")).toHaveText("⚠");
  await expect(folder).toHaveAttribute("aria-invalid", "true");
  const notice = form.locator(".launch-composer-folder-problem");
  await expect(notice).toContainText("This folder does not exist on");
  await expect(folder).toHaveAttribute("aria-describedby", "launcher-folder-problem");

  // Farhelm currently has one dark palette. Check both OS preferences at
  // phone width and retain the actual rendering for visual inspection.
  await page.setViewportSize({ width: 390, height: 844 });
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    await expect(alert).toBeVisible();
    const bounds = await alert.boundingBox();
    expect(bounds).not.toBeNull();
    expect(bounds!.x).toBeGreaterThanOrEqual(0);
    expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(390);
    await testInfo.attach(`launcher-refusal-phone-${colorScheme}`, { body: await form.screenshot(), contentType: "image/png" });
  }

  await folder.fill("/tmp");
  await expect(folder, "another folder is not the one refused").not.toHaveAttribute("aria-invalid", "true");
  await expect(notice).toHaveCount(0);
  await folder.fill(missing);
  await expect(folder, "the refused folder on the same host is still refused").toHaveAttribute("aria-invalid", "true");
  await expect(notice).toBeVisible();

  await chooseLauncherHost(hostPicker, String(remote));
  await expect(folder, "a typed folder survives the host switch").toHaveValue(missing);
  await expect(folder, "the refusal was about the other host").not.toHaveAttribute("aria-invalid", "true");
  await expect(notice).toHaveCount(0);

  await chooseLauncherHost(hostPicker, String(local));
  await expect(folder).toHaveAttribute("aria-invalid", "true");
  await form.getByRole("group", { name: "Destination type", exact: true })
    .getByRole("button", { name: "Managed checkout", exact: true }).click();
  await expect(folder, "a checkout preview is not the dormant plain folder").toHaveValue("");
  await expect(folder).not.toHaveAttribute("aria-invalid", "true");
  await expect(notice).toHaveCount(0);

  const listing = await (await request.get("/api/sessions")).json();
  expect(listing.sessions.some((session: { title: string }) => session.title === title)).toBe(false);
});

/** A known missing folder warns before any launch without blocking the
 * command. A valid folder clears the advice; the host still decides when
 * Launch is pressed, so a refused create remains an announced alert. */
test("folder checks warn before Launch and never prevent an attempt", async ({ page }) => {
  await page.goto("/");
  const checked = page.waitForResponse(response => {
    if (!response.url().endsWith("/api/browse-directory")) return false;
    const input = response.request().postDataJSON();
    return input.membership_only && input.cwd === "/tmp";
  });
  const form = await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title: "advisory-refusal" });
  expect((await checked).ok(), "the real existing folder must pass the host's check").toBe(true);
  await expect(form).toHaveAttribute("data-folder-check-pending", "false");
  const folder = form.getByLabel("Folder", { exact: true });
  const notice = form.locator(".launch-composer-folder-problem");
  await expect(notice).toHaveCount(0);
  await folder.fill(`/nonexistent/advisory-${Date.now()}`);
  await expect(notice).toContainText("This folder does not exist on");
  await expect(folder).toHaveAttribute("aria-invalid", "true");
  const launch = form.locator(".create-session-submit");
  await expect(launch).toBeEnabled();
  await launch.click();
  await expect(form.getByRole("alert")).toContainText("Not launched.");
  await expect(form.getByRole("alert")).toContainText("working directory does not exist:");
  await folder.fill("/tmp");
  await expect(notice).toHaveCount(0);
});

/** Hold the first response past a path edit. Its stale missing-folder answer
 * must be dropped, and the next check must wait for that response rather than
 * starting an overlapping read every time the debounce restarts. */
test("a stale check is dropped and checks remain serial while a reply is held", async ({ page }) => {
  const old = `/nonexistent/old-check-${Date.now()}`;
  const first = routeGate();
  const second = routeGate();
  const paths: string[] = [];
  const stop = await drainedRoute(page, "**/api/browse-directory", async route => {
    const input = route.request().postDataJSON();
    if (!input.membership_only) return route.continue();
    if (paths.length === 0 && input.cwd !== old) return checkReply(route);
    paths.push(input.cwd);
    if (input.cwd === old) {
      await first.wait();
      await checkReply(route, `working directory does not exist: ${old}`);
    } else {
      await second.wait();
      await checkReply(route);
    }
  });
  try {
    await page.goto("/");
    const form = await fillCreateForm(page, { cwd: old, invocation: "true", title: "serial-check" });
    await expect.poll(() => paths).toEqual([old]);
    const completedBefore = Number(await form.getAttribute("data-folder-check-completions"));
    await form.getByLabel("Folder", { exact: true }).fill("/tmp");
    await expect(form.getByLabel("Folder", { exact: true })).toHaveValue("/tmp");
    // sleep-ok: hold the first reply past the 150ms debounce to expose an overlapping second read.
    await page.waitForTimeout(300);
    expect(paths, "a debounce restart must not overlap the held request").toEqual([old]);
    first.release();
    await expect.poll(() => paths).toEqual([old, "/tmp"]);
    await expect.poll(async () => Number(await form.getAttribute("data-folder-check-completions"))).toBe(completedBefore + 1);
    await expect(form.locator(".launch-composer-folder-problem"), "the stale missing-folder response must not mark /tmp").toHaveCount(0);
    second.release();
    await expect.poll(async () => Number(await form.getAttribute("data-folder-check-completions"))).toBe(completedBefore + 2);
    await expect(form.getByLabel("Folder", { exact: true })).not.toHaveAttribute("aria-invalid", "true");
  } finally {
    first.release();
    second.release();
    await stop();
  }
});

/** Permission and busy errors are inconclusive, even though the request
 * failed. Calling a folder missing in those cases would invent a host fact. */
test("an inconclusive folder check shows no warning", async ({ page }) => {
  const answer = routeGate();
  let admitted = false;
  const stop = await drainedRoute(page, "**/api/browse-directory", async route => {
    const input = route.request().postDataJSON();
    if (!input.membership_only) return route.continue();
    if (input.cwd === "/tmp") {
      admitted = true;
      await answer.wait();
      await checkReply(route, "working directory is not usable: /tmp (permission denied)");
    } else await checkReply(route);
  });
  try {
    await page.goto("/");
    const form = await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title: "inconclusive-check" });
    await expect.poll(() => admitted, { message: "hold the check for the actual /tmp draft" }).toBe(true);
    const before = Number(await form.getAttribute("data-folder-check-completions"));
    answer.release();
    await expect(form).toHaveAttribute("data-folder-check-completions", String(before + 1));
    await expect(form.locator(".launch-composer-folder-problem")).toHaveCount(0);
    await expect(form.getByLabel("Folder", { exact: true })).not.toHaveAttribute("aria-invalid", "true");
  } finally {
    answer.release();
    await stop();
  }
});

/**
 * Failure before dispatch is still a refused Launch press. Make both browser
 * key generators fail so the async preparation exit, rather than a supervisor
 * response, must publish the same announced alert without sending a create.
 */
test("failed asynchronous launch preparation is an announced refusal", async ({ page }) => {
  await page.goto("/");
  const form = await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title: "unsent-refusal" });
  let creates = 0;
  await page.route("**/api/sessions", async route => {
    if (route.request().method() === "POST") creates++;
    await route.continue();
  });
  await page.evaluate(() => {
    const uuid = Object.getOwnPropertyDescriptor(globalThis.crypto, "randomUUID");
    const random = Math.random;
    Object.defineProperty(globalThis.crypto, "randomUUID", { value: () => "", configurable: true });
    Math.random = () => { throw new Error("fixture key generation failure"); };
    (window as any).__restoreRefusalRandom = () => {
      if (uuid) Object.defineProperty(globalThis.crypto, "randomUUID", uuid);
      else delete (globalThis.crypto as any).randomUUID;
      Math.random = random;
    };
  });
  try {
    expect(await page.evaluate(() => crypto.randomUUID())).toBe("");
    await form.locator(".create-session-submit").click();
    const alert = form.getByRole("alert");
    await expect(alert).toContainText("Not launched. Could not generate an idempotency key");
    await expect(alert).toContainText("so it was not sent");
    expect(creates, "the failure must precede host dispatch").toBe(0);
  } finally {
    await page.evaluate(() => {
      (window as any).__restoreRefusalRandom();
      delete (window as any).__restoreRefusalRandom;
    });
  }
});

/**
 * Spec: an error that does not come from a launch press, here "save as
 * template" refusing an unfinished wrapper, appears beside Launch without the
 * "Not launched." alert, even right after a refused launch put one there.
 *
 * Why: the error slot is shared, and labelling a template-save refusal "Not
 * launched." would say a launch happened that the user never asked for.
 */
test("a save-as-template refusal is not labelled Not launched", async ({ page }) => {
  const form = await openNew(page);
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  await form.getByLabel("Folder", { exact: true }).fill("/tmp");
  const wrapper = form.locator(".launch-composer-wrapper");
  await expect(wrapper).toBeVisible();
  await wrapper.fill("with-endpoint 'unfinished");
  await form.locator(".create-session-submit").click({ force: true });
  const alert = form.locator(".launch-composer-refusal-alert");
  await expect(alert, "a refused launch press is an alert").toContainText("Not launched. Invalid wrapper prefix");

  await form.getByRole("button", { name: "Save as template", exact: true }).click();

  await expect(form.locator(".launch-composer-refusal")).toContainText("Invalid wrapper prefix");
  await expect(alert).toHaveCount(0);
  await expect(form.locator(".launch-composer-refusal")).not.toContainText("Not launched.");
});
