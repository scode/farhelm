/**
 * Restart mode of the session launcher edits a stored launch while keeping its conversation.
 *
 * An injected resumable row gives the UI an offer that the fake agent cannot
 * produce on demand. The real listing supplies host metadata; restart answers
 * are controlled so assertions stay on browser consent and request contents.
 * Cases that require particular model or effort choices also supply a stamped
 * catalog, keeping that fixture premise independent of the shipped inventory.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator, type Page } from "@playwright/test";
import { clickDialogGap } from "./helpers/dialog-gap";
import {
  agentLaunchRow,
  commandLaunchRow,
  listHosts,
  patchPreferences,
  readPreferences,
  SESSION_LISTING,
  stopSession,
} from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { attachSession, cleanupSession, waitForTermText } from "./helpers/term";
import {
  addTab,
  armTerminalFocusSpy,
  closeTabAndAwaitAgentFallback,
  createTabSession,
  fulfillAsHelm,
  installTerminalFocusSpy,
  installTerminalSuiteHooks,
  selectTabOverRevealedAgent,
  terminalFocusCount,
} from "./helpers/terminal-suite";

// The tab sweep removes the scratch working directory `createTabSession` makes
// for the tab-fallback focus test's own session.
installTerminalSuiteHooks({ tabSweep: true });

const SESSION_ID = "77777777-2222-3333-4444-555555555555";
const TITLE = "restart-with-browser-fixture";
const BASELINE = {
  harness: "codex",
  model: "gpt-6-astra" as string | null,
  effort: "high" as string | null,
  permissions: null as string | null,
  workspace_trust: null as boolean | null,
};

/** A restart shows its immutable destination without exposing editors that
 * could silently turn a resume into a different launch. The same assertions
 * apply to both stored launch kinds. */
async function expectHeldContext(dialog: Locator, agent: string) {
  const context = dialog.locator(".launch-composer-held-context");
  await expect(context).toContainText(new RegExp(agent, "i"));
  await expect(context).toContainText("/tmp");
  await expect(context).toContainText(/host/i);
  await expect(dialog.locator(".create-session-submit"))
    .toHaveText(new RegExp(`^(?:stop and )?restart\\s+${agent} · .+ · /tmp$`, "i"));
  await expect(dialog.getByRole("tab")).toHaveCount(0);
  await expect(dialog.locator(".launch-composer-harness-choice")).toHaveCount(0);
  await expect(dialog.locator(".launch-command-agent, .launch-command-resume-toggle")).toHaveCount(0);
  await expect(dialog.getByLabel("folder", { exact: true })).toHaveCount(0);
  await expect(dialog.getByLabel("name (optional)", { exact: true })).toHaveCount(0);
  await expect(dialog.getByLabel("initial prompt", { exact: true })).toHaveCount(0);
  await expect(dialog.locator(".create-session-host, .launch-composer-host-picker, .launch-composer-browser")).toHaveCount(0);
}

/** Without capture the ordinary Restart stays unavailable, but Restart with
 * accepts a known conversation without requiring an unrelated settings edit.
 * The intercepted request proves that the value is outside saved choices.
 */
test("named restart with requires a target when no conversation was captured", async ({ page }) => {
  await injectSession(page, BASELINE, "not_captured", "interrupted", [
    "codex", "resume", "{conversation}", "{farhelm_args}",
  ]);
  const bodies = await acceptYoloRestart(page);
  const dialog = await openInjectedDialog(page);
  const target = dialog.locator(".named-conversation");
  await expect(target).toHaveValue("");
  await expect(dialog).toContainText("Farhelm has no captured conversation");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  await target.fill("named-conversation");
  await expect(dialog.locator(".create-session-submit")).toBeEnabled();
  await dialog.locator(".create-session-submit").click();
  await expect(dialog).toHaveCount(0);
  expect(bodies).toHaveLength(1);
  // Unset workspace trust is omitted by the ordinary selection's wire shape.
  expect(bodies[0]).toMatchObject({
    with: { harness: BASELINE.harness, model: BASELINE.model, effort: BASELINE.effort, permissions: BASELINE.permissions },
    conversation: "named-conversation",
  });
  expect((bodies[0] as { with: object }).with).not.toHaveProperty("conversation");
});

/**
 * Cancel is the safe opening focus, including after a gap click. Container
 * Enter must respect the unchanged draft's disabled action, then submit one
 * edited restart through the same button a pointer would press.
 */
test("restart with keeps gap focus and container Enter uses its enabled action", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  const bodies = await acceptYoloRestart(page);
  const dialog = await openInjectedDialog(page);
  const cancel = dialog.locator(".launch-composer-cancel");
  await expect(cancel).toBeFocused();
  await clickDialogGap(page, dialog);
  await expect(cancel).toBeFocused();
  await dialog.focus();
  await expect(dialog).toBeFocused();
  await dialog.press("Enter");
  expect(bodies, "an unchanged draft has no action to perform").toHaveLength(0);

  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  await expect(dialog.locator(".create-session-submit")).toBeEnabled();
  await dialog.focus();
  await expect(dialog).toBeFocused();
  await dialog.press("Enter");
  await expect(dialog).toHaveCount(0);
  expect(bodies, "one container Enter must request one restart").toHaveLength(1);
});

/** Restart refusal text can quote a peer's hidden characters. Its display
 * must expose them and isolate direction while the editable dialog stays open.
 * Restart requires an edited launch choice; an unchanged draft never submits
 * the request whose refusal this test is meant to observe. */
test("Restart with escapes and isolates peer refusal text", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    await fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: "refused \u202Ehost\u200B" });
  });
  const dialog = await openInjectedDialog(page);
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  const submit = dialog.locator(".create-session-submit");
  await expect(submit).toBeEnabled();
  await submit.click();
  const error = dialog.locator(".launch-composer-refusal-alert .peer-value");
  await expect(error).toHaveText("refused <U+202E>host<U+200B>");
  await expect(error).toHaveAttribute("dir", "ltr");
  expect(await error.evaluate((node) => getComputedStyle(node).unicodeBidi)).toBe("isolate");
  await expect(dialog).toBeVisible();
});

/**
 * A stored prefix may have come from another host. Restart with must expose
 * its direction-changing bytes without letting the escaped display spelling
 * turn into new launch words; clearing the choice makes the field editable.
 */
test("restart with shows an unsafe stored wrapper prefix without editing its escape text", async ({ page }) => {
  await injectSession(page, { ...BASELINE, wrapper_prefix: ["wrapper\u202e", "--"] }, "resume");
  const dialog = await openInjectedDialog(page);
  const wrapper = dialog.locator(".launch-composer-wrapper");
  await expect(wrapper).toHaveValue(/wrapper<U\+202E>/);
  await expect(wrapper).toHaveJSProperty("readOnly", true);
  await dialog.getByRole("button", { name: "clear wrapper prefix" }).click();
  await expect(wrapper).toHaveValue("");
  await expect(wrapper).toHaveJSProperty("readOnly", false);
});

/**
 * Publish one controlled row through the real listing response.
 *
 * Copying the stack's existing row keeps host and transport metadata valid;
 * the test changes only the state whose offer and stored selection matter.
 * A named restart without capture must also supply its stored resume command:
 * the shared row builder deliberately has none, for generic fixture callers.
 */
async function injectSession(
  page: Page, launch: typeof BASELINE | null, offer: string, state = "interrupted", resume?: string[],
) {
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const listing = await response.json();
    const source = listing.sessions.find((session: { title: string }) => session.title === "e2e-session");
    if (!source) throw new Error("the shared stack must list its local e2e-session source");
    listing.sessions.push({
      ...source,
      id: SESSION_ID,
      title: TITLE,
      cwd: "/tmp",
      invocation: "codex",
      // `null` stands for a session from before launch kinds, which the
      // helm lists with a legacy launch.
      launch:
        launch === null
          ? { kind: "legacy", invocation: "codex", agent_kind: "codex", resume_template: null }
          : { ...agentLaunchRow(launch), ...(resume ? { resume } : {}) },
      status: { state },
      restart_offer: offer,
    });
    listing.total += 1;
    await route.fulfill({ response, json: listing });
  });
}

/**
 * Answer the injected row's restarts as a successful restart to yolo would.
 *
 * Returns the request bodies in arrival order, so a test can prove how many
 * restarts the UI sent and what they carried.
 */
async function acceptYoloRestart(page: Page): Promise<unknown[]> {
  const bodies: unknown[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    bodies.push(route.request().postDataJSON());
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: SESSION_ID,
        title: TITLE,
        cwd: "/tmp",
        invocation: "codex --yolo",
        launch: agentLaunchRow({ ...BASELINE, permissions: "yolo" }),
        status: { state: "unknown" },
        restart_offer: "resume",
        created_at: 0,
        last_activity_at: 0,
        tabs: [],
      }),
    });
  });
  return bodies;
}

/**
 * Open the injected row's session and its restart-with dialog.
 *
 * Returns once the dialog's initial focus has landed on cancel, which is also
 * the moment the dialog isolates the page behind it.
 */
async function openInjectedDialog(page: Page) {
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  // Navigation finishes before WASM startup and the injected list render. Use
  // the same readiness budget as openTerminal: WebKit can receive the listing
  // near five seconds and paint its healthy row only after that deadline.
  try {
    await expect(row).toBeVisible({ timeout: 20_000 });
  } catch (error) {
    // The row exists only in the page's intercepted response. A direct API
    // probe checks its real source premise, not whether the synthetic row is
    // stored in the helm; the DOM probe says whether injection reached paint.
    const source = await page.request.get("/api/sessions", { timeout: 5_000 })
      .then(async (response) => {
        if (!response.ok()) return `source listing HTTP ${response.status()}`;
        const listing = await response.json();
        return `source session present: ${listing.sessions.length > 0}`;
      })
      .catch((probeError) => `source listing unavailable: ${probeError}`);
    const titlePainted = await page.getByRole("button", { name: TITLE, exact: false }).count();
    throw new Error(`injected row readiness failed; ${source}; matching title buttons: ${titlePainted}`, { cause: error });
  }
  await row.locator(".session-row-open").click();
  await expect(page.locator(".restart-with-trigger")).toHaveAttribute("aria-disabled", "false");
  await page.locator(".restart-with-trigger").click();
  const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".launch-composer-cancel")).toBeFocused();
  return dialog;
}

/**
 * Cancel must be side effect free, and consent must carry the edited choice once.
 *
 * The reply exercises the ordinary restart success path, including closing the
 * modal, while the request body proves the UI did not create a fresh session.
 */
