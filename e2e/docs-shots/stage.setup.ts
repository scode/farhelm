/**
 * Stage the docs fleet once per capture run (docs/docs-shots/SPEC.md).
 *
 * A setup project rather than a step in each page's spec: staging waits for
 * every session to be classified, which takes most of a minute, and every
 * page's shots photograph the same fleet. The shot projects depend on this
 * one and read the staged ids from the fleet file it writes.
 */
import { test } from "@playwright/test";
import { loadScenario } from "../readme-hero/scenario";
import { readStackInfo, stageFleet } from "../readme-hero/stage";
import { DOCS_SHOTS_DIR, DOCS_STACK_INFO_PATH } from "./paths";
import { writeFleet } from "./stage-docs";

test("stage the docs fleet", async ({ page, request }) => {
  const scenario = loadScenario(DOCS_SHOTS_DIR);
  const fleet = await stageFleet(page, request, scenario, DOCS_SHOTS_DIR, readStackInfo(DOCS_STACK_INFO_PATH));
  writeFleet(fleet);
});
