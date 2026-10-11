/**
 * Keep published release shots frozen (docs/docs-shots/SPEC.md, "Release
 * shots are frozen").
 *
 * A release shot is a shot one of website/src/release-notes/<version>.json
 * lists, and it is published once the manifest (website/src/data/docs-shots.json)
 * lists it, meaning the snapshot the docs pin holds it. Release notes do not
 * change once published, and a capture of today's UI would show a past
 * release something it never had, so a capture skips every published release
 * shot and the publish script carries the published image over unchanged. A
 * release shot nobody has published yet, a new release's first screenshot,
 * is captured like any other.
 *
 * The one way past the skip is naming the shot with
 * `scripts/docs-screenshots.sh --recapture-release-shot NAME`, for a
 * deliberate correction; the script checks the name and passes it through
 * RECAPTURE_RELEASE_SHOTS_ENV.
 */
import { test } from "@playwright/test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { DOCS_MANIFEST_PATH, RECAPTURE_RELEASE_SHOTS_ENV, RELEASE_NOTES_DIR } from "./paths";

/**
 * Every shot name that is both listed by a release's screenshot list and held
 * by the published snapshot. Empty before the first publish. The same
 * definition scripts/docs-screenshots.sh and scripts/publish-docs-shots.sh
 * apply, so the three agree on which shots are frozen.
 */
export function publishedReleaseShots(): Set<string> {
  if (!existsSync(DOCS_MANIFEST_PATH)) return new Set();
  const manifest = JSON.parse(readFileSync(DOCS_MANIFEST_PATH, "utf-8")) as { shots?: Record<string, unknown> };
  const published = new Set(Object.keys(manifest.shots ?? {}));
  const frozen = new Set<string>();
  const listings = existsSync(RELEASE_NOTES_DIR) ? readdirSync(RELEASE_NOTES_DIR) : [];
  for (const listing of listings.filter((file) => file.endsWith(".json"))) {
    const { screenshots = [] } = JSON.parse(readFileSync(path.join(RELEASE_NOTES_DIR, listing), "utf-8")) as {
      screenshots?: { name?: string }[];
    };
    for (const { name } of screenshots) if (name && published.has(name)) frozen.add(name);
  }
  return frozen;
}

/**
 * Skip the calling test when `name` is a published release shot the capture
 * was not told to recapture. Call it first in the test body, before opening
 * the fleet, so a skipped shot costs nothing.
 */
export function skipFrozenReleaseShot(name: string): void {
  if (!publishedReleaseShots().has(name)) return;
  const targeted = (process.env[RECAPTURE_RELEASE_SHOTS_ENV] ?? "").split(",").filter(Boolean);
  test.skip(
    !targeted.includes(name),
    `${name} is a published release shot and stays as published; ` +
      `scripts/docs-screenshots.sh --recapture-release-shot ${name} replaces it, for a deliberate correction only`,
  );
}
