/** Helm-wide settings must persist and affect the next action in this client.
 * These tests keep real configuration or preference reads and the host registry,
 * so a checkbox that changes only its appearance cannot satisfy the round trip.
 */
import { expect, test } from "./helpers/evidence";
import { resolveToken } from "./helpers/color";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { clickDialogGap } from "./helpers/dialog-gap";
import { routeGate } from "./helpers/route-gate";
import { drainedRoute } from "./helpers/drained-route";
import { requireHelmBuild } from "./helpers/helm-build";
import { attachSession } from "./helpers/term";
import { openSettings, selectSettingsSection, settingsSectionLabels, type SettingsSection } from "./helpers/settings";
import type { APIRequestContext, Page, Request, Route } from "@playwright/test";
import { request as independentRequest } from "@playwright/test";
import { harnessAuthorizationHeaders } from "./helpers/device-auth";
import { currentStackSlot } from "../stack-slots";
import {
  cleanupSession,
  createSession,
  forceBuildSkew,
  hostRowByName,
  openHostMenu,
  patchPreferences,
  readPreferences,
  showHostsPanel,
} from "./helpers/fleet";

/** The web fixture has no updater, so its navigation omits the app pane. */
const webSettingsSections: SettingsSection[] = ["sessions", "Appearance", "hosts", "terminal", "checkouts", "agent trust", "notifications"];

/** A checked switch's inverse outline must be drawn against its own fill.
 * Real native Tab focus and a supported gap click protect the failure premise:
 * a color-only assertion would pass with the old invisible outer white ring.
 * Label/help distances witness grouping without treating wrapper classes as
 * the behavior. Checkout actions also keep the shared compact button metrics.
 */
test("settings focus stays on its painted fill and descriptions stay with their labels", async ({ page, request }) => {
  test.setTimeout(60_000);
  const previous = await readPreferences(request);
  try {
    await patchPreferences(request, { name_session_on_quick_launch: true, skip_session_confirmations: false });
    await page.goto("/");
    const dialog = await openSettings(page);
    const first = dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true });
    const naming = dialog.getByRole("checkbox", { name: "name sessions on quick launch", exact: true });
    for (const colorScheme of ["dark", "light"] as const) {
      await page.emulateMedia({ colorScheme });
      for (const width of [1280, 390]) {
        await page.setViewportSize({ width, height: 800 });
        await selectSettingsSection(page, "sessions");
        await expect(first).not.toBeChecked();
        await expect(naming).toBeChecked();
        await expect(naming).toBeEnabled();
        await first.focus();
        await page.keyboard.press("Tab");
        await expect(naming).toBeFocused();
        expect(await naming.evaluate((element) => element.matches(":focus-visible"))).toBe(true);
        await clickDialogGap(page, dialog);
        await expect(naming).toBeFocused();
        await expect(naming).toBeChecked();
        await expect(naming).toBeEnabled();
        expect(await naming.evaluate((element) => element.matches(":focus-visible"))).toBe(true);
        await expect(naming).toHaveCSS("outline-color", await resolveToken(page, "--on-fill"));
        await expect(naming).toHaveCSS("outline-offset", "-3px");
        const distances = await dialog.locator("#settings-sessions input").evaluateAll((inputs) => inputs.map((input, index) => {
          const label = input.closest("label")!;
          const help = document.getElementById(input.getAttribute("aria-describedby")!)!;
          const next = inputs[index + 1]?.closest("label");
          return {
            within: help.getBoundingClientRect().top - label.getBoundingClientRect().bottom,
            between: next ? next.getBoundingClientRect().top - help.getBoundingClientRect().bottom : null,
          };
        }));
        expect(distances).toHaveLength(3);
        for (const distance of distances) {
          expect(distance.within).toBeGreaterThanOrEqual(0);
          if (distance.between !== null) expect(distance.between).toBeGreaterThan(distance.within);
        }
        await selectSettingsSection(page, "checkouts");
        const close = dialog.getByRole("button", { name: "Close", exact: true });
        for (const field of [".checkout-root-field", ".checkout-post-clone-field"]) {
          await expect(dialog.locator(field).locator("input, textarea")).toBeEditable();
          const save = dialog.locator(field).getByRole("button", { name: "save", exact: true });
          await expect(save).toHaveAttribute("aria-disabled", "true");
          await expect(save).toHaveCSS("font-size", await close.evaluate((element) => getComputedStyle(element).fontSize));
          await expect(save).toHaveCSS("color", await resolveToken(page, "--fg-2"));
          await expect(save).toHaveCSS("opacity", "1");
        }
      }
    }
    expect((await readPreferences(request)).name_session_on_quick_launch).toBe(true);
  } finally {
    try { await page.close(); }
    finally { await patchPreferences(request, {
      name_session_on_quick_launch: previous.name_session_on_quick_launch ?? null,
      skip_session_confirmations: previous.skip_session_confirmations ?? null,
    }); }
  }
});

/** End the page's preference queue before releasing held replies or restoring
 * shared state. Otherwise a later queued click could overwrite cleanup after
 * its route disappears and contaminate another spec using the same helm. */
async function endPreferenceSaveTest(
  page: Page,
  stopRoute: () => Promise<void>,
  release: () => void,
  restore: () => Promise<void>,
  bodyFailed: boolean,
) {
  await cleanupOwnedSteps(bodyFailed, [
    ["preference page", () => page.goto("about:blank")],
    ["preference route", async () => { release(); await stopRoute(); }],
    ["shared preferences", restore],
  ]);
}

// Claude and Codex cover the two hint implementations. Muse uses the same
// workspace component as Codex; Pi has no question-hint switch.
for (const choice of [
  { field: "color_terminal_frames", saved: false, next: true, section: "sessions", role: "checkbox",
    name: "color terminal frames by host", oldName: "color terminal frames by host", oldChecked: false,
    error: "Could not save whether to color terminal frames. Try again." },
  { field: "name_session_on_quick_launch", saved: true, next: false, section: "sessions", role: "checkbox",
    name: "name sessions on quick launch", oldName: "name sessions on quick launch", oldChecked: true,
    error: "Could not save whether to name quick-launched sessions. Try again." },
  { field: "terminal_link_click", saved: "intercept", next: "program", section: "terminal", role: "radio",
    name: "pass the click on only", oldName: "open in Farhelm, don't pass the click on", oldChecked: true,
    error: "Could not save how terminal links are clicked. Try again." },
  { field: "claude_trust_hint_dismissed", saved: false, next: true, section: "agent trust", role: "checkbox",
    name: "show the hint when Claude asks about a managed checkout", oldName: "show the hint when Claude asks about a managed checkout", oldChecked: true,
    error: "Could not save whether to show the Claude trust hint. Try again." },
  { field: "codex_trust_hint_dismissed", saved: false, next: true, section: "agent trust", role: "checkbox",
    name: "show the hint when Codex asks about a managed checkout", oldName: "show the hint when Codex asks about a managed checkout", oldChecked: true,
    error: "Could not save whether to show the Codex trust hint. Try again." },
] as const) {
  /** A preference refusal must restore the helm's choice and report beside the
   * control. Held replies establish that the clickable optimistic state is
   * provisional; a later successful click must clear the warning and persist. */
  test(`settings reports and reverts a refused ${choice.field} save`, async ({ page, request }) => {
    const previous = await readPreferences(request);
    const build = requireHelmBuild(await request.get("/api/preferences"), "settings preference refusals");
    const reached = routeGate();
    const release = routeGate();
    let stopRoute = async () => {};
    let refuse = true;
    let writes = 0;
    let bodyFailed = true;
    try {
      await patchPreferences(request, { [choice.field]: choice.saved });
      expect((await readPreferences(request))[choice.field], "the helm holds the explicit fixture choice").toBe(choice.saved);
      stopRoute = await drainedRoute(page, "**/api/preferences", async (route) => {
        const outgoing = route.request();
        if (outgoing.method() !== "PUT" || !(choice.field in outgoing.postDataJSON())) return route.fallback();
        writes++;
        expect(outgoing.postDataJSON()[choice.field]).toBe(choice.next);
        if (!refuse) return route.continue();
        reached.release();
        await release.wait();
        await route.fulfill({ status: 503, headers: { "x-farhelm-build": build }, body: "fixture preference save refused" });
      });
      await page.goto("/");
      const dialog = await openSettings(page, choice.section);
      const pane = dialog.locator(`section.app-settings-panel[aria-label="${choice.section}"]`);
      const control = pane.getByRole(choice.role, { name: choice.name, exact: true });
      const old = pane.getByRole(choice.role, { name: choice.oldName, exact: true });
      await expect(old).toBeChecked({ checked: choice.oldChecked });
      if (choice.role === "radio") await expect(control).not.toBeChecked();
      await expect(control).toBeEnabled();
      // click() permits the refused state to roll back before the action
      // finishes; check()/uncheck() would demand that it remain changed.
      // A segmented radio keeps a clipped native input; its label is the
      // visible pointer surface, while checked state still belongs to input.
      await (choice.role === "radio" ? control.locator("..") : control).click();
      await reached.wait();
      await expect(control).toBeEnabled();
      await expect(control).toBeChecked({ checked: choice.role === "radio" || !choice.oldChecked });
      expect((await readPreferences(request))[choice.field], "the held PUT has not changed persistence").toBe(choice.saved);
      release.release();
      const error = pane.getByRole("alert");
      await expect(error).toHaveText(choice.error);
      await expect(old).toBeChecked({ checked: choice.oldChecked });
      if (choice.role === "radio") await expect(control).not.toBeChecked();
      expect((await readPreferences(request))[choice.field]).toBe(choice.saved);
      expect(writes).toBe(1);
      const controlBox = await (choice.role === "radio" ? control.locator("..") : control).boundingBox();
      const errorBox = await error.boundingBox();
      expect(controlBox, "the restored control is rendered beside its failure").not.toBeNull();
      expect(errorBox, "the failure is rendered below the control").not.toBeNull();
      expect(errorBox!.y).toBeGreaterThanOrEqual(controlBox!.y + controlBox!.height);
      await expect(page.locator(".build-skew")).toHaveCount(0);
      refuse = false;
      await (choice.role === "radio" ? control.locator("..") : control).click();
      await expect.poll(async () => (await readPreferences(request))[choice.field]).toBe(choice.next);
      await expect(error).toHaveCount(0);
      bodyFailed = false;
    } finally {
      await endPreferenceSaveTest(page, stopRoute, release.release,
        () => patchPreferences(request, { [choice.field]: previous[choice.field] ?? null }), bodyFailed);
    }
  });
}

