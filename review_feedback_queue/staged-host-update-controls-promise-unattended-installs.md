# Always update and Settings promise unattended installs in staged-build mode

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Host update controls can still promise automatic installation while a staged build requires a manual update.

## Details

SPEC.md:178–191 and SPEC_impl.md:4977–4982 require skipping every automatic attempt with a staged payload source, even
when the preference is on. SPEC.md:2889–2898 still describes the sidebar tooltip as an unconditional promise once
enabled. crates/farhelm-desktop/src/bundle.rs:28–31 and lib.rs:537 pass the configured directory to the helm.
crates/farhelm-helm/src/provisioning/payloads.rs:1181–1186 chooses DirectoryPayloads ahead of either default-build case,
and :237–240 makes the scan and admission refuse unattended updates in automatic.rs:115–117 and service.rs:1883–1892.
Manual Update continues through service.rs:1841–1873. The sidebar exposes Always update from the saved preference and
displayed update rows at crates/gui/farhelm-gui/src/session_list/attention.rs:450–462, :553–557. A lower peer protocol
can produce a manual update row even in a development build (hosts/update.rs:305–318. Hosts/status.rs:86–95). The action
sends manual updates and saves the general preference at hosts/status.rs:538–545. Its tooltip at attention.rs:709–716
and both Settings help surfaces at settings/choices.rs:284–287 and :338–341 omit the staged exception. The staged gate
was added in 7ac1d1dba4545499d76626dcd92972b3e6d38b8d. The later name fix 5cd375fc5d3b7a4af4d4acf7c7db28181433482c
leaves that wording unchanged, and the Settings-card change 7271212a1bcdfe35ffc4df6193412e1debcbcfc5 retains the same
tooltip and visible caption. The pushed Preferences field carries only the nullable choice
(crates/farhelm-proto/src/ui_channel.rs:664–674), and its “Only a saved false prevents” doc comment also omits the
staged-source condition. Qualify the spec and both UI surfaces with the staged-source rule. If effective policy is
exposed, show when a saved choice is inactive for this launch. Retain manual actions and the saved choice for later
non-staged launches.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `hostUpdates/f2`, `terminal/l8`.
