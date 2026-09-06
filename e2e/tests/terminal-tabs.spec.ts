// ---------------------------------------------------------------------
// Terminal tabs (PLAN_M4.md item 6).
//
// SPEC.md's session view "supports additional terminal tabs: plain shells
// spawned in the session's working directory". These drive the real strip
// against a real supervisor, a real tmux, and a real login shell — the
// fake agent is only ever the SESSION's agent here; every tab below runs
// the user's actual `$SHELL`, which is the whole point of the feature.
//
// Every test here works on its own session rather than the shared
// "e2e-session": a leftover tab would be visible to every test after it in
// this serially-run suite, and the per-project `beforeAll` reset is too
// coarse to catch that within a project's own pass.
// ---------------------------------------------------------------------
import { expect, newObservedContext, test } from "./helpers/evidence";
import { type Page, type APIRequestContext } from "@playwright/test";
import fs from "node:fs";
import { stubFeed } from "./helpers/fleet";
import { attachSession, cleanupSession, termText, waitForTermText } from "./helpers/term";
import { waitForSessionReady, waitForSessionRevealed } from "./helpers/terminal-readiness";
import {
  addTab,
  createTabSession,
  FLOOD_AGENT_COMMAND,
  fulfillAsHelm,
  islandText,
  runInShell,
  selectTerminal,
  sharedSessionRow,
  shellMarker,
  waitForIslandText,
  installTerminalSuiteHooks,
} from "./helpers/terminal-suite";

installTerminalSuiteHooks({ tabSweep: true });



/**
 * Every mounted island's `?lease=` value, keyed by terminal — `agent` for
 * the agent terminal, the tab id for a tab.
 *
 * Read off each island's actual socket URL rather than from any UI state,
 * because the lease is a WIRE fact: what matters is what the supervisor
 * was told, not what the view believes it sent.
 */
async function islandLeases(page: Page): Promise<Record<string, string>> {
  return page.evaluate(() => {
    const out: Record<string, string> = {};
    const islands = (window as any).__farhelmIslands ?? {};
    for (const el of Object.keys(islands)) {
      const url = new URL(islands[el].ws.url);
      const key = el === "terminal" ? "agent" : el.replace(/^terminal-/, "");
      out[key] = url.searchParams.get("lease") ?? "";
    }
    return out;
  });
}


/**
 * A snapshot of a live process's identity: its pid plus the boot-relative
 * start time Linux records for it.
 *
 * The start time is not decoration. A bare `kill(pid, 0)` answers "does
 * SOME process have this pid", which is a different question from "is the
 * process I was watching still running" — pids are recycled, and on a busy
 * host the shell this test killed can be replaced by an unrelated process
 * wearing the same number well inside the poll window. The supervisor
 * itself refuses to signal on a pid whose start time does not match (see
 * `signal_validated` in its own tests); this mirrors that discipline
 * rather than inventing a weaker one for the test suite.
 */
interface ProcessIdentity {
  pid: number;
  startTime: string;
}

/**
 * Read `/proc/<pid>/stat`, returning the process's state character and its
 * start time, or `undefined` if it is gone.
 *
 * The comm field (field 2) is parenthesized and may itself contain spaces
 * and parentheses, so the split starts after the LAST `)` — the standard
 * way to parse this file, and not optional: a shell whose argv made its
 * comm contain a space would otherwise shift every field after it.
 */
function readProcStat(pid: number): { state: string; startTime: string } | undefined {
  let raw: string;
  try {
    raw = fs.readFileSync(`/proc/${pid}/stat`, "utf8");
  } catch {
    return undefined;
  }
  const after = raw.slice(raw.lastIndexOf(")") + 2).split(" ");
  // Fields are 1-based in proc(5): state is 3 and starttime is 22, so
  // after dropping pid and comm they sit at indices 0 and 19.
  return { state: after[0], startTime: after[19] };
}

/** The identity of a process that must be running right now. */
function processIdentity(pid: number): ProcessIdentity {
  const stat = readProcStat(pid);
  expect(stat, `process ${pid} must exist`).toBeDefined();
  return { pid, startTime: stat!.startTime };
}

/**
 * Whether the process `identity` names is still running — reaped and
 * recycled both count as gone.
 *
 * A ZOMBIE counts as gone on purpose: SPEC.md's promise is that closing a
 * tab kills the shell, and a zombie is a dead shell whose parent has not
 * collected its status yet. Treating it as alive would fail this suite for
 * a kill that fully succeeded, which is a test bug dressed as a product
 * bug. A start time that no longer matches counts as gone for the opposite
 * reason: the pid is live, but it is not the process we killed.
 */
function pidAlive(identity: ProcessIdentity): boolean {
  const stat = readProcStat(identity.pid);
  if (!stat) return false;
  if (stat.state === "Z") return false;
  return stat.startTime === identity.startTime;
}

// The headline tab contract, end to end: a tab is a real shell in the
// SESSION's working directory (SPEC.md, and PLAN_M4.md acceptance 1), and
// it is a genuinely separate terminal — the agent's own pane must be
// untouched by anything typed into it. Both halves are asserted, because a
// tab wired to the agent's pane would pass the first on its own.
test("a tab runs a real shell in the session's working directory, leaving the agent terminal alone", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `tab-shell-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);
    await waitForTermText(page, "FAKE-AGENT READY");

    const tabId = await addTab(page, 0);
    const element = `terminal-${tabId}`;
    // Opening a tab selects it, so its pane is the visible one already;
    // this asserts that rather than assuming it.
    await expect(
      page.locator(`.terminal-pane[data-terminal="${tabId}"]`),
    ).toBeVisible();
    await waitForSessionReady(page, id, { tabId });

    // `$PWD` is what makes this about the SHELL's directory rather than
    // about the echo: the typed line carries the variable, only the
    // expansion carries the path (see `runInShell`).
    await runInShell(page, element, "echo \"TAB-CWD:$PWD\"", `TAB-CWD:${session.cwd}`);

    // The agent terminal is a different terminal, not a second view of the
    // same one: nothing typed into the tab may show up in it.
    expect(await termText(page)).not.toContain("TAB-CWD:");
    // ...and it is still live underneath, which is what makes the absence
    // above meaningful rather than merely a dead pane.
    await selectTerminal(page, "agent");
    await page.locator("#terminal").click();
    await page.keyboard.type("agent-still-live");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:agent-still-live", 15_000);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The strip's own contract: labels are positional and one-based
// (PLAN_M4.md item 6 — SPEC.md gives tabs no names, so this IS the naming
// rule), the agent terminal comes first, and it carries no close
// affordance at all. "No close affordance" is asserted structurally — the
// agent tab is not one of the closable slots, and the number of close
// buttons equals the number of TABS — rather than by looking for a
// disabled control, because an unclosable agent terminal means the button
// is absent, not merely inert.
test("the strip labels tabs positionally and gives the agent terminal no close control", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `tab-labels-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);

    await expect(page.locator(".tab-strip .tab-agent")).toHaveText("agent");
    await expect(page.locator(".tab-slot")).toHaveCount(0);
    await expect(page.locator(".tab-close")).toHaveCount(0);

    await addTab(page, 0);
    await addTab(page, 1);

    await expect(page.locator(".tab-slot .tab")).toHaveText([
      "Terminal 1",
      "Terminal 2",
    ]);
    // One close control per TAB, and the agent tab is not inside a slot —
    // together these say the agent terminal cannot be closed from here.
    await expect(page.locator(".tab-close")).toHaveCount(2);
    await expect(page.locator(".tab-slot .tab-agent")).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md's durability promise, extended to tabs: "Tabs survive client
