/**
 * Review the real app across palettes and useful window widths on the docs fleet.
 * These unannotated pictures belong to development review, never to a docs page
 * or publication. Later Steel changes extend this set when they add a surface.
 */
import { expect, test } from "@playwright/test";
import { hostRowByName, openHostMenu, openRowMenu, patchPreferences, readPreferences } from "../tests/helpers/fleet";
import { openSettings, selectSettingsSection } from "../tests/helpers/settings";
import { attachSession, waitForTermText } from "../tests/helpers/term";
import { lastTranscriptLine } from "../readme-hero/stage";
import { open, openLauncher, rowOf } from "./open";
import { DOCS_SHOTS_DIR } from "./paths";
import { shot } from "./shot";
import { readFleet } from "./stage-docs";

for (const colorScheme of ["dark", "light"] as const) {
  for (const width of ["wide", "main-650"] as const) {
    /**
     * The main-pane measurement keeps the narrow case meaningful when a later
     * plan widens the sidebar for the host rail. Native controls and real menus
     * remain visible in a full-window capture; no review overlay hides defects.
     */
    test(`${colorScheme} ${width}`, async ({ page, request }) => {
      await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
      await page.setViewportSize({ width: 1600, height: 1000 });
      const shortcuts = (await readPreferences(request)).template_shortcuts ?? {};
      await patchPreferences(request, { template_shortcuts: { "1": "codex-deep", "8": "quick-claude" } });
      const { scenario } = await open(page, request, { checkoutTrash: true });
      const main = page.locator(".app-main");
      await expect(main).toBeVisible();
      if (width === "main-650") {
        const box = await main.boundingBox();
        expect(box, "the open main pane establishes the sidebar's occupied width").not.toBeNull();
        await page.setViewportSize({ width: Math.round(box!.x + 650), height: 1000 });
        await expect.poll(async () => (await main.boundingBox())?.width, {
          message: "the constrained capture has a 650px main pane",
        }).toBeCloseTo(650, 0);
      }
      await page.evaluate(() => document.fonts.ready.then(() => undefined));
      await expect(page.locator(".session-details")).toBeVisible();
      await expect(page.locator(".build-skew")).toHaveCount(0);
      const row = rowOf(page, scenario, (session) => session.status === "idle" && session.seen === false);
      const hostName = row.locator(".session-row-meta .session-host");
      await expect(hostName).toHaveText("gpu-1");
      await expect(hostName).toHaveCSS("font-family", /^Inter/);
      await expect(row.locator(".session-host-separator")).toHaveText(":");
      await expect(row.locator(".session-cwd")).toHaveCSS("font-family", /JetBrains Mono/);
      // Short proportional names must join their colon as text, rather than
      // leaving the old mono layout's minimum-width cell between host and path.
      const hostGap = await hostName.evaluate((host) => {
        const colon = host.parentElement?.querySelector(".session-host-separator");
        if (!colon) throw new Error("the staged short-host row must have its path separator");
        const text = document.createRange();
        text.selectNodeContents(host);
        const gap = colon.getBoundingClientRect().left - text.getBoundingClientRect().right;
        text.detach();
        return gap;
      });
      expect(Math.abs(hostGap), "the short host and colon have no empty width floor").toBeLessThan(1);
      const name = `visual-review/${colorScheme}-${width}`;
      const window = [page.locator("body")];
      await shot(page, `${name}-main`, window, { pad: 0 });

      await openRowMenu(row);
      const menu = page.locator(".session-row-menu-panel");
      await expect(menu).toBeVisible();
      await shot(page, `${name}-session-menu`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(menu).toBeHidden();

      const settings = await openSettings(page);
      await shot(page, `${name}-settings`, window, { pad: 0 });
      // Directory and command fields must keep their mono role beside Inter
      // labels. The ordinary sessions panel alone does not exercise that split.
      await selectSettingsSection(page, "checkouts");
      // Unsaved examples reveal the fields' actual glyphs without changing
      // checkout configuration or running a command on the staged hosts.
      await settings.locator(".checkout-root-field input").fill("~/work/checkouts");
      await settings.locator(".checkout-post-clone-field textarea").fill("cargo fetch --locked");
      await shot(page, `${name}-settings-checkouts`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(settings).toBeHidden();

      const remote = scenario.hosts.find((host) => host.kind === "remote");
      if (!remote) throw new Error("the visual review needs a staged remote host");
      const host = hostRowByName(page, remote.alias);
      await openHostMenu(host);
      await expect(page.locator(".host-row-menu-panel")).toBeVisible();
      await shot(page, `${name}-host-menu`, window, { pad: 0 });
      await host.locator(".host-settings").click();
      const hostSettings = page.locator(".host-settings-dialog");
      await expect(hostSettings).toBeVisible();
      await shot(page, `${name}-host-settings`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(hostSettings).toBeHidden();

      await page.locator(".templates-button").click();
      const templates = page.locator(".templates-dialog");
      await expect(templates.locator(".templates-sidebar")).toHaveAttribute("aria-busy", "false");
      await templates.locator(".templates-row").filter({ has: page.getByText("codex-deep", { exact: true }) }).click();
      await expect(templates.locator(".templates-editor-heading h3")).toHaveText("edit template");
      await shot(page, `${name}-template-editor`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(templates).toBeHidden();

      // These are the approved staged archives. Photograph the listing only;
      // deletion and host operations remain outside the review capture.
      await page.locator(".checkout-trash-button").click();
      const trash = page.locator(".checkout-trash-dialog");
      await expect(trash.locator(".checkout-trash-entry")).toHaveCount(3);
      await shot(page, `${name}-trash`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(trash).toBeHidden();

      const launcher = await openLauncher(page);
      await launcher.locator(".launch-composer-harness-choice button", { hasText: /^Codex$/ }).click();
      await expect(launcher.locator(".launch-composer-model-choice")).toBeVisible();
      await shot(page, `${name}-launcher`, window, { pad: 0 });
      // The footer is fixed while launch choices scroll. A top-only picture
      // misses lower controls and cannot establish that they remain readable.
      const launch = launcher.locator(".create-session-submit");
      await launch.scrollIntoViewIfNeeded();
      await launcher.evaluate((form) => {
        for (const region of [form, ...form.querySelectorAll<HTMLElement>("*")]) {
          if (region.scrollHeight > region.clientHeight && /auto|scroll/.test(getComputedStyle(region).overflowY)) {
            region.scrollTop = region.scrollHeight;
          }
        }
      });
      await shot(page, `${name}-launcher-end`, window, { pad: 0 });
      await page.keyboard.press("Escape");
      await expect(launcher).toBeHidden();

      // A real shared pin and compact preference exercise the mixed-font
      // baseline the ordinary rows cannot show. Restore both for other shots.
      const id = await row.getAttribute("data-session-id");
      expect(id, "the staged compact row has a real session identity").toBeTruthy();
      const pinUrl = `/api/sessions/${encodeURIComponent(id!)}/pin`;
      const pin = await request.put(pinUrl, { data: { pinned: true } });
      expect(pin.ok()).toBe(true);
      try {
        await patchPreferences(request, { compact: true });
        await page.reload();
        // Reload starts a new terminal attachment. A visible details bar is
        // not replay/reveal readiness; retain the ordinary fleet's live oracle.
        await attachSession(page, readFleet().openId);
        const selected = scenario.sessions.find((session) => session.open);
        if (selected?.transcript) await waitForTermText(page, lastTranscriptLine(DOCS_SHOTS_DIR, selected.transcript), 30_000);
        await expect(row.locator(".session-identity-copy")).toHaveClass(/\bcompact\b/);
        await expect(row).toHaveAttribute("data-session-pinned", "true");
        await expect(row.locator(".session-pin")).toBeVisible();
        await page.evaluate(() => document.fonts.ready.then(() => undefined));
        await expect(page.locator(".session-details")).toBeVisible();
        await expect(page.locator(".build-skew")).toHaveCount(0);
        await shot(page, `${name}-compact-pinned`, window, { pad: 0 });
      } finally {
        const unpin = await request.put(pinUrl, { data: { pinned: false } });
        expect(unpin.ok()).toBe(true);
        await patchPreferences(request, { compact: false, template_shortcuts: shortcuts });
      }
    });
  }
}
