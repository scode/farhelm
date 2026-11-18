"""Declare captures for new release-note entries here.

For v1.2.3, define an `open_control(bridge)` that opens the feature and waits for its stable `id:` target. Add
`Shot("v1-2-3-new-control", open_control, ["id:new-control"])` to SHOTS and list `release-notes/v1-2-3-new-control`
in `website/src/release-notes/v1.2.3.json`. The listing supplies the entry's PR number and alt text.

If the shot needs another host, session or template, extend `docs/docs-shots/scenario.json5` with invented data
that preserves the existing shots. If the engine cannot stage that state yet, extend its staging against the
private fleet before adding the shot. The capture skips images that are already published.
"""

from docs_shots import Shot


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
