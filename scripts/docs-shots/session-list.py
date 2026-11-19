"""The Read the session list page's shots (website/src/content/docs/docs/using/session-list.mdx).

Ported from the browser UI's `e2e/docs-shots/session-list.spec.ts` (deleted in #3401; `git show
4aec73c6cdd8:e2e/docs-shots/session-list.spec.ts`), keeping its shot names, callout wording and placement where the
desktop app still does the same thing. Nothing here changes a session or a setting: menus and the trash dialog are
opened and photographed, never used.

The trash shots need the scenario's `checkout_trash` staged in the app, which the browser capture did for those two
shots alone. The desktop app reads a host's trash when it first sees the host, so in one shared app run the staging
holds for the whole run, and every other shot's heading shows the full can too.
"""

from docs_shots import Callout, Shot

WAITING = "migrate auth tokens"
UNREAD = "bench tokenizer"
EXITED = "release notes"
#: Every staged row. The app orders running and waiting sessions first, then by when each last started work, which a
#: run does not fix, so a shot spanning the list names all of them rather than guessing which comes first and last.
ROWS = ["fix flaky attach test", WAITING, UNREAD, EXITED, "Review terminal links"]


# --- Steps ------------------------------------------------------------------------------------------------------------


def nothing(bridge):
    """The list as the engine's reset leaves it: every row, no layer open."""


# The waiting row is the list's first (running and waiting sessions come first), so its callout stays level with it
# rather than above it, where it would pull the heading half into the crop. The other two stay level with their rows
# too: their boxes are taller than a row, so the second of them is pushed below the first, and which of the two comes
# first is settled per run (`statuses`).
STATUS_CALLOUTS = [
    Callout(f"id:session-item-{{session:{WAITING}}}",
            "Waiting: the agent is asking you something. Answer it in its terminal.", side="right"),
    Callout(f"id:session-item-{{session:{UNREAD}}}",
            "Idle and unread: it finished since you last looked. Opening it marks it read.", side="right"),
    Callout(f"id:session-item-{{session:{EXITED}}}",
            "Ended: the agent quit, here with exit code 0. Its terminal stays readable.", side="right"),
]


def statuses(bridge):
    """The whole list, with the idle and ended callouts reordered to match their rows' order on screen.

    The engine lays callouts out in declaration order and pushes a box that would cover an earlier one below it. The
    idle and ended sessions both last started work long ago, and which the app lists first varies from run to run, so
    a fixed order would sometimes stack the boxes against the rows' order and cross their arrows. Swapping the two
    entries of the shared list in place, before the engine reads it, keeps each box beside or below its own row.
    """
    top = {title: bridge.box(f"id:session-item-{{session:{title}}}")["y"] for title in (UNREAD, EXITED)}
    STATUS_CALLOUTS[1:] = sorted(STATUS_CALLOUTS[1:], key=lambda callout: top[UNREAD if UNREAD in callout.target
                                                                                else EXITED])


def row_menu(bridge):
    """Open the unread idle row's menu, which offers Mark read, without choosing anything in it."""
    bridge.click(f"text:Session actions for {UNREAD}")
    bridge.wait("id:session-menu-rename")


def trash_button(bridge):
    """The heading with the staged trash counted: the full can and its count, as the engine's reset leaves the list.

    The app takes each host's staged listing when it first sees the host, long before the first shot, so there is
    nothing to wait for. The count has no element of its own to wait on, and `wait_text` reads only drawn text, where
    a bare "3" could be any number on screen.
    """


def trash_dialog(bridge):
    """Open the trash over the staged listing and wait for build-box's card, the last one, to show where it keeps them.

    It stops at the listing: an entry's Delete acts at once without asking, and the staging refuses deletes anyway.
    """
    bridge.click("id:checkout-trash-button")
    bridge.wait("id:trash-folder-{host:build-box}-0")


# --- Shots ------------------------------------------------------------------------------------------------------------

