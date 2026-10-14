/** Quick launch must use New's ordinary authority without painting its fields
 * on success. Real input and API replies distinguish an invisible launch from
 * a shortcut that also types into the old terminal or quietly retries a refusal. */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Page, Route } from "@playwright/test";
import {
  cleanupSession,
  createSession,
  FAKE_AGENT,
  listHosts,
  openHostsPanel,
  openRowMenu,
  patchPreferences,
  pinAutoSelect,
  readPreferences,
  resetPreferences,
  setLocalYoloWithoutAsking,
  stubFeed,
} from "./helpers/fleet";
import { attachSession, waitForTermText } from "./helpers/term";
import { routeGate } from "./helpers/route-gate";
import { requireHelmBuild } from "./helpers/helm-build";

/** Fabricated replies retain the owned helm's real build identity. The
 * terminal-family helper depends on that family's reset hooks, which these
 * independent specs do not install; an empty stamp would instead test skew. */
async function fulfillQuickFixture(
  route: Route,
  request: APIRequestContext,
  status: number,
  body: string,
  contentType = "text/plain",
) {
  const probe = await request.get("/api/preferences");
  expect(probe.ok(), await probe.text()).toBe(true);
  const stamp = requireHelmBuild(probe, "quick-launch reply fixture");
  await route.fulfill({ status, contentType, body, headers: { "x-farhelm-build": stamp } });
}

/** The shortcut and terminal must both classify the platform before mounting. */
async function platform(page: Page, mac: boolean) {
  await page.addInitScript((value) => {
    Object.defineProperty(Navigator.prototype, "platform", { get: () => value });
  }, mac ? "MacIntel" : "Linux x86_64");
}

/** Seed an actual command template; the fixture agent has no outbound tools. */
async function commandTemplate(request: APIRequestContext, name: string, yolo = false) {
  const response = await request.put(`/api/templates/${encodeURIComponent(name)}`, {
    data: { kind: "command", command: FAKE_AGENT, yolo, name: `${name}-session` },
  });
  expect(response.ok(), await response.text()).toBe(true);
}

/** A real key chord, including repeat, must pass through the capture listener. */
async function chord(page: Page, mac = false, repeat = false) {
  await expect(page.locator(".quick-launch-trigger")).toHaveCount(1);
  await page.keyboard.down(mac ? "Meta" : "Control");
  if (!mac) await page.keyboard.down("Shift");
  try {
    await page.keyboard.down("p");
    if (repeat) await page.keyboard.down("p");
  } finally {
    await page.keyboard.up("p");
    if (!mac) await page.keyboard.up("Shift");
    await page.keyboard.up(mac ? "Meta" : "Control");
  }
}

/** Number shortcuts always use the physical digit's web chord, even on a Mac.
 * Waiting for all triggers establishes that the Rust list has admitted input.
 */
async function numberChord(page: Page, key = "1", repeat = false) {
  await expect(page.locator(".template-number-trigger")).toHaveCount(10);
  await page.keyboard.down("Control");
  await page.keyboard.down("Shift");
  try {
    await page.keyboard.down(`Digit${key}`);
    if (repeat) await page.keyboard.down(`Digit${key}`);
  } finally {
    await page.keyboard.up(`Digit${key}`);
    await page.keyboard.up("Shift");
    await page.keyboard.up("Control");
  }
}

test.beforeEach(async ({ request }) => {
  await resetPreferences(request);
});
test.afterEach(async ({ request }) => {
  await resetPreferences(request);
});

/** A completed preparation read belongs to its opening, even if the user
 * opens and cancels New while the read is held. The diagnostic generation
 * proves that the old continuation finished; absence alone is not readiness. */
