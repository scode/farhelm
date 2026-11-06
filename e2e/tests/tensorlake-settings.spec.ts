/** Settings → Tensorlake against the helm's E2E-only Tensorlake fake.
 *
 * start-stack.sh starts every helm with FARHELM_E2E_TENSORLAKE_DIR naming a
 * marked directory in its state; this spec owns that directory's config.json
 * (which key the fake accepts, which sandboxes the account has) and reads its
 * events.jsonl. Everything above the API seam is the shipped helm: the stored
 * key, the routes, the Test button's handling and the generated ssh key. The
 * fake never writes a key down; it logs only whether each call's key matched
 * the one it accepts, which is how these tests prove the helm sends the key it
 * saved without the key appearing anywhere the test reads.
 */
import { expect, test } from "./helpers/evidence";
import type { APIRequestContext } from "@playwright/test";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { currentStackSlot } from "../stack-slots";
import { openSettings, selectSettingsSection } from "./helpers/settings";
import { observeFeedReaders, readFeedReaders, stubFeed } from "./helpers/fleet";
import { drainedRoute } from "./helpers/drained-route";
import { routeGate } from "./helpers/route-gate";

type Settings = {
  api_key_set: boolean;
  create_without_asking: boolean;
  helm_tag: string;
  ssh_key:
    | {
      state: "ready";
      public_key: string;
      public_key_path: string;
      name: string;
      register_command: string;
    }
    | { state: "not_created" }
    | { state: "unavailable"; reason: string };
};

type FakeConfig = {
  accepted_key?: string;
  sandboxes?: { id: string; name?: string; status: string }[];
};

type FakeEvent = { op: string; key_accepted: boolean };

/** The key the fake accepts, and one it refuses. Neither may ever appear in a
 * helm reply; both are distinctive so a leak is unmistakable. */
const GOOD_KEY = "tl_apiKey_e2e-good-marker";
const BAD_KEY = "tl_apiKey_e2e-bad-marker";

/** Read the fake's directory only after Playwright has started the stack. */
async function fakeRoot(): Promise<string> {
  const raw = await readFile(currentStackSlot().stackInfoPath, "utf8");
  return (JSON.parse(raw) as { tensorlake_fake: string }).tensorlake_fake;
}

/** Atomically publish the fake's behavior and reset its event log. */
async function configureFake(config: FakeConfig): Promise<void> {
  const root = await fakeRoot();
  const next = path.join(root, `config.${process.pid}.next`);
  await writeFile(next, `${JSON.stringify(config)}\n`, { mode: 0o600 });
  await rename(next, path.join(root, "config.json"));
  await writeFile(path.join(root, "events.jsonl"), "", { mode: 0o600 });
}

/** Every complete event the fake recorded since the last configureFake. */
async function fakeEvents(): Promise<FakeEvent[]> {
  const raw = await readFile(
    path.join(await fakeRoot(), "events.jsonl"),
    "utf8",
  );
  return raw.split("\n").filter(Boolean).map((line) =>
    JSON.parse(line) as FakeEvent
  );
}

/** The helm's saved snapshot, also proving the reply never carries a key. */
async function saved(request: APIRequestContext): Promise<Settings> {
  const response = await request.get("/api/tensorlake");
  const body = await response.text();
  expect(response.ok(), body).toBe(true);
  expect(body).not.toContain(GOOD_KEY);
  expect(body).not.toContain(BAD_KEY);
  return JSON.parse(body) as Settings;
}

/** Put the shared stack back as the test found it: no key, the switch as it
 * was, and a fake that accepts any key and lists nothing. */
async function restore(
  request: APIRequestContext,
  original: Settings,
): Promise<void> {
  const response = await request.put("/api/tensorlake", {
    data: {
      clear_api_key: true,
      create_without_asking: original.create_without_asking,
    },
  });
  expect(response.ok(), await response.text()).toBe(true);
  await configureFake({});
}

