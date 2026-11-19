"""The Launch templates page's shots (website/src/content/docs/docs/using/launch-templates.mdx).

Ported from the browser UI's `e2e/docs-shots/launch-templates.spec.ts` (deleted in #3401; `git show
4aec73c6cdd8:e2e/docs-shots/launch-templates.spec.ts`), keeping its shot names, callout wording and placement where the
desktop app still does the same thing. The templates shown are the scenario's, which the engine stores on the helm
before any shot runs (docs/docs-shots/scenario.json5). Nothing here saves or deletes a template or launches a session:
Templates' editor is opened and photographed, templates are applied in the launcher, which only fills in its choices,
and the save-as-template panel is filled in and left unsaved.
"""

from docs_shots import Callout, Shot


# --- Steps ------------------------------------------------------------------------------------------------------------


def templates_button(bridge):
    """The session list's heading as the engine's reset leaves it, with Templates beside + (New session)."""


def open_templates(bridge):
    """Open Templates and wait for the staged list's first row, so the picture never shows the list still loading."""
    bridge.click("id:templates-button")
    bridge.wait("id:template-row-api-repo")


def edit_form(bridge):
    """Open codex-deep in the editor, the template that sets an agent, a model and an effort, without changing it.

    Waiting for its Effort row proves the editor shows the saved template's fields rather than the empty editor.
    """
    open_templates(bridge)
    bridge.click("id:template-open-codex-deep")
    bridge.wait("id:templates-field-effort")


def apply(bridge):
    """Show the launcher's tl: suggestions from the scenario's real template catalog, without choosing one."""
    bridge.click("id:new-session-button")
    # New session hands focus to Search. Typing there avoids a redundant click while the dialog opens.
    bridge.type_into_focused("id:launcher-search", "tl:")
    # The catalog is name-ordered: api-repo precedes codex-deep in this scenario's unfiltered tl: results.
    bridge.wait("id:launcher-search-option-1")


def applied(bridge):
    """Apply codex-deep and then api-repo in a new launcher, the stacking case the page describes.

    One template picks the agent and how it runs, the next picks where, and the launcher's footer then shows both.
    Each search narrows to one template, so its first option is the one meant.
    """
    bridge.click("id:new-session-button")
    bridge.type_into_focused("id:launcher-search", "tl:codex")
    bridge.wait("id:launcher-search-option-0")
    bridge.click("id:launcher-search-option-0")
    # The choices line's drawn tail, one text run, shows codex-deep's effort took. The target line cannot serve as the
    # oracle for api-repo: its harness, host and folder are separate runs, and `wait_text` matches one run at a time.
    bridge.wait_text(" · Effort: High · Permissions: ")
    bridge.type_into_focused("id:launcher-search", "tl:api")
    bridge.wait("id:launcher-search-option-0")
    bridge.click("id:launcher-search-option-0")


def save_as_template(bridge):
    """Fill in the launcher's save-as-template panel from a setup made by hand, stopping short of Save.

    Codex and High are chosen by hand so the panel has something checked; the launcher's starting host and directory
    are offered unchecked. The name field is clicked before it is filled, which makes its focus a pointer focus: focus
    gained from the keyboard would show the field's hover help over the panel.
    """
    bridge.click("id:new-session-button")
    bridge.click("id:launcher-harness-Codex")
    bridge.wait("id:launcher-effort-High")
    bridge.click("id:launcher-effort-High")
    bridge.click("id:launcher-save-template")
    bridge.wait("id:save-template-panel")
    bridge.scroll_into_view("id:save-template-save")
    bridge.click("id:save-template-name")
    bridge.fill("id:save-template-name", "codex-high")


# --- Shots ------------------------------------------------------------------------------------------------------------

SHOTS = [
    Shot(
        "templates-button",
        templates_button,
        ["id:templates-button", "id:new-session-button"],
        annotations=[
            # Below the button: from the right, the arrow would land on the + (New session) button beside it.
            Callout("id:templates-button", "Opens Templates, where you create, edit and delete templates.",
                    side="bottom", dy=60),
        ],
        max_width=930,
    ),
    Shot(
        "panel",
        open_templates,
        ["id:templates"],
        # The dialog's static title has no hover reaction and sits clear of every row.
        park="id:templates-frame-title",
        annotations=[
            # The empty editor gives this callout room without hiding the template rows.
            Callout("id:templates-new", "Start a new template here, or choose a row below to edit one.", side="right",
                    dy=35),
        ],
    ),
    Shot(
        "edit-form",
        edit_form,
        ["id:template-row-codex-deep", "id:templates-editor"],
        park="id:templates-frame-title",
        annotations=[
            # Both callouts sit in the list's empty space below its rows, so the selected template stays visible beside
            # the editor's fields, removal buttons and footer.
            Callout("id:templates-switch",
                    "Applying this template opens the Agent tab with Codex, this model and this effort.", side="left",
                    dy=220),
            Callout("id:templates-add",
                    "Add only the choices this template should make. Fields not shown stay as they are.", side="left",
                    dy=30),
        ],
    ),
    Shot(
        "apply",
        apply,
        ["id:launcher-search", "id:launcher-search-results"],
        # The static title has no hover help or reaction and stays above the search results, clear of their popover.
        park="id:launcher-frame-title",
        annotations=[
            Callout("id:launcher-search", "tl: lists your templates. Type part of a name to narrow the list.",
                    side="left"),
            Callout("id:launcher-search-results",
                    "Choosing one makes its choices in the launcher, just as if you had made them by hand.",
                    side="left", dy=60),
        ],
        # The old shot's width: the launcher's full search row would make the picture far wider than the docs' column,
        # and the results' right half is empty.
        max_width=900,
    ),
    Shot(
        "applied",
        applied,
        ["id:launcher-summary-target", "id:launcher-summary-choices", "id:launcher-launch"],
        park="id:launcher-frame-title",
        # Both lines stretch to the footer's buttons, past their words, and an arrow from above ends level with its
        # box's centre. So each box is shifted until its arrow lands on words: the first on the target line's host, the
        # second at the end of the choices line, and the second sits wholly above the first, its arrow passing beside
        # it, since side by side the two boxes would overlap and the engine would push the second down over the
        # footer's buttons.
        annotations=[
            Callout("id:launcher-summary-target",
                    "After codex-deep, then api-repo: Codex from the first, build-box and ~/src/api from the second.",
                    side="top", dx=-127),
            Callout("id:launcher-summary-choices",
                    "The model and effort codex-deep set. Change anything before you launch.", side="top", dx=42,
                    dy=-150),
        ],
    ),
    Shot(
        "save-as-template",
        save_as_template,
        ["id:save-template-panel"],
        park="id:launcher-frame-title",
        annotations=[
            # Down the left of the launcher, each level with what it explains.
            Callout("id:save-template-name", "A new name. One already in use is refused.", side="left", dy=-20),
            Callout("id:save-template-field-agent",
                    "Checked: the choices you made for this launch. Tick or untick each one; the template keeps only "
                    "what is ticked.", side="left", dy=20),
            Callout("id:save-template-save",
                    "Saves the template, closes the launcher and opens the template in Templates. Nothing is launched.",
                    side="left", dy=110),
        ],
    ),
]
