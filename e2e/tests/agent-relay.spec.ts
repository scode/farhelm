// =====================================================================
// The agent relay's LITMUS TEST for SPEC.md's agent section ("A session can
// also ASK" and "The verbs also CREATE").
//
// Everything else in the stack tests a piece of the relay. This file tests
// the thing the feature exists for. ONE thing on the path is simulated and
// it is named below — the vendor's decision to fire a SessionStart hook —
// while the supervisor, the helm, the browser, the CLI and every verb are
// the shipped ones:
//
//   a session on the REMOTE host, whose agent was started under a
//   Claude-kind profile, reads the SessionStart hook the supervisor
//   injected into its own command line, runs it, obeys the pointer line it
//   printed, and then — driven by a `$farhelm ...` line typed into its
//   terminal through the BROWSER — clones itself onto the other host, with
//   its agent resolved by profile NAME against the helm catalog, and the new
//   row appears in the UI without a reload.
//
// That case is chosen because it is precisely what a supervisor-local
// implementation cannot do. It needs the host list the helm owns, the
// helm's create path, cross-host profile resolution, and the upcall round
// trip — and it needs a second real machine, which is why it lives here
// rather than in the Rust suite: `crates/farhelm/tests/e2e/
// agent_listing_real_stack.rs` settles for one host because a second one
// would require passwordless self-ssh, a prerequisite that suite does not
// have. This stack does (start-stack.sh registers a second real supervisor
// as an ssh-to-localhost host), so the cross-host half is here and the
// same-host half is there.
//
// The ONE thing standing in for a vendor is what decides a conversation
// started: the fake agent parses its own `--settings` argv and fires the
// hook itself (`fake_agent.rs`'s `agent-relay` script). Everything on
// either side of that is the shipped product — the settings JSON is the
// supervisor's, the hook is `farhelm internal hook`, the pointer is the
// line it prints, the manual is `farhelm agent instructions`, and the
// verbs are the shipped `farhelm agent hosts` and `farhelm agent clone`,
// resolved by bare name off the launch shim's PATH.
//
// SKIPPING is decided by an INDEPENDENT self-ssh probe, never by "the
// remote host did not connect" — the same rule, and the same asymmetry in
// CI, that `terminal-multihost.spec.ts` documents at length.
//
// `installTerminalSuiteHooks()` is deliberately NOT called: its reset
// deletes every session in the merged listing, remote ones included, and
// this file owns its own fixtures.
// =====================================================================

import { test, expect, APIRequestContext, Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  cleanupProfile,
  cleanupSession,
  createProfile,
  createSession,
  listHosts,
  selfSshAvailable,
} from "./helpers/fleet";
import { stackScratchDir } from "./helpers/scratch";
import { termText } from "./helpers/term";
import { submitPrompt, waitUntilAgentReady } from "./helpers/real-agent";

/**
 * The one thing this file needs out of what `start-stack.sh` publishes:
 * which binary the fixture profiles must invoke.
 *
 * Deliberately narrower than `terminal-multihost.spec.ts`'s reading of the
 * same file — nothing here kills or relaunches a supervisor, so nothing
 * here has any business knowing its pid or its state directory.
 */
type StackInfo = { farhelm: string };

/**
 * Read the published stack description, failing loudly if it is absent.
 *
 * A missing file is a broken harness rather than a condition to degrade
 * around: every test here would otherwise fail one assertion at a time
 * with no hint of the cause.
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
 * The display name the helm gives its OWN machine — the clone target
 * throughout this file.
 *
 * A literal rather than a lookup because it is a product constant
 * (`aggregate::host_display_name` renders `HostKind::Local` as exactly
 * this) and because it is the name an AGENT would read out of `farhelm
 * agent hosts` and type back. Pinning the literal here is what makes a
 * rename of that word fail this test rather than silently changing what an
 * agent has to say.
 */
const LOCAL_HOST_NAME = "this machine";

/** The fake-agent script that acts out the whole hook→pointer→verb chain. */
function relayAgentInvocation(): string {
  return `"${stackInfo().farhelm}" internal fake-agent --script agent-relay`;
}