for (const hint of [
  { name: "Claude", key: "claude", field: "claude_trust_hint_dismissed", endpoint: "/api/checkout-claude-trust" },
  { name: "Codex", key: "codex", field: "codex_trust_hint_dismissed", endpoint: "/api/checkout-agent-trust/codex" },
] as const) {
  /** A pending hint read cannot undo the clicked native state. Both reads
   * and writes stay held so a fast GET cannot hide a stale render: the next
   * click must mean the opposite choice and supersede the refused save. */
  test(`settings keeps the pending ${hint.name} hint choice clickable through a held read`, async ({ page, request }) => {
    const previous = await readPreferences(request);
    const build = requireHelmBuild(await request.get(hint.endpoint), "pending trust hint choices");
    const readReached = routeGate();
    const readRelease = routeGate();
    const firstReached = routeGate();
    const firstRelease = routeGate();
    const latestReached = routeGate();
    const latestRelease = routeGate();
    let stopRead = async () => {};
    let stopWrite = async () => {};
    let latestRequest: Request | undefined;
    const dismissals: boolean[] = [];
    let bodyFailed = true;
    try {
      await patchPreferences(request, { [hint.field]: false });
      expect((await readPreferences(request))[hint.field]).toBe(false);
      await page.goto("/");
      const dialog = await openSettings(page, "agent trust");
      const field = dialog.locator(`#checkout-${hint.key}-trust`);
      const toggle = field.getByRole("checkbox", { name: `show the hint when ${hint.name} asks about a managed checkout`, exact: true });
      await expect(toggle).toBeEnabled();
      await expect(toggle).toBeChecked();
      // Install only after the successful mounted read above. A loading
      // checkbox would not exercise synchronization of the previous value.
      stopRead = await drainedRoute(page, (url) => url.pathname === hint.endpoint, async (route) => {
        if (route.request().method() !== "GET") return route.fallback();
        readReached.release();
        await readRelease.wait();
        await route.continue();
      });
      stopWrite = await drainedRoute(page, "**/api/preferences", async (route) => {
        const outgoing = route.request();
        if (outgoing.method() !== "PUT" || !(hint.field in outgoing.postDataJSON())) return route.fallback();
        dismissals.push(outgoing.postDataJSON()[hint.field]);
        if (dismissals.length === 1) {
          firstReached.release();
          await firstRelease.wait();
          await route.fulfill({ status: 503, headers: { "x-farhelm-build": build }, body: "fixture superseded hint save refused" });
        } else {
          expect(dismissals, "the opposite native click must reach the serialized writer").toEqual([true, false]);
          latestRequest = outgoing;
          latestReached.release();
          await latestRelease.wait();
          await route.continue();
        }
      });
      await toggle.click();
      await Promise.all([firstReached.wait(), readReached.wait()]);
      expect(dismissals).toEqual([true]);
      await expect(toggle).not.toBeChecked();
      await expect(toggle).toBeEnabled();
      expect((await readPreferences(request))[hint.field], "both requests remain held before persistence").toBe(false);
      await toggle.click();
      await expect(toggle).toBeChecked();
      await expect(toggle).toBeEnabled();
      // The GET remains held through both clicks and the old refusal. The
      // optimistic checkbox cannot depend on a replacement read completing.
      firstRelease.release();
      await latestReached.wait();
      expect(dismissals).toEqual([true, false]);
      await expect(toggle).toBeChecked();
      await expect(toggle).toBeEnabled();
      await expect(field.getByRole("alert")).toHaveCount(0);
      latestRelease.release();
      const saved = await latestRequest!.response();
      expect(saved, "the newest choice receives its own acknowledgment").not.toBeNull();
      expect(saved!.ok()).toBe(true);
      await saved!.finished();
      expect((await readPreferences(request))[hint.field]).toBe(false);
      readRelease.release();
      await stopRead();
      await expect(toggle).toBeChecked();
      await expect(toggle).toBeEnabled();
      await expect(field.getByRole("alert")).toHaveCount(0);
      await expect(page.locator(".build-skew")).toHaveCount(0);
      bodyFailed = false;
    } finally {
      await endPreferenceSaveTest(page, async () => {
        const results = await Promise.allSettled([stopRead(), stopWrite()]);
        expect(results.filter((result) => result.status === "rejected"), "both hint handlers must drain").toEqual([]);
      }, () => { readRelease.release(); firstRelease.release(); latestRelease.release(); },
      () => patchPreferences(request, { [hint.field]: previous[hint.field] ?? null }), bodyFailed);
    }
  });

  /** A successful old GET may cross the acknowledgment of a pending click.
   * Capture its real old body before committing the PUT, then hold delivery
   * across that commit. Automatic cancellation or invalidation may retire
   * the old read; either way a fresh read must leave the saved choice current. */
  test(`settings repairs a ${hint.name} trust read crossing a successful hint save`, async ({ page, request }) => {
    const previous = await readPreferences(request);
    const oldCaptured = routeGate();
    const oldRelease = routeGate();
    const oldFinished = routeGate();
    const writeReached = routeGate();
    const writeRelease = routeGate();
    const currentRead = routeGate();
    let stopRead = async () => {};
    let stopWrite = async () => {};
    let captureOld = true;
    let submitted: Request | undefined;
    let writes = 0;
    let bodyFailed = true;
    try {
      await patchPreferences(request, { [hint.field]: false });
      expect((await readPreferences(request))[hint.field]).toBe(false);
      await page.goto("/");
      const dialog = await openSettings(page, "agent trust");
      const field = dialog.locator(`#checkout-${hint.key}-trust`);
      const toggle = field.getByRole("checkbox", { name: `show the hint when ${hint.name} asks about a managed checkout`, exact: true });
      await expect(toggle).toBeChecked();
      await expect(toggle).toBeEnabled();
      stopRead = await drainedRoute(page, (url) => url.pathname === hint.endpoint, async (route) => {
        if (route.request().method() !== "GET") return route.fallback();
        const response = await route.fetch();
        expect(response.ok()).toBe(true);
        const raw = await response.json();
        let held = false;
        if (captureOld) {
          captureOld = false;
          held = true;
          expect(raw.hint_dismissed, "the actual backend body precedes the held PUT's commit").toBe(false);
          oldCaptured.release();
          await oldRelease.wait();
        }
        // A resource may legitimately cancel this delivery after settlement.
        // drainedRoute tolerates that lifecycle race; a fresh successful GET
        // and the final UI, rather than acceptance of stale bytes, are required.
        try {
          await route.fulfill({ response });
          if (raw.hint_dismissed === true) currentRead.release();
        } finally {
          if (held) oldFinished.release();
        }
      });
      stopWrite = await drainedRoute(page, "**/api/preferences", async (route) => {
        const outgoing = route.request();
        if (outgoing.method() !== "PUT" || !(hint.field in outgoing.postDataJSON())) return route.fallback();
        expect(outgoing.postDataJSON()[hint.field]).toBe(true);
        writes++;
        submitted = outgoing;
        writeReached.release();
        await writeRelease.wait();
        const response = await route.fetch();
        expect(response.status(), "the owned dismissal really commits").toBe(204);
        await route.fulfill({ response });
      });
      await toggle.click();
      await Promise.all([oldCaptured.wait(), writeReached.wait()]);
      expect((await readPreferences(request))[hint.field]).toBe(false);
      await expect(toggle).not.toBeChecked();
      await expect(toggle).toBeEnabled();
      writeRelease.release();
      const saved = await submitted!.response();
      expect(saved).not.toBeNull();
      expect(saved!.status()).toBe(204);
      await saved!.finished();
      expect((await readPreferences(request))[hint.field]).toBe(true);
      oldRelease.release();
      await oldFinished.wait();
      await currentRead.wait();
      await expect(toggle).not.toBeChecked();
      await expect(toggle).toBeEnabled();
      await expect(field.getByRole("alert")).toHaveCount(0);
      expect((await readPreferences(request))[hint.field]).toBe(true);
      expect(writes).toBe(1);
      bodyFailed = false;
    } finally {
      await endPreferenceSaveTest(page, async () => {
        const results = await Promise.allSettled([stopRead(), stopWrite()]);
        expect(results.filter((result) => result.status === "rejected"), "crossing-read routes must drain").toEqual([]);
      }, () => { oldRelease.release(); writeRelease.release(); },
      () => patchPreferences(request, { [hint.field]: previous[hint.field] ?? null }), bodyFailed);
    }
  });

  for (const unreadable of hint.key === "claude" ? [false, true] : [false]) {
  /** A reopened panel can read an optimistic dismissal while its original
   * owner is gone. Failure must retire that overlay in the already-mounted
   * replacement view; another click or reopen would conceal a stuck value.
   * A refused corrective read leaves the existing disabled/read-error state,
   * rather than retaining the failed choice or inventing a known old value.
   * The old panel's save warning must not migrate into the new panel. */
  test(`settings reverts a pending ${hint.name} hint save after reopening before refusal${unreadable ? " with an unreadable corrective read" : ""}`, async ({ page, request }) => {
    const previous = await readPreferences(request);
    const build = requireHelmBuild(await request.get(hint.endpoint), "reopened trust hint refusal");
    const reached = routeGate();
    const release = routeGate();
    const failedRead = routeGate();
    let stopRoute = async () => {};
    let stopRead = async () => {};
    let submitted: Request | undefined;
    let writes = 0;
    let bodyFailed = true;
    try {
      await patchPreferences(request, { [hint.field]: false });
      expect((await readPreferences(request))[hint.field]).toBe(false);
      stopRoute = await drainedRoute(page, "**/api/preferences", async (route) => {
        const outgoing = route.request();
        if (outgoing.method() !== "PUT" || !(hint.field in outgoing.postDataJSON())) return route.fallback();
        expect(outgoing.postDataJSON()[hint.field]).toBe(true);
        writes++;
        submitted = outgoing;
        reached.release();
        await release.wait();
        await route.fulfill({ status: 503, headers: { "x-farhelm-build": build }, body: "fixture reopened hint save refused" });
      });
      await page.goto("/");
      const dialog = await openSettings(page, "agent trust");
      const field = dialog.locator(`#checkout-${hint.key}-trust`);
      const toggle = field.getByRole("checkbox", { name: `show the hint when ${hint.name} asks about a managed checkout`, exact: true });
      await expect(toggle).toBeEnabled();
      await expect(toggle).toBeChecked();
      await toggle.click();
      await reached.wait();
      await expect(toggle).not.toBeChecked();
      await expect(toggle).toBeEnabled();
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
      await expect(dialog).toHaveCount(0);
      const reopenedRead = page.waitForResponse((reply) => reply.request().method() === "GET"
        && new URL(reply.url()).pathname === hint.endpoint);
      await openSettings(page, "agent trust");
      const fresh = await reopenedRead;
      expect(fresh.ok(), "the replacement panel gets a successful trust read while the PUT remains held").toBe(true);
      expect((await fresh.json()).hint_dismissed, "the helm still holds the original shown-hint choice").toBe(false);
      await expect(toggle).toBeEnabled();
      await expect(toggle).not.toBeChecked();
      expect((await readPreferences(request))[hint.field]).toBe(false);
      expect(writes).toBe(1);
      if (unreadable) {
        // Start refusing only after the replacement view has copied the
        // pending choice from a successful raw read. Initial-load failure
        // would never establish the provisional state that must be retired.
        stopRead = await drainedRoute(page, (url) => url.pathname === hint.endpoint, async (route) => {
          if (route.request().method() !== "GET") return route.fallback();
          await route.fulfill({ status: 503, headers: { "x-farhelm-build": build }, body: "fixture corrective trust read refused" });
          failedRead.release();
        });
      }
      // Stay in this mounted replacement panel across the failure. A manual
      // refresh, further click or second reopen would erase the lifetime bug.
      release.release();
      const refused = await submitted!.response();
      expect(refused, "the owned pending write receives its refusal").not.toBeNull();
      expect(refused!.status()).toBe(503);
      await refused!.finished();
      if (unreadable) {
        await failedRead.wait();
        await expect(toggle).toBeDisabled();
        await expect(field.getByRole("alert")).toContainText("Could not save or read Claude trust settings:");
        await expect(field.getByRole("alert")).toContainText("fixture corrective trust read refused");
        await expect(field.getByRole("alert")).not.toContainText("Could not save whether to show the Claude trust hint.");
      } else {
        await expect(toggle).toBeChecked();
        await expect(toggle).toBeEnabled();
        await expect(field.getByRole("alert")).toHaveCount(0);
      }
      expect((await readPreferences(request))[hint.field]).toBe(false);
      expect(writes).toBe(1);
      await expect(page.locator(".build-skew")).toHaveCount(0);
      bodyFailed = false;
    } finally {
      await endPreferenceSaveTest(page, async () => {
        const results = await Promise.allSettled([stopRead(), stopRoute()]);
        expect(results.filter((result) => result.status === "rejected"), "both reopened-hint handlers must drain").toEqual([]);
      }, release.release,
        () => patchPreferences(request, { [hint.field]: previous[hint.field] ?? null }), bodyFailed);
    }
  });
  }
}

