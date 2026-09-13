/**
 * Produces the F20 visual acceptance matrix from the ordinary isolated E2E
 * stack. This is evidence, not a product fixture: every capture routes a
 * complete catalog and history before navigation so the image names the state
 * it actually shows instead of inheriting another test's remembered launches.
 *
 * The matrix deliberately keeps long near-identical destinations, defaults,
 * and a Claude recent together. Those are the combinations whose distinction
 * is easy to lose at narrow width, while a screenshot alone cannot prove the
 * route or launch lifecycle behind them.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator, type Page } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const sharedPrefix = "/workspace/very-long-shared-prefix/launch-composer/acceptance/";
const launches = [
  {
    host: 1, canonical_cwd: "/canonical/visual-recents", cwd: `${sharedPrefix}alpha-the-ending-that-matters`,
    selection: { harness: "codex", model: "gpt-6-astra", effort: "high", permissions: "yolo" }, created_at: 30, creation_seq: 30,
  },
  {
    host: 1, canonical_cwd: "/canonical/visual-recents", cwd: `${sharedPrefix}bravo-the-different-ending`,
    selection: { harness: "codex", model: null, effort: null, permissions: null }, created_at: 20, creation_seq: 20,
  },
  {
    host: 1, canonical_cwd: "/canonical/visual-recents", cwd: `${sharedPrefix}claude-complete-recent`,
    selection: { harness: "claude", model: "claude-sonnet", effort: "low", permissions: null }, created_at: 10, creation_seq: 10,
  },
];
const models = [
  { id: "gpt-6-astra", harness: "codex", efforts: ["low", "medium", "high"] },
  { id: "gpt-5.6-terra", harness: "codex", efforts: ["low", "medium", "high"] },
  { id: "claude-sonnet", harness: "claude", efforts: ["low", "medium", "high"] },
];
const folders = launches.map((entry) => ({
  host: 1, canonical_cwd: entry.canonical_cwd, canonical_proven: true, display_cwd: entry.cwd,
  created_at: entry.created_at, creation_seq: entry.creation_seq,
}));

type Capture = { file: string; viewport: { width: number; height: number }; state: string; ready: string[]; scrollTop: number };

/** Route the same complete source of truth every capture needs before opening the form. */
async function installVisualFixture(page: Page) {
  const build = (await page.request.get("/api/sessions")).headers()["x-farhelm-build"] ?? "";
  expect(build, "the visual fixture must retain the isolated stack build identity").toBeTruthy();
  await page.route("**/api/launch-catalog", (route) => route.fulfill({
    status: 200, headers: { "content-type": "application/json", "x-farhelm-build": build }, body: JSON.stringify(models),
  }));
  await page.route("**/api/launch-history**", (route) => route.fulfill({
    status: 200, headers: { "content-type": "application/json", "x-farhelm-build": build },
    body: JSON.stringify({ launches, folders }),
  }));
}

/** Reopen a fresh form so each image has one named, reproducible precondition. */
async function openComposer(page: Page): Promise<Locator> {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByLabel("folder", { exact: true }).fill(launches[0].cwd);
  await expect(form.locator(".launch-composer-recent-slots > button")).toHaveCount(3);
  return form;
}

