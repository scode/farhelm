"""The Start a session page's shots (website/src/content/docs/docs/using/start-a-session.mdx).

Ported from the browser UI's `e2e/docs-shots/start-a-session.spec.ts` (deleted in #3401; `git show
4aec73c6cdd8:e2e/docs-shots/start-a-session.spec.ts`), keeping its wording and placement where the desktop app still
does the same thing. Nothing here launches a session or changes a setting: every shot stops at the state it photographs
and the engine's reset dismisses it. The YOLO shot does press Launch, which the helm refuses with the YOLO question
because the staged hosts still ask; the engine's session guard fails the shot should that ever stop being true.

Most launcher crops name the launcher's labelled rows (`launcher-<field>-row`) or its Destination block
(`launcher-destination-section`) rather than the controls inside them, because the controls start at the control
column and a crop around them alone would cut the labels ("Where", "Harness", "Effort", ...) off.
"""

from docs_shots import Callout, Highlight, Shot

# The launcher's own title: static, with no hover help or reaction, and above anything its popovers draw.
LAUNCHER_PARK = "id:launcher-frame-title"
# A remote host whose destination is a plain invented name (docs/docs-shots/scenario.json5), so its menu and settings
# show nothing the capture's leak check refuses.
HOST = "build-box"


# --- Steps -----------------------------------------------------------------------------------------------------------


def nothing(bridge):
    """The session list as the engine's reset leaves it, with + (New session) in its heading."""


def open_launcher(bridge):
    """Open the session launcher from + (New session), as a person would; it opens with no agent chosen."""
    bridge.click("id:new-session-button")
    bridge.wait("id:launcher-search")


def codex(bridge):
    """Open the launcher and choose Codex, which fills in the agent's own choices and makes the launch complete."""
    open_launcher(bridge)
    bridge.click("id:launcher-harness-Codex")
    bridge.wait("id:launcher-model")


def folder_list(bridge):
    """Open the directory list under the Folder field with a click and ↓, as the old shot showed it."""
    open_launcher(bridge)
    bridge.click("id:launcher-folder")
    bridge.press("down")
    bridge.wait("id:launcher-folder-list")


def checkout_setup(bridge):
    """Choose Managed checkout on a helm with no checkout directory yet, the staged fleet's state, and type one.

    Stops before Save, so the directory stays unset for every other shot.
    """
    open_launcher(bridge)
    bridge.click("id:launcher-destination-Managed checkout")
    bridge.wait("id:checkout-folder")
    bridge.fill("id:checkout-folder", "~/src")


def search_claude(bridge):
    """Type a plain word into the search box, which has the focus when the launcher opens, and wait for its results."""
    bridge.click("id:new-session-button")
    bridge.type_into_focused("id:launcher-search", "claude")
    bridge.wait("id:launcher-search-option-1")


def search_name(bridge):
    """Type the same word behind name:, which turns it into a session name instead of the Claude agent."""
    bridge.click("id:new-session-button")
    bridge.type_into_focused("id:launcher-search", "name: claude")
    # The name group's one row, Set session name: claude.
    bridge.wait("id:launcher-search-option-0")


def recent_setups(bridge):
    """Open the launcher and wait for the recent setups that match its starting directory."""
    bridge.click("id:new-session-button")
    bridge.wait("id:launcher-recent-0")


def yolo_question(bridge):
    """Choose Codex and YOLO and press Launch: the host asks before a YOLO launch, so the question appears instead."""
    codex(bridge)
    bridge.click("id:launcher-permissions-YOLO")
    bridge.click("id:launcher-launch")
    bridge.wait("id:prompt-confirm")


def host_menu(bridge):
    """Open the Hosts panel and the remote host's ⋯ menu, without choosing anything in it."""
    bridge.click("id:hosts-panel-toggle")
    bridge.click(f"id:host-menu-toggle-{{host:{HOST}}}")
    bridge.wait("id:host-menu-settings")


def host_settings(bridge):
    """Open the remote host's settings from its menu and scroll its Permissions section into view."""
    host_menu(bridge)
    bridge.click("id:host-menu-settings")
    bridge.wait("id:host-yolo-without-asking")
    bridge.scroll_into_view("id:host-settings-permissions")


# --- Shots, in page order --------------------------------------------------------------------------------------------

