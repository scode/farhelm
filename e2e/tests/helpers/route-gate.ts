/**
 * Hold a routed fixture request until its test has observed the intended ordering boundary.
 *
 * A fixed response delay only keeps the request open when the browser and
 * test both run quickly enough. This gate leaves the release decision with
 * the assertion that establishes the fixture premise. Always release in
 * finally as well, so a failed assertion cannot strand a route handler.
 * The surrounding Playwright test owns the overall deadline.
 */
export function routeGate(): { wait(): Promise<void>; release(): void } {
  let release!: () => void;
  const opened = new Promise<void>((resolve) => {
    release = resolve;
  });
  return { wait: () => opened, release };
}
