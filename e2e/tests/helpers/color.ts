import { type Page } from "@playwright/test";

/** Resolve a named palette role to the browser's computed color syntax.
 * Tests choose the role independently of the element under test; the probe
 * only normalizes its value, so palette changes do not duplicate RGB values
 * throughout the browser suite. Root roles are sufficient here: this does
 * not read a component's contextual custom-property overrides.
 */
export async function resolveToken(page: Page, token: string): Promise<string> {
  return page.evaluate((name) => {
    const probe = document.createElement("span");
    probe.style.color = `var(${name})`;
    document.body.appendChild(probe);
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  }, token);
}
