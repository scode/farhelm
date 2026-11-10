/** A submitted launcher owns its request without owning the rest of the page.
 * Real terminals and accepted creates distinguish usable navigation from a
 * backdrop that merely looks transparent or a request cancelled by switching. */
import { expect, test } from "./helpers/evidence";
import type { JSHandle, Page } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import {
  cleanupSession,
  createResumableSession,
  createSession,
  FAKE_AGENT,
  listHosts,
  listSessions,
  localHostId,
  observeFeedReaders,
  openRowMenu,
  patchPreferences,
  pinAutoSelect,
  readPreferences,
  readFeedReaders,
  showHostsPanel,
  showSessionsPanel,
  stubFeed,
  waitForFeedReadersSettled,
} from "./helpers/fleet";
import { answerYolo, attachSession, fillCreateForm, terminalInput, terminalScreen, waitForTermText } from "./helpers/term";
import { waitForSessionReady, waitForSessionRevealed } from "./helpers/terminal-readiness";
import { drainedRoute } from "./helpers/drained-route";
import { routeGate } from "./helpers/route-gate";
import { requireHelmBuild } from "./helpers/helm-build";
import { selectSessionActionWith } from "./helpers/session-actions";
import { type InputFrame, programInput, recordInput } from "./helpers/program-input";

/** Watch only B's real attachment, so a focused field cannot stand in for input
 * reaching the session the user kept. Register before its first attachment. */
function recordSessionInput(page: Page, id: string): InputFrame[] {
  const frames: InputFrame[] = [];
  page.on("websocket", (socket) => {
    if (new URL(socket.url()).pathname === `/api/sessions/${encodeURIComponent(id)}/term`) {
      recordInput(socket, frames);
    }
  });
  return frames;
}

/** Cross reply effects and their scheduled focus handoff before the next key.
 * Attachment readiness is a separate premise; two frames observe only the
 * mounted completion's chance to move focus or dismiss another surface. */
async function settleCompletion(page: Page): Promise<void> {
  await page.evaluate(() => new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  }));
}

/** Holding a create POST must leave both navigation paths and terminal input
 * usable. Queued events against the hidden controls must still be refused:
 * visibility is presentation, not the lifetime or admission guard. B lives on
 * another install so navigation also challenges New's implicit host default. */
