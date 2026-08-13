// Browser authentication and token rotation, end to end against the real
// helm. Storage and middleware mechanics stay in Rust tests; this file owns
// the behavior only a browser can prove: the in-page prompt, localStorage's
// natural persistence in a closed-and-reopened browser profile, and live
// WebSocket teardown on rotation.
import { chromium, expect, Page, Route, test, webkit } from "@playwright/test";
import { execFile } from "node:child_process";
import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import {
  AUTH_STORAGE_STATE_PATH,
  DEVICE_SECRET_KEY,
  refreshHarnessAuthorization,
  requireProductPageAuth,
} from "./helpers/device-auth";

const run = promisify(execFile);

interface StackInfo {
  farhelm: string;
  state: string;
}

async function stackInfo(): Promise<StackInfo> {
  return JSON.parse(
    await readFile(path.join(__dirname, "..", ".stack-info.json"), "utf8"),
  ) as StackInfo;
}

/** Run one shipped token command against the harness's actual state dir. */
async function token(command: "show" | "rotate"): Promise<string> {
  const info = await stackInfo();
  const { stdout } = await run(info.farhelm, [
    "helm",
    "token",
    command,
    "--state-dir",
    info.state,
  ]);
  const value = stdout.trim();
  if (!value) throw new Error(`farhelm helm token ${command} returned no token`);
  return value;
}

/** Exchange the token and wait for the auth surface to yield the page.
 *
 * This does not assume which application surface comes back. Authentication
 * temporarily unmounts the current surface; it does not reset navigation, so
 * first-time login restores the list while rotation from an open session
 * restores that session. Each caller asserts the surface it actually owns.
 */
async function authenticate(page: Page, value: string) {
  await page.locator(".auth-token-input").fill(value);
  await page.locator(".auth-submit").click();
  await expect(page.locator(".auth-page")).toHaveCount(0);
}

