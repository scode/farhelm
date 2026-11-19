"""The Add a remote host page's shots (website/src/content/docs/docs/get-started/add-a-remote-host.mdx).

Ported from the browser UI's `e2e/docs-shots/add-a-remote-host.spec.ts` (deleted in #3401; `git show
4aec73c6cdd8:e2e/docs-shots/add-a-remote-host.spec.ts`), keeping its wording where the desktop app still does the same
thing. The add-host check for the scenario's invented `add_host` destination is answered by the scripted provisioning
backend the engine stages (`stage_add_host` in `scripts/docs_shots.py`), so the setup question shows the helm's real
plan for an invented host. No shot confirms the setup: the dialog is photographed and dismissed by the engine's reset,
and no host is ever added.
"""

from docs_shots import Callout, Shot

# The scenario's `add_host.ssh` (docs/docs-shots/scenario.json5); the scripted backend answers only for this spelling.
NEW_HOST = "dev@new-box"


def nothing(bridge):
    """The app as the engine's reset leaves it: the navigation rail beside the session list."""


def add_dialog(bridge):
    """Open Add host from the navigation rail and type the invented destination, without pressing Add."""
    bridge.click("id:host-rail-add")
    bridge.wait("id:add-host-destination")
    bridge.fill("id:add-host-destination", NEW_HOST)


def setup_plan(bridge):
    """Press Add for the invented destination and wait for the Set up host question with its plan.

    The question is left unanswered; the engine's reset dismisses it, which is the same as Cancel and changes nothing.
    """
    add_dialog(bridge)
    bridge.click("id:add-host-submit")
    bridge.wait("id:setup-plan")
    bridge.wait("id:prompt-confirm")


def hosts_panel(bridge):
    """Show the Hosts panel, the host list that Hosts at the foot of the navigation rail opens in the list's place."""
    bridge.click("id:hosts-panel-toggle")
    bridge.wait("id:host-list")


SHOTS = [
    Shot(
        "add-button",
        nothing,
        # The rail runs the window's full height; its top, down to Add host, and the start of the list are the picture.
        ["id:host-rail-add", "id:host-rail-all"],
        annotations=[
            Callout("id:host-rail-add",
                    "Adds a host: a Linux machine or Apple silicon Mac you reach over ssh, or a new Tensorlake "
                    "sandbox.",
                    side="right"),
        ],
        max_width=900,
    ),
    Shot(
        "add-dialog",
        add_dialog,
        ["id:add-host"],
        park="id:add-host-frame-title",
        annotations=[
            Callout("id:add-host-destination",
                    "How you would ssh to it: user@host, or a Host name from your ssh config.", side="left", dy=-40),
            # Down the left margin, below the first, so the boxes do not overlap each other or the dialog.
            Callout("id:add-host-submit", "Checks the host first. Nothing changes on it yet.", side="left", dy=110),
        ],
        max_width=930,
    ),
    Shot(
        "setup-plan",
        setup_plan,
        ["id:prompt"],
        # The question has no static title element to rest on (its title is drawn as a host sentence), and the
        # capture's own search finds no spot clear of hover reactions with the plan's path chips and answers up. The
        # plan's middle falls on a step without paths, whose words have no hover help.
        park="id:setup-plan",
        annotations=[
            Callout("id:setup-plan",
                    "Everything setup will create on the host, all inside your own home directory. Nothing has "
                    "changed yet.", side="left"),
            # Both below the row of answers, side by side and far enough apart that the engine need not move one out of
            # the other's way, which would stack them and put a box over Cancel.
            Callout("id:prompt-confirm", "Sets the host up as listed.", side="bottom", dx=-40, dy=40),
            Callout("id:prompt-alternative-0", "Sets it up, and sets up later hosts without asking.", side="bottom",
                    dx=-300, dy=40),
        ],
        max_width=930,
    ),
    Shot(
        "connected-host",
        hosts_panel,
        ["id:host-count", "id:host-row-{host:gpu-1}"],
        annotations=[
            Callout("id:host-row-{host:build-box}",
                    "A set-up host: the green dot means it is connected and ready for sessions.", side="right"),
        ],
        max_width=900,
    ),
]
