import { defineConfig, devices } from "@playwright/test";

/**
 * The grid terminal component on a static page, fed recorded grid streams,
 * in both engines. It needs no Farhelm stack: the page and the shipped
 * assets are served through request routing, so nothing binds a port and
 * nothing is built first. These specs stay outside the main config's
 * discovery (its `testDir` is `tests/`); the stack-level specs for grid
 * terminals arrive once the supervisor and helm send grid streams.
 */
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  // The main suite's bound: a browser's first launch on a loaded machine
  // can take most of a shorter one.
  timeout: 60_000,
  use: { trace: "retain-on-failure" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
});
