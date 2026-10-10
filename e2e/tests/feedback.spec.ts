/**
 * The sidebar bar's help menu and the feedback dialog it opens (SPEC.md
 * "Feedback"). The dialog's promise to the user is that what it shows is the
 * whole submission and that nothing is sent until Send, so these tests read
 * the attached values off the page and compare them with the request the
 * browser actually made.
 *
 * Every feedback request is intercepted with `page.route` before it leaves
 * the browser. The stack's helm is a real one whose feedback route forwards
 * to the production endpoint, so a send that slipped past the route would
 * post to the internet; the helm's own forwarding is covered by its Rust
 * tests against a stand-in server instead.
 */
import { Locator, Page, Route } from "@playwright/test";
import { openHostMenu, openRowMenu, patchPreferences, readPreferences, resetPreferences } from "./helpers/fleet";
import { expect, test } from "./helpers/evidence";

const DOCS_URL = "https://farhelm.io/docs/";

// The premise every test here rests on: no feedback request reaches the real
// helm. Routes registered later by a test take precedence over this one.
test.beforeEach(async ({ page, request }) => {
  // Successful sends remember contact on the shared helm. Each case
  // needs its own seed rather than inheriting the previous case's contact.
  await resetPreferences(request);
  await page.route("**/api/feedback", (route) =>
    route.fulfill({ status: 500, contentType: "text/plain", body: "Couldn't send feedback: not routed by this test." }));
});

// The next spec uses the same helm, even when only one feedback case ran.
test.afterEach(async ({ request }) => {
  await resetPreferences(request);
});

async function openHelpMenu(page: Page) {
  const toggle = page.getByRole("button", { name: "help", exact: true });
  await toggle.click();
  const menu = page.getByRole("menu", { name: "help", exact: true });
  await expect(menu).toBeVisible();
  return { toggle, menu };
}

