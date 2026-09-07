import { defineConfig, devices } from "@playwright/test";

/** Isolate intentional failing children from ordinary and parent contract discovery. */
export default defineConfig({
  testDir: ".",
  testMatch: "timeline-child.failure.ts",
  timeout: 15_000,
  use: { trace: "retain-on-failure" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
});
