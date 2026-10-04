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
  await director.callout(panel.locator(".host-settings"), "Its destination, its name, and whether it asks before YOLO launches.", {
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
  // The edit buttons sit at the dialog's right edge, so their callouts go to
  // the right of the dialog, where the arrows stay short and cross no text.
  // The YOLO one goes underneath, shifted right: on the left it would widen
  // the crop past the docs' text column.
  await director.callout(dialog.locator(".host-edit"), "Change how Farhelm reaches the host over ssh.", {
    side: "right",
    dy: -40,
  });
  await director.callout(dialog.locator(".host-alias"), "The name the host goes by in Farhelm.", {
    side: "right",
    dy: 30,
  });
  await director.callout(
    dialog.locator(".host-yolo-without-asking-toggle"),
    "Ticked: YOLO launches here start without asking.",
    { side: "bottom", dx: 120, dy: 95 },
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
    dialog.locator(".host-permanent-answer .btn-outline"),
    "Removes it, and removes later hosts without asking.",
    { side: "bottom", dy: 20 },
  );
  await shot(page, `${PAGE}/remove-dialog`, [dialog], { maxWidth: 930 });
});

test("app settings", async ({ page, request }) => {
  const { director } = await open(page, request);
  await page.locator(".app-settings-toggle").click();
  const dialog = page.locator(".app-settings-dialog");
  await expect(dialog).toBeVisible();
  await director.callout(dialog, "Both questions Farhelm asks about hosts can be turned off, or back on, here.", {
    side: "left",
  });
  await shot(page, `${PAGE}/app-settings`, [dialog, page.locator(".app-settings-toggle")], { maxWidth: 930 });
});
