# Permission-switch Undo restores draft values with the wrong setup-source marker

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

Undo can restore the earlier launch settings and leave the reverted template marked as their source.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view.rs:1017–1049;
crates/gui/farhelm-gui/src/launcher/view/escalation.rs:70–109,167–179;
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:263–265,347–356; pinned
1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/view.rs:1025–1058;
crates/gui/farhelm-gui/src/launcher/view/recents.rs:93–125

Undo can leave the launcher naming a template after removing its edits. SPEC.md:1430–1435 says the marker belongs to the
row that filled the form, and SPEC_impl.md:8943–8950 calls it the last applied template with edits or the setup that
filled it. SPEC.md:1107–1115 and SPEC_impl.md:8969–8971 define Undo as restoring the pre-switch or pre-batch draft, so
the corresponding source is the earlier marker (or None). Capture that single view marker at the same point as the
draft/notice being saved, before apply_now replaces it, and restore it on Undo. Do the same for a hand switch: a later
successful template batch with no agent switch leaves its prior escalation banner alive, so restoring only when the
banner was created by a template misses a possible stale marker. A recent fill explicitly retires the old banner before
setting its own marker. Keep keyboard highlight independent and do not assign field-by-field ownership or derive launch
settings from the marker. The replay contract in SPEC_impl.md:8960–8971 forbids using provenance to drive hand-edit
behavior, while the view marker is explicitly required. The current code restores request fields and notice but not the
marker; this is a display problem. The reviewer suggested other. Confidence: confirmed by source.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-options/LO-04`, with
`launcher-matching-contract/LMC-15`.
