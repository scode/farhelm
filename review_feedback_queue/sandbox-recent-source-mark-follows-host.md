# Changing the host removes the source mark from a still-listed sandbox setup

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Changing the host can remove the mark from a still-listed sandbox setup while the form still names it as the source.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:48–54,267–269,319–325,352–363;
crates/gui/farhelm-gui/src/launcher/view/recents.rs:48–66,99–125;
crates/gui/farhelm-gui/src/launcher/view/history.rs:175–195;
crates/gui/farhelm-gui/src/launcher/recents.rs:96–113,155–168,252–265;
crates/gui/farhelm-gui/src/launcher/view/host_picker.rs:311–324; crates/gui/farhelm-gui/src/launcher/draft.rs:986–1015;
SPEC.md:1429–1436; SPEC_impl.md:8945–8949;
14fda80233aa69231c0b44431fc950764fbcdfd1:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:44–50,263–265;
crates/gui/farhelm-gui/src/launcher/view/recents.rs:121–124

After a sandbox recent fills New, picking a different ordinary host can remove the source mark from that same recent row
while the form still says ‘From a recent setup.’ In the reachable case with that sandbox setup still among the top three
(e.g. the single matching recent), it comes from helm-wide environment history, keeps its own recipe and place, and
remains eligible in folder mode. The fill saves SetupSource::Recent(old ordinary host, setup_key). A host pick changes
the draft host but is only a hand edit and leaves source set. Drawing the identical environment row then compares
against SetupSource::Recent(new ordinary host, same setup_key), so it loses the provenance selection. The spec requires
the row that filled the form to remain marked until Reset choices or another fill. Use the environment’s own setup key
for environment provenance, retaining host authority for ordinary host recents. This affects display after a valid
manual edit, and supplies no unintended submission. Distinct from LMC-15’s Undo and from history rebinding. The reviewer
suggested other; confidence confirmed from source; ; Possible coverage: no exact duplicate identified. The original
assigned revision introduces both the host-qualified source record and the equality used for the applied row. The normal
move to Existing host changes the destination and host without replacing agent choices or the source mark record (pinned
draft.rs:1310–1329,1363–1377; view.rs:2514 renders the retained badge).

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-match-actions/environment-recent-provenance-follows-unrelated-host`.
