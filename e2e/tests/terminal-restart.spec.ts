// ---------------------------------------------------------------------
// Restart with resume, at the browser level (PLAN_M3.md item 9).
//
// SPEC.md's restart is a lifecycle operation with a UI contract of its
// own: an interrupted session's view leads with the resume offer,
// declining it changes nothing, a live session confirms first, and a
// reused terminal keeps whatever scrollback tmux itself retained from the
// previous run, with the new run drawing below it. All four are below.
// ---------------------------------------------------------------------

import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Page } from "@playwright/test";
import {
  createResumableSession,
  hideSeenState,
  localHostId,
  openRowMenu,
  SESSION_LISTING,
  stopSession,
  agentLaunchRow,
} from "./helpers/fleet";
import { cleanupSession, restartIdleAgent, termText, waitForTermText } from "./helpers/term";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";
import {
  findSessionIdByTitle,
  fulfillAsHelm,
  installTerminalSuiteHooks,
  LIVE_BADGE,
  LIVE_STATES,
  rowByTitle,
  sharedSessionRow,
} from "./helpers/terminal-suite";

installTerminalSuiteHooks();

// The interrupted state cannot be produced by driving this stack: it takes
// a host reboot (or the injected boot-id change the Rust suite uses), so
// the listing is intercepted exactly like terminal.spec.ts's "deleting a
// session with unknown status confirms first, with wording that admits
// uncertainty" test does for the same reason. Everything else here is
// real — the component, its wording, and the fact that no request is sent.
//
// "Declining" has no control of its own by design (SPEC.md: opening an
// interrupted session OFFERS restart-with-resume; declining leaves it
// interrupted): the user simply does not click. So what this pins is that
// navigating away sends nothing and leaves the row exactly as it was —
// a restart affordance that fired on open, or on back, would be the bug.
/**
 * Add one interrupted, resumable session to the real listing and intercept
 * its restart and replace routes, counting the requests that reach them.
 *
 * The real listing plus one injected row, so every other test's session
 * (and the shared "e2e-session") keeps coming through untouched. The
 * counter is what every test here ultimately asserts on: SPEC.md's
 * "nothing respawns unattended" is a claim about requests, and only a
 * count can show that a decline sent none and a click sent exactly one.
 */
async function injectInterruptedSession(
  page: Page,
  sessionId: string,
  title: string,
  offer = "resume",
  // The injected row's launch, when a test is about a particular launch
  // kind; omitted, the row carries none, which the card treats as an
  // ordinary session.
  launch?: Record<string, unknown>,
) {
  // The replacement a successful Replace answers with. Like the real helm,
  // the listing reports it from then on: the sidebar re-selects when the
  // selected session drops out of a listing, and the helm's change hints
  // mean a listing can be fetched at any moment, so a replacement missing
  // from it would be deselected by whichever fetch landed first.
  const replacement = {
    id: `${sessionId.slice(0, -1)}9`,
    title: `${title} (replaced)`,
    cwd: "/tmp",
    invocation: "claude",
    status: { state: "unknown" },
    restart_offer: "resume",
    created_at: 0,
    last_activity_at: 0,
    tabs: [],
  };
  const counter = { restartRequests: 0, replaceRequests: 0 };
  // Set only by the successful Replace reply below. A test that overrides
  // that route with a refusal never sets it, so no phantom row appears.
  let replaced = false;
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const listing = await response.json();
    listing.sessions.push({
      id: sessionId,
      title,
      cwd: "/tmp",
      invocation: "claude",
      ...(launch === undefined ? {} : { launch }),
      status: { state: "interrupted" },
      restart_offer: offer,
    });
    listing.total += 1;
    if (replaced) {
      listing.sessions.push(replacement);
      listing.total += 1;
    }
    await route.fulfill({ response, json: listing });
  });
  // The reply is the shape a real restart returns — the session with the
  // supervisor's deliberate `unknown` for a run it cannot vouch for yet —
  // so the view takes its SUCCESS path (a bare `{}` would fail to decode
  // as a session and exercise the error line instead). The listing route
  // above keeps reporting the row interrupted, which is exactly the
  // window the view has to bridge on its own after a restart.
  await page.route(`**/api/sessions/${sessionId}/restart`, async (route) => {
    counter.restartRequests++;
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: sessionId,
        title,
        cwd: "/tmp",
        invocation: "claude",
        status: { state: "unknown" },
        restart_offer: "resume",
        created_at: 0,
        last_activity_at: 0,
        tabs: [],
      }),
    });
  });
  await page.route(`**/api/sessions/${sessionId}/replace`, async (route) => {
    counter.replaceRequests++;
    replaced = true;
    await fulfillAsHelm(route, {
      status: 200,
      contentType: "application/json",
      // The replacement must have a different identity (see its
      // definition). Reusing the source id would let this test pass without
      // proving selection moved.
      body: JSON.stringify(replacement),
    });
  });
  return counter;
}