test("restart with shows fixed context and submits one edited resume", async ({ page, request }) => {
  const previous = await readPreferences(request);
  try {
    // The settings dialog is still the action, even when session questions are skipped.
    await patchPreferences(request, { skip_session_confirmations: true });
    await injectSession(page, BASELINE, "resume");
    const bodies = await acceptYoloRestart(page);

    await page.goto("/");
    const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
    await expect(row).toBeVisible();
    await row.locator(".session-row-open").click();
    const trigger = page.locator(".restart-with-trigger");
    await expect(trigger).toBeVisible();
    await expect(trigger).not.toHaveAttribute("aria-disabled", "true");
    await trigger.click();

    const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
    await expect(dialog).toBeVisible();
    await expectHeldContext(dialog, "codex");

    const submit = dialog.locator(".create-session-submit");
    await expect(submit).toBeDisabled();
    await dialog.locator(".launch-composer-cancel").click();
    await expect(dialog).toHaveCount(0);
    expect(bodies, "cancel must not send a restart").toEqual([]);

    await trigger.click();
    await expect(dialog).toBeVisible();
    await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
    await expect(dialog.locator(".launch-composer-permissions-choice .launch-composer-changed-marker"))
      .toHaveText("● changed · was agent's own setting");
    await expect(submit).toBeEnabled();
    await expect(submit).toHaveText(/^restart\s+codex · .+ · \/tmp$/i);
    await submit.click();
    await expect(dialog).toHaveCount(0);
    expect(bodies, "one consent must send one restart request").toEqual([{
      stop_if_running: false,
      with: { harness: "codex", model: "gpt-6-astra", effort: "high", permissions: "yolo" },
    }]);
  } finally {
    try {
      await page.unrouteAll({ behavior: "wait" });
    } finally {
      await patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null });
    }
  }
});

/**
 * An older no-flag session can adopt guarded by editing only its permission.
 * The comparison must preserve the old command's own-setting intent, or the
 * selected guarded chip would appear unchanged and block the upgrade path.
 */
test("restart with upgrades an older own-setting launch to guarded", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  const bodies = await acceptYoloRestart(page);
  await page.goto("/");
  const dialog = await openInjectedDialog(page);
  const submit = dialog.locator(".create-session-submit");
  await expect(submit, "the stored launch is initially unchanged").toBeDisabled();
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
  await expect(dialog.locator(".launch-composer-permissions-choice .launch-composer-changed-marker"))
    .toHaveText("● changed · was agent's own setting");
  await expect(submit).toBeEnabled();
  await submit.click();
  await expect(dialog).toHaveCount(0);
  expect(bodies).toEqual([{
    stop_if_running: false,
    with: { harness: "codex", model: "gpt-6-astra", effort: "high", permissions: "guarded" },
  }]);
});

/**
 * A restart with YOLO settings that the helm refuses for a host that asks before YOLO launches asks
 * inside the dialog, and confirming resends the dialog's settings with the
 * override.
 *
 * Why: the dialog is modal, so a confirmation rendered beside it would be
 * inert and the refused restart could never be confirmed. The refusal is
 * route-mocked with the helm's own header (the helm tests pin the helm's side);
 * what only a browser shows is where the question appears and what the
 * confirmed request carries.
 */
test("a YOLO restart-with requires confirmation inside the dialog", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  const bodies: any[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    const body = route.request().postDataJSON();
    bodies.push(body);
    if (!body.confirm_yolo) {
      await fulfillAsHelm(route, {
        status: 409,
        contentType: "text/plain",
        headers: { "x-farhelm-yolo-confirmation": "confirmation-required" },
        body: "this machine asks before YOLO launches; confirm with --confirm-yolo",
      });
      return;
    }
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: SESSION_ID,
        title: TITLE,
        cwd: "/tmp",
        invocation: "codex --yolo",
        launch: agentLaunchRow({ ...BASELINE, permissions: "yolo" }),
        status: { state: "unknown" },
        restart_offer: "resume",
        created_at: 0,
        last_activity_at: 0,
        tabs: [],
      }),
    });
  });

  const dialog = await openInjectedDialog(page);
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  await dialog.locator(".create-session-submit").click();
  const confirmation = dialog.locator(".yolo-confirmation");
  await expect(confirmation, "the question must appear inside the modal dialog").toBeVisible();
  await expect(confirmation).toContainText("This launch uses YOLO permissions.");
  await expect(confirmation).not.toContainText("--confirm-yolo");
  expect(bodies).toHaveLength(1);

  // Declining from the keyboard hands focus back to the dialog rather than
  // dropping it to the page body with the modal still open. Cancel is the
  // question's initial focus, given once its buttons are usable (it can mount
  // while the refused restart is still finishing).
  await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(confirmation).toHaveCount(0);
  await expect(dialog.locator(".launch-composer-cancel")).toBeFocused();
  expect(bodies, "declining must not send anything").toHaveLength(1);

  // Asking again and confirming from the keyboard sends the override.
  await dialog.locator(".create-session-submit").click();
  await expect(confirmation).toBeVisible();
  expect(bodies).toHaveLength(2);
  await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
  await confirmation.locator(".yolo-confirm").focus();
  await expect(confirmation.locator(".yolo-confirm")).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  expect(bodies).toHaveLength(3);
  expect(bodies[2]).toMatchObject({
    with: { harness: "codex", permissions: "yolo" },
    confirm_yolo: true,
  });
});

/**
 * Guarded answers change the visible draft and restart without YOLO consent.
 * Holding the guarded request establishes the busy boundary: the permission
 * row must already show guarded and focus must survive the question's removal.
 * No host setting is needed to take this safer path.
 */
test("restart with can answer the YOLO question by restarting guarded", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  const restarts: any[] = [];
  const marks: unknown[] = [];
  const gate = routeGate();
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    const body = route.request().postDataJSON();
    restarts.push(body);
    if (body.with?.permissions === "yolo") {
      await fulfillAsHelm(route, {
        status: 409,
        contentType: "text/plain",
        headers: { "x-farhelm-yolo-confirmation": "confirmation-required" },
        body: "this machine asks before YOLO launches; confirm with --confirm-yolo",
      });
      return;
    }
    await gate.wait();
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: SESSION_ID,
        title: TITLE,
        cwd: "/tmp",
        invocation: "codex --sandbox workspace-write",
        launch: agentLaunchRow({ ...BASELINE, permissions: "guarded" }),
        status: { state: "unknown" },
        restart_offer: "resume",
        created_at: 0,
        last_activity_at: 0,
        tabs: [],
      }),
    });
  });
  await page.route("**/api/hosts/*/yolo-without-asking", async (route) => {
    marks.push(route.request().postDataJSON());
    await fulfillAsHelm(route, { status: 200, contentType: "application/json", body: "{}" });
  });

  try {
    const dialog = await openInjectedDialog(page);
    const permissions = dialog.locator(".launch-composer-permissions-choice");
    await permissions.getByRole("button", { name: "yolo", exact: true }).click();
    const submit = dialog.locator(".create-session-submit");
    await submit.click();
    const confirmation = dialog.locator(".yolo-confirmation");
    await expect(confirmation).toBeVisible();
    expect(restarts).toHaveLength(1);
    expect(restarts[0]).toMatchObject({ with: { permissions: "yolo" } });
    await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
    const guarded = confirmation.locator(".yolo-launch-guarded");
    await expect(guarded).toHaveText("start guarded session instead");
    await expect(guarded).toHaveClass(/\bbtn-primary\b/);
    await expect(confirmation.locator("button").first()).toHaveClass(/\byolo-launch-guarded\b/);
    await guarded.click();

    await expect.poll(() => restarts.length).toBe(2);
    await expect(submit).toHaveAttribute("aria-disabled", "true");
    await expect(confirmation).toHaveCount(0);
    await expect(permissions.getByRole("button", { name: "guarded", exact: true }))
      .toHaveAttribute("aria-pressed", "true");
    await expect(permissions.locator(".launch-composer-changed-marker"))
      .toHaveText("● changed · was agent's own setting");
    await expect(submit).toBeFocused();
    expect(restarts[1]).toEqual({
      stop_if_running: false,
      with: { harness: "codex", model: "gpt-6-astra", effort: "high", permissions: "guarded" },
    });
    expect(marks, "guarded answers do not mark the host safe for YOLO").toHaveLength(0);
    gate.release();
    await expect(dialog).toHaveCount(0);
    expect(restarts, "one guarded answer sends one additional restart").toHaveLength(2);
    expect(marks).toHaveLength(0);
  } finally {
    gate.release();
  }
});

/**
 * Returning only the permission to its stored guarded value would leave no
 * edit for Restart with to submit. Another changed field makes the guarded
 * alternative useful again, so stored guarded alone must not hide it.
 */
test("restart with hides guarded when it would restore the unchanged launch", async ({ page }) => {
  await injectSession(page, { ...BASELINE, permissions: "guarded" }, "resume");
  const restarts: unknown[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    restarts.push(route.request().postDataJSON());
    await fulfillAsHelm(route, {
      status: 409,
      contentType: "text/plain",
      headers: { "x-farhelm-yolo-confirmation": "confirmation-required" },
      body: "this machine asks before YOLO launches; confirm with --confirm-yolo",
    });
  });
  const dialog = await openInjectedDialog(page);
  const submit = dialog.locator(".create-session-submit");
  await expect(submit, "the stored guarded launch starts unchanged").toBeDisabled();
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  await submit.click();
  const confirmation = dialog.locator(".yolo-confirmation");
  await expect(confirmation).toBeVisible();
  expect(restarts).toHaveLength(1);
  await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
  await expect(confirmation.locator(".yolo-launch-guarded")).toHaveCount(0);
  await expect(confirmation.locator(".yolo-confirm")).toBeVisible();
  await expect(confirmation.locator(".yolo-confirm-stop-asking")).toBeVisible();

  await confirmation.locator(".yolo-cancel").click();
  await expect(confirmation).toHaveCount(0);
  const model = dialog.locator(".launch-composer-model-choice");
  await model.getByRole("combobox", { name: "model" }).click();
  await model.getByRole("option", { name: "harness default" }).click();
  await expect(model.locator(".launch-composer-changed-marker")).toBeVisible();
  await submit.click();
  await expect(confirmation).toBeVisible();
  expect(restarts).toHaveLength(2);
  await expect(confirmation.locator(".yolo-launch-guarded")).toBeVisible();
});

/**
 * "Start, and don't ask again on this host" from inside the restart-with
 * dialog keeps focus in the dialog while it runs, launches nothing when the
 * host cannot be marked, and restarts with the override once it can.
 *
 * Why: this dialog never lets focus fall to the page body (its module doc),
 * because its modal isolation then swallows the next keystroke. Answering
 * takes the question down while the host is marked, which would drop the
 * focus of the button that was pressed; and the button's promise ("don't ask
 * again") is only kept if the host is marked before the restart goes out.
 * Specifies: activated from the keyboard, focus moves to the dialog's submit
 * (which stays enabled) while the mark is in flight and the question is gone;
 * a refused mark brings the question back with its reason and sends no
 * restart; a mark that succeeds is followed by one restart that carries the
 * override, and the dialog closes. The host write and the restart
 * are both route-mocked, so the shared helm's real host setting is untouched.
 */
