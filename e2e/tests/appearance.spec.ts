/**
 * Device appearance must work before the renderer and throughout a live page.
 *
 * These tests use the shipped head asset, stylesheet and Settings controls.
 * Holding only the real renderer module exposes the initial blank canvas;
 * ordinary app readiness afterward proves that this was the actual entry page.
 * Storage refusal and malformed-input edges belong to the asset's unit tests.
 */
import { expect, test } from "./helpers/evidence";
import type { Locator, Page } from "@playwright/test";
import { drainedRoute } from "./helpers/drained-route";
import { routeGate } from "./helpers/route-gate";
import { openSettings, selectSettingsSection } from "./helpers/settings";
import { installTerminalSuiteHooks, openTerminal } from "./helpers/terminal-suite";

type Palette = "dark" | "light";
type Choice = "system" | Palette;

const appearanceKey = "farhelm.appearance";
const labels: Record<Choice, string> = { system: "System", dark: "Dark", light: "Light" };
// Pin the product colors independently of its custom properties: resolving
// --bg-0 here would let the implementation and the expectation drift together.
const canvas: Record<Palette, string> = { dark: "rgb(24, 28, 36)", light: "rgb(244, 246, 250)" };
// A root canvas can be correct while an open dialog remains in the old palette.
// Pin actual surface/text colors independently of inherited custom properties.
const dialogColors = {
  dark: { ground: "rgb(40, 46, 58)", header: "rgb(51, 59, 73)", footer: "rgb(29, 34, 44)", primary: "rgb(226, 230, 240)", secondary: "rgb(162, 170, 187)" },
  light: { ground: "rgb(255, 255, 255)", header: "rgb(221, 227, 238)", footer: "rgb(233, 237, 244)", primary: "rgb(27, 33, 48)", secondary: "rgb(79, 90, 112)" },
};

installTerminalSuiteHooks();

/** The Settings gear is the ordinary app boundary, independent of terminal selection. */
async function waitForAppReady(page: Page): Promise<void> {
  await expect(page.getByRole("button", { name: "settings", exact: true })).toBeVisible({ timeout: 20_000 });
}

/** Verify the actual canvas as well as the owner's resolved palette. */
async function expectPalette(page: Page, palette: Palette): Promise<void> {
  await expect(page.locator("html")).toHaveAttribute("data-appearance", palette);
  await expect(page.locator("html")).toHaveCSS("background-color", canvas[palette]);
  await expect(page.locator("body")).toHaveCSS("background-color", canvas[palette]);
  await expect(page.locator("#main")).toHaveCSS("background-color", canvas[palette]);
  await expect(page.locator("html")).toHaveCSS("color-scheme", palette);
}

/** Native checked properties must agree on exactly one choice after every update. */
async function expectChoice(panel: Locator, choice: Choice): Promise<void> {
  for (const candidate of ["system", "dark", "light"] as const) {
    const radio = panel.getByRole("radio", { name: labels[candidate], exact: true });
    await expect(radio).toHaveAttribute("name", "farhelm-appearance");
    await expect(radio).toHaveAttribute("value", candidate);
    await expect(radio).toBeChecked({ checked: candidate === choice });
  }
}

/** Pointer users activate the visible label, while the clipped native input owns checked state. */
async function clickAppearanceChoice(panel: Locator, choice: Choice): Promise<void> {
  const label = panel.locator("label").filter({ hasText: new RegExp(`^${labels[choice]}$`) });
  const text = label.locator("span");
  await expect(text).toBeVisible();
  await text.click();
  await expect(panel.getByRole("radio", { name: labels[choice], exact: true })).toBeChecked();
}

/** Follow the actual Settings navigation rather than invoking the JS owner. */
async function openAppearance(page: Page): Promise<Locator> {
  await openSettings(page, "Appearance");
  return page.locator("#settings-appearance");
}

/** A second media listener proves the OS change was delivered even under an override.
 *
 * Waiting only for an unchanged root could pass before the product receives
 * the event it is supposed to ignore. Register this witness before emulation;
 * the existing page owner's listener runs during the same native event.
 */
