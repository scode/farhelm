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

/** This explanation must survive changing preparation messages in live setup. */
const PREPARATION_EXPLANATION =
  "Only the first time, and then at most once every 24 hours or after Farhelm updates.";

/** Action targets distinguish the temporary installer from the host it prepares. */
async function backendEvents(root: string): Promise<{ event: string; target: string }[]> {
  return (await readFile(path.join(root, "events.jsonl"), "utf8"))
    .split("\n").filter(Boolean).map((line) => JSON.parse(line));
}

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
        base: { kind: "tensorlake_ubuntu" },
        packages: ["git", "ripgrep"],
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
              "Prepare snapshot",
              `Create sandbox ${sandbox} with 4 CPUs, 16 GB memory and 50 GB disk from the prepared snapshot`,
              "Configure the recipe's agent credentials",
              "Connect to the supervisor that starts at boot and open the recipe's tunnels",
            ];
            const confirmation =
              `Create Tensorlake sandbox ${sandbox}\nSize: 4 CPUs,  16 GB memory, 50 GB disk\n${
                names.map((name, index) => index === 1 ? `${name}\n${PREPARATION_EXPLANATION}` : name).join("\n")
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
                    explanation: index === 1 ? PREPARATION_EXPLANATION : null,
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
          "4 CPU · 16 GB · 50 GB · base: Tensorlake Ubuntu · 2 packages · 1 tunnel",
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
        await expect(steps.getByRole("listitem")).toHaveCount(plans[0].names.length);
        await expect(steps.locator(".provisioning-plan-number")).toHaveText(
          plans[0].names.map((_, index) => String(index + 1)),
        );
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
          "4 CPU · 16 GB · 50 GB · base: Tensorlake Ubuntu · 2 packages · 1 tunnel",
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
          plans[1].names[2],
          "Back followed by Continue displays the fresh plan",
        ).not.toBe(plans[0].names[2]);
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
 * recipe builds and prepares lazily, live progress explains the first wait,
 * and a second host reuses that installation. Why: a completed dialog alone
 * cannot show whether every new host still pays for installation. */
test("adding a Tensorlake host confirms billing and keeps setup progress", async ({ page, request }) => {
  const name = "e2e-tensorlake-host";
  const recipe = "e2e-host-recipe";
  const fake = await controls("tensorlake_fake", { snapshot_polls: 0 });
  const backend = await controls("provisioning_backend", { default: { hold_actions: true } });
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
          base: { kind: "tensorlake_ubuntu" },
          packages: [],
          claude: null,
          codex: null,
          tunnels: [],
        },
      },
    })).ok(),
  ).toBe(true);
  await patchPreferences(request, { skip_host_setup_confirmation: true });
  let host: any;
  let secondHost: any;
  try {
    await page.goto("/");
    await openHostsPanel(page);
    // Global details deliberately suppresses the per-run Collapse control.
    // Exercise the ordinary auto-expanded run before folding it below.
    await page.locator(".host-details-toggle").uncheck();
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
    await expect(dialog.getByText("Prepare snapshot", { exact: true })).toBeVisible();
    await expect(dialog.getByText(PREPARATION_EXPLANATION, { exact: true })).toBeVisible();
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
        .steps.find((s: any) => s.step === "prepare-snapshot")?.status
    ).toBe("running");
    await expect.poll(async () =>
      (await backendEvents(backend)).some((event) => event.event === "prepare-sandbox")
    ).toBe(true);
    const row = hostRowByName(page, name);
    const preparing = row.locator('.provisioning-step[data-step="prepare-snapshot"]');
    await expect(preparing).toHaveAttribute("data-status", "running");
    await expect(preparing.getByText(PREPARATION_EXPLANATION, { exact: true })).toBeVisible();
    await row.locator(".provisioning-run-fold").click();
    await row.locator(".host-update-verb").hover();
    await expect(row.locator(".host-update-popup").getByText(PREPARATION_EXPLANATION, { exact: true })).toBeVisible();
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
    const preparedRecipe = (await (await request.get("/api/tensorlake/recipes")).json())
      .recipes.find((r: any) => r.name === recipe);
    expect(preparedRecipe.prepared.source_snapshot).toBe(preparedRecipe.snapshot.id);
    expect(preparedRecipe.preparation.state).toBe("idle");
    const beforeSecond = JSON.parse(await readFile(path.join(fake, "state.json"), "utf8"));
    const firstClone = beforeSecond.sandboxes.find((s: any) => s.name === host.tensorlake.sandbox);
    expect(firstClone.snapshot_id).toBe(preparedRecipe.prepared.id);
    const installerEvents = await backendEvents(backend);
    expect(installerEvents.filter((event) => event.event === "finish-snapshot-install")).toHaveLength(1);
    await page.getByRole("button", { name: "Add host", exact: true }).last().click();
    await dialog.getByRole("button", { name: "Tensorlake sandbox", exact: true }).click();
    await dialog.getByRole("textbox", { name: "Name", exact: true }).fill(`${name}-second`);
    await dialog.getByRole("combobox", { name: "Recipe", exact: true }).selectOption(recipe);
    await dialog.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(dialog.getByRole("button", { name: "Create sandbox", exact: true })).toBeVisible();
    await expect(dialog.getByText("Prepare snapshot", { exact: true })).toHaveCount(0);
    await expect(dialog.getByText(PREPARATION_EXPLANATION, { exact: true })).toHaveCount(0);
    await dialog.getByRole("button", { name: "Create sandbox", exact: true }).click();
    await expect.poll(async () => {
      secondHost = (await hosts(request)).find((h) => h.alias === `${name}-second`);
      return secondHost?.kind;
    }).toBe("tensorlake");
    await expect.poll(async () =>
      (await (await request.get(`/api/hosts/${secondHost.id}/provisioning`)).json()).status
    ).toBe("completed");
    const secondProgress = await (await request.get(`/api/hosts/${secondHost.id}/provisioning`)).json();
    expect(secondProgress.steps.map((s: any) => s.step)).not.toContain("prepare-snapshot");
    const afterSecond = JSON.parse(await readFile(path.join(fake, "state.json"), "utf8"));
    const newSandboxes = afterSecond.sandboxes.filter((s: any) => !beforeSecond.sandboxes.some((old: any) => old.id === s.id));
    expect(newSandboxes, "fresh reuse allocates only the new host").toHaveLength(1);
    expect(newSandboxes[0].snapshot_id).toBe(preparedRecipe.prepared.id);
    expect((await backendEvents(backend)).slice(installerEvents.length).some((event) =>
      ["prepare-sandbox", "install-farhelm", "enable-supervisor", "finish-snapshot-install"].includes(event.event)
    )).toBe(false);
    expect((await request.delete(`/api/hosts/${secondHost.id}`)).ok()).toBe(true);
    secondHost = null;
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
    if (secondHost) await request.delete(`/api/hosts/${secondHost.id}`);
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

/** Spec: saving an inline image-base recipe selects it, and only the acknowledged
 * Tensorlake preference skips creation's confirmation. The new host still builds
 * its snapshot first, and the host starts from that completed result. Why: the
 * step exists even when skipped, so its status and the fake's creation source
 * must distinguish an actual build from merely listing the normal steps. The
 * two setup preferences must remain independent. */
test("an inline image-base recipe builds before creating through the Tensorlake preference", async ({ page, request }) => {
  const name = "e2e-inline-sandbox";
  const recipe = "e2e-inline-base";
  const fake = await controls("tensorlake_fake", { snapshot_polls: 0 });
  await controls("provisioning_backend", {});
  await request.put("/api/tensorlake", {
    data: { api_key: "test-key", create_without_asking: true },
  });
  let host: any;
  try {
    expect((await hosts(request)).some((h) => h.alias === name)).toBe(false);
    const listed = await (await request.get("/api/tensorlake/recipes")).json();
    expect(listed.recipes.some((r: any) => r.name === recipe)).toBe(false);
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
      name: /^Your image/,
    }).click();
    await editor.getByRole("textbox", { name: "Image", exact: true }).fill(
      "example/systemd",
    );
    await expect(editor.locator(".recipe-base-requirements"))
      .toContainText("sudo without a password");
    await editor.getByRole("textbox", { name: "Ubuntu packages", exact: true })
      .fill("jq");
    await dialog.locator(":scope > .recipe-editor-footer").getByRole("button", {
      name: "Save",
      exact: true,
    }).click();
    await expect(dialog.getByRole("combobox", { name: "Recipe", exact: true }))
      .toHaveValue(recipe);
    await expect(dialog.locator(".add-tensorlake-recipe-summary"))
      .toContainText("base: your image example/systemd · 1 package");
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
    expect(progress.steps.find((s: any) => s.step === "build-snapshot")?.status)
      .toBe("completed");
    const recipeView = (await (await request.get("/api/tensorlake/recipes")).json())
      .recipes.find((r: any) => r.name === recipe);
    const built = recipeView.snapshot;
    expect(built, "the inline recipe recorded a built snapshot").toBeTruthy();
    expect(built.base).toEqual({ kind: "image", image: "example/systemd" });
    expect(built.packages).toEqual(["jq"]);
    const model = JSON.parse(await readFile(path.join(fake, "state.json"), "utf8"));
    const sandbox = model.sandboxes.find((s: any) => s.name === host.tensorlake.sandbox);
    expect(sandbox, "the registered host has its own sandbox in the fake").toBeTruthy();
    expect(sandbox.status).toBe("running");
    const prepared = recipeView.prepared;
    expect(prepared, "the inline recipe also recorded Farhelm's installation").toBeTruthy();
    expect(prepared.source_snapshot).toBe(built.id);
    const checkpoint = model.snapshots.find((s: any) => s.id === prepared.id);
    const preparer = model.sandboxes.find((s: any) => s.id === checkpoint.sandbox_id);
    expect(preparer.name).toMatch(/^farhelm-prepare-/);
    expect(preparer.snapshot_id).toBe(built.id);
    expect(preparer.status).toBe("terminated");
    expect(sandbox.snapshot_id).toBe(prepared.id);
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
            base: { kind: "image", image: "example/systemd" },
            packages: [],
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
          base: { kind: "image", image: "example/systemd" },
          packages: [],
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
  /** A failed host create leaves a registered host with no sandbox. Build and
   * prepare first so the refusal cannot hit a builder or preparer instead.
   * Rerun must use only Tensorlake's billing preference, with no answer that
   * changes SSH's, and create that host from the same recorded snapshot. */
  test(`rerun before first create uses Tensorlake skip=${skipTensorlake}`, async ({ page, request }) => {
    const name = `e2e-rerun-policy-${skipTensorlake}`;
    const recipe = `e2e-rerun-recipe-${skipTensorlake}`;
    let host: any;
    let primerHost: any;
    const fake = await controls("tensorlake_fake", { snapshot_polls: 0 });
    await controls("provisioning_backend", {});
    expect(
      (await request.put("/api/tensorlake", {
        data: { api_key: "test-key", create_without_asking: skipTensorlake },
      })).ok(),
    ).toBe(true);
    await patchPreferences(request, {
      skip_host_setup_confirmation: !skipTensorlake,
    });
    expect((await hosts(request)).some((h) => h.alias === name)).toBe(false);
    const existingRecipes = await (await request.get("/api/tensorlake/recipes")).json();
    expect(existingRecipes.recipes.some((r: any) => r.name === recipe)).toBe(false);
    expect(
      (await request.post("/api/tensorlake/recipes", {
        data: {
          name: recipe,
          recipe: {
            size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
            base: { kind: "image", image: "example/systemd" },
            packages: [],
            claude: null,
            codex: null,
            tunnels: [],
          },
        },
      })).ok(),
    ).toBe(true);
    try {
      const key = await request.post("/api/tensorlake/ssh-key");
      expect(key.ok(), await key.text()).toBe(true);
      const build = await request.post(`/api/tensorlake/recipes/${recipe}/build`);
      expect(build.status(), await build.text()).toBe(202);
      let built: any;
      await expect.poll(async () => {
        const view = (await (await request.get("/api/tensorlake/recipes")).json())
          .recipes.find((r: any) => r.name === recipe);
        built = view.snapshot;
        return [view.build.state, view.cleanup_pending, Boolean(built)];
      }, { message: "the recipe build must finish cleanup before host-create failure injection" })
        .toEqual(["idle", false, true]);
      expect(built.base).toEqual({ kind: "image", image: "example/systemd" });
      expect(built.packages).toEqual([]);
      // Lazy preparation has no button. A short-lived first host establishes
      // the real cache before the next create is deliberately refused.
      const primerProbe = await (await request.post("/api/hosts/probe", {
        data: { target: { kind: "tensorlake", name: `${name}-primer`, recipe } },
      })).json();
      const primerAccepted = await request.post("/api/hosts/provision", {
        data: { probe_id: primerProbe.probe_id },
      });
      expect(primerAccepted.ok(), await primerAccepted.text()).toBe(true);
      primerHost = { id: (await primerAccepted.json()).host_id };
      await expect.poll(async () =>
        (await (await request.get(`/api/hosts/${primerHost.id}/provisioning`)).json()).status,
      ).toBe("completed");
      const ready = (await (await request.get("/api/tensorlake/recipes")).json())
        .recipes.find((r: any) => r.name === recipe);
      const prepared = ready.prepared;
      expect(prepared?.source_snapshot).toBe(built.id);
      expect(prepared.stale_reason).toBeNull();
      expect(ready.preparation.state).toBe("idle");
      const removedPrimer = await request.delete(`/api/hosts/${primerHost.id}`);
      expect(removedPrimer.ok(), await removedPrimer.text()).toBe(true);
      primerHost = null;
      const beforeCreate = JSON.parse(await readFile(path.join(fake, "state.json"), "utf8"));
      const snapshot = beforeCreate.snapshots.find((s: any) => s.id === built.id);
      expect(snapshot, "the fixture owns a completed snapshot").toBeTruthy();
      expect(snapshot.status).toBe("completed");
      // Terminated builders retain their reused names across projects. Only
      // the snapshot's recorded source ID identifies this particular build.
      const builder = beforeCreate.sandboxes.find((s: any) => s.id === snapshot.sandbox_id);
      expect(builder, "the fixture's snapshot came from its own builder").toBeTruthy();
      expect(builder.name).toMatch(/^farhelm-build-/);
      expect(builder.name.endsWith(`-${recipe}`)).toBe(true);
      expect(builder.status).toBe("terminated");
      const priorSandboxIds = beforeCreate.sandboxes.map((s: any) => s.id);
      await controls("tensorlake_fake", {
        fail: { create_sandbox: { status: 401, message: "key refused" } },
      });
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
      const failed = await (await request.get(`/api/hosts/${host.id}/provisioning`)).json();
      expect(failed.steps.find((s: any) => s.step === "build-snapshot")?.status)
        .toBe("skipped");
      expect(failed.steps.find((s: any) => s.step === "create-sandbox")?.status)
        .toBe("failed");
      expect(failed.steps.some((s: any) => s.step === "prepare-snapshot")).toBe(false);
      const model = JSON.parse(
        await readFile(path.join(fake, "state.json"), "utf8"),
      );
      expect(model.sandboxes.map((s: any) => s.id), "the failed host create leaves the prebuilt fixture unchanged")
        .toEqual(priorSandboxIds);
      expect(model.snapshots.some((s: any) => s.id === built.id)).toBe(true);
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
      const completed = await (await request.get(`/api/hosts/${host.id}/provisioning`)).json();
      expect(completed.steps.find((s: any) => s.step === "build-snapshot")?.status)
        .toBe("skipped");
      expect(completed.steps.find((s: any) => s.step === "create-sandbox")?.status)
        .toBe("completed");
      const afterRerun = JSON.parse(await readFile(path.join(fake, "state.json"), "utf8"));
      const newSandboxes = afterRerun.sandboxes.filter((s: any) => !priorSandboxIds.includes(s.id));
      expect(newSandboxes, "the retry creates one host and no new builder or preparer").toHaveLength(1);
      expect(newSandboxes[0].name).toMatch(/^farhelm-host-/);
      expect(newSandboxes[0].status).toBe("running");
      expect(newSandboxes[0].snapshot_id).toBe(prepared.id);
      const prefs = await (await request.get("/api/preferences")).json();
      expect(prefs.skip_host_setup_confirmation).toBe(!skipTensorlake);
    } finally {
      await controls("tensorlake_fake", {});
      if (host) await request.delete(`/api/hosts/${host.id}`);
      if (primerHost) await request.delete(`/api/hosts/${primerHost.id}`);
      await request.delete(`/api/tensorlake/recipes/${recipe}`);
      await request.put("/api/tensorlake", {
        data: { clear_api_key: true, create_without_asking: false },
      });
      await patchPreferences(request, { skip_host_setup_confirmation: false });
    }
  });
}
