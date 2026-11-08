// Spawn from inside a real Farhelm terminal (PLAN_M7.md item 4).
//
// The always-on leg uses the deterministic fake-agent `spawn` script and a
// second page as the observer. The observer never reloads or navigates while
// the terminal creates children, so a row appearing there proves the normal
// drain and invalidation feed carry spawn visibility end to end. Every spawn
// asks unless the host has the applicable permission (SPEC.md, Agent-spawned
// sessions), so the observer answers required cards and observes remembered
// same-host authority through a later spawn that needs no answer.
//
// The final leg repeats the product contract with real Claude. It is gated on
// FARHELM_REAL_AGENT=1 because vendor credentials and network access are
// intentionally absent from CI, and it emits the same visible skip line as
// real-agent.spec.ts when ungated.
// Making real Claude usable in the stack's private login home (login and any
// configuration) is the maintainer's job; the repository handles no credentials.
// The flag also exposes real Claude to structured specs selecting that harness,
// so limit opted-in runs to the two real-agent tests with a title filter.
import { expect, test } from "./helpers/evidence";
import { type APIRequestContext, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import {
  cleanupSession,
  createSession,
  listHosts,
  listSessions,
  setLocalYoloWithoutAsking,
  type SessionRow,
  type HostRow,
} from "./helpers/fleet";
import { answerCard, setLocalCommandsWithoutAsking, setLocalOwnSessionsWithoutAsking } from "./helpers/approvals";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import {
  CLAUDE_CODE_MARKERS,
  submitPrompt,
  waitForReplyMarker,
  waitUntilAgentReady,
} from "./helpers/real-agent";
import { requireProductPageAuth } from "./helpers/device-auth";
import { stackScratchDir } from "./helpers/scratch";
import { attachSession } from "./helpers/term";

const FIXTURES = path.resolve(__dirname, "../../target/debug/farhelm-fixtures");
const SPAWN_AGENT = `"${FIXTURES}" fake-agent --script spawn`;

/** One list row by its supervisor-minted id. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/** Open a session and wait until its terminal is genuinely accepting input. */
async function openReadyTerminal(
  page: Page,
  id: string,
  markers: { trustDialogMarkers: string[]; readyMarker: string },
) {
  await page.goto("/");
  await attachSession(page, id);
  await waitUntilAgentReady(page, markers);
}

/** Poll the real list endpoint for the uniquely titled child a spawn minted. */
async function childByTitle(request: APIRequestContext, title: string): Promise<SessionRow> {
  let child: SessionRow | undefined;
  await expect
    .poll(
      async () => {
        child = (await listSessions(request)).sessions.find(
          (session) => session.title === title,
        );
        return child?.id;
      },
      { timeout: 20_000, message: `waiting for spawned child ${title}` },
    )
    .toBeTruthy();
  return child!;
}

test("a fake agent spawns children that appear without refreshing the observer", async ({
  page,
  context,
  request,
}) => {
  // This acceptance path owns two pages and their feed/terminal sockets.
  // Remove Playwright's ambient bearer so both engines prove the UI send
  // wrapper and WebSocket device subprotocol instead of browser behavior.
  const stamp = `${Date.now()}-${process.pid}`;
  // Setup is inside the cleanup boundary; each optional records only the
  // resource this test actually acquired before a partial failure.
  let root: string | undefined;
  let parent: SessionRow | undefined;
  let unparented: SessionRow | undefined;
  let parented: SessionRow | undefined;
  let driver: Page | undefined;

  try {
    await requireProductPageAuth(context);
    // The fake agent's launch is a command launch, which an agent may start
    // only on a host that starts YOLO sessions without asking (SPEC.md); the
    // spec is about spawning, not that rule. The commands setting must start
    // off, or no card would be asked for.
    await setLocalYoloWithoutAsking(request, true);
    await setLocalCommandsWithoutAsking(request, false);
    root = stackScratchDir(`farhelm-spawn-${stamp}-`);
    const unparentedDir = path.join(root, "unparented-child");
    const parentedDir = path.join(root, "parented-child");
    fs.mkdirSync(unparentedDir);
    fs.mkdirSync(parentedDir);
    parent = await createSession(request, {
      title: `spawn-parent-${stamp}`,
      cwd: root,
      invocation: SPAWN_AGENT,
    });
    driver = await context.newPage();

    await page.goto("/");
    await expect(row(page, parent.id)).toBeVisible({ timeout: 20_000 });
    const observerUrl = page.url();
    await openReadyTerminal(driver, parent.id, {
      trustDialogMarkers: [],
      readyMarker: "FAKE-AGENT READY",
    });

    // This is the acceptance command: inheritance is an explicit choice. The
    // spawn waits for the user, whose card names the folder and the inherited
    // launch.
    await submitPrompt(driver, `spawn ${unparentedDir}`, 100);
    await answerCard(page, "allow", [
      "Start a new session running the asking session's own launch",
      unparentedDir,
      "fake-agent --script spawn",
    ]);
    await waitForReplyMarker(driver, "SPAWNED:");
    unparented = await childByTitle(request, path.basename(unparentedDir));
    expect(
      unparented.invocation,
      "explicit inheritance must preserve the parent's own command",
    ).toBe(SPAWN_AGENT);
    await expect(row(page, unparented.id)).toBeVisible({ timeout: 20_000 });
    expect(page.url(), "the observer must not navigate to discover the child").toBe(observerUrl);

    // The second command proves an authenticated parent reaches the spawned
    // session without changing how the observer discovers it. Answered with
    // "always allow", which must turn the host's setting on as well.
    await submitPrompt(driver, `spawn-parented ${parentedDir}`, 100);
    await answerCard(page, "always-allow", [parentedDir]);
    await waitForReplyMarker(driver, "SPAWNED-PARENTED:");
    const hosts = await request.get("/api/hosts");
    const local = (await hosts.json()).hosts.find((host: { kind: string }) => host.kind === "local");
    expect(local.commands_without_asking, "always allow turns the host's setting on").toBe(true);
    parented = await childByTitle(request, path.basename(parentedDir));
    await expect(row(page, parented.id)).toBeVisible({ timeout: 20_000 });
    expect(page.url()).toBe(observerUrl);

    const parentedListing = await request.get(`/api/sessions?parent=${encodeURIComponent(parent.id)}`);
    expect(parentedListing.ok(), await parentedListing.text()).toBeTruthy();
    const parentedRows = (await parentedListing.json()).sessions as Array<{ id: string }>;
    expect(parentedRows.some((session) => session.id === parented.id)).toBe(true);
    expect(parentedRows.some((session) => session.id === unparented!.id)).toBe(false);
  } finally {
    if (driver) await driver.close();
    if (parented) await cleanupSession(request, parented.id);
    if (unparented) await cleanupSession(request, unparented.id);
    if (parent) await cleanupSession(request, parent.id);
    if (root) fs.rmSync(root, { recursive: true, force: true });
    await setLocalCommandsWithoutAsking(request, false);
    await setLocalYoloWithoutAsking(request, false);
  }
});

/** A real card grants only same-host session authority, and a later spawn
 * uses that stored permission without a second answer. Distinct reply markers
 * and child directories prevent the first success from satisfying the second.
 * Cleanup settles the parent before finding any children whose replies were
 * lost, and restores each original policy independently of cleanup failures. */
test("a host-only grant lets the next same-host spawn proceed without another card", async ({ page, context, request }) => {
  const stamp = `${Date.now()}-${process.pid}`;
  const parentTitle = `spawn-own-host-parent-${stamp}`;
  let original: HostRow | undefined;
  let root: string | undefined;
  let firstTitle: string | undefined;
  let secondTitle: string | undefined;
  let parent: SessionRow | undefined;
  let first: SessionRow | undefined;
  let second: SessionRow | undefined;
  let driver: Page | undefined;
  let bodyFailed = false;
  try {
    await requireProductPageAuth(context);
    original = (await listHosts(request)).find((host) => host.kind === "local");
    if (!original) throw new Error("the fixture needs the reserved local host");
    expect(typeof original.own_sessions_without_asking).toBe("boolean");
    expect(typeof original.commands_without_asking).toBe("boolean");
    expect(typeof original.yolo_without_asking).toBe("boolean");
    await setLocalCommandsWithoutAsking(request, false);
    await setLocalOwnSessionsWithoutAsking(request, false);
    // The fake agent inherits a command launch. Its separate YOLO guard
    // must permit that launch before this test can exercise an approval.
    await setLocalYoloWithoutAsking(request, true);
    expect((await listHosts(request)).find((host) => host.id === original!.id)).toMatchObject({
      commands_without_asking: false,
      own_sessions_without_asking: false,
      yolo_without_asking: true,
    });
    root = stackScratchDir(`farhelm-spawn-own-host-${stamp}-`);
    firstTitle = `own-host-first-${stamp}`;
    secondTitle = `own-host-second-${stamp}`;
    const firstDir = path.join(root, firstTitle);
    const secondDir = path.join(root, secondTitle);
    fs.mkdirSync(firstDir);
    fs.mkdirSync(secondDir);
    parent = await createSession(request, { title: parentTitle, cwd: root, invocation: SPAWN_AGENT });
    driver = await context.newPage();
    await page.goto("/");
    await expect(row(page, parent.id)).toBeVisible({ timeout: 20_000 });
    const observerUrl = page.url();
    await openReadyTerminal(driver, parent.id, {
      trustDialogMarkers: [],
      readyMarker: "FAKE-AGENT READY",
    });

    await submitPrompt(driver, `spawn ${firstDir}`, 100);
    await answerCard(page, "always-allow-own-host", [
      "Start a new session running the asking session's own launch",
      firstDir,
      "fake-agent --script spawn",
      "Always allow on ",
    ]);
    await waitForReplyMarker(driver, "SPAWNED:");
    first = await childByTitle(request, firstTitle);
    expect(first.invocation).toBe(SPAWN_AGENT);
    await expect(row(page, first.id)).toBeVisible({ timeout: 20_000 });
    expect((await listHosts(request)).find((host) => host.id === original!.id)).toMatchObject({
      own_sessions_without_asking: true,
      commands_without_asking: false,
      yolo_without_asking: true,
    });
    expect(page.url(), "the observer discovers the first child in place").toBe(observerUrl);

    await submitPrompt(driver, `spawn-parented ${secondDir}`, 100);
    await waitForReplyMarker(driver, "SPAWNED-PARENTED:");
    second = await childByTitle(request, secondTitle);
    expect(second.id).not.toBe(first.id);
    expect(second.invocation).toBe(SPAWN_AGENT);
    await expect(row(page, second.id)).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".approval-card"), "the second successful spawn needed no card answer").toHaveCount(0);
    expect((await listHosts(request)).find((host) => host.id === original!.id)).toMatchObject({
      own_sessions_without_asking: true,
      commands_without_asking: false,
    });
    expect(page.url(), "remembered same-host authority must not navigate the observer").toBe(observerUrl);
    const listing = await listSessions(request, `parent=${encodeURIComponent(parent.id)}`);
    expect(listing.sessions.some((session) => session.id === second!.id)).toBe(true);
    expect(listing.sessions.some((session) => session.id === first!.id)).toBe(false);
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    await cleanupOwnedSteps(bodyFailed, [
      ["driver page", () => driver?.close()],
      ["observer page", () => page.close()],
      ["parent session and any unfinished spawn", async () => {
        const owned = parent ?? (await listSessions(request)).sessions.find((session) => session.title === parentTitle);
        if (owned) await cleanupSession(request, owned.id);
      }],
      ["second child, including a lost reply", async () => {
        const owned = second ?? (secondTitle && (await listSessions(request)).sessions.find((session) => session.title === secondTitle));
        if (owned) await cleanupSession(request, owned.id);
      }],
      ["first child, including a lost reply", async () => {
        const owned = first ?? (firstTitle && (await listSessions(request)).sessions.find((session) => session.title === firstTitle));
        if (owned) await cleanupSession(request, owned.id);
      }],
      ["owned scratch", () => { if (root) fs.rmSync(root, { recursive: true, force: true }); }],
      ["original same-host permission", () => original && setLocalOwnSessionsWithoutAsking(request, original.own_sessions_without_asking)],
      ["original fleet-wide permission", () => original && setLocalCommandsWithoutAsking(request, original.commands_without_asking)],
      ["original YOLO permission", () => original && setLocalYoloWithoutAsking(request, original.yolo_without_asking)],
    ]);
  }
});

