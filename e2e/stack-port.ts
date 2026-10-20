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

/** Environment handoff of the browser suite's per-slot ports, comma-separated in slot order.
 *
 * The suite boots one stack per worker slot (`stack-slots.ts`), so it needs
 * several ports chosen together, published once by the runner and read back
 * by every worker for the same reason as the single port above. Each stack
 * script still receives its own port through {@link STACK_PORT_ENV}; this
 * variable is only the runner-to-worker handoff.
 */
export const STACK_PORTS_ENV = "FARHELM_E2E_STACK_PORTS";

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
    return parsePort(published, STACK_PORT_ENV);
  }
  const [port] = freeLoopbackPorts(1);
  process.env[STACK_PORT_ENV] = String(port);
  return port;
}

/** Resolve `count` distinct helm ports once per run, one per stack slot.
 *
 * The multi-stack counterpart of {@link harnessStackPort}, with the same
 * contract: the first evaluation in a run asks the kernel and publishes the
 * answer through {@link STACK_PORTS_ENV}, and every later evaluation (each
 * worker re-imports the config) reads the published list back. An explicit
 * override must name exactly `count` distinct valid ports; anything else is
 * refused rather than padded or truncated, since a slot without its own port
 * would point a worker at another slot's stack.
 */
export function harnessStackPorts(count: number): number[] {
  const published = process.env[STACK_PORTS_ENV];
  if (published !== undefined && published !== "") {
    const ports = published.split(",").map((value) => parsePort(value, STACK_PORTS_ENV));
    if (ports.length !== count || new Set(ports).size !== count) {
      throw new Error(`${STACK_PORTS_ENV} must list ${count} distinct ports, got ${JSON.stringify(published)}`);
    }
    return ports;
  }
  const ports = freeLoopbackPorts(count);
  process.env[STACK_PORTS_ENV] = ports.join(",");
  return ports;
}

/** Validate one port number from the environment before it can reach an argv or a URL. */
function parsePort(value: string, variable: string): number {
  if (!/^[0-9]{1,5}$/.test(value)) {
    throw new Error(`${variable} must hold TCP port numbers, got ${JSON.stringify(value)}`);
  }
  const port = Number(value);
  if (port < 1 || port > 65535) {
    throw new Error(`${variable} is out of range: ${port}`);
  }
  return port;
}

/** Ask the kernel for `count` currently free loopback ports, synchronously.
 *
 * Node has no synchronous listen, and Playwright does not await an async
 * config export (it reads `module.default` as a plain object), so the probe
 * runs in a child node process whose only output is the ports it was given.
 * All of the child's listeners are open at once, which is what makes the
 * ports distinct; it releases them before exiting. See the module comment for
 * why the pick-then-bind window is accepted.
 */
function freeLoopbackPorts(count: number): number[] {
  const probe =
    'const net = require("net"); const n = Number(process.argv[1]); const servers = [];' +
    'const done = () => { if (servers.length < n) return;' +
    '  process.stdout.write(servers.map((s) => s.address().port).join(",")); servers.forEach((s) => s.close()); };' +
    'for (let i = 0; i < n; i++) { const s = net.createServer(); servers.push(s); }' +
    'let listening = 0; servers.forEach((s) => s.listen(0, "127.0.0.1", () => { if (++listening === n) done(); }));';
  const output = execFileSync(process.execPath, ["-e", probe, String(count)], { encoding: "utf8" }).trim();
  const ports = output.split(",");
  if (ports.length !== count || ports.some((port) => !/^[0-9]{1,5}$/.test(port))) {
    throw new Error(`free-port probe printed ${JSON.stringify(output)} instead of ${count} ports`);
  }
  return ports.map(Number);
}
