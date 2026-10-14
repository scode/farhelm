/**
 * Screenshots for the docs page "Manage hosts"
 * (website/src/content/docs/docs/using/manage-hosts.mdx).
 *
 * Menus and dialogs are opened and photographed, never used: no host is
 * renamed, updated, or removed, and no setting changes.
 */
import { expect, test } from "@playwright/test";
import { hostRowByName, openHostMenu } from "../tests/helpers/fleet";
import { open } from "./open";
import { shot } from "./shot";

const PAGE = "manage-hosts";

test("host menu", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote) throw new Error("the docs scenario needs a remote host");
  const row = hostRowByName(page, remote.alias);
  await openHostMenu(row);
  const panel = page.locator(".host-row-menu-panel");
  await expect(panel).toBeVisible();
  // Four callouts stacked down the right; the offsets keep each box clear of
  // the next, since the menu items sit closer together than the boxes are tall.
  await director.callout(panel.locator(".host-retry"), "Try the connection again now, instead of waiting.", {
    side: "right",
    dy: -40,
  });
  await director.callout(panel.locator(".host-settings"), "Its destination, its name, and what Farhelm asks you before doing there.", {
    side: "right",
    dy: 0,
  });
  await director.callout(panel.locator(".provisioning-uninstall"), "Remove Farhelm from the host, keeping its data.", {
    side: "right",
    dy: 40,
  });
  await director.callout(panel.locator(".host-remove"), "Forget the host. Its sessions keep running there.", {
    side: "right",
    dy: 110,
  });
  await shot(page, `${PAGE}/host-menu`, [row, panel], { maxWidth: 930 });
});

test("host settings", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote) throw new Error("the docs scenario needs a remote host");
  const row = hostRowByName(page, remote.alias);
  await openHostMenu(row);
  await row.locator(".host-settings").click();
  const dialog = page.locator(".host-settings-dialog");
  await expect(dialog).toBeVisible();
  // All the callouts down the right of the dialog, where the arrows stay
  // short and cross no text: the edit buttons and the switches sit at the
  // dialog's right edge, and the icon grid ends short of it. Anywhere else
  // either crosses the help text or widens the crop past the docs' text column.
  await director.callout(dialog.locator(".host-edit"), "Change how Farhelm reaches the host over ssh.", {
    side: "right",
    dy: -40,
  });
  await director.callout(dialog.locator(".host-alias"), "The name the host goes by in Farhelm.", {
    side: "right",
    dy: 30,
  });
  await director.callout(
    dialog.locator(".host-appearance-icons"),
    "The mark this host's sessions carry in the session list, the host list, and the launcher's host menu.",
    { side: "right" },
  );
  await director.callout(
    dialog.locator(".host-yolo-without-asking"),
    "On: YOLO launches here start without asking.",
    { side: "right", dy: 60 },
  );
  await director.callout(
    dialog.locator(".host-commands-without-asking"),
    "On: agents here run farhelm commands without asking you first.",
    { side: "right", dy: 50 },
  );
  await shot(page, `${PAGE}/host-settings`, [dialog], { maxWidth: 930 });
});

test("remove dialog", async ({ page, request }) => {
  const { scenario, director } = await open(page, request);
  const remote = scenario.hosts.find((host) => host.kind === "remote");
  if (!remote) throw new Error("the docs scenario needs a remote host");
  const row = hostRowByName(page, remote.alias);
  await openHostMenu(row);
  await row.locator(".host-remove").click();
  const dialog = page.locator(".host-remove-dialog");
  await expect(dialog).toBeVisible();
  await director.callout(
    dialog.locator(".host-dialog-answers .btn-outline"),
    "Removes it, and removes later hosts without asking.",
    { side: "bottom", dy: 20 },
  );
  await shot(page, `${PAGE}/remove-dialog`, [dialog], { maxWidth: 930 });
});

test("app settings", async ({ page, request }) => {
  // Tall enough that every section fits without scrolling: Settings scrolls
  // its sections under a fixed heading and close button, so at a shorter
  // height the sounds section sits below the fold and its callout would
  // point at the close button instead. The terminal links section pushed
  // sounds past a height of 1500.
  await page.setViewportSize({ width: 1600, height: 1900 });
  const { director } = await open(page, request);
  await page.locator(".app-settings-toggle").click();
  const dialog = page.locator(".app-settings-dialog");
  await expect(dialog).toBeVisible();
  // Down the left of the dialog, one per part, each level with what it
  // explains; the dialog's own help text says the rest.
  await director.callout(
    dialog.locator(".app-settings-choice").first(),
    "Both questions Farhelm asks about hosts can be turned off, or back on, here.",
    { side: "left" },
  );
  await director.callout(
    dialog.getByRole("region", { name: "terminal links" }).locator(".app-settings-choice").first(),
    "What clicking a web link in a terminal does, for every client of this helm.",
    { side: "left" },
  );
  await director.callout(
    dialog.locator(".checkout-root-field"),
    "The directory managed checkouts go into, on every host.",
    { side: "left" },
  );
  await director.callout(
    dialog.locator(".checkout-post-clone-field"),
    "A command to run in each new managed checkout before its agent starts, on every host.",
    { side: "left" },
  );
  await director.callout(
    dialog.locator("section[aria-label=sounds]"),
    "Sounds for when a session needs you, on this device only.",
    { side: "left" },
  );
  await shot(page, `${PAGE}/app-settings`, [dialog, page.locator(".app-settings-toggle")], { maxWidth: 930 });
});

test("details", async ({ page, request }) => {
  const { director } = await open(page, request);
  const toggle = page.locator(".host-details-toggle");
  await toggle.check();
  // The second host is a remote one with an alias, so its details also show
  // the real destination behind the name.
  const remote = page.locator(".host-row").nth(1);
  await expect(remote.locator(".host-destination-detail")).toBeVisible();
  await director.callout(toggle, "Shows more about each host.", { side: "right", dx: 230, dy: -30 });
  await director.callout(
    remote.locator(".host-detail"),
    "Its Farhelm version and how many sessions it runs, and for a renamed host, its real destination.",
    { side: "right", dy: 40 },
  );
  await shot(page, `${PAGE}/details`, [page.locator(".host-list"), toggle], { maxWidth: 930 });
});
