"""The Your first session page's shots (website/src/content/docs/docs/get-started/first-session.mdx).

Ported from the browser UI's `e2e/docs-shots/first-session.spec.ts` (deleted in #3401; `git show
4aec73c6cdd8:e2e/docs-shots/first-session.spec.ts`), keeping its wording where the desktop app still does the same
thing. The staged fleet already has a working session to show, so every shot photographs the app as the engine's reset
leaves it: nothing launches a session or changes a setting.
"""

from docs_shots import Callout, Highlight, Shot

OPEN = "fix flaky attach test"
WAITING = "migrate auth tokens"
UNREAD = "bench tokenizer"
SEEN = "Review terminal links"


def nothing(bridge):
    """The fleet as the engine's reset leaves it: the open session selected, its terminal beside the list."""


SHOTS = [
    Shot(
        "this-machine",
        nothing,
        # The rail runs the window's full height; its top three entries and the start of the list are the picture.
        ["id:host-rail-host-{host:local}", "id:host-rail-host-{host:gpu-1}"],
        annotations=[
            Highlight("id:host-rail-host-{host:local}", pad=2),
            Callout("id:host-rail-host-{host:local}",
                    "The Mac you are on is already a host, so sessions can start here straight away.", side="right"),
        ],
        max_width=900,
    ),
    Shot(
        "running-session",
        nothing,
        # The terminal fills the window's height and has no element per line, so the crop is capped a little below the
        # open session's row, which keeps the list's top and the terminal's first lines.
        [f"id:session-item-{{session:{OPEN}}}", "id:terminal-grid"],
        annotations=[
            # Over the terminal, beside the row, a little low so the terminal's first lines stay readable.
            Callout(f"id:session-item-{{session:{OPEN}}}",
                    "Your session in the session list. The mark beside it shows what its agent is doing: a filled "
                    "circle, pulsing, while it works.", side="right", dy=30),
            # The terminal's box is the whole pane, so no side puts a box inside it with the arrow pointing in. This
            # one sits over the list's lower rows, which this shot is not about, and points across into the terminal;
            # dy lifts it from the pane's middle, far below the crop, to the rows under the open session.
            Callout("id:terminal-grid",
                    "The agent's own terminal. Type your first prompt here, as you would in any terminal.",
                    side="left", dy=-450),
        ],
        max_width=930,
        max_height=360,
    ),
    Shot(
        "statuses",
        nothing,
        # The rows' container rather than the first and last rows: the app orders the list itself (running and waiting
        # first, then by when each session last started work), so which title comes first can change between runs.
        # Capped below the five rows, since the container fills the sidebar to the window's foot.
        ["id:session-rows"],
        annotations=[
            # No offsets: with the rows' order free to change, any nudge could push a box off its row, and the engine
            # already moves a box down when it would cover an earlier one.
            Callout(f"id:session-item-{{session:{WAITING}}}", "Waiting: the agent is asking you something.",
                    side="right"),
            Callout(f"id:session-item-{{session:{UNREAD}}}",
                    "A filled diamond: idle, and it finished since you last looked.", side="right"),
            Callout(f"id:session-item-{{session:{SEEN}}}", "A ring: idle, nothing new since you looked.",
                    side="right"),
        ],
        max_width=900,
        max_height=380,
    ),
]
