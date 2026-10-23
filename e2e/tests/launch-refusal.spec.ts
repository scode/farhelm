/**
 * How the session launcher presents a refused launch (SPEC.md, the launcher's
 * working-directory field and its precondition failures): a bordered,
 * announced "Not launched." alert beside Launch for every refusal, and the
 * folder field marked while a folder refusal still describes the form's
 * destination.
 *
 * Why: the refusal used to be one plain red line under the launch row, with
 * nothing pointing at the folder, and the maintainer launched into a missing
 * folder without noticing the message (2026-10-22).
 */
import { expect, test } from "./helpers/evidence";
import { type Page } from "@playwright/test";
import { chooseLauncherHost } from "./helpers/host-picker";
import { localHostId } from "./helpers/fleet";
import { fillCreateForm } from "./helpers/term";

/** Open New and return its form, waiting until it is on screen. */
async function openNew(page: Page) {
  await page.goto("/");
  await page.locator(".new-session-button").click();
  const form = page.locator(".create-session-form");
  await expect(form).toBeVisible();
  return form;
}

/**
 * Spec: a create into a folder that does not exist is refused with the
 * supervisor's own words inside an alert that starts "Not launched."; the
 * folder field is marked invalid and says the folder does not exist on the
 * host; the marking goes away when the folder changes, comes back for the
 * same folder on the same host, and goes away when the host changes. Nothing
 * is created.
 */
test("a missing folder is refused as an alert and marks the folder until the destination changes", async ({ page, request }) => {
  const local = await localHostId(request);
  // The remote row only matters for the host switch, but it must be live
  // there, or the launcher would not offer it as a destination.
  let remote: number | undefined;
  await expect.poll(async () => {
    const listing = await (await request.get("/api/hosts")).json();
    const row = listing.hosts.find((host: { id: number }) => host.id !== local);
    remote = row?.id;
    return row?.state.phase;
  }, { message: "the owned browser stack must supply a connected remote host" }).toBe("connected");
  const missing = `/nonexistent/launch-refusal-${Date.now()}`;
  const title = `launch-refusal-${Date.now()}`;
  await page.goto("/");
  const form = await fillCreateForm(page, { cwd: missing, invocation: "true", title });
  const hostPicker = form.locator(".create-session-host");
  await expect(hostPicker, "the refusal must come from the local host").toHaveAttribute("data-host-id", String(local));
  const folder = form.getByLabel("folder", { exact: true });
  await expect(folder, "nothing is marked before an attempt").not.toHaveAttribute("aria-invalid", "true");

  await form.locator(".create-session-submit").click();

  const alert = form.locator(".launch-composer-refusal-alert");
  await expect(alert).toHaveAttribute("role", "alert");
  // The helm wraps the supervisor's sentence in its own words, so the
  // refusal is pinned by its start and the supervisor's part, not verbatim.
  const refusalText = alert.locator(".launch-composer-refusal-text");
  await expect(refusalText).toHaveText(/^Not launched\. /);
  await expect(refusalText).toContainText(`working directory does not exist: ${missing}`);
  await expect(alert.locator(".launch-composer-refusal-icon")).toHaveText("⚠");
  await expect(folder).toHaveAttribute("aria-invalid", "true");
  const notice = form.locator(".launch-composer-folder-problem");
  await expect(notice).toContainText("This folder does not exist on");
  await expect(folder).toHaveAttribute("aria-describedby", "launcher-folder-problem");

  await folder.fill("/tmp");
  await expect(folder, "another folder is not the one refused").not.toHaveAttribute("aria-invalid", "true");
  await expect(notice).toHaveCount(0);
  await folder.fill(missing);
  await expect(folder, "the refused folder on the same host is still refused").toHaveAttribute("aria-invalid", "true");
  await expect(notice).toBeVisible();

  await chooseLauncherHost(hostPicker, String(remote));
  await expect(folder, "a typed folder survives the host switch").toHaveValue(missing);
  await expect(folder, "the refusal was about the other host").not.toHaveAttribute("aria-invalid", "true");
  await expect(notice).toHaveCount(0);

  await chooseLauncherHost(hostPicker, String(local));
  await expect(folder).toHaveAttribute("aria-invalid", "true");
  await form.getByRole("group", { name: "destination type", exact: true })
    .getByRole("button", { name: "managed checkout", exact: true }).click();
  await expect(folder, "a checkout preview is not the dormant plain folder").toHaveValue("");
  await expect(folder).not.toHaveAttribute("aria-invalid", "true");
  await expect(notice).toHaveCount(0);

  const listing = await (await request.get("/api/sessions")).json();
  expect(listing.sessions.some((session: { title: string }) => session.title === title)).toBe(false);
});

/**
 * Failure before dispatch is still a refused Launch press. Make both browser
 * key generators fail so the async preparation exit, rather than a supervisor
 * response, must publish the same announced alert without sending a create.
 */
test("failed asynchronous launch preparation is an announced refusal", async ({ page }) => {
  await page.goto("/");
  const form = await fillCreateForm(page, { cwd: "/tmp", invocation: "true", title: "unsent-refusal" });
  let creates = 0;
  await page.route("**/api/sessions", async route => {
    if (route.request().method() === "POST") creates++;
    await route.continue();
  });
  await page.evaluate(() => {
    const uuid = Object.getOwnPropertyDescriptor(globalThis.crypto, "randomUUID");
    const random = Math.random;
    Object.defineProperty(globalThis.crypto, "randomUUID", { value: () => "", configurable: true });
    Math.random = () => { throw new Error("fixture key generation failure"); };
    (window as any).__restoreRefusalRandom = () => {
      if (uuid) Object.defineProperty(globalThis.crypto, "randomUUID", uuid);
      else delete (globalThis.crypto as any).randomUUID;
      Math.random = random;
    };
  });
  try {
    expect(await page.evaluate(() => crypto.randomUUID())).toBe("");
    await form.locator(".create-session-submit").click();
    const alert = form.getByRole("alert");
    await expect(alert).toContainText("Not launched. could not generate an idempotency key");
    await expect(alert).toContainText("so it was not sent");
    expect(creates, "the failure must precede host dispatch").toBe(0);
  } finally {
    await page.evaluate(() => {
      (window as any).__restoreRefusalRandom();
      delete (window as any).__restoreRefusalRandom;
    });
  }
});

/**
 * Spec: an error that does not come from a launch press, here "save as
 * template" refusing an unfinished wrapper, appears beside Launch without the
 * "Not launched." alert, even right after a refused launch put one there.
 *
 * Why: the error slot is shared, and labelling a template-save refusal "Not
 * launched." would say a launch happened that the user never asked for.
 */
test("a save-as-template refusal is not labelled Not launched", async ({ page }) => {
  const form = await openNew(page);
  await form.getByRole("button", { name: "Codex", exact: true }).click();
  await form.getByLabel("folder", { exact: true }).fill("/tmp");
  const wrapper = form.locator(".launch-composer-wrapper");
  await expect(wrapper).toBeVisible();
  await wrapper.fill("with-endpoint 'unfinished");
  await form.locator(".create-session-submit").click({ force: true });
  const alert = form.locator(".launch-composer-refusal-alert");
  await expect(alert, "a refused launch press is an alert").toContainText("Not launched. invalid wrapper prefix");

  await form.getByRole("button", { name: "save as template", exact: true }).click();

  await expect(form.locator(".launch-composer-refusal")).toContainText("invalid wrapper prefix");
  await expect(alert).toHaveCount(0);
  await expect(form.locator(".launch-composer-refusal")).not.toContainText("Not launched.");
});
