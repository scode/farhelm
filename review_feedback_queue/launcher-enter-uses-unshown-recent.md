# Search Enter can take a recent shell setup that was not highlighted

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

While recents load, Enter can fill a shell setup that differs from the highlighted one.

## Details

**high**. Confirmed by source and contract inspection.

Confirmed ordering defect by source. No runtime observation. View/history.rs:109–143 and :152–174 deliver host and
environment history independently. A matching shell recent from the first reply can already be visible while the other
read is outstanding. A second reply stores the merged input and calls reset_search_highlight before notifying the view.
Even where the old row remains in the same position, this resets an arrow-moved highlight to the default. `recent:` uses
ordered ranked results (search.rs:683–703), with default index zero (search.rs:752–784). View/search.rs:121–126 changes
the index immediately, :541–553 and :589–620 separately draw the row and capture the pointer target, but :176–196
re-queries live rows on Enter instead of taking the presented target. This repository explicitly accounts for GPUI
dispatch before presentation at prompt.rs:1149–1162. In scope because shell history now uses the same editable recent
rows. The keyboard mechanism itself predates the landing. SPEC.md Creation :1110–1123 explicitly calls for the
`visible highlighted row`; :1354–1358 says a recent fills all choices and destination. Estimated bucket high for wrong
setup/destination in the documented two-Enter workflow. The first Enter is a fill, so a user can notice before
submitting. No established loss or security consequence. Possible_cover: none. No subsequent change in these launcher
files at tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-launcher/f5`.
