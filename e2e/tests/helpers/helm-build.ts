import { expect, type APIResponse } from "@playwright/test";

/**
 * Read the live helm's build stamp for a fixture that will author replies.
 *
 * Route fixtures stand in for the helm, so their identity must come from a
 * real response in the same test stack. Keeping extraction and the missing-
 * stamp failure here prevents a version bump from turning a hardcoded test
 * value into a latched build-skew failure somewhere unrelated.
 */
export function requireHelmBuild(response: APIResponse, consumer: string): string {
  const stamp = response.headers()["x-farhelm-build"]?.trim() ?? "";
  expect(stamp, `${consumer}: the live helm must stamp its replies`).toBeTruthy();
  return stamp;
}
