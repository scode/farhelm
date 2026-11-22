# Muted waiting dots bypass the ringless status-tooltip path

Reviewed commit: 2dc0555c3db4a6d776b6ca5a9cc20ca6dc068f90

## TLDR

On a muted session waiting on a connected host, hovering the waiting dot shows the row summary. If the summary fits
beside the sidebar, its notch and ring identify the row's open control. The separate Unmute bell still has its own
action help.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/session_list/row.rs:373`.

SPEC.md:2845–2855 requires live dots to explain their status and adds action words only for clickable dots.
SPEC.md:3033–3044 keeps the waiting dot and separate bell in this case. status.rs:307–318 and :345–353 retain a live
waiting badge during mute. mute.rs:120–131 and row.rs:373–377 route the dot to seen::dot. seen.rs:98–104 and :142–149
return the raw Live::mark SVG because only the bell is actionable. status.rs:177–193 registers no tooltip owner for that
SVG. The ringless behavior in components/status_mark.rs:120–125, components/mod.rs:295–303, and layers/beside.rs:446–459
applies when a StatusMark registers help. The dot never reaches that path. The enclosing ListRow at row.rs:361 and
:487–491 owns the help at that pointer position, and the Unmute bell at mute.rs:152–172 is a separate MarkToggle. The
patch therefore neither introduces the missing dot help nor supplies it. compact_tests.rs:349–378 checks bell layout and
row wording without testing the dot's own help. Give the non-clickable waiting dot a StatusMark owner with the status
and mute wording so its own beside help gets the notch alone. Bucket: other, desktop status help. Confidence: confirmed
by pinned source and existing fixture assertions. No runtime observation. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `status/f11`.
