/** Agent switches must make an automatic permission escalation conspicuous.
 * These cases exercise the actual launcher and CSS under both engines; the
 * catalog is controlled only to make cross-agent model choices deterministic.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator } from "@playwright/test";
import { patchPreferences, readPreferences } from "./helpers/fleet";

/** Use the visible agent buttons so reconciliation is the same as a person's. */
async function chooseAgent(form: Locator, name: string) {
  const button = form.locator(".launch-composer-harness-choice").getByRole("button", { name, exact: true });
  await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "true");
}

/** Read the browser's own lifetime animation, rather than infer a timer from
 * elapsed test time. A missing animation must fail instead of skipping proof.
 */
async function lifetimeState(banner: Locator) {
  return banner.evaluate(node => {
    const animation = node.getAnimations().find(animation => (animation as CSSAnimation).animationName === "permission-banner-lifetime");
    if (!animation) throw new Error("permission banner has no lifetime animation");
    return { state: animation.playState, duration: animation.effect!.getTiming().duration };
  });
}

/** Advance only the notice's CSS clock near expiry. Readiness remains the
 * real animationend event and the resulting removal of the actual notice.
 */
async function nearExpiry(banner: Locator) {
  await banner.evaluate(node => {
    const animation = node.getAnimations().find(animation => (animation as CSSAnimation).animationName === "permission-banner-lifetime");
    if (!animation) throw new Error("permission banner has no lifetime animation");
    animation.currentTime = 7900;
  });
}

test.beforeEach(async ({ page }) => {
  const build = (await page.request.get("/api/sessions")).headers()["x-farhelm-build"];
  expect(build, "catalog fixtures must retain the running helm's build stamp").toBeTruthy();
  await page.route("**/api/launch-catalog", route => route.fulfill({
    status: 200,
    headers: { "content-type": "application/json", "x-farhelm-build": build! },
    json: [
      { id: "claude-warning-model", harness: "claude", efforts: ["high"] },
      { id: "pi-warning-model", harness: "pi", efforts: ["high"] },
    ],
  }));
});

/** The original defect is Guarded becoming YOLO despite OMP offering Guarded.
 * With memory cleared, both the initial default and that explicit switch must
 * remain Guarded. Undo of a real escalation restores the complete old draft.
 */
