// Agent profiles in a real browser (PLAN_M6_75.md item 8): the hosts panel's
// per-host CRUD surface, the create dialog's picker and its ask-don't-guess
// fallback, and SPEC.md's snapshot rule as the session list shows it.
//
// A per-area spec of its own, per this milestone's convention (see
// feed.spec.ts's and titlebar.spec.ts's headers). Its helpers are the shared
// ones in helpers/fleet.ts, extended there with the profile CRUD calls these
// fixtures need.
//
// ## Most tests stub the feed; one deliberately does not
//
// Stubbed for filters.spec.ts's reason: the shared stack's other sessions keep
// changing status, and each change is a revision bump that would re-read — and
// therefore repaint — underneath the assertions below at unpredictable
// moments. A silent feed makes each test's DOM change exactly when the test
// asks it to, which is also what lets the invalidation tests prove that a
// notification is what moved a surface rather than a poll that happened to
// land.
//
// The exception is the two-client test, whose whole subject is the real path
// end to end: a real second browser, a real edit through the UI, a real feed
// on the observer, and no injected notification anywhere. A stub cannot detect
// a mutation that fails to publish, which is precisely what that test is for.
//
// ## Every test cleans its profiles up, and registers them before asserting
//
// Cleanup is not tidiness. A successful profile-backed create writes that
// host's REMEMBERED DEFAULT, which every later create dialog — in this file and
// in every other spec — then answers for. Neither outcome is neutral: a LIVE
// remembered profile is preselected (so the command field is disabled), and a
// DELETED one leaves the dialog selecting nothing and blocking the create until
// somebody answers. Both would break a spec that means to type a command, which
// is why every create helper in this suite now states "custom command"
// explicitly rather than relying on whatever the stack was left in — and why
// these fixtures still clean up after themselves, so what is left behind is at
// least a state the suite has a name for.
//
// Registration happens as soon as the object EXISTS server-side, before any
// assertion about the page — a repaint that never happens must not leak a
// profile or a session into a stack every later run shares.
import { expect, Page, Route, test } from "@playwright/test";
import {
  cleanupProfile,
  countReads,
  cleanupSession,
  createProfile,
  createSession,
  FAKE_AGENT,
  listHosts,
  listProfiles,
  listSessions,
  localHostId,
  ProfileRow,
  stubFeed,
  updateProfile,
} from "./helpers/fleet";
import type { FeedStub } from "./helpers/fleet";

/** The value of the picker's placeholder — `profiles::UNRESOLVED_VALUE`, the
 * option a dialog shows while nothing is selected and a create is blocked. */
const UNRESOLVED = "__unresolved__";

/** The row for one session id, as the list renders it. */
function row(page: Page, id: string) {
  return page.locator(`[data-session-id="${id}"]`);
}

/** One host's expanded profiles section. */
function section(page: Page, host: number) {
  return page.locator(`.profiles-section[data-profiles-host="${host}"]`);
}

/** One profile's row inside that section. */
function profileRow(page: Page, host: number, id: string) {
  return section(page, host).locator(`[data-profile-id="${id}"]`);
}

/**
 * Load the list with a stubbed, healthy feed, and hand the stub back.
 *
 * The handshake is played explicitly (`notify`) rather than armed, because
 * every test here wants the page HEALTHY from the start: an ungreeted socket
 * is torn down and reopened on the client's own schedule, and the fallback
 * poll would then be running underneath assertions about what a notification
 * caused.
 */
async function listWithStubbedFeed(page: Page): Promise<FeedStub> {
  const feed = await stubFeed(page);
  await page.goto("/");
  await feed.waitForConnection(1);
  feed.notify(1);
  await expect(page.locator(".hosts-panel")).toBeVisible({ timeout: 20_000 });
  return feed;
}

/** Expand one host's profiles section from its row in the hosts panel. */
async function openProfiles(page: Page, host: number) {
  await page.locator(`[data-host-id="${host}"] .host-profiles-toggle`).click();
  await expect(section(page, host)).toBeVisible({ timeout: 20_000 });
}

/** Collapse it again. */
async function closeProfiles(page: Page, host: number) {
  await page.locator(`[data-host-id="${host}"] .host-profiles-toggle`).click();
  await expect(section(page, host)).toHaveCount(0, { timeout: 20_000 });
}

/** Open the create dialog and wait for its agent picker. */
async function openCreateDialog(page: Page) {
  await page.locator(".new-session-button").click();
  await expect(page.locator(".create-session-profile")).toBeVisible({ timeout: 20_000 });
}

/** Wait until the picker offers one profile, which it can only do after the
 * catalog read for its target lands. */
async function waitForOption(page: Page, id: string) {
  await expect(page.locator(`.create-session-profile option[value="${id}"]`)).toHaveCount(1, {
    timeout: 20_000,
  });
}

/**
 * Per-host observation of the catalog endpoint, with a switch for making one
 * host unable to answer.
 *
 * Counting is what proves the two surfaces are independent: rows and pickers
 * can only show what a read produced, but "the closed one performed no read at
 * all" is a claim about requests and nothing else. The abort switch is what
 * turns "independent" from a coincidence into a demonstration — one host's
 * catalog failing must leave the other's surface untouched.
 */
interface CatalogWatch {
  /** Catalog GETs seen for `host` since the watch was installed. */
  reads(host: number): number;
  /** Every catalog GET, for a failure message that can name what was read. */
  total(): number;
  /** Fail every later catalog read for `host`, as an unreachable host would. */
  cut(host: number): void;
}

async function watchCatalogReads(page: Page): Promise<CatalogWatch> {
  const seen = new Map<number, number>();
  const cutOff = new Set<number>();
  await page.route(
    (url) => /^\/api\/hosts\/\d+\/profiles$/.test(url.pathname),
    async (route: Route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }
      const host = Number(new URL(route.request().url()).pathname.split("/")[3]);
      seen.set(host, (seen.get(host) ?? 0) + 1);
      if (cutOff.has(host)) {
        await route.abort();
        return;
      }
      await route.continue();
    },
  );
  return {
    reads: (host: number) => seen.get(host) ?? 0,
    total: () => [...seen.values()].reduce((sum, count) => sum + count, 0),
    cut: (host: number) => {
      cutOff.add(host);
    },
  };
}

/**
 * Observe the page's REAL feed sockets without interfering with them.
 *
 * A route would replace the socket, which is the one thing the two-client test
 * must not do; Playwright's `websocket` event only reports. What it makes
 * checkable is the attribution the test would otherwise have to assume: that
 * the observer was subscribed before the change was made, and that no
 * reconnect happened in between — a reconnect's own handshake triggers a
 * re-read, so an update seen across one says nothing about publication.
 */
function watchFeedSockets(page: Page): {
  opened(): number;
  closed(): number;
  greeted(): number;
} {
  let opened = 0;
  let closed = 0;
  let greeted = 0;
  page.on("websocket", (socket) => {
    if (!socket.url().includes("/api/events")) return;
    opened += 1;
    // The `websocket` event fires when the page ASKS for a socket, which is
    // not the same as being subscribed — the upgrade may still be in flight,
    // and a page that has not been greeted yet will re-read on its handshake
    // whether or not anything was published. Counting received FRAMES is what
    // establishes the subscription: the helm answers every (re)subscription
    // with the current revision immediately, so the first frame IS the proof.
    socket.on("framereceived", () => {
      greeted += 1;
    });
    socket.on("close", () => {
      closed += 1;
    });
  });
  return { opened: () => opened, closed: () => closed, greeted: () => greeted };
}

/** The bodies of every create this page POSTs, in order — what a claim about
 * idempotency keys and creation modes has to be made against, since neither is
 * visible in the DOM. */
async function watchCreateBodies(page: Page): Promise<Record<string, unknown>[]> {
  const bodies: Record<string, unknown>[] = [];
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route: Route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      bodies.push(route.request().postDataJSON());
      await route.continue();
    },
  );
  return bodies;
}