/** The fleet's ssh row — the harness's second, isolated supervisor. */
async function remoteHost(request: APIRequestContext): Promise<any | undefined> {
  return (await listHosts(request)).find((host: any) => host.kind === "ssh");
}

/** The reserved local row, which every clone here targets. */
async function localHost(request: APIRequestContext): Promise<any> {
  const host = (await listHosts(request)).find((row: any) => row.kind === "local");
  expect(host, "every helm has a local row").toBeTruthy();
  return host;
}

/** Whether the two-host fleet is actually up, decided once per project pass. */
let fleetReady = false;

/**
 * Gate one fleet test — skip where self-ssh is genuinely unavailable, FAIL
 * in CI.
 *
 * The asymmetry is the point, and it matters more here than anywhere else
 * in the suite: this is the ONE test that proves the feature works at all,
 * so a silent skip in CI would let the whole thing regress while the run
 * stayed green.
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

/**
 * The terminal buffer with its row breaks removed.
 *
 * xterm stores a wrapped line as several buffer rows, and every one of
 * this fixture's interesting outputs — a refusal quoting a long path, a
 * hosts table folded into one line — is longer than the terminal is wide.
 * A `toContain` against the raw buffer therefore fails on exactly the
 * assertions that matter most. Joining the rows back together is correct
 * because a wrapped row is full-width with no trailing whitespace, so the
 * pieces concatenate to the original text; the cost is that two unrelated
 * SHORT lines also become adjacent, which is why every assertion below
 * looks for a distinctive substring rather than a line shape.
 */
async function flatTermText(page: Page): Promise<string> {
  return (await termText(page)).replace(/\n/g, "");
}

/** Wait until the terminal shows `needle`, reading through line wraps. */
async function waitForFlatText(page: Page, needle: string, timeout = 30_000) {
  await expect
    .poll(() => flatTermText(page), { timeout, message: `waiting for ${needle}` })
    .toContain(needle);
}

/**
 * Open a session's terminal and wait until the relay fixture has walked
 * the whole startup chain.
 *
 * Both markers are awaited, not just readiness, and each is a distinct
 * link that can break on its own. `POINTER:` means the supervisor really
 * injected `--settings`, the fixture found it, and the shipped hook ran
 * and wrote to stdout — which is what a vendor splices into the model's
 * context. `INSTRUCTIONS:ok` means the fixture then did what that line
 * told it to and got back a manual that documents the verb it is about to
 * use; `INSTRUCTIONS:missing-clone-verb` would mean the manual and the CLI
 * have drifted apart, which is the silent failure the generated verb list
 * exists to prevent.
 */
async function openRelayTerminal(page: Page, id: string): Promise<void> {
  await page.goto("/");
  const row = page.locator(`[data-session-id="${id}"]`);
  await expect(row).toBeVisible({ timeout: 20_000 });
  await row.locator(".session-row-open").click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitUntilAgentReady(page, { trustDialogMarkers: [], readyMarker: "FAKE-AGENT READY" });
  await waitForFlatText(page, "POINTER:farhelm: when the user writes");
  await waitForFlatText(page, "INSTRUCTIONS:ok");
}

/** The session id a `CLONED:` marker carries, once one appears. */
async function clonedId(page: Page, timeout = 30_000): Promise<string> {
  await waitForFlatText(page, "CLONED:", timeout);
  const match = (await flatTermText(page)).match(/CLONED:([0-9a-fA-F-]{36})/);
  expect(match, "the CLONED marker must carry a session id").toBeTruthy();
  return match![1];
}

/**
 * Run every cleanup step even when an earlier one throws, then surface all
 * failures together rather than letting the first one hide the rest.
 *
 * The two alias tests below have several independently fallible teardown
 * steps (closing the driver page, deleting a session, deleting a profile)
 * ahead of the one that matters most — restoring the shared fleet host's
 * alias — inside a single `finally` block. A plain sequential `finally`
 * abandons every step after the first thrown exception, which is exactly
 * how the alias restore two tests ago could be skipped by an unrelated
 * `cleanupSession` failure. This runs the whole list regardless, and
 * raises one aggregate error afterward if anything failed, so a broken
 * step still gets reported instead of silently swallowing its neighbors'
 * outcomes.
 */
