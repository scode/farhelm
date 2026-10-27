import { randomUUID } from "node:crypto";
import { type APIRequestContext, type Locator, type Page, type Response } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { drainedRoute } from "./helpers/drained-route";
import { expect, test } from "./helpers/evidence";
import { patchPreferences, readPreferences } from "./helpers/fleet";
import { requireHelmBuild } from "./helpers/helm-build";
import { routeGate } from "./helpers/route-gate";

/** These cases hold real local refresh replies so closure before publication
 * cannot masquerade as movement. The open dialog has no cross-client feed
 * refresh: the second popup's ordinary binding edit supplies its fresh read.
 */
type OwnedTemplates = {
  names: string[];
  dialog: Locator;
  cleanup: [string, () => unknown | Promise<unknown>][];
};

/** Each allocation enters cleanup before another allocation can fail. Original
 * shortcuts are restored before deleting owned templates, even on setup failure.
 */
async function withTemplates(
  page: Page,
  request: APIRequestContext,
  body: (fixture: OwnedTemplates) => Promise<void>,
) {
  const original = await readPreferences(request);
  // One prefix keeps the owned rows adjacent in the real name-sorted catalog.
  const prefix = `anchor-${randomUUID()}`;
  const names = ["a", "b", "c", "d"].map(suffix => `${prefix}-${suffix}`);
  const owned: string[] = [];
  const cleanup: OwnedTemplates["cleanup"] = [];
  let failed = false;
  try {
    for (const name of names) {
      const response = await request.put(`/api/templates/${encodeURIComponent(name)}`, {
        data: { kind: "agent", agent: "claude" },
      });
      expect(response.ok(), `store owned template: ${response.status()}`).toBe(true);
      owned.push(name);
    }
    await patchPreferences(request, { welcome_seen: true, quick_launch_template: null, template_shortcuts: {} });
    await page.setViewportSize({ width: 1100, height: 1100 });
    await page.goto("/");
    // The sidebar icon and the dialog keep distinct accessible names.
    await page.getByRole("button", { name: "Templates", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "templates", exact: true });
    await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
    for (const name of names) await expect(numberToggle(dialog, name)).toBeEnabled();
    await expect(dialog.locator(".templates-retry")).toHaveCount(0);
    await page.mouse.move(1090, 1090);
    await body({ names, dialog, cleanup });
  } catch (error) {
    failed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(failed, [
      ...cleanup,
      ["original bindings", () => patchPreferences(request, {
        welcome_seen: original.welcome_seen ?? null,
        quick_launch_template: original.quick_launch_template ?? null,
        template_shortcuts: original.template_shortcuts ?? null,
      })],
      ...owned.map((name): [string, () => Promise<void>] => ["owned template", async () => {
        const response = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
        expect(response.ok() || response.status() === 404, "remove owned template").toBe(true);
      }]),
      // Rename tests add their destination before activating Save.
      ...names.filter(name => !owned.includes(name)).map((name): [string, () => Promise<void>] => [
        "renamed template", async () => {
          const response = await request.delete(`/api/templates/${encodeURIComponent(name)}`);
          expect(response.ok() || response.status() === 404, "remove rename destination").toBe(true);
        },
      ]),
    ]);
  }
}

/** Accessible identities use the real raw owned name, independent of list index. */
function numberToggle(dialog: Locator, name: string) {
  return dialog.getByRole("button", { name: `number shortcut ${name}`, exact: true });
}

/** The popup remains inside Templates' isolation, as the production menu does. */
function numberMenu(dialog: Locator, name: string) {
  return dialog.getByRole("menu", { name: `number key for ${name}`, exact: true });
}

/** The exact name span identifies the saved card, not an option owner label. */
function templateRow(dialog: Locator, name: string) {
  return dialog.getByText(name, { exact: true }).locator("..").locator("..");
}

/** Open with independent keyboard focus; pointer input would dismiss an earlier
 * popup before the causal change. Establish actual placement as well as open state.
 */
async function openNumber(page: Page, dialog: Locator, name: string) {
  const toggle = numberToggle(dialog, name);
  await toggle.focus();
  await expect(toggle).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const menu = numberMenu(dialog, name);
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitem").first()).toBeFocused();
  await expect(templateRow(dialog, name).locator(".templates-number-flyout")).toHaveCSS("opacity", "1");
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
}

