# A visible Settings Save can replace the Tensorlake key with a draft from another page

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A stale Settings footer can save the wrong page’s draft and overwrite the stored Tensorlake key.

## Details

The Settings footer decides whether it can Save using the section it draws, but the handler calls `save_page` and reads
the currently selected page again (`crates/gui/farhelm-gui/src/settings/editing.rs:283–337`). Settings updates the
selected section before presenting the new frame (`crates/gui/farhelm-gui/src/settings.rs:870–875`). A previous-frame
click can therefore save the other page’s draft, including its value for the one stored Tensorlake key. Settings cannot
recover the prior saved key (`SPEC.md:2449–2458,2490–2496,2520–2524`). The Discard handler at
`settings/editing.rs:319–321` also reads the live page and can discard the wrong unsaved draft after a page change. Bind
both footer actions to the page and applicable draft shown when they were rendered. Reject stale actions when that page
or draft changes. The pinned regression tests exercise direct Save and Discard on the current page and do not challenge
this presentation dependency.

Confidence: confirmed by inspection of the navigation, pointer dispatch and save destination. Depends on input reaching
the dialog before its next draw.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `listCardEditors/l4`, `recipeCards/l4`.
