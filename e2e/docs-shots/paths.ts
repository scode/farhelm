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
