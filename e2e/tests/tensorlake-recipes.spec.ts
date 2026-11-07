/** Tensorlake recipes in Settings: the list, the in-place editor, snapshot
 * builds, and the helm's recipe API behind them. Recipes themselves are local
 * records, so those tests use the real helm and read the API directly to
 * check what was stored. Snapshot builds call Tensorlake and run commands in
 * a sandbox; start-stack.sh gives every helm the E2E-only Tensorlake fake
 * (FARHELM_E2E_TENSORLAKE_DIR), whose config.json the build test owns (hold
 * or fail a build step) and whose state.json and events.jsonl say what the
 * helm created, ran and deleted. Recipe names carry an `e2e-` prefix and
 * every test removes what it created. */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext, Locator, Page } from "@playwright/test";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { currentStackSlot } from "../stack-slots";
import { openSettings, selectSettingsSection } from "./helpers/settings";

/** A token that must never appear in a reply or on the page once saved. */
const TOKEN = "e2e-private-endpoint-token";

/** Every recipe the helm lists, as the API returns them. */
async function recipes(request: APIRequestContext): Promise<any[]> {
  const response = await request.get("/api/tensorlake/recipes");
  expect(response.ok(), await response.text()).toBe(true);
  return (await response.json()).recipes;
}

/** Delete the named recipes if they exist; cleanup after a test, ignoring 404s. */
async function removeRecipes(request: APIRequestContext, names: string[]) {
  for (const name of names) {
    const response = await request.delete(`/api/tensorlake/recipes/${name}`);
    expect([200, 404], await response.text()).toContain(response.status());
  }
}

/** Open Settings → Tensorlake and return its recipe list region. */
async function openRecipes(page: Page): Promise<Locator> {
  const dialog = await openSettings(page, "tensorlake");
  const list = dialog.getByRole("region", { name: "Recipes", exact: true });
  await expect(list.getByRole("button", { name: "New recipe", exact: true }))
    .toBeVisible();
  return list;
}

/** Every current answer must be hit-testable inside the body's scrollport,
 * above the separate Close footer. Checking before a click matters: browser
 * automation would otherwise scroll an ordinary bottom-of-form Save for us. */
async function expectSettingsRecipeAnswers(dialog: Locator, editor: Locator) {
  const answers = editor.locator(".recipe-editor-sticky-actions");
  await expect(answers).toHaveCSS("position", "sticky");
  await expect(answers.getByRole("button").first()).toBeVisible();
  for (const answer of await answers.getByRole("button").all()) {
    await expect.poll(() =>
      answer.evaluate((element) => {
        const box = element.getBoundingClientRect();
        const body = element.closest(".host-settings-body")!
          .getBoundingClientRect();
        const dialog = element.closest('[role="dialog"]')!;
        const footer = dialog.querySelector(":scope > .dialog-footer")!
          .getBoundingClientRect();
        const hit = document.elementFromPoint(
          box.x + box.width / 2,
          box.y + box.height / 2,
        );
        const inside = box.width > 0 && box.height > 0 &&
          box.top >= Math.max(body.top, 0) &&
          box.bottom <= Math.min(body.bottom, footer.top, innerHeight) &&
          box.left >= body.left && box.right <= body.right &&
          Boolean(hit && element.contains(hit));
        return inside ? "visible" : JSON.stringify({
          answer: element.textContent,
          box: box.toJSON(),
          body: body.toJSON(),
          footer: footer.toJSON(),
          hit: hit?.tagName,
        });
      }), {
      message: "recipe answers must stay inside Settings' body above Close",
    }).toBe("visible");
  }
  await expect(
    dialog.locator(":scope > .dialog-footer").getByRole("button", {
      name: "Close",
      exact: true,
    }),
  ).toBeVisible();
}

/** Persistent Save makes failures possible while the last field is offscreen.
 * Require the reason itself inside the sticky surface and visible scrollport;
 * finding an alert in the editor would miss a reason stranded below the fields. */
async function expectSettingsRecipeErrorVisible(error: Locator) {
  await expect.poll(() =>
    error.evaluate((element) => {
      const box = element.getBoundingClientRect();
      const surface = element.closest(".recipe-editor-sticky-actions")!
        .getBoundingClientRect();
      const body = element.closest(".host-settings-body")!
        .getBoundingClientRect();
      const footer = element.closest('[role="dialog"]')!.querySelector(
        ":scope > .dialog-footer",
      )!.getBoundingClientRect();
      const hit = document.elementFromPoint(
        box.x + box.width / 2,
        box.y + box.height / 2,
      );
      const inside = box.width > 0 && box.height > 0 &&
        box.top >= Math.max(surface.top, body.top, 0) &&
        box.bottom <=
          Math.min(surface.bottom, body.bottom, footer.top, innerHeight) &&
        box.left >= Math.max(surface.left, body.left, 0) &&
        box.right <= Math.min(surface.right, body.right, innerWidth) &&
        Boolean(hit && element.contains(hit));
      return inside ? "visible" : JSON.stringify({
        box: box.toJSON(),
        surface: surface.toJSON(),
        body: body.toJSON(),
        footer: footer.toJSON(),
        hit: hit?.tagName,
      });
    }), {
    message:
      "recipe refusal must remain visible beside Settings' sticky answers",
  }).toBe("visible");
}

/** Settings keeps account content before the editor and other settings after
 * it. Sample its own top/middle/bottom, rather than the whole panel's bottom
 * where a correctly bounded sticky row has already left the viewport. */
