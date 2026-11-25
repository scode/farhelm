# SPEC.md says a file click must be released over the painted card, which the app does not require

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

The product spec now says a click on a terminal file downloads only if it is pressed and released over the file's
details card. In the shipped app the press itself closes that card, and the release is accepted while the card is gone.
A reader trusting the spec would believe the card must still be on screen when the button comes up, which is not what
the app checks, and a maintainer who changed the code to match the sentence would break every file click.

## Details

Concerns 5fd36869f5. SPEC.md:3708-3709 (terminal file-path bullet): "Only a click pressed and released over the same
painted disclosure, with the link text unchanged since that hover, transfers or opens the file."

Code: `crates/gui/farhelm-gui/src/terminal/files.rs:207-243` (`FileLinks::clicked`),
`crates/gui/farhelm-gui/src/terminal/attached.rs:2803-2823` (`link_released`), `attached.rs:2885-2890`
(`Disclosure.pressed` doc), `attached.rs:4092-4101` (`LinkPress.file_disclosure`).

The press records the disclosure's generation only when it has painted and is ready (`attached.rs:4092-4101`), and a
press in the terminal closes the popover: "A press closes it (the layer manager closes a popover on any press outside
it) and it comes back once the press ends" (`attached.rs:2885-2890`; the doc of `Files::click` in `files/tests.rs` says
the same). At release, `link_released` requires only that the same Disclosure record (same generation) is still current
(`attached.rs:2806-2812`) and passes its `disclosure::visibility` as `released`. `FileLinks::clicked` downloads on
`(Ok(()), _)` unless the release saw Covered, with the comment "At release an observed cover still refuses the click,
though a hidden panel is normal after a press closes it" (`files.rs:209-210`, `222-227`), and admits
`(Ok(()), Err(NotVisible))` (`files.rs:222-233`). So what is required is: pressed while the same painted disclosure was
fully visible, released while that same record is still current (not replaced), and not covered at release. Not
"released over" a painted disclosure.

The commit message paraphrases the new sentence as "the press and the release must both land while the card is on
screen, the same card both times", which shows its own author reads it that way, and that reading contradicts the code.
A maintainer reconciling code to this sentence could add an on-screen requirement at release, which would refuse every
file click (the card is always closed at release). SPEC_impl.md:3484-3485 states the shipped rule accurately ("A click
requires a painted disclosure at press and the same disclosure and ready check at release"), so the two specs now
disagree.

Fix: reword SPEC.md to the rule the code enforces, adopting SPEC_impl.md's wording, for example "Only a click pressed
while that hover's disclosure is painted and fully visible, and released before the disclosure is replaced, with the
link text unchanged since that hover, transfers or opens the file."

Bucket: other (specification wording that misdescribes the release condition; no user-facing malfunction today). Not
highest: a documentation inaccuracy with no security or data-loss route.

Confidence: confirmed from source (the press closes the popover, and `clicked` admits a hidden panel at release).

Possible cover: none known.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g10 record f2; claude-opus-5-5
reviewer, source analysis only, nothing executed.