for (const takeover of ["New", "settings", "Rename"]) {
  test(
    takeover !== "New"
      ? `closing ${takeover} retires an older quick-launch read`
      : "cancelling a newer New draft retires an older quick-launch read",
    async ({ page, request }) => {
      const name = `quick-read-cancel-${Date.now()}`;
      const source = takeover === "Rename" ? await createSession(request, { title: `${name}-source` }) : null;
      const gate = routeGate();
      let reads = 0;
      let held = 0;
      let creates = 0;
      await page.route("**/api/templates", async (route) => {
        if (route.request().method() !== "GET" || ++reads !== 1) return route.continue();
        held++;
        await gate.wait();
        await route.fulfill({ response: await route.fetch() });
      });
      await page.route("**/api/sessions", async (route) => {
        if (route.request().method() === "POST") creates++;
        await route.continue();
      });
      try {
        await commandTemplate(request, name);
        await patchPreferences(request, { quick_launch_template: name });
        await page.goto("/");
        await chord(page);
        await expect.poll(() => held).toBe(1);
        const trigger = page.locator(".quick-launch-trigger");
        const opening = await trigger.getAttribute("data-opening-generation");
        expect(opening).toBeTruthy();
        const form = page.locator(".create-session-form");
        if (takeover === "settings") {
          await page.getByRole("button", { name: "settings", exact: true }).click();
          const dialog = page.getByRole("dialog", { name: "settings", exact: true });
          await expect(dialog).toBeVisible();
          await dialog.getByRole("button", { name: "close settings", exact: true }).click();
          await expect(dialog).toHaveCount(0);
        } else if (source) {
          const row = page.locator(`[data-session-id="${source.id}"]`);
          await expect(row).toBeVisible();
          await openRowMenu(row);
          await row.locator(".session-row-rename").click();
          const dialog = page.locator(".rename-dialog");
          await expect(dialog.locator(".rename-input")).toBeFocused();
          await dialog.locator(".rename-cancel").click();
          await expect(dialog).toHaveCount(0);
        } else {
          await page.locator(".new-session-button").click();
          await expect(form).toBeVisible();
          await form.getByRole("button", { name: "cancel", exact: true }).click();
        }
        await expect(form).toHaveCount(0);
        gate.release();
        await expect(trigger).toHaveAttribute("data-read-completed-generation", opening!);
        await expect(form).toHaveCount(0);
        expect(creates).toBe(0);
      } finally {
        gate.release();
        await page.unroute("**/api/templates");
        await page.unroute("**/api/sessions");
        if (source) await cleanupSession(request, source.id);
        await request.delete(`/api/templates/${name}`);
      }
    },
  );
}

/** Background keyboard input cannot activate Replace with while New is
 * hidden. A callback already admitted before isolation may still arrive, so
 * explicitly deliver that callback and require ordinary replacement consent. */
test("hidden preparation isolates keys and a queued Replace with requires Launch", async ({ page, request }) => {
  const name = `quick-replace-fence-${Date.now()}`;
  const source = await createSession(request, { title: `${name}-source` });
  const gate = routeGate();
  let catalogReads = 0;
  const bodies: any[] = [];
  await page.route("**/api/launch-catalog", async (route) => {
    catalogReads++;
    await gate.wait();
    await route.fulfill({ response: await route.fetch() });
  });
  await page.route("**/api/sessions**", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    bodies.push(route.request().postDataJSON());
    expect(new URL(route.request().url()).pathname).toBe(`/api/sessions/${source.id}/replace`);
    await fulfillQuickFixture(route, request, 409, "replacement fixture refusal");
  });
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const replace = page.locator(".header-replace-with");
    await expect(replace).toBeVisible();
    await expect(page.locator("#terminal .xterm-helper-textarea")).toBeFocused();
    await chord(page);
    await expect.poll(() => catalogReads).toBeGreaterThan(0);
    const form = page.locator(".create-session-form");
    await expect(form).toBeHidden();
    await expect.poll(() => replace.evaluate((node) => !!node.closest("[inert]")), {
      message: "the hidden form owns background keyboard isolation",
    }).toBe(true);
    await page.keyboard.press("Tab");
    await page.keyboard.press("Enter");
    expect(bodies).toHaveLength(0);
    await expect(form).toBeHidden();
    // Programmatic delivery models a callback queued before the modal became
    // inert; it bypasses pointer hit-testing without bypassing the real handler.
    await replace.evaluate((node: HTMLButtonElement) => node.click());
    await expect(form).toBeVisible();
    gate.release();
    await expect(form.getByLabel("name (optional)")).toHaveValue(`${name}-source`);
    await expect(form.locator(".create-session-submit")).toContainText("replace");
    await expect(form.locator(".create-session-submit")).toBeEnabled();
    expect(bodies).toHaveLength(0);
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".create-session-error")).toContainText("replacement fixture refusal");
    expect(bodies).toHaveLength(1);
  } finally {
    gate.release();
    await page.unroute("**/api/launch-catalog");
    await page.unroute("**/api/sessions**");
    await cleanupSession(request, source.id);
    await request.delete(`/api/templates/${name}`);
  }
});

/** Failed readiness is a settled result, not a pending launch. Recovery
 * applies the saved template to the revealed draft but cannot revive automatic
 * submission; a controlled feed names the host-read recovery boundary. */
