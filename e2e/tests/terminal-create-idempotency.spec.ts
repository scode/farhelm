// Server-enforced create idempotency, at the level only the browser can
// exercise: the UI's own key lifecycle (PLAN_M3.md item 6, "the UI
// generates one key per intended create and reuses it across retries of
// that intent"). The supervisor's side of the contract — replay, conflict,
// crash reconciliation, the gone-error — is pinned in the Rust e2e suite,
// which can restart supervisors and inject crashes; what only this suite
// can show is that the retry a USER performs actually carries the same key
// the first attempt did.

import { expect, test } from "./helpers/evidence";
import { type Locator } from "@playwright/test";
import { fillCreateForm } from "./helpers/term";
import { waitForSessionRevealed } from "./helpers/terminal-readiness";
import { cleanupOwnedSteps } from "./helpers/cleanup";
import { listHosts, localHostId, setLocalYoloWithoutAsking } from "./helpers/fleet";
import {
  cleanUpSessionsTitled,
  FAKE_AGENT_INVOCATION,
  installTerminalSuiteHooks,
  rowByTitle,
} from "./helpers/terminal-suite";

/**
 * Read a visible fixture row's session id for a readiness oracle.
 *
 * The bounded locator observation ties the id to the row this test chose;
 * cleanup's best-effort title lookup is deliberately not a setup oracle.
 * Give row publication its attachment allowance rather than the shorter
 * default assertion timeout; attachment readiness is observed afterward.
 */
async function sessionIdFor(row: Locator, timeout = 20_000): Promise<string> {
  await expect(row).toHaveAttribute("data-session-id", /.+/, { timeout });
  return (await row.getAttribute("data-session-id"))!;
}

installTerminalSuiteHooks();

for (const harness of ["goose", "codex"]) {
  /** Null permissions are a real template reset, not a visible permission click.
   * Drop an admitted reply to prove that the mint re-read and the next submit
   * retain one identity. Goose also has to carry its consent through that retry.
   */
  test(`a null-permission ${harness} template retains consent and one session after a lost reply`, async ({ page, request }) => {
    const title = `reset-retry-${harness}-${Date.now()}`;
    const local = await localHostId(request);
    const previousYolo = (await listHosts(request)).find(host => host.id === local)!.yolo_without_asking;
    const bodies: { intent_key: string; confirm_yolo?: boolean; launch: { harness: string; permissions: string } }[] = [];
    const statuses: number[] = [];
    let dropped = false;
    let bodyFailed = false;
    try {
      await setLocalYoloWithoutAsking(request, false);
      expect((await request.put(`/api/templates/${title}`, { data: {
        kind: "agent", agent: harness, permissions: null, destination: { folder: "/tmp" },
      } })).ok()).toBe(true);
      const stored = await request.get("/api/templates");
      expect(stored.ok()).toBe(true);
      expect((await stored.json()).templates.find((template: { name: string }) => template.name === title).fields)
        .toMatchObject({ agent: harness, permissions: null });
      await page.route("**/api/sessions", async route => {
        if (route.request().method() !== "POST") return route.continue();
        bodies.push(route.request().postDataJSON());
        const response = await route.fetch();
        statuses.push(response.status());
        if (response.ok() && !dropped) {
          dropped = true;
          await route.abort();
        } else await route.fulfill({ response });
      });
      await page.goto("/");
      await page.locator(".new-session-button").click();
      const form = page.locator(".create-session-form");
      await expect(form).toHaveAttribute("data-initial-templates-ready", "true");
      const agent = form.locator(".launch-composer-harness-choice").getByRole("button", { name: harness === "goose" ? "Goose" : "Codex", exact: true });
      await agent.click();
      const permissions = form.locator(".launch-composer-permissions-choice");
      const opposite = permissions.getByRole("button", { name: harness === "goose" ? "approve" : "yolo", exact: true });
      await opposite.click();
      await expect(opposite, "premise: only the template reset can change this explicit choice").toHaveAttribute("aria-pressed", "true");
      const search = form.getByRole("combobox", { name: "search folders, harnesses, models, and efforts" });
      await search.fill(`tl:${title}`);
      await search.press("Enter");
      await expect(search).toHaveValue("");
      await expect(agent).toHaveAttribute("aria-pressed", "true");
      await expect(opposite).toHaveAttribute("aria-pressed", "false");
      await expect(permissions.getByRole("button", { name: harness === "goose" ? "yolo" : "guarded", exact: true }))
        .toHaveAttribute("aria-pressed", "true");
      await form.getByLabel("name (optional)").fill(title);
      await form.locator(".create-session-submit").click();
      if (harness === "goose") {
        const confirmation = form.locator(".yolo-confirmation");
        await expect(confirmation).toBeVisible();
        expect(statuses).toEqual([409]);
        await confirmation.locator(".yolo-confirm").click();
      }
      await expect(form.locator(".create-session-error")).toBeVisible();
      expect(dropped, "premise: the lost response followed an admitted create").toBe(true);
      expect(statuses).toEqual(harness === "goose" ? [409, 200] : [200]);
      await form.locator(".create-session-submit").click();
      const id = await sessionIdFor(rowByTitle(page, title));
      await waitForSessionRevealed(page, id);
      expect(bodies).toHaveLength(harness === "goose" ? 3 : 2);
      expect(bodies[0].intent_key).toBeTruthy();
      expect(new Set(bodies.map(body => body.intent_key)).size).toBe(1);
      for (const body of bodies) expect(body.launch).toMatchObject({ harness, permissions: harness === "goose" ? "yolo" : "guarded" });
      expect(bodies.map(body => body.confirm_yolo ?? false)).toEqual(harness === "goose" ? [false, true, true] : [false, false]);
      expect((await (await request.get("/api/sessions")).json()).sessions.filter((session: any) => session.title === title)).toHaveLength(1);
    } catch (error) {
      bodyFailed = true;
      throw error;
    } finally {
      await cleanupOwnedSteps(bodyFailed, [
        // Admission can outlive a failing assertion. Settle it before looking
        // up sessions, so a late successful create cannot escape deletion.
        ["pending creates", () => page.unrouteAll({ behavior: "wait" })],
        ["sessions", () => cleanUpSessionsTitled(request, title)],
        ["template", async () => { expect((await request.delete(`/api/templates/${title}`)).ok()).toBe(true); }],
        ["host consent setting", () => setLocalYoloWithoutAsking(request, previousYolo)],
      ]);
    }
  });
}

