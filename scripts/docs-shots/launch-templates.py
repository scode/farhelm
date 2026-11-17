"""The Launch templates page's entry point and template search; neither shot changes a template or launches a session."""

from docs_shots import Shot


def templates_button(bridge):
    """Show Templates beside New session in the ordinary session list after the engine's reset."""
    pass


def apply(bridge):
    """Show the launcher's tl: suggestions from the scenario's real template catalog, without choosing one."""
    bridge.click("id:new-session-button")
    bridge.wait_text("Destination")
    bridge.fill("id:launcher-search", "tl:")
    # The catalog is name-ordered: api-repo precedes codex-deep in this scenario's unfiltered tl: results.
    bridge.wait("id:launcher-search-option-1")


SHOTS = [
    Shot("templates-button", templates_button, ["id:templates-button", "id:new-session-button"]),
    # The static title has no hover help or reaction and stays above the search results, clear of their popover.
    Shot("apply", apply, ["id:launcher-search-box", "id:launcher-search-results"], park="id:launcher-frame-title"),
]