// disconnects and supervisor restarts exactly like the agent terminal."
// A reload is the harshest client-side form of it — a brand-new page with
// empty buffers, so everything on screen afterwards came from replay.
//
// The reattached tab is checked while it is still HIDDEN, before anything
// selects it, which pins the other half of PLAN_M4.md item 6: every open
// tab attaches concurrently rather than on selection. A view that attached
// on select would show an empty buffer here and only fill it after the
// click below.
test("a tab survives a reload, reattaching with its scrollback while still unselected", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-reload-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);

    const tabId = await addTab(page, 0);
    const element = `terminal-${tabId}`;
    const before = shellMarker("BEFORE-RELOAD");
    await runInShell(page, element, before.command, before.expected);

    await page.reload();
    // A reload resets the app's navigation state, so it lands on the list
    // again — the same round trip the agent-terminal reload test makes.
    await attachSession(page, id);

    // The tab is listed again from the server's own rediscovery, not from
    // anything this client remembered across the reload.
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    await expect(page.locator(".tab-slot")).toHaveAttribute("data-tab-id", tabId);
    // Still on the agent terminal: selection is not persisted, which is
    // what makes the assertion below one about a HIDDEN, attached tab.
    await expect(page.locator(`.terminal-pane[data-terminal="${tabId}"]`)).toBeHidden();
    await waitForSessionRevealed(page, id, { tabId });
    await waitForIslandText(page, element, before.expected, 30_000);

    // And it is genuinely live once shown, not just replaying history.
    await selectTerminal(page, tabId);
    const after = shellMarker("AFTER-RELOAD");
    await runInShell(page, element, after.command, after.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Close is the whole per-tab operation set in v1, and SPEC.md makes it a
// kill: "A tab can be closed individually, which kills that shell and its
// processes." The confirmation in front of it is in-page for the same
// reason delete's and restart's are (wry ships no native JS dialogs on
// macOS's WKWebView, where `window.confirm()` silently does nothing).
//
// Three things are pinned, in order of how badly a regression in each
// would hurt: cancel sends nothing, confirm actually kills the shell
// process (checked against the pid the shell itself printed, not merely
// against the UI's own state), and the tab is gone from the server's tab
// list afterwards.
test("closing a tab confirms in-page, then kills its shell and drops it from the session", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-close-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);

    const tabId = await addTab(page, 0);
    const element = `terminal-${tabId}`;
    // `sh -c` for `shellMarker`'s portability reason, and `$PPID` rather
    // than `$$` BECAUSE of it: inside the `sh` this spawns, `$$` would be
    // that short-lived child, while the parent is the tab's own login
    // shell — which is the process SPEC.md's "closing kills that shell"
    // is about. A regex, not the literal prefix, because the interactive
    // shell echoes the command line first and only the expanded digits
    // prove anything ran (see `runInShell`).
    await runInShell(page, element, "sh -c 'echo TAB-PID:$PPID'", /TAB-PID:\d+/);
    const pidMatch = (await islandText(page, element)).match(/TAB-PID:(\d+)/);
    expect(pidMatch, "the tab's shell must report its own pid").toBeTruthy();
    const shell = processIdentity(Number(pidMatch![1]));
    expect(pidAlive(shell)).toBe(true);

    // Cancel first: the prompt is the only thing the × does, and backing
    // out of it must leave the tab (and its shell) exactly as they were.
    await page.locator(".tab-close").click();
    await expect(page.locator(".tab-confirm .confirm-consequence")).toContainText(
      "kills this terminal's shell",
    );
    await expect(page.locator(".tab-confirm .confirm-title")).toHaveText("Terminal 1");
    await page.locator(".tab-confirm .confirm-cancel").click();
    await expect(page.locator(".tab-confirm")).toHaveCount(0);
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    expect(pidAlive(shell)).toBe(true);

    await page.locator(".tab-close").click();
    await page.locator(".confirm-close-tab").click();

    // Gone from the strip, and the view falls back to the agent terminal
    // rather than leaving a selection pointing at nothing.
    //
    // The generous timeout is not slack for a slow assertion, it is the
    // shape of the operation: the strip only drops the tab once the DELETE
    // returns, and that reply waits on the whole tab-scoped reap — M2's
    // stop ordering, which walks the process tree, quiesces with a grace
    // period, kills, and re-enumerates (up to `MAX_QUIESCE_PASSES` times),
    // plus a systemd scope teardown where a user manager exists. Several
    // seconds is a NORMAL close on a loaded host, so Playwright's 5s
    // default would make this a timing test rather than a behavior one.
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 30_000 });
    await expect(page.locator('.terminal-pane[data-terminal="agent"]')).toBeVisible();

    // The kill is the real contract, so it is checked against the OS, not
    // against the UI. Polled rather than asserted once: the close reply
    // only comes after the reap, but the process table is a separate
    // observer with its own timing.
    await expect
      .poll(() => pidAlive(shell), {
        timeout: 15_000,
        message: "closing a tab must kill its shell",
      })
      .toBe(false);

    // ...and the server agrees the tab is gone, which is what a client
    // opening this session later will see.
    const detail = await (await request.get(`/api/sessions/${id}`)).json();
    expect(detail.tabs ?? []).toEqual([]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md's changes-appear-automatically rule, on the tab list: the
// session view polls the session DETAIL at the list's own cadence
// (PLAN_M4.md item 6), so a tab opened from another client shows up
// without a reload. The HTTP API stands in for "another client" the same
// way `list polls and picks up a session created elsewhere` in
// terminal.spec.ts uses it for a session created elsewhere.
//
// Bounded at ~15s — comfortably above the 3s poll interval — so a
// regression to "never polls the detail" fails rather than hangs.
test("a tab opened from another client appears in the strip without a reload", async ({
  page,
  request,
}) => {
  test.setTimeout(90_000);
  const title = `tab-poll-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);
    await expect(page.locator(".tab-slot")).toHaveCount(0);

    const opened = await request.post(`/api/sessions/${id}/tabs`);
    expect(opened.status(), await opened.text()).toBe(200);
    const tabId = (await opened.json()).tab.id;

    await expect(page.locator(".tab-slot")).toHaveCount(1, { timeout: 15_000 });
    await expect(page.locator(".tab-slot")).toHaveAttribute("data-tab-id", tabId);
    await expect(page.locator(".tab-slot .tab")).toHaveText("Terminal 1");
    // Discovered through polling, and then actually attached and live — a
    // strip entry with no working terminal behind it would be a worse bug
    // than not showing the tab at all.
    await waitForSessionRevealed(page, id, { tabId });
    await selectTerminal(page, tabId);
    const marker = shellMarker("POLLED-TAB");
    await runInShell(page, `terminal-${tabId}`, marker.command, marker.expected);

    // The counterpart: a tab closed from another client leaves the strip
    // the same way, with no reload and nothing left pointing at it.
    const closed = await request.delete(`/api/sessions/${id}/tabs/${tabId}`);
    expect(closed.status(), await closed.text()).toBe(200);
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 15_000 });
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// PLAN_M4.md item 3's session-scoped takeover, seen from the browser.
// SPEC.md's one-attached-client rule is per SESSION, not per terminal: a
// second view of the same session takes over ALL of the first view's
// terminals at once, because every terminal a view opens carries that
// view's single lease.
//
// This is the deliberate semantic change the lease introduces, so it is
// pinned on BOTH terminals rather than just the agent's — a build that
// leased only some of its terminals would still detach the agent here and
// look correct. Each losing terminal banners its own detach (the protocol
// sends one `Detached` per channel, with no session-wide message).
//
// The MECHANISM is asserted too, not just the outcome: the two terminals
// of one view must carry the SAME non-empty lease and the second view a
// different one. Without that, a build that simply failed to reuse leases
// would detach everything here for the wrong reason (each terminal taking
// over the last) and pass on the banners alone.
test("a second view of the same session detaches every terminal of the first", async ({
  browser,
  timeline,
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-takeover-${Date.now()}`;
  let id: string | undefined;
  let second: Awaited<ReturnType<typeof browser.newContext>> | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;

    await page.goto("/");
    await attachSession(page, id);
    const tabId = await addTab(page, 0);
    // A REAL attach signal, not merely a mounted island: terminal.js
    // publishes an island at the end of `mount()`, which is before the
    // socket's `onopen` and well before the supervisor-side attach it
    // triggers. Opening the second view against a first view whose tab had
    // not actually attached yet would test nothing — there would be no
    // second attachment to displace. A command the tab's own shell answers
    // is proof the whole path is up.
    const first = shellMarker("TAKEOVER-FIRST");
    await selectTerminal(page, tabId);
    await runInShell(page, `terminal-${tabId}`, first.command, first.expected);

    // Both of this view's terminals must be attached under ONE lease —
    // that shared identity is what makes the takeover below session-scoped
    // rather than per-terminal.
    const firstLeases = await islandLeases(page);
    expect(firstLeases.agent, "the agent terminal must carry a lease too").toBeTruthy();
    expect(firstLeases[tabId]).toBe(firstLeases.agent);

    second = await newObservedContext(browser, timeline);
    const page2 = await second.newPage();
    await page2.goto("/");
    // Playwright's `use.baseURL` reaching a MANUALLY created context is
    // load-bearing for the line above and easy to assume wrongly, so it is
    // checked rather than trusted: the two pages must have resolved "/"
    // against the same origin. If a future Playwright stopped applying
    // config options to a manual context, this fails here with a
    // clear reason instead of somewhere downstream as a mysterious
    // navigation.
    expect(new URL(page2.url()).origin).toBe(new URL(page.url()).origin);
    await attachSession(page2, id);

    // Both of the first view's terminals lost their attachment. Asserted
    // on text rather than visibility: only one pane is on screen at a
    // time, and the hidden one's banner is exactly as real (and as
    // load-bearing when the user switches back) as the visible one's.
    const agentBanner = page.locator('.terminal-pane[data-terminal="agent"] .banner');
    const tabBanner = page.locator(`.terminal-pane[data-terminal="${tabId}"] .banner`);
    await expect(agentBanner).toContainText("Detached", { timeout: 15_000 });
    await expect(tabBanner).toContainText("Detached", { timeout: 15_000 });

    // A DIFFERENT client, per the lease: same session, new view instance,
    // new identity. Equal leases would have been an ordinary reconnect
    // (farhelm-proto's `DETACH_REASON_REPLACED`), not the takeover this
    // test is named for.
    const secondLeases = await islandLeases(page2);
    expect(secondLeases.agent).toBeTruthy();
    expect(secondLeases.agent).not.toBe(firstLeases.agent);

    // The winner holds BOTH terminals, live. The agent terminal is
    // asserted as well as the tab: a takeover that handed over only the
    // terminal the winner happened to look at first would be a
    // half-transferred session, which is precisely what session-scoped
    // ownership is supposed to rule out.
    await selectTerminal(page2, tabId);
    const winner = shellMarker("TAKEOVER-WINNER");
    await runInShell(page2, `terminal-${tabId}`, winner.command, winner.expected);
    await selectTerminal(page2, "agent");
    await page2.locator("#terminal").click();
    await page2.keyboard.type("takeover-agent-live");
    await page2.keyboard.press("Enter");
    await waitForTermText(page2, "echo:takeover-agent-live", 15_000);
  } finally {
    if (second) await second.close();
    if (id) await cleanupSession(request, id);
  }
});