for (const recover of [false, true]) {
  test(
    recover
      ? "an initial host-read failure reveals New and recovery stays manual"
      : "a host-read fallback permits keyboard cancellation before recovery",
    async ({ page, request }) => {
      const name = `quick-host-read-${Date.now()}`;
      const feed = await stubFeed(page);
      let failing = true;
      let creates = 0;
      await page.route("**/api/hosts", async (route) => {
        // Fail this reader's decoding without taking down the helm transport or
        // its feed, which independently supplies the recovery notification.
        if (failing) {
          await fulfillQuickFixture(
            route,
            request,
            200,
            JSON.stringify({ hosts: "quick host-list fixture failure" }),
            "application/json",
          );
        } else await route.continue();
      });
      await page.route("**/api/sessions", async (route) => {
        if (route.request().method() === "POST") creates++;
        await route.continue();
      });
      try {
        await commandTemplate(request, name);
        await patchPreferences(request, { quick_launch_template: name });
        await page.goto("/");
        await feed.waitForConnection(1);
        await openHostsPanel(page);
        await expect(page.locator(".hosts-load-error")).toContainText("error decoding response body");
        await chord(page);
        const form = page.locator(".create-session-form");
        await expect(form).toBeVisible();
        await expect(form.locator(".create-session-error")).toContainText("the host list failed");
        await expect(form.getByRole("button", { name: "cancel", exact: true })).toBeEnabled();
        await expect(form.getByRole("button", { name: "cancel", exact: true })).toBeFocused();
        expect(creates).toBe(0);
        if (!recover) {
          await page.keyboard.press("Tab");
          await expect.poll(() => form.evaluate((node) => node.contains(document.activeElement))).toBe(true);
          await page.keyboard.press("Enter");
          await expect(form).toHaveCount(0);
          expect(creates).toBe(0);
          return;
        }
        failing = false;
        await expect.poll(() => feed.openSockets(), { message: "the owned feed is live before recovery notification" })
          .toBeGreaterThan(0);
        feed.notify(2);
        await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
        await expect(form.locator(".create-session-submit")).toBeEnabled();
        await expect(form).toBeVisible();
        expect(creates).toBe(0);
        await form.getByRole("button", { name: "cancel", exact: true }).click();
      } finally {
        await page.unroute("**/api/hosts");
        await page.unroute("**/api/sessions");
        await request.delete(`/api/templates/${name}`);
      }
    },
  );
}

/** Fresh shared reads may replace acknowledged state, but they cannot undo
 * this window's explicit dirty choice. Hold the PUT before it reaches the helm
 * (or refuse it), then close Templates and observe the actual create payload. */
for (const failed of [false, true]) {
  test(`quick launch honors a ${failed ? "failed" : "held"} local mark over the shared read`, async ({ page, request }) => {
    const prefix = `quick-local-mark-${Date.now()}`;
    const [old, chosen] = ["old", "chosen"].map((suffix) => `${prefix}-${suffix}`);
    const gate = routeGate();
    let held = 0;
    let settled = 0;
    const bodies: any[] = [];
    await page.route("**/api/preferences", async (route) => {
      if (route.request().method() !== "PUT" || route.request().postDataJSON().quick_launch_template !== chosen) {
        return route.continue();
      }
      held++;
      if (failed) {
        await fulfillQuickFixture(route, request, 503, "local mark fixture refusal");
        settled++;
      } else {
        await gate.wait();
        await route.fulfill({ response: await route.fetch() });
      }
    });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      bodies.push(route.request().postDataJSON());
      await fulfillQuickFixture(route, request, 409, "local mark launch fixture refusal");
    });
    try {
      await commandTemplate(request, old);
      await commandTemplate(request, chosen);
      await patchPreferences(request, { quick_launch_template: old });
      await page.goto("/");
      await page.locator(".templates-button").click();
      const dialog = page.getByRole("dialog", { name: "templates", exact: true });
      await expect(dialog.getByRole("button", { name: `quick launch ${old}`, exact: true })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
      const mark = dialog.getByRole("button", { name: `quick launch ${chosen}`, exact: true });
      await mark.click();
      await expect(mark).toHaveAttribute("aria-pressed", "true");
      await expect.poll(() => held).toBe(1);
      if (failed) await expect.poll(() => settled).toBe(1);
      expect((await readPreferences(request)).quick_launch_template).toBe(old);
      await dialog.locator(".templates-close").click();
      await expect(dialog).toHaveCount(0);
      await chord(page);
      const form = page.locator(".create-session-form");
      await expect(form).toBeVisible();
      await expect(form.locator(".create-session-error")).toContainText("local mark launch fixture refusal");
      expect(bodies).toHaveLength(1);
      expect(bodies[0].title).toBe(`${chosen}-session`);
      await expect(page.locator(".templates-button")).toHaveAttribute("data-tooltip", new RegExp(chosen));
    } finally {
      gate.release();
      await page.unrouteAll({ behavior: "wait" });
      for (const name of [old, chosen]) await request.delete(`/api/templates/${name}`);
    }
  });
}

/** A preparation refusal has no host ownership. Binding the ordinary New
 * defaults must keep its explanation, rather than clearing it as an error
 * from a previous target. Only the shortcut's second preference read fails. */