async function scrollRecipeEditor(editor: Locator, fraction: number) {
  return editor.evaluate((element, fraction) => {
    const body = element.closest(".host-settings-body")!;
    const style = getComputedStyle(body);
    const top = body.scrollTop + element.getBoundingClientRect().top -
      body.getBoundingClientRect().top;
    const travel = element.getBoundingClientRect().height - body.clientHeight +
      parseFloat(style.paddingBottom);
    body.scrollTop = top + Math.max(0, travel) * fraction;
    return { top, travel, actual: body.scrollTop };
  }, fraction);
}

/** The editor opens at its own top, and normal/leave/delete answers stay
 * reachable without replacing Settings' Close. Both palettes retain actual
 * screenshots for visual review; geometry and Tab assert behavior that a
 * screenshot or a successful auto-scrolling click cannot establish alone.
 * Section changes keep the draft mounted but hide its actions, and closing
 * from another section must reveal the guarded question before saving. */
for (const palette of ["dark", "light"] as const) {
  test(
    `Settings keeps recipe answers sticky and isolated to Tensorlake in ${palette}`,
    async ({ page, request }, info) => {
      const existing = `e2e-sticky-${palette}`;
      const saved = `e2e-sticky-new-${palette}`;
      const before = (await recipes(request)).map((row) => row.name);
      expect(before, "fixture owns its recipe names").not.toContain(existing);
      expect(before).not.toContain(saved);
      const created = await request.post("/api/tensorlake/recipes", {
        data: {
          name: existing,
          recipe: {
            size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
            source: { kind: "packages", packages: [] },
            claude: null,
            codex: null,
            tunnels: [],
          },
        },
      });
      expect(created.ok(), await created.text()).toBe(true);
      try {
        await page.emulateMedia({ colorScheme: palette });
        await page.setViewportSize({ width: 1280, height: 720 });
        await page.goto("/");
        await expect(page.locator("html")).toHaveAttribute(
          "data-appearance",
          palette,
        );
        let list = await openRecipes(page);
        const dialog = page.getByRole("dialog", {
          name: "Settings",
          exact: true,
        });
        const body = dialog.locator(":scope > .host-settings-body");
        const newRecipe = list.getByRole("button", {
          name: "New recipe",
          exact: true,
        });
        await expect(list.getByRole("listitem").filter({ hasText: existing }))
          .toBeVisible();
        await newRecipe.scrollIntoViewIfNeeded();
        expect(
          (await newRecipe.boundingBox())!.width,
          "New recipe keeps an intrinsic width inside the section's grid",
        ).toBeLessThan((await list.boundingBox())!.width - 60);
        await expect.poll(() => body.evaluate((element) => element.scrollTop), {
          message:
            "premise: opening the editor must start from a scrolled recipe list",
        }).toBeGreaterThan(0);
        await newRecipe.click();
        const editor = dialog.getByRole("region", {
          name: "New recipe",
          exact: true,
        });
        const heading = editor.getByRole("heading", {
          name: "New recipe",
          level: 4,
          exact: true,
        });
        const name = editor.getByRole("textbox", { name: "Name", exact: true });
        await expect(heading).toBeFocused();
        await page.evaluate(() => document.fonts.ready);
        await expect.poll(() =>
          editor.evaluate((element) => {
            const body = element.closest(".host-settings-body")!;
            return Math.abs(
              element.getBoundingClientRect().top -
                body.getBoundingClientRect().top,
            );
          }), {
          message: "the editor begins at the scrollport's top",
        }).toBeLessThan(2);
        const back = editor.getByRole("button", {
          name: "‹ Recipes",
          exact: true,
        });
        await expect.poll(() =>
          back.evaluate((element) => {
            const box = element.getBoundingClientRect();
            const body = element.closest(".host-settings-body")!
              .getBoundingClientRect();
            const hit = document.elementFromPoint(
              box.x + box.width / 2,
              box.y + box.height / 2,
            );
            return box.width > 0 && box.height > 0 &&
              box.top >= Math.max(body.top, 0) &&
              box.bottom <= Math.min(body.bottom, innerHeight) &&
              box.left >= body.left &&
              box.right <= body.right && Boolean(hit && element.contains(hit));
          }), {
          message:
            "opening the editor must reveal the whole Recipes back button",
        }).toBe(true);
        await expect(heading).toHaveCSS("font-size", "18px");
        await expect(editor.locator("label.checkout-root-label > span").first())
          .toHaveCSS("font-size", "13px");
        await expect(
          editor.getByRole("spinbutton", { name: /^CPUs/ }).locator("..")
            .locator("span").first(),
        ).toHaveCSS("font-size", "13px");
        await expect(
          editor.getByLabel("Claude Code base URL", { exact: true }).locator(
            "..",
          ).locator("span").first(),
        ).toHaveCSS("font-size", "13px");
        const captions = editor.locator("div.checkout-root-label > span");
        await expect(captions).toHaveText([
          "Size", "Software", "Tunnels to your side", "GitHub", "Agents",
        ]);
        for (const caption of await captions.all()) {
          await expect(caption).toHaveCSS("font-size", "14px");
        }
        await expectSettingsRecipeAnswers(dialog, editor);
        await info.attach(`settings-recipe-initial-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });

        const answers = editor.locator(".recipe-editor-sticky-actions");
        const save = answers.getByRole("button", { name: "Save", exact: true });
        const error = answers.getByRole("alert");
        await name.fill("Bad Name");
        const invalidTop = await scrollRecipeEditor(editor, 0);
        expect(
          Math.abs(invalidTop.actual - invalidTop.top),
          "invalid Save starts at the editor's top",
        ).toBeLessThan(2);
        await expectSettingsRecipeAnswers(dialog, editor);
        await save.click();
        await expect(error).toContainText(
          "lowercase letters, digits and hyphens",
        );
        await expectSettingsRecipeErrorVisible(error);
        await expectSettingsRecipeAnswers(dialog, editor);
        await info.attach(`settings-recipe-invalid-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });

        await name.fill(existing);
        await expect(error).toHaveCount(0);
        const refusedTop = await scrollRecipeEditor(editor, 0);
        expect(
          Math.abs(refusedTop.actual - refusedTop.top),
          "API-refused Save starts at the editor's top",
        ).toBeLessThan(2);
        await expectSettingsRecipeAnswers(dialog, editor);
        const [refused] = await Promise.all([
          page.waitForResponse((response) =>
            response.request().method() === "POST" &&
            new URL(response.url()).pathname === "/api/tensorlake/recipes"
          ),
          save.click(),
        ]);
        expect(
          refused.status(),
          "the saved-name refusal came from the real recipe API",
        ).toBe(409);
        await expect(save).toBeEnabled();
        await expect(error).toHaveText(
          `a recipe named ${existing} already exists`,
        );
        await expectSettingsRecipeErrorVisible(error);
        await info.attach(`settings-recipe-refused-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });

        await answers.getByRole("button", { name: "Cancel", exact: true })
          .click();
        const leaveQuestion = answers.getByText(
          "Save these changes before leaving?",
          { exact: true },
        );
        await expect(leaveQuestion).toBeVisible();
        const [leaveRefused] = await Promise.all([
          page.waitForResponse((response) =>
            response.request().method() === "POST" &&
            new URL(response.url()).pathname === "/api/tensorlake/recipes"
          ),
          save.click(),
        ]);
        expect(leaveRefused.status()).toBe(409);
        await expect(save).toBeEnabled();
        await expect(leaveQuestion).toBeVisible();
        await expect(error).toHaveText(
          `a recipe named ${existing} already exists`,
        );
        await expectSettingsRecipeErrorVisible(error);
        await expectSettingsRecipeAnswers(dialog, editor);
        await info.attach(`settings-recipe-leave-refused-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });
        await name.fill(saved);
        await expect(error).toHaveCount(0);
        await expect(leaveQuestion).toBeVisible();
        await answers.getByRole("button", { name: "Keep editing", exact: true })
          .click();

        for (
          const [position, fraction] of [["top", 0], ["middle", 0.5], [
            "bottom",
            1,
          ]] as const
        ) {
          const scroll = await scrollRecipeEditor(editor, fraction);
          expect(scroll.travel, "premise: fields exceed the scrollport")
            .toBeGreaterThan(100);
          expect(
            Math.abs(scroll.actual - scroll.top - scroll.travel * fraction),
            "the body reached the requested editor position",
          ).toBeLessThan(2);
          await expectSettingsRecipeAnswers(dialog, editor);
          await info.attach(`settings-recipe-${position}-${palette}`, {
            body: await dialog.screenshot(),
            contentType: "image/png",
          });
        }

        const finalField = editor.getByLabel("Codex endpoint token", {
          exact: true,
        });
        await expect(finalField).toBeEnabled();
        await finalField.focus();
        await expect(finalField).toBeFocused();
        await page.keyboard.press("Tab");
        await expect(
          editor.getByRole("button", { name: "Cancel", exact: true }),
        ).toBeFocused();
        await page.keyboard.press("Tab");
        await expect(editor.getByRole("button", { name: "Save", exact: true }))
          .toBeFocused();
        await editor.getByRole("button", { name: "Cancel", exact: true })
          .click();
        await expect(
          editor.getByText("Save these changes before leaving?", {
            exact: true,
          }),
        ).toBeVisible();
        await expectSettingsRecipeAnswers(dialog, editor);
        await info.attach(`settings-recipe-leave-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });
        await editor.getByRole("button", { name: "Keep editing", exact: true })
          .click();

        await selectSettingsSection(page, "sessions");
        // Role locators omit hidden regions. Inspect the still-mounted CSS
        // owner so a missing editor cannot masquerade as preserved hidden state.
        await expect(dialog.locator(".recipe-editor-sticky-actions"))
          .toHaveCount(1);
        await expect(dialog.locator(".recipe-editor-sticky-actions"))
          .toBeHidden();
        await expect(dialog.locator(".recipe-save")).toHaveCount(1);
        await expect(dialog.locator(".recipe-save")).toBeHidden();
        const close = dialog.locator(":scope > .dialog-footer").getByRole(
          "button",
          { name: "Close", exact: true },
        );
        await expect(close).toBeVisible();
        await close.click();
        await expect(editor).toBeVisible();
        await expect(
          editor.getByText("Save these changes before leaving?", {
            exact: true,
          }),
        ).toBeVisible();
        await expectSettingsRecipeAnswers(dialog, editor);
        await editor.getByRole("button", { name: "Keep editing", exact: true })
          .click();
        await expect(name).toHaveValue(saved);
        await close.click();
        await expect(
          editor.getByText("Save these changes before leaving?", {
            exact: true,
          }),
        ).toBeVisible();
        await editor.getByRole("button", { name: "Save", exact: true }).click();
        await expect(dialog).toHaveCount(0);
        expect((await recipes(request)).map((row) => row.name)).toContain(
          saved,
        );

        // Reuse the existing recipe to exercise delete's replacement answers;
        // the create/edit/delete test below already proves actual deletion.
        list = await openRecipes(page);
        await list.getByRole("button", {
          name: `Edit ${existing}`,
          exact: true,
        }).click();
        const deleting = dialog.getByRole("region", {
          name: `Recipe: ${existing}`,
          exact: true,
        });
        await expect(
          deleting.getByRole("heading", {
            name: `Recipe: ${existing}`,
            exact: true,
          }),
        ).toBeFocused();
        await expectSettingsRecipeAnswers(dialog, deleting);
        await deleting.getByRole("button", { name: "Delete", exact: true })
          .click();
        await expect(deleting.locator(".recipe-delete-confirm")).toContainText(
          `Delete recipe ${existing}?`,
        );
        await expectSettingsRecipeAnswers(dialog, deleting);
        await info.attach(`settings-recipe-delete-${palette}`, {
          body: await dialog.screenshot(),
          contentType: "image/png",
        });
        await deleting.getByRole("button", { name: "Cancel", exact: true })
          .click();
        await close.click();
        await expect(dialog).toHaveCount(0);
      } finally {
        await removeRecipes(request, [existing, saved]);
      }
    },
  );
}

/** Spec: a managed recipe created in the editor is stored with its size,
 * packages (split on spaces and new lines), tunnel, Claude Code endpoint and
 * GitHub selection. Pickers filter credential kinds; only names are returned. Reopening keeps
 * the explicit selection, including across a URL change, and deleting the
 * credential is refused until the recipe no longer uses it.
 *
 * Why: this is the whole create/edit/delete loop a user goes through, and the
 * named token is the part a regression would quietly break (a value echoed
 * into the page, or a selection cleared by an unrelated edit). */
test("a managed recipe is created, edited and deleted without exposing its token", async ({ page, request }, info) => {
  const name = "e2e-rust";
  const credentialName = "e2e-rust-endpoint";
  const githubName = "e2e-rust-github";
  const githubSecret = "e2e-private-github-marker";
  let ownsCredential = false;
  let ownsGithub = false;
  await page.goto("/");
  expect(
    (await recipes(request)).map((r) => r.name),
    "fixture premise: no leftover recipe",
  ).not.toContain(name);
  try {
    const created = await request.post("/api/credentials", { data: { name: credentialName, kind: "endpoint_token", secret: TOKEN } });
    expect(created.status(), await created.text()).toBe(201);
    ownsCredential = true;
    const githubCreated = await request.post("/api/credentials", { data: { name: githubName, kind: "github_token", secret: githubSecret } });
    expect(githubCreated.status(), await githubCreated.text()).toBe(201);
    ownsGithub = true;
    const list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    const editor = page.getByRole("region", {
      name: "New recipe",
      exact: true,
    });
    await expect(editor).toBeVisible();
    await editor.getByRole("textbox", { name: "Name", exact: true }).fill(name);
    const github = editor.getByLabel("GitHub token", { exact: true });
    await expect(github).toBeEnabled();
    await expect(github.locator("option")).toHaveText(["None", githubName]);
    await github.selectOption(githubName);
    await expect(github).toHaveValue(githubName);
    await github.scrollIntoViewIfNeeded();
    await expect(github).toBeInViewport();
    await info.attach("recipe-github-selection", { body: await page.getByRole("dialog", { name: "Settings", exact: true }).screenshot(), contentType: "image/png" });
    await expect(editor.getByRole("spinbutton", { name: /^CPUs/ })).toHaveValue(
      "2",
    );
    await expect(editor.getByRole("spinbutton", { name: /^Memory, GB/ }))
      .toHaveValue("4");
    await expect(editor.getByRole("spinbutton", { name: /^Disk, GB/ }))
      .toHaveValue("20");
    await expect(editor.getByRole("button", { name: /^Install packages/ }))
      .toHaveAttribute("aria-pressed", "true");
    await editor.getByRole("textbox", { name: "Ubuntu packages", exact: true })
      .fill("git ripgrep\njq git");
    await editor.getByRole("button", { name: "Add tunnel", exact: true })
      .click();
    const tunnel = editor.getByRole("group", { name: "Tunnel 1", exact: true });
    await tunnel.getByRole("spinbutton", { name: "Sandbox port", exact: true })
      .fill("8080");
    await tunnel.getByRole("textbox", { name: "Host", exact: true }).fill(
      "127.0.0.1",
    );
    await tunnel.getByRole("spinbutton", { name: "Port", exact: true }).fill(
      "8080",
    );
    const claude = editor.getByRole("group", {
      name: "Claude Code endpoint",
      exact: true,
    });
    await claude.getByRole("textbox", {
      name: "Claude Code base URL",
      exact: true,
    }).fill("http://127.0.0.1:8080");
    const picker = claude.getByLabel("Claude Code endpoint token", { exact: true });
    await expect(picker).toBeEnabled();
    await expect(picker.locator("option")).toHaveText(["None", credentialName]);
    await picker.selectOption(credentialName);
    await expect(
      editor.getByText(
        "Add or replace endpoint tokens in Settings → Credentials.",
        { exact: false },
      ),
    ).toBeVisible();
    await editor.getByRole("button", { name: "Save", exact: true }).click();

    // Saving returns to the list, which shows the stored recipe.
    const row = list.getByRole("listitem").filter({ hasText: name });
    await expect(row).toContainText(
      "2 CPU · 4 GB · 20 GB · 3 packages · 1 tunnel",
    );
    await expect(row).toContainText("No snapshot yet");
    const listed = await request.get("/api/tensorlake/recipes");
    const body = await listed.text();
    expect(body).not.toContain(TOKEN);
    expect(body).not.toContain(githubSecret);
    const stored = JSON.parse(body).recipes.find((r: any) => r.name === name);
    expect(stored).toEqual({
      name,
      size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
      source: { kind: "packages", packages: ["git", "ripgrep", "jq"] },
      claude: { base_url: "http://127.0.0.1:8080", token_credential: credentialName },
      codex: null,
      github_token_credential: githubName,
      tunnels: [{ sandbox_port: 8080, host: "127.0.0.1", port: 8080 }],
      snapshot: null,
      build: { state: "idle" },
      cleanup_pending: false,
    });
    expect(await page.content()).not.toContain(TOKEN);
    expect(await page.content()).not.toContain(githubSecret);

    const refusedDelete = await request.delete(`/api/credentials/${credentialName}`);
    expect(refusedDelete.status()).toBe(409);
    const refusedGithubDelete = await request.delete(`/api/credentials/${githubName}`);
    expect(refusedGithubDelete.status()).toBe(409);
    // Reopen: the recipe's name is fixed and the chosen identity is visible.
    await row.getByRole("button", { name: `Edit ${name}`, exact: true })
      .click();
    const reopened = page.getByRole("region", {
      name: `Recipe: ${name}`,
      exact: true,
    });
    await expect(reopened.getByRole("textbox", { name: "Name", exact: true }))
      .not.toBeEditable();
    const token = reopened.getByLabel("Claude Code endpoint token", {
      exact: true,
    });
    await expect(token).toHaveValue(credentialName);
    await expect(reopened.getByLabel("GitHub token", { exact: true })).toHaveValue(githubName);
    await reopened.getByLabel("Claude Code base URL", { exact: true }).fill("https://another.example/v1");
    await expect(
      reopened.getByRole("textbox", { name: "Ubuntu packages", exact: true }),
    ).toHaveValue("git\nripgrep\njq");
    await reopened.getByRole("spinbutton", { name: /^Disk, GB/ }).fill("30");
    await reopened.getByRole("button", { name: "Save", exact: true }).click();
    await expect(row).toContainText(
      "2 CPU · 4 GB · 30 GB · 3 packages · 1 tunnel",
    );
    const edited = (await recipes(request)).find((r) => r.name === name);
    expect(edited.size.disk_gb).toBe(30);
    expect(edited.claude.token_credential, "a URL change retains the explicit named choice").toBe(credentialName);
    expect(edited.github_token_credential).toBe(githubName);

    // Delete asks first, then removes the recipe.
    await row.getByRole("button", { name: `Edit ${name}`, exact: true })
      .click();
    const deleting = page.getByRole("region", {
      name: `Recipe: ${name}`,
      exact: true,
    });
    await deleting.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(
      deleting.getByText(`Delete recipe ${name}? This can't be undone.`, {
        exact: true,
      }),
    ).toBeVisible();
    expect(
      (await recipes(request)).map((r) => r.name),
      "asking deleted nothing yet",
    ).toContain(name);
    await deleting.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(list.getByRole("listitem").filter({ hasText: name }))
      .toHaveCount(0);
    expect((await recipes(request)).map((r) => r.name)).not.toContain(name);
  } finally {
    await removeRecipes(request, [name]);
    if (ownsCredential) {
      const removed = await request.delete(`/api/credentials/${credentialName}`);
      expect([200, 404], await removed.text()).toContain(removed.status());
    }
    if (ownsGithub) {
      const removed = await request.delete(`/api/credentials/${githubName}`);
      expect([200, 404], await removed.text()).toContain(removed.status());
    }
  }
});

