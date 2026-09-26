/**
 * Restart with edits a session's stored launch choice while keeping its conversation.
 *
 * An injected resumable row gives the UI an offer that the fake agent cannot
 * produce on demand. Only its restart route is mocked; the real listing supplies
 * host metadata and the real controls supply their model catalog. This keeps the
 * assertion on the browser's consent and request, without relaunching an agent.
 */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { SESSION_LISTING } from "./helpers/fleet";
import { routeGate } from "./helpers/route-gate";
import { attachSession, cleanupSession } from "./helpers/term";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";
import {
  addTab,
  createTabSession,
  fulfillAsHelm,
  installTerminalSuiteHooks,
  selectTerminal,
} from "./helpers/terminal-suite";

// The tab sweep removes the scratch working directory `createTabSession` makes
// for the tab-fallback focus test's own session.
installTerminalSuiteHooks({ tabSweep: true });

const SESSION_ID = "77777777-2222-3333-4444-555555555555";
const TITLE = "restart-with-browser-fixture";
const BASELINE = {
  harness: "codex",
  model: "gpt-6-astra",
  effort: "high",
  permissions: null,
  workspace_trust: null,
};

/**
 * Publish one controlled row through the real listing response.
 *
 * Copying the stack's existing row keeps host and transport metadata valid;
 * the test changes only the state whose offer and stored selection matter.
 */
async function injectSession(page: Page, launch: typeof BASELINE | null, offer: string) {
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
      launch,
      status: { state: "interrupted" },
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
        launch: { ...BASELINE, permissions: "yolo" },
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
  await expect(row).toBeVisible();
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
    mode: "resume",
    stop_if_running: false,
    with: { harness: "codex", model: "gpt-6-astra", effort: "high", permissions: "yolo", workspace_trust: null },
  }]);
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
  const reason = "restart with needs a session launched from structured settings";
  await expect(trigger).toBeVisible();
  await expect(trigger).toHaveAttribute("aria-disabled", "true");
  await trigger.hover();
  await expect(trigger).toHaveAttribute("title", reason);
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
    launch: BASELINE,
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

/**
 * Count every focus that enters the primary terminal once the test arms it.
 *
 * This is the mechanism the focus tests are about, recorded by the page at the
 * moment it happens: a reveal or a selection change that focuses the agent
 * behind the modal fires `focusin` on xterm's helper textarea whether or not a
 * later observation happens to catch it there.
 */
async function installTerminalFocusSpy(page: Page) {
  await page.addInitScript(() => {
    const spy = { armed: false, count: 0 };
    (window as any).__restartWithTerminalFocus = spy;
    document.addEventListener(
      "focusin",
      (event) => {
        if (spy.armed && event.target instanceof Element && event.target.closest("#terminal")) {
          spy.count += 1;
        }
      },
      true,
    );
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
    await page.evaluate(() => {
      (window as any).__restartWithTerminalFocus.armed = true;
    });

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
      await page.evaluate(() => (window as any).__restartWithTerminalFocus.count),
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
    await waitForSessionRevealed(page, id, { tabId });
    // Premise: the tab is the selected terminal and holds focus, so the view
    // has a focus target to fall back from, and the agent is attached and
    // revealed, so the fallback lands on an existing island. Selecting the
    // agent first makes the tab's selection a real change: the view moves
    // focus only when its focus target changes, and a click on an
    // already-selected tab would leave focus on the strip button.
    await selectTerminal(page, "agent");
    await selectTerminal(page, tabId);
    await expect
      .poll(() =>
        page.evaluate(
          (el) => !!document.getElementById(el)?.contains(document.activeElement),
          `terminal-${tabId}`,
        )
      )
      .toBe(true);
    await waitForSessionRevealed(page, id);

    const trigger = page.locator(".restart-with-trigger");
    await expect(trigger).not.toHaveAttribute("aria-disabled", "true");
    await trigger.click();
    const dialog = page.locator(".restart-with-dialog");
    const cancel = dialog.locator(".restart-with-cancel");
    await expect(dialog).toBeVisible();
    await expect(cancel).toBeFocused();
    await page.evaluate(() => {
      (window as any).__restartWithTerminalFocus.armed = true;
    });

    const closed = await request.delete(`/api/sessions/${id}/tabs/${tabId}`);
    expect(closed.ok(), await closed.text()).toBe(true);
    await expect(page.locator(`.tab-slot[data-tab-id="${tabId}"]`)).toHaveCount(0, {
      timeout: 20_000,
    });
    await expect(page.locator(".tab-agent")).toHaveClass(/selected/);
    // The view unmounts the departed tab's island in the same synchronous
    // call that decides the new focus target, so once it is gone that
    // decision has been made.
    await expect
      .poll(() => page.evaluate(() => Object.keys((window as any).__farhelmIslands ?? {}).sort()))
      .toEqual(["terminal"]);

    await expect(dialog).toBeVisible();
    await expect(cancel).toBeFocused();
    for (const key of ["q", "w"]) {
      await page.keyboard.press(key);
      expect(await focusInsideDialog(page), `focus after ${key} following the tab's removal`).toBe(true);
    }
    expect(
      await page.evaluate(() => (window as any).__restartWithTerminalFocus.count),
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
  await page.evaluate(() => {
    (window as any).__restartWithTerminalFocus.armed = true;
  });

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
    await page.evaluate(() => (window as any).__restartWithTerminalFocus.count),
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