/** A newer link choice owns the field even while the old HTTP request is
 * outstanding. The old refusal must neither flash an alert nor put back an
 * older radio value before the newest choice reaches persistence. */
test("settings ignores a superseded link-choice failure", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const build = requireHelmBuild(await request.get("/api/preferences"), "superseded link preference");
  const firstReached = routeGate();
  const firstRelease = routeGate();
  const newestReached = routeGate();
  const newestRelease = routeGate();
  let stopRoute = async () => {};
  let writes = 0;
  let bodyFailed = true;
  try {
    await patchPreferences(request, { terminal_link_click: "intercept" });
    expect((await readPreferences(request)).terminal_link_click).toBe("intercept");
    stopRoute = await drainedRoute(page, "**/api/preferences", async (route) => {
      const outgoing = route.request();
      if (outgoing.method() !== "PUT" || !("terminal_link_click" in outgoing.postDataJSON())) return route.fallback();
      writes++;
      if (writes === 1) {
        expect(outgoing.postDataJSON().terminal_link_click).toBe("program");
        firstReached.release();
        await firstRelease.wait();
        await route.fulfill({ status: 503, headers: { "x-farhelm-build": build }, body: "fixture older save refused" });
      } else {
        expect(outgoing.postDataJSON().terminal_link_click).toBe("both");
        newestReached.release();
        await newestRelease.wait();
        await route.continue();
      }
    });
    await page.goto("/");
    const dialog = await openSettings(page, "terminal");
    const program = dialog.getByRole("radio", { name: "pass the click on only", exact: true });
    const both = dialog.getByRole("radio", { name: "open in Farhelm and also pass the click on", exact: true });
    await expect(dialog.getByRole("radio", { name: "open in Farhelm, don't pass the click on", exact: true })).toBeChecked();
    // Retain even a warning removed by the newer click: a final no-alert
    // assertion alone could miss supersession incorrectly reporting failure.
    const warnings = await dialog.evaluateHandle((element) => {
      const seen: string[] = [];
      const observer = new MutationObserver((records) => {
        for (const record of records) for (const node of record.addedNodes) {
          if (node instanceof Element && node.matches('[role="alert"]') && seen.length < 20) seen.push(node.textContent ?? "");
        }
      });
      observer.observe(element, { childList: true, subtree: true });
      return { seen, observer };
    });
    await program.locator("..").click();
    await firstReached.wait();
    await expect(program).toBeChecked();
    await expect(both).toBeEnabled();
    await both.locator("..").click();
    await expect(both).toBeChecked();
    firstRelease.release();
    await newestReached.wait();
    expect(writes).toBe(2);
    expect((await readPreferences(request)).terminal_link_click).toBe("intercept");
    await expect(both).toBeChecked();
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    expect(await warnings.evaluate((state) => state.seen)).toEqual([]);
    newestRelease.release();
    await expect.poll(async () => (await readPreferences(request)).terminal_link_click).toBe("both");
    await expect(both).toBeChecked();
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    expect(await warnings.evaluate((state) => state.seen)).toEqual([]);
    await warnings.evaluate((state) => state.observer.disconnect());
    await warnings.dispose();
    bodyFailed = false;
  } finally {
    await endPreferenceSaveTest(page, stopRoute, () => { firstRelease.release(); newestRelease.release(); },
      () => patchPreferences(request, { terminal_link_click: previous.terminal_link_click ?? null }), bodyFailed);
  }
});

/** Closing drops the panel's error, not its obligation to undo an unsaved
 * shared choice. The terminal consumes frame colors outside Settings, so its
 * attribute proves rollback before reopening can fetch or rebuild controls. */
test("settings frame-color rollback survives closing before failure", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const build = requireHelmBuild(await request.get("/api/preferences"), "closed settings rollback");
  const reached = routeGate();
  const release = routeGate();
  let stopRoute = async () => {};
  let owned: string | undefined;
  let bodyFailed = true;
  try {
    await patchPreferences(request, { color_terminal_frames: false });
    expect((await readPreferences(request)).color_terminal_frames).toBe(false);
    owned = (await createSession(request, { title: `settings-frame-close-${Date.now()}` })).id;
    stopRoute = await drainedRoute(page, "**/api/preferences", async (route) => {
      const outgoing = route.request();
      if (outgoing.method() !== "PUT" || !("color_terminal_frames" in outgoing.postDataJSON())) return route.fallback();
      expect(outgoing.postDataJSON().color_terminal_frames).toBe(true);
      reached.release();
      await release.wait();
      await route.fulfill({ status: 503, headers: { "x-farhelm-build": build }, body: "fixture closed settings save refused" });
    });
    await page.goto("/");
    await attachSession(page, owned);
    const panes = page.locator(".terminal-panes");
    await expect(panes).toBeVisible();
    await expect(panes).toHaveAttribute("data-colored-frame", "false");
    const dialog = await openSettings(page);
    const toggle = dialog.getByRole("checkbox", { name: "color terminal frames by host", exact: true });
    await expect(toggle).not.toBeChecked();
    await toggle.click();
    await reached.wait();
    await expect(toggle).toBeChecked();
    await expect(toggle).toBeEnabled();
    await expect(panes).toHaveAttribute("data-colored-frame", "true");
    expect((await readPreferences(request)).color_terminal_frames).toBe(false);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    release.release();
    await expect(panes).toHaveAttribute("data-colored-frame", "false");
    await expect(dialog).toHaveCount(0);
    expect((await readPreferences(request)).color_terminal_frames).toBe(false);
    await openSettings(page);
    await expect(toggle).not.toBeChecked();
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    bodyFailed = false;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["frame preference fixture", () => endPreferenceSaveTest(page, stopRoute, release.release,
        () => patchPreferences(request, { color_terminal_frames: previous.color_terminal_frames ?? null }), bodyFailed)],
      ["frame fixture session", async () => { if (owned) await cleanupSession(request, owned); }],
    ]);
  }
});

/** The prepared-checkout choice is explicit configuration. Unknown reads stay
 * locked, refusals restore the saved native checkbox, and a dismissed field's
 * pending save must fence the next mounted read rather than lose that write.
 * Its external help link must get the global readable style, not browser defaults. */
test("prepared checkout settings preserve saved choices across reads, refusals and close", async ({ page, request }) => {
  let build: string | undefined;
  const read = async () => {
    const reply = await request.get("/api/checkout-spare");
    expect(reply.ok(), await reply.text()).toBe(true);
    build = reply.headers()["x-farhelm-build"];
    return await reply.json() as { enabled: boolean };
  };
  const previous = await read();
  expect(build, "fixture refusals must preserve the real helm build stamp").toBeTruthy();
  const readGate = routeGate();
  const writeGate = routeGate();
  let mode: "read-failure" | "normal" | "refuse" | "hold-write" = "read-failure";
  let readEntered = false;
  let heldRequest: Request | undefined;
  // Every save the page sends, not only the held one: cleanup restores the
  // shared setting only after each has settled, or a late one could overtake it.
  const submitted: Request[] = [];
  let bodyFailed = true;
  const intercept = async (route: Route) => {
    if (route.request().method() === "GET" && mode === "read-failure") {
      readEntered = true;
      await readGate.wait();
      return route.fulfill({ status: 500, headers: { "x-farhelm-build": build! }, body: "fixture initial read refusal" });
    }
    if (route.request().method() === "PUT") {
      submitted.push(route.request());
      if (mode === "refuse") return route.fulfill({ status: 500, headers: { "x-farhelm-build": build! }, body: "fixture spare save refusal" });
      if (mode === "hold-write") {
        heldRequest = route.request();
        await writeGate.wait();
      }
    }
    await route.continue();
  };
  await page.route("**/api/checkout-spare", intercept);
  try {
    const seeded = await request.put("/api/checkout-spare", { data: { enabled: true } });
    expect(seeded.ok(), await seeded.text()).toBe(true);
    await page.goto("/");
    await openSettings(page, "checkouts");
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    const field = dialog.locator("#checkout-spare");
    const toggle = field.getByRole("checkbox", { name: "keep a checkout ready for faster launches", exact: true });
    await expect.poll(() => readEntered, { message: "the initial configuration read is held" }).toBe(true);
    await expect(toggle).toBeDisabled();
    readGate.release();
    await expect(field.getByRole("alert")).toContainText("fixture initial read refusal");
    await expect(toggle).toBeDisabled();
    mode = "normal";
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(toggle).toBeEnabled();
    await expect(toggle).toBeChecked();
    await expect(field).toContainText("one extra checkout per repository and host");
    const help = field.getByRole("link");
    await expect(help).toHaveAttribute("href", "https://farhelm.io/docs/using/start-a-session/#prepared-checkouts");
    await expect(help).toHaveText("How prepared checkouts differ from a fresh clone ↗");
    await expect(help).toHaveCSS("color", await resolveToken(page, "--link"));
    await expect(help).toHaveCSS("text-decoration-line", "underline");
    mode = "refuse";
    // Refusal may restore the native state before Playwright checks an
    // uncheck() postcondition. Click tests that rollback without demanding
    // that the deliberately refused choice remain selected.
    await toggle.click();
    await expect(field.getByRole("alert")).toContainText("fixture spare save refusal");
    await expect(toggle).toBeChecked();
    await expect(toggle).toBeEnabled();
    expect(await read()).toEqual({ enabled: true });
    mode = "hold-write";
    await toggle.uncheck();
    await expect.poll(() => heldRequest !== undefined, { message: "the explicit opt-out PUT is held" }).toBe(true);
    expect(heldRequest!.postDataJSON()).toEqual({ enabled: false });
    await expect(toggle).not.toBeChecked();
    await expect(toggle).toBeDisabled();
    await selectSettingsSection(page, "sessions");
    await selectSettingsSection(page, "checkouts");
    await expect(toggle).toBeDisabled();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(toggle).toBeDisabled();
    expect(await read(), "held write has not reached persistence").toEqual({ enabled: true });
    writeGate.release();
    await expect.poll(read).toEqual({ enabled: false });
    await expect(toggle).toBeEnabled();
    await expect(toggle).not.toBeChecked();
    mode = "normal";
    await toggle.check();
    await expect.poll(read).toEqual({ enabled: true });
    await expect(toggle).toBeEnabled();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(toggle).toBeChecked();
    await expect(toggle).toBeEnabled();
    bodyFailed = false;
  } finally {
    readGate.release();
    writeGate.release();
    await cleanupOwnedSteps(bodyFailed, [
      ["owned spare saves", async () => {
        for (const outgoing of submitted) {
          const reply = await outgoing.response().catch(() => null);
          if (reply) await reply.finished().catch(() => null);
        }
      }],
      ["spare route", () => page.unroute("**/api/checkout-spare", intercept)],
      ["prepared checkout choice", async () => {
        // Timeout can dispose the per-test request before finally runs. This
        // shared mutation still needs restoration after every owned save settles.
        const slot = currentStackSlot();
        const cleanup = await independentRequest.newContext({ baseURL: slot.baseURL,
          extraHTTPHeaders: harnessAuthorizationHeaders(slot.authStatePath), timeout: 10_000 });
        try {
          const reply = await cleanup.put("/api/checkout-spare", { data: previous });
          expect(reply.ok(), await reply.text()).toBe(true);
        } finally { await cleanup.dispose(); }
      }],
    ]);
  }
});

