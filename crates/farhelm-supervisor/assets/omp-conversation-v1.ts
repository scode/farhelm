import { execFile } from "node:child_process";
import { existsSync } from "node:fs";

/** Keep Farhelm's resume target aligned with OMP's current conversation.
 *
 * Unlike Pi, OMP has no isPersisted() and no single session_start reason: the
 * active conversation changes through session_start (startup), session_switch
 * (new/resume/fork), and session_branch, and persistence is lazy — the file
 * appears once history holds an assistant message, though a /new session is
 * persisted eagerly, so an empty session can legitimately already have its
 * file. A report for the current id always goes out, with session_file: null
 * when the file does not exist, which deliberately withdraws the previous
 * resume offer until OMP persists.
 *
 * Known gaps, accepted rather than papered over: an interactive session move
 * that emits no switch event keeps the previous path until the next
 * subscribed event fires, and a session on a non-file storage backend stays
 * fresh-only (there is no file to verify against resume).
 */
export default function farhelmConversation(omp) {
    const executable = process.env.FARHELM_OMP_REPORTER_EXE;
    if (!executable) return;
    let pending = Promise.resolve();

    /** Serialize reports so a slow old session cannot win after a switch.
     *
     * The id is captured when the event fires, the stale-id check reruns when
     * the queued report finally executes, and only then is the session file's
     * existence re-read — so a delayed check can neither resurrect an old
     * session's path nor drop the file a just-persisted session now has.
     *
     * The whole report body, initial identity capture included, sits inside a
     * silent-failure boundary: a vendor getter throwing is a contract
     * violation this reporter absorbs (nothing is reported, nothing escapes
     * into OMP), never an error the user's terminal sees.
     */
    function report(source, ctx) {
        try {
            const sessionId = ctx.sessionManager.getSessionId();
            pending = pending.then(() => new Promise((resolve) => {
                if (ctx.sessionManager.getSessionId() !== sessionId) {
                    resolve();
                    return;
                }
                try {
                    const file = ctx.sessionManager.getSessionFile();
                    const payload = JSON.stringify({
                        vendor: "omp",
                        session_id: sessionId,
                        session_file: file && existsSync(file) ? file : null,
                        source,
                    });
                    const child = execFile(executable, ["internal", "hook"], {
                        timeout: 2000,
                        maxBuffer: 8192,
                    }, () => resolve());
                    child.stdin?.on("error", () => {});
                    child.stdin?.end(payload);
                } catch {
                    resolve();
                }
            })).catch(() => {});
            return pending;
        } catch {
            return pending;
        }
    }

    omp.on("session_start", (_event, ctx) => report("session_start", ctx));
    omp.on("session_switch", (event, ctx) =>
        report(`session_switch:${event?.reason ?? "unknown"}`, ctx),
    );
    omp.on("session_branch", (_event, ctx) => report("session_branch", ctx));
    omp.on("agent_end", (_event, ctx) => report("agent_end", ctx));
}
