"""The Work in a session page's shots (website/src/content/docs/docs/using/work-in-a-session.mdx).

Ported from the browser UI's `e2e/docs-shots/work-in-a-session.spec.ts` (deleted in #3401; `git show
4aec73c6cdd8:e2e/docs-shots/work-in-a-session.spec.ts`), keeping its shot names, wording and placement. Every shot shows
the session the scenario opens; nothing is clicked except to show something, so no tab opens and no file downloads.
"""

from docs_shots import Callout, Shot


def nothing(bridge):
    """The open session as the engine's reset leaves it: its terminal, tabs and details bar, no layer open."""


def download_hover(bridge):
    """Show the hover the open session's terminal gives the `e2e/attach.spec.ts` path it printed, answered with the
    scenario's `terminal_files` size and full path.

    The path is a run of terminal cells, not an element the bridge can hover, so an agent build's
    `farhelm_agent::HoverStagedTerminalFile` action rests the hover on it (docs/gui-agent.md, "Scenario runs"). That
    hover lasts until the terminal next sees the pointer, so the pointer goes outside the terminal first; the capture's
    own search for a park spot keeps it outside (the terminal reacts to hover, so no park spot lies on it). The
    session's real directory is the run's private state; the hover's answer is the scenario's.
    """
    bridge.hover("text:Recently active")
    bridge.action("farhelm_agent::HoverStagedTerminalFile")
    bridge.wait_text("click to download")


SHOTS = [
    Shot(
        "header",
        nothing,
        # Not the whole bar: its copy fields stretch it to the pane's full width, far past the command's end.
        ["id:session-details-status", "id:session-details-command"],
        annotations=[
            # Above the bar, over the terminal's empty lower half: under it is only the window's edge.
            Callout("id:session-details-directory",
                    "Click to copy the session's directory, or next to it, the command it runs.", side="top", dx=40),
        ],
        max_width=930,
    ),
    Shot(
        "tabs",
        nothing,
        ["id:session-terminal-tab-agent", "id:session-terminal-add"],
        annotations=[
            Callout("id:session-terminal-add",
                    "Opens a shell in the session's directory, as another tab beside the agent.", side="bottom", dx=60),
        ],
        max_width=930,
    ),
    Shot(
        "download-hover",
        download_hover,
        # The header's Stop button keeps the hovered path above the display in the picture, and the header's buttons
        # above that, as the old shot had them.
        ["id:terminal-file-disclosure", "id:session-header-stop"],
        annotations=[
            Callout("id:terminal-file-disclosure",
                    "Hover a path to check it on the session's host: which host, the full path, and its size. Click to "
                    "download it.", side="right", dx=40),
        ],
        max_width=930,
    ),
]
