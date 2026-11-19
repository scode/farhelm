"""The Manage hosts page's shots (website/src/content/docs/docs/using/manage-hosts.mdx).

Ported from the browser UI's `e2e/docs-shots/manage-hosts.spec.ts` (deleted in #3401; `git show
4aec73c6cdd8:e2e/docs-shots/manage-hosts.spec.ts`), keeping its shot names and placement. Menus and dialogs are opened
and photographed, never used: no host is renamed, updated or removed, and no setting changes. The remote host shown is
the scenario's build-box, whose invented destination the run's private sshd answers, so its destination shows as is.
"""

from docs_shots import Callout, Shot

REMOTE = "build-box"


def host_menu(bridge):
    """Show the host list and open build-box's menu from its `⋯`, without choosing anything in it."""
    bridge.click("id:hosts-panel-toggle")
    bridge.click(f"id:host-menu-toggle-{{host:{REMOTE}}}")
    bridge.wait("id:host-menu-remove")


def host_settings(bridge):
    """Open build-box's settings from its menu, as it opens: scrolled to the top. At the scenario's height the whole
    dialog shows except the last switch's help line, under the pinned footer, so one shot holds every callout."""
    host_menu(bridge)
    bridge.click("id:host-menu-settings")
    bridge.wait("id:host-settings")


def remove_dialog(bridge):
    """Choose Remove in build-box's menu, which asks first on a fresh helm; the question stays unanswered."""
    host_menu(bridge)
    bridge.click("id:host-menu-remove")
    bridge.wait("id:prompt-alternative-0")


def app_settings(bridge):
    """Open Settings from the navigation rail's gear and choose its Hosts section, without changing a switch."""
    bridge.click("id:app-settings-toggle")
    bridge.click("id:settings-section-hosts")
    bridge.wait("id:settings-skip-host-remove")


def details(bridge):
    """Show the host list with Details switched on, which only changes what the list shows, not a setting."""
    bridge.click("id:hosts-panel-toggle")
    bridge.click("id:host-details-toggle")
    bridge.wait(f"id:host-detail-{{host:{REMOTE}}}-0")


SHOTS = [
    Shot(
        "host-menu",
        host_menu,
        [f"id:host-row-{{host:{REMOTE}}}", "id:host-menu"],
        annotations=[
            # Retry and Update explain themselves in the menu's own descriptions. Two boxes stack down the right, each
            # level with its item, and Remove's sits in the empty host list left of the menu.
            Callout("id:host-menu-settings",
                    "Its destination, name and mark, its checkout folder, and what Farhelm asks you before doing "
                    "there.", side="right"),
            Callout("id:host-menu-uninstall",
                    "Shows what it will remove first, and refuses while a session there is still running.",
                    side="right", dy=20),
            Callout("id:host-menu-remove", "Forget the host. Its sessions keep running there.", side="left"),
        ],
        max_width=930,
    ),
    Shot(
        "host-settings",
        host_settings,
        ["id:host-settings"],
        park="id:host-settings-status",
        annotations=[
            # All down the right of the dialog, where the arrows stay short and cross no text: the edit buttons and
            # the switches sit at the dialog's right edge, and the icon grid ends short of it.
            Callout("id:host-settings-edit-destination", "Change how Farhelm reaches the host over ssh.",
                    side="right", dy=-40),
            Callout("id:host-settings-edit-alias", "The name the host goes by in Farhelm.", side="right", dy=30),
            Callout("id:host-appearance-icons",
                    "The mark this host carries on the navigation rail, beside its sessions, and in the session "
                    "launcher's host picker.", side="right"),
            Callout("id:host-yolo-without-asking", "On: YOLO launches here start without asking.", side="right",
                    dy=-10),
            Callout("id:host-own-sessions-without-asking",
                    "On: agents here start, stop, restart, rename and delete sessions on this host without asking.",
                    side="right"),
            Callout("id:host-commands-without-asking",
                    "On: agents here act on sessions on any of your hosts, and change templates, without asking.",
                    side="right", dy=30),
            Callout("id:host-links-without-asking",
                    "On: agents here open public links in your browser without asking.", side="right", dy=60),
            Callout("id:host-local-links-without-asking",
                    "Links to your own machine or private network have their own switch.", side="right", dy=120),
        ],
        max_width=930,
    ),
    Shot(
        "remove-dialog",
        remove_dialog,
        ["id:prompt"],
        # The question's middle is its plain explanation, which has no hover reaction; the capture's own search for a
        # spot finds none while the question covers the window.
        park="id:prompt",
        annotations=[
            Callout("id:prompt-alternative-0",
                    "Removes it, and turns on Remove hosts without asking, so later hosts go without asking.",
                    side="bottom", dy=20),
        ],
        max_width=930,
    ),
    Shot(
        "app-settings",
        app_settings,
        ["id:settings"],
        park="id:settings-frame-title",
        annotations=[
            # Both boxes above the dialog, as the old shot had them: beside it they would make the picture far wider
            # than the docs' text column, and the dialog leaves room above it at the scenario's height.
            Callout("id:settings-sections", "The gear opens Settings at Sessions; choose Hosts here.", side="top",
                    dx=-80, dy=-140),
            Callout("id:settings-skip-host-setup",
                    "Turn either on to skip that question; turn it off to be asked again.", side="top", dx=-90,
                    dy=-180),
        ],
        max_width=930,
    ),
    Shot(
        "details",
        details,
        # The list runs to the window's foot; its three hosts end well above that.
        ["id:host-list", "id:host-details-toggle"],
        annotations=[
            Callout("id:host-details-toggle", "Shows more about each host.", side="right", dx=230, dy=-30),
            # The evidence line under build-box's Destination line (its detail lines number from 0).
            Callout(f"id:host-detail-{{host:{REMOTE}}}-1",
                    "Its Farhelm version and how many sessions it runs, and for a host with an alias, its destination.",
                    side="right", dy=40),
        ],
        max_width=930,
        max_height=400,
    ),
]
