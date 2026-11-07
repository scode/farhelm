/** Tensorlake Add host through the real probe/provision routes. The API and
 * host execution are fakes; registration, one-use confirmation, steps, recipe
 * storage and sandbox deletion are the shipped implementations. Presentation
 * captures intercept planning and refuse provisioning so inspecting a dialog
 * never creates a host that could dial the real Tensorlake gateway. */
import { expect, test } from "./helpers/evidence";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { currentStackSlot } from "../stack-slots";
import {
  hostRowByName,
  openHostMenu,
  openHostsPanel,
  patchPreferences,
} from "./helpers/fleet";
import type { APIRequestContext, Locator } from "@playwright/test";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { drainedRoute } from "./helpers/drained-route";
import { requireHelmBuild } from "./helpers/helm-build";

/** Atomic controls keep a running step from ever observing half a file. */
async function controls(
  kind: "tensorlake_fake" | "provisioning_backend",
  config: unknown,
) {
  const info = JSON.parse(
    await readFile(currentStackSlot().stackInfoPath, "utf8"),
  );
  const root = info[kind];
  const next = path.join(root, `config.${process.pid}.next`);
  await writeFile(next, JSON.stringify(config));
  await rename(next, path.join(root, "config.json"));
  return root as string;
}

/** The registry is the readiness oracle, rather than the add dialog closing. */
async function hosts(request: APIRequestContext): Promise<any[]> {
  const response = await request.get("/api/hosts");
  expect(response.ok(), await response.text()).toBe(true);
  return (await response.json()).hosts;
}

/** Visibility alone includes offscreen controls. Before any click can scroll
 * an answer into view, require its whole box and center hit target inside the
 * visible dialog. This also applies after the short-card outer scroll. */
async function expectInlineAnswerVisible(answer: Locator) {
  await expect.poll(() =>
    answer.evaluate((element) => {
      const box = element.getBoundingClientRect();
      const dialog = element.closest('[role="dialog"]')!
        .getBoundingClientRect();
      const hit = document.elementFromPoint(
        box.x + box.width / 2,
        box.y + box.height / 2,
      );
      return box.width > 0 && box.height > 0 &&
        box.top >= Math.max(0, dialog.top) &&
        box.bottom <= Math.min(innerHeight, dialog.bottom) && box.left >= 0 &&
        box.right <= innerWidth &&
        Boolean(hit && element.contains(hit));
    }), {
    message: "recipe answer must be reachable before a click auto-scrolls it",
  }).toBe(true);
}

/** A top-of-form Save can fail while the final fields are offscreen. Its
 * reason must fit in the visible footer with the answers, rather than merely
 * exist in the DOM at the bottom of the scrolling fields. */
async function expectInlineRecipeErrorVisible(error: Locator) {
  await expect.poll(() =>
    error.evaluate((element) => {
      const box = element.getBoundingClientRect();
      const footer = element.closest(".recipe-editor-footer")!
        .getBoundingClientRect();
      const hit = document.elementFromPoint(
        box.x + box.width / 2,
        box.y + box.height / 2,
      );
      const inside = box.width > 0 && box.height > 0 &&
        box.top >= Math.max(footer.top, 0) &&
        box.bottom <= Math.min(footer.bottom, innerHeight) &&
        box.left >= Math.max(footer.left, 0) &&
        box.right <= Math.min(footer.right, innerWidth) &&
        Boolean(hit && element.contains(hit));
      return inside ? "visible" : JSON.stringify({
        box: box.toJSON(),
        footer: footer.toJSON(),
        hit: hit?.tagName,
      });
    }), { message: "recipe refusal must be visible beside the pinned answers" })
    .toBe("visible");
}

/** The inline editor must give up only its body height: Save and the departure
 * answers stay in Add host's real footer, in both palettes. Real overflow,
 * unchanged footer geometry and native Tab distinguish this from a button
 * that Playwright only makes reachable by auto-scrolling before a click.
 * Saving a recipe and cancelling Add host exercises no sandbox or gateway. */