test("don't ask again from restart with keeps focus in the dialog and marks before restarting", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  const restarts: any[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    const body = route.request().postDataJSON();
    restarts.push(body);
    if (!body.confirm_yolo) {
      await fulfillAsHelm(route, {
        status: 409,
        contentType: "text/plain",
        headers: { "x-farhelm-yolo-confirmation": "confirmation-required" },
        body: "this machine asks before YOLO launches; confirm with --confirm-yolo",
      });
      return;
    }
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: SESSION_ID,
        title: TITLE,
        cwd: "/tmp",
        invocation: "codex --yolo",
        launch: agentLaunchRow({ ...BASELINE, permissions: "yolo" }),
        status: { state: "unknown" },
        restart_offer: "resume",
        created_at: 0,
        last_activity_at: 0,
        tabs: [],
      }),
    });
  });
  // The first mark is held, then refused; the second succeeds. Nothing
  // reaches the real helm's host registry.
  let releaseMark!: () => void;
  let markHeld = new Promise<void>((resolve) => (releaseMark = resolve));
  let refuseMark = true;
  const marks: unknown[] = [];
  await page.route("**/api/hosts/*/yolo-without-asking", async (route) => {
    marks.push(route.request().postDataJSON());
    await markHeld;
    if (refuseMark) {
      await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "held by the test" });
    } else {
      await fulfillAsHelm(route, { status: 200, contentType: "application/json", body: "{}" });
    }
  });

  const dialog = await openInjectedDialog(page);
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  await dialog.locator(".create-session-submit").click();
  const confirmation = dialog.locator(".yolo-confirmation");
  await expect(confirmation).toBeVisible();
  expect(restarts).toHaveLength(1);

  const stopAsking = confirmation.locator(".yolo-confirm-stop-asking");
  // The question's initial focus lands on cancel once its buttons are usable;
  // waiting for that handoff first keeps it from landing after this test has
  // moved focus to another answer.
  await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
  await stopAsking.focus();
  await expect(stopAsking).toBeFocused();
  try {
    await page.keyboard.press("Enter");
    await expect.poll(() => marks.length, { message: "the mark reached the route" }).toBe(1);
    await expect(confirmation, "the answer took the question down while the mark runs").toHaveCount(0);
    await expect(dialog.locator(".create-session-submit"), "focus stays in the dialog").toBeFocused();
  } finally {
    releaseMark();
  }
  await expect(confirmation.locator(".yolo-confirmation-error")).toContainText("held by the test");
  await expect(confirmation, "a refused mark brings the question back").toBeVisible();
  expect(restarts, "a refused mark sends no restart").toHaveLength(1);

  refuseMark = false;
  markHeld = Promise.resolve();
  await stopAsking.click();
  await expect(dialog).toHaveCount(0);
  expect(marks).toEqual([{ yolo_without_asking: true }, { yolo_without_asking: true }]);
  expect(restarts).toHaveLength(2);
  expect(restarts[1]).toMatchObject({
    with: { harness: "codex", permissions: "yolo" },
    confirm_yolo: true,
  });
});

/**
 * Cancel and an answer to Restart with's YOLO question delivered in one
 * burst, before the render that removes the question, restart nothing. Why:
 * an accepted answer restarts the agent with no approval prompts (and can
 * stop a working agent first), and the session view used to forward the
 * dialog's approval whatever the state of the question. Specifies, for each
 * answer: after the burst no restart is sent and no host is marked, and a
 * following plain restart is asked about again; a genuine answer afterwards
 * still restarts with the override. Both endpoints are route-mocked.
 */
test("cancel and an answer to restart with's YOLO question in one burst restart nothing", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  const restarts: any[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    const body = route.request().postDataJSON();
    restarts.push(body);
    if (!body.confirm_yolo) {
      await fulfillAsHelm(route, {
        status: 409,
        contentType: "text/plain",
        headers: { "x-farhelm-yolo-confirmation": "confirmation-required" },
        body: "this machine asks before YOLO launches; confirm with --confirm-yolo",
      });
      return;
    }
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: SESSION_ID,
        title: TITLE,
        cwd: "/tmp",
        invocation: "codex --yolo",
        launch: agentLaunchRow({ ...BASELINE, permissions: "yolo" }),
        status: { state: "unknown" },
        restart_offer: "resume",
        created_at: 0,
        last_activity_at: 0,
        tabs: [],
      }),
    });
  });
  const marks: unknown[] = [];
  await page.route("**/api/hosts/*/yolo-without-asking", async (route) => {
    marks.push(route.request().postDataJSON());
    await fulfillAsHelm(route, { status: 200, contentType: "application/json", body: "{}" });
  });

  const dialog = await openInjectedDialog(page);
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  const confirmation = dialog.locator(".yolo-confirmation");
  const isRestart = (candidate: { request(): { method(): string }; url(): string }) =>
    candidate.request().method() === "POST" && candidate.url().endsWith(`/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`);
  await Promise.all([page.waitForResponse(isRestart), dialog.locator(".create-session-submit").click()]);
  await expect(confirmation).toBeVisible();
  await expect(confirmation.locator(".yolo-cancel")).toBeFocused();

  for (const answer of [".yolo-confirm", ".yolo-confirm-stop-asking", ".yolo-launch-guarded"]) {
    // Both buttons are captured, then clicked in one synchronous block:
    // Cancel first, the answer second, before any render can remove the
    // answer. The answer must still be connected and enabled when clicked,
    // or the click would reach no handler and prove nothing.
    const answerWasLive = await confirmation.evaluate((node, selector) => {
      const cancel = node.querySelector<HTMLButtonElement>(".yolo-cancel")!;
      const start = node.querySelector<HTMLButtonElement>(selector)!;
      cancel.click();
      const live = start.isConnected && !start.disabled;
      start.click();
      return live;
    }, answer);
    expect(answerWasLive, `premise: ${answer} was still live when clicked`).toBe(true);
    await expect(confirmation).toHaveCount(0);
    await expect(dialog, "cancelling the question keeps the dialog open").toBeVisible();
    await expect(dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }))
      .toHaveAttribute("aria-pressed", "true");

    // A plain restart afterwards: a restart the stale answer had approved
    // would show up below as a body carrying the override (and a 200 reply),
    // and this one must be asked about again rather than ride on it.
    const [again] = await Promise.all([
      page.waitForResponse(isRestart),
      dialog.locator(".create-session-submit").click(),
    ]);
    expect(again.status(), `after cancel then ${answer}, the restart is not approved`).toBe(409);
    await expect(confirmation).toBeVisible();
    await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
  }
  expect(
    restarts.map((body) => body.confirm_yolo ?? false),
    "no restart after a burst carries the override",
  ).toEqual([false, false, false, false]);
  expect(restarts.map((body) => body.with.permissions), "stale answers leave every submitted draft on YOLO")
    .toEqual(["yolo", "yolo", "yolo", "yolo"]);
  expect(marks, "the host is never marked").toHaveLength(0);

  // Positive control: a genuine answer restarts with the override.
  await confirmation.locator(".yolo-confirm").click();
  await expect(dialog).toHaveCount(0);
  expect(restarts).toHaveLength(5);
  expect(restarts[4]).toMatchObject({
    with: { harness: "codex", permissions: "yolo" },
    confirm_yolo: true,
  });
});

/**
 * A legacy session has no structured selection to edit even if it can resume.
 * The inert button must remain visible and explain the missing prerequisite to
 * both a pointer user and assistive technology, while refusing activation.
 */
test("unavailable restart with stays visible and inert with a reason", async ({ page }) => {
  await injectSession(page, null, "resume");
  const bodies: unknown[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    bodies.push(route.request().postDataJSON());
    await route.abort();
  });

  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  await expect(row).toBeVisible();
  await row.locator(".session-row-open").click();
  const trigger = page.locator(".restart-with-trigger");
  const reason = "this session was created before launch kinds, so its launch cannot be changed; use replace with";
  await expect(trigger).toBeVisible();
  await expect(trigger).toHaveAttribute("aria-disabled", "true");
  await trigger.hover();
  await expect(trigger).toHaveAttribute("data-tooltip", reason);
  const descriptionId = await trigger.getAttribute("aria-describedby");
  expect(descriptionId, "the reason needs an assistive-technology target").toBeTruthy();
  await expect(page.locator(`#${descriptionId}`)).toHaveText(reason);
  // Playwright treats aria-disabled as non-actionable; dispatch the DOM click
  // to prove the handler also guards activation if an engine sends one.
  await trigger.evaluate((element) => (element as HTMLElement).click());
  await expect(page.locator(".create-session-form[data-launcher-mode=restart]")).toHaveCount(0);
  expect(bodies, "an inert button must not send a restart").toEqual([]);
});

/**
 * Present a real, live session as a structured, resumable one.
 *
 * The focus tests need a real terminal attached beneath the dialog, which an
 * injected row cannot supply; they use the suite's shared `e2e-session`, or a
 * session of their own when they change its tabs. The listing and the session's own detail read
 * are both rewritten, because the view takes its availability decision from
 * whichever of the two answered last; everything else, including the live
 * status and the terminal socket, stays real.
 */
async function presentAsResumable(page: Page, id: string) {
  const resumable = (session: Record<string, unknown>) => ({
    ...session,
    launch: agentLaunchRow(BASELINE),
    restart_offer: "resume",
  });
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const listing = await response.json();
    listing.sessions = listing.sessions.map((session: Record<string, unknown>) =>
      session.id === id ? resumable(session) : session
    );
    await route.fulfill({ response, json: listing });
  });
  await page.route((url) => url.pathname === `/api/sessions/${encodeURIComponent(id)}`, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    if (!response.ok()) {
      await route.fulfill({ response });
      return;
    }
    await route.fulfill({ response, json: resumable(await response.json()) });
  });
}

/** Whether the document's focused element is inside the restart-with dialog. */
async function focusInsideDialog(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const dialog = document.querySelector(".create-session-form[data-launcher-mode=restart]");
    return !!dialog && dialog.contains(document.activeElement);
  });
}

/**
 * Drop focus to the document body, as a scrim click or a removed control does.
 *
 * Asserts the premise too: the tests that use this are about what the next
 * key does with focus outside the dialog, so focus must really be there.
 */
async function blurToBody(page: Page) {
  const onBody = await page.evaluate(() => {
    (document.activeElement as HTMLElement | null)?.blur();
    return document.activeElement === document.body;
  });
  expect(onBody, "premise: focus must be on the body before the next key").toBe(true);
}

/**
 * The modal keeps keyboard focus, and so keystrokes, away from the live agent.
 *
 * A pending request natively disables the dialog's controls, which used to let
 * focus fall out of the modal. A refused restart then reattaches the running
 * session's terminal behind the still-open dialog, and that reattachment's
 * reveal used to take focus from the dialog's button. Either way the next
 * keystroke aimed at the dialog reached the agent. This specifies both
 * boundaries against a real attached terminal: while the request is held, and
 * after the refusal's reattachment has revealed, focus stays on the dialog's
 * primary action, Tab and typing keep it inside the dialog, and the terminal
 * never receives focus. Enter and Space are never pressed: either would
 * re-activate the focused submit and send a second request.
 */