/**
 * Open a session in `page` with `count` revealed tabs, returning its ids in
 * strip order.
 *
 * The agent attachment is input-ready and every returned tab has completed
 * replay, but this does not promise final input focus or a shell prompt.
 * Callers deliberately choose focus and use their own command witness when
 * either is part of the behavior they exercise.
 *
 * Adds them through the UI rather than the API because that is also how a
 * user gets here, and because the returned ids are read back out of the
 * rendered strip either way (see `addTab`).
 */
async function openSessionWithTabs(
  page: Page,
  request: APIRequestContext,
  title: string,
  count: number,
): Promise<{ id: string; cwd: string; tabs: string[] }> {
  const session = await createTabSession(request, title);
  await page.goto("/");
  await attachSession(page, session.id);
  const tabs: string[] = [];
  for (let i = 0; i < count; i++) {
    const id = await addTab(page, i);
    await waitForSessionRevealed(page, session.id, { tabId: id });
    tabs.push(id);
  }
  return { ...session, tabs };
}

/** The element ids terminal.js currently has mounted, sorted. */
async function mountedIslands(page: Page): Promise<string[]> {
  return page.evaluate(() => Object.keys((window as any).__farhelmIslands ?? {}).sort());
}

/**
 * THE takeover-reclaim contract (the reason terminal.js has a latch at
 * all), driven with two real clients against the real stack.
 *
 * The bug this pins is an eviction loop, and it is worth stating plainly
 * because the fix looks like mere politeness otherwise: a displaced view
 * keeps polling the session detail, so it LEARNS about a tab the winner
 * opens. Without a latch it hands that tab to `sync()`, which attaches it
 * under the displaced view's still-valid lease — and the supervisor,
 * seeing a different lease, detaches the winner. The user who lost the
 * session silently steals it back, triggered by the winner doing nothing
 * more provocative than opening a terminal. That inverts SPEC.md's
 * one-attached-client rule rather than enforcing it.
 *
 * So this asserts the negative (A discovers the tab and does NOT attach
 * it, and B keeps working) and then the positive (A's explicit "take
 * control" reattaches everything and displaces B) — the second half
 * matters because a latch that could not be released would just be a
 * different way to lose the session.
 */