test(
  "a pending New keeps one immutable draft while sidebar, switcher and terminal work",
  async ({ page, request }, testInfo) => {
    const prefix = `pending-new-${Date.now()}`;
    const owned: string[] = [];
    const previous = await readPreferences(request);
    const gate = routeGate();
    const bodies: any[] = [];
    let stopRoute: (() => Promise<void>) | undefined;
    let stopOtherRoute: (() => Promise<void>) | undefined;
    let otherStops = 0;
    try {
      const local = await localHostId(request);
      const hosts = await listHosts(request);
      const origin = hosts.find((host) => host.id === local);
      const remote = hosts.find((host) => host.kind === "ssh" && host.state.phase === "connected" && host.id !== local);
      expect(origin?.identity, "the launch origin has a recorded installation identity").toBeTruthy();
      expect(origin?.state.phase).toBe("connected");
      expect(remote?.identity, "B belongs to the stack's connected remote installation").toBeTruthy();
      expect(remote!.identity).not.toBe(origin!.identity);
      const first = await createSession(request, { title: `${prefix}-first`, host: local });
      owned.push(first.id);
      const other = await createSession(request, { title: `${prefix}-other`, host: remote!.id });
      owned.push(other.id);
      const saved = await request.put(`/api/templates/${prefix}`, {
        data: {
          kind: "command",
          command: "true",
          yolo: false,
          name: "must not replace the draft",
          destination: { folder: "/tmp" },
        },
      });
      expect(saved.ok(), await saved.text()).toBe(true);
      await patchPreferences(request, {
        quick_launch_template: prefix,
        template_shortcuts: { "1": prefix },
        skip_session_confirmations: true,
      });
      const build = requireHelmBuild(await request.get("/api/hosts"), "held launcher refusal");
      stopRoute = await drainedRoute(page, "**/api/sessions", async (route) => {
        if (route.request().method() !== "POST") return route.continue();
        bodies.push(route.request().postDataJSON());
        await gate.wait();
        await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "held create refused" });
      });
      stopOtherRoute = await drainedRoute(
        page,
        `**/api/sessions/${encodeURIComponent(other.id)}/stop`,
        async (route) => {
          expect(route.request().method()).toBe("POST");
          otherStops++;
          await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "independent Stop refused" });
        },
      );
      await pinAutoSelect(page, first.id);
      await page.goto("/");
      await attachSession(page, first.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      const form = await fillCreateForm(page, { cwd: "/tmp", invocation: FAKE_AGENT, title: prefix });
      const originalHost = await form.getByRole("combobox", { name: "Host", exact: true }).textContent();
      await expect(form).toHaveAttribute("data-folder-check-pending", "false");
      await form.locator(".create-session-submit").click();
      await expect.poll(() => bodies.length, { message: "one create is captured before exercising the page" }).toBe(1);
      expect(bodies[0]).toMatchObject({ host: local, expected_incarnation: origin!.incarnation });
      const submitted = JSON.stringify(bodies[0]);
      await expect(form).toHaveCount(1);
      await expect(form).toBeHidden();
      await expect(form).not.toHaveAttribute("aria-modal", "true");

      await terminalScreen(page).click();
      await expect(terminalInput(page)).toBeFocused();
      await page.keyboard.type("input-during-create");
      await page.keyboard.press("Enter");
      await waitForTermText(page, "echo:input-during-create");
      const otherRow = page.locator(`[data-session-id="${other.id}"]`);
      await otherRow.locator(".session-row-open").click();
      await expect(otherRow).toHaveAttribute("data-session-selected", "true");
      await waitForSessionReady(page, other.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      await expect(page.locator(".header-stop")).toBeEnabled();
      await page.locator(".header-stop").click();
      await expect.poll(() => otherStops, {
        message: "B's Stop is admitted while New still holds its host-operation token",
      }).toBe(1);
      await expect(page.locator(".stop-error")).toContainText("independent Stop refused");
      await expect(form).toBeHidden();
      await expect(form.getByRole("combobox", { name: "Host", exact: true, includeHidden: true })).toHaveText(
        originalHost!,
      );
      expect(bodies).toHaveLength(1);
      await page.keyboard.press("Control+Shift+k");
      const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
      await expect(switcher.getByRole("combobox")).toBeFocused();
      await switcher.getByRole("combobox").fill(first.title);
      await expect(switcher.locator(".quick-switcher-results").getByRole("option")).toHaveCount(1);
      await page.keyboard.press("Enter");
      await expect(switcher).toHaveCount(0);
      await expect(page.locator(`[data-session-id="${first.id}"]`)).toHaveAttribute("data-session-selected", "true");
      await waitForSessionReady(page, first.id);

      // Dispatch queued relays as well as physical shortcuts: neither path may
      // replace the mounted request owner or enqueue work for after its reply.
      await page.locator(".new-session-button").dispatchEvent("click");
      await page.locator(".templates-button").dispatchEvent("click");
      await expect(page.locator(".templates-dialog")).toHaveCount(0);
      await page.keyboard.press("Control+Shift+Digit1");
      await page.keyboard.press("Control+Shift+x");
      await page.locator('.launch-sequence-trigger[data-sequence-action="begin"]').dispatchEvent("click");
      await page.locator('.template-number-trigger[data-number-key="1"]').dispatchEvent("click");
      await form.dispatchEvent("submit");
      await form.locator(".launch-composer-cancel").dispatchEvent("click");
      await expect(form).toHaveCount(1);
      await expect(form).toBeHidden();
      expect(bodies).toHaveLength(1);
      expect(JSON.stringify(bodies[0])).toBe(submitted);
      await page.keyboard.press("Control+Shift+k");
      await expect(switcher).toBeVisible();
      const replied = page.waitForResponse((response) =>
        response.request().method() === "POST" &&
        new URL(response.url()).pathname === "/api/sessions"
      );
      gate.release();
      await (await replied).finished();
      // Let the reply's reveal attempt cross its browser-frame boundary before
      // proving the switcher still owns the keyboard. Returning errors must wait
      // for this newer modal rather than isolating it beneath a second dialog.
      await page.evaluate(() =>
        new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
      );
      await expect(form).toBeHidden();
      await expect(form).not.toHaveAttribute("aria-modal", "true");
      await expect(switcher.getByRole("combobox")).toBeFocused();
      await switcher.getByRole("combobox").fill(`${prefix}-must-not-replace`);
      await expect(switcher.locator(".quick-switcher-new")).toHaveAttribute("aria-selected", "true");
      // This New pick closes the newer modal before its callback runs. The
      // returning draft must retain admission through that handoff as well.
      await page.keyboard.press("Enter");
      await expect(switcher).toHaveCount(0);
      await expect(form).toBeVisible();
      await expect(form).toHaveAttribute("aria-modal", "true");
      await expect(form.getByRole("alert")).toContainText("Not launched. held create refused");
      await expect(form.getByLabel("Name (optional)")).toHaveValue(prefix);
      await expect(form.getByLabel("Agent command")).toHaveValue(FAKE_AGENT);
      await expect(form.getByRole("combobox", { name: "Host", exact: true })).toHaveText(originalHost!);
      await expect.poll(() => form.evaluate((node) => node.contains(document.activeElement)), {
        message: "the returned editor owns focus before Tab is exercised",
      }).toBe(true);
      await page.keyboard.press("Tab");
      expect(await form.evaluate((node) => node.contains(document.activeElement)), "Tab stays in the restored modal")
        .toBe(true);
      expect(bodies).toHaveLength(1);
    } finally {
      gate.release();
      await stopRoute?.();
      await stopOtherRoute?.();
      await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
        ["preferences", () =>
          patchPreferences(request, {
            quick_launch_template: previous.quick_launch_template ?? null,
            template_shortcuts: previous.template_shortcuts ?? {},
            skip_session_confirmations: previous.skip_session_confirmations ?? false,
          })],
        ["template", async () => {
          const removed = await request.delete(`/api/templates/${prefix}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }],
        ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
      ]);
    }
  },
);

for (const retired of [false, true]) {
  /** Rename has its own editor lifetime, but must not retire a submitted launch.
   * Its newer modal owns focus until closed; the original refusal then returns
   * to the same draft. The adversarial variant revokes only the DOM admission
   * identity, exercising real denied-return settlement without a product hook. */
  test(
    retired
      ? "a retired launcher settles without displacing Rename"
      : "Rename during a pending launch preserves its refusal and draft",
    async ({ page, request }, testInfo) => {
      const name = `pending-rename-${Date.now()}`;
      const owned: string[] = [];
      const gate = routeGate();
      let calls = 0;
      let stopRoute: (() => Promise<void>) | undefined;
      try {
        const source = await createSession(request, { title: `${name}-source` });
        owned.push(source.id);
        const other = await createSession(request, { title: `${name}-other` });
        owned.push(other.id);
        const build = requireHelmBuild(await request.get("/api/hosts"), "Rename overlap refusal");
        stopRoute = await drainedRoute(page, "**/api/sessions", async (route) => {
          if (route.request().method() !== "POST") return route.continue();
          calls++;
          await gate.wait();
          await route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "Rename overlap refused" });
        });
        await pinAutoSelect(page, source.id);
        await page.goto("/");
        await attachSession(page, source.id);
        await waitForTermText(page, "FAKE-AGENT READY");
        const form = await fillCreateForm(page, { cwd: "/tmp", invocation: FAKE_AGENT, title: name });
        await expect(form).toHaveAttribute("data-folder-check-pending", "false");
        const opening = await form.getAttribute("data-opening-generation");
        expect(opening, "the submitted launcher has an opening identity").toBeTruthy();
        await form.locator(".create-session-submit").click();
        await expect.poll(() => calls, { message: "Rename is exercised after the launch was dispatched" }).toBe(1);
        await expect(form).toBeHidden();
        const progress = page.locator(".launch-progress-pill .launch-progress");
        await expect(progress).toBeVisible();
        await expect(progress).toContainText(name);
        const otherRow = page.locator(`.session-row[data-session-id="${other.id}"]`);
        await openRowMenu(otherRow);
        await otherRow.getByRole("menuitem", { name: "Rename", exact: true }).click();
        const rename = page.locator(".rename-dialog");
        await expect(rename).toBeVisible();
        await expect(rename.locator(".rename-input")).toBeFocused();
        await expect(progress).toBeVisible();
        await expect(form).toHaveAttribute("data-opening-generation", opening!);
        if (retired) {
          // This is an adversarial admission-fence premise, not a user opening a
          // different Rust draft. The real held reply must still settle its owner.
          await form.evaluate((node) => node.setAttribute("data-opening-generation", "retired"));
          await expect(form).toHaveAttribute("data-opening-generation", "retired");
        }
        const replied = page.waitForResponse((response) =>
          response.request().method() === "POST" && new URL(response.url()).pathname === "/api/sessions"
        );
        gate.release();
        await (await replied).finished();
        if (retired) {
          await expect(form).toHaveCount(0);
          await expect(page.locator(".launch-progress-pill")).toHaveCount(0);
          await expect(rename.locator(".rename-input")).toBeFocused();
          await rename.locator(".rename-cancel").click();
          await expect(rename).toHaveCount(0);
          await page.locator(".new-session-button").click();
          await expect(form).toBeVisible();
          await expect(form).toHaveAttribute("aria-modal", "true");
          await expect(form).not.toHaveAttribute("data-opening-generation", opening!);
          await expect(form.getByLabel("Name (optional)")).not.toHaveValue(name);
          await form.locator(".launch-composer-cancel").click();
          await expect(form).toHaveCount(0);
          expect(calls).toBe(1);
          return;
        }
        // Cross the reply's presentation boundary while Rename remains mounted,
        // rather than observing only the request before its return was processed.
        await page.evaluate(() =>
          new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
        );
        await expect(form).toBeHidden();
        await expect(form).not.toHaveAttribute("aria-modal", "true");
        await expect(rename.locator(".rename-input")).toBeFocused();
        await rename.locator(".rename-cancel").click();
        await expect(rename).toHaveCount(0);
        await expect(form).toBeVisible();
        await expect(form.getByRole("alert")).toContainText("Not launched. Rename overlap refused");
        await expect(form.getByLabel("Name (optional)")).toHaveValue(name);
        await expect(form.getByLabel("Agent command")).toHaveValue(FAKE_AGENT);
        await expect(form.locator(".create-session-submit")).toBeEnabled();
        await expect(page.locator(".launch-progress-pill")).toHaveCount(0);
        await expect.poll(() => form.evaluate((node) => node.contains(document.activeElement)), {
          message: "the returned refusal owns focus after Rename closes",
        }).toBe(true);
        expect(calls).toBe(1);
      } finally {
        gate.release();
        await stopRoute?.();
        await cleanupOwnedSteps(
          testInfo.status !== testInfo.expectedStatus,
          owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
        );
      }
    },
  );
}

for (const action of ["New", "Clone", "Replace with"] as const) {
  /** The accepted session must appear without taking the rest of B's typing.
   * New and Clone use intervening input; Replace with keeps its source-selected
   * rule. The socket and live echo distinguish focus protection from merely
   * preserving the sidebar's selection marker. */
  test(
    `${action} leaves typing in the selected session after switching during its request`,
    async ({ page, request }, testInfo) => {
      const replace = action === "Replace with";
      const prefix = `pending-selection-${action.replaceAll(" ", "-")}-${Date.now()}`;
      const owned: string[] = [];
      const gate = routeGate();
      const bodies: any[] = [];
      let stopRoute: (() => Promise<void>) | undefined;
      try {
        const source = await createSession(request, { title: `${prefix}-source` });
        owned.push(source.id);
        const other = await createSession(request, { title: `${prefix}-other` });
        owned.push(other.id);
        const frames = recordSessionInput(page, other.id);
        await pinAutoSelect(page, source.id);
        await page.goto("/");
        await attachSession(page, source.id);
        await waitForTermText(page, "FAKE-AGENT READY");
        if (replace) await selectSessionActionWith(page, "Replace");
        else if (action === "Clone") {
          const sourceRow = page.locator(`.session-row[data-session-id="${source.id}"]`);
          await openRowMenu(sourceRow);
          await sourceRow.getByRole("menuitem", { name: "Clone", exact: true }).click();
        } else await fillCreateForm(page, { cwd: "/tmp", invocation: FAKE_AGENT, title: `${prefix}-accepted` });
        const form = page.locator(".create-session-form");
        await expect(form).toBeVisible();
        await form.getByLabel("Name (optional)").fill(`${prefix}-accepted`);
        await expect(form).toHaveAttribute("data-folder-check-pending", "false");
        const path = replace ? `**/api/sessions/${encodeURIComponent(source.id)}/replace` : "**/api/sessions";
        stopRoute = await drainedRoute(page, path, async (route) => {
          if (route.request().method() !== "POST") return route.continue();
          bodies.push(route.request().postDataJSON());
          await gate.wait();
          const response = await route.fetch();
          expect(response.ok(), await response.text()).toBe(true);
          const accepted = (await listSessions(request)).sessions.find((row) => row.title === `${prefix}-accepted`);
          expect(accepted, "the server reply names a real accepted launch").toBeDefined();
          owned.push(accepted!.id);
          await route.fulfill({ response });
        });
        await form.locator(".create-session-submit").click();
        await expect.poll(() => bodies.length, { message: "the admitted request is held before switching" }).toBe(1);
        await expect(form).toBeHidden();
        const otherRow = page.locator(`[data-session-id="${other.id}"]`);
        if (action === "Clone") {
          await openRowMenu(otherRow);
          const otherMenu = otherRow.getByRole("menu");
          await otherMenu.getByRole("menuitem", { name: "Clone", exact: true }).focus();
          await page.keyboard.press("Escape");
          await expect(otherMenu).toHaveCount(0);
          await expect(otherRow.locator(".session-row-menu")).toBeFocused();
        }
        await otherRow.locator(".session-row-open").click();
        await expect(otherRow).toHaveAttribute("data-session-selected", "true");
        await waitForSessionReady(page, other.id);
        await waitForTermText(page, "FAKE-AGENT READY");
        await waitForTermText(page, "> ");
        frames.length = 0;
        await page.keyboard.type("kept-input-across-launch");
        await expect.poll(() => programInput(frames), {
          message: "B receives the unfinished line before the launch reply",
        }).toBe("kept-input-across-launch");
        gate.release();
        await expect(form).toHaveCount(0);
        await expect.poll(
          async () => (await listSessions(request)).sessions.filter((row) => row.title === `${prefix}-accepted`).length,
          { message: "the real helm accepted the retained launch" },
        ).toBe(1);
        const accepted = (await listSessions(request)).sessions.find((row) => row.title === `${prefix}-accepted`)!;
        await expect(page.locator(`[data-session-id="${accepted.id}"]`)).toBeVisible();
        await expect(page.locator(`[data-session-id="${other.id}"]`)).toHaveAttribute(
          "data-session-selected",
          "true",
        );
        if (replace) await expect(page.locator(`[data-session-id="${source.id}"]`)).toHaveCount(0);
        await settleCompletion(page);
        await expect(terminalInput(page)).toBeFocused();
        frames.length = 0;
        await page.keyboard.press("Enter");
        await expect.poll(() => programInput(frames), {
          message: "the next Enter reaches B rather than opening New",
        }).toBe("\r");
        await waitForTermText(page, "echo:kept-input-across-launch");
        await expect.poll(async () => (await readPreferences(request)).last_selected, {
          message: "the completed launch does not replace B as the remembered selection",
        }).toBe(other.id);
        expect(bodies).toHaveLength(1);
      } finally {
        gate.release();
        await stopRoute?.();
        // A launch may succeed immediately before a failed UI assertion. Read
        // its owned title too, so that failure cannot leave a real agent behind.
        for (const row of (await listSessions(request)).sessions.filter((row) => row.title === `${prefix}-accepted`)) {
          if (!owned.includes(row.id)) owned.push(row.id);
        }
        await cleanupOwnedSteps(
          testInfo.status !== testInfo.expectedStatus,
          owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
        );
      }
    },
  );
}

/** A YOLO refusal is a follow-up question rather than a completed launch.
 * It waits behind a newer switcher, then regains modal focus, keeps the draft
 * and binds its one-time answer to exactly the same request identity. Leaving
 * the source for another install must not retarget this implicit New draft. */
test(
  "a held YOLO refusal restores the modal question and retries the same draft",
  async ({ page, request }, testInfo) => {
    const gate = routeGate();
    const bodies: any[] = [];
    const owned: string[] = [];
    const build = requireHelmBuild(await request.get("/api/hosts"), "held YOLO question");
    const stopRoute = await drainedRoute(page, "**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      bodies.push(route.request().postDataJSON());
      if (bodies.length === 1) {
        await gate.wait();
        await route.fulfill({
          status: 409,
          headers: {
            "x-farhelm-build": build,
            "x-farhelm-yolo-confirmation": "confirmation-required",
          },
          body: "the fixture host requires YOLO confirmation",
        });
      } else {
        await route.fulfill({
          status: 409,
          headers: { "x-farhelm-build": build },
          body: "confirmed fixture refused",
        });
      }
    });
    try {
      const local = await localHostId(request);
      const hosts = await listHosts(request);
      const origin = hosts.find((host) => host.id === local);
      const remote = hosts.find((host) => host.kind === "ssh" && host.state.phase === "connected" && host.id !== local);
      expect(origin?.identity, "the implicit launch origin has a recorded installation identity").toBeTruthy();
      expect(origin?.state.phase).toBe("connected");
      expect(remote?.identity, "the switch target is a connected remote installation").toBeTruthy();
      expect(remote!.identity).not.toBe(origin!.identity);
      const source = await createSession(request, { title: `yolo-origin-${Date.now()}`, host: local });
      owned.push(source.id);
      const other = await createSession(request, { title: `yolo-other-host-${Date.now()}`, host: remote!.id });
      owned.push(other.id);
      await pinAutoSelect(page, source.id);
      await page.goto("/");
      await attachSession(page, source.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      const form = await fillCreateForm(page, {
        cwd: "/tmp",
        invocation: "true",
        title: "retained YOLO draft",
        yolo: true,
      });
      const originalHost = await form.getByRole("combobox", { name: "Host", exact: true }).textContent();
      await expect(form).toHaveAttribute("data-folder-check-pending", "false");
      await form.locator(".create-session-submit").click();
      await expect.poll(() => bodies.length).toBe(1);
      expect(bodies[0]).toMatchObject({ host: local, expected_incarnation: origin!.incarnation });
      await expect(form).toBeHidden();
      await page.keyboard.press("Control+Shift+k");
      const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
      await expect(switcher.getByRole("combobox")).toBeFocused();
      await switcher.getByRole("combobox").fill(other.title);
      await expect(switcher.locator(".quick-switcher-results").getByRole("option")).toHaveCount(1);
      await page.keyboard.press("Enter");
      await expect(switcher).toHaveCount(0);
      const otherRow = page.locator(`[data-session-id="${other.id}"]`);
      await expect(otherRow).toHaveAttribute("data-session-selected", "true");
      await waitForSessionReady(page, other.id);
      await page.keyboard.press("Control+Shift+k");
      await expect(switcher.getByRole("combobox")).toBeFocused();
      const replied = page.waitForResponse((response) =>
        response.request().method() === "POST" &&
        new URL(response.url()).pathname === "/api/sessions"
      );
      gate.release();
      await (await replied).finished();
      await page.evaluate(() =>
        new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
      );
      await expect(form).toBeHidden();
      await expect(switcher.getByRole("combobox")).toBeFocused();
      await switcher.getByRole("combobox").fill("switcher still owns this query");
      await expect(switcher.getByRole("combobox")).toHaveValue("switcher still owns this query");
      await switcher.getByRole("combobox").press("Escape");
      await expect(switcher).toHaveCount(0);
      await expect(form).toBeVisible();
      await expect(form).toHaveAttribute("aria-modal", "true");
      const question = form.locator(".yolo-confirmation");
      await expect(question).toBeVisible();
      await expect(question.locator(".yolo-cancel")).toBeFocused();
      await expect(form.getByLabel("Name (optional)")).toHaveValue("retained YOLO draft");
      await expect(form.getByLabel("Folder", { exact: true })).toHaveValue("/tmp");
      await expect(form.getByLabel("Agent command")).toHaveValue("true");
      await expect(form.getByRole("combobox", { name: "Host", exact: true })).toHaveText(originalHost!);
      await expect(otherRow).toHaveAttribute("data-session-selected", "true");
      await question.locator(".yolo-confirm").click();
      await expect(form.getByRole("alert")).toContainText("confirmed fixture refused");
      expect(bodies).toHaveLength(2);
      expect(bodies[0].intent_key).toBeTruthy();
      expect(bodies[1]).toEqual({ ...bodies[0], confirm_yolo: true });
    } finally {
      gate.release();
      await stopRoute();
      await cleanupOwnedSteps(
        testInfo.status !== testInfo.expectedStatus,
        owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
      );
    }
  },
);

/** Opening the switcher is intervening user input, so quick-launch naming is
 * skipped entirely. The accepted row and short report still expose the result;
 * closing the switcher must not uncover a deferred naming or selection jump. */
test("quick-launch naming is skipped after a switcher opened during its create", async ({ page, request }, testInfo) => {
  const name = `pending-quick-name-${Date.now()}`;
  const previous = await readPreferences(request);
  const gate = routeGate();
  const owned: string[] = [];
  let calls = 0;
  const stopRoute = await drainedRoute(page, "**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    calls++;
    await gate.wait();
    const response = await route.fetch();
    expect(response.ok(), await response.text()).toBe(true);
    owned.push((await response.json()).id);
    await route.fulfill({ response });
  });
  try {
    const source = await createSession(request, { title: `${name}-source` });
    owned.push(source.id);
    await pinAutoSelect(page, source.id);
    const saved = await request.put(`/api/templates/${name}`, {
      data: {
        kind: "command",
        command: FAKE_AGENT,
        yolo: false,
        name,
        destination: { folder: "/tmp" },
      },
    });
    expect(saved.ok(), await saved.text()).toBe(true);
    await patchPreferences(request, { quick_launch_template: name, name_session_on_quick_launch: true });
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await expect(page.locator(".quick-launch-trigger")).toHaveCount(1);
    await page.keyboard.press("Control+Shift+p");
    await expect.poll(() => calls, { message: "the quick create is held after dispatch" }).toBe(1);
    const form = page.locator(".create-session-form");
    await expect(form).toBeHidden();
    await page.keyboard.press("Control+Shift+k");
    const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
    await expect(switcher.getByRole("combobox")).toBeFocused();
    const replied = page.waitForResponse((response) =>
      response.request().method() === "POST" &&
      new URL(response.url()).pathname === "/api/sessions"
    );
    gate.release();
    await (await replied).finished();
    await expect(form).toHaveCount(0);
    await settleCompletion(page);
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    await expect(switcher.getByRole("combobox")).toBeFocused();
    await expect(page.locator(`[data-session-id="${source.id}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(".launch-progress-pill")).toContainText("Started");
    await expect(page.locator(".launch-progress-pill")).toContainText(name);
    await switcher.getByRole("combobox").fill("keep this query after completion");
    await expect(switcher.getByRole("combobox")).toHaveValue("keep this query after completion");
    await switcher.getByRole("combobox").press("Escape");
    await expect(switcher).toHaveCount(0);
    await expect(form).toHaveCount(0);
    await settleCompletion(page);
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    expect(calls).toBe(1);
    expect(owned).toHaveLength(2);
    await expect(page.locator(`[data-session-id="${owned[1]}"]`)).toBeVisible();
    await expect(page.locator(`[data-session-id="${source.id}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect.poll(async () => (await readPreferences(request)).last_selected).toBe(source.id);
  } finally {
    gate.release();
    await stopRoute();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      [
        "preferences",
        () =>
          patchPreferences(request, {
            quick_launch_template: previous.quick_launch_template ?? null,
            name_session_on_quick_launch: previous.name_session_on_quick_launch ?? true,
          }),
      ],
      ["template", async () => {
        const removed = await request.delete(`/api/templates/${name}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
      ...owned.map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** A host planning result can open without any press after quick dispatch.
 * Naming is therefore still owed and must wait behind that real modal. Its
 * eventual dismissal is a new user press: naming must check again afterward,
 * rather than acting on the unchanged count it saw when create completed. */
test("quick-launch naming waits for an asynchronous host result and rechecks input after it closes", async ({ page, request }, testInfo) => {
  const name = `quick-name-host-result-${Date.now()}`;
  const previous = await readPreferences(request);
  const owned: string[] = [];
  const planReply = routeGate();
  const createReply = routeGate();
  const disposers: (() => Promise<void>)[] = [];
  let admissionTrace: JSHandle<{ count(): number; stop(): void }> | undefined;
  let plans = 0;
  let creates = 0;
  try {
    const local = await localHostId(request);
    const hosts = await listHosts(request);
    const origin = hosts.find((host) => host.id === local);
    const remote = hosts.find((host) => host.kind === "ssh" && host.state.phase === "connected" && host.id !== local);
    expect(origin?.identity, "the quick template has the stack's real local installation target").toBeTruthy();
    expect(remote?.identity, "Update planning belongs to the stack's real connected SSH host").toBeTruthy();
    const source = await createSession(request, { title: `${name}-source`, host: local });
    owned.push(source.id);
    const saved = await request.put(`/api/templates/${name}`, {
      data: {
        kind: "command", command: FAKE_AGENT, yolo: false, name,
        host: origin!.identity, destination: { folder: "/tmp" },
      },
    });
    expect(saved.ok(), await saved.text()).toBe(true);
    await patchPreferences(request, { quick_launch_template: name, name_session_on_quick_launch: true });
    const build = requireHelmBuild(await request.get("/api/hosts"), "asynchronous host planning refusal");
    disposers.push(await drainedRoute(page, (url) => url.pathname === `/api/hosts/${remote!.id}/update`, async (route) => {
      expect(route.request().method()).toBe("POST");
      // Only the read-only planning request is admitted. A successful plan
      // and its token submission never reach the real provisioning backend.
      expect(route.request().postData(), "Update has not submitted a plan token").toBeFalsy();
      plans++;
      await planReply.wait();
      await route.fulfill({
        status: 409, headers: { "x-farhelm-build": build }, body: "held Update planning refused",
      });
    }));
    disposers.push(await drainedRoute(page, "**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      creates++;
      await createReply.wait();
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBe(true);
      owned.push((await response.json()).id);
      await route.fulfill({ response });
    }));
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const avatar = page.locator(`.host-rail-avatar[data-rail-host-id="${remote!.id}"]`);
    await expect(avatar).toBeVisible();
    await avatar.click({ button: "right" });
    const hostRow = page.locator(`[data-host-id="${remote!.id}"]`);
    await expect(hostRow.locator(".host-row-menu-panel")).toBeVisible();
    const update = hostRow.locator(".provisioning-update");
    await expect(update).not.toHaveAttribute("aria-disabled", "true");
    await update.click();
    await expect.poll(() => plans, { message: "the bodyless Update plan is held before quick dispatch" }).toBe(1);
    await expect(page.locator(".host-refusal-dialog")).toHaveCount(0);
    await expect(page.locator(".quick-launch-trigger")).toBeEnabled();
    await page.keyboard.press("Control+Shift+p");
    await expect.poll(() => creates, { message: "Update planning does not block the independent quick create" }).toBe(1);
    const form = page.locator(".create-session-form");
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");

    // From dispatch until dismissal below, only host-side gates and passive
    // assertions run. The host result's own opening does not advance input.
    planReply.release();
    const refusal = page.locator(".host-refusal-dialog");
    await expect(refusal).toBeVisible();
    await expect(refusal).toContainText("held Update planning refused");
    await expect(refusal.locator(".host-refusal-close")).toBeFocused();
    const acceptedReply = page.waitForResponse((response) =>
      response.request().method() === "POST" && new URL(response.url()).pathname === "/api/sessions"
    );
    createReply.release();
    await (await acceptedReply).finished();
    await expect.poll(() => owned.length, { message: "the quick create is accepted while the host result stays open" }).toBe(2);
    await expect(page.locator(`[data-session-id="${owned[1]}"]`)).toBeVisible();
    await settleCompletion(page);
    await expect(form).toHaveCount(1);
    await expect(form).toBeHidden();
    await expect(form).not.toHaveAttribute("aria-modal", "true");
    await expect(page.locator(".launch-progress-pill")).toBeVisible();
    await expect(page.locator(".new-session-button")).toBeDisabled();
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    await expect(refusal.locator(".host-refusal-close")).toBeFocused();
    await expect(page.locator(`[data-session-id="${source.id}"]`)).toHaveAttribute("data-session-selected", "true");

    // Attribute old values retain a true→false pair even when both writes
    // happen before observer delivery. Eventual dialog absence alone would
    // miss a transient modal reservation swallowing the next terminal key.
    admissionTrace = await form.evaluateHandle((root) => {
      let reservations = 0;
      const observer = new MutationObserver((records) => {
        for (const record of records) {
          if (record.oldValue === "true" || root.getAttribute("aria-modal") === "true") reservations++;
        }
      });
      observer.observe(root, { attributes: true, attributeOldValue: true, attributeFilter: ["aria-modal"] });
      return { count: () => reservations, stop: () => observer.disconnect() };
    });
    await refusal.locator(".host-refusal-close").click();
    await expect(refusal).toHaveCount(0);
    await expect(form).toHaveCount(0);
    await settleCompletion(page);
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    await expect(page.locator(`[data-session-id="${source.id}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(".launch-progress-pill")).toContainText("Started");
    await expect(page.locator(".launch-progress-pill")).toContainText(name);
    await expect(page.locator(".new-session-button")).toBeEnabled();
    await expect(avatar).toBeFocused();
    expect(await admissionTrace.evaluate((trace) => trace.count()),
      "the changed input count refuses naming before any hidden-launcher modal reservation").toBe(0);
    await expect.poll(async () => (await readPreferences(request)).last_selected).toBe(source.id);
    expect(plans).toBe(1);
    expect(creates).toBe(1);
  } finally {
    planReply.release();
    createReply.release();
    if (admissionTrace) {
      await admissionTrace.evaluate((trace) => trace.stop());
      await admissionTrace.dispose();
    }
    for (const dispose of disposers) await dispose();
    for (const row of (await listSessions(request)).sessions.filter((row) => row.title === name)) {
      if (!owned.includes(row.id)) owned.push(row.id);
    }
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ["preferences", () => patchPreferences(request, {
        quick_launch_template: previous.quick_launch_template ?? null,
        name_session_on_quick_launch: previous.name_session_on_quick_launch ?? true,
      })],
      ["template", async () => {
        const removed = await request.delete(`/api/templates/${name}`);
        expect(removed.ok() || removed.status() === 404).toBe(true);
      }],
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

for (const needsAnswer of [false, true]) {
  /** Protection against late input must not disable ordinary launch navigation.
   * In the YOLO case the user moves on before the question, then answers it:
   * that answer establishes a new baseline, so earlier input cannot suppress
   * opening the session when nothing else was pressed afterward. */
  test(
    needsAnswer
      ? "New opens after its YOLO answer when no later input intervenes"
      : "New opens its accepted session when no later input intervenes",
    async ({ page, request }, testInfo) => {
      const name = `attended-new-${needsAnswer}-${Date.now()}`;
      const owned: string[] = [];
      const firstReply = routeGate();
      const acceptedReply = routeGate();
      const bodies: any[] = [];
      let stopRoute: (() => Promise<void>) | undefined;
      try {
        const source = await createSession(request, { title: `${name}-source` });
        owned.push(source.id);
        const build = requireHelmBuild(await request.get("/api/hosts"), "attended launch acceptance");
        stopRoute = await drainedRoute(page, "**/api/sessions", async (route) => {
          if (route.request().method() !== "POST") return route.continue();
          bodies.push(route.request().postDataJSON());
          if (needsAnswer && bodies.length === 1) {
            await firstReply.wait();
            await route.fulfill({
              status: 409,
              headers: {
                "x-farhelm-build": build,
                "x-farhelm-yolo-confirmation": "confirmation-required",
              },
              body: "the fixture requires an explicit YOLO answer",
            });
            return;
          }
          await acceptedReply.wait();
          const response = await route.fetch();
          expect(response.ok(), await response.text()).toBe(true);
          owned.push((await response.json()).id);
          await route.fulfill({ response });
        });
        await pinAutoSelect(page, source.id);
        await page.goto("/");
        await attachSession(page, source.id);
        await waitForTermText(page, "FAKE-AGENT READY");
        const form = await fillCreateForm(page, {
          cwd: "/tmp", invocation: FAKE_AGENT, title: name, yolo: needsAnswer,
        });
        await expect(form).toHaveAttribute("data-folder-check-pending", "false");
        await form.locator(".create-session-submit").click();
        await expect.poll(() => bodies.length).toBe(1);
        await expect(form).toBeHidden();
        if (needsAnswer) {
          await page.keyboard.press("Control+Shift+k");
          const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
          await expect(switcher.getByRole("combobox")).toBeFocused();
          await switcher.getByRole("combobox").press("Escape");
          await expect(switcher).toHaveCount(0);
          firstReply.release();
          const question = form.locator(".yolo-confirmation");
          await expect(question).toBeVisible();
          await expect(question.locator(".yolo-cancel")).toBeFocused();
          await question.locator(".yolo-confirm").click();
          await expect.poll(() => bodies.length).toBe(2);
          expect(bodies[1]).toEqual({ ...bodies[0], confirm_yolo: true });
          await expect(form).toBeHidden();
        }
        // Gate release and assertions are host-side observations, not page
        // input. No new answer or pointer press occurs before the reply.
        acceptedReply.release();
        await expect(form).toHaveCount(0);
        await expect.poll(() => owned.length).toBe(2);
        await expect(page.locator(`[data-session-id="${owned[1]}"]`)).toHaveAttribute("data-session-selected", "true");
        await waitForSessionReady(page, owned[1]);
        await waitForTermText(page, "FAKE-AGENT READY");
        await page.keyboard.type("accepted-launch-kept-input");
        await page.keyboard.press("Enter");
        await waitForTermText(page, "echo:accepted-launch-kept-input");
      } finally {
        firstReply.release();
        acceptedReply.release();
        await stopRoute?.();
        for (const row of (await listSessions(request)).sessions.filter((row) => row.title === name)) {
          if (!owned.includes(row.id)) owned.push(row.id);
        }
        await cleanupOwnedSteps(
          testInfo.status !== testInfo.expectedStatus,
          owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
        );
      }
    },
  );
}

/** A hidden launch has no visible anchor to move when it unmounts. Pinning the
 * menu's row keeps a new row from moving that anchor either, so this checks
 * completion-driven dismissal rather than legitimate layout invalidation. */
test("a row menu opened during New survives its accepted completion", async ({ page, request }, testInfo) => {
  const name = `launch-keeps-menu-${Date.now()}`;
  const owned: string[] = [];
  const reply = routeGate();
  let calls = 0;
  let stopRoute: (() => Promise<void>) | undefined;
  try {
    const source = await createSession(request, { title: `${name}-source` });
    owned.push(source.id);
    const pinned = await request.put(`/api/sessions/${encodeURIComponent(source.id)}/pin`, { data: { pinned: true } });
    expect(pinned.ok(), await pinned.text()).toBe(true);
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const form = await fillCreateForm(page, { cwd: "/tmp", invocation: FAKE_AGENT, title: name });
    await expect(form).toHaveAttribute("data-folder-check-pending", "false");
    stopRoute = await drainedRoute(page, "**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      calls++;
      await reply.wait();
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBe(true);
      owned.push((await response.json()).id);
      await route.fulfill({ response });
    });
    await form.locator(".create-session-submit").click();
    await expect.poll(() => calls).toBe(1);
    await expect(form).toBeHidden();
    const sourceRow = page.locator(`.session-row[data-session-id="${source.id}"]`);
    await openRowMenu(sourceRow);
    const menu = sourceRow.getByRole("menu", { name: `Session actions for ${source.title}`, exact: true });
    const anchor = sourceRow.locator(".session-row-menu");
    const before = await anchor.boundingBox();
    expect(before, "the pinned row's measured menu anchor exists before completion").not.toBeNull();
    const clone = menu.getByRole("menuitem", { name: "Clone", exact: true });
    await clone.focus();
    await expect(clone).toBeFocused();
    reply.release();
    await expect(form).toHaveCount(0);
    await expect.poll(() => owned.length).toBe(2);
    await expect(page.locator(`[data-session-id="${owned[1]}"]`)).toBeVisible();
    await settleCompletion(page);
    const after = await anchor.boundingBox();
    expect(after, "the pinned anchor survives the new row's listing refresh").not.toBeNull();
    expect(after!.x).toBeCloseTo(before!.x, 1);
    expect(after!.y).toBeCloseTo(before!.y, 1);
    await expect(menu).toBeVisible();
    await expect(anchor).toHaveAttribute("aria-expanded", "true");
    await expect(clone).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    await expect(anchor).toBeFocused();
  } finally {
    reply.release();
    await stopRoute?.();
    for (const row of (await listSessions(request)).sessions.filter((row) => row.title === name)) {
      if (!owned.includes(row.id)) owned.push(row.id);
    }
    await cleanupOwnedSteps(
      testInfo.status !== testInfo.expectedStatus,
      owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    );
  }
});

/** A success hidden by Hosts still owes a visible outcome, but the short
 * display-only report must retain neither the old launcher nor its seeds.
 * Returning to Sessions and pressing New during that report opens a fresh draft. */
test("New preserves Hosts and reports its success without retaining its launcher", async ({ page, request }, testInfo) => {
  const name = `launch-keeps-hosts-${Date.now()}`;
  const owned: string[] = [];
  const reply = routeGate();
  let calls = 0;
  let stopRoute: (() => Promise<void>) | undefined;
  try {
    const source = await createSession(request, { title: `${name}-source` });
    owned.push(source.id);
    await pinAutoSelect(page, source.id);
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const form = await fillCreateForm(page, { cwd: "/tmp", invocation: FAKE_AGENT, title: name });
    await expect(form).toHaveAttribute("data-folder-check-pending", "false");
    stopRoute = await drainedRoute(page, "**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      calls++;
      await reply.wait();
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBe(true);
      owned.push((await response.json()).id);
      await route.fulfill({ response });
    });
    await form.locator(".create-session-submit").click();
    await expect.poll(() => calls).toBe(1);
    await expect(form).toBeHidden();
    await showHostsPanel(page);
    const hostsToggle = page.locator(".hosts-panel-toggle");
    await expect(hostsToggle).toHaveAttribute("aria-pressed", "true");
    reply.release();
    await expect(form).toHaveCount(0);
    await expect(page.locator(".hosts-heading")).toBeVisible();
    await expect(hostsToggle).toHaveAttribute("aria-pressed", "true");
    const report = page.locator(".launch-progress-pill").filter({ hasText: "Started" });
    await expect(report).toBeVisible();
    await expect(report).toContainText(name);
    await expect(report.getByRole("button")).toHaveCount(0);
    await expect(report.getByRole("link")).toHaveCount(0);
    await expect.poll(async () => (await readPreferences(request)).last_selected).toBe(source.id);
    await showSessionsPanel(page);
    await expect(report).toBeVisible();
    await page.locator(".new-session-button").click();
    await expect(form).toBeVisible();
    await expect(form).toHaveAttribute("data-launcher-mode", "create");
    await expect(form.getByLabel("Name (optional)")).not.toHaveValue(name);
    await form.locator(".launch-composer-cancel").click();
    expect(calls).toBe(1);
  } finally {
    reply.release();
    await stopRoute?.();
    for (const row of (await listSessions(request)).sessions.filter((row) => row.title === name)) {
      if (!owned.includes(row.id)) owned.push(row.id);
    }
    await cleanupOwnedSteps(
      testInfo.status !== testInfo.expectedStatus,
      owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    );
  }
});

for (const action of ["Restart", "Replace"] as const) {
  for (const outcome of ["refusal", "unknown", "retired"] as const) {
    /** A returning launcher may wait for another modal; its source may not.
     * The real session remains alive because this route never sends a mutation.
     * Row/header settlement and Restart's new revealed attachment therefore
     * prove that shared state was released before modal return admission. */
    test(`${action} with settles its ${outcome} before its launcher can return`, async ({ page, request }, testInfo) => {
      const name = `settled-${action}-${outcome}-${Date.now()}`;
      const owned: string[] = [];
      const reply = routeGate();
      let calls = 0;
      let attachments = 0;
      let stopRoute: (() => Promise<void>) | undefined;
      let resumeClock = false;
      try {
        const source = action === "Restart"
          ? await createResumableSession(request, { title: name })
          : await createSession(request, { title: name });
        owned.push(source.id);
        const other = await createSession(request, { title: `${name}-other` });
        owned.push(other.id);
        page.on("websocket", (socket) => {
          if (new URL(socket.url()).pathname === `/api/sessions/${encodeURIComponent(source.id)}/term`) attachments++;
        });
        const build = requireHelmBuild(await request.get("/api/hosts"), "source settles before launcher return");
        stopRoute = await drainedRoute(
          page,
          `**/api/sessions/${encodeURIComponent(source.id)}/${action.toLowerCase()}`,
          async (route) => {
            expect(route.request().method()).toBe("POST");
            calls++;
            await reply.wait();
            await route.fulfill({
              status: 409,
              headers: {
                "x-farhelm-build": build,
                "x-farhelm-create-outcome": "definitely-unaccepted",
              },
              body: "held source operation refused",
            });
          },
        );
        await pinAutoSelect(page, source.id);
        if (outcome === "unknown") await page.clock.install();
        await page.goto("/");
        await attachSession(page, source.id);
        await waitForTermText(page, "FAKE-AGENT READY");
        await expect.poll(() => attachments, { message: "the original source attachment is established" }).toBeGreaterThan(0);
        await selectSessionActionWith(page, action);
        const form = page.locator(".create-session-form");
        await expect(form).toBeVisible();
        if (action === "Restart") {
          // Restart with submits an edit, so the unchanged fixture draft is
          // deliberately unavailable. This owned flag changes the command
          // while preserving its required report placeholder; the intercepted
          // request never executes that changed command.
          const command = form.getByLabel("Agent command", { exact: true });
          const edited = `${await command.inputValue()} --completion-fixture`;
          await command.fill(edited);
          await expect(command).toHaveValue(edited);
        }
        await expect(form.locator(".create-session-submit")).toBeEnabled();
        if (action === "Replace") await expect(form).toHaveAttribute("data-folder-check-pending", "false");
        const opening = await form.getAttribute("data-opening-generation");
        await form.locator(".create-session-submit").click();
        await expect.poll(() => calls, { message: "the exact source request remains held" }).toBe(1);
        await expect(form).toBeHidden();
        await expect(page.locator(".header-stop")).toBeDisabled();
        const sourceRow = page.locator(`.session-row[data-session-id="${source.id}"]`);
        const sourceProgress = action === "Restart"
          ? page.locator(".session-action-progress .launch-progress")
          : sourceRow.locator(".launch-progress");
        await expect(sourceProgress).toBeVisible();

        // A hidden Clone/Replace/Restart launcher no longer owns this menu's
        // dismissal focus. This is an explicit user close, before any reply.
        const otherRow = page.locator(`.session-row[data-session-id="${other.id}"]`);
        await openRowMenu(otherRow);
        const otherMenu = otherRow.getByRole("menu");
        await otherMenu.getByRole("menuitem", { name: "Clone", exact: true }).focus();
        await page.keyboard.press("Escape");
        await expect(otherMenu).toHaveCount(0);
        await expect(otherRow.locator(".session-row-menu")).toBeFocused();
        await page.keyboard.press("Control+Shift+k");
        const switcher = page.getByRole("dialog", { name: "Quick switcher", exact: true });
        await expect(switcher.getByRole("combobox")).toBeFocused();
        const beforeReplyAttachments = attachments;
        if (outcome === "retired") {
          // The real returning request must settle even when its captured
          // DOM admission identity has gone. This is the same denied-return
          // boundary as the New/Rename case above, now with a source claim.
          await form.evaluate((node) => node.setAttribute("data-opening-generation", "retired"));
          await expect(form).toHaveAttribute("data-opening-generation", "retired");
        }
        if (outcome === "unknown") {
          const now = await page.evaluate(() => Date.now());
          await page.clock.pauseAt(new Date(now + 1_000));
          resumeClock = true;
          await page.clock.runFor(60_100);
          await page.clock.resume();
          resumeClock = false;
        } else {
          reply.release();
        }
        const sourceError = sourceRow.locator(".session-row-error");
        await expect(sourceError).toContainText(outcome === "unknown" ? "Still waiting" : "held source operation refused");
        await expect(sourceRow.locator(".launch-progress")).toHaveCount(0);
        await expect(page.locator(".session-action-progress .launch-progress")).toHaveCount(0);
        await expect(page.locator(".header-stop")).toBeEnabled();
        await expect(page.locator(".header-replace")).toBeEnabled();
        if (action === "Restart") {
          await expect.poll(() => attachments, {
            message: "Restart reattaches its terminal before the blocked launcher returns",
          }).toBeGreaterThan(beforeReplyAttachments);
        }
        await waitForSessionRevealed(page, source.id);
        if (outcome === "retired") {
          await expect(form).toHaveCount(0);
          await expect(page.locator(".launch-progress-pill")).toHaveCount(0);
          await expect(page.locator(".new-session-button")).toBeEnabled();
          await expect(switcher.getByRole("combobox")).toBeFocused();
          await switcher.getByRole("combobox").press("Escape");
          await expect(switcher).toHaveCount(0);
          await page.locator(".new-session-button").click();
          await expect(form).toBeVisible();
          await expect(form).toHaveAttribute("data-launcher-mode", "create");
          await expect(form.getByLabel("Name (optional)")).not.toHaveValue(name);
          await form.locator(".launch-composer-cancel").click();
          expect(calls).toBe(1);
          return;
        }
        await expect(form).toBeHidden();
        await expect(form).not.toHaveAttribute("aria-modal", "true");
        await expect(form).toHaveAttribute("data-opening-generation", opening!);
        await expect(page.locator(".launch-progress-pill")).toBeVisible();
        await expect(page.locator(".new-session-button")).toBeDisabled();
        await expect(switcher.getByRole("combobox")).toBeFocused();
        await switcher.getByRole("combobox").press("Escape");
        await expect(switcher).toHaveCount(0);
        await expect(form).toBeVisible();
        await expect(form).toHaveAttribute("data-opening-generation", opening!);
        await expect(form.getByRole("alert")).toContainText(outcome === "unknown" ? "Still waiting" : "held source operation refused");
        await expect(form.locator(".create-session-submit")).toBeEnabled();
        await form.locator(".launch-composer-cancel").click();
        expect(calls).toBe(1);
      } finally {
        reply.release();
        if (resumeClock) await page.clock.resume();
        await stopRoute?.();
        await cleanupOwnedSteps(
          testInfo.status !== testInfo.expectedStatus,
          owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
        );
      }
    });
  }
}

for (const quick of [false, true]) {
  /** The first new row is not permission to navigate after the user moved on.
   * The shared stack retains its other sessions: only this page's complete
   * listing is controlled from empty to the real accepted row. No remembered
   * choice exists, so this also challenges newest-session automatic selection. */
  test(`${quick ? "quick launch" : "New"} from an empty visible fleet preserves no selection after opening Hosts`, async ({ page, request }, testInfo) => {
    const name = `empty-fleet-launch-${quick}-${Date.now()}`;
    const previous = await readPreferences(request);
    const owned: string[] = [];
    const reply = routeGate();
    const disposers: (() => Promise<void>)[] = [];
    let acceptedId: string | undefined;
    let guiReplyDelivered = false;
    let creates = 0;
    try {
      const local = await localHostId(request);
      const origin = (await listHosts(request)).find((host) => host.id === local);
      expect(origin?.identity, "the empty-page launch still targets a real connected installation").toBeTruthy();
      if (quick) {
        const saved = await request.put(`/api/templates/${name}`, {
          data: {
            kind: "command", command: FAKE_AGENT, yolo: false, name,
            host: origin!.identity, destination: { folder: "/tmp" },
          },
        });
        expect(saved.ok(), await saved.text()).toBe(true);
      }
      await patchPreferences(request, {
        last_selected: null,
        ...(quick ? { quick_launch_template: name, name_session_on_quick_launch: true } : {}),
      });
      const emptyPreferences = await readPreferences(request);
      expect(emptyPreferences.last_selected ?? null).toBeNull();
      expect(emptyPreferences.visit_history ?? []).toEqual([]);
      disposers.push(await drainedRoute(page, (url) => url.pathname === "/api/sessions", async (route) => {
        if (route.request().method() !== "GET") return route.fallback();
        const response = await route.fetch();
        expect(response.ok(), await response.text()).toBe(true);
        const listing = await response.json();
        const visible = listing.sessions.filter((session: { id: string }) => session.id === acceptedId);
        await route.fulfill({
          response,
          json: { ...listing, sessions: visible, total: visible.length, matching: visible.length, truncated: false },
        });
      }));
      disposers.push(await drainedRoute(page, "**/api/sessions", async (route) => {
        if (route.request().method() !== "POST") return route.fallback();
        expect(route.request().postDataJSON().host, "the submitted destination is the real Local host").toBe(local);
        creates++;
        const response = await route.fetch();
        expect(response.ok(), await response.text()).toBe(true);
        acceptedId = (await response.json()).id;
        expect(acceptedId).toEqual(expect.any(String));
        owned.push(acceptedId!);
        // The server has accepted and may publish this row while the GUI's
        // own reply is held. Reply-time selection suppression is too late.
        await reply.wait();
        await route.fulfill({ response });
        guiReplyDelivered = true;
      }));
      await observeFeedReaders(page);
      const feed = await stubFeed(page);
      await page.goto("/");
      await feed.waitForConnection(1);
      feed.notify(1);
      await expect(page.locator(".main-empty")).toHaveText("No sessions — create one");
      await expect(page.locator(".session-row")).toHaveCount(0);
      await expect(page.locator(".session-details")).toHaveCount(0);
      await waitForFeedReadersSettled(page);
      if (quick) {
        await expect(page.locator(".quick-launch-trigger")).toBeEnabled();
        await page.keyboard.press("Control+Shift+p");
      } else {
        const form = await fillCreateForm(page, { cwd: "/tmp", invocation: FAKE_AGENT, title: name });
        await expect(form).toHaveAttribute("data-folder-check-pending", "false");
        await form.locator(".create-session-submit").click();
      }
      await expect.poll(() => creates, { message: "the first visible session's create is held" }).toBe(1);
      const form = page.locator(".create-session-form");
      await expect(form).toBeHidden();
      await showHostsPanel(page);
      await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
      await expect.poll(() => acceptedId, { message: "the server accepted a real session before delivering its GUI reply" }).toBeTruthy();
      const refreshed = page.waitForResponse((response) => response.request().method() === "GET"
        && new URL(response.url()).pathname === "/api/sessions");
      feed.notify(2);
      const populatedReply = await refreshed;
      expect(populatedReply.ok(), await populatedReply.text()).toBe(true);
      const populated = await populatedReply.json();
      expect(populated.truncated, "the pre-reply listing is authoritative for this controlled visible fleet").toBe(false);
      expect(populated.sessions.map((session: { id: string }) => session.id)).toEqual([acceptedId]);
      await expect(page.locator(`.session-row[data-session-id="${acceptedId!}"]`)).toHaveCount(1);
      // The ordinary all-reader helper assumes every nonempty fleet selects
      // something. This new explicit no-selection policy needs its narrower
      // read boundary, using the same existing observation registry.
      const unselectedListingSettled = async () => {
        const list = (await readFeedReaders(page)).find((reader) => reader.role === "list");
        return list?.has_rows === true && list.selected === null && list.resolving_selection === false
          && list.listing_answered === true
          && list.readers.every((reader) => !reader.running && !reader.task && reader.demand === "None");
      };
      await expect.poll(unselectedListingSettled, {
        message: "the accepted row's listing settles without selection before the launch reply",
      }).toBe(true);
      expect(guiReplyDelivered, "the GUI has not consumed its create result at the pre-reply selection boundary").toBe(false);
      await expect(form).toHaveCount(1);
      await expect(form).toBeHidden();
      await expect(page.locator(".new-session-button")).toBeDisabled();
      await expect(page.locator('.session-row[data-session-selected="true"]')).toHaveCount(0);
      await expect(page.locator(".session-details")).toHaveCount(0);
      await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
      const beforeReply = await readPreferences(request);
      expect(beforeReply.last_selected ?? null).toBeNull();
      expect(beforeReply.visit_history ?? []).toEqual([]);

      reply.release();
      await expect(form).toHaveCount(0);
      await expect(page.locator(".launch-progress-pill")).toContainText("Started");
      await expect(page.locator(".launch-progress-pill")).toContainText(name);
      await expect.poll(() => guiReplyDelivered).toBe(true);
      await expect.poll(unselectedListingSettled, {
        message: "completion's own listing refresh preserves the pre-reply no-selection state",
      }).toBe(true);
      await settleCompletion(page);
      await expect(page.locator('.session-row[data-session-selected="true"]')).toHaveCount(0);
      await expect(page.locator(".session-details")).toHaveCount(0);
      await expect(page.locator(".hosts-heading")).toBeVisible();
      await expect(page.locator(".hosts-panel-toggle")).toHaveAttribute("aria-pressed", "true");
      await expect(page.locator(".rename-dialog")).toHaveCount(0);
      const after = await readPreferences(request);
      expect(after.last_selected ?? null).toBeNull();
      expect(after.visit_history ?? []).toEqual([]);
      expect(creates).toBe(1);
    } finally {
      reply.release();
      for (const dispose of disposers) await dispose();
      for (const session of (await listSessions(request)).sessions.filter((session) => session.title === name)) {
        if (!owned.includes(session.id)) owned.push(session.id);
      }
      await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
        ...owned.map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
        ["preferences and visit history", async () => {
          await patchPreferences(request, {
            last_selected: null,
            quick_launch_template: previous.quick_launch_template ?? null,
            name_session_on_quick_launch: previous.name_session_on_quick_launch ?? true,
          });
          // Visits are helm-owned. Replaying the original accepted choices
          // oldest-first restores their order through the public selection API.
          for (const id of [...(previous.visit_history ?? [])].reverse()) {
            await patchPreferences(request, { last_selected: id });
          }
          if (previous.last_selected) await patchPreferences(request, { last_selected: previous.last_selected });
          const restored = await readPreferences(request);
          expect(restored.last_selected ?? null).toBe(previous.last_selected ?? null);
          expect(restored.visit_history ?? []).toEqual(previous.visit_history ?? []);
        }],
        ...(quick ? [["template", async () => {
          const removed = await request.delete(`/api/templates/${name}`);
          expect(removed.ok() || removed.status() === 404).toBe(true);
        }] as [string, () => Promise<void>]] : []),
      ]);
    }
  });
}

/** Replace with still follows its selected source when that source vanishes
 * from an authoritative listing before the accepted reply is delivered. The
 * listing proves real server removal; the retained details prove that no user
 * navigation happened. Completion must not yield that selection to fallback. */
test("attended Replace with opens its replacement after authoritative source absence", async ({ page, request }, testInfo) => {
  const name = `attended-replace-absence-${Date.now()}`;
  const owned: string[] = [];
  const reply = routeGate();
  let replacement: string | undefined;
  let accepted = false;
  let stopRoute: (() => Promise<void>) | undefined;
  try {
    const source = await createSession(request, { title: `${name}-source` });
    owned.push(source.id);
    const other = await createSession(request, { title: `${name}-other` });
    owned.push(other.id);
    await pinAutoSelect(page, source.id);
    await observeFeedReaders(page);
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await selectSessionActionWith(page, "Replace");
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await form.getByLabel("Name (optional)").fill(name);
    await expect(form).toHaveAttribute("data-folder-check-pending", "false");
    await waitForFeedReadersSettled(page);
    stopRoute = await drainedRoute(page, `**/api/sessions/${encodeURIComponent(source.id)}/replace`, async (route) => {
      expect(route.request().method()).toBe("POST");
      const response = await route.fetch({ maxRetries: 0 });
      expect(response.ok(), await response.text()).toBe(true);
      replacement = (await response.json()).id;
      expect(replacement).toEqual(expect.any(String));
      owned.push(replacement!);
      accepted = true;
      await reply.wait();
      await route.fulfill({ response });
    });
    await form.locator(".create-session-submit").click();
    await expect.poll(() => accepted, { message: "the real replacement exists but its success remains held" }).toBe(true);
    await expect(form).toBeHidden();
    const refreshed = page.waitForResponse((response) => response.request().method() === "GET"
      && new URL(response.url()).pathname === "/api/sessions");
    feed.notify(2);
    const absentReply = await refreshed;
    expect(absentReply.ok(), await absentReply.text()).toBe(true);
    const listing = await absentReply.json();
    expect(listing.truncated, "only a complete listing proves the source absent").toBe(false);
    expect(listing.sessions.some((session: { id: string }) => session.id === source.id)).toBe(false);
    expect(listing.sessions.some((session: { id: string }) => session.id === replacement)).toBe(true);
    await expect(page.locator(`.session-row[data-session-id="${source.id}"]`)).toHaveCount(0);
    await expect(page.locator(`.session-row[data-session-id="${replacement!}"]`)).toBeVisible();
    await expect(page.locator(".session-details .title")).toHaveText(source.title);
    expect((await readFeedReaders(page)).find((reader) => reader.role === "list")?.selected,
      "the held request still owns its selected source despite sidebar absence").toBe(source.id);
    reply.release();
    await expect(form).toHaveCount(0);
    await expect(page.locator(`.session-row[data-session-id="${replacement!}"]`)).toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(`.session-row[data-session-id="${other.id}"]`)).not.toHaveAttribute("data-session-selected", "true");
    await expect(page.locator(".session-details .title")).toHaveText(name);
    await waitForSessionRevealed(page, replacement!);
    await expect.poll(async () => (await readPreferences(request)).last_selected).toBe(replacement);
  } finally {
    reply.release();
    await stopRoute?.();
    for (const session of (await listSessions(request)).sessions.filter((session) => session.title === name)) {
      if (!owned.includes(session.id)) owned.push(session.id);
    }
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus,
      owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]));
  }
});

for (const press of ["pointer", "Enter"] as const) {
  /** An accidental repeat belongs to Launch even after its control disappears.
   * The physical second press targets the released page, not a synthetic event
   * on the old button. A paused browser clock bounds the gesture interval while
   * named create/hidden oracles still establish the actual asynchronous handoff. */
  test(`New opens after a repeated ${press} submit retargets to the released page`, async ({ page, request }, testInfo) => {
    const name = `retargeted-submit-${press}-${Date.now()}`;
    const owned: string[] = [];
    const reply = routeGate();
    let calls = 0;
    let acceptedId: string | undefined;
    let acceptedInput: InputFrame[] = [];
    let clockPaused = false;
    let stopRoute: (() => Promise<void>) | undefined;
    try {
      const source = await createSession(request, { title: `${name}-source` });
      owned.push(source.id);
      await pinAutoSelect(page, source.id);
      await page.clock.install();
      await page.goto("/");
      await attachSession(page, source.id);
      await waitForTermText(page, "FAKE-AGENT READY");
      const form = await fillCreateForm(page, { cwd: "/tmp", invocation: FAKE_AGENT, title: name });
      await expect(form).toHaveAttribute("data-folder-check-pending", "false");
      const submit = form.locator(".create-session-submit");
      await expect(submit).toBeEnabled();
      const box = await submit.boundingBox();
      expect(box, "the first physical press has a visible Launch target").not.toBeNull();
      const point = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };
      if (press === "Enter") {
        await submit.focus();
        await expect(submit).toBeFocused();
      } else {
        await page.mouse.move(point.x, point.y);
      }
      stopRoute = await drainedRoute(page, "**/api/sessions", async (route) => {
        if (route.request().method() !== "POST") return route.continue();
        calls++;
        await reply.wait();
        const response = await route.fetch();
        expect(response.ok(), await response.text()).toBe(true);
        acceptedId = (await response.json()).id;
        expect(acceptedId).toEqual(expect.any(String));
        owned.push(acceptedId!);
        acceptedInput = recordSessionInput(page, acceptedId!);
        await route.fulfill({ response });
      });
      const now = await page.evaluate(() => Date.now());
      await page.clock.pauseAt(new Date(now + 1_000));
      clockPaused = true;
      const before = await page.evaluate(() => (window as any).__farhelmLaunchPressCount);
      expect(Number.isSafeInteger(before), "the page capture counter is already installed").toBe(true);
      const firstAt = await page.evaluate(() => performance.now());
      if (press === "pointer") await page.mouse.click(point.x, point.y, { clickCount: 1 });
      else await page.keyboard.press("Enter");
      await expect.poll(async () => {
        if (calls === 1 && await form.isHidden()) return true;
        const elapsed = await page.evaluate((at) => performance.now() - at, firstAt);
        if (elapsed >= 400) throw new Error(`the first submit did not hide within the repeat fixture's interval (${elapsed}ms)`);
        // Advance only enough virtual frames to observe the hidden-request
        // boundary. Host-side polling does not consume the 500ms repeat budget.
        await page.clock.runFor(20);
        return false;
      }, { message: "the first Launch is hidden and its create held before the second press" }).toBe(true);
      await expect(form).not.toHaveAttribute("aria-modal", "true");
      const submitted = await page.evaluate(() => (window as any).__farhelmLaunchPressCount);
      expect(submitted).toBe(before + 1);
      if (press === "pointer") {
        expect(await page.evaluate(({ x, y }) =>
          !document.elementFromPoint(x, y)?.closest('.create-session-form[role="dialog"]'), point),
          "the second pointer press really targets the page beneath the hidden Launch button").toBe(true);
        // Playwright's mouse.click(clickCount: 2) sends two extra pairs.
        // One down/up pair with detail two is the actual second press.
        await page.mouse.down({ clickCount: 2 });
        await page.mouse.up({ clickCount: 2 });
      } else {
        expect(await page.evaluate(() => document.activeElement === document.body
          || document.activeElement === document.documentElement),
          "hiding the submitted button retargets the second Enter to the document").toBe(true);
        await page.keyboard.press("Enter");
      }
      expect(await page.evaluate((at) => performance.now() - at, firstAt),
        "both physical presses remain inside the bounded submit-repeat interval").toBeLessThan(500);
      expect(await page.evaluate(() => (window as any).__farhelmLaunchPressCount),
        "the retargeted accidental repeat is not intervening user input").toBe(submitted);
      expect(calls).toBe(1);
      await page.clock.resume();
      clockPaused = false;
      reply.release();
      await expect(form).toHaveCount(0);
      await expect.poll(() => acceptedId).toBeTruthy();
      await expect(page.locator(`.session-row[data-session-id="${acceptedId!}"]`)).toHaveAttribute("data-session-selected", "true");
      await waitForSessionReady(page, acceptedId!);
      await waitForTermText(page, "FAKE-AGENT READY");
      acceptedInput.length = 0;
      await page.keyboard.type("accepted-after-repeat");
      await page.keyboard.press("Enter");
      await expect.poll(() => programInput(acceptedInput), {
        message: "the next typed line reaches the accepted session's own socket",
      }).toBe("accepted-after-repeat\r");
      await waitForTermText(page, "echo:accepted-after-repeat");
      expect(calls).toBe(1);
    } finally {
      reply.release();
      if (clockPaused) await page.clock.resume();
      await stopRoute?.();
      for (const session of (await listSessions(request)).sessions.filter((session) => session.title === name)) {
        if (!owned.includes(session.id)) owned.push(session.id);
      }
      await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus,
        owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]));
    }
  });
}

for (const action of ["Restart", "Replace"] as const) {
  /** A YOLO question is an unanswered continuation, not a source failure.
   * This route dispatches no mutation, so cancelling its returned question
   * must leave the real source alive without a row/header error invented
   * from the host's confirmation-required text. */
  test(`${action} with YOLO cancellation leaves no source failure`, async ({ page, request }, testInfo) => {
    const name = `yolo-no-source-failure-${action}-${Date.now()}`;
    let source: Awaited<ReturnType<typeof createSession>> | undefined;
    let stopRoute: (() => Promise<void>) | undefined;
    let calls = 0;
    try {
      source = action === "Restart"
        ? await createResumableSession(request, { title: name })
        : await createSession(request, { title: name });
      const id = source.id;
      const build = requireHelmBuild(await request.get("/api/hosts"), "YOLO source continuation");
      stopRoute = await drainedRoute(page, `**/api/sessions/${encodeURIComponent(id)}/${action.toLowerCase()}`, async (route) => {
        expect(route.request().method()).toBe("POST");
        calls++;
        await route.fulfill({
          status: 409,
          headers: { "x-farhelm-build": build, "x-farhelm-yolo-confirmation": "confirmation-required" },
          body: "confirmation is required, not a failed source operation",
        });
      });
      await pinAutoSelect(page, id);
      await page.goto("/");
      await attachSession(page, id);
      await waitForTermText(page, "FAKE-AGENT READY");
      const row = page.locator(`.session-row[data-session-id="${id}"]`);
      await expect(row.locator(".session-row-error")).toHaveCount(0);
      await expect(page.locator(".restart-error, .replace-error")).toHaveCount(0);
      await selectSessionActionWith(page, action);
      const form = page.locator(".create-session-form");
      await expect(form).toBeVisible();
      // The source's stored launch is guarded; this is an actual edit for
      // Restart as well as the condition the returned YOLO question answers.
      await answerYolo(form, true);
      if (action === "Replace") await expect(form).toHaveAttribute("data-folder-check-pending", "false");
      await expect(form.locator(".create-session-submit")).toBeEnabled();
      await form.locator(".create-session-submit").click();
      const question = form.locator(".yolo-confirmation");
      await expect(question).toBeVisible();
      await expect(question.locator(".yolo-cancel")).toBeFocused();
      await expect(row.locator(".session-row-error")).toHaveCount(0);
      await expect(page.locator(".restart-error, .replace-error")).toHaveCount(0);
      await question.locator(".yolo-cancel").click();
      await expect(question).toHaveCount(0);
      await form.locator(".launch-composer-cancel").click();
      await expect(form).toHaveCount(0);
      await expect(row.locator(".session-row-error")).toHaveCount(0);
      await expect(page.locator(".restart-error, .replace-error")).toHaveCount(0);
      await expect(page.locator(".header-stop")).toBeEnabled();
      await waitForSessionRevealed(page, id);
      await terminalInput(page).focus();
      await waitForSessionReady(page, id);
      await page.keyboard.type("source-kept-after-yolo-cancel");
      await page.keyboard.press("Enter");
      await waitForTermText(page, "echo:source-kept-after-yolo-cancel");
      expect(calls).toBe(1);
    } finally {
      await stopRoute?.();
      await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus,
        source ? [[source.id, () => cleanupSession(request, source!.id)]] : []);
    }
  });
}

/** Local validation has not replaced the source operation's last result.
 * A queued submit still enters Rust despite the disabled presentation; its
 * temporary source claim must unwind without erasing an earlier Stop failure. */
test("a locally refused Replace with preserves the source's earlier Stop report", async ({ page, request }, testInfo) => {
  const previous = await readPreferences(request);
  let source: Awaited<ReturnType<typeof createSession>> | undefined;
  const disposers: (() => Promise<void>)[] = [];
  let stops = 0;
  let writes = 0;
  const report = "owned Stop refusal before local Replace validation";
  try {
    source = await createSession(request, { title: `replace-local-refusal-${Date.now()}` });
    const id = source.id;
    const build = requireHelmBuild(await request.get("/api/hosts"), "local Replace validation");
    await patchPreferences(request, { skip_session_confirmations: true });
    disposers.push(await drainedRoute(page, "**/api/**", async (route) => {
      if (route.request().method() !== "POST") return route.fallback();
      const path = new URL(route.request().url()).pathname;
      if (path === `/api/sessions/${encodeURIComponent(id)}/stop`) {
        stops++;
        return route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: report });
      }
      if (path === "/api/sessions" || path.endsWith("/replace") || path.endsWith("/resume")) {
        writes++;
        return route.fulfill({ status: 409, headers: { "x-farhelm-build": build }, body: "unexpected mutation after invalid wrapper" });
      }
      return route.fallback();
    }));
    await pinAutoSelect(page, id);
    await page.goto("/");
    await attachSession(page, id);
    await waitForTermText(page, "FAKE-AGENT READY");
    await page.locator(".header-stop").click();
    const row = page.locator(`.session-row[data-session-id="${id}"]`);
    await expect(row.locator(".session-row-error")).toContainText(report);
    await expect(page.locator(".stop-error")).toContainText(report);
    expect(stops).toBe(1);
    await selectSessionActionWith(page, "Replace");
    const form = page.locator(".create-session-form");
    await form.getByRole("tab", { name: "Agent", exact: true }).click();
    await form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Codex", exact: true }).click();
    await expect(form).toHaveAttribute("data-folder-check-pending", "false");
    const wrapper = form.getByLabel("Wrapper prefix", { exact: true });
    await wrapper.fill("'unfinished");
    await expect(wrapper).toHaveAttribute("aria-invalid", "true");
    await expect(form.locator(".create-session-submit")).toHaveAttribute("aria-disabled", "true");
    const refusal = await form.locator(".launch-composer-wrapper-choice .launch-composer-choice-error").innerText();
    expect(refusal.length).toBeGreaterThan(0);
    await expect(row.locator(".session-row-error")).toContainText(report);
    // This models an already queued submit, rather than asking Playwright to
    // click an action whose current render correctly refuses physical input.
    await form.dispatchEvent("submit");
    await expect(form.locator(".launch-composer-refusal-alert")).toContainText(refusal);
    await settleCompletion(page);
    expect(writes, "invalid local input sends no Replace, create or wake request").toBe(0);
    await expect(row.locator(".session-row-error")).toContainText(report);
    await expect(page.locator(".stop-error")).toContainText(report);
    await form.locator(".launch-composer-cancel").click();
    await expect(form).toHaveCount(0);
    await expect(page.locator(".header-stop")).toBeEnabled();
    await expect(page.getByRole("button", { name: "Replace options", exact: true })).toBeEnabled();
    await expect(row.locator(".session-row-error")).toContainText(report);
    await expect(page.locator(".stop-error")).toContainText(report);
    expect(writes).toBe(0);
  } finally {
    for (const dispose of disposers.reverse()) await dispose();
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ["confirmation preference", () => patchPreferences(request, {
        skip_session_confirmations: previous.skip_session_confirmations ?? null,
      })],
      ...(source ? [[source.id, () => cleanupSession(request, source!.id)]] as [string, () => Promise<void>][] : []),
    ]);
  }
});