// Spec: denying an agent's spawn on its card refuses it with a readable
// message in the agent's terminal and creates nothing.
//
// Why: the card's whole point is that the user's "no" stops the agent;
// a denied spawn that still created a child, or that left the agent with
// no answer, would make the prompt decorative.
test("denying a spawn's card creates nothing and tells the agent", async ({ page, context, request }) => {
  const stamp = `${Date.now()}-${process.pid}`;
  let root: string | undefined;
  let parent: SessionRow | undefined;
  let driver: Page | undefined;
  try {
    await requireProductPageAuth(context);
    await setLocalYoloWithoutAsking(request, true);
    await setLocalCommandsWithoutAsking(request, false);
    root = stackScratchDir(`farhelm-spawn-deny-${stamp}-`);
    const childDir = path.join(root, "denied-child");
    fs.mkdirSync(childDir);
    parent = await createSession(request, {
      title: `spawn-deny-parent-${stamp}`,
      cwd: root,
      invocation: SPAWN_AGENT,
    });
    driver = await context.newPage();
    await page.goto("/");
    await expect(row(page, parent.id)).toBeVisible({ timeout: 20_000 });
    await openReadyTerminal(driver, parent.id, {
      trustDialogMarkers: [],
      readyMarker: "FAKE-AGENT READY",
    });
    await submitPrompt(driver, `spawn ${childDir}`, 100);
    await answerCard(page, "deny", [childDir]);
    await waitForReplyMarker(driver, "declined");
    const sessions = (await listSessions(request)).sessions;
    expect(
      sessions.some((session) => session.title === path.basename(childDir)),
      "a denied spawn creates no child",
    ).toBe(false);
  } finally {
    if (driver) await driver.close();
    if (parent) await cleanupSession(request, parent.id);
    if (root) fs.rmSync(root, { recursive: true, force: true });
    await setLocalCommandsWithoutAsking(request, false);
    await setLocalYoloWithoutAsking(request, false);
  }
});

