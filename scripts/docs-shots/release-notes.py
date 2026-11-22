"""Declare captures for new release-note entries here.

For v1.2.3, define an `open_control(bridge)` that opens the feature and waits for its stable `id:` target. Add
`Shot("v1-2-3-new-control", open_control, ["id:new-control"])` to SHOTS and list `release-notes/v1-2-3-new-control`
in `website/src/release-notes/v1.2.3.json`. The listing supplies the entry's PR number and alt text.

If the shot needs another host, session or template, extend `docs/docs-shots/scenario.json5` with invented data
that preserves the existing shots. If the engine cannot stage that state yet, extend its staging against the
private fleet before adding the shot. The capture skips images that are already published.
"""

from docs_shots import Callout, Shot


def open_session_actions(bridge):
    """Use the staged session's visible title because each capture gets a new session ID."""
    bridge.click("text:Session actions for fix flaky attach test")


def new_group(bridge):
    """Show the naming dialog with a draft name so the shared fleet stays as staged."""
    open_session_actions(bridge)
    bridge.click("id:session-menu-group")
    bridge.click("id:session-menu-group-new")
    bridge.wait("id:rename-title")
    bridge.fill("id:rename-title", "Release preparation")


def mute_choices(bridge):
    """Show the duration choices at the point where the session is still unmuted."""
    open_session_actions(bridge)
    bridge.click("id:session-menu-mute")
    bridge.wait("id:session-menu-mute-indefinite")


SHOTS = [
    Shot("v0-41-0-session-groups", new_group, ["id:rename"], park="id:rename-frame-title"),
    Shot("v0-41-0-session-mute", mute_choices, ["id:session-menu-mute", "id:submenu"]),
]

# --- v0.42.0 --------------------------------------------------------------------------------------------------------

OPEN_SESSION = "fix flaky attach test"
LOOSE_SESSION = "Review terminal links"
GROUP_NAME = "Release preparation"


def grouped_sessions(bridge):
    """Drag a loose row into the open session's group and show the resulting indentation.

    The first member is seeded through the staged helm so the shot can demonstrate the drag itself. The returned
    cleanup removes the temporary group from the shared fleet before another shot uses these rows.
    """
    request = bridge.gui.helm_request
    open_key = bridge.session_key(OPEN_SESSION)
    loose_key = bridge.session_key(LOOSE_SESSION)
    group = request(bridge.run, "POST", "/api/session-groups", {"name": GROUP_NAME, "sessions": [open_key]})["id"]

    def remove_group():
        """Restore the loose rows and wait for the sidebar to receive the removal."""
        request(bridge.run, "DELETE", f"/api/session-groups/{group}")
        bridge.wait_gone_text(GROUP_NAME)

    def member():
        """Confirm the helm accepted the drop before framing the resulting rows."""
        rows = request(bridge.run, "GET", "/api/sessions")["sessions"]
        row = next(row for row in rows if row["id"] == loose_key)
        return (True if row.get("group") == group else None), f"the moved session's group is {row.get('group')}"

    try:
        toggle = f"id:session-group-{group}-toggle"
        grip = f"id:session-pin-grip-{loose_key}"
        bridge.wait(toggle)
        bridge.hover(f"id:session-item-{loose_key}")
        bridge.wait(grip)
        bridge.drag(grip, toggle)
        bridge.gui.poll("the session to join the illustrated group", member, 10, 0.2)
    except Exception:
        remove_group()
        raise
    return remove_group


def settings_search(bridge):
    """Find settings across sections with one plain search, leaving the results open."""
    bridge.click("id:app-settings-toggle")
    bridge.wait("id:settings-search")
    bridge.fill("id:settings-search", "ssh")
    bridge.wait("id:settings-search-results")


def antigravity(bridge):
    """Show Antigravity in the session launcher with its conversation field, before starting anything."""
    bridge.click("id:new-session-button")
    bridge.wait("id:launcher-harness-Antigravity")
    bridge.click("id:launcher-harness-Antigravity")
    bridge.wait("id:launcher-conversation")