/** An actual selected-session removal still owes a successor during New.
 * The displayed fleet contains only these owned rows, so B is the sole
 * eligible successor; other stack fixtures are neither deleted nor used as
 * accidental fallbacks. New's later moved-on result must preserve that pick. */
test("Delete selects its successor while New is held and New completion keeps it", async ({ page, request }, testInfo) => {
  const name = `delete-during-new-${Date.now()}`;
  const previous = await readPreferences(request);
  const owned: string[] = [];
  const reply = routeGate();
  const disposers: (() => Promise<void>)[] = [];
  let acceptedId: string | undefined;
  let calls = 0;
  try {
    const source = await createSession(request, { title: `${name}-source` });
    owned.push(source.id);
    const other = await createSession(request, { title: `${name}-successor` });
    owned.push(other.id);
    const frames = recordSessionInput(page, other.id);
    await patchPreferences(request, { skip_session_confirmations: true });
    await pinAutoSelect(page, source.id);
    disposers.push(await drainedRoute(page, (url) => url.pathname === "/api/sessions", async (route) => {
      if (route.request().method() !== "GET") return route.fallback();
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBe(true);
      const listing = await response.json();
      const rows = listing.sessions.filter((session: { id: string }) =>
        session.id === source.id || session.id === other.id || session.id === acceptedId);
      await route.fulfill({ response,
        json: { ...listing, sessions: rows, total: rows.length, matching: rows.length, truncated: false } });
    }));
    disposers.push(await drainedRoute(page, "**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.fallback();
      calls++;
      await reply.wait();
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBe(true);
      acceptedId = (await response.json()).id;
      owned.push(acceptedId!);
      await route.fulfill({ response });
    }));
    await page.goto("/");
    await attachSession(page, source.id);
    await waitForTermText(page, "FAKE-AGENT READY");
    const form = await fillCreateForm(page, { cwd: "/tmp", invocation: FAKE_AGENT, title: name });
    await expect(form).toHaveAttribute("data-folder-check-pending", "false");
    await form.locator(".create-session-submit").click();
    await expect.poll(() => calls, { message: "New holds its create while the selected source remains independent" }).toBe(1);
    await expect(form).toBeHidden();
    await expect(page.locator(".header-delete")).toBeEnabled();
    await page.locator(".header-delete").click();
    await expect(page.locator(`.session-row[data-session-id="${source.id}"]`)).toHaveCount(0);
    expect((await listSessions(request)).sessions.some((session) => session.id === source.id),
      "the selected source was actually removed by Delete").toBe(false);
    const successor = page.locator(`.session-row[data-session-id="${other.id}"]`);
    await expect(successor).toHaveAttribute("data-session-selected", "true");
    await expect(form).toHaveCount(1);
    await expect(form).toBeHidden();
    await waitForSessionRevealed(page, other.id);
    await terminalInput(page).focus();
    await waitForSessionReady(page, other.id);
    frames.length = 0;
    await page.keyboard.type("successor-input-during-new");
    await expect.poll(() => programInput(frames)).toBe("successor-input-during-new");
    reply.release();
    await expect(form).toHaveCount(0);
    await expect.poll(() => acceptedId).toBeTruthy();
    await expect(page.locator(`.session-row[data-session-id="${acceptedId!}"]`)).toBeVisible();
    await expect(successor).toHaveAttribute("data-session-selected", "true");
    await settleCompletion(page);
    await expect(terminalInput(page)).toBeFocused();
    frames.length = 0;
    await page.keyboard.press("Enter");
    await expect.poll(() => programInput(frames)).toBe("\r");
    await waitForTermText(page, "echo:successor-input-during-new");
    await expect.poll(async () => (await readPreferences(request)).last_selected).toBe(other.id);
    expect(calls).toBe(1);
  } finally {
    reply.release();
    for (const dispose of disposers) await dispose();
    for (const session of (await listSessions(request)).sessions.filter((session) => session.title === name)) {
      if (!owned.includes(session.id)) owned.push(session.id);
    }
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ["confirmation preference", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? false })],
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
    ]);
  }
});