test("an interrupted session's view leads with the resume offer, and declining changes nothing", async ({
  page,
}) => {
  const sessionId = "11111111-2222-3333-4444-555555555555";
  const title = `interrupted-offer-${Date.now()}`;
  const counter = await injectInterruptedSession(page, sessionId, title);

  await page.goto("/");
  await rowByTitle(page, title).locator(".session-row-open").click();

  // The offer still states WHY the terminal is gone and what restarting
  // would do to the conversation — both, because the user is being asked
  // to act on something they did not do. Since the header consolidation it
  // says so on the restart control itself rather than in a permanent band:
  // `data-tooltip` as a mouse's hover tooltip, and an `aria-describedby` target as
  // assistive technology's accessible description. Both are asserted,
  // because either alone leaves one of those two channels unable to read
  // it.
  const restart = page.locator(".restart-primary");
  await expect(restart).toHaveAttribute(
    "data-tooltip",
    /interrupted by a host reboot.*resumes this session's own conversation/,
  );
  const described = await restart.getAttribute("aria-describedby");
  expect(described, "the explanation must exist as a real element, not only as a tooltip").toBe(
    "restart-offer-description",
  );
  const offer = page.locator(`#${described}`);
  await expect(offer).toContainText("interrupted by a host reboot");
  await expect(offer).toContainText("resumes this session's own conversation");
  // The VISIBLE glyph is the compact "restart" every header action uses —
  // the header's supported minimum width has no room for the longest
  // offer's ~320px of wording on the button's face. SPEC.md's "restart says
  // so" instead reaches the accessible name: `aria-label` names the offer,
  // not the mechanism, which is what a screen reader announces regardless
  // of hover.
  await expect(restart).toHaveText("restart");
  await expect(restart).toHaveAttribute("aria-label", "resume conversation");
  // And the header states the session's last-known status beside the
  // title, which is where the reason a user is being offered a restart now
  // lives (the offer prose used to carry it in a band of its own).
  await expect(page.locator(".titlebar .status-badge")).toHaveText("interrupted");
  // An interrupted session has nothing running, so there is no confirm
  // step in front of it.
  await expect(page.locator(".restart-confirm")).toHaveCount(0);
  expect(counter.restartRequests).toBe(0);

  // The reboot took the terminal, so the view mounts none and retries
  // nothing (SPEC.md: metadata and the reason, never an empty pane): no
  // terminal element for terminal.js to attach, no catch-up or reconnect
  // overlay, and no tab strip. Before this, the view attached to the
  // destroyed pane and climbed a "connection lost" ladder whose "reconnect
  // now" could never succeed. In their place, the surface itself says why
  // and carries the restart, so the explanation is not only a tooltip.
  await expect(page.locator("#terminal")).toHaveCount(0);
  await expect(page.locator("#term-connecting")).toHaveCount(0);
  await expect(page.locator(".terminal-panes")).toHaveCount(0);
  await expect(page.locator(".tab-strip")).toHaveCount(0);
  const notice = page.locator(".interrupted-card");
  await expect(notice).toBeVisible();
  await expect(notice).toContainText("host restart paused this session");
  await expect(notice).toContainText("resumes this session's own conversation");
  await expect(notice.locator(".restart-from-notice")).toHaveText("restart");
  await expect(notice.locator(".restart-from-notice")).toHaveClass(/btn-primary/);
  await expect(notice.locator(".restart-from-notice")).toHaveAttribute("aria-label", "resume conversation");
  expect(counter.restartRequests).toBe(0);

  // Declining means LEAVING — selecting another session — and the leave
  // itself must not send anything: a regression that fired the restart on
  // navigate-away would pass a stay-put assertion.
  await sharedSessionRow(page).click();
  await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
  const row = rowByTitle(page, title);
  await expect(row.locator(".status-badge")).toHaveText("interrupted");
  expect(counter.restartRequests).toBe(0);
});

/**
 * Why this matters: Restart only ever resumes the session's own conversation
 * (SPEC.md), so a session that cannot resume must not offer a Restart that
 * would start it fresh, and the user must learn why from the control itself.
 * Spec: for a `not_captured` interrupted session, the header Restart stays
 * visible but greyed out (`aria-disabled`, not natively disabled, so its
 * tooltip stays readable), its accessible name says Restart is unavailable,
 * its tooltip and description give the reason and point at Replace, a click
 * sends nothing, and the interrupted card offers only Replace.
 */
test("an interrupted session that cannot resume greys out Restart and offers only Replace", async ({ page }) => {
  const sessionId = "11111111-2222-3333-4444-888888888888";
  const title = `interrupted-unresumable-${Date.now()}`;
  const counter = await injectInterruptedSession(page, sessionId, title, "not_captured");

  await page.goto("/");
  await rowByTitle(page, title).locator(".session-row-open").click();
  await expect(page.locator(".titlebar .status-badge")).toHaveText("interrupted");

  const restart = page.locator(".restart-primary");
  await expect(restart).toBeVisible();
  await expect(restart).toHaveAttribute("aria-disabled", "true");
  await expect(restart).toHaveAttribute("aria-label", "restart unavailable");
  await expect(restart).toHaveAttribute("data-tooltip", /no conversation Farhelm can resume was captured.*replace/);
  await expect(page.locator("#restart-offer-description")).toContainText("no conversation Farhelm can resume");
  await expect(page.locator(".restart-with-trigger")).toHaveAttribute("aria-disabled", "true");

  const notice = page.locator(".interrupted-card");
  await expect(notice).toBeVisible();
  await expect(notice).toContainText("choose Replace");
  await expect(notice.locator(".restart-from-notice")).toHaveCount(0);
  await expect(notice.locator(".replace-from-notice")).toBeVisible();

  // A greyed-out control that still sent the request would be the fresh
  // restart SPEC.md rules out; nothing may open a confirmation either.
  // `force`, because Playwright treats `aria-disabled` as disabled and would
  // otherwise wait for the control to become enabled; a user's click still
  // reaches it, which is exactly what is under test.
  await restart.click({ force: true });
  await expect(page.locator(".restart-confirm")).toHaveCount(0);
  // sleep-ok: give a wrongly sent restart time to reach the counting route before asserting none did.
  await page.waitForTimeout(500);
  expect(counter.restartRequests).toBe(0);
});

/**
 * Spec: an interrupted session from before launch kinds offers Replace with,
 * not plain Replace, says so in its text, and the button opens the launcher
 * with the session's command filled in.
 *
 * Why: plain Replace refuses a legacy session (SPEC.md's launch-kinds
 * upgrade), so the card would otherwise offer a button that can only fail;
 * Replace with is the way forward the spec names for it.
 */
test("an interrupted legacy session offers Replace with, prefilled with its command", async ({ page }) => {
  const sessionId = "11111111-2222-3333-4444-666666666666";
  const title = `interrupted-legacy-${Date.now()}`;
  await injectInterruptedSession(page, sessionId, title, "not_captured", {
    kind: "legacy",
    invocation: "claude",
    agent_kind: "claude",
    resume_template: null,
  });

  await page.goto("/");
  await rowByTitle(page, title).locator(".session-row-open").click();
  const notice = page.locator(".interrupted-card");
  await expect(notice).toBeVisible();
  await expect(notice).toContainText("choose Replace with");
  await expect(notice).not.toContainText("replace starts it over");
  await expect(notice.locator(".replace-from-notice")).toHaveCount(0);
  await notice.locator(".replace-with-from-notice").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await expect(form.getByLabel("agent command")).toHaveValue("claude");
});

