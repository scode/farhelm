/** Stop's consent belongs to the rendered agent and tab state. These cases use real
 * tabs and Stop requests; only the partial-notice case supplies a reply the
 * browser cannot reliably induce by making a terminal host unreadable. */
import type { APIRequestContext, Page } from "@playwright/test";
import { expect, test } from "./helpers/evidence";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import {
  cleanupSession, createResumableSession, createSession, listSessions, observeFeedReaders, openRowMenu,
  patchPreferences, pinAutoSelect, readPreferences, stopSession, stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { attachSession, waitForTermText } from "./helpers/term";
import { routeGate } from "./helpers/route-gate";
import { drainedRoute } from "./helpers/drained-route";

type Surface = "row" | "header";

/** A browser left open across an update still sends Stop without a body.
 * Omission must preserve its old agent-only authority; explicit tab consent
 * then proves that a refusal is the guard rather than an unusable endpoint. */
test("legacy HTTP Stop refuses tabs until tab consent is explicit", async ({ page, request }) => {
  const session = await createSession(request, { title: "legacy-stop-consent" });
  let bodyFailed = false;
  try {
    await prepare(page, request, session.id, false, 1);
    const url = `/api/sessions/${encodeURIComponent(session.id)}/stop`;
    for (const options of [undefined, { data: {} }]) {
      const refused = await request.post(url, options);
      expect(refused.status()).toBe(409);
      expect(await refused.text()).toContain("terminal tabs");
      const kept = (await listSessions(request)).sessions.find((entry) => entry.id === session.id);
      expect(kept?.status?.state).toMatch(/^(running|waiting|idle)$/);
      expect(kept?.tabs?.length).toBe(1);
    }
    const allowed = await request.post(url, { data: { refuse_if_tabs_open: false, refuse_if_agent_running: false } });
    expect(allowed.ok(), await allowed.text()).toBe(true);
    await expect.poll(async () => {
      const listed = (await listSessions(request)).sessions.find((entry) => entry.id === session.id);
      return [listed?.status?.state, listed?.tabs?.length ?? 0];
    }, { timeout: 20_000 }).toEqual(["exited", 0]);
  } catch (error) { bodyFailed = true; throw error; }
  finally {
    await cleanupOwnedSteps(bodyFailed, [["owned session", () => cleanupSession(request, session.id)]]);
  }
});
type StopGuards = { refuse_if_tabs_open: boolean; refuse_if_agent_running: boolean };

/** A running row can still describe the launch shim. Observe the real agent's
 * output through this page's attachment before Stop establishes a tabs-only
 * fixture; otherwise stopping startup can correctly classify a launch error.
 * Opening tabs afterwards also exercises the supported post-Stop path. */
async function prepare(page: Page, request: APIRequestContext, id: string, ended: boolean, tabs: number) {
  await page.goto("/");
  await attachSession(page, id);
  await waitForTermText(page, "FAKE-AGENT READY");
  await expect.poll(async () => (await listSessions(request)).sessions.find((session) => session.id === id)?.status?.state,
    { timeout: 20_000, message: "the fixture agent must start before lifecycle actions" }).toMatch(/^(running|waiting|idle)$/);
  if (ended) {
    await stopSession(request, id);
    await expect.poll(async () => (await listSessions(request)).sessions.find((session) => session.id === id)?.status?.state,
      { timeout: 20_000 }).toBe("exited");
  }
  for (let n = 0; n < tabs; n++) await openTab(request, id);
  expect((await listSessions(request)).sessions.find((session) => session.id === id)?.tabs?.length ?? 0).toBe(tabs);
}

/** Opening finishes on the supervisor before the helm's merged listing catches
 * up. Wait for this exact tab there before testing consent from that listing. */
async function openTab(request: APIRequestContext, id: string) {
  const reply = await request.post(`/api/sessions/${encodeURIComponent(id)}/tabs`);
  expect(reply.ok(), await reply.text()).toBe(true);
  const tabId = (await reply.json()).tab.id;
  await expect.poll(async () => {
    const session = (await listSessions(request)).sessions.find((entry) => entry.id === id);
    return session?.tabs?.some((tab) => tab.id === tabId) ?? false;
  }, { timeout: 20_000, message: "the opened tab must reach the helm listing before consent is tested" }).toBe(true);
}

/** A successful Restart reply alone does not prove the replacement agent
 * runs. Keep the live status and retained tab count in the readiness oracle
 * so a failure identifies which race premise was missing. */
async function restartBehindDisplay(request: APIRequestContext, id: string, tabs: number) {
  const reply = await request.post(`/api/sessions/${encodeURIComponent(id)}/restart`, { data: { stop_if_running: true } });
  expect(reply.ok(), await reply.text()).toBe(true);
  await expect.poll(async () => {
    const session = (await listSessions(request)).sessions.find((entry) => entry.id === id);
    return `${session?.status?.state}/${session?.tabs?.length ?? 0}`;
  }, { timeout: 20_000, message: "the competing agent must run with its existing tabs before Stop" }).toMatch(new RegExp(`^(running|waiting|idle)/${tabs}$`));
}

/** Freeze unsolicited refreshes after a settled initial render. Later API
 * changes can then disagree with that render without a timing-dependent race. */
async function load(page: Page, id: string) {
  await observeFeedReaders(page);
  const feed = await stubFeed(page);
  await pinAutoSelect(page, id);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);
  await attachSession(page, id);
  await waitForFeedReadersSettled(page);
  return feed;
}