test("a displaced view discovers the winner's new tab without attaching it, until take control", async ({
  browser,
  timeline,
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-reclaim-${Date.now()}`;
  let id: string | undefined;
  let second: Awaited<ReturnType<typeof browser.newContext>> | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [firstTab] = session.tabs;
    const alive = shellMarker("RECLAIM-A-ALIVE");
    await runInShell(page, `terminal-${firstTab}`, alive.command, alive.expected);

    // B takes the session.
    second = await newObservedContext(browser, timeline);
    const page2 = await second.newPage();
    await page2.goto("/");
    await attachSession(page2, id);
    await expect(
      page.locator('.terminal-pane[data-terminal="agent"] .banner'),
    ).toContainText("Detached", { timeout: 15_000 });

    // B opens a second tab, through its own UI — the ordinary thing a
    // session's owner does, and the trigger for the whole bug.
    const secondTab = await addTab(page2, 1);
    await waitForSessionReady(page2, id, { tabId: secondTab });
    const winnerTab = shellMarker("RECLAIM-B-TAB");
    await runInShell(page2, `terminal-${secondTab}`, winnerTab.command, winnerTab.expected);

    // A's poll sees the new tab: it renders in A's strip, which is the
    // premise of the bug — A cannot attach what it never learned about.
    await expect(page.locator(".tab-slot")).toHaveCount(2, { timeout: 15_000 });

    // ...and A did NOT attach it. Both halves are checked, because either
    // alone could pass for the wrong reason: A holds no island for that
    // tab, and B — which a reattach would have evicted — is still live.
    expect(await mountedIslands(page)).not.toContain(`terminal-${secondTab}`);
    const stillB = shellMarker("RECLAIM-B-STILL");
    await runInShell(page2, `terminal-${secondTab}`, stillB.command, stillB.expected);
    await expect(
      page2.locator(`.terminal-pane[data-terminal="${secondTab}"] .banner`),
    ).toBeHidden();

    // The discovered-but-unattached tab says so rather than showing an
    // unexplained blank pane.
    await selectTerminal(page, secondTab);
    await expect(
      page.locator(`.terminal-pane[data-terminal="${secondTab}"] .banner`),
    ).toContainText("Detached");

    // Take control: an explicit act, in the banner where the loss was
    // reported. A must come back with EVERY terminal — including the one
    // it only ever saw listed — and B must lose them, which is the same
    // visible takeover B performed a moment ago.
    await page.locator(`.terminal-pane[data-terminal="${secondTab}"] .banner-reclaim`).click();
    await expect
      .poll(() => mountedIslands(page), {
        timeout: 20_000,
        message: "take control must reattach every terminal, the newly discovered tab included",
      })
      .toEqual(["terminal", `terminal-${firstTab}`, `terminal-${secondTab}`].sort());
    const reclaimed = shellMarker("RECLAIM-A-BACK");
    await runInShell(page, `terminal-${secondTab}`, reclaimed.command, reclaimed.expected);
    await expect(
      page2.locator('.terminal-pane[data-terminal="agent"] .banner'),
    ).toContainText("Detached", { timeout: 15_000 });
  } finally {
    if (second) await second.close();
    if (id) await cleanupSession(request, id);
  }
});

// PLAN_M4.md item 3's isolation claim, at the browser level: per-terminal
// flow control is what makes it safe to leave background tabs attached, so
// a viewer that stops draining ONE tab must pause only that tab.
//
// Driven with a real producer inside a real tab (the fake agent's flood
// script, run as an ordinary command — which is what a tab being a plain
// shell buys) and a write patch scoped to that one xterm instance, so the
// stall is genuinely one terminal's and not a page-wide freeze. The agent
// terminal and the sibling tab are proven unaffected by INPUT round trips,
// not merely by their counters staying zero: a page that had wedged
// entirely would also report zero pauses everywhere.
test("stalling one tab's writes pauses only that tab; the agent and a sibling stay live", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-isolation-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [stalled, sibling] = session.tabs;

    // Swallow completion callbacks for ONE island's terminal only. Applied
    // after mount (unlike the page-wide patches earlier in this file)
    // precisely so it can be scoped by instance identity — the producer
    // below has not started yet, so nothing is missed by patching late.
    await page.evaluate((el) => {
      const target = (window as any).__farhelmIslands[el].term;
      const real = (window as any).Terminal.prototype.write;
      (window as any).__heldWrites = [];
      (window as any).Terminal.prototype.write = function (data: unknown, cb?: () => void) {
        if (this === target && cb) {
          return real.call(this, data, () => (window as any).__heldWrites.push(cb));
        }
        return real.call(this, data, cb);
      };
      (window as any).__realWrite = real;
    }, `terminal-${stalled}`);

    // A tab is a plain shell, so the suite's own flood fixture runs in it
    // as an ordinary command — no special support needed on either side.
    await selectTerminal(page, stalled);
    await page.locator(`[id="terminal-${stalled}"]`).click();
    await page.keyboard.type(`${FLOOD_AGENT_COMMAND}`);
    await page.keyboard.press("Enter");

    await expect
      .poll(
        () =>
          page.evaluate(
            (el) => (window as any).__farhelmIslands[el].test.pauseCount,
            `terminal-${stalled}`,
          ),
        { timeout: 60_000, message: "the stalled tab must cross HIGH_WATER and pause" },
      )
      .toBeGreaterThanOrEqual(1);

    // The other two terminals never paused...
    const others = await page.evaluate(
      (els) =>
        els.map((el: string) => (window as any).__farhelmIslands[el].test.pauseCount),
      ["terminal", `terminal-${sibling}`],
    );
    expect(others).toEqual([0, 0]);
    // ...and, more to the point, still work. This is the assertion that
    // would fail if one wedged terminal had pinned the whole connection.
    await selectTerminal(page, "agent");
    await page.locator("#terminal").click();
    await page.keyboard.type("isolation-agent");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:isolation-agent", 20_000);
    const live = shellMarker("ISOLATION-SIBLING");
    await selectTerminal(page, sibling);
    await runInShell(page, `terminal-${sibling}`, live.command, live.expected);

    // Releasing the held callbacks drains the backlog and the paused tab
    // resumes on its own — the other half of the watermark contract.
    await page.evaluate(() => {
      (window as any).Terminal.prototype.write = (window as any).__realWrite;
      const held = (window as any).__heldWrites;
      (window as any).__heldWrites = [];
      for (const cb of held) cb();
    });
    await expect
      .poll(
        () =>
          page.evaluate(
            (el) => (window as any).__farhelmIslands[el].test.resumeCount,
            `terminal-${stalled}`,
          ),
        { timeout: 60_000, message: "draining must resume the paused tab" },
      )
      .toBeGreaterThanOrEqual(1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The multi-island version of `switching sessions tears down the mounted
// terminal; reselecting mounts a fresh one` in terminal.spec.ts: leaving a
// session with tabs open must tear down EVERY island, not just the agent's,
// and reopening must build genuinely new ones.
//
// Asserted at depth (registry empty, every socket actually CLOSED, fresh
// objects afterwards) rather than by the agent's singletons alone, because
// a per-island teardown that missed the tabs would leave the agent's
// globals looking perfectly clean while orphaned tab sockets kept the
// supervisor's attachments alive behind the user's back.
test("leaving a session with tabs tears down every island and reopening builds new ones", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-teardown-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;

    const before = await page.evaluate(() => {
      const islands = (window as any).__farhelmIslands;
      (window as any).__sockets = Object.keys(islands).map((el) => islands[el].ws);
      (window as any).__terms = Object.keys(islands).map((el) => islands[el].term);
      return Object.keys(islands).sort();
    });
    expect(before).toEqual(
      ["terminal", `terminal-${session.tabs[0]}`, `terminal-${session.tabs[1]}`].sort(),
    );

    // Leaving = selecting another session; the shared view mounts its own
    // single agent island, so "every island of THIS session is gone" is
    // the tab islands' absence plus all three stashed sockets closing —
    // the stash is what keeps the assertion about the departed session
    // rather than about whatever mounted next.
    await sharedSessionRow(page).click();
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");

    const after = await mountedIslands(page);
    expect(after).not.toContain(`terminal-${session.tabs[0]}`);
    expect(after).not.toContain(`terminal-${session.tabs[1]}`);
    await expect
      .poll(() =>
        page.evaluate(() => (window as any).__sockets.map((ws: WebSocket) => ws.readyState))
      )
      // 3 is CLOSED; there is no browser `WebSocket` global in this
      // Node-side context to name the constant by.
      .toEqual([3, 3, 3]);

    await attachSession(page, id);
    await expect(page.locator(".tab-slot")).toHaveCount(2);
    await expect
      .poll(() => mountedIslands(page), { timeout: 20_000 })
      .toEqual(before);
    // Genuinely new instances, not the old ones somehow surviving: the
    // same "fresh, not reused" property the single-terminal test pins,
    // extended across every island.
    expect(
      await page.evaluate(() => {
        const islands = (window as any).__farhelmIslands;
        const old = (window as any).__terms;
        return Object.keys(islands).every((el) => !old.includes(islands[el].term));
      }),
    ).toBe(true);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Restart touches the AGENT terminal alone (SPEC.md; the supervisor's
// `detach_for_restart` is scoped to it), and the UI has to respect that
// scope or it undoes the guarantee: rebuilding a tab's island would tear
// down an attachment the restart never touched, costing a full replay and
// interrupting a shell that was minding its own business.
//
// Pinned by IDENTITY, not by appearance: the tab's socket object must be
// the very same one after the restart, and it must still be interactive.
test("restarting the agent rebuilds only the agent island; a tab keeps its socket", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-restart-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [tabId] = session.tabs;
    const before = shellMarker("RESTART-TAB-BEFORE");
    await runInShell(page, `terminal-${tabId}`, before.command, before.expected);

    await page.evaluate((el) => {
      const islands = (window as any).__farhelmIslands;
      (window as any).__tabWs = islands[el].ws;
      (window as any).__agentWs = islands["terminal"].ws;
    }, `terminal-${tabId}`);

    const restartButton = page.locator(".restart-primary");
    await expect(restartButton).toHaveAttribute("data-confirms", "true", {
      timeout: 20_000,
    });
    await restartButton.click();
    await page.locator(".restart-confirm").click();

    // The agent's socket really was replaced (otherwise "the tab's was
    // not" would be a claim about a restart that never remounted
    // anything).
    await expect
      .poll(
        () => page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws !== (window as any).__agentWs),
        { timeout: 60_000, message: "the restart must rebuild the agent island" },
      )
      .toBe(true);

    // ...and the tab's was not: same object, still open, still answering.
    expect(
      await page.evaluate(
        (el) => (window as any).__farhelmIslands[el].ws === (window as any).__tabWs,
        `terminal-${tabId}`,
      ),
    ).toBe(true);
    await selectTerminal(page, tabId);
    const after = shellMarker("RESTART-TAB-AFTER");
    await runInShell(page, `terminal-${tabId}`, after.command, after.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Selecting a tab must put the keyboard where the user is looking, without
// them having to click into the terminal first — otherwise every switch
// costs an extra click and the first keystroke after a switch goes nowhere.
//
// Typed with `page.keyboard` straight after the strip click, with no click
// into the pane at all: that is the whole point, and it only works because
// terminal.js moves focus as part of applying the selection.
test("selecting a terminal focuses it, so typing works without clicking the pane", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-focus-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [tabId] = session.tabs;

    await selectTerminal(page, "agent");
    await page.keyboard.type("focus-agent");
    await page.keyboard.press("Enter");
    await waitForTermText(page, "echo:focus-agent", 20_000);

    await selectTerminal(page, tabId);
    await page.keyboard.type("sh -c 'echo FOCUS-TAB-$((6*7))'");
    await page.keyboard.press("Enter");
    await waitForIslandText(page, `terminal-${tabId}`, "FOCUS-TAB-42", 30_000);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// One island failing to mount must not take its siblings down with it —
// with tabs, an exception during a mount would otherwise leave a session
// view with no terminals at all instead of one broken one.
//
// The failure is injected at `WebSocket` construction for exactly one
// path (the same technique the single-terminal rollback test uses, made
// selective), which also exercises the rollback: the failed island must
// leave nothing registered and must say why in its own banner.
test("a tab whose mount fails is rolled back and bannered while its siblings stay live", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-mount-fail-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    // Both tabs are opened through the API first, so both exist before the
    // page ever loads and BOTH mount in the same `sync()` — which is what
    // makes "the sibling survived" a statement about that sync rather than
    // about two unrelated mounts at different times.
    const doomed = (await (await request.post(`/api/sessions/${id}/tabs`)).json()).tab.id;
    const healthy = (await (await request.post(`/api/sessions/${id}/tabs`)).json()).tab.id;

    await page.goto("/");
    await page.addInitScript((bad) => {
      const Real = window.WebSocket;
      const Shim = function (url: string, protocols?: any) {
        if (String(url).includes(`tab=${bad}`)) {
          throw new Error("injected failure for one island");
        }
        return new Real(url, protocols);
      } as unknown as typeof WebSocket;
      Shim.prototype = Real.prototype;
      // The readyState CONSTANTS have to come along, and forgetting them
      // is not a cosmetic omission: terminal.js gates every send on
      // `ws.readyState === WebSocket.OPEN`, which against a shim missing
      // `OPEN` compares to `undefined` and silently swallows all input.
      // The healthy siblings this test is about would then look attached
      // and answer nothing — a test artifact indistinguishable from the
      // product bug it is meant to rule out.
      for (const key of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
        (Shim as any)[key] = (Real as any)[key];
      }
      window.WebSocket = Shim;
    }, doomed);
    await page.reload();
    await page.locator(`[data-session-id="${id}"]`).click();

    // The failed island rolled back: nothing registered under its id...
    await expect
      .poll(() => mountedIslands(page), {
        timeout: 30_000,
        message: "the healthy terminals must mount even though a sibling threw",
      })
      .toEqual(["terminal", `terminal-${healthy}`].sort());
    // ...and it says so where the user is looking, rather than showing an
    // unexplained blank pane.
    await expect(
      page.locator(`.terminal-pane[data-terminal="${doomed}"] .banner`),
    ).toContainText("Failed to start terminal");

    // The siblings are not merely mounted but working.
    await waitForTermText(page, "FAKE-AGENT READY", 30_000);
    const live = shellMarker("MOUNT-FAIL-SIBLING");
    await selectTerminal(page, healthy);
    await runInShell(page, `terminal-${healthy}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// terminal.js keys its pending mounts by element id so one terminal
// waiting on xterm's globals cannot hold up another — and so a wait for a
// terminal that has since LEFT the desired set is cancelled rather than
// left to fire into a view that no longer wants it. That second property
// is what this pins: the M2-era "stale mount retry" bug, in its per-tab
// form.
//
// `window.Terminal` is withheld to force a genuinely pending mount (an
// unloaded box otherwise resolves the first readiness check immediately),
// exactly as the single-terminal regression test does.
test("a pending tab mount is cancelled when the tab leaves the desired set", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-pending-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.goto("/");
    await attachSession(page, id);

    // Withheld AFTER the agent mounted, so only the tab below is left
    // pending — the agent terminal staying up is what lets this test keep
    // observing the page at all.
    await page.evaluate(() => {
      (window as any).__stashedTerminal = (window as any).Terminal;
      delete (window as any).Terminal;
    });

    const opened = await request.post(`/api/sessions/${id}/tabs`);
    const tabId = (await opened.json()).tab.id;
    await expect(page.locator(".tab-slot")).toHaveCount(1, { timeout: 15_000 });
    expect(await mountedIslands(page)).toEqual(["terminal"]);

    // Gone again before it could ever mount.
    const closed = await request.delete(`/api/sessions/${id}/tabs/${tabId}`);
    expect(closed.ok()).toBe(true);
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 20_000 });

    // Restoring the global would let any surviving retry fire. Nothing
    // may: the pending attempt was cancelled when the tab left the set,
    // and a mount now would attach a terminal the supervisor destroyed.
    await page.evaluate(() => {
      (window as any).Terminal = (window as any).__stashedTerminal;
      delete (window as any).__stashedTerminal;
    });
    await expect
      .poll(() => mountedIslands(page), { timeout: 5_000, intervals: [500, 500, 500, 500] })
      .toEqual(["terminal"]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The add control's re-entry guard: a slow open must not become several
// tabs because the user pressed the button again. Asserted on the WIRE
// (how many POSTs left the browser), which is the only place the answer is
// unambiguous — counting rendered tabs afterwards would be satisfied by a
// UI that merely deduplicated its own mistake.
test("repeated activations while an open is in flight produce exactly one tab", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-reentry-${Date.now()}`;
  let id: string | undefined;
  let posts = 0;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.route(`**/api/sessions/${session.id}/tabs`, async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      posts++;
      // Long enough that the extra activations below land while the first
      // request is genuinely still open.
      await new Promise((resolve) => setTimeout(resolve, 2_000));
      await route.continue();
    });

    await page.goto("/");
    await attachSession(page, id);

    // Three activations in ONE synchronous turn, so they all land before
    // any re-render can disable the control — which is the window the
    // signal-level guard exists for.
    await page.evaluate(() => {
      const add = document.querySelector(".tab-add") as HTMLButtonElement;
      add.click();
      add.click();
      add.click();
    });

    await expect(page.locator(".tab-slot")).toHaveCount(1, { timeout: 30_000 });
    expect(posts, "one intended open must send one request").toBe(1);
    // The control comes back once the operation finishes, or the user
    // could never open a second tab.
    await expect(page.locator(".tab-add")).toBeEnabled();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md requires an open that cannot happen to fail with a clear error
// naming the reason, and PLAN_M4.md item 2 gives two of them: a working
// directory that has vanished, and a session whose tmux session is gone
// (which must be restarted first rather than growing a tab-only terminal).
//
// This drives the first against the REAL supervisor by deleting the
// session's working directory out from under it, so the message asserted
// is the supervisor's own rather than a fixture's. The control must also
// come back: an error that left the button stuck disabled would make the
// failure permanent from the user's side.
test("an open the supervisor refuses shows its own words and leaves the control usable", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-refusal-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.goto("/");
    await attachSession(page, id);

    // The session's working directory disappears. Removed only now, with
    // the session already up, so this is the vanished-cwd case rather
    // than a create that never worked.
    fs.rmSync(session.cwd, { recursive: true, force: true });

    await page.locator(".tab-add").click();
    const error = page.locator('.tab-error[data-tab-error="open"]');
    await expect(error).toBeVisible({ timeout: 30_000 });
    // The supervisor's own message, naming the directory — not a generic
    // "could not open a tab".
    await expect(error).toContainText(session.cwd);
    await expect(page.locator(".tab-slot")).toHaveCount(0);
    await expect(page.locator(".tab-add")).toBeEnabled();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Two guards in one flow, both about a destructive action firing more than
// the user asked for. A confirmed close must send exactly ONE DELETE for
// its tab even when the confirm button is activated twice in the same turn
// (the re-render that removes the prompt has not happened yet), while a
// DIFFERENT tab's close is not blocked by it — the guard is per tab, not a
// global lock, or closing two tabs would mean waiting out each reap in
// turn.
test("a confirmed close sends one DELETE per tab, and a sibling can close alongside it", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-close-guard-${Date.now()}`;
  let id: string | undefined;
  const deletes: string[] = [];
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [a, b] = session.tabs;
    await page.route(`**/api/sessions/${session.id}/tabs/*`, async (route) => {
      if (route.request().method() === "DELETE") {
        deletes.push(new URL(route.request().url()).pathname.split("/").pop()!);
      }
      await route.continue();
    });

    // Tab A: confirm twice in one synchronous turn.
    await page.locator(`.tab-slot[data-tab-id="${a}"] .tab-close`).click();
    await page.evaluate(() => {
      const confirm = document.querySelector(".confirm-close-tab") as HTMLButtonElement;
      confirm.click();
      confirm.click();
    });
    await expect(page.locator(`.tab-slot[data-tab-id="${a}"]`)).toHaveCount(0, {
      timeout: 30_000,
    });

    // Tab B closes normally afterwards — the per-tab guard released, and
    // was never holding B in the first place.
    await page.locator(`.tab-slot[data-tab-id="${b}"] .tab-close`).click();
    await page.locator(".confirm-close-tab").click();
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 30_000 });

    expect(deletes.sort()).toEqual([a, b].sort());
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Cancel and confirm dispatched in the same tick must never destroy
// anything — the mirror of the session list's own "dispatching cancel and
// confirm in the same tick never deletes the session" test, for a control
// whose consequence is killing a shell and everything under it.
//
// The order is cancel-then-confirm on purpose: that is the dangerous one,
// where a confirm click already queued behind a cancel would act on a
// decision the user just reversed.
test("cancel and confirm in the same tick close nothing", async ({ page, request }) => {
  test.setTimeout(120_000);
  const title = `tab-close-race-${Date.now()}`;
  let id: string | undefined;
  let deletes = 0;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    await page.route(`**/api/sessions/${session.id}/tabs/*`, async (route) => {
      if (route.request().method() === "DELETE") deletes++;
      await route.continue();
    });

    await page.locator(".tab-close").click();
    await expect(page.locator(".tab-confirm")).toHaveCount(1);
    await page.evaluate(() => {
      (document.querySelector(".tab-confirm .confirm-cancel") as HTMLButtonElement).click();
      (document.querySelector(".confirm-close-tab") as HTMLButtonElement)?.click();
    });

    await expect(page.locator(".tab-confirm")).toHaveCount(0);
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    expect(deletes).toBe(0);
    // The tab is not merely still listed but still attached and working.
    const live = shellMarker("CANCEL-RACE-LIVE");
    await runInShell(page, `terminal-${session.tabs[0]}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A close that FAILS must say so and change nothing: the tab stays listed,
// its island stays attached, and the control comes back. The failure is
// injected (a healthy stack has no reason to refuse) for the same reason
// the list view's stop/delete failure tests inject theirs.
//
// The error is also checked to be keyed to ITS OWN tab, which is what
// keeps a later success on a sibling from wiping a message the user has
// not read.
test("a failed close surfaces on that tab and leaves it attached", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-close-fail-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [failing, other] = session.tabs;
    await page.route(`**/api/sessions/${session.id}/tabs/${failing}`, async (route) => {
      if (route.request().method() !== "DELETE") {
        await route.continue();
        return;
      }
      await fulfillAsHelm(route, {
        status: 500,
        contentType: "text/plain",
        body: "injected close failure",
      });
    });

    await page.locator(`.tab-slot[data-tab-id="${failing}"] .tab-close`).click();
    await page.locator(".confirm-close-tab").click();

    const error = page.locator(`.tab-error[data-tab-error="${failing}"]`);
    await expect(error).toBeVisible({ timeout: 20_000 });
    await expect(error).toContainText("injected close failure");
    // Nothing was destroyed, and nothing is stuck: the tab is still there,
    // still attached, and its close control works again.
    await expect(page.locator(`.tab-slot[data-tab-id="${failing}"]`)).toHaveCount(1);
    expect(await mountedIslands(page)).toContain(`terminal-${failing}`);
    await expect(page.locator(`.tab-slot[data-tab-id="${failing}"] .tab-close`)).toBeEnabled();
    const live = shellMarker("CLOSE-FAIL-LIVE");
    await selectTerminal(page, failing);
    await runInShell(page, `terminal-${failing}`, live.command, live.expected);

    // A SUCCESSFUL close of the other tab must not erase the failure the
    // user has not acted on yet — the per-operation keying, made visible.
    await page.locator(`.tab-slot[data-tab-id="${other}"] .tab-close`).click();
    await page.locator(".confirm-close-tab").click();
    await expect(page.locator(`.tab-slot[data-tab-id="${other}"]`)).toHaveCount(0, {
      timeout: 30_000,
    });
    await expect(error).toBeVisible();
    await expect(error).toContainText("injected close failure");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Hidden panes are hidden with `visibility`, never `display: none`, and
// this is the assertion that pins the difference where it actually bites:
// a `display: none` element has no layout box, so `FitAddon.fit()` would
// size an unselected terminal to zero columns — and in a session with more
// than one terminal, every tab mounts while unselected.
//
// So the check is on the hidden terminal's GEOMETRY, not on its CSS: real
// pixel dimensions, and a real non-degenerate grid.
test("an unselected terminal keeps real geometry while hidden", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-hidden-geometry-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [tabId] = session.tabs;

    // Select the agent, so the TAB is the hidden one.
    await selectTerminal(page, "agent");
    await expect(page.locator(`.terminal-pane[data-terminal="${tabId}"]`)).toBeHidden();

    const hidden = await page.evaluate((el) => {
      const node = document.getElementById(el)!;
      const box = node.getBoundingClientRect();
      const term = (window as any).__farhelmIslands[el].term;
      return { width: box.width, height: box.height, cols: term.cols, rows: term.rows };
    }, `terminal-${tabId}`);
    expect(hidden.width).toBeGreaterThan(0);
    expect(hidden.height).toBeGreaterThan(0);
    // The floor is xterm.js's own minimum, so anything at it would mean
    // `fit()` measured nothing; a real pane is far above it.
    expect(hidden.cols).toBeGreaterThan(10);
    expect(hidden.rows).toBeGreaterThan(4);

    // And it is not merely sized but usable the instant it is shown.
    await selectTerminal(page, tabId);
    const live = shellMarker("HIDDEN-GEOMETRY");
    await runInShell(page, `terminal-${tabId}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The confirmation row and the tab-error lines sit above `.terminal-panes`
// in the same flex column, so opening either resizes every terminal while
// the window never moves. Before the per-island ResizeObserver, a terminal
// in that state kept stale geometry — the pane and the pty disagreeing
// about how many rows exist, which is what full-screen TUIs render as
// garbage.
//
// Toggling the confirmation is the cheapest real trigger for it, and its
// effect is asserted on the terminal's own row count rather than on
// pixels, because that is the number the pty is told.
test("opening the close confirmation refits the terminals below it", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-refit-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const rows = () =>
      page.evaluate(() => (window as any).__farhelmIslands["terminal"].term.rows);

    const before = await rows();
    await page.locator(".tab-close").click();
    await expect(page.locator(".tab-confirm")).toHaveCount(1);
    await expect
      .poll(rows, {
        timeout: 10_000,
        message: "a row appearing above the panes must shrink the terminals",
      })
      .toBeLessThan(before);

    // ...and closing it gives the rows back, so the refit is a live
    // response to the box rather than a one-way shrink.
    await page.locator(".tab-confirm .confirm-cancel").click();
    await expect(page.locator(".tab-confirm")).toHaveCount(0);
    await expect.poll(rows, { timeout: 10_000 }).toBe(before);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The window-resize path, extended across islands: every terminal must
// reflow, not just the one the user is looking at. A hidden tab that kept
// its old geometry would be wrong on screen the moment it was selected,
// and — worse — would have told the pty the wrong size in the meantime.
test("a viewport resize reflows every island, hidden ones included", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-resize-all-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const dims = () =>
      page.evaluate(() => {
        const islands = (window as any).__farhelmIslands;
        const out: Record<string, string> = {};
        for (const el of Object.keys(islands).sort()) {
          out[el] = `${islands[el].term.cols}x${islands[el].term.rows}`;
        }
        return out;
      });

    const before = await dims();
    expect(Object.keys(before)).toHaveLength(3);
    await page.setViewportSize({ width: 640, height: 480 });
    await expect
      .poll(
        async () => {
          const after = await dims();
          return Object.keys(before).every((el) => after[el] !== before[el]);
        },
        { timeout: 15_000, message: "every island must reflow, hidden ones included" },
      )
      .toBe(true);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A tab closed from ANOTHER client must be torn down here, not merely
// hidden: its island unmounted and its socket closed. A strip entry
// removed while its WebSocket stayed open would leave the supervisor
// holding an attachment for a window that no longer exists, invisible from
// this side. The sibling is checked too — teardown must be surgical.
test("a tab closed elsewhere is torn down here, leaving its sibling untouched", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-remote-close-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [gone, kept] = session.tabs;
    await page.evaluate((el) => {
      (window as any).__goneWs = (window as any).__farhelmIslands[el].ws;
    }, `terminal-${gone}`);

    const closed = await request.delete(`/api/sessions/${id}/tabs/${gone}`);
    expect(closed.ok(), await closed.text()).toBe(true);

    await expect(page.locator(`.tab-slot[data-tab-id="${gone}"]`)).toHaveCount(0, {
      timeout: 20_000,
    });
    await expect
      .poll(() => mountedIslands(page))
      .toEqual(["terminal", `terminal-${kept}`].sort());
    // 3 is CLOSED — the socket is genuinely gone, not merely unreferenced.
    await expect
      .poll(() => page.evaluate(() => (window as any).__goneWs.readyState))
      .toBe(3);

    const live = shellMarker("REMOTE-CLOSE-SIBLING");
    await selectTerminal(page, kept);
    await runInShell(page, `terminal-${kept}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A detail read that FAILS is not evidence about anything. The strip must
// keep showing the tabs it knows about and their terminals must keep
// working — a view that emptied itself on a transient 500 would tear down
// live attachments over a dropped request, which is the opposite of what
// the read is for.
//
// The reads are triggered rather than waited for: with the feed healthy
// nothing re-reads on a timer, so the test notifies to produce the first
// failure and the surface reader's own retry ladder (`reader`) produces the
// rest. Three failures in a row is what makes this "a run of failures the
// view rode out" rather than "one failure it happened to survive".
test("a failing detail read leaves the tabs and their terminals alone", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-read-fail-${Date.now()}`;
  let id: string | undefined;
  const feed = await stubFeed(page);
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    await feed.waitForConnection(1);
    feed.notify(1);
    const [tabId] = session.tabs;
    const before = await mountedIslands(page);

    let failures = 0;
    await page.route(`**/api/sessions/${session.id}`, async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      failures++;
      await fulfillAsHelm(route, { status: 500, contentType: "text/plain", body: "injected" });
    });

    // Nothing but failures for as long as the reader keeps asking.
    feed.notify(2);
    await expect.poll(() => failures, { timeout: 20_000 }).toBeGreaterThanOrEqual(3);

    await expect(page.locator(".tab-slot")).toHaveCount(1);
    expect(await mountedIslands(page)).toEqual(before);
    const live = shellMarker("POLL-FAIL-LIVE");
    await selectTerminal(page, tabId);
    await runInShell(page, `terminal-${tabId}`, live.command, live.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A 404 from the detail route is NOT proof the session is gone, and the
// view must neither act on it nor swallow it. The helm's detail route is
// not a per-session query at all — it fetches the listing and searches it
// (`get_session` in farhelm-helm), so it inherits the supervisor's listing
// cap and a perfectly healthy session past that cap answers 404 forever.
//
// A 404 is therefore ambiguous between "deleted elsewhere" and "not in
// this page", which is why the view says what it observed, keeps
// everything it has, and names both readings rather than picking one. The
// 404 is injected because provoking the real one would mean standing up
// hundreds of sessions to overflow the cap.
test("a session the helm stops listing is reported as stale, not torn down", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-stale-${Date.now()}`;
  let id: string | undefined;
  // Stubbed: both the arrival and the clearing of the notice are the
  // RESULT of a detail read, and a healthy page performs none on its own.
  const feed = await stubFeed(page);
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    await feed.waitForConnection(1);
    feed.notify(1);
    const [tabId] = session.tabs;
    const before = await mountedIslands(page);
    await expect(page.locator(".refresh-stale")).toHaveCount(0);

    let missing = true;
    await page.route(`**/api/sessions/${session.id}`, async (route) => {
      if (route.request().method() !== "GET" || !missing) {
        await route.continue();
        return;
      }
      await fulfillAsHelm(route, {
        status: 404,
        contentType: "text/plain",
        body: `no such session: ${session.id}\n`,
      });
    });

    const stale = page.locator(".refresh-stale");
    feed.notify(2);
    await expect(stale).toBeVisible({ timeout: 15_000 });
    // Both readings named, neither claimed.
    await expect(stale).toContainText("deleted from another client");
    await expect(stale).toContainText("more sessions than the helm lists");

    // Nothing was torn down on the strength of an ambiguous answer: the
    // strip, the islands, and the terminals all survive.
    await expect(page.locator(".tab-slot")).toHaveCount(1);
    expect(await mountedIslands(page)).toEqual(before);
    const live = shellMarker("STALE-LIVE");
    await selectTerminal(page, tabId);
    await runInShell(page, `terminal-${tabId}`, live.command, live.expected);

    // And it clears itself once the helm answers again — a staleness
    // notice that outlived its cause would be its own lie. A 404 is an
    // ANSWER rather than a failure, so the reader is idle and something has
    // to ask again; the next notification is that something.
    missing = false;
    feed.notify(3);
    await expect(stale).toHaveCount(0, { timeout: 15_000 });
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md's automatic tab reap (BUGS_BURNDOWN.md issue 3): a tab whose
// shell exits is reaped as if closed — gone from the server's tab list,
// gone from the strip, its island torn down, and the user back on the
// agent terminal. This test REPLACED the pre-reap contract, which kept a
// dead tab listed with its scrollback readable; the user decided an
// exited tab is done (2026-08-13), scrollback loss included. The agent
// terminal's exited-stays-viewable behavior is untouched and covered
// elsewhere. The sibling tab pins the reap's surgical scope, exactly as
// the closed-elsewhere test does for a manual remote close.
test("a tab whose shell exits is reaped: strip entry, island, and selection", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-shell-exit-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [dying, kept] = session.tabs;
    const marker = shellMarker("KEPT-ALIVE");
    await runInShell(page, `terminal-${kept}`, marker.command, marker.expected);

    await selectTerminal(page, dying);
    // Armed BEFORE the exit: the reap must be SILENT (SPEC.md — no notice,
    // no exit code), and the banner surface unmounts with the island, so
    // only an observer running through the whole removal can prove nothing
    // was ever painted on it. A transient "Detached:" flash passes every
    // after-the-fact query and is exactly the regression this catches.
    await page.evaluate((bannerId) => {
      (window as any).__reapBannerSeen = "";
      const banner = document.getElementById(bannerId);
      if (!banner) return;
      const observer = new MutationObserver(() => {
        const text = banner.textContent ?? "";
        if (text) (window as any).__reapBannerSeen = text;
      });
      observer.observe(banner, { childList: true, characterData: true, subtree: true });
    }, `term-banner-${dying}`);
    await page.locator(`[id="terminal-${dying}"]`).click();
    await page.keyboard.type("exit");
    await page.keyboard.press("Enter");

    // The server stops listing it (immediately at the listing layer; the
    // ticker's kill follows within a couple of seconds).
    await expect
      .poll(
        async () => {
          const detail = await (await request.get(`/api/sessions/${id}`)).json();
          return (detail.tabs ?? []).map((t: any) => t.id);
        },
        { timeout: 20_000, message: "an exited tab must stop being listed" },
      )
      .toEqual([kept]);

    // The UI follows: strip entry gone, island torn down, selection back
    // on the agent tab — the exact teardown a remote close gets.
    await expect(page.locator(`.tab-slot[data-tab-id="${dying}"]`)).toHaveCount(0, {
      timeout: 20_000,
    });
    await expect(page.locator(`[id="terminal-${dying}"]`)).toHaveCount(0);
    await expect(page.locator(".tab-agent")).toHaveClass(/selected/);

    // Silent means SILENT: nothing was ever painted on the dying tab's
    // banner (the observer above watched the whole removal), and no tab
    // error surface appeared anywhere in the view.
    expect(
      await page.evaluate(() => (window as any).__reapBannerSeen),
      "the reaped tab must never show a banner",
    ).toBe("");
    await expect(page.locator(".tab-error")).toHaveCount(0);

    // Surgical: the sibling tab's island and scrollback are untouched.
    await expect(page.locator(`.tab-slot[data-tab-id="${kept}"]`)).toHaveCount(1);
    expect(await islandText(page, `terminal-${kept}`)).toContain(marker.expected);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The repro behind BUGS_BURNDOWN.md issue 4: close a tab, open a fresh
// one, and the new island renders a single clean frame — no residue row
// parked BELOW the cursor. The residue came from a torn replay snapshot
// (content and cursor sampled from different frames while the
// just-opened shell repainted for an attach-time resize), so the
// assertion is shape-independent on purpose: whatever the host shell's
// prompt looks like, its cursor ends on the last painted line, and
// anything below that line is exactly the torn-snapshot artifact. The
// fix behind it is the open path pre-sizing the tab window to the
// agent's geometry, which makes the attach-time resize a no-op — a
// snapshot-side consistency retry was tried and rejected because it
// broke the cutover's losslessness (see tmux.rs's command-group comment).
test("a tab opened after closing another starts with a clean island", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-clean-reopen-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    const [first] = session.tabs;
    // Server-side close, like a remote client's x: the strip's own
    // confirm flow is covered elsewhere, and the supervisor-side close is
    // identical either way.
    const closed = await request.delete(`/api/sessions/${id}/tabs/${first}`);
    expect(closed.ok(), await closed.text()).toBe(true);
    await expect(page.locator(`.tab-slot[data-tab-id="${first}"]`)).toHaveCount(0, {
      timeout: 20_000,
    });

    const fresh = await addTab(page, 0);
    await waitForSessionRevealed(page, id, { tabId: fresh });
    await selectTerminal(page, fresh);
    // Wait for the shell to have painted anything at all, then assert the
    // cursor is the bottom of the painted content.
    await expect
      .poll(async () => (await islandText(page, `terminal-${fresh}`)).trim().length, {
        timeout: 20_000,
        message: "the fresh tab's shell never painted a prompt",
      })
      .toBeGreaterThan(0);
    const below = await page.evaluate((el) => {
      const island = (window as any).__farhelmIslands[el];
      const buf = island.term.buffer.active;
      const junk: string[] = [];
      for (let row = buf.baseY + buf.cursorY + 1; row < buf.length; row++) {
        const line = buf.getLine(row)?.translateToString(true).trimEnd();
        if (line) junk.push(`${row}: ${line}`);
      }
      return junk;
    }, `terminal-${fresh}`);
    expect(below, "no content may sit below the cursor in a fresh tab").toEqual([]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The other half of that split: a tab whose WINDOW is gone renders the
// session view's existing no-terminal explanation rather than a blank
// pane. Reaching that state honestly is not possible from outside the
// supervisor — a window it lost track of is by definition one it no longer
// lists — so the tab list is intercepted to name an id the supervisor
// never minted, which takes the same attach-refused path a vanished window
// takes and produces the same relayed explanation. Only the LISTING is
// synthetic; the attach, the refusal, and the banner are all real.
test("a tab the supervisor cannot attach explains itself instead of showing a blank pane", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-window-gone-${Date.now()}`;
  const phantom = "00000000-0000-4000-8000-00000000dead";
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    // Snapshotted once and served statically, for the reason the island-cap
    // test below spells out: a per-request `route.fetch()` on a route the
    // view polls leaves several `APIResponse`s in flight, and Playwright
    // disposes them as their routes complete.
    const detail = await (await request.get(`/api/sessions/${session.id}`)).json();
    detail.tabs = [...(detail.tabs ?? []), { id: phantom }];
    await page.route(`**/api/sessions/${session.id}`, async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      await fulfillAsHelm(route, {
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(detail),
      });
    });

    await page.goto("/");
    await attachSession(page, id);
    await expect(page.locator(".tab-slot")).toHaveCount(1, { timeout: 15_000 });

    await selectTerminal(page, phantom);
    const banner = page.locator(`.terminal-pane[data-terminal="${phantom}"] .banner`);
    await expect(banner).toBeVisible({ timeout: 20_000 });
    const text = await banner.textContent();
    expect(text).toMatch(/^Detached: .+/);
    expect(text).toContain(phantom);
    // Nothing was rendered into it — the explanation is instead of the
    // terminal's content, not on top of it.
    expect((await islandText(page, `terminal-${phantom}`)).trim()).toBe("");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The confirmation names a tab POSITIONALLY, and positions move: a
// lower-numbered sibling closed from another client renumbers everything
// after it. The label must follow, or the prompt would be asking about
// "Terminal 2" while pointing at what is now Terminal 1 — and, worse, the
// id it acts on must NOT follow, or the click would close whatever
// happened to land in that position.
test("a confirm prompt renumbers with the strip while still targeting its own tab", async ({
  page,
  request,
}) => {
  test.setTimeout(180_000);
  const title = `tab-confirm-reorder-${Date.now()}`;
  let id: string | undefined;
  const deleted: string[] = [];
  try {
    const session = await openSessionWithTabs(page, request, title, 2);
    id = session.id;
    const [first, second] = session.tabs;
    await page.route(`**/api/sessions/${session.id}/tabs/*`, async (route) => {
      if (route.request().method() === "DELETE") {
        deleted.push(new URL(route.request().url()).pathname.split("/").pop()!);
      }
      await route.continue();
    });

    // Confirm a close on the SECOND tab, then remove the first from
    // elsewhere so the confirmed tab becomes Terminal 1 under the prompt.
    await page.locator(`.tab-slot[data-tab-id="${second}"] .tab-close`).click();
    await expect(page.locator(".tab-confirm .confirm-title")).toHaveText("Terminal 2");

    const closed = await request.delete(`/api/sessions/${id}/tabs/${first}`);
    expect(closed.ok(), await closed.text()).toBe(true);
    await expect(page.locator(".tab-confirm .confirm-title")).toHaveText("Terminal 1", {
      timeout: 20_000,
    });
    // The prompt survived the reshuffle rather than being dismissed by it:
    // the user is mid-decision, and a list refresh is not an answer.
    await expect(page.locator(".tab-confirm")).toHaveCount(1);

    await page.locator(".confirm-close-tab").click();
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 30_000 });
    // The click acted on the tab the user chose, not on whatever now
    // occupies the position its label showed.
    expect(deleted).toEqual([second]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The close prompt's exact wording, asserted verbatim: it is the last
// thing a user reads before destroying a shell and every process under it,
// so it is a contract rather than an implementation detail. The Rust side
// pins the same sentence at its source (`CLOSE_TAB_CONSEQUENCE`); this
// pins that the sentence actually reaches the screen.
test("the close confirmation shows its exact consequence sentence", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-confirm-copy-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 1);
    id = session.id;
    await page.locator(".tab-close").click();
    await expect(page.locator(".tab-confirm .confirm-consequence")).toHaveText(
      "closing kills this terminal's shell and every process it started:",
    );
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A tab opened and then closed before any poll ever observed it must not
// come back. The optimistic entry that renders it immediately is retired
// by the close itself; without that, `closed_tabs` — which is pruned as
// soon as the server stops listing the id, and for a never-listed tab that
// is at once — would stop suppressing it and the strip would show a tab
// that attaches to nothing for the rest of the view's life.
//
// The poll interval is waited out deliberately: the bug only appears once
// a reconciliation runs, so asserting immediately would pass either way.
test("a tab opened and closed before any poll observes it does not come back", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-phantom-local-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.goto("/");
    await attachSession(page, id);

    const tabId = await addTab(page, 0);
    await page.locator(`.tab-slot[data-tab-id="${tabId}"] .tab-close`).click();
    await page.locator(".confirm-close-tab").click();
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 30_000 });

    // Several poll intervals later, still gone — and the server agrees.
    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 12_000 });
    const detail = await (await request.get(`/api/sessions/${id}`)).json();
    expect(detail.tabs ?? []).toEqual([]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The same phantom, arrived at from the other direction: this view opens a
// tab, ANOTHER client closes it before any poll here has ever listed it,
// and nothing this view did retires the optimistic entry. Only a poll that
// STARTED after the open can settle it — which is what the sequence number
// on each optimistic entry exists to establish. Without it, "absent" is
// indistinguishable from "the reply predates the open", and the entry can
// never be retired at all.
test("a tab closed elsewhere before this view ever listed it stops being shown", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-phantom-remote-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    await page.goto("/");
    await attachSession(page, id);

    const tabId = await addTab(page, 0);
    // Closed from elsewhere immediately, so this view's own state never
    // learns about it except through polling.
    const closed = await request.delete(`/api/sessions/${id}/tabs/${tabId}`);
    expect(closed.ok(), await closed.text()).toBe(true);

    await expect(page.locator(".tab-slot")).toHaveCount(0, { timeout: 20_000 });
    expect(await mountedIslands(page)).toEqual(["terminal"]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The island cap (`MAX_MOUNTED_TAB_ISLANDS`) is a bound on what this client
// will do with a LIST IT DID NOT AUTHOR, so it is exercised the way the
// threat arrives: a tab list the supervisor never produced. Intercepting
// the detail reply is not a shortcut here, it IS the case — a supervisor
// that is compromised, or merely wrong, is exactly what the cap defends
// against, and no healthy stack will produce it on request.
//
// Both halves are asserted: the strip tells the truth about every tab the
// session claims, and the browser is not asked to build an island for all
// of them.
test("a tab list past the island cap is listed in full but only partly attached", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const title = `tab-cap-${Date.now()}`;
  // Mirrors `MAX_MOUNTED_TAB_ISLANDS` in farhelm-ui/src/lib.rs. Duplicated
  // across the language boundary for the same reason `FLOOD_RECORDS` is,
  // and used the same way — in an equality check, so drift fails loudly
  // rather than weakening the assertion.
  const CAP = 32;
  const EXTRA = 3;
  let id: string | undefined;
  try {
    const session = await createTabSession(request, title);
    id = session.id;
    const phantoms = Array.from(
      { length: CAP + EXTRA },
      (_, i) => `00000000-0000-4000-8000-${String(i).padStart(12, "0")}`,
    );
    // Snapshotted ONCE and then served statically, rather than re-fetched
    // per request: the session view polls this route every few seconds, and
    // a handler that fetches on each call has several `APIResponse`s in
    // flight at a time — Playwright disposes those as their routes
    // complete, so a later handler can find the object it is reading
    // already gone. Nothing in this test needs the reply to stay live; it
    // needs the tab list to be a fixed, oversized fixture.
    const detail = await (await request.get(`/api/sessions/${session.id}`)).json();
    detail.tabs = phantoms.map((tabId) => ({ id: tabId }));
    await page.route(`**/api/sessions/${session.id}`, async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      await fulfillAsHelm(route, {
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(detail),
      });
    });

    await page.goto("/");
    await attachSession(page, id);

    // Every tab the session claims is listed — silently dropping some
    // would be its own lie about what the session holds.
    await expect(page.locator(".tab-slot")).toHaveCount(CAP + EXTRA, { timeout: 20_000 });
    // The ones past the cap say why they are not attached...
    await expect(page.locator(".terminal-not-mounted")).toHaveCount(EXTRA);
    // ...and no island was ever built for them. (The capped ones do mount
    // and then fail their attach, since these ids name no real window —
    // which is the ordinary refusal path, not what this test is about.)
    const islands = await mountedIslands(page);
    for (const tabId of phantoms.slice(CAP)) {
      expect(islands).not.toContain(`terminal-${tabId}`);
    }
    expect(islands.length).toBeLessThanOrEqual(CAP + 1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

/**
 * Resolves a `:root` custom property to the RGB string a browser would
 * compute for any real element that used it, via a live probe element's
 * `getComputedStyle`. Duplicated from chrome.spec.ts's identical helper
 * rather than shared: the two files have no other coupling, and a shared
 * helper module would exist for this one function alone.
 */
async function resolveToken(page: Page, token: string): Promise<string> {
  return page.evaluate((name) => {
    const probe = document.createElement("div");
    probe.style.color = `var(${name})`;
    document.body.appendChild(probe);
    const value = getComputedStyle(probe).color;
    probe.remove();
    return value;
  }, token);
}

/**
 * Keyboard focus and pointer hover are independent states here. A selected
 * tab keeps the lighter accent-hover fill under a pointer on its own, but a
 * keyboard user must still read selection as the resting accent even when
 * their pointer happens to be over the tab. The test therefore establishes
 * their intersection rather than relying on the mouse position a prior click
 * happened to leave behind.
 *
 * `.focus()` on the target rather than a full `Tab` key crawl to it, same
 * reasoning as chrome.spec.ts's own focus-visible test: a script `.focus()`
 * call is treated as keyboard arrival by the `:focus-visible` heuristic
 * without having to crawl the page's whole tab order to land on this one
 * element. A throwaway `Tab` press first establishes keyboard modality
 * regardless of whether setup needed a selection click. This caller adds
 * no tabs, and auto-selection can make its attachment entirely click-free.
 * Where Tab sends focus does not matter: the explicit `.focus()` that
 * follows lands it on the tab under test.
 */
test("a keyboard-focused selected tab keeps its resting accent even while hovered", async ({
  page,
  request,
}) => {
  const title = `tab-selected-focus-${Date.now()}`;
  let id: string | undefined;
  try {
    const session = await openSessionWithTabs(page, request, title, 0);
    id = session.id;
    const agentTab = page.locator('.tab-strip [data-terminal="agent"]');
    await expect(agentTab).toHaveClass(/selected/);

    const accentFill = await resolveToken(page, "--accent-fill");
    const accentFillHover = await resolveToken(page, "--accent-fill-hover");
    // A sanity check on the fixture itself: the two states this test
    // distinguishes are the resting accent and the accent-hover fill. If
    // those tokens ever collided, both assertions below would pass whether
    // or not keyboard focus restored the accent, and this test would stop
    // meaning anything.
    expect(accentFill).not.toBe(accentFillHover);

    // Establish keyboard modality explicitly before the focused sample;
    // `.focus()` alone would inherit whichever modality setup left behind.
    // A prior mount can legitimately leave the selected tab focused. Clear
    // that state before sampling ordinary hover; the later Tab-plus-focus
    // sequence is the distinct keyboard-visible state this test specifies.
    await agentTab.evaluate((node) => (node as HTMLElement).blur());
    await agentTab.hover();
    // `.btn` transitions its background for 100ms. `hover()` only starts
    // that fade, so a direct style read can catch an interpolated color under
    // load instead of the semantic ordinary-hover endpoint.
    await expect(agentTab).toHaveCSS("background-color", accentFillHover);
    expect(await agentTab.evaluate((node) => node.matches(":focus-visible"))).toBe(false);

    await page.keyboard.press("Tab");
    await agentTab.focus();
    // Hover after moving focus: this pins the real overlap at issue without
    // using a click, which would replace the keyboard modality the test needs
    // WebKit to expose through `:focus-visible`.
    await agentTab.hover();
    await expect(agentTab).toHaveCSS("background-color", accentFill);
    expect(
      await agentTab.evaluate((node) => node.matches(":focus-visible")),
      "the test must establish keyboard-visible focus",
    ).toBe(true);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});