/** Why: the key is a credential Farhelm promises never to show again, and Test
 * is how the user learns the pasted key works. A key that leaked into a reply,
 * or a Test that used the field's text instead of the saved key, would break
 * both promises without any visible symptom.
 *
 * Spec: Save stores a pasted key and empties the field; no reply carries the
 * key; Test uses the saved key, so a key the account rejects reports the
 * rejection with what to check, and an accepted one reports how many
 * sandboxes it can see, also while the field holds unsaved text the account
 * would reject. Failed discovery consumes unrelated notices without another
 * read; a successful Test after external key replacement reloads it once.
 * Reopening Settings shows that a key is saved without showing
 * it; Clear removes it and disables Test. The ssh key group creates the key on
 * request and then shows the registration command, which names this helm's
 * tag and carries the public key's text, and the section fits a phone-width
 * dialog. */
test("Tensorlake settings save a write-only key, test the saved key and clear it", async ({ page, request }) => {
  const original = await saved(request);
  expect(
    original.api_key_set,
    "fixture premise: no key it would have to restore",
  ).toBe(false);
  // Both surfaces legitimately list sandboxes. Serialize their HTTP reads
  // only in this test so each Test response has an exact vendor-call slice;
  // all requests still use the shipped routes and real fake API.
  const testEvents: FakeEvent[][] = [];
  const orphanEvents: FakeEvent[][] = [];
  let orphanRequests = 0;
  let reading = Promise.resolve();
  await observeFeedReaders(page);
  const feed = await stubFeed(page);
  feed.notifyOnConnect(1);
  // Pull the orphan consumer's own handled-notice and reload admission state
  // synchronously. HTTP responses from other readers cannot establish either.
  const orphanSnapshot = async () => {
    const snapshot = (await readFeedReaders(page)).find((snapshot) =>
      snapshot.role === "tensorlake-orphans"
    );
    expect(snapshot, "mounted orphan consumer registered its observation")
      .toBeDefined();
    return snapshot!;
  };
  await page.route(/\/api\/tensorlake\/(test|orphans)$/, async (route) => {
    const explicitTest = route.request().url().endsWith("/test");
    if (!explicitTest) orphanRequests += 1;
    const next = reading.then(async () => {
      const before = (await fakeEvents()).length;
      const response = await route.fetch();
      (explicitTest ? testEvents : orphanEvents).push(
        (await fakeEvents()).slice(before),
      );
      await route.fulfill({ response });
    });
    reading = next;
    await next;
  });
  try {
    await configureFake({
      accepted_key: GOOD_KEY,
      sandboxes: [
        { id: "sb-1", name: "farhelm-host-abcd-one-x1y2", status: "running" },
        { id: "sb-2", name: "someone-elses", status: "suspended" },
      ],
    });
    await page.goto("/");
    const dialog = await openSettings(page, "tensorlake");
    const section = dialog.getByRole("region", {
      name: "Tensorlake API key",
      exact: true,
    });
    const field = section.getByLabel("API key", { exact: true });
    const save = section.getByRole("button", { name: "Save", exact: true });
    const testKey = section.getByRole("button", { name: "Test", exact: true });
    // A previous accepted message must not satisfy the next identical Test
    // outcome before its POST has actually completed.
    const testSavedKey = async () => {
      const response = page.waitForResponse("**/api/tensorlake/test");
      await testKey.click();
      await response;
    };
    const clear = section.getByRole("button", { name: "Clear", exact: true });
    await expect(field).toBeEditable();
    await expect(section.getByText("API key", { exact: true })).toHaveCount(1);
    await expect(section.getByText("API key", { exact: true })).toBeVisible();
    await expect(field).toHaveAttribute("placeholder", "No key saved");
    await expect(field).toHaveAttribute("type", "password");
    await expect(testKey).toBeDisabled();
    await expect(clear).toBeDisabled();
    await expect(save).toBeDisabled();
    await expect(section.getByText("Not encrypted on disk", { exact: true })).toBeVisible();
    await expect(section).toContainText("Anyone who can read that folder can read it.");
    await expect.poll(async () => (await orphanSnapshot()).acted_on).toBe("1");

    // Saving the first key also loads the orphan list. Settle that separate
    // vendor read before measuring Test; neither request proves the other.
    const orphanLoad = page.waitForResponse("**/api/tensorlake/orphans");
    // A key the account rejects: saved, but Test reports the rejection.
    await field.fill(BAD_KEY);
    await save.click();
    await expect(section.getByRole("status")).toHaveText("API key saved");
    await expect(field).toHaveValue("");
    await expect(field).toHaveAttribute(
      "placeholder",
      "Key saved; paste a new one to replace it",
    );
    expect((await saved(request)).api_key_set).toBe(true);
    expect((await orphanLoad).status()).toBe(502);
    expect(orphanEvents[0]).toEqual([{
      op: "list_sandboxes",
      key_accepted: false,
    }]);
    const orphans = dialog.getByRole("region", {
      name: "Sandboxes not on this helm",
      exact: true,
    });
    await expect(orphans.getByRole("alert")).toContainText(
      "Tensorlake rejected the API key",
    );
    // Retire the initial discovery, then deliver exactly one controlled
    // notice. The consumer's handled counter orders the silence assertion;
    // its generation catches even a reload whose task has not sent HTTP yet.
    await reading;
    const beforeNotice = await orphanSnapshot();
    expect(beforeNotice).toMatchObject({
      notices: "1",
      acted_on: "1",
      failed: true,
    });
    const discoveryRequests = orphanRequests;
    feed.notify(2);
    await expect.poll(async () => (await orphanSnapshot()).acted_on).toBe("2");
    expect(await orphanSnapshot()).toMatchObject({
      notices: "2",
      generation: beforeNotice.generation,
      failed: true,
    });
    expect(orphanRequests).toBe(discoveryRequests);
    await testSavedKey();
    const rejection = section.getByRole("alert");
    await expect(rejection).toContainText(
      "Tensorlake rejected the API key (HTTP 401)",
    );
    await expect(rejection).toContainText(
      "Check that the key belongs to the Tensorlake project",
    );
    await expect(rejection).not.toContainText(BAD_KEY);
    expect(testEvents).toEqual([[{
      op: "list_sandboxes",
      key_accepted: false,
    }]]);

    // Another client replaces the saved key without changing this dialog's
    // local revision. Successful Test must release the rejected discovery
    // exactly once; neither the replacement nor failed Test does so alone.
    const replaced = await request.put("/api/tensorlake", {
      data: { api_key: GOOD_KEY },
    });
    expect(replaced.ok(), await replaced.text()).toBe(true);
    expect(await orphanSnapshot()).toMatchObject({
      generation: beforeNotice.generation,
      failed: true,
    });
    expect(orphanRequests).toBe(discoveryRequests);
    const testedDiscovery = page.waitForResponse("**/api/tensorlake/orphans");
    await testSavedKey();
    expect((await testedDiscovery).ok()).toBe(true);
    await expect(orphans.getByRole("alert")).toHaveCount(0);
    expect(await orphanSnapshot()).toMatchObject({
      generation: String(Number(beforeNotice.generation) + 1),
      failed: false,
    });
    expect(orphanRequests).toBe(discoveryRequests + 1);
    expect(orphanEvents).toEqual([
      [{ op: "list_sandboxes", key_accepted: false }],
      [{ op: "list_sandboxes", key_accepted: true }],
    ]);

    // The accepted key replaces it; surrounding spaces from a paste are trimmed.
    const recoveredDiscovery = page.waitForResponse(
      "**/api/tensorlake/orphans",
    );
    await field.fill(`  ${GOOD_KEY}  `);
    await save.click();
    await expect(section.getByRole("status")).toHaveText("API key saved");
    expect((await recoveredDiscovery).ok()).toBe(true);
    await expect(orphans.getByRole("alert")).toHaveCount(0);
    await testSavedKey();
    await expect(section.getByRole("status")).toHaveText(
      "The key works. It can see 2 sandboxes.",
    );
    expect(testEvents).toEqual([
      [{ op: "list_sandboxes", key_accepted: false }],
      [{ op: "list_sandboxes", key_accepted: true }],
      [{ op: "list_sandboxes", key_accepted: true }],
    ]);
    // Unsaved field text plays no part: with a rejected key typed but not
    // saved, Test still presents the saved, accepted one.
    await field.fill(BAD_KEY);
    await testSavedKey();
    await expect(section.getByRole("status")).toHaveText(
      "The key works. It can see 2 sandboxes.",
    );
    expect(testEvents).toEqual([
      [{ op: "list_sandboxes", key_accepted: false }],
      [{ op: "list_sandboxes", key_accepted: true }],
      [{ op: "list_sandboxes", key_accepted: true }],
      [{ op: "list_sandboxes", key_accepted: true }],
    ]);

    // Reopening reads the snapshot again: saved, never shown.
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "tensorlake");
    await expect(field).toHaveValue("");
    await expect(field).toHaveAttribute(
      "placeholder",
      "Key saved; paste a new one to replace it",
    );
    await expect(testKey).toBeEnabled();

    // Farhelm's own ssh key and its one-time registration command. The
    // stack is shared, so an earlier project on it may already have created
    // the key; a key that exists must not be created again, and one that does
    // not must be created only by the explicit button.
    const ssh = dialog.getByRole("region", {
      name: "Farhelm's SSH key",
      exact: true,
    });
    const before = await saved(request);
    if (before.ssh_key.state === "not_created") {
      await expect(ssh.locator(".tensorlake-register-command")).toHaveCount(0);
      await ssh.getByRole("button", { name: "Create SSH key", exact: true })
        .click();
    } else {
      expect(before.ssh_key.state).toBe("ready");
      await expect(
        ssh.getByRole("button", { name: "Create SSH key", exact: true }),
      ).toHaveCount(0);
    }
    await expect(ssh.locator(".tensorlake-register-command")).toBeVisible();
    const snapshot = await saved(request);
    if (snapshot.ssh_key.state !== "ready") {
      throw new Error(`no ssh key: ${JSON.stringify(snapshot.ssh_key)}`);
    }
    expect(snapshot.helm_tag).toMatch(/^[a-z0-9]{4}$/);
    await expect(ssh.locator(".tensorlake-register-command")).toHaveText(
      snapshot.ssh_key.register_command,
    );
    expect(snapshot.ssh_key.public_key).toMatch(
      new RegExp(`^ssh-ed25519 \\S+ farhelm-${snapshot.helm_tag}$`),
    );
    expect(snapshot.ssh_key.register_command).toBe(
      `tl sbx ssh keys add --name farhelm-${snapshot.helm_tag} '${snapshot.ssh_key.public_key}'`,
    );
    await ssh.getByRole("button", {
      name: "Copy the registration command",
      exact: true,
    }).click();
    await expect(ssh.getByRole("status")).toHaveText("Command copied");
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 800 });
      const panel = await selectSettingsSection(page, "tensorlake");
      await ssh.locator(".tensorlake-register-command")
        .scrollIntoViewIfNeeded();
      const geometry = await panel.evaluate((el) => ({
        width: el.clientWidth,
        content: el.scrollWidth,
      }));
      expect(geometry.content, `the section fits a ${width}px dialog`)
        .toBeLessThanOrEqual(geometry.width);
    }

    await clear.click();
    await expect(section.getByRole("status")).toHaveText("API key cleared");
    await expect(field).toHaveAttribute("placeholder", "No key saved");
    await expect(testKey).toBeDisabled();
    expect((await saved(request)).api_key_set).toBe(false);
    await expect(
      dialog.getByRole("region", {
        name: "Sandboxes not on this helm",
        exact: true,
      }),
    ).toHaveCount(0);
  } finally {
    // Finish the observing handlers before restore resets their event log.
    await page.unrouteAll({ behavior: "wait" });
    await restore(request, original);
  }
});