async function runCleanupSteps(steps: Array<() => Promise<void>>): Promise<void> {
  const failures: unknown[] = [];
  for (const step of steps) {
    try {
      await step();
    } catch (error) {
      failures.push(error);
    }
  }
  if (failures.length > 0) {
    throw new Error(`${failures.length} cleanup step(s) failed: ${failures.map(String).join("; ")}`);
  }
}

test.describe("agent relay: an agent clones its own session across hosts", () => {
  test.beforeAll(async ({ request }) => {
    test.setTimeout(180_000);
    if (!(await selfSshAvailable())) {
      console.log(
        "SKIPPED agent-relay: passwordless `ssh localhost` is unavailable, so the stack has no second host",
      );
      return;
    }
    await expect
      .poll(async () => (await remoteHost(request))?.state?.phase, {
        timeout: 45_000,
        message: "waiting for the harness's ssh host to connect",
      })
      .toBe("connected");
    fleetReady = true;
  });

  /**
   * Spec: a `$farhelm clone this session onto <host>` typed into a remote
   * session's terminal creates a session on the OTHER host, in the same
   * directory and with the same title, whose agent came from the profile of
   * the SAME NAME in the helm catalog — and the new row appears in a browser
   * that never reloaded.
   *
   * This is SPEC.md's cross-host clone, clause for clause. Every assertion
   * is a separate way for the feature to be built wrong:
   *
   * - **The new session is on the other host.** A supervisor-local
   *   implementation would have created it beside the source, which is the
   *   one outcome that looks like success and is not.
   * - **Its profile still exists under its snapshotted name.** The helm-wide
   *   catalog is the authority, so the clone carries the same profile id only
   *   while that row still has the same name.
   * - **Title and cwd are copied.** A clone that re-derived a title from
   *   the directory is the difference a user notices first.
   * - **`CLONED:<id>` matches the new row.** That is what proves the id on
   *   the CLI's stdout is the session that actually exists, rather than
   *   something the fixture happened to print.
   * - **No reload.** The observer page's URL is captured before the clone
   *   and asserted unchanged, exactly as `spawn.spec.ts` does: SPEC.md's
   *   promise is that the fleet view is live, and a test that navigated
   *   would prove nothing about it.
   */
  test("clones onto the other host from the matching helm profile", async ({
    page,
    context,
    request,
  }) => {
    requireFleet();
    test.setTimeout(180_000);

    const stamp = Date.now();
    const name = `agent-relay-${stamp}`;
    const remote = await remoteHost(request);
    const local = await localHost(request);
    const work = stackScratchDir(`agent-relay-${stamp}-`);

    let sourceProfile: string | undefined;
    let source: { id: string } | undefined;
    let cloned: string | undefined;
    let driver: Page | undefined;
    try {
      // The SOURCE profile: Claude-kind, so the supervisor injects the
      // SessionStart hook into the launch's argv, which is the first link
      // of the chain under test.
      sourceProfile = (
        await createProfile(request, {
          name,
          invocation: relayAgentInvocation(),
          // Claude-kind with NO explicit resume template: the supervisor
          // derives one from the invocation's own program, which is what
          // makes this a legal integrated profile without this fixture
          // inventing a resume command nothing here ever runs.
          agent_kind: "claude",
        })
      ).id;
      source = await createSession(request, {
        title: `relay-source-${stamp}`,
        cwd: work,
        host: remote.id,
        profile_id: sourceProfile,
      });

      // The observer sits on the list and never navigates again; the
      // driver types. Two pages, as in `spawn.spec.ts`, because the
      // no-reload claim is about a page that did not act.
      await page.goto("/");
      await expect(page.locator(`[data-session-id="${source.id}"]`)).toBeVisible({
        timeout: 20_000,
      });
      const observerUrl = page.url();

      driver = await context.newPage();
      await openRelayTerminal(driver, source.id);
      await submitPrompt(driver, `$farhelm clone this session onto ${LOCAL_HOST_NAME}`, 200);
      cloned = await clonedId(driver);

      // The row appears in a browser that never reloaded.
      await expect(page.locator(`[data-session-id="${cloned}"]`)).toBeVisible({ timeout: 30_000 });
      expect(page.url(), "the observer must not navigate to discover the clone").toBe(observerUrl);

      // ...on the other host, from the same helm profile, carrying the
      // source's directory and title.
      const listing = await request.get("/api/sessions?include_archived=true");
      expect(listing.ok(), `GET /api/sessions: ${listing.status()}`).toBe(true);
      const rows = (await listing.json()).sessions;
      const row = rows.find((entry: any) => entry.id === cloned);
      expect(row, `the clone must be in the merged listing: ${JSON.stringify(rows)}`).toBeTruthy();
      expect(row.host, "the clone lands on the TARGET host's registry row").toBe(local.id);
      expect(row.host_name).toBe(LOCAL_HOST_NAME);
      expect(row.title, "a clone copies the source's title").toBe(`relay-source-${stamp}`);
      expect(row.cwd, "a clone copies the source's directory").toBe(work);
      expect(
        row.source_profile?.id,
        "the agent must come from the helm row that still matches the snapshot",
      ).toBe(sourceProfile);
      expect(row.source_profile?.name).toBe(name);
    } finally {
      await driver?.close();
      if (cloned) await cleanupSession(request, cloned);
      if (source) await cleanupSession(request, source.id);
      // Profiles after sessions, deliberately: a session outliving its
      // profile is the snapshot rule the product has, while the reverse
      // order briefly leaves rows describing a profile that is already
      // gone. `profiles.spec.ts` documents the same ordering.
      //
      // What this deliberately does NOT clean up is the helm's REMEMBERED
      // DEFAULT, which the clone's own success wrote and which
      // now names a deleted profile. That is the shared state every
      // profile-using spec leaves behind, and the create dialog already
      // has a defined answer for it (SPEC.md's ask-don't-guess: it
      // selects nothing and waits) that `fillCreateForm` is written
      // around. There is no API to forget a default, so the alternative
      // would be leaving the profile itself behind — strictly worse.
      if (sourceProfile) await cleanupProfile(request, sourceProfile);
      fs.rmSync(work, { recursive: true, force: true });
    }
  });

  /**
   * Spec: when the source's snapshotted profile no longer exists under that
   * name in the helm catalog, the clone is refused and creates no session.
   *
   * SPEC.md's agent section calls this out in those words: "No match is a
   * refusal naming the host and the profile", with "deliberately no
   * fallback to the source's raw invocation". The fallback
   * is tempting precisely because the source row is carrying a perfectly
   * good invocation, and here that invocation is a path into the harness's
   * own tree that would happen to work on the target — which is exactly
   * what makes a silent fallback look like success in a test and like a
   * mystery in production, where the other machine has different software
   * installed.
   *
   * A differently named profile remains, so the refusal is proven to be
   * about the snapshot match rather than an empty catalog.
   */
  test("refuses, naming the host and the profile, when the helm has no such name", async ({
    context,
    request,
  }) => {
    requireFleet();
    test.setTimeout(180_000);

    const stamp = Date.now();
    const wanted = `agent-relay-wanted-${stamp}`;
    const decoy = `agent-relay-decoy-${stamp}`;
    const remote = await remoteHost(request);
    const local = await localHost(request);
    const work = stackScratchDir(`agent-relay-miss-${stamp}-`);

    let sourceProfile: string | undefined;
    let decoyProfile: string | undefined;
    let source: { id: string } | undefined;
    let driver: Page | undefined;
    try {
      sourceProfile = (
        await createProfile(request, {
          name: wanted,
          invocation: relayAgentInvocation(),
          // Claude-kind with NO explicit resume template: the supervisor
          // derives one from the invocation's own program, which is what
          // makes this a legal integrated profile without this fixture
          // inventing a resume command nothing here ever runs.
          agent_kind: "claude",
        })
      ).id;
      decoyProfile = (await createProfile(request, { name: decoy })).id;

      source = await createSession(request, {
        title: `relay-miss-${stamp}`,
        cwd: work,
        host: remote.id,
        profile_id: sourceProfile,
      });
      await cleanupProfile(request, sourceProfile);

      driver = await context.newPage();
      await openRelayTerminal(driver, source.id);
      const before = await (await request.get("/api/sessions?include_archived=true")).json();

      await submitPrompt(driver, `$farhelm clone this session onto ${LOCAL_HOST_NAME}`, 200);
      await waitForFlatText(driver, "CLONE-ERROR:");
      const transcript = await flatTermText(driver);
      expect(transcript).toContain(wanted);
      expect(transcript).toContain(LOCAL_HOST_NAME);
      expect(
        transcript,
        "a refused clone must not report a session id — that is what a silent fallback would look like",
      ).not.toContain("CLONED:");

      const after = await (await request.get("/api/sessions?include_archived=true")).json();
      const ids = new Set(before.sessions.map((row: any) => row.id));
      const created = after.sessions.filter((row: any) => !ids.has(row.id));
      expect(created, `a refused clone must create nothing: ${JSON.stringify(created)}`).toEqual([]);
    } finally {
      await driver?.close();
      if (source) await cleanupSession(request, source.id);
      if (decoyProfile) await cleanupProfile(request, decoyProfile);
      if (sourceProfile) await cleanupProfile(request, sourceProfile);
      fs.rmSync(work, { recursive: true, force: true });
    }
  });

  /**
   * Spec: a `--cwd` the target does not have produces the TARGET
   * supervisor's own refusal, reported verbatim.
   *
   * SPEC.md's agent section requires those words — "a directory that does
   * not exist on the target is that supervisor's own refusal, reported
   * verbatim rather than paraphrased on the way back" — and the verbatim
   * clause is the one worth a test. Every hop on the way back has
   * the material to write a friendlier sentence (the helm knows the host
   * and the verb; the CLI knows the flags), and any of them doing so would
   * replace the only description of what actually went wrong on a machine
   * nobody is looking at.
   */
  test("reports the target's own refusal verbatim for a directory it does not have", async ({
    context,
    request,
  }) => {
    requireFleet();
    test.setTimeout(180_000);

    const stamp = Date.now();
    const name = `agent-relay-cwd-${stamp}`;
    const remote = await remoteHost(request);
    const local = await localHost(request);
    const work = stackScratchDir(`agent-relay-cwd-${stamp}-`);
    // Never created, on either machine — both supervisors run on this one
    // host, so "absent on the target" and "absent everywhere" coincide
    // here; what the test is about is WHOSE sentence comes back.
    const absent = path.join(work, "no-such-directory");

    let sourceProfile: string | undefined;
    let source: { id: string } | undefined;
    let driver: Page | undefined;
    try {
      sourceProfile = (
        await createProfile(request, {
          name,
          invocation: relayAgentInvocation(),
          // Claude-kind with NO explicit resume template: the supervisor
          // derives one from the invocation's own program, which is what
          // makes this a legal integrated profile without this fixture
          // inventing a resume command nothing here ever runs.
          agent_kind: "claude",
        })
      ).id;
      source = await createSession(request, {
        title: `relay-cwd-${stamp}`,
        cwd: work,
        host: remote.id,
        profile_id: sourceProfile,
      });

      driver = await context.newPage();
      await openRelayTerminal(driver, source.id);
      await submitPrompt(
        driver,
        `$farhelm clone this session onto ${LOCAL_HOST_NAME} in ${absent}`,
        200,
      );
      await waitForFlatText(driver, "CLONE-ERROR:");
      expect(
        await flatTermText(driver),
        "the target supervisor's own words must survive the helm, the relay and the CLI",
      ).toContain(`working directory does not exist: ${absent}`);
    } finally {
      await driver?.close();
      if (source) await cleanupSession(request, source.id);
      if (sourceProfile) await cleanupProfile(request, sourceProfile);
      fs.rmSync(work, { recursive: true, force: true });
    }
  });

  /**
   * Spec: once a host carries an alias, cross-host naming resolves it —
   * `$farhelm clone this session onto <alias>` reaches the aliased host
   * exactly as naming its raw destination used to
   * (`aggregate::host_display_name`: the alias IS the display name once
   * set). This is the browser-level pin of the same rule
   * `agent_requests.rs`'s `create_resolves_by_alias_once_one_is_set` proves
   * at the Rust level, run here against the real second host and the real
   * CLI rather than a fabricated fleet.
   *
   * The source session lives on the LOCAL host this time — the reverse of
   * the clone direction every test above uses — because the host under
   * test has to be the TARGET being named, and this suite's only
   * alias-safe target is the harness's shared ssh host: aliasing the local
   * row instead would rename "this machine" out from under every other
   * spec that assumes it, including the three tests above.
   */
  test("clones onto the remote host by its alias", async ({ context, request }) => {
    requireFleet();
    test.setTimeout(180_000);

    const stamp = Date.now();
    const name = `agent-relay-alias-${stamp}`;
    const alias = `relay-alias-${stamp}`;
    const remote = await remoteHost(request);
    const local = await localHost(request);
    const work = stackScratchDir(`agent-relay-alias-${stamp}-`);
    // Captured before this test writes anything, so the restore below puts
    // back the host's REAL prior value (ordinarily `null`, but this does
    // not assume that) rather than a guess.
    const originalAlias: string | null = remote.alias ?? null;

    // The alias restore is its OWN, OUTERMOST try/finally, independent of
    // every other cleanup step below — it is the one piece of teardown
    // that leaks into every OTHER test on the shared fleet host if
    // skipped, so it must run even when an unrelated step (closing the
    // driver page, deleting the session or profile) throws first.
    try {
      const set = await request.post(`/api/hosts/${remote.id}/alias`, { data: { alias } });
      expect(set.ok(), `aliasing the remote host: ${await set.text()}`).toBe(true);

      let sourceProfile: string | undefined;
      let source: { id: string } | undefined;
      let cloned: string | undefined;
      let driver: Page | undefined;
      try {
        sourceProfile = (
          await createProfile(request, {
            name,
            invocation: relayAgentInvocation(),
            agent_kind: "claude",
          })
        ).id;
        source = await createSession(request, {
          title: `relay-alias-${stamp}`,
          cwd: work,
          host: local.id,
          profile_id: sourceProfile,
        });

        driver = await context.newPage();
        await openRelayTerminal(driver, source.id);
        await submitPrompt(driver, `$farhelm clone this session onto ${alias}`, 200);
        cloned = await clonedId(driver);

        const listing = await request.get("/api/sessions?include_archived=true");
        expect(listing.ok(), `GET /api/sessions: ${listing.status()}`).toBe(true);
        const rows = (await listing.json()).sessions;
        const row = rows.find((entry: any) => entry.id === cloned);
        expect(row, `the clone must be in the merged listing: ${JSON.stringify(rows)}`).toBeTruthy();
        expect(row.host, "the clone lands on the ALIASED host's registry row").toBe(remote.id);
        expect(row.host_name, "the clone's row names the host by its alias").toBe(alias);
      } finally {
        const capturedCloned = cloned;
        const capturedSource = source;
        const capturedProfile = sourceProfile;
        await runCleanupSteps([
          async () => {
            await driver?.close();
          },
          async () => {
            if (capturedCloned) await cleanupSession(request, capturedCloned);
          },
          async () => {
            if (capturedSource) await cleanupSession(request, capturedSource.id);
          },
          // See the first test in this file for why the remembered default
          // is deliberately NOT cleaned up here too.
          async () => {
            if (capturedProfile) await cleanupProfile(request, capturedProfile);
          },
          async () => {
            fs.rmSync(work, { recursive: true, force: true });
          },
        ]);
      }
    } finally {
      const cleared = await request.post(`/api/hosts/${remote.id}/alias`, {
        data: { alias: originalAlias },
      });
      expect(
        cleared.ok(),
        `restoring the shared fleet host's alias: ${await cleared.text()}`,
      ).toBe(true);
    }
  });

  /**
   * Spec: once a host is aliased, its RAW destination stops resolving — it
   * is refused exactly like any other unregistered name, and the refusal's
   * known-hosts list shows the alias rather than the address it replaced.
   * The browser-level pin of `agent_requests.rs`'s
   * `create_on_the_raw_destination_is_refused_once_aliased`, run here
   * against the real second host and the real CLI.
   */
  test("cloning onto the remote host's raw destination is refused once aliased", async ({
    context,
    request,
  }) => {
    requireFleet();
    test.setTimeout(180_000);

    const stamp = Date.now();
    const name = `agent-relay-unaliased-${stamp}`;
    const alias = `relay-hides-${stamp}`;
    const remote = await remoteHost(request);
    const local = await localHost(request);
    const work = stackScratchDir(`agent-relay-unaliased-${stamp}-`);
    const rawDestination = remote.destination as string;
    // Captured before this test writes anything — see the previous test's
    // own comment for why this is not assumed to be `null`.
    const originalAlias: string | null = remote.alias ?? null;

    // Same nesting as the previous test, for the same reason: the alias
    // restore is the one piece of teardown that leaks into every OTHER
    // test on the shared fleet host if an unrelated cleanup step throws
    // first, so it gets its own outermost try/finally.
    try {
      const set = await request.post(`/api/hosts/${remote.id}/alias`, { data: { alias } });
      expect(set.ok(), `aliasing the remote host: ${await set.text()}`).toBe(true);

      let sourceProfile: string | undefined;
      let source: { id: string } | undefined;
      let driver: Page | undefined;
      try {
        sourceProfile = (
          await createProfile(request, {
            name,
            invocation: relayAgentInvocation(),
            agent_kind: "claude",
          })
        ).id;
        source = await createSession(request, {
          title: `relay-unaliased-${stamp}`,
          cwd: work,
          host: local.id,
          profile_id: sourceProfile,
        });

        driver = await context.newPage();
        await openRelayTerminal(driver, source.id);
        const before = await (await request.get("/api/sessions?include_archived=true")).json();

        await submitPrompt(driver, `$farhelm clone this session onto ${rawDestination}`, 200);
        await waitForFlatText(driver, "CLONE-ERROR:");
        const transcript = await flatTermText(driver);
        expect(transcript).toContain(rawDestination);
        expect(
          transcript,
          "the known-hosts list must show the ALIAS the raw destination was replaced by",
        ).toContain(alias);
        expect(
          transcript,
          "a refused clone must not report a session id — that is what a silent fallback would look like",
        ).not.toContain("CLONED:");

        const after = await (await request.get("/api/sessions?include_archived=true")).json();
        const ids = new Set(before.sessions.map((row: any) => row.id));
        const created = after.sessions.filter((row: any) => !ids.has(row.id));
        expect(created, `a refused clone must create nothing: ${JSON.stringify(created)}`).toEqual([]);
      } finally {
        const capturedSource = source;
        const capturedProfile = sourceProfile;
        await runCleanupSteps([
          async () => {
            await driver?.close();
          },
          async () => {
            if (capturedSource) await cleanupSession(request, capturedSource.id);
          },
          async () => {
            if (capturedProfile) await cleanupProfile(request, capturedProfile);
          },
          async () => {
            fs.rmSync(work, { recursive: true, force: true });
          },
        ]);
      }
    } finally {
      const cleared = await request.post(`/api/hosts/${remote.id}/alias`, {
        data: { alias: originalAlias },
      });
      expect(
        cleared.ok(),
        `restoring the shared fleet host's alias: ${await cleared.text()}`,
      ).toBe(true);
    }
  });
});