/**
 * The shared installer serves Settings even though Settings has no single
 * confirming action. Empty space preserves the current control, container
 * Enter stays inert, and a real pointer drag can still select help text.
 */
test("settings gap keeps focus while text and container Enter keep their own behavior", async ({ page, request }) => {
  // Loading the page saves the automatic session selection (the last selection and the
  // visit history); only the Settings interactions below must leave preferences untouched.
  const withoutSelection = (preferences: Awaited<ReturnType<typeof readPreferences>>) => {
    const { last_selected: _selected, visit_history: _visits, ...rest } = preferences;
    return rest;
  };
  const before = withoutSelection(await readPreferences(request));
  await page.goto("/");
  await page.getByRole("button", { name: "settings", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  await expect(dialog).toBeVisible();
  const choice = dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true });
  const wasChecked = await choice.isChecked();
  await choice.focus();
  await expect(choice).toBeFocused();
  await clickDialogGap(page, dialog);
  await expect(choice).toBeFocused();
  await dialog.focus();
  await expect(dialog).toBeFocused();
  await dialog.press("Enter");
  await expect(dialog).toBeVisible();
  expect(await choice.isChecked(), "container Enter must not toggle a Settings choice").toBe(wasChecked);
  expect(withoutSelection(await readPreferences(request))).toEqual(before);

  const help = dialog.locator("#session-confirmations-help");
  const box = await help.boundingBox();
  expect(box, "the visible help text must have a painted selection target").not.toBeNull();
  await page.mouse.move(box!.x + 12, box!.y + 10);
  await page.mouse.down();
  await page.mouse.move(box!.x + 180, box!.y + 10, { steps: 8 });
  await page.mouse.up();
  expect(await page.evaluate(() => document.getSelection()?.toString().trim()), "dragging over help text must still select it").not.toBe("");
});

/**
 * Container Enter in a destructive dialog must use the visible confirmation.
 * Host removal closes the prompt as soon as it accepts the answer, so its
 * disabled-button contract is checked by setting the button's native disabled
 * state before the real confirmation. That must prevent a deletion.
 */
test("host removal container Enter confirms once and respects the disabled button", async ({ page, request }, testInfo) => {
  const previous = await readPreferences(request);
  const name = `focus-remove-${testInfo.project.name}-${Date.now()}.invalid`;
  const created = await request.post("/api/hosts", { data: { ssh: name } });
  expect(created.ok(), await created.text()).toBe(true);
  const host = await created.json();
  let deletes = 0;
  const intercept = async (route: Route) => {
    if (route.request().method() !== "DELETE") return route.continue();
    deletes += 1;
    await route.continue();
  };
  try {
    await patchPreferences(request, { skip_host_remove_confirmation: false });
    await page.route(`**/api/hosts/${host.id}`, intercept);
    await page.goto("/");
    const row = hostRowByName(page, name);
    await showHostsPanel(page);
    await expect(row).toBeVisible();
    await openHostMenu(row);
    await row.locator(".host-remove").click();
    const dialog = page.locator(".host-remove-dialog");
    const confirm = dialog.getByRole("button", { name: "remove", exact: true });
    await expect(confirm).toBeFocused();
    await clickDialogGap(page, dialog);
    await expect(confirm).toBeFocused();
    await confirm.evaluate((button: HTMLButtonElement) => { button.disabled = true; });
    await expect(confirm).toBeDisabled();
    await dialog.focus();
    await dialog.press("Enter");
    expect(deletes, "disabled confirmation must not send a deletion").toBe(0);
    await expect(row).toBeVisible();
    await confirm.evaluate((button: HTMLButtonElement) => { button.disabled = false; });
    await dialog.focus();
    await dialog.press("Enter");
    await expect.poll(() => deletes).toBe(1);
    await expect(row).toHaveCount(0);
  } finally {
    await page.unroute(`**/api/hosts/${host.id}`, intercept);
    await cleanupOwnedSteps(false, [
      [`host ${host.id}`, () => request.delete(`/api/hosts/${host.id}`)],
      ["host removal consent", () => patchPreferences(request, { skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null })],
    ]);
  }
});

/** Closing must remain possible without scrolling a long settings body.
 * Real overflow and a changed scroll offset establish the short-window premise;
 * unchanged frame geometry and hit testing distinguish pinned controls from
 * controls merely reported visible outside the viewport. Both exits restore
 * focus to the gear without changing the initial focus on the first switch.
 */
test("settings keeps both close controls pinned around its grouped scrolling body", async ({ page }, testInfo) => {
  await page.goto("/");
  const gear = page.getByRole("button", { name: "settings", exact: true });
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  const body = dialog.locator(".host-settings-body");
  const dismiss = dialog.getByRole("button", { name: "close settings", exact: true });
  const close = dialog.getByRole("button", { name: "Close", exact: true });
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 560 });
    await gear.click();
    await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
    await expect(dialog.getByRole("checkbox", { includeHidden: true })).toHaveCount(18);
    await page.evaluate(() => document.fonts.ready);
    await selectSettingsSection(page, "agent trust");
    await expect(dismiss).toHaveAttribute("data-tooltip", "close settings; unsaved fields are discarded");
    await expect(dismiss).not.toHaveClass(/app-settings-close/);
    const overflow = await body.evaluate((el) => ({
      content: el.scrollHeight, height: el.clientHeight, width: el.clientWidth, contentWidth: el.scrollWidth,
    }));
    expect(overflow.height).toBeGreaterThan(0);
    expect(overflow.content, "premise: settings really exceed the short body").toBeGreaterThan(overflow.height);
    expect(overflow.contentWidth).toBeLessThanOrEqual(overflow.width);
    const header = (await dialog.locator(".host-settings-header").boundingBox())!;
    const footer = (await dialog.locator(".host-settings-footer").boundingBox())!;
    await dialog.screenshot({ path: testInfo.outputPath(`settings-pinned-top-${width}.png`) });
    await body.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await expect.poll(() => body.evaluate((el) => el.scrollTop), {
      message: "body must actually scroll before frame stability is meaningful",
    }).toBeGreaterThan(0);
    expect(await dialog.locator(".host-settings-header").boundingBox()).toEqual(header);
    expect(await dialog.locator(".host-settings-footer").boundingBox()).toEqual(footer);
    const bodyBox = (await body.boundingBox())!;
    expect(header.y).toBeGreaterThanOrEqual(0);
    expect(header.y + header.height).toBeLessThanOrEqual(bodyBox.y + 1);
    expect(bodyBox.y + bodyBox.height).toBeLessThanOrEqual(footer.y + 1);
    expect(footer.y + footer.height).toBeLessThanOrEqual(560);
    expect(await dialog.evaluate((el) => el.scrollTop)).toBe(0);
    for (const control of [dismiss, close]) {
      expect(await control.evaluate((el) => {
        const box = el.getBoundingClientRect();
        const hit = document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2);
        return Boolean(hit && el.contains(hit));
      }), "pinned close control must take a real click in the viewport").toBe(true);
    }
    await dialog.screenshot({ path: testInfo.outputPath(`settings-pinned-bottom-${width}.png`) });
    await dismiss.click();
    await expect(dialog).toHaveCount(0);
    await expect(gear).toBeFocused();
    await gear.click();
    await expect(dialog).toBeVisible();
    await close.click();
    await expect(dialog).toHaveCount(0);
    await expect(gear).toBeFocused();
  }
  // Measure the same long pane in a tall window, so the no-overflow proof
  // cannot pass merely because opening returns to the shorter sessions pane.
  await page.setViewportSize({ width: 1280, height: 5000 });
  await gear.click();
  await selectSettingsSection(page, "agent trust");
  await expect(dialog.getByRole("checkbox", { includeHidden: true })).toHaveCount(18);
  const tall = await body.evaluate((el) => ({ content: el.scrollHeight, height: el.clientHeight }));
  expect(tall.height).toBeGreaterThan(0);
  expect(tall.content, "a tall window must fit the settings without scrolling").toBeLessThanOrEqual(tall.height);
  await dialog.screenshot({ path: testInfo.outputPath("settings-pinned-tall.png") });
  await close.click();
  await expect(gear).toBeFocused();
});

/** Icon-and-label navigation keeps full names and moves focus to one mounted pane.
 * Phone navigation must use the real list/back path, reset body scrolling,
 * and forget its selection on close. Mounted hidden controls are checked
 * separately from visible panes so a missing section cannot pass the sweep.
 * Computed heading case catches transforms that conceal the displayed label;
 * Notifications' groups remain subordinate to its navigation target, while
 * neither heading level becomes visually smaller than the field labels.
 */
