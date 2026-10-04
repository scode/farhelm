/**
 * The starting points every docs page's shots share: the staged fleet opened
 * with the docs overlay installed, and the session launcher opened from it.
 * Kept here so a change in how the UI opens these moves once for every page.
 */
import { expect, type APIRequestContext, type Page } from "@playwright/test";
import { Director } from "../readme-video/overlay";
import { loadScenario } from "../readme-hero/scenario";
import { DOCS_SHOTS_DIR } from "./paths";
import { DOCS_THEME, openDocsFleet } from "./stage-docs";

/** Open the staged fleet with the docs overlay installed. */
export async function open(page: Page, request: APIRequestContext) {
  const scenario = loadScenario(DOCS_SHOTS_DIR);
  await openDocsFleet(page, request, scenario);
  return { scenario, director: await Director.install(page, DOCS_THEME) };
}

/** Open the session launcher from the session list and wait for it to show. */
export async function openLauncher(page: Page) {
  await page.locator(".new-session-button").click();
  const form = page.locator("form.create-session-form");
  await expect(form).toBeVisible();
  return form;
}
