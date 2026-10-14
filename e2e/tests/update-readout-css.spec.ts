/**
 * Desktop update affordances use the shipped stylesheet and bundled font.
 * The web app has no updater to put this button in its bar, so isolate the
 * native control's CSS contract here; its real markup/click dispatch has a
 * headless Rust render test. No helm or installed app is needed for this proof.
 */
import { test, expect } from "./helpers/evidence";
import type { Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ASSETS = join(__dirname, "../../crates/farhelm-ui/assets");
const FIXTURE_URL = "http://readout-fixture.invalid/";

/** Load real CSS/fonts before measuring, so fallback glyphs cannot satisfy the premise. */
async function mountReadout(page: Page, spinning: boolean): Promise<void> {
  const css = readFileSync(join(ASSETS, "app.css"), "utf8");
  const html = `<!doctype html><meta charset="utf-8"><style>${css}</style>
    <button id="parking">parking</button>
    <div class="app-bar" style="margin:20px;width:320px">
      <button type="button" class="app-version app-version-check" aria-label="check for updates">
        <span class="app-version-text">v0.28.2</span>
        <span class="app-version-refresh ${spinning ? "app-version-refresh-spinning" : ""}">↻</span>
      </button>
      <span class="app-update-status">${spinning ? "checking…" : ""}</span>
    </div>
    <span id="normal" style="color:var(--fg-1)"></span>
    <span id="dim" style="color:var(--fg-2)"></span>
    <span id="bright" style="color:var(--fg-0)"></span>`;
  await page.route(`${FIXTURE_URL}**`, async (route) => {
    const name = new URL(route.request().url()).pathname.split("/").at(-1)!;
    if (name.endsWith(".woff2")) {
      await route.fulfill({ body: readFileSync(join(ASSETS, "fonts", name)), contentType: "font/woff2" });
    } else await route.fulfill({ body: html, contentType: "text/html; charset=utf-8" });
  });
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.goto(FIXTURE_URL);
  // The app uses standards mode; a bare fragment can put :hover matching
  // under quirks rules before this test ever reaches the stylesheet's contract.
  expect(await page.evaluate(() => document.compatMode)).toBe("CSS1Compat");
  await page.evaluate(() => document.fonts.ready);
  expect(await page.evaluate(() => document.fonts.check('12px "JetBrains Mono"', "↻"))).toBe(true);
  expect(await page.locator(".app-version-refresh").textContent()).toBe("↻");
}

/** Read bounded computed values; assertion polling waits for CSS transitions, not elapsed time. */
async function readoutStyle(page: Page) {
  return page.locator(".app-version-check").evaluate((button) => {
    const glyph = button.querySelector(".app-version-refresh")!;
    const style = getComputedStyle(glyph);
    const matrix = new DOMMatrix(style.transform === "none" ? undefined : style.transform);
    return {
      version: getComputedStyle(button).color,
      glyph: style.color,
      transform: style.transform,
      quarterTurn: Math.abs(matrix.a) < 0.001 && Math.abs(matrix.b - 1) < 0.001,
      animation: style.animationName,
    };
  });
}

/** Resolve the CSS tokens through the engine instead of duplicating the palette in the test. */
async function palette(page: Page) {
  return page.evaluate(() => Object.fromEntries(["normal", "dim", "bright"].map((name) =>
    [name, getComputedStyle(document.getElementById(name)!).color]
  )));
}

/** The persistent hint stays dim at rest and responds identically to pointer and keyboard discovery. */
test("version hint brightens and turns on hover and keyboard focus", async ({ page }) => {
  await mountReadout(page, false);
  const button = page.getByRole("button", { name: "check for updates", exact: true });
  const colors = await palette(page);
  await page.mouse.move(0, 0);
  await expect(button).not.toBeFocused();
  expect(await button.evaluate((el) => el.matches(":hover"))).toBe(false);
  expect(await readoutStyle(page)).toMatchObject({ version: colors.normal, glyph: colors.dim, quarterTurn: false });

  await button.hover();
  await expect.poll(() => button.evaluate((el) => el.matches(":hover"))).toBe(true);
  await expect.poll(() => readoutStyle(page)).toMatchObject({
    version: colors.bright, glyph: colors.bright, quarterTurn: true, animation: "none",
  });

  await page.mouse.move(0, 0);
  await page.locator("#parking").focus();
  await page.keyboard.press("Tab");
  await expect(button).toBeFocused();
  await expect.poll(() => button.evaluate((el) => el.matches(":focus-visible") && !el.matches(":hover"))).toBe(true);
  await expect.poll(() => readoutStyle(page)).toMatchObject({
    version: colors.bright, glyph: colors.bright, quarterTurn: true, animation: "none",
  });
});

/** Progress retains its own motion through hover/focus, and reduced motion keeps its words without a spin. */
test("visible check owns the refresh spin and honors reduced motion", async ({ page }) => {
  await mountReadout(page, true);
  const button = page.getByRole("button", { name: "check for updates", exact: true });
  await expect(page.locator(".app-update-status")).toHaveText("checking…");
  expect((await readoutStyle(page)).animation).toBe("farhelm-delete-spin");
  await button.hover();
  const hovered = (await readoutStyle(page)).transform;
  await expect.poll(async () => (await readoutStyle(page)).transform).not.toBe(hovered);
  expect((await readoutStyle(page)).animation).toBe("farhelm-delete-spin");
  await page.mouse.move(0, 0);
  await page.locator("#parking").focus();
  await page.keyboard.press("Tab");
  await expect(button).toBeFocused();
  const focused = (await readoutStyle(page)).transform;
  await expect.poll(async () => (await readoutStyle(page)).transform).not.toBe(focused);
  expect((await readoutStyle(page)).animation).toBe("farhelm-delete-spin");

  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect.poll(() => readoutStyle(page)).toMatchObject({ animation: "none", transform: "none" });
  await expect(page.locator(".app-update-status")).toHaveText("checking…");
});
