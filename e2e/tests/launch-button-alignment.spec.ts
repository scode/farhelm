/**
 * The launch button carries two texts of different weights — the bold verb
 * and the lighter host/folder context — and they must read as one line. The
 * context hides its overflow for its ellipsis, and an inline-block with
 * hidden overflow takes its bottom edge as its baseline (CSS 2 §10.8.1),
 * which once sat the verb visibly lower than the context (5 px at desktop
 * width). This pins the fixed layout with a measurement, not a screenshot:
 * the verb's text and the context's first text line share a vertical centre
 * within a pixel at desktop width, and at phone width the wrapped context
 * lines start flush rather than centred inside their box.
 */
import { expect, test } from "./helpers/evidence";
import { type Locator, type Page } from "@playwright/test";

/** Vertical centres of the verb's text and of the context's FIRST text line, plus the first line's left edge. */
async function measure(button: Locator) {
  return button.evaluate((el) => {
    const span = el.querySelector(".launch-composer-launch-context") as HTMLElement;
    const textNode = Array.from(el.childNodes).find(
      (n) => n.nodeType === Node.TEXT_NODE && (n.textContent ?? "").trim() === "launch",
    ) as Text;
    const verbRange = document.createRange();
    verbRange.selectNodeContents(textNode);
    const verb = verbRange.getBoundingClientRect();
    const ctxRange = document.createRange();
    ctxRange.selectNodeContents(span);
    // One client rect per inline fragment (the context holds several
    // spans), so lines are the distinct rows those fragments sit on.
    const fragments = Array.from(ctxRange.getClientRects()).filter((r) => r.width > 0);
    const rows = new Map<number, DOMRect[]>();
    for (const r of fragments) {
      const key = Math.round(r.top);
      rows.set(key, [...(rows.get(key) ?? []), r]);
    }
    const firstRow = [...rows.entries()].sort((a, b) => a[0] - b[0])[0][1];
    const firstTop = Math.min(...firstRow.map((r) => r.top));
    const firstBottom = Math.max(...firstRow.map((r) => r.bottom));
    return {
      verbMid: (verb.top + verb.bottom) / 2,
      firstLineMid: (firstTop + firstBottom) / 2,
      firstLineLeft: Math.min(...firstRow.map((r) => r.left)),
      spanContentLeft: span.getBoundingClientRect().left,
      lineCount: rows.size,
    };
  });
}

async function openComposer(page: Page): Promise<Locator> {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  await form.getByLabel("folder", { exact: true }).fill("/home/someone/git/farhelm");
  return form;
}

test("the launch verb sits on the context's first line at desktop width", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  const form = await openComposer(page);
  const button = form.locator(".create-session-submit");
  await expect(button).toBeVisible();
  const m = await measure(button);
  expect(m.lineCount, "the context stays on one line at desktop width").toBe(1);
  expect(Math.abs(m.verbMid - m.firstLineMid), "verb and context must share a vertical centre").toBeLessThan(1.5);
});

test("a wrapped context starts flush beside the verb at phone width", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const form = await openComposer(page);
  const button = form.locator(".create-session-submit");
  await expect(button).toBeVisible();
  const m = await measure(button);
  expect(Math.abs(m.verbMid - m.firstLineMid), "the verb aligns to the context's first line").toBeLessThan(1.5);
  expect(Math.abs(m.firstLineLeft - m.spanContentLeft), "the first line must not be centred inside its box").toBeLessThan(1.5);
});
