/** The controlled stand-in for `node:child_process.execFile` that the
 * serialization scenarios import in place of the real dependency.
 *
 * The asset's temporary copy imports THIS module (its import of
 * `node:child_process` is rewritten to the file URL the scenario passes), so
 * the production dispatch path — `execFile(executable, args, options,
 * callback)` with a `stdin.end(payload)` child — runs unchanged while the
 * scenario decides when, and whether, each completion callback fires.
 *
 * State is shared with the scenario by module identity: the scenario imports
 * the same file, so `dispatches` is one array both sides see.
 */

/** Every dispatch in order: the reporter invocation plus the payload its
 * child carried. */
export const dispatches = [];

/** The scenario's completion policy: invoked per dispatch with the record
 * and the completion callback. null means "complete immediately". */
let controller = null;

export function setCompletionController(next) {
    controller = next;
}

export function execFile(file, args, options, callback) {
    const record = { file, args, payload: null };
    dispatches.push(record);
    const child = {
        stdin: {
            on() {},
            end(data) {
                record.payload = data ?? null;
            },
        },
    };
    if (controller) {
        controller(record, callback);
    } else {
        callback();
    }
    return child;
}