test("restart with keeps focus inside the dialog over a live terminal while pending and after refusal", async ({
  page,
  request,
}) => {
  const listing = await (await request.get("/api/sessions")).json();
  const shared = listing.sessions.find((s: { title: string }) => s.title === "e2e-session");
  expect(shared, "the terminal suite's reset must leave the shared e2e-session").toBeTruthy();
  const id: string = shared.id;
  const refusal = "restart refused by fixture: the session changed";

  await presentAsResumable(page, id);
  await installTerminalFocusSpy(page);
  const gate = routeGate();
  const bodies: unknown[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(id)}/restart`, async (route) => {
    bodies.push(route.request().postDataJSON());
    await gate.wait();
    await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: refusal });
  });

  try {
    await page.goto("/");
    // Premise: a real terminal is attached, revealed and focused beneath the
    // dialog about to open, so there is something that could steal focus.
    await attachSession(page, id);

    const trigger = page.locator(".restart-with-trigger");
    await expect(trigger).not.toHaveAttribute("aria-disabled", "true");
    await trigger.click();
    const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".launch-composer-cancel")).toBeFocused();
    await armTerminalFocusSpy(page);

    await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true })
      .click();
    const submit = dialog.locator(".create-session-submit");
    await expect(submit).toBeEnabled();
    await submit.click();

    // Held: the request reached the fixture and the dialog rendered busy.
    await expect.poll(() => bodies.length).toBe(1);
    await expect(submit).toHaveAttribute("aria-disabled", "true");
    await expect(dialog.locator(".launch-composer-cancel")).toBeDisabled();
    await expect(submit).toBeFocused();
    for (const key of ["Tab", "Shift+Tab", "q", "w"]) {
      await page.keyboard.press(key);
      expect(await focusInsideDialog(page), `focus after ${key} while the request is held`).toBe(true);
    }

    // Escape that reaches the page with focus outside the dialog still obeys
    // the busy rule: the keydown net brings focus back but must not cancel a
    // request already in flight.
    await blurToBody(page);
    await page.keyboard.press("Escape");
    await expect(dialog).toBeVisible();
    expect(await focusInsideDialog(page), "focus after Escape from the body while held").toBe(true);
    // The net parks focus on the dialog container; put it back on the
    // primary action, where a click-submitted request keeps it, so the
    // refusal half below starts from the same place as before.
    await submit.focus();
    await expect(submit).toBeFocused();

    // Tag the attachment that predates the refusal, so the wait below can
    // tell the refusal's own reattachment apart from it.
    await page.evaluate(() => {
      (window as any).__farhelmIslands.terminal.test.__beforeRefusal = true;
    });
    gate.release();
    await expect(dialog.locator(".launch-composer-refusal-alert")).toContainText(refusal);
    await expect
      .poll(
        () =>
          page.evaluate(() => {
            const hook = (window as any).__farhelmIslands?.terminal?.test;
            return !!hook && !hook.__beforeRefusal && hook.replay.revealed === true;
          }),
        { timeout: 60_000, message: "waiting for the refusal's reattachment to reveal" },
      )
      .toBe(true);

    await expect(dialog).toBeVisible();
    await expect(submit).toBeFocused();
    for (const key of ["q", "w", "Shift+Tab"]) {
      await page.keyboard.press(key);
      expect(await focusInsideDialog(page), `focus after ${key} following the refusal`).toBe(true);
    }
    expect(
      await terminalFocusCount(page),
      "the terminal beneath the modal must never take focus",
    ).toBe(0);
    expect(bodies, "keys pressed inside the dialog must not resend the restart").toHaveLength(1);
  } finally {
    gate.release();
  }
});

/**
 * A selected tab going away under the dialog does not hand focus to the agent.
 *
 * When the selected tab's shell exits, or another client closes it, the view
 * falls back to the agent tab. The agent terminal is already attached and
 * revealed, so that fallback focuses it directly rather than through a reveal,
 * and it used to do so beneath the modal: the user's next keystroke meant for
 * the dialog reached the agent. This specifies that with focus on the dialog,
 * removing the selected tab leaves focus on the dialog, the agent terminal gets
 * no focus event, and typing stays inside the dialog. The tab is closed through
 * the API as a second client would, because exiting its shell would need
 * keystrokes in the tab and so focus outside the dialog; both reach the view as
 * the same tab disappearing from the session.
 */
test("restart with keeps focus inside the dialog when the selected tab goes away", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  let id: string | undefined;
  try {
    const session = await createTabSession(request, `restart-with-tab-${Date.now()}`);
    id = session.id;
    await presentAsResumable(page, id);
    await installTerminalFocusSpy(page);

    await page.goto("/");
    await attachSession(page, id);
    const tabId = await addTab(page, 0);
    await selectTabOverRevealedAgent(page, id, tabId);

    const trigger = page.locator(".restart-with-trigger");
    await expect(trigger).not.toHaveAttribute("aria-disabled", "true");
    await trigger.click();
    const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
    const cancel = dialog.locator(".launch-composer-cancel");
    await expect(dialog).toBeVisible();
    await expect(cancel).toBeFocused();
    await armTerminalFocusSpy(page);

    await closeTabAndAwaitAgentFallback(page, request, id, tabId);

    await expect(dialog).toBeVisible();
    await expect(cancel).toBeFocused();
    for (const key of ["q", "w"]) {
      await page.keyboard.press(key);
      expect(await focusInsideDialog(page), `focus after ${key} following the tab's removal`).toBe(true);
    }
    expect(
      await terminalFocusCount(page),
      "the agent terminal beneath the modal must never take focus",
    ).toBe(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

/**
 * The changed marker shows the stored model through the peer rendering rules.
 *
 * The old model id is a stored string the helm accepts with invisible and
 * directional characters in it. Interpolated raw into the app's own marker
 * text, a zero-width space makes it look like a different id. The marker must
 * show the escaped spelling inside its own direction-isolated run.
 */
test("restart with escapes the old model in its changed marker", async ({ page }) => {
  await injectSession(page, { ...BASELINE, model: "gpt-6-\u200bastra" }, "resume");
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  await expect(row).toBeVisible();
  await row.locator(".session-row-open").click();
  await page.locator(".restart-with-trigger").click();
  const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
  await expect(dialog).toBeVisible();

  const modelChoice = dialog.locator(".launch-composer-model-choice");
  await expect(modelChoice.locator(".launch-composer-changed-marker")).toHaveCount(0);
  await modelChoice.getByRole("combobox", { name: "model" }).click();
  await modelChoice.getByRole("option", { name: "harness default" }).click();
  const marker = modelChoice.locator(".launch-composer-changed-marker");
  await expect(marker).toHaveText("● changed · was gpt-6-<U+200B>astra");
  await expect(marker.locator('.peer-value[dir="ltr"]')).toHaveText("gpt-6-<U+200B>astra");
});

/**
 * Tab out of the model field moves to the next dialog control, never out of the modal.
 *
 * Focusing the model input opens its option list. Those rows used to be
 * sequential tab stops, so forward Tab targeted the first row while the
 * input's blur closed the list and unmounted it. Focus then fell to the
 * document, outside the modal, where the dialog's own Tab trap never sees the
 * next key and a keystroke can reach the page behind it. This specifies that
 * Tab from the open combobox closes the list and lands on the next persistent
 * control (Codex's effort `default`), that a full forward Tab cycle returns
 * to the model without leaving the launcher, and that backward navigation
 * crosses the preceding persistent control without visiting a removed row.
 */
test("restart with keeps Tab from the open model list inside the dialog", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  await expect(row).toBeVisible();
  await row.locator(".session-row-open").click();
  await page.locator(".restart-with-trigger").click();
  const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".launch-composer-cancel")).toBeFocused();

  const model = dialog.getByRole("combobox", { name: "model", exact: true });
  const listbox = dialog.getByRole("listbox");
  const effortDefault = dialog.locator(".launch-composer-effort-choice").getByRole("button", {
    name: "default",
    exact: true,
  });
  // Premise: the list is open with at least one transient row, so the row
  // that used to be Tab's target exists when Tab is pressed.
  await model.focus();
  await expect(model).toBeFocused();
  await expect(listbox).toBeVisible();
  await expect(listbox.getByRole("option").first()).toBeVisible();
  // The shared launcher has Search and action groups around the launch
  // controls. Their endpoints may change; only the open-model -> effort
  // transition and containment across a complete cycle matter here.
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();

  await page.keyboard.press("Tab");
  await expect(listbox).toHaveCount(0);
  await expect(effortDefault).toBeFocused();

  // Walk forward from there until the trap's wrap brings focus back to the
  // model field, through the shared launcher's later controls and its wrap. The
  // bound accommodates its action and search groups; running out means
  // focus never came back, which the assertion after the loop reports.
  let presses = 0;
  let returned = false;
  while (presses < 40 && !returned) {
    await page.keyboard.press("Tab");
    presses += 1;
    expect(await focusInsideDialog(page), `focus after forward Tab ${presses}`).toBe(true);
    returned = await model.evaluate((node) => node === document.activeElement);
  }
  expect(returned, "a forward Tab cycle must wrap back to the model field").toBe(true);
  expect(presses, "the cycle must pass the later controls, not wrap at once").toBeGreaterThan(2);
  await expect(listbox).toBeVisible();

  // Search precedes the model in the shared launcher. Backward navigation
  // must stay modal without requiring the removed dialog's endpoint order.
  await page.keyboard.press("Shift+Tab");
  await expect(listbox).toHaveCount(0);
  expect(await focusInsideDialog(page), "backward navigation stays modal").toBe(true);
  await page.keyboard.press("Tab");
  await expect(model).toBeFocused();
  await expect(listbox).toBeVisible();
  await page.keyboard.press("Tab");
  await expect(listbox).toHaveCount(0);
  await expect(effortDefault).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(model).toBeFocused();
  expect(await focusInsideDialog(page), "focus after the final Shift+Tab").toBe(true);
});

/**
 * A click on the scrim, then Tab or Shift+Tab, keeps keyboard focus inside the dialog.
 *
 * Clicking the backdrop moves focus out of the dialog to the document body,
 * where the dialog's own Tab trap never sees the next key. Browsers start
 * sequential navigation from the click point, which lies inside the backdrop
 * just before the dialog, so Shift+Tab used to walk backwards out of the
 * modal into the page behind it: the header, the sidebar, or the agent
 * terminal. (Forward Tab from that point happened to reach the dialog in
 * Chromium and WebKit, but nothing guaranteed it.) This specifies that after
 * a backdrop click, either direction and the presses after it leave focus
 * inside the dialog at every step.
 */
test("restart with keeps Tab inside the dialog after a click on its backdrop", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  const dialog = await openInjectedDialog(page);

  // Premise: the corner point is backdrop, not dialog.
  const point = { x: 5, y: 5 };
  const hit = await page.evaluate(({ x, y }) => document.elementFromPoint(x, y)?.className ?? null, point);
  expect(hit, "premise: the click point must be the backdrop").toBe("launch-composer-backdrop");

  for (const keys of [["Shift+Tab", "Shift+Tab", "Tab"], ["Tab", "Tab", "Shift+Tab"]]) {
    await page.mouse.click(point.x, point.y);
    await expect(dialog).toBeVisible();
    expect(await focusInsideDialog(page), "premise: the backdrop click moved focus out of the dialog").toBe(false);
    for (const key of keys) {
      await page.keyboard.press(key);
      expect(await focusInsideDialog(page), `focus after ${key} following the backdrop click (${keys[0]} first)`)
        .toBe(true);
    }
  }
});

/**
 * With focus pushed out of the dialog, the next key is swallowed and focus returns.
 *
 * However focus got out (a scrim click, a focused control unmounted, a
 * future caller of `focus()`), the keystroke that follows must not reach the
 * live agent beneath the modal, and Escape must still close the dialog. This
 * specifies, over a real attached terminal: with focus on the body, a letter
 * and a Tab each bring focus back inside the dialog without the terminal
 * receiving focus, and Escape from the body cancels the dialog and returns
 * focus to the header action. The busy half of the Escape rule is in the
 * held-request test above.
 */
