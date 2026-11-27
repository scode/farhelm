# The quick switcher's empty-field hint strip reads as fragments to screen readers

Reviewed commit: ab455003b87d87743e1a0dc83dfb0f39d213066a

## TLDR

A screen reader user who opens the quick switcher (Cmd+K) or the template list (Cmd+J) hears the hints under the empty
search field as loose pieces ("tl:", "search templates", "fix login bug", "⏎", "on no match starts a session with that
name"; in the template list an unnamed bolt image, "quick launch", the chord, "number keys", two chips and "…"). The fix
in #5001 gave the footer key strip one spoken sentence for exactly this reason but left this strip, which explains the
switcher's hidden commands (tl: and naming a new session), unlabelled.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t11b-dialogs-welcome` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/session_list/quick_switcher.rs:1295-1337 (tip)`

3df440f521 added header_hints (tip quick_switcher.rs:1295-1337), drawn as the DialogFrame heading_control detail
whenever the query is empty. It is a plain div with id quick-switcher-hints and no role or aria_label; its parts are
key_chip and chips_and_words fragments, and in Mode::Templates the first item starts with
command_glyph(Icon::BoltFilled) (components/command_surface.rs:116), an svg with no accessible name. ab455003b8
(quick_switcher.rs:1351-1357 at tip) added .role(Role::Label).aria_label("Up and Down to move, Enter to {enter}, Escape
to close") to footer_hints, citing that key caps alone read as fragments and that SPEC.md asks such text to reach
assistive technology (SPEC.md:3886, "Consent, warning and status text in the desktop app must be available to assistive
technology"). The launcher's own equivalent strips are labelled the same way (launcher/view/search/start.rs:446-450
navigation_hint, Role::Label + aria_label). The header strip carries more meaning than the footer (it is the only place
the switcher teaches tl: and the New-by-name behavior in words) and has the same fragment problem, so the fix is
incomplete. Also, ab455003b8 added no test: nothing asserts the footer strip's label or role (grep for 'Up and Down to
move' finds only the source), so a later restyle could drop it silently. Verify: read header_hints at the tip and the
accessibility node the test driver reports for quick-switcher-hints. Fix: give the strip Role::Label and one sentence
per mode (e.g. "Type tl: to search templates. Type a name and press Enter on no match to start a session with that
name." / "Quick launch with <chord>; number keys <1> to <0> start their templates."), and add a driver assertion on both
strips' labels in quick_switcher/tests.rs and template_list_tests.rs.

Suggested bucket: other — accessibility wording gap with no data, security or work impact.

Confidence: confirmed (absence of role/label and of a test is source-established; how a specific screen reader voices
the fragments is not observed).

proposed_drop: none

possible_cover: none known
