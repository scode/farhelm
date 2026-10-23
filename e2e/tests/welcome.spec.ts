/** First-open welcome behavior uses the real shared helm preference. Each
 * case restores seen even on failure so later specs and captures cannot be
 * obscured. Feedback submissions and external docs navigation are intercepted;
 * the card's handoff must never send a message on the user's behalf.
 */
import { Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { patchPreferences, readPreferences, resetPreferences } from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { DEVICE_SECRET_KEY } from "./helpers/device-auth";
import { requireHelmBuild } from "./helpers/helm-build";
import { waitForReplayReveal } from "./helpers/terminal-readiness";
import { currentStackSlot } from "../stack-slots";

const GET_STARTED_URL = "https://farhelm.io/docs/get-started/first-session/";
let helmBuild: string;

// Reset every shared preference, not only the welcome, so a case's writes
// through the real helm cannot leak into later specs sharing this helm.
test.beforeEach(async ({ page, request }) => {
  await resetPreferences(request);
  await patchPreferences(request, { welcome_seen: false });
  expect((await readPreferences(request)).welcome_seen, "premise: this helm is unseen").toBe(false);
  const stamp = await request.get("/api/preferences");
  expect(stamp).toBeOK();
  helmBuild = requireHelmBuild(stamp, "welcome refusal fixtures");
  await page.route("**/api/feedback", route => route.fulfill({ status: 500, body: "blocked by welcome test" }));
});

test.afterEach(async ({ request }) => {
  await resetPreferences(request);
});

/** Wait for the real mounted modal and its initial focus before sending input. */
async function firstWelcome(page: Page) {
  await page.goto("/");
  const dialog = page.getByRole("dialog", { name: "welcome to farhelm", exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".welcome-got-it")).toBeFocused();
  return dialog;
}

for (const dismissal of ["got it", "close", "Escape"] as const) {
  /** Every first-open dismissal saves seen. Reload and another browser context
   * must read the helm's answer, rather than a device-local dismissal cache.
   */
  test(`welcome dismissal by ${dismissal} is shared and survives reload`, async ({ page, request, browser }) => {
    const dialog = await firstWelcome(page);
    await waitForReplayReveal(page, "terminal");
    await expect(dialog.locator(".welcome-got-it")).toBeFocused();
    if (dismissal === "Escape") await page.keyboard.press("Escape");
    else if (dismissal === "close") await dialog.getByRole("button", { name: "close welcome", exact: true }).click();
    else await dialog.getByRole("button", { name: "got it", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.locator("#terminal .xterm-helper-textarea")).toBeFocused();
    await expect.poll(async () => (await readPreferences(request)).welcome_seen).toBe(true);
    await page.reload();
    await expect(page.locator(".new-session-button")).toBeVisible();
    await expect(dialog).toHaveCount(0);
    const other = await browser.newContext({ storageState: currentStackSlot().authStatePath });
    try {
      const seed = await other.storageState();
      expect(seed.origins.flatMap(origin => origin.localStorage.map(item => item.name)),
        "premise: second client has authentication only, no dismissal cache").toEqual([DEVICE_SECRET_KEY]);
      const second = await other.newPage();
      await second.goto(new URL("/", page.url()).href);
      await expect(second.locator(".new-session-button")).toBeVisible();
      await expect(second.getByRole("dialog", { name: "welcome to farhelm", exact: true })).toHaveCount(0);
    } finally {
      await other.close();
    }
  });
}

/** Read failure and the gate's bounded timeout both use defaults for the app,
 * but neither is evidence that the welcome is unseen. A mounted New control
 * establishes that the gate finished before checking the absence of a modal.
 */
for (const failure of ["refused", "timed out"] as const) {
  test(`welcome stays closed when preferences are ${failure}`, async ({ page }) => {
    const gate = routeGate();
    let readStarted = false;
    await page.route("**/api/preferences", async route => {
      if (route.request().method() !== "GET") return route.continue();
      readStarted = true;
      if (failure === "timed out") {
        await gate.wait();
        // The seed deadline may have aborted this request before release.
        await route.fulfill({ status: 500, headers: { "x-farhelm-build": helmBuild }, body: "preferences unavailable" }).catch(() => {});
      } else await route.fulfill({ status: 500, headers: { "x-farhelm-build": helmBuild }, body: "preferences unavailable" });
    });
    try {
      await page.goto("/");
      await expect.poll(() => readStarted).toBe(true);
      await expect(page.locator(".new-session-button")).toBeVisible({ timeout: 15_000 });
      await expect(page.getByRole("dialog", { name: "welcome to farhelm", exact: true })).toHaveCount(0);
      await expect(page.locator(".build-skew")).toHaveCount(0);
    } finally {
      gate.release();
    }
  });
}

/** A recognized startup auth refusal is not an authenticated welcome decision.
 * Once sign-in completes, the first successful seed still offers the card.
 */
test("welcome waits for initial sign-in before deciding", async ({ page }) => {
  const secret = await page.context().storageState().then(state => state.origins
    .flatMap(origin => origin.localStorage).find(item => item.name === DEVICE_SECRET_KEY)?.value);
  expect(secret, "premise: the harness supplies a device identity").toBeTruthy();
  let reads = 0;
  await page.route("**/api/preferences", async route => {
    if (route.request().method() === "GET" && ++reads === 1) {
      await route.fulfill({ status: 401, headers: { "x-farhelm-build": helmBuild }, contentType: "application/json",
        body: JSON.stringify({ code: "device_auth_required", error: "sign in" }) });
    } else await route.continue();
  });
  await page.route("**/api/auth/token", route => route.fulfill({ status: 200,
    headers: { "x-farhelm-build": helmBuild }, contentType: "application/json",
    body: JSON.stringify({ device_secret: secret }) }));
  await page.goto("/");
  await expect(page.locator(".auth-page")).toBeVisible();
  await page.locator(".auth-token-input").fill("AAAAAAAAAAAAAAAAAAAAAA");
  await page.locator(".auth-submit").click();
  await expect(page.getByRole("dialog", { name: "welcome to farhelm", exact: true })).toBeVisible();
  // The refused read, the reloaded page's read after sign-in, and automatic selection's
  // own reread before it picks a session.
  await expect.poll(() => reads).toBe(3);
});

/** Failed persistence cannot trap a user in the card. A later full load may
 * offer it again because no successful write changed the shared helm row.
 */
test("a refused seen write still dismisses the welcome", async ({ page, request }) => {
  let seenWrites = 0;
  await page.route("**/api/preferences", async route => {
    if (route.request().method() === "PUT" && route.request().postDataJSON().welcome_seen === true) {
      seenWrites++;
      await route.fulfill({ status: 500, headers: { "x-farhelm-build": helmBuild }, body: "write unavailable" });
    } else await route.continue();
  });
  const dialog = await firstWelcome(page);
  await dialog.locator(".welcome-got-it").click();
  await expect(dialog).toHaveCount(0);
  await expect.poll(() => seenWrites).toBe(1);
  await expect(page.locator(".build-skew")).toHaveCount(0);
  expect((await readPreferences(request)).welcome_seen).toBe(false);
  await page.reload();
  await expect(dialog).toBeVisible();
});

/** Get started keeps the card open; global launch chords leave its focused
 * control alone. Short windows keep the closing controls outside body scroll.
 */
test("welcome links, modal isolation and short-window dismissal", async ({ page, context }, testInfo) => {
  await context.route(GET_STARTED_URL, route => route.fulfill({ contentType: "text/html", body: "guide fixture" }));
  const dialog = await firstWelcome(page);
  await page.keyboard.press("Control+Shift+P");
  await page.keyboard.press("Control+Shift+1");
  await expect(dialog.locator(".welcome-got-it")).toBeFocused();
  await expect(page.getByRole("dialog")).toHaveCount(1);
  const hint = (await dialog.locator(".welcome-footer > span").boundingBox())!;
  const answer = (await dialog.locator(".welcome-got-it").boundingBox())!;
  expect(hint.x + hint.width, "footer guidance stays beside its closing answer").toBeLessThanOrEqual(answer.x);
  await dialog.screenshot({ path: testInfo.outputPath("welcome.png") });
  const external = context.waitForEvent("page");
  await dialog.getByRole("link", { name: "Read Get started ↗", exact: true }).click();
  const guide = await external;
  try {
    await guide.waitForURL(GET_STARTED_URL);
    await expect(dialog).toBeVisible();
  } finally {
    await guide.close();
  }
  await page.setViewportSize({ width: 380, height: 320 });
  const body = dialog.locator(".welcome-body");
  await expect.poll(() => body.evaluate(node => node.scrollHeight > node.clientHeight)).toBe(true);
  for (const control of [".welcome-got-it", ".welcome-dismiss"]) {
    expect(await dialog.locator(control).evaluate(node => {
      const box = node.getBoundingClientRect();
      return node.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
    }), `${control} stays reachable`).toBe(true);
  }
  await dialog.screenshot({ path: testInfo.outputPath("welcome-short.png") });
  await dialog.locator(".welcome-dismiss").click();
  await expect(dialog).toHaveCount(0);
});

/** The handoff replaces the welcome with the ordinary feedback dialog and
 * records first-open dismissal, without initiating any submission.
 */
test("send feedback now records seen and hands focus to feedback", async ({ page, request }) => {
  let sends = 0;
  page.on("request", request => { if (request.url().endsWith("/api/feedback")) sends++; });
  const dialog = await firstWelcome(page);
  await dialog.getByRole("button", { name: "Send feedback now", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  const feedback = page.getByRole("dialog", { name: "send feedback", exact: true });
  await expect(feedback).toBeVisible();
  await expect(feedback.locator(".feedback-message")).toBeFocused();
  await expect.poll(async () => (await readPreferences(request)).welcome_seen).toBe(true);
  expect(sends).toBe(0);
});

/** Reopening is an explicit help action, not another dismissal write. Escape
 * returns keyboard focus to the help toggle that opened the card.
 */
test("help reopens welcome without rewriting seen", async ({ page, request }) => {
  await patchPreferences(request, { welcome_seen: true });
  let seenWrites = 0;
  page.on("request", request => {
    if (request.url().endsWith("/api/preferences") && request.method() === "PUT" && request.postDataJSON().welcome_seen !== undefined) seenWrites++;
  });
  await page.goto("/");
  const toggle = page.getByRole("button", { name: "help", exact: true });
  await toggle.click();
  const menu = page.getByRole("menu", { name: "help", exact: true });
  await expect(menu).toBeVisible();
  await menu.getByRole("menuitem", { name: /welcome/ }).click();
  const dialog = page.getByRole("dialog", { name: "welcome to farhelm", exact: true });
  await expect(dialog.locator(".welcome-got-it")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(toggle).toBeFocused();
  expect(seenWrites).toBe(0);
});
