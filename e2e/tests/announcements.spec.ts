/**
 * Announcements use route-local pending/history state. Every spec shares a
 * real helm, so writing its durable history here would change later Help
 * menus permanently. Rust tests cover persistence; these cases cover what
 * the client reads, displays and records across its modal lifecycle.
 */
import { Page, APIRequestContext } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { DEVICE_SECRET_KEY } from "./helpers/device-auth";
import { requireHelmBuild } from "./helpers/helm-build";
import { createSession, cleanupSession } from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { attachSession, terminalInput, programInput, recordInput, type InputFrame } from "./helpers/term";
import { waitForReplayReveal } from "./helpers/terminal-readiness";
import { expectKeyboardScroll } from "./helpers/keyboard-scroll";

type Announcement = {
  id: string;
  title: string;
  body: string;
  detail: string | null;
  link: { label: string; url: string } | null;
};

const DETAILS_URL = "https://announcements.example.test/details";
const OLDER: Announcement = {
  id: "older-information", title: "An earlier notice", body: "An earlier message.", detail: null, link: null,
};
const NEWER: Announcement = {
  id: "newer-information", title: "A newer notice", body: "A plain <strong>message</strong>.\n\nA second paragraph.",
  detail: "More context", link: { label: "More information", url: DETAILS_URL },
};

/** Mutable fake state survives page reload, while never reaching the helm. */
type Fixture = {
  pending: Announcement[];
  history: Announcement[];
  reads: number;
  writes: string[][];
  failWrite: boolean;
  preferenceReads: number;
  preferences: Record<string, unknown>;
  refuseFirstPreference: boolean;
  failPreferences: boolean;
  preferenceGate?: ReturnType<typeof routeGate>;
  queueGate?: ReturnType<typeof routeGate>;
  stamp: string;
};

/**
 * Use the live stack's build identity and preference shape, then contain all
 * preference and announcement writes in this page. A missing announcement
 * route fake would mutate history shared by unrelated specs.
 */
async function routeAnnouncements(page: Page, request: APIRequestContext): Promise<Fixture> {
  const seed = await request.get("/api/preferences");
  await expect(seed).toBeOK();
  const fixture: Fixture = {
    pending: [OLDER, NEWER], history: [], reads: 0, writes: [], failWrite: false,
    preferenceReads: 0, preferences: { ...await seed.json(), welcome_seen: true },
    refuseFirstPreference: false, failPreferences: false,
    stamp: requireHelmBuild(seed, "announcement route fixtures"),
  };
  const headers = { "x-farhelm-build": fixture.stamp };
  await page.route("**/api/preferences", async route => {
    if (route.request().method() === "GET") {
      fixture.preferenceReads++;
      if (fixture.refuseFirstPreference && fixture.preferenceReads === 1) {
        return route.fulfill({ status: 401, headers, json: { code: "device_auth_required", error: "sign in" } });
      }
      if (fixture.preferenceGate) await fixture.preferenceGate.wait();
      if (fixture.failPreferences) return route.fulfill({ status: 500, headers, body: "preferences unavailable" });
    } else {
      Object.assign(fixture.preferences, route.request().postDataJSON());
    }
    await route.fulfill({ status: 200, headers, json: fixture.preferences });
  });
  await page.route("**/api/announcements", async route => {
    expect(route.request().method(), "queue reads use GET").toBe("GET");
    fixture.reads++;
    if (fixture.queueGate) await fixture.queueGate.wait();
    await route.fulfill({ status: 200, headers, json: { pending: fixture.pending, history: fixture.history } });
  });
  await page.route("**/api/announcements/shown", async route => {
    expect(route.request().method(), "shown writes use POST").toBe("POST");
    const ids: string[] = route.request().postDataJSON().ids;
    fixture.writes.push(ids);
    if (fixture.failWrite) return route.fulfill({ status: 500, headers, body: "shown write unavailable" });
    const shown = fixture.pending.filter(item => ids.includes(item.id)).reverse();
    fixture.pending = fixture.pending.filter(item => !ids.includes(item.id));
    fixture.history = [...shown, ...fixture.history];
    await route.fulfill({ status: 204, headers });
  });
  // Welcome's feedback handoff must only open an editor, never send anything.
  await page.route("**/api/feedback", route => route.fulfill({ status: 500, headers, body: "blocked by announcement test" }));
  return fixture;
}

/** Observe the mounted dialog and its initial handoff before keyboard input. */
async function announcementDialog(page: Page) {
  const dialog = page.getByRole("dialog", { name: "Announcements", exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".announcements-got-it")).toBeFocused();
  return dialog;
}

/** Open the real Help menu so visibility checks include the rendered order. */
async function helpMenu(page: Page) {
  const toggle = page.getByRole("button", { name: "Help", exact: true });
  await toggle.click();
  const menu = page.getByRole("menu", { name: "Help", exact: true });
  await expect(menu).toBeVisible();
  return { toggle, menu };
}