test("a quick preparation error survives New's first host binding", async ({ page, request }) => {
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local?.identity).toBeTruthy();
  let reads = 0;
  await page.route("**/api/preferences", async (route) => {
    if (route.request().method() !== "GET" || ++reads === 1) return route.continue();
    await fulfillQuickFixture(route, request, 503, "quick preference-read fixture failure");
  });
  try {
    await page.goto("/");
    await openHostsPanel(page);
    await expect(page.locator(`[data-host-id="${local!.id}"] .host-name`)).toBeVisible();
    await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.getByRole("combobox", { name: "host", exact: true })).toHaveAttribute(
      "data-host-id",
      String(local!.id),
    );
    await expect(form).not.toHaveAttribute("data-create-target-fingerprint", "");
    await expect(form.locator(".create-session-error")).toContainText("couldn't prepare quick launch");
    await expect(form.locator(".create-session-error")).toContainText("quick preference-read fixture failure");
    expect(reads).toBe(2);
  } finally {
    await page.unroute("**/api/preferences");
  }
});

/** The old target's refusal must not describe a newly retargeted host, but
 * the admitted attempt still ends visibly. The form's bound fingerprint is
 * the readiness oracle; only the client's host read is retargeted, preserving
 * the real owned supervisor and its sessions. */
test("a refusal after target change reveals the spent quick-launch draft", async ({ page, request }) => {
  const name = `quick-target-refusal-${Date.now()}`;
  const feed = await stubFeed(page);
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local?.identity).toBeTruthy();
  const gate = routeGate();
  let moved = false;
  let creates = 0;
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    if (moved) {
      const host = body.hosts.find((row: any) => row.id === local!.id);
      host.destination = "fixture-retarget";
      host.identity = "fixture-replaced-install";
      host.state = { ...host.state, identity: host.identity };
    }
    await route.fulfill({ response, json: body });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    creates++;
    await gate.wait();
    await fulfillQuickFixture(route, request, 409, "old-target fixture refusal");
  });
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    await page.goto("/");
    await feed.waitForConnection(1);
    await chord(page);
    await expect.poll(() => creates, { message: "the owned create refusal is held" }).toBe(1);
    const form = page.locator(".create-session-form");
    await expect(form).toBeHidden();
    const original = await form.getAttribute("data-create-target-fingerprint");
    expect(original).toBeTruthy();
    moved = true;
    feed.notify(2);
    await expect(form).not.toHaveAttribute("data-create-target-fingerprint", original!);
    await expect(form).not.toHaveAttribute("data-create-target-fingerprint", "");
    gate.release();
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-error")).toContainText("the destination changed");
    await expect(form.locator(".create-session-error")).not.toContainText("old-target fixture refusal");
    await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
    expect(creates).toBe(1);
  } finally {
    gate.release();
    await page.unroute("**/api/hosts");
    await page.unroute("**/api/sessions");
    await request.delete(`/api/templates/${name}`);
  }
});

for (const movedAway of [false, true]) {
  /** A completed other-window mark change must outrank a sign-in seed during
   * rename. These are sequential operations; concurrent changes during the
   * rename remain the ordinary shared-field last-writer-wins contract. */
  test(`rename reads a mark ${movedAway ? "moved away" : "added"} in another window`, async ({ page, request }) => {
    const prefix = `quick-rename-fresh-${Date.now()}`;
    const [old, renamed, other] = ["old", "renamed", "other"].map((s) => `${prefix}-${s}`);
    try {
      await commandTemplate(request, old);
      await commandTemplate(request, other);
      if (movedAway) await patchPreferences(request, { quick_launch_template: old });
      await page.goto("/");
      await page.locator(".templates-button").click();
      const dialog = page.getByRole("dialog", { name: "templates", exact: true });
      await expect(dialog.getByRole("button", { name: `quick launch ${old}`, exact: true })).toHaveAttribute(
        "aria-pressed",
        String(movedAway),
      );
      await dialog.getByText(old, { exact: true }).click();
      await dialog.locator(".templates-name").fill(renamed);
      await patchPreferences(request, { quick_launch_template: movedAway ? other : old });
      expect((await readPreferences(request)).quick_launch_template).toBe(movedAway ? other : old);
      await dialog.getByRole("button", { name: "save", exact: true }).click();
      await expect(dialog.getByText(renamed, { exact: true })).toBeVisible();
      await expect(dialog.getByText(old, { exact: true })).toHaveCount(0);
      expect((await readPreferences(request)).quick_launch_template).toBe(movedAway ? other : renamed);
      await expect(dialog.locator('.templates-quick-mark[aria-pressed="true"]')).toHaveCount(1);
    } finally {
      for (const name of [old, renamed, other]) await request.delete(`/api/templates/${name}`);
    }
  });
}

/** The switcher preserves its first recent selection and releases the modal
 * before direct launch. Its cached label is only advertising: a mark added
 * from another client after mount still controls the action's fresh read. */