async function openFeedbackDialog(page: Page) {
  const { menu } = await openHelpMenu(page);
  await menu.locator('[data-bar-menu-item="send feedback"]').click();
  const dialog = page.getByRole("dialog", { name: "send feedback", exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** Route every feedback POST to `answer`, recording each JSON body sent. */
async function interceptFeedback(page: Page, answer: (route: Route) => Promise<void>) {
  const sent: unknown[] = [];
  await page.route("**/api/feedback", async (route) => {
    sent.push(route.request().postDataJSON());
    await answer(route);
  });
  return sent;
}

/**
 * The help menu lives in the bar to the right of the gear and behaves like
 * the other sidebar menus. A pointer opens and closes it from its toggle.
 * From the keyboard, ArrowDown on the toggle opens it with focus on its
 * first item, arrow keys move between its two items, and Escape closes it
 * and gives focus back to the toggle (focus is asserted on the keyboard
 * path, as the row menus' own tests do, because a pointer open does not
 * move focus the same way in every engine). A pointer press outside closes
 * it.
 */
test("the help menu sits right of the gear and follows the menu conventions", async ({ page }) => {
  await page.goto("/");
  const gear = page.getByRole("button", { name: "settings", exact: true });
  const { toggle, menu } = await openHelpMenu(page);
  const gearBox = await gear.boundingBox();
  const toggleBox = await toggle.boundingBox();
  expect(toggleBox!.x).toBeGreaterThan(gearBox!.x);
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  const items = menu.getByRole("menuitem");
  await expect(items).toHaveCount(2);
  await expect(items.nth(0)).toContainText("send feedback");
  await expect(items.nth(1)).toContainText("documentation");
  await toggle.click();
  await expect(menu).toHaveCount(0);

  await toggle.focus();
  await expect(toggle).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(menu).toBeVisible();
  await expect(items.nth(0)).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(items.nth(1)).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(toggle).toBeFocused();
  await expect(toggle).toHaveAttribute("aria-expanded", "false");

  await toggle.click();
  await expect(menu).toBeVisible();
  // A press on the session list header, well away from the menu.
  const header = page.locator(".session-heading").first();
  await header.click({ position: { x: 4, y: 4 } });
  await expect(menu).toHaveCount(0);

  // A layout change that can move things under the menu (here the window,
  // and with it the sidebar, getting shorter) invalidates its measured
  // position, so it closes, as the session and host menus do, rather than
  // staying detached from its toggle.
  const size = page.viewportSize()!;
  await toggle.click();
  await expect(menu).toBeVisible();
  await page.setViewportSize({ width: size.width, height: size.height - 60 });
  await expect(menu).toHaveCount(0);
  await page.setViewportSize(size);
});

/**
 * Why the stacking-context check: the sidebar's menus open beside it, over
 * the session pane, as fixed-position flyouts that live inside the sidebar's
 * scroll container. Fixed positioning lets a flyout escape that container's
 * clip in every engine as long as no ancestor between the flyout and the
 * root forms a stacking context inside the container. Once one does, WebKit
 * clips the fixed flyout to the container anyway (WebKit bug 160953). That
 * is the bug 0.22.0-rc.5's macOS app shipped: the help menu's flyout sat
 * inside the sticky (hence stacking-context) sidebar bar, and only its
 * pointer and a few pixels of the panel showed. Layout, visibility and hit
 * testing stayed correct, and the WebKit Playwright runs on Linux painted it
 * fine, so neither the menu tests above nor a screenshot caught it. This
 * check asserts the structure the fix depends on instead, for every sidebar
 * menu flyout: no ancestor that forms a stacking context either clips
 * itself (the literal shape of WebKit's bug report, an `overflow` element
 * with a `z-index`) or sits inside an ancestor that does. Each offender is
 * reported as the stacking context and the clipping element it found.
 */
async function stackingContextsInsideScrollers(flyout: Locator): Promise<string[]> {
  return flyout.evaluate((el) => {
    const createsStackingContext = (node: Element): boolean => {
      const style = getComputedStyle(node);
      const parentDisplay = node.parentElement ? getComputedStyle(node.parentElement).display : "";
      const flexOrGridItem = /flex|grid/.test(parentDisplay);
      return (
        style.position === "fixed" ||
        style.position === "sticky" ||
        (style.zIndex !== "auto" && (style.position !== "static" || flexOrGridItem)) ||
        parseFloat(style.opacity) < 1 ||
        style.transform !== "none" ||
        style.filter !== "none" ||
        style.isolation === "isolate" ||
        style.mixBlendMode !== "normal" ||
        /paint|layout|strict|content/.test(style.contain) ||
        /transform|opacity|filter/.test(style.willChange)
      );
    };
    const clips = (node: Element) => getComputedStyle(node).overflow !== "visible";
    const describe = (node: Element) => node.tagName.toLowerCase() + (node.className ? "." + [...node.classList].join(".") : "");
    const offenders: string[] = [];
    for (let node = el.parentElement; node && node !== document.documentElement; node = node.parentElement) {
      if (!createsStackingContext(node)) continue;
      // Starts at the stacking context itself: one that also clips is the
      // bug's own shape, not just one nested in a clipping ancestor.
      for (let above: Element | null = node; above && above !== document.documentElement; above = above.parentElement) {
        if (clips(above)) {
          offenders.push(above === node ? `${describe(node)} clips itself` : `${describe(node)} inside ${describe(above)}`);
          break;
        }
      }
    }
    return offenders;
  });
}

/**
 * Every sidebar menu flyout (help, session row, host row) keeps the
 * structure that lets WebKit paint it beside the sidebar: no stacking
 * context between it and the root clips or sits inside a clipping
 * container. See `stackingContextsInsideScrollers` for the bug this guards.
 *
 * The help menu is checked twice: in the default layout, where the sidebar
 * bar is sticky, and in the macOS app's narrow-window layout, where the bar
 * becomes fixed (`.macos-window` at 661px and below; forced here the way
 * window-chrome.spec.ts forces it). Each menu is closed, and seen closed,
 * before the next opens: the help flyout shares the host flyout's class, so
 * a help menu left open would otherwise be the one the host check reads.
 */
test("sidebar menu flyouts sit under no stacking context inside the sidebar", async ({ page }) => {
  await page.goto("/");
  // The selected session's terminal takes focus once it has connected,
  // even from an open menu (its reveal only yields to an editable field or
  // a dialog). Each menu below is closed with Escape, which reaches the
  // menu only while focus is inside it, so a terminal that connects after
  // the help menu opens would swallow the first Escape. Waiting for the
  // terminal to take focus first removes that race.
  await expect(page.getByRole("textbox", { name: "Terminal input" })).toBeFocused();
  const helpFlyout = page.locator('.bar-menu-flyout[data-bar-menu="help"]');
  await openHelpMenu(page);
  expect(await stackingContextsInsideScrollers(helpFlyout)).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(helpFlyout).toHaveCount(0);

  const sessionRow = page.locator(".session-row").first();
  await openRowMenu(sessionRow);
  const sessionFlyout = sessionRow.locator(".session-row-menu-flyout");
  expect(await stackingContextsInsideScrollers(sessionFlyout)).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(sessionFlyout).toHaveCount(0);

  const hostRow = page.locator(".host-row").first();
  await openHostMenu(hostRow);
  const hostFlyout = hostRow.locator(".host-row-menu-flyout");
  expect(await stackingContextsInsideScrollers(hostFlyout)).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(hostFlyout).toHaveCount(0);

  await page.setViewportSize({ width: 600, height: 700 });
  await page.locator(".app-shell").evaluate((element) => element.classList.add("macos-window"));
  await expect(page.locator(".app-bar")).toHaveCSS("position", "fixed");
  await openHelpMenu(page);
  expect(await stackingContextsInsideScrollers(helpFlyout)).toEqual([]);
});

/**
 * Documentation opens the docs site through the page's shared link opener,
 * the one that reaches the system browser from the desktop webview. The
 * opener is replaced with a recorder so the test checks the target and the
 * call without navigating anywhere.
 */
test("documentation opens the docs site through the shared link opener", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("button", { name: "help", exact: true })).toBeVisible();
  // The opener is its own script asset, loaded asynchronously; replace it
  // only once it exists, or the help menu would use its fallback instead.
  await page.waitForFunction(() => "farhelmTerminalLinks" in window);
  await page.evaluate(() => {
    const links = (window as unknown as { farhelmTerminalLinks: { openTerminalUrl: (uri: string) => void } })
      .farhelmTerminalLinks;
    const opened: string[] = [];
    (window as unknown as { openedLinks: string[] }).openedLinks = opened;
    links.openTerminalUrl = (uri: string) => {
      opened.push(uri);
    };
  });
  const { menu } = await openHelpMenu(page);
  await menu.locator('[data-bar-menu-item="documentation"]').click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { openedLinks: string[] }).openedLinks))
    .toEqual([DOCS_URL]);
  await expect(menu).toHaveCount(0);
});