test("settings navigation keeps mounted panes and follows the narrow list path", async ({ page, browserName }) => {
  await page.setViewportSize({ width: 1280, height: 560 });
  await page.goto("/");
  const dialog = await openSettings(page);
  const navigation = dialog.getByRole("navigation", { name: "settings sections", exact: true });
  const body = dialog.locator(".host-settings-body");
  const panels = dialog.locator("section.app-settings-panel");
  await expect(navigation.getByRole("button")).toHaveText(webSettingsSections.map(section => settingsSectionLabels[section]));
  await expect(navigation.getByRole("button", { name: "App", exact: true })).toHaveCount(0);
  await expect(panels).toHaveCount(webSettingsSections.length);
  await expect(dialog.getByRole("checkbox", { includeHidden: true })).toHaveCount(18);
  await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
  for (const section of webSettingsSections) {
    const panel = await selectSettingsSection(page, section);
    await expect(dialog.locator("section.app-settings-panel:visible")).toHaveCount(1);
    await expect(panel.locator(":scope > h3")).toHaveText(settingsSectionLabels[section]);
    await expect(panel.locator(":scope > h3")).toHaveAttribute("tabindex", "-1");
    await expect(panel.locator(":scope > h3")).toHaveCSS("text-transform", "none");
    const button = navigation.getByRole("button", { name: settingsSectionLabels[section], exact: true });
    const icon = button.locator('.settings-section-icon[aria-hidden="true"] > svg');
    await expect(icon).toBeVisible();
    await expect(icon).toHaveCSS("color", await button.evaluate(el => getComputedStyle(el).color));
    await expect(button).toHaveAttribute("aria-pressed", "true");
    await expect(button).toHaveAttribute("aria-controls", (await panel.getAttribute("id"))!);
    // The longest section name must fit its own column rather than forcing
    // its selected fill across the divider into the settings themselves.
    expect(await button.evaluate(el => {
      const button = el.getBoundingClientRect();
      const list = el.closest("nav")!.getBoundingClientRect();
      return button.left >= list.left && button.right <= list.right && el.scrollWidth <= el.clientWidth;
    })).toBe(true);
    await expect(navigation.locator('[aria-pressed="true"]')).toHaveCount(1);
    await expect(dialog.locator("section.app-settings-panel[hidden]")).toHaveCount(webSettingsSections.length - 1);
    if (section !== "sessions") await expect(panel.locator(":scope > h3")).toBeFocused();
  }
  const notifications = await selectSettingsSection(page, "notifications");
  await expect(notifications.getByRole("region", { name: "sounds", exact: true })).toBeVisible();
  await expect(notifications.getByRole("region", { name: "ntfy alerts", exact: true })).toBeVisible();
  for (const name of ["Sounds", "ntfy alerts"]) {
    const heading = notifications.getByRole("heading", { name, level: 4, exact: true });
    await expect(heading).toBeVisible();
    await expect(heading).toHaveCSS("text-transform", "none");
    const sizes = await heading.evaluate(el => {
      const group = el.closest("section")!;
      const label = group.querySelector("label")!;
      const panelTitle = el.closest(".app-settings-panel")!.querySelector(":scope > h3")!;
      return {
        group: parseFloat(getComputedStyle(el).fontSize),
        label: parseFloat(getComputedStyle(label).fontSize),
        panel: parseFloat(getComputedStyle(panelTitle).fontSize),
      };
    });
    expect(sizes.group, `${name} must head its labels visually as well as semantically`).toBeGreaterThanOrEqual(sizes.label);
    expect(sizes.panel, "the navigation target heads its subordinate groups").toBeGreaterThan(sizes.group);
  }

  // Plain buttons use the browser's normal Tab order. WebKit deliberately
  // skips buttons under its default Tab policy; no extra tab stops compensate.
  if (browserName !== "webkit") {
    const sessions = navigation.getByRole("button", { name: "Sessions", exact: true });
    const appearance = navigation.getByRole("button", { name: "Appearance", exact: true });
    await sessions.focus();
    await expect(sessions).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(appearance).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(dialog.locator('section.app-settings-panel[aria-label="Appearance"] > h3')).toBeFocused();
  }

  await selectSettingsSection(page, "agent trust");
  await page.setViewportSize({ width: 390, height: 560 });
  await expect(navigation).toBeHidden();
  const back = dialog.getByRole("button", { name: "‹ Settings", exact: true });
  await expect(back).toBeVisible();
  expect(await body.evaluate(el => el.scrollHeight > el.clientHeight), "agent trust must overflow before testing scroll reset").toBe(true);
  await body.evaluate(el => { el.scrollTop = el.scrollHeight; });
  await expect.poll(() => body.evaluate(el => el.scrollTop)).toBeGreaterThan(0);
  await back.click();
  await expect(navigation).toBeVisible();
  await expect(dialog.locator("section.app-settings-panel:visible")).toHaveCount(0);
  await expect(back).toBeHidden();
  const trustButton = navigation.getByRole("button", { name: "Agent trust", exact: true });
  await expect(trustButton).toBeFocused();
  // Back supplies a native focus origin in both engines, including WebKit's
  // button-skipping Tab policy. Enter must activate this ordinary button.
  await page.keyboard.press("Enter");
  const trust = dialog.locator('section.app-settings-panel[aria-label="agent trust"]');
  await expect(trust).toBeVisible();
  await expect(trust.locator(":scope > h3")).toBeFocused();
  await expect.poll(() => body.evaluate(el => el.scrollTop)).toBe(0);
  await back.click();
  await navigation.getByRole("button", { name: "Checkouts", exact: true }).click();
  await expect(dialog.locator('section.app-settings-panel[aria-label="checkouts"] > h3')).toBeFocused();
  await expect(navigation).toBeHidden();
  await page.setViewportSize({ width: 1280, height: 560 });
  await expect(navigation).toBeVisible();
  await expect(dialog.locator('section.app-settings-panel[aria-label="checkouts"]')).toBeVisible();
  await dialog.getByRole("button", { name: "Close", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 560 });
  await openSettings(page);
  await expect(dialog.locator('section.app-settings-panel[aria-label="sessions"]')).toBeVisible();
  await expect(navigation).toBeHidden();
  await expect(back).toBeVisible();
  await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
});

/** Naming defaults on without writing a choice during Settings open. The real
 * route and a reload distinguish durable opt-out from an optimistic checkbox;
 * session confirmations stay independent of this adjacent switch.
 */
test("quick-launch naming defaults on and its settings switch persists independently", async ({ page, request }) => {
  const previous = await readPreferences(request);
  const gear = page.getByRole("button", { name: "settings", exact: true });
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  const naming = dialog.getByRole("checkbox", { name: "name sessions on quick launch", exact: true });
  try {
    for (const value of [null, true, false]) {
      await patchPreferences(request, { name_session_on_quick_launch: value });
      expect((await readPreferences(request)).name_session_on_quick_launch ?? null).toBe(value);
      await page.goto("/");
      await gear.click();
      await expect(naming).toBeChecked({ checked: value !== false });
      expect((await readPreferences(request)).name_session_on_quick_launch ?? null).toBe(value);
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
    }
    await gear.click();
    await expect(naming).not.toBeChecked();
    for (const value of [true, false]) {
      await naming.setChecked(value);
      await expect.poll(async () => (await readPreferences(request)).name_session_on_quick_launch).toBe(value);
      expect((await readPreferences(request)).skip_session_confirmations).toBe(previous.skip_session_confirmations);
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
      await page.reload();
      await gear.click();
      await expect(naming).toBeChecked({ checked: value });
    }
  } finally {
    await patchPreferences(request, { name_session_on_quick_launch: previous.name_session_on_quick_launch ?? null });
  }
});

/** Read the protected public shape to verify persistence independently of the
 * mounted field's local draft or success line. */
async function readCheckoutRoot(request: APIRequestContext): Promise<{ root: string | null }> {
  const reply = await request.get("/api/checkout-root");
  expect(reply.ok(), await reply.text()).toBe(true);
  return await reply.json() as { root: string | null };
}

/** Inspect the stored all-hosts command independently of the field's draft.
 * Null and an explicit disable must stay distinct unless the user saves. */
async function readPostClone(request: APIRequestContext): Promise<{ command: string | null }> {
  const reply = await request.get("/api/checkout-post-clone");
  expect(reply.ok(), await reply.text()).toBe(true);
  return await reply.json() as { command: string | null };
}

/** Each link switch persists its own grant without changing command or YOLO authority.
 * An owned disconnected host keeps this proof independent of the suite's live session host. */
test("host browser permissions persist independently through the settings switches", async ({ page, request }, testInfo) => {
  const name = `browser-settings-${testInfo.project.name}.invalid`;
  const created = await request.post("/api/hosts", { data: { ssh: name } });
  expect(created.ok(), await created.text()).toBe(true);
  const host = await created.json();
  /** Read the stored view, so an optimistic checkbox cannot stand in for persistence. */
  const read = async () => {
    const response = await request.get("/api/hosts");
    expect(response.ok(), await response.text()).toBe(true);
    const body = await response.json();
    return body.hosts.find((row: { id: number }) => row.id === host.id);
  };
  try {
    expect(await read()).toMatchObject({ links_without_asking: false, local_links_without_asking: false, commands_without_asking: false, yolo_without_asking: false });
    await page.goto("/");
    const row = hostRowByName(page, name);
    await showHostsPanel(page);
    await expect(row).toBeVisible();
    await openHostMenu(row);
    await row.locator(".host-settings").click();
    const dialog = page.locator(".host-settings-dialog");
    await expect(dialog).toBeVisible();
    const publicLinks = dialog.getByRole("checkbox", { name: "Open links without asking", exact: true });
    const localLinks = dialog.getByRole("checkbox", { name: "Open local-network links without asking", exact: true });
    await expect(publicLinks).not.toBeChecked();
    await expect(localLinks).not.toBeChecked();
    await publicLinks.check();
    await expect.poll(read).toMatchObject({ links_without_asking: true, local_links_without_asking: false, commands_without_asking: false, yolo_without_asking: false });
    await expect(localLinks).not.toBeChecked();
    await localLinks.check();
    await expect.poll(read).toMatchObject({ links_without_asking: true, local_links_without_asking: true, commands_without_asking: false });
    await publicLinks.uncheck();
    await expect.poll(read).toMatchObject({ links_without_asking: false, local_links_without_asking: true, commands_without_asking: false });
    await dialog.locator(".host-settings-close").click();
    await openHostMenu(row);
    await row.locator(".host-settings").click();
    await expect(publicLinks).not.toBeChecked();
    await expect(localLinks).toBeChecked();
    await localLinks.uncheck();
    await expect.poll(read).toMatchObject({ links_without_asking: false, local_links_without_asking: false, commands_without_asking: false });
  } finally {
    const removed = await request.delete(`/api/hosts/${host.id}`);
    expect(removed.ok(), await removed.text()).toBe(true);
  }
});

/** Opening is read-only for all three stored states. Explicit Save preserves
 * executable spacing and newlines, refusal keeps the draft and outcome across
 * navigation, and an all-whitespace save clears rather than storing a disable. */
test("settings saves and clears the all-hosts post-clone command", async ({ page, request }, testInfo) => {
  const previous = await readPostClone(request);
  const read = () => readPostClone(request);
  await page.goto("/");
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  const field = dialog.locator(".checkout-post-clone-field");
  const command = field.getByRole("textbox", { name: "post-clone command", exact: true });
  const save = field.getByRole("button", { name: "save", exact: true });
  const chosen = "  printf 'first'\nprintf 'second'  ";
  try {
    for (const value of [null, "", "  existing command\nnext line  "]) {
      const seeded = await request.put("/api/checkout-post-clone", { data: { command: value } });
      expect(seeded.ok(), await seeded.text()).toBe(true);
      expect(await read()).toEqual({ command: value });
      await openSettings(page, "checkouts");
      await expect(command).toBeEditable();
      await expect(command).toHaveValue(value ?? "");
      expect(await read()).toEqual({ command: value });
      if (value === null) {
        await field.scrollIntoViewIfNeeded();
        await dialog.screenshot({ path: testInfo.outputPath("post-clone-empty.png") });
      }
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
      expect(await read()).toEqual({ command: value });
    }
    await openSettings(page, "checkouts");
    await expect(command).toBeEditable();
    const before = await read();
    const refused = "x".repeat(16385);
    await command.fill(refused);
    await save.click();
    await expect(field.getByRole("alert")).toHaveText("post-clone command is 16385 bytes; the limit is 16384 bytes");
    await expect(command).toHaveValue(refused);
    expect(await read()).toEqual(before);
    await selectSettingsSection(page, "sessions");
    await selectSettingsSection(page, "checkouts");
    await expect(command).toHaveValue(refused);
    await expect(field.getByRole("alert")).toHaveText("post-clone command is 16385 bytes; the limit is 16384 bytes");
    expect(await read()).toEqual(before);
    await dialog.screenshot({ path: testInfo.outputPath("post-clone-refused.png") });

    await command.fill("  printf 'first'");
    await command.focus();
    await expect(command).toBeFocused();
    await command.press("End");
    await command.press("Enter");
    await command.pressSequentially("printf 'second'  ");
    await expect(command).toHaveValue(chosen);
    expect(await read()).toEqual(before);
    const refetched = page.waitForResponse((response) =>
      response.url().endsWith("/api/checkout-post-clone") && response.request().method() === "GET"
    );
    await save.click();
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    const refreshed = await refetched;
    expect(refreshed.ok(), await refreshed.text()).toBe(true);
    expect(await refreshed.json()).toEqual({ command: chosen });
    expect(await read()).toEqual({ command: chosen });
    await expect(field.getByRole("alert")).toHaveCount(0);
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 900 });
      await field.scrollIntoViewIfNeeded();
      await dialog.screenshot({ path: testInfo.outputPath(`post-clone-command-${width}.png`) });
      const inputBounds = await command.boundingBox();
      const fieldBounds = await field.boundingBox();
      expect(inputBounds).not.toBeNull();
      expect(fieldBounds).not.toBeNull();
      expect(inputBounds!.x + inputBounds!.width).toBeLessThanOrEqual(fieldBounds!.x + fieldBounds!.width + 1);
    }
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(chosen);
    await command.fill(" \n\t ");
    await save.click();
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    expect(await read()).toEqual({ command: null });
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(command).toBeEditable();
    await expect(command).toHaveValue("");
  } finally {
    const restored = await request.put("/api/checkout-post-clone", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await read()).toEqual(previous);
  }
});