/**
 * The missing-terminal choices stay visually coherent with the compact
 * Farhelm controls, and Replace cannot discard a conversation without a
 * deliberate confirmation. A successful choice must show the new session;
 * a refusal must leave an actionable error on the interrupted one.
 */
test("Replace confirms inline, can cancel, selects the fresh session, and surfaces refusal", async ({
  page,
}) => {
  const sessionId = "11111111-2222-3333-4444-777777777777";
  const title = `interrupted-replace-${Date.now()}`;
  const counter = await injectInterruptedSession(page, sessionId, title);

  await page.goto("/");
  await rowByTitle(page, title).locator(".session-row-open").click();
  const notice = page.locator(".interrupted-card");
  const replace = notice.locator(".replace-from-notice");
  await expect(replace).toHaveCSS("padding-top", "4px");
  await expect(notice.locator(".restart-from-notice")).toHaveCSS("padding-top", "4px");
  await replace.click();
  await expect(notice.locator(".replace-confirm")).toBeVisible();
  await expect(replace).toHaveClass(/btn-primary/);
  await expect(notice.locator(".replace-confirm-submit")).toHaveClass(/btn-danger/);
  await expect(replace).toHaveCSS("opacity", "1");
  await expect(notice.locator(".replace-confirm-submit")).toHaveCSS("font-size", "12px");
  await expect(notice.locator(".replace-confirm-submit")).toHaveCSS("padding-left", "8px");
  await notice.locator(".replace-cancel").click();
  await expect(notice.locator(".replace-confirm")).toHaveCount(0);
  expect(counter.replaceRequests).toBe(0);

  await replace.click();
  await notice.locator(".replace-confirm-submit").click();
  await expect.poll(() => counter.replaceRequests).toBe(1);
  await expect(page.locator(".titlebar .title")).toHaveText(`${title} (replaced)`);

  const refusedId = "11111111-2222-3333-4444-888888888888";
  const refusedTitle = `interrupted-replace-refused-${Date.now()}`;
  const refused = await injectInterruptedSession(page, refusedId, refusedTitle);
  await page.route(`**/api/sessions/${refusedId}/replace`, async (route) => {
    refused.replaceRequests++;
    await fulfillAsHelm(route, {
      status: 409,
      contentType: "text/plain",
      body: "replacement refused: source is no longer available",
    });
  });
  // The refused session exists only in this page's listing route, so
  // nothing on the helm changes to make the page fetch the listing again,
  // and its row appears only through a fetch made after the route above
  // was installed. Reload so that fetch happens by construction. Waiting
  // for an unrelated refresh to come along is what made this test time out
  // on WebKit whenever it ran behind the rest of its file: the retained
  // trace shows the page's last listing fetch landing before the route.
  await page.reload();
  await rowByTitle(page, refusedTitle).locator(".session-row-open").click();
  await page.locator(".replace-from-notice").click();
  await page.locator(".replace-confirm-submit").click();
  await expect.poll(() => refused.replaceRequests).toBe(1);
  await expect(page.locator(".replace-error")).toContainText("source is no longer available");
  await expect(page.locator(".replace-error")).toHaveCSS("font-size", "12px");
  await expect(page.locator(".replace-from-notice")).toBeVisible();
});

/**
 * Inject one interrupted session whose launch is YOLO on the local host, and
 * mock its Replace and the local host's YOLO confirmation-setting write: a Replace without the
 * override is refused with the helm's YOLO header, one with it succeeds and
 * the replacement joins the listing; the host write is refused while
 * `control.refuseMark` is set and succeeds otherwise. Every
 * request is recorded, in order, in `sequence` ("replace", "mark",
 * "replace+override"), which is what the "don't ask again" tests assert on.
 * `withHost: false` omits the row's host fields, as a helm that sends none
 * would.
 * Nothing reaches the shared helm's real host setting or sessions.
 */
async function injectYoloReplaceSession(
  page: Page,
  request: APIRequestContext,
  sessionId: string,
  title: string,
  { withHost = true }: { withHost?: boolean } = {},
) {
  const local = await localHostId(request);
  const replacement = {
    id: `${sessionId.slice(0, -1)}9`,
    title: `${title} (replaced)`,
    cwd: "/tmp",
    invocation: "codex --yolo",
    status: { state: "unknown" },
    restart_offer: "resume",
    created_at: 0,
    last_activity_at: 0,
    tabs: [],
  };
  let replaced = false;
  await page.route(SESSION_LISTING, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const listing = await response.json();
    listing.sessions.push({
      id: sessionId,
      title,
      cwd: "/tmp",
      invocation: "codex --yolo",
      launch: agentLaunchRow({ harness: "codex", model: null, effort: null, permissions: "yolo", workspace_trust: null }),
      ...(withHost ? { host: local, host_name: "this machine" } : {}),
      status: { state: "interrupted" },
      restart_offer: "resume",
    });
    listing.total += 1;
    if (replaced) {
      listing.sessions.push(replacement);
      listing.total += 1;
    }
    await route.fulfill({ response, json: listing });
  });
  const sequence: string[] = [];
  const replaceBodies: any[] = [];
  await page.route(`**/api/sessions/${sessionId}/replace`, async (route) => {
    const body = route.request().postDataJSON();
    replaceBodies.push(body);
    sequence.push(body.confirm_yolo ? "replace+override" : "replace");
    if (!body.confirm_yolo) {
      await fulfillAsHelm(route, {
        status: 409,
        contentType: "text/plain",
        headers: { "x-farhelm-yolo-confirmation": "confirmation-required" },
        body: "this machine asks before YOLO launches; confirm with --confirm-yolo",
      });
      return;
    }
    replaced = true;
    await fulfillAsHelm(route, { status: 200, contentType: "application/json", body: JSON.stringify(replacement) });
  });
  const marks: unknown[] = [];
  const control = { refuseMark: false };
  await page.route(`**/api/hosts/${local}/yolo-without-asking`, async (route) => {
    marks.push(route.request().postDataJSON());
    sequence.push("mark");
    if (control.refuseMark) {
      await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: "held by the test" });
      return;
    }
    await fulfillAsHelm(route, { status: 200, contentType: "application/json", body: "{}" });
  });

  return { sequence, marks, replaceBodies, control };
}

