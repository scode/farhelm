import { execFile } from "node:child_process";
import { existsSync } from "node:fs";

/** Keep Farhelm's resume target aligned with OMP's FOREGROUND conversation.
 *
 * OMP loads this reporter into every built-in delegated context too: native
 * tasks, workpools, and revived children inherit the parent's prepared
 * extension factories while running under a non-interactive context of their
 * own. Those children share the OS process, use their own session managers,
 * and emit the subscribed events below — so without a gate of its own this
 * reporter would hand a child's conversation to the supervisor as the
 * pane's. The gate is the actual interactive context,
 * `ctx.hasUI === true && ctx.mode === "tui"`, checked BEFORE reading
 * session identity, queueing, or touching any factory state: a child-shaped
 * callback is a complete no-op, including when a vendor getter throws. The
 * check runs again, as a cancellation fence, inside the queued closure
 * before the stale-id/file reads and subprocess creation — see `report`.
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
 * fresh-only (there is no file to verify against resume). A separately
 * launched interactive OMP child passes this gate — it IS an interactive
 * context — so the supervisor's foreground-process admission (not this
 * file) is what refuses it; see the supervisor's OMP ownership proof.
 */
export default function farhelmConversation(omp) {
    const executable = process.env.FARHELM_OMP_REPORTER_EXE;
    if (!executable) return;
    let pending = Promise.resolve();
    // The serial counts ELIGIBLE parent events only, and latestEligible is
    // the latest observed eligible context. Upstream builds each delivered
    // ctx as a snapshot (`mode`/`hasUI` are materialized values, not live
    // getters), so re-reading the captured ctx at execution time cannot
    // detect a later change of ownership — the fence instead cancels a
    // queued entry when a LATER eligible event named a DIFFERENT session.
    // Comparison is by session id, never by serial recency: legitimate
    // multi-event sequences (start, switch, branch, end) still dispatch
    // every report in FIFO order, and only a superseded identity is
    // dropped. A disallowed child callback never advances either piece of
    // parent state.
    let serial = 0;
    let latestEligible = null;

    function isEligibleContext(ctx) {
        try {
            return ctx?.hasUI === true && ctx?.mode === "tui";
        } catch {
            return false;
        }
    }

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
        // The doorway: eligibility before identity, before queueing, before
        // any factory state. An ineligible (child-shaped, unknown, or
        // throwing) callback returns the existing chain unenqueued and
        // unmutated — it must not enqueue a null-file withdrawal, poison
        // ordering, or release/reset the parent's state.
        let eligible = false;
        try {
            eligible = isEligibleContext(ctx);
        } catch {
            eligible = false;
        }
        if (!eligible) {
            return pending;
        }
        try {
            const sessionId = ctx.sessionManager.getSessionId();
            serial += 1;
            const observed = { serial, sessionId };
            latestEligible = observed;
            pending = pending.then(() => new Promise((resolve) => {
                // The fence: a later eligible event for a different session
                // supersedes this entry. Same-session successors do not —
                // their reports queue behind this one in order.
                if (latestEligible !== observed && latestEligible?.sessionId !== sessionId) {
                    resolve();
                    return;
                }
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
                    // The envelope discriminator, sourced from this
                    // asset's own entry point: the payload `vendor`
                    // above stays as a consistency check only.
                    const child = execFile(executable, ["internal", "hook", "--vendor", "omp"], {
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