/** The live vendor must actually create and spawn, beyond the fake-agent proof.
 * Cleanup also owns shared launch policy: even incomplete setup or a failed
 * session/workspace cleanup must reach scratch removal and both restores,
 * while secondary cleanup diagnostics preserve the body's original failure. */
test("a real Claude creates a jj workspace and spawns into it without refreshing the observer", async ({
  page,
  context,
  request,
}, testInfo) => {
  if (process.env.FARHELM_REAL_AGENT !== "1") {
    console.log(
      "SKIPPED: real-agent spawn leg — FARHELM_REAL_AGENT!=1 (needs vendor credentials and " +
        "network CI does not have; set FARHELM_REAL_AGENT=1 to run it deliberately — " +
        "PLAN_M7.md item 4)",
    );
  }
  test.skip(
    process.env.FARHELM_REAL_AGENT !== "1",
    "set FARHELM_REAL_AGENT=1 to run this against a real `claude` binary on PATH, already " +
      "authenticated for the test user",
  );
  test.setTimeout(180_000);

  const stamp = `${Date.now()}-${process.pid}`;
  const repository = path.resolve(__dirname, "../..");
  // Real-agent setup has more failure points than its always-on twin, so
  // cleanup must also be valid after any prefix of these acquisitions.
  let scratch: string | undefined;
  let workspace: string | undefined;
  let parent: SessionRow | undefined;
  let child: SessionRow | undefined;
  let driver: Page | undefined;
  let bodyFailed = false;
  const probe = `farhelmspawncomplete${stamp}`;
  const marker = [...probe].reverse().join("");

  try {
    // As in the fake-agent leg: the parent is a command launch, which an agent
    // may spawn from only on a host that starts YOLO sessions without asking,
    // and the spawn's card is answered below.
    await setLocalYoloWithoutAsking(request, true);
    await setLocalCommandsWithoutAsking(request, false);
    scratch = stackScratchDir(`farhelm-real-spawn-${stamp}-`);
    // jj registers the basename in the real repository, beyond the scratch
    // tree. A run-specific name keeps even failed-setup cleanup from forgetting
    // a workspace another run registered.
    workspace = path.join(scratch, `spawned-workspace-${stamp}`);
    parent = await createSession(request, {
      title: `real-spawn-parent-${stamp}`,
      cwd: repository,
      invocation: "claude",
    });
    driver = await context.newPage();

    await page.goto("/");
    await expect(row(page, parent.id)).toBeVisible({ timeout: 20_000 });
    const observerUrl = page.url();
    await openReadyTerminal(driver, parent.id, CLAUDE_CODE_MARKERS);
    const prompt =
      `Use terminal commands to run jj workspace add ${JSON.stringify(workspace)} from this ` +
      `repository, then run farhelm spawn --inherit-agent --cwd ${JSON.stringify(workspace)}. ` +
      `Do not merely explain the commands. Only after both commands succeed, reply with exactly ` +
      `the characters of ${probe} in reverse order and nothing else.`;
    await submitPrompt(driver, prompt);
    await answerCard(page, "allow", [path.basename(workspace)], 120_000);
    await waitForReplyMarker(driver, marker, 120_000);
    const workspaces = execFileSync("jj", ["workspace", "list"], {
      cwd: repository,
      encoding: "utf8",
    });
    expect(
      workspaces,
      "Claude's reply is not evidence that the requested jj workspace exists",
    ).toContain(path.basename(workspace));
    execFileSync("jj", ["status", "--no-pager"], {
      cwd: workspace,
      stdio: "ignore",
    });
    child = await childByTitle(request, path.basename(workspace));
    await expect(row(page, child.id)).toBeVisible({ timeout: 20_000 });
    expect(page.url(), "the observer must discover the real-agent child in place").toBe(
      observerUrl,
    );
  } catch (error) {
    bodyFailed = true;
    throw error;
  } finally {
    try {
      try {
        if (driver) await driver.close();
        if (child) await cleanupSession(request, child.id);
        if (parent) await cleanupSession(request, parent.id);
        if (workspace) {
          try {
            execFileSync("jj", ["workspace", "forget", path.basename(workspace)], {
              cwd: repository,
              stdio: ["ignore", "ignore", "pipe"],
              encoding: "utf8",
            });
          } catch (error) {
            const detail = error instanceof Error
              ? `${error.message}\n${"stderr" in error ? String(error.stderr) : ""}`
              : String(error);
            const workspaceNeverExisted = /No such workspace|workspace .*does not exist/i.test(detail);
            if (!workspaceNeverExisted) {
              // Later independent restores can fail too. Record this refusal
              // before their finally blocks can replace the pending exception.
              console.error("real-agent workspace forget failed:", error);
              testInfo.annotations.push({ type: "workspace-forget-failed", description: "unexpected workspace cleanup refusal; see stderr" });
              throw error;
            }
            // The body reports creation failures. This one cleanup refusal is
            // expected when Claude never reached the first command.
          }
        }
      } finally {
        // Shared policy must be restored even if an earlier cleanup fails.
        // Each nested finally also protects the next independent restore.
        try {
          if (scratch) fs.rmSync(scratch, { recursive: true, force: true });
        } finally {
          try {
            await setLocalCommandsWithoutAsking(request, false);
          } finally {
            await setLocalYoloWithoutAsking(request, false);
          }
        }
      }
    } catch (error) {
      if (!bodyFailed) throw error;
      console.error("real-agent spawn cleanup failed:", error);
      testInfo.annotations.push({ type: "cleanup-failed", description: "secondary cleanup failure; see stderr" });
    }
  }
});
