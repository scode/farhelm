/**
 * Restart with edits a session's stored launch choice while keeping its conversation.
 *
 * An injected resumable row gives the UI an offer that the fake agent cannot
 * produce on demand. The real listing supplies host metadata; restart answers
 * are controlled so assertions stay on browser consent and request contents.
 * Cases that require particular model or effort choices also supply a stamped
 * catalog, keeping that fixture premise independent of the shipped inventory.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator, type Page } from "@playwright/test";
import { agentLaunchRow, commandLaunchRow, SESSION_LISTING } from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { attachSession, cleanupSession } from "./helpers/term";
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
  permissions: null,
  workspace_trust: null,
};

/** Restart refusal text can quote a peer's hidden characters. Its display
 * must expose them and isolate direction while the editable dialog stays open.
 * Restart requires an edited launch choice; an unchanged draft never submits
 * the request whose refusal this test is meant to observe. */
test("Restart with escapes and isolates peer refusal text", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
    await fulfillAsHelm(route, { status: 400, contentType: "text/plain", body: "refused \u202Ehost\u200B" });
  });
  const dialog = await openInjectedDialog(page);
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  const submit = dialog.locator(".restart-with-submit");
  await expect(submit).toBeEnabled();
  await submit.click();
  const error = dialog.locator(".restart-with-error .peer-value");
  await expect(error).toHaveText("refused <U+202E>host<U+200B>");
  await expect(error).toHaveAttribute("dir", "ltr");
  expect(await error.evaluate((node) => getComputedStyle(node).unicodeBidi)).toBe("isolate");
  await expect(dialog).toBeVisible();
});

/**
 * Publish one controlled row through the real listing response.
 *
 * Copying the stack's existing row keeps host and transport metadata valid;
 * the test changes only the state whose offer and stored selection matter.
 */
