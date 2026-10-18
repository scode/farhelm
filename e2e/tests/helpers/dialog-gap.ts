import { expect, type Locator, type Page } from "@playwright/test";

/**
 * Click empty painted space inside a dialog or confirmation panel.
 *
 * A click on a child control or text would exercise its native behavior, so
 * this helper first proves the pointer will target an element where the gap
 * listener can cancel focus. Framed dialogs paint header and body children
 * across their whole box; requiring the dialog node itself would reject
 * their real gaps. The edge search tolerates different padding and headers.
 */
export async function clickDialogGap(page: Page, dialog: Locator) {
  const point = await dialog.evaluate((element) => {
    const box = element.getBoundingClientRect();
    const interactiveOrText =
      'button, input, textarea, select, a[href], label, summary, [contenteditable], [tabindex]:not([tabindex="-1"]), p, span, dt, dd, li, h1, h2, h3, h4, h5, h6';
    for (const inset of [4, 8, 12, 16, 24, 32]) {
      for (const [x, y] of [
        [box.left + inset, box.top + inset],
        [box.right - inset, box.top + inset],
        [box.left + inset, box.bottom - inset],
        [box.right - inset, box.bottom - inset],
      ]) {
        const target = document.elementFromPoint(x, y);
        if (!(target instanceof Element) || !element.contains(target)) continue;
        if (target.closest(interactiveOrText)) continue;
        if ([...target.childNodes].some((node) => node.nodeType === Node.TEXT_NODE && /\S/.test(node.textContent))) {
          continue;
        }
        return { x, y };
      }
    }
    return null;
  });
  expect(point, "the mounted panel must expose empty space that can retain focus").not.toBeNull();
  await page.mouse.click(point!.x, point!.y);
  return point!;
}