async function changeSystemAppearance(page: Page, palette: Palette): Promise<void> {
  const witness = await page.evaluateHandle(() => {
    const query = matchMedia("(prefers-color-scheme: light)");
    const observed = { light: null as boolean | null, before: query.matches };
    query.addEventListener("change", (event) => { observed.light = event.matches; }, { once: true });
    return observed;
  });
  try {
    expect(await witness.evaluate((observed) => observed.before), "the stimulus must change the system appearance")
      .toBe(palette !== "light");
    await page.emulateMedia({ colorScheme: palette });
    await expect.poll(() => witness.evaluate((observed) => observed.light), {
      timeout: 5_000,
      message: "the browser must deliver the actual system appearance change",
    }).toBe(palette === "light");
  } finally {
    await witness.dispose();
  }
}

// Startup is intentionally observed before any application child exists. A
// final screenshot alone cannot establish that the wrong palette never showed.
for (const initial of [
  { system: "dark", stored: null, expected: "dark", name: "fresh System uses dark" },
  { system: "light", stored: null, expected: "light", name: "fresh System uses light" },
  { system: "light", stored: "dark", expected: "dark", name: "stored Dark overrides a light system" },
  { system: "dark", stored: "light", expected: "light", name: "stored Light overrides a dark system" },
] as const) {
  /** The classic head must color the canvas before the real renderer can mount. */
  test(`${initial.name} before application mount`, async ({ page, request }) => {
    await page.emulateMedia({ colorScheme: initial.system });
    if (initial.stored !== null) {
      await page.addInitScript(({ key, choice }) => localStorage.setItem(key, choice), {
        key: appearanceKey,
        choice: initial.stored,
      });
    }
    const entry = await request.get("/");
    expect(entry.ok(), "the held module must come from the actual served entry document").toBe(true);
    const html = await entry.text();
    const rendererPaths = [...new Set(html.match(/\/assets\/farhelm-ui-dx[^"'<>\s]+\.js/g) ?? [])];
    expect(rendererPaths, "the generated entry must identify one real renderer module").toHaveLength(1);
    const rendererUrl = new URL(rendererPaths[0], entry.url()).href;
    const release = routeGate();
    let requested = 0;
    const stopRoute = await drainedRoute(page, rendererUrl, async (route) => {
      requested++;
      await release.wait();
      await route.continue();
    });
    try {
      await page.goto("/", { waitUntil: "commit" });
      await expect.poll(() => requested, {
        timeout: 10_000,
        message: "the actual renderer module must be requested and held",
      }).toBe(1);
      await expect(page.locator("#main")).toHaveCount(1);
      await expect(page.locator("#main > *")).toHaveCount(0);
      await expect(page.locator("#main")).toHaveText("");
      await expect(page.getByRole("button", { name: "settings", exact: true })).toHaveCount(0);
      await expectPalette(page, initial.expected);
      expect(await page.evaluate((key) => localStorage.getItem(key), appearanceKey)).toBe(initial.stored);

      const head = await page.evaluate((moduleUrl) => {
        const module = [...document.scripts].find((script) => script.src === moduleUrl);
        const appearances = [...document.head.querySelectorAll<HTMLScriptElement>("script[src]")]
          .filter((script) => /^appearance[^/]*\.js$/.test(new URL(script.src).pathname.split("/").pop()!));
        const styles = [...document.head.querySelectorAll<HTMLLinkElement>('link[rel="stylesheet"]')]
          .filter((link) => /^app[^/]*\.css$/.test(new URL(link.href).pathname.split("/").pop()!));
        return {
          rendererIsBodyModule: module?.parentElement === document.body && module.type === "module",
          appearances: appearances.map((script) => ({
            classic: script.type === "" || script.type === "text/javascript",
            async: script.hasAttribute("async"),
            defer: script.hasAttribute("defer"),
            beforeRenderer: !!module && !!(script.compareDocumentPosition(module) & Node.DOCUMENT_POSITION_FOLLOWING),
          })),
          styles: styles.map((link) => ({
            loaded: link.sheet !== null,
            beforeRenderer: !!module && !!(link.compareDocumentPosition(module) & Node.DOCUMENT_POSITION_FOLLOWING),
          })),
        };
      }, rendererUrl);
      expect(head.rendererIsBodyModule).toBe(true);
      expect(head.appearances).toEqual([{ classic: true, async: false, defer: false, beforeRenderer: true }]);
      expect(head.styles).toEqual([{ loaded: true, beforeRenderer: true }]);
    } finally {
      release.release();
      await stopRoute();
    }
    await waitForAppReady(page);
    await expectPalette(page, initial.expected);
    await expectChoice(await openAppearance(page), initial.stored ?? "system");
  });
}

/** Every real control updates a mounted page; System alone follows subsequent OS changes.
 * The retained terminal, its full theme and its surfaces guard the exception
 * that Steel Light restyles the frame while the terminal itself stays dark.
 */
test("all appearance choices update live while the mounted terminal stays dark", async ({ page }, testInfo) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await openTerminal(page);
  const documentIdentity = await page.evaluateHandle(() => document);
  const owner = await page.evaluateHandle(() => (window as any).farhelmAppearance);
  expect(await owner.evaluate((value) => typeof value?.mountSettings)).toBe("function");
  const terminal = await page.evaluateHandle(() => (window as any).__farhelmTerm);
  const theme = await terminal.evaluate((term) => term.options.theme);
  expect(theme.background.toLowerCase()).toBe("#282c34");
  // xterm's viewport is backing behind the renderer's canvas, with its own
  // vendored black default. Preserve that baseline; the theme and outer gutter
  // below independently pin the terminal's actual dark palette.
  const viewportBackground = await page.locator("#terminal .xterm-viewport")
    .evaluate((element) => getComputedStyle(element).backgroundColor);
  const panel = await openAppearance(page);
  const dialog = await page.locator('.app-settings-dialog[role="dialog"]').elementHandle();
  expect(dialog, "the live test must retain the actual open Settings root").not.toBeNull();
  const observations: unknown[] = [];
  let capture = 0;

  /** Exercise a visible label rather than clicking an input in a hidden panel.
   * The adjacent span remains the pointer target when PR5 clips the native
   * radio drawing; native checked state and keyboard behavior still belong to it.
   */
  async function clickChoice(choice: Choice): Promise<void> {
    expect(await dialog!.evaluate((original) => original.isConnected
      && document.querySelector('.app-settings-dialog[role="dialog"]') === original),
    "pointer input must reach the retained Settings dialog").toBe(true);
    const target = panel.locator("label").filter({ hasText: new RegExp(`^${labels[choice]}$`) }).locator("span");
    await expect(target).toBeVisible();
    expect(await target.evaluate((element) => {
      const bounds = element.getBoundingClientRect();
      const hit = document.elementFromPoint(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
      return !!hit && element.closest("label")!.contains(hit);
    }), "the visible appearance label must own its center hit target").toBe(true);
    await target.click();
    await expect(panel.getByRole("radio", { name: labels[choice], exact: true })).toBeChecked();
  }

  /** Assert dialog computation and retain its real pixels beside the root oracle.
   * Reading computed styles or receiving a frame callback is not evidence of
   * presentation; full open-dialog screenshots remain available for inspection.
   */
  async function expectLivePalette(palette: Palette, choice: Choice): Promise<void> {
    await expectPalette(page, palette);
    await expectChoice(panel, choice);
    await expect(panel).toBeVisible();
    await expect(page.locator('button[aria-controls="settings-appearance"]')).toHaveAttribute("aria-pressed", "true");
    await expect(panel.locator(":scope > h3")).toHaveText("Appearance");
    const observed = await dialog!.evaluate(async (original) => {
      const required = (selector: string): Element => {
        const node = original.querySelector(selector);
        if (!node) throw new Error(`missing dialog observation target ${selector}`);
        return node;
      };
      const style = (node: Element) => {
        const css = getComputedStyle(node);
        const bounds = node.getBoundingClientRect();
        return { background: css.backgroundColor, color: css.color,
          tokens: Object.fromEntries(["--bg-2", "--control-hover-bg", "--bg-1", "--fg-0", "--fg-1"]
            .map((name) => [name, css.getPropertyValue(name).trim()])),
          bounds: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height } };
      };
      const read = () => {
        const body = required(":scope > .host-settings-body");
        let backing: Element | null = body;
        while (backing && getComputedStyle(backing).backgroundColor === "rgba(0, 0, 0, 0)") backing = backing.parentElement;
        if (!backing) throw new Error("dialog body has no supplying background");
        const radio = required('#settings-appearance input[data-appearance-choice]:not(:checked)');
        const label = radio.closest("label")!.querySelector("span")!;
        return { at: performance.now(), timeOrigin: performance.timeOrigin,
          sameDialog: original.isConnected && document.querySelector('.app-settings-dialog[role="dialog"]') === original,
          shownPanels: [...original.querySelectorAll<HTMLElement>(".app-settings-panel")].filter((node) => !node.hidden).map((node) => node.id),
          root: style(document.documentElement), dialog: style(original),
          header: style(required(":scope > .host-settings-header")), title: style(required(".host-settings-title")),
          body: style(body), bodyBacking: style(backing), bodyBackingIsDialog: backing === original,
          heading: style(required("#settings-appearance > h3")), label: style(label),
          help: style(required("#appearance-help")), footer: style(required(":scope > .host-settings-footer")),
          footerHelp: style(required(":scope > .host-settings-footer > span")) };
      };
      const immediate = read();
      const nextFrame = await new Promise<ReturnType<typeof read>>((resolve) => requestAnimationFrame(() => resolve(read())));
      return { immediate, nextFrame };
    });
    observations.push({ palette, choice, ...observed });
    // Capture before asserting color equality so stale pixels and computed
    // values survive a failure. No artificial browser observation delay.
    await page.screenshot({ path: testInfo.outputPath(`appearance-dialog-${String(++capture).padStart(2, "0")}-${palette}-${choice}.png`) });
    const expected = dialogColors[palette];
    for (const state of [observed.immediate, observed.nextFrame]) {
      expect(state.sameDialog, "palette actions must keep the original Settings dialog mounted").toBe(true);
      expect(state.shownPanels).toEqual(["settings-appearance"]);
      expect(state.dialog.background).toBe(expected.ground);
      expect(state.header.background).toBe(expected.header);
      expect(state.footer.background).toBe(expected.footer);
      expect(state.body.background).toBe("rgba(0, 0, 0, 0)");
      expect(state.bodyBackingIsDialog, "the transparent body must expose this dialog's fill").toBe(true);
      expect(state.bodyBacking.background).toBe(expected.ground);
      expect(state.title.color).toBe(expected.primary);
      expect(state.label.color).toBe(expected.primary);
      expect(state.heading.color).toBe(expected.primary);
      expect(state.help.color).toBe(expected.secondary);
      expect(state.footerHelp.color).toBe(expected.secondary);
    }
    // Settings intentionally isolates the background from accessibility. Its
    // still-painted wordmark must nevertheless follow the live palette.
    const wordmark = page.locator('.app-wordmark:visible svg[role="img"]');
    await expect(wordmark).toHaveCount(1);
    await expect(wordmark).toHaveAttribute("aria-label", "farhelm");
    await expect(wordmark.locator("g").first()).toHaveCSS(
      "fill",
      palette === "light" ? "rgb(31, 35, 40)" : "rgb(230, 235, 241)",
    );
    expect(await documentIdentity.evaluate((original) => original === document)).toBe(true);
    expect(await owner.evaluate((original) => original === (window as any).farhelmAppearance)).toBe(true);
    expect(await terminal.evaluate((original) => original === (window as any).__farhelmTerm)).toBe(true);
    expect(await terminal.evaluate((term) => term.options.theme)).toEqual(theme);
    await expect(page.locator("#terminal")).toHaveCSS("background-color", "rgb(40, 44, 52)");
    await expect(page.locator("#terminal .xterm-viewport")).toHaveCSS("background-color", viewportBackground);
  }

  try {
    await expectLivePalette("dark", "system");
    await changeSystemAppearance(page, "light");
    await expectLivePalette("light", "system");
    // Establish native focus before testing Space and radio-group arrows.
    // These must restyle the same open dialog as visible pointer activation.
    const dark = panel.getByRole("radio", { name: "Dark", exact: true });
    await dark.focus();
    await expect(dark).toBeFocused();
    await dark.press("Space");
    await expect(dark).toBeFocused();
    await expectLivePalette("dark", "dark");
    await dark.press("ArrowRight");
    const light = panel.getByRole("radio", { name: "Light", exact: true });
    await expect(light).toBeFocused();
    await expectLivePalette("light", "light");
    // Use adjacent choices: native last-to-first wrapping differs by engine.
    // This tests the product's native group without inventing a keyboard policy.
    await light.press("ArrowLeft");
    await expect(dark).toBeFocused();
    await expectLivePalette("dark", "dark");
    await dark.press("ArrowLeft");
    await expect(panel.getByRole("radio", { name: "System", exact: true })).toBeFocused();
    await expectLivePalette("light", "system");
    await clickChoice("dark");
    await expectLivePalette("dark", "dark");
    await changeSystemAppearance(page, "dark");
    await expectLivePalette("dark", "dark");
    await changeSystemAppearance(page, "light");
    await expectLivePalette("dark", "dark");
    await clickChoice("light");
    await expectLivePalette("light", "light");
    await changeSystemAppearance(page, "dark");
    await expectLivePalette("light", "light");
    await clickChoice("system");
    await expectLivePalette("dark", "system");
    await changeSystemAppearance(page, "light");
    await expectLivePalette("light", "system");
  } finally {
    await testInfo.attach("appearance-dialog-observations", { body: JSON.stringify(observations, null, 2), contentType: "application/json" });
    await dialog?.dispose();
    await terminal.dispose();
    await owner.dispose();
    await documentIdentity.dispose();
  }
});

/** Recreated radios and a new document must recover the user's actual UI choice.
 * Switching sections first also catches a second renderer-owned checked state
 * that would overwrite the page owner's choice when unrelated Settings render.
 */
test("appearance choices survive Settings remounts and reloads", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await waitForAppReady(page);
  for (const choice of ["light", "dark", "system"] as const) {
    const panel = await openAppearance(page);
    await clickAppearanceChoice(panel, choice);
    await expectChoice(panel, choice);
    expect(await page.evaluate((key) => localStorage.getItem(key), appearanceKey)).toBe(choice);
    await selectSettingsSection(page, "terminal");
    await selectSettingsSection(page, "Appearance");
    await expectChoice(panel, choice);
    const oldRadio = await panel.getByRole("radio", { name: labels[choice], exact: true }).elementHandle();
    expect(oldRadio).not.toBeNull();
    await page.locator(".app-settings-close").click();
    await expect(page.getByRole("dialog", { name: "settings", exact: true })).toHaveCount(0);
    expect(await oldRadio!.evaluate((radio) => radio.isConnected), "closing Settings must discard the old inputs").toBe(false);
    await oldRadio!.dispose();
    await expectChoice(await openAppearance(page), choice);
    await page.reload();
    await waitForAppReady(page);
    await expectPalette(page, choice === "system" ? "dark" : choice);
    await expectChoice(await openAppearance(page), choice);
    await page.locator(".app-settings-close").click();
  }
  // System persistence stores the choice, not the formerly resolved dark value.
  await changeSystemAppearance(page, "light");
  await expectPalette(page, "light");
  expect(await page.evaluate((key) => localStorage.getItem(key), appearanceKey)).toBe("system");
});

/** Real same-origin storage events must update an existing owner, including with Settings closed.
 * The second page writes through its controls before clearing actual storage;
 * synthetic StorageEvent dispatch would bypass the browser integration seam.
 */
test("another same-origin page updates appearance and clearing restores System", async ({ page, context }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  await waitForAppReady(page);
  const owner = await page.evaluateHandle(() => (window as any).farhelmAppearance);
  expect(await owner.evaluate((value) => typeof value?.mountSettings)).toBe("function");
  let panel = await openAppearance(page);
  await clickAppearanceChoice(panel, "dark");
  await expectPalette(page, "dark");
  const other = await context.newPage();
  try {
    await other.emulateMedia({ colorScheme: "dark" });
    await other.goto("/");
    await waitForAppReady(other);
    expect(new URL(other.url()).origin, "both pages must share the actual storage origin").toBe(new URL(page.url()).origin);
    const otherPanel = await openAppearance(other);
    await expectChoice(otherPanel, "dark");
    await clickAppearanceChoice(otherPanel, "light");
    await expectPalette(other, "light");
    await expectPalette(page, "light");
    await expectChoice(panel, "light");
    await page.locator(".app-settings-close").click();
    await expect(page.getByRole("dialog", { name: "settings", exact: true })).toHaveCount(0);
    await clickAppearanceChoice(otherPanel, "dark");
    await expectPalette(page, "dark");
    panel = await openAppearance(page);
    await expectChoice(panel, "dark");
    expect(await page.evaluate((key) => localStorage.getItem(key), appearanceKey)).toBe("dark");
    // Keep this an appearance test rather than logging the live app out:
    // clear emits the real null-key event, then unrelated device state is
    // restored in the same task so the fixture retains its authentication.
    await other.evaluate((key) => {
      const retained = Object.entries(localStorage).filter(([name]) => name !== key);
      localStorage.clear();
      for (const [name, value] of retained) localStorage.setItem(name, value);
    }, appearanceKey);
    await expectPalette(page, "light");
    await expectChoice(panel, "system");
    expect(await page.evaluate((key) => localStorage.getItem(key), appearanceKey)).toBeNull();
    expect(await owner.evaluate((original) => original === (window as any).farhelmAppearance)).toBe(true);
    await changeSystemAppearance(page, "dark");
    await expectPalette(page, "dark");
    await expectChoice(panel, "system");
  } finally {
    await other.close();
    await owner.dispose();
  }
});