// "Start, and don't ask again on this host" on a Replace the helm refused as
// a YOLO launch. The interrupted card's Replace runs through the same
// session-view path as the header's, so this covers both. Why: the button's
// promise is only kept if the host is marked before the replace goes out,
// and a replace that went out first would leave the user asked again next
// time. Specifies: the refused replace shows the question with the GUI's own
// explanation; the button sends the host mark, then exactly one more replace
// carrying the override, in that order, and the view moves to the
// replacement. Every request is route-mocked, so the shared helm's real host
// setting and sessions are untouched.
test("don't ask again on a refused YOLO replace marks the host, then replaces with the override", async ({
  page,
  request,
}) => {
  const sessionId = "11111111-2222-3333-4444-aaaaaaaaaaa0";
  const title = `interrupted-replace-yolo-${Date.now()}`;
  const { sequence, marks, replaceBodies, control } = await injectYoloReplaceSession(page, request, sessionId, title);

  await page.goto("/");
  await rowByTitle(page, title).locator(".session-row-open").click();
  await page.locator(".replace-from-notice").click();
  await page.locator(".replace-confirm-submit").click();
  const confirmation = page.locator(".yolo-confirmation");
  await expect(confirmation).toBeVisible();
  await expect(confirmation).toContainText("This launch uses YOLO permissions.");
  await expect(confirmation).toContainText("this machine asks before every YOLO launch.");
  expect(sequence).toEqual(["replace"]);

  // A refused mark replaces nothing, and the question stays with its reason.
  control.refuseMark = true;
  await confirmation.locator(".yolo-confirm-stop-asking").click();
  await expect(confirmation.locator(".yolo-confirmation-error")).toContainText("held by the test");
  expect(sequence, "a refused mark sends no replace").toEqual(["replace", "mark"]);
  await expect(confirmation).toBeVisible();

  control.refuseMark = false;
  await confirmation.locator(".yolo-confirm-stop-asking").click();
  await expect(page.locator(".titlebar .title")).toHaveText(`${title} (replaced)`);
  expect(sequence, "the host is marked before the replace goes out").toEqual([
    "replace",
    "mark",
    "mark",
    "replace+override",
  ]);
  expect(marks).toEqual([{ yolo_without_asking: true }, { yolo_without_asking: true }]);
  expect(replaceBodies).toHaveLength(2);
});

// The same answer on a Replace started from the session row's menu, which
// runs a separate path in the session list (it holds a row operation rather
// than the page's lock). Specifies what the session-view test above does, for
// that path: the question appears above the list, and the button marks the
// host before the one replace that carries the override.
test("don't ask again on a refused YOLO replace from the row menu marks the host first", async ({
  page,
  request,
}) => {
  const sessionId = "11111111-2222-3333-4444-bbbbbbbbbbb0";
  const title = `row-replace-yolo-${Date.now()}`;
  const { sequence, marks, replaceBodies, control } = await injectYoloReplaceSession(page, request, sessionId, title);

  await page.goto("/");
  const row = rowByTitle(page, title);
  await openRowMenu(row);
  await row.locator(".session-row-replace").click();
  await row.locator(".confirm-replace").click();
  const confirmation = page.locator(".yolo-confirmation");
  await expect(confirmation).toBeVisible();
  await expect(confirmation).toContainText("This launch uses YOLO permissions.");
  expect(sequence).toEqual(["replace"]);

  // A refused mark replaces nothing, the question stays with its reason, and
  // the row operation is over, so the question is usable again.
  control.refuseMark = true;
  await confirmation.locator(".yolo-confirm-stop-asking").click();
  await expect(confirmation.locator(".yolo-confirmation-error")).toContainText("held by the test");
  expect(sequence, "a refused mark sends no replace").toEqual(["replace", "mark"]);
  await expect(confirmation).toBeVisible();
  await expect(confirmation.locator(".yolo-cancel"), "the question's buttons are usable again").toBeEnabled();

  control.refuseMark = false;
  await confirmation.locator(".yolo-confirm-stop-asking").click();
  await expect.poll(() => sequence).toEqual(["replace", "mark", "mark", "replace+override"]);
  await expect(confirmation).toHaveCount(0);
  expect(marks).toEqual([{ yolo_without_asking: true }, { yolo_without_asking: true }]);
  expect(replaceBodies).toHaveLength(2);
});

// "Don't ask again" needs a host to mark. A row whose host the helm did not
// send cannot offer it: a button that marked nothing and then launched would
// be the one-off override under a misleading label. Specifies: the question
// for such a row shows the one-off and cancel, and no "don't ask again".
test("a YOLO question for a row with no known host offers no don't-ask-again", async ({ page, request }) => {
  const sessionId = "11111111-2222-3333-4444-ccccccccccc0";
  const title = `row-replace-yolo-nohost-${Date.now()}`;
  const { sequence } = await injectYoloReplaceSession(page, request, sessionId, title, { withHost: false });

  await page.goto("/");
  const row = rowByTitle(page, title);
  await openRowMenu(row);
  await row.locator(".session-row-replace").click();
  await row.locator(".confirm-replace").click();
  const confirmation = page.locator(".yolo-confirmation");
  await expect(confirmation).toBeVisible();
  expect(sequence).toEqual(["replace"]);
  await expect(confirmation.locator(".yolo-confirm")).toBeVisible();
  await expect(confirmation.locator(".yolo-cancel")).toBeVisible();
  await expect(confirmation.locator(".yolo-confirm-stop-asking")).toHaveCount(0);
  await confirmation.locator(".yolo-cancel").click();
  await expect(confirmation).toHaveCount(0);
});

