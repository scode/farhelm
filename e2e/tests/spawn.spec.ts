// Spawn from inside a real Farhelm terminal (PLAN_M7.md item 4).
//
// The always-on leg uses the deterministic fake-agent `spawn` script and a
// second page as the observer. The observer never reloads or navigates while
// the terminal creates children, so a row appearing there proves the normal
// drain and invalidation feed carry spawn visibility end to end.
//
// The final leg repeats the product contract with real Claude. It is gated on
// FARHELM_REAL_AGENT=1 because vendor credentials and network access are
// intentionally absent from CI, and it emits the same visible skip line as
// real-agent.spec.ts when ungated.
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import {
  cleanupProfile,
  cleanupSession,
  createProfile,
  createSession,
  listProfiles,
  listSessions,
  localHostId,
  type ProfileRow,
  type SessionRow,
} from "./helpers/fleet";
import {
  CLAUDE_CODE_MARKERS,
  submitPrompt,
  waitForReplyMarker,
  waitUntilAgentReady,
} from "./helpers/real-agent";
import { requireProductPageAuth } from "./helpers/device-auth";

const FARHELM = path.resolve(__dirname, "../../target/debug/farhelm");
const SPAWN_AGENT = `"${FARHELM}" internal fake-agent --script spawn`;

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
  await expect(row(page, id)).toBeVisible({ timeout: 20_000 });
  await row(page, id).locator(".session-row-open").click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitUntilAgentReady(page, markers);
}

/** Poll the real list endpoint for the uniquely titled child a spawn minted. */
async function childByTitle(request: APIRequestContext, title: string): Promise<SessionRow> {
  let child: SessionRow | undefined;
  await expect
    .poll(
      async () => {
        child = (await listSessions(request, `title=${encodeURIComponent(title)}`)).sessions.find(
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
  let host: number | undefined;
  let olderProfile: ProfileRow | undefined;
  let olderSession: SessionRow | undefined;
  let profile: ProfileRow | undefined;
  let parent: SessionRow | undefined;
  let unparented: SessionRow | undefined;
  let parented: SessionRow | undefined;
  let driver: Page | undefined;

  try {
    await requireProductPageAuth(context);
    root = fs.mkdtempSync(path.join(os.tmpdir(), `farhelm-spawn-${stamp}-`));
    const unparentedDir = path.join(root, "unparented-child");
    const parentedDir = path.join(root, "parented-child");
    fs.mkdirSync(unparentedDir);
    fs.mkdirSync(parentedDir);
    host = await localHostId(request);
    olderProfile = await createProfile(request, host, {
      name: `Older spawn fixture ${stamp}`,
      invocation: SPAWN_AGENT,
    });
    olderSession = await createSession(request, {
      title: `older-spawn-source-${stamp}`,
      cwd: root,
      profile_id: olderProfile.id,
    });
    profile = await createProfile(request, host, {
      name: `Spawn fixture ${stamp}`,
      invocation: SPAWN_AGENT,
    });
    parent = await createSession(request, {
      title: `spawn-parent-${stamp}`,
      cwd: root,
      profile_id: profile.id,
    });
    driver = await context.newPage();

    await page.goto("/");
    await expect(row(page, parent.id)).toBeVisible({ timeout: 20_000 });
    const observerUrl = page.url();
    await openReadyTerminal(driver, parent.id, {
      trustDialogMarkers: [],
      readyMarker: "FAKE-AGENT READY",
    });

    // This is the acceptance command: --cwd is the only supplied option.
    await submitPrompt(driver, `spawn ${unparentedDir}`, 100);
    await waitForReplyMarker(driver, "SPAWNED:");
    unparented = await childByTitle(request, path.basename(unparentedDir));
    expect(
      unparented.source_profile?.id,
      "selectorless spawn must derive the newest surviving profile-backed session",
    ).toBe(profile.id);
    await expect(row(page, unparented.id)).toBeVisible({ timeout: 20_000 });
    expect(page.url(), "the observer must not navigate to discover the child").toBe(observerUrl);

    // The fixture's second command adds the authenticated parent solely to
    // exercise the UI's exact direct-child filter.
    await submitPrompt(driver, `spawn-parented ${parentedDir}`, 100);
    await waitForReplyMarker(driver, "SPAWNED-PARENTED:");
    parented = await childByTitle(request, path.basename(parentedDir));
    await expect(row(page, parented.id)).toBeVisible({ timeout: 20_000 });
    expect(page.url()).toBe(observerUrl);

    await page.locator(".filter-parent").fill(parent.id);
    await page.locator(".filter-apply").click();
    await expect(row(page, parented.id)).toBeVisible({ timeout: 20_000 });
    await expect(row(page, unparented.id)).toHaveCount(0);
    await expect(page.locator(".session-row")).toHaveCount(1);
  } finally {
    if (driver) await driver.close();
    if (parented) await cleanupSession(request, parented.id);
    if (unparented) await cleanupSession(request, unparented.id);
    if (parent) await cleanupSession(request, parent.id);
    if (olderSession) await cleanupSession(request, olderSession.id);
    if (host !== undefined && profile) await cleanupProfile(request, host, profile.id);
    if (host !== undefined && olderProfile) await cleanupProfile(request, host, olderProfile.id);
    if (root) fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a real Claude creates a jj workspace and spawns into it without refreshing the observer", async ({
  page,
  context,
  request,
}) => {
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
  const probe = `farhelmspawncomplete${stamp}`;
  const marker = [...probe].reverse().join("");

  try {
    scratch = fs.mkdtempSync(path.join(os.tmpdir(), `farhelm-real-spawn-${stamp}-`));
    workspace = path.join(scratch, "spawned-workspace");
    const host = await localHostId(request);
    const claude = (await listProfiles(request, host)).profiles.find(
      (profile) => profile.name === "Claude Code",
    );
    if (!claude) throw new Error("the local supervisor has no exact `Claude Code` profile");
    parent = await createSession(request, {
      title: `real-spawn-parent-${stamp}`,
      cwd: repository,
      profile_id: claude.id,
    });
    driver = await context.newPage();

    await page.goto("/");
    await expect(row(page, parent.id)).toBeVisible({ timeout: 20_000 });
    const observerUrl = page.url();
    await openReadyTerminal(driver, parent.id, CLAUDE_CODE_MARKERS);
    const prompt =
      `Use terminal commands to run jj workspace add ${JSON.stringify(workspace)} from this ` +
      `repository, then run farhelm spawn --cwd ${JSON.stringify(workspace)}. ` +
      `Do not merely explain the commands. Only after both commands succeed, reply with exactly ` +
      `the characters of ${probe} in reverse order and nothing else.`;
    await submitPrompt(driver, prompt);
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
  } finally {
    if (driver) await driver.close();
    if (child) await cleanupSession(request, child.id);
    if (parent) await cleanupSession(request, parent.id);
    if (workspace) {
      try {
        execFileSync("jj", ["workspace", "forget", path.basename(workspace)], {
          cwd: repository,
          stdio: "ignore",
        });
      } catch (error) {
        const detail = error instanceof Error
          ? `${error.message}\n${"stderr" in error ? String(error.stderr) : ""}`
          : String(error);
        const workspaceNeverExisted = /No such workspace|workspace .*does not exist/i.test(detail);
        if (!workspaceNeverExisted) throw error;
        // The body reports creation failures. This one cleanup refusal is
        // expected when Claude never reached the first command.
      }
    }
    if (scratch) fs.rmSync(scratch, { recursive: true, force: true });
  }
});