test.describe("agent profiles", () => {
  const created: string[] = [];
  const profiles: { host: number; id: string }[] = [];

  test.afterEach(async ({ request }) => {
    while (created.length) {
      const id = created.pop();
      if (id) await cleanupSession(request, id);
    }
    // After the sessions, deliberately: a session outliving its profile is
    // fine (that is the snapshot rule), and deleting the profile first would
    // leave the rows this suite creates describing a deleted profile for the
    // moment in between — which is a state some other spec could then read.
    while (profiles.length) {
      const profile = profiles.pop();
      if (profile) await cleanupProfile(request, profile.host, profile.id);
    }
  });

  /**
   * Find the profile this test just created through the UI and register it for
   * cleanup, before anything is asserted about the page.
   *
   * The order is the point. A profile that exists server-side and is not
   * registered is a leak the moment any later assertion fails, and it is not a
   * harmless one: a live profile changes what every later create dialog — in
   * this file and in every other spec — preselects.
   */
  async function registerByName(
    request: Parameters<typeof listProfiles>[0],
    host: number,
    name: string,
  ): Promise<ProfileRow> {
    let found: ProfileRow | undefined;
    await expect
      .poll(
        async () => {
          found = (await listProfiles(request, host)).profiles.find(
            (profile) => profile.name === name,
          );
          return found?.id;
        },
        { timeout: 20_000, message: `the profile ${name} must reach the supervisor's catalog` },
      )
      .toBeTruthy();
    profiles.push({ host, id: found!.id });
    return found!;
  }

  /**
   * The whole CRUD round trip, driven through the panel: define a profile,
   * edit it, delete it — each step confirmed in the catalog the helm proxies
   * from the owning supervisor.
   *
   * The API assertions are what make this more than a DOM test. A create that
   * only repainted locally, or a delete that only hid a row, would look
   * identical on screen; reading the host's catalog back proves the request
   * reached the supervisor that owns it.
   */
  test("profile CRUD round-trips from the hosts panel to the supervisor", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const name = `panel-profile-${Date.now()}`;
    const renamed = `${name}-edited`;

    await listWithStubbedFeed(page);
    await openProfiles(page, local);

    await section(page, local).locator(".new-profile-button").click();
    const form = section(page, local).locator(".profile-form");
    await form.locator(".profile-name-input").fill(name);
    await form.locator(".profile-invocation-input").fill(FAKE_AGENT);
    await form.locator(".profile-save").click();

    // Registered from the API's answer the moment it exists, before any
    // assertion about the page — see `registerByName`.
    const stored = await registerByName(request, local, name);
    await expect(profileRow(page, local, stored.id)).toBeVisible({ timeout: 20_000 });

    // Editing replaces the definition; the row is the same row (same id) with
    // a new name, which is what a rename IS here.
    await profileRow(page, local, stored.id).locator(".profile-edit").click();
    await profileRow(page, local, stored.id).locator(".profile-name-input").fill(renamed);
    await profileRow(page, local, stored.id).locator(".profile-save").click();
    await expect(profileRow(page, local, stored.id).locator(".profile-name")).toHaveText(renamed, {
      timeout: 20_000,
    });
    expect(
      (await listProfiles(request, local)).profiles.find((p) => p.id === stored.id)?.name,
    ).toBe(renamed);

    // Deleting confirms first — wry ships no native JS dialogs on macOS's
    // WKWebView, so every confirmation in this UI is in-page — and the
    // consequence it opens with is the snapshot rule itself.
    await profileRow(page, local, stored.id).locator(".profile-delete").click();
    await expect(profileRow(page, local, stored.id).locator(".confirm-consequence")).toContainText(
      "leaves every session already created from it running",
    );
    await profileRow(page, local, stored.id).locator(".profile-confirm-delete").click();

    await expect(profileRow(page, local, stored.id)).toHaveCount(0, { timeout: 20_000 });
    expect(
      (await listProfiles(request, local)).profiles.some((p) => p.id === stored.id),
      "the delete must reach the catalog, not merely the DOM",
    ).toBe(false);
  });

  /**
   * Editing ONE field of a profile leaves every other field exactly as it was
   * — kind, invocation, and a resume template the editor cannot fully express.
   *
   * This is the test the name-only round trip above cannot be: a save REPLACES
   * the whole definition, so the editor is one omitted field away from
   * silently clearing something, and one naive re-split away from rewriting an
   * argv it merely displayed. The fixture is deliberately hostile on both
   * counts — a non-generic kind (which the picker must preselect rather than
   * default away from) and a resume argv with a space inside an element (which
   * the single-line field can only round-trip by sending back what it was
   * seeded with).
   */
  test("editing only the name preserves the rest of a profile's definition", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const name = `verbatim-${Date.now()}`;
    // `{conversation}` is required for an integrated kind, and `--note=a b` is
    // the element that cannot survive a re-split.
    const template = ["claude", "--resume", "{conversation}", "--note=a b"];
    const invocation = "claude --dangerously-skip-permissions";
    const profile = await createProfile(request, local, {
      name,
      invocation,
      agent_kind: "claude",
      resume_template: template,
    });
    profiles.push({ host: local, id: profile.id });

    await listWithStubbedFeed(page);
    await openProfiles(page, local);
    const editing = profileRow(page, local, profile.id);
    await expect(editing).toBeVisible({ timeout: 20_000 });
    await editing.locator(".profile-edit").click();

    // The editor must OPEN on what is stored, not on a default: a kind select
    // showing `generic` here would save `generic`.
    await expect(editing.locator(".profile-kind-select")).toHaveValue("claude");
    await expect(editing.locator(".profile-invocation-input")).toHaveValue(invocation);
    // The field's exact spelling is the editor's business — it quotes what has
    // to be quoted so the value can be typed back (`profiles::resume_text`) —
    // so this asserts what the user can SEE rather than a rendering. The real
    // contract is the round trip, which the API read-back at the end of this
    // test makes: save with only the name touched, and the stored argv is
    // unchanged.
    const shownResume = await editing.locator(".profile-resume-input").inputValue();
    for (const argument of template) {
      expect(shownResume, `the field must show every argument, including ${argument}`)
        .toContain(argument);
    }

    await editing.locator(".profile-name-input").fill(`${name}-edited`);
    await editing.locator(".profile-save").click();
    await expect(editing.locator(".profile-name")).toHaveText(`${name}-edited`, {
      timeout: 20_000,
    });

    const stored = await (await request.get(`/api/hosts/${local}/profiles`)).json();
    const after = stored.profiles.find((p: { id: string }) => p.id === profile.id);
    expect(after.name).toBe(`${name}-edited`);
    expect(after.agent_kind, "the kind must not be rewritten by an edit that never touched it")
      .toBe("claude");
    expect(after.invocation).toBe(invocation);
    expect(
      after.resume_template,
      "an untouched resume field must send back the argv that was stored, not a re-split of how " +
        "it was displayed",
    ).toEqual(template);
  });

  /**
   * A save is visible to the NEXT editor immediately, even while the
   * authoritative catalog read is still in flight.
   *
   * The window is small and what happens inside it is durable: the operation
   * token is released when the save's own reply lands, so the row can be
   * reopened before the re-read commits. An editor seeded from the pre-edit
   * definition there would save it back and undo an update the supervisor had
   * already accepted — silently, since both saves succeed.
   */
  test("a saved profile is what the next editor sees, before the re-read lands", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const name = `delayed-read-${Date.now()}`;
    const profile = await createProfile(request, local, { name });
    profiles.push({ host: local, id: profile.id });

    await listWithStubbedFeed(page);
    // One route for the whole test, with a FLAG rather than a route added
    // later: adding one leaves a gap (a read already in flight cannot be
    // un-issued), and a flag can be flipped in the same statement sequence as
    // the click it must precede.
    let held = false;
    await page.route(
      (url) => /^\/api\/hosts\/\d+\/profiles$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() === "GET" && held) {
          await route.abort();
          return;
        }
        await route.continue();
      },
    );
    await openProfiles(page, local);
    const editing = profileRow(page, local, profile.id);
    await expect(editing).toBeVisible({ timeout: 20_000 });

    // From here on the authoritative read cannot answer, so everything below
    // is what the SAVE's own reply produced.
    held = true;
    await editing.locator(".profile-edit").click();
    await editing.locator(".profile-invocation-input").fill("edited-invocation");
    await editing.locator(".profile-save").click();
    // The form closes on success; the row now has to show the accepted
    // definition from the reply alone.
    await expect(editing.locator(".profile-form")).toHaveCount(0, { timeout: 20_000 });
    await expect(editing.locator(".profile-invocation")).toContainText("edited-invocation");

    // And reopening seeds the editor from it rather than from what the last
    // successful read said.
    await editing.locator(".profile-edit").click();
    await expect(
      editing.locator(".profile-invocation-input"),
      "an editor seeded from the pre-edit definition would save it back and undo the update",
    ).toHaveValue("edited-invocation");
    held = false;
  });

  /**
   * The create dialog defaults to the profile a session was last created from
   * on the target host — SPEC.md's creation rule, first half.
   *
   * The remembered default is the HELM's own state, written only by a
   * successful profile-backed create, so the fixture makes one through the
   * API: this test is about what the dialog does with that memory, not about
   * how it is recorded.
   *
   * The disabled command field is asserted alongside, because it is the
   * user-visible half of the wire's mutual exclusion: a create names either an
   * invocation or a profile and is refused for naming both, so a live field
   * beside a selected profile would invite typing a command that is not what
   * launches.
   */
  test("the create dialog preselects the profile last used on the target host", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, local, { name: `remembered-${Date.now()}` });
    profiles.push({ host: local, id: profile.id });
    const session = await createSession(request, {
      title: `remembered-session-${Date.now()}`,
      profile_id: profile.id,
      host: local,
    });
    created.push(session.id);

    await listWithStubbedFeed(page);
    await openCreateDialog(page);

    await expect(page.locator(".create-session-profile")).toHaveValue(profile.id, {
      timeout: 20_000,
    });
    await expect(page.locator(".create-session-profile-note")).toHaveCount(0);
    await expect(page.locator(".create-session-form input[type=\"text\"]").nth(1)).toBeDisabled();
    await expect(page.locator(".create-session-submit")).toBeEnabled();
  });

  /**
   * A remembered profile that no longer exists selects NOTHING and BLOCKS the
   * create until the user answers — SPEC.md's ask-don't-guess rule, and the
   * half of it that is easy to lose.
   *
   * Two substitutions are ruled out here, and the second is the one that hid
   * behind a friendly-looking fallback: quietly choosing another profile, and
   * quietly reverting to the command field — which is not empty, because a
   * user who typed a command before picking a profile still has it there.
   * Either would launch something nobody selected from a dialog whose own note
   * says nothing is.
   *
   * The catalog is deliberately NOT empty when this runs, so preselecting
   * nothing is a choice rather than the only option available.
   */
  test("a deleted last-used profile blocks the create until an agent is chosen", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const doomed = await createProfile(request, local, { name: `doomed-${Date.now()}` });
    profiles.push({ host: local, id: doomed.id });
    const survivor = await createProfile(request, local, { name: `survivor-${Date.now()}` });
    profiles.push({ host: local, id: survivor.id });

    const session = await createSession(request, {
      title: `doomed-session-${Date.now()}`,
      profile_id: doomed.id,
      host: local,
    });
    created.push(session.id);
    await cleanupProfile(request, local, doomed.id);
    expect(
      (await listProfiles(request, local)).default_profile,
      "the helm keeps the remembered id after the profile is gone; without that there is nothing " +
        "to ask about",
    ).toBe(doomed.id);

    await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await waitForOption(page, survivor.id);

    await expect(page.locator(".create-session-profile")).toHaveValue(UNRESOLVED);
    await expect(page.locator(".create-session-profile-note")).toContainText("no longer exists");
    await expect(
      page.locator(".create-session-submit"),
      "a dialog that cannot say what it would launch must not be submittable",
    ).toBeDisabled();
    // The survivor is OFFERED rather than chosen: the dialog has profiles
    // available and still picks none.
    await expect(page.locator(`.create-session-profile option[value="${survivor.id}"]`))
      .toHaveCount(1);

    // Choosing the command path explicitly is one of the two ways out, and it
    // unblocks the dialog — which is what makes the block a question rather
    // than a dead end.
    await page.locator(".create-session-profile").selectOption("");
    await expect(page.locator(".create-session-submit")).toBeEnabled();
    await expect(page.locator(".create-session-form input[type=\"text\"]").nth(1)).toBeEnabled();
  });

  /**
   * An explicit choice survives an edit to the profile it names, and a DELETE
   * of it blocks rather than substituting — with a surviving profile right
   * there to be substituted with.
   *
   * The rename half proves the choice is held by id rather than by label: the
   * picker follows the profile through the change instead of losing it. The
   * delete half is where a "helpful" implementation goes wrong, and the
   * presence of the survivor is what makes the assertion mean something.
   *
   * The last step is the host-change rule, which is not cosmetic: a profile id
   * means nothing on another supervisor, and because every fresh supervisor
   * seeds the same starter profiles, carrying one across does not fail loudly
   * — it resolves, to a profile nobody chose.
   */
  test("an explicit choice follows a rename, blocks on a delete, and never crosses hosts", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the e2e stack registers a second host; without it this test proves nothing")
      .toBeTruthy();
    const stamp = Date.now();
    const chosen = await createProfile(request, local, { name: `chosen-${stamp}` });
    profiles.push({ host: local, id: chosen.id });
    const survivor = await createProfile(request, local, { name: `bystander-${stamp}` });
    profiles.push({ host: local, id: survivor.id });

    const feed = await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await waitForOption(page, chosen.id);
    await page.locator(".create-session-profile").selectOption(chosen.id);

    // A rename keeps the id, so the choice holds and only the label moves.
    await updateProfile(request, local, chosen.id, { name: `chosen-${stamp}-renamed` });
    feed.notify(2);
    await expect(page.locator(`.create-session-profile option[value="${chosen.id}"]`))
      .toHaveText(`chosen-${stamp}-renamed`, { timeout: 20_000 });
    await expect(page.locator(".create-session-profile")).toHaveValue(chosen.id);
    await expect(page.locator(".create-session-submit")).toBeEnabled();

    // A delete takes the choice away, and nothing replaces it.
    await cleanupProfile(request, local, chosen.id);
    feed.notify(3);
    await expect(page.locator(".create-session-profile")).toHaveValue(UNRESOLVED, {
      timeout: 20_000,
    });
    await expect(page.locator(".create-session-profile-note")).toContainText("no longer on this host");
    await expect(page.locator(".create-session-profile")).not.toHaveValue(survivor.id);
    await expect(page.locator(".create-session-submit")).toBeDisabled();

    // And moving to another host discards the whole catalog before offering
    // anything: the previous host's options must not be selectable there.
    await page.locator(".create-session-host").selectOption(String(remote!.id));
    await expect(page.locator(`.create-session-profile option[value="${survivor.id}"]`))
      .toHaveCount(0, { timeout: 20_000 });
  });

  /**
   * Choosing a profile creates from it and the session records the snapshot —
   * the create dialog's half of SPEC.md's "creating a session offers the
   * target host's profiles".
   *
   * Asserted through the created session's own `source_profile` rather than
   * through the request body: the snapshot is what every later surface reads,
   * and a create that sent the profile but recorded nothing would leave the
   * session list unable to say what it was launched from.
   */
  test("choosing a profile creates from it and the session records the snapshot", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, local, { name: `chosen-create-${Date.now()}` });
    profiles.push({ host: local, id: profile.id });
    const title = `chosen-session-${Date.now()}`;

    await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await waitForOption(page, profile.id);
    await page.locator(".create-session-profile").selectOption(profile.id);
    const form = page.locator(".create-session-form");
    // Directory and title only: the command field is inert while a profile is
    // selected, which is the point — the profile says what runs.
    await form.locator('input[type="text"]').nth(0).fill("/tmp");
    await form.locator('input[type="text"]').nth(2).fill(title);
    await form.locator('button[type="submit"]').click();

    // Registered as soon as the session exists, before the view is asserted
    // about: a create that succeeded and then failed to navigate must not
    // leave an agent running in the shared stack.
    let matching: { id: string }[] = [];
    await expect
      .poll(
        async () => {
          matching = (await listSessions(request, `title=${encodeURIComponent(title)}`)).sessions;
          // EVERY match is registered as it is seen, not just the first: if a
          // second one somehow exists, the assertion below fails and the
          // cleanup still reaps both rather than leaving an agent running in
          // the shared stack.
          for (const found of matching) {
            if (!created.includes(found.id)) created.push(found.id);
          }
          return matching.length;
        },
        { timeout: 30_000, message: "the create must produce a session" },
      )
      .toBeGreaterThan(0);
    expect(
      matching.length,
      "one press of create is one session — the idempotency key exists so a retry cannot make two",
    ).toBe(1);
    const session = matching[0];

    await expect(page.locator(".titlebar .title")).toHaveText(title, { timeout: 30_000 });

    const detail = await (await request.get(`/api/sessions/${session.id}`)).json();
    expect(detail.source_profile?.id, "the session must record the profile it came from").toBe(
      profile.id,
    );
    expect(detail.source_profile?.existence).toBe("present");
    expect(
      (await listProfiles(request, local)).default_profile,
      "and a successful profile-backed create is what makes a profile the remembered default",
    ).toBe(profile.id);
  });

  /**
   * The idempotency key is bound to the creation MODE, not only to the text
   * fields: changing which agent a create would launch starts a new intent,
   * and the key that travels is paired with the mode that travels beside it.
   *
   * Only the request bodies can show this — neither the key nor the mode is on
   * screen — so the POSTs are recorded and compared. The create is made to
   * FAIL (a directory that does not exist), which is what keeps the form open
   * for the next submit and is also the case the key exists for: a retry of an
   * unchanged intent must reuse it.
   *
   * NOT covered here, deliberately: a selection changing DURING the key mint.
   * That window is a browser-side UUID call, too short to drive from
   * Playwright without pretending; what it must produce — one binding per
   * (fields, host, mode) triple — is pinned as a unit test over the binding
   * itself (`list::an_intent_binding_changes_with_the_host_incarnation_and_with_the_fields`).
   */
  test("the create's idempotency key is re-minted when its agent changes", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const stamp = Date.now();
    const first = await createProfile(request, local, { name: `key-a-${stamp}` });
    profiles.push({ host: local, id: first.id });
    const second = await createProfile(request, local, { name: `key-b-${stamp}` });
    profiles.push({ host: local, id: second.id });

    await listWithStubbedFeed(page);
    const bodies = await watchCreateBodies(page);
    await openCreateDialog(page);
    await waitForOption(page, first.id);

    const form = page.locator(".create-session-form");
    await form.locator('input[type="text"]').nth(0).fill("/nonexistent/definitely/not/here");
    await form.locator('input[type="text"]').nth(2).fill(`key-session-${stamp}`);
    await page.locator(".create-session-profile").selectOption(first.id);

    await form.locator('button[type="submit"]').click();
    await expect(form.locator(".create-session-error")).toBeVisible({ timeout: 20_000 });
    // An unchanged retry is the SAME intent — the whole reason the key
    // survives a failure.
    await form.locator('button[type="submit"]').click();
    await expect.poll(() => bodies.length, { timeout: 20_000 }).toBe(2);
    expect(bodies[0].profile_id).toBe(first.id);
    // The create names the connection it was prepared against, byte for byte
    // as the hosts read reported it — a profile-mode create is exactly the
    // request that would otherwise succeed on the WRONG install, because
    // starter ids collide across them.
    expect(bodies[0].expected_incarnation).toBe(
      (await listHosts(request)).find((host) => host.id === local)!.incarnation,
    );
    expect(bodies[0].invocation, "a profile-backed create must not also name an invocation")
      .toBeUndefined();
    expect(bodies[1].intent_key, "a retry of an unchanged intent reuses its key").toBe(
      bodies[0].intent_key,
    );

    // A different profile is a different intended create.
    await page.locator(".create-session-profile").selectOption(second.id);
    await form.locator('button[type="submit"]').click();
    await expect.poll(() => bodies.length, { timeout: 20_000 }).toBe(3);
    expect(bodies[2].profile_id).toBe(second.id);
    expect(bodies[2].intent_key).not.toBe(bodies[0].intent_key);

    // So is switching modes entirely.
    await page.locator(".create-session-profile").selectOption("");
    await form.locator('input[type="text"]').nth(1).fill(FAKE_AGENT);
    await form.locator('button[type="submit"]').click();
    await expect.poll(() => bodies.length, { timeout: 20_000 }).toBe(4);
    expect(bodies[3].profile_id, "a command create must not name a profile").toBeUndefined();
    expect(bodies[3].invocation).toBe(FAKE_AGENT);
    expect(bodies[3].intent_key).not.toBe(bodies[2].intent_key);
  });

  /**
   * The two catalog surfaces are independent, and a surface that is not shown
   * reads nothing at all.
   *
   * Counting requests is the only way to make either claim: what a picker
   * shows cannot distinguish "read for this host" from "read for some host and
   * rendered anyway", and "the closed one performed no read" is not visible on
   * screen by construction. The cut is what turns independence from a
   * coincidence into a demonstration — one host's catalog failing must leave
   * the other's surface exactly as it was.
   */
  test("each profile surface reads only its own host, and a closed one reads nothing", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const remote = (await listHosts(request)).find((host) => host.id !== local);
    expect(remote, "the e2e stack registers a second host; without it this test proves nothing")
      .toBeTruthy();
    const profile = await createProfile(request, local, { name: `isolated-${Date.now()}` });
    profiles.push({ host: local, id: profile.id });

    const feed = await listWithStubbedFeed(page);
    const watch = await watchCatalogReads(page);
    // A second counter, over an endpoint the FEED is known to drive, used as
    // the causal barrier for the negative assertion at the end.
    const listingReads = countReads(page);
    expect(
      watch.total(),
      "with no dialog open and no section expanded there is no catalog anyone is looking at",
    ).toBe(0);

    // The dialog reads the host it would create on, and only that one.
    await openCreateDialog(page);
    await waitForOption(page, profile.id);
    expect(watch.reads(local)).toBeGreaterThan(0);
    expect(watch.reads(remote!.id), "the dialog must not read a host it is not aimed at").toBe(0);

    // The panel's section is a second surface, asking a second question.
    await openProfiles(page, remote!.id);
    await expect
      .poll(() => watch.reads(remote!.id), { timeout: 20_000 })
      .toBeGreaterThan(0);

    // One host's catalog failing leaves the other's surface untouched — and
    // the failure has to be shown to have HAPPENED, not merely to have been
    // armed: both counts are watched across the notice, so the remote read
    // reached the cut and the local one answered anyway.
    const localReadsBefore = watch.reads(local);
    const remoteReadsBefore = watch.reads(remote!.id);
    watch.cut(remote!.id);
    feed.notify(2);
    await expect
      .poll(() => watch.reads(remote!.id), {
        timeout: 20_000,
        message: "the cut host must actually be read, or nothing was proven to fail",
      })
      .toBeGreaterThan(remoteReadsBefore);
    await expect
      .poll(() => watch.reads(local), { timeout: 20_000 })
      .toBeGreaterThan(localReadsBefore);
    await expect(page.locator(`.create-session-profile option[value="${profile.id}"]`))
      .toHaveCount(1);

    // Closed surfaces ignore notifications entirely.
    await closeProfiles(page, remote!.id);
    await page.locator(".new-session-button").click();
    await expect(page.locator(".create-session-profile")).toHaveCount(0);
    const quiet = { local: watch.reads(local), remote: watch.reads(remote!.id) };
    // The barrier that makes the negative assertion mean something: the
    // LISTING is notification-driven too, so waiting for its count to advance
    // proves this notice was received and acted on. A sleep would only prove
    // the test waited — and would fail or pass on how fast the machine is.
    const listings = listingReads;
    const before = listings.count();
    feed.notify(3);
    await expect
      .poll(() => listings.count(), {
        timeout: 20_000,
        message: "the page must have acted on the notification before absence proves anything",
      })
      .toBeGreaterThan(before);
    expect(watch.reads(local), "a closed dialog reads no catalog").toBe(quiet.local);
    expect(watch.reads(remote!.id), "a collapsed section reads no catalog").toBe(quiet.remote);
  });

  /**
   * Editing a profile does not touch the sessions already created from it —
   * SPEC.md's snapshot rule, as the list shows it.
   *
   * The rename happens with the page ALREADY OPEN and is then settled and
   * announced, rather than being staged before the page loads. Both halves are
   * deliberate. Renaming under an open page is the case the rule is about, and
   * the settle-then-notify is the stubbed-feed convention: a session's
   * `existence` is derived per reply by the owning supervisor and reaches the
   * merged list only when the helm's session cache next refreshes, so a
   * notification played before that lands would have the page re-read a view
   * that has not moved.
   */
  test("an edited profile leaves existing sessions naming what they snapshotted", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const before = `snapshot-before-${Date.now()}`;
    const after = `${before}-renamed`;
    const title = `snapshot-session-${Date.now()}`;
    const profile = await createProfile(request, local, { name: before });
    profiles.push({ host: local, id: profile.id });
    const session = await createSession(request, {
      title,
      profile_id: profile.id,
      host: local,
    });
    created.push(session.id);

    const feed = await listWithStubbedFeed(page);
    const label = row(page, session.id).locator(".session-profile");
    await expect(label).toBeVisible({ timeout: 20_000 });
    await expect(label).toContainText(before);
    await expect(label).toHaveAttribute("data-profile-existence", "present", { timeout: 20_000 });

    await updateProfile(request, local, profile.id, { name: after });
    await settleExistence(request, title, "renamed");
    feed.notify(2);

    await expect(label).toHaveAttribute("data-profile-existence", "renamed", { timeout: 20_000 });
    await expect(
      label,
      "the row must keep the name it snapshotted; adopting the profile's new one would rewrite " +
        "what the session was created from",
    ).toContainText(before);
    await expect(label).not.toContainText(after);

    // The catalog, meanwhile, says the new name — the two surfaces disagree
    // on purpose.
    await openProfiles(page, local);
    await expect(profileRow(page, local, profile.id).locator(".profile-name")).toHaveText(after, {
      timeout: 20_000,
    });
  });

  /**
   * A DELETED profile's sessions keep their snapshot too, and say so.
   *
   * The complement of the rename case, and the one where "leaves existing
   * sessions alone" is most easily broken in the other direction: a row that
   * vanished with its profile, or one that quietly dropped the name it was
   * created from, would both destroy the record of what a session actually
   * launched. The session is neither removed nor renamed — only the qualifier
   * beside the name changes.
   */
  test("a deleted profile's sessions keep their snapshot and say it is gone", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const name = `deleted-snapshot-${Date.now()}`;
    const title = `deleted-snapshot-session-${Date.now()}`;
    const profile = await createProfile(request, local, { name });
    profiles.push({ host: local, id: profile.id });
    const session = await createSession(request, { title, profile_id: profile.id, host: local });
    created.push(session.id);

    const feed = await listWithStubbedFeed(page);
    const label = row(page, session.id).locator(".session-profile");
    await expect(label).toHaveAttribute("data-profile-existence", "present", { timeout: 20_000 });

    await cleanupProfile(request, local, profile.id);
    await settleExistence(request, title, "deleted");
    feed.notify(2);

    await expect(label).toHaveAttribute("data-profile-existence", "deleted", { timeout: 20_000 });
    await expect(label).toContainText(name);
    await expect(
      row(page, session.id),
      "a session outlives the profile it was created from; removing the row would destroy the " +
        "record of what it launched",
    ).toBeVisible();
  });

  /**
   * A profile deleted elsewhere closes whatever this client had open on it —
   * an editor or a delete confirmation — and says so.
   *
   * Otherwise the row and its form vanish with the refresh while the draft,
   * the confirmation and their state go on existing invisibly: the next save
   * or confirm would then act on a profile that is gone, and the user would
   * have no idea why it failed. Both states are staged, because they are
   * tracked separately and only one of them would have been noticed.
   */
  test("a profile deleted elsewhere closes an open editor and confirmation", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const stamp = Date.now();
    const edited = await createProfile(request, local, { name: `vanish-edit-${stamp}` });
    profiles.push({ host: local, id: edited.id });
    const confirmed = await createProfile(request, local, { name: `vanish-delete-${stamp}` });
    profiles.push({ host: local, id: confirmed.id });

    const feed = await listWithStubbedFeed(page);
    await openProfiles(page, local);

    // An open EDITOR on a profile that then goes away.
    await profileRow(page, local, edited.id).locator(".profile-edit").click();
    await expect(profileRow(page, local, edited.id).locator(".profile-form")).toBeVisible();
    await cleanupProfile(request, local, edited.id);
    feed.notify(2);
    await expect(profileRow(page, local, edited.id)).toHaveCount(0, { timeout: 20_000 });
    const notice = section(page, local).locator(".profiles-notice");
    await expect(notice).toContainText("no longer on this host");

    // And an open CONFIRMATION, tracked separately.
    await profileRow(page, local, confirmed.id).locator(".profile-delete").click();
    await expect(profileRow(page, local, confirmed.id).locator(".profile-confirm-delete"))
      .toBeVisible();
    await cleanupProfile(request, local, confirmed.id);
    feed.notify(3);
    await expect(profileRow(page, local, confirmed.id)).toHaveCount(0, { timeout: 20_000 });
    await expect(notice).toContainText("already gone");
    // Nothing is left that could act on either: the section is back to its
    // ordinary state, with no form and no prompt anywhere in it.
    await expect(section(page, local).locator(".profile-form")).toHaveCount(0);
    await expect(section(page, local).locator(".profile-confirm-delete")).toHaveCount(0);
  });

  /**
   * A refused save keeps every draft field and changes nothing server-side.
   *
   * The refusal is a REAL one from the supervisor — a definition past the
   * per-profile size cap — rather than a routed reply, because the sentence
   * the user acts on is the supervisor's and a fabricated one would prove only
   * that this UI can render a string it was handed. What must survive is the
   * whole draft: a refused name is usually one keystroke from an accepted one,
   * and a form that cleared itself would make the user retype a definition
   * that was nearly right.
   */
  test("a refused profile save keeps the draft and leaves the catalog alone", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const before = (await listProfiles(request, local)).profiles.length;
    const oversized = "x".repeat(9_000);

    await listWithStubbedFeed(page);
    await openProfiles(page, local);
    await section(page, local).locator(".new-profile-button").click();
    const form = section(page, local).locator(".profile-form");
    await form.locator(".profile-name-input").fill(oversized);
    await form.locator(".profile-invocation-input").fill(FAKE_AGENT);
    await form.locator(".profile-kind-select").selectOption("codex");
    await form.locator(".profile-save").click();

    await expect(form.locator(".profile-form-error")).toBeVisible({ timeout: 20_000 });
    // Preserved, not cleared or reset — including the fields the refusal was
    // not about.
    await expect(form.locator(".profile-invocation-input")).toHaveValue(FAKE_AGENT);
    await expect(form.locator(".profile-kind-select")).toHaveValue("codex");
    expect(await form.locator(".profile-name-input").inputValue()).toHaveLength(oversized.length);
    expect(
      (await listProfiles(request, local)).profiles.length,
      "a refused create must leave the catalog exactly as it was",
    ).toBe(before);
  });

  /**
   * A success this build cannot READ is still a success: the form closes, the
   * page says its reply was unreadable, and the catalog re-read is what
   * settles what happened.
   *
   * The distinction is the whole point of the warning line. Treating an
   * undecodable 2xx as a refusal would tell the user their profile was
   * rejected when it demonstrably exists — and they would create it again.
   */
  test("a profile create whose reply cannot be read warns rather than refusing", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const name = `unreadable-${Date.now()}`;
    // ONE route for this path, handling both verbs. Two routes over the same
    // pattern would not compose: Playwright runs the LAST matching handler
    // and the earlier one never sees the request, so a second route added for
    // the reads would silently take the writes with it — and this test's
    // malformed reply would quietly become the real, readable one.
    //
    // The write is answered with a body this build cannot decode while the
    // real request still happens (the profile IS created). The read is HELD
    // while `held` is set, so what the section shows in between is the
    // client's own decision rather than a re-read papering over it.
    let held = true;
    await page.route(
      (url) => /^\/api\/hosts\/\d+\/profiles$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() === "POST") {
          const response = await route.fetch();
          await route.fulfill({
            response,
            body: "{ this is not the profile you are looking for",
            headers: { ...response.headers(), "content-type": "application/json" },
          });
          return;
        }
        if (held) {
          await route.abort();
          return;
        }
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    held = false;
    await openProfiles(page, local);
    const before = section(page, local).locator(".profile-row");
    await expect(before.first()).toBeVisible({ timeout: 20_000 });
    await section(page, local).locator(".new-profile-button").click();
    const form = section(page, local).locator(".profile-form");
    await form.locator(".profile-name-input").fill(name);
    await form.locator(".profile-invocation-input").fill(FAKE_AGENT);
    held = true;
    await form.locator(".profile-save").click();

    const stored = await registerByName(request, local, name);
    await expect(section(page, local).locator(".profiles-warning")).toBeVisible({
      timeout: 20_000,
    });
    // The held catalog is DROPPED, not left reopenable: this build cannot say
    // what the accepted change produced, so every row it still held is
    // suspect — and an editor seeded from one would save a definition known to
    // be superseded.
    await expect(
      section(page, local).locator(".profile-row"),
      "a success this build could not read must not leave stale rows editable",
    ).toHaveCount(0);
    // Deliberately NOT asserting which of the two empty states is showing.
    // The held read fails rather than hanging, so the section may be pending
    // or reporting that failure depending on whether the request had been
    // issued yet — and both are honest. What must hold is that nothing is
    // there to reopen and edit.
    await expect(section(page, local).locator(".profile-edit")).toHaveCount(0);

    // Only the authoritative read fills it back in.
    held = false;
    // An accepted change closes its form — the same as a readable success,
    // because the change is what closed it.
    await expect(form).toHaveCount(0, { timeout: 20_000 });
    // And the authoritative read is what puts the rows back, which is exactly
    // what the warning says it will.
    await expect(profileRow(page, local, stored.id)).toBeVisible({ timeout: 30_000 });
  });

  /**
   * A refused DELETE keeps the row, reports the refusal on that row, and
   * leaves its controls usable.
   *
   * The refusal a user meets here is the host-state one ("… is
   * unreachable-reprobing, so this operation is refused"), which cannot be
   * staged against a healthy stack for a host the test also needs alive — so
   * this one IS routed, and what it pins is the client's handling: the row
   * must not be removed optimistically, the message must land on the row it is
   * about rather than in a shared slot, and the operation token must be
   * released so the next attempt is possible.
   */
  test("a refused profile delete keeps the row and reports on it", async ({ page, request }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, local, { name: `undeletable-${Date.now()}` });
    profiles.push({ host: local, id: profile.id });
    await page.route(
      (url) => /^\/api\/hosts\/\d+\/profiles\/[^/]+$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "DELETE") {
          await route.continue();
          return;
        }
        await route.fulfill({
          status: 409,
          contentType: "text/plain",
          body: "host is unreachable-reprobing, so this operation is refused and nothing was queued",
        });
      },
    );

    await listWithStubbedFeed(page);
    await openProfiles(page, local);
    const target = profileRow(page, local, profile.id);
    await expect(target).toBeVisible({ timeout: 20_000 });
    await target.locator(".profile-delete").click();
    await target.locator(".profile-confirm-delete").click();

    await expect(target.locator(".profile-error")).toContainText("refused", { timeout: 20_000 });
    await expect(target, "a refused delete must not remove the row").toBeVisible();
    await expect(
      target.locator(".profile-delete"),
      "the operation token must be released, or the section is inert with nothing explaining why",
    ).toBeEnabled();
    expect(
      (await listProfiles(request, local)).profiles.some((p) => p.id === profile.id),
      "and the catalog still holds it",
    ).toBe(true);
  });

  /**
   * A profile edited elsewhere reaches an open profiles section on the next
   * notification, and NOT before — the panel surface's half of the
   * invalidation contract.
   *
   * The stub is what makes both halves provable: the socket is silent until
   * this test sends a revision, so the repaint cannot be a poll that happened
   * to land, and the assertion made BEFORE the notification is what shows the
   * surface is genuinely notification-driven rather than merely fast.
   *
   * No settle step here, deliberately, unlike the session-row tests: a catalog
   * is a LIVE read from the owning supervisor rather than something the helm
   * caches, so the awaited edit has already committed by the time it answers
   * and polling for it would assert nothing.
   */
  test("a profile edited elsewhere reaches an open profiles section on notification", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const before = `feed-profile-${Date.now()}`;
    const after = `${before}-elsewhere`;
    const profile = await createProfile(request, local, { name: before });
    profiles.push({ host: local, id: profile.id });

    const feed = await listWithStubbedFeed(page);
    await openProfiles(page, local);
    const name = profileRow(page, local, profile.id).locator(".profile-name");
    await expect(name).toHaveText(before, { timeout: 20_000 });

    await updateProfile(request, local, profile.id, { name: after });
    // Still showing the old name: nothing polls this surface while the feed is
    // healthy, which is what makes the assertion after the notice meaningful.
    await expect(name).toHaveText(before);

    feed.notify(2);
    await expect(name).toHaveText(after, { timeout: 20_000 });
  });

  /**
   * A dialog that is ASKING does not stop asking because another client's
   * create supplied a new remembered default.
   *
   * Staged with the dialog in the unresolved state deliberately: an explicit
   * choice would pass this test before the fix as well as after, because a
   * choice is what the old consumption check looked for. What it could not
   * represent is a dialog whose first catalog answered "the profile you last
   * used is gone" — that leaves NO choice behind, so every later refresh
   * re-consulted the default, and another client creating a session would
   * quietly select a profile under someone reading the question. The latch is
   * what makes the first answer the answer.
   */
  test("a dialog that is asking keeps asking when the remembered default moves", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const stamp = Date.now();
    // The dialog's first catalog must have a remembered default that is GONE.
    const doomed = await createProfile(request, local, { name: `ask-doomed-${stamp}` });
    profiles.push({ host: local, id: doomed.id });
    const survivor = await createProfile(request, local, { name: `ask-survivor-${stamp}` });
    profiles.push({ host: local, id: survivor.id });
    const first = await createSession(request, {
      title: `ask-first-${stamp}`,
      profile_id: doomed.id,
      host: local,
    });
    created.push(first.id);
    await cleanupProfile(request, local, doomed.id);

    const feed = await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await waitForOption(page, survivor.id);
    await expect(page.locator(".create-session-profile")).toHaveValue(UNRESOLVED);
    await expect(page.locator(".create-session-submit")).toBeDisabled();

    // Another client creates from the SURVIVOR, which makes it this host's
    // remembered default — a profile that resolves, so a dialog still
    // consulting the default would select it.
    const second = await createSession(request, {
      title: `ask-second-${stamp}`,
      profile_id: survivor.id,
      host: local,
    });
    created.push(second.id);
    await expect
      .poll(async () => (await listProfiles(request, local)).default_profile, { timeout: 20_000 })
      .toBe(survivor.id);
    feed.notify(2);

    // The page re-reads the catalog on that notice (the option list is proof
    // it did) — and the question is still the question.
    await expect(page.locator(`.create-session-profile option[value="${survivor.id}"]`))
      .toHaveCount(1, { timeout: 20_000 });
    await expect(
      page.locator(".create-session-profile"),
      "the first catalog decided, once; a later default is not this dialog's answer",
    ).toHaveValue(UNRESOLVED);
    await expect(page.locator(".create-session-submit")).toBeDisabled();
    await expect(page.locator(".create-session-profile-note")).toBeVisible();
  });

  /**
   * A selection changed and submitted in the SAME turn sends the new
   * selection, not the one the last render computed.
   *
   * One JavaScript turn is the whole window, and it is entirely reachable: a
   * keyboard user tabbing off the picker onto the create button produces
   * exactly this ordering. A handler that used a value captured at render time
   * would send the PREVIOUS profile under a freshly minted key — a key that
   * faithfully describes an intent nobody had.
   */
  test("a selection changed in the same turn as the submit is what gets sent", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const stamp = Date.now();
    const first = await createProfile(request, local, { name: `turn-a-${stamp}` });
    profiles.push({ host: local, id: first.id });
    const second = await createProfile(request, local, { name: `turn-b-${stamp}` });
    profiles.push({ host: local, id: second.id });

    await listWithStubbedFeed(page);
    const bodies = await watchCreateBodies(page);
    await openCreateDialog(page);
    await waitForOption(page, second.id);
    const form = page.locator(".create-session-form");
    await form.locator('input[type="text"]').nth(0).fill("/nonexistent/definitely/not/here");
    await form.locator('input[type="text"]').nth(2).fill(`turn-session-${stamp}`);
    await page.locator(".create-session-profile").selectOption(first.id);

    // Both events dispatched from ONE evaluation, with no chance for a render
    // to land in between: the select changes and the form submits in the same
    // turn.
    await page.evaluate((id) => {
      const picker = document.querySelector<HTMLSelectElement>(".create-session-profile")!;
      picker.value = id;
      picker.dispatchEvent(new Event("change", { bubbles: true }));
      document.querySelector<HTMLFormElement>(".create-session-form")!.requestSubmit();
    }, second.id);

    await expect.poll(() => bodies.length, { timeout: 20_000 }).toBeGreaterThan(0);
    expect(
      bodies[0].profile_id,
      "the create must carry the selection as it stood when the button was pressed",
    ).toBe(second.id);
  });

  /**
   * A dialog whose catalog has not answered yet selects nothing and blocks —
   * it does not fall back to the command field.
   *
   * SPEC.md's ask-don't-guess rule read conservatively, and the reason it is
   * not merely pedantic: the command field is not necessarily empty, and a
   * dialog that defaulted to it while still reading would let a create go out
   * carrying text typed for another intention. The state is transient and
   * always escapable, which this test also shows — the block lifts the moment
   * the catalog arrives.
   */
  test("a dialog blocks while its catalog is still unread", async ({ page, request }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, local, { name: `pending-${Date.now()}` });
    profiles.push({ host: local, id: profile.id });

    let held = true;
    await page.route(
      (url) => /^\/api\/hosts\/\d+\/profiles$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() === "GET" && held) {
          // Never answered while the assertion below runs: the dialog is
          // deciding with no catalog at all.
          await route.abort();
          return;
        }
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    await openCreateDialog(page);
    await expect(page.locator(".create-session-profile")).toHaveValue(UNRESOLVED, {
      timeout: 20_000,
    });
    await expect(
      page.locator(".create-session-submit"),
      "a dialog that cannot say what it would launch must not be submittable",
    ).toBeDisabled();

    // Typing a command IS an answer — the user said what they want.
    await page.locator(".create-session-form").locator('input[type="text"]').nth(1)
      .fill(FAKE_AGENT);
    await expect(page.locator(".create-session-submit")).toBeEnabled();
  });

  /**
   * The editor shows peer text ESCAPED, and saving an untouched field writes
   * back the original bytes.
   *
   * A profile name or invocation comes from a supervisor, which under `--ssh`
   * is a machine this helm does not control, and an `<input>` is the one place
   * this UI cannot isolate what it renders: a right-to-left override stays
   * active there, so what a person reads while editing can differ from what
   * they save — on a value that is about to be executed. The escaped form is
   * what makes the field say what is stored; the API read-back is what proves
   * nothing was mangled by saying so.
   */
  test("an editor shows escaped peer text and saves the original bytes", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    // A right-to-left override inside the name, and a zero-width space inside
    // the invocation.
    const name = `rlo-‮txt.exe-${Date.now()}`;
    const invocation = `${FAKE_AGENT}​`;
    const profile = await createProfile(request, local, { name, invocation });
    profiles.push({ host: local, id: profile.id });

    await listWithStubbedFeed(page);
    await openProfiles(page, local);
    const editing = profileRow(page, local, profile.id);
    await expect(editing).toBeVisible({ timeout: 20_000 });
    await editing.locator(".profile-edit").click();

    await expect(
      editing.locator(".profile-name-input"),
      "the field must show the escaped form: an active override inside an input can make the " +
        "text read as something other than what is stored",
    ).toHaveValue(name.replace("‮", "<U+202E>"));
    await expect(editing.locator(".profile-invocation-input")).toHaveValue(
      invocation.replace("​", "<U+200B>"),
    );

    // Save with only the KIND touched: every text field is untouched, so every
    // one of them must round-trip byte for byte.
    await editing.locator(".profile-kind-select").selectOption("codex");
    await editing.locator(".profile-save").click();
    await expect(editing.locator(".profile-form")).toHaveCount(0, { timeout: 20_000 });

    const stored = (await listProfiles(request, local)).profiles.find((p) => p.id === profile.id);
    expect(stored?.name, "an untouched field saves what it was seeded with").toBe(name);
    expect(stored?.invocation).toBe(invocation);
    expect(stored?.agent_kind).toBe("codex");
  });

  /**
   * A mutation's reply that arrives after the surface has been re-pointed
   * writes NOTHING — not a warning, not an error, not a row.
   *
   * The window is real and the failure is quiet: an adoption in another client
   * re-points this host while a save is in flight, and the reply that lands
   * afterwards describes a machine nobody is looking at. Gating only the
   * CATALOG on the lease would still leave the message lines writable, so the
   * section would report a refusal (or a success) about the predecessor
   * install under the successor's rows.
   *
   * The incarnation is moved by rewriting the hosts reply rather than by a
   * real adoption, which cannot be staged against a stack the rest of the
   * suite is using — what is under test is the client's lease, not the helm's
   * bookkeeping.
   */
  test("a reply that lands after the surface moved writes nothing", async ({ page, request }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, local, { name: `lease-${Date.now()}` });
    profiles.push({ host: local, id: profile.id });

    // The save is held open until this test releases it.
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => /^\/api\/hosts\/\d+\/profiles\/[^/]+$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "POST") {
          await route.continue();
          return;
        }
        await held;
        // Answers with a refusal, because a refusal is the loudest thing a
        // completion could write: it would take over the form's error line.
        await route.fulfill({
          status: 409,
          contentType: "text/plain",
          body: "a refusal about an install nobody is looking at any more",
        });
      },
    );
    // What moves is the row's INSTALL fields, which is what a retarget looks
    // like to this client: the target changes, the surface re-activates, and
    // the lease the held save is running under stops being current.
    //
    // Deliberately NOT the connection token. That is the other half of the
    // same identity, but it is also what every guarded request now hands back
    // as `expected_incarnation` — so a fabricated one makes the helm refuse
    // this section's own catalog reads, and the successor rows this test then
    // waits for would never arrive. Moving a field the helm does not compare
    // changes the identity while leaving every request answerable.
    let moved = false;
    await page.route(
      (url) => url.pathname === "/api/hosts",
      async (route: Route) => {
        const response = await route.fetch();
        const body = await response.json();
        if (moved) {
          for (const host of body.hosts) {
            if (host.id === local) host.remote_state_dir = "/moved/by/the/test";
          }
        }
        await route.fulfill({ response, json: body });
      },
    );

    const feed = await listWithStubbedFeed(page);
    // Catalog reads are counted, so every wait below is on a READ having
    // landed rather than on time passing — the successor's rows look exactly
    // like the predecessor's, so nothing about the DOM can say which
    // activation produced them.
    const reads = await watchCatalogReads(page);
    await openProfiles(page, local);
    const editing = profileRow(page, local, profile.id);
    await expect(editing).toBeVisible({ timeout: 20_000 });
    await editing.locator(".profile-edit").click();
    await editing.locator(".profile-name-input").fill(`lease-${Date.now()}-edited`);
    await editing.locator(".profile-save").click();

    // While it hangs, the host moves and the successor's catalog is read. The
    // barrier is that read: the hosts refresh has to land, re-point the
    // surface, and produce a catalog GET of its own before the save is
    // released, or the reply would be arriving under the ORIGINAL lease and
    // this test would be asserting nothing.
    const beforeMove = reads.reads(local);
    moved = true;
    feed.notify(2);
    await expect
      .poll(() => reads.reads(local), {
        timeout: 20_000,
        message: "the successor's catalog must have been read before the save is released",
      })
      .toBeGreaterThan(beforeMove);
    await expect(profileRow(page, local, profile.id)).toBeVisible({ timeout: 20_000 });

    release!();

    // Nothing the held save produces may reach the screen. Waited on through a
    // positive signal rather than a sleep: one more read is asked for and has
    // to land, and only then does absence mean anything.
    const beforeSettle = reads.reads(local);
    feed.notify(3);
    await expect
      .poll(() => reads.reads(local), { timeout: 20_000 })
      .toBeGreaterThan(beforeSettle);
    await expect(profileRow(page, local, profile.id)).toBeVisible({ timeout: 20_000 });
    await expect(
      section(page, local).locator(".profile-form-error"),
      "a refusal about the previous install must not appear under the successor's rows",
    ).toHaveCount(0);
    await expect(section(page, local).locator(".profiles-notice")).toHaveCount(0);
    await expect(section(page, local).locator(".profiles-warning")).toHaveCount(0);
    await expect(editing.locator(".profile-error")).toHaveCount(0);
  });

  /**
   * A host verb REFUSED by the operation token leaves the profiles section
   * exactly as it was; an accepted one folds it away.
   *
   * Folding is the right answer to a retarget or an adoption — the section is
   * about an install that is being replaced — but only once the verb is
   * actually out. A click the token refuses started nothing, and collapsing
   * the section for it would throw away an editor draft over an action that
   * never happened.
   */
  test("a host verb refused by the operation token leaves the profiles section alone", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, local, { name: `fold-${Date.now()}` });
    profiles.push({ host: local, id: profile.id });

    // A profile delete that never answers holds the page's operation token.
    let release: (() => void) | undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route(
      (url) => /^\/api\/hosts\/\d+\/profiles\/[^/]+$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "DELETE") {
          await route.continue();
          return;
        }
        await held;
        await route.fulfill({ status: 409, contentType: "text/plain", body: "refused" });
      },
    );
    let retries = 0;
    await page.route(
      (url) => /^\/api\/hosts\/\d+\/retry$/.test(url.pathname),
      async (route: Route) => {
        retries += 1;
        await route.continue();
      },
    );

    await listWithStubbedFeed(page);
    await openProfiles(page, local);
    await profileRow(page, local, profile.id).locator(".profile-delete").click();
    await profileRow(page, local, profile.id).locator(".profile-confirm-delete").click();

    // With the token held, a host verb on the same row is refused before it
    // starts — and the section must survive it.
    await page.locator(`[data-host-id="${local}"] .host-retry`).click({ force: true });
    await expect(
      section(page, local),
      "a verb that never started must not collapse the surface it would have replaced",
    ).toBeVisible();
    expect(retries, "and it must not have reached the helm either").toBe(0);

    release!();
    await expect(profileRow(page, local, profile.id).locator(".profile-error")).toBeVisible({
      timeout: 20_000,
    });
  });

  /**
   * A save prepared against one connection is REFUSED after the host is
   * re-pointed, and the UI says so instead of retrying.
   *
   * The precondition the helm added exists for a window this client cannot
   * close on its own — between its own check and the helm's routing — and the
   * consequence of losing that race is not a failure but a SUCCESS on the
   * wrong machine, because profile ids collide across installs by
   * construction. Here the refusal is staged (a real adoption mid-save cannot
   * be arranged against a stack the rest of the suite is using), and what is
   * pinned is the client's half: the request carries the expectation, and a
   * marked conflict closes the editor with the helm's sentence rather than
   * resubmitting.
   */
  test("a save refused by its precondition reports the conflict and does not retry", async ({
    page,
    request,
  }) => {
    const local = await localHostId(request);
    const profile = await createProfile(request, local, { name: `stale-${Date.now()}` });
    profiles.push({ host: local, id: profile.id });
    // What the helm serves is what the request has to echo — captured here so
    // the assertions below are equality rather than existence.
    const hostRow = (await listHosts(request)).find((host) => host.id === local);
    const servedFingerprint = (await listProfiles(request, local)).definitions[profile.id];
    expect(servedFingerprint, "the helm serves a fingerprint per profile").toBeTruthy();

    const sent: Record<string, unknown>[] = [];
    await page.route(
      (url) => /^\/api\/hosts\/\d+\/profiles\/[^/]+$/.test(url.pathname),
      async (route: Route) => {
        if (route.request().method() !== "POST") {
          await route.continue();
          return;
        }
        sent.push(route.request().postDataJSON());
        await route.fulfill({
          status: 409,
          contentType: "text/plain",
          body:
            "host 1 is not the connection this request was prepared against: a retarget, an " +
            "adoption, or a reconnection has replaced what answers on that host, so nothing was " +
            "changed. Re-read the host and try again [farhelm:precondition/incarnation]",
        });
      },
    );

    await listWithStubbedFeed(page);
    await openProfiles(page, local);
    const editing = profileRow(page, local, profile.id);
    await expect(editing).toBeVisible({ timeout: 20_000 });
    await editing.locator(".profile-edit").click();
    await editing.locator(".profile-name-input").fill(`stale-${Date.now()}-edited`);
    await editing.locator(".profile-save").click();

    const notice = section(page, local).locator(".profiles-notice");
    await expect(notice).toBeVisible({ timeout: 20_000 });
    await expect(notice).toContainText("prepared against");
    await expect(
      notice,
      "the marker is machine vocabulary and must not be shown to a person",
    ).not.toContainText("farhelm:precondition");
    await expect(
      editing.locator(".profile-form"),
      "a conflict closes the editor: the definition it was seeded from is not the one out there",
    ).toHaveCount(0);

    // Exactly one attempt, ever — a resubmit would be this client insisting on
    // a state the helm just told it is gone. The barrier is the conflict's OWN
    // consequence: a marked refusal invalidates the held catalog and asks for
    // an authoritative read, so once that read has landed (the rows are back)
    // any retry the client was going to make has had its chance. A sleep would
    // only prove the test waited.
    await expect(profileRow(page, local, profile.id)).toBeVisible({ timeout: 20_000 });
    expect(sent.length, "a marked conflict must never be retried automatically").toBe(1);
    // Byte for byte against what the helm SERVED, not merely "something was
    // sent": a precondition that carries the wrong connection or a fingerprint
    // this client computed itself is a precondition that refuses forever with
    // nothing actually wrong, and "is defined" cannot tell those apart.
    expect(
      sent[0].expected_incarnation,
      "the request must name the connection the hosts read reported",
    ).toBe(hostRow!.incarnation);
    expect(
      sent[0].expected_definition,
      "and the fingerprint the catalog served for this profile, echoed opaque",
    ).toBe(servedFingerprint);
  });

  /**
   * Two real clients: an edit made through ONE browser's panel reaches the
   * other's open profiles section, with no stub and no injected notification
   * anywhere.
   *
   * This is the milestone's multi-client promise for profiles, executable —
   * and it is the one test here that a stubbed feed cannot stand in for. A
   * stub proves the observer re-reads when TOLD to; only the real path proves
   * the helm tells it, which is a mutation that fails to publish away from
   * being silently broken.
   *
   * The observer therefore runs on the real socket. Everything it asserts is
   * bounded by generous timeouts rather than by a settle-then-notify handshake,
   * because on this path the test does not control when the news arrives.
   *
   * ## What this cannot prove, stated rather than implied
   *
   * The stack is shared and never truly quiet — another spec's session
   * settling into `idle` bumps the same revision counter — so a re-read that
   * follows the edit cannot be attributed to the edit with certainty by
   * watching the observer alone. What IS established is the pair that makes
   * the failure it guards against impossible to pass silently: the observer's
   * socket was open before the edit (so it cannot have re-read merely because
   * it connected), it stayed open throughout (so no reconnect handshake
   * explains the update), and the new name appeared on it without this test
   * injecting anything. A helm that published nothing would leave the observer
   * showing the old name until some unrelated churn happened to arrive — which
   * is exactly why the deterministic half of this contract is pinned by the
   * stubbed test above, where silence is guaranteed.
   */
  test("a profile edited in another browser reaches this one over the real feed", async ({
    page,
    browser,
    request,
  }) => {
    const local = await localHostId(request);
    const before = `two-client-${Date.now()}`;
    const after = `${before}-elsewhere`;
    const profile = await createProfile(request, local, { name: before });
    profiles.push({ host: local, id: profile.id });

    // The OBSERVER: a real page with a real feed, and the socket is proven up
    // BEFORE the edit is made. Without that, an observer that connected late
    // would re-read on its own handshake and look exactly like one that was
    // told — which is the difference this test exists to establish.
    const observerSockets = watchFeedSockets(page);
    await page.goto("/");
    await expect
      .poll(() => observerSockets.greeted(), {
        timeout: 20_000,
        message:
          "the observer must be SUBSCRIBED before the edit is made — a socket that has only been " +
          "requested proves nothing, and one greeted afterwards would re-read on its own handshake",
      })
      .toBeGreaterThan(0);
    await openProfiles(page, local);
    const observed = profileRow(page, local, profile.id).locator(".profile-name");
    await expect(observed).toHaveText(before, { timeout: 20_000 });

    const second = await browser.newContext({ baseURL: new URL(page.url()).origin });
    try {
      const editor = await second.newPage();
      await editor.goto("/");
      await openProfiles(editor, local);
      const editing = profileRow(editor, local, profile.id);
      await expect(editing).toBeVisible({ timeout: 20_000 });
      await editing.locator(".profile-edit").click();
      await editing.locator(".profile-name-input").fill(after);
      await editing.locator(".profile-save").click();
      await expect(editing.locator(".profile-name")).toHaveText(after, { timeout: 20_000 });

      await expect(
        observed,
        "the helm must PUBLISH a profile edit, and the other client must re-read on it — with " +
          "nothing in this test telling it to",
      ).toHaveText(after, { timeout: 30_000 });
      expect(
        observerSockets.closed(),
        "and it must have arrived on the socket that was already open, not on a reconnect that " +
          "would have re-read regardless",
      ).toBe(0);
    } finally {
      await second.close();
    }
  });
});

/**
 * Wait until the helm's own merged view reports `existence` for the session
 * with this title.
 *
 * The settle half of the stubbed-feed convention, and it is needed for exactly
 * one reason: a session's `existence` is derived per reply by the owning
 * supervisor and reaches the merged list only when the helm's session cache
 * next refreshes. A notification played before that lands tells the page to
 * re-read a view that has not moved — which passes or fails on timing rather
 * than on the rule under test.
 *
 * Deliberately NOT used for catalog reads: those are live reads from the
 * supervisor, so an awaited mutation has already settled by the time it
 * answers, and a poll there would assert nothing.
 */
async function settleExistence(
  request: Parameters<typeof listSessions>[0],
  title: string,
  existence: string,
): Promise<void> {
  await expect
    .poll(
      async () =>
        (await listSessions(request, `title=${encodeURIComponent(title)}`)).sessions[0]
          ?.source_profile?.existence,
      {
        timeout: 20_000,
        message:
          "the helm's own view must carry the derived existence before the page is told to " +
          "re-read; until its session cache refreshes there is nothing new to see",
      },
    )
    .toBe(existence);
}
