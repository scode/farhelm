import { execFile } from "node:child_process";
import { existsSync } from "node:fs";

/** Keep Farhelm's resume target aligned with Pi's current conversation.
 *
 * Pi emits session_start for startup, new, resume, fork, clone, and reload. A
 * fresh session's file may not exist until its first assistant message, so a
 * report without a file deliberately withdraws the previous resume offer.
 * agent_end upgrades that target once Pi has persisted its messages.
 */
export default function farhelmConversation(pi) {
    const executable = process.env.FARHELM_PI_REPORTER_EXE;
    if (!executable) return;
    let pending = Promise.resolve();

    /** Serialize reports so a slow old session cannot win after a switch. */
    function report(source, ctx) {
        const sessionId = ctx.sessionManager.getSessionId();
        const file = ctx.sessionManager.getSessionFile();
        const payload = JSON.stringify({
            vendor: "pi",
            session_id: sessionId,
            session_file: ctx.sessionManager.isPersisted() && file && existsSync(file) ? file : null,
            source,
        });
        pending = pending.then(() => new Promise((resolve) => {
            if (ctx.sessionManager.getSessionId() !== sessionId) {
                resolve();
                return;
            }
            try {
                // The envelope discriminator, sourced from this
                // asset's own entry point: the payload `vendor`
                // above stays as a consistency check only.
                const child = execFile(executable, ["internal", "hook", "--vendor", "pi"], {
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
    }

    pi.on("session_start", (event, ctx) => report(event.reason, ctx));
    pi.on("agent_end", (_event, ctx) => report("agent_end", ctx));
}
