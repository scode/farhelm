/**
 * Stage the README hero fleet and photograph it (docs/readme-hero/SPEC.md).
 *
 * Not a test in the ordinary sense: it produces a PNG, not a verdict, and
 * nothing in CI runs it. It still uses Playwright's test runner because
 * that is what already knows how to boot the stack, authenticate, drive
 * the real UI, and wait on the terminal buffer.
 *
 * The staging itself, and the account of what is real and what is
 * rewritten in transit, live in `stage.ts`, which the demo video shares.
 */
import { test } from "@playwright/test";
import { mkdirSync } from "node:fs";
import path from "node:path";
import { loadScenario, SCENARIO_DIR, STACK_INFO_PATH } from "./scenario";
import { openStagedFleet, readStackInfo, stageFleet } from "./stage";

/** Where the PNG goes. The capture script sets it; a bare run lands under target/. */
const OUTPUT_ENV = "FARHELM_HERO_OUTPUT";

test("capture the README hero", async ({ page, request }) => {
  const scenario = loadScenario();
  const info = readStackInfo(STACK_INFO_PATH);
  const output = process.env[OUTPUT_ENV] ||
    path.resolve(__dirname, "../../target/readme-hero/readme-hero.png");

  const fleet = await stageFleet(page, request, scenario, SCENARIO_DIR, info);
  await openStagedFleet(page, request, scenario, SCENARIO_DIR, fleet);
  // Park the pointer where nothing has a hover state.
  await page.mouse.move(0, 0);
  // One settle beat so the terminal's last paint and the list's status
  // dots are on screen together; nothing here is a readiness oracle, the
  // waits above were.
  await page.waitForTimeout(1_500); // sleep-ok: observation window after every readiness wait passed

  mkdirSync(path.dirname(output), { recursive: true });
  await page.screenshot({ path: output, fullPage: false });
  console.log(`README hero written to ${output}`);
});
