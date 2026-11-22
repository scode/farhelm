# Quick-launch fallback loses the source marker after a template filled the draft

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

A launcher opened for review or correction by quick launch can omit the source marker for the template that filled it.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/templates/dispatch.rs:414–448;
crates/gui/farhelm-gui/src/launcher/view.rs:298–306,450–476; pinned 1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/launcher/view.rs:301–309,450–479,1039–1058;
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:267–269,352–362

New can open with template choices filled in while its persistent source chip and row marker are missing.
SPEC.md:1311–1317 distinguishes an incomplete or refused launch that reveals the applied draft, and SPEC.md:1430–1439
together with SPEC_impl.md:8943–8950 promises the form's persistent source marker. apply_from_store already returns the
last applicable template with edits, but on_the_page carries only the agent notice and optional banner into Prepared;
neither its successful Show branch nor its successful Launch branch preserves the marker. Store the returned marker on
the same Launcher as soon as application succeeds, before either branch starts or reveals it, so evaluation can still
happen unseen and a later refusal or question reveals the already-marked draft. A rejected batch showed New on its
untouched prefill, so it has no applied source to mark. Successful direct launches that never draw need no visible
marker. The incomplete banner names the template temporarily and may expire or be dismissed on edit, so it does not
fulfill the persistence promise. The reviewer suggested other. Confidence: confirmed by source at the introducing commit
and pinned tip. Possible coverage: overlaps LO-04 only in the view state that stores the marker; the missing assignment
and stale Undo assignment are separate paths.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-options/LO-10`.