test("the quick switcher row launches through the fresh direct path", async ({ page, request }) => {
  const name = `quick-switch-row-${Date.now()}`;
  const source = await createSession(request, { title: `${name}-source` });
  const created: string[] = [];
  const gate = routeGate();
  let posts = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts++;
    await gate.wait();
    const response = await route.fetch();
    if (response.ok()) created.push((await response.json()).id);
    await route.fulfill({ response });
  });
  try {
    await commandTemplate(request, name);
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await expect(page.locator(".quick-switcher-trigger")).toHaveCount(1);
    await page.keyboard.press("Control+Shift+k");
    const dialog = page.getByRole("dialog", { name: "quick switcher" });
    await expect(dialog.getByRole("combobox")).toBeFocused();
    const quick = dialog.locator(".quick-switcher-quick-launch");
    await expect(quick).toContainText("mark a template");
    await expect(dialog.getByRole("option").first()).toHaveAttribute("aria-selected", "true");
    await expect(quick).toHaveAttribute("aria-selected", "false");
    await patchPreferences(request, { quick_launch_template: name });
    await quick.click();
    await expect.poll(() => posts, { message: "the quick-switcher create request is held" }).toBe(1);
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".create-session-form")).toBeHidden();
    gate.release();
    await expect.poll(() => created.length).toBe(1);
    await expect(page.locator(`[data-session-id="${created[0]}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(".create-session-form")).toHaveCount(0);
  } finally {
    gate.release();
    await page.unroute("**/api/sessions");
    for (const id of created) await cleanupSession(request, id);
    await cleanupSession(request, source.id);
    expect((await request.delete(`/api/templates/${name}`)).ok()).toBe(true);
  }
});

for (const [mac, numbered] of [[false, false], [true, false], [false, true], [true, true]]) {
  /** The held create reply establishes the hidden in-flight boundary. Actual
   * binary input frames and a fresh echo prove both taking the chord away from
   * the old agent and handing keyboard ownership to the newly selected one. */
  test(`${mac ? "Mac" : "non-Mac"} ${numbered ? "number shortcut" : "quick launch"} owns input and submits once without showing New`, async ({ page, request }) => {
    await platform(page, mac);
    const source = await createSession(request, { title: `quick-source-${Date.now()}` });
    const name = `quick-direct-${Date.now()}`;
    const created: string[] = [];
    const binary: Buffer[] = [];
    page.on("websocket", (socket) => {
      if (new URL(socket.url()).pathname.endsWith("/term")) {
        socket.on("framesent", ({ payload }) => {
          if (Buffer.isBuffer(payload)) binary.push(payload);
        });
      }
    });
    const gate = routeGate();
    const bodies: any[] = [];
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      bodies.push(route.request().postDataJSON());
      await gate.wait();
      const response = await route.fetch();
      if (response.ok()) created.push((await response.json()).id);
      await route.fulfill({ response });
    });
    try {
      await commandTemplate(request, name);
      await pinAutoSelect(page, source.id);
      await page.goto("/");
      await attachSession(page, source.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await expect(page.locator("#terminal .xterm-helper-textarea")).toBeFocused();
      // A later hidden snapshot would miss a launcher flashed before submit.
      // Retain one bounded observation bit across every actual DOM transition.
      await page.evaluate(() => {
        const root = window as unknown as { quickLauncherPainted: boolean; quickPaintObserver: MutationObserver };
        root.quickLauncherPainted = false;
        root.quickPaintObserver = new MutationObserver(() => {
          if (document.querySelector(".create-session-form")?.getClientRects().length) root.quickLauncherPainted = true;
        });
        root.quickPaintObserver.observe(document.body, { childList: true, subtree: true, attributes: true });
      });
      // The page was seeded without a mark: only a fresh shortcut read can
      // discover this other client's change without a reload.
      await patchPreferences(request, numbered ? { template_shortcuts: { "1": name } } : { quick_launch_template: name });
      if (numbered) await numberChord(page, "1", true);
      else await chord(page, mac, true);
      await expect.poll(() => bodies.length, { message: "the owned create request is captured and held" }).toBe(1);
      const form = page.locator(".create-session-form");
      await expect(form).toHaveCount(1);
      await expect(form).toBeHidden();
      expect(bodies).toHaveLength(1);
      expect(bodies[0]).toMatchObject({
        cwd: source.cwd,
        title: `${name}-session`,
        command: { command: FAKE_AGENT, yolo: false },
      });
      expect(Buffer.concat(binary).toString(), "the captured chord and repeat reach no agent input frame").toBe("");
      const response = page.waitForResponse((r) =>
        r.request().method() === "POST" && new URL(r.url()).pathname === "/api/sessions"
      );
      gate.release();
      const accepted = await response;
      expect(accepted.ok(), await accepted.text()).toBe(true);
      const session = await accepted.json();
      await expect(form).toHaveCount(0);
      await expect(page.locator(`[data-session-id="${session.id}"]`)).toHaveAttribute("data-session-selected", "true");
      await waitForTermText(page, "FAKE-AGENT READY");
      await expect(page.locator("#terminal .xterm-helper-textarea")).toBeFocused();
      const probe = `quick-live-${Date.now()}`;
      await page.keyboard.type(probe);
      await page.keyboard.press("Enter");
      await waitForTermText(page, `echo:${probe}`);
      expect(bodies).toHaveLength(1);
      expect(Buffer.concat(binary).toString()).toBe(`${probe}\r`);
      expect(await page.evaluate(() => (window as unknown as { quickLauncherPainted: boolean }).quickLauncherPainted))
        .toBe(false);
    } finally {
      gate.release();
      await page.unroute("**/api/sessions");
      await page.evaluate(() =>
        (window as unknown as { quickPaintObserver?: MutationObserver }).quickPaintObserver?.disconnect()
      );
      for (const id of created) await cleanupSession(request, id);
      await cleanupSession(request, source.id);
      expect((await request.delete(`/api/templates/${encodeURIComponent(name)}`)).ok()).toBe(true);
    }
  });
}

/** The host owns YOLO admission. Quick launch reveals the same draft and
 * question with Cancel focus, and cancelling it must never dispatch a retry. */
for (const numbered of [false, true]) {
test(`${numbered ? "number shortcut" : "quick launch"} reveals the ordinary YOLO question and retains its draft`, async ({ page, request }) => {
  const name = `quick-yolo-${Date.now()}`;
  const posts: any[] = [];
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") posts.push(route.request().postDataJSON());
    await route.continue();
  });
  try {
    await setLocalYoloWithoutAsking(request, false);
    await commandTemplate(request, name, true);
    await patchPreferences(request, numbered ? { template_shortcuts: { "1": name } } : { quick_launch_template: name });
    await page.goto("/");
    if (numbered) await numberChord(page); else await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".yolo-confirmation")).toBeVisible();
    await expect(form.locator(".yolo-cancel")).toBeFocused();
    expect(posts).toHaveLength(1);
    await form.locator(".yolo-cancel").click();
    await expect(form.locator(".yolo-confirmation")).toHaveCount(0);
    await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
    await expect(form.getByLabel("name (optional)")).toHaveValue(`${name}-session`);
    expect(posts).toHaveLength(1);
  } finally {
    await page.unroute("**/api/sessions");
    expect((await request.delete(`/api/templates/${encodeURIComponent(name)}`)).ok()).toBe(true);
  }
});
}

/** A refusal spends the shortcut attempt, while a later deliberate Launch
 * reuses the mounted draft. Showing an error without that draft would lose the
 * setup; automatic retries would turn an ambiguous refusal into extra work. */
for (const numbered of [false, true]) {
test(`${numbered ? "number shortcut" : "quick launch"} reveals a refusal without automatically retrying`, async ({ page, request }) => {
  const name = `quick-refused-${Date.now()}`;
  const bodies: any[] = [];
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    bodies.push(route.request().postDataJSON());
    await fulfillQuickFixture(route, request, 409, "quick fixture refusal");
  });
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, numbered ? { template_shortcuts: { "1": name } } : { quick_launch_template: name });
    await page.goto("/");
    if (numbered) await numberChord(page); else await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-error")).toContainText("quick fixture refusal");
    await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
    expect(bodies).toHaveLength(1);
    await form.locator(".create-session-submit").click();
    await expect.poll(() => bodies.length).toBe(2);
    expect(bodies[1]).toEqual(bodies[0]);
  } finally {
    await page.unroute("**/api/sessions");
    expect((await request.delete(`/api/templates/${encodeURIComponent(name)}`)).ok()).toBe(true);
  }
});
}

/** Unbound and incomplete number bindings use the existing Templates/New
 * fallbacks, while a visible modal must keep input and launch authority.
 * The disabled primary action pins incompleteness rather than a timed absence.
 */
test("number shortcuts respect modals and explain unbound or incomplete templates", async ({ page, request }) => {
  const name = `number-partial-${Date.now()}`;
  const runnable = `${name}-ready`;
  let creates = 0;
  page.on("request", request => {
    if (request.method() === "POST" && new URL(request.url()).pathname === "/api/sessions") creates++;
  });
  try {
    expect((await request.put(`/api/templates/${name}`, { data: { effort: "high" } })).ok()).toBe(true);
    await commandTemplate(request, runnable);
    await patchPreferences(request, { template_shortcuts: { "1": runnable } });
    expect((await readPreferences(request)).template_shortcuts).toEqual({ "1": runnable });
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".templates-new")).toBeFocused();
    await dialog.evaluate(node => {
      const received: { code: string; prevented: boolean }[] = [];
      (window as unknown as { numberModalInput: typeof received }).numberModalInput = received;
      node.addEventListener("keydown", event => {
        const key = event as KeyboardEvent;
        if (key.code === "Digit1" && key.ctrlKey && key.shiftKey) received.push({ code: key.code, prevented: key.defaultPrevented });
      }, true);
    });
    await numberChord(page);
    expect(await page.evaluate(() => (window as unknown as {
      numberModalInput: { code: string; prevented: boolean }[];
    }).numberModalInput)).toEqual([{ code: "Digit1", prevented: false }]);
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".templates-number-hint")).toHaveCount(0);
    await dialog.locator(".templates-close").click();
    await expect(dialog).toHaveCount(0);
    await numberChord(page, "0");
    await expect(dialog.locator(".templates-number-hint")).toContainText("Give a template a number key");
    await dialog.locator(".templates-close").click();
    await expect(dialog).toHaveCount(0);
    await patchPreferences(request, { template_shortcuts: { "1": name } });
    await numberChord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    expect(creates).toBe(0);
  } finally {
    for (const template of [name, runnable]) await request.delete(`/api/templates/${template}`);
  }
});

/** An unavailable destination never supplies a ready create target. A hidden
 * form must reveal that precondition rather than waiting forever or sending a
 * request. The real local identity pins template selection; only its displayed
 * connection state is replaced, leaving the owned supervisor alive. */
test("quick launch reveals an offline host without dispatching a create", async ({ page, request }) => {
  const name = `quick-offline-${Date.now()}`;
  const local = (await listHosts(request)).find((host) => host.kind === "local");
  expect(local?.identity, "the connected local fixture has an install identity").toBeTruthy();
  const stored = await request.put(`/api/templates/${name}`, {
    data: { kind: "command", command: FAKE_AGENT, yolo: false, host: local!.identity, name: `${name}-session` },
  });
  expect(stored.ok(), await stored.text()).toBe(true);
  let unavailableReads = 0;
  let creates = 0;
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    const host = body.hosts.find((row: any) => row.id === local!.id);
    expect(host.identity).toBe(local!.identity);
    host.state = { phase: "unreachable-reprobing", cause: "fixture-offline", last_error: "fixture offline" };
    unavailableReads++;
    await route.fulfill({ response, json: body });
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") creates++;
    await route.continue();
  });
  try {
    await patchPreferences(request, { quick_launch_template: name });
    await page.goto("/");
    await expect.poll(() => unavailableReads, { message: "the page consumed the unavailable-host fixture" })
      .toBeGreaterThan(0);
    await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-host-note")).toContainText("the selected host is unavailable");
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    await expect(form.getByLabel("agent command", { exact: true })).toHaveValue(FAKE_AGENT);
    expect(creates).toBe(0);
  } finally {
    await page.unroute("**/api/hosts");
    await page.unroute("**/api/sessions");
    expect((await request.delete(`/api/templates/${name}`)).ok()).toBe(true);
  }
});

/** Other modals keep the chord, and fresh reads distinguish a stale cached
 * mark from a deleted template or one edited into an incomplete setup. */
test("quick launch keeps modal ownership and explains missing or incomplete templates", async ({ page, request }) => {
  const name = `quick-incomplete-${Date.now()}`;
  let creates = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() === "POST") creates++;
    await route.continue();
  });
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    await page.goto("/");
    await page.locator(".templates-button").click();
    await expect(page.getByRole("dialog", { name: "templates", exact: true })).toBeVisible();
    await chord(page);
    expect(creates).toBe(0);
    await expect(page.locator(".create-session-form")).toHaveCount(0);
    await page.getByRole("dialog", { name: "templates", exact: true }).getByRole("button", {
      name: "close",
      exact: true,
    }).click();
    // Each chord below must meet a page with no dialog left over, or a
    // dialog still rendering out could swallow it and pass for the guard.
    await expect(page.getByRole("dialog", { name: "templates", exact: true })).toHaveCount(0);
    expect((await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: { effort: "high" } })).ok()).toBe(
      true,
    );
    await chord(page);
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect(form.locator(".create-session-submit")).toBeDisabled();
    expect(creates).toBe(0);
    await form.getByRole("button", { name: "cancel", exact: true }).click();
    await expect(form).toHaveCount(0);
    expect((await request.delete(`/api/templates/${encodeURIComponent(name)}`)).ok()).toBe(true);
    await chord(page);
    await expect(page.locator(".templates-quick-hint")).toContainText("Mark a template with ⚡");
    expect(creates).toBe(0);
  } finally {
    await page.unroute("**/api/sessions");
    await request.delete(`/api/templates/${encodeURIComponent(name)}`);
  }
});

/** Marking is independent of editor fields, one mark moves between names,
 * rename repoints it, and Delete/Undo cannot resurrect the old mark. */
test("the template mark moves, survives rename, and stays cleared after undo", async ({ page, request }) => {
  const prefix = `quick-marks-${Date.now()}`;
  const [a, b, partial, renamed] = ["a", "b", "partial", "renamed"].map((s) => `${prefix}-${s}`);
  try {
    await commandTemplate(request, a);
    await commandTemplate(request, b);
    expect((await request.put(`/api/templates/${partial}`, { data: { effort: "high" } })).ok()).toBe(true);
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await expect(dialog.getByRole("button", { name: `quick launch ${partial}`, exact: true })).toBeDisabled();
    await dialog.getByRole("button", { name: `quick launch ${a}`, exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBe(a);
    await dialog.getByRole("button", { name: `quick launch ${b}`, exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBe(b);
    await expect(dialog.locator('.templates-quick-mark[aria-pressed="true"]')).toHaveCount(1);
    await dialog.getByText(b, { exact: true }).click();
    await expect(dialog.locator(".templates-quick-state")).toContainText("quick launch");
    await dialog.locator(".templates-name").fill(renamed);
    await dialog.getByRole("button", { name: "save", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBe(renamed);
    await expect(dialog.getByText(renamed, { exact: true })).toBeVisible();
    await expect(dialog.getByText(b, { exact: true })).toHaveCount(0);
    await dialog.getByRole("button", { name: "delete", exact: true }).click();
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template).toBeUndefined();
    await dialog.getByRole("button", { name: "undo", exact: true }).click();
    await expect(dialog.getByText(renamed, { exact: true })).toBeVisible();
    await expect(dialog.locator('.templates-quick-mark[aria-pressed="true"]')).toHaveCount(0);
    expect((await readPreferences(request)).quick_launch_template).toBeUndefined();
  } finally {
    for (const name of [a, b, partial, renamed]) await request.delete(`/api/templates/${encodeURIComponent(name)}`);
  }
});

/** A mark can commit while its response is still in flight. Deletion must
 * wait for that owned writer rather than racing it, then Undo restores only
 * fields. The held response and disabled Delete prove the unsettled boundary;
 * no delay or credential remount is used as a proxy for ordering. */
test("deletion drains an in-flight quick mark before clearing it", async ({ page, request }) => {
  const name = `quick-pending-delete-${Date.now()}`;
  const gate = routeGate();
  let held = 0;
  let deletes = 0;
  await page.route("**/api/preferences", async (route) => {
    if (route.request().method() !== "PUT" || route.request().postDataJSON().quick_launch_template !== name) {
      return route.continue();
    }
    const response = await route.fetch();
    expect(response.ok()).toBe(true);
    held++;
    await gate.wait();
    await route.fulfill({ response });
  });
  await page.route(`**/api/templates/${name}`, async (route) => {
    if (route.request().method() === "DELETE") deletes++;
    await route.continue();
  });
  try {
    await commandTemplate(request, name);
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await dialog.getByText(name, { exact: true }).click();
    await dialog.getByRole("button", { name: `quick launch ${name}`, exact: true }).click();
    await expect.poll(() => held, { message: "the mark committed and its response is held" }).toBe(1);
    expect((await readPreferences(request)).quick_launch_template).toBe(name);
    const remove = dialog.getByRole("button", { name: "delete", exact: true });
    await remove.click();
    await expect(remove).toBeDisabled();
    expect(deletes, "DELETE cannot overtake the held preference response").toBe(0);
    gate.release();
    await expect(dialog.getByText(name, { exact: true })).toHaveCount(0);
    expect(deletes).toBe(1);
    expect((await readPreferences(request)).quick_launch_template).toBeUndefined();
    await dialog.getByRole("button", { name: "undo", exact: true }).click();
    await expect(dialog.getByText(name, { exact: true })).toBeVisible();
    await expect(dialog.locator('.templates-quick-mark[aria-pressed="true"]')).toHaveCount(0);
    expect((await readPreferences(request)).quick_launch_template).toBeUndefined();
  } finally {
    gate.release();
    await page.unroute("**/api/preferences");
    await page.unroute(`**/api/templates/${name}`);
    await request.delete(`/api/templates/${name}`);
  }
});

/** A marked template edited into one that can no longer be the quick launch
 * (here through the API, which the editor's eligibility rule does not see)
 * still shows a lit, clickable bolt, and clicking it clears the mark. Why:
 * clicking the lit bolt is the only way to turn a mark off in the UI, so a
 * disabled lit bolt left a mark nobody could remove short of deleting the
 * template. */
test("a marked template that became ineligible can still be unmarked", async ({ page, request }) => {
  const name = `quick-ineligible-${Date.now()}`;
  try {
    await commandTemplate(request, name);
    await patchPreferences(request, { quick_launch_template: name });
    expect((await request.put(`/api/templates/${encodeURIComponent(name)}`, { data: { effort: "high" } })).ok()).toBe(
      true,
    );
    expect((await readPreferences(request)).quick_launch_template, "premise: the mark survives the edit").toBe(name);
    await page.goto("/");
    await page.locator(".templates-button").click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    const bolt = dialog.getByRole("button", { name: `quick launch ${name}`, exact: true });
    await expect(bolt).toHaveAttribute("aria-pressed", "true");
    await expect(bolt).toBeEnabled();
    await bolt.click();
    await expect(bolt).toHaveAttribute("aria-pressed", "false");
    await expect.poll(async () => (await readPreferences(request)).quick_launch_template ?? null).toBeNull();
    await expect(bolt, "an ineligible template cannot be marked again").toBeDisabled();
  } finally {
    await request.delete(`/api/templates/${encodeURIComponent(name)}`);
  }
});