test("restart with pulls stray focus back and still cancels on Escape", async ({ page, request }) => {
  const listing = await (await request.get("/api/sessions")).json();
  const shared = listing.sessions.find((s: { title: string }) => s.title === "e2e-session");
  expect(shared, "the terminal suite's reset must leave the shared e2e-session").toBeTruthy();
  const id: string = shared.id;
  await presentAsResumable(page, id);
  await installTerminalFocusSpy(page);
  const bodies: unknown[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(id)}/restart`, async (route) => {
    bodies.push(route.request().postDataJSON());
    await route.abort();
  });

  await page.goto("/");
  // Premise: a real terminal is attached and revealed beneath the dialog.
  await attachSession(page, id);
  const trigger = page.locator(".restart-with-trigger");
  await expect(trigger).not.toHaveAttribute("aria-disabled", "true");
  await trigger.click();
  const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".launch-composer-cancel")).toBeFocused();
  await armTerminalFocusSpy(page);

  for (const key of ["q", "Tab"]) {
    await blurToBody(page);
    await page.keyboard.press(key);
    // The container itself, not merely somewhere inside: that is where the
    // net puts focus, which tells it apart from the Tab trap or a control.
    await expect(dialog).toBeFocused();
  }

  await blurToBody(page);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(trigger).toBeFocused();
  expect(
    await terminalFocusCount(page),
    "the terminal beneath the modal must never take focus",
  ).toBe(0);
  expect(bodies, "stray keys and Escape must not send a restart").toEqual([]);
});

/**
 * The page behind the dialog is inert while it is open, and exactly as before once it closes.
 *
 * `inert` is what keeps other code from focusing the terminal behind the
 * modal and keeps Tab from reaching it. It must also come off again, and only
 * where the dialog put it: a leaked attribute freezes part of the app, and
 * removing one the page set for its own reasons unfreezes something that
 * should stay frozen. This specifies that with the dialog open the dialog is
 * not inert while the header action, the session row and New are;
 * and that after cancel, and again after a successful restart, the set of
 * inert elements is exactly the one element that was inert before opening.
 */
test("restart with makes the page inert while open and restores it exactly", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  const bodies = await acceptYoloRestart(page);
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  await expect(row).toBeVisible();
  await row.locator(".session-row-open").click();
  const trigger = page.locator(".restart-with-trigger");
  await expect(trigger).toBeVisible();

  // The launcher now lives in the sidebar's subtree. An independent body
  // sibling, rather than that ancestor, supplies the preexisting inert claim.
  // Making the sidebar inert would disable the modal being tested.
  await page.evaluate(() => {
    const outside = document.createElement("button");
    outside.dataset.inertFixture = "outside";
    outside.textContent = "preexisting inert fixture";
    outside.setAttribute("inert", "");
    document.body.append(outside);
  });
  const inertSet = () =>
    page.evaluate(() =>
      [...document.querySelectorAll("[inert]")].map((node) =>
        (node as HTMLElement).dataset.inertFixture ?? `${node.tagName}.${node.className}`
      )
    );
  expect(await inertSet(), "premise: only the fixture's own inert element").toEqual(["outside"]);
  const underInert = () =>
    page.evaluate((sessionId) => {
      const inside = (selector: string) => document.querySelector(selector)?.closest("[inert]") != null;
      return {
        dialog: inside(".create-session-form[data-launcher-mode=restart]"),
        trigger: inside(".restart-with-trigger"),
        row: inside(`[data-session-id="${sessionId}"]`),
        new: inside(".new-session-button"),
      };
    }, SESSION_ID);

  const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
  const cancel = dialog.locator(".launch-composer-cancel");
  await trigger.click();
  await expect(cancel).toBeFocused();
  expect(
    await page.evaluate(() => {
      const outside = document.querySelector('[data-inert-fixture="outside"]')!;
      const dialog = document.querySelector(".create-session-form[data-launcher-mode=restart]")!;
      return !outside.contains(dialog) && outside.parentElement!.contains(dialog);
    }),
    "premise: the preexisting inert element is a sibling on the dialog's ancestor path",
  ).toBe(true);
  expect(await underInert()).toEqual({ dialog: false, trigger: true, row: true, new: true });

  await cancel.click();
  await expect(dialog).toHaveCount(0);
  await expect(trigger).toBeFocused();
  expect(await inertSet(), "after cancel").toEqual(["outside"]);

  await trigger.click();
  await expect(cancel).toBeFocused();
  expect(await underInert()).toEqual({ dialog: false, trigger: true, row: true, new: true });
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true })
    .click();
  await dialog.locator(".create-session-submit").click();
  await expect(dialog).toHaveCount(0);
  expect(bodies, "premise: the dialog closed through a successful restart").toHaveLength(1);
  expect(await inertSet(), "after a successful restart").toEqual(["outside"]);
});

/** Custom words from a stored selection must seed the editor and an edited
 * vector must be the restart request, even when quoted arguments contain spaces.
 * Malformed quotes disable the action rather than dropping the custom choice.
 */
test("restart with re-displays and submits custom permission words", async ({ page }) => {
  const custom = { ...BASELINE, permissions: "custom", permission_flags: ["--mode", "ask me", "$HOME"] };
  await injectSession(page, custom, "resume");
  const bodies = await acceptYoloRestart(page);
  const dialog = await openInjectedDialog(page);
  const flags = dialog.locator(".custom-permission-flags");
  await expect(flags).toHaveValue("--mode 'ask me' '$HOME'");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  await flags.fill("--mode '");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  await expect(flags).toHaveValue("--mode '");
  await flags.fill('--mode "different words"');
  await expect(dialog.locator(".create-session-submit")).toBeEnabled();
  await dialog.locator(".create-session-submit").click();
  await expect(dialog).toHaveCount(0);
  expect(bodies).toHaveLength(1);
  expect(bodies[0]).toMatchObject({ with: { permissions: "custom", permission_flags: ["--mode", "different words"] } });
});

/** Older default-YOLO sessions stored an omitted permission. Pressing their
 * already-selected YOLO button must not invent an edit or enable a restart.
 */
test("restart with compares an older omitted permission by its effective mode", async ({ page }) => {
  await injectSession(page, { ...BASELINE, harness: "omp", model: null, effort: null }, "resume");
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  await expect(row).toBeVisible();
  await row.locator(".session-row-open").click();
  await page.locator(".restart-with-trigger").click();
  const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
  await expect(dialog).toBeVisible();
  const permissions = dialog.locator(".launch-composer-permissions-choice");
  const yolo = permissions.getByRole("button", { name: "yolo", exact: true });
  await expect(yolo).toHaveAttribute("aria-pressed", "true");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  // A real edit establishes that compatibility permits submission. Returning
  // to YOLO must then disable it, giving the negative marker check a completed
  // state transition to observe rather than racing the click's render.
  await permissions.getByRole("button", { name: "approve", exact: true }).click();
  await expect(dialog.locator(".create-session-submit")).toBeEnabled();
  await yolo.click();
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  await expect(permissions.locator(".launch-composer-changed-marker")).toHaveCount(0);
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
});

/**
 * Publish one command-launch row through the real listing, the command-launch
 * counterpart of `injectSession`: `resume` null makes a command launch that
 * cannot resume.
 */
async function injectCommandSession(
  page: Page, resume: string | null, offer: string, command = "claude {farhelm_args}",
) {
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const listing = await response.json();
    const source = listing.sessions.find((session: { title: string }) => session.title === "e2e-session");
    if (!source) throw new Error("the shared stack must list its local e2e-session source");
    listing.sessions.push({
      ...source,
      id: SESSION_ID,
      title: TITLE,
      cwd: "/tmp",
      invocation: command,
      launch: commandLaunchRow(command, {
        agent: "claude",
        ...(resume === null ? {} : { resume }),
      }),
      status: { state: "interrupted" },
      restart_offer: offer,
    });
    listing.total += 1;
    await route.fulfill({ response, json: listing });
  });
}

/**
 * Spec: Restart with of a command launch edits its command, resume command
 * and YOLO answer, with the launch kind and agent type shown as fixed; an
 * edited field is marked, the submit stays inactive until something changes
 * and while the edit breaks a create's rules (the reason shown), and the
 * request carries the edit whole under `with_command`.
 *
 * Why: SPEC.md makes Restart with available exactly when Restart is, for a
 * command launch too, and validates the edit as a create would before
 * anything is stopped. Only the restart route is mocked; the dialog's
 * behavior and the body it sends are what is under test.
 */
test("restart with edits a command launch's command, resume command and YOLO answer", async ({ page }) => {
  const resume = "claude --resume {conversation} {farhelm_args}";
  await injectCommandSession(page, resume, "resume");
  const bodies: unknown[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    bodies.push(route.request().postDataJSON());
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: SESSION_ID,
        title: TITLE,
        cwd: "/tmp",
        invocation: "claude --model sonnet {farhelm_args}",
        launch: commandLaunchRow("claude --model sonnet {farhelm_args}", { agent: "claude", resume }),
        status: { state: "unknown" },
        restart_offer: "resume",
        created_at: 0,
        last_activity_at: 0,
        tabs: [],
      }),
    });
  });
  const dialog = await openInjectedDialog(page);
  await expectHeldContext(dialog, "claude");

  const command = dialog.getByLabel("agent command", { exact: true });
  const submit = dialog.locator(".create-session-submit");
  await expect(command).toHaveValue("claude {farhelm_args}");
  await expect(dialog.getByLabel("resume command", { exact: true })).toHaveValue(resume);
  await expect(
    dialog.getByRole("group", { name: "runs in YOLO mode" }).getByLabel("no", { exact: true }),
  ).toBeChecked();
  await expect(submit).toBeDisabled();
  await expect(dialog.locator(".launch-composer-changed-marker")).toHaveCount(0);

  // An edit that breaks a create's rules is explained and cannot be sent.
  await command.fill("claude --model sonnet");
  await expect(dialog.locator(".launch-command-error")).toContainText("{farhelm_args}");
  await expect(submit).toBeDisabled();
  await command.fill("claude --model sonnet {farhelm_args}");
  await expect(dialog.locator(".launch-command-error")).toHaveCount(0);
  await expect(dialog.locator(".launch-composer-changed-marker")).toHaveCount(1);
  await expect(submit).toBeEnabled();
  // The other two fields reach the request too, each marked once edited.
  const editedResume = "claude --model sonnet --resume {conversation} {farhelm_args}";
  await dialog.getByLabel("resume command", { exact: true }).fill(editedResume);
  await expect(dialog.locator(".launch-composer-changed-marker")).toHaveCount(2);
  await dialog
    .getByRole("group", { name: "runs in YOLO mode" })
    .getByLabel("yes (YOLO)", { exact: true })
    .check();
  await expect(dialog.locator(".launch-composer-changed-marker")).toHaveCount(3);
  await submit.click();
  await expect(dialog).toHaveCount(0);
  expect(bodies, "one consent must send one restart request").toEqual([{
    stop_if_running: false,
    with_command: {
      command: "claude --model sonnet {farhelm_args}",
      yolo: true,
      agent: "claude",
      resume: editedResume,
    },
  }]);
});

/**
 * Spec: Restart with stays visible but unavailable for a command launch
 * with no resume command, and says why.
 *
 * Why: such a launch can never resume (SPEC.md), so Restart with, which is
 * available exactly when Restart is, must not open; the reason names the
 * missing resume command rather than a capture that could still arrive.
 */
test("restart with is unavailable for a command launch without a resume command", async ({ page }) => {
  await injectCommandSession(page, null, "no_resume_command");
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  await expect(row).toBeVisible();
  await row.locator(".session-row-open").click();
  const trigger = page.locator(".restart-with-trigger");
  await expect(trigger).toBeVisible();
  await expect(trigger).toHaveAttribute("aria-disabled", "true");
  await expect(trigger).toHaveAttribute("data-tooltip", /no resume command/);
});

/**
 * Spec: a command edit asserted YOLO that the helm refuses on a host that
 * asks first raises the YOLO question inside the dialog, worded for an
 * asserted command, and confirming resends the same edit with the override.
 *
 * Why: the question's wording comes from the launch kind (a command's YOLO
 * is the user's own assertion, not a permission Farhelm chose), and the
 * dialog is modal, so the question must appear inside it. The refusal is
 * route-mocked with the helm's own header, as in the agent-launch test.
 */
test("a YOLO command restart-with asks inside the dialog, worded for an assertion", async ({ page }) => {
  const resume = "claude --resume {conversation} {farhelm_args}";
  await injectCommandSession(page, resume, "resume");
  const bodies: any[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    const body = route.request().postDataJSON();
    bodies.push(body);
    if (!body.confirm_yolo) {
      await fulfillAsHelm(route, {
        status: 409,
        contentType: "text/plain",
        headers: { "x-farhelm-yolo-confirmation": "confirmation-required" },
        body: "this machine asks before YOLO launches; confirm with --confirm-yolo",
      });
      return;
    }
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: SESSION_ID,
        title: TITLE,
        cwd: "/tmp",
        invocation: "claude {farhelm_args}",
        launch: commandLaunchRow("claude {farhelm_args}", { yolo: true, agent: "claude", resume }),
        status: { state: "unknown" },
        restart_offer: "resume",
        created_at: 0,
        last_activity_at: 0,
        tabs: [],
      }),
    });
  });
  const dialog = await openInjectedDialog(page);
  await dialog
    .getByRole("group", { name: "runs in YOLO mode" })
    .getByLabel("yes (YOLO)", { exact: true })
    .check();
  await dialog.locator(".create-session-submit").click();
  const confirmation = dialog.locator(".yolo-confirmation");
  await expect(confirmation, "the question must appear inside the modal dialog").toBeVisible();
  await expect(confirmation).toContainText("This command was asserted to bypass its safety checks.");
  expect(bodies).toHaveLength(1);
  await confirmation.locator(".yolo-confirm").click();
  await expect(dialog).toHaveCount(0);
  expect(bodies).toHaveLength(2);
  expect(bodies[1].confirm_yolo).toBe(true);
  expect(bodies[1].with_command).toEqual(bodies[0].with_command);
  expect(bodies[1].with_command.yolo).toBe(true);
});

/**
 * Give the new Enter fixtures a known effort vocabulary before opening the dialog.
 *
 * These cases assert same-event effort changes, not the shipped model inventory.
 * The injected stored model may leave that inventory over time; an explicit
 * catalog keeps Low available and lets the test establish its actual premise.
 */
async function installEnterCatalog(page: Page) {
  await page.route("**/api/launch-catalog", async (route) => fulfillAsHelm(route, { json: [
    { id: BASELINE.model, harness: "codex", efforts: ["high", "low"] },
    { id: "enter-model", harness: "codex", efforts: ["high", "low"] },
  ] }));
}

/**
 * Keep each Enter attempt observable without starting a real resumed agent.
 * Numbered stamped refusals prove that the specific request completed and
 * released the ordinary operation lock before another choice is exercised.
 */
async function refuseEnterRestarts(page: Page): Promise<unknown[]> {
  const bodies: unknown[] = [];
  await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
    bodies.push(route.request().postDataJSON());
    await fulfillAsHelm(route, {
      status: 409,
      contentType: "text/plain",
      body: `Enter fixture refused restart ${bodies.length}`,
    });
  });
  return bodies;
}

/** Focus is a fixture premise; the new refusal identifies this attempt's completed primary action. */
async function enterRestartChoice(dialog: Locator, control: Locator, bodies: unknown[]) {
  const before = bodies.length;
  const submit = dialog.locator(".create-session-submit");
  const submitWasDisabled = await submit.isDisabled();
  await expect(control, "the Enter fixture must offer the choice before input").toBeVisible();
  await control.focus();
  await expect(control).toBeFocused();
  await control.press("Enter");
  await expect(dialog.locator(".launch-composer-refusal-alert")).toContainText(`Enter fixture refused restart ${before + 1}`);
  await expect(dialog.locator(".create-session-submit")).not.toHaveAttribute("aria-disabled", "true");
  // A first choice can still see the unchanged render's disabled submit.
  // Either surviving target is valid; body or an agent control is not.
  await expect.poll(() => dialog.evaluate((node) => node.contains(document.activeElement)), {
    message: "focus must survive the primary action inside the restart dialog",
  }).toBe(true);
  if (!submitWasDisabled) await expect(submit).toBeFocused();
  expect(bodies, "one deliberate Enter sends one ordinary restart").toHaveLength(before + 1);
  return bodies[before];
}

/** Choice Enter applies the current edit before submitting. Historical trust
 * stays absent because Restart with composes trust from current Settings. */
test("restart with choice Enter applies effort and permissions without old trust", async ({ page }) => {
  await injectSession(page, { ...BASELINE, workspace_trust: true }, "resume");
  await installEnterCatalog(page);
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  await expect(dialog.locator(".launch-composer-trust-choice")).toHaveCount(0);
  const choice = (group: string, name: string) => dialog.locator(group).getByRole("button", { name, exact: true });
  expect(await enterRestartChoice(dialog, choice(".launch-composer-effort-choice", "low"), bodies))
    .toMatchObject({ stop_if_running: false, with: { harness: "codex", model: BASELINE.model, effort: "low" } });
  expect(await enterRestartChoice(dialog, choice(".launch-composer-permissions-choice", "yolo"), bodies))
    .toMatchObject({ with: { effort: "low", permissions: "yolo" } });
  for (const body of bodies) {
    expect((body as { with: object }).with).not.toHaveProperty("workspace_trust");
  }
});

/** Enter confirms exactly the displayed stop-and-restart warning for a working agent. */
test("restart with choice Enter carries the displayed working-agent confirmation", async ({ page }) => {
  await injectSession(page, BASELINE, "resume", "running");
  await installEnterCatalog(page);
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  await expect(dialog.locator(".create-session-submit")).toHaveText(/^stop and restart\s+codex · .+ · \/tmp$/i);
  await expect(dialog.locator(".launch-composer-restart-warning"))
    .toHaveText("The agent is working. This action stops it before restarting.");
  const low = dialog.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true });
  expect(await enterRestartChoice(dialog, low, bodies)).toMatchObject({ stop_if_running: true, with: { effort: "low" } });
});

/** Unchanged, repeating and composing keys cannot apply a choice or bypass primary admission. */
test("restart with unchanged repeated and composing choice Enter sends nothing", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await installEnterCatalog(page);
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  const efforts = dialog.locator(".launch-composer-effort-choice");
  const high = efforts.getByRole("button", { name: "high", exact: true });
  await expect(high).toHaveAttribute("aria-pressed", "true");
  await high.focus();
  await expect(high).toBeFocused();
  await high.press("Enter");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  const low = efforts.getByRole("button", { name: "low", exact: true });
  await expect(low, "the catalog fixture must offer Low before repeat input").toBeVisible();
  for (const flags of [{ repeat: true }, { isComposing: true }]) {
    await low.focus();
    await expect(low).toBeFocused();
    const uncanceled = await low.evaluate((node, flags) => node.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Enter", code: "Enter", bubbles: true, cancelable: true, ...flags,
    })), flags);
    expect(uncanceled, "native activation must be canceled too").toBe(false);
    await expect(high).toHaveAttribute("aria-pressed", "true");
    await expect(low).toHaveAttribute("aria-pressed", "false");
  }
  expect(bodies).toHaveLength(0);
  // Positive control: the same ready fixture admits the deliberate edge.
  expect(await enterRestartChoice(dialog, low, bodies)).toMatchObject({ with: { effort: "low" } });
});

/** Model Enter uses the launcher's picker policy; a separate primary action
 * restarts the selected model, including an explicit default represented by null. */
test("restart with model Enter chooses before a separate restart action", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await page.route("**/api/launch-catalog", async (route) => fulfillAsHelm(route, { json: [
    { id: BASELINE.model, harness: "codex", efforts: ["high", "low"] },
    { id: "enter-model", harness: "codex", efforts: ["high", "low"] },
  ] }));
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  const model = dialog.getByRole("combobox", { name: "model", exact: true });
  await model.fill("enter-model");
  await expect(dialog.getByRole("option", { name: "enter-model", exact: true })).toBeVisible();
  await expect(model).toBeFocused();
  await model.press("Enter");
  await expect(model).toHaveValue("enter-model");
  expect(bodies, "choosing a model must not submit the launcher").toHaveLength(0);
  expect(await enterRestartChoice(dialog, dialog.locator(".create-session-submit"), bodies))
    .toMatchObject({ with: { model: "enter-model" } });
  // Pointer selection closes the list without restarting and retains field
  // focus. Closed-field Enter keeps the choice without submitting.
  await model.click();
  await dialog.getByRole("option", { name: "harness default", exact: true }).click();
  await expect(model).toBeFocused();
  await expect(model).toHaveValue("harness default");
  await expect(dialog.locator("#launch-composer-model-results")).toHaveCount(0);
  await model.press("Enter");
  await expect(model).toHaveValue("harness default");
  expect(bodies).toHaveLength(1);
  expect(await enterRestartChoice(dialog, dialog.locator(".create-session-submit"), bodies))
    .toMatchObject({ with: { harness: "codex" } });
  expect(bodies[1]).toHaveProperty("with.model", null);
});

/** A refused model draft cannot restart the old model, even when another setting already changed. */
test("restart with model Enter refuses an unselected draft and held keys", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await page.route("**/api/launch-catalog", async (route) => fulfillAsHelm(route, { json: [
    { id: BASELINE.model, harness: "codex", efforts: ["high", "low"] },
    { id: "other-harness-model", harness: "claude", efforts: ["high"] },
  ] }));
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  await dialog.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true }).click();
  const model = dialog.getByRole("combobox", { name: "model", exact: true });
  await model.fill("other-harness-model");
  await expect(model).toBeFocused();
  for (const flags of [{ repeat: true }, { isComposing: true }]) {
    const uncanceled = await model.evaluate((node, flags) => node.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Enter", code: "Enter", bubbles: true, cancelable: true, ...flags,
    })), flags);
    expect(uncanceled).toBe(false);
    await expect(model).toHaveValue("other-harness-model");
  }
  await model.press("Enter");
  await expect(dialog.locator(".launch-composer-choice-error"))
    .toHaveText("other-harness-model is offered by Claude; choose Claude to use it");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  const cancel = dialog.locator(".launch-composer-cancel");
  await cancel.focus();
  await expect(cancel).toBeFocused();
  await cancel.press("Enter");
  await expect(dialog).toHaveCount(0);
  expect(bodies, "the refused draft, held keys and Cancel send no restart").toHaveLength(0);
});

/** Command fields invoke primary, while an unselected YOLO radio submits the shown assertion without toggling it. */
test("restart with command Enter validates and keeps the shown YOLO value", async ({ page }) => {
  await injectCommandSession(page, "claude --resume {conversation} {farhelm_args}", "resume");
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  const command = dialog.getByLabel("agent command", { exact: true });
  await command.fill("claude --model sonnet");
  await expect(command).toBeFocused();
  await command.press("Enter");
  await expect(dialog.locator(".launch-command-error")).toContainText("{farhelm_args}");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  expect(bodies).toHaveLength(0);
  await command.fill("claude --model sonnet {farhelm_args}");
  expect(await enterRestartChoice(dialog, command, bodies)).toMatchObject({
    with_command: { command: "claude --model sonnet {farhelm_args}", yolo: false },
  });
  const yes = dialog.getByRole("radio", { name: "yes (YOLO)", exact: true });
  await expect(yes).not.toBeChecked();
  expect(await enterRestartChoice(dialog, yes, bodies)).toMatchObject({ with_command: { yolo: false } });
  await expect(yes).not.toBeChecked();
  const resume = dialog.getByLabel("resume command", { exact: true });
  await resume.fill("claude --model sonnet --resume {conversation} {farhelm_args}");
  expect(await enterRestartChoice(dialog, resume, bodies)).toMatchObject({
    with_command: { resume: "claude --model sonnet --resume {conversation} {farhelm_args}" },
  });
});

for (const controlKind of ["effort", "model"] as const) {
  /** The first submission must hand focus off before busy disables its source
   * control. Model Enter only chooses; the primary action then submits. */
  test(`restart with first ${controlKind} edit keeps focus inside while the request is held`, async ({ page }) => {
    await injectSession(page, BASELINE, "resume");
    await page.route("**/api/launch-catalog", async (route) => fulfillAsHelm(route, { json: [
      { id: BASELINE.model, harness: "codex", efforts: ["high", "low"] },
      { id: "enter-model", harness: "codex", efforts: ["high", "low"] },
    ] }));
    const gate = routeGate();
    const bodies: unknown[] = [];
    await page.route(`**/api/sessions/${encodeURIComponent(SESSION_ID)}/restart`, async (route) => {
      bodies.push(route.request().postDataJSON());
      await gate.wait();
      await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "held Enter restart refused" });
    });
    try {
      const dialog = await openInjectedDialog(page);
      const submit = dialog.locator(".create-session-submit");
      const control = controlKind === "effort"
        ? dialog.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true })
        : dialog.getByRole("combobox", { name: "model", exact: true });
      if (controlKind === "model") {
        await control.fill("enter-model");
        await expect(dialog.getByRole("option", { name: "enter-model", exact: true })).toBeVisible();
      }
      await control.focus();
      await expect(control).toBeFocused();
      await expect(submit, "the prior unchanged or pending-model render cannot submit").toBeDisabled();
      await control.press("Enter");
      if (controlKind === "model") {
        await expect(control).toHaveValue("enter-model");
        expect(bodies, "model selection alone must not submit").toHaveLength(0);
        await expect(submit).toBeEnabled();
        await submit.focus();
        await expect(submit).toBeFocused();
        await submit.press("Enter");
      }
      await expect(submit).toHaveAttribute("aria-disabled", "true");
      await expect(control, "busy must disable the control that held focus before Enter").toBeDisabled();
      // The shared primary stays focusable through aria-disabled; it owns
      // input before busy disables the choice that initiated this request.
      await expect(submit, "the surviving action owns focus before any recovery key").toBeFocused();
      await expect.poll(() => bodies.length, { message: "the held restart route must capture this request" }).toBe(1);
      await expect(dialog.locator(".launch-composer-refusal-alert"), "an in-flight request is progress").toHaveCount(0);
      expect(bodies[0]).toMatchObject({ with: controlKind === "effort" ? { effort: "low" } : { model: "enter-model" } });
      gate.release();
      await expect(dialog.locator(".launch-composer-refusal-alert")).toContainText("held Enter restart refused");
      await expect(submit).toBeFocused();
    } finally {
      gate.release();
    }
  });
}

// Restart inherits template/search/shortcut application from the launcher.
// These cases pin the new immutable-field boundary, rather than repeating
// New's layout or the template editor's persistence tests.

type RestartTemplate = { name: string; fields: Record<string, unknown> };

/** Keep template reads identified as helm data without writing the shared
 * store. Shortcut resolution and composer search consume the same reply. */
async function offerRestartTemplates(page: Page, templates: RestartTemplate[]) {
  await page.route("**/api/templates", route => fulfillAsHelm(route, { json: { templates } }));
}

/** Apply only a real offered row; a missing result is a fixture failure,
 * not evidence that a hidden held-field action was correctly refused. */
async function applyRestartTemplate(dialog: Locator, name: string) {
  const search = dialog.locator('.launch-composer-search input[role="combobox"]');
  await search.fill(`tl:${name}`);
  const option = dialog.getByRole("option", { name: `Template: ${name}`, exact: true });
  await expect(option, "the supplied template must be offered before application").toBeVisible();
  await option.click();
}

/** Partial templates make ordinary launcher features useful for resumes,
 * without carrying a launch kind or destination into the held restart. */
test("restart with applies an allowed partial agent template", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await installEnterCatalog(page);
  await offerRestartTemplates(page, [{ name: "resume-settings", fields: {
    model: "enter-model", effort: "low", permissions: null, wrapper_prefix: ["env", "RESUME_MODE=1"],
  } }]);
  const bodies = await acceptYoloRestart(page);
  const dialog = await openInjectedDialog(page);
  await applyRestartTemplate(dialog, "resume-settings");
  await expectHeldContext(dialog, "codex");
  await expect(dialog.getByRole("combobox", { name: "model", exact: true })).toHaveValue("enter-model");
  await expect(dialog.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true }))
    .toHaveAttribute("aria-pressed", "true");
  await expect(dialog.locator(".launch-composer-wrapper")).toHaveValue("env 'RESUME_MODE=1'");
  await dialog.locator(".create-session-submit").click();
  await expect(dialog).toHaveCount(0);
  expect(bodies).toEqual([{ stop_if_running: false, with: {
    harness: "codex", model: "enter-model", effort: "low", permissions: "guarded",
    wrapper_prefix: ["env", "RESUME_MODE=1"],
  } }]);
});

/** Command templates edit only command-owned fields. No declared type or
 * kind is needed to preserve the session's existing resume contract. */
test("restart with applies an allowed partial command template", async ({ page }) => {
  await injectCommandSession(page, "claude --resume {conversation} {farhelm_args}", "resume");
  const command = "claude --model sonnet {farhelm_args}";
  const resume = "claude --model sonnet --resume {conversation} {farhelm_args}";
  await offerRestartTemplates(page, [{ name: "resume-command", fields: {
    command, resume_command: resume, yolo: true,
  } }]);
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  await applyRestartTemplate(dialog, "resume-command");
  await expectHeldContext(dialog, "claude");
  await expect(dialog.getByLabel("agent command", { exact: true })).toHaveValue(command);
  await expect(dialog.getByLabel("resume command", { exact: true })).toHaveValue(resume);
  await dialog.locator(".create-session-submit").click();
  await expect(dialog.locator(".launch-composer-refusal-alert")).toContainText("Enter fixture refused restart 1");
  expect(bodies).toEqual([{ stop_if_running: false, with_command: { command, resume, yolo: true, agent: "claude" } }]);
});

for (const field of ["kind", "agent", "host", "destination", "name", "prompt"] as const) {
  /** Even a matching immutable value is an attempted field assignment.
   * Refusal must be atomic: the accompanying Low edit cannot leak through. */
  test(`restart with refuses a template's held ${field} atomically`, async ({ page, request }) => {
    await injectSession(page, BASELINE, "resume");
    await installEnterCatalog(page);
    const local = (await listHosts(request)).find(host => host.kind === "local");
    expect(local?.identity, "the host template must name a real installation").toBeTruthy();
    const held = {
      kind: "agent", agent: "codex", host: local!.identity, destination: { folder: "/tmp" }, name: TITLE,
      prompt: "an initial message cannot be sent by restart",
    };
    await offerRestartTemplates(page, [{ name: `held-${field}`, fields: { [field]: held[field], effort: "low" } }]);
    const bodies = await refuseEnterRestarts(page);
    const dialog = await openInjectedDialog(page);
    await applyRestartTemplate(dialog, `held-${field}`);
    const error = dialog.locator(".launch-composer-template-error");
    await expect(error).toContainText("cannot change here");
    await expect(error).toContainText({ kind: "launch kind", agent: "agent type", host: "host",
      destination: "destination", name: "session name", prompt: "initial prompt" }[field]);
    await expect(dialog.locator(".launch-composer-effort-choice").getByRole("button", { name: "high", exact: true }))
      .toHaveAttribute("aria-pressed", "true");
    await expect(dialog.locator(".create-session-submit")).toBeDisabled();
    await expectHeldContext(dialog, "codex");
    expect(bodies, "a refused template must not restart").toHaveLength(0);
  });
}

