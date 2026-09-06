import { expect, test } from "../tests/helpers/evidence";

// The hook marker and the default context's close marker must both precede the
// automatic fixture's attachment. The teardown case throws only after its
// marker, so the parent can distinguish lifecycle failure from body failure.
test.afterEach(async ({ page, timeline }, testInfo) => {
  timeline.record("after-each", [["title", testInfo.title]]);
  await page.locator("body").count();
  if (testInfo.title === "intentional timeline teardown failure") {
    throw new Error("intentional fixed-category teardown failure");
  }
});

/** A body assertion failure must retain input ordering without retaining input text. */
test("intentional timeline body failure", async ({ page, timeline }) => {
  await page.goto("data:text/html,%3Cinput%20id%3Dchild%3E");
  await page.locator("#child").focus();
  await page.locator("#child").pressSequentially("private-value");
  timeline.record("child-body-complete", [["outcome", "failure"]]);
  expect(false).toBe(true);
});

/** The named premise proves setup and input happened before the timeout fired. */
test("intentional timeline timeout", async ({ page, timeline }) => {
  test.setTimeout(5_000);
  await page.goto("data:text/html,%3Cinput%20id%3Dtimeout-input%3E");
  await page.locator("#timeout-input").focus();
  await page.locator("#timeout-input").pressSequentially("private-timeout-value");
  timeline.record("timeout-premise", [["input_observed", true]]);
  await page.waitForTimeout(30_000);
});

/** A passing expected-failure is unexpected and therefore keeps evidence. */
test("intentional timeline unexpected pass", async ({ page, timeline }) => {
  test.fail();
  await page.setContent("<main>unexpected pass</main>");
  timeline.record("child-body-complete", [["outcome", "unexpected-pass"]]);
  await expect(page.locator("main")).toHaveText("unexpected pass");
});

/** A hook failure after a passing body exercises the final-status decision. */
test("intentional timeline teardown failure", async ({ page, timeline }) => {
  await page.setContent("<main>teardown</main>");
  timeline.record("child-body-complete", [["outcome", "teardown"]]);
  await expect(page.locator("main")).toHaveText("teardown");
});
