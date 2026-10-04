/**
 * Take one docs screenshot (docs/docs-shots/SPEC.md): crop to the parts of
 * the UI the shot is about plus every annotation on screen, check the page
 * for anything from the capturing machine, and write the PNG under its name.
 */
import type { Locator, Page } from "@playwright/test";
import { mkdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { DOCS_DEFAULT_OUTPUT, DOCS_OUTPUT_ENV } from "./paths";

/** `<page>/<shot>`, both lowercase kebab case: the name a docs page refers to the image by. */
const NAME = /^[a-z0-9]+(-[a-z0-9]+)*\/[a-z0-9]+(-[a-z0-9]+)*$/;

/**
 * Fail if any visible text or form value names something from the capturing
 * machine: its account name, its home directory, or the run's temporary
 * state. The scenario and the rewrites are supposed to keep all of those out
 * of the picture; this is the check that they did, run against the page the
 * PNG is taken from rather than trusted. Text the terminal draws on a canvas
 * is not covered, but terminals only show invented transcripts.
 */
async function assertNoMachineDetails(page: Page): Promise<void> {
  const user = os.userInfo().username;
  const forbidden = [os.homedir(), "/tmp/fh-e2e", `${user}@`, `/${user}/`].filter((needle) => needle.length > 2);
  // The machine's own name too: a shot of host details or a terminal title
  // could carry it.
  const hostname = os.hostname();
  if (hostname.length >= 4 && hostname !== "localhost") forbidden.push(hostname);
  const text = await page.evaluate(() => {
    const parts = [document.body.innerText];
    for (const input of Array.from(document.querySelectorAll("input, textarea, select"))) {
      parts.push((input as HTMLInputElement).value ?? "");
    }
    return parts.join("\n");
  });
  for (const needle of forbidden) {
    if (text.includes(needle)) throw new Error(`the page shows "${needle}", a detail of the capturing machine`);
  }
  // The bare account name too, as a whole word. Very short names are skipped:
  // they would match ordinary words in the UI and fail every capture.
  if (user.length >= 4 && new RegExp(`\\b${user.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`).test(text)) {
    throw new Error(`the page shows the capturing account's name, ${user}`);
  }
}

/** How a shot is cropped beyond its regions. */
export interface CropOptions {
  /** Margin around the union of regions and annotations, in CSS pixels. */
  pad?: number;
  /**
   * Widest the crop may be, in CSS pixels, keeping its left edge. For a
   * region far wider than what matters in it (a full-width list whose rows
   * are all left-aligned text), so the image is not shrunk to fit the docs'
   * text column, about 930 CSS pixels wide.
   */
  maxWidth?: number;
}

/**
 * Write the shot `name` (`<page>/<shot>`) as a PNG cropped to `regions` and
 * every annotation the docs overlay is drawing, padded and kept inside the
 * viewport. Annotations are included automatically so a callout placed
 * beside its target is never cut off by the crop.
 */
export async function shot(page: Page, name: string, regions: Locator[], options: CropOptions = {}): Promise<string> {
  const pad = options.pad ?? 16;
  if (!NAME.test(name)) throw new Error(`shot name ${name} must be <page>/<shot> in lowercase kebab case`);
  await assertNoMachineDetails(page);

  const annotations = page.locator("farhelm-demo-overlay .callout, farhelm-demo-overlay .ring");
  const boxes = [];
  for (const region of [...regions, ...(await annotations.all())]) {
    const box = await region.boundingBox();
    if (box && box.width > 0 && box.height > 0) boxes.push(box);
  }
  if (boxes.length === 0) throw new Error(`shot ${name}: none of its regions is on screen`);
  // Arrows run between a callout and its target, so the union of both
  // boxes already contains every arrow.
  const viewport = page.viewportSize() as { width: number; height: number };
  const left = Math.max(0, Math.min(...boxes.map((box) => box.x)) - pad);
  const top = Math.max(0, Math.min(...boxes.map((box) => box.y)) - pad);
  const right = Math.min(
    viewport.width,
    Math.max(...boxes.map((box) => box.x + box.width)) + pad,
    options.maxWidth === undefined ? Infinity : left + options.maxWidth,
  );
  const bottom = Math.min(viewport.height, Math.max(...boxes.map((box) => box.y + box.height)) + pad);

  const output = path.join(process.env[DOCS_OUTPUT_ENV] || DOCS_DEFAULT_OUTPUT, `${name}.png`);
  mkdirSync(path.dirname(output), { recursive: true });
  await page.mouse.move(0, 0);
  await page.screenshot({
    path: output,
    clip: { x: left, y: top, width: right - left, height: bottom - top },
    animations: "disabled",
  });
  console.log(`docs shot written to ${output}`);
  return output;
}
