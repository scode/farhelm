"""The Launch templates page's shots (website/src/content/docs/docs/using/launch-templates.mdx).

Ported from the browser UI's `e2e/docs-shots/launch-templates.spec.ts` (deleted in #3401; `git show
4aec73c6cdd8:e2e/docs-shots/launch-templates.spec.ts`), keeping its shot names, callout wording and placement where the
desktop app still does the same thing. The templates shown are the scenario's, which the engine stores on the helm
before any shot runs (docs/docs-shots/scenario.json5). Nothing here saves or deletes a template or launches a session:
Templates' editor is opened and photographed, templates are applied in the launcher, which only fills in its choices,
and the save-as-template panel is filled in and left unsaved.
"""

from docs_shots import Callout, Shot

# The command field occupies the old title's spot. Keep its focus and move the pointer clear of its help.
LAUNCHER_PARK = "id:launcher-destination-heading"


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

    One template picks the agent and how it runs, the next picks where, and the launcher form shows both.
    Each search narrows to one template, so its first option is the one meant.
    """
    bridge.click("id:new-session-button")
    bridge.type_into_focused("id:launcher-search", "tl:codex")
    bridge.wait("id:launcher-search-option-0")
    bridge.click("id:launcher-search-option-0")
    # The empty launcher has no Codex effort row. Wait for the first template's agent controls before stacking.
    bridge.wait("id:launcher-effort-High")
    bridge.type_into_focused("id:launcher-search", "tl:api")
    bridge.wait("id:launcher-search-option-0")
    bridge.click("id:launcher-search-option-0")
    bridge.wait_text("~/src/api", target="id:launcher-folder")


def save_as_template(bridge):
    """Fill in the launcher's save-as-template panel from a setup made by hand, stopping short of Save.

    Choose the same Codex model as the staged codex-deep template so the model catalog has answered before choosing
    High. The launcher's starting host and directory are offered unchecked. The name field is clicked before it is
    filled so that keyboard-focus help does not appear over the panel.
    """
    bridge.click("id:new-session-button")
    bridge.wait("id:launcher-search")
    bridge.click("id:launcher-harness-Codex")
    # Typing opens the model choices even in the headless window, where focus events do not fire.
    bridge.fill("id:launcher-model", "sol")
    model = "id:launcher-model-option-Codex-gpt-6.2-sol"
    bridge.wait(model)
    bridge.click(model)
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
        park=LAUNCHER_PARK,
        annotations=[
            Callout("id:launcher-search", "tl: lists your templates. Type part of a name to narrow the list.",
                    side="left"),
            Callout("id:launcher-search-option-0",
                    "Choosing one makes its choices in the launcher, just as if you had made them by hand.",
                    side="left", dy=60),
        ],
        # Keep the query, the choices beside the form and their notes together as a compact crop.
        max_width=900,
        max_height=320,
    ),
    Shot(
        "applied",
        applied,
        ["id:launcher-provenance", "id:launcher-destination-section", "id:launcher-model-row", "id:launcher-effort-row"],
        park=LAUNCHER_PARK,
        annotations=[
            Callout("id:launcher-host",
                    "api-repo sets build-box and ~/src/api.", side="left"),
            Callout("id:launcher-effort-row",
                    "codex-deep sets Codex, the model and effort. Change any choice before you launch.",
                    side="bottom", dy=50),
        ],
    ),
    Shot(
        "save-as-template",
        save_as_template,
        ["id:save-template-panel"],
        # Scrolling the Save panel into view can take Destination above the viewport.
        park="id:save-template-choices-heading",
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