/**
 * The submission the browser sends is exactly the message, the contact and
 * the attached values the dialog displayed, from the web UI. Send is
 * disabled until there is a message; on success the dialog thanks the user,
 * closes by itself, and focus returns to the help toggle.
 * Contact reuse keeps its ordinary leading checkbox even though Feedback
 * shares the Settings dialog's shell and label styles.
 */
test("send feedback sends exactly what the dialog shows, then thanks and closes", async ({ page }, testInfo) => {
  const sent = await interceptFeedback(page, (route) => route.fulfill({ status: 204 }));
  await page.goto("/");
  const dialog = await openFeedbackDialog(page);
  const send = dialog.getByRole("button", { name: "send", exact: true });
  await expect(send).toBeDisabled();
  await expect(dialog).toContainText("privately to Farhelm's maintainer");

  await dialog.locator(".feedback-message").fill("The sidebar is great.\nSecond line.");
  await dialog.locator(".feedback-contact").fill("someone@example.com");
  const reuse = dialog.getByRole("checkbox", { name: "Re-use for future feedback", exact: true });
  await expect(reuse).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  await dialog.screenshot({ path: testInfo.outputPath("feedback-reuse-layout.png") });
  const reuseLayout = await reuse.evaluate((input) => {
    const caption = Array.from(input.parentElement!.childNodes).find((node) => node !== input && node.textContent?.trim())!;
    const range = document.createRange();
    range.selectNode(caption);
    return {
      controlRight: input.getBoundingClientRect().right,
      textLeft: range.getBoundingClientRect().left,
      appearance: getComputedStyle(input).appearance,
    };
  });
  expect(reuseLayout.textLeft).toBeGreaterThan(reuseLayout.controlRight);
  expect(reuseLayout.textLeft - reuseLayout.controlRight).toBeLessThanOrEqual(16);
  expect(reuseLayout.appearance).not.toBe("none");
  await expect(dialog.locator(".feedback-surface")).toHaveText("web UI");
  // The operating system is read asynchronously; wait until it is no longer
  // the placeholder before reading the value the dialog will send.
  await expect(dialog.locator(".feedback-os")).not.toHaveText("unknown");
  const version = (await dialog.locator(".feedback-version").textContent())!;
  // The version sent is the one the sidebar shows.
  await expect(page.locator(".app-version")).toHaveText(version);
  const os = (await dialog.locator(".feedback-os").textContent())!;
  expect(sent).toEqual([]);

  await send.click();
  await expect(dialog.locator(".feedback-thanks")).toContainText("Thanks");
  // The announcement lives in a status region outside the dialog. While the
  // dialog is open that region is inert (modal isolation), so it must stay
  // empty until the dialog has closed, or the change would go unannounced.
  const notice = page.locator(".feedback-notice[role=status]");
  await expect(notice).toHaveText("");
  expect(sent).toEqual([
    {
      message: "The sidebar is great.\nSecond line.",
      contact: "someone@example.com",
      version,
      surface: "web",
      os,
    },
  ]);
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole("button", { name: "help", exact: true })).toBeFocused();
  await expect(notice).toContainText("Thanks");
  expect(await notice.evaluate((el) => el.closest("[inert]") === null)).toBe(true);
});