/** Retain all competing geometry causes so a passing close cannot merely be an
 * old resize/scroll close. Readiness uses the held response and visible sentinels.
 */
async function geometry(toggle: Locator) {
  return toggle.evaluate(node => {
    const rect = (element: Element) => {
      const box = element.getBoundingClientRect();
      return { x: box.x, y: box.y, width: box.width, height: box.height };
    };
    const dialog = node.closest(".templates-dialog") as HTMLElement;
    const row = node.closest("li") as HTMLElement;
    const list = dialog.querySelector(".templates-list") as HTMLElement;
    const rows = Array.from(list.children);
    const sidebar = document.querySelector(".sidebar-content") as HTMLElement;
    const shell = document.querySelector(".app-shell") as HTMLElement;
    return {
      toggle: rect(node), label: node.textContent,
      index: rows.indexOf(row), row: rect(row), list: rect(list), dialog: rect(dialog),
      rows: rows.map(element => ({ name: element.querySelector(".templates-row-name")!.textContent!, ...rect(element) })),
      scroll: [dialog.scrollLeft, dialog.scrollTop, sidebar.scrollLeft, sidebar.scrollTop, shell.scrollLeft, shell.scrollTop],
      viewport: [innerWidth, innerHeight],
    };
  });
}

/** Compare only stable container geometry; per-row movement is asserted by each
 * case and must not be hidden by a broad snapshot equality check.
 */
function expectStableContainers(before: Awaited<ReturnType<typeof geometry>>, after: Awaited<ReturnType<typeof geometry>>) {
  for (const part of ["list", "dialog"] as const) {
    for (const axis of ["x", "y", "width", "height"] as const) {
      expect(Math.abs(after[part][axis] - before[part][axis]), `${part} ${axis} stays equal`).toBeLessThanOrEqual(1);
    }
  }
  expect(after.scroll, "no dialog/sidebar/shell scrolling").toEqual(before.scroll);
  expect(after.viewport, "no viewport change").toEqual(before.viewport);
  expect(after.label, "target label did not cause its own width movement").toBe(before.label);
}

const transfers = [
  { title: "closes a number popup below a new bolt chord", from: 2, to: 0, target: 1, moves: true, own: false },
  { title: "closes a number popup centered in its newly marked card", from: 2, to: 0, target: 0, moves: true, own: true },
  { title: "keeps a number popup above a bolt transfer", from: 2, to: 1, target: 0, moves: false, own: false },
  { title: "keeps a number popup below a balanced bolt transfer", from: 2, to: 0, target: 3, moves: false, own: false },
];