/**
 * Enter on an already-selected choice is a retry, not a new launch intent.
 * Losing a successful reply distinguishes server deduplication from merely
 * sending two requests that happened to fail before creating anything.
 * Codex's explicit Guarded choice stays selected across both attempts, so
 * permission normalization cannot turn the retry into an edited launch.
 */
test("Enter on an unchanged permission retries a lost reply with one key and one session", async ({ page, request }) => {
  const title = `intent-choice-retry-${Date.now()}`;
  const keys: string[] = [];
  let firstStatus = 0;
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    keys.push(route.request().postDataJSON().intent_key);
    if (keys.length === 1) {
      firstStatus = (await route.fetch()).status();
      await route.abort();
    } else await route.continue();
  });
  try {
    await page.goto("/");
    await page.locator(".new-session-button").click();
    const form = page.locator(".create-session-form");
    await expect(form).toBeVisible();
    await form.getByRole("button", { name: "Codex", exact: true }).click();
    await form.getByLabel("folder", { exact: true }).fill("/tmp");
    await form.getByLabel("name (optional)").fill(title);
    const choice = form.locator(".launch-composer-permissions-choice").getByRole("button", { name: "guarded", exact: true });
    await choice.click();
    await expect(choice).toHaveAttribute("aria-pressed", "true");
    await form.locator(".create-session-submit").click();
    await expect(form.locator(".create-session-error")).toBeVisible();
    expect(firstStatus, "the dropped reply must follow an actual successful create").toBe(200);
    await expect(choice).toBeEnabled();
    await choice.focus();
    await expect(choice).toBeFocused();
    await choice.press("Enter");
    const id = await sessionIdFor(rowByTitle(page, title));
    await waitForSessionRevealed(page, id);
    expect(keys).toHaveLength(2);
    expect(keys[0]).toBeTruthy();
    expect(keys[1]).toBe(keys[0]);
    const listing = await (await request.get("/api/sessions")).json();
    expect(listing.sessions.filter((session: any) => session.title === title)).toHaveLength(1);
  } finally {
    await cleanUpSessionsTitled(request, title);
  }
});

