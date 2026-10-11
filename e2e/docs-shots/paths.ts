// Where the docs screenshots' design and per-run state live
// (docs/docs-shots/SPEC.md). Its own module so the Playwright config can
// import the paths without importing a spec, which only the test runner may
// load.
import path from "node:path";

/** The design directory: scenario, transcripts, SPEC. */
export const DOCS_SHOTS_DIR = path.resolve(__dirname, "../../docs/docs-shots");
/** Where the stack script publishes what it booted for a docs-shots run. Gitignored, per run. */
export const DOCS_STACK_INFO_PATH = path.join(__dirname, ".stack-info.json");
/**
 * The staged fleet's session ids and clock, written by the staging project
 * and read by every shot project, so the fleet is staged once per run rather
 * than once per page. Gitignored, per run.
 */
export const DOCS_FLEET_PATH = path.join(__dirname, ".fleet.json");
/** The capture script names the output directory through this variable. */
export const DOCS_OUTPUT_ENV = "FARHELM_DOCS_SHOTS_OUTPUT";
/**
 * Where shots land when the variable is unset: the docs website's gitignored
 * local directory, which the running preview serves straight away and which
 * the publish script reads from.
 */
export const DOCS_DEFAULT_OUTPUT = path.resolve(__dirname, "../../website/public/docs-shots-local");
/**
 * The published release shots a capture should take after all, comma
 * separated. `scripts/docs-screenshots.sh --recapture-release-shot NAME` sets
 * it on the Playwright command line only, for a deliberate correction of a
 * past release's screenshot; unset, every published release shot is left
 * alone (release-shots.ts says why).
 */
export const RECAPTURE_RELEASE_SHOTS_ENV = "FARHELM_DOCS_RECAPTURE_RELEASE_SHOTS";
/** The published manifest: which snapshot the docs pin and which shots it holds. */
export const DOCS_MANIFEST_PATH = path.resolve(__dirname, "../../website/src/data/docs-shots.json");
/** Each release's screenshot list for the Release notes page, one `<version>.json` per release. */
export const RELEASE_NOTES_DIR = path.resolve(__dirname, "../../website/src/release-notes");