/** A startup preference read is authority to choose only for that startup.
 * Hold the real second preference GET after the gate has loaded preferences
 * and the list has begun automatic selection. Explicit New takes that empty
 * opportunity over; its older read must not later select the bystander. */
test("a stale startup preference read cannot select after New takes over the empty pane", async ({ page, request }, testInfo) => {
  const name = `startup-read-new-${Date.now()}`;
  const previous = await readPreferences(request);
  const owned: string[] = [];
  const preferencesReply = routeGate();
  const createReply = routeGate();
  const disposers: (() => Promise<void>)[] = [];
  let preferenceReads = 0;
  let preferenceHeld = false;
  let preferenceDelivered = false;
  let creates = 0;
  let acceptedId: string | undefined;
  try {
    const candidate = await createSession(request, { title: `${name}-candidate` });
    owned.push(candidate.id);
    await patchPreferences(request, { last_selected: null });
    disposers.push(await drainedRoute(page, "**/api/preferences", async (route) => {
      if (route.request().method() !== "GET") return route.fallback();
      preferenceReads++;
      const held = preferenceReads === 2;
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBe(true);
      if (held) {
        preferenceHeld = true;
        await preferencesReply.wait();
      }
      await route.fulfill({ response });
      if (held) preferenceDelivered = true;
    }));
    disposers.push(await drainedRoute(page, (url) => url.pathname === "/api/sessions", async (route) => {
      if (route.request().method() !== "GET") return route.fallback();
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBe(true);
      const listing = await response.json();
      const rows = listing.sessions.filter((session: { id: string }) => session.id === candidate.id || session.id === acceptedId);
      await route.fulfill({ response,
        json: { ...listing, sessions: rows, total: rows.length, matching: rows.length, truncated: false } });
    }));
    disposers.push(await drainedRoute(page, "**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") return route.fallback();
      creates++;
      await createReply.wait();
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBe(true);
      acceptedId = (await response.json()).id;
      owned.push(acceptedId!);
      await route.fulfill({ response });
    }));
    await observeFeedReaders(page);
    const feed = await stubFeed(page);
    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await expect.poll(() => preferenceHeld, { message: "the startup selector's preference response is held" }).toBe(true);
    await expect(page.locator(`.session-row[data-session-id="${candidate.id}"]`)).toBeVisible();
    await expect.poll(async () => {
      const list = (await readFeedReaders(page)).find((reader) => reader.role === "list");
      return list?.selected === null && list.resolving_selection === true;
    }, { message: "the held response belongs to an unsettled empty-pane startup selection" }).toBe(true);
    await expect(page.locator(".session-details")).toHaveCount(0);
    const form = await fillCreateForm(page, { cwd: "/tmp", invocation: FAKE_AGENT, title: name });
    await expect(form).toHaveAttribute("data-folder-check-pending", "false");
    await form.locator(".create-session-submit").click();
    await expect.poll(() => creates, { message: "explicit New has dispatched before the older preference read returns" }).toBe(1);
    await expect(form).toBeHidden();
    await showHostsPanel(page);
    preferencesReply.release();
    await expect.poll(() => preferenceDelivered).toBe(true);
    await expect.poll(async () => {
      const list = (await readFeedReaders(page)).find((reader) => reader.role === "list");
      return list?.selected === null && list.resolving_selection === false;
    }, { message: "the old startup read retires without selecting its candidate during New" }).toBe(true);
    await expect(form).toBeHidden();
    createReply.release();
    await expect(form).toHaveCount(0);
    await expect(page.locator(".launch-progress-pill")).toContainText("Started");
    await expect(page.locator(".launch-progress-pill")).toContainText(name);
    await expect.poll(() => acceptedId).toBeTruthy();
    await expect(page.locator(`.session-row[data-session-id="${acceptedId!}"]`)).toHaveCount(1);
    await settleCompletion(page);
    await expect(page.locator('.session-row[data-session-selected="true"]')).toHaveCount(0);
    await expect(page.locator(".session-details")).toHaveCount(0);
    await expect(page.locator(".hosts-heading")).toBeVisible();
    const after = await readPreferences(request);
    expect(after.last_selected ?? null).toBeNull();
    expect(after.visit_history ?? []).toEqual([]);
    expect(creates).toBe(1);
  } finally {
    preferencesReply.release();
    createReply.release();
    for (const dispose of disposers) await dispose();
    for (const session of (await listSessions(request)).sessions.filter((session) => session.title === name)) {
      if (!owned.includes(session.id)) owned.push(session.id);
    }
    await cleanupOwnedSteps(testInfo.status !== testInfo.expectedStatus, [
      ...owned.reverse().map((id): [string, () => Promise<void>] => [id, () => cleanupSession(request, id)]),
      ["selection history", async () => {
        await patchPreferences(request, { last_selected: null });
        for (const id of [...(previous.visit_history ?? [])].reverse()) await patchPreferences(request, { last_selected: id });
        if (previous.last_selected) await patchPreferences(request, { last_selected: previous.last_selected });
        const restored = await readPreferences(request);
        expect(restored.last_selected ?? null).toBe(previous.last_selected ?? null);
        expect(restored.visit_history ?? []).toEqual(previous.visit_history ?? []);
      }],
    ]);
  }
});