test("a create whose reply is lost is retried with the same key and yields one session", async ({
  page,
  request,
}) => {
  const title = `intent-retry-${Date.now()}`;
  const keys: (string | undefined)[] = [];
  const targets: (number | undefined)[] = [];
  let firstStatus = 0;
  // The first POST really reaches the server — `route.fetch()` performs
  // it — and only its RESPONSE is thrown away, which is precisely the
  // ambiguous failure this feature exists for: a session now exists that
  // the browser has no way of knowing about. Aborting instead would test
  // only that the key is reused, not that the server dedups against a
  // session the client never heard of.
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    const body = JSON.parse(route.request().postData() ?? "{}");
    keys.push(body.intent_key);
    targets.push(body.host);
    if (keys.length === 1) {
      const response = await route.fetch();
      firstStatus = response.status();
      await route.abort();
      return;
    }
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
    // The dropped response surfaces as an ordinary create error, leaving
    // the form usable — the state a user retries from.
    await expect(form.locator(".create-session-error")).toBeVisible();
    // The first attempt SUCCEEDED on the server; without that, the retry
    // below would merely be creating the session for the first time and
    // would prove nothing about deduplication.
    expect(firstStatus).toBe(200);

    await form.locator('button[type="submit"]').click();
    const id = await sessionIdFor(rowByTitle(page, title));
    await waitForSessionRevealed(page, id);
    await expect(page.locator(".session-details .title")).toHaveText(title);

    expect(keys).toHaveLength(2);
    expect(keys[0]).toBeTruthy();
    expect(keys[1]).toBe(keys[0]);
    // The retry must name the SAME HOST as well as the same key, and that
    // is not decoration: the helm scopes idempotency keys per host, so a
    // retry that carried the key to a different machine would not dedup
    // there at all — it would launch a second real agent, which is the one
    // outcome this whole feature exists to prevent.
    expect(targets[0]).toBeTruthy();
    expect(targets[1]).toBe(targets[0]);
    // The point of all of it: one intended create, one session, even
    // though the server genuinely handled two requests.
    const listing = await (await request.get("/api/sessions")).json();
    expect(listing.sessions.filter((s: any) => s.title === title)).toHaveLength(1);
  } finally {
    await cleanUpSessionsTitled(request, title);
  }
});

// The other edge of the same rule, and the reason the key is minted at
// first submit rather than when the form opens: editing a field makes the
// next submit a DIFFERENT intent, so it must carry a different key.
//
// What reusing the old key would cost depends on how far the first attempt
// got. Here it failed on a precondition, which the supervisor records as
// that intent's outcome — so a resubmission under the same key would
// REPLAY "working directory does not exist" no matter what the user fixed,
// leaving the form permanently unable to succeed. Where the first attempt
// got further, the same reuse is refused as a conflict instead. Both are
// dead ends; minting a new key is what makes "fix it and try again" work.
//
// Each field gets its own pass, because the key is cleared by each input's
// own handler and a missed one would only show up in whichever field the
// user happened to edit.
//
// Each edit is to another value that ALSO fails, so the assertion is about
// the key alone rather than about whether the corrected request happens to
// succeed — and so both attempts land in the same observable state.
for (const field of [
  { name: "working directory", label: "folder", edit: "/nonexistent/also/not/here" },
  { name: "agent command", label: "agent command", edit: "also-not-an-agent" },
]) {
  test(`editing the ${field.name} after a failed create mints a new intent key`, async ({
    page,
    request,
  }) => {
    const title = `intent-new-${field.name}-${Date.now()}`;
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
      // Both fields start wrong so that fixing EITHER one leaves a
      // request that still differs from the first attempt.
      const form = await fillCreateForm(page, {
        cwd: "/nonexistent/definitely/not/here",
        invocation: "definitely-not-an-agent",
        title,
      });
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      await form.getByLabel(field.label, { exact: true }).fill(field.edit);
      await form.locator('button[type="submit"]').click();
      await expect(form.locator(".create-session-error")).toBeVisible();

      expect(keys).toHaveLength(2);
      expect(keys[0]).toBeTruthy();
      expect(keys[1]).toBeTruthy();
      expect(keys[1]).not.toBe(keys[0]);
    } finally {
      await cleanUpSessionsTitled(request, title);
    }
  });
}

