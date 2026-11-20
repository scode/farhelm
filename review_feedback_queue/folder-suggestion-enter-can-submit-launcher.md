# Enter on a folder suggestion can submit the launcher

Reviewed commit: 959cde41e6d525db005de9da5b16862283cb7da5

## TLDR

An arriving folder-history update can turn Enter on a highlighted folder suggestion into a launcher submission. In
Replace with, a successful submission can retire the source session even though Enter was meant to choose a folder.

## Details

The history response clears the active folder suggestion before the changed frame is presented. The Enter handler reads
the current suggestion and can fall through to the launch action. The existing Replace guard covers the source state and
permission to retire it. The key handler also needs to establish that the action the user saw was Submit.

At reviewed tip `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see
`crates/gui/farhelm-gui/src/launcher/view/history.rs:113–144`,
`crates/gui/farhelm-gui/src/launcher/view/folder_list.rs:211–225`, and
`crates/gui/farhelm-gui/src/launcher/view.rs:823–835`. `SPEC.md:1353–1360` assigns Enter to the shown folder suggestion.
The in-range text-input aliases reach the same action. The successful Replace consequence is documented in
`SPEC.md:2064–2092`.

Suggested bucket **highest** for the established source-session retirement path on successful Replace. Verified through
source by g019/l7 and g041/l3 and l8. Timing was not observed at runtime.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g019/l7, g041/l3, l8.