/** Empty discovery must visibly finish, and a later failure must not keep
 * claiming that no sandboxes exist. Hold each reply until the pending status
 * and disabled Refresh are observed; no vendor latency is needed to expose
 * this boundary. Both palettes retain a capture of the successful empty state. */
test(
  "sandbox refresh shows pending, empty, failure and recovery feedback",
  async ({ page, request }, info) => {
    const original = await saved(request);
    expect(
      original.api_key_set,
      "fixture can restore the original empty account",
    ).toBe(false);
    await configureFake({});
    const saving = await request.put("/api/tensorlake", {
      data: { api_key: GOOD_KEY },
    });
    expect(saving.ok(), await saving.text()).toBe(true);
    const stamp = saving.headers()["x-farhelm-build"];
    expect(stamp, "routed discovery carries the serving helm's build stamp")
      .toBeTruthy();
    await stubFeed(page);
    let gate = routeGate();
    let status = 200;
    let body: unknown = [];
    let reads = 0;
    const stop = await drainedRoute(
      page,
      "**/api/tensorlake/orphans",
      async (route) => {
        const held = gate;
        const reply = { status, json: body };
        reads++;
        await held.wait();
        await route.fulfill({
          ...reply,
          headers: { "x-farhelm-build": stamp },
        });
      },
    );
    try {
      await page.goto("/");
      const dialog = await openSettings(page, "tensorlake");
      const list = dialog.getByRole("region", {
        name: "Sandboxes not on this helm",
        exact: true,
      });
      const refresh = list.getByRole("button", {
        name: "Refresh",
        exact: true,
      });
      const feedback = list.locator(".tensorlake-orphans-status");
      await expect.poll(() => reads, {
        message: "initial discovery is held before its reply",
      }).toBe(1);
      await expect(
        list.getByRole("heading", {
          name: "Sandboxes not on this helm",
          exact: true,
        }),
      ).toBeVisible();
      await expect(feedback).toHaveText("Checking…");
      await expect(refresh).toBeDisabled();
      await expect(list).not.toContainText("None found");
      gate.release();
      await expect(feedback).toHaveText(/^None found · checked \d{2}:\d{2}$/);
      await expect(refresh).toBeEnabled();
      await expect(list.locator(".tensorlake-orphan")).toHaveCount(0);
      for (const palette of ["dark", "light"] as const) {
        await page.emulateMedia({ colorScheme: palette });
        await expect(page.locator("html")).toHaveAttribute(
          "data-appearance",
          palette,
        );
        await info.attach(`sandbox-refresh-empty-${palette}`, {
          body: await list.screenshot(),
          contentType: "image/png",
        });
      }

      gate = routeGate();
      status = 503;
      body = { error: "sandbox discovery fixture unavailable" };
      await refresh.click();
      await expect.poll(() => reads).toBe(2);
      await expect(feedback).toHaveText("Checking…");
      await expect(refresh).toBeDisabled();
      await expect(list).not.toContainText("None found");
      gate.release();
      await expect(list.getByRole("alert")).toContainText(
        "sandbox discovery fixture unavailable",
      );
      await expect(list).not.toContainText("None found");
      await expect(refresh).toBeEnabled();

      gate = routeGate();
      status = 200;
      body = [];
      await refresh.click();
      await expect.poll(() => reads).toBe(3);
      await expect(feedback).toHaveText("Checking…");
      await expect(refresh).toBeDisabled();
      await expect(list.getByRole("alert")).toContainText(
        "sandbox discovery fixture unavailable",
      );
      gate.release();
      await expect(feedback).toHaveText(/^None found · checked \d{2}:\d{2}$/);
      await expect(list.getByRole("alert")).toHaveCount(0);
      await expect(refresh).toBeEnabled();
    } finally {
      gate.release();
      await stop();
      await restore(request, original);
    }
  },
);