// The interrupted surface's own restart control is the same request the
// header's is: one click, one restart, no confirmation (nothing is running
// to confirm stopping). Counted rather than merely observed, because two
// controls wired to one closure could still double-send if the second one
// were given its own handler by mistake — and a restart is not idempotent
// from the user's side.
test("restart from the interrupted surface sends the resume request exactly once", async ({
  page,
}) => {
  const sessionId = "11111111-2222-3333-4444-666666666666";
  const title = `interrupted-surface-restart-${Date.now()}`;
  const counter = await injectInterruptedSession(page, sessionId, title);

  await page.goto("/");
  await rowByTitle(page, title).locator(".session-row-open").click();
  const restart = page.locator(".interrupted-card .restart-from-notice");
  await expect(restart).toBeVisible();
  await expect(restart).toBeEnabled();
  await restart.click();
  await expect.poll(() => counter.restartRequests).toBe(1);
  // No confirmation panel was interposed, and the click was not
  // re-delivered by the surface re-rendering around the in-flight request.
  await expect(page.locator(".restart-confirm")).toHaveCount(0);
  // The reply ends the interrupted surface on its own, ahead of the
  // listing (which this fixture keeps pinned to `interrupted`): the card
  // and its control are gone and the terminal element is back for the new
  // run to attach into. A view that waited for the listing would still be
  // showing "did not survive" over a session that is now running.
  await expect(page.locator(".interrupted-card")).toHaveCount(0);
  await expect(page.locator("#terminal")).toHaveCount(1);
  await expect(page.locator(".restart-error")).toHaveCount(0);
  // Begin the duplicate-request window only after the reply has replaced
  // the interrupted surface; a slow reply must not consume the observation.
  // sleep-ok: count any duplicate restart after the response-driven surface transition.
  await page.waitForTimeout(500);
  expect(counter.restartRequests).toBe(1);
});

// A working agent is the one case SPEC.md requires a confirmation for
// ("Restart on a session whose agent is working (its status reads working)
// confirms, stops the agent, then relaunches"), and the confirmation is in-page for the same
// reason delete's is: wry ships no native JS dialogs on macOS's WKWebView,
// where a `window.confirm()` would silently do nothing at all.
//
// Driven against a REAL session, so the request that finally goes out is
// the real one — including `stop_if_running`, which is the whole point of
// the confirmation and is asserted on the wire rather than assumed.
// PLAN_M6_75.md item 3's no-badge rule on the RESTART path — the half its
// create-path sibling explicitly does not cover, because the mechanism is
// a different one.
//
// A restart puts a session briefly back into "nothing has classified this
// yet", exactly as a create does. What must NOT happen is the badge
// blinking out and back: the helm's merge rule refuses to let an unknown
// status overwrite one it already knows definitely, so the PREVIOUS status
// stays on screen across the whole restart. Eventual-running is not the
// assertion — a badge that vanished for two seconds and came back would
// satisfy that while being precisely the flicker this rule exists to
// prevent — so the badge is sampled CONTINUOUSLY and every sample must
// find one.
//
// Driven against the real stack, and the restart is issued through the API
// rather than the view's own button: the property is about what the LIST
// shows while a restart runs, and the button lives on the other page. What
// restarts the session is irrelevant to it.
//
// `LIVE_BADGE` matches the bare status word — never the idle-unseen
// annotation, which this test's own subject has nothing to do with, so
// `hideSeenState` keeps this session's row from ever growing the "idle —
// new output" text it would otherwise genuinely earn (SPEC.md, Status: a
// session created here and never opened is unseen from the moment a real
// classifier settles it into idle, exactly what this fixture is). Widening
// `LIVE_BADGE` instead would have been the wrong fix: that constant is
// shared, and every other caller's assertion is about the plain live word.
test("restart-keeps-a-badge-on-screen-throughout", async ({ page, request }) => {
  // The observation window alone is 20 seconds, and it sits between a real
  // create and a real restart — comfortably past the 60-second default.
  test.setTimeout(120_000);
  const title = `restart-badge-${Date.now()}`;
  let id: string | undefined;
  try {
    // Restart only resumes, so the session reports a conversation.
    id = (await createResumableSession(request, { cwd: "/tmp", title })).id;

    await hideSeenState(page);
    await page.goto("/");
    const badge = rowByTitle(page, title).locator(".status-badge");
    // A DEFINITE status first: the rule under test is about not losing one,
    // so there has to be one to lose.
    await expect(badge).toHaveText(LIVE_BADGE, { timeout: 20_000 });

    // Observe each DOM mutation batch in the browser, so a slow driver does
    // not need to squeeze an arbitrary number of samples into the window.
    // As with polling, this does not assert every transient paint within a
    // single batch; it checks the badge's presence after DOM updates settle.
    await page.evaluate((sessionId) => {
      const selector = `[data-session-id="${sessionId}"] .status-badge`;
      if (!document.querySelector(selector)) throw new Error("restart badge missing before observation");
      const observed = { checks: 0, missing: 0, firstMissingMs: null as number | null };
      const start = performance.now();
      const sample = () => {
        observed.checks += 1;
        if (!document.querySelector(selector)) {
          observed.missing += 1;
          observed.firstMissingMs ??= performance.now() - start;
        }
      };
      const observer = new MutationObserver(sample);
      observer.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["class"] });
      sample();
      (window as any).__stopRestartBadgeObservation = () => {
        sample();
        observer.disconnect();
        return observed;
      };
    }, id);
    // Observed while the restart is in flight AND for a stretch afterwards,
    // since the window this is about — the gap between the relaunch and
    // the first classification of the new run — opens after the request
    // returns, not during it.
    const restarted = await request.post(`/api/sessions/${id}/restart`, {
      data: { stop_if_running: true },
    });
    expect(restarted.ok(), `restarting ${id}`).toBe(true);
    // sleep-ok: retain the finite restart/reclassification observation window, with mutation evidence captured in-page.
    await page.waitForTimeout(20_000);
    const observed = await page.evaluate(() => (window as any).__stopRestartBadgeObservation());
    expect(
      observed.missing,
      "the list must never blank a session's status badge across a restart: the helm holds " +
        "the previous definite status precisely so this window shows something true rather " +
        `than nothing; observation: ${JSON.stringify(observed)}`,
    ).toBe(0);
    // The final displayed status is still live. The word alone does not
    // distinguish a new classification from the retained pre-restart value.
    await expect(badge).toHaveText(LIVE_BADGE, { timeout: 20_000 });
  } finally {
    if (!page.isClosed()) {
      await page.evaluate(() => (window as any).__stopRestartBadgeObservation?.());
    }
    if (id) await cleanupSession(request, id);
  }
});

