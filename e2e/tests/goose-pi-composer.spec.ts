import { expect, test } from "./helpers/evidence";
import { patchPreferences } from "./helpers/fleet";

/**
 * Pi's only mode is a normalized draft value, not a display-only label. This
 * drives the real signal handlers because a pure reconciliation test cannot
 * catch the original failure where handlers discarded the returned
 * permission while applying the returned model and effort.
 * Goose's scoped smart-approve word must reach the same pressed mode and
 * explain it on hover; the button alone would not cover the search path.
 * Only pressed-state buttons are modes; the permission help button remains
 * available without changing Pi's single-choice contract.
 */
test("Goose and Pi permission transitions update the rendered draft", async ({ page, request }) => {
  await patchPreferences(request, { remembered_permissions: "smart_approve" });
  try {
    await page.goto("/");
    await page.locator(".new-session-button").click();

  const form = page.locator(".create-session-form");
  const harnesses = form.locator(".launch-composer-harness-choice");
  const permissions = form.locator(".launch-composer-permissions-choice");
  const status = form.getByRole("status");

  // Premise: the passive helm-wide memory reached the mounted component.
  await expect(form.locator(".launch-composer-summary")).toContainText("permissions: smart approve");
  await form.getByRole("button", { name: "reset choices", exact: true }).click();
  await expect(form.locator(".launch-composer-summary")).toContainText("permissions: smart approve");

  await harnesses.getByRole("button", { name: "Pi", exact: true }).click();
  await expect(permissions.locator("button[aria-pressed]")).toHaveText(["yolo"]);
  await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(permissions.getByRole("button", { name: "default", exact: true })).toHaveCount(0);
  await expect(form.locator(".launch-composer-summary")).toContainText("permissions: yolo");
  await expect(status, "normalizing a passive remembered mode must stay silent").toHaveCount(0);

  await harnesses.getByRole("button", { name: "Goose", exact: true }).click();
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("perms:smart approve");
  const smartApprove = form.getByRole("option", { name: "Permissions: smart approve", exact: true });
  await expect(smartApprove).toHaveAttribute("aria-selected", "true");
  await expect(smartApprove).toHaveAttribute("data-tooltip", "smart approve: the agent asks only before actions it judges risky");
  await search.press("Enter");
  await expect(permissions.getByRole("button", { name: "smart approve", exact: true })).toHaveAttribute("aria-pressed", "true");
  await harnesses.getByRole("button", { name: "Pi", exact: true }).click();
  await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(status).toHaveText(
    "the selected permission is unavailable for this harness, so it was replaced by YOLO, this harness's default",
  );

  // Search activation uses a separate handler from the harness buttons. Its
  // reconciled permission must land in the same signal.
  await harnesses.getByRole("button", { name: "Goose", exact: true }).click();
  await permissions.getByRole("button", { name: "chat", exact: true }).click();
  await search.fill("Pi");
  await form.getByRole("option", { name: "Harness: Pi", exact: true }).click();
  await expect(permissions.locator("button[aria-pressed]")).toHaveText(["yolo"]);
  await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  // A model row can also switch harness ownership. The shared OpenRouter id
  // must keep the Pi owner carried by that row and apply Pi's permission
  // normalization through the model callback.
  await harnesses.getByRole("button", { name: "Goose", exact: true }).click();
  await permissions.getByRole("button", { name: "approve", exact: true }).click();
  const model = form.getByRole("combobox", { name: "model", exact: true });
  await model.focus();
  await form.getByRole("option", { name: "show every harness's models", exact: true }).click();
  await form.getByRole("option", { name: "x-ai/grok-4.6 (Pi)", exact: true }).click();
  await expect(harnesses.getByRole("button", { name: "Pi", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(permissions.locator("button[aria-pressed]")).toHaveText(["yolo"]);
  await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  // Shared model IDs must use Pi's effort vocabulary even without a harness
  // transition. Goose appears first in the catalog and lacks these levels.
  // Pi's longer vocabulary is a native dropdown; its committed value must
  // survive the same-model-owner edit just as a selected segment would.
  const effort = form.getByRole("combobox", { name: "effort", exact: true });
  await expect(effort).toBeVisible();
  for (const level of ["minimal", "xhigh"]) {
    // selectOption changes the value without moving focus. A real dropdown
    // edit leaves the model input first, so focusing it again reopens its list.
    await effort.focus();
    await expect(effort).toBeFocused();
    await effort.selectOption(level);
    await expect(effort).toHaveValue(level);
    await model.focus();
    await form.getByRole("option", { name: "z-ai/glm-5.3-flash (Pi)", exact: true }).click();
    await expect(effort).toHaveValue(level);
    await expect(status).toHaveCount(0);
  }
} finally {
    await patchPreferences(request, { remembered_permissions: null });
  }
});

/** Historical trust memory must not restore a control, a keyboard action or
 * an ordinary-folder launch override. Controlled refusals expose the submitted
 * requests without starting agents; permission modes still follow the harness. */
for (const rememberedTrust of [true, false]) {
  test(`workspace trust is absent despite old remembered value ${rememberedTrust}`, async ({ page, request }) => {
    const build = (await request.get("/api/sessions")).headers()["x-farhelm-build"];
    expect(build, "controlled refusals retain the live helm identity").toBeTruthy();
    let seeded = false;
    await page.route("**/api/preferences", async route => {
      if (route.request().method() !== "GET") return route.fallback();
      const response = await route.fetch();
      const json = await response.json();
      json.remembered_workspace_trust = rememberedTrust;
      await route.fulfill({ response, json });
      seeded = true;
    });
    const bodies: { launch: Record<string, unknown> }[] = [];
    await page.route("**/api/sessions", async route => {
      if (route.request().method() !== "POST") return route.fallback();
      bodies.push(route.request().postDataJSON());
      await route.fulfill({ status: 400, headers: { "x-farhelm-build": build }, body: `trust fixture refused create ${bodies.length}` });
    });
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await expect.poll(() => seeded, { message: "the page received the historical preference" }).toBe(true);
    await form.getByLabel("folder", { exact: true }).fill("/tmp");
    const harnesses = form.locator(".launch-composer-harness-choice");
    const permissions = form.locator(".launch-composer-permissions-choice");
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    for (const [label, harness] of [["Pi", "pi"], ["Codex", "codex"], ["Claude", "claude"], ["Muse", "muse"]]) {
      await harnesses.getByRole("button", { name: label, exact: true }).click();
      await expect(harnesses.getByRole("button", { name: label, exact: true })).toHaveAttribute("aria-pressed", "true");
      await expect(form.locator(".launch-composer-trust-choice")).toHaveCount(0);
      if (harness === "pi") await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute("aria-pressed", "true");
      else await permissions.getByRole("button", { name: "guarded", exact: true }).click();
      for (const query of ["trust:", "trust:true", "trust:false"]) {
        await search.fill(query);
        await expect(form.getByRole("option", { name: /^Trust workspace:/ })).toHaveCount(0);
      }
      await search.fill("");
      const count = bodies.length + 1;
      await form.locator(".create-session-submit").click();
      await expect(form.locator(".launch-composer-refusal")).toContainText(`trust fixture refused create ${count}`);
      expect(bodies).toHaveLength(count);
      expect(bodies[count - 1].launch).toMatchObject({ harness });
      expect(bodies[count - 1].launch).not.toHaveProperty("workspace_trust");
    }
  });
}
