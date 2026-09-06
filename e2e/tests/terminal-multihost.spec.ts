// =====================================================================
// Multi-host: the hosts panel, the stale list, and host management
// (PLAN_M6.md item 6).
//
// The stack Playwright drives is a two-host FLEET (see start-stack.sh):
// the helm's own machine as the reserved local row, plus a second real
// supervisor on an isolated state directory registered as an
// ssh-to-localhost "remote". Everything below drives that fleet for real
// — a killed supervisor, a genuinely re-registered host — with a set of
// deliberate exceptions that use `page.route` instead, each noted at the
// test that takes it: the local host's not-running state (provoking it for
// real would mean stopping the developer's own supervisor), the identity
// states (provoking those for real means wiping and reinstalling a
// supervisor mid-suite; the helm-side contract for them is pinned in Rust,
// and what is left to prove here is the RENDERING — `identity-mismatch-
// surfaced` — and the request body — `adopt-requires-current-identity`,
// which is the one that actually sends an adoption), the full phase table
// (seven of its
// nine phases have no cheap real cause), and the create dialog's
// vanishing-selection case.
//
// SKIPPING is decided by an INDEPENDENT self-ssh probe, never by "the host
// did not connect" — see `selfSshAvailable`. The distinction is the
// difference between skipping a precondition this suite may not create and
// skipping the bugs it exists to catch. In CI, where self-ssh is
// provisioned, an absent fleet FAILS (`requireFleet`).
//
// Nothing here reads terminal scrollback. Keeping these tests in their own
// file also contains their deliberate fleet mutations behind this file's
// reset and restoration hooks.
// =====================================================================

import { test, expect, Page, APIRequestContext, Locator, Route } from "@playwright/test";
import {
  cleanupProfile,
  createProfile,
  createSession,
  openHostMenu,
  openHostsPanel,
  openRowMenu,
  selfSshAvailable,
  stubFeed,
} from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";
import path from "node:path";
import fs from "node:fs";
import { ChildProcess, spawn, spawnSync } from "node:child_process";
import net from "node:net";
import { cleanupSession, fillCreateForm, waitForTermText } from "./helpers/term";
import {
  cleanUpSessionsTitled,
  FAKE_AGENT_INVOCATION,
  findSessionIdByTitle,
  fulfillAsHelm,
  helmBuild,
  installTerminalSuiteHooks,
  LIVE_BADGE,
  rowByTitle,
  sharedSessionRow,
} from "./helpers/terminal-suite";

installTerminalSuiteHooks();

/**
 * What `start-stack.sh` publishes about the stack it booted.
 *
 * The tests here need four things no API exposes and none of which they
 * may guess: which binary to relaunch the "remote" supervisor from, which
 * state directory it serves (the isolated one — never the developer's real
 * `~/.local/state/farhelm`), which process to kill to make that host go away,
 * and where the injected provisioning backend reads its per-target behavior.
 *
 * The pid is a CLAIM, not an identity, and is never signalled on its own
 * authority — see `verifiedRemoteSupervisorPid`.
 */
type StackInfo = {
  farhelm: string;
  remote_state: string;
  remote_supervisor_pid: number;
  remote_ssh: string;
  provisioning_backend: string;
  /**
   * The file the "remote" supervisor reads its boot id from
   * (`--boot-id-file`, its test-only seam). Rewriting it between a kill
   * and a respawn is how this file simulates a REBOOT of that host — a
   * changed boot id is the one thing the supervisor's interrupted
   * classification rests on, and the only other way to change it is to
   * reboot the machine. "Changed" means a value the supervisor has NEVER
   * recorded, not merely one different from the harness's initial
   * `boot-1`: it persists whatever it adopts, and the second engine's pass
   * through this file reboots again from where the first left off (see
   * `rebootRemoteHost`).
   */
  remote_boot_id_file: string;
};

/**
 * Read the published stack description, failing loudly if it is absent.
 *
 * A missing file means the harness did not write it — a real breakage,
 * not a condition to degrade around, since every fleet test below would
 * otherwise fail one assertion at a time with no hint of the cause.
 */
function stackInfo(): StackInfo {
  const at = path.resolve(__dirname, "../.stack-info.json");
  if (!fs.existsSync(at)) {
    throw new Error(
      `${at} is missing: start-stack.sh publishes it before the helm starts, so the stack under test is not the one this suite expects`,
    );
  }
  return JSON.parse(fs.readFileSync(at, "utf8"));
}

/**
 * Make one injected probe report a supervisor at concrete dial coordinates.
 *
 * The injected backend defaults to `absent`, which is right for provisioning
 * scenarios but wrong for the harness's already-running self-SSH supervisor.
 * This target-only override leaves every unrelated destination on the absent
 * path. Publishing by rename matters because the helm reads this file for
 * each probe and must never observe a half-written JSON document.
 */
function configureDiscoveredProbe(
  destination: string,
  dialFarhelm: string,
  dialStateDir: string | null,
): void {
  const root = stackInfo().provisioning_backend;
  const at = path.join(root, "config.json");
  const config = JSON.parse(fs.readFileSync(at, "utf8"));
  config.targets ??= {};
  config.targets[`ssh:${destination}`] = {
    probe: "supervisor",
    build_version: helmBuild(),
    dial_farhelm: dialFarhelm,
    dial_state_dir: dialStateDir,
  };
  const next = path.join(root, `config.${process.pid}.next`);
  fs.writeFileSync(next, `${JSON.stringify(config)}\n`, { mode: 0o600 });
  fs.renameSync(next, at);
}

/** Every registered host as `GET /api/hosts` currently reports it. */
async function apiHosts(request: APIRequestContext): Promise<any[]> {
  const resp = await request.get("/api/hosts");
  expect(resp.ok(), `GET /api/hosts: ${resp.status()}`).toBe(true);
  return (await resp.json()).hosts;
}

/**
 * The fleet's ssh row — the harness's second supervisor.
 *
 * Found by KIND rather than by id, because its id changes whenever a test
 * removes and re-adds it (a fresh registry row is exactly what SPEC.md's
 * remove-then-re-add contract produces), and by kind rather than by name
 * because the local row is the only other one and is never `ssh`.
 */
async function apiRemoteHost(
  request: APIRequestContext,
): Promise<any | undefined> {
  return (await apiHosts(request)).find((host: any) => host.kind === "ssh");
}

/** Wait until the fleet's ssh row reports `phase`, or fail the test. */
async function waitForRemotePhase(
  request: APIRequestContext,
  phase: string,
  timeout: number,
) {
  await expect
    .poll(async () => (await apiRemoteHost(request))?.state?.phase, {
      timeout,
      message: `waiting for the ssh host to reach ${phase}`,
    })
    .toBe(phase);
}

/**
 * The same wait as an ANSWER rather than an assertion, for the one caller
 * that has to decide something from it instead of failing on it (the fleet
 * probe below).
 */
function remoteReachesPhase(
  request: APIRequestContext,
  phase: string,
  timeout: number,
): Promise<boolean> {
  return waitForRemotePhase(request, phase, timeout).then(
    () => true,
    () => false,
  );
}

/**
 * Escape a literal for use inside a `RegExp`.
 *
 * Host names here are ssh DESTINATIONS, which routinely contain regex
 * metacharacters — a dotted hostname is the common case, and `.` matches
 * anything. Interpolating one raw builds a pattern that quietly matches more
 * rows than it names, so `user@a.b` would also select `user@axb`; with a
 * bracket or a paren in a name it stops being a valid pattern at all and the
 * test fails for a reason that has nothing to do with what it asserts.
 */
function escapeRegExp(literal: string): string {
  return literal.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Locator for one host's row in the panel, matched against `.host-name`
 * exactly — the same anchoring `rowByTitle` uses and for the same reason:
 * a row's full text contains its state detail too, which mentions other
 * hosts (a duplicate names its twin), so `hasText` on the row would match
 * rows that merely refer to the wanted host.
 */
function hostRowByName(page: Page, name: string) {
  return page.locator(".host-row").filter({
    has: page.locator(".host-name", {
      hasText: new RegExp(`^${escapeRegExp(name)}$`),
    }),
  });
}

/**
 * Assert that `locator`'s element is not merely present in the DOM and
 * geometrically inside the viewport, but fully PAINTED there — its center
 * AND all four corners resolve, under a real hit test, to the element
 * itself rather than to a clipping ancestor or a covering surface.
 *
 * A center-only hit test (this function's predecessor, in the clipping
 * regression this closes — F8/TEST-CLIP-EDGE) passes for a button whose
 * EDGE is clipped while its center stays reachable, which is exactly the
 * shape of failure a sidebar's `overflow: hidden` produces: it clips
 * PAINT, not the element's own `getBoundingClientRect()`, so the box lies
 * fully inside the viewport right up until the one pixel of margin this
 * function's corner probes are what actually exercise. Five points — the
 * center plus all four corners, each inset off the boundary edge so the
 * probe lands ON the element's own paint rather than exactly on an edge
 * where rounding could tip either way — is what proves the WHOLE control
 * is reachable, not merely some pixel of it.
 *
 * Each corner's inset is that corner's own `border-radius`, floored at one
 * pixel, rather than a flat pixel for all four. A rounded corner's
 * outermost pixel belongs to NO element: it is outside the element's own
 * painted shape, so `elementFromPoint` there resolves to whatever is
 * painted behind it (and the engines disagree about which side of the
 * curve a point exactly on it falls — Chromium and WebKit both answered
 * three of the panel's four 1px-inset corners with the panel and the
 * fourth with its parent). Insetting by the radius lands the probe on the
 * first pixel of the corner that is genuinely the element's. This costs
 * nothing against the failure the corners exist to catch: a sidebar's
 * `overflow: hidden` eats whole chunks of a control, never just the four
 * pixels of a corner's curve.
 *
 * Throws on a detached element or a page with no real viewport rather than
 * silently returning: a caller here is asserting reachability, and a hard
 * assertion two lines above already means execution never reaches a
 * `null` box or viewport in practice — a dead branch that RETURNED
 * successfully (this function's predecessor) reads as a passing case that
 * can never actually be observed.
 */
async function assertFullyPaintedAndHitTestable(page: Page, locator: Locator): Promise<void> {
  const box = await locator.boundingBox();
  if (box === null) {
    throw new Error("element has no bounding box — is it detached, or display:none?");
  }
  const viewport = page.viewportSize();
  if (viewport === null) {
    throw new Error("page reports no viewport size");
  }

  // Entirely inside the viewport — not merely overlapping it, which a
  // half-clipped button could still do.
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);

  // `parseFloat` reads the computed pixel length these classes all use;
  // a percentage radius would parse to its own number and land the probe
  // somewhere arbitrary, so this deliberately does not pretend to support
  // one — nothing in app.css writes a percentage radius, and a future rule
  // that did would have to teach this helper about it.
  const radii = await locator.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      topLeft: parseFloat(style.borderTopLeftRadius) || 0,
      topRight: parseFloat(style.borderTopRightRadius) || 0,
      bottomLeft: parseFloat(style.borderBottomLeftRadius) || 0,
      bottomRight: parseFloat(style.borderBottomRightRadius) || 0,
    };
  });
  const inset = (radius: number) => Math.max(1, Math.ceil(radius));
  const points = [
    { x: box.x + box.width / 2, y: box.y + box.height / 2 },
    { x: box.x + inset(radii.topLeft), y: box.y + inset(radii.topLeft) },
    { x: box.x + box.width - inset(radii.topRight), y: box.y + inset(radii.topRight) },
    { x: box.x + inset(radii.bottomLeft), y: box.y + box.height - inset(radii.bottomLeft) },
    {
      x: box.x + box.width - inset(radii.bottomRight),
      y: box.y + box.height - inset(radii.bottomRight),
    },
  ];

  // Evaluated ON the element's own handle, not via a CSS selector re-query
  // from `page.evaluate`: a locator can be scoped through several levels
  // of filtering with no single selector string that names it, while
  // `Locator.evaluate` hands the callback the exact bound DOM node.
  //
  // Each probe reports WHAT it hit, not merely whether it hit: a corner
  // that resolves to a clipping ancestor and one that resolves to a
  // covering surface are different bugs, and the failure message is the
  // only place that distinction survives.
  const probes = await locator.evaluate(
    (element, points) =>
      points.map(({ x, y }) => {
        const hit = document.elementFromPoint(x, y);
        return {
          ok: !!hit && (hit === element || element.contains(hit)),
          hit: hit ? `${hit.tagName}.${(hit as HTMLElement).className}` : "nothing",
        };
      }),
    points,
  );
  probes.forEach((probe, index) => {
    expect(
      probe.ok,
      `probe point ${index} (${points[index].x}, ${points[index].y}) missed the element — hit ${probe.hit}`,
    ).toBe(true);
  });
}

/**
 * Whether the two-host fleet is actually up, decided once per project pass.
 *
 * Passwordless `ssh localhost` is a precondition this suite is not entitled
 * to create (writing to the developer's `known_hosts` to get it would be
 * worse than skipping), so where it is absent every test that needs a second
 * real machine is skipped — LOUDLY, with the reason on the skip, exactly as
 * the Rust ssh tests and the cgroup tests skip.
 */
let fleetReady = false;

/**
 * Gate one fleet test — skip where self-ssh is genuinely unavailable, FAIL
 * in CI.
 *
 * The asymmetry is the point. CI provisions self-ssh explicitly (keygen,
 * authorized_keys, sshd — see the workflow), so a fleet that is not up there
 * means the provisioning or the harness broke, and a skip would let the
 * entire multi-host surface go unexercised while the run stayed green. On a
 * developer's machine the same condition is an environment this suite may
 * not modify, and skipping is correct.
 */
function requireFleet() {
  if (process.env.CI) {
    expect(
      fleetReady,
      "CI provisions passwordless self-ssh, so a fleet that is not up is a broken harness rather than a missing prerequisite — this must not be skipped here",
    ).toBe(true);
    return;
  }
  test.skip(
    !fleetReady,
    "the harness's ssh-to-localhost host is not connected (passwordless `ssh localhost` is unavailable here; CI provisions it)",
  );
}


/** The replacement supervisor a down-host test started, if any. */
let restartedRemote: ChildProcess | undefined;