/** A superseded discovery must not retire a newer check or resurrect rows
 * after the saved key is cleared. These are response-order boundaries:
 * reserved-prefix rows exist only in the routed replies, never in the helm's
 * host table, so this test cannot dial Tensorlake's SSH gateway. */
test("sandbox discovery ignores replies superseded by key replacement or clear", async ({ page, request }) => {
  const original = await saved(request);
  expect(original.api_key_set, "fixture can restore the original empty account")
    .toBe(false);
  await configureFake({});
  const saving = await request.put("/api/tensorlake", {
    data: { api_key: GOOD_KEY },
  });
  expect(saving.ok(), await saving.text()).toBe(true);
  const stamp = saving.headers()["x-farhelm-build"];
  expect(stamp, "routed discovery carries the serving helm's build stamp")
    .toBeTruthy();
  await stubFeed(page);
  await observeFeedReaders(page);
  const staleRow = {
    id: "superseded-orphan",
    name: "farhelm-host-zzzz-stale-abcd",
    state: "running",
    from_another_helm: true,
    created_at: null,
    cleanup: null,
  };
  const replies = [[], [staleRow], [], [staleRow]].map((json) => ({
    json,
    gate: routeGate(),
  }));
  let reads = 0;
  const stop = await drainedRoute(
    page,
    "**/api/tensorlake/orphans",
    async (route) => {
      const reply = replies[reads++];
      expect(reply, "only the four named discovery boundaries are admitted")
        .toBeDefined();
      await reply.gate.wait();
      await route.fulfill({
        json: reply.json,
        headers: { "x-farhelm-build": stamp },
      });
    },
  );
  try {
    await page.goto("/");
    const dialog = await openSettings(page, "tensorlake");
    const list = dialog.getByRole("region", {
      name: "Sandboxes not on this helm",
      exact: true,
    });
    const account = dialog.getByRole("region", {
      name: "Tensorlake API key",
      exact: true,
    });
    const refresh = list.getByRole("button", { name: "Refresh", exact: true });
    const feedback = list.locator(".tensorlake-orphans-status");
    await expect.poll(() => reads).toBe(1);
    replies[0].gate.release();
    await expect(feedback).toHaveText(/^None found · checked \d{2}:\d{2}$/);
    await refresh.click();
    await expect.poll(() => reads).toBe(2);
    await expect(feedback).toHaveText("Checking…");
    const before = (await readFeedReaders(page)).find((snapshot) =>
      snapshot.role === "tensorlake-orphans"
    );
    expect(before?.generation, "the mounted consumer exposes its admitted read")
      .toBeDefined();

    await account.getByLabel("API key", { exact: true }).fill(
      `${GOOD_KEY}-replacement`,
    );
    await account.getByRole("button", { name: "Save", exact: true }).click();
    await expect(account.getByRole("status")).toHaveText("API key saved");
    await expect.poll(() => reads).toBe(3);
    expect(
      (await readFeedReaders(page)).find((snapshot) =>
        snapshot.role === "tensorlake-orphans"
      )?.generation,
    )
      .toBe(String(Number(before!.generation) + 1));
    const stale = page.waitForResponse("**/api/tensorlake/orphans");
    replies[1].gate.release();
    await (await stale).finished();
    // Observe the next render after the old body arrives while the newer
    // reply remains held. Network completion alone is not the pending UI.
    await page.evaluate(() =>
      new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    );
    await expect(feedback).toHaveText("Checking…");
    await expect(refresh).toBeDisabled();
    await expect(list.locator(".tensorlake-orphan")).toHaveCount(0);
    await expect(list).not.toContainText("None found");
    replies[2].gate.release();
    await expect(feedback).toHaveText(/^None found · checked \d{2}:\d{2}$/);
    await expect(list.locator(".tensorlake-orphan")).toHaveCount(0);

    await refresh.click();
    await expect.poll(() => reads).toBe(4);
    await expect(feedback).toHaveText("Checking…");
    await account.getByRole("button", { name: "Clear", exact: true }).click();
    await expect(account.getByRole("status")).toHaveText("API key cleared");
    await expect(list).toHaveCount(0);
    const cleared = page.waitForResponse("**/api/tensorlake/orphans");
    replies[3].gate.release();
    await (await cleared).finished();
    await page.evaluate(() =>
      new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    );
    await expect(list).toHaveCount(0);
  } finally {
    for (const reply of replies) reply.gate.release();
    await stop();
    await restore(request, original);
  }
});

