import { expect, test } from "./helpers/evidence";

/**
 * OMP's composer is Pi's neighbor, not Pi's twin: the model is required, the
 * effort list is OMP's closed vocabulary, and the three permission choices
 * include a REAL harness default — an omitted OMP permission must survive as
 * absent, never be displayed as Pi's rewritten yolo. The composer half pins
 * the serialized POST selection; argv compilation belongs to the helm tests
 * and stored-selection decoding to the supervisor test, so a composer that
 * offered the right controls but serialized the wrong selection is what
 * fails here. The row half pins the rendered glyph and the effective
 * permission against independently fabricated row data.
 */
test("the OMP composer offers OMP's own vocabulary and the row shows the effective permission", async ({ page, request }) => {
  const catalogResponse = await request.get("/api/launch-catalog");
  expect(catalogResponse.ok()).toBe(true);
  const build = catalogResponse.headers()["x-farhelm-build"];
  expect(build).toBeTruthy();

  await page.route("**/api/launch-history**", (route) =>
    route.fulfill({
      status: 200,
      headers: { "content-type": "application/json", "x-farhelm-build": build },
      body: JSON.stringify({ launches: [], folders: [] }),
    }),
  );
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  const harnesses = form.locator(".launch-composer-harness-choice");
  const permissions = form.locator(".launch-composer-permissions-choice");
  const efforts = form.locator(".launch-composer-effort-choice");
  const status = form.getByRole("status");

  // Harness selection: the OMP chip exists, is searchable by its word, and
  // switching to it from another harness keeps the composer structured.
  await harnesses.getByRole("button", { name: "Codex", exact: true }).click();
  const search = form.locator('.launch-composer-search input[role="combobox"]');
  await search.fill("omp");
  await form.getByRole("option", { name: "Harness: Omp", exact: true }).click();
  await expect(harnesses.getByRole("button", { name: "OMP", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  // The model-required rule: no harness default, the placeholder names the
  // rule, the refusal names OMP, and the submit stays disabled.
  const model = form.getByRole("combobox", { name: "model", exact: true });
  await expect(model).toHaveAttribute("placeholder", "model required");
  await expect(model).toHaveValue("");
  await expect(form.getByText("choose an OMP model before launching", { exact: true })).toBeVisible();
  await expect(form.locator(".create-session-submit")).toBeDisabled();
  await model.focus();
  await expect(
    form
      .locator("#launch-composer-model-results")
      .getByRole("option", { name: "harness default", exact: true }),
  ).toHaveCount(0);

  // OMP's effort vocabulary is the closed seven-level list: OMP's `auto`
  // mode and the shared enum's `ultra` are deliberately absent.
  for (const level of ["off", "minimal", "low", "medium", "high", "xhigh", "max"]) {
    await expect(efforts.getByRole("button", { name: level, exact: true })).toBeVisible();
  }
  await expect(efforts.getByRole("button", { name: "ultra", exact: true })).toHaveCount(0);

  // The three permission choices: default, yolo, approve — the harness
  // default is a real choice, and the Goose-only labels are not offered.
  await expect(permissions.getByRole("button")).toHaveCount(3);
  for (const choice of ["default", "yolo", "approve"]) {
    await expect(permissions.getByRole("button", { name: choice, exact: true })).toBeVisible();
  }
  await expect(
    permissions.getByRole("button", { name: "smart approve", exact: true }),
  ).toHaveCount(0);

  // A suggested model with the maximal explicit choices serializes OMP's
  // documented selection: provider-qualified model, OMP effort, and the
  // explicit approval mode. (The helm turns this selection into argv; this
  // spec only pins what the composer sends.) The option carries no harness
  // suffix because the OMP selection already filters the list to OMP-owned
  // rows.
  await model.fill("x-ai/grok-4.6");
  await form.getByRole("option", { name: "x-ai/grok-4.6", exact: true }).click();
  await expect(model).toHaveValue("x-ai/grok-4.6");
  await efforts.getByRole("button", { name: "max", exact: true }).click();
  await permissions.getByRole("button", { name: "approve", exact: true }).click();
  await expect(status).toHaveCount(0);
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  await expect(form.locator(".create-session-submit")).toBeEnabled();
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await route.fulfill({ status: 400, headers: { "x-farhelm-build": build }, body: "fixture captured launch" });
  });
  const submitted = page.waitForRequest(
    (request) => new URL(request.url()).pathname === "/api/sessions" && request.method() === "POST",
  );
  await form.locator(".create-session-submit").click();
  expect((await submitted).postDataJSON().launch).toEqual({
    harness: "omp",
    model: "x-ai/grok-4.6",
    effort: "max",
    permissions: "approve",
  });
  await expect(form).toContainText("fixture captured launch");
});

test("an OMP session row shows the OMP glyph and its actual permission", async ({ page, request }) => {
  const catalogResponse = await request.get("/api/launch-catalog");
  const build = catalogResponse.headers()["x-farhelm-build"];
  const activity = Math.floor(Date.now() / 1000);
  await page.route("**/api/sessions**", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    await route.fulfill({
      headers: { "x-farhelm-build": build, "content-type": "application/json" },
      json: {
        sessions: [
          {
            id: "omp-approve",
            title: "omp approve row",
            cwd: "/srv/omp",
            invocation: "omp --provider openrouter --model x-ai/grok-4.6 --approval-mode always-ask",
            launch: { harness: "omp", model: "x-ai/grok-4.6", effort: null, permissions: "approve" },
            host: 999_993,
            host_name: "remote-omp-host",
            status: { state: "running" },
            last_activity_at: activity,
          },
          {
            id: "omp-omitted",
            title: "omp default row",
            cwd: "/srv/omp",
            invocation: "omp --provider openrouter --model z-ai/glm-5.3",
            launch: { harness: "omp", model: "z-ai/glm-5.3", effort: null, permissions: null },
            host: 999_993,
            host_name: "remote-omp-host",
            status: { state: "running" },
            last_activity_at: activity - 10,
          },
        ],
      },
    });
  });
  await page.goto("/");

  // The explicit Approve row shows OMP's glyph beside the approve mark.
  const approveRow = page.locator(".session-row", { hasText: "omp approve row" });
  await expect(approveRow.locator(".session-agent svg[data-glyph='omp']")).toBeVisible();
  await expect(
    approveRow.locator(".session-agent svg.permission-glyph[data-glyph='approve']"),
  ).toBeVisible();

  // The omitted-permission row is the rule this harness exists for: the OMP
  // glyph renders, and NO permission mark is invented for the harness
  // default — unlike Pi, whose omitted snapshot is displayed as yolo.
  const omittedRow = page.locator(".session-row", { hasText: "omp default row" });
  await expect(omittedRow.locator(".session-agent svg[data-glyph='omp']")).toBeVisible();
  await expect(omittedRow.locator(".session-agent svg.permission-glyph")).toHaveCount(0);
});
