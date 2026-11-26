# The Rename dialog dropped the only escaped view of the session title

Reviewed commit: fb80d7dd616f6b86f0fb258dab43354af58a0c35

## TLDR

An agent or the command line can set a session title that contains zero-width or right-to-left override characters. The
session list shows those characters visibly escaped. Before this change the Rename dialog also showed the current title
escaped above the field. Now it shows only the editable field, which holds the raw title, so the hidden characters are
invisible (and an override can reorder the text) exactly where the user edits it. A user can save a title that still
carries characters they cannot see, or fail to notice that the title differs from what it appears to say.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t11d-dialogs-welcome` `l1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept; it would move up only if a spoofing route through Rename were shown to mislead a
security decision. The reviewer proposed a possible drop citing SPEC_impl.md "Host-supplied text"; the independent drop
check DISAGREED: that text says which value is edited, not that no escaped view is needed, and SPEC.md still requires
escaping wherever such text appears. No other coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/session_list/rename.rs:574`

Revision (turn 2): the unverified premise was checked in the kit's source, and confidence moves to confirmed. No later
commit in the range restores the escape.

At the parent commit, rename.rs drew `host_text::element(&self.current)` as `.rename-current-title` above the field
(module docs: "The current title above the field always follows, escaped"). fb80d7dd61 removed that line (snapshot hunk
@@ -552,31 +574,33 @@) and the module-doc sentence. `current` is now kept only for naming mode's unchanged-save check
(rename.rs:389 doc at the commit). The field is seeded with the raw value (the parent's tests.rs:164 pins
`field == "fix\u{200B}build"`, raw, unescaped). The new context line (`session_context`, rename.rs:684-755 at the
commit) shows agent, host, directory and status but not the title. Nothing else in the dialog presents the title
escaped. The kit's editor has no escape path. gpui-base 0.9.1's only invisible-character rendering is
`layout_whitespace_indicators` (src/input/base/element.rs:1502-1550), which marks spaces and tabs only, and only when
`show_whitespaces` is set. Farhelm never sets that flag (no `show_whitespaces` in crates/gui). The field therefore
shapes the raw string through GPUI's text system, where U+200B has no width and direction controls take effect. The
later commit 21310c007 touches rename.rs only to factor the Save key chip into `return_key_content`; the tip has no
`rename-current-title` or escaped draw of `current`. SPEC.md "Confirmed 2026-10-10" (tip ~line 5759) says text a host
supplied that the GUI shows outside a terminal is escaped so invisible and direction-changing characters are visible
"wherever it appears". A comparable editor in SPEC.md keeps them visible: template messages with invisible formatting
"are shown escaped and read-only until explicitly cleared" (SPEC.md ~1277). Verify: rename a session from the command
line to a title containing U+200B or U+202E, then open Rename in the GUI. The row shows the escape and the dialog does
not. Fix: restore an escaped presentation of the current title whenever the escape table changes it, for example in the
header's detail slot beside the context line. Alternatively, mark the characters in the field.

Suggested bucket: other. This is a display-only spec mismatch with no wrong target and no data loss. It would move up
only if a spoofing route through Rename were shown to mislead a security decision.

Confidence: confirmed (source-established: the escaped line is gone, and neither Farhelm nor the kit escapes the field's
text). How a given platform shaper draws each format character was not observed.

proposed_drop: possibly, on SPEC_impl.md "Host-supplied text". There the raw value is "used for editing and operations",
which can be read as accepting a raw editing field. The same commit's SPEC.md edit also deliberately removed the current
title from the Rename description. Neither text says the dialog may show a host title with no escaped spelling at all.

possible_cover: none