/** Why: the switch authorizes creating billed sandboxes without a question,
 * so it must show only a choice the helm has actually stored, and survive a
 * reload.
 *
 * Spec: turning it on stores it and a reopened Settings shows it on; a
 * refused save leaves the switch showing the stored choice with the reason
 * beside it; turning it off stores that.
 *
 * The switch is driven with click(), not check()/uncheck(): its handler puts
 * the native box back to the stored choice at once and shows the new one only
 * after the helm's reply, and check() fails when its own read of the box beats
 * that reply. The helm's stored value is the oracle, then the box. */
test("Tensorlake's create-without-asking switch shows only the stored choice", async ({ page, request }) => {
  const original = await saved(request);
  expect(
    original.create_without_asking,
    "fixture premise: the switch starts off",
  ).toBe(false);
  try {
    await page.goto("/");
    const dialog = await openSettings(page, "tensorlake");
    const toggle = dialog.getByRole("checkbox", {
      name: "Create Tensorlake sandboxes without asking",
      exact: true,
    });
    await expect(toggle).toBeEnabled();
    await expect(toggle).not.toBeChecked();
    await toggle.click();
    await expect.poll(async () => (await saved(request)).create_without_asking)
      .toBe(true);
    await expect(toggle).toBeChecked();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await openSettings(page, "tensorlake");
    await expect(toggle).toBeChecked();

    // A refused save: the switch returns to the stored choice.
    await page.route("**/api/tensorlake", async (route) => {
      if (route.request().method() === "PUT") {
        await route.fulfill({
          status: 500,
          body: "could not save Tensorlake settings",
        });
      } else await route.continue();
    });
    await toggle.click();
    await expect(dialog.getByRole("alert")).toHaveText(
      "could not save Tensorlake settings",
    );
    await expect(toggle).toBeChecked();
    expect((await saved(request)).create_without_asking).toBe(true);
    await page.unroute("**/api/tensorlake");

    await toggle.click();
    await expect.poll(async () => (await saved(request)).create_without_asking)
      .toBe(false);
    await expect(toggle).not.toBeChecked();
  } finally {
    await restore(request, original);
  }
});