test("an unauthenticated browser gets an in-page token prompt", async ({
  browser,
  baseURL,
}) => {
  const context = await browser.newContext({
    storageState: { cookies: [], origins: [] },
    extraHTTPHeaders: {},
  });
  const page = await context.newPage();
  let dialogs = 0;
  page.on("dialog", async (dialog) => {
    dialogs += 1;
    await dialog.dismiss();
  });

  await page.goto(baseURL!);
  await expect(page.locator(".auth-page")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Authenticate this device" })).toBeVisible();
  expect(dialogs, "authentication must never use a browser dialog").toBe(0);
  await authenticate(page, await token("show"));
  await expect(page.locator(".session-list")).toBeVisible();
  await context.close();
});

test("a device secret survives a browser restart", async ({ browserName, baseURL }) => {
  const profile = await mkdtemp(path.join(os.tmpdir(), "farhelm-auth-profile-"));
  try {
    const browserType = browserName === "webkit" ? webkit : chromium;
    const first = await browserType.launchPersistentContext(profile, {
      storageState: { cookies: [], origins: [] },
      extraHTTPHeaders: {},
    });
    const page = first.pages()[0] ?? (await first.newPage());
    await page.goto(baseURL!);
    await expect(page.locator(".auth-page")).toBeVisible();
    await authenticate(page, await token("show"));
    await expect(page.locator(".session-list")).toBeVisible();
    await first.close();

    const restarted = await browserType.launchPersistentContext(profile, {
      storageState: { cookies: [], origins: [] },
      extraHTTPHeaders: {},
    });
    const reopened = restarted.pages()[0] ?? (await restarted.newPage());
    await reopened.goto(baseURL!);
    await expect(reopened.locator(".session-list")).toBeVisible();
    await expect(reopened.locator(".auth-page")).toHaveCount(0);
    await restarted.close();
  } finally {
    await rm(profile, { recursive: true, force: true });
  }
});

test("blank and rejected tokens stay on the prompt and allow retry", async ({
  browser,
  baseURL,
}) => {
  const context = await browser.newContext({
    storageState: { cookies: [], origins: [] },
    extraHTTPHeaders: {},
  });
  const page = await context.newPage();
  let exchanges = 0;
  page.on("request", (request) => {
    if (request.method() === "POST" && new URL(request.url()).pathname === "/api/auth/token") {
      exchanges += 1;
    }
  });
  await page.goto(baseURL!);

  await page.locator(".auth-submit").click();
  await expect(page.getByRole("alert")).toContainText("enter the token");
  expect(exchanges, "blank input must not reach the exchange endpoint").toBe(0);

  await page.locator(".auth-token-input").fill("AAAAAAAAAAAAAAAAAAAAAA");
  await page.locator(".auth-submit").click();
  await expect(page.getByRole("alert")).toContainText("that token was not accepted");
  await expect(page.locator(".auth-page")).toBeVisible();
  expect(exchanges).toBe(1);

  await authenticate(page, await token("show"));
  await expect(page.locator(".session-list")).toBeVisible();
  await context.close();
});

test("a mismatched build stamp beats a 401 without raising the token prompt", async ({
  browser,
  baseURL,
}) => {
  const context = await browser.newContext({
    storageState: AUTH_STORAGE_STATE_PATH,
  });
  const page = await context.newPage();
  await page.route("**/api/sessions**", async (route) => {
    await route.fulfill({
      status: 401,
      contentType: "application/json",
      headers: { "x-farhelm-build": "intentionally-mismatched-build" },
      body: '{"error":"unauthenticated","code":"device_auth_required"}',
    });
  });
  await page.goto(baseURL!);
  await expect(page.locator(".build-skew")).toBeVisible();
  await expect(page.locator(".auth-page")).toHaveCount(0);
  await context.close();
});

test("a same-build supervisor 401 does not invalidate the device", async ({
  browser,
  baseURL,
}) => {
  const context = await browser.newContext({
    storageState: AUTH_STORAGE_STATE_PATH,
  });
  const probe = await context.request.get(baseURL!);
  const build = probe.headers()["x-farhelm-build"];
  if (!build) throw new Error("the helm readiness response has no build stamp");
  const page = await context.newPage();
  await page.route("**/api/sessions**", async (route) => {
    await route.fulfill({
      status: 401,
      contentType: "text/plain",
      headers: { "x-farhelm-build": build },
      body: "this spawn identity is not authorized",
    });
  });
  const refused = page.waitForResponse(
    (response) => new URL(response.url()).pathname === "/api/sessions" && response.status() === 401,
  );
  await page.goto(baseURL!);
  await refused;
  await expect(page.locator(".auth-page")).toHaveCount(0);
  await expect(page.locator(".build-skew")).toHaveCount(0);
  await context.close();
});

test("rotation logs out an open client and drops its feed and terminal sockets", async ({
  page,
  baseURL,
  extraHTTPHeaders,
}) => {
  // Install before navigation so both sockets are born through the tracker.
  // The native class still does every protocol operation; this records only
  // route and close, which are the two facts the assertion needs.
  await page.addInitScript(() => {
    const NativeWebSocket = window.WebSocket;
    (window as any).__authSockets = { opened: [] as string[], closed: [] as string[] };
    class TrackingWebSocket extends NativeWebSocket {
      constructor(url: string | URL, protocols?: string | string[]) {
        super(url, protocols ?? []);
        const route = new URL(String(url)).pathname;
        (window as any).__authSockets.opened.push(route);
        this.addEventListener("close", () => {
          (window as any).__authSockets.closed.push(route);
        });
      }
    }
    window.WebSocket = TrackingWebSocket;
  });

  await page.goto(baseURL!);
  await expect(page.locator(".session-list")).toBeVisible();
  const sessionRow = page.locator("[data-session-id]").first();
  const sessionId = await sessionRow.getAttribute("data-session-id");
  if (!sessionId) throw new Error("the startup session row has no session id");
  await sessionRow.locator(".session-row-open").click();
  await expect
    .poll(() =>
      page.evaluate(() => ({
        terminal: (window as any).__farhelmWs?.readyState,
        opened: (window as any).__authSockets.opened as string[],
      })),
    )
    .toMatchObject({ terminal: 1 });
  await expect
    .poll(() =>
      page.evaluate(() => (window as any).__authSockets.opened as string[]),
    )
    .toEqual(expect.arrayContaining(["/api/events"]));
  await expect
    .poll(() =>
      page.evaluate(() => (window as any).__authSockets.opened as string[]),
    )
    .toEqual(expect.arrayContaining([expect.stringMatching(/\/api\/sessions\/.*\/term$/)]));

  // These sockets are not owned by the application component tree. Their
  // close therefore proves each server route observed revocation itself;
  // a feed failure unmounting the UI cannot close them as a side effect.
  // The context header is cleared first so the subprotocol below is the
  // only credential on their upgrade requests in either browser engine.
  await requireProductPageAuth(page.context());
  await page.evaluate(async ({ id, secretKey }) => {
    const secret = localStorage.getItem(secretKey);
    if (!secret) throw new Error("the authenticated page has no device secret");
    const protocols = ["farhelm", `farhelm-device-${secret}`];
    const open = (route: string) =>
      new Promise<WebSocket>((resolve, reject) => {
        const socket = new WebSocket(
          `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}${route}`,
          protocols,
        );
        socket.addEventListener("open", () => resolve(socket), { once: true });
        socket.addEventListener("error", () => reject(new Error(`failed to open ${route}`)), {
          once: true,
        });
      });
    const [feed, terminal] = await Promise.all([
      open("/api/events"),
      open(`/api/sessions/${id}/term?cols=80&rows=24`),
    ]);
    (window as any).__rawAuthSockets = { feed, terminal };
  }, { id: sessionId, secretKey: DEVICE_SECRET_KEY });

  const replacement = await token("rotate");
  await expect
    .poll(() =>
      page.evaluate(() => ({
        feed: (window as any).__rawAuthSockets.feed.readyState,
        terminal: (window as any).__rawAuthSockets.terminal.readyState,
      })),
    )
    .toEqual({ feed: 3, terminal: 3 });
  const refused = await page.evaluate(async () => (await fetch("/api/sessions")).status);
  expect(refused).toBe(401);
  await expect
    .poll(() =>
      page.evaluate(() => (window as any).__authSockets.closed as string[]),
    )
    .toEqual(expect.arrayContaining(["/api/events"]));
  await expect
    .poll(() =>
      page.evaluate(() => (window as any).__authSockets.closed as string[]),
    )
    .toEqual(expect.arrayContaining([expect.stringMatching(/\/api\/sessions\/.*\/term$/)]));
  await expect(page.locator(".auth-page")).toBeVisible({ timeout: 20_000 });

  // Authentication removes the prompt but does not throw away the
  // selection. Recovery therefore owes this open session a fresh detail
  // read and a new terminal attachment — not a silent switch to whatever
  // auto-select would have picked on a cold load.
  const detailRead = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return url.pathname === `/api/sessions/${sessionId}` && response.status() === 200;
  });
  let replacementSecret: string | undefined;
  const advanceContextHeader = async (route: Route) => {
    const response = await route.fetch();
    const body = (await response.json()) as { device_secret?: unknown };
    if (typeof body.device_secret !== "string" || body.device_secret === "") {
      throw new Error("the rotation exchange returned no replacement device secret");
    }
    replacementSecret = body.device_secret;

    // The context-level header authenticates non-page clients, but its old
    // value would otherwise outrank the page's freshly stored credential on
    // the recovery read. Advance it before the exchange response reaches the
    // application, so the product sees one coherent credential transition.
    await page.context().setExtraHTTPHeaders({
      Authorization: `Bearer ${replacementSecret}`,
    });
    refreshHarnessAuthorization(extraHTTPHeaders, replacementSecret);
    await route.fulfill({ response });
  };
  await page.route("**/api/auth/token", advanceContextHeader);
  await authenticate(page, replacement);
  await page.unroute("**/api/auth/token", advanceContextHeader);
  await detailRead;
  // The EXACT session, not whichever one auto-select would pick: the
  // titlebar shows its title and the live terminal socket carries its id.
  const expectedTitle = await page
    .locator(`[data-session-id="${sessionId}"] .session-title`)
    .textContent();
  await expect(page.locator(".titlebar .title")).toHaveText(expectedTitle ?? "", {
    timeout: 20_000,
  });
  await expect
    .poll(() => page.evaluate(() => (window as any).__farhelmWs?.readyState))
    .toBe(1);
  expect(
    await page.evaluate(() => (window as any).__farhelmWs?.url ?? ""),
    "recovery must reattach the session that was open, not a fallback",
  ).toContain(sessionId);
  const storedSecret = await page.evaluate(
    (key) => localStorage.getItem(key),
    DEVICE_SECRET_KEY,
  );
  expect(storedSecret, "re-authentication must persist the replacement device secret").toBe(
    replacementSecret,
  );
  // Leave the shared suite authenticated under the replacement. Playwright
  // reloads this file for later contexts and workers, while the header refresh
  // above advances request fixtures that remain in this worker.
  const state = await page.context().storageState();
  await writeFile(
    AUTH_STORAGE_STATE_PATH,
    JSON.stringify(state),
    { mode: 0o600 },
  );
  await chmod(AUTH_STORAGE_STATE_PATH, 0o600);
});
