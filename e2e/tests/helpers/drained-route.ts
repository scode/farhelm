import type { Page, Route } from "@playwright/test";

/** Install a response rewrite whose cleanup waits for its own callback work.
 *
 * Playwright 1.64's `unrouteAll({ behavior: "wait" })` can deadlock after a
 * callback catches a channel error during fulfillment:
 * its internal handling promise stays pending even though our callback ended.
 * Track the work directly, stop admitting rewrites, then remove only this
 * handler. New reads fall through while the existing reads drain, so removing
 * interception cannot dispose a response still being inspected by this helper.
 * Stop is idempotent. Call it in finally, before the page/context closes:
 * leaving this handler for a later wait-style unroute retains the deadlock.
 */
export async function drainedRoute(
  page: Page,
  matches: Parameters<Page["route"]>[0],
  rewrite: (route: Route) => Promise<void>,
): Promise<() => Promise<void>> {
  let stopped = false;
  const inFlight = new Set<Promise<void>>();
  const handler = async (route: Route) => {
    if (stopped) return route.fallback();
    const work = rewrite(route);
    inFlight.add(work);
    try {
      await work;
    } catch (error) {
      // A route can lose ownership while route.fetch reads its independent
      // reply. Only these known lifecycle errors are moot;
      // malformed fixture data and ordinary fetch failures must still fail.
      const cancelled = error instanceof Error
        && /Route is already handled|Target page, context or browser has been closed|Test ended/.test(error.message);
      if (!cancelled) throw error;
    } finally {
      inFlight.delete(work);
    }
  };
  await page.route(matches, handler);
  return async () => {
    stopped = true;
    await Promise.allSettled([...inFlight]);
    await page.unroute(matches, handler);
  };
}