/**
 * Whether anything is actually SERVING the "remote" supervisor's socket.
 *
 * The socket FILE is not the answer: a supervisor that is killed leaves it
 * behind (the next one to hold the state dir's ownership lock is what
 * unlinks it — see the supervisor's own `serve`), so an existence check
 * cannot tell a live supervisor from the corpse of one. Connecting can.
 *
 * Two callers, both of which need that distinction rather than the file's
 * existence: the fleet probe, deciding whether an ssh host that is not
 * connected means an earlier pass through this file took the supervisor
 * down (put one back) or something else is wrong (fail); and the kill path,
 * which polls this to know the supervisor it signalled has genuinely stopped
 * answering before the tests that depend on that begin. Skipping is decided
 * by `selfSshAvailable`, never by this.
 */
async function remoteSupervisorAlive(): Promise<boolean> {
  const socket = path.join(stackInfo().remote_state, "supervisor.sock");
  if (!fs.existsSync(socket)) return false;
  return await new Promise<boolean>((resolve) => {
    const probe = net.connect(socket);
    probe.on("connect", () => {
      probe.destroy();
      resolve(true);
    });
    probe.on("error", () => {
      probe.destroy();
      resolve(false);
    });
  });
}

/**
 * Confirm that `pid` really is this run's remote supervisor before anything
 * signals it.
 *
 * A pid read from a file is a claim, not an identity: the process it named
 * can have exited and the number been reused by something else entirely, and
 * on a developer's machine the something else is their editor as readily as
 * anything. Signalling on the strength of the file alone is how a test
 * harness kills a bystander.
 *
 * The check is a BIRTH-IDENTITY one — the process's own argv, which no
 * later pid reuse can imitate: it must be a `supervisor run` serving exactly
 * this run's isolated remote state directory. Refusing loudly where `/proc`
 * is unavailable is deliberate: this suite runs on Linux (see the CI job),
 * and the alternative to verifying is signalling blind.
 */
function verifiedRemoteSupervisorPid(): number {
  const info = stackInfo();
  const pid = info.remote_supervisor_pid;
  const at = `/proc/${pid}/cmdline`;
  if (!fs.existsSync(at)) {
    throw new Error(
      `refusing to signal pid ${pid}: ${at} does not exist, so it cannot be confirmed to be this run's remote supervisor`,
    );
  }
  // NUL-delimited argv, with a trailing NUL to drop.
  const argv = fs.readFileSync(at, "utf8").split("\0").filter(Boolean);
  const looksRight =
    argv.some((arg) => arg === "supervisor") &&
    argv.some((arg) => arg === info.remote_state);
  if (!looksRight) {
    throw new Error(
      `refusing to signal pid ${pid}: its argv is ${JSON.stringify(argv)}, which is not a supervisor serving ${info.remote_state} — the pid was probably reused`,
    );
  }
  return pid;
}

/**
 * Kill whichever supervisor is currently serving the "remote" state dir, and
 * wait for it to actually be gone.
 *
 * Awaiting matters: the caller's next move is asserting that the helm has
 * noticed, and a supervisor that is merely signalled is still answering.
 */
async function killRemoteSupervisor() {
  // A replacement THIS file started takes precedence: after one down-host
  // pass the pid the harness published is a corpse, and signalling it would
  // either throw or — worse, after pid reuse — hit something else.
  if (restartedRemote) {
    await stopRestartedRemote();
    return;
  }
  process.kill(verifiedRemoteSupervisorPid(), "SIGTERM");
  await expect
    .poll(remoteSupervisorAlive, {
      timeout: 30_000,
      message: "waiting for the killed remote supervisor to stop answering",
    })
    .toBe(false);
}

/**
 * Stop the replacement supervisor this file started, waiting for its exit.
 *
 * Awaited rather than fire-and-forget because the next thing that happens is
 * either another test or the whole run ending: a signalled-but-not-yet-dead
 * supervisor still holds its state directory's ownership lock, so a
 * replacement started immediately afterwards would refuse to serve, and a
 * run that ended here would leak a live process past the suite.
 */