def remote_updates(bridge):
    """Show the default remote update setting in a fresh staged helm."""
    bridge.click("id:app-settings-toggle")
    bridge.click("id:settings-section-hosts")
    bridge.wait("id:settings-auto-update-remote-hosts")


def ssh_sign_in_sound(bridge):
    """Show the new SSH sign-in sound setting without sending an authentication request."""
    bridge.click("id:app-settings-toggle")
    bridge.click("id:settings-section-notifications")
    bridge.wait("id:settings-sound-ssh-sign-in")
    bridge.scroll_into_view("id:settings-sound-ssh-sign-in")


def hover_mute(bridge):
    """Open Mute's duration menu by resting the pointer on Mute, without picking a duration."""
    open_session_actions(bridge)
    bridge.hover("id:session-menu-mute")
    bridge.wait("id:session-menu-mute-indefinite")


def new_session_help(bridge):
    """Show the toolbar tooltip below New session, where it leaves the controls beside it visible."""
    bridge.hover("id:new-session-button")
    bridge.wait_text("New session: start an agent or a command on any host")


SHOTS += [
    Shot(
        "v0-42-0-group-membership",
        grouped_sessions,
        ["id:session-order", f"id:session-item-{{session:{OPEN_SESSION}}}",
         f"id:session-item-{{session:{LOOSE_SESSION}}}"],
        annotations=[
            Callout(f"id:session-item-{{session:{LOOSE_SESSION}}}",
                    "Group members are indented. Drag a session by the grip that appears on hover.", side="right"),
        ],
    ),
    Shot(
        "v0-42-0-settings-search",
        settings_search,
        ["id:settings-search-field", "id:settings-search-heading", "id:settings-search-results"],
        park="id:settings-frame-title",
        annotations=[
            Callout("id:settings-search-field", "Find settings by name or description.", side="left"),
        ],
        max_height=400,
    ),
    Shot(
        "v0-42-0-antigravity",
        antigravity,
        ["id:launcher-harness-row", "id:launcher-conversation-row"],
        park="id:launcher-frame-title",
        annotations=[
            Callout("id:launcher-harness-Antigravity", "Choose Antigravity to start an agy session.", side="top",
                    dy=-95),
        ],
    ),
    Shot(
        "v0-42-0-automatic-remote-updates",
        remote_updates,
        ["id:settings-section-hosts", "id:settings-heading", "id:settings-auto-update-remote-hosts"],
        park="id:settings-frame-title",
        annotations=[
            Callout("id:settings-auto-update-remote-hosts", "Turn this off to update remote hosts yourself.",
                    side="right"),
        ],
    ),
    Shot(
        "v0-42-0-ssh-sign-in-sound",
        ssh_sign_in_sound,
        ["id:settings-sounds"],
        park="id:settings-frame-title",
        annotations=[
            Callout("id:settings-sound-ssh-sign-in", "Farhelm can chime when a remote host needs you to sign in.",
                    side="right"),
        ],
    ),
    Shot(
        "v0-42-0-hover-submenu",
        hover_mute,
        ["id:session-menu", "id:submenu"],
        park="id:session-menu-mute",
        annotations=[
            Callout("id:session-menu-mute", "Hover over Mute to see the durations.", side="right", dy=-135),
        ],
    ),
    Shot(
        "v0-42-0-tooltip-placement",
        new_session_help,
        ["id:new-session-button", f"id:session-item-{{session:{OPEN_SESSION}}}"],
        park="id:new-session-button",
        annotations=[
            Callout("id:new-session-button", "The tooltip opens below the button.", side="right", dx=150, dy=24),
        ],
    ),
]

# --- v0.43.0 --------------------------------------------------------------------------------------------------------