for (const scenario of transfers) {
  /** A real server-only preference update becomes visible through C's ordinary
   * local binding read. Busy starts after both independently owned menus open.
   */
  test(scenario.title, async ({ page, request }) => {
    await withTemplates(page, request, async ({ names, dialog, cleanup }) => {
      const from = names[scenario.from];
      const to = names[scenario.to];
      const target = names[scenario.target];
      // Reopen the dialog to establish C's marked baseline without inventing a
      // feed subscription. All later publication goes through the actual edit.
      await dialog.locator(".templates-close").click();
      await expect(dialog).toHaveCount(0);
      await patchPreferences(request, { quick_launch_template: from });
      await page.reload();
      await page.getByRole("button", { name: "Templates", exact: true }).click();
      await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
      await expect(dialog.getByRole("button", { name: `quick launch ${from}`, exact: true })).toHaveAttribute("aria-pressed", "true");
      await expect(templateRow(dialog, from).locator(".templates-quick-chord")).toHaveCount(1);
      await page.mouse.move(1090, 1090);
      await openNumber(page, dialog, target);
      await openNumber(page, dialog, from);
      await expect(numberMenu(dialog, target)).toBeVisible();
      const before = await geometry(numberToggle(dialog, target));
      expect(before.rows.find(row => row.name === from)!.height).toBeGreaterThan(before.rows.find(row => row.name === to)!.height);

      await patchPreferences(request, { quick_launch_template: to });
      expect((await readPreferences(request)).quick_launch_template).toBe(to);
      await expect(templateRow(dialog, from).locator(".templates-quick-chord")).toHaveCount(1);
      await expect(templateRow(dialog, to).locator(".templates-quick-chord")).toHaveCount(0);

      const arrived = routeGate();
      const release = routeGate();
      const retired = routeGate();
      let held = false;
      let captured = false;
      const stop = await drainedRoute(page, url => url.pathname === "/api/preferences", async route => {
        if (route.request().method() !== "GET" || held) return route.fallback();
        held = true;
        const response = await route.fetch();
        expect(response.ok(), "ordinary fresh preferences read succeeded").toBe(true);
        requireHelmBuild(response, "template movement fresh preferences");
        expect((await response.json()).quick_launch_template).toBe(to);
        captured = true;
        arrived.release();
        await release.wait();
        await route.fulfill({ response });
        retired.release();
      });
      cleanup.push(["release held preferences", () => release.release()], ["drain preference route", stop]);
      // A listener can be removed on an earlier assertion failure without
      // leaving an unawaited waitForResponse promise to reject during teardown.
      let stored: Response | undefined;
      const observeStored = (response: Response) => {
        if (response.url().endsWith("/api/preferences") && response.request().method() === "PUT"
          && response.request().postDataJSON()?.template_shortcuts?.["1"] === from) stored = response;
      };
      page.on("response", observeStored);
      cleanup.push(["retire started binding edit", async () => {
        if (!captured) return;
        await retired.wait();
        await expect.poll(() => stored?.ok() ?? false, { message: "accepted binding edit finishes before restoring preferences" }).toBe(true);
        expect(await stored!.finished()).toBeNull();
      }], ["binding response listener", () => { page.off("response", observeStored); }]);
      await page.keyboard.press("ArrowDown");
      await expect(numberMenu(dialog, from).getByRole("menuitem").nth(1)).toBeFocused();
      await page.keyboard.press("Enter");
      await arrived.wait();
      await expect(numberMenu(dialog, from)).toHaveCount(0);
      await expect(numberToggle(dialog, target)).toBeDisabled();
      await expect(numberMenu(dialog, target), "hard premise: target survived until actual fresh GET release").toBeVisible();
      expect(await geometry(numberToggle(dialog, target)), "held reply has not changed target geometry").toEqual(before);
      release.release();
      await retired.wait();
      await expect.poll(() => stored?.ok() ?? false, { message: "number binding write completed" }).toBe(true);
      expect(await stored!.finished()).toBeNull();
      await expect.poll(async () => (await readPreferences(request)).template_shortcuts).toEqual({ "1": from });
      await expect(numberToggle(dialog, target)).toBeEnabled();
      await expect(templateRow(dialog, to).locator(".templates-quick-chord")).toHaveCount(1);
      await expect(templateRow(dialog, from).locator(".templates-quick-chord")).toHaveCount(0);
      const after = await geometry(numberToggle(dialog, target));
      expectStableContainers(before, after);
      expect(after.index, "same target identity and order").toBe(before.index);
      const gained = after.rows.find(row => row.name === to)!.height - before.rows.find(row => row.name === to)!.height;
      const lost = before.rows.find(row => row.name === from)!.height - after.rows.find(row => row.name === from)!.height;
      expect(gained, "the marked card gained a real third line").toBeGreaterThan(1);
      expect(Math.abs(gained - lost), "the list height exchange is balanced").toBeLessThanOrEqual(1);
      expect(Math.abs(after.toggle.x - before.toggle.x), "the target's horizontal label position stays equal").toBeLessThanOrEqual(1);
      if (scenario.moves) {
        expect(after.toggle.y - before.toggle.y, "the target toggle actually moved").toBeGreaterThan(1);
        if (scenario.own) {
          expect(Math.abs(after.row.y - before.row.y), "own card top stays fixed").toBeLessThanOrEqual(1);
          expect(Math.abs((after.toggle.y - before.toggle.y) - gained / 2), "own toggle stays centered").toBeLessThanOrEqual(1);
        }
        await expect(numberMenu(dialog, target)).toHaveCount(0);
      } else {
        expect(Math.abs(after.toggle.y - before.toggle.y), "the unaffected anchor stays in place").toBeLessThanOrEqual(1);
        await expect(numberMenu(dialog, target)).toBeVisible();
      }
      await expect(dialog).toBeVisible();
    });
  });
}