SHOTS = [
    Shot(
        "row",
        nothing,
        [f"id:session-item-{{session:{WAITING}}}"],
        annotations=[
            # The first two callouts are pushed right past the list, so they sit in the terminal's space and stack
            # without covering other rows.
            Callout(
                f"id:session-seen-{{session:{WAITING}}}",
                "Status: a pulsing green circle while the agent works, a red triangle when it waits for you, a blue "
                "diamond when it finished since you last looked, a grey ring once you have.",
                side="top", dx=520, dy=-20,
            ),
            Callout(
                f"id:session-row-{{session:{WAITING}}}-agent",
                "Which agent, and its permissions: an amber slashed shield is YOLO; a green shield is guarded, where a "
                "safety check can refuse an action or ask you.",
                side="right", dx=70, dy=85,
            ),
            # Keep the box below the host and directory line so its arrow clears both callouts to the right.
            # The left shift keeps the navigation rail's foot in the crop.
            Callout(f"id:session-row-{{session:{WAITING}}}-place", "The host and directory it runs in.", side="bottom",
                    dx=-90, dy=10),
        ],
        max_width=930,
    ),
    Shot(
        "statuses",
        statuses,
        # The rows span the list; the list's own element fills the sidebar to the window's foot.
        [f"id:session-item-{{session:{title}}}" for title in ROWS],
        # `statuses` puts the idle and ended callouts in the order their rows appear.
        annotations=STATUS_CALLOUTS,
        max_width=930,
    ),
    Shot(
        "controls",
        nothing,
        ["id:host-rail-host-{host:local}", "id:session-order", "id:session-status"],
        annotations=[
            # The menu stays closed: its choices carry their own descriptions, and opening it hides the status filter.
            Callout("id:session-order",
                    "The list's order. Click it for Recently active, Newest created or Title A–Z, or for Compact: one "
                    "line per session.", side="right", dx=320),
            Callout("id:session-status",
                    "Show only running, waiting or idle sessions. Ended sessions appear under All.", side="right",
                    dx=40, dy=120),
            # Below the rail's top rather than beside it, so its arrow runs up the rail instead of across the list.
            Callout("id:host-rail-host-{host:local}",
                    "Show only this host's sessions; All above it shows every host.", side="bottom", dx=120, dy=300),
        ],
        max_width=930,
    ),
    Shot(
        "row-menu",
        row_menu,
        [f"id:session-item-{{session:{UNREAD}}}", "id:session-menu"],
        annotations=[
            Callout("id:session-menu-rename", "Give the session a name you will recognize.", side="right"),
            Callout("id:session-menu-seen", "Mark it read, or unread again to come back to later.", side="right",
                    dy=90),
        ],
        max_width=930,
    ),
    Shot(
        "trash-button",
        trash_button,
        ["id:checkout-trash-button", "id:new-session-button"],
        annotations=[
            # Up and right, into the terminal's space, so the box covers neither the heading's buttons nor the list.
            Callout("id:checkout-trash-button",
                    "Managed checkouts whose last session was deleted. The lid opens and a count shows while it holds "
                    "any.", side="right", dx=160, dy=-70),
        ],
        max_width=930,
    ),
    Shot(
        "trash-dialog",
        trash_dialog,
        ["id:checkout-trash-dialog"],
        # The first host's totals line is static text with no hover reaction. The dialog draws its own heading, so the
        # frame's title id the other dialogs park on does not exist here, and capture's own search for a quiet spot
        # finds none over a dialog this full of controls.
        park="id:trash-host-totals-{host:local}",
        annotations=[
            # The delete buttons get no callout: they sit at the dialog's top and right edges, where a callout would
            # cover the listing or the dialog's own explanation, so the page's text explains them instead.
            Callout("id:trash-folder-{host:build-box}-0",
                    "Where each host keeps them. Copy out anything you want back before deleting.", side="bottom",
                    dx=-40, dy=60),
        ],
    ),
]