/** Spec: the editor refuses an invalid name locally, shows the helm's refusal
 * for a name that already exists (without overwriting it), asks before Back
 * drops unsaved changes, and stores a reused image as "uses image" with the
 * systemd requirement spelled out in user terms.
 *
 * Why: "New recipe" must never overwrite another recipe, refusals must leave
 * the draft editable, and leaving with unsaved changes must not lose them
 * silently. The systemd wording is a user decision ("never the bare phrase
 * 'must boot systemd'"). */
test("the recipe editor refuses bad names, asks before leaving, and saves a reused image", async ({ page, request }) => {
  const existing = "e2e-existing";
  const reuse = "e2e-reuse";
  await page.goto("/");
  const before = (await recipes(request)).map((r) => r.name);
  expect(before, "fixture premise: no leftover recipes").not.toContain(
    existing,
  );
  expect(before).not.toContain(reuse);
  try {
    const created = await request.post("/api/tensorlake/recipes", {
      data: {
        name: existing,
        recipe: {
          size: { cpus: 1, memory_gb: 1, disk_gb: 10 },
          source: { kind: "packages", packages: [] },
          claude: null,
          codex: null,
          tunnels: [],
        },
      },
    });
    expect(created.ok(), await created.text()).toBe(true);

    const list = await openRecipes(page);
    await expect(list.getByRole("listitem").filter({ hasText: existing }))
      .toContainText("1 CPU · 1 GB · 10 GB · 0 packages");
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    const editor = page.getByRole("region", {
      name: "New recipe",
      exact: true,
    });
    const nameField = editor.getByRole("textbox", {
      name: "Name",
      exact: true,
    });

    await nameField.fill("Bad Name");
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(editor.getByRole("alert")).toContainText(
      "lowercase letters, digits and hyphens",
    );
    await expect(nameField).toHaveValue("Bad Name");

    await nameField.fill(existing);
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(editor.getByRole("alert")).toHaveText(
      `a recipe named ${existing} already exists`,
    );
    const kept = (await recipes(request)).find((r) => r.name === existing);
    expect(kept.size, "the existing recipe was not overwritten").toEqual({
      cpus: 1,
      memory_gb: 1,
      disk_gb: 10,
    });

    // Back with unsaved changes asks; Keep editing stays, Discard leaves.
    await editor.getByRole("button", { name: "‹ Recipes", exact: true })
      .click();
    await expect(
      editor.getByText("Save these changes before leaving?", { exact: true }),
    ).toBeVisible();
    await editor.getByRole("button", { name: "Keep editing", exact: true })
      .click();
    await expect(nameField).toHaveValue(existing);
    await editor.getByRole("button", { name: "‹ Recipes", exact: true })
      .click();
    await editor.getByRole("button", { name: "Discard", exact: true }).click();
    await expect(editor).toHaveCount(0);

    // A reused image: the systemd requirement in user terms, no packages.
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    await nameField.fill(reuse);
    await editor.getByRole("button", {
      name: /^Use an existing image or snapshot/,
    }).click();
    await expect(
      editor.getByRole("button", { name: "Image name", exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(
      editor.getByText(
        /Your image must have systemd installed, enabled, and running as the init system at startup/,
      ),
    ).toBeVisible();
    await expect(editor.getByText(/must boot systemd/)).toHaveCount(0);
    await editor.getByRole("textbox", { name: "Image", exact: true }).fill(
      "team-base",
    );
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    const row = list.getByRole("listitem").filter({ hasText: reuse });
    await expect(row).toContainText(
      "2 CPU · 4 GB · 20 GB · uses image team-base",
    );
    await expect(row).toContainText("Uses the image as is");
    expect((await recipes(request)).find((r) => r.name === reuse).source)
      .toEqual({ kind: "image", image: "team-base" });
  } finally {
    await removeRecipes(request, [existing, reuse]);
  }
});

/** Spec: with unsaved changes in the recipe editor, Escape and both of
 * Settings' close controls ask "Save these changes before leaving?" instead of
 * closing. Keep editing keeps the dialog and the draft; Discard closes it and
 * stores nothing; Save stores the recipe and then closes. Without changes,
 * Close closes at once.
 *
 * Why: the editor sits inside the Settings modal, so the modal's own
 * dismissal paths would otherwise unmount it and drop a draft (tokens
 * included) that the editor's Back button promises to ask about. */
test("closing Settings with an unsaved recipe asks first and finishes the close", async ({ page, request }) => {
  const discarded = "e2e-close-discard";
  const saved = "e2e-close-save";
  await page.goto("/");
  const before = (await recipes(request)).map((r) => r.name);
  expect(before, "fixture premise: no leftover recipes").not.toContain(
    discarded,
  );
  expect(before).not.toContain(saved);
  const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
  const question = dialog.getByText("Save these changes before leaving?", {
    exact: true,
  });
  try {
    // Escape: Keep editing stays, then Discard closes without storing.
    let list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    const editor = dialog.getByRole("region", {
      name: "New recipe",
      exact: true,
    });
    const nameField = editor.getByRole("textbox", {
      name: "Name",
      exact: true,
    });
    await nameField.fill(discarded);
    await nameField.press("Escape");
    await expect(question).toBeVisible();
    await expect(dialog).toBeVisible();
    await editor.getByRole("button", { name: "Keep editing", exact: true })
      .click();
    await expect(question).toHaveCount(0);
    await expect(nameField).toHaveValue(discarded);
    await nameField.press("Escape");
    await expect(question).toBeVisible();
    await editor.getByRole("button", { name: "Discard", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect(
      (await recipes(request)).map((r) => r.name),
      "Discard stored nothing",
    ).not.toContain(discarded);

    // The footer's Close: Save stores the recipe, then closes.
    list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    await nameField.fill(saved);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(question).toBeVisible();
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect((await recipes(request)).map((r) => r.name)).toContain(saved);

    // The header's close control asks the same; an unchanged editor closes at once.
    list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    await nameField.fill("e2e-close-header");
    await dialog.getByRole("button", { name: "Close settings", exact: true })
      .click();
    await expect(question).toBeVisible();
    await editor.getByRole("button", { name: "Discard", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    list = await openRecipes(page);
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    await expect(editor).toBeVisible();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
  } finally {
    await removeRecipes(request, [discarded, saved, "e2e-close-header"]);
  }
});

/** Opening a recipe must yield to a field chosen before its deferred handoff.
 * Holding the real animation frame exposes the interval in which fast typing
 * can start: a mount-time heading focus must not redirect subsequent input or
 * scroll away from that newer field. The enclosing close still asks about the
 * draft that the user entered. */
test("a recipe opening preserves a newer field focus before header close", async ({ page, request }) => {
  const name = "e2e-focus-handoff";
  await page.goto("/");
  const list = await openRecipes(page);
  const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
  const editor = dialog.getByRole("region", { name: "New recipe", exact: true });
  const field = editor.getByRole("textbox", { name: "Name", exact: true });
  const body = dialog.locator(":scope > .host-settings-body");
  await page.clock.install();
  await page.clock.pauseAt(new Date(Date.now() + 5_000));
  try {
    await list.getByRole("button", { name: "New recipe", exact: true }).click();
    await expect(editor, "premise: the editor mounts while its handoff frame is held").toBeVisible();
    await expect(editor.locator(".recipe-editor-title"), "premise: the opening frame has not yet run").not.toBeFocused();
    await field.focus();
    await expect(field, "premise: the user chose Name before the opening handoff").toBeFocused();
    const scrollTop = await body.evaluate((node) => node.scrollTop);
    // Release the actual opening frame after the newer focus choice, without
    // waiting for or replacing the product's focus code.
    await page.clock.runFor(50);
    await expect(field, "the delayed opening must yield to the chosen field").toBeFocused();
    expect(await body.evaluate((node) => node.scrollTop), "the handoff must also preserve the chosen field's viewport")
      .toBe(scrollTop);
    await page.clock.resume();
    await page.keyboard.insertText(name);
    await expect(field).toHaveValue(name);
    await dialog.getByRole("button", { name: "Close settings", exact: true }).click();
    await expect(dialog.getByText("Save these changes before leaving?", { exact: true })).toBeVisible();
    await editor.getByRole("button", { name: "Discard", exact: true }).click();
    await expect(dialog).toHaveCount(0);
  } finally {
    await page.clock.resume();
    await removeRecipes(request, [name]);
  }
});

/** Spec: when a re-read of the recipe list fails after an earlier read
 * succeeded, the list keeps its rows, says it may be out of date and offers
 * Retry loading; a successful retry shows the current recipes and clears the
 * error.
 *
 * Why: the invalidation-feed notice that triggered the failed read is spent,
 * so without a visible error and Retry the list would stay stale with no way
 * to recover short of reopening Settings. */
test("a failed recipe refresh says so and Retry loading recovers", async ({ page, request }) => {
  const name = "e2e-refresh";
  await page.goto("/");
  expect(
    (await recipes(request)).map((r) => r.name),
    "fixture premise: no leftover recipe",
  ).not.toContain(name);
  try {
    const list = await openRecipes(page);
    // Recipe rows carry their own snapshot status; the list-level ones are
    // the loading line and the refresh error.
    await expect(list.getByText("Loading recipes…", { exact: true }))
      .toHaveCount(0);
    await expect(list.getByRole("alert")).toHaveCount(0);
    // Fail every browser read from now on; the API fixture below is not routed.
    let failing = true;
    let failedReads = 0;
    await page.route("**/api/tensorlake/recipes", async (route) => {
      if (failing && route.request().method() === "GET") {
        failedReads++;
        await route.fulfill({ status: 500, body: "recipe store unavailable" });
      } else {
        await route.continue();
      }
    });
    // Another client's create bumps the feed, which makes this list re-read.
    const created = await request.post("/api/tensorlake/recipes", {
      data: {
        name,
        recipe: {
          size: { cpus: 1, memory_gb: 1, disk_gb: 10 },
          source: { kind: "packages", packages: [] },
          claude: null,
          codex: null,
          tunnels: [],
        },
      },
    });
    expect(created.ok(), await created.text()).toBe(true);
    await expect(list.getByRole("alert")).toContainText(
      "Could not refresh the recipes, so this list may be out of date",
    );
    expect(failedReads, "the refresh went through the failing route")
      .toBeGreaterThan(0);
    await expect(list.getByRole("listitem").filter({ hasText: name }))
      .toHaveCount(0);
    failing = false;
    await list.getByRole("button", { name: "Retry loading", exact: true })
      .click();
    await expect(list.getByRole("listitem").filter({ hasText: name }))
      .toBeVisible();
    await expect(list.getByRole("alert")).toHaveCount(0);
  } finally {
    await page.unroute("**/api/tensorlake/recipes");
    await removeRecipes(request, [name]);
  }
});

// ---------------------------------------------------------------------------
// Snapshot builds, against the Tensorlake fake
// ---------------------------------------------------------------------------

type FakeShell = { hold?: boolean; fail?: Record<string, string> };
type FakeModel = {
  sandboxes: {
    id: string;
    name: string;
    status: string;
    image: string | null;
    auto_resume: boolean;
  }[];
  snapshots: { id: string; sandbox_id: string; status: string }[];
};

/** The fake's control directory, read once Playwright has started the stack. */
async function fakeRoot(): Promise<string> {
  const raw = await readFile(currentStackSlot().stackInfoPath, "utf8");
  return (JSON.parse(raw) as { tensorlake_fake: string }).tensorlake_fake;
}

/** Atomically publish how the fake's shell behaves. */
async function configureShell(shell: FakeShell): Promise<void> {
  const root = await fakeRoot();
  const next = path.join(root, `config.${process.pid}.next`);
  await writeFile(next, `${JSON.stringify({ shell })}\n`, { mode: 0o600 });
  await rename(next, path.join(root, "config.json"));
}

/** Empty the fake's model and event log. */
async function resetFake(): Promise<void> {
  const root = await fakeRoot();
  await writeFile(path.join(root, "state.json"), "{}\n", { mode: 0o600 });
  await writeFile(path.join(root, "events.jsonl"), "", { mode: 0o600 });
}

/** What the fake holds: every sandbox and snapshot the helm created. */
async function fakeModel(): Promise<FakeModel> {
  const raw = await readFile(path.join(await fakeRoot(), "state.json"), "utf8");
  const model = JSON.parse(raw) as Partial<FakeModel>;
  return { sandboxes: model.sandboxes ?? [], snapshots: model.snapshots ?? [] };
}

/** The shell steps the fake ran, in order. */
async function shellSteps(): Promise<string[]> {
  const raw = await readFile(
    path.join(await fakeRoot(), "events.jsonl"),
    "utf8",
  );
  return raw
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line) as { op: string; label?: string })
    .filter((event) => event.op === "shell")
    .map((event) => event.label ?? "");
}

/** The recipe's view as the helm lists it. */
async function recipe(request: APIRequestContext, name: string): Promise<any> {
  const found = (await recipes(request)).find((r) => r.name === name);
  expect(found, `recipe ${name} is listed`).toBeTruthy();
  return found;
}

/** Spec: with a key saved, a managed recipe without a snapshot offers Build
 * snapshot; while a build runs the row says which step it is on ("step n of
 * 9") and offers no action; once it ends the row says when it was built and
 * with which agents, the fake shows the builder terminated, only the recorded
 * snapshot left, and the steps run in order (ready, packages, claude, codex,
 * prepare). Rebuild records a new snapshot and deletes the old one. A failing
 * step shows its message with Retry and keeps the built snapshot; Retry
 * builds. Deleting the recipe deletes its snapshot.
 *
 * Why: this is the user's whole loop with snapshots, and the fake's state is
 * the evidence that nothing the helm created is left billing; a regression
 * in the cleanup would otherwise be invisible until a Tensorlake bill. */
test("a recipe's snapshot builds, rebuilds, recovers from a failed step and goes with the recipe", async ({ page, request }) => {
  const name = "e2e-build";
  const original = await (await request.get("/api/tensorlake")).json();
  expect(
    original.api_key_set,
    "fixture premise: no key it would have to restore",
  ).toBe(false);
  await page.goto("/");
  expect(
    (await recipes(request)).map((r) => r.name),
    "fixture premise: no leftover recipe",
  ).not.toContain(name);
  await resetFake();
  await configureShell({ hold: true });
  try {
    let response = await request.put("/api/tensorlake", {
      data: { api_key: "e2e-build-key" },
    });
    expect(response.ok(), await response.text()).toBe(true);
    response = await request.post("/api/tensorlake/ssh-key");
    expect(response.ok(), await response.text()).toBe(true);
    response = await request.post("/api/tensorlake/recipes", {
      data: {
        name,
        recipe: {
          size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
          source: { kind: "packages", packages: ["jq"] },
          claude: null,
          codex: null,
          tunnels: [],
        },
      },
    });
    expect(response.ok(), await response.text()).toBe(true);
    const premise = await fakeModel();
    expect(
      [premise.sandboxes.length, premise.snapshots.length],
      "fixture premise: the fake holds nothing",
    ).toEqual([0, 0]);

    const list = await openRecipes(page);
    const row = list.getByRole("listitem").filter({ hasText: name });
    await expect(row).toContainText("No snapshot yet");
    await row.getByRole("button", {
      name: `Build snapshot for ${name}`,
      exact: true,
    }).click();
    await expect(row).toContainText(
      "Building snapshot: connecting to the build sandbox (step 4 of 9)",
    );
    await expect(row.getByRole("button", { name: /snapshot for|Retry/ }))
      .toHaveCount(0);

    await configureShell({});
    await expect(row).toContainText("Snapshot built");
    await expect(row).toContainText("1.0.0 (Claude Code) · codex-cli 0.1.0");
    // "Snapshot built" shows once the snapshot is recorded, which is before
    // the builder is deleted; the build is over only once it is idle again.
    await expect(
      row.getByRole("button", {
        name: `Rebuild snapshot for ${name}`,
        exact: true,
      }),
    ).toBeEnabled();
    await expect.poll(async () => {
      const view = await recipe(request, name);
      return [view.build.state, view.cleanup_pending];
    }).toEqual(["idle", false]);
    const first = (await recipe(request, name)).snapshot.id as string;
    let model = await fakeModel();
    expect(model.sandboxes.map((s) => [s.status, s.image, s.auto_resume]))
      .toEqual([["terminated", "tensorlake/ubuntu-systemd", false]]);
    expect(model.sandboxes[0].name).toMatch(
      /^farhelm-build-[a-z0-9]{4}-e2e-build$/,
    );
    expect(model.snapshots.map((s) => s.id)).toEqual([first]);
    expect(await shellSteps()).toEqual([
      "ready",
      "packages",
      "claude",
      "codex",
      "prepare",
    ]);

    // Rebuild: a new snapshot, the old one deleted.
    await row.getByRole("button", {
      name: `Rebuild snapshot for ${name}`,
      exact: true,
    }).click();
    await expect.poll(async () => (await recipe(request, name)).snapshot?.id)
      .not.toBe(first);
    await expect.poll(async () => (await recipe(request, name)).build.state)
      .toBe("idle");
    await expect(
      row.getByRole("button", {
        name: `Rebuild snapshot for ${name}`,
        exact: true,
      }),
    ).toBeEnabled();
    const second = (await recipe(request, name)).snapshot.id as string;
    model = await fakeModel();
    expect(model.snapshots.map((s) => s.id)).toEqual([second]);
    expect(model.sandboxes.map((s) => s.status)).toEqual([
      "terminated",
      "terminated",
    ]);

    // A failing step: its message and Retry, with the built snapshot kept.
    await configureShell({
      fail: { packages: "E: Unable to locate package nope" },
    });
    await row.getByRole("button", {
      name: `Rebuild snapshot for ${name}`,
      exact: true,
    }).click();
    await expect(row).toContainText(
      "Snapshot build failed while installing packages: E: Unable to locate package nope",
    );
    await expect(row).toContainText("Snapshot built");
    expect((await recipe(request, name)).snapshot.id).toBe(second);
    model = await fakeModel();
    expect(
      model.snapshots.map((s) => s.id),
      "the failed build left no snapshot",
    ).toEqual([second]);
    expect(
      model.sandboxes.every((s) => s.status === "terminated"),
      "the failed builder was terminated",
    ).toBe(true);
    await configureShell({});
    await row.getByRole("button", { name: `Retry for ${name}`, exact: true })
      .click();
    await expect.poll(async () => (await recipe(request, name)).snapshot?.id)
      .not.toBe(second);
    await expect.poll(async () => (await recipe(request, name)).build.state)
      .toBe("idle");
    await expect(row).not.toContainText("failed");

    // Deleting the recipe deletes its snapshot, after saying so.
    await row.getByRole("button", { name: `Edit ${name}`, exact: true })
      .click();
    const editor = page.getByRole("region", {
      name: `Recipe: ${name}`,
      exact: true,
    });
    await editor.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(
      editor.getByText(
        `Delete recipe ${name} and its snapshot on Tensorlake? This can't be undone.`,
        { exact: true },
      ),
    ).toBeVisible();
    await editor.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(list.getByRole("listitem").filter({ hasText: name }))
      .toHaveCount(0);
    model = await fakeModel();
    expect(model.snapshots).toEqual([]);
    expect(model.sandboxes.every((s) => s.status === "terminated")).toBe(true);
  } finally {
    await configureShell({});
    await removeRecipes(request, [name]);
    const cleared = await request.put("/api/tensorlake", {
      data: { clear_api_key: true },
    });
    expect(cleared.ok(), await cleared.text()).toBe(true);
    await resetFake();
  }
});
