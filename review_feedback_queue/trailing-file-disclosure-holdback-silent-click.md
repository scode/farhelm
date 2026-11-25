# A file card held back over the pointer leaves a silent click with no explanation

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When a file's details card in the terminal is too tall to fit beside the pointer (a long path in a small window), it
flashes for one frame and disappears, the underline and pointing hand go with it, and clicking the path does nothing at
all: no download, and no message telling the user to enlarge the window. The implementation spec promises a "not
visible" refusal in exactly this case. Each move to another character of the same path repeats the flash.

## Details

The silent click is pre-existing: at the parent of 3ed37d4c20
(`git show 3ed37d4c20~1:crates/gui/farhelm-gui/src/terminal/attached.rs`, lines 3119 and 3151) the `!current` branch
deferred `files.follow(file_hover)` with no disclosure, and at line 4051 `file_ready` used `files.ready_for`, false
after `follow(None)`. 3ed37d4c20 keeps the silent click, now also removes the underline and pointing hand, and adds a
SPEC.md sentence this case contradicts.

Locations: `crates/gui/farhelm-gui/src/terminal/attached.rs:2804-2812` (file branch of `link_released`), `:2916-2919`
(`file_hover_ready`), `:2922-2931` (`follow_files`), `:4090-4100` (press records `file_disclosure`), `:2429-2432`
(hold-back in the hover listener), `:520-524` (`refusal_waits_for_hover` doc); SPEC_impl.md:3556-3564.

The hover listener (`attached.rs:2429`) sets `refusal_waits_for_hover` when the drawn display covers the resting
pointer. With that flag set, `file_hover_ready` returns false (`:2918`), so `disclosed_link` yields no file disclosure,
the grid's `path_hover_ready` goes false (no underline, and no pointing hand via `points_at_link`), and `follow_files`
passes None (`:2927`) so `FileLinks` drops the hover and its check. A press then records `file_disclosure = None`
(`:4092-4100`: the filter requires `file_hover_ready`), and `link_released` returns early at
`let Some(shown) = ... else { return; }` (`:2804-2811`) before `FileLinks::clicked` can map press visibility
`Err(NotVisible)` to the refusal words. Each move to another cell of the same link clears the hold-back, re-issues a
host stat (`follow` sees tracking None), repaints the card for a frame and holds it back again.

SPEC_impl.md:3561-3562 says the hold-back exists "so a click there finds no display and gets the \"not visible\"
refusal", and the field doc at `attached.rs:522-523` says "with no panel shown, a click there gets the \"not visible\"
refusal"; both are false for file links. SPEC.md's web-link bullet (SPEC.md:3645-3646) says such a held-back hover's
click "is refused the same way". The file bullet added by this commit says "A click on a file whose disclosure painted
but is clipped or covered is refused with a visible explanation to move the pointer or enlarge the window and hover
again", and the held-back case is a disclosure that painted but could only be shown over the link.

Verify by reading the listed lines in order. Fix: when `refusal_waits_for_hover` was set by `display_covers_pointer` for
a file link that had a painted, ready disclosure, have the release raise `Hidden::NotVisible.file_words()` (for example
keep the hover's check alive across the hold-back and track a "held back" state that `link_released` turns into a
refusal), or amend SPEC_impl.md and the field doc to say file links stay silent.

Bucket: other (no data or security consequence; the user cannot download a long-named file in a small window and is not
told why). Not highest: the failure is closed, and nothing transfers.

Confidence: confirmed from source.

Possible cover: SPEC.md's file bullet "A click cannot transfer or open a file while the full hover disclosure does not
fit in the window" covers the no-transfer outcome but not the missing explanation.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g09 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
