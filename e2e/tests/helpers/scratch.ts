// Per-test scratch directories that die with the stack.
//
// Specs used to mkdtemp their working directories straight under
// os.tmpdir(). Each test removes its own in a finally/afterAll, but a
// SIGKILLed run never gets there, and nothing else matched those names —
// they were exactly the orphan class behind the 22 GB /tmp incident that
// farhelm-teststate (crates/farhelm-teststate) now sweeps, yet outside
// its prefix family. Creating them INSIDE the running stack's state dir
// closes that hole without any naming discipline here: the state dir is
// reclaimed as one tree — by the trap on a handled exit, by the sweep
// after an unhandled death — taking every nested scratch dir with it.
// While the stack runs, the state dir is liveness-locked where flock(1)
// exists (start-stack.sh holds the flock); where it does not (macOS),
// the sweep's conservative no-lock-file age backstop governs instead.
// Prefixes stay free-form and descriptive, because the sweep judges the
// container, never these children.
//
// The one path constraint inherited from the parent: anything that ends
// up binding a pathname unix socket in here shares the state dir's
// SUN_LEN budget, which is why the stack keeps its root directly under
// /tmp. No converted caller is known to bind one today; the note exists
// for the future scratch user that does.

import fs from "node:fs";
import path from "node:path";

/** The running stack's own state directory, from the handoff file
 * start-stack.sh publishes before the helm starts serving. Read lazily —
 * at module-load time the file may not exist yet. */
function stackStateDir(): string {
  const raw = fs.readFileSync(path.resolve(__dirname, "../../.stack-info.json"), "utf8");
  const state = (JSON.parse(raw) as { state?: unknown }).state;
  if (typeof state !== "string" || state.length === 0) {
    throw new Error("stack-info.json is missing the stack state directory");
  }
  return state;
}

/** mkdtemp inside the live stack's state dir: a scratch dir with any
 * descriptive prefix, cleaned up with the stack no matter how the run
 * ends. Callers may still remove it themselves earlier; they no longer
 * have to for the machine's sake. */
export function stackScratchDir(prefix: string): string {
  return fs.mkdtempSync(path.join(stackStateDir(), prefix));
}