for (const dismissal of ["Got it", "close", "Escape"] as const) {
  /** Each dismissal drains exactly the displayed ids; reload reads the
   * shared answer again rather than remembering a client-local dismissal.
   */
  test(`announcement dismissal by ${dismissal} survives reload`, async ({ page, request }) => {
    const fixture = await routeAnnouncements(page, request);
    await page.goto("/");
    const dialog = await announcementDialog(page);
    await expect(dialog.locator(".announcement h3")).toHaveText([NEWER.title, OLDER.title]);
    await waitForReplayReveal(page, "terminal");
    await expect(dialog.locator(".announcements-got-it")).toBeFocused();
    if (dismissal === "Escape") await page.keyboard.press("Escape");
    else if (dismissal === "close") await dialog.getByRole("button", { name: "Close announcements", exact: true }).click();
    else await dialog.getByRole("button", { name: "Got it", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(terminalInput(page)).toBeFocused();
    await expect.poll(() => fixture.writes).toEqual([[NEWER.id, OLDER.id]]);
    expect(fixture.pending).toEqual([]);
    expect(fixture.reads, "one queue check on this page load").toBe(1);
    const firstMenu = await helpMenu(page);
    await expect(firstMenu.menu.locator('[data-bar-menu-item="Announcements"]')).toBeVisible();
    await page.reload();
    await expect(page.locator(".new-session-button")).toBeVisible();
    await expect.poll(() => fixture.reads).toBe(2);
    await expect(dialog).toHaveCount(0);
    const { menu } = await helpMenu(page);
    await expect(menu.locator('[data-bar-menu-item="Announcements"]')).toBeVisible();
    expect(fixture.writes).toHaveLength(1);
  });
}

/** Initial welcome owns the first modal slot, including its feedback handoff.
 * Queue reads begin only once that introduction has yielded the slot.
 */
for (const feedback of [false, true]) {
  test(`announcements wait for welcome${feedback ? " and its feedback handoff" : ""}`, async ({ page, request }) => {
    const fixture = await routeAnnouncements(page, request);
    fixture.preferences.welcome_seen = false;
    await page.goto("/");
    const welcome = page.getByRole("dialog", { name: "Welcome to Farhelm", exact: true });
    await expect(welcome.locator(".welcome-got-it")).toBeFocused();
    expect(fixture.reads, "welcome owns the initial modal decision").toBe(0);
    await expect(page.getByRole("dialog")).toHaveCount(1);
    if (feedback) {
      await welcome.getByRole("button", { name: "Send feedback now", exact: true }).click();
      const editor = page.getByRole("dialog", { name: "Send feedback", exact: true });
      await expect(editor.locator(".feedback-message")).toBeFocused();
      expect(fixture.reads, "feedback still owns the handoff").toBe(0);
      await page.keyboard.press("Escape");
      await expect(editor).toHaveCount(0);
    } else await welcome.getByRole("button", { name: "Got it", exact: true }).click();
    await expect(welcome).toHaveCount(0);
    await announcementDialog(page);
    await expect(page.getByRole("dialog")).toHaveCount(1);
    expect(fixture.reads).toBe(1);
  });
}

/** A user can open Welcome while a slow queue read is in flight. Observe the
 * ready batch's admission wrapper before closing Welcome, so absence of a
 * second dialog cannot pass merely because the response has not rendered yet.
 */
test("a late queue result waits behind a reopened welcome", async ({ page, request }) => {
  const fixture = await routeAnnouncements(page, request);
  fixture.queueGate = routeGate();
  try {
    await page.goto("/");
    await expect.poll(() => fixture.reads).toBe(1);
    const { menu } = await helpMenu(page);
    await menu.locator('[data-bar-menu-item="Welcome"]').click();
    const welcome = page.getByRole("dialog", { name: "Welcome to Farhelm", exact: true });
    await expect(welcome.locator(".welcome-got-it")).toBeFocused();
    fixture.queueGate.release();
    await expect(page.locator('[id^="host-modal-"]')).toHaveCount(1);
    await expect(page.getByRole("dialog")).toHaveCount(1);
    await expect(page.getByRole("dialog", { name: "Announcements", exact: true })).toHaveCount(0);
    await welcome.locator(".welcome-got-it").click();
    await expect(welcome).toHaveCount(0);
    await announcementDialog(page);
    expect(fixture.reads, "closing a reopened welcome does not check twice").toBe(1);
  } finally {
    fixture.queueGate.release();
  }
});

/** A successful preference read is required for Welcome alone. Its decision
 * still finishes after a failed read, and announcements have their own read.
 */
test("announcements wait for the preference decision and survive a failed preference read", async ({ page, request }) => {
  const fixture = await routeAnnouncements(page, request);
  fixture.failPreferences = true;
  const gate = routeGate();
  fixture.preferenceGate = gate;
  try {
    await page.goto("/");
    await expect.poll(() => fixture.preferenceReads).toBeGreaterThan(0);
    expect(fixture.reads, "initial preferences have not decided yet").toBe(0);
    await expect(page.getByRole("dialog")).toHaveCount(0);
    gate.release();
    await announcementDialog(page);
    await expect(page.getByRole("dialog", { name: "Welcome to Farhelm", exact: true })).toHaveCount(0);
    expect(fixture.reads).toBe(1);
  } finally {
    gate.release();
  }
});

/** An auth refusal unmounts the gate. No queue request is admitted until the
 * signed-in page has completed its own preference decision.
 */
test("announcements wait for sign-in", async ({ page, request }) => {
  const fixture = await routeAnnouncements(page, request);
  fixture.refuseFirstPreference = true;
  const secret = await page.context().storageState().then(state => state.origins
    .flatMap(origin => origin.localStorage).find(item => item.name === DEVICE_SECRET_KEY)?.value);
  expect(secret, "premise: harness authentication has a device identity").toBeTruthy();
  await page.route("**/api/auth/token", route => route.fulfill({ status: 200,
    headers: { "x-farhelm-build": fixture.stamp }, json: { device_secret: secret } }));
  await page.goto("/");
  await expect(page.locator(".auth-page")).toBeVisible();
  expect(fixture.reads, "signed-out pages cannot read announcements").toBe(0);
  await expect(page.getByRole("dialog", { name: "Announcements", exact: true })).toHaveCount(0);
  await page.locator(".auth-token-input").fill("AAAAAAAAAAAAAAAAAAAAAA");
  await page.locator(".auth-submit").click();
  await announcementDialog(page);
  expect(fixture.reads).toBe(1);
});

/** Best-effort persistence cannot trap the user or manufacture shown history.
 * The same pending batch is offered again on the next page load.
 */
test("a failed shown write dismisses and leaves history hidden", async ({ page, request }) => {
  const fixture = await routeAnnouncements(page, request);
  fixture.failWrite = true;
  await page.goto("/");
  const dialog = await announcementDialog(page);
  await dialog.locator(".announcements-got-it").click();
  await expect(dialog).toHaveCount(0);
  await expect.poll(() => fixture.writes.length).toBe(1);
  const { menu } = await helpMenu(page);
  await expect(menu.locator('[data-bar-menu-item="Announcements"]')).toHaveCount(0);
  await expect(page.locator(".build-skew")).toHaveCount(0);
  expect(fixture.pending).toEqual([OLDER, NEWER]);
  await page.reload();
  await announcementDialog(page);
  expect(fixture.reads).toBe(2);
});

/** History follows the helm's shown order, independent of content or ids.
 * Its extra Help row still fits a short window. Reopening uses the same
 * dialog but never posts another shown record.
 */
test("Help exposes shown history after release notes and reopening writes nothing", async ({ page, request }) => {
  const fixture = await routeAnnouncements(page, request);
  fixture.pending = [];
  fixture.history = [OLDER, NEWER];
  await page.goto("/");
  await expect.poll(() => fixture.reads).toBe(1);
  await page.setViewportSize({ width: 900, height: 240 });
  const { toggle, menu } = await helpMenu(page);
  const flyout = page.locator('.bar-menu-flyout[data-bar-menu="help"]');
  await expect(flyout).toHaveCSS("opacity", "1");
  const bounds = (await flyout.boundingBox())!;
  expect(bounds.y).toBeGreaterThanOrEqual(7);
  expect(bounds.y + bounds.height).toBeLessThanOrEqual(240);
  expect(await menu.locator("[data-bar-menu-item]").evaluateAll(items => items.map(item => item.getAttribute("data-bar-menu-item"))))
    .toEqual(["Send feedback", "Chat on Discord", "Documentation", "Keyboard shortcuts", "Release notes", "Announcements", "SSH needs 2FA?", "Welcome"]);
  const item = menu.locator('[data-bar-menu-item="Announcements"]');
  await expect(item).toContainText("Shows previously shown announcements again");
  await menu.locator('[data-bar-menu-item="Release notes"]').focus();
  await page.keyboard.press("ArrowDown");
  await expect(item).toBeFocused();
  await page.keyboard.press("Enter");
  const dialog = await announcementDialog(page);
  await expect(dialog.locator(".announcement h3")).toHaveText([OLDER.title, NEWER.title]);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(toggle).toBeFocused();
  expect(fixture.writes).toEqual([]);
  expect(fixture.reads).toBe(1);
});

/** Empty pending/history has no visible footprint, including in Help. */
test("an empty queue leaves the history item hidden", async ({ page, request }) => {
  const fixture = await routeAnnouncements(page, request);
  fixture.pending = [];
  await page.goto("/");
  await expect.poll(() => fixture.reads).toBe(1);
  const { menu } = await helpMenu(page);
  await expect(menu.locator('[data-bar-menu-item="Announcements"]')).toHaveCount(0);
  await expect(page.getByRole("dialog", { name: "Announcements", exact: true })).toHaveCount(0);
  expect(fixture.writes).toEqual([]);
});

/** Pinned closing buttons cannot supply keyboard access to a sibling body.
 * A linkless batch must admit Tab and native scrolling in WebKit too.
 */
test("long announcements without links are keyboard-scrollable", async ({ page, request }) => {
  const fixture = await routeAnnouncements(page, request);
  fixture.pending = [{ ...OLDER, body: "A paragraph that needs keyboard access.\n\n".repeat(40) }];
  await page.setViewportSize({ width: 380, height: 320 });
  await page.goto("/");
  const dialog = await announcementDialog(page);
  const body = dialog.locator(".announcements-body");
  await expect(body.getByRole("link")).toHaveCount(0);
  await expectKeyboardScroll(page, dialog.locator(".announcements-dismiss"), body);
  await dialog.locator(".announcements-got-it").click();
  await expect(dialog).toHaveCount(0);
});

/** The framed dialog escapes markup, keeps links open, isolates shortcuts and
 * pins its closing controls while a long batch scrolls in a short window.
 * A post-close key is a positive control for the terminal input recorder.
 */
test("announcement content, modal isolation and short-window controls", async ({ page, request, context }, testInfo) => {
  const fixture = await routeAnnouncements(page, request);
  await context.route(DETAILS_URL, route => route.fulfill({ contentType: "text/html", body: "announcement details fixture" }));
  const session = await createSession(request, { title: `announcement-input-${Date.now()}` });
  fixture.preferences.last_selected = session.id;
  fixture.queueGate = routeGate();
  fixture.pending = [OLDER, { ...NEWER, body: `${NEWER.body}\n\n${"Long announcement paragraph. ".repeat(80)}` }];
  const frames: InputFrame[] = [];
  page.on("websocket", socket => {
    if (new URL(socket.url()).pathname.endsWith("/term")) recordInput(socket, frames);
  });
  try {
    await page.goto("/");
    await expect.poll(() => fixture.reads).toBe(1);
    await attachSession(page, session.id);
    await waitForReplayReveal(page, "terminal");
    await expect(terminalInput(page)).toBeFocused();
    fixture.queueGate.release();
    const dialog = await announcementDialog(page);
    await expect(dialog.locator(".announcement").first().locator(".announcement-paragraph")).toHaveCount(3);
    await expect(dialog.locator(".announcement-paragraph").first()).toHaveText("A plain <strong>message</strong>.");
    await expect(dialog.locator(".announcement-paragraph").nth(1)).toHaveText("A second paragraph.");
    await expect(dialog.locator(".announcement-paragraph strong")).toHaveCount(0);
    await expect(dialog.locator(".announcement-detail")).toHaveText("More context");
    const link = dialog.getByRole("link", { name: "More information ↗", exact: true });
    const external = context.waitForEvent("page");
    await link.click();
    const details = await external;
    try {
      await details.waitForURL(DETAILS_URL);
      await expect(dialog).toBeVisible();
    } finally {
      await details.close();
    }
    await dialog.locator(".announcements-got-it").focus();
    await expect(dialog.locator(".announcements-got-it")).toBeFocused();
    frames.length = 0;
    for (const chord of ["Control+Shift+P", "Control+Shift+1", "Control+Shift+Slash"]) await page.keyboard.press(chord);
    await expect(dialog.locator(".announcements-got-it")).toBeFocused();
    await expect(page.getByRole("dialog")).toHaveCount(1);
    await page.keyboard.press("Tab");
    await expect(dialog.locator(".announcements-dismiss")).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog.locator(".announcements-got-it")).toBeFocused();
    await page.setViewportSize({ width: 380, height: 320 });
    const body = dialog.locator(".announcements-body");
    await expect.poll(() => body.evaluate(node => node.scrollHeight > node.clientHeight)).toBe(true);
    for (const selector of [".announcements-dismiss", ".announcements-got-it"]) {
      expect(await dialog.locator(selector).evaluate(node => {
        const box = node.getBoundingClientRect();
        return node.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
      }), `${selector} stays reachable`).toBe(true);
    }
    await dialog.screenshot({ path: testInfo.outputPath("announcements-short.png") });
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(terminalInput(page)).toBeFocused();
    await page.keyboard.press("a");
    await expect.poll(() => programInput(frames), { message: "only the post-close key reaches the program" }).toBe("a");
  } finally {
    fixture.queueGate.release();
    await cleanupSession(request, session.id);
  }
});