/** Why: a section that seeded an editable default after a failed load could
 * show "no key saved" for an account that has one.
 *
 * Spec: a failed load shows the error with every control locked; Retry loads
 * the helm's snapshot and unlocks them. */
/**
 * Why: Tensorlake support is experimental and bills real money, and the maintainer wants every user to meet that
 * warning before they can use it. Everyone has to open this section to save an API key, so the warning heads it.
 *
 * Spec: the section shows the maintainer's exact warning text, ahead of the API key, and still shows it when the
 * section's settings fail to load.
 */
test("Tensorlake settings open with the experimental warning", async ({ page }) => {
  const warning =
    "EXPERIMENTAL: Tensorlake support is currently early and experimental. User experience is not yet " +
    "reasonable. Expect rough edges. Things will change randomly between releases. Try at your own risk. You " +
    "are responsible for making sure sandboxes are not running and billing you when you don't want them to.";
  await page.route("**/api/tensorlake", async (route) => {
    if (route.request().method() === "GET") {
      await route.fulfill({
        status: 500,
        body: "could not read Tensorlake settings",
      });
    } else await route.continue();
  });
  await page.goto("/");
  const dialog = await openSettings(page, "tensorlake");
  const note = dialog.getByRole("note", {
    name: "Tensorlake support is experimental",
    exact: true,
  });
  await expect(note).toHaveText(warning);
  const apiKey = dialog.getByRole("region", {
    name: "Tensorlake API key",
    exact: true,
  });
  // Premise of the order check: the load really failed, and both elements are in the page.
  await expect(apiKey.getByRole("alert")).toHaveText(
    "could not read Tensorlake settings",
  );
  const precedes = await note.evaluate(
    (warningElement, keyElement) =>
      Boolean(
        warningElement.compareDocumentPosition(keyElement as Node) &
          Node.DOCUMENT_POSITION_FOLLOWING,
      ),
    await apiKey.elementHandle(),
  );
  expect(precedes, "the warning comes before the API key").toBe(true);
});

test("Tensorlake settings load failure locks the section until a retry succeeds", async ({ page }) => {
  let reads = 0;
  await page.route("**/api/tensorlake", async (route) => {
    if (route.request().method() === "GET" && ++reads === 1) {
      await route.fulfill({
        status: 500,
        body: "could not read Tensorlake settings",
      });
    } else await route.continue();
  });
  await page.goto("/");
  const dialog = await openSettings(page, "tensorlake");
  const section = dialog.getByRole("region", {
    name: "Tensorlake API key",
    exact: true,
  });
  await expect(section.getByRole("alert")).toHaveText(
    "could not read Tensorlake settings",
  );
  await expect(section.getByLabel("API key", { exact: true })).not
    .toBeEditable();
  await expect(
    dialog.getByRole("checkbox", {
      name: "Create Tensorlake sandboxes without asking",
      exact: true,
    }),
  )
    .toBeDisabled();
  await section.getByRole("button", { name: "Retry loading", exact: true })
    .click();
  await expect(section.getByLabel("API key", { exact: true })).toBeEditable();
  await expect(
    dialog.getByRole("checkbox", {
      name: "Create Tensorlake sandboxes without asking",
      exact: true,
    }),
  )
    .toBeEnabled();
});