/** Textarea normalization must not silently rewrite an executable command.
 * Seed real CR and CRLF values through the API, prove editing cannot persist
 * their normalized form, then clear and save normally in the same mounted field.
 */
test("post-clone commands with carriage returns refuse edits but can be cleared", async ({ page, request }) => {
  const previous = await readPostClone(request);
  let writes = 0;
  const submitted: Request[] = [];
  const intercept = async (route: Route) => {
    if (route.request().method() === "PUT") {
      writes++;
      submitted.push(route.request());
    }
    await route.continue();
  };
  await page.route("**/api/checkout-post-clone", intercept);
  try {
    await page.goto("/");
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    const field = dialog.locator(".checkout-post-clone-field");
    const command = field.getByRole("textbox", { name: "post-clone command", exact: true });
    const save = field.getByRole("button", { name: "save", exact: true });
    for (const stored of ["printf 'one\rtwo'", "printf 'one\r\ntwo'"]) {
      const seeded = await request.put("/api/checkout-post-clone", { data: { command: stored } });
      expect(seeded.ok(), await seeded.text()).toBe(true);
      expect(await readPostClone(request)).toEqual({ command: stored });
      await openSettings(page, "checkouts");
      await expect(command).toBeEditable();
      await expect(command).toHaveValue(stored.replace(/\r\n?/g, "\n"));
      await command.focus();
      await expect(command).toBeFocused();
      await command.press("End");
      await command.pressSequentially(" # edited");
      const edited = await command.inputValue();
      expect(edited).not.toContain("\r");
      expect(edited).toContain("# edited");
      const before = writes;
      await save.click();
      await expect(field.getByRole("alert")).toContainText("carriage return this field would turn into a newline");
      await expect(command).toHaveValue(edited);
      expect(writes).toBe(before);
      expect(await readPostClone(request)).toEqual({ command: stored });

      await command.fill(" \n\t ");
      const refetched = page.waitForResponse(response =>
        response.url().endsWith("/api/checkout-post-clone") && response.request().method() === "GET"
      );
      await save.click();
      await expect(field.getByRole("status")).toHaveText("post-clone command saved");
      expect(await (await refetched).json()).toEqual({ command: null });
      expect(await readPostClone(request)).toEqual({ command: null });
      await expect(field.getByRole("alert")).toHaveCount(0);
      await command.fill("printf 'ordinary'");
      await save.click();
      await expect(field.getByRole("status")).toHaveText("post-clone command saved");
      expect(await readPostClone(request)).toEqual({ command: "printf 'ordinary'" });
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
    }
  } finally {
    // Even an assertion during a save cannot let its late PUT undo restoration.
    for (const put of submitted) {
      const reply = await put.response();
      if (reply) await reply.finished();
    }
    await page.unroute("**/api/checkout-post-clone", intercept);
    const restored = await request.put("/api/checkout-post-clone", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await readPostClone(request)).toEqual(previous);
  }
});

/** A delayed initial read cannot turn the blank placeholder into a clear.
 * Hold the GET at the network boundary and try Save while locked, then release
 * it and require the stored command to seed the field without any PUT. */
test("post-clone command stays locked until its initial read completes", async ({ page, request }) => {
  const previous = await readPostClone(request);
  let release!: () => void;
  let signalReading!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const reading = new Promise<void>((resolve) => { signalReading = resolve; });
  let writes = 0;
  const intercept = async (route: Route) => {
    if (route.request().method() === "GET") {
      signalReading();
      await held;
    } else if (route.request().method() === "PUT") {
      writes++;
    }
    await route.continue();
  };
  await page.route("**/api/checkout-post-clone", intercept);
  try {
    await page.goto("/");
    await openSettings(page, "checkouts");
    await reading;
    const field = page.locator(".checkout-post-clone-field");
    const command = field.getByRole("textbox", { name: "post-clone command", exact: true });
    const save = field.getByRole("button", { name: "save", exact: true });
    await expect(command).not.toBeEditable();
    await expect(command).toHaveValue("");
    await expect(save).toHaveAttribute("aria-disabled", "true");
    // aria-disabled retains focusability for tooltips and keyboard discovery;
    // dispatch a real click to prove the handler refuses a locked Save.
    await save.dispatchEvent("click");
    release();
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(previous.command ?? "");
    expect(writes).toBe(0);
    expect(await readPostClone(request)).toEqual(previous);
    // Loaded but untouched, Save still writes nothing: a disabled command shows
    // as an empty box, and writing that back would store "no command" instead.
    await expect(save).toHaveAttribute("aria-disabled", "true");
    await save.dispatchEvent("click");
    await command.fill("printf 'edited after load'");
    await save.click();
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    expect(writes, "saves go out in click order, so only the edited one was sent").toBe(1);
  } finally {
    release();
    await page.unroute("**/api/checkout-post-clone", intercept);
    const restored = await request.put("/api/checkout-post-clone", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
  }
});

/** A save keeps its lock across section changes and outlives its dismissed field.
 * Reopening must wait before seeding an editable draft, or an older held save
 * could overwrite a newer one while reopened Settings claims it was saved. */
test("a pending post-clone save orders reopened settings before the next save", async ({ page, request }) => {
  const previous = await readPostClone(request);
  let release!: () => void;
  let signalSubmitted!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const submitted = new Promise<void>((resolve) => { signalSubmitted = resolve; });
  let submittedRequest: Request | undefined;
  const intercept = async (route: Route) => {
    if (route.request().method() !== "PUT") return route.continue();
    submittedRequest = route.request();
    signalSubmitted();
    await held;
    await route.continue();
  };
  const chosen = "printf 'save survives close'";
  await page.route("**/api/checkout-post-clone", intercept);
  try {
    await page.goto("/");
    await openSettings(page, "checkouts");
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    const field = dialog.locator(".checkout-post-clone-field");
    const command = field.getByRole("textbox", { name: "post-clone command", exact: true });
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(previous.command ?? "");
    await command.fill(chosen);
    await field.getByRole("button", { name: "save", exact: true }).click();
    await submitted;
    const pendingSave = field.getByRole("button", { name: "saving…", exact: true });
    await expect(pendingSave).toBeFocused();
    await expect(pendingSave).toHaveAttribute("aria-disabled", "true");
    await expect(command).not.toBeEditable();
    expect(submittedRequest!.postDataJSON()).toEqual({ command: chosen });
    expect(await readPostClone(request)).toEqual(previous);
    await selectSettingsSection(page, "sessions");
    await selectSettingsSection(page, "checkouts");
    await expect(command).toHaveValue(chosen);
    await expect(command).not.toBeEditable();
    await expect(pendingSave).toHaveAttribute("aria-disabled", "true");
    expect(await readPostClone(request)).toEqual(previous);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    // The old field is gone: this lock must belong to the reopened read,
    // not the previous component's local saving flag.
    await openSettings(page, "checkouts");
    await expect(dialog).toBeVisible();
    await expect(command).not.toBeEditable();
    expect(await readPostClone(request)).toEqual(previous);
    // sleep-ok: observe the reopened field while the earlier save is held; initial loading alone is not an ordering proof.
    await page.waitForTimeout(150);
    await expect(command).not.toBeEditable();
    release();
    await expect.poll(() => readPostClone(request)).toEqual({ command: chosen });
    await expect(command).toBeEditable();
    await expect(command).toHaveValue(chosen);
    const newer = "printf 'newer save wins'";
    await command.fill(newer);
    await field.getByRole("button", { name: "save", exact: true }).click();
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    expect(await readPostClone(request)).toEqual({ command: newer });
    await selectSettingsSection(page, "hosts");
    await selectSettingsSection(page, "checkouts");
    await expect(command).toHaveValue(newer);
    await expect(field.getByRole("status")).toHaveText("post-clone command saved");
    expect(await readPostClone(request)).toEqual({ command: newer });
  } finally {
    release();
    // Settle the owned PUT before restoration even when an assertion fails,
    // or a late forwarded request could overwrite the previous command.
    const reply = await submittedRequest?.response();
    if (reply) await reply.finished();
    await page.unroute("**/api/checkout-post-clone", intercept);
    const restored = await request.put("/api/checkout-post-clone", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await readPostClone(request)).toEqual(previous);
  }
});

/** Editing a folder is an explicit configuration write, not a switch or a
 * local-only draft. Real route readback and reopening distinguish persisted
 * save/clear from appearance; navigation must preserve refused drafts and
 * both refusal and success outcomes without changing the stored root.
 */
test("settings saves and clears the all-hosts checkout folder", async ({ page, request }, testInfo) => {
  await page.goto("/");
  await openSettings(page, "checkouts");
  const dialog = page.getByRole("dialog", { name: "settings", exact: true });
  await expect(dialog).toBeVisible();
  const read = () => readCheckoutRoot(request);
  const previous = await read();
  const field = dialog.locator(".checkout-root-field");
  const folder = field.getByRole("textbox", { name: "checkout folder", exact: true });
  const save = field.getByRole("button", { name: "save", exact: true });
  const chosen = "~/managed-checkouts";
  const submitted: Request[] = [];
  let bodyFailed = false;
  page.on("request", outgoing => {
    if (outgoing.url().endsWith("/api/checkout-root") && outgoing.method() === "PUT") submitted.push(outgoing);
  });
  try {
    await expect(folder).toHaveValue(previous.root ?? "");
    await expect(folder).toBeEditable();
    await dialog.screenshot({ path: testInfo.outputPath("checkout-settings-loaded.png") });
    await folder.fill("relative-folder");
    await save.click();
    await expect(field.getByRole("alert")).toContainText("not the relative path 'relative-folder'");
    expect(await read()).toEqual(previous);
    await expect(folder).toHaveValue("relative-folder");
    await selectSettingsSection(page, "sessions");
    await selectSettingsSection(page, "checkouts");
    await expect(folder).toHaveValue("relative-folder");
    await expect(field.getByRole("alert")).toContainText("not the relative path 'relative-folder'");
    expect(await read()).toEqual(previous);

    await folder.fill(chosen);
    // Editing alone must not write; a separate click commits configuration.
    expect(await read()).toEqual(previous);
    // The mounted field must refetch without waiting for the revision watcher.
    // Independent route readback and reopening cannot prove this client action.
    const refetched = page.waitForResponse((response) =>
      response.url().endsWith("/api/checkout-root") && response.request().method() === "GET"
    );
    await save.click();
    await expect(field.getByRole("status")).toHaveText("checkout folder saved");
    const refreshed = await refetched;
    expect(refreshed.ok(), await refreshed.text()).toBe(true);
    expect(await refreshed.json()).toEqual({ root: chosen });
    expect(await read()).toEqual({ root: chosen });
    await expect(field.getByRole("alert")).toHaveCount(0);
    await selectSettingsSection(page, "hosts");
    await selectSettingsSection(page, "checkouts");
    await expect(folder).toHaveValue(chosen);
    await expect(field.getByRole("status")).toHaveText("checkout folder saved");
    expect(await read()).toEqual({ root: chosen });
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 900 });
      await field.screenshot({ path: testInfo.outputPath(`checkout-settings-saved-${width}.png`) });
      const inputBounds = await folder.boundingBox();
      const fieldBounds = await field.boundingBox();
      expect(inputBounds).not.toBeNull();
      expect(fieldBounds).not.toBeNull();
      expect(inputBounds!.x + inputBounds!.width).toBeLessThanOrEqual(fieldBounds!.x + fieldBounds!.width + 1);
    }
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(folder).toHaveValue(chosen);
    await expect(folder).toBeEditable();
    await folder.fill("");
    await save.click();
    await expect(field.getByRole("status")).toHaveText("checkout folder saved");
    expect(await read()).toEqual({ root: null });
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "checkouts");
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue("");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["submitted checkout saves", async () => {
        // A page-owned save can outlive the failed assertion. Its terminal
        // response (including a transport failure) must precede restoration.
        for (const outgoing of submitted) {
          const reply = await outgoing.response().catch(() => null);
          if (reply) await reply.finished().catch(() => null);
        }
      }],
      ["checkout folder", async () => {
        const restored = await request.put("/api/checkout-root", { data: previous });
        expect(restored.ok(), await restored.text()).toBe(true);
        expect(await read()).toEqual(previous);
      }],
    ]);
  }
});

