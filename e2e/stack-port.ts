// The one place the browser suite's helm port is decided.
//
// The stack used to listen on a fixed 7434, which meant two Playwright runs on
// one machine (two agents in sibling checkouts, say) collided outright: the
// second helm failed to bind and its run died in `webServer` startup. Every
// other harness in this repo already lets the kernel pick, and the helm itself
// accepts `--port 0`; the only reason this suite could not do the same is that
// Playwright needs `baseURL` and the readiness URL BEFORE it starts the stack,
// so the choice has to happen here, at config load, rather than inside
// `start-stack.sh` where a bind-to-zero would be race-free.
//
// The port therefore travels through the environment. That is not a
// convenience: Playwright re-imports the config in every worker process, and a
// config that picked a fresh port on each import would hand each worker a
// different `baseURL` than the stack was started on. Publishing the first
// choice into `process.env` (workers inherit the runner's environment, the
// same handoff `device-auth.ts` uses for the credential) pins every later
// evaluation to the same value, and `webServer.env` forwards it to the stack
// script, which is otherwise a shell process with no view of this module.
//
// NOTE: the pick-then-bind window is real. Between this module closing its
// probe listener and `farhelm helm run` binding the port, another process can
// take it. The loss is loud (the helm exits, the run fails at startup) rather
// than silent, and the window is milliseconds, which is the same trade
// `scripts/test-install-sh.sh` makes for its fixture server. A run through
// `scripts/record-test-run.py` never sees an ambient override: the recorder
// scrubs `FARHELM_*` from the child environment on purpose, so a recorded run
// always gets a fresh ephemeral port.
import { execFileSync } from "node:child_process";

/** Environment handoff of the chosen port from the runner to workers and the stack script. */
export const STACK_PORT_ENV = "FARHELM_E2E_PORT";

/** Resolve the stack's helm port once per run, honoring an explicit override.
 *
 * Returns the port already published in the environment when there is one
 * (an earlier evaluation in this run, or a human who wants a stable port for
 * poking at the stack by hand), and otherwise asks the kernel for a free
 * loopback port and publishes it. Rejects a malformed override rather than
 * letting it reach the shell as an argument. Never returns 0: an ephemeral
 * choice is resolved here so that the readiness URL and `baseURL` name the
 * real port.
 */
export function harnessStackPort(): number {
  const published = process.env[STACK_PORT_ENV];
  if (published !== undefined && published !== "") {
    if (!/^[0-9]{1,5}$/.test(published)) {
      throw new Error(`${STACK_PORT_ENV} must be a TCP port number, got ${JSON.stringify(published)}`);
    }
    const port = Number(published);
    if (port < 1 || port > 65535) {
      throw new Error(`${STACK_PORT_ENV} is out of range: ${port}`);
    }
    return port;
  }
  const port = freeLoopbackPort();
  process.env[STACK_PORT_ENV] = String(port);
  return port;
}

/** Ask the kernel for a currently free loopback port, synchronously.
 *
 * Node has no synchronous listen, and Playwright does not await an async
 * config export (it reads `module.default` as a plain object), so the probe
 * runs in a child node process whose only output is the port it was given.
 * The child releases the port before exiting; see the module comment for why
 * that window is accepted.
 */
function freeLoopbackPort(): number {
  const probe =
    'const s = require("net").createServer();' +
    's.listen(0, "127.0.0.1", () => { process.stdout.write(String(s.address().port)); s.close(); });';
  const output = execFileSync(process.execPath, ["-e", probe], { encoding: "utf8" }).trim();
  if (!/^[0-9]{1,5}$/.test(output)) {
    throw new Error(`free-port probe printed ${JSON.stringify(output)} instead of a port`);
  }
  return Number(output);
}