/** Both surfaces reach the same Stop contract through their ordinary controls. */
async function clickStop(page: Page, id: string, surface: Surface) {
  if (surface === "header") await page.locator(".header-stop").click();
  else {
    const row = page.locator(`.session-row[data-session-id="${id}"]`);
    await openRowMenu(row);
    await row.locator(".session-row-stop").click();
  }
}

/** Keep the operation and its confirmation distinct so cancellation and focus
 * assertions inspect the same prompt the user answers. */
function question(page: Page, id: string, surface: Surface) {
  return surface === "header" ? page.locator(".header-stop-confirm")
    : page.locator(`.session-row[data-session-id="${id}"] .session-row-menu-panel.menu-prompt`);
}

for (const surface of ["row", "header"] as const) {
  for (const ended of [false, true]) {
    /** A prompt names the visible tabs, permits cancellation, and authorizes
     * every tab at dispatch, including one another client opened meanwhile.
     * Tabs-only wording must not claim the already-ended agent is running. */
    test(`${surface} Stop confirms ${ended ? "tabs only" : "agent and tabs"} and closes a later tab`, async ({ page, request }) => {
      const previous = await readPreferences(request);
      const session = await createSession(request, { title: `stop-${surface}-${ended}` });
      let bodyFailed = false;
      try {
        await patchPreferences(request, { skip_session_confirmations: false });
        await prepare(page, request, session.id, ended, 1);
        const feed = await load(page, session.id);
        await expect(page.locator(".tab-strip .tab-slot")).toHaveCount(1);
        const sent: StopGuards[] = [];
        page.on("request", (outgoing) => {
          if (new URL(outgoing.url()).pathname === `/api/sessions/${encodeURIComponent(session.id)}/stop`) {
            sent.push(outgoing.postDataJSON());
          }
        });
        await clickStop(page, session.id, surface);
        const prompt = question(page, session.id, surface);
        const answer = prompt.locator(surface === "header" ? ".header-stop-confirm-submit" : ".confirm-stop");
        const text = ended ? "Close 1 terminal tab?" : "Stop the agent and close 1 terminal tab?";
        await expect(prompt.locator(".confirm-consequence")).toHaveText(text);
        await expect(answer).toBeFocused();
        if (surface === "header") {
          // The open header question already owns this session, even before
          // its answer dispatches a request. A row action cannot bypass it.
          const source = page.locator(`.session-row[data-session-id="${session.id}"]`);
          await openRowMenu(source);
          await expect(source.locator(".session-row-stop")).toBeDisabled();
          await expect(source.locator(".session-row-delete")).toBeDisabled();
          await source.locator(".session-row-stop").dispatchEvent("click");
          await source.locator(".session-row-delete").dispatchEvent("click");
          await expect(prompt.locator(".confirm-consequence")).toHaveText(text);
          expect(sent).toEqual([]);
          await source.locator(".session-row-menu").click();
        }
        await prompt.getByRole("button", { name: "Cancel", exact: true }).click();
        await expect(prompt).toHaveCount(0);
        expect(sent).toEqual([]);
        expect((await listSessions(request)).sessions.find((entry) => entry.id === session.id)?.tabs?.length).toBe(1);
        await clickStop(page, session.id, surface);
        await expect(prompt.locator(".confirm-consequence")).toHaveText(text);
        await openTab(request, session.id);
        expect((await listSessions(request)).sessions.find((entry) => entry.id === session.id)?.tabs?.length).toBe(2);
        await expect(prompt.locator(".confirm-consequence")).toHaveText(text);
        await answer.click();
        await expect.poll(() => sent).toEqual([{ refuse_if_tabs_open: false, refuse_if_agent_running: ended }]);
        await expect.poll(async () => {
          const listed = (await listSessions(request)).sessions.find((entry) => entry.id === session.id);
          return [listed?.status?.state, listed?.tabs?.length ?? 0];
        }, { timeout: 20_000 }).toEqual(["exited", 0]);
        if (surface === "row") feed.notify(2);
        await expect(prompt).toHaveCount(0);
        await expect(page.locator(".tab-strip .tab-slot")).toHaveCount(0);
      } catch (error) { bodyFailed = true; throw error; }
      finally {
        await cleanupOwnedSteps(bodyFailed, [
          ["owned session", () => cleanupSession(request, session.id)],
          ["session confirmations", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
        ]);
      }
    });
  }

  for (const skip of [false, true]) {
    /** A click drawn without tabs never authorizes a later tab, even with
     * confirmations skipped. The real supervisor refuses before stopping the
     * agent; the UI shows that refusal and refreshes without opening a prompt. */
    test(`${surface} tabless Stop retains its precondition with skip ${skip}`, async ({ page, request }) => {
      const previous = await readPreferences(request);
      const session = await createSession(request, { title: `stop-guard-${surface}-${skip}` });
      let bodyFailed = false;
      try {
        await patchPreferences(request, { skip_session_confirmations: skip });
        await prepare(page, request, session.id, false, 0);
        await load(page, session.id);
        await expect(page.locator(".tab-strip .tab-slot")).toHaveCount(0);
        await openTab(request, session.id);
        expect((await listSessions(request)).sessions.find((entry) => entry.id === session.id)?.tabs?.length).toBe(1);
        const reply = page.waitForResponse((response) => new URL(response.url()).pathname === `/api/sessions/${encodeURIComponent(session.id)}/stop`);
        await clickStop(page, session.id, surface);
        const refused = await reply;
        expect(refused.request().postDataJSON()).toEqual({ refuse_if_tabs_open: true, refuse_if_agent_running: false });
        expect(refused.status()).toBe(409);
        const error = surface === "header" ? page.locator(".stop-error")
          : page.locator(`.session-row[data-session-id="${session.id}"] .action-error`);
        await expect(error).toContainText("Stop:");
        await expect(question(page, session.id, surface)).toHaveCount(0);
        const kept = (await listSessions(request)).sessions.find((entry) => entry.id === session.id);
        expect(kept?.status?.state).toMatch(/^(running|waiting|idle)$/);
        expect(kept?.tabs?.length).toBe(1);
        // Header Stop refreshes its detail; row Stop refreshes the listing.
        if (surface === "header") await expect(page.locator(".tab-strip .tab-slot")).toHaveCount(1);
        await waitForFeedReadersSettled(page);
      } catch (error) { bodyFailed = true; throw error; }
      finally {
        await cleanupOwnedSteps(bodyFailed, [
          ["owned session", () => cleanupSession(request, session.id)],
          ["session confirmations", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
        ]);
      }
    });
  }

  /** A partial Stop result must outlive the source view. Hold the real Stop's
   * reply across two switches, then report its injected failed-tab notice on
   * the row and on the remounted header. Unreadable-host cleanup has Rust coverage. */
  test(`${surface} Stop keeps its partial notice after switching sessions`, async ({ page, request }) => {
    const previous = await readPreferences(request);
    const session = await createSession(request, { title: `stop-notice-${surface}` });
    let other: Awaited<ReturnType<typeof createSession>> | undefined;
    const held = routeGate();
    let disposeRoute: (() => Promise<void>) | undefined;
    let bodyFailed = false;
    try {
      other = await createSession(request, { title: `stop-notice-other-${surface}` });
      await patchPreferences(request, { skip_session_confirmations: true });
      await prepare(page, request, session.id, false, 1);
      await load(page, session.id);
      await expect(page.locator(".tab-strip .tab-slot")).toHaveCount(1);
      const build = requireHelmBuild(await request.get("/api/hosts"), "Stop notice");
      const sent: StopGuards[] = [];
      let replyHeld = false;
      disposeRoute = await drainedRoute(page, (url) => url.pathname === `/api/sessions/${encodeURIComponent(session.id)}/stop`, async (route) => {
        sent.push(route.request().postDataJSON());
        const stopped = await route.fetch({ maxRetries: 0 });
        expect(stopped.ok(), await stopped.text()).toBe(true);
        replyHeld = true;
        await held.wait();
        await route.fulfill({ status: 200, headers: { "content-type": "application/json", "x-farhelm-build": build },
          json: { notice: "Terminal 2 could not be closed: host unavailable" } });
      });
      await clickStop(page, session.id, surface);
      await expect.poll(() => replyHeld, { message: "the real Stop finished, but this page has not received its outcome" }).toBe(true);
      const source = page.locator(`.session-row[data-session-id="${session.id}"]`);
      const sibling = page.locator(`.session-row[data-session-id="${other.id}"]`);
      await expect(source).toContainText("Stopping…");
      await sibling.locator(".session-row-open").click();
      await expect(sibling).toHaveAttribute("data-session-selected", "true");
      await waitForTermText(page, "FAKE-AGENT READY");
      await source.locator(".session-row-open").click();
      await expect(page.locator(".session-topbar")).toContainText("Stopping…");
      await expect(page.locator(".header-stop")).toBeDisabled();
      await page.locator(".header-stop").dispatchEvent("click");
      await sibling.locator(".session-row-open").click();
      await expect(sibling).toHaveAttribute("data-session-selected", "true");
      held.release();
      await expect(source.locator(".action-error")).toContainText("Terminal 2 could not be closed: host unavailable");
      await expect(sibling).toHaveAttribute("data-session-selected", "true");
      await source.locator(".session-row-open").click();
      await expect(page.locator(".stop-error")).toContainText("Terminal 2 could not be closed: host unavailable");
      expect(sent).toEqual([{ refuse_if_tabs_open: false, refuse_if_agent_running: false }]);
      await expect(question(page, session.id, surface)).toHaveCount(0);
      await expect(page.locator(`.session-row[data-session-id="${session.id}"]`)).toBeVisible();
      expect((await listSessions(request)).sessions.find((entry) => entry.id === session.id)?.status?.state).toBe("exited");
    } catch (error) { bodyFailed = true; throw error; }
    finally {
      held.release();
      await cleanupOwnedSteps(bodyFailed, [
        ["held Stop route", () => disposeRoute?.()],
        ["owned session", () => cleanupSession(request, session.id)],
        ["other session", () => other && cleanupSession(request, other.id)],
        ["session confirmations", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
      ]);
    }
  });
}

for (const skip of [false, true]) {
  /** One tabbed target asks for the whole batch. Its tab authority must not
   * expand the tabless target's consent, even when the question is skipped.
   * A real late tab makes only that target refuse; the other Stop completes. */
  test(`bulk Stop carries each target's tab precondition with skip ${skip}`, async ({ page, request }) => {
    const previous = await readPreferences(request);
    const ids: string[] = [];
    let bodyFailed = false;
    try {
      await patchPreferences(request, { skip_session_confirmations: skip });
      for (const tabs of [0, 1]) {
        const session = await createSession(request, { title: `bulk-stop-tabs-${tabs}` });
        ids.push(session.id);
        await prepare(page, request, session.id, false, tabs);
      }
      await load(page, ids[0]);
      for (const id of ids) await page.locator(`.session-row[data-session-id="${id}"] .session-row-open`).click({ modifiers: ["Control"] });
      await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "2");
      const sent = new Map<string, StopGuards>();
      page.on("request", (outgoing) => {
        const path = new URL(outgoing.url()).pathname;
        for (const id of ids) if (path === `/api/sessions/${encodeURIComponent(id)}/stop`) sent.set(id, outgoing.postDataJSON());
      });
      await openTab(request, ids[0]);
      expect((await listSessions(request)).sessions.find((entry) => entry.id === ids[0])?.tabs?.length).toBe(1);
      await page.locator(".marked-stop").click();
      const dialog = page.getByRole("dialog", { name: "Stop 2 sessions?", exact: true });
      if (!skip) {
        await expect(dialog).toBeVisible();
        await expect(dialog.locator(`[data-delete-target="${ids[1]}"] .bulk-delete-consequence`)).toHaveText("Stop the agent and close 1 terminal tab?");
        await dialog.getByRole("button", { name: "Stop sessions", exact: true }).click();
      }
      await expect.poll(() => sent.size).toBe(2);
      expect(sent.get(ids[0])).toEqual({ refuse_if_tabs_open: true, refuse_if_agent_running: false });
      expect(sent.get(ids[1])).toEqual({ refuse_if_tabs_open: false, refuse_if_agent_running: false });
      await expect(page.locator(`.session-row[data-session-id="${ids[0]}"] .action-error`)).toContainText("Stop:");
      await expect(page.locator(".marked-bar")).toHaveCount(0);
      const listed = (await listSessions(request)).sessions;
      expect(listed.find((entry) => entry.id === ids[0])?.status?.state).toMatch(/^(running|waiting|idle)$/);
      expect(listed.find((entry) => entry.id === ids[0])?.tabs?.length).toBe(1);
      expect(listed.find((entry) => entry.id === ids[1])?.status?.state).toBe("exited");
      expect(listed.find((entry) => entry.id === ids[1])?.tabs?.length ?? 0).toBe(0);
      await expect(dialog).toHaveCount(0);
    } catch (error) { bodyFailed = true; throw error; }
    finally {
      await cleanupOwnedSteps(bodyFailed, [
        ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
        ["session confirmations", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
      ]);
    }
  });
}

for (const surface of ["row", "header"] as const) {
  for (const skip of [false, true]) {
    for (const tabs of [0, 1]) {
      /** Ended-agent consent is independent of tab consent. Restart from a
       * different client while this surface still displays the ended agent:
       * answered tabs-only, skipped tabs-only and direct tabless clicks must
       * all refuse before stopping the replacement agent or closing any tab. */
      test(`${surface} ended-display Stop refuses a restarted agent with ${tabs} tabs and skip ${skip}`, async ({ page, request }) => {
        const previous = await readPreferences(request);
        const session = await createResumableSession(request, { title: `stop-agent-guard-${surface}-${tabs}-${skip}` });
        let bodyFailed = false;
        try {
          await patchPreferences(request, { skip_session_confirmations: skip });
          await prepare(page, request, session.id, true, tabs);
          await load(page, session.id);
          const row = page.locator(`.session-row[data-session-id="${session.id}"]`);
          await expect(row.locator(".status-badge")).toContainText("Exited");
          await expect(page.locator(".session-details .status-badge")).toContainText("Exited");
          await expect(page.locator(".tab-strip .tab-slot")).toHaveCount(tabs);
          const prompt = question(page, session.id, surface);
          if (tabs > 0 && !skip) {
            await clickStop(page, session.id, surface);
            await expect(prompt.locator(".confirm-consequence")).toHaveText("Close 1 terminal tab?");
          }
          await restartBehindDisplay(request, session.id, tabs);
          // These are the facts authorizing the next click. The real listing
          // above must disagree, rather than a mocked refusal hiding the gap.
          await expect(row.locator(".status-badge")).toContainText("Exited");
          await expect(page.locator(".session-details .status-badge")).toContainText("Exited");
          const response = page.waitForResponse((reply) => new URL(reply.url()).pathname === `/api/sessions/${encodeURIComponent(session.id)}/stop`);
          if (tabs > 0 && !skip) {
            await expect(prompt.locator(".confirm-consequence")).toHaveText("Close 1 terminal tab?");
            await prompt.locator(surface === "header" ? ".header-stop-confirm-submit" : ".confirm-stop").click();
          } else await clickStop(page, session.id, surface);
          const refused = await response;
          expect(refused.request().postDataJSON()).toEqual({ refuse_if_tabs_open: tabs === 0, refuse_if_agent_running: true });
          expect(refused.status()).toBe(409);
          const error = surface === "header" ? page.locator(".stop-error") : row.locator(".action-error");
          await expect(error).toContainText("Stop:");
          await expect(prompt).toHaveCount(0);
          const kept = (await listSessions(request)).sessions.find((entry) => entry.id === session.id);
          expect(kept?.status?.state).toMatch(/^(running|waiting|idle)$/);
          expect(kept?.tabs?.length ?? 0).toBe(tabs);
        } catch (error) { bodyFailed = true; throw error; }
        finally {
          await cleanupOwnedSteps(bodyFailed, [
            ["owned session", () => cleanupSession(request, session.id)],
            ["session confirmations", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
          ]);
        }
      });
    }
  }
}

for (const skip of [false, true]) {
  /** A batch's live target cannot grant agent-stop consent to ended targets.
   * Restart both ended targets after display, including the tabless one,
   * and keep the real live target to prove refusals remain target-local. */
  test(`bulk ended-display Stop preserves restarted agents and tabs with skip ${skip}`, async ({ page, request }) => {
    const previous = await readPreferences(request);
    const ids: string[] = [];
    let bodyFailed = false;
    try {
      await patchPreferences(request, { skip_session_confirmations: skip });
      for (const [ended, tabs] of [[true, 0], [true, 1], [false, 1]] as const) {
        const session = await createResumableSession(request, { title: `bulk-stop-agent-${ended}-${tabs}` });
        ids.push(session.id);
        await prepare(page, request, session.id, ended, tabs);
      }
      await load(page, ids[0]);
      for (const id of ids) await page.locator(`.session-row[data-session-id="${id}"] .session-row-open`).click({ modifiers: ["Control"] });
      await expect(page.locator(".marked-bar")).toHaveAttribute("data-marked-count", "3");
      const sent = new Map<string, StopGuards>();
      page.on("request", (outgoing) => {
        const path = new URL(outgoing.url()).pathname;
        for (const id of ids) if (path === `/api/sessions/${encodeURIComponent(id)}/stop`) sent.set(id, outgoing.postDataJSON());
      });
      const dialog = page.getByRole("dialog", { name: "Stop 3 sessions?", exact: true });
      if (!skip) {
        await page.locator(".marked-stop").click();
        await expect(dialog.locator(`[data-delete-target="${ids[1]}"] .bulk-delete-consequence`)).toHaveText("Close 1 terminal tab?");
      }
      await restartBehindDisplay(request, ids[0], 0);
      await restartBehindDisplay(request, ids[1], 1);
      for (const id of ids.slice(0, 2)) await expect(page.locator(`.session-row[data-session-id="${id}"] .status-badge`)).toContainText("Exited");
      if (skip) await page.locator(".marked-stop").click();
      else {
        await expect(dialog.locator(`[data-delete-target="${ids[1]}"] .bulk-delete-consequence`)).toHaveText("Close 1 terminal tab?");
        await dialog.getByRole("button", { name: "Stop sessions", exact: true }).click();
      }
      await expect.poll(() => sent.size).toBe(3);
      expect(sent.get(ids[0])).toEqual({ refuse_if_tabs_open: true, refuse_if_agent_running: true });
      expect(sent.get(ids[1])).toEqual({ refuse_if_tabs_open: false, refuse_if_agent_running: true });
      expect(sent.get(ids[2])).toEqual({ refuse_if_tabs_open: false, refuse_if_agent_running: false });
      for (const id of ids.slice(0, 2)) await expect(page.locator(`.session-row[data-session-id="${id}"] .action-error`)).toContainText("Stop:");
      await expect(page.locator(".marked-bar")).toHaveCount(0);
      const listed = (await listSessions(request)).sessions;
      for (const [index, tabs] of [0, 1].entries()) {
        const kept = listed.find((entry) => entry.id === ids[index]);
        expect(kept?.status?.state).toMatch(/^(running|waiting|idle)$/);
        expect(kept?.tabs?.length ?? 0).toBe(tabs);
      }
      expect(listed.find((entry) => entry.id === ids[2])?.status?.state).toBe("exited");
      expect(listed.find((entry) => entry.id === ids[2])?.tabs?.length ?? 0).toBe(0);
      await expect(dialog).toHaveCount(0);
    } catch (error) { bodyFailed = true; throw error; }
    finally {
      await cleanupOwnedSteps(bodyFailed, [
        ...ids.map((id): [string, () => Promise<void>] => ["owned session", () => cleanupSession(request, id)]),
        ["session confirmations", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
      ]);
    }
  });
}