/** Search still offers its normal vocabulary in restart mode. Accepting a
 * held assignment must explain the refusal without changing a launchable edit;
 * foreign model ownership cannot become a back door to another agent type. */
test("restart with refuses held composer actions and scopes model search", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await page.route("**/api/launch-catalog", route => fulfillAsHelm(route, { json: [
    { id: BASELINE.model, harness: "codex", efforts: ["high", "low"] },
    { id: "held-claude-model", harness: "claude", efforts: ["high"] },
  ] }));
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  const search = dialog.locator('.launch-composer-search input[role="combobox"]');
  for (const [query, label, field] of [
    ["host:local", "Host: local (this machine)", "host"],
    ["name:held-name", "Set session name: held-name", "session name"],
    ["/tmp", "Use this path: /tmp", "destination"],
    ["claude", "Harness: Claude", "agent type"],
  ]) {
    await search.fill(query);
    // In the full suite the composer has offered /tmp twice, presumably from
    // state earlier tests leave on the shared helm. Either option is the same
    // held destination action, so the first one will do.
    const option = dialog.getByRole("option", { name: label, exact: true }).first();
    await expect(option, `premise: ${label} is offered`).toBeVisible();
    await option.click();
    await expect(dialog.locator(".launch-composer-template-error")).toContainText("cannot change here");
    await expect(dialog.locator(".launch-composer-template-error")).toContainText(field);
    await expectHeldContext(dialog, "codex");
    await expect(dialog.locator(".create-session-submit")).toBeDisabled();
    // An allowed equal-value action clears the previous refusal. Each next
    // held result must therefore produce its own reason, not inherit one.
    await search.fill("effort:high");
    const high = dialog.getByRole("option", { name: "Effort: high", exact: true });
    await expect(high).toBeVisible();
    await high.click();
    await expect(dialog.locator(".launch-composer-template-error")).toHaveCount(0);
  }
  await search.fill("held-claude-model");
  await expect(dialog.getByRole("option", { name: "Set session name: held-claude-model", exact: true })).toBeVisible();
  await expect(dialog.getByRole("option", { name: "Model: held-claude-model (Claude)", exact: true })).toHaveCount(0);
  expect(bodies).toHaveLength(0);
  // Positive control: a permitted search choice still edits this restart.
  await search.fill("effort:low");
  const low = dialog.getByRole("option", { name: "Effort: low", exact: true });
  await expect(low).toBeVisible();
  await low.click();
  await expect(dialog.locator(".create-session-submit")).toBeEnabled();
  await dialog.locator(".create-session-submit").click();
  await expect(dialog.locator(".launch-composer-refusal-alert")).toContainText("Enter fixture refused restart 1");
  expect(bodies).toEqual([{ stop_if_running: false, with: {
    harness: "codex", model: BASELINE.model, effort: "low", permissions: null,
  } }]);
});

