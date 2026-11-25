# Antigravity release-notes recapture parks the pointer on a field with a tooltip

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When the maintainer recaptures the v0.42.0 Antigravity release-notes picture, the capture leaves the mouse pointer
resting on the Conversation text field. That field has hover help, so the picture shows its tooltip ("Conversation:
start Antigravity on this conversation, this start only") drawn above the field, over the Conversation label and toward
the Antigravity choice the callout points at, instead of the clean launcher form. This is worked out from the capture
scripts and the GUI source, not seen in an actual capture.

## Details

Concerns 0f6d332c6f (#4809). Main reference `scripts/docs-shots/release-notes.py:159`
(`park="id:launcher-conversation-row"` for shot v0-42-0-antigravity).

The fix replaced the removed park target `id:launcher-frame-title` with `id:launcher-conversation-row`.
`scripts/docs_shots.py:428-431` passes it as `--park`; `park()` in `scripts/gui-agent.py` (`:1858-1877`) then runs
`bridge hover <target>` with no screening (its docstring: "`--park` uses the shot's chosen spot directly; the bridge
still refuses a covered target"). The automatic search's `park_targets` filter (`gui-agent.py:1666-1673`), which rejects
spots whose centre lies in a `hover` or `tooltip` element because a tooltip once leaked into a picture, is bypassed.

The bridge aims at the element's centre. `row()` (`crates/gui/farhelm-gui/src/launcher/view.rs:1575-1590`; the
conversation row at `:127`) is a flex column: a 12 px label (TEXT_SM), a 4 px gap (LABEL_GAP), then the control, a Kit
TextInput 32 px tall (`text_input.rs:646`) spanning the column's full width and wrapped by `with_hover_help`
(`crates/gui/farhelm-gui/src/components/mod.rs:368`) with `HoverHelp::tooltip`
(`crates/gui/farhelm-gui/src/launcher/view/more_options.rs:47-60`, tooltip at `:56-59`). The centre lies inside the
input whenever the label's line height L satisfies (L+36)/2 > L+4, that is L < 28 px. GPUI's default line height is phi
(`vendor/gpui-pre/src/style.rs:494`), about 19.4 px for 12 px text, and gpui-component's Root sets none, so the centre
is about 4 px inside the field's top edge, mid-width. Ordinary help "sits above its control, or below when space runs
out" (`components/mod.rs:150-165`), which is inside the shot's crop regions [launcher-harness-row,
launcher-conversation-row].

Timing: the tooltip shows `HOVER_DELAY` = 300 ms after the hover (`crates/gui/farhelm-gui/src/layers/tooltip.rs:15,74`).
`park()` then calls `settle()`, which returns only after the tree has been unchanged for 0.3 s (`IDLE_STILL_SECONDS`,
`gui-agent.py:1592-1594`) measured from its first read, which comes after the hover. The tooltip's appearance lands
inside the still window, resets it, and `settle` returns with the tooltip drawn; the screenshot follows. The bridge's
`hover` does drive GPUI hover reactions: `docs_shots.py:247-248` documents a hover step "for a shot of what hovering
shows".

The shell shot keeps `LAUNCHER_PARK` (the Destination heading, a plain section label, `view.rs:1704-1710`), and the
comment at `release-notes.py:158` explains only that Destination can scroll away. The new test
`test_selected_launcher_release_recaptures_use_current_controls` (`scripts/test-docs-shots.py`) asserts only the park
string, so it cannot catch this.

Verify by reading the geometry and timing above, or by checking the retained `<page>-<shot>-full.png` of a recapture.
Fix: park on a non-reactive element that stays visible after the disclosure opens (for example give the conversation
row's label its own id, or use the Agent tab's section heading), or have `--park` reject a target whose centre lies in a
hover or tooltip element, reusing the check in `park_targets`.

Bucket: other (maintainer docs tooling; release shots change only on deliberate recapture, and the maintainer reviews
the picture before publishing). Not highest: there is no security or data-loss route, and the impact stays in maintainer
tooling.

Confidence: confirmed by source analysis (geometry, placement, delay and settle timing all read at the tip); not
observed in an executed capture.

Possible cover: none known; SPEC.md and SPEC_impl.md do not address docs-shot park spots.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g07 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