/**
 * Contact reuse is helm-wide and preserves exactly what was sent. A second
 * client's write after the first page authenticated must still be overwritten
 * by that page's next successful send, even if its local contact is unchanged.
 * Reload proves the server value, rather than only the dialog's local signal.
 */
test("successful feedback remembers the sent contact and writes it again from a stale client", async ({ page, request }) => {
  const sent = await interceptFeedback(page, (route) => route.fulfill({ status: 204 }));
  await page.goto("/");
  let dialog = await openFeedbackDialog(page);
  const reuse = dialog.getByRole("checkbox", { name: "Re-use for future feedback" });
  await expect(reuse).toHaveCount(0);
  const contact = "  contact@example.test  ";
  await dialog.locator(".feedback-contact").fill(contact);
  await expect(reuse).toBeChecked();
  await dialog.locator(".feedback-message").fill("Remember this contact.");
  await dialog.locator(".feedback-send").click();
  await expect(dialog).toHaveCount(0);
  await expect.poll(async () => (await readPreferences(request)).feedback_contact).toBe(contact);
  expect((sent[0] as { contact: string }).contact).toBe(contact);

  dialog = await openFeedbackDialog(page);
  await expect(dialog.locator(".feedback-contact")).toHaveValue(contact);
  await expect(dialog.getByRole("checkbox", { name: "Re-use for future feedback" })).toBeChecked();
  await patchPreferences(request, { feedback_contact: "other-client@example.test" });
  expect((await readPreferences(request)).feedback_contact).toBe("other-client@example.test");
  await dialog.locator(".feedback-message").fill("Keep my contact.");
  await dialog.locator(".feedback-send").click();
  await expect(dialog).toHaveCount(0);
  await expect.poll(async () => (await readPreferences(request)).feedback_contact).toBe(contact);
  await page.reload();
  dialog = await openFeedbackDialog(page);
  await expect(dialog.locator(".feedback-contact")).toHaveValue(contact);
  await expect(dialog.getByRole("checkbox", { name: "Re-use for future feedback" })).toBeChecked();
  await dialog.locator(".feedback-cancel").click();
});

/**
 * A successful send forgets prior contact both when reuse is unchecked and
 * when a prefilled field is emptied. Unchecking changes memory, not the
 * feedback submission; emptying sends null, the existing wire spelling for
 * no contact. The feedback request shape must remain unchanged by reuse.
 */
test("successful feedback clears contact when reuse is unchecked or the field is emptied", async ({ page, request }) => {
  const sent = await interceptFeedback(page, (route) => route.fulfill({ status: 204 }));
  for (const [round, choice] of ["unchecked", "empty"].entries()) {
    await patchPreferences(request, { feedback_contact: "remembered@example.test" });
    expect((await readPreferences(request)).feedback_contact).toBe("remembered@example.test");
    await page.goto("/");
    const dialog = await openFeedbackDialog(page);
    await expect(dialog.locator(".feedback-contact")).toHaveValue("remembered@example.test");
    const reuse = dialog.getByRole("checkbox", { name: "Re-use for future feedback" });
    await expect(reuse).toBeChecked();
    if (choice === "unchecked") {
      await reuse.uncheck();
    } else {
      await dialog.locator(".feedback-contact").fill("");
      await expect(reuse).toHaveCount(0);
    }
    await dialog.locator(".feedback-message").fill("Forget the contact next time.");
    await dialog.locator(".feedback-send").click();
    await expect(dialog).toHaveCount(0);
    await expect.poll(async () => (await readPreferences(request)).feedback_contact).toBeUndefined();
    const body = sent[round] as { contact: string | null };
    expect(body.contact).toBe(choice === "unchecked" ? "remembered@example.test" : null);
    await page.reload();
    const reopened = await openFeedbackDialog(page);
    await expect(reopened.locator(".feedback-contact")).toHaveValue("");
    await reopened.locator(".feedback-cancel").click();
    await expect(reopened).toHaveCount(0);
  }
});

