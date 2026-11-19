"""The Stop, restart, and resume page's shots (website/src/content/docs/docs/using/stop-restart-resume.mdx).

Ported from the browser UI's `e2e/docs-shots/stop-restart-resume.spec.ts` (deleted in #3401; `git show
4aec73c6cdd8:e2e/docs-shots/stop-restart-resume.spec.ts`), keeping its shot names and placement. Every action on this
page ends or replaces a session, so the shots only ever show buttons and open menus, never use them. Restart's
confirmation is not shown: it needs a session Farhelm can resume, which the staged fleet's fake agents cannot honestly
provide, and for the same reason the open session's Restart is greyed out, which the header shot explains.
"""

from docs_shots import Callout, Shot

WAITING = "migrate auth tokens"


def nothing(bridge):
    """The open session's header as the engine's reset leaves it, with no menu or question open."""


def row_menu(bridge):
    """Open the waiting session's menu from its row in the session list, without choosing anything in it."""
    bridge.click(f"text:Session actions for {WAITING}")
    bridge.wait("id:session-menu-delete")


SHOTS = [
    Shot(
        "header-actions",
        nothing,
        # Stop through Delete, not the whole row, which is wider than the docs' text column. (The actions group's own
        # `session-header-actions` id is not one the bridge finds.) The Stop box's offset widens the crop leftward over
        # the text size buttons, as in the old shot.
        ["id:session-header-stop", "id:session-header-delete"],
        annotations=[
            # The boxes hang down over the terminal, staggered diagonally in button order: each sits lower and further
            # right than the one before, so each arrow drops past the right edge of every box above it.
            Callout("id:session-header-stop", "Stop: ends the agent and closes its shell tabs. The session stays.",
                    side="bottom", dx=-210),
            Callout("id:session-header-restart",
                    "Restart: resumes this session's conversation. Greyed out, as here, when Farhelm cannot resume it; "
                    "hover to see why.", side="bottom", dx=-90, dy=100),
            Callout("id:session-header-replace", "Replace: a fresh conversation with the same settings.",
                    side="bottom", dx=-50, dy=230),
            Callout("id:session-header-clone", "Clone: another session like this one, which stays.", side="bottom",
                    dy=340),
        ],
    ),
    Shot(
        "row-menu",
        row_menu,
        [f"id:session-item-{{session:{WAITING}}}", "id:session-menu"],
        annotations=[
            Callout("id:session-menu-stop",
                    "Stop: ends the agent and closes its terminal tabs. Jobs you run under your own tmux can keep "
                    "running. The session stays.", side="right", dy=-50),
            Callout("id:session-menu-delete", "Delete: removes the session altogether.", side="right", dy=30),
        ],
        max_width=930,
    ),
]
