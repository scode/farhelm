# Recent options in Replace with are named Replace even when activated to fill only

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Recent setup options can announce Replace while activating the option only fills the form.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:120–147,273–330;
crates/gui/farhelm-gui/src/components/mod.rs:387–402;
crates/gui/farhelm-gui/src/components/option_row.rs:171–177,205–208;
crates/gui/farhelm-gui/src/launcher/recents_view_tests.rs:246–272;
f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6:crates/gui/farhelm-gui/src/launcher/view/recents.rs:258–271,283–320

An empty-list recent option says Replace even if activating the option only fills the form. LMC-09 independently
identifies the same accessible name and traces the predecessor: its focusable option could itself submit on Enter. The
redesign moved that Enter to the command field, and the option's callback now explicitly supplies keyboard=false; it
retained Replace in the accessible name even when not highlighted. The active recent now has a distinct Enter key cap.
This differs from LM-02/LMC-01, which attach the false cap to a typed result whose Enter only fills. The option label is
misleading but the option activation cannot itself delete. The reviewer suggested other; confidence confirmed for the
wiring and name, platform announcements unmeasured; ; Possible coverage: LMC-09 exact, typed-recent-false-replace-cue
separate.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-match-actions/replace-option-accessible-action`, with `launcher-matching-contract/LMC-09`.