for (const palette of ["dark", "light"] as const) {
  test(
    `Add host pins inline recipe answers and focuses Name in ${palette}`,
    async ({ page, request }, info) => {
      const recipe = `e2e-inline-frame-${palette}`;
      const original = await (await request.get("/api/tensorlake")).json();
      expect(original.api_key_set, "fixture owns a newly saved API key").toBe(
        false,
      );
      const listed = await request.get("/api/tensorlake/recipes");
      expect(listed.ok(), await listed.text()).toBe(true);
      expect(
        (await listed.json()).recipes.some((row: any) => row.name === recipe),
        "fixture owns a fresh recipe name",
      ).toBe(false);
      const beforeHosts = (await hosts(request)).map((row) => row.id).sort();
      let submissions = 0;
      page.on("request", (request) => {
        if (
          request.method() === "POST" &&
          new URL(request.url()).pathname === "/api/hosts/provision"
        ) submissions++;
      });
      const configured = await request.put("/api/tensorlake", {
        data: { api_key: "test-key" },
      });
      expect(configured.ok(), await configured.text()).toBe(true);
      try {
        await page.emulateMedia({ colorScheme: palette });
        await page.setViewportSize({ width: 1280, height: 720 });
        await page.goto("/");
        await expect(page.locator("html")).toHaveAttribute(
          "data-appearance",
          palette,
        );
        await openHostsPanel(page);
        await page.getByRole("button", { name: "Add host", exact: true }).last()
          .click();
        const dialog = page.getByRole("dialog", {
          name: "Add host",
          exact: true,
        });
        await dialog.getByRole("button", {
          name: "Tensorlake sandbox",
          exact: true,
        }).click();
        await dialog.getByRole("combobox", { name: "Recipe", exact: true })
          .selectOption("__new__");
        const editor = dialog.getByRole("region", {
          name: "New recipe",
          exact: true,
        });
        const name = editor.getByRole("textbox", { name: "Name", exact: true });
        const body = dialog.locator(":scope > .host-settings-body");
        const footer = dialog.locator(":scope > .recipe-editor-footer");
        const save = footer.getByRole("button", { name: "Save", exact: true });
        const cancel = footer.getByRole("button", {
          name: "Cancel",
          exact: true,
        });
        await expect(body).toHaveCount(1);
        await expect(footer).toHaveCount(1);
        await expect(name).toBeFocused();
        await expect(
          editor.getByRole("heading", {
            name: "New recipe",
            level: 3,
            exact: true,
          }),
        ).toBeVisible();
        await expect(
          dialog.getByRole("button", { name: "‹ Recipes", exact: true }),
        ).toHaveCount(0);
        await page.evaluate(() => document.fonts.ready);
        await expect(editor.locator(".recipe-editor-title")).toHaveCSS(
          "font-size",
          "18px",
        );
        await expect(editor.locator("label.checkout-root-label > span").first())
          .toHaveCSS("font-size", "13px");
        await expect(
          editor.getByRole("spinbutton", { name: /^CPUs/ }).locator("..")
            .locator("span").first(),
        ).toHaveCSS("font-size", "13px");
        // Agents start in None mode, so the nested label sampled here is the
        // GitHub token picker's, which every recipe shows.
        await expect(
          editor.getByLabel("GitHub token", { exact: true }).locator("..")
            .locator("span").first(),
        ).toHaveCSS("font-size", "13px");
        const captions = editor.locator("div.checkout-root-label > span");
        await expect(captions).toHaveCount(5);
        for (const caption of await captions.all()) {
          await expect(caption).toHaveCSS("font-size", "14px");
        }
        expect(
          await body.evaluate((element) =>
            element.scrollHeight - element.clientHeight
          ),
          "premise: the recipe fields really overflow",
        ).toBeGreaterThan(100);
        expect(
          await body.evaluate((element) => element.scrollTop),
          "premise: initial Save visibility was not repaired by scrolling",
        ).toBe(0);
        await expectInlineAnswerVisible(save);
        await expectInlineAnswerVisible(cancel);
        const footerBox = (await footer.boundingBox())!;
        const nameBox = (await name.boundingBox())!;
        await info.attach(`inline-recipe-top-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });

        await name.fill("Bad Name");
        await body.evaluate((element) => {
          element.scrollTop = 0;
        });
        expect(await body.evaluate((element) => element.scrollTop)).toBe(0);
        await expectInlineAnswerVisible(save);
        await save.click();
        const error = footer.getByRole("alert");
        await expect(error).toContainText(
          "lowercase letters, digits and hyphens",
        );
        await expectInlineRecipeErrorVisible(error);
        await expectInlineAnswerVisible(save);
        await info.attach(`inline-recipe-invalid-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });
        await cancel.click();
        const leaveQuestion = footer.getByText(
          "Save these changes before leaving?",
          { exact: true },
        );
        await expect(leaveQuestion).toBeVisible();
        await save.click();
        await expect(error).toContainText(
          "lowercase letters, digits and hyphens",
        );
        await expect(leaveQuestion).toBeVisible();
        await expectInlineRecipeErrorVisible(error);
        for (const answer of await footer.getByRole("button").all()) {
          await expectInlineAnswerVisible(answer);
        }
        await info.attach(`inline-recipe-leave-invalid-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });
        await name.fill(recipe);
        await expect(error).toHaveCount(0);
        await expect(leaveQuestion).toBeVisible();
        await footer.getByRole("button", { name: "Keep editing", exact: true })
          .click();
        await body.evaluate((element) => {
          element.scrollTop = element.scrollHeight;
        });
        await expect.poll(() => body.evaluate((element) => element.scrollTop))
          .toBeGreaterThan(100);
        expect((await name.boundingBox())!.y, "fields move within the body")
          .toBeLessThan(nameBox.y - 100);
        expect(
          await footer.boundingBox(),
          "the footer stays fixed while the body moves",
        ).toEqual(footerBox);
        await expectInlineAnswerVisible(save);
        // In None mode the editor's last control is Codex's last mode button.
        const finalField = editor.getByRole("radiogroup", {
          name: "Codex authentication mode",
          exact: true,
        }).getByRole("radio", { name: "Subscription", exact: true });
        await finalField.focus();
        await expect(finalField).toBeFocused();
        await page.keyboard.press("Tab");
        await expect(cancel).toBeFocused();
        await page.keyboard.press("Tab");
        await expect(save).toBeFocused();
        await info.attach(`inline-recipe-bottom-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });

        await cancel.click();
        await expect(
          footer.getByText("Save these changes before leaving?", {
            exact: true,
          }),
        ).toBeVisible();
        for (
          const answer of await footer.getByRole("button").all()
        ) await expectInlineAnswerVisible(answer);
        await info.attach(`inline-recipe-leave-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });
        await footer.getByRole("button", { name: "Keep editing", exact: true })
          .click();
        await expect(name).toHaveValue(recipe);

        // Chrome cannot fit in this window. The standard frame's fallback
        // scrolls the whole card, rather than crushing the field body to zero.
        await page.setViewportSize({ width: 1280, height: 180 });
        await expect.poll(() =>
          dialog.evaluate((element) =>
            element.scrollHeight - element.clientHeight
          )
        ).toBeGreaterThan(0);
        await dialog.evaluate((element) => {
          element.scrollTop = element.scrollHeight;
        });
        await expect.poll(() => dialog.evaluate((element) => element.scrollTop))
          .toBeGreaterThan(0);
        await expectInlineAnswerVisible(save);
        await page.setViewportSize({ width: 1280, height: 720 });
        await expectInlineAnswerVisible(save);
        await save.click();
        await expect(
          dialog.getByRole("combobox", { name: "Recipe", exact: true }),
        ).toHaveValue(recipe);
        await dialog.getByRole("button", { name: "Cancel", exact: true })
          .click();
        await expect(dialog).toHaveCount(0);
        expect(submissions, "editing never submitted a host creation").toBe(0);
        expect((await hosts(request)).map((row) => row.id).sort()).toEqual(
          beforeHosts,
        );
      } finally {
        const removed = await request.delete(
          `/api/tensorlake/recipes/${recipe}`,
        );
        expect([200, 404], await removed.text()).toContain(removed.status());
        const cleared = await request.put("/api/tensorlake", {
          data: { clear_api_key: true },
        });
        expect(cleared.ok(), await cleared.text()).toBe(true);
      }
    },
  );
}

/** Saved recipe facts must remain legible through the form and shared plan.
 * Captures use intercepted, build-stamped plans and a refusing provision route:
 * Back, Copy, Cancel and Escape can then prove their no-create contract without
 * registering a sandbox host or making a gateway connection for a screenshot. */
for (const palette of ["dark", "light"] as const) {
  test(
    `Tensorlake Add host structures its form and creation plan in ${palette}`,
    async ({ page, request }, info) => {
      const recipe = `e2e-plan-recipe-${palette}`;
      const hostName = `e2e-plan-${palette}`;
      const accountResponse = await request.get("/api/tensorlake");
      expect(accountResponse.ok(), await accountResponse.text()).toBe(true);
      const original = await accountResponse.json();
      expect(
        original.api_key_set,
        "fixture must not replace a key it cannot restore",
      ).toBe(false);
      const listedResponse = await request.get("/api/tensorlake/recipes");
      expect(listedResponse.ok(), await listedResponse.text()).toBe(true);
      expect(
        (await listedResponse.json()).recipes.some((row: any) =>
          row.name === recipe
        ),
        "fixture owns its recipe name",
      ).toBe(false);
      const fleetResponse = await request.get("/api/hosts");
      expect(fleetResponse.ok(), await fleetResponse.text()).toBe(true);
      const build = requireHelmBuild(
        fleetResponse,
        "Tensorlake creation-plan captures",
      );
      const beforeHosts = (await fleetResponse.json()).hosts.map((row: any) =>
        row.id
      ).sort();
      const definition = {
        size: { cpus: 4, memory_gb: 16, disk_gb: 50 },
        source: { kind: "packages", packages: ["git", "ripgrep"] },
        claude: null,
        codex: null,
        tunnels: [{ sandbox_port: 8080, host: "127.0.0.1", port: 9090 }],
      };
      const probes: { name: string; recipe: string }[] = [];
      const plans: { confirmation: string; names: string[] }[] = [];
      let submissions = 0;
      let stopProbe = async () => {};
      let stopProvision = async () => {};
      let bodyFailed = false;
      try {
        const configured = await request.put("/api/tensorlake", {
          data: { api_key: "test-key", create_without_asking: false },
        });
        expect(configured.ok(), await configured.text()).toBe(true);
        const created = await request.post("/api/tensorlake/recipes", {
          data: { name: recipe, recipe: definition },
        });
        expect(created.ok(), await created.text()).toBe(true);
        const stored = await request.get("/api/tensorlake/recipes");
        expect(stored.ok(), await stored.text()).toBe(true);
        expect(
          (await stored.json()).recipes.find((row: any) => row.name === recipe),
          "summary measurements use the actual saved recipe",
        ).toMatchObject({ name: recipe, ...definition });

        stopProvision = await drainedRoute(
          page,
          "**/api/hosts/provision",
          async (route) => {
            expect(route.request().method()).toBe("POST");
            submissions++;
            await route.fulfill({
              status: 409,
              headers: { "x-farhelm-build": build },
              body: "capture plans authorize no host creation",
            });
          },
        );
        stopProbe = await drainedRoute(
          page,
          "**/api/hosts/probe",
          async (route) => {
            expect(route.request().method()).toBe("POST");
            const target = route.request().postDataJSON().target;
            expect(target).toMatchObject({ kind: "tensorlake", recipe });
            probes.push({ name: target.name, recipe: target.recipe });
            const round = probes.length;
            const sandbox = `farhelm-host-test-${palette}-${round}`;
            const names = [
              `Build the snapshot for recipe ${recipe} (packages, Claude Code and Codex; takes a few minutes)`,
              `Create sandbox ${sandbox} with 4 CPUs, 16 GB memory and 50 GB disk`,
              "Start the sandbox's login manager and keep the user manager running",
              "Install Farhelm's supervisor and open the recipe's tunnels when the host connects",
            ];
            const confirmation =
              `Create Tensorlake sandbox ${sandbox}\nSize: 4 CPUs,  16 GB memory, 50 GB disk\n${
                names.join("\n")
              }\n`;
            plans.push({ confirmation, names });
            await route.fulfill({
              headers: { "x-farhelm-build": build },
              json: {
                result: "tensorlake-provisionable",
                probe_id: `capture-${palette}-${round}`,
                sandbox,
                destination: "capture@fixture.invalid",
                recipe,
                resources: definition.size,
                confirmation,
                steps: {
                  lead: "Create a Tensorlake sandbox and set it up as a host",
                  kept: null,
                  steps: names.map((name, index) => ({
                    id: `step-${index}`,
                    name,
                    paths: [],
                    explanation: null,
                  })),
                },
              },
            });
          },
        );
        await page.emulateMedia({ colorScheme: palette });
        await page.setViewportSize({ width: 1280, height: 800 });
        await page.goto("/");
        await expect(page.locator("html")).toHaveAttribute(
          "data-appearance",
          palette,
        );
        await openHostsPanel(page);
        const dialog = page.getByRole("dialog", {
          name: "Add host",
          exact: true,
        });
        /** Reopen through the real kind selector; each dismissal must release
         * the previous offer rather than leaving hidden confirmation authority. */
        const openForm = async () => {
          await page.getByRole("button", { name: "Add host", exact: true })
            .last().click();
          await dialog.getByRole("button", {
            name: "Tensorlake sandbox",
            exact: true,
          }).click();
          await expect(
            dialog.getByRole("combobox", { name: "Recipe", exact: true }),
          ).toBeEnabled();
        };
        await openForm();
        const name = dialog.getByRole("textbox", { name: "Name", exact: true });
        const select = dialog.getByRole("combobox", {
          name: "Recipe",
          exact: true,
        });
        await expect(name).toHaveAccessibleDescription(
          "Shown in the rail and on session rows. The sandbox is named farhelm-host-…",
        );
        await expect(select).toHaveAccessibleDescription(
          "What the sandbox runs. New recipe… opens the editor here.",
        );
        await expect(dialog.locator(".add-tensorlake-recipe-summary"))
          .toHaveCount(0);
        await name.fill(hostName);
        await select.selectOption(recipe);
        const summary = dialog.locator(".add-tensorlake-recipe-summary");
        await expect(summary).toHaveText(
          "4 CPU · 16 GB · 50 GB · 2 packages · 1 tunnel",
        );
        await page.evaluate(() => document.fonts.ready);
        const nameStyle = await name.evaluate((element) => {
          const style = getComputedStyle(element);
          return {
            height: element.getBoundingClientRect().height,
            border: style.border,
            background: style.backgroundColor,
            fontFamily: style.fontFamily,
            fontSize: style.fontSize,
          };
        });
        const selectStyle = await select.evaluate((element) => {
          const style = getComputedStyle(element);
          return {
            height: element.getBoundingClientRect().height,
            border: style.border,
            background: style.backgroundColor,
            fontFamily: style.fontFamily,
            fontSize: style.fontSize,
          };
        });
        expect(
          selectStyle,
          "Recipe is styled as the adjacent native Name input",
        ).toEqual(nameStyle);
        const nameBox = (await name.boundingBox())!;
        const selectBox = (await select.boundingBox())!;
        const summaryBox = (await summary.boundingBox())!;
        expect(Math.abs(selectBox.x - nameBox.x)).toBeLessThan(1);
        expect(Math.abs(selectBox.width - nameBox.width)).toBeLessThan(1);
        expect(summaryBox.x).toBeGreaterThanOrEqual(selectBox.x);
        expect(summaryBox.x + summaryBox.width).toBeLessThanOrEqual(
          selectBox.x + selectBox.width,
        );
        const footer = dialog.locator(":scope > .dialog-footer");
        await expect(footer.getByRole("button")).toHaveText([
          "Cancel",
          "Continue",
        ]);
        for (const answer of await footer.getByRole("button").all()) {
          await expectInlineAnswerVisible(answer);
        }
        await info.attach(`tensorlake-add-form-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });

        await footer.getByRole("button", { name: "Continue", exact: true })
          .click();
        await expect.poll(() => probes).toEqual([{ name: hostName, recipe }]);
        const header = dialog.locator(":scope > .dialog-header");
        const body = dialog.locator(":scope > .host-settings-body");
        await expect(header.getByRole("heading", { level: 2 })).toHaveText(
          `Create Tensorlake sandbox · ${hostName}`,
        );
        await expect(header.locator(".host-settings-summary")).toHaveText(
          "4 CPUs · 16 GB memory · 50 GB disk",
        );
        await expect(header).toHaveCount(1);
        await expect(body).toHaveCount(1);
        await expect(footer).toHaveCount(1);
        await expect(footer.getByRole("button")).toHaveText([
          "Cancel",
          "Back",
          "Create sandbox",
        ]);
        await expect(
          footer.getByRole("button", { name: "Create sandbox", exact: true }),
        ).toBeFocused();
        for (const answer of await footer.getByRole("button").all()) {
          await expectInlineAnswerVisible(answer);
        }
        const steps = body.locator("ol.provisioning-plan-steps");
        await expect(steps.getByRole("listitem")).toHaveCount(4);
        await expect(steps.locator(".provisioning-plan-number")).toHaveText([
          "1",
          "2",
          "3",
          "4",
        ]);
        await expect(steps.locator(".provisioning-plan-step-name")).toHaveText(
          plans[0].names,
        );
        for (const step of await steps.getByRole("listitem").all()) {
          const boxes = await step.evaluate((element) => {
            const number = element.querySelector(".provisioning-plan-number")!
              .getBoundingClientRect();
            const detail = element.querySelector(".provisioning-plan-detail")!
              .getBoundingClientRect();
            const body = element.closest(".host-settings-body")!
              .getBoundingClientRect();
            return {
              numberRight: number.right,
              detailLeft: detail.left,
              detailRight: detail.right,
              bodyRight: body.right,
            };
          });
          expect(
            boxes.detailLeft,
            "each ordered step has a separate numbered rail",
          ).toBeGreaterThan(boxes.numberRight);
          expect(boxes.detailRight).toBeLessThanOrEqual(boxes.bodyRight);
        }
        const billing = body.locator(".tensorlake-billing");
        await expect(billing).toHaveCount(1);
        await expect(billing).toContainText(
          "This sandbox bills while running.",
        );
        await expect(billing).toContainText(
          "On Usage Credits, Tensorlake suspends a sandbox after 24 hours without a connection.",
        );
        await expect(billing).toContainText(
          "Farhelm never resumes it on its own.",
        );
        const pricing = billing.getByRole("link", {
          name: "Tensorlake pricing",
          exact: true,
        });
        await expect(pricing).toHaveAttribute(
          "href",
          "https://www.tensorlake.ai/pricing",
        );
        await expect(pricing).toHaveAttribute("target", "_blank");
        await expect(pricing).toHaveAttribute("rel", "noopener noreferrer");
        const paint = await billing.evaluate((element) => {
          const style = getComputedStyle(element);
          // Normalize token hex and computed rgb spellings without adding a
          // color-probe node that could disturb the layout being measured.
          const context = document.createElement("canvas").getContext("2d")!;
          const color = (value: string) => {
            context.fillStyle = value;
            return context.fillStyle;
          };
          const border = color(style.borderLeftColor);
          return {
            background: style.backgroundColor,
            borderWidth: parseFloat(style.borderLeftWidth),
            warningBorder: ["--warn", "--warn-border"].some((token) =>
              color(style.getPropertyValue(token).trim()) === border
            ),
          };
        });
        expect(paint.background).not.toBe("rgba(0, 0, 0, 0)");
        expect(paint.borderWidth).toBeGreaterThanOrEqual(2);
        expect(
          paint.warningBorder,
          "billing uses the current palette's warning border",
        ).toBe(true);
        await expect(
          body.getByRole("button", { name: "Copy plan", exact: true }),
        ).toHaveCount(1);
        await expect(
          footer.getByRole("button", { name: "Copy plan", exact: true }),
        ).toHaveCount(0);
        await info.attach(`tensorlake-add-confirmation-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });
        // The displayed step list cannot reconstruct the raw plan's repeated
        // spaces and final newline. Copy must preserve both exactly.
        await page.evaluate(() => {
          (window as any).__tensorlakePlanCopies = [];
          (window as any).__farhelmNativeClipboardWrite = (value: string) => {
            (window as any).__tensorlakePlanCopies.push(value);
          };
        });
        await body.getByRole("button", { name: "Copy plan", exact: true })
          .click();
        await expect.poll(() =>
          page.evaluate(() => (window as any).__tensorlakePlanCopies)
        ).toEqual([plans[0].confirmation]);
        expect(
          submissions,
          "reading or copying the plan supplies no creation authority",
        ).toBe(0);

        await footer.getByRole("button", { name: "Back", exact: true }).click();
        await expect(name).toBeFocused();
        await expect(name).toHaveValue(hostName);
        await expect(select).toHaveValue(recipe);
        await expect(summary).toHaveText(
          "4 CPU · 16 GB · 50 GB · 2 packages · 1 tunnel",
        );
        expect(submissions, "Back submits no creation request").toBe(0);
        await name.fill(`${hostName}-revised`);
        await footer.getByRole("button", { name: "Continue", exact: true })
          .click();
        await expect.poll(() => probes).toEqual([{ name: hostName, recipe }, {
          name: `${hostName}-revised`,
          recipe,
        }]);
        await expect(header.getByRole("heading", { level: 2 })).toHaveText(
          `Create Tensorlake sandbox · ${hostName}-revised`,
        );
        await expect(steps.locator(".provisioning-plan-step-name")).toHaveText(
          plans[1].names,
        );
        expect(
          plans[1].names[1],
          "Back followed by Continue displays the fresh plan",
        ).not.toBe(plans[0].names[1]);
        await footer.getByRole("button", { name: "Cancel", exact: true })
          .click();
        await expect(dialog).toHaveCount(0);
        await openForm();
        await name.fill(hostName);
        await select.selectOption(recipe);
        await footer.getByRole("button", { name: "Continue", exact: true })
          .click();
        await expect.poll(() => probes.length).toBe(3);
        await expect(
          footer.getByRole("button", { name: "Create sandbox", exact: true }),
        ).toBeFocused();
        await page.keyboard.press("Escape");
        await expect(dialog).toHaveCount(0);
        expect(submissions, "Back, Cancel and Escape create no sandbox").toBe(
          0,
        );
        expect((await hosts(request)).map((row) => row.id).sort()).toEqual(
          beforeHosts,
        );
      } catch (error) {
        bodyFailed = true;
        throw error;
      } finally {
        await cleanupOwnedSteps(bodyFailed, [
          ["probe route", stopProbe],
          ["provision refusal route", stopProvision],
          ["capture recipe", async () => {
            const response = await request.delete(
              `/api/tensorlake/recipes/${recipe}`,
            );
            expect([200, 404], await response.text()).toContain(
              response.status(),
            );
          }],
          ["Tensorlake account", async () => {
            const response = await request.put("/api/tensorlake", {
              data: {
                clear_api_key: true,
                create_without_asking: original.create_without_asking,
              },
            });
            expect(response.ok(), await response.text()).toBe(true);
          }],
        ]);
      }
    },
  );
}

/** Spec: SSH's skip preference cannot authorize billed creation. The managed
 * recipe builds, progress grows into ordinary setup steps, and deleting the
 * host terminates its sandbox. Why: a dialog-only assertion would miss billed
 * resources left behind or a setup placeholder that never becomes real work. */
test("adding a Tensorlake host confirms billing and keeps setup progress", async ({ page, request }) => {
  const name = "e2e-tensorlake-host";
  const recipe = "e2e-host-recipe";
  const fake = await controls("tensorlake_fake", { snapshot_polls: 0 });
  await controls("provisioning_backend", { default: { hold_actions: true } });
  expect((await hosts(request)).some((h) => h.alias === name)).toBe(false);
  expect(
    (await request.put("/api/tensorlake", {
      data: { api_key: "test-key", create_without_asking: false },
    })).ok(),
  ).toBe(true);
  expect(
    (await request.post("/api/tensorlake/recipes", {
      data: {
        name: recipe,
        recipe: {
          size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
          source: { kind: "packages", packages: [] },
          claude: null,
          codex: null,
          tunnels: [],
        },
      },
    })).ok(),
  ).toBe(true);
  await patchPreferences(request, { skip_host_setup_confirmation: true });
  let host: any;
  try {
    await page.goto("/");
    await openHostsPanel(page);
    await page.getByRole("button", { name: "Add host", exact: true }).last()
      .click();
    const dialog = page.getByRole("dialog", { name: "Add host", exact: true });
    await dialog.getByRole("button", {
      name: "Tensorlake sandbox",
      exact: true,
    }).click();
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    await dialog.getByRole("combobox", { name: "Recipe", exact: true })
      .selectOption(recipe);
    await dialog.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(
      dialog.getByRole("heading", {
        name: `Create Tensorlake sandbox · ${name}`,
        level: 2,
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      dialog.getByText("This sandbox bills while running.", { exact: false }),
    ).toBeVisible();
    await expect(
      dialog.getByRole("link", { name: "Tensorlake pricing", exact: true }),
    ).toHaveAttribute("href", "https://www.tensorlake.ai/pricing");
    await expect(dialog.getByText("On Usage Credits", { exact: false }))
      .toBeVisible();
    expect(
      (await hosts(request)).some((h) => h.alias === name),
      "confirmation alone creates nothing",
    ).toBe(false);
    await dialog.getByRole("button", { name: "Create sandbox", exact: true })
      .click();
    await expect(dialog).toHaveCount(0);
    await expect.poll(async () => {
      host = (await hosts(request)).find((h) => h.alias === name);
      return host?.kind;
    }).toBe("tensorlake");
    expect(host.yolo_without_asking).toBe(true);
    expect(host.tensorlake.recipe).toBe(recipe);
    await expect.poll(async () =>
      (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json())
        .steps.map((s: any) => s.step)
    ).toContain("prepare-sandbox");
    await controls("provisioning_backend", {});
    await expect.poll(async () =>
      (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json())
        .status
    ).toBe("completed");
    const progress =
      await (await request.get(`/api/hosts/${host.id}/provisioning`)).json();
    expect(progress.steps.map((s: any) => s.step)).toContain(
      "attach-supervisor",
    );
    expect(progress.steps.map((s: any) => s.step)).not.toContain(
      "set-up-farhelm",
    );
    expect((await request.delete(`/api/hosts/${host.id}`)).ok()).toBe(true);
    host = null;
    const model = JSON.parse(
      await readFile(path.join(fake, "state.json"), "utf8"),
    );
    expect(model.sandboxes.every((s: any) => s.status === "terminated")).toBe(
      true,
    );
  } finally {
    await controls("provisioning_backend", {});
    if (host) await request.delete(`/api/hosts/${host.id}`);
    await request.delete(`/api/tensorlake/recipes/${recipe}`);
    await request.put("/api/tensorlake", {
      data: { clear_api_key: true, create_without_asking: false },
    });
    await patchPreferences(request, { skip_host_setup_confirmation: false });
  }
});

/** Spec: the no-key form opens Settings at Tensorlake; an inline recipe's
 * dirty draft survives Escape until Save or Discard. Why: embedding the full
 * editor must preserve the same departure contract as Settings. */
test("the Tensorlake form links Settings and protects an inline recipe draft", async ({ page, request }) => {
  await controls("provisioning_backend", {});
  await request.put("/api/tensorlake", { data: { clear_api_key: true } });
  await page.goto("/");
  await openHostsPanel(page);
  await page.getByRole("button", { name: "Add host", exact: true }).last()
    .click();
  const dialog = page.getByRole("dialog", { name: "Add host", exact: true });
  await dialog.getByRole("button", { name: "Tensorlake sandbox", exact: true })
    .click();
  await dialog.getByRole("button", {
    name: "Open Settings → Tensorlake",
    exact: true,
  }).click();
  const settings = page.getByRole("dialog", { name: "Settings", exact: true });
  await expect(
    settings.getByRole("heading", { name: "Tensorlake", exact: true }),
  ).toBeVisible();
  await request.put("/api/tensorlake", { data: { api_key: "test-key" } });
  await page.keyboard.press("Escape");
  await expect(dialog.getByRole("combobox", { name: "Recipe", exact: true }))
    .toBeVisible();
  await dialog.getByRole("combobox", { name: "Recipe", exact: true })
    .selectOption("__new__");
  const editor = dialog.getByRole("region", {
    name: "New recipe",
    exact: true,
  });
  const answers = dialog.locator(":scope > .recipe-editor-footer");
  await editor.getByRole("textbox", { name: "Name", exact: true }).fill(
    "e2e-unsaved",
  );
  await page.keyboard.press("Escape");
  await expect(
    answers.getByText("Save these changes before leaving?", { exact: true }),
  ).toBeVisible();
  await answers.getByRole("button", { name: "Keep editing", exact: true })
    .click();
  await expect(editor.getByRole("textbox", { name: "Name", exact: true }))
    .toHaveValue("e2e-unsaved");
  // The removed answer button no longer owns focus. Name the editor input
  // as the premise of the next keyboard close request.
  await editor.getByRole("textbox", { name: "Name", exact: true }).focus();
  await expect(editor.getByRole("textbox", { name: "Name", exact: true }))
    .toBeFocused();
  await page.keyboard.press("Escape");
  await expect(
    answers.getByText("Save these changes before leaving?", { exact: true }),
  ).toBeVisible();
  await answers.getByRole("button", { name: "Discard", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await request.put("/api/tensorlake", { data: { clear_api_key: true } });
});

/** Spec: saving an inline reuse recipe selects it, and only the acknowledged
 * Tensorlake preference skips creation's confirmation. Why: this covers the
 * second add path without a managed build and keeps the two preferences apart. */
test("an inline reuse recipe can create without asking through the Tensorlake preference", async ({ page, request }) => {
  const name = "e2e-inline-sandbox";
  const recipe = "e2e-inline-reuse";
  await controls("provisioning_backend", {});
  await request.put("/api/tensorlake", {
    data: { api_key: "test-key", create_without_asking: true },
  });
  let host: any;
  try {
    await page.goto("/");
    await openHostsPanel(page);
    await page.getByRole("button", { name: "Add host", exact: true }).last()
      .click();
    const dialog = page.getByRole("dialog", { name: "Add host", exact: true });
    await dialog.getByRole("button", {
      name: "Tensorlake sandbox",
      exact: true,
    }).click();
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    await dialog.getByRole("combobox", { name: "Recipe", exact: true })
      .selectOption("__new__");
    const editor = dialog.getByRole("region", {
      name: "New recipe",
      exact: true,
    });
    await editor.getByRole("textbox", { name: "Name", exact: true }).fill(
      recipe,
    );
    await editor.getByRole("button", {
      name: /^Use an existing image or snapshot/,
    }).click();
    await editor.getByRole("textbox", { name: "Image", exact: true }).fill(
      "example/systemd",
    );
    await dialog.locator(":scope > .recipe-editor-footer").getByRole("button", {
      name: "Save",
      exact: true,
    }).click();
    await expect(dialog.getByRole("combobox", { name: "Recipe", exact: true }))
      .toHaveValue(recipe);
    await dialog.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect.poll(async () => {
      host = (await hosts(request)).find((h) => h.alias === name);
      return host?.kind;
    }).toBe("tensorlake");
    await expect.poll(async () =>
      (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json())
        .status
    ).toBe("completed");
    const progress =
      await (await request.get(`/api/hosts/${host.id}/provisioning`)).json();
    expect(progress.steps.some((s: any) => s.step === "build-snapshot")).toBe(
      false,
    );
  } finally {
    if (host) await request.delete(`/api/hosts/${host.id}`);
    await request.delete(`/api/tensorlake/recipes/${recipe}`);
    await request.put("/api/tensorlake", {
      data: { clear_api_key: true, create_without_asking: false },
    });
  }
});

/** A confirmation may outlive its recipe. Refuse and roll back the new row
 * before billing when deletion won; a frozen plan must not leave a host
 * referring to a recipe the user can no longer edit. */
test("a recipe deleted during confirmation refuses without registering a host", async ({ request }) => {
  const recipe = "e2e-deleted-recipe";
  const name = "e2e-deleted-recipe-host";
  await request.put("/api/tensorlake", { data: { api_key: "test-key" } });
  try {
    expect(
      (await request.post("/api/tensorlake/recipes", {
        data: {
          name: recipe,
          recipe: {
            size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
            source: { kind: "image", image: "example/systemd" },
            claude: null,
            codex: null,
            tunnels: [],
          },
        },
      })).ok(),
    ).toBe(true);
    const planned = await request.post("/api/hosts/probe", {
      data: { target: { kind: "tensorlake", name, recipe } },
    });
    expect(planned.ok(), await planned.text()).toBe(true);
    const probe = await planned.json();
    expect((await request.delete(`/api/tensorlake/recipes/${recipe}`)).ok())
      .toBe(true);
    const refused = await request.post("/api/hosts/provision", {
      data: { probe_id: probe.probe_id },
    });
    expect(refused.status()).toBe(409);
    expect(await refused.text()).toContain("recipe was deleted");
    expect((await hosts(request)).some((host) => host.alias === name)).toBe(
      false,
    );
  } finally {
    await request.delete(`/api/tensorlake/recipes/${recipe}`);
    await request.put("/api/tensorlake", { data: { clear_api_key: true } });
  }
});

/** Cancel and a queued confirm can run in one event burst before unmount.
 * The accepted close must synchronously retire creation authority. */
test("cancel retires a Tensorlake creation before a queued answer", async ({ page, request }) => {
  const recipe = "e2e-cancel-create";
  const name = "e2e-cancel-create-host";
  await request.put("/api/tensorlake", {
    data: { api_key: "test-key", create_without_asking: false },
  });
  expect(
    (await request.post("/api/tensorlake/recipes", {
      data: {
        name: recipe,
        recipe: {
          size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
          source: { kind: "image", image: "example/systemd" },
          claude: null,
          codex: null,
          tunnels: [],
        },
      },
    })).ok(),
  ).toBe(true);
  let submissions = 0;
  page.on("request", (req) => {
    if (
      req.method() === "POST" &&
      new URL(req.url()).pathname === "/api/hosts/provision"
    ) submissions++;
  });
  try {
    await page.goto("/");
    await openHostsPanel(page);
    await page.getByRole("button", { name: "Add host", exact: true }).last()
      .click();
    const dialog = page.getByRole("dialog", { name: "Add host", exact: true });
    await dialog.getByRole("button", {
      name: "Tensorlake sandbox",
      exact: true,
    }).click();
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    await dialog.getByRole("combobox", { name: "Recipe", exact: true })
      .selectOption(recipe);
    await dialog.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(
      dialog.getByRole("button", { name: "Create sandbox", exact: true }),
    ).toBeEnabled();
    await dialog.evaluate((element) => {
      const cancel = element.querySelector<HTMLButtonElement>(
        ".add-host-cancel",
      )!;
      const confirm = element.querySelector<HTMLButtonElement>(
        ".provisioning-confirm",
      )!;
      cancel.click();
      confirm.click();
    });
    await expect(dialog).toHaveCount(0);
    expect(submissions).toBe(0);
    expect((await hosts(request)).some((host) => host.alias === name)).toBe(
      false,
    );
  } finally {
    const host = (await hosts(request)).find((host) => host.alias === name);
    if (host) await request.delete(`/api/hosts/${host.id}`);
    await request.delete(`/api/tensorlake/recipes/${recipe}`);
    await request.put("/api/tensorlake", { data: { clear_api_key: true } });
  }
});

for (const skipTensorlake of [false, true]) {
  /** A failed run may not yet have created anything. Rerun must use only
   * Tensorlake's billing preference, with no answer that changes SSH's. */
  test(`rerun before first create uses Tensorlake skip=${skipTensorlake}`, async ({ page, request }) => {
    const name = `e2e-rerun-policy-${skipTensorlake}`;
    const recipe = `e2e-rerun-recipe-${skipTensorlake}`;
    let host: any;
    const fake = await controls("tensorlake_fake", {
      fail: { create_sandbox: { status: 401, message: "key refused" } },
    });
    await controls("provisioning_backend", {});
    expect(
      (await request.put("/api/tensorlake", {
        data: { api_key: "test-key", create_without_asking: skipTensorlake },
      })).ok(),
    ).toBe(true);
    await patchPreferences(request, {
      skip_host_setup_confirmation: !skipTensorlake,
    });
    expect(
      (await request.post("/api/tensorlake/recipes", {
        data: {
          name: recipe,
          recipe: {
            size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
            source: { kind: "image", image: "example/systemd" },
            claude: null,
            codex: null,
            tunnels: [],
          },
        },
      })).ok(),
    ).toBe(true);
    try {
      const probe = await (await request.post("/api/hosts/probe", {
        data: { target: { kind: "tensorlake", name, recipe } },
      })).json();
      const accepted = await (await request.post("/api/hosts/provision", {
        data: { probe_id: probe.probe_id },
      })).json();
      host = { id: accepted.host_id };
      await expect.poll(async () =>
        (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json())
          .status
      ).toBe("failed");
      const model = JSON.parse(
        await readFile(path.join(fake, "state.json"), "utf8").catch(() =>
          '{"sandboxes":[]}'
        ),
      );
      expect(model.sandboxes.filter((s: any) => s.status !== "terminated"))
        .toHaveLength(0);
      await controls("tensorlake_fake", {});
      await page.goto("/");
      await openHostsPanel(page);
      const row = hostRowByName(page, name);
      await openHostMenu(row);
      await row.locator(".provisioning-rerun").click();
      const confirmation = page.getByRole("dialog", {
        name: "Host setup",
        exact: true,
      });
      if (!skipTensorlake) {
        const planBody = confirmation.locator(":scope > .host-settings-body");
        const billing = planBody.locator(".tensorlake-billing");
        await expect(billing).toBeVisible();
        await expect(billing).toContainText(
          "This sandbox bills while running.",
        );
        await expect(billing).toContainText(
          "On Usage Credits, Tensorlake suspends a sandbox after 24 hours without a connection.",
        );
        await expect(
          billing.getByRole("link", {
            name: "Tensorlake pricing",
            exact: true,
          }),
        ).toHaveAttribute("href", "https://www.tensorlake.ai/pricing");
        await expect(
          confirmation.getByRole("button", { name: "Back", exact: true }),
        ).toHaveCount(0);
        await expect(
          planBody.getByRole("button", { name: "Copy plan", exact: true }),
        ).toHaveCount(1);
        await expect(
          confirmation.locator(":scope > .dialog-footer").getByRole("button", {
            name: "Copy plan",
            exact: true,
          }),
        ).toHaveCount(0);
        await expect(
          confirmation.getByRole("button", {
            name: "Set up, don't ask again",
            exact: true,
          }),
        ).toHaveCount(0);
        expect(
          (await (await request.get(`/api/hosts/${host.id}/provisioning`))
            .json()).status,
        ).toBe("failed");
        await confirmation.getByRole("button", {
          name: "Create sandbox",
          exact: true,
        }).click();
      }
      await expect.poll(async () =>
        (await (await request.get(`/api/hosts/${host.id}/provisioning`)).json())
          .status
      ).toBe("completed");
      await expect(confirmation).toHaveCount(0);
      const prefs = await (await request.get("/api/preferences")).json();
      expect(prefs.skip_host_setup_confirmation).toBe(!skipTensorlake);
    } finally {
      await controls("tensorlake_fake", {});
      if (host) await request.delete(`/api/hosts/${host.id}`);
      await request.delete(`/api/tensorlake/recipes/${recipe}`);
      await request.put("/api/tensorlake", {
        data: { clear_api_key: true, create_without_asking: false },
      });
      await patchPreferences(request, { skip_host_setup_confirmation: false });
    }
  });
}