test("OMP defaults to Guarded and escalation Undo restores the pre-switch draft", async ({ page, request }, testInfo) => {
  const preferences = await readPreferences(request);
  try {
    await patchPreferences(request, { remembered_permissions: null });
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    const permissions = form.locator(".launch-composer-permissions-choice");
    const banner = form.locator(".permission-escalation-banner");
    await chooseAgent(form, "OMP");
    await expect(permissions.getByRole("button", { name: "guarded", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(banner).toHaveCount(0);
    await chooseAgent(form, "Claude");
    await permissions.getByRole("button", { name: "guarded", exact: true }).click();
    await chooseAgent(form, "OMP");
    await expect(permissions.getByRole("button", { name: "guarded", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(banner).toHaveCount(0);
    const name = form.locator(".launch-composer-name input");
    await name.fill("before switch");
    await form.locator(".named-conversation").fill("before-conversation");
    await chooseAgent(form, "Pi");
    await expect(banner).toHaveAttribute("role", "alert");
    await expect(banner).toContainText("Permission changed: Guarded → YOLO");
    await expect(banner).toContainText("Pi does not support Guarded");
    const screenshot = testInfo.outputPath("permission-escalation-banner.png");
    await form.screenshot({ path: screenshot });
    await testInfo.attach("permission-escalation-banner", { path: screenshot, contentType: "image/png" });
    await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute("aria-pressed", "true");
    // The plan promises the whole pre-switch draft, including later edits.
    await name.fill("after switch");
    await form.locator(".named-conversation").fill("after-conversation");
    await banner.getByRole("button", { name: "undo", exact: true }).click();
    await expect(banner).toHaveCount(0);
    await expect(form.locator(".launch-composer-harness-choice").getByRole("button", { name: "OMP", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(permissions.getByRole("button", { name: "guarded", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(name).toHaveValue("before switch");
    await expect(form.locator(".named-conversation")).toHaveValue("before-conversation");
    await chooseAgent(form, "Pi");
    await expect(banner).toBeVisible();
    await banner.getByRole("button", { name: "dismiss", exact: true }).click();
    await expect(banner).toHaveCount(0);
    await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute("aria-pressed", "true");
  } finally {
    await patchPreferences(request, { remembered_permissions: preferences.remembered_permissions });
  }
});

/** A template is one transaction. Its explicit permission overrides the
 * intermediate harness fallback, while an omitted mode is still automatic.
 * Undo must also restore the name and destination that transaction replaced.
 */
test("template permissions are compared after the complete template transaction", async ({ page, request }) => {
  const prefix = `permission-template-${Date.now()}`;
  const names = [`${prefix}-automatic`, `${prefix}-explicit`];
  try {
    for (const [index, name] of names.entries()) {
      const response = await request.put(`/api/templates/${name}`, { data: {
        kind: "agent", agent: "pi", name: "template name", destination: { folder: "/template-choice" },
        ...index === 1 ? { permissions: "yolo" } : {},
      } });
      expect(response.ok(), `store warning template: ${response.status()}`).toBe(true);
    }
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    const search = form.getByRole("combobox", { name: "search folders, harnesses, models, and efforts" });
    await expect(form).toHaveAttribute("data-initial-templates-ready", "true");
    await chooseAgent(form, "Claude");
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
    const name = form.locator(".launch-composer-name input");
    await name.fill("original name");
    const folder = form.getByRole("combobox", { name: "folder", exact: true });
    const oldFolder = await folder.inputValue();
    await search.fill(`tl:${names[0]}`);
    await search.focus();
    await expect(search).toBeFocused();
    await search.press("Enter");
    const banner = form.locator(".permission-escalation-banner");
    await expect(banner).toContainText("Pi does not support Guarded");
    await expect(name).toHaveValue("template name");
    await expect(folder).toHaveValue("/template-choice");
    await banner.getByRole("button", { name: "undo", exact: true }).click();
    await expect(name).toHaveValue("original name");
    await expect(folder).toHaveValue(oldFolder);
    await search.fill(`tl:${names[1]}`);
    await search.focus();
    await expect(search).toBeFocused();
    await search.press("Enter");
    await expect(name).toHaveValue("template name");
    await expect(form.locator(".launch-composer-harness-choice").getByRole("button", { name: "Pi", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(banner).toHaveCount(0);
  } finally {
    for (const name of names) {
      const response = await request.delete(`/api/templates/${name}`);
      expect(response.ok() || response.status() === 404).toBe(true);
    }
  }
});

/** Search and the cross-agent model picker have separate mutation paths.
 * Neither may evade the warning; an explicit permission edit creates none.
 */
for (const path of ["search", "model picker"]) {
  test(`permission escalation through ${path} warns and direct permission choices do not`, async ({ page }) => {
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await chooseAgent(form, "Claude");
    const permissions = form.locator(".launch-composer-permissions-choice");
    await permissions.getByRole("button", { name: "guarded", exact: true }).click();
    const banner = form.locator(".permission-escalation-banner");
    await expect(banner).toHaveCount(0);
    if (path === "search") {
      const search = form.getByRole("combobox", { name: "search folders, harnesses, models, and efforts" });
      await search.fill("harness:pi");
      await search.focus();
      await expect(search).toBeFocused();
      await search.press("Enter");
    } else {
      const model = form.getByRole("combobox", { name: "model", exact: true });
      await model.focus();
      await form.getByRole("option", { name: "show every harness's models", exact: true }).click();
      await form.getByRole("option", { name: "pi-warning-model (Pi)", exact: true }).click();
    }
    await expect(banner).toContainText("Pi does not support Guarded");
    await banner.getByRole("button", { name: "undo", exact: true }).click();
    await expect(banner).toHaveCount(0);
    await permissions.getByRole("button", { name: "yolo", exact: true }).click();
    await expect(banner).toHaveCount(0);
  });
}

/** Hover and keyboard focus pause the real CSS lifetime. Reduced motion
 * removes visible animation without removing expiry or its static outline.
 */
test("permission banner lifetime pauses for hover and focus and survives reduced-motion styling", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  const search = form.getByRole("combobox", { name: "search folders, harnesses, models, and efforts" });
  await chooseAgent(form, "Claude");
  await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true }).click();
  await chooseAgent(form, "Pi");
  const banner = form.locator(".permission-escalation-banner");
  await banner.hover();
  await expect.poll(() => lifetimeState(banner)).toEqual({ state: "paused", duration: 8000 });
  await expect(banner).toHaveCSS("animation-name", "permission-banner-lifetime");
  await expect(form.locator(".launch-composer-permissions-choice")).toHaveCSS("outline-width", "4px");
  // The outline width holds with or without reduced motion; only the absent
  // fade proves the reduced-motion override applied.
  await expect(form.locator(".launch-composer-permissions-choice")).toHaveCSS("animation-name", "none");
  await nearExpiry(banner);
  await search.hover();
  await expect(banner).toHaveCount(0);
  await chooseAgent(form, "Claude");
  await chooseAgent(form, "Pi");
  await banner.getByRole("button", { name: "undo", exact: true }).focus();
  await expect(banner.getByRole("button", { name: "undo", exact: true })).toBeFocused();
  await expect.poll(() => lifetimeState(banner)).toEqual({ state: "paused", duration: 8000 });
  await nearExpiry(banner);
  await search.focus();
  await expect(banner).toHaveCount(0);
});