for (const moves of [true, false]) {
  /** Rename a different card through Save. The target's keyed popup must survive
   * the accepted action and remain mounted until the real resource refresh lands.
   */
  test(`rename ${moves ? "closes a displaced" : "keeps an unchanged"} template number popup`, async ({ page, request }) => {
    await withTemplates(page, request, async ({ names, dialog, cleanup }) => {
      const source = names[moves ? 0 : 2];
      const target = names[moves ? 1 : 0];
      const renamed = `${names[0].slice(0, -1)}z`;
      names.push(renamed);
      await templateRow(dialog, source).locator(".templates-row").click();
      await dialog.locator(".templates-name").fill(renamed);
      await expect(dialog.locator(".templates-unsaved")).toBeVisible();
      await page.mouse.move(1090, 1090);
      await openNumber(page, dialog, target);
      await dialog.locator(".templates-save").focus();
      await expect(dialog.locator(".templates-save")).toBeFocused();
      await expect(numberMenu(dialog, target), "focus transfer preserves the independent popup").toBeVisible();
      const before = await geometry(numberToggle(dialog, target));
      const arrived = routeGate();
      const release = routeGate();
      const retired = routeGate();
      let expectedOrder: string[] = [];
      // A rename re-reads the template list right before it writes, to refuse
      // a name another client took since the dialog loaded (#2658). That read
      // is not the refresh this test holds, so list reads pass through until
      // the browser has sent the rename's own write.
      const renamedPath = `/api/templates/${encodeURIComponent(renamed)}`;
      let written = false;
      page.on("request", sent => {
        if (sent.method() === "PUT" && new URL(sent.url()).pathname === renamedPath) written = true;
      });
      const stop = await drainedRoute(page, url => url.pathname === "/api/templates", async route => {
        if (route.request().method() !== "GET" || !written) return route.fallback();
        const response = await route.fetch();
        expect(response.ok(), "actual post-rename list read succeeded").toBe(true);
        requireHelmBuild(response, "template movement rename refresh");
        const body = await response.json() as { templates: { name: string }[] };
        expectedOrder = body.templates.map(template => template.name);
        expect(expectedOrder).toContain(renamed);
        expect(expectedOrder).not.toContain(source);
        expect(expectedOrder).toContain(target);
        expect(expectedOrder.length, "rename keeps list count").toBe(before.rows.length);
        arrived.release();
        await release.wait();
        await route.fulfill({ response });
        retired.release();
      });
      cleanup.push(["release held templates", () => release.release()], ["drain template route", stop]);
      await page.keyboard.press("Enter");
      await arrived.wait();
      await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "true");
      await expect(numberMenu(dialog, target), "hard premise: target survived until actual list refresh release").toBeVisible();
      const held = await geometry(numberToggle(dialog, target));
      expect(held.toggle).toEqual(before.toggle);
      release.release();
      await retired.wait();
      await expect(dialog.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
      await expect(dialog.locator(".templates-row-name")).toHaveText(expectedOrder);
      await expect(dialog.locator(".templates-unsaved")).toHaveCount(0);
      const after = await geometry(numberToggle(dialog, target));
      expectStableContainers(before, after);
      expect(after.rows.map(row => row.height).sort(), "same row height multiset").toEqual(before.rows.map(row => row.height).sort());
      if (moves) {
        expect(after.index).not.toBe(before.index);
        expect(Math.abs(after.toggle.y - before.toggle.y), "surviving target really moved").toBeGreaterThan(1);
        await expect(numberMenu(dialog, target)).toHaveCount(0);
      } else {
        expect(after.index).toBe(before.index);
        expect(Math.abs(after.toggle.y - before.toggle.y), "later rename leaves target in place").toBeLessThanOrEqual(1);
        await expect(numberMenu(dialog, target)).toBeVisible();
      }
      await expect(dialog).toBeVisible();
    });
  });
}