/**
 * Failure and Cancel cannot change shared contact memory. Observe both the
 * actual preference writes and the server row: an attempted clear that later
 * failed would violate this boundary even if the row still looked unchanged.
 */
test("failed feedback and cancel preserve remembered contact without a preference write", async ({ page, request }) => {
  await patchPreferences(request, { feedback_contact: "remembered@example.test" });
  expect((await readPreferences(request)).feedback_contact).toBe("remembered@example.test");
  const writes: unknown[] = [];
  await page.route("**/api/preferences", async (route) => {
    if (route.request().method() === "PUT") writes.push(route.request().postDataJSON());
    await route.continue();
  });
  await interceptFeedback(page, (route) => route.fulfill({ status: 502, body: "Couldn't send feedback: unavailable." }));
  await page.goto("/");
  const dialog = await openFeedbackDialog(page);
  await expect(dialog.locator(".feedback-contact")).toHaveValue("remembered@example.test");
  await dialog.locator(".feedback-contact").fill("new@example.test");
  await dialog.getByRole("checkbox", { name: "Re-use for future feedback" }).uncheck();
  await dialog.locator(".feedback-message").fill("Keep memory if sending fails.");
  await dialog.locator(".feedback-send").click();
  await expect(dialog.getByRole("alert")).toContainText("Couldn't send feedback");
  expect((await readPreferences(request)).feedback_contact).toBe("remembered@example.test");
  expect(writes).toEqual([]);
  await dialog.locator(".feedback-cancel").click();
  await expect(dialog).toHaveCount(0);
  expect(writes).toEqual([]);
  await page.reload();
  const reopened = await openFeedbackDialog(page);
  await expect(reopened.locator(".feedback-contact")).toHaveValue("remembered@example.test");
  await reopened.locator(".feedback-cancel").click();
});

/**
 * A failed send says so in plain words and keeps everything typed, so the
 * user can retry or copy it. The helm's own sentence is shown as it is;
 * anything else the helm answers with (axum's text for an oversized body,
 * say) becomes a plain sentence naming the status, and a helm that cannot be
 * reached at all gets a plain sentence too, never the HTTP client's own
 * error text. A retry that succeeds then sends the same text.
 */
test("a failed send keeps the text and says it failed", async ({ page }) => {
  const answers = [
    (route: Route) =>
      route.fulfill({
        status: 502,
        contentType: "text/plain",
        body: "Couldn't send feedback: the feedback service could not be reached.",
      }),
    (route: Route) => route.fulfill({ status: 413, contentType: "text/plain", body: "length limit exceeded" }),
    // The helm itself unreachable: the request never gets an answer.
    (route: Route) => route.abort("connectionrefused"),
    (route: Route) => route.fulfill({ status: 204 }),
  ];
  let answered = 0;
  const sent = await interceptFeedback(page, (route) => answers[Math.min(answered++, 3)](route));
  await page.goto("/");
  const dialog = await openFeedbackDialog(page);
  const message = dialog.locator(".feedback-message");
  await message.fill("Keep this text.");
  const send = dialog.getByRole("button", { name: "send", exact: true });

  await send.click();
  await expect(dialog.getByRole("alert")).toContainText(
    "Couldn't send feedback: the feedback service could not be reached.",
  );
  await expect(message).toHaveValue("Keep this text.");
  await expect(send).toBeEnabled();

  await send.click();
  await expect(dialog.getByRole("alert")).toContainText(
    "Couldn't send feedback: Farhelm could not take the request (HTTP 413).",
  );
  await expect(message).toHaveValue("Keep this text.");

  await send.click();
  await expect(dialog.getByRole("alert")).toContainText("Couldn't send feedback: the request to Farhelm failed.");
  await expect(dialog.getByRole("alert")).not.toContainText("error sending request");
  await expect(message).toHaveValue("Keep this text.");

  await send.click();
  await expect(dialog.locator(".feedback-thanks")).toContainText("Thanks");
  expect(sent).toHaveLength(4);
  for (const body of sent) expect((body as { message: string }).message).toBe("Keep this text.");
});

