import { expect, type Locator, type Page } from "@playwright/test";

/** Return a real, keyboard-reachable filter button, including All's escape. */
export function hostChoice(page: Page, host?: number | string): Locator {
  return host === undefined || host === ""
    ? page.locator(".host-rail-all")
    : page.locator(`.host-rail-avatar[data-rail-host-id="${host}"]`);
}

/** Observe the applied host choice without changing focus or opening menus. */
export function selectedHost(page: Page): Locator {
  return page.locator('.host-rail button[aria-pressed="true"]');
}

/** Exercise the actual rail activation; selected state is its local oracle.
 * Callers still await their own listing reply or rows when those matter. */
export async function chooseHost(page: Page, host?: number | string): Promise<void> {
  const choice = hostChoice(page, host);
  await expect(choice).toBeVisible({ timeout: 20_000 });
  await choice.click();
  await expect(choice).toHaveAttribute("aria-pressed", "true");
}