/** Navigation keeps both unsaved checkout drafts, while closing discards them.
 * Real API readback and a zero-PUT assertion distinguish a mounted draft from
 * accidental autosave. No configuration write is needed for this boundary.
 */
test("checkout drafts survive switching sections and are discarded on close", async ({ page, request }) => {
  const previousRoot = await readCheckoutRoot(request);
  const previousCommand = await readPostClone(request);
  const writes: Request[] = [];
  page.on("request", outgoing => {
    if (outgoing.method() === "PUT" && /\/api\/checkout-(root|post-clone)$/.test(outgoing.url())) writes.push(outgoing);
  });
  await page.goto("/");
  const dialog = await openSettings(page, "checkouts");
  const folder = dialog.getByRole("textbox", { name: "checkout folder", exact: true });
  const command = dialog.getByRole("textbox", { name: "post-clone command", exact: true });
  await expect(folder).toBeEditable();
  await expect(command).toBeEditable();
  await expect(folder).toHaveValue(previousRoot.root ?? "");
  await expect(command).toHaveValue((previousCommand.command ?? "").replace(/\r\n?/g, "\n"));
  const folderDraft = `${previousRoot.root ?? "~/checkouts"}/unsaved-section-draft`;
  const commandDraft = `${(previousCommand.command ?? "").replace(/\r\n?/g, "\n")}\nprintf 'unsaved section draft'`;
  await folder.fill(folderDraft);
  await command.fill(commandDraft);
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 800 });
    await selectSettingsSection(page, "notifications");
    await selectSettingsSection(page, "checkouts");
    await expect(folder).toHaveValue(folderDraft);
    await expect(command).toHaveValue(commandDraft);
    expect(await readCheckoutRoot(request)).toEqual(previousRoot);
    expect(await readPostClone(request)).toEqual(previousCommand);
    expect(writes).toHaveLength(0);
  }
  await dialog.getByRole("button", { name: "Close", exact: true }).click();
  await openSettings(page);
  await expect(dialog.locator('section.app-settings-panel[aria-label="sessions"]')).toBeVisible();
  await selectSettingsSection(page, "checkouts");
  await expect(folder).toBeEditable();
  await expect(command).toBeEditable();
  await expect(folder).toHaveValue(previousRoot.root ?? "");
  await expect(command).toHaveValue((previousCommand.command ?? "").replace(/\r\n?/g, "\n"));
  expect(await readCheckoutRoot(request)).toEqual(previousRoot);
  expect(await readPostClone(request)).toEqual(previousCommand);
  expect(writes).toHaveLength(0);
});

/** An untouched Save writes nothing: Settings read the folder once, so writing
 * that draft back would undo a command-line change made while it was open
 * (SPEC.md, "One GUI at a time"). Only edited text is saved, without its
 * surrounding whitespace; independent API readback checks the stored spelling.
 */
test("an untouched checkout folder save writes nothing and edited paths are trimmed", async ({ page, request }) => {
  const previous = await readCheckoutRoot(request);
  const submitted: Request[] = [];
  let bodyFailed = false;
  page.on("request", outgoing => {
    if (outgoing.url().endsWith("/api/checkout-root") && outgoing.method() === "PUT") submitted.push(outgoing);
  });
  try {
    const literal = "~/literal-checkouts ";
    const seeded = await request.put("/api/checkout-root", { data: { root: literal } });
    expect(seeded.ok()).toBe(true);
    expect(await readCheckoutRoot(request)).toEqual({ root: literal });
    await page.goto("/");
    await openSettings(page, "checkouts");
    const field = page.getByRole("dialog", { name: "settings", exact: true }).locator(".checkout-root-field");
    const folder = field.getByRole("textbox", { name: "checkout folder", exact: true });
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue(literal);
    /** Each Save needs its own terminal reply; an old success notice cannot
     * establish that the next configuration write has completed.
     */
    const saveAndWait = async () => {
      const finished = page.waitForResponse(response =>
        response.url().endsWith("/api/checkout-root") && response.request().method() === "PUT"
      );
      await field.getByRole("button", { name: "save", exact: true }).click();
      expect((await finished).ok()).toBe(true);
      await expect(field.getByRole("status")).toHaveText("checkout folder saved");
    };
    const outside = await request.put("/api/checkout-root", { data: { root: "~/changed-outside" } });
    expect(outside.ok(), "premise: the folder changes while Settings shows the old one").toBe(true);
    await expect(folder).toHaveValue(literal);
    const save = field.getByRole("button", { name: "save", exact: true });
    await expect(save).toHaveAttribute("aria-disabled", "true");
    // Playwright waits on aria-disabled buttons; force the click a user can still make.
    await save.click({ force: true });
    await folder.fill("  ~/edited-checkouts  ");
    await saveAndWait();
    // Saves are sent in click order, so an untouched write would show up here.
    expect(submitted.map((put) => put.postDataJSON())).toEqual([{ root: "~/edited-checkouts" }]);
    expect(await readCheckoutRoot(request)).toEqual({ root: "~/edited-checkouts" });
    await expect(folder).toHaveValue("~/edited-checkouts");
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["submitted checkout saves", async () => {
        for (const outgoing of submitted) {
          const reply = await outgoing.response();
          if (reply) await reply.finished();
        }
      }],
      ["checkout folder", async () => {
        const restored = await request.put("/api/checkout-root", { data: previous });
        expect(restored.ok()).toBe(true);
        expect(await readCheckoutRoot(request)).toEqual(previous);
      }],
    ]);
  }
});

/** Save is a submitted operation even if Settings closes while it is pending.
 * Hold the real PUT before forwarding it, dismiss the mounted field, then
 * release it and inspect persistent state. Component-owned cancellation would
 * leave the old folder, so the success line alone cannot satisfy this test. */
test("a pending checkout folder save survives closing settings", async ({ page, request }) => {
  const previous = await readCheckoutRoot(request);
  let release!: () => void;
  let signalSubmitted!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const submitted = new Promise<void>((resolve) => { signalSubmitted = resolve; });
  let submittedRequest: Request | undefined;
  const intercept = async (route: Route) => {
    if (route.request().method() !== "PUT") return route.continue();
    submittedRequest = route.request();
    signalSubmitted();
    await held;
    await route.continue();
  };
  await page.route("**/api/checkout-root", intercept);
  try {
    await page.goto("/");
    await openSettings(page, "checkouts");
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    const folder = dialog.getByRole("textbox", { name: "checkout folder", exact: true });
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue(previous.root ?? "");
    await folder.fill("~/save-survives-close");
    await dialog.locator(".checkout-root-field").getByRole("button", { name: "save", exact: true }).click();
    await submitted;
    const pendingSave = dialog.getByRole("button", { name: "saving…", exact: true });
    await expect(pendingSave).toBeFocused();
    await expect(pendingSave).toHaveAttribute("aria-disabled", "true");
    expect(await readCheckoutRoot(request)).toEqual(previous);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    release();
    await expect.poll(() => readCheckoutRoot(request)).toEqual({ root: "~/save-survives-close" });
    await openSettings(page, "checkouts");
    await expect(folder).toBeEditable();
    await expect(folder).toHaveValue("~/save-survives-close");
  } finally {
    release();
    // A failed assertion can leave the released PUT in flight. Wait for its
    // response or cancellation before restoring, or it can overwrite cleanup.
    const reply = await submittedRequest?.response();
    if (reply) await reply.finished();
    await page.unroute("**/api/checkout-root", intercept);
    const restored = await request.put("/api/checkout-root", { data: previous });
    expect(restored.ok(), await restored.text()).toBe(true);
    expect(await readCheckoutRoot(request)).toEqual(previous);
  }
});

/** Host-removal consent can be changed back without reloading the client.
 * Independent cleanup must restore both host and session consent even when
 * removing one of the owned hosts fails, since later specs share the helm.
 */
test("settings reverses a permanent removal answer without reloading", async ({ page, request }, testInfo) => {
  const previous = await readPreferences(request);
  const owned: number[] = [];
  const names: string[] = [];
  let bodyFailed = false;
  try {
    // The session opt-out does not answer a host's independent remove question.
    await patchPreferences(request, { skip_host_remove_confirmation: null, skip_session_confirmations: true });
    for (let n = 0; n < 3; n++) {
      const name = `settings-${testInfo.project.name}-${n}.invalid`;
      const reply = await request.post("/api/hosts", { data: { ssh: name } });
      expect(reply.ok(), await reply.text()).toBe(true);
      const host = await reply.json();
      owned.push(host.id);
      names.push(name);
    }
    await page.goto("/");
    const first = hostRowByName(page, names[0]);
    await showHostsPanel(page);
    await expect(first).toBeVisible();
    await openHostMenu(first);
    await first.locator(".host-remove").click();
    const removal = page.locator(".host-remove-dialog");
    await expect(removal).toBeVisible();
    await expect(removal.getByText(`undo "don't ask again" with the gear at the top of the sidebar`)).toBeVisible();
    await removal.getByRole("button", { name: "remove, don't ask again", exact: true }).click();
    await expect(first).toHaveCount(0);
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(true);

    // The permanent answer itself takes effect before settings changes it.
    const second = hostRowByName(page, names[1]);
    await openHostMenu(second);
    await second.locator(".host-remove").click();
    await expect(second).toHaveCount(0);
    await expect(removal).toHaveCount(0);

    const gear = page.getByRole("button", { name: "settings", exact: true });
    await openSettings(page, "hosts");
    const settings = page.getByRole("dialog", { name: "settings", exact: true });
    const toggle = settings.getByRole("checkbox", { name: "remove hosts without asking", exact: true });
    await expect(toggle).toBeChecked();
    await toggle.uncheck();
    await expect(toggle).not.toBeChecked();
    await expect(settings.locator("#host-remove-preference-help")).toContainText("asks before forgetting");
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(false);
    await settings.getByRole("button", { name: "Close", exact: true }).click();
    await expect(gear).toBeFocused();

    const third = hostRowByName(page, names[2]);
    await openHostMenu(third);
    await third.locator(".host-remove").click();
    await expect(removal).toBeVisible();
    await expect(third).toBeVisible();
    await removal.getByRole("button", { name: "cancel", exact: true }).click();
    // Ticking the settings checkbox is also a permanent answer, not merely
    // a way to clear one. The same still-registered host now removes at once.
    await openSettings(page, "hosts");
    await toggle.check();
    await expect.poll(async () => (await readPreferences(request)).skip_host_remove_confirmation).toBe(true);
    await settings.getByRole("button", { name: "Close", exact: true }).click();
    await openHostMenu(third);
    await third.locator(".host-remove").click();
    await expect(third).toHaveCount(0);
    await expect(removal).toHaveCount(0);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ...owned.map((id): [string, () => Promise<unknown>] => [`host ${id}`, () => request.delete(`/api/hosts/${id}`)]),
      ["host removal consent", () => patchPreferences(request, { skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null })],
      ["session consent", () => patchPreferences(request, { skip_session_confirmations: previous.skip_session_confirmations ?? null })],
    ]);
  }
});