// The title is prose rather than something that gets executed, but it is
// still part of what makes a create the create it is (the server
// fingerprints it), so editing it starts a new intent exactly like the
// other two fields. Kept separate from the loop above because a bad title
// cannot fail a create. Keep the invalid folder unchanged so both attempts
// fail identically: fixing the folder would renew the key independently and
// hide a title-only regression.
test("editing the title after a failed create mints a new intent key", async ({
  page,
  request,
}) => {
  const title = `intent-title-${Date.now()}`;
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
    const form = await fillCreateForm(page, {
      cwd: "/nonexistent/definitely/not/here",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();
    await expect(form.locator(".create-session-error")).toBeVisible();

    await form.getByLabel("name (optional)").fill(`${title}-renamed`);
    const [retry] = await Promise.all([
      page.waitForResponse((response) =>
        response.request().method() === "POST" && new URL(response.url()).pathname === "/api/sessions"
      ),
      form.locator('button[type="submit"]').click(),
    ]);
    expect(retry.ok(), "the unchanged invalid folder must still refuse the title-only retry").toBe(false);
    await expect(form.locator(".create-session-error")).toBeVisible();

    expect(keys).toHaveLength(2);
    expect(keys[0]).toBeTruthy();
    expect(keys[1]).toBeTruthy();
    expect(keys[1]).not.toBe(keys[0]);
  } finally {
    await cleanUpSessionsTitled(request, title);
    await cleanUpSessionsTitled(request, `${title}-renamed`);
  }
});

// The form is inert for the whole submission — inputs included, not just
// the submit button — which is what makes the key lifecycle a rule rather
// than a race: key generation runs in the renderer and is asynchronous, so
// a keystroke landing between minting a key and sending it would otherwise
// publish a key belonging to values the user has already changed.
test("the create form's inputs are disabled while a create is in flight", async ({
  page,
  request,
}) => {
  const title = `intent-inert-${Date.now()}`;
  // Keep the POST pending for the disabled-input assertions. If they fail,
  // abort the held request so cleanup cannot race a late session creation.
  let release!: (proceed: boolean) => void;
  const held = new Promise<boolean>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/sessions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    if (await held) await route.continue();
    else await route.abort();
  });

  try {
    await page.goto("/");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await Promise.all([
      page.waitForRequest(
        (req) => req.method() === "POST" && new URL(req.url()).pathname === "/api/sessions",
        { timeout: 20_000 },
      ),
      form.locator('button[type="submit"]').click(),
    ]);
    for (const label of ["name (optional)", "folder", "agent command"]) {
      await expect(form.getByLabel(label, { exact: true })).toBeDisabled();
    }
    release(true);
    const id = await sessionIdFor(rowByTitle(page, title));
    await waitForSessionRevealed(page, id);
  } finally {
    release(false);
    await cleanUpSessionsTitled(request, title);
  }
});

// The two id generators diverge exactly here, and this pins both halves of
// that divergence in one run, because they are a single decision: with no
// CSPRNG available, an intent key falls back to a weak generator and a
// session LEASE refuses outright (see `mint_intent_key` and `mint_lease` in
// farhelm-ui).
//
// The asymmetry is not fussiness. An intent key only has to be unique among
// one user's own creates, and refusing every create on such a browser would
// be strictly worse for a value that authorizes nothing. A lease is grouped
// by BARE EQUALITY across clients, so a colliding one silently fuses two
// clients into one attachment and bypasses the visible takeover SPEC.md's
// one-attached-client rule is built on — a wrong answer that looks like a
// working one, which is the case for failing closed.
//
// This test USED to wait for the terminal to come up after the create, and
// that is precisely what no longer happens: the wait was replaced with the
// lease refusal, which is the behavior change being pinned.
test("with no CSPRNG, a create still carries a key while the session view refuses to attach", async ({
  page,
  request,
}) => {
  const title = `intent-fallback-${Date.now()}`;
  const keys: (string | undefined)[] = [];
  // Defined away on the PROTOTYPE, where it actually lives: deleting an
  // own property of `crypto` would silently do nothing and the test would
  // pass while exercising the ordinary path.
  await page.addInitScript(() => {
    Object.defineProperty(Crypto.prototype, "randomUUID", {
      value: undefined,
      configurable: true,
    });
  });
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
    expect(
      await page.evaluate(() => typeof (globalThis.crypto as any)?.randomUUID),
    ).not.toBe("function");
    const form = await fillCreateForm(page, {
      cwd: "/tmp",
      invocation: FAKE_AGENT_INVOCATION,
      title,
    });
    await form.locator('button[type="submit"]').click();

    // The create went through on the weak key, and landed in the session
    // view — the titlebar is the proof, since it renders from the created
    // session regardless of whether any terminal attaches.
    await expect(page.locator(".session-details .title")).toHaveText(title, {
      timeout: 15_000,
    });
    expect(keys).toHaveLength(1);
    expect(keys[0]).toBeTruthy();

    // ...and the terminal deliberately did NOT: the refusal is visible,
    // names entropy as the reason, and — the part that actually matters —
    // no socket was opened. A view that degraded to a weak or empty lease
    // would attach here, and its terminals would then take each other
    // over the moment a second one existed.
    await expect(page.locator(".lease-error")).toBeVisible({ timeout: 15_000 });
    await expect(page.locator(".lease-error")).toContainText("high-entropy");
    expect(
      await page.evaluate(() => Object.keys((window as any).__farhelmIslands ?? {}).length),
    ).toBe(0);
    expect(await page.evaluate(() => (window as any).__farhelmTermReady)).toBeUndefined();
  } finally {
    await cleanUpSessionsTitled(request, title);
  }
});
