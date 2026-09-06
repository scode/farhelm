import { defineConfig, devices } from "@playwright/test";

/** Standalone contracts for evidence helpers; they never start Farhelm's browser harness. */
export default defineConfig({
  testDir: ".",
  testMatch: "*.contract.ts",
  timeout: 15_000,
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
});