/** Described segments must retain native radio keys and the real preference
 * writer. Keeping all explanations visible matters because two choices can
 * pass clicks to a program, including one that opens another browser tab.
 */
test("terminal link segments keep native keys and descriptions", async ({ page, request }) => {
  const previous = await readPreferences(request);
  let bodyFailed = true;
  const choices = [
    { value: "intercept", label: "open in Farhelm, don't pass the click on", help: "Farhelm opens the link once. The program does not receive that click or a drag starting there." },
    { value: "both", label: "open in Farhelm and also pass the click on", help: "Use this when the program needs link clicks. A program that opens links itself may open a second tab." },
    { value: "program", label: "pass the click on only", help: "Let the program handle links. Farhelm does not open them; without mouse reporting, clicking a link does nothing." },
  ] as const;
  try {
    await patchPreferences(request, { terminal_link_click: "intercept" });
    expect((await readPreferences(request)).terminal_link_click).toBe("intercept");
    await page.goto("/");
    const dialog = await openSettings(page, "terminal");
    const group = dialog.getByRole("radiogroup", { name: "Terminal web links", exact: true });
    for (const colorScheme of ["dark", "light"] as const) {
      await page.emulateMedia({ colorScheme });
      for (const width of [1280, 390]) {
        await page.setViewportSize({ width, height: 800 });
        await expect(group.getByRole("radio")).toHaveCount(3);
        for (const choice of choices) {
          const radio = group.getByRole("radio", { name: choice.label, exact: true });
          const helpId = `terminal-link-${choice.value}-help`;
          await expect(radio).toHaveAttribute("aria-describedby", helpId);
          await expect(group.locator(`#${helpId}`)).toHaveText(choice.help);
          await expect(group.locator(`#${helpId}`)).toBeVisible();
        }
        const first = group.getByRole("radio", { name: choices[0].label, exact: true });
        const both = group.getByRole("radio", { name: choices[1].label, exact: true });
        const program = group.getByRole("radio", { name: choices[2].label, exact: true });
        await first.locator("..").click();
        await expect(first).toBeChecked();
        await expect.poll(async () => (await readPreferences(request)).terminal_link_click).toBe("intercept");
        await first.focus();
        await expect(first).toBeFocused();
        await first.press("ArrowRight");
        await expect(both).toBeChecked();
        await expect(both).toBeFocused();
        await expect.poll(async () => (await readPreferences(request)).terminal_link_click).toBe("both");
        // Focusing an unselected radio must not itself choose it. Space uses
        // the native input even though its adjacent span supplies the drawing.
        await program.focus();
        await expect(program).toBeFocused();
        await expect(program).not.toBeChecked();
        await program.press("Space");
        await expect(program).toBeChecked();
        await expect.poll(async () => (await readPreferences(request)).terminal_link_click).toBe("program");
        const painted = program.locator("..").locator(":scope > span");
        await expect(painted).toHaveCSS("background-color", await resolveToken(page, "--accent-fill"));
        await expect(painted).toHaveCSS("outline-color", await resolveToken(page, "--on-fill"));
        const geometry = await group.evaluate((element) => ({ width: element.clientWidth, content: element.scrollWidth }));
        expect(geometry.content, `${colorScheme} terminal choices fit width ${width}`).toBeLessThanOrEqual(geometry.width);
      }
    }
    bodyFailed = false;
  } finally {
    await endPreferenceSaveTest(page, async () => {}, () => {},
      () => patchPreferences(request, { terminal_link_click: previous.terminal_link_click ?? null }), bodyFailed);
  }
});

/** Every pane's switches must fit beside their labels and keep native keyboard
 * behavior. The modal also isolates the page and returns focus to its opener;
 * styling a replacement control must not bypass either lifecycle.
 */
test("settings dismisses a row menu, isolates the page and returns focus on Escape", async ({ page, request }, testInfo) => {
  const previous = await readPreferences(request);
  try {
    await patchPreferences(request, { skip_host_setup_confirmation: null, skip_host_remove_confirmation: false });
    await page.goto("/");
    const row = page.locator(".host-row").first();
    await showHostsPanel(page);
    await expect(row).toBeVisible();
    await openHostMenu(row);
    await expect(row.locator('[role="menu"]')).toBeVisible();
    const gear = page.getByRole("button", { name: "settings", exact: true });
    await gear.click();
    const dialog = page.getByRole("dialog", { name: "settings", exact: true });
    await expect(dialog).toBeVisible();
    await expect(page.locator('[role="menu"]')).toHaveCount(0);
    await expect(dialog.getByRole("checkbox", { includeHidden: true })).toHaveCount(18);
    const setup = dialog.getByRole("checkbox", { name: "set up new hosts without asking", exact: true });
    await expect(dialog.getByRole("checkbox", { name: "skip session confirmations", exact: true })).toBeFocused();
    await selectSettingsSection(page, "hosts");
    await expect(setup).not.toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "remove hosts without asking", exact: true })).not.toBeChecked();
    expect(await dialog.evaluate((el) => el.closest(".app-bar"))).toBeNull();
    expect(await gear.evaluate((el) => Boolean(el.closest("[inert]")))).toBe(true);
    await page.evaluate(() => document.fonts.ready);
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 800 });
      let measuredSwitches = 0;
      for (const section of webSettingsSections) {
        const panel = await selectSettingsSection(page, section);
        await dialog.screenshot({ path: testInfo.outputPath(`settings-switches-${width}-${section}.png`) });
        // Measure rendered text independently of the label's full-width box:
        // a checkbox above or before its caption must fail this contract. Native
        // radios paint a separate segmented group rather than switch geometry.
        const rows = await panel.locator('.app-settings-choice:has(input[type="checkbox"])').evaluateAll((labels) => labels.map((label) => {
          const input = label.querySelector("input")!;
          const range = document.createRange();
          const text = Array.from(label.childNodes).find((node) => node !== input && node.textContent?.trim())!;
          range.selectNode(text);
          const caption = range.getBoundingClientRect();
          const control = input.getBoundingClientRect();
          const row = label.getBoundingClientRect();
          return {
            textRight: caption.right, textTop: caption.top, textBottom: caption.bottom,
            controlLeft: control.left, controlRight: control.right,
            controlTop: control.top, controlBottom: control.bottom,
            width: control.width, height: control.height, rowRight: row.right,
            appearance: getComputedStyle(input).appearance,
          };
        }));
        measuredSwitches += rows.length;
        for (const row of rows) {
          expect(row.textRight).toBeLessThan(row.controlLeft);
          expect(row.controlTop).toBeLessThan(row.textBottom);
          expect(row.controlBottom).toBeGreaterThan(row.textTop);
          expect(row.controlRight).toBeCloseTo(row.rowRight, 0);
          expect(row.width).toBe(34);
          expect(row.height).toBe(20);
          expect(row.appearance).toBe("none");
        }
        const alignment = await panel.evaluate((el) => ({
          choices: Array.from(el.querySelectorAll(".app-settings-choice:not(.segment-radio), .terminal-link-choices"), (row) => row.getBoundingClientRect().left),
          help: Array.from(el.querySelectorAll(".host-settings-help"), (help) => help.getBoundingClientRect().left),
          heading: el.querySelector(":scope > h3")!.getBoundingClientRect().left,
          overflowing: el.scrollWidth > el.clientWidth,
        }));
        expect(alignment.overflowing).toBe(false);
        for (const left of [...alignment.choices, ...alignment.help]) expect(left).toBeCloseTo(alignment.heading, 0);
      }
      expect(measuredSwitches, "every mounted switch must be measured in its visible pane").toBe(18);
    }
    // Space exercises the native input rather than Playwright's checked-state
    // setter, and the real helm reply proves the preference writer still runs.
    await selectSettingsSection(page, "hosts");
    await setup.focus();
    await expect(setup).toBeFocused();
    await setup.press("Space");
    await expect(setup).toBeChecked();
    await expect.poll(async () => (await readPreferences(request)).skip_host_setup_confirmation).toBe(true);
    await setup.press("Space");
    await expect(setup).not.toBeChecked();
    await expect.poll(async () => (await readPreferences(request)).skip_host_setup_confirmation).toBe(false);
    await setup.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(gear).toBeFocused();
    expect(await gear.evaluate((el) => Boolean(el.closest("[inert]")))).toBe(false);
  } finally {
    await patchPreferences(request, {
      skip_host_setup_confirmation: previous.skip_host_setup_confirmation ?? null,
      skip_host_remove_confirmation: previous.skip_host_remove_confirmation ?? null,
    });
  }
});

/** The rail keeps Settings reachable independently of a long version readout
 * in both normal and macOS chrome. This exercises real hit testing, not
 * AppKit dragging, while retaining the title band's height and drag fences.
 */
test("settings gear stays clickable in the rail beside a long version in narrow windows", async ({ page }, testInfo) => {
  await forceBuildSkew(page, "9.9.9-" + "long-version-".repeat(4));
  await page.goto("/");
  const version = page.locator(".app-version");
  await expect(version).toContainText("long-version");
  const gear = page.getByRole("button", { name: "settings", exact: true });
  for (const macos of [false, true]) {
    await page.locator(".app-shell").evaluate((el, active) => el.classList.toggle("macos-window", active), macos);
    await page.locator(".window-root").evaluate((el, active) => el.classList.toggle("macos-root", active), macos);
    for (const width of [900, 400]) {
      await page.setViewportSize({ width, height: 600 });
      const bar = (await page.locator(".app-bar").boundingBox())!;
      // The footer is a separate fixed sibling of the scrolling avatars.
      // Settings must stay inside that footer even when host overflow scrolls.
      const rail = (await page.locator(".host-rail-footer").boundingBox())!;
      const button = (await gear.boundingBox())!;
      expect(button.width).toBeGreaterThan(0);
      expect(button.x).toBeGreaterThanOrEqual(rail.x);
      expect(button.x + button.width).toBeLessThanOrEqual(rail.x + rail.width);
      // One 22px button row plus padding/border fits below 40px; macOS
      // reserves its existing 48px band for the native titlebar controls.
      expect(bar.height).toBeLessThanOrEqual(macos ? 48 : 40);
      expect(button.y).toBeGreaterThanOrEqual(bar.y + bar.height);
      expect(button.y + button.height).toBeLessThanOrEqual(rail.y + rail.height);
      expect(await gear.evaluate((el) => Boolean(el.closest(".window-drag-region")))).toBe(false);
      await gear.click();
      const dialog = page.getByRole("dialog", { name: "settings", exact: true });
      await expect(dialog).toBeVisible();
      if (macos && width === 400) {
        // Retain the narrow modal for visual inspection alongside geometry evidence.
        await page.screenshot({ path: testInfo.outputPath("settings-narrow.png") });
      }
      await dialog.getByRole("button", { name: "Close", exact: true }).click();
      await expect(gear).toBeFocused();
    }
  }
});