/** Template shortcut batches are one application. A later held assignment
 * must not leave an earlier allowed edit behind, and a direct bolt chord
 * must obey the same boundary instead of creating a fresh session. */
test("restart with refuses held shortcut batches and bolt templates", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await installEnterCatalog(page);
  await offerRestartTemplates(page, [
    { name: "allowed-low", fields: { effort: "low" } },
    { name: "held-name", fields: { name: TITLE } },
    { name: "held-bolt", fields: { kind: "agent", agent: "codex", effort: "low" } },
  ]);
  await page.route("**/api/preferences", async route => {
    if (route.request().method() !== "GET") return route.continue();
    const response = await route.fetch();
    await route.fulfill({ response, json: { ...await response.json(), quick_launch_template: "held-bolt",
      template_shortcuts: { "1": "allowed-low", "2": "held-name" } } });
  });
  const creates: unknown[] = [];
  page.on("request", sent => {
    if (sent.method() === "POST" && sent.url().endsWith("/api/sessions")) creates.push(sent.postDataJSON());
  });
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  const cancel = dialog.locator(".launch-composer-cancel");
  await expect(cancel).toBeFocused();
  await page.keyboard.press("Control+Shift+KeyX");
  await expect(page.locator(".launch-sequence-indicator")).toContainText("apply");
  await page.keyboard.press("Digit1");
  await page.keyboard.press("Digit2");
  await page.keyboard.press("Enter");
  await expect(page.locator(".launch-sequence-indicator")).toHaveCount(0);
  await expect(dialog.locator(".launch-composer-template-error")).toContainText("session name cannot change here");
  await expect(dialog.locator(".launch-composer-effort-choice").getByRole("button", { name: "high", exact: true }))
    .toHaveAttribute("aria-pressed", "true");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  await cancel.focus();
  await page.keyboard.press("Control+Shift+KeyP");
  await expect(dialog.locator(".launch-composer-template-error")).toContainText("launch kind cannot change here");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  await expectHeldContext(dialog, "codex");
  expect(creates, "restart shortcuts cannot start New").toHaveLength(0);
  expect(bodies).toHaveLength(0);
});