async function injectSession(page: Page, launch: typeof BASELINE | null, offer: string, state = "interrupted") {
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const listing = await response.json();
    const source = listing.sessions[0];
    if (!source) throw new Error("the shared stack must list a source session");
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
          : agentLaunchRow(launch),
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
  await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
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
  await page.locator(".restart-with-trigger").click();
  const dialog = page.locator(".restart-with-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".restart-with-cancel")).toBeFocused();
  return dialog;
}

/**
 * Cancel must be side effect free, and consent must carry the edited choice once.
 *
 * The reply exercises the ordinary restart success path, including closing the
 * modal, while the request body proves the UI did not create a fresh session.
 */
test("restart with shows fixed context and submits one edited resume", async ({ page }) => {
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

  const dialog = page.locator(".restart-with-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText(`restart with · ${TITLE}`);
  const context = dialog.locator(".restart-with-context");
  await expect(context.locator("dt")).toHaveText(["harness", "host", "folder", "resumes"]);
  await expect(context.locator("dd").nth(0)).toHaveText("codex");
  await expect(context.locator("dd").nth(1)).not.toBeEmpty();
  await expect(context.locator("dd").nth(2)).toHaveText("/tmp");
  await expect(context.locator("dd").nth(3)).toHaveText("this session's conversation");
  await expect(dialog).toContainText("harness, host and folder stay fixed; use replace with for another harness or folder, or clone for another host");

  const submit = dialog.locator(".restart-with-submit");
  await expect(submit).toBeDisabled();
  await dialog.locator(".restart-with-cancel").click();
  await expect(dialog).toHaveCount(0);
  expect(bodies, "cancel must not send a restart").toEqual([]);

  await trigger.click();
  await expect(dialog).toBeVisible();
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true }).click();
  await expect(dialog.locator(".launch-composer-permissions-choice .launch-composer-changed-marker"))
    .toHaveText("● changed · was default");
  await expect(submit).toBeEnabled();
  await expect(submit).toHaveText("restart");
  await submit.click();
  await expect(dialog).toHaveCount(0);
  expect(bodies, "one consent must send one restart request").toEqual([{
    stop_if_running: false,
    with: { harness: "codex", model: "gpt-6-astra", effort: "high", permissions: "yolo" },
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
  await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
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
  await dialog.locator(".restart-with-submit").click();
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
  await expect(dialog.locator(".restart-with-cancel")).toBeFocused();
  expect(bodies, "declining must not send anything").toHaveLength(1);

  // Asking again and confirming from the keyboard sends the override.
  await dialog.locator(".restart-with-submit").click();
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
  await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
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
  await dialog.locator(".restart-with-submit").click();
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
    await expect(dialog.locator(".restart-with-submit"), "focus stays in the dialog").toBeFocused();
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
  await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
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
    candidate.request().method() === "POST" && candidate.url().endsWith(`/api/sessions/${SESSION_ID}/restart`);
  await Promise.all([page.waitForResponse(isRestart), dialog.locator(".restart-with-submit").click()]);
  await expect(confirmation).toBeVisible();
  await expect(confirmation.locator(".yolo-cancel")).toBeFocused();

  for (const answer of [".yolo-confirm", ".yolo-confirm-stop-asking"]) {
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

    // A plain restart afterwards: a restart the stale answer had approved
    // would show up below as a body carrying the override (and a 200 reply),
    // and this one must be asked about again rather than ride on it.
    const [again] = await Promise.all([
      page.waitForResponse(isRestart),
      dialog.locator(".restart-with-submit").click(),
    ]);
    expect(again.status(), `after cancel then ${answer}, the restart is not approved`).toBe(409);
    await expect(confirmation).toBeVisible();
    await expect(confirmation.locator(".yolo-cancel")).toBeFocused();
  }
  expect(
    restarts.map((body) => body.confirm_yolo ?? false),
    "no restart after a burst carries the override",
  ).toEqual([false, false, false]);
  expect(marks, "the host is never marked").toHaveLength(0);

  // Positive control: a genuine answer restarts with the override.
  await confirmation.locator(".yolo-confirm").click();
  await expect(dialog).toHaveCount(0);
  expect(restarts).toHaveLength(4);
  expect(restarts[3]).toMatchObject({
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
  await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
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
  await expect(page.locator(".restart-with-dialog")).toHaveCount(0);
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
  await page.route((url) => url.pathname === `/api/sessions/${id}`, async (route) => {
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
    const dialog = document.querySelector(".restart-with-dialog");
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
  await page.route(`**/api/sessions/${id}/restart`, async (route) => {
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
    const dialog = page.locator(".restart-with-dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".restart-with-cancel")).toBeFocused();
    await armTerminalFocusSpy(page);

    await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true })
      .click();
    const submit = dialog.locator(".restart-with-submit");
    await expect(submit).toBeEnabled();
    await submit.click();

    // Held: the request reached the fixture and the dialog rendered busy.
    await expect.poll(() => bodies.length).toBe(1);
    await expect(submit).toHaveAttribute("aria-disabled", "true");
    await expect(dialog.locator(".restart-with-cancel")).toBeDisabled();
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
    await expect(dialog.locator(".restart-with-error")).toContainText(refusal);
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
    const dialog = page.locator(".restart-with-dialog");
    const cancel = dialog.locator(".restart-with-cancel");
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
  const dialog = page.locator(".restart-with-dialog");
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
 * control (Codex's effort `default`), that a full forward Tab cycle from there
 * wraps back to the model field inside the dialog at every step, and that
 * Shift+Tab wraps from the model field to the last stop and returns to it.
 */
test("restart with keeps Tab from the open model list inside the dialog", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  await expect(row).toBeVisible();
  await row.locator(".session-row-open").click();
  await page.locator(".restart-with-trigger").click();
  const dialog = page.locator(".restart-with-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".restart-with-cancel")).toBeFocused();

  const model = dialog.getByRole("combobox", { name: "model", exact: true });
  const listbox = dialog.getByRole("listbox");
  const effortDefault = dialog.locator(".launch-composer-effort-choice").getByRole("button", {
    name: "default",
    exact: true,
  });
  const cancel = dialog.locator(".restart-with-cancel");
  // Premise: the list is open with at least one transient row, so the row
  // that used to be Tab's target exists when Tab is pressed.
  await model.focus();
  await expect(model).toBeFocused();
  await expect(listbox).toBeVisible();
  await expect(listbox.getByRole("option").first()).toBeVisible();
  // Premise: with no edit the submit is natively disabled, so cancel is the
  // trap's last tab stop for the Shift+Tab wrap below.
  await expect(dialog.locator(".restart-with-submit")).toBeDisabled();

  await page.keyboard.press("Tab");
  await expect(listbox).toHaveCount(0);
  await expect(effortDefault).toBeFocused();

  // Walk forward from there until the trap's wrap brings focus back to the
  // model field, through every later control and the wrap from cancel. The
  // bound is generous for this dialog's handful of controls; running out means
  // focus never came back, which the assertion after the loop reports.
  let presses = 0;
  let returned = false;
  while (presses < 20 && !returned) {
    await page.keyboard.press("Tab");
    presses += 1;
    expect(await focusInsideDialog(page), `focus after forward Tab ${presses}`).toBe(true);
    returned = await model.evaluate((node) => node === document.activeElement);
  }
  expect(returned, "a forward Tab cycle must wrap back to the model field").toBe(true);
  expect(presses, "the cycle must pass the later controls, not wrap at once").toBeGreaterThan(2);
  await expect(listbox).toBeVisible();

  // Backward: the model field is the trap's first stop, so Shift+Tab from it
  // (list open) wraps to cancel; Tab wraps back, and Shift+Tab from the next
  // control returns to the model field.
  await page.keyboard.press("Shift+Tab");
  await expect(listbox).toHaveCount(0);
  await expect(cancel).toBeFocused();
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
  expect(hit, "premise: the click point must be the backdrop").toBe("restart-with-backdrop");

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
  await page.route(`**/api/sessions/${id}/restart`, async (route) => {
    bodies.push(route.request().postDataJSON());
    await route.abort();
  });

  await page.goto("/");
  // Premise: a real terminal is attached and revealed beneath the dialog.
  await attachSession(page, id);
  const trigger = page.locator(".restart-with-trigger");
  await expect(trigger).not.toHaveAttribute("aria-disabled", "true");
  await trigger.click();
  const dialog = page.locator(".restart-with-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".restart-with-cancel")).toBeFocused();
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
 * not inert while the header action, the session row and the sidebar are;
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

  // An element the page made inert on its own, which the dialog must leave
  // alone. The sidebar sits beside the main pane, so the dialog's walk up
  // its ancestors meets it as a sibling.
  await page.evaluate(() => {
    const sidebar = document.querySelector(".app-sidebar") as HTMLElement;
    sidebar.dataset.inertFixture = "sidebar";
    sidebar.setAttribute("inert", "");
  });
  const inertSet = () =>
    page.evaluate(() =>
      [...document.querySelectorAll("[inert]")].map((node) =>
        (node as HTMLElement).dataset.inertFixture ?? `${node.tagName}.${node.className}`
      )
    );
  expect(await inertSet(), "premise: only the fixture's own inert element").toEqual(["sidebar"]);
  const underInert = () =>
    page.evaluate((sessionId) => {
      const inside = (selector: string) => document.querySelector(selector)?.closest("[inert]") != null;
      return {
        dialog: inside(".restart-with-dialog"),
        trigger: inside(".restart-with-trigger"),
        row: inside(`[data-session-id="${sessionId}"]`),
        sidebar: inside(".app-sidebar"),
      };
    }, SESSION_ID);

  const dialog = page.locator(".restart-with-dialog");
  const cancel = dialog.locator(".restart-with-cancel");
  await trigger.click();
  await expect(cancel).toBeFocused();
  expect(
    await page.evaluate(() => {
      const sidebar = document.querySelector(".app-sidebar")!;
      const dialog = document.querySelector(".restart-with-dialog")!;
      return !sidebar.contains(dialog) && sidebar.parentElement!.contains(dialog);
    }),
    "premise: the sidebar is a sibling on the dialog's ancestor path",
  ).toBe(true);
  expect(await underInert()).toEqual({ dialog: false, trigger: true, row: true, sidebar: true });

  await cancel.click();
  await expect(dialog).toHaveCount(0);
  await expect(trigger).toBeFocused();
  expect(await inertSet(), "after cancel").toEqual(["sidebar"]);

  await trigger.click();
  await expect(cancel).toBeFocused();
  expect(await underInert()).toEqual({ dialog: false, trigger: true, row: true, sidebar: true });
  await dialog.locator(".launch-composer-permissions-choice").getByRole("button", { name: "yolo", exact: true })
    .click();
  await dialog.locator(".restart-with-submit").click();
  await expect(dialog).toHaveCount(0);
  expect(bodies, "premise: the dialog closed through a successful restart").toHaveLength(1);
  expect(await inertSet(), "after a successful restart").toEqual(["sidebar"]);
});

/**
 * Older default-YOLO sessions stored an omitted permission. Pressing their
 * already-selected YOLO button must not invent an edit or enable a restart.
 */
test("restart with compares an older omitted permission by its effective mode", async ({ page }) => {
  await injectSession(page, { ...BASELINE, harness: "omp", model: null, effort: null }, "resume");
  await page.goto("/");
  const row = page.locator(`[data-session-id="${SESSION_ID}"]`);
  await expect(row).toBeVisible();
  await row.locator(".session-row-open").click();
  await page.locator(".restart-with-trigger").click();
  const dialog = page.locator(".restart-with-dialog");
  await expect(dialog).toBeVisible();
  const permissions = dialog.locator(".launch-composer-permissions-choice");
  const yolo = permissions.getByRole("button", { name: "yolo", exact: true });
  await expect(yolo).toHaveAttribute("aria-pressed", "true");
  await expect(dialog.locator(".restart-with-submit")).toBeDisabled();
  // A real edit establishes that compatibility permits submission. Returning
  // to YOLO must then disable it, giving the negative marker check a completed
  // state transition to observe rather than racing the click's render.
  await permissions.getByRole("button", { name: "approve", exact: true }).click();
  await expect(dialog.locator(".restart-with-submit")).toBeEnabled();
  await yolo.click();
  await expect(dialog.locator(".restart-with-submit")).toBeDisabled();
  await expect(permissions.locator(".launch-composer-changed-marker")).toHaveCount(0);
  await expect(dialog.locator(".restart-with-submit")).toBeDisabled();
});

/**
 * Publish one command-launch row through the real listing, the command-launch
 * counterpart of `injectSession`: `resume` null makes a command launch that
 * cannot resume.
 */
async function injectCommandSession(page: Page, resume: string | null, offer: string) {
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const listing = await response.json();
    const source = listing.sessions[0];
    if (!source) throw new Error("the shared stack must list a source session");
    listing.sessions.push({
      ...source,
      id: SESSION_ID,
      title: TITLE,
      cwd: "/tmp",
      invocation: "claude {farhelm_args}",
      launch: commandLaunchRow("claude {farhelm_args}", {
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
  await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
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
  const context = dialog.locator(".restart-with-context");
  await expect(context.locator("dt")).toHaveText(["launch", "agent type", "host", "folder", "resumes"]);
  await expect(context.locator("dd").nth(0)).toHaveText("command");
  await expect(context.locator("dd").nth(1)).toHaveText("claude");

  const command = dialog.locator(".restart-with-command-input");
  const submit = dialog.locator(".restart-with-submit");
  await expect(command).toHaveValue("claude {farhelm_args}");
  await expect(dialog.locator(".restart-with-resume-input")).toHaveValue(resume);
  await expect(
    dialog.getByRole("group", { name: "runs without approval prompts" }).getByLabel("no", { exact: true }),
  ).toBeChecked();
  await expect(submit).toBeDisabled();
  await expect(dialog.locator(".restart-with-edited")).toHaveCount(0);

  // An edit that breaks a create's rules is explained and cannot be sent.
  await command.fill("claude --model sonnet");
  await expect(dialog.locator(".restart-with-command-error")).toContainText("{farhelm_args}");
  await expect(submit).toBeDisabled();
  await command.fill("claude --model sonnet {farhelm_args}");
  await expect(dialog.locator(".restart-with-command-error")).toHaveCount(0);
  await expect(dialog.locator(".restart-with-edited")).toHaveCount(1);
  await expect(submit).toBeEnabled();
  // The other two fields reach the request too, each marked once edited.
  const editedResume = "claude --model sonnet --resume {conversation} {farhelm_args}";
  await dialog.locator(".restart-with-resume-input").fill(editedResume);
  await expect(dialog.locator(".restart-with-edited")).toHaveCount(2);
  await dialog
    .getByRole("group", { name: "runs without approval prompts" })
    .getByLabel("yes (YOLO)", { exact: true })
    .check();
  await expect(dialog.locator(".restart-with-edited")).toHaveCount(3);
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
  await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
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
    .getByRole("group", { name: "runs without approval prompts" })
    .getByLabel("yes (YOLO)", { exact: true })
    .check();
  await dialog.locator(".restart-with-submit").click();
  const confirmation = dialog.locator(".yolo-confirmation");
  await expect(confirmation, "the question must appear inside the modal dialog").toBeVisible();
  await expect(confirmation).toContainText("This command was asserted to run without approval prompts.");
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
  await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
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
  const submit = dialog.locator(".restart-with-submit");
  const submitWasDisabled = await submit.isDisabled();
  await expect(control, "the Enter fixture must offer the choice before input").toBeVisible();
  await control.focus();
  await expect(control).toBeFocused();
  await control.press("Enter");
  await expect(dialog.locator(".restart-with-error")).toHaveText(`Enter fixture refused restart ${before + 1}`);
  await expect(dialog.locator(".restart-with-submit")).not.toHaveAttribute("aria-disabled", "true");
  // A first choice can still see the unchanged render's disabled submit.
  // Either surviving target is valid; body or an agent control is not.
  await expect.poll(() => dialog.evaluate((node) => node.contains(document.activeElement)), {
    message: "focus must survive the primary action inside the restart dialog",
  }).toBe(true);
  if (!submitWasDisabled) await expect(submit).toBeFocused();
  expect(bodies, "one deliberate Enter sends one ordinary restart").toHaveLength(before + 1);
  return bodies[before];
}

/** A choice must reach the restart in the same key event, rather than sending the previous rendered edit. */
test("restart with choice Enter applies effort permissions and trust before restarting", async ({ page }) => {
  await injectSession(page, BASELINE, "resume");
  await installEnterCatalog(page);
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  const choice = (group: string, name: string) => dialog.locator(group).getByRole("button", { name, exact: true });
  expect(await enterRestartChoice(dialog, choice(".launch-composer-effort-choice", "low"), bodies))
    .toMatchObject({ stop_if_running: false, with: { harness: "codex", model: BASELINE.model, effort: "low" } });
  expect(await enterRestartChoice(dialog, choice(".launch-composer-permissions-choice", "yolo"), bodies))
    .toMatchObject({ with: { effort: "low", permissions: "yolo" } });
  expect(await enterRestartChoice(dialog, choice(".launch-composer-trust-choice", "true"), bodies))
    .toMatchObject({ with: { effort: "low", permissions: "yolo", workspace_trust: true } });
});

/** Enter confirms exactly the displayed stop-and-restart warning for a working agent. */
test("restart with choice Enter carries the displayed working-agent confirmation", async ({ page }) => {
  await injectSession(page, BASELINE, "resume", "running");
  await installEnterCatalog(page);
  const bodies = await refuseEnterRestarts(page);
  const dialog = await openInjectedDialog(page);
  await expect(dialog.locator(".restart-with-submit")).toHaveText("stop and restart");
  await expect(dialog.locator(".restart-with-stop-note")).toHaveText("agent is working; it is stopped first");
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
  await expect(dialog.locator(".restart-with-submit")).toBeDisabled();
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

/** The model field's Restart with policy chooses and restarts; New session's policy remains separate. */
test("restart with model Enter chooses and restarts from open and closed fields", async ({ page }) => {
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
  expect(await enterRestartChoice(dialog, model, bodies)).toMatchObject({ with: { model: "enter-model" } });
  // Pointer selection closes the list without restarting and retains field
  // focus. Closed-field Enter must perform primary without reviving its query.
  await model.click();
  await dialog.getByRole("option", { name: "harness default", exact: true }).click();
  await expect(model).toBeFocused();
  await expect(model).toHaveValue("harness default");
  await expect(dialog.locator("#restart-with-model-results")).toHaveCount(0);
  expect(await enterRestartChoice(dialog, model, bodies)).toMatchObject({ with: { harness: "codex" } });
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
  await expect(dialog.locator(".launch-composer-choice-error")).toHaveText("choose a model for this harness");
  await expect(dialog.locator(".restart-with-submit")).toBeDisabled();
  const cancel = dialog.locator(".restart-with-cancel");
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
  const command = dialog.locator(".restart-with-command-input");
  await command.fill("claude --model sonnet");
  await expect(command).toBeFocused();
  await command.press("Enter");
  await expect(dialog.locator(".restart-with-command-error")).toContainText("{farhelm_args}");
  await expect(dialog.locator(".restart-with-submit")).toBeDisabled();
  expect(bodies).toHaveLength(0);
  await command.fill("claude --model sonnet {farhelm_args}");
  expect(await enterRestartChoice(dialog, command, bodies)).toMatchObject({
    with_command: { command: "claude --model sonnet {farhelm_args}", yolo: false },
  });
  const yes = dialog.getByRole("radio", { name: "yes (YOLO)", exact: true });
  await expect(yes).not.toBeChecked();
  expect(await enterRestartChoice(dialog, yes, bodies)).toMatchObject({ with_command: { yolo: false } });
  await expect(yes).not.toBeChecked();
  const resume = dialog.locator(".restart-with-resume-input");
  await resume.fill("claude --model sonnet --resume {conversation} {farhelm_args}");
  expect(await enterRestartChoice(dialog, resume, bodies)).toMatchObject({
    with_command: { resume: "claude --model sonnet --resume {conversation} {farhelm_args}" },
  });
});

for (const controlKind of ["effort", "model"] as const) {
  /** The first changed Enter must hand focus off before busy disables its source control. */
  test(`restart with first ${controlKind} Enter keeps focus inside while the request is held`, async ({ page }) => {
    await injectSession(page, BASELINE, "resume");
    await page.route("**/api/launch-catalog", async (route) => fulfillAsHelm(route, { json: [
      { id: BASELINE.model, harness: "codex", efforts: ["high", "low"] },
      { id: "enter-model", harness: "codex", efforts: ["high", "low"] },
    ] }));
    const gate = routeGate();
    const bodies: unknown[] = [];
    await page.route(`**/api/sessions/${SESSION_ID}/restart`, async (route) => {
      bodies.push(route.request().postDataJSON());
      await gate.wait();
      await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "held Enter restart refused" });
    });
    try {
      const dialog = await openInjectedDialog(page);
      const submit = dialog.locator(".restart-with-submit");
      const control = controlKind === "effort"
        ? dialog.locator(".launch-composer-effort-choice").getByRole("button", { name: "low", exact: true })
        : dialog.getByRole("combobox", { name: "model", exact: true });
      if (controlKind === "model") {
        await control.fill("enter-model");
        await expect(dialog.getByRole("option", { name: "enter-model", exact: true })).toBeVisible();
      }
      await control.focus();
      await expect(control).toBeFocused();
      await expect(submit, "the prior unchanged or pending-model render cannot receive focus").toBeDisabled();
      await control.press("Enter");
      await expect(submit).toHaveAttribute("aria-disabled", "true");
      await expect(control, "busy must disable the control that held focus before Enter").toBeDisabled();
      await expect(dialog, "the surviving container owns focus before any recovery key").toBeFocused();
      await expect.poll(() => bodies.length, { message: "the held restart route must capture this request" }).toBe(1);
      expect(bodies[0]).toMatchObject({ with: controlKind === "effort" ? { effort: "low" } : { model: "enter-model" } });
      gate.release();
      await expect(dialog.locator(".restart-with-error")).toHaveText("held Enter restart refused");
      await expect(dialog).toBeFocused();
    } finally {
      gate.release();
    }
  });
}