test("F20 visual capture matrix", async ({ page, browserName }, testInfo) => {
  test.skip(browserName !== "chromium", "one current Chromium matrix is the reviewed pixel evidence; behavior runs in both engines elsewhere");
  // The runner owns this directory and supplies it to the test. Captures do
  // not assume an operator's home or execution-artifact path, and a later
  // evidence collector can copy this complete private directory unchanged.
  const output = testInfo.outputPath("f20-visual-captures");
  mkdirSync(output, { recursive: true });
  await installVisualFixture(page);
  const records: Capture[] = [];

  async function capture(
    state: string,
    viewport: { width: number; height: number },
    prepare: (form: Locator) => Promise<void>,
    ready: string[],
    scroll = false,
  ) {
    await page.setViewportSize(viewport);
    const form = await openComposer(page);
    await prepare(form);
    if (scroll) {
      await form.evaluate((node) => { node.scrollTop = node.scrollHeight; });
      await expect.poll(() => form.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
    }
    const scrollTop = await form.evaluate((node) => node.scrollTop);
    const file = `${viewport.width}x${viewport.height}-${state}.png`;
    await page.screenshot({ path: join(output, file) });
    records.push({ file, viewport, state, ready, scrollTop });
  }

  const desktop = { width: 1280, height: 900 };
  const narrow = { width: 390, height: 844 };
  const selectExplicit = async (form: Locator) => {
    await form.getByRole("button", { name: "gpt-6-astra (Codex)", exact: true }).click();
    await form.locator(".launch-composer-effort-choice").getByRole("button", { name: "high", exact: true }).click();
    await form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "YOLO", exact: true }).click();
  };
  const prefillSavedDefaults = async (form: Locator) => {
    // Ordinary recents honour the complete explicit draft and therefore
    // correctly hide this default-valued entry. Search is the deliberate
    // whole-draft replacement surface, so it is the only honest way to show
    // saved defaults replacing explicit choices here.
    await form.locator('.launch-composer-search input[role="combobox"]').fill("bravo-the-different-ending");
    const recent = form.getByRole("group", { name: "Recent setups" }).getByRole("option");
    await expect(recent).toHaveCount(1);
    await recent.click();
  };
  const prefillCompleteRecent = async (form: Locator) => {
    // This is deliberately separate from the default-over-explicit state:
    // reviewers need to see an ordinary complete setup update the summary
    // while the action row, heading, and first saved row stay together.
    await form.locator('.launch-composer-search input[role="combobox"]').fill("alpha-the-ending-that-matters");
    const recent = form.getByRole("group", { name: "Recent setups" }).getByRole("option");
    await expect(recent, "the controlled search must expose the complete Astra/High/YOLO setup").toHaveCount(1);
    await recent.click();
  };

  await capture("three-recents-defaults", desktop, async (form) => {
    await expect(form.getByTitle(new RegExp("alpha-the-ending-that-matters"))).toBeVisible();
    await expect(form.getByTitle(new RegExp("bravo-the-different-ending"))).toBeVisible();
  }, ["three 36px recent rows", "two long shared-prefix destinations", "default and explicit selections"]);
  await capture("claude-search-recents", desktop, async (form) => {
    await form.locator('.launch-composer-search input[role="combobox"]').fill("Claude");
    await expect(form.getByRole("group", { name: "Harnesses" })).toBeVisible();
    await expect(form.getByRole("group", { name: "Recent setups" })).toBeVisible();
  }, ["Claude Harnesses group", "Claude Recent setups group"]);
  await capture("prefill-default-over-explicit", desktop, async (form) => {
    await selectExplicit(form);
    await prefillSavedDefaults(form);
    await expect(form.locator(".launch-composer-summary")).toContainText("model: default · effort: default · permissions: default");
  }, ["explicit Codex draft", "saved default recent applied without launch"]);
  await capture("keyboard-active-hover", desktop, async (form) => {
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill("Claude");
    const options = form.getByRole("option");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    const activeId = await search.getAttribute("aria-activedescendant");
    await expect(form.locator(`#${activeId}`)).toHaveAttribute("aria-selected", "true");
    await options.nth(0).hover();
  }, ["keyboard active descendant", "different pointer-hovered option"]);
  await capture("three-recents-defaults", narrow, async (form) => {
    await expect(form.getByTitle(new RegExp("alpha-the-ending-that-matters"))).toBeVisible();
  }, ["three 36px recent rows", "one-line rows truncate long detail without launch"]);
  await capture("explicit-long-folder", narrow, async (form) => {
    await selectExplicit(form);
    await form.evaluate((node) => { node.scrollTop = 0; });
    await expect.poll(() => form.evaluate((node) => node.scrollTop)).toBe(0);
    // The caption claims the explicit choices are visible; prove the draft
    // holds them before the screenshot rather than trusting the clicks.
    await expect(form.locator(".launch-composer-summary")).toHaveText("model: gpt-6-astra · effort: high · permissions: yolo");
    await expect(form.getByText("recent setups", { exact: true })).toBeVisible();
    await expect(form.locator(".launch-composer-recent-slots > button").first()).toBeVisible();
  }, ["Codex/Astra/High/YOLO summary", "recent heading and first row at scrollTop 0"]);
  await capture("complete-recent-prefill-upper-strip", narrow, async (form) => {
    await prefillCompleteRecent(form);
    await form.evaluate((node) => { node.scrollTop = 0; });
    await expect.poll(() => form.evaluate((node) => node.scrollTop)).toBe(0);
    await expect(form.locator(".launch-composer-summary")).toHaveText("model: gpt-6-astra · effort: high · permissions: yolo");
    await expect(form.getByText("recent setups", { exact: true })).toBeVisible();
    await expect(form.locator(".launch-composer-recent-slots > button").first()).toBeVisible();
  }, ["complete recent prefill", "recent heading and first row at scrollTop 0"]);
  await capture("focus-wrap-launch-visible", narrow, async (form) => {
    // Launch no longer renders last — the action row moved to the top of the
    // dialog — so the wrap boundary is whatever the trap's own query finds
    // last, found the same way `install_composer_focus_trap` does rather
    // than assuming a control name.
    const launch = form.getByRole("button", { name: /^launch\b/ });
    await selectExplicit(form);
    await form.evaluate((dialog) => {
      const nodes = [...dialog.querySelectorAll(
        'button:not([disabled]), input:not([disabled]), select:not([disabled]), summary, [tabindex]:not([tabindex="-1"])',
      )].filter((node) => !(node as HTMLElement).hidden && node.getClientRects().length);
      nodes[nodes.length - 1].setAttribute("data-focus-trap-last-probe", "true");
    });
    const last = form.locator('[data-focus-trap-last-probe="true"]');
    await expect(last, "the trap's own last node must be a real focus boundary").toBeVisible();
    await last.focus();
    await form.evaluate((node) => { node.scrollTop = node.scrollHeight; });
    await expect.poll(() => form.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
    await page.keyboard.press("Tab");
    await expect(launch).toBeFocused();
    await expect.poll(() => launch.evaluate((node) => {
      const target = node.getBoundingClientRect();
      const viewport = node.closest(".create-session-form")!.getBoundingClientRect();
      return target.top < viewport.bottom && target.bottom > viewport.top;
    })).toBe(true);
  }, ["Tab wraps from the trap's last control", "Launch focused", "wrapped target visible in the composer viewport"]);
  await capture("prefill-default-over-explicit", narrow, async (form) => {
    await selectExplicit(form);
    await prefillSavedDefaults(form);
    await expect(form.locator(".launch-composer-summary")).toContainText("model: default · effort: default · permissions: default");
  }, ["saved defaults replace explicit draft", "composer remains editable"], true);
  await capture("claude-search-recents", narrow, async (form) => {
    await form.locator('.launch-composer-search input[role="combobox"]').fill("Claude");
    await expect(form.getByRole("group", { name: "Recent setups" })).toBeVisible();
  }, ["Claude harness and complete recent search results"]);
  await capture("keyboard-active-hover", narrow, async (form) => {
    const search = form.locator('.launch-composer-search input[role="combobox"]');
    await search.fill("Claude");
    await page.keyboard.press("ArrowDown");
    const activeId = await search.getAttribute("aria-activedescendant");
    await expect(form.locator(`#${activeId}`)).toHaveAttribute("aria-selected", "true");
    await form.getByRole("option").nth(0).hover();
  }, ["keyboard active result differs from pointer hover"]);

  writeFileSync(join(output, "manifest.json"), `${JSON.stringify({
    source: "e2e/tests/f20_visual_capture.spec.ts",
    engine: browserName,
    captures: records,
    note: "Each image used a fresh mounted composer after both controlled route replies were installed.",
  }, null, 2)}\n`);
});
