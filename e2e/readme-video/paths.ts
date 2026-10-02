// Where the demo video's design and per-run state live. Its own module so
// the Playwright config can import the paths without importing the capture
// spec, which only the test runner may load.
import path from "node:path";

/** The video's design directory: scenario, transcripts, intent, SPEC. */
export const VIDEO_DIR = path.resolve(__dirname, "../../docs/readme-video");
/** Where the stack script publishes what it booted for a video run. Gitignored, per run. */
export const VIDEO_STACK_INFO_PATH = path.join(__dirname, ".stack-info.json");