async function stopRestartedRemote() {
  const child = restartedRemote;
  restartedRemote = undefined;
  if (!child || child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  const exited = new Promise<void>((resolve) => {
    child.once("exit", () => resolve());
  });
  child.kill("SIGTERM");
  await exited;
}

/**
 * Put the shared fleet's ssh row back, whatever state a test left it in, and
 * wait for it to be usable again.
 *
 * The two tests that deliberately unregister that host call this from a
 * `finally`. Without it, a failure between the removal and the re-add leaves
 * every later fleet test — and the entire second engine's pass — running
 * against a one-host stack, which reports as a cascade of unrelated
 * failures with one cause buried at the top.
 *
 * Idempotent, and idempotent about the RIGHT row: it checks the destination
 * AND both install fields, not merely that some ssh host exists. A test that
 * failed mid-way can leave a row that is ssh-shaped but wrong — the
 * destination re-added without the harness's install fields, say, which
 * registers happily and then never connects — and accepting it would hand
 * every later test a fleet that looks restored and is not. A row that does
 * not match is replaced rather than adjusted, because the retarget verb
 * deliberately cannot change install fields.
 */
async function restoreFleetRow(request: APIRequestContext) {
  const info = stackInfo();
  const wanted = (host: any) =>
    host.destination === info.remote_ssh &&
    host.remote_farhelm === info.farhelm &&
    host.remote_state_dir === info.remote_state;

  const existing = await apiRemoteHost(request);
  if (existing && !wanted(existing)) {
    const removed = await request.delete(`/api/hosts/${existing.id}`);
    expect(
      removed.ok(),
      `dropping a wrong shared fleet row: ${await removed.text()}`,
    ).toBe(true);
  }
  if (!(await apiRemoteHost(request))) {
    const added = await request.post("/api/hosts", {
      data: {
        ssh: info.remote_ssh,
        remote_farhelm: info.farhelm,
        remote_state_dir: info.remote_state,
      },
    });
    expect(
      added.ok(),
      `restoring the shared fleet row: ${await added.text()}`,
    ).toBe(true);
  }
  await waitForRemotePhase(request, "connected", 60_000);
}

/**
 * Bring the "remote" supervisor back up on its own isolated state dir and
 * wait for the helm to notice.
 *
 * Retries are DRIVEN rather than waited out: a host past its active-retry
 * window is re-probed every 45 seconds, which is most of a test timeout
 * spent asleep, and forcing an attempt is exactly what the retry verb
 * exists for. Poking on each poll is safe — retry is one attempt, not a
 * fresh ladder — and it also covers the window where the new supervisor has
 * not finished binding, since an attempt that lands too early simply fails
 * and the next one does not.
 *
 * Restarting over the killed supervisor's leftover socket is safe: the new
 * process takes the state dir's ownership lock first, which is what proves
 * the file is a corpse rather than a rival.
 */
async function restoreRemoteSupervisor(request: APIRequestContext) {
  const info = stackInfo();
  // The same test-only boot-id seam start-stack.sh launched it with: a
  // respawn that dropped the option would read the machine's real boot id,
  // and every session of the harness's "boot-1" would come back interrupted
  // on a host that never rebooted.
  restartedRemote = spawn(
    info.farhelm,
    ["supervisor", "run", "--state-dir", info.remote_state, "--boot-id-file", info.remote_boot_id_file],
    { stdio: "ignore" },
  );
  await expect
    .poll(
      async () => {
        const host = await apiRemoteHost(request);
        if (host && host.state.phase !== "connected") {
          await request.post(`/api/hosts/${host.id}/retry`);
        }
        return host?.state?.phase;
      },
      {
        timeout: 90_000,
        intervals: [2_000],
        message: "waiting for the restarted remote supervisor to be reconnected",
      },
    )
    .toBe("connected");
}

test.describe("multi-host", () => {
  test.beforeAll(async ({ request }) => {
    // The hook's own budget, not a test's: the resurrection path below can
    // legitimately spend a minute and a half, and the default would abort it
    // half-way and report a fleet that is missing when it is merely slow.
    test.setTimeout(180_000);

    // The skip decision comes from the PRECONDITION, probed directly —
    // never from "the host did not connect", which would let a broken
    // transport, a supervisor that will not start, or a mis-registered
    // ensure file all masquerade as a missing prerequisite and skip the
    // tests that exist to catch them.
    if (!(await selfSshAvailable())) {
      fleetReady = false;
      console.log(
        "SKIPPED the multi-host fleet tests: passwordless `ssh localhost` is unavailable here, " +
          "and this suite may not create it (writing to your known_hosts to get it would be " +
          "worse). CI provisions it, where these tests FAIL rather than skip.",
      );
      return;
    }

    const info = stackInfo();
    configureDiscoveredProbe(info.remote_ssh, info.farhelm, info.remote_state);

    // Self-ssh works, so the fleet is expected to be up — and anything that
    // goes wrong from here is a real failure rather than a reason to skip.
    // In a full run, 45 seconds detects the previous engine's deliberate
    // teardown without spending the longer resurrection budget. When this
    // project is selected directly, it still gives the stack's initial remote
    // supervisor enough time to make its first connection.
    fleetReady = await remoteReachesPhase(request, "connected", 45_000);
    if (!fleetReady) {
      // Nothing serving that state dir is what an EARLIER project's pass
      // through this file leaves behind: its down-host group killed the
      // harness's supervisor, and the replacement was reaped at the end of
      // the file. Put one back — and let a failure to do so FAIL the hook,
      // because with self-ssh working there is no honest reading of it as a
      // missing prerequisite.
      expect(
        await remoteSupervisorAlive(),
        "the ssh host is not connected and its supervisor IS serving: the fleet is broken in a way self-ssh cannot explain",
      ).toBe(false);
      await restoreRemoteSupervisor(request);
      fleetReady = true;
    }
  });

  // Both hosts, both statuses, both identities: the detailed two-host
  // baseline, and the one assertion that proves the fleet is a fleet rather
  // than one host drawn twice. Sidebar coverage pins the collapsed state;
  // this test opens details to verify the evidence behind both rows.
  test("host-list-states: both harness hosts render connected statuses with identities", async ({
    page,
    request,
  }) => {
    requireFleet();

    await page.goto("/");
    await openHostsPanel(page);
    const rows = page.locator(".host-row");
    await expect(rows).toHaveCount(2);

    // The local row: named as itself, never as an address, and with no
    // management affordances — SPEC.md's "never a ghost, never needing
    // registration" is also a promise that it cannot be removed.
    const local = hostRowByName(page, "this machine");
    await expect(local).toHaveAttribute("data-host-phase", "connected");
    await expect(local).toHaveAttribute("data-host-kind", "local");
    // The host panel draws the same locality glyphs as the session row
    // (2026-09-03, `icons.rs`) — a local row gets the local glyph and its
    // hidden word, never the remote one. `data-glyph` pins the actual SVG
    // shape: the count and hidden-word checks alone would still pass if
    // `HostRow` swapped which icon component it called for `Local`/`Ssh`.
    await expect(local.locator(".host-kind-icon")).toHaveCount(1);
    await expect(local.locator(".host-kind-icon")).toHaveAttribute("data-glyph", "local");
    await expect(local.locator(".host-kind-icon + .visually-hidden")).toHaveText("local");
    await expect(local.locator(".host-status .status-dot")).toBeVisible();
    await expect(local.locator(".host-status-label")).toHaveCount(0);
    // Profiles lives in the app bar. The local row menu contains Retry and
    // whichever provisioning command its current setup state permits, but
    // never destination management.
    await openHostMenu(local);
    await expect(local.locator(".host-remove")).toHaveCount(0);
    await expect(local.locator(".host-edit")).toHaveCount(0);

    const info = stackInfo();
    const remote = hostRowByName(page, info.remote_ssh);
    await expect(remote).toHaveAttribute("data-host-phase", "connected");
    await expect(remote).toHaveAttribute("data-host-kind", "ssh");
    await expect(remote.locator(".host-kind-icon")).toHaveCount(1);
    await expect(remote.locator(".host-kind-icon")).toHaveAttribute("data-glyph", "remote");
    await expect(remote.locator(".host-kind-icon + .visually-hidden")).toHaveText("remote");
    await openHostMenu(remote);
    await expect(remote.locator(".host-remove")).toHaveCount(1);

    // The identities are the point of the two rows being two rows: the
    // helm records one per install, and two hosts reporting the SAME one
    // would be a duplicate rather than a fleet. Compared against the API's
    // own answer so this asserts what is rendered matches what is served,
    // rather than merely that something is on screen.
    const hosts = await apiHosts(request);
    const identities = hosts.map((host: any) => host.identity);
    expect(identities.every((identity: string | null) => !!identity)).toBe(true);
    expect(new Set(identities).size).toBe(2);
    for (const host of hosts) {
      await expect(
        page.locator(`[data-host-id="${host.id}"] .host-detail`),
      ).toContainText(host.identity);
    }
  });

  // The regression test for TODO.md's now-removed near-term entry, and the
  // one this whole menu redesign exists to make possible: `toBeVisible()`
  // — used everywhere else in this file — does NOT notice clipping by an
  // ancestor's `overflow: hidden`, which is exactly how `.host-remove`
  // used to render invisible and unclickable off the sidebar's right edge
  // while every other assertion in this suite kept passing.
  // `assertFullyPaintedAndHitTestable` checks the two things `toBeVisible()`
  // cannot: the button's own bounding box lies entirely inside the
  // viewport, and its center AND all four corners actually hit-test to IT
  // (or a descendant of it) rather than to whatever `.app-sidebar`'s clip
  // left painted on top. A `position: fixed` panel escaping that clip (see
  // `.host-row-menu-panel`'s own app.css comment) is what makes both true;
  // a regression back to an inline, sidebar-clipped `.host-remove` would
  // fail the bounding-box check outright and the hit-test the moment the
  // clip started eating even one pixel of it.
  test("host-remove-escapes-the-sidebar-clip: the menu panel is not merely present, it is reachable", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9005,
          kind: "ssh",
          destination: "user@manageable",
          name: "user@manageable",
          identity: "identity-manageable",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "connected",
            identity: "identity-manageable",
            build_version: "0.1.0",
            refresh: { status: "ok", sessions: 0 },
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@manageable");
    await openHostMenu(row);
    const remove = row.locator(".host-remove");
    await expect(remove).toBeVisible();

    // Center-only used to be the whole check here, and a button whose EDGE
    // is clipped while its center stays reachable would still pass it —
    // `assertFullyPaintedAndHitTestable` probes all four corners too. See
    // its own doc (F8/TEST-CLIP-EDGE).
    await assertFullyPaintedAndHitTestable(page, remove);
  });

  // F1/COR-CONFIRM-CLIP: choosing `remove` used to replace `.host-row-main`'s
  // contents with the name, the status, an unshrinkable warning sentence, a
  // second copy of the name, AND both buttons — all on one non-wrapping
  // line, which the 340px sidebar clips exactly the way it once clipped
  // `remove` itself (the regression the previous test guards). This proves
  // the FIX rather than merely the shape of the old bug: both confirmation
  // controls are fully painted and hit-testable after proceeding through
  // the real click path, not just present in the DOM.
  test("host-confirm-remove-fits-the-sidebar: both prompt buttons stay reachable", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9006,
          kind: "ssh",
          // Long enough that the old single-line layout could never have
          // fit it plus the warning sentence plus both buttons — the exact
          // condition F1 describes, not merely a name at the edge of it.
          destination: "user@a-rather-long-manageable-hostname.example.internal",
          name: "user@a-rather-long-manageable-hostname.example.internal",
          identity: "identity-manageable",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "connected",
            identity: "identity-manageable",
            build_version: "0.1.0",
            refresh: { status: "ok", sessions: 0 },
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@a-rather-long-manageable-hostname.example.internal");
    await openHostMenu(row);
    await row.locator(".host-remove").click();

    const confirm = row.locator(".host-confirm-remove");
    const cancel = row.locator(".host-cancel-remove");
    await expect(confirm).toBeVisible();
    await expect(cancel).toBeVisible();

    await assertFullyPaintedAndHitTestable(page, confirm);
    await assertFullyPaintedAndHitTestable(page, cancel);
  });

  /**
   * Removal begins and confirms while Details is collapsed. A refused DELETE
   * must leave its concrete error on the row instead of returning to an
   * apparently unchanged resting state.
   */
  test("a failed removal stays visible with details collapsed", async ({ page }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9021,
          kind: "ssh",
          destination: "user@remove-refusal",
          name: "user@remove-refusal",
          identity: "identity-remove-refusal",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "connected",
            identity: "identity-remove-refusal",
            build_version: "0.1.0",
            refresh: { status: "ok", sessions: 0 },
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });
    await page.route("**/api/hosts/9021", async (route) => {
      if (route.request().method() === "DELETE") {
        await route.fulfill({ status: 409, body: "the host is still required" });
      } else {
        await route.continue();
      }
    });

    await page.goto("/");
    await expect(page.locator(".host-details-toggle")).not.toBeChecked();
    const row = hostRowByName(page, "user@remove-refusal");
    await openHostMenu(row);
    await row.locator(".host-remove").click();
    await row.locator(".host-confirm-remove").click();

    await expect(row.locator(".host-error")).toContainText("the host is still required");
    await expect(page.locator(".host-details-toggle")).not.toBeChecked();
  });

  // F2/COR-HOST-MENU-OFFSCREEN: the longest real phase word
  // (`unreachable-reprobing`) is long enough BY ITSELF — no unusually long
  // host name required — to overflow the header line's available width
  // once the name, the status, and the "⋯" toggle are all accounted for.
  // `.host-row-main` used to allow that line to WRAP (to make room for the
  // removal confirmation below it — see `.host-confirm-remove-panel` in
  // app.css), and wrapping sent the "⋯" onto a line of its own starting at
  // the row's LEFT edge, anchoring its `position: fixed` floating menu off
  // the sidebar's own left edge with it — clipped and unclickable, the
  // same class of bug `host-remove-escapes-the-sidebar-clip` above already
  // guards on the RIGHT edge. This proves the header now stays on one
  // line (the toggle painted at the row's usual right-hand spot) and that
  // its menu, once opened, is fully reachable at every corner — not merely
  // present in the DOM.
  test("host-menu-survives-the-longest-phase-word: the header never wraps its toggle off-screen", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9020,
          kind: "ssh",
          destination: "user@remote-host",
          name: "user@remote-host",
          identity: null,
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "unreachable-reprobing",
            cause: "dial-failed",
            last_error: "connection refused",
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@remote-host");
    await expect(row.locator(".host-status-label")).toHaveText("unreachable, retrying");

    // The toggle itself, BEFORE opening anything: still on the header's one
    // line, fully inside the sidebar, not wrapped away to the left.
    const toggle = row.locator(".host-row-menu");
    await assertFullyPaintedAndHitTestable(page, toggle);

    const [nameBox, statusBox, toggleBox] = await Promise.all([
      row.locator(".host-name").boundingBox(),
      row.locator(".host-status").boundingBox(),
      toggle.boundingBox(),
    ]);
    if (!nameBox || !statusBox || !toggleBox) {
      throw new Error("the host header elements must all have measurable boxes");
    }
    // Same line means the three boxes share a horizontal band, not that
    // their centers coincide: `.host-row-main` aligns them by BASELINE, so a
    // taller toggle and a shorter status word sit on one line with different
    // centers. A wrapped toggle would have no vertical overlap with the name.
    const boxes = [nameBox, statusBox, toggleBox];
    const bandTop = Math.max(...boxes.map((box) => box.y));
    const bandBottom = Math.min(...boxes.map((box) => box.y + box.height));
    expect(bandBottom - bandTop, "the header elements must overlap vertically").toBeGreaterThan(0);

    await openHostMenu(row);
    const panel = row.locator(".host-row-menu-panel");
    await assertFullyPaintedAndHitTestable(page, panel);
    // Every corner of every item too, not just the panel's own box — the
    // same standard `host-remove-escapes-the-sidebar-clip` holds `.host-remove`
    // to, applied here to the whole item set this phase's menu offers (no
    // `.host-adopt`: this phase carries no identity to adopt).
    for (const item of [
      row.locator(".host-retry"),
      row.locator(".provisioning-update"),
      row.locator(".host-edit"),
      row.locator(".host-remove"),
    ]) {
      await assertFullyPaintedAndHitTestable(page, item);
    }
  });

  // F2/COR-STALE-ANCHOR: the add-host form mounts ABOVE `.host-list`, so
  // opening it moves every host row's vertical position — and an already
  // open row menu is a `position: fixed` panel measured at its toggle's OLD
  // coordinates (see `menu_panel_style`'s own doc), left stranded there
  // unless something notices and closes it. Toggling the form open must
  // close a host menu that predates it, not merely leave the panel visually
  // near enough to look plausible.
  test("add-host-form-closes-a-stray-host-menu: the add form's own layout shift is covered", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9007,
          kind: "ssh",
          destination: "user@manageable",
          name: "user@manageable",
          identity: "identity-manageable",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "connected",
            identity: "identity-manageable",
            build_version: "0.1.0",
            refresh: { status: "ok", sessions: 0 },
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@manageable");
    await openHostMenu(row);
    await expect(row.locator(".host-row-menu-panel")).toBeVisible();

    await page.locator(".add-host-button").click();
    await expect(page.locator(".add-host-form")).toBeVisible();
    await expect(row.locator(".host-row-menu-panel")).toHaveCount(0);
    await expect(row.locator(".host-row-menu")).toHaveAttribute("aria-expanded", "false");
  });

  /**
   * The Add host form sits above session rows as well as host rows. Its mount
   * must therefore dismiss a session menu measured against the old layout.
   */
  test("add-host-form-closes-a-stray-session-menu", async ({ page, request }) => {
    const session = await createSession(request, {
      title: `add-form-menu-${Date.now()}`,
      cwd: "/tmp",
      invocation: "sleep 300",
    });
    try {
      await page.goto("/");
      const sessionRow = page.locator(`[data-session-id="${session.id}"]`);
      await expect(sessionRow).toBeVisible();
      await openRowMenu(sessionRow);

      await page.locator(".add-host-button").click();
      await expect(page.locator(".add-host-form")).toBeVisible();
      await expect(sessionRow.locator(".session-row-menu-panel")).toHaveCount(0);
      await expect(sessionRow.locator(".session-row-menu")).toHaveAttribute(
        "aria-expanded",
        "false",
      );
    } finally {
      await cleanupSession(request, session.id);
    }
  });

  // F9/TEST-HOST-KEYBOARD: `menu_panel.rs`'s keyboard mechanics are pinned
  // headlessly, but nothing before this proved `HostRow` wired them to real
  // DOM elements — every other host-menu test drives it by pointer click and
  // finds items by class alone. An adoptable ssh host is used deliberately:
  // it offers five actions, including Update, so the separator's
  // position and the wrap boundary are both exercised on the full list
  // rather than a shorter one.
  test("host-menu-keyboard-contract: role=menu, every item reachable, wraps, Escape/Tab restore focus", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9009,
          kind: "ssh",
          destination: "user@mismatched-keyboard",
          name: "user@mismatched-keyboard",
          identity: "identity-recorded",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "identity-mismatch",
            recorded: "identity-recorded",
            reported: "identity-reported",
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@mismatched-keyboard");
    const toggle = row.locator(".host-row-menu");
    const retry = row.locator(".host-retry");
    const adopt = row.locator(".host-adopt");
    const update = row.locator(".provisioning-update");
    const edit = row.locator(".host-edit");
    const remove = row.locator(".host-remove");
    for (const [key, expected] of [
      ["Enter", retry],
      ["Space", retry],
      ["ArrowDown", retry],
      ["ArrowUp", remove],
    ] as const) {
      await toggle.focus();
      await page.keyboard.press(key);
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
      await expect(expected).toBeFocused();
      await page.keyboard.press("Escape");
      await expect(toggle).toHaveAttribute("aria-expanded", "false");
      await expect(toggle).toBeFocused();
    }

    // Keep the pointer-open path separate from the closed-toggle key cases.
    await openHostMenu(row);
    const menu = row.locator(".host-row-menu-items");
    await expect(menu).toHaveAttribute("role", "menu");
    for (const item of [retry, adopt, update, edit, remove]) {
      await expect(item).toHaveAttribute("role", "menuitem");
    }

    // Focus is put on the toggle EXPLICITLY, rather than left where the
    // opening click put it: this walks the list from OUTSIDE it.
    await toggle.focus();
    await expect(toggle).toBeFocused();

    await page.keyboard.press("ArrowDown");
    await expect(retry).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(adopt).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(update).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(edit).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(remove, "the separator is not a stop on the way to remove").toBeFocused();
    // Both wrap boundaries, in the two directions that reach them.
    await page.keyboard.press("ArrowDown");
    await expect(retry).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(remove).toBeFocused();

    await page.keyboard.press("Home");
    await expect(retry).toBeFocused();
    await page.keyboard.press("End");
    await expect(remove).toBeFocused();

    await page.keyboard.press("Escape");
    await expect(row.locator(".host-row-menu-panel")).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    // The half a plain "Escape closes it" assertion would miss: closing
    // destroys the element that held focus, and without the handoff the
    // user lands on the document body, dozens of Tab presses from the row
    // they were working on.
    await expect(toggle).toBeFocused();

    // Tab exits and restores focus identically to Escape.
    await openHostMenu(row);
    await toggle.focus();
    await page.keyboard.press("ArrowDown");
    await expect(retry).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(row.locator(".host-row-menu-panel")).toHaveCount(0);
    await expect(toggle).toBeFocused();
  });

  /**
   * Automatic setup occupies its canonical slot after Retry in the truthful
   * local setup menu. The three-item fixture pins ordering and both wrap
   * boundaries without fabricating mutually exclusive provisioning offers;
   * the third item is "edit alias", offered on the local row like any
   * other (plans/host-aliases.md), which is what the wrap boundaries now
   * land on — automatic setup sits between, so its slot is proven by the
   * arrow step from Retry rather than by End.
   */
  test("automatic-setup-menu-order: keyboard navigation includes the conditional setup command", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      const local = body.hosts.find((host: any) => host.kind === "local");
      local.state = {
        phase: "unreachable-reprobing",
        cause: "local-supervisor-not-running",
        last_error: "the local supervisor is absent",
      };
      body.hosts = [local];
      await route.fulfill({ response, json: body });
    });
    await page.route("**/api/hosts/*/provisioning", async (route) => {
      const hostId = Number(new URL(route.request().url()).pathname.split("/").at(-2));
      await route.fulfill({
        status: 200,
        headers: { "content-type": "application/json", "x-farhelm-build": helmBuild() },
        body: JSON.stringify({
          host_id: hostId,
          run_id: null,
          operation: null,
          status: "completed",
          steps: [],
          message: null,
        }),
      });
    });
    await page.route("**/api/hosts/probe", async (route) => {
      const body = route.request().postDataJSON() as { target?: { kind?: string } };
      if (body.target?.kind === "local") {
        await route.fulfill({ status: 503, body: "automatic setup needs retry" });
      } else {
        await route.continue();
      }
    });

    await page.goto("/");
    const row = page.locator('[data-host-kind="local"]');
    await expect(row.locator(".provisioning-error")).toContainText("automatic setup needs retry");
    await openHostMenu(row);
    const items = row.getByRole("menuitem");
    await expect(items).toHaveText(["retry", "set up automatically", "edit alias"]);
    const retry = row.locator(".host-retry");
    const automatic = row.locator(".provisioning-auto-setup");
    const alias = row.locator(".host-alias");

    await expect(retry).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(automatic).toBeFocused();
    await page.keyboard.press("End");
    await expect(alias).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(retry).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(alias).toBeFocused();
    await page.keyboard.press("Home");
    await expect(retry).toBeFocused();
  });

  // F11/TEST-EDIT-CLOSE: nothing before this ACTIVATED Edit — every existing
  // test only checked it was present or disabled. This proves the whole
  // lifecycle: choosing it closes the "⋯" menu before the row swaps to the
  // destination field, the existing destination is copied into the draft,
  // and cancelling returns an ordinary row with no menu revived behind it.
  test("host-edit-destination-lifecycle: closes the menu, prefills, cancel restores the row", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9011,
          kind: "ssh",
          destination: "user@editable",
          name: "user@editable",
          identity: "identity-editable",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "connected",
            identity: "identity-editable",
            build_version: "0.1.0",
            refresh: { status: "ok", sessions: 0 },
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await expect(page.locator(".host-details-toggle")).not.toBeChecked();
    const row = hostRowByName(page, "user@editable");
    await openHostMenu(row);
    await row.locator(".host-edit").click();
    await expect(page.locator(".host-details-toggle")).toBeChecked();

    // Gone the instant the row swaps to the destination field — nothing
    // here dismisses it by hand.
    await expect(row.locator(".host-row-menu-panel")).toHaveCount(0);
    const input = row.locator(".host-destination-input");
    await expect(input).toBeVisible();
    await expect(input).toHaveValue("user@editable");

    await page.locator(".host-details-toggle").click();
    await expect(page.locator(".host-details-toggle")).not.toBeChecked();
    await expect(input).toBeVisible();
    await expect(input).toHaveValue("user@editable");

    await row.locator(".host-cancel-edit").click();

    // Back to the ordinary row, and no menu revived behind it.
    await expect(row.locator(".host-destination-input")).toHaveCount(0);
    await expect(row.locator(".host-row-menu-panel")).toHaveCount(0);
    await expect(row.locator(".host-row-menu")).toHaveAttribute("aria-expanded", "false");
  });

  // F14/TEST-BUSY-GUARDS: `aria-disabled`, unlike a native `disabled`
  // attribute, does not stop a click or a keyboard activation from
  // reaching the button — each rendered item handler has to refuse
  // busy on its OWN. Existing tests check only the disabled APPEARANCE.
  // This holds one operation open on the (always-present) local host, then
  // arrows through every item of an UNRELATED, fully adoptable host's menu
  // and activates each one, proving that no request any of them could have
  // caused reaches that other host (the row's own background provisioning
  // read is named and excluded below) and that nothing opens — while also
  // proving keyboard navigation still REACHES every item, so a fix that
  // swapped `aria-disabled` for native `disabled` (which would also satisfy
  // "no request, nothing opens") fails this test too.
  test("host-menu-busy-guards: aria-disabled items refuse activation but stay reachable", async ({
    page,
    request,
  }) => {
    const local = (await apiHosts(request)).find((host: any) => host.kind === "local");
    let releaseLocalRetry: () => void = () => {};
    const localRetryHeld = new Promise<void>((resolve) => {
      releaseLocalRetry = resolve;
    });
    await page.route(`**/api/hosts/${local.id}/retry`, async (route) => {
      await localRetryHeld;
      await route.continue();
    });
    // Everything addressed to this host, under BOTH shapes its endpoints
    // take: the sub-resources (`/retry`, `/adopt`, `/update`,
    // `/destination`) and the host resource itself, which is what a
    // removal's `DELETE` targets (api.rs). A glob covering only the
    // sub-resources would make `remove`'s guard unobservable here.
    const targetHits: string[] = [];
    const recordTargetHit = async (route: Route) => {
      targetHits.push(`${route.request().method()} ${new URL(route.request().url()).pathname}`);
      await route.continue();
    };
    await page.route("**/api/hosts/9013", recordTargetHit);
    await page.route("**/api/hosts/9013/*", recordTargetHit);
    // The one request on this host that no menu item can cause: every host
    // row mounts a provisioning panel that reads its own host's retained
    // run (`provisioning.rs`) on mount, on every feed notice, and on a
    // fallback poll. It is background traffic with no relationship to the
    // menu, it arrives at times this test does not control, and none of
    // the six item handlers would ever issue it — so it is named and
    // excluded here rather than allowed to stand in for the guards this
    // test is actually about.
    const backgroundProvisioningRead = "GET /api/hosts/9013/provisioning";
    const menuAttributableHits = () =>
      targetHits.filter((hit) => hit !== backgroundProvisioningRead);

    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9013,
          kind: "ssh",
          destination: "user@busy-target",
          name: "user@busy-target",
          identity: "identity-recorded",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "identity-mismatch",
            recorded: "identity-recorded",
            reported: "identity-reported",
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });
    await page.route("**/api/hosts/9013/provisioning", async (route) => {
      await route.fulfill({
        status: 200,
        headers: { "content-type": "application/json", "x-farhelm-build": helmBuild() },
        body: JSON.stringify({
          host_id: 9013,
          run_id: "failed-busy-menu",
          operation: "update",
          status: "failed",
          steps: [],
          message: "retry the failed update",
        }),
      });
    });

    // Every step below runs while a stalled route holds the shared
    // operation token. Releasing it has to happen even when an assertion
    // throws: a route left waiting outlives the test body, and Playwright's
    // own page teardown then blocks on it until the suite's `afterEach`
    // times out — which buries the real failure under a second, unrelated
    // one.
    try {
      await page.goto("/");
      await openHostsPanel(page);
      const source = page.locator(`[data-host-id="${local.id}"]`);
      const target = hostRowByName(page, "user@busy-target");

      // Hold ONE operation open on an unrelated row — the page's single
      // shared operation token, so every OTHER row's items go
      // `aria-disabled` too.
      await openHostMenu(source);
      await source.locator(".host-retry").click();

      await openHostMenu(target);
      const retry = target.locator(".host-retry");
      const adopt = target.locator(".host-adopt");
      const rerun = target.locator(".provisioning-rerun");
      const update = target.locator(".provisioning-update");
      const edit = target.locator(".host-edit");
      const remove = target.locator(".host-remove");
      const items = [retry, adopt, rerun, update, edit, remove];
      for (const item of items) {
        await expect(item).toHaveAttribute("aria-disabled", "true");
      }

      // Reachable despite being disabled: arrow navigation still lands on
      // every one of them, in the declared order.
      const toggle = target.locator(".host-row-menu");
      await toggle.focus();
      for (const item of items) {
        await page.keyboard.press("ArrowDown");
        await expect(item).toBeFocused();
      }

      // The baseline the activation assertion below is measured against:
      // opening the panel and walking it with the arrow keys must already
      // have reached nothing on this host. Asserted rather than assumed,
      // so a request that arrives BEFORE any activation is reported here,
      // where it happened, instead of being blamed on a failed guard.
      expect(menuAttributableHits()).toEqual([]);

      // Activated anyway. `aria-disabled` (unlike the native `disabled`
      // attribute) does not remove an element from the accessibility tree or
      // stop it receiving events — it is a semantic hint, not a browser
      // enforcement mechanism — but Playwright's own actionability checks
      // treat it as disabled and WAIT for it to clear before a plain
      // `.click()` will dispatch anything, which would time this test out
      // without ever exercising a single handler's guard
      // (F6/TEST-ARIA-DISABLED-CLICK). `dispatchEvent` bypasses Playwright's
      // actionability checks entirely and fires the event directly on the
      // element — exactly the activation a native `<button>` still answers
      // to despite `aria-disabled`, which is the real hazard this test
      // exists to catch: each handler's OWN guard has to refuse, because the
      // DOM will not refuse it first.
      for (const item of items) {
        await item.dispatchEvent("click");
      }

      // Nothing opened…
      await expect(target.locator(".host-destination-input")).toHaveCount(0);
      await expect(target.locator(".host-confirm-remove")).toHaveCount(0);
      await expect(target.locator(".host-row-menu-panel")).toBeVisible();
      // …and nothing the menu could have caused reached this host's own
      // endpoints.
      expect(menuAttributableHits()).toEqual([]);
    } finally {
      releaseLocalRetry();
    }
  });

  /**
   * Automatic setup has its own conditional item and click handler. Holding
   * an unrelated retry proves that item remains focusable but cannot enqueue
   * a second local probe while the page operation lock is busy.
   */
  test("automatic setup refuses forced activation while another host operation is busy", async ({
    page,
  }) => {
    let releaseSource!: () => void;
    const heldSource = new Promise<void>((resolve) => {
      releaseSource = resolve;
    });
    let localProbeCount = 0;
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      const local = body.hosts.find((host: any) => host.kind === "local");
      local.state = {
        phase: "unreachable-reprobing",
        cause: "local-supervisor-not-running",
        last_error: "the local supervisor is absent",
      };
      body.hosts = [
        local,
        {
          id: 9014,
          kind: "ssh",
          destination: "user@busy-source",
          name: "user@busy-source",
          identity: "identity-busy-source",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "connected",
            identity: "identity-busy-source",
            build_version: "0.1.0",
            refresh: { status: "ok", sessions: 0 },
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });
    await page.route("**/api/hosts/*/provisioning", async (route) => {
      const hostId = Number(new URL(route.request().url()).pathname.split("/").at(-2));
      await route.fulfill({
        status: 200,
        headers: { "content-type": "application/json", "x-farhelm-build": helmBuild() },
        body: JSON.stringify({
          host_id: hostId,
          run_id: null,
          operation: null,
          status: "completed",
          steps: [],
          message: null,
        }),
      });
    });
    await page.route("**/api/hosts/probe", async (route) => {
      const body = route.request().postDataJSON() as { target?: { kind?: string } };
      if (body.target?.kind === "local") {
        localProbeCount += 1;
        await route.fulfill({ status: 503, body: "automatic setup needs retry" });
      } else {
        await route.continue();
      }
    });
    await page.route("**/api/hosts/9014/retry", async (route) => {
      await heldSource;
      await route.continue();
    });

    try {
      await page.goto("/");
      const local = page.locator('[data-host-kind="local"]');
      const source = page.locator('[data-host-id="9014"]');
      await expect(local.locator(".provisioning-error")).toContainText(
        "automatic setup needs retry",
      );
      expect(localProbeCount).toBe(1);

      await openHostMenu(source);
      await source.locator(".host-retry").click();
      await openHostMenu(local);
      const automatic = local.locator(".provisioning-auto-setup");
      await expect(automatic).toHaveAttribute("aria-disabled", "true");
      await automatic.focus();
      await expect(automatic).toBeFocused();
      await automatic.dispatchEvent("click");
      expect(localProbeCount).toBe(1);
    } finally {
      releaseSource();
    }
  });

  // The local host is ALWAYS listed, and when its supervisor is not running
  // it says so with a manual path — never an offer to install anything
  // (provisioning is M7's, and PLAN_M6.md is explicit that a registered
  // destination with no supervisor gets a hint, not an installer).
  //
  // Route-intercepted, and this is the one place in this file where that is
  // not merely convenient: producing the state for real means stopping the
  // supervisor this suite's every other test depends on, on the developer's
  // own machine. The state itself is the helm's to produce; what is under
  // test here is entirely this UI's rendering of it.
  test("local-host-always-listed: the local row renders its manual-start hint", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = body.hosts.map((host: any) =>
        host.kind === "local"
          ? {
              ...host,
              state: {
                phase: "unreachable-reprobing",
                cause: "local-supervisor-not-running",
                // The helm's OWN dial failure, in the shape
                // `farhelm_supervisor::service::connect` actually produces:
                // an anyhow chain whose middle layer carries the exact
                // start command, state directory and all. That exactness is
                // the point of the fixture — the UI's job here is to hand
                // that command to the user rather than paraphrase it from
                // facts it does not have (the state dir is not on
                // /api/hosts and never will be).
                last_error:
                  "no supervisor is running on this machine: supervisor does not appear to be "
                  + "running (socket /srv/fh-state/supervisor.sock is not accepting connections); "
                  + "start it with `farhelm supervisor run --state-dir /srv/fh-state`: "
                  + "Connection refused (os error 111)",
              },
            }
          : host,
      );
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const local = hostRowByName(page, "this machine");
    await expect(local).toHaveAttribute(
      "data-host-phase",
      "unreachable-reprobing",
    );
    await expect(local.locator(".host-status-label")).toHaveText(
      "unreachable, retrying",
    );
    // The remedy is the helm's own sentence, with the state directory
    // intact: a hint that said only `farhelm supervisor run` would send the
    // user to start a supervisor their helm never dials, and leave the row
    // exactly as it was after they did what it told them.
    await expect(local.locator(".host-remedy")).toContainText(
      "farhelm supervisor run --state-dir /srv/fh-state",
    );
    // And it appears ONCE: the diagnosis line beside it says what happened,
    // not the same long chain over again.
    await expect(local.locator(".host-detail")).not.toContainText(
      "farhelm supervisor run",
    );
    // The row stays unmanageable even while it is down: it is still the
    // reserved local row, and offering remove would offer an operation the
    // helm refuses outright. Checked inside the OPEN menu — `.host-remove`
    // now lives there, not on the row line.
    await openHostMenu(local);
    await expect(local.locator(".host-remove")).toHaveCount(0);
  });

  // An identity change at a known destination freezes the host and asks the
  // user to decide, naming BOTH identities — SPEC.md forbids silently
  // merging two installs, and the decision is not presentable without both.
  //
  // Route-intercepted by choice, and the choice is worth stating: producing
  // a real mismatch means wiping and reinstalling a supervisor mid-suite,
  // which would destroy the very host the tests around this one depend on.
  // The helm-side contract is already pinned against a real manager in Rust
  // (farhelm-helm's `adopting_resolves_an_identity_mismatch_and_purges_the_old_cache`
  // and `adopting_without_the_displayed_identity_is_refused`), so what is
  // left for a browser to prove — and what only a browser can — is that
  // this UI renders the decision and sends the identity it displayed.
  test("identity-mismatch-surfaced: both identities and an adopt for the reported one", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9001,
          kind: "ssh",
          destination: "user@reinstalled",
          name: "user@reinstalled",
          identity: "identity-before",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "identity-mismatch",
            recorded: "identity-before",
            reported: "identity-after",
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@reinstalled");
    await expect(row).toHaveAttribute("data-host-phase", "identity-mismatch");
    await expect(row.locator(".host-status-label")).toHaveText("identity mismatch");
    // Both, because the decision is between them.
    await expect(row.locator(".host-detail")).toContainText("identity-before");
    await expect(row.locator(".host-detail")).toContainText("identity-after");
    // The control names what adopting would accept, so the click and the
    // sentence above it cannot disagree. `.host-adopt` now lives inside
    // the row's "⋯" menu.
    await openHostMenu(row);
    await expect(row.locator(".host-adopt")).toHaveText("adopt identity-after");
  });

  // An identity-UNVERIFIED host must offer no adopt at all. It looks
  // adjacent to a mismatch and is not: the host answered with no identity,
  // so there is nothing to compare and the helm refuses the verb. Offering
  // it would put a button on screen whose only possible outcome is a
  // refusal, while implying a decision the user does not have — which is
  // why the helm's own state docs make this a renderer obligation rather
  // than a suggestion.
  test("identity-unverified-offers-no-adopt: the remedies are named instead", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9002,
          kind: "ssh",
          destination: "user@silent",
          name: "user@silent",
          identity: "identity-recorded",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "identity-unverified",
            recorded: "identity-recorded",
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@silent");
    await expect(row).toHaveAttribute("data-host-phase", "identity-unverified");
    await openHostMenu(row);
    await expect(row.locator(".host-adopt")).toHaveCount(0);
    await expect(row.locator(".host-detail")).toContainText(
      "identity-recorded",
    );
    // The three things that DO help, since adopting is not one of them.
    await expect(row.locator(".host-remedy")).toContainText("retarget");
    await expect(row.locator(".host-remedy")).toContainText("remove");
  });

  // The adopt request must carry the identity the user was SHOWN, not
  // whatever the host reports when the click lands. A re-probe can change
  // the reported identity between the prompt appearing and the request
  // arriving, and an empty-bodied adopt would then silently adopt a third
  // install — so the helm 409s a stale approval, and this pins both halves
  // of the UI's side: what it sends, and that it surfaces the refusal.
  test("adopt-requires-current-identity: the displayed identity is sent, and a 409 is shown", async ({
    page,
  }) => {
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = [
        ...body.hosts.filter((host: any) => host.kind === "local"),
        {
          id: 9003,
          kind: "ssh",
          destination: "user@racing",
          name: "user@racing",
          identity: "identity-before",
          remote_farhelm: null,
          remote_state_dir: null,
          state: {
            phase: "identity-mismatch",
            recorded: "identity-before",
            reported: "identity-displayed",
          },
        },
      ];
      await route.fulfill({ response, json: body });
    });

    // The helm's refusal, in its own words — the shape `http_error` gives a
    // superseded adoption, which is prose rather than JSON.
    const refusal =
      "host 9003 now reports identity-since-changed, not identity-displayed; look again and decide against what it reports now";
    let adoptBody: any;
    await page.route("**/api/hosts/9003/adopt", async (route) => {
      adoptBody = JSON.parse(route.request().postData() ?? "null");
      await fulfillAsHelm(route, { status: 409, contentType: "text/plain", body: refusal });
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, "user@racing");
    await openHostMenu(row);
    await row.locator(".host-adopt").click();

    // The refusal is the helm's, verbatim, in this row's own error line —
    // never a generic "adopt failed", which would leave the user unable to
    // tell a race from a bug.
    await expect(row.locator(".host-error")).toContainText(refusal);
    expect(
      adoptBody,
      "the adopt must name the identity that was displayed, which is the whole content of the promise the helm checks",
    ).toEqual({ reported: "identity-displayed" });
  });

  // Adding a host through the form registers it and the status then reports
  // what was found — here, a real supervisor, so the row progresses to
  // connected on its own.
  //
  // The host it adds is the harness's own "remote", deliberately: a second
  // entry for the same install would be a DUPLICATE (correctly), and a
  // destination with no supervisor behind it could only ever prove the
  // unreachable path, which `unreachable-host-goes-stale` already covers
  // against a real one. So the entry is dropped through the API first —
  // setup, not the thing under test — and re-registered through the form,
  // which is also what exercises the two optional install fields the
  // harness's isolated state directory makes mandatory.
  test("add-host-discovers: a form-registered ssh host progresses to connected", async ({
    page,
    request,
  }) => {
    requireFleet();
    const info = stackInfo();

    // The DELETE is inside the protected block, not before it. Outside, a
    // failure between removing the row and entering the `try` would skip the
    // restore entirely and leave every later fleet test — and the whole
    // second engine's pass — running against a one-host stack.
    try {
      const existing = await apiRemoteHost(request);
      expect(existing, "the harness registers its remote through --ensure-hosts").toBeTruthy();
      const removed = await request.delete(`/api/hosts/${existing.id}`);
      expect(removed.ok(), `removing the ssh host: ${await removed.text()}`).toBe(true);

      await page.goto("/");
      await openHostsPanel(page);
      await expect(hostRowByName(page, info.remote_ssh)).toHaveCount(0);

      await page.locator(".add-host-button").click();
      const form = page.locator(".add-host-form");
      await form.locator(".add-host-ssh").fill(info.remote_ssh);
      await form.locator(".add-host-farhelm").fill(info.farhelm);
      await form.locator(".add-host-state-dir").fill(info.remote_state);
      await form.locator(".add-host-submit").click();

      // The row appears at once — registration does not wait for a
      // connection — and reaches connected without any further action.
      const row = hostRowByName(page, info.remote_ssh);
      await expect(row).toBeVisible();
      await expect(row).toHaveAttribute("data-host-phase", "connected", {
        timeout: 60_000,
      });
      // The install fields reached the row, not just the form: without them
      // this entry would dial a farhelm and a state directory that are not
      // the harness's, and would never have connected at all.
      const readded = await apiRemoteHost(request);
      expect(readded.remote_farhelm).toBe(info.farhelm);
      expect(readded.remote_state_dir).toBe(info.remote_state);
    } finally {
      // The shared fleet row is restored whatever happened above. This test
      // deliberately unregisters the host every other fleet test depends on,
      // and a failure between the removal and the re-add would otherwise
      // leave the rest of the file — and the whole second engine's pass —
      // running against a one-host stack, reporting a cascade of failures
      // that all have one cause.
      await restoreFleetRow(request);
    }
  });

  // The create dialog's host selector, end to end: a session created on the
  // SECOND host appears in the one merged list, tagged to that host.
  test("create-dialog-host-selector: a session is created on the chosen host", async ({
    page,
    request,
  }) => {
    requireFleet();
    const info = stackInfo();
    const title = `on-the-remote-${Date.now()}`;
    let id: string | undefined;
    try {
      await page.goto("/");
      await openHostsPanel(page);
      await expect(hostRowByName(page, info.remote_ssh)).toHaveAttribute(
        "data-host-phase",
        "connected",
      );

      const form = await fillCreateForm(page, {
        cwd: "/tmp",
        invocation: FAKE_AGENT_INVOCATION,
        title,
      });
      // By LABEL, which is the helm's own display name for the host — the
      // same string the session row will carry, so selecting and asserting
      // key off one vocabulary rather than two.
      await form
        .locator(".create-session-host")
        .selectOption({ label: info.remote_ssh });
      await form.locator(".create-session-submit").click();

      // A successful create navigates into the new session, exactly as a
      // local one does; back out to see how the list describes it.
      await expect(page.locator(".titlebar .title")).toHaveText(title, {
        timeout: 30_000,
      });
      const row = rowByTitle(page, title);
      await expect(row.locator(".session-host")).toHaveText(info.remote_ssh, {
        timeout: 30_000,
      });
      // A remote row carries `data-host-locality` and draws the remote
      // glyph beside its name (2026-09-03) — the multihost half of the
      // locality rule; sidebar.spec.ts covers the local and unknown
      // verdicts, which do not need a live second host.
      await expect(row).toHaveAttribute("data-host-locality", "remote");
      await expect(row.locator(".host-kind-icon")).toHaveCount(1);
      // `data-glyph`, independently of `data-host-locality`: the row's own
      // attribute proves the VERDICT, not which svg component actually
      // rendered — a `HostLocality::Remote` arm that called `LocalHostIcon`
      // by mistake would still satisfy every other assertion here.
      await expect(row.locator(".host-kind-icon")).toHaveAttribute("data-glyph", "remote");
      // Adjacent-sibling, not a bare `.visually-hidden` lookup: a live
      // status badge (this session is `running`) hides its own word the
      // same way, and a loose selector would be ambiguous between the two
      // clipped spans on the same row.
      await expect(row.locator(".host-kind-icon + .visually-hidden")).toHaveText("remote");
      id = await findSessionIdByTitle(request, title);

      // The list's own answer must agree with the row: the session lives on
      // the selected host, not on the local one the body would have
      // defaulted to had the selection been dropped.
      const remote = await apiRemoteHost(request);
      const listing = await (await request.get("/api/sessions")).json();
      const created = listing.sessions.find((s: any) => s.title === title);
      expect(created.host).toBe(remote.id);
    } finally {
      if (!id) id = await findSessionIdByTitle(request, title);
      if (id) await cleanupSession(request, id);
    }
  });

  // Clone across hosts, end to end (`create_form.rs`'s `pending_choice`):
  // cloning a row that lives on the SECOND host, while the create
  // dialog's ordinary default points at the FIRST, must still land the
  // create on the row's own host with the row's own profile — not on
  // wherever the dialog would otherwise have defaulted. The profile comes
  // from the helm-wide catalog, so changing hosts must preserve it.
  test("clone-cross-host: a remote row's clone carries its own host and profile across the handoff", async ({
    page,
    request,
  }) => {
    requireFleet();
    const info = stackInfo();
    const remote = await apiRemoteHost(request);

    const profile = await createProfile(request, {
      name: `clone-remote-profile-${Date.now()}`,
    });
    const title = `clone-remote-source-${Date.now()}`;
    const source = await createSession(request, {
      title,
      cwd: "/tmp",
      host: remote.id,
      profile_id: profile.id,
    });
    // Opened FIRST so the dialog's ordinary default (SPEC.md's "the open
    // session's host") names the LOCAL machine — the opposite of where
    // the row this test clones actually lives. Without this anchor, a
    // clone that landed on the remote host by coincidence of already
    // being the effective default would look identical to one that
    // genuinely carried its own row's host across.
    const anchorTitle = `clone-remote-anchor-${Date.now()}`;
    const anchor = await createSession(request, { title: anchorTitle, cwd: "/tmp" });

    let cloneId: string | undefined;
    try {
      await page.goto("/");
      const anchorRow = rowByTitle(page, anchorTitle);
      await expect(anchorRow).toBeVisible({ timeout: 20_000 });
      await anchorRow.locator(".session-row-open").click();
      await expect(page.locator(".titlebar .title")).toHaveText(anchorTitle, {
        timeout: 20_000,
      });

      const sourceRow = rowByTitle(page, title);
      await expect(sourceRow).toBeVisible({ timeout: 20_000 });
      await openRowMenu(sourceRow);
      await sourceRow.locator(".session-row-clone").click();

      const form = page.locator(".create-session-form");
      await expect(form).toBeVisible();
      // The selector follows the CLONED row's host across the handoff
      // (`pending_choice`), not the anchor session's host the dialog
      // would otherwise have opened onto.
      await expect(form.locator(".create-session-host")).toHaveValue(String(remote.id), {
        timeout: 20_000,
      });
      await expect(form.locator(".create-session-profile")).toHaveValue(profile.id, {
        timeout: 20_000,
      });

      const newCwd = stackScratchDir("clone-remote-e2e-");
      await form.locator('input[type="text"]').nth(0).fill(newCwd);
      const [response] = await Promise.all([
        page.waitForResponse(
          (r) => r.request().method() === "POST" && r.url().endsWith("/api/sessions"),
        ),
        form.locator(".create-session-submit").click(),
      ]);
      // The wire body itself, not just what the picker showed — the two
      // could disagree if the reseed effect wrote the picker's display
      // without also writing what submit actually reads.
      const sent = response.request().postDataJSON();
      expect(sent.host).toBe(remote.id);
      expect(sent.profile_id).toBe(profile.id);

      const body = await response.json();
      cloneId = body.id as string;
      const clonedRow = page.locator(`.session-row[data-session-id="${cloneId}"]`);
      await expect(clonedRow).toBeVisible({ timeout: 20_000 });
      await expect(clonedRow.locator(".session-host")).toHaveText(info.remote_ssh, {
        timeout: 20_000,
      });
    } finally {
      if (cloneId) await cleanupSession(request, cloneId);
      await cleanupSession(request, anchor.id);
      await cleanupSession(request, source.id);
      await cleanupProfile(request, profile.id);
    }
  });

  // Clone host reconciliation can remain Waiting while the helm-wide catalog
  // has already made every agent option usable. The hosts GET is deliberately
  // held below so the explicit pick is made inside that state, then released:
  // the later host Bind may select the source installation, but it must never
  // reclaim ownership of an agent choice the user already made.
  test("clone-cross-host-explicit-pick: an agent chosen while hosts are pending survives bind", async ({
    page,
    request,
  }) => {
    requireFleet();
    const remote = await apiRemoteHost(request);

    const clonedProfile = await createProfile(request, {
      name: `clone-remote-cloned-${Date.now()}`,
    });
    const explicitProfile = await createProfile(request, {
      name: `clone-remote-explicit-${Date.now()}`,
    });
    const title = `clone-remote-explicit-source-${Date.now()}`;
    const source = await createSession(request, {
      title,
      cwd: "/tmp",
      host: remote.id,
      profile_id: clonedProfile.id,
    });
    // Opened first for the same reason `clone-cross-host` opens one: with
    // no anchor the dialog's ordinary default might already name the
    // remote host by coincidence, and this handoff only exists at all when
    // the clone has to MOVE the selector away from wherever it opened.
    const anchorTitle = `clone-remote-explicit-anchor-${Date.now()}`;
    const anchor = await createSession(request, { title: anchorTitle, cwd: "/tmp" });

    let releaseHosts: (() => void) | undefined;
    const heldHosts = new Promise<void>((resolve) => {
      releaseHosts = resolve;
    });
    await page.route(
      (url) => url.pathname === "/api/hosts",
      async (route: Route) => {
        if (route.request().method() === "GET") await heldHosts;
        await route.continue();
      },
    );

    let cloneId: string | undefined;
    try {
      await page.goto("/");
      const anchorRow = rowByTitle(page, anchorTitle);
      await expect(anchorRow).toBeVisible({ timeout: 20_000 });
      await anchorRow.locator(".session-row-open").click();
      await expect(page.locator(".titlebar .title")).toHaveText(anchorTitle, {
        timeout: 20_000,
      });

      const sourceRow = rowByTitle(page, title);
      await expect(sourceRow).toBeVisible({ timeout: 20_000 });
      await openRowMenu(sourceRow);
      await sourceRow.locator(".session-row-clone").click();

      const form = page.locator(".create-session-form");
      await expect(form).toBeVisible();
      await expect(form.locator(".create-session-host")).toHaveValue("");
      await expect(form.locator(`.create-session-profile option[value="${explicitProfile.id}"]`))
        .toHaveCount(1, { timeout: 20_000 });
      await form.locator(".create-session-profile").selectOption(explicitProfile.id);

      releaseHosts!();

      // The host still followed the clone across the handoff — only the
      // agent was overridden.
      await expect(form.locator(".create-session-host")).toHaveValue(String(remote.id), {
        timeout: 20_000,
      });
      await expect(form.locator(".create-session-profile")).toHaveValue(explicitProfile.id);

      const newCwd = stackScratchDir("clone-remote-explicit-e2e-");
      await form.locator('input[type="text"]').nth(0).fill(newCwd);
      const [response] = await Promise.all([
        page.waitForResponse(
          (r) => r.request().method() === "POST" && r.url().endsWith("/api/sessions"),
        ),
        form.locator(".create-session-submit").click(),
      ]);
      const sent = response.request().postDataJSON();
      expect(sent.host).toBe(remote.id);
      expect(
        sent.profile_id,
        "the explicit pick must reach the wire, not the clone's own profile the handoff had queued",
      ).toBe(explicitProfile.id);

      const body = await response.json();
      cloneId = body.id as string;
      await expect(page.locator(`.session-row[data-session-id="${cloneId}"]`)).toBeVisible({
        timeout: 20_000,
      });
    } finally {
      if (cloneId) await cleanupSession(request, cloneId);
      await cleanupSession(request, anchor.id);
      await cleanupSession(request, source.id);
      await cleanupProfile(request, clonedProfile.id);
      await cleanupProfile(request, explicitProfile.id);
    }
  });

  // A host that GOES AWAY, driven for real: the harness's remote supervisor
  // is killed, and everything that follows from that — the row's phase,
  // its sessions' staleness, what an operation against them does, and what
  // opening one shows — is asserted against the actual helm.
  //
  // Serial and grouped because they share one expensive, destructive setup:
  // there is exactly one remote supervisor, and killing it per test would
  // pay the active-retry window (about a minute) three times over. The
  // group restores it afterwards, so everything after this file's point
  // still sees a two-host fleet.
  test.describe.serial("with the remote supervisor killed", () => {
    let staleSessionId: string | undefined;
    const staleTitle = `stale-on-remote-${Date.now()}`;

    test.beforeAll(async ({ request }) => {
      if (!fleetReady) return;
      const remote = await apiRemoteHost(request);
      const created = await request.post("/api/sessions", {
        data: {
          cwd: "/tmp",
          invocation: "sleep 600",
          title: staleTitle,
          host: remote.id,
        },
      });
      expect(
        created.ok(),
        `creating a session on the remote host: ${await created.text()}`,
      ).toBe(true);
      staleSessionId = (await created.json()).id;

      // Wait for the helm to have REFRESHED this session from its host
      // before taking that host away.
      //
      // Not padding: a create's reply reports `Unknown` deliberately (the
      // supervisor cannot claim the agent has execed yet) and the helm seeds
      // its cache from exactly that reply, so a host killed inside the first
      // refresh interval leaves "unknown" as the honest last-known status.
      // That is correct behavior and a coin toss to assert against — it
      // failed on one engine and passed on the other. Waiting for a probed
      // status makes the last-known one a real observation, which is what
      // `stale-session-metadata-view` is actually about.
      await expect
        .poll(
          async () => {
            const listing = await (await request.get("/api/sessions")).json();
            return listing.sessions.find((s: any) => s.id === staleSessionId)
              ?.status?.state;
          },
          {
            timeout: 30_000,
            message: "waiting for the helm to refresh the remote session's status",
          },
        )
        // Any LIVE status will do — the point is that the helm has probed
        // this session at all, so its last-known status is a real
        // observation rather than the create-time placeholder. Asserting
        // one exact word here would fail the moment the classifier decided
        // a quiet agent was idle.
        .toMatch(LIVE_BADGE);

      // The one thing no API can do. SIGTERM rather than SIGKILL so the
      // supervisor unwinds; either way the helm loses its connection when
      // the process serving it goes. Awaited, so the tests below start
      // against a host that is genuinely gone rather than one that has
      // merely been signalled.
      await killRemoteSupervisor();
    });

    test.afterAll(async ({ request }) => {
      if (!fleetReady) return;
      // The hook's own budget: bringing the host back is a real reconnect,
      // and every test after this group depends on it having happened.
      test.setTimeout(180_000);
      await restoreRemoteSupervisor(request);
      if (staleSessionId) await cleanupSession(request, staleSessionId);
    });

    // SPEC.md: sessions on an unreachable host "stay in the list from the
    // helm's last-known knowledge, clearly marked". Both halves are the
    // assertion — the status reaching the phase that says re-probing
    // continues forever, and the rows staying put with their marking.
    test("unreachable-host-goes-stale: the status re-probes and its sessions are marked", async ({
      page,
      request,
    }) => {
      requireFleet();
      // The active-retry window is about a minute of real backoff before
      // the phase becomes `unreachable-reprobing`, which is the phase under
      // test: a shorter wait would only ever observe `connecting`.
      test.setTimeout(240_000);
      const info = stackInfo();

      await page.goto("/");
      await openHostsPanel(page);
      // Staleness arrives FIRST and does not wait for the retry ladder: a
      // host stops being connected the moment its connection drops, and
      // every one of its rows is last-known knowledge from that instant.
      const row = rowByTitle(page, staleTitle);
      await expect(row.locator(".stale-badge")).toBeVisible({ timeout: 60_000 });
      await expect(row).toHaveAttribute("data-session-stale", "true");
      // Still listed, not vanished — the actual promise.
      await expect(row.locator(".session-title")).toHaveText(staleTitle);

      await waitForRemotePhase(request, "unreachable-reprobing", 180_000);
      await expect(hostRowByName(page, info.remote_ssh)).toHaveAttribute(
        "data-host-phase",
        "unreachable-reprobing",
        { timeout: 30_000 },
      );
      // The transport's own words, which is the only thing anyone can
      // actually search for when a host will not answer.
      await expect(
        hostRowByName(page, info.remote_ssh).locator(".host-detail"),
      ).not.toBeEmpty();
    });

    // SPEC.md: "Opening such a session shows its metadata — title,
    // directory, last-known status — behind a clear host-unreachable
    // notice; there is no terminal to show." All three clauses are
    // asserted, the last one negatively: no terminal element at all, rather
    // than a terminal that happens to be blank.
    test("stale-session-metadata-view: metadata behind the notice, and no terminal", async ({
      page,
    }) => {
      requireFleet();
      const info = stackInfo();

      await page.goto("/");
      await rowByTitle(page, staleTitle).locator(".session-row-open").click();

      // SPEC.md's metadata triple, all three of it: title, directory, and
      // the LAST-KNOWN status. The status is the one an earlier shape of
      // this view dropped — the titlebar carries the first two, and the
      // restart offer beside them describes what a relaunch WOULD do rather
      // than what the session last was.
      await expect(page.locator(".titlebar .title")).toHaveText(staleTitle);
      await expect(page.locator(".titlebar .meta")).toHaveText(
        "/tmp — sleep 600",
      );
      const badge = page.locator(".stale-metadata .status-badge");
      await expect(badge).toBeVisible();
      // `sleep 600` was running, and the group's setup waited for the helm
      // to have OBSERVED that before killing the host — so the last thing
      // the helm knew is that it was alive. Rendered with the list's own
      // badge, so the two surfaces cannot describe one session differently.
      await expect(badge).toHaveText(LIVE_BADGE);
      // The one-badge rule's other half, asserted on the real browser
      // rather than only in `status_badge_destination`'s unit test: a
      // stale session's status appears in EXACTLY the stale band and
      // NOWHERE in the header, never both.
      await expect(page.locator(".titlebar .status-badge")).toHaveCount(0);
      await expect(page.locator(".stale-metadata .status-badge")).toHaveCount(1);

      const notice = page.locator(".host-stale-notice");
      await expect(notice).toBeVisible();
      // The host is named, and by its ACTUAL state rather than a generic
      // "unreachable" — a skewed or identity-frozen host reaching this
      // surface must not be described as merely down, and the only way to
      // keep that true is to render the phase the helm reports.
      await expect(notice).toContainText(info.remote_ssh);
      await expect(notice).toContainText("unreachable, retrying");
      await expect(notice).toContainText("no terminal");

      expect(
        await page.locator(".terminal").count(),
        "a stale session must mount no terminal at all, not an empty one",
      ).toBe(0);
      expect(await page.locator(".tab-strip").count()).toBe(0);
    });

    // SPEC.md: operations against a session on an unreachable host "are
    // refused with a clear error; nothing queues for later delivery in v1".
    // The row's data attribute keeps the stable wire phase for selectors;
    // the visible refusal must name that state in humanized prose because
    // "it failed" is not something a user can act on.
    test("op-refused-on-unreachable: the helm's own 409 words, and nothing queued", async ({
      page,
      request,
    }) => {
      requireFleet();

      await page.goto("/");
      const row = rowByTitle(page, staleTitle);
      // The controls are deliberately still live on a stale row: the
      // helm's refusal is a better answer than a disabled button that
      // explains nothing.
      await openRowMenu(row);
      await row.locator(".session-row-stop").click();

      const error = row.locator(".action-error");
      await expect(error).toContainText("unreachable-reprobing");
      await expect(error).toContainText("nothing was queued");

      // "Nothing queued" is a claim about the SERVER, so it is checked
      // there: the session is still exactly as it was, and no stop is
      // waiting to be delivered when the host returns.
      const listing = await (await request.get("/api/sessions")).json();
      const still = listing.sessions.find((s: any) => s.title === staleTitle);
      expect(still, "a refused stop must not remove the row").toBeTruthy();
      expect(still.stale).toBe(true);
    });
  });

  // A REBOOT of the remote host, simulated the way the Rust suite's reboot
  // tests define one: the supervisor and its tmux server go away, the boot
  // id the supervisor reads changes (start-stack.sh's `--boot-id-file`
  // seam, rewritten between the kill and the respawn), and the supervisor
  // comes back on the same state directory. A changed boot id alone is not
  // a reboot — reload keeps any pane it still finds — which is why the tmux
  // server is killed too. What the helm then shows for sessions that were
  // live is SPEC.md's interrupted contract, end to end against real
  // binaries rather than an injected listing row: no terminal is mounted or
  // retried, the surface says why, declining changes nothing, and only the
  // user's restart relaunches anything.
  //
  // Serial and grouped for the same reason as the group above: one
  // expensive, destructive setup that every test here observes a different
  // side of. The first test performs the reboot itself, because the
  // transition — a session OPEN while its host reboots — is one of the
  // things under test and cannot be observed from a `beforeAll`.
  test.describe.serial("after a simulated reboot of the remote host", () => {
    const openTitle = `open-through-reboot-${Date.now()}`;
    const closedTitle = `closed-through-reboot-${Date.now()}`;
    const ids: string[] = [];

    /** Create a fake-agent session on the remote and wait until the helm has seen it live. */
    async function createLiveRemoteSession(request: APIRequestContext, title: string) {
      const remote = await apiRemoteHost(request);
      const created = await request.post("/api/sessions", {
        data: { cwd: "/tmp", invocation: FAKE_AGENT_INVOCATION, title, host: remote.id },
      });
      expect(created.ok(), `creating ${title} on the remote host: ${await created.text()}`).toBe(true);
      const id: string = (await created.json()).id;
      ids.push(id);
      // Live as OBSERVED by the helm before the reboot: only a session last
      // known running is interrupted by a boot, and the create reply's
      // placeholder status is not an observation.
      await expect
        .poll(
          async () => {
            const listing = await (await request.get("/api/sessions")).json();
            return listing.sessions.find((s: any) => s.id === id)?.status?.state;
          },
          { timeout: 30_000, message: `waiting for the helm to see ${title} running` },
        )
        .toMatch(LIVE_BADGE);
      return id;
    }

    /**
     * Reboot the remote host, as far as a test can: supervisor down, its
     * tmux server down, a boot id the supervisor has never recorded,
     * supervisor back on the same state directory and reconnected.
     *
     * The boot id must be one the supervisor has NEVER stored, not merely
     * "different from the harness's initial one": the supervisor persists
     * the id it adopts, and a rewrite to a value it already holds is the
     * same-boot path, under which a live row whose pane is gone becomes
     * `exited`, never `interrupted`. The second engine's pass through this
     * file, `--retries`, and `--repeat-each` all reboot again from
     * whatever the previous reboot left, so a fixed literal would make
     * every reboot after the first a silent no-op.
     *
     * The tmux kill is checked both ways — the command ran, and the server
     * is gone afterwards — because the classification does not depend on
     * it: with the boot id changed, every live row converts to interrupted
     * whether or not its pane survived, and a test that let the server
     * live would pass while the agents kept running in it.
     */
    async function rebootRemoteHost(request: APIRequestContext) {
      const info = stackInfo();
      const sock = path.join(info.remote_state, "tmux.sock");
      // The same tmux the supervisor drives: it honors `FARHELM_TMUX` ahead
      // of PATH, and a client from a different build fails the socket
      // handshake with a protocol-version mismatch rather than killing
      // anything.
      const tmux = process.env.FARHELM_TMUX || "tmux";
      await killRemoteSupervisor();
      const killed = spawnSync(tmux, ["-S", sock, "kill-server"], { encoding: "utf8" });
      expect(killed.error, "tmux must be runnable from the test process").toBeUndefined();
      expect(
        killed.status === 0 || /no server running|No such file/.test(killed.stderr),
        `kill-server on the remote's tmux: ${killed.stderr}`,
      ).toBe(true);
      const probe = spawnSync(tmux, ["-S", sock, "list-sessions"], { encoding: "utf8" });
      expect(probe.status, `the remote's tmux server must be gone: ${probe.stdout}${probe.stderr}`).not.toBe(0);
      fs.writeFileSync(info.remote_boot_id_file, `reboot-${process.pid}-${Date.now()}\n`);
      await restoreRemoteSupervisor(request);
    }

    test.beforeAll(async ({ request }) => {
      if (!fleetReady) return;
      test.setTimeout(120_000);
      await createLiveRemoteSession(request, openTitle);
      await createLiveRemoteSession(request, closedTitle);
    });

    // The reboot happens inside a test body, so a failure between the kill
    // and the restore would otherwise leave the host down for every later
    // test in the file and make `cleanupSession` throw on the first id.
    // Restore only when nothing is serving: a second supervisor on a served
    // state directory bails on the ownership lock, and tracking it would
    // orphan the live one past the file's reaper.
    test.afterAll(async ({ request }) => {
      if (!fleetReady) return;
      test.setTimeout(180_000);
      if (!(await remoteSupervisorAlive())) await restoreRemoteSupervisor(request);
      for (const id of ids) {
        try {
          await cleanupSession(request, id);
        } catch (error) {
          console.log(`cleanup of ${id} after the reboot group failed: ${error}`);
        }
      }
    });

    // The transition: a session whose terminal is on screen when its host
    // reboots. The view passes through the host-unreachable surface while
    // the host is down (that part is the group above's), and must settle on
    // the interrupted surface once the host is back — with no terminal
    // element left behind for the reconnect ladder to keep retrying into.
    test("reboot-while-open: an attached session settles on the interrupted surface", async ({
      page,
      request,
    }) => {
      requireFleet();
      test.setTimeout(240_000);

      await page.goto("/");
      await rowByTitle(page, openTitle).locator(".session-row-open").click();
      await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
        timeout: 20_000,
      });
      await waitForTermText(page, "FAKE-AGENT READY");

      await rebootRemoteHost(request);

      const notice = page.locator(".interrupted-notice");
      await expect(notice).toBeVisible({ timeout: 60_000 });
      await expect(notice).toContainText("did not survive the host reboot");
      await expect(page.locator(".titlebar .status-badge")).toHaveText("interrupted");
      await expect(page.locator("#terminal")).toHaveCount(0);
      await expect(page.locator("#term-connecting")).toHaveCount(0);
      await expect(page.locator(".host-stale-notice")).toHaveCount(0);
      // SETTLED is what these assert, deliberately: everything above is
      // "eventually", so a brief remount while the host reads as connected
      // but not yet refreshed (its rows still last-known live) is not
      // excluded here. What must not survive the refresh is any terminal
      // element, because that is what the reconnect ladder retries into;
      // a view that kept one would show the overlay again within its first
      // retry, which is well inside this window.
      await page.waitForTimeout(5_000);
      await expect(page.locator("#terminal")).toHaveCount(0);
      await expect(page.locator("#term-connecting")).toHaveCount(0);
      await expect(page.locator(".titlebar .status-badge")).toHaveText("interrupted");
    });

    // Opening a session that was ALREADY interrupted when the view loads,
    // and declining: the row and the session stay exactly as they were.
    test("reboot-already-interrupted: opening mounts no terminal, and leaving changes nothing", async ({
      page,
      request,
    }) => {
      requireFleet();
      await page.goto("/");
      const row = rowByTitle(page, closedTitle);
      await expect(row.locator(".status-badge")).toHaveText("interrupted", { timeout: 60_000 });
      await row.locator(".session-row-open").click();
      await expect(page.locator(".interrupted-notice")).toBeVisible();
      await expect(page.locator("#terminal")).toHaveCount(0);
      await expect(page.locator("#term-connecting")).toHaveCount(0);
      await expect(page.locator(".tab-strip")).toHaveCount(0);
      // Declining is leaving. Nothing was sent, so the supervisor still
      // classifies the session as interrupted and the row still says so.
      await sharedSessionRow(page).click();
      await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
      await expect(rowByTitle(page, closedTitle).locator(".status-badge")).toHaveText("interrupted");
      const listing = await (await request.get("/api/sessions")).json();
      const still = listing.sessions.find((s: any) => s.title === closedTitle);
      expect(still?.status?.state, "declining must leave the session interrupted").toBe("interrupted");
    });

    // The one way forward: restart from the surface relaunches the agent in
    // a new terminal, which the view then attaches like any other. What this
    // proves is relaunch-and-attach, not resume: the fake agent has no
    // conversation identity, so its offer is a fresh launch, and the resume
    // template's own behavior is the Rust restart suite's to pin.
    test("reboot-restart: restart from the interrupted surface attaches the new terminal", async ({
      page,
    }) => {
      requireFleet();
      test.setTimeout(120_000);
      await page.goto("/");
      await rowByTitle(page, closedTitle).locator(".session-row-open").click();
      const restart = page.locator(".interrupted-notice .restart-from-notice");
      await expect(restart).toBeVisible();
      await restart.click();
      await expect(page.locator(".interrupted-notice")).toHaveCount(0, { timeout: 60_000 });
      await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
        timeout: 60_000,
      });
      await waitForTermText(page, "FAKE-AGENT READY");
      await expect(page.locator(".titlebar .status-badge")).toHaveText(LIVE_BADGE, {
        timeout: 60_000,
      });
    });
  });

  // SPEC.md's remove-merely-forgets contract, executable end to end
  // (PLAN_M6.md names this test): removing forgets the host AND the
  // sessions the helm cached for it, while the host itself — its
  // supervisor, its running agent — is untouched, so re-adding the same
  // destination rediscovers everything.
  //
  // The session is created through the API and never stopped: that is what
  // makes the rediscovery meaningful. A session that had been stopped would
  // reappear just as readily from a re-registration that had somehow killed
  // things on the way out.
  test("remove-and-re-add-host: removal forgets, re-adding rediscovers", async ({
    page,
    request,
  }) => {
    requireFleet();
    test.setTimeout(120_000);
    const info = stackInfo();
    const title = `survives-removal-${Date.now()}`;
    let id: string | undefined;

    try {
      const remote = await apiRemoteHost(request);
      const created = await request.post("/api/sessions", {
        data: {
          cwd: "/tmp",
          invocation: "sleep 600",
          title,
          host: remote.id,
        },
      });
      expect(created.ok(), `creating on the remote: ${await created.text()}`).toBe(true);
      id = (await created.json()).id;

      await page.goto("/");
      await openHostsPanel(page);
      // Explicitly bounded rather than left on the 5s default: the row
      // appears only after the client's next listing poll (a three-second
      // cadence, and a walk is several round trips), so the default is
      // barely one interval and flakes on a loaded runner.
      await expect(rowByTitle(page, title).locator(".session-host")).toHaveText(
        info.remote_ssh,
        { timeout: 30_000 },
      );

      // Remove through the in-page confirmation — wry has no native
      // dialogs, so there is no browser prompt to accept, and the flow is
      // the same on both renderers.
      const row = hostRowByName(page, info.remote_ssh);
      await openHostMenu(row);
      await row.locator(".host-remove").click();
      await expect(row.locator(".confirm-consequence")).toContainText(
        "leaves its supervisor and sessions running",
      );
      await row.locator(".host-confirm-remove").click();

      await expect(hostRowByName(page, info.remote_ssh)).toHaveCount(0, {
        timeout: 30_000,
      });
      // The cached sessions went with the host: a row left behind would be
      // a session with no host to name.
      await expect(rowByTitle(page, title)).toHaveCount(0, { timeout: 30_000 });

      // Re-register the same destination. A fresh registry row, a fresh
      // host id — and the same install behind it.
      await page.locator(".add-host-button").click();
      const form = page.locator(".add-host-form");
      await form.locator(".add-host-ssh").fill(info.remote_ssh);
      await form.locator(".add-host-farhelm").fill(info.farhelm);
      await form.locator(".add-host-state-dir").fill(info.remote_state);
      await form.locator(".add-host-submit").click();

      await expect(hostRowByName(page, info.remote_ssh)).toHaveAttribute(
        "data-host-phase",
        "connected",
        { timeout: 60_000 },
      );
      // The session the removal forgot is back, live rather than stale,
      // because it never stopped running.
      const rediscovered = rowByTitle(page, title);
      await expect(rediscovered).toBeVisible({ timeout: 30_000 });
      await expect(rediscovered).toHaveAttribute("data-session-stale", "false");
      await expect(rediscovered.locator(".session-host")).toHaveText(
        info.remote_ssh,
      );
      // The AGENT survived, which is the half of SPEC.md's contract a
      // reappearing row does not prove on its own: a re-registration that
      // had killed and relaunched things would produce exactly the same row.
      // `sleep 600` was never stopped, so anything other than alive means
      // the removal reached past the registry.
      await expect(rediscovered.locator(".status-badge")).toHaveText(LIVE_BADGE);
      const listing = await (await request.get("/api/sessions")).json();
      const survivor = listing.sessions.find((s: any) => s.title === title);
      expect(
        survivor.id,
        "the rediscovered session must be the SAME session, not a lookalike relaunched under one name",
      ).toBe(id);
    } finally {
      // The HOST goes back FIRST, and the order is load-bearing rather than
      // tidy. This test can fail while the host is unregistered, and the
      // helm routes a session operation by owner lookup — with no host, the
      // session is not in the merged view at all, so `cleanupSession`'s
      // 404-tolerant stop and delete both succeed at doing nothing and the
      // `sleep 600` on the remote is leaked into every test that follows.
      // Restoring first puts the session back in the view where the cleanup
      // can actually reach it.
      await restoreFleetRow(request);
      if (!id) id = await findSessionIdByTitle(request, title);
      if (id) await cleanupSession(request, id);
    }
  });

  // Every phase, in one intercepted reply, each field carrying a sentinel
  // no other field could produce.
  //
  // A per-phase spot check cannot catch what this does: a renderer that
  // dropped one field, or printed some other variant's payload, still emits
  // plausible-looking text. Unique sentinels make each assertion about THAT
  // field in THAT row, and the table is exhaustive so a phase added later
  // arrives here without coverage rather than silently unrendered.
  test("host-list-phase-table: every phase renders status and details", async ({
    page,
  }) => {
    const phases = [
      {
        id: 8001,
        phase: "connecting",
        display: "connecting",
        state: { phase: "connecting", attempt: 3, last_error: "sentinel-connecting" },
        needles: ["3", "sentinel-connecting"],
      },
      {
        id: 8002,
        phase: "unreachable-reprobing",
        display: "unreachable, retrying",
        state: {
          phase: "unreachable-reprobing",
          cause: "transport-failure",
          last_error: "sentinel-unreachable",
        },
        needles: ["sentinel-unreachable"],
      },
      {
        id: 8003,
        phase: "connected",
        display: null,
        state: {
          phase: "connected",
          identity: "sentinel-identity",
          build_version: "sentinel-build",
          refresh: { status: "ok", sessions: 7 },
        },
        needles: ["sentinel-identity", "sentinel-build", "7 sessions"],
      },
      {
        id: 8004,
        phase: "version-skew",
        display: "version skew",
        state: {
          phase: "version-skew",
          peer_protocol: 99,
          peer_build: "sentinel-peer-build",
          our_protocol: 8,
          our_build: "sentinel-our-build",
          remediation: "sentinel-remediation",
        },
        needles: ["99", "sentinel-peer-build", "sentinel-our-build"],
        remedy: "sentinel-remediation",
      },
      {
        id: 8005,
        phase: "identity-mismatch",
        display: "identity mismatch",
        state: {
          phase: "identity-mismatch",
          recorded: "sentinel-recorded",
          reported: "sentinel-reported",
        },
        needles: ["sentinel-recorded", "sentinel-reported"],
      },
      {
        id: 8006,
        phase: "identity-unverified",
        display: "identity unverified",
        state: { phase: "identity-unverified", recorded: "sentinel-unverified" },
        needles: ["sentinel-unverified"],
      },
      {
        id: 8007,
        phase: "duplicate",
        display: "duplicate",
        state: { phase: "duplicate", twin: 4242, identity: "sentinel-duplicate" },
        needles: ["4242", "sentinel-duplicate"],
      },
      {
        id: 8008,
        phase: "retired",
        display: "retired",
        state: { phase: "retired", reason: "sentinel-retired" },
        needles: ["sentinel-retired"],
      },
      // Not a state the helm can be in — it is what a UI one version behind
      // sees, and the panel must degrade that ONE row rather than the fleet.
      {
        id: 8009,
        phase: "unrecognized",
        display: "unrecognized",
        state: { phase: "invented-by-a-later-helm" },
        needles: ["does not know"],
      },
      // A different forward-compat seam from the one above: not an
      // unrecognized PHASE, but an unrecognized host `kind` — the wire
      // value a newer helm might send for a registry row kind this build
      // has no name for (`HostKind::Unrecognized`, `#[serde(other)]`).
      // Every entry above hardcodes `kind: "ssh"` below, which is exactly
      // why this seam had no coverage: a `HostRow` that drew a local or
      // remote glyph for an unrecognized kind would not fail any of them.
      {
        id: 8010,
        phase: "connected",
        display: null,
        kind: "quantum-mesh",
        state: {
          phase: "connected",
          identity: "sentinel-unrecognized-kind",
          build_version: "sentinel-unrecognized-kind-build",
          refresh: { status: "ok", sessions: 0 },
        },
        needles: ["sentinel-unrecognized-kind", "sentinel-unrecognized-kind-build"],
      },
    ];

    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = phases.map((entry) => ({
        id: entry.id,
        kind: entry.kind ?? "ssh",
        destination: `user@${entry.phase}`,
        name: `user@${entry.phase}`,
        identity: null,
        remote_farhelm: null,
        remote_state_dir: null,
        state: entry.state,
      }));
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await openHostsPanel(page);
    await expect(page.locator(".host-row")).toHaveCount(phases.length);
    for (const entry of phases) {
      const row = page.locator(`[data-host-id="${entry.id}"]`);
      await expect(row).toBeVisible();
      await expect(row).toHaveAttribute("data-host-phase", entry.phase);
      const status = row.locator(".host-status");
      await expect(status.locator(".status-dot")).toBeVisible();
      if (entry.display === null) {
        await expect(row.getByRole("status", { name: "connected" })).toHaveCount(1);
        await expect(status.locator(".host-status-label")).toHaveCount(0);
      } else {
        await expect(status.locator(".host-status-label")).toHaveText(entry.display);
      }
      const detail = row.locator(".host-detail");
      await expect(detail).toBeVisible();
      for (const needle of entry.needles) {
        await expect(detail).toContainText(needle);
      }
      if (entry.remedy) {
        await expect(row.locator(".host-remedy")).toContainText(entry.remedy);
      }
      // The unrecognized-kind entry: `data-host-kind` degrades to its own
      // forward-compat token, and the row draws NEITHER locality glyph —
      // asserting either one would be the same invented claim
      // `list::shared::session_locality`'s `Unknown` case refuses to make
      // for a session row (`hosts.rs`'s icon-selection doc).
      if (entry.kind && entry.kind !== "ssh") {
        await expect(row).toHaveAttribute("data-host-kind", "unrecognized");
        await expect(row.locator(".host-kind-icon")).toHaveCount(0);
        await expect(row.locator(".visually-hidden")).toHaveCount(0);
      }
    }
  });

  // Retry is offered in every state and must actually DIAL: it is one
  // attempt rather than a shortened wait, and for a retired host it is the
  // only thing that brings the actor back at all.
  test("host-retry-click: the control posts the retry verb for its own host", async ({
    page,
    request,
  }) => {
    const local = (await apiHosts(request)).find((host: any) => host.kind === "local");
    const retried: string[] = [];
    await page.route("**/api/hosts/*/retry", async (route) => {
      retried.push(new URL(route.request().url()).pathname);
      await route.continue();
    });

    await page.goto("/");
    await openHostsPanel(page);
    const row = page.locator(`[data-host-id="${local.id}"]`);
    await openHostMenu(row);
    await expect(row.locator(".host-retry")).toBeVisible();
    await row.locator(".host-retry").click();

    await expect
      .poll(() => retried, { message: "waiting for the retry POST" })
      .toEqual([`/api/hosts/${local.id}/retry`]);
    // The local row survives its own retry: this is a reconnect, not a
    // removal, and SPEC.md has the helm's own machine always listed.
    await expect(row).toBeVisible();
  });

  // Cancelling a removal must leave the host exactly as it was — the safe
  // half of the confirmation, and the one a focus-on-cancel default exists
  // to make easy to reach by accident.
  test("remove-cancel: backing out of the prompt forgets nothing", async ({
    page,
    request,
  }) => {
    requireFleet();
    const info = stackInfo();
    const before = await apiRemoteHost(request);

    await page.goto("/");
    await openHostsPanel(page);
    const row = hostRowByName(page, info.remote_ssh);
    await openHostMenu(row);
    await row.locator(".host-remove").click();
    await expect(row.locator(".host-confirm-remove")).toBeVisible();
    // Focus lands on the way OUT of the destructive action, so a stray
    // Enter after the remove click backs out rather than in.
    await expect(row.locator(".host-cancel-remove")).toBeFocused();
    await row.locator(".host-cancel-remove").click();

    // Back to the ordinary controls, same host, same id — a cancel that
    // "worked" by re-adding the host would look identical without this.
    // The menu itself closed when `remove` was clicked (see `HostRow`'s
    // own doc for why), so `.host-remove` is checked inside a freshly
    // reopened one rather than on the row line.
    await openHostMenu(row);
    await expect(row.locator(".host-remove")).toBeVisible();
    await expect(row.locator(".host-confirm-remove")).toHaveCount(0);
    expect((await apiRemoteHost(request)).id).toBe(before.id);
  });


  // A chosen host leaving the registry must be reconciled VISIBLY. The
  // failure this rules out is the quiet one: a selector still displaying
  // host A while the create body carries host B, which is an agent launched
  // on a machine nobody picked.
  test("create-dialog-selector-disappearance: a vanished choice is announced, not substituted", async ({
    page,
  }) => {
    // The registry read that discovers the disappearance is a feed-triggered
    // one (PLAN_M6_75.md item 6 removed the hosts poll), so the moment it
    // happens is this test's to choose rather than the shared fleet's.
    const feed = await stubFeed(page);
    let offerExtra = true;
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      if (offerExtra) {
        body.hosts = [
          ...body.hosts,
          {
            id: 8100,
            kind: "ssh",
            destination: "user@ephemeral",
            name: "user@ephemeral",
            identity: null,
            remote_farhelm: null,
            remote_state_dir: null,
            state: {
              phase: "connected",
              identity: "identity-ephemeral",
              build_version: "0.0.1",
              refresh: { status: "ok", sessions: 0 },
            },
          },
        ];
      }
      await route.fulfill({ response, json: body });
    });

    await page.goto("/");
    await feed.waitForConnection(1);
    feed.notify(1);
    await page.locator(".new-session-button").click();
    const selector = page.locator(".create-session-host");
    await selector.selectOption({ label: "user@ephemeral" });
    await expect(selector).toHaveValue("8100");
    await expect(page.locator(".create-session-host-note")).toHaveCount(0);

    // The host is removed from under the open dialog, and the page finds out
    // the way it finds out about anything now: a revision notification and
    // its own re-read.
    offerExtra = false;
    feed.notify(2);
    await expect(page.locator(".create-session-host-note")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.locator(".create-session-host-note")).toContainText(
      "no longer registered",
    );
    await expect(
      selector,
      "the selector must SHOW the target that would actually be used",
    ).not.toHaveValue("8100");
  });

  // The host selector is inert for the whole round trip, exactly like the
  // text fields — and for a sharper reason than tidiness: the idempotency
  // key is minted BOUND to the target, so a selection that changed between
  // minting and sending would publish a key belonging to a different
  // machine.
  test("create-dialog-selector-disabled-in-flight: the target cannot move under a create", async ({
    page,
    request,
  }) => {
    const title = `selector-inflight-${Date.now()}`;
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      await held;
      await route.continue();
    });

    try {
      await page.goto("/");
      const form = await fillCreateForm(page, {
        cwd: "/tmp",
        invocation: FAKE_AGENT_INVOCATION,
        title,
      });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-host")).toBeDisabled();
      await expect(form.locator('input[type="text"]').nth(0)).toBeDisabled();
      release?.();
      await page.waitForFunction(() => (window as any).__farhelmTermReady === true, {
        timeout: 20_000,
      });
    } finally {
      release?.();
      await cleanUpSessionsTitled(request, title);
    }
  });

  // PLAN_M6.md names this one: a create against a host that is not connected
  // is a PRECONDITION FAILURE — a visible error naming the host's state, and
  // no session anywhere.
  //
  // The host is registered for real (a destination nothing answers at)
  // rather than mocked, because what is under test is the helm's refusal
  // reaching the form. The row's data attribute proves the stable wire
  // phase, while the visible refusal must carry the corresponding humanized
  // wording.
  test("create-on-unreachable-refused: the helm's words in place, and no session", async ({
    page,
    request,
  }) => {
    const title = `refused-on-down-${Date.now()}`;
    const added = await request.post("/api/hosts", {
      data: { ssh: "user@nothing-answers-here.invalid" },
    });
    expect(added.ok(), `registering a host that is down: ${await added.text()}`).toBe(true);
    const down = (await added.json()).id;

    try {
      await page.goto("/");
      await openHostsPanel(page);
      // Its phase is whatever the dial has reached — connecting first, then
      // unreachable-reprobing — and either refuses a create. The label is
      // what proves a non-connected host is still SELECTABLE, which is the
      // half of SPEC.md's default this test exists alongside.
      const row = page.locator(`[data-host-id="${down}"]`);
      await expect(row).toBeVisible();

      await page.locator(".new-session-button").click();
      const form = page.locator(".create-session-form");
      await form.locator(".create-session-host").selectOption(String(down));
      // Command mode explicitly, as `fillCreateForm` does and for the same
      // reason. A host change now preserves any profile choice, so this reset
      // states that the test is intentionally exercising a typed command.
      await form.locator(".create-session-profile").selectOption("");
      await form.locator('input[type="text"]').nth(0).fill("/tmp");
      await form.locator('input[type="text"]').nth(1).fill(FAKE_AGENT_INVOCATION);
      await form.locator('input[type="text"]').nth(2).fill(title);
      await form.locator('button[type="submit"]').click();

      const error = form.locator(".create-session-error");
      await expect(error).toBeVisible({ timeout: 30_000 });
      // The helm's own sentence: the host's state named, and nothing
      // queued for when it comes back (SPEC.md v1 refuses rather than
      // deferring).
      await expect(error).toContainText(`host ${down} is`);
      await expect(error).toContainText("refused");
      // And no session anywhere — a precondition failure creates nothing.
      const listing = await (await request.get("/api/sessions")).json();
      expect(listing.sessions.filter((s: any) => s.title === title)).toHaveLength(0);
    } finally {
      await cleanUpSessionsTitled(request, title);
      await request.delete(`/api/hosts/${down}`).catch(() => {});
    }
  });

  // The two optional install fields left blank must reach the helm as
  // ABSENT, never as empty strings: the helm takes `""` literally, and a
  // host registered to dial a binary named nothing never connects for a
  // reason no status can explain.
  test("add-host-blank-optional-fields: blanks are omitted rather than sent empty", async ({
    page,
    request,
  }) => {
    const destination = `user@blank-fields-${Date.now()}.invalid`;
    configureDiscoveredProbe(destination, "farhelm", null);
    let body: any;
    await page.route("**/api/hosts/probe", async (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() ?? "{}");
      }
      await route.continue();
    });

    let added: number | undefined;
    try {
      await page.goto("/");
      await openHostsPanel(page);
      await page.locator(".add-host-button").click();
      const form = page.locator(".add-host-form");
      await form.locator(".add-host-ssh").fill(destination);
      await form.locator(".add-host-submit").click();

      await expect(hostRowByName(page, destination)).toBeVisible({
        timeout: 30_000,
      });
      expect(body.remote_farhelm ?? null).toBeNull();
      expect(body.remote_state_dir ?? null).toBeNull();
      // The row records what discovery actually dialed. The plain `farhelm`
      // value comes from the injected supervisor observation, not an empty
      // form field silently converted into a path.
      const row = (await apiHosts(request)).find(
        (host: any) => host.destination === destination,
      );
      added = row.id;
      expect(row.remote_farhelm).toBe("farhelm");
      expect(row.remote_state_dir).toBeNull();
    } finally {
      if (added) await request.delete(`/api/hosts/${added}`).catch(() => {});
    }
  });

  // Changing the target after a failed create must mint a NEW key and send
  // the NEW host — the two together, from one submit.
  //
  // The key alone is not enough to assert: the helm scopes idempotency keys
  // per host, so a body pairing the old key with the new host would not
  // dedup on that machine at all, and a retry after an ambiguous failure
  // would launch a second real agent there.
  test("create-intent-key-rebinds-on-host-change: a new target is a new intent", async ({
    page,
    request,
  }) => {
    requireFleet();
    const info = stackInfo();
    const title = `rebind-${Date.now()}`;
    const bodies: any[] = [];
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      bodies.push(JSON.parse(route.request().postData() ?? "{}"));
      await route.continue();
    });

    try {
      await page.goto("/");
      // A directory that exists on neither host, so BOTH attempts fail and
      // the assertion is about the body rather than about which create
      // happened to succeed.
      const form = await fillCreateForm(page, {
        cwd: "/nonexistent/definitely/not/here",
        invocation: FAKE_AGENT_INVOCATION,
        title,
      });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      await form
        .locator(".create-session-host")
        .selectOption({ label: info.remote_ssh });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      expect(bodies).toHaveLength(2);
      expect(bodies[0].host).toBeTruthy();
      expect(bodies[1].host).not.toBe(bodies[0].host);
      expect(bodies[1].intent_key).toBeTruthy();
      expect(
        bodies[1].intent_key,
        "a key carried to another machine would not dedup there",
      ).not.toBe(bodies[0].intent_key);
    } finally {
      await cleanUpSessionsTitled(request, title);
    }
  });

  // Retargeting a host between a failed create and its retry must mint a NEW
  // key, even though the host's ID never changed.
  //
  // This is the case an id-keyed binding cannot see, and the expensive one:
  // the registry row is the same row, so a key bound to the id survives into
  // a retry aimed at a machine that has never seen it — where it dedups
  // nothing and launches a second real agent. The binding is the host's
  // INCARNATION (`hosts::host_incarnation`), which the destination is part
  // of.
  //
  // The retarget is applied to the hosts READ rather than to the real
  // registry: what is under test is the client's binding, and actually
  // retargeting the harness's remote would disconnect the fleet for a
  // minute to prove something about a string comparison. The read that
  // discovers it is triggered from here through a stubbed feed — a healthy
  // page re-reads the registry when it is told something changed, and
  // nothing else (PLAN_M6_75.md item 6).
  test("create-intent-key-rebinds-on-retarget: a moved host is a new intent", async ({
    page,
    request,
  }) => {
    const feed = await stubFeed(page);
    const local = (await apiHosts(request)).find((host: any) => host.kind === "local");
    const title = `rebind-retarget-${Date.now()}`;
    let moved = false;
    await page.route("**/api/hosts", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.hosts = body.hosts.map((host: any) =>
        host.id === local.id && moved
          ? {
              ...host,
              // The REGISTRY fields are what the incarnation is built from
              // (`hosts::host_incarnation`), and changing them is the whole
              // point of the test.
              destination: "user@moved-elsewhere",
              identity: "identity-after-the-move",
              // The connection state carries its own copy, which is the one
              // the row's detail line renders — so this is what gives the
              // test something observable to wait on before resubmitting.
              state: { ...host.state, identity: "identity-after-the-move" },
            }
          : host,
      );
      await route.fulfill({ response, json: body });
    });

    const keys: (string | undefined)[] = [];
    await page.route("**/api/sessions", async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      keys.push(JSON.parse(route.request().postData() ?? "{}").intent_key);
      await route.continue();
    });

    try {
      await page.goto("/");
      await openHostsPanel(page);
      await feed.waitForConnection(1);
      feed.notify(1);
      // A directory that does not exist, so both attempts fail and the
      // assertion is about the key rather than about which create happened
      // to succeed.
      const form = await fillCreateForm(page, {
        cwd: "/nonexistent/definitely/not/here",
        invocation: FAKE_AGENT_INVOCATION,
        title,
      });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      // Same row, same id, different machine behind it — and a notification
      // to send the page looking, since nothing else will.
      moved = true;
      feed.notify(2);
      await expect(page.locator(`[data-host-id="${local.id}"] .host-name`)).toHaveText(
        "this machine",
        { timeout: 15_000 },
      );
      await expect
        .poll(
          async () =>
            await page
              .locator(`[data-host-id="${local.id}"] .host-detail`)
              .textContent(),
          { timeout: 15_000, message: "waiting for the retargeted host to reach the panel" },
        )
        .toContain("identity-after-the-move");

      // The picker must remain in command mode without dispatching another
      // change event. Re-selecting it here would rotate the key itself and let
      // broken retarget invalidation pass this test.
      await expect(form.locator(".create-session-profile")).toHaveValue("");

      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      expect(keys).toHaveLength(2);
      expect(keys[0]).toBeTruthy();
      expect(
        keys[1],
        "the id is unchanged, but the machine behind it is not — replaying the key there would not dedup",
      ).not.toBe(keys[0]);
    } finally {
      await cleanUpSessionsTitled(request, title);
    }
  });

  test.afterAll(async () => {
    // Only ever the replacement THIS file started; the harness's own
    // supervisor is start-stack.sh's to reap. Left running it would leak a
    // process past the suite, and killing indiscriminately would take the
    // harness's original down with the fleet.
    //
    // Awaited rather than fired and forgotten: the next project's pass
    // through this file starts by looking for a supervisor on that state
    // directory, and a signalled-but-not-yet-dead one still holds the
    // ownership lock its replacement would need.
    await stopRestartedRemote();
  });
});