test("restarting a working agent confirms first, and only then sends the request with consent", async ({
  page,
  request,
}) => {
  const title = `restart-confirm-${Date.now()}`;
  const bodies: any[] = [];
  await page.route("**/api/sessions/*/restart", async (route) => {
    bodies.push(route.request().postDataJSON());
    await route.continue();
  });

  try {
    await page.goto("/");
    // Restart only resumes, so the session reports a conversation as it
    // starts; the create form cannot declare a command's agent type, so the
    // API creates it and the page opens it.
    const created = await createResumableSession(request, { cwd: "/tmp", title });
    await rowByTitle(page, title).locator(".session-row-open").click();
    await waitForSessionRevealed(page, created.id);
    await waitForTermText(page, "FAKE-AGENT READY");

    // The >= 2 banner count at the end needs the FIRST run's banner to
    // survive the respawn, and only tmux history survives one — the
    // visible grid, where a two-line run's banner still sits, is wiped
    // (SPEC.md, Lifecycle operations/Restart). Scroll it into history
    // before restarting: `spam 60` is more lines than any terminal this
    // test runs in has rows, and the last line's arrival is the barrier,
    // exactly as in the scrollback-retention test below.
    await page.locator("#terminal").click();
    await page.keyboard.type("spam 60");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-60");
    // Restart asks only while the agent is working, and a quiet fixture
    // agent reads idle after a few samples; `busy` keeps its screen
    // changing for the rest of the test so the status stays working.
    await page.keyboard.type("busy");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "busy-tick-");

    // Wait until the view's own status-derived decision says this click
    // will confirm rather than restart outright (`data-confirms`, set from
    // the session's status): the view opens on the create reply's
    // deliberate `Unknown` placeholder and refreshes once, so clicking
    // before that lands would exercise the stale-hint path instead of the
    // confirmation this test is about.
    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 15_000,
    });
    // Closed before the first click — the popover this button controls
    // does not exist yet, and `aria-expanded` must say so rather than
    // defaulting to open.
    await expect(restartButton).toHaveAttribute("aria-expanded", "false");

    // The first click only opens the prompt: nothing is sent, and the
    // consequence text says what restarting would do to the running agent.
    await restartButton.click();
    await expect(page.locator(".restart-offer .confirm-consequence")).toContainText(
      "still running",
    );
    expect(bodies).toHaveLength(0);
    // Open now — this is the state a sidebar row menu opened at the same
    // time could visually cover (see app.css's `.header-confirm` z-index
    // comment), so the trigger's own record of it matters independently of
    // the popover being on screen.
    await expect(restartButton).toHaveAttribute("aria-expanded", "true");

    // Cancel returns the view to its normal state, still having sent
    // nothing — the same "cancel is the only way back" rule the delete
    // prompt follows.
    await page.locator(".restart-cancel").click();
    await expect(page.locator(".restart-primary")).toBeVisible();
    expect(bodies).toHaveLength(0);
    await expect(restartButton).toHaveAttribute("aria-expanded", "false");

    await restartButton.click();
    await expect(restartButton).toHaveAttribute("aria-expanded", "true");
    await page.locator(".restart-confirm").click();
    await expect.poll(() => bodies.length).toBe(1);
    expect(bodies[0].stop_if_running).toBe(true);
    // There is no mode: a restart always resumes the session's own
    // conversation.
    expect(bodies[0].mode).toBeUndefined();

    // And the relaunch actually comes up. Counted rather than merely
    // matched: the spam above pushed the FIRST run's banner into the
    // reused terminal's retained history, so `toContain` would pass
    // without the new run having printed anything at all.
    await expect
      .poll(
        async () => (await termText(page)).split("FAKE-AGENT READY").length - 1,
        {
          timeout: 30_000,
          message: "the relaunched agent's own ready banner",
        },
      )
      .toBeGreaterThanOrEqual(2);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

/**
 * Why this matters: the header's restart prompt rewords itself while it
 * stays open, and its confirm used to consent to stopping a running agent
 * whatever it said by then. A prompt that came to say the agent had exited
 * therefore stopped an agent started again before the click, unasked
 * (SPEC.md "Lifecycle operations": a confirmation authorizes only what its
 * prompt said). Spec: a prompt opened on a working agent that rewords to the
 * exited case sends the restart without `stop_if_running`.
 *
 * The drift is real: the agent is stopped through the API while the prompt is
 * open, and the view's own detail refresh rewords it. The restart request
 * itself goes through, which is harmless for an exited agent.
 */