for (const editCommand of [false, true]) {
  /** Escaped peer display is not a new shell command. An untouched field
   * retains its original bytes even when another command field is replaced. */
  test(`restart with preserves raw command bytes${editCommand ? " beside an edited command" : " for a YOLO-only edit"}`, async ({ page }) => {
    const command = "claude  --tag 'literal value'\u200b {farhelm_args}  ";
    const resume = "claude   --resume {conversation} --tag '\u202eold' {farhelm_args} ";
    await injectCommandSession(page, resume, "resume", command);
    const bodies = await refuseEnterRestarts(page);
    const dialog = await openInjectedDialog(page);
    const commandField = dialog.getByLabel("agent command", { exact: true });
    await expect(commandField).toHaveValue("claude  --tag 'literal value'<U+200B> {farhelm_args}  ");
    await expect(dialog.getByLabel("resume command", { exact: true }))
      .toHaveValue("claude   --resume {conversation} --tag '<U+202E>old' {farhelm_args} ");
    const edited = "claude --model sonnet {farhelm_args}";
    await expect(commandField).toHaveJSProperty("readOnly", true);
    await expect(dialog.getByLabel("resume command", { exact: true })).toHaveJSProperty("readOnly", true);
    if (editCommand) {
      await dialog.getByRole("button", { name: "clear agent command", exact: true }).click();
      await expect(commandField).toHaveValue("");
      await expect(commandField).toHaveJSProperty("readOnly", false);
      await commandField.fill(edited);
    }
    await dialog.getByRole("radio", { name: "yes (YOLO)", exact: true }).check();
    await dialog.locator(".create-session-submit").click();
    await expect(dialog.locator(".launch-composer-refusal-alert")).toContainText("Enter fixture refused restart 1");
    expect(bodies).toEqual([{ stop_if_running: false, with_command: {
      command: editCommand ? edited : command, resume, yolo: true, agent: "claude",
    } }]);
  });
}

/** A recent setup owns destination and agent fields even when they happen
 * to match this session. It cannot partially apply Low or supply a remembered
 * folder to a restart whose source already owns its directory. */
test("restart with refuses a matching recent and ignores folder memory", async ({ page, request }) => {
  await injectSession(page, { ...BASELINE, effort: null }, "resume");
  await installEnterCatalog(page);
  const local = (await listHosts(request)).find(host => host.kind === "local");
  expect(local?.identity, "history must identify the real local installation").toBeTruthy();
  const recent = { host: local!.id, canonical_cwd: "/tmp", cwd: "/tmp",
    selection: { ...BASELINE, effort: "low" }, created_at: 1, creation_seq: 1 };
  await page.route("**/api/launch-history**", route => fulfillAsHelm(route, { json: {
    launches: [{ ...recent, cwd: "/remembered/other-folder", canonical_cwd: "/remembered/other-folder",
      created_at: 2, creation_seq: 2 }, recent], folders: [],
  } }));
  const writes: string[] = [];
  page.on("request", sent => {
    if (["POST", "PUT", "PATCH"].includes(sent.method()) && !sent.url().endsWith("/restart")) writes.push(sent.url());
  });
  const bodies = await acceptYoloRestart(page);
  const dialog = await openInjectedDialog(page);
  await expectHeldContext(dialog, "codex");
  await expect(dialog.locator(".launch-composer-recents"), "direct recents would expose held destination controls")
    .toHaveCount(0);
  await dialog.locator('.launch-composer-search input[role="combobox"]').fill("recent:/tmp");
  const offered = dialog.getByRole("option").filter({ hasText: "Recent setup: /tmp" });
  await expect(offered, "the matching recent must really be offered").toHaveCount(1);
  await offered.click();
  await expect(dialog.locator(".launch-composer-template-error")).toContainText("cannot change here");
  await expect(dialog.locator(".launch-composer-effort-choice").getByRole("button", { name: "default", exact: true }))
    .toHaveAttribute("aria-pressed", "true");
  await expect(dialog.locator(".create-session-submit")).toBeDisabled();
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
  await dialog.locator(".create-session-submit").click();
  await expect(dialog).toHaveCount(0);
  expect(bodies).toHaveLength(1);
  expect(bodies[0]).not.toHaveProperty("cwd");
  expect(bodies[0]).not.toHaveProperty("intent_key");
  expect(writes.filter(url => /launch-history|\/api\/sessions$/.test(url)),
    "restart cannot record a new setup or create a session to update folder defaults").toEqual([]);
});

/** Retry belongs to the shared catalog reader, not a new dialog opening.
 * Recovering inventory must preserve both the held source and prior edits. */
test("restart with retries the catalog without reseeding its draft", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  let reads = 0;
  await page.route("**/api/launch-catalog", async route => {
    reads += 1;
    if (reads === 1) {
      await fulfillAsHelm(route, { status: 503, contentType: "text/plain", body: "catalog unavailable for restart fixture" });
    } else {
      await fulfillAsHelm(route, { json: [{ id: BASELINE.model, harness: "codex", efforts: ["high", "low"] }] });
    }
  });
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  const failure = dialog.locator(".create-catalog-error");
  await expect(failure).toContainText("model catalog unavailable");
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
  await expect(dialog.locator(".create-session-submit")).toBeEnabled();
  await failure.getByRole("button", { name: "retry", exact: true }).click();
  await expect.poll(() => reads, { message: "catalog retry reached the controlled reader" }).toBe(2);
  await expect(failure).toHaveCount(0);
  await expectHeldContext(dialog, "codex");
  await expect(dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }))
    .toHaveAttribute("aria-pressed", "true");
  await expect(dialog.locator(".create-session-submit")).toBeEnabled();
  await dialog.locator(".create-session-submit").click();
  const refusal = dialog.locator(".launch-composer-refusal-alert");
  await expect(refusal).toContainText("Restart error.");
  await expect(refusal).toContainText("Enter fixture refused restart 1");
  expect(bodies).toHaveLength(1);
  expect(bodies[0]).toMatchObject({ with: { model: BASELINE.model, effort: "high", permissions: "guarded" } });
  expect(bodies[0]).not.toHaveProperty("intent_key");
});

/** Consent follows the action shown now, rather than the working state when
 * the editor opened. A real stop and subsequent detail refresh establish the
 * transition before the edited restart request is intercepted. */
test("restart with withdraws working-agent consent when its source exits", async ({ page, request }) => {
  let id: string | undefined;
  const bodies: any[] = [];
  try {
    const source = await createTabSession(request, `restart-with-consent-${Date.now()}`);
    id = source.id;
    await presentAsResumable(page, id);
    await page.route(`**/api/sessions/${encodeURIComponent(id)}/restart`, async route => {
      bodies.push(route.request().postDataJSON());
      await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "the agent is working now; confirm stopping it" });
    });
    await page.goto("/");
    await attachSession(page, id);
    await page.keyboard.type("busy");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "busy-tick-");
    await expect(page.locator(".restart-primary")).toHaveAttribute("data-confirms", "true", { timeout: 15_000 });
    await page.locator(".restart-with-trigger").click();
    const dialog = page.locator(".create-session-form[data-launcher-mode=restart]");
    await expect(dialog.locator(".launch-composer-cancel")).toBeFocused();
    const submit = dialog.locator(".create-session-submit");
    await expect(submit).toHaveText(/^stop and restart\s+/);
    await stopSession(request, id);
    await expect(submit).toHaveText(/^restart\s+/, { timeout: 20_000 });
    expect(bodies, "the editor has not consented while source state changed").toHaveLength(0);
    await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
    await expect(submit).toBeEnabled();
    await submit.click();
    await expect(dialog.locator(".launch-composer-refusal-alert")).toContainText("the agent is working now; confirm stopping it");
    expect(bodies).toHaveLength(1);
    expect(bodies[0].stop_if_running, "an exited-render action cannot authorize an unseen working agent").toBe(false);
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }))
      .toHaveAttribute("aria-pressed", "true");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});
