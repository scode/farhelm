# Restart and recovery text escape host setup errors without isolating their direction

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

Restart and recovery text escape host-supplied hook errors without isolating their text direction.

## Details

**other.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/session_list/restart.rs:104-107,134-140;
crates/gui/farhelm-gui/src/session_pane/card.rs:292-320,547-553;
crates/gui/farhelm-gui/src/session_pane/header.rs:238-249,366-379,1493-1517

The Restart tooltip and menu and the recovery card put the host's hook setup error in the same text element as Farhelm's
prose without isolating the error's direction. Pinned SPEC_impl.md:1161-1163 says the drawing element “escapes hidden
characters and isolates text direction”; :1173-1185 explains per-value isolation and requires host-sentence hover help.
At bb31959c2f22c96bbbb72da2e60eee4a0b288a6b, the supervisor sends the same underlying install error as the per-run
Failed reason and the persistent-install notice (crates/farhelm-supervisor/src/service/core.rs:14771-14788).
crates/farhelm-proto/src/lib.rs:959-981 uses text::escaped and flattens the reason into app wording.
text.rs:71-103,122-132 performs the escaping and adds no isolate. The new consumers use a String as the card child or as
app tooltip/menu text at the cited lines; the split menu's command-launch Restart with explanation also uses
restart_blocker through launcher/entry.rs:369-385,394-403. components/mod.rs:155-167 distinguishes the app tooltip from
HoverHelp::sentence. The notification bell already draws the notification body as its own host-text element
(notices_bell.rs:754-781; host_text.rs:189-193), so the body cannot reorder surrounding GUI labels. That is isolation of
the whole supervisor-authored entry, not per-value isolation of the reason inside the prose formatted by
notifications.rs:284-292. The launcher hook-setting save refusal is separately isolated per host value by
HostSentence::host at launcher/view/hook_question.rs:239-245 and host_text.rs:425-439. Carry the raw reason separately
to the affected GUI renderers and isolate it there. The violation and pathways are source-confirmed at the reviewed
commit and pinned tip 1a53d9d35889d7dde96e31320cc1681dd013737d; exact visual behavior of an individual error is
unverified. The reviewer suggested other because the text is diagnostic and does not control action admission.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-session/HS-05`.