SHOTS = [
    Shot(
        "new-button",
        nothing,
        ["id:new-session-button", "id:session-status"],
        annotations=[
            # Above the button's row, over the terminal, so it covers neither the neighbouring controls nor the list.
            Callout("id:new-session-button", "Opens the session launcher, where every session starts", side="right",
                    dy=-70),
        ],
    ),
    Shot(
        "launch-button",
        codex,
        ["id:launcher-launch", "id:launcher-summary-target", "id:launcher-summary-choices"],
        park=LAUNCHER_PARK,
        annotations=[
            Highlight("id:launcher-launch", pad=3),
            # Both above the footer, side by side, clear of the target line, the choices line and the buttons.
            Callout("id:launcher-summary-target",
                    "What Launch will start: the agent, host and directory. Launch stays grey until the setup is "
                    "complete; hover it to see what is missing.", side="top", dx=-50),
            Callout("id:launcher-summary-choices",
                    "The choices the launch will use. Check it before launching: YOLO shows in red.", side="top",
                    dx=350),
        ],
    ),
    Shot(
        "destination",
        folder_list,
        ["id:launcher-destination-section", "id:launcher-folder-list"],
        park=LAUNCHER_PARK,
        annotations=[
            # Three callouts down the left margin, nudged apart so the arrows stay level with their controls; the
            # host's sits to its right, in the empty space beside the host menu.
            Callout("id:launcher-destination",
                    "Folder starts in a directory that already exists; Managed checkout clones a GitHub repository "
                    "into a new one.", side="left", dy=-70),
            Callout("id:launcher-host",
                    "The host the session runs on. Starts on the host of the session you have open.", side="right"),
            Callout("id:launcher-folder",
                    "The directory the agent starts in. ~ is your home on that host, not on the machine in front of "
                    "you.", side="left", dy=75),
            Callout("id:launcher-folder-list",
                    "Opens on a click or ↓: up to eight recent directories on this host, newest first; typing filters "
                    "them. Home is ~ on that host, Local home ~ on the machine in front of you.", side="left",
                    dy=100),
        ],
    ),
    Shot(
        "checkout-setup",
        checkout_setup,
        ["id:launcher-destination-section"],
        park=LAUNCHER_PARK,
        annotations=[
            Callout("id:checkout-folder",
                    "Asked once: where managed checkouts go on every host without its own. ~ is each host's own home, "
                    "and Farhelm creates the directory on first use.", side="left"),
            Callout("id:checkout-folder-save",
                    "Saves the default. Change it later in Settings → Checkouts, or give a host its own in its "
                    "settings.", side="left", dy=40),
        ],
        max_width=930,
    ),
    Shot(
        "harness-and-model",
        codex,
        ["id:launcher-harness-row", "id:launcher-model-row"],
        park=LAUNCHER_PARK,
        annotations=[
            # In the empty margin right of the launcher: dx keeps the arrows visible, dy keeps the boxes, each taller
            # than its row, from touching.
            Callout("id:launcher-harness",
                    "Pick the agent first. Nothing is chosen when the launcher opens, and the choices below change to "
                    "what that agent offers.", side="right", dx=60, dy=-60),
            # The whole row rather than the field, which ends mid-column: from the field the box would land on the
            # launcher's own controls.
            Callout("id:launcher-model-row",
                    "Which model. Harness default leaves it to the agent's own setting; type a name to use one the "
                    "list doesn't show.", side="right", dx=60, dy=50),
        ],
    ),
    Shot(
        "effort-and-permissions",
        codex,
        ["id:launcher-effort-row", "id:launcher-permissions-row"],
        park=LAUNCHER_PARK,
        annotations=[
            # Arrows to the choices themselves rather than the rows' blank space; the boxes share the margin right of
            # the launcher.
            Callout("id:launcher-effort-High", "How much effort the model puts in. Default leaves it to the agent.",
                    side="right", dx=160, dy=-130),
            Callout("id:launcher-permissions",
                    "Guarded lets the agent approve its own actions through a safety check, which can still refuse or "
                    "ask you. YOLO skips those checks: the agent runs commands and changes files without asking. "
                    "Custom passes your own flags and counts as YOLO.", side="right", dx=600, dy=120),
        ],
    ),
    Shot(
        "search",
        search_claude,
        ["id:launcher-search", "id:launcher-search-results"],
        park=LAUNCHER_PARK,
        annotations=[
            Callout("id:launcher-search",
                    "Type to find anything the launcher can set: hosts, agents, models, directories, recent setups.",
                    side="left"),
            Callout("id:launcher-search-results",
                    "Choosing a result applies it and empties the box. Enter on the empty box launches.", side="left",
                    dy=60),
        ],
        max_width=900,
    ),
    Shot(
        "search-name",
        search_name,
        ["id:launcher-search", "id:launcher-search-results"],
        park=LAUNCHER_PARK,
        annotations=[
            Callout("id:launcher-search",
                    "name: makes the words a session name even when they match another choice, such as the Claude "
                    "agent.", side="left"),
        ],
        max_width=900,
    ),
    Shot(
        "recent-setups",
        recent_setups,
        ["id:launcher-recents"],
        park=LAUNCHER_PARK,
        annotations=[
            Highlight("id:launcher-recents", pad=4),
            # Below the list rather than beside it, which keeps the crop no wider than the list itself.
            Callout("id:launcher-recents",
                    "Setups you launched before that match the directory and choices so far, most used first. Click "
                    "one to fill in the launcher; Enter on it launches straight away.", side="bottom", dx=200),
        ],
        max_width=900,
    ),
    Shot(
        "yolo-confirmation",
        yolo_question,
        ["id:prompt", "id:launcher-summary-target"],
        park=LAUNCHER_PARK,
        annotations=[
            # Side by side below the answers, clear of the question's text.
            Callout("id:prompt-confirm", "Starts this launch. The host still asks next time.", side="bottom", dx=-40,
                    dy=40),
            # Alternative 0 is Start guarded session instead, which Codex offers; 1 is the host-wide answer.
            Callout("id:prompt-alternative-1", "Starts it, and this host stops asking from now on.", side="bottom",
                    dx=-110, dy=40),
        ],
        max_width=930,
    ),
    Shot(
        "host-menu",
        host_menu,
        # Remove, the last item, too: the panel's own box can end above its last items.
        [f"id:host-row-{{host:{HOST}}}", "id:host-menu", "id:host-menu-remove"],
        park="id:host-count",
        annotations=[
            Callout("id:host-menu-settings",
                    "Each host's own settings, including whether it asks before YOLO launches", side="right"),
        ],
    ),
    Shot(
        "host-yolo-setting",
        host_settings,
        ["id:host-settings-permissions"],
        # The capture's own search finds no spot clear of hover reactions in the settings dialog. The section's middle
        # falls among its switches' explanations, which have none.
        park="id:host-settings-permissions",
        annotations=[
            Highlight("id:host-yolo-without-asking", pad=4),
            # Right of the switch, which sits at the dialog's right edge, so the setting's own words stay readable.
            Callout("id:host-yolo-without-asking",
                    "On: YOLO launches on this host start without asking. Off: Farhelm asks every time.",
                    side="right"),
        ],
    ),
]