/**
 * A close queued right behind Send, before the render that disables Cancel,
 * does not close the dialog while the send is unresolved. Why: closing then
 * drops the send with no word on whether it arrived and loses the typed text,
 * which SPEC.md promises a failed send keeps; Cancel used to close
 * unconditionally, and Escape checked the sending flag of the render before
 * Send. Specifies, for Cancel and for Escape (whose modal fallback clicks
 * Cancel): with the send held, Send then the close in one synchronous block
 * leaves the dialog open with its text; the held send then fails and the text
 * is still there, and that same close then closes the dialog. (Cancel before
 * any Send is covered by the blank-message test below.)
 */
test("a close queued behind send waits for the send", async ({ page }) => {
  let release!: () => void;
  const sent = await interceptFeedback(page, async (route) => {
    await new Promise<void>((resolve) => (release = resolve));
    await route.fulfill({
      status: 502,
      contentType: "text/plain",
      body: "Couldn't send feedback: the feedback service could not be reached.",
    });
  });
  await page.goto("/");

  for (const [round, close] of ["cancel", "escape"].entries()) {
    const dialog = await openFeedbackDialog(page);
    const message = dialog.locator(".feedback-message");
    await message.fill(`Keep this text (${close}).`);
    // The DOM click below does not wait for Send to become enabled the way a
    // locator click would; without this, a burst landing before that render
    // would click a disabled Send and test nothing.
    await expect(dialog.locator(".feedback-send")).toBeEnabled();
    const closeWasLive = await dialog.evaluate((node, how) => {
      const send = node.querySelector<HTMLButtonElement>(".feedback-send")!;
      const cancel = node.querySelector<HTMLButtonElement>(".feedback-cancel")!;
      send.click();
      // Still enabled: the render that disables it has not happened, which
      // is the window this test exists for. Focus inside the dialog, so the
      // Escape reaches the dialog's own handler and not only the modal's
      // fallback.
      const live = cancel.isConnected && !cancel.disabled && node.contains(document.activeElement);
      if (how === "cancel") {
        cancel.click();
      } else {
        node.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
      }
      return live;
    }, close);
    expect(closeWasLive, `premise: the ${close} path was still live behind send`).toBe(true);
    await expect.poll(() => sent.length, { message: "the send reached the route" }).toBe(round + 1);
    await expect(dialog.getByRole("button", { name: "sending…", exact: true })).toBeVisible();
    await expect(dialog, `${close} behind send leaves the dialog open`).toBeVisible();

    release();
    await expect(dialog.getByRole("alert")).toContainText("Couldn't send feedback");
    await expect(message, "the failed send keeps the text").toHaveValue(`Keep this text (${close}).`);

    // With no send in flight, the same close works.
    if (close === "cancel") {
      await dialog.locator(".feedback-cancel").click();
    } else {
      await message.focus();
      await page.keyboard.press("Escape");
    }
    await expect(dialog).toHaveCount(0);
  }
  expect(sent).toHaveLength(2);
});

/**
 * The dialog refuses what the helm would refuse, before anything is sent: a
 * blank message keeps Send disabled, and a message over the shared cap says
 * why. Cancel closes without sending.
 */
test("the dialog refuses blank and over-long messages without sending", async ({ page }) => {
  const sent = await interceptFeedback(page, (route) => route.fulfill({ status: 204 }));
  await page.goto("/");
  const dialog = await openFeedbackDialog(page);
  const send = dialog.getByRole("button", { name: "send", exact: true });
  const message = dialog.locator(".feedback-message");

  // Enabled first, so the blank check below shows the refusal rather than
  // the disabled state the dialog opens in.
  await message.fill("hello");
  await expect(send).toBeEnabled();
  await message.fill("   \n  ");
  await expect(send).toBeDisabled();
  await message.fill("x".repeat(4001));
  await expect(dialog.getByRole("alert")).toContainText("longer than 4000 characters");
  await expect(send).toBeDisabled();
  await message.fill("x".repeat(4000));
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  await expect(send).toBeEnabled();

  await dialog.getByRole("button", { name: "cancel", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  expect(sent).toEqual([]);
});
