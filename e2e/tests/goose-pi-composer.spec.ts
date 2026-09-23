import { expect, test } from "./helpers/evidence";
import { patchPreferences } from "./helpers/fleet";

/**
 * Pi's sole mode is a normalized draft value, not a display-only label. This
 * drives the real signal handlers because a pure reconciliation test cannot
 * catch the original failure where handlers discarded the returned
 * permission while applying the returned model and effort.
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
  await expect(permissions.getByRole("button")).toHaveCount(1);
  await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(permissions.getByRole("button", { name: "default", exact: true })).toHaveCount(0);
  await expect(form.locator(".launch-composer-summary")).toContainText("permissions: yolo");
  await expect(status, "normalizing a passive remembered mode must stay silent").toHaveCount(0);

  await harnesses.getByRole("button", { name: "Goose", exact: true }).click();
  await permissions.getByRole("button", { name: "smart approve", exact: true }).click();
  await harnesses.getByRole("button", { name: "Pi", exact: true }).click();
  await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(status).toHaveText(
    "the selected permission is unavailable because Pi supports only YOLO, so it was replaced",
  );

  // Search activation uses a separate handler from the harness buttons. Its
  // reconciled permission must land in the same signal.
  await harnesses.getByRole("button", { name: "Goose", exact: true }).click();
  await permissions.getByRole("button", { name: "chat", exact: true }).click();
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("Pi");
  await form.getByRole("option", { name: "Harness: Pi", exact: true }).click();
  await expect(permissions.getByRole("button")).toHaveCount(1);
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
  await expect(permissions.getByRole("button")).toHaveCount(1);
  await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  // Shared model IDs must use Pi's effort vocabulary even without a harness
  // transition. Goose appears first in the catalog and lacks these levels.
  const efforts = form.locator(".launch-composer-effort-choice");
  for (const level of ["minimal", "xhigh"]) {
    await efforts.getByRole("button", { name: level, exact: true }).click();
    await model.focus();
    await form.getByRole("option", { name: "z-ai/glm-5.3-flash (Pi)", exact: true }).click();
    await expect(efforts.getByRole("button", { name: level, exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(status).toHaveCount(0);
  }
} finally {
    await patchPreferences(request, { remembered_permissions: null });
  }
});

/**
 * Workspace trust is a separate draft choice from Pi's YOLO tool mode. Search
 * and buttons must agree across Pi and Codex. Changing to Claude must clear
 * the choice before a later launch can submit it. Muse must also explain that
 * false cannot reverse the trust implied by YOLO.
 */
test("workspace trust stays explicit and scoped to supported harnesses", async ({ page, request }) => {
  await patchPreferences(request, { remembered_workspace_trust: false });
  try {
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    const harnesses = form.locator(".launch-composer-harness-choice");
    const trust = form.locator(".launch-composer-trust-choice");
    const permissions = form.locator(".launch-composer-permissions-choice");

    await harnesses.getByRole("button", { name: "Pi", exact: true }).click();
    await expect(trust.getByRole("button", { name: "false", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute("aria-pressed", "true");

    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill("trust:true");
    await form.getByRole("option", { name: "Trust workspace: true", exact: true }).click();
    await expect(search).toHaveValue("");
    await expect(trust.getByRole("button", { name: "true", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(permissions.getByRole("button", { name: "yolo", exact: true })).toHaveAttribute("aria-pressed", "true");

    await harnesses.getByRole("button", { name: "Codex", exact: true }).click();
    await expect(trust.getByRole("button", { name: "true", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(trust.locator(".launch-composer-choice-help")).toContainText("false runs it as untrusted");
    await search.fill("trust:false");
    await form.getByRole("option", { name: "Trust workspace: false", exact: true }).click();
    await expect(trust.getByRole("button", { name: "false", exact: true })).toHaveAttribute("aria-pressed", "true");

    await harnesses.getByRole("button", { name: "Claude", exact: true }).click();
    await expect(trust).toHaveCount(0);
    await search.fill("trust:false");
    await expect(form.getByRole("option", { name: "Trust workspace: false", exact: true })).toHaveCount(0);

    await harnesses.getByRole("button", { name: "Muse", exact: true }).click();
    await expect(trust.getByRole("button", { name: "default", exact: true })).toHaveAttribute("aria-pressed", "true");
    await trust.getByRole("button", { name: "false", exact: true }).click();
    await expect(trust.getByRole("button", { name: "false", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(trust.locator(".launch-composer-choice-help")).toContainText(
      "YOLO or vendor settings may still trust this workspace",
    );
  } finally {
    await patchPreferences(request, { remembered_workspace_trust: null });
  }
});