def shell_sessions(bridge):
    """Show both shell choices together as they appear before a session starts."""
    bridge.click("id:new-session-button")
    bridge.wait("id:launcher-shells-Bash")
    bridge.wait("id:launcher-shells-Zsh")


SHOTS += [
    Shot(
        "v0-43-0-shell-sessions",
        shell_sessions,
        ["id:launcher-shells-row"],
        park="id:launcher-frame-title",
        annotations=[
            Callout("id:launcher-shells-Zsh", "Start a shell session.", side="right"),
        ],
        max_width=480,
    ),
]

# --- v0.44.1 --------------------------------------------------------------------------------------------------------

LAUNCHER_PARK = "id:launcher-destination-heading"
SETTINGS_PARK = "id:settings-frame-title"


def hook_setup_prompt(bridge):
    """Show the choices offered on a first Grok pick without saving consent or launching."""
    bridge.click("id:new-session-button")
    bridge.click("id:launcher-harness-Grok")
    bridge.wait("id:hook-setup-consent")
    bridge.wait("id:prompt-confirm")


def tailscale_host(bridge):
    """Show the Tailscale route on an Add host draft before Farhelm contacts the destination."""
    bridge.click("id:host-rail-add")
    bridge.fill("id:add-host-destination", "build-node")
    bridge.click("id:add-host-tailscale")


def antigravity_trust(bridge):
    """Show the trust switch with the selected navigation row so readers can find it in Settings."""
    bridge.click("id:app-settings-toggle")
    bridge.scroll_into_view("id:settings-section-antigravity")
    bridge.click("id:settings-section-antigravity")
    bridge.wait("id:settings-antigravity-trust")


def launcher_layout(bridge):
    """Show the wide launcher before typing, with the form and recent setups visible together."""
    bridge.click("id:new-session-button")
    bridge.wait("id:launcher-search")
    bridge.wait("id:launcher-recent-0")


def settings_layout(bridge):
    """Use Appearance to show the new navigation, pinned page heading and side-by-side theme previews."""
    bridge.click("id:app-settings-toggle")
    bridge.click("id:settings-section-appearance")
    bridge.wait("id:settings-appearance-Light")


SHOTS += [
    Shot(
        "v0-44-1-conversation-hooks",
        hook_setup_prompt,
        ["id:launcher-launch", "id:prompt"],
        park=LAUNCHER_PARK,
        annotations=[
            Callout("id:prompt-confirm", "Lets Restart resume Grok conversations on all hosts.", side="bottom",
                    dy=34),
        ],
    ),
    Shot(
        "v0-44-1-tailscale-host",
        tailscale_host,
        ["id:add-host"],
        park="id:add-host-frame-title",
        annotations=[
            Callout("id:add-host-tailscale", "Verifies the SSH host key that this node advertises on your tailnet.",
                    side="left"),
        ],
    ),
    Shot(
        "v0-44-1-antigravity-trust",
        antigravity_trust,
        [SETTINGS_PARK, "id:settings-section-antigravity", "id:settings-antigravity-trust"],
        park=SETTINGS_PARK,
        annotations=[
            Callout("id:settings-antigravity-trust", "Answers Antigravity's workspace trust question for new sessions.",
                    side="right"),
        ],
    ),
    Shot(
        "v0-44-1-session-launcher",
        launcher_layout,
        ["id:launcher"],
        park=LAUNCHER_PARK,
        annotations=[
            Callout("id:launcher-search", "Start here to find a host, agent, directory or template.", side="top"),
            # The form starts beside Recent. Keep the callout on the far side of that column.
            Callout("id:launcher-recent-0", "A recent setup fills in the form.", side="left"),
        ],
    ),
    Shot(
        "v0-44-1-settings",
        settings_layout,
        ["id:settings"],
        park=SETTINGS_PARK,
        annotations=[
            Callout("id:settings-appearance-Light", "Compare color themes here before choosing one.", side="bottom"),
        ],
        max_height=620,
    ),
]