test("a restart prompt that drifted to an exited agent no longer consents to a stop", async ({
  page,
  request,
}) => {
  const title = `restart-drift-${Date.now()}`;
  const bodies: any[] = [];
  await page.route("**/api/sessions/*/restart", async (route) => {
    bodies.push(route.request().postDataJSON());
    await route.continue();
  });

  try {
    await page.goto("/");
    // Restart only resumes, so the session reports a conversation as it
    // starts; the create form cannot declare a command's agent type, so the
    // API creates it and the page opens it.
    const created = await createResumableSession(request, { cwd: "/tmp", title });
    await rowByTitle(page, title).locator(".session-row-open").click();
    const id = created.id;
    await waitForSessionRevealed(page, id);
    await waitForTermText(page, "FAKE-AGENT READY");
    // `busy` keeps the agent reading working, the one status that opens
    // the prompt (see the confirmation test above).
    await page.locator("#terminal").click();
    await page.keyboard.type("busy");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "busy-tick-");
    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", { timeout: 15_000 });

    // Premise: the prompt opens offering to stop the running agent.
    await restartButton.click();
    const consequence = page.locator(".restart-offer .confirm-consequence");
    await expect(consequence).toContainText("still running");

    // The agent ends behind the open prompt, which rewords to the exited case.
    await stopSession(request, id);
    await expect(consequence).toContainText("the agent has exited", { timeout: 20_000 });
    expect(bodies, "premise: nothing was sent while the prompt was open").toHaveLength(0);

    await page.locator(".restart-confirm").click();
    await expect.poll(() => bodies.length).toBe(1);
    expect(
      bodies[0].stop_if_running,
      "a prompt that no longer offered to stop a running agent must not consent to one",
    ).toBe(false);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// The other half of Restart's confirmation rule (SPEC.md, Lifecycle
// operations): an agent that reads idle restarts on the first click, with no
// prompt and without `stop_if_running`, and the supervisor accepts that
// because its own reading agrees. Asking on every live agent is what the rule
// removed: a prompt shown that often gets clicked through unread. The
// supervisor's reply is the oracle that the unconfirmed request was accepted
// rather than refused as touching a working agent.
test("restarting an idle agent restarts at once, without asking", async ({ page, request }) => {
  const title = `restart-idle-${Date.now()}`;
  const bodies: any[] = [];
  await page.route("**/api/sessions/*/restart", async (route) => {
    bodies.push(route.request().postDataJSON());
    await route.continue();
  });

  try {
    await page.goto("/");
    // Restart only resumes, so the session reports a conversation as it
    // starts; the create form cannot declare a command's agent type, so the
    // API creates it and the page opens it.
    const created = await createResumableSession(request, { cwd: "/tmp", title });
    await rowByTitle(page, title).locator(".session-row-open").click();
    await waitForSessionRevealed(page, created.id);
    await waitForTermText(page, "FAKE-AGENT READY");

    const replied = page.waitForResponse((response) =>
      response.url().endsWith("/restart") && response.request().method() === "POST"
    );
    await restartIdleAgent(page);
    const reply = await replied;
    expect(reply.ok(), `the unconfirmed restart of an idle agent: ${await reply.text()}`).toBe(true);
    await expect(page.locator(".restart-confirm")).toHaveCount(0);
    expect(bodies).toHaveLength(1);
    expect(bodies[0].stop_if_running).toBe(false);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// SPEC.md: "Restart reuses the session's terminal when it still exists —
// whatever scrollback the terminal itself retained is still there" — and,
// in the same paragraph, restart "does NOT preserve the previous run's
// last visible screen: the pane is blank until the new agent draws". So
// what survives a restart is HISTORY, not the visible grid: `respawn-pane`
// keeps tmux's scrollback and reinitializes the screen. The marker is
// therefore pushed off the visible screen before the restart — `spam 60`
// exceeds the fitted browser terminal's height (about 45 rows at most in
// this suite's 1280x720 viewports) — so that its survival is the retained
// scrollback and nothing else. Asserted from the BROWSER's own buffer
// after the restart's remount, which is the only view a user actually has
// of that promise: the buffer starts empty on remount, so everything in it
// afterwards came back through replay of the reused pane's scrollback.
//
// The marker is typed rather than taken from the startup banner, because
// both runs print the same banner — text only the FIRST run could have
// produced is what makes this about retention rather than about the new
// run having printed something.
test("a restarted session's terminal still shows the previous run's scrollback above the new one", async ({
  page,
  request,
}) => {
  const title = `restart-scrollback-${Date.now()}`;
  try {
    await page.goto("/");
    // Restart only resumes, so the session reports a conversation as it
    // starts; the create form cannot declare a command's agent type, so the
    // API creates it and the page opens it.
    const created = await createResumableSession(request, { cwd: "/tmp", title });
    await rowByTitle(page, title).locator(".session-row-open").click();
    await waitForSessionRevealed(page, created.id);
    await waitForTermText(page, "FAKE-AGENT READY");

    await page.locator("#terminal").click();
    await page.keyboard.type("PRIOR-RUN-MARKER");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:PRIOR-RUN-MARKER");
    // Push the marker into tmux's history: more lines than any terminal
    // this test runs in has rows. The last spam line's arrival is the
    // barrier, so the restart cannot land while the marker is still on
    // the visible grid that the respawn wipes.
    await page.keyboard.type("spam 60");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-60");
    // And one marker that stays on the VISIBLE GRID only, typed after the
    // spam so nothing ever scrolls it into history. Its absence after the
    // restart is the other half of the contract: finding it would mean
    // something preserved the visible grid across the respawn — the
    // forbidden shrink-and-restore trick.
    await page.keyboard.type("GRID-ONLY-MARKER");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:GRID-ONLY-MARKER");

    await restartIdleAgent(page, { afterInput: true });

    // Three facts at once, and in order: the prior run's typed marker came
    // back out of retained scrollback, the new run's banner is BELOW it,
    // and the grid-only marker did NOT come back. A plain "contains both"
    // would also pass if the restart had never happened (the pre-restart
    // buffer contains both too), so the anchor is the marker's position
    // relative to the LAST banner — and the buffer starts empty on the
    // restart's remount, so a grid-only marker in it could only have come
    // from the replay, never from the pre-restart screen.
    await expect
      .poll(
        async () => {
          const text = await termText(page);
          const marker = text.indexOf("PRIOR-RUN-MARKER");
          const banner = text.lastIndexOf("FAKE-AGENT READY");
          return marker >= 0 && banner > marker && !text.includes("GRID-ONLY-MARKER");
        },
        {
          timeout: 30_000,
          message:
            "prior run's retained scrollback (and nothing from its visible grid) above the relaunched agent's output",
        },
      )
      .toBe(true);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// A restart whose RESPONSE is lost is not a restart that did not happen:
// the request reaches the supervisor, the agent is relaunched, and only
// the reply dies on the way back. The view has to recover from that on its
// own, because the server has already torn its attachment down — a client
// that treated the failure as "nothing happened" would leave the user
// staring at a permanently detached terminal for a session that is running
// perfectly well.
//
// `route.fetch()` then `route.abort()` reproduces exactly that: the real
// request is performed, and the page sees a network error instead of its
// answer.
test("a restart whose response is lost still recovers the terminal", async ({
  page,
  request,
}) => {
  const title = `restart-lost-reply-${Date.now()}`;
  await page.route("**/api/sessions/*/restart", async (route) => {
    await route.fetch();
    await route.abort("connectionfailed");
  });

  try {
    await page.goto("/");
    // Restart only resumes, so the session reports a conversation as it
    // starts; the create form cannot declare a command's agent type, so the
    // API creates it and the page opens it.
    const created = await createResumableSession(request, { cwd: "/tmp", title });
    await rowByTitle(page, title).locator(".session-row-open").click();
    await waitForSessionRevealed(page, created.id);
    await waitForTermText(page, "FAKE-AGENT READY");

    await page.locator("#terminal").click();
    await page.keyboard.type("BEFORE-LOST-RESTART");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:BEFORE-LOST-RESTART");

    // The marker anchors the ordering assertion below, so it has to
    // survive the respawn — and only tmux history survives one; the
    // visible grid, where the marker still sits, is wiped (SPEC.md,
    // Lifecycle operations/Restart). Scroll it off-screen first.
    await page.keyboard.type("spam 60");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-60");

    await restartIdleAgent(page, { afterInput: true });

    // The failure is surfaced rather than swallowed — the user is owed
    // that much when their action's outcome is genuinely unknown to the
    // client.
    await expect(page.locator(".restart-error")).toBeVisible({ timeout: 15_000 });

    // ...and the view recovers anyway: it re-reads the session, remounts,
    // and the relaunched agent's own banner appears BELOW the previous
    // run's output in the reused terminal. A view that had concluded
    // "nothing happened" would sit detached here forever.
    await expect
      .poll(
        async () => {
          const text = await termText(page);
          const marker = text.indexOf("BEFORE-LOST-RESTART");
          const banner = text.lastIndexOf("FAKE-AGENT READY");
          return marker >= 0 && banner > marker;
        },
        {
          timeout: 30_000,
          message: "the relaunched agent's terminal, recovered after a lost reply",
        },
      )
      .toBe(true);

    // The session really was restarted, which is what makes the recovery
    // the correct behavior rather than a lucky one.
    const listing = await (await request.get("/api/sessions")).json();
    const session = listing.sessions.find((s: any) => s.title === title);
    expect(session).toBeTruthy();
    expect(LIVE_STATES).toContain(session.status.state);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});

// MT-4 (manual testing): restarting a live session left the red "Detached:
// session restarted" banner painted over a terminal that had, by the time
// a human noticed it, already reattached and was working fine — typing
// round-tripped, output rendered, the banner just never went away. Root
// cause was `#term-banner` (farhelm-ui/src/lib.rs) living OUTSIDE the
// `#terminal` div terminal.js remounts, so nothing about a later mount
// ever told a PRIOR mount's sticky banner to clear (terminal.js's
// `showBanner` is deliberately sticky FOR THE LIFE OF ITS OWN SOCKET, so a
// takeover reason survives the generic close that follows it — see that
// function's docs — but nothing was clearing it for the NEXT socket
// either). The fix hooks the new socket's `onopen` — a transport-level
// signal (the upgrade completed), not proof the supervisor-side attach
// succeeded; clearing there is still honest because a failed attach
// closes that same socket and its own close handler re-banners.
//
// This is exactly the restart sequence that produces the bug: a live
// agent's restart tears the OLD attachment down with reason "session
// restarted" (`detach_for_restart`, farhelm-supervisor/src/service.rs)
// before the new one ever exists. Proving the banner APPEARED cannot be a
// locator poll — the fix clears it as soon as the new socket opens, which
// on loopback routinely beats Playwright's first poll, so the transient
// visible state is unobservable from outside (this test flaked exactly
// that way when it polled). A MutationObserver installed before the page
// loads records every banner transition instead, so the assertion reads
// the recorded history: shown with the exact restart reason, then hidden
// once the relaunch is confirmed live — the real sticky-then-clear
// sequence, with no window for the poll to miss.
test("a restarted session's banner clears once the new attachment is live", async ({
  page,
  request,
}) => {
  const title = `restart-banner-clears-${Date.now()}`;
  try {
    // Armed ON DEMAND rather than at DOMContentLoaded: auto-select mounts
    // a different session's view (and #term-banner) at load, and an
    // observer armed on THAT element would record the wrong terminal's
    // banner while the created session's went unobserved.
    await page.addInitScript(() => {
      (window as any).__bannerLog = [];
      (window as any).__armBannerLog = () => {
        const el = document.getElementById("term-banner");
        if (!el) throw new Error("no term-banner to observe");
        new MutationObserver(() => {
          (window as any).__bannerLog.push({
            shown: el.style.display === "block",
            text: el.textContent,
          });
        }).observe(el, {
          attributes: true,
          attributeFilter: ["style"],
          childList: true,
          characterData: true,
          subtree: true,
        });
      };
    });
    await page.goto("/");
    // Restart only resumes, so the session reports a conversation as it
    // starts; the create form cannot declare a command's agent type, so the
    // API creates it and the page opens it.
    const created = await createResumableSession(request, { cwd: "/tmp", title });
    await rowByTitle(page, title).locator(".session-row-open").click();
    await expect(page.locator(".titlebar .title")).toHaveText(title, { timeout: 15_000 });
    await waitForSessionRevealed(page, created.id);
    await waitForTermText(page, "FAKE-AGENT READY");

    // The count-of-two anchor at the end needs the FIRST run's banner in
    // tmux history, because the respawn wipes the visible grid where it
    // would otherwise still sit (SPEC.md, Lifecycle operations/Restart).
    // Done before arming the banner observer so the observer's window
    // stays tight around the restart itself.
    await page.locator("#terminal").click();
    await page.keyboard.type("spam 60");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "spam-line-60");

    await page.evaluate(() => (window as any).__armBannerLog());

    await restartIdleAgent(page, { afterInput: true });

    // The banner's appearance is read from the observer's recorded
    // history (see the doc comment above for why a locator poll cannot
    // see it): the old attachment's detach must have painted the banner
    // with the restart's exact reason at SOME point, however briefly.
    await expect
      .poll(
        async () =>
          await page.evaluate(() =>
            (window as any).__bannerLog.some(
              (e: { shown: boolean; text: string }) =>
                e.shown && e.text.includes("Detached: session restarted"),
            ),
          ),
        { timeout: 15_000, message: "the restart's detach banner was recorded" },
      )
      .toBe(true);

    // The relaunch comes up in the SAME (reused) terminal, so its ready
    // banner is the SECOND occurrence in the buffer — the same anchor the
    // confirm test above uses to prove the new run actually printed
    // something rather than merely reattaching to stale output.
    await expect
      .poll(
        async () => (await termText(page)).split("FAKE-AGENT READY").length - 1,
        { timeout: 30_000, message: "the relaunched agent's own ready banner" },
      )
      .toBeGreaterThanOrEqual(2);

    // The bug: the OLD attachment's detach banner stayed painted over a
    // terminal that is now genuinely live again.
    await expect(page.locator("#term-banner")).toBeHidden({ timeout: 15_000 });

    // And "live" is proven functionally, not just by the banner's
    // absence: typing still round-trips through the new attachment.
    await page.locator("#terminal").click();
    await page.keyboard.type("post-restart-roundtrip");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:post-restart-roundtrip", 10_000);
  } finally {
    const id = await findSessionIdByTitle(request, title).catch(() => undefined);
    if (id) {
      await cleanupSession(request, id);
    }
  }
});
