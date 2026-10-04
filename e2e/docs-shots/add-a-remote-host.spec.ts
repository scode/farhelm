/**
 * Screenshots for the docs page "Add a remote host"
 * (website/src/content/docs/docs/get-started/add-a-remote-host.mdx).
 *
 * The add-host check answers for the scenario's invented add_host destination
 * from Farhelm's test stand-in (docs/docs-shots/SPEC.md), so the setup
 * confirmation shows the real plan for an invented host. No shot confirms the
 * setup: the dialog is photographed and left, and no host is ever added.
 */
import { expect, test, type Page } from "@playwright/test";
import { open } from "./open";
import { shot } from "./shot";

const PAGE = "add-a-remote-host";

/** Open the add-host dialog from the hosts panel. */
async function openAddDialog(page: Page) {
  await page.locator(".add-host-button").click();
  const dialog = page.locator(".host-add-dialog");
  await expect(dialog).toBeVisible();
  return dialog;
}

test("add button", async ({ page, request }) => {
  const { director } = await open(page, request);
  const button = page.locator(".add-host-button");
  await director.callout(button, "Adds a host: a Linux machine you can reach over ssh.", { side: "right" });
  await shot(page, `${PAGE}/add-button`, [button, page.locator(".host-list")], { maxWidth: 900 });
});

test("add dialog", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  if (!scenario.add_host) throw new Error("the docs scenario needs an add_host");
  const dialog = await openAddDialog(page);
  await dialog.locator(".add-host-ssh").fill(scenario.add_host.ssh);
  await director.callout(
    dialog.locator(".add-host-ssh"),
    "How you would ssh to it: user@host, or a host alias from your ssh config.",
    { side: "left", dy: -40 },
  );
  // Stacked down the left margin so the boxes do not overlap each other.
  await director.callout(dialog.locator(".add-host-submit"), "Checks the host first. Nothing changes on it yet.", {
    side: "left",
    dy: 110,
  });
  await shot(page, `${PAGE}/add-dialog`, [dialog], { maxWidth: 930 });
});

test("setup plan", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  if (!scenario.add_host) throw new Error("the docs scenario needs an add_host");
  const dialog = await openAddDialog(page);
  await dialog.locator(".add-host-ssh").fill(scenario.add_host.ssh);
  await dialog.locator(".add-host-submit").click();
  const plan = dialog.locator(".provisioning-plan");
  await expect(plan).toContainText(scenario.add_host.ssh);
  const yes = dialog.locator(".provisioning-confirm");
  await expect(yes).toBeVisible();
  await director.callout(
    plan,
    "Everything setup will create on the host, all inside your own home directory. Nothing has changed yet.",
    { side: "left" },
  );
  await director.callout(yes, "Sets the host up as listed.", { side: "left", dy: 30 });
  await director.callout(
    dialog.locator(".host-permanent-answer .btn-outline"),
    "Sets it up, and sets up later hosts without asking.",
    { side: "bottom", dy: 20 },
  );
  await shot(page, `${PAGE}/setup-plan`, [dialog], { maxWidth: 930 });
});

test("connected host", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote) throw new Error("the docs scenario needs a remote host");
  const row = page.locator(".host-row").filter({ has: page.locator(".host-name", { hasText: remote.alias }) });
  await expect(row).toBeVisible();
  await director.callout(row, "A set-up host: the green dot means it is connected and ready for sessions.", {
    side: "right",
  });
  await shot(page, `${PAGE}/connected-host`, [page.locator(".host-list")], { maxWidth: 900 });
});
